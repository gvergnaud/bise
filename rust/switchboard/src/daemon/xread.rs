//! Stream C (bise desktop S2 step 3): bise's home hub reads the other
//! projects' threads on disk: `sb history "<words>" --project <p>|--all`,
//! `sb show <p>/<agent>#<pos>` and `sb inspect <p>/<agent>`.
//!
//! One code path whether that project's hub runs or not (architect
//! m_8524): its threads are `<hubs>/<id>/agents/<dir>/transcript.log`
//! (append-only), its agents `<hubs>/<id>/view.json` (their dir and
//! aliases since S2 step 3; a view written before: the dir is the name).
//! No other hub is asked, pinged or started.
//!
//! Only the home hub's agents read across projects; a project hub refuses
//! (a project's agent reading every other project is what an injection
//! would want). The caller's `<p>` resolves only through the registry
//! (`projects::target`), and every folder name joined under
//! `<hubs>/<id>/agents/` is one plain path component ([`plain`]).
//!
//! The other projects' threads are one more search index (the history
//! worker's `others`, history_work.rs: these reads run off the hub loop),
//! keyed `<project>/<dir>` (`Index::refresh_as`), so `--all` ranks home and
//! projects with one query and one scoring. It refreshes only when a query
//! runs, never on a tick (core-idle-cpu's burn).

use super::history::{inspect_page, respond, Look};
use super::history_work::{HistoryJob, XCtx};
use super::Shell;
use crate::search::{Index, Who};
use crate::util::now_ms;
use bise_proto::sb::{History, Inspect};
use serde_json::{json, Value};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// The prefix of bise's own threads under `--all`.
pub(super) const HOME: &str = "bise";

const REFUSED: &str = "only bise reads other projects: ask bise";

/// One plain path component: not empty, not `.` or `..`, no separator.
pub(super) fn plain(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\', '\0'])
}

/// `<p>/<agent>`: the project and the agent, the agent part checked.
pub(super) fn split_ref(s: &str) -> Result<(String, String), String> {
    let (p, a) = s.split_once('/').ok_or_else(|| format!("{s}: not <project>/<agent>"))?;
    let a = a.trim_start_matches('@');
    if !plain(p) || !plain(a) {
        return Err(format!("{s}: <project>/<agent>, each one plain name"));
    }
    Ok((p.to_string(), a.to_string()))
}

/// Whether `sb history` request `h` reads another project.
pub(super) fn wanted(h: &History) -> bool {
    h.project.as_deref().is_some_and(|p| !p.is_empty()) || h.all
}

/// Whether `sb show` or `sb inspect` of `agent` reads another project.
pub(super) fn elsewhere(agent: &str) -> bool {
    agent.contains('/')
}

/// A registered project as a reader sees it.
struct Proj {
    name: String,
    state: PathBuf,
}

/// The project `p` (a name or a hub id) through the registry, never this
/// hub itself.
fn project(home: &bise_home::Home, p: &str, own: &str) -> Result<Proj, String> {
    let rows = bise_home::projects::list(home, &bise_home::Home::from_env().bise_root());
    let id = bise_home::projects::target(&rows, p, own)?;
    let name = rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.clone());
    if !plain(&name) {
        return Err(format!("{name}: not a plain project name"));
    }
    Ok(Proj { name, state: home.hub_dir(&id) })
}

/// Every registered project but this hub.
fn projects(home: &bise_home::Home, own: &str) -> Vec<Proj> {
    let rows = bise_home::projects::list(home, &bise_home::Home::from_env().bise_root());
    rows.iter()
        .filter(|r| r.id != own && plain(&r.name))
        .map(|r| Proj { name: r.name.clone(), state: home.hub_dir(&r.id) })
        .collect()
}

/// A project's agents from its view.json, keyed `<p>/...` (none without
/// a view: its threads show by their folders).
pub(super) fn who_of(name: &str, state: &Path) -> Vec<Who> {
    let Some(v) = crate::view::read(state) else { return Vec::new() };
    v.agents
        .iter()
        .filter_map(|a| {
            // a view written before `dir`: its name
            let dir = if a.dir.is_empty() { a.name.clone() } else { a.dir.clone() };
            plain(&dir).then(|| Who {
                name: format!("{name}/{}", a.name),
                dir: format!("{name}/{dir}"),
                aliases: a.aliases.iter().map(|x| format!("{name}/{x}")).collect(),
                archived: a.archived,
            })
        })
        .collect()
}

