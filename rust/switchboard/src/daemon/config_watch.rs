//! The config files the hub's loop reads, each read once per change
//! (c4-loop-never-waits): the user's config.toml with the catalog's
//! cache files (`bise_catalog::Setup`), the repo's
//! `.switchboard/config.toml` (its `[flow]` and `[pr] trusted_bots`) and
//! each agent's choice.toml (`/model`, `/reasoning`). Before, each state
//! push read the repo config twice and every agent's choice file (his
//! hub: 1,275 agents, archived ones included): the push's median was
//! 1.7 s.
//!
//! A look is a stat (`live_fetch::stamp` for the setup); a file is read
//! again only when its mtime moved. An archived agent's choice file is
//! read once: no REPL writes it any more (a restored agent is live again
//! and looked at). The hub's own writes need no word: they move the mtime.
//!
//! Owns no file: it only reads. Not here: the auth files (`key_ready`).

use crate::flow::FlowConfig;
use bise_catalog::{Choice, Setup};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

type Stamp = Option<SystemTime>;

fn mtime(p: &Path) -> Stamp {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

pub(super) struct ConfigWatch {
    /// the user's config.toml and the catalog's cache folder
    home_config: PathBuf,
    cache_dir: PathBuf,
    setup: Option<(Stamp, Setup)>,
    /// the repo's `.switchboard/config.toml`
    repo_config: PathBuf,
    repo: Option<(Stamp, Repo)>,
    /// each agent's choice file, by its path
    choices: HashMap<PathBuf, (Stamp, Choice)>,
}

/// What the hub takes from the repo's config.toml.
#[derive(Clone, Default)]
pub(super) struct Repo {
    pub(super) flow: FlowConfig,
    /// pr-news: whose bots' comments reach the agents
    pub(super) pr_bots: Vec<String>,
}

impl Repo {
    fn of(text: &str) -> Repo {
        Repo { flow: FlowConfig::parse(text), pr_bots: crate::forge::news::trusted_bots(text) }
    }
}

impl ConfigWatch {
    pub(super) fn new(home: &bise_home::Home, paths: &crate::paths::Paths) -> ConfigWatch {
        ConfigWatch {
            home_config: home.config_file(),
            cache_dir: home.cache_dir(),
            setup: None,
            repo_config: paths.config(),
            repo: None,
            choices: HashMap::new(),
        }
    }

    /// The catalog merged with config.toml, read again when it or the
    /// catalog's cache files changed.
    pub(super) fn setup(&mut self) -> &Setup {
        let now = bise_catalog::live_fetch::stamp(&self.home_config, &self.cache_dir);
        if self.setup.as_ref().is_none_or(|(t, _)| *t != now) {
            self.setup = Some((now, Setup::load(&self.home_config)));
        }
        &self.setup.as_ref().expect("just set").1
    }

    /// The repo's config: its flow and its trusted bots.
    pub(super) fn repo(&mut self) -> &Repo {
        let now = mtime(&self.repo_config);
        if self.repo.as_ref().is_none_or(|(t, _)| *t != now) {
            let text = std::fs::read_to_string(&self.repo_config).unwrap_or_default();
            self.repo = Some((now, Repo::of(&text)));
        }
        &self.repo.as_ref().expect("just set").1
    }

    /// An agent's choice file; `live`: a REPL may write it, so it is
    /// looked at (an archived agent's is read once).
    pub(super) fn choice(&mut self, path: &Path, live: bool) -> Choice {
        if !live {
            if let Some((_, c)) = self.choices.get(path) {
                return c.clone();
            }
        }
        let now = mtime(path);
        match self.choices.get(path) {
            Some((t, c)) if *t == now => c.clone(),
            _ => {
                let c = Choice::read(path);
                self.choices.insert(path.to_path_buf(), (now, c.clone()));
                c
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-config-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn watch(d: &Path) -> ConfigWatch {
        ConfigWatch {
            home_config: d.join("config.toml"),
            cache_dir: d.join("cache"),
            setup: None,
            repo_config: d.join("repo.toml"),
            repo: None,
            choices: HashMap::new(),
        }
    }

    /// A file is read once per change: the same mtime keeps what was
    /// read even when the bytes moved (the proof that it was not read).
    fn same_mtime(p: &Path, text: &str) {
        let t = std::fs::metadata(p).unwrap().modified().unwrap();
        std::fs::write(p, text).unwrap();
        std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    }

    #[test]
    fn the_repo_config_is_read_again_only_when_it_changes() {
        let d = tmp("repo");
        let p = d.join("repo.toml");
        std::fs::write(&p, "[flow]\nmode = \"trunk\"\n").unwrap();
        let mut w = watch(&d);
        assert_eq!(w.repo().flow.mode, Some(crate::flow::FlowMode::Trunk));
        same_mtime(&p, "[flow]\nmode = \"pr\"\n");
        assert_eq!(w.repo().flow.mode, Some(crate::flow::FlowMode::Trunk), "same mtime: not read");
        let later = std::fs::metadata(&p).unwrap().modified().unwrap() + std::time::Duration::from_secs(2);
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(later).unwrap();
        assert_eq!(w.repo().flow.mode, Some(crate::flow::FlowMode::Pr), "a change: read again");
    }

    #[test]
    fn an_archived_agents_choice_is_read_once_a_live_ones_on_change() {
        let d = tmp("choice");
        let p = d.join("choice.toml");
        std::fs::write(&p, "model = \"a/one\"\n").unwrap();
        let mut w = watch(&d);
        assert_eq!(w.choice(&p, true).model, "a/one");
        std::fs::write(&p, "model = \"a/two\"\n").unwrap();
        let later = std::fs::metadata(&p).unwrap().modified().unwrap() + std::time::Duration::from_secs(2);
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(later).unwrap();
        assert_eq!(w.choice(&p, false).model, "a/one", "archived: the one read");
        assert_eq!(w.choice(&p, true).model, "a/two", "live: its change");
    }
}
