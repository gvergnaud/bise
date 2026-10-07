//! The ambient core's projects (bise desktop S1): which hubs it holds and
//! what the window's sidebar shows of each. Pure: the registry rows
//! (`bise_home::projects::list`), each hub's `view.json` and whether its
//! hub runs come in as arguments, so the tests need no HOME. The core
//! (S3b, amb-core) wires the command, the event and the connections;
//! [`read_view`] is the one file read, for it.
//!
//! Holds (decided, desktop plan A): the home hub always; a project's hub
//! while the window shows it, a followed job runs there, or a thread of
//! it is subscribed. A held hub is
//! one `hello` connection (idle-exit counts it); delivering a message to
//! a stopped hub starts it on its own, so routing takes no hold.

use bise_home::projects::Row;
use bise_proto::draft::{ProjectRow, ProjectView};
use bise_proto::rows::Status;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The workspaces whose hub the core holds: `home`, every project the
/// window shows, every project a followed job runs in, every project with
/// a live thread subscription (architect m_8461: one rule, S3b calls it).
pub fn wanted(home: &Path, shown: &[PathBuf], followed: &[PathBuf], subscribed: &[PathBuf]) -> BTreeSet<PathBuf> {
    std::iter::once(home.to_path_buf())
        .chain(shown.iter().cloned())
        .chain(followed.iter().cloned())
        .chain(subscribed.iter().cloned())
        .collect()
}

/// A registry row and what is known of its hub.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub row: Row,
    /// its folder is gone (moved or deleted)
    pub missing: bool,
    /// its hub runs (its pid lives)
    pub running: bool,
    /// its hub's `view.json` (None: never written, or unreadable)
    pub view: Option<ProjectView>,
}

/// The facts of each row, in the registry's order (home first). `view`
/// and `running` take the hub id, `exists` the folder.
pub fn facts(
    rows: &[Row],
    view: impl Fn(&str) -> Option<ProjectView>,
    running: impl Fn(&str) -> bool,
    exists: impl Fn(&Path) -> bool,
) -> Vec<Facts> {
    rows.iter()
        .map(|r| Facts { missing: !exists(&r.path), running: running(&r.id), view: view(&r.id), row: r.clone() })
        .collect()
}

/// What the core reads of a project's checkout (its files, never a git
/// process).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checkout {
    /// its current branch (None: detached, or not a repo)
    pub branch: Option<String>,
    /// a git repo (bise's home is not)
    pub git: bool,
    /// its GitHub remote's web page (`origin`)
    pub web: Option<String>,
}

/// The `url` of `[remote "origin"]` in a git config file.
pub fn origin_url(config: &str) -> Option<String> {
    let mut inside = false;
    for l in config.lines().map(str::trim) {
        if l.starts_with('[') {
            inside = l.replace(' ', "") == "[remote\"origin\"]";
        } else if inside {
            if let Some((k, v)) = l.split_once('=') {
                if k.trim() == "url" {
                    return Some(v.trim().to_string()).filter(|u| !u.is_empty());
                }
            }
        }
    }
    None
}

/// A GitHub remote's web page (`https://github.com/<owner>/<repo>`) from
/// its url in any form git takes (https, `git@github.com:`, ssh://);
/// None for any other host (ambient m_8830: no GitHub, copy the sha).
pub fn github_web(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = ["https://github.com/", "http://github.com/", "ssh://git@github.com/", "git@github.com:", "git://github.com/"]
        .iter()
        .find_map(|p| u.strip_prefix(p))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    (ok(owner) && ok(repo) && parts.next().is_none()).then(|| format!("https://github.com/{owner}/{repo}"))
}

/// The sidebar's row of `f`, at position `order`, with its checkout `c`
/// (the core reads it). Counts from the view: agents (main and archived
/// ones aside), the working ones, his open cards.
pub fn row(f: &Facts, order: u32, c: &Checkout) -> ProjectRow {
    let Checkout { branch, git, web } = c.clone();
    let agents = f.view.as_ref().map(|v| v.agents.iter().filter(|a| !a.main && !a.archived).collect::<Vec<_>>()).unwrap_or_default();
    ProjectRow {
        project: f.row.id.clone(),
        name: f.row.name.clone(),
        path: f.row.path.to_string_lossy().to_string(),
        branch,
        git,
        home: f.row.home,
        order,
        agents: agents.len() as u32,
        working: agents.iter().filter(|a| a.status == Status::Working).count() as u32,
        waits: f.view.as_ref().map_or(0, |v| v.cards.len() as u32),
        running: f.running,
        missing: f.missing,
        cards: f.view.as_ref().map(|v| v.cards.clone()).unwrap_or_default(),
        web,
    }
}