/// `--agent` names inside `<p>`: a bare name is that project's; `scope`
/// the project the search keeps to (none: `--all`).
fn qualify(h: &History, p: &str, scope: Option<&str>) -> Look {
    let mut look = Look::history(h);
    if let Look::Search(q) = &mut look {
        q.agents = q.agents.iter().map(|x| if x.contains('/') { x.to_string() } else { format!("{p}/{x}") }).collect();
        q.scope = scope.map(String::from);
    }
    look
}

/// `p`'s threads read into `index` (bise's own under `bise`), and its
/// agents (history worker).
fn refresh_one(index: &mut Index, x: &XCtx, p: &str) -> Result<(String, Vec<Who>), String> {
    if p == HOME {
        index.refresh_as(&x.agents, HOME);
        return Ok((HOME.to_string(), x.who.clone()));
    }
    let pr = project(&x.home, p, &crate::paths::workspace_id(&x.workspace))?;
    index.refresh_as(&pr.state.join("agents"), &pr.name);
    let who = who_of(&pr.name, &pr.state);
    Ok((pr.name, who))
}

/// `sb show <p>/<agent>#<pos>` on the history worker.
pub(super) fn show(index: &mut Index, x: &XCtx, agent: &str, pos: u64, context: Option<u64>) -> Value {
    let t0 = std::time::Instant::now();
    if !super::xhub::is_home(&x.workspace) {
        return json!({"ok": false, "error": REFUSED});
    }
    let res = split_ref(agent).and_then(|(p, a)| {
        let (name, who) = refresh_one(index, x, &p)?;
        Ok(respond(index, &who, &Look::show(&format!("{name}/{a}"), pos, context), now_ms(), t0))
    });
    res.unwrap_or_else(|e| json!({"ok": false, "error": e}))
}

/// `sb history --project|--all` on the history worker.
pub(super) fn search(index: &mut Index, x: &XCtx, h: &History) -> Value {
    let t0 = std::time::Instant::now();
    if !super::xhub::is_home(&x.workspace) {
        return json!({"ok": false, "error": REFUSED});
    }
    let res = if h.all {
        let ps = projects(&x.home, &crate::paths::workspace_id(&x.workspace));
        let mut keep = vec![HOME.to_string()];
        let mut who = x.who.clone();
        index.refresh_as(&x.agents, HOME);
        for pr in &ps {
            index.refresh_as(&pr.state.join("agents"), &pr.name);
            who.extend(who_of(&pr.name, &pr.state));
            keep.push(pr.name.clone());
        }
        index.retain_prefixes(&keep);
        Ok(respond(index, &who, &qualify(h, HOME, None), now_ms(), t0))
    } else {
        refresh_one(index, x, h.project.as_deref().unwrap_or_default()).map(|(name, who)| respond(index, &who, &qualify(h, &name, Some(&name)), now_ms(), t0))
    };
    res.unwrap_or_else(|e| json!({"ok": false, "error": e}))
}

impl Shell {
    /// What a read of other projects needs of this hub, from memory: its
    /// agents keyed `bise/...` (the worker reads the rest).
    fn xctx(&self) -> XCtx {
        let who = self
            .hub
            .st
            .agents
            .values()
            .map(|a| Who {
                name: format!("{HOME}/{}", a.name),
                dir: format!("{HOME}/{}", a.dir),
                aliases: a.aliases.iter().map(|x| format!("{HOME}/{x}")).collect(),
                archived: a.status() == crate::model::Status::Archived,
            })
            .collect();
        XCtx { home: self.opts.home.clone(), workspace: self.opts.paths.workspace.clone(), agents: self.opts.paths.state.join("agents"), who }
    }

    /// `sb show <p>/<agent>#<pos>`: the history worker's job.
    pub(super) fn xshow(&self, stream: UnixStream, from: &str, agent: String, pos: u64, context: Option<u64>) {
        self.history.ask(stream, from, HistoryJob::XShow { x: self.xctx(), agent, pos, context });
    }

