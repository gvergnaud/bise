//! The methods answered later (`Answer::Later`, architect m_66698), on a
//! fake JSON-RPC connection to the shell the hub runs: each request gets
//! exactly one response, its thread's result, never `{}` at its step's
//! end. A task's Activity spun forever in the window because `task/log`
//! got `{}` at once and its thread's answer then found no request.

use super::*;
use bise_proto::rpc::{self, Answer};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// One agent and one task in sb-core's journal.
fn journal() -> Vec<Value> {
    vec![
        json!({"type": "task_created", "name": "t0", "parent": "main", "at_ms": 1,
            "brief": {"objective": "a task", "constraints": [], "done_when": "landed", "report_format": null},
            "ws": {"base_commit": null, "branch": null, "dropped": false, "mode": "shared", "path": "", "state": "ready", "with_changes": false}}),
        json!({"type": "work_created", "work": {"agents": ["main"], "by": "main", "created_ms": 2, "goals": [], "id": "fix-it", "kind": "task", "last": 2, "name": "Fix it", "origin": 0, "said": "fix it", "why": "it broke"}}),
    ]
}

/// The shell with a JSON-RPC client connected: its lines come on the receiver.
struct Fake {
    sh: Shell,
    rx: Receiver<Msg>,
    lines: Receiver<Value>,
    root: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fake(name: &str) -> Fake {
    let root = std::env::temp_dir().join(format!("sb-rpc-answer-{}-{}-{}", name, std::process::id(), now_ms()));
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let paths = Paths { workspace: ws.clone(), state: root.join("state"), worktrees: root.join("wt"), run: root.join("run") };
    std::fs::create_dir_all(&paths.state).unwrap();
    let mut hub = Hub::new(&ws.to_string_lossy());
    let lines = journal();
    let skipped = hub.replay(&lines);
    assert!(skipped.is_empty(), "sb-core skipped {} events", skipped.len());
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(paths.journal(), text).unwrap();
    let opts = Opts {
        paths: paths.clone(),
        home: bise_home::Home::at(root.join("home")),
        app_root: root.clone(),
        core_bin: PathBuf::new(),
        jsrt_bin: None,
        exe: root.join("bise"),
        repl_bin: root.join("repl"),
        spawn_env: None,
    };
    let journal = journal_file::Out::new(std::fs::OpenOptions::new().create(true).append(true).open(paths.journal()).unwrap());
    let (tx, rx) = channel();
    let photos = photo_take::off(paths.state.join("photo"));
    let mut sh = Shell::assemble(assemble::Parts { opts, hub, tx, journal, photos, proc_hub: "rpc-answer".into() });
    let (ours, theirs) = UnixStream::pair().unwrap();
    let (out, lines) = channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for l in std::io::BufReader::new(theirs).lines() {
            let Ok(l) = l else { return };
            if let Ok(v) = serde_json::from_str::<Value>(&l) {
                if out.send(v).is_err() {
                    return;
                }
            }
        }
    });
    let init = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"proto": bise_proto::PROTO, "client": {"name": "fake", "version": "0", "kind": "desktop"}}});
    sh.rpc_new(1, ours, init, "fake".into());
    let mut f = Fake { sh, rx, lines, root };
    assert!(!f.responses(0, Duration::from_secs(5)).is_empty(), "initialize answered");
    f
}

impl Fake {
    /// Runs the threads' answers on the loop, as the hub does, and gathers
    /// the responses to request `id` until one came and `quiet` passed
    /// with nothing more (or 10 s).
    fn responses(&mut self, id: u64, quiet: Duration) -> Vec<Value> {
        let (mut got, end) = (Vec::new(), Instant::now() + Duration::from_secs(10));
        let mut last = Instant::now();
        while Instant::now() < end && (got.is_empty() || last.elapsed() < quiet) {
            while let Ok(m) = self.rx.try_recv() {
                if let Msg::Typed { to, v } = m {
                    self.sh.typed_msg(to, v);
                }
            }
            while let Ok(v) = self.lines.recv_timeout(Duration::from_millis(20)) {
                if v.get("id").and_then(Value::as_u64) == Some(id) && (v.get("result").is_some() || v.get("error").is_some()) {
                    got.push(v);
                    last = Instant::now();
                }
            }
        }
        got
    }

    /// `method` with `params` as request `id`: its one response.
    fn ask(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.sh.rpc_line(1, json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let got = self.responses(id, Duration::from_millis(300));
        assert_eq!(got.len(), 1, "{method}: exactly one response, got {got:?}");
        got.into_iter().next().unwrap()
    }
}

/// The result of a response, read back as the method's event (what the
/// desktop's core does, hub_rpc.rs): never `{}`, never an error.
fn answered(method: &str, r: &Value) -> HubEv {
    let result = r.get("result").cloned().unwrap_or_else(|| panic!("{method}: an error, not its result: {r}"));
    assert_ne!(result, json!({}), "{method}: answered {{}} at its step's end, its thread's answer lost");
    match rpc::ev_of_result(method, result) {
        Ok(Some(ev)) => ev,
        other => panic!("{method}: its result is not its event: {other:?}"),
    }
}

#[test]
fn a_tasks_activity_is_answered_once_with_its_lines() {
    let mut f = fake("task-log");
    let p = f.sh.project();
    let r = f.ask(7, "task/log", json!({"project": p, "id": "fix-it", "req": 3}));
    match answered("task/log", &r) {
        HubEv::TaskLog { req, id, lines, error, .. } => {
            assert_eq!((req, id.as_str(), error), (3, "fix-it", None));
            assert!(!lines.is_empty(), "its birth gives a line (who started it)");
        }
        ev => panic!("task/log: {ev:?}"),
    }
}

#[test]
fn the_library_is_answered_once_from_its_thread() {
    let mut f = fake("library");
    let p = f.sh.project();
    let r = f.ask(8, "artifacts/library", json!({"project": p}));
    assert!(matches!(answered("artifacts/library", &r), HubEv::Library { .. }));
}

#[test]
fn a_threads_search_and_marks_are_answered_once_from_their_thread() {
    let mut f = fake("find");
    let p = f.sh.project();
    let r = f.ask(9, "thread/find", json!({"project": p, "agent": "t0", "needle": "x", "rid": 1}));
    assert!(matches!(answered("thread/find", &r), HubEv::ThreadHits { .. }));
    let r = f.ask(10, "thread/marks", json!({"project": p, "agent": "t0", "limit": 5}));
    assert!(matches!(answered("thread/marks", &r), HubEv::ThreadMarks { .. }));
}

/// The table's side: every method that answers from a thread in this
/// file's tests is `Later`, so a row moved back to `Step` turns them red
/// here and not only on a fake connection.
#[test]
fn the_thread_answered_methods_say_later() {
    for m in ["task/log", "artifacts/library", "artifacts/move", "thread/find", "thread/marks"] {
        assert_eq!(rpc::method_row(m).map(|r| r.answer), Some(Answer::Later), "{m}");
    }
}