/// The `view.json` in a hub's folder (`Home::hub_dir(id)`), read-only:
/// None when absent, not JSON, or of another shape.
pub fn read_view(hub_dir: &Path) -> Option<ProjectView> {
    let v: ProjectView = serde_json::from_str(&std::fs::read_to_string(hub_dir.join("view.json")).ok()?).ok()?;
    (v.v == 1).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_proto::rows::Agent;

    fn r(name: &str, path: &str, home: bool) -> Row {
        Row { path: path.into(), name: name.into(), id: bise_home::hub_id(Path::new(path)), home, added_ms: 0 }
    }

    fn agent(name: &str, main: bool, status: Status, archived: bool) -> Agent {
        Agent {
            name: name.into(),
            main,
            status,
            archived,
            title: String::new(),
            purpose: String::new(),
            since_ms: 0,
            waits: 0,
            parent: None,
            branch: None,
            worktree: None,
            turn_ms: None,
            report: None,
            queued: vec![],
            dir: None,
            aliases: vec![],
            model: None,
            vision: None,
            effort: None,
            usage: None,
        }
    }

    #[test]
    fn home_is_always_held_and_projects_while_shown_followed_or_subscribed() {
        let home = PathBuf::from("/u/bise");
        let (api, web) = (PathBuf::from("/c/api"), PathBuf::from("/c/web"));
        let one = |p: &PathBuf| vec![p.clone()];
        assert_eq!(wanted(&home, &[], &[], &[]), BTreeSet::from([home.clone()]));
        assert_eq!(wanted(&home, &one(&api), &[], &[]), BTreeSet::from([home.clone(), api.clone()]));
        // the window leaves api, a followed job runs in web: web stays held
        assert_eq!(wanted(&home, &[], &one(&web), &[]), BTreeSet::from([home.clone(), web.clone()]));
        // a thread of api is still subscribed (a side panel): api stays held
        assert_eq!(wanted(&home, &[], &[], &one(&api)), BTreeSet::from([home.clone(), api.clone()]));
        // shown, followed and subscribed: held once
        assert_eq!(wanted(&home, &one(&web), &one(&web), &one(&web)).len(), 2);
    }

    #[test]
    fn facts_and_rows_come_from_the_registry_the_views_and_the_pids() {
        let rows = vec![r("bise", "/u/bise", true), r("api", "/c/api", false), r("old", "/c/old", false)];
        let api_id = rows[1].id.clone();
        let view = ProjectView {
            v: 1,
            project: api_id.clone(),
            written_ms: 10,
            stopped_ms: Some(10),
            last_activity_ms: 9,
            agents: vec![
                agent("main", true, Status::Idle, false),
                agent("p99", false, Status::Working, false),
                agent("docs", false, Status::Done, false),
                agent("gone", false, Status::Done, true),
            ],
            cards: vec![],
            artifacts: vec![],
            artifacts_total: 0,
            scheduled: vec![],
        };
        let v2 = view.clone();
        let f = facts(
            &rows,
            move |id| (id == api_id).then(|| v2.clone()),
            |id| id.starts_with("bise-"),
            |p| p != Path::new("/c/old"),
        );
        assert_eq!(f.iter().map(|x| (x.running, x.missing, x.view.is_some())).collect::<Vec<_>>(), [(true, false, false), (false, false, true), (false, true, false)]);
        let api = Checkout { branch: Some("main".into()), git: true, web: Some("https://github.com/acme/api".into()) };
        let p = row(&f[1], 1, &api);
        assert_eq!((p.name.as_str(), p.home, p.order, p.agents, p.working, p.waits), ("api", false, 1, 2, 1, 0));
        assert_eq!(p.project, rows[1].id);
        assert_eq!(p.web.as_deref(), Some("https://github.com/acme/api"));
        let h = row(&f[0], 0, &Checkout::default());
        assert!(h.home && h.agents == 0 && !h.git && h.web.is_none());
    }

    #[test]
    fn a_github_remote_gives_the_projects_web_page() {
        // amb-web m_8831: the merged sha opens <web>/commit/<sha>
        let web = Some("https://github.com/acme/engine".to_string());
        for u in ["https://github.com/acme/engine", "https://github.com/acme/engine.git", "git@github.com:acme/engine.git", "ssh://git@github.com/acme/engine.git", "https://github.com/acme/engine/"] {
            assert_eq!(github_web(u), web, "{u}");
        }
        for u in ["https://gitlab.com/acme/engine.git", "git@example.com:acme/engine.git", "/srv/git/engine.git", "https://github.com/acme", "https://github.com/acme/engine/tree/x", "https://github.com/acme/en gine"] {
            assert_eq!(github_web(u), None, "{u}");
        }
        let config = "[core]\n\tbare = false\n[remote \"upstream\"]\n\turl = git@github.com:other/engine.git\n[remote \"origin\"]\n\turl = git@github.com:acme/engine.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n";
        assert_eq!(origin_url(config).as_deref(), Some("git@github.com:acme/engine.git"), "origin's, not another remote's");
        assert_eq!(origin_url("[core]\n\tbare = false\n"), None);
    }

    #[test]
    fn a_view_file_is_read_only_when_it_is_ours() {
        let d = std::env::temp_dir().join(format!("amb-projects-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(read_view(&d), None);
        std::fs::write(d.join("view.json"), "{not json").unwrap();
        assert_eq!(read_view(&d), None);
        let v = ProjectView { v: 1, project: "x-1".into(), written_ms: 1, stopped_ms: None, last_activity_ms: 1, agents: vec![], cards: vec![], artifacts: vec![], artifacts_total: 0, scheduled: vec![] };
        std::fs::write(d.join("view.json"), serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(read_view(&d), Some(v.clone()));
        std::fs::write(d.join("view.json"), serde_json::to_string(&ProjectView { v: 2, ..v }).unwrap()).unwrap();
        assert_eq!(read_view(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
