//! Artifacts and diffs on the hub's side (docs/artifacts.md): the
//! `artifacts` event and ops, `sb artifact`, the thread lines, the
//! `diff` and `branches` ops (computed off the hub's loop), each agent's
//! `changes` in the state, and the `landed` line after a land.

use super::*;
use crate::artifacts::{self, Add, Added, Store};
use crate::diff;

/// How often an agent's `changes` may be computed while it edits.
const CHANGES_EVERY: Duration = Duration::from_secs(5);

/// The tool calls that change files (the `changes` follow them).
fn edits(line: &str) -> bool {
    match crate::wire::parse(line) {
        crate::wire::Wire::Tool { name, .. } => {
            matches!(name.as_str(), "edit" | "write_file" | "apply_patch" | "bash")
        }
        _ => false,
    }
}

/// The artifacts and diffs state of the shell.
#[derive(Default)]
pub(super) struct Art {
    /// The last `artifacts` event sent (rows and new): an idle that finds
    /// the same sends nothing.
    last: String,
    /// Each agent's `changes` ({files, add, del} or null), by name.
    pub(super) changes: BTreeMap<String, Value>,
    /// When each agent's changes were last asked, and those being
    /// computed now.
    asked: BTreeMap<String, std::time::Instant>,
    busy: BTreeSet<String>,
    /// An edit came while busy or too soon: ask again at the next idle.
    stale: BTreeSet<String>,
}

/// Where an agent works and what its changes are measured against.
#[derive(Clone, Debug)]
enum Where {
    /// Its own checkout (a worktree), against this base.
    Checkout { dir: PathBuf, base: Option<String> },
    /// The shared folder: its own files only.
    Shared { dir: PathBuf, files: Vec<String> },
}

impl Shell {
    fn art_store(&self) -> Store {
        Store::new(&self.opts.paths.state)
    }

