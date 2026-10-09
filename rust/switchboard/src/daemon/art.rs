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

    /// `{"ev":"artifacts","rows":[…],"new":N,"seen_ms":T}`.
    pub(super) fn artifacts_ev(&self) -> Value {
        let store = self.art_store();
        let who = self.art_who();
        let now = now_ms();
        json!({
            "ev": "artifacts",
            "rows": store.rows(&who, &self.hub.workspace),
            "new": store.new_count(now),
            "seen_ms": store.seen_ms(now),
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
                // `at_ms`: when the user looked (an older client: now)
                let now = now_ms();
                let at = v.get("at_ms").and_then(|x| x.as_u64()).unwrap_or(now);
                let _ = self.art_store().saw(at, now);
                self.artifacts_refresh(true);
            }
            "add" => {
                let title = Some(s("title")).filter(|t| !t.trim().is_empty());
                let ev = match self.art_add(&s("agent"), &s("target"), title) {
                    Ok(text) => json!({"ev": "notice", "text": text}),
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

    /// `/artifacts add <path or link>` by the user in `agent`'s view (the
    /// TUI's `artifacts` op and the window's `slash`): the store's add,
    /// its thread lines, and the words for the one who asked (`↗ added:
    /// <title>`) or the store's refusal. The caller sends them and then
    /// `artifacts_refresh(true)`.
    pub(super) fn art_add(&mut self, agent: &str, target: &str, title: Option<String>) -> Result<String, String> {
        let agent = match agent {
            "" => MAIN.to_string(),
            a => a.to_string(),
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
        let add = Add { target: target.to_string(), title, kind: None, agent, by: "you".into(), cwd };
        let a = self.art_store().add(&add, now_ms())?;
        if a.new_version {
            self.art_lines(&a);
        }
        Ok(format!("↗ added: {}", a.meta.title))
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
            let v = changes_of(&place);
            let _ = tx.send(Msg::Changes { name, v });
        });
    }

    /// An agent's changes are computed: the state again when they moved.
    pub(super) fn on_changes(&mut self, name: String, v: Value) {
        self.art.busy.remove(&name);
        let moved = self.art.changes.get(&name) != Some(&v);
        self.art.changes.insert(name.clone(), v);
        if moved {
            self.state_now();
        }
        if self.art.stale.contains(&name) {
            self.changes_ask(&name, false);
        }
    }

    /// The `diff` op: an agent's changes, a branch, a range or a PR,
    /// answered with a `diff` event to that client.
    pub(super) fn diff_op(&mut self, id: ClientId, v: &Value) {
        let req = v.get("req").cloned().unwrap_or(Value::Null);
        let shared = PathBuf::from(&self.hub.workspace);
        let ask = self.diff_ask(v);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let ev = diff_answer(ask, &shared, req);
            let _ = tx.send(Msg::ToClient { id, v: ev });
        });
    }

    /// The typed `diff {project, agent}` (desktop S7): the same answer as
    /// the op, in bise-proto's shape (`proto_view::diff`), to that client.
    pub(super) fn diff_typed(&mut self, id: ClientId, project: String, agent: String, commit: Option<String>) {
        let shared = PathBuf::from(&self.hub.workspace);
        // one commit: the range ask the TUI's door under a land uses
        let ask = match &commit {
            Some(c) => self.diff_ask(&json!({"agent": agent, "range": format!("{c}^..{c}")})),
            None => self.diff_ask(&json!({"agent": agent})),
        };
        // emitter 5: what its change measured (`sb report --result`), from
        // its latest report
        let result = self.hub.st.agents.get(&agent).and_then(|a| a.last_report.as_ref()).and_then(|r| r.result.clone());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut ev = diff_answer(ask, &shared, Value::Null);
            // a binary file's size, read here (the mapper stays pure):
            // its file in the agent's checkout, when it is still there
            for f in ev.get_mut("files").and_then(Value::as_array_mut).into_iter().flatten() {
                let abs = f.get("abs").and_then(Value::as_str).map(PathBuf::from);
                if f["binary"] == true {
                    if let Some(m) = abs.and_then(|p| std::fs::metadata(p).ok()).filter(|m| m.is_file()) {
                        f["size"] = json!(m.len());
                    }
                }
            }
            let mut typed = crate::proto_view::diff(&ev, &project, &agent);
            // the answer says which commit it is (not the live change)
            if let bise_proto::hub::HubEv::Diff { commit: c, result: r, .. } = &mut typed {
                *c = commit;
                *r = result.and_then(|v| serde_json::from_value(v).ok()).map(Box::new);
            }
            let _ = tx.send(Msg::ToClient { id, v: typed.to_value() });
        });
    }

    /// What a diff request asks: an agent's changes, a branch, a range or
    /// a PR.
    fn diff_ask(&self, v: &Value) -> Ask {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let shared = PathBuf::from(&self.hub.workspace);
        // a range first: the door under a landed line names its agent
        // too (for the title), and must show that land, never the
        // agent's branch vs today's main (empty once landed)
        if !s("range").is_empty() {
            Ask::Range(s("range"), s("agent"))
        } else if !s("agent").is_empty() {
            match self.hub.st.agents.get(&s("agent")).or_else(|| {
                let n = self.art_who()(&s("agent")).map(|(n, _)| n)?;
                self.hub.st.agents.get(&n)
            }) {
                Some(a) => {
                    let branch = a.ws.branch.clone().or_else(|| a.place_branch.clone());
                    let archived = a.status() == crate::model::Status::Archived;
                    // an archived or dropped agent's worktree is removed (or
                    // soon): not the shared folder
                    let place = match self.art_where(a) {
                        Where::Shared { dir, .. } if dir != shared && (archived || !dir.is_dir()) => Where::Checkout { dir, base: None },
                        w => w,
                    };
                    Ask::Agent(a.name.clone(), place, branch, archived)
                }
                None => Ask::Bad(format!("no agent {}", s("agent"))),
            }
        } else if !s("branch").is_empty() {
            Ask::Branch(s("branch"))
        } else if let Some(n) = v.get("pr").and_then(|x| x.as_u64()) {
            Ask::Pr(n)
        } else {
            Ask::Bad("diff: agent, branch, range or pr".into())
        }
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

/// An agent's `changes` in the state: `{files, add, del}` of its
/// checkout against its base (commits and uncommitted edits, untracked
/// files included) or of its own files in the shared folder; null when
/// there are none or git cannot say (a folder that is gone: diff.rs
/// never runs git in it).
fn changes_of(place: &Where) -> Value {
    let files = match place {
        Where::Checkout { dir, base } => {
            let base = base.clone().unwrap_or_else(|| diff::trunk(dir));
            diff::checkout(dir, &base).map(|(f, _, _)| f)
        }
        Where::Shared { dir, files } => diff::own_files(dir, files),
    };
    match files {
        Ok(f) if !f.is_empty() => {
            let (files, add, del) = diff::stat(&f);
            json!({"files": files, "add": add, "del": del})
        }
        _ => Value::Null,
    }
}

/// What the `diff` op asks for.
#[derive(Clone, Debug)]
enum Ask {
    /// name, where, branch, archived
    Agent(String, Where, Option<String>, bool),
    Branch(String),
    Range(String, String),
    Pr(u64),
    Bad(String),
}

/// The `diff` event answering `ask` (git runs in `shared` or the
/// agent's own folder, never in one that is gone).
fn diff_answer(ask: Ask, shared: &Path, req: Value) -> Value {
    let shared = shared.to_path_buf();
    let base_of = |d: &Path| diff::trunk(d);
    let mut ev = json!({"ev": "diff", "req": req, "working": false, "uncommitted": false, "landed_ms": null});
    let out: Result<(Vec<diff::File>, Option<PathBuf>), String> = match ask {
        // its folder is gone (archived, or removed by hand): never
        // git in it; its branch vs main if the branch is still
        // there, else one plain line (designer m_7393)
        Ask::Agent(name, Where::Checkout { dir, .. }, branch, archived) if archived || !dir.is_dir() => {
            let base = base_of(&shared);
            ev["title"] = json!(format!("{} vs {}", name, base));
            ev["base"] = json!(base);
            ev["branch"] = json!(branch);
            match branch.filter(|b| diff::has_ref(&shared, b)) {
                Some(b) => diff::branch(&shared, &base, &b).map(|(f, c)| {
                    ev["commits"] = json!(c);
                    (f, Some(shared.clone()))
                }),
                None => {
                    ev["gone"] = json!(true);
                    ev["note"] = json!(if archived {
                        format!("{} is archived and its folder is gone", name)
                    } else {
                        format!("{}'s folder is gone", name)
                    });
                    Ok((Vec::new(), None))
                }
            }
        }
        Ask::Agent(name, place, branch, _) => match place {
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
                // the shared folder isn't only main's (designer
                // m_7354): `your folder vs main`, the same shape
                // as a branch's; the files are still the agent's
                ev["title"] = json!(format!("your folder vs {}", base_of(&dir)));
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
            if diff::has_ref(&shared, &b) {
                diff::branch(&shared, &base, &b).map(|(f, c)| {
                    ev["commits"] = json!(c);
                    (f, Some(shared.clone()))
                })
            } else {
                ev["note"] = json!(format!("there's no branch named {}.", b));
                Ok((Vec::new(), None))
            }
        }
        Ask::Range(r, agent) => {
            // `diff-focus landed on main · e0f3df5` (the TUI's
            // head adds `· 11 files`)
            let to: String = r.split("..").last().unwrap_or(&r).trim_start_matches('.').chars().take(7).collect();
            let title = if agent.is_empty() { r.clone() } else { format!("{} landed on {} · {}", agent, base_of(&shared), to) };
            ev["title"] = json!(title);
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
    ev
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(ok.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&ok.stderr));
    }

    /// A repo with an agent's worktree on sb/t1 (one commit), then the
    /// worktree removed as an archive does.
    fn gone(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("sb-art-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let shared = root.join("repo");
        let wt = root.join("wt-t1");
        std::fs::create_dir_all(&shared).unwrap();
        sh(&shared, &["init", "-q", "-b", "main"]);
        std::fs::write(shared.join("a.txt"), "one\n").unwrap();
        sh(&shared, &["add", "."]);
        sh(&shared, &["commit", "-q", "-m", "base"]);
        sh(&shared, &["worktree", "add", "-q", "-b", "sb/t1", wt.to_str().unwrap()]);
        std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap();
        sh(&wt, &["commit", "-q", "-am", "t1"]);
        sh(&shared, &["worktree", "remove", "--force", wt.to_str().unwrap()]);
        assert!(!wt.exists());
        (shared, wt)
    }

    fn agent(wt: &Path, branch: Option<&str>, archived: bool) -> Ask {
        Ask::Agent("t1".into(), Where::Checkout { dir: wt.to_path_buf(), base: None }, branch.map(String::from), archived)
    }

    fn no_git_fatal(ev: &Value) {
        let s = ev.to_string();
        assert!(!s.contains("fatal") && !s.contains("cannot change to"), "{s}");
    }

    #[test]
    fn a_gone_worktree_shows_its_branch_vs_main() {
        let (shared, wt) = gone("branch");
        for archived in [true, false] {
            let ev = diff_answer(agent(&wt, Some("sb/t1"), archived), &shared, json!(1));
            no_git_fatal(&ev);
            assert_eq!(ev["title"], "t1 vs main");
            assert_eq!(ev["error"], Value::Null);
            assert_eq!(ev["note"], Value::Null);
            assert_eq!(ev["commits"], 1);
            assert_eq!(ev["files"][0]["path"], "a.txt");
        }
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }

    #[test]
    fn a_gone_worktree_without_its_branch_says_so_in_one_line() {
        let (shared, wt) = gone("nobranch");
        sh(&shared, &["branch", "-q", "-D", "sb/t1"]);
        let ev = diff_answer(agent(&wt, Some("sb/t1"), true), &shared, json!(1));
        no_git_fatal(&ev);
        assert_eq!(ev["note"], "t1 is archived and its folder is gone");
        assert_eq!(ev["gone"], true);
        assert_eq!(ev["error"], Value::Null);
        assert_eq!(ev["files"], json!([]));
        let ev = diff_answer(agent(&wt, None, false), &shared, json!(1));
        assert_eq!(ev["note"], "t1's folder is gone");
        // the same folder seen as a shared-folder agent's: never git in it
        let ev = diff_answer(Ask::Agent("t1".into(), Where::Shared { dir: wt.clone(), files: vec!["a.txt".into()] }, None, false), &shared, json!(1));
        no_git_fatal(&ev);
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }

    #[test]
    fn changes_count_a_commit_and_an_uncommitted_edit() {
        let root = std::env::temp_dir().join(format!("sb-art-changes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (shared, wt) = (root.join("repo"), root.join("wt-t1"));
        std::fs::create_dir_all(&shared).unwrap();
        sh(&shared, &["init", "-q", "-b", "main"]);
        std::fs::write(shared.join("README"), "hello\n").unwrap();
        sh(&shared, &["add", "."]);
        sh(&shared, &["commit", "-q", "-m", "init"]);
        sh(&shared, &["worktree", "add", "-q", "-b", "sb/t1", wt.to_str().unwrap()]);
        let place = Where::Checkout { dir: wt.clone(), base: None };
        assert_eq!(changes_of(&place), Value::Null, "nothing yet");
        std::fs::create_dir_all(wt.join("out")).unwrap();
        std::fs::write(wt.join("out/plans.csv"), "plan,price\nfree,0\npro,20\n").unwrap();
        sh(&wt, &["add", "out"]);
        sh(&wt, &["commit", "-q", "-m", "plans"]);
        std::fs::write(wt.join("README"), "hello\nmore\n").unwrap();
        std::fs::write(wt.join("new.txt"), "x\n").unwrap();
        assert_eq!(changes_of(&place), json!({"files": 3, "add": 5, "del": 0}));
        // its folder gone: null, and no git in it
        sh(&shared, &["worktree", "remove", "--force", wt.to_str().unwrap()]);
        assert_eq!(changes_of(&place), Value::Null);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn other_git_errors_read_as_one_line() {
        let (shared, _) = gone("errors");
        let ev = diff_answer(Ask::Branch("sb/nope".into()), &shared, json!(1));
        assert_eq!(ev["note"], "there's no branch named sb/nope.");
        let ev = diff_answer(Ask::Range("abc1234..def5678".into(), String::new()), &shared, json!(1));
        let e = ev["error"].as_str().unwrap();
        assert!(e.starts_with("git couldn't read this diff: ") && !e.contains('\n') && !e.contains("fatal"), "{e}");
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }
}