    /// `sb history --project|--all`: the history worker's job.
    pub(super) fn xsearch(&self, stream: UnixStream, from: &str, h: History) {
        self.history.ask(stream, from, HistoryJob::XSearch { x: self.xctx(), h });
    }

    /// `sb inspect <p>/<agent>`: a page of that project's thread.
    pub(super) fn xinspect(&mut self, q: &Inspect) -> Value {
        if !super::xhub::is_home(&self.opts.paths.workspace) {
            return json!({"ok": false, "error": REFUSED});
        }
        if q.origin {
            return json!({"ok": false, "error": "--origin reads your own thread: sb inspect <agent> --origin"});
        }
        let res = split_ref(&q.agent).and_then(|(p, a)| {
            let pr = project(&self.opts.home, &p, &self.project())?;
            let who = who_of(&pr.name, &pr.state);
            let full = format!("{}/{a}", pr.name);
            let dir = who
                .iter()
                .find(|w| w.name == full)
                .or_else(|| who.iter().find(|w| w.aliases.contains(&full)))
                .map_or(a.clone(), |w| w.dir[pr.name.len() + 1..].to_string());
            let path = pr.state.join("agents").join(&dir).join("transcript.log");
            if !plain(&dir) || !path.is_file() {
                return Err(format!("no agent named {a} in {}", pr.name));
            }
            Ok(inspect_page(&full, &path, q, now_ms()))
        });
        res.unwrap_or_else(|e| json!({"ok": false, "error": e}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // the caller's <p>/<agent> never leaves <hubs>/<id>/agents/
    #[test]
    fn a_ref_is_two_plain_names() {
        assert_eq!(split_ref("shop/perf"), Ok(("shop".into(), "perf".into())));
        assert_eq!(split_ref("shop/@perf"), Ok(("shop".into(), "perf".into())));
        for bad in ["shop/..", "shop/.", "shop/", "/perf", "shop/a/b", "../x/perf", "shop/a\\b", "perf"] {
            assert!(split_ref(bad).is_err(), "{bad}");
        }
        assert!(!plain("..") && !plain("") && !plain("a/b") && plain("perf-2"));
    }

    // view.json's agents become the reader's Who: dir and aliases when
    // the view has them, else the name; an unsafe dir is never used
    #[test]
    fn a_projects_agents_come_from_its_view() {
        let st = std::env::temp_dir().join(format!("sb-xread-{}", std::process::id()));
        std::fs::create_dir_all(&st).unwrap();
        let row = |name: &str, extra: Value| {
            let mut a = json!({"name": name, "main": false, "status": "idle", "archived": false, "title": "", "purpose": "", "since_ms": 1, "waits": 0});
            a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            a
        };
        let view = json!({"v": 1, "project": "shop-1", "written_ms": 1, "last_activity_ms": 1, "cards": [], "agents": [
            row("perf", json!({"dir": "perf-2", "aliases": ["speed"]})),
            row("old", json!({"archived": true})),
            row("evil", json!({"dir": "../../x"})),
        ]});
        std::fs::write(st.join("view.json"), view.to_string()).unwrap();
        let who = who_of("shop", &st);
        let _ = std::fs::remove_dir_all(&st);
        let got: Vec<(String, String, Vec<String>, bool)> = who.into_iter().map(|w| (w.name, w.dir, w.aliases, w.archived)).collect();
        assert_eq!(got, vec![
            ("shop/perf".into(), "shop/perf-2".into(), vec!["shop/speed".into()], false),
            ("shop/old".into(), "shop/old".into(), vec![], true),
        ]);
    }

    #[test]
    fn only_a_project_or_a_ref_reads_across() {
        let h = |v: serde_json::Value| serde_json::from_value::<History>(v).unwrap();
        assert!(wanted(&h(json!({"project": "shop"}))));
        assert!(wanted(&h(json!({"all": true}))));
        assert!(!wanted(&h(json!({"query": "x"}))));
        assert!(!wanted(&h(json!({"project": ""}))));
        assert!(elsewhere("shop/perf"));
        assert!(!elsewhere("perf"));
    }
}