    /// An agent's name now (its old names are aliases) and whether it is
    /// archived.
    fn art_who(&self) -> impl Fn(&str) -> Option<(String, bool)> + '_ {
        move |name: &str| {
            self.hub
                .st
                .agents
                .values()
                .find(|a| a.name == name || a.dir == name || a.aliases.iter().any(|x| x == name))
                .map(|a| (a.name.clone(), a.status() == crate::model::Status::Archived))
        }
    }

    /// `{"ev":"artifacts","rows":[…],"new":N}`.
    pub(super) fn artifacts_ev(&self) -> Value {
        let store = self.art_store();
        let who = self.art_who();
        json!({
            "ev": "artifacts",
            "rows": store.rows(&who, &self.hub.workspace),
            "new": store.new_count(now_ms()),
        })
    }

    /// Send the list when it changed since the last one sent (`force`:
    /// always).
    pub(super) fn artifacts_refresh(&mut self, force: bool) {
        let ev = self.artifacts_ev();
        let s = ev.to_string();
        if force || s != self.art.last {
            self.art.last = s;
            self.broadcast(&ev);
        }
    }

    /// The TUI's `artifacts` op: the list again, `seen`, `add`.
    pub(super) fn artifacts_op(&mut self, id: ClientId, v: &Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match s("do").as_str() {
            "seen" => {
                let _ = self.art_store().set_seen(now_ms());
                self.artifacts_refresh(true);
            }
            "add" => {
                let agent = match s("agent") {
                    a if a.is_empty() => MAIN.to_string(),
                    a => a,
                };
                let cwd = self
                    .hub
                    .st
                    .agents
                    .get(&agent)
                    .map(|a| self.art_where(a))
                    .map(|w| match w {
                        Where::Checkout { dir, .. } | Where::Shared { dir, .. } => dir.to_string_lossy().to_string(),
                    })
                    .unwrap_or_else(|| self.hub.workspace.clone());
                let add = Add {
                    target: s("target"),
                    title: Some(s("title")).filter(|t| !t.trim().is_empty()),
                    kind: None,
                    agent,
                    by: "you".into(),
                    cwd,
                };
                let ev = match self.art_store().add(&add, now_ms()) {
                    Ok(a) => {
                        let text = format!("↗ added: {}", a.meta.title);
                        if a.new_version {
                            self.art_lines(&a);
                        }
                        json!({"ev": "notice", "text": text})
                    }
                    Err(e) => json!({"ev": "warn", "text": format!("▲ {}", e)}),
                };
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &ev);
                }
                self.artifacts_refresh(true);
            }
            _ => {
                let ev = self.artifacts_ev();
                self.art.last = ev.to_string();
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &ev);
                }
            }
        }
    }

    /// `sb artifact add|list` (an agent's request).
    pub(super) fn artifact_cmd(&mut self, from: &str, v: &Value) -> Value {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let store = self.art_store();
        match s("do").as_str() {
            "add" => {
                let add = Add {
                    target: s("target"),
                    title: v.get("title").and_then(|x| x.as_str()).map(String::from),
                    kind: v.get("kind").and_then(|x| x.as_str()).map(String::from),
                    agent: from.to_string(),
                    by: from.to_string(),
                    cwd: s("cwd"),
                };
                match store.add(&add, now_ms()) {
                    Ok(a) => {
                        if a.new_version {
                            self.art_lines(&a);
                        }
                        self.artifacts_refresh(false);
                        json!({"ok": true, "text": artifacts::added_text(&a), "id": a.meta.id})
                    }
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
            "list" => {
                let agent = Some(s("agent")).filter(|a| !a.is_empty());
                json!({"ok": true, "text": store.list_text(&s("words"), agent.as_deref(), now_ms())})
            }
            _ => json!({"ok": false, "error": "usage: sb artifact add <path or link> | sb artifact list"}),
        }
    }

    /// The ↗ line of a new artifact or version: in the maker's thread and
    /// in main's.
    fn art_lines(&mut self, a: &Added) {
        let line = format!("sb artifact : {}", wire_escape(&artifacts::thread_line(&a.meta)));
        let maker = self.art_who()(&a.meta.agent).map(|(n, _)| n);
        if let Some(m) = &maker {
            if m != MAIN {
                self.feed(m, &line);
            }
        }
        self.feed(MAIN, &line);
    }

    /// An agent's turn ended: the list (a page may have been published)
    /// and its changes.
    pub(super) fn art_on_line(&mut self, name: &str, line: &str) {
        if line == "--- idle" {
            self.artifacts_refresh(false);
            self.changes_ask(name, true);
        } else if edits(line) {
            self.changes_ask(name, false);
        }
    }

    fn art_where(&self, a: &crate::model::Agent) -> Where {
        if let Some(p) = &a.place {
            return Where::Checkout { dir: PathBuf::from(p), base: None };
        }
        match a.ws.mode {
            crate::model::Mode::Worktree if !a.ws.dropped => Where::Checkout {
                dir: PathBuf::from(&a.ws.path),
                base: a.ws.feature().map(String::from),
            },
            _ => Where::Shared {
                dir: PathBuf::from(&a.ws.path),
                files: a.files.iter().cloned().collect(),
            },
        }
    }

    /// Compute an agent's `changes` off the loop: at its idle (`now`), or
    /// after an edit at most every 5 s (else at its next idle).
    fn changes_ask(&mut self, name: &str, now: bool) {
        let Some(a) = self.hub.st.agents.get(name) else { return };
        if a.is_main {
            return;
        }
        let soon = self.art.asked.get(name).is_some_and(|t| t.elapsed() < CHANGES_EVERY);
        if self.art.busy.contains(name) || (!now && soon) {
            self.art.stale.insert(name.to_string());
            return;
        }
        self.art.stale.remove(name);
        self.art.busy.insert(name.to_string());
        self.art.asked.insert(name.to_string(), std::time::Instant::now());
        let place = self.art_where(a);
        let (tx, name) = (self.tx.clone(), name.to_string());
        std::thread::spawn(move || {
            let files = match &place {
                Where::Checkout { dir, base } => {
                    let base = base.clone().unwrap_or_else(|| diff::trunk(dir));
                    diff::checkout(dir, &base).map(|(f, _, _)| f)
                }
                Where::Shared { dir, files } => diff::own_files(dir, files),
            };
            let v = match files {
                Ok(f) if !f.is_empty() => {
                    let (files, add, del) = diff::stat(&f);
                    json!({"files": files, "add": add, "del": del})
                }
                _ => Value::Null,
            };
            let _ = tx.send(Msg::Changes { name, v });
        });
    }

    /// An agent's changes are computed: the state again when they moved.
    pub(super) fn on_changes(&mut self, name: String, v: Value) {
        self.art.busy.remove(&name);
        let moved = self.art.changes.get(&name) != Some(&v);
        self.art.changes.insert(name.clone(), v);
        if moved {
            let snap = self.snapshot();
            self.broadcast(&snap);
        }
        if self.art.stale.contains(&name) {
            self.changes_ask(&name, false);
        }
    }

    /// The `diff` op: an agent's changes, a branch, a range or a PR,
    /// answered with a `diff` event to that client.
    pub(super) fn diff_op(&mut self, id: ClientId, v: &Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let req = v.get("req").cloned().unwrap_or(Value::Null);
        let shared = PathBuf::from(&self.hub.workspace);
        enum Ask {
            Agent(String, Where, Option<String>),
            Branch(String),
            Range(String),
            Pr(u64),
            Bad(String),
        }
        let ask = if !s("agent").is_empty() {
            match self.hub.st.agents.get(&s("agent")).or_else(|| {
                let n = self.art_who()(&s("agent")).map(|(n, _)| n)?;
                self.hub.st.agents.get(&n)
            }) {
                Some(a) => {
                    let branch = a.ws.branch.clone().or_else(|| a.place_branch.clone());
                    Ask::Agent(a.name.clone(), self.art_where(a), branch)
                }
                None => Ask::Bad(format!("no agent {}", s("agent"))),
            }
        } else if !s("branch").is_empty() {
            Ask::Branch(s("branch"))
        } else if !s("range").is_empty() {
            Ask::Range(s("range"))
        } else if let Some(n) = v.get("pr").and_then(|x| x.as_u64()) {
            Ask::Pr(n)
        } else {
            Ask::Bad("diff: agent, branch, range or pr".into())
        };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let base_of = |d: &Path| diff::trunk(d);
            let mut ev = json!({"ev": "diff", "req": req, "working": false, "uncommitted": false, "landed_ms": null});
            let out: Result<(Vec<diff::File>, Option<PathBuf>), String> = match ask {
                Ask::Agent(name, place, branch) => match place {
                    Where::Checkout { dir, base } => {
                        let base = base.unwrap_or_else(|| base_of(&dir));
                        ev["title"] = json!(format!("{} vs {}", name, base));
                        ev["base"] = json!(base);
                        ev["branch"] = json!(branch);
                        diff::checkout(&dir, &base).map(|(f, c, u)| {
                            ev["commits"] = json!(c);
                            ev["uncommitted"] = json!(u);
                            ev["working"] = json!(true);
                            (f, Some(dir))
                        })
                    }
                    Where::Shared { dir, files } => {
                        ev["title"] = json!(format!("{} · its files vs HEAD", name));
                        ev["base"] = json!("HEAD");
                        ev["branch"] = json!(null);
                        ev["commits"] = json!(0);
                        ev["uncommitted"] = json!(true);
                        ev["working"] = json!(true);
                        diff::own_files(&dir, &files).map(|f| (f, Some(dir)))
                    }
                },
                Ask::Branch(b) => {
                    let base = base_of(&shared);
                    ev["title"] = json!(format!("{} vs {}", b, base));
                    ev["base"] = json!(base);
                    ev["branch"] = json!(b);
                    diff::branch(&shared, &base, &b).map(|(f, c)| {
                        ev["commits"] = json!(c);
                        (f, Some(shared.clone()))
                    })
                }
                Ask::Range(r) => {
                    ev["title"] = json!(r);
                    ev["branch"] = json!(null);
                    ev["base"] = json!(r.split("..").next().unwrap_or(""));
                    diff::range(&shared, &r).map(|(f, c)| {
                        ev["commits"] = json!(c);
                        (f, Some(shared.clone()))
                    })
                }
                Ask::Pr(n) => {
                    ev["title"] = json!(format!("PR #{}", n));
                    ev["pr"] = json!(n);
                    diff::pr(&shared, n).map(|f| (f, None))
                }
                Ask::Bad(e) => Err(e),
            };
            match out {
                Ok((files, root)) => {
                    let (n, add, del) = diff::stat(&files);
                    ev["stat"] = json!({"files": n, "add": add, "del": del});
                    ev["files"] = json!(files.iter().map(|f| diff::file_json(f, root.as_deref())).collect::<Vec<_>>());
                }
                Err(e) => {
                    ev["files"] = json!([]);
                    ev["error"] = json!(e);
                }
            }
            let _ = tx.send(Msg::ToClient { id, v: ev });
        });
    }

    /// The `branches` op: the /diff picker's rows.
    pub(super) fn branches_op(&mut self, id: ClientId) {
        let shared = PathBuf::from(&self.hub.workspace);
        // who works on which branch, and the PRs the poller knows
        let mut on: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for a in self.hub.st.agents.values() {
            if a.status() == crate::model::Status::Archived {
                continue;
            }
            if let Some(b) = a.ws.branch.clone().or_else(|| a.place_branch.clone()) {
                on.entry(b).or_default().push(a.name.clone());
            }
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let base = diff::trunk(&shared);
            let rows: Vec<Value> = diff::branches(&shared, &base)
                .into_iter()
                .map(|(b, commits, add, del)| {
                    json!({"branch": b, "agents": on.get(&b).cloned().unwrap_or_default(), "commits": commits,
                           "uncommitted": false, "pr": null, "landed_ms": null, "add": add, "del": del})
                })
                .collect();
            let _ = tx.send(Msg::ToClient { id, v: json!({"ev": "branches", "base": base, "rows": rows}) });
        });
    }
}

/// The `landed` line of a land, after its `info` line in main's feed:
/// `agent : target : from : sha : files : add : del` (from = the
/// target's tip before, short). None when git cannot say.
pub(super) fn landed_fields(shared: &Path, agent: &str, target: &str, sha: &str, commits: usize) -> Option<String> {
    if commits == 0 {
        return None;
    }
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(shared)
        .args(["rev-parse", "--short", &format!("{}~{}", sha, commits)])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let from = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (files, add, del) = diff::range_stat(shared, &from, sha);
    Some(crate::core::join_fields(&[
        agent.to_string(),
        target.to_string(),
        from,
        sha.to_string(),
        files.to_string(),
        add.to_string(),
        del.to_string(),
    ]))
}
