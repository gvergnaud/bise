//! The composer's lists in the window (desktop C/A/B, his bug 1; plan
//! approved by architect m_10724): `/` commands, `$` skills and `@`
//! files, from the TUI's own commands.rs, skills.rs and files.rs (the
//! same crate), so the window lists and ranks exactly what the TUI does,
//! with no copy in TypeScript.
//!
//! - `commands`: COMMANDS (bise_proto's catalog) as data, each with how
//!   it runs (`hub`: his line goes to the hub's parser; `window`: a
//!   screen of the window, the catalog's `client`) and
//!   `runnable`: every hub command runs (his typed line goes as amb-feed's
//!   HubCmd slash, the hub's router parses it; architect m_11011); the
//!   window owns its screens' readiness.
//! - `skills {project}`: skills::index of that project's workspace (the
//!   TUI's fingerprint cache, no watcher).
//! - `files {project, q, rid}`: files::pick on a worker thread (a walk can
//!   take a while), answered through the tick with its rid: `partial`
//!   while the workspace's first walk runs, then once more when it ends.
//!   An answer older than the project's newest rid is dropped, so a late
//!   final never overwrites a newer query's rows.

use super::*;
use crate::commands::{Arg, COMMANDS};
use bise_proto::draft::{PickArg, PickChoice, PickCommand, PickFile, PickFolder, PickSkill, Runs};
use std::collections::HashMap;

/// At most this many `@` rows (the TUI's popup holds as many).
const FILE_ROWS: usize = 50;
/// How long a worker waits for a workspace's first walk before it stops
/// answering (the partial answer stands).
const WALK_WAIT: Duration = Duration::from_secs(20);

fn words(w: &[(&str, &str)]) -> PickArg {
    PickArg::Words { words: w.iter().map(|(value, desc)| PickChoice { value: value.to_string(), desc: desc.to_string() }).collect() }
}

fn arg(a: Arg) -> PickArg {
    match a {
        Arg::Words(w) | Arg::Version(w) | Arg::DevVersion(w) => words(w),
        Arg::ComputerUse => words(&[("off", "turn computer use off"), ("uninstall", "turn it off and remove its setup")]),
        Arg::Keychain => words(&[("on", "move them to the macOS keychain"), ("off", "move them back to files in ~/.bise")]),
        Arg::Task => PickArg::Agent,
        Arg::Archived => PickArg::Archived,
        Arg::Card => PickArg::Card,
        Arg::Branch => PickArg::Branch,
        Arg::Model => PickArg::Model,
        Arg::Effort => PickArg::Effort,
        Arg::Plugin => PickArg::Plugin,
        Arg::Text => PickArg::Text,
        Arg::Note => PickArg::Note,
    }
}

/// The TUI's slash commands as the window lists them.
pub fn commands() -> Vec<PickCommand> {
    COMMANDS
        .iter()
        .map(|c| {
            // the catalog's `client`: a screen of the window's own
            let hub = !c.client;
            PickCommand {
                name: c.name.to_string(),
                desc: c.desc.to_string(),
                args: c.args.iter().map(|a| arg(*a)).collect(),
                runs: if hub { Runs::Hub } else { Runs::Window },
                // his typed line runs through HubCmd slash (the hub's router)
                runnable: hub,
            }
        })
        .collect()
}

/// The `@` rows for `q` in `root`, as the TUI's popup has them: a file's
/// path as it is inserted (outside the workspace, the form the tools
/// read), a folder's as browsed, then the browsed folder's own row.
pub fn files(root: &Path, q: &str, limit: usize) -> (Vec<PickFile>, Option<PickFolder>) {
    let crate::files::Pick { hits, this, outside, locked } = crate::files::pick(root, q, limit);
    let sent = |p: &str| if outside { crate::files::sent_path(p) } else { p.to_string() };
    let items = hits.into_iter().map(|h| PickFile { path: if h.dir { h.path.clone() } else { sent(&h.path) }, dir: h.dir, protected: h.protected }).collect();
    (items, this.map(|p| PickFolder { path: sent(&p), locked }))
}

/// The answers the workers send back, and each project's newest rid.
pub(super) struct Picks {
    tx: Sender<Value>,
    rx: Receiver<Value>,
    newest: HashMap<String, u64>,
}

impl Default for Picks {
    fn default() -> Picks {
        let (tx, rx) = mpsc::channel();
        Picks { tx, rx, newest: HashMap::new() }
    }
}

fn files_ev(project: &str, rid: u64, q: &str, root: &Path, limit: usize, partial: bool) -> Value {
    let (items, folder) = files(root, q, limit);
    let mut v = json!({"ev": "files", "project": project, "rid": rid, "q": q, "items": items});
    if let Some(f) = folder {
        v["folder"] = json!(f);
    }
    if partial {
        v["partial"] = json!(true);
    }
    v
}

impl Core {
    /// The project's folder (bise's home included), from the registry.
    fn pick_root(&self, project: &str) -> Result<PathBuf, String> {
        self.workspace_of(Some(project))
    }

    pub(super) fn pick_commands(&mut self) {
        self.emit(json!({"ev": "commands", "items": commands()}));
    }

