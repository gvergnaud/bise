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
//! The other projects' threads are one more search index (`Shell.xsearch`),
//! keyed `<project>/<dir>` (`Index::refresh_as`), so `--all` ranks home and
//! projects with one query and one scoring. It refreshes only when a query
//! runs, never on a tick (core-idle-cpu's burn).

use super::history::{inspect_page, respond};
use super::Shell;
use crate::search::Who;
use crate::util::now_ms;
use serde_json::{json, Value};
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

/// Whether request `v` of `cmd` reads another project.
pub(super) fn wanted(cmd: &str, v: &Value) -> bool {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("");
    match cmd {
        "history" => !s("project").is_empty() || v.get("all") == Some(&json!(true)),
        "show" | "inspect" => s("agent").contains('/'),
        _ => false,
    }
}

/// A registered project as a reader sees it.
struct Proj {
    name: String,
    state: PathBuf,
}

/// The project `p` (a name or a hub id) through the registry, never this
/// hub itself.
fn project(p: &str, own: &str) -> Result<Proj, String> {
    let home = bise_home::Home::from_env();
    let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
    let id = bise_home::projects::target(&rows, p, own)?;
    let name = rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.clone());
    if !plain(&name) {
        return Err(format!("{name}: not a plain project name"));
    }
    Ok(Proj { name, state: home.hub_dir(&id) })
}

/// Every registered project but this hub.
fn projects(own: &str) -> Vec<Proj> {
    let home = bise_home::Home::from_env();
    let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
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

/// `--agent` names inside `<p>`: a bare name is that project's.
fn qualify(v: &Value, p: &str) -> Value {
    let mut v = v.clone();
    let agents: Vec<String> = v
        .get("agents")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(|x| if x.contains('/') { x.to_string() } else { format!("{p}/{x}") }).collect())
        .unwrap_or_default();
    v["agents"] = json!(agents);
    v
}

impl Shell {
    fn home_who(&self) -> Vec<Who> {
        self.hub
            .st
            .agents
            .values()
            .map(|a| Who {
                name: format!("{HOME}/{}", a.name),
                dir: format!("{HOME}/{}", a.dir),
                aliases: a.aliases.iter().map(|x| format!("{HOME}/{x}")).collect(),
                archived: a.status() == crate::model::Status::Archived,
            })
            .collect()
    }

    /// `p`'s threads read (bise's own under `bise`) and its agents.
    fn refresh_one(&mut self, p: &str) -> Result<(String, Vec<Who>), String> {
        if p == HOME {
            let dir = self.opts.paths.state.join("agents");
            self.xsearch.refresh_as(&dir, HOME);
            return Ok((HOME.to_string(), self.home_who()));
        }
        let pr = project(p, &self.project())?;
        self.xsearch.refresh_as(&pr.state.join("agents"), &pr.name);
        let who = who_of(&pr.name, &pr.state);
        Ok((pr.name, who))
    }

    /// `sb history --project|--all` and `sb show <p>/<agent>#<pos>`.
    pub(super) fn xsearch(&mut self, cmd: &str, v: &Value) -> Value {
        let t0 = std::time::Instant::now();
        if !super::xhub::is_home(&self.opts.paths.workspace) {
            return json!({"ok": false, "error": REFUSED});
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let all = v.get("all") == Some(&json!(true));
        let res = if cmd == "show" {
            split_ref(&s("agent")).and_then(|(p, a)| {
                let (name, who) = self.refresh_one(&p)?;
                let mut v = v.clone();
                v["agent"] = json!(format!("{name}/{a}"));
                Ok(respond(&self.xsearch, &who, cmd, &v, now_ms(), t0))
            })
        } else if all {
            let ps = projects(&self.project());
            let mut keep = vec![HOME.to_string()];
            let mut who = self.home_who();
            self.xsearch.refresh_as(&self.opts.paths.state.join("agents"), HOME);
            for pr in &ps {
                self.xsearch.refresh_as(&pr.state.join("agents"), &pr.name);
                who.extend(who_of(&pr.name, &pr.state));
                keep.push(pr.name.clone());
            }
            self.xsearch.retain_prefixes(&keep);
            Ok(respond(&self.xsearch, &who, cmd, &qualify(v, HOME), now_ms(), t0))
        } else {
            self.refresh_one(&s("project")).map(|(name, who)| {
                let mut v = qualify(v, &name);
                v["scope"] = json!(name);
                respond(&self.xsearch, &who, cmd, &v, now_ms(), t0)
            })
        };
        res.unwrap_or_else(|e| json!({"ok": false, "error": e}))
    }

    /// `sb inspect <p>/<agent>`: a page of that project's thread.
    pub(super) fn xinspect(&mut self, v: &Value) -> Value {
        if !super::xhub::is_home(&self.opts.paths.workspace) {
            return json!({"ok": false, "error": REFUSED});
        }
        if v.get("origin") == Some(&json!(true)) {
            return json!({"ok": false, "error": "--origin reads your own thread: sb inspect <agent> --origin"});
        }
        let target = v.get("agent").and_then(Value::as_str).unwrap_or("");
        let res = split_ref(target).and_then(|(p, a)| {
            let pr = project(&p, &self.project())?;
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
            Ok(inspect_page(&full, &path, v, now_ms()))
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
        assert!(wanted("history", &json!({"project": "shop"})));
        assert!(wanted("history", &json!({"all": true})));
        assert!(!wanted("history", &json!({"query": "x"})));
        assert!(wanted("show", &json!({"agent": "shop/perf"})));
        assert!(!wanted("inspect", &json!({"agent": "perf"})));
    }
}