    pub(super) fn pick_skills(&mut self, project: String) {
        match self.pick_root(&project) {
            Ok(ws) => {
                let items: Vec<PickSkill> = crate::skills::index(&ws).into_iter().map(|s| PickSkill { name: s.name, desc: s.desc }).collect();
                self.emit(json!({"ev": "skills", "project": project, "items": items}));
            }
            Err(e) => self.emit(json!({"ev": "error", "cmd": "skills", "project": project, "text": e})),
        }
    }

    pub(super) fn pick_files(&mut self, project: String, q: String, rid: u64, limit: Option<u32>) {
        let root = match self.pick_root(&project) {
            Ok(r) => r,
            Err(e) => return self.emit(json!({"ev": "error", "cmd": "files", "project": project, "text": e})),
        };
        let newest = self.picks.newest.entry(project.clone()).or_insert(0);
        *newest = (*newest).max(rid);
        let limit = limit.map_or(FILE_ROWS, |l| (l as usize).clamp(1, 200));
        let tx = self.picks.tx.clone();
        let _ = std::thread::Builder::new().name("ambient-files".into()).spawn(move || {
            let walking = crate::files::first_walk(&root);
            if tx.send(files_ev(&project, rid, &q, &root, limit, walking)).is_err() || !walking {
                return;
            }
            // the first walk runs: once more when it ends
            let t0 = Instant::now();
            while crate::files::first_walk(&root) && t0.elapsed() < WALK_WAIT {
                std::thread::sleep(Duration::from_millis(50));
            }
            if !crate::files::first_walk(&root) {
                let _ = tx.send(files_ev(&project, rid, &q, &root, limit, false));
            }
        });
    }

    /// R14: he took `path` from `project`'s `@` list: it ranks first in
    /// that list's next answers (files::picked_in, the TUI's memory, on
    /// that workspace's own index; an evicted one remembers nothing).
    pub(super) fn file_picked(&mut self, project: String, path: String) {
        match self.pick_root(&project) {
            Ok(root) => crate::files::picked_in(&root, path.trim()),
            Err(e) => self.emit(json!({"ev": "error", "cmd": "file_picked", "project": project, "text": e})),
        }
    }

    /// The workers' answers out, an older rid's dropped.
    pub(super) fn tick_picks(&mut self) {
        while let Ok(v) = self.picks.rx.try_recv() {
            let project = v["project"].as_str().unwrap_or("");
            let rid = v["rid"].as_u64().unwrap_or(0);
            if self.picks.newest.get(project).is_some_and(|n| rid < *n) {
                continue;
            }
            self.emit(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Law: every TUI command once, in its order, with its args and how
    /// it runs; the tables name only real commands.
    #[test]
    fn every_command_is_listed_once_with_how_it_runs() {
        let list = commands();
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, COMMANDS.iter().map(|c| c.name).collect::<Vec<_>>());
        for (c, t) in list.iter().zip(COMMANDS) {
            assert_eq!((c.desc.as_str(), c.args.len()), (t.desc, t.args.len()), "{}", c.name);
        }
        for (c, t) in list.iter().zip(COMMANDS) {
            assert_eq!(c.runs == Runs::Hub, !t.client, "{}: runs as the catalog says", c.name);
            assert_eq!(c.runnable, c.runs == Runs::Hub, "{}: every hub command runs (slash), no window one is the hub's", c.name);
        }
        let new = list.iter().find(|c| c.name == "/new").unwrap();
        assert_eq!((new.runs, new.runnable), (Runs::Hub, true));
        assert_eq!(new.args[1], PickArg::Text);
        assert!(matches!(&new.args[0], PickArg::Words { words } if words[0].value == "-w"));
        let inbox = list.iter().find(|c| c.name == "/inbox").unwrap();
        assert_eq!((inbox.runs, inbox.runnable), (Runs::Window, false), "the window owns its screens");
        let compact = list.iter().find(|c| c.name == "/compact").unwrap();
        assert_eq!((compact.runs, compact.runnable), (Runs::Hub, true), "his line goes as slash");
    }

    fn repo() -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("amb-picks-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        for f in ["src/main.rs", "src/math/add.rs", "README.md", "docs/guide.md"] {
            let p = d.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        d
    }

    fn walked(root: &Path) {
        crate::files::start(root.to_path_buf());
        let t0 = Instant::now();
        while crate::files::first_walk(root) {
            assert!(t0.elapsed() < Duration::from_secs(5), "the walk never ended");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Law: the window's rows are files::pick's (the TUI popup's): a
    /// query's best file first, a folder query lists its children and
    /// then its own row.
    #[test]
    fn the_windows_at_rows_are_the_tuis() {
        let root = repo();
        walked(&root);
        let (items, folder) = files(&root, "main", FILE_ROWS);
        let tui = crate::files::pick(&root, "main", FILE_ROWS);
        assert_eq!(items.iter().map(|f| (f.path.clone(), f.dir)).collect::<Vec<_>>(), tui.hits.iter().map(|h| (h.path.clone(), h.dir)).collect::<Vec<_>>());
        assert_eq!(items[0].path, "src/main.rs");
        assert!(folder.is_none());
        let (items, folder) = files(&root, "src/", FILE_ROWS);
        let paths: Vec<&str> = items.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/main.rs") && paths.contains(&"src/math"), "{paths:?}");
        assert_eq!(folder, Some(PickFolder { path: "src".into(), locked: false }));
        let _ = std::fs::remove_dir_all(&root);
    }
}
