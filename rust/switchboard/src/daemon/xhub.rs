//! The hub's side of bise's cross-hub messages (bise desktop S2).
//!
//! sb-core decides (hub/xhub.bend, core.bend's `cross-hub` section); this
//! file only carries:
//!
//! - bise's home hub: `Effect::XDeliver` sends outbox entry `xid` to the
//!   project's hub as `{"op":"xin"}` on its hub.sock (the hub started
//!   first when it isn't running, through `client::start_hub`: the
//!   environment of `env_for(Child::Hub)`, nothing of this hub's), on a
//!   thread; the answer comes back as `Input::XAck` or `Input::XFail`
//!   (sb-core sets the next try with its timers).
//! - a project hub: `{"op":"xin"}` becomes `Input::XIn` (sb-core answers
//!   the caller: an ack once main has it, or the error), and
//!   `Effect::XReply` sends main's answer back as `{"op":"xreply"}` to
//!   bise's hub, which steps `Input::XReply`.
//!
//! The target is a hub id of the registry (`bise_home::projects`), never
//! this hub (`projects::target` refuses it). Trust: `xin` and `xreply`
//! trust their caller like `input` does (same uid on hub.sock): an agent
//! could forge one as it could forge an input today; not new, not fixed
//! here.

use super::{log_line, write_json, Msg, Shell};
use crate::core::{Input, Token};
use crate::paths::Paths;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long a hub has to answer one request (an `xin` waits for its step).
const ANSWER: Duration = Duration::from_secs(20);
/// How long a stopped hub has to come up.
const START: Duration = Duration::from_secs(15);
/// The tries of an answer back to bise's hub (it has no outbox of its own).
const REPLY_TRIES: u32 = 5;

impl Shell {
    /// `{"op":"xin"|"xreply", ...}` on hub.sock: one request, one reply.
    pub(super) fn xhub_op(&mut self, token: Token, mut stream: UnixStream, v: Value) {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let xid = v.get("xid").and_then(Value::as_u64).unwrap_or(0);
        match s("op").as_str() {
            "xin" => {
                // sb-core answers through the token (Effect::Reply)
                self.replies.insert(token, stream);
                // a routed message's fn context (S2, 9dc23d9c): rendered
                // here once, the same block as a direct input's (S9)
                let text = match crate::fn_context::of(v.get("context")) {
                    Some(ctx) => crate::fn_context::with_context(&s("text"), &ctx),
                    None => s("text"),
                };
                self.step(Input::XIn { token, hub: s("hub"), xid, kind: s("kind"), text, name: s("name") });
            }
            // S10: bise's `sb follow <p>/<agent>`: main's follow here, its
            // answer (ok or why not) back to bise's hub at once
            "xfollow" => {
                self.replies.insert(token, stream);
                // J: `hub` asked: the job's end goes back there (its outbox)
                let req = crate::core::AgentReq::Follow { agent: s("agent"), on: v.get("on").and_then(Value::as_bool).unwrap_or(true), hub: s("hub") };
                self.step(Input::Agent { token, from: crate::model::MAIN.into(), req });
            }
            _ => {
                self.step(Input::XReply { xid, name: s("name"), text: s("text") });
                write_json(&mut stream, &json!({"ok": true}));
            }
        }
    }

    /// bise's home hub: deliver outbox entry `xid` to the hub `project`.
    /// `context`: a routed message's fn context (FnContext JSON text, ""
    /// none), sent as the op's `context` for the project to render.
    pub(super) fn xdeliver(&mut self, xid: u64, project: String, kind: String, text: String, context: String) {
        let own = crate::paths::workspace_id(&self.opts.paths.workspace);
        let ws = self.opts.paths.workspace.clone();
        let (tx, exe, root, log) = (self.tx.clone(), self.opts.exe.clone(), self.opts.app_root.clone(), self.opts.paths.clone());
        std::thread::spawn(move || {
            // J: `name` says who sends (a followed task's end is said from it)
            let mut req = json!({"op": "xin", "hub": own, "xid": xid, "kind": kind, "text": text, "name": own_name(&ws)});
            if let Some(c) = serde_json::from_str::<Value>(&context).ok().filter(Value::is_object) {
                req["context"] = c;
            }
            let res = hub_paths(&project).and_then(|p| call(&p, &exe, &root, &req));
            let input = match res {
                Ok(_) => Input::XAck { xid },
                Err(e) => {
                    log_line(&log, &format!("xhub: x{xid} to {project} not delivered: {e}"));
                    Input::XFail { xid }
                }
            };
            let _ = tx.send(Msg::In(input));
        });
    }

    /// Whether `sb follow` request `v` goes to another project's hub: on
    /// bise's home hub, an agent named `<p>/<agent>`.
    pub(super) fn wants_follow(&self, v: &Value) -> bool {
        crate::paths::is_home(&self.opts.paths.workspace) && v.get("agent").and_then(Value::as_str).is_some_and(|a| a.contains('/'))
    }

    /// S10, bise's home hub: `sb follow <p>/<agent> [--off]` (main only):
    /// the project's hub follows its task (started first when stopped);
    /// a command, not a message: no outbox, its answer or its failure
    /// comes back to main at once (on a thread: the hub never waits).
    pub(super) fn xfollow(&mut self, mut stream: UnixStream, v: &Value) {
        let from = v.get("from").and_then(Value::as_str).unwrap_or("");
        let target = v.get("agent").and_then(Value::as_str).unwrap_or("");
        let on = !v.get("off").and_then(Value::as_bool).unwrap_or(false);
        if from != crate::model::MAIN {
            write_json(&mut stream, &json!({"ok": false, "error": "only main follows a task: ask main"}));
            return;
        }
        let (p, agent) = match super::xread::split_ref(target) {
            Ok(x) => x,
            Err(e) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
                return;
            }
        };
        let own = crate::paths::workspace_id(&self.opts.paths.workspace);
        let home = bise_home::Home::from_env();
        let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
        let id = match bise_home::projects::target(&rows, &p, &own) {
            Ok(id) => id,
            Err(e) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
                return;
            }
        };
        let (exe, root) = (self.opts.exe.clone(), self.opts.app_root.clone());
        std::thread::spawn(move || {
            // J: `hub` (this one): the project's hub sends the job's end back here
            let req = json!({"op": "xfollow", "agent": agent, "on": on, "hub": own});
            let body = match hub_paths(&id).and_then(|paths| call(&paths, &exe, &root, &req)) {
                Ok(_) => json!({"ok": true, "agent": format!("{p}/{agent}"), "on": on}),
                Err(e) => json!({"ok": false, "error": format!("{p}: {e}")}),
            };
            write_json(&mut stream, &body);
        });
    }

    /// A project hub: main's answer to bise's message `xid` goes back to
    /// bise's hub `hub`, named as this project.
    pub(super) fn xreply_back(&mut self, hub: String, xid: u64, text: String) {
        let ws = self.opts.paths.workspace.clone();
        let (exe, root, log) = (self.opts.exe.clone(), self.opts.app_root.clone(), self.opts.paths.clone());
        std::thread::spawn(move || {
            let req = json!({"op": "xreply", "xid": xid, "name": own_name(&ws), "text": text});
            for i in 1..=REPLY_TRIES {
                match hub_paths(&hub).and_then(|p| call(&p, &exe, &root, &req)) {
                    Ok(_) => return,
                    Err(e) => {
                        log_line(&log, &format!("xhub: answer to x{xid} not sent to {hub} (try {i}): {e}"));
                        std::thread::sleep(Duration::from_secs(2u64.pow(i)));
                    }
                }
            }
        });
    }
}

/// `sb project list`: the registry's rows, one line each (the name `sb
/// project send|ask` takes, the folder).
pub(super) fn list_body() -> Value {
    let home = bise_home::Home::from_env();
    let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
    let lines: Vec<String> = rows
        .iter()
        .map(|r| format!("{}{} · {}", r.name, if r.home { " (you: bise's home)" } else { "" }, r.path.display()))
        .collect();
    json!({"ok": true, "text": lines.join("\n")})
}

/// This hub is bise's home hub: its main reads the user's message ids.
pub(super) fn is_home(ws: &Path) -> bool {
    bise_home::projects::canonical(ws) == bise_home::projects::canonical(&crate::paths::home_workspace())
}

/// The paths of the registered hub `id` (its workspace from the registry).
fn hub_paths(id: &str) -> Result<Paths, String> {
    let home = bise_home::Home::from_env();
    let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
    let row = rows.iter().find(|r| r.id == id).ok_or_else(|| format!("{id} is not a registered project"))?;
    if !row.path.is_dir() {
        return Err(format!("{} is gone", row.path.display()));
    }
    Ok(Paths::for_workspace(&row.path))
}

/// This workspace's name in the registry (its folder's when absent).
fn own_name(ws: &Path) -> String {
    let home = bise_home::Home::from_env();
    let id = crate::paths::workspace_id(ws);
    bise_home::projects::list(&home, &crate::paths::home_workspace())
        .into_iter()
        .find(|r| r.id == id)
        .map(|r| r.name)
        .unwrap_or_else(|| ws.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// One request to the hub of `paths` (started when none runs), its
/// answer: Ok when it says `ok`, else why not.
fn call(paths: &Paths, exe: &Path, root: &Path, req: &Value) -> Result<Value, String> {
    let mut s = match UnixStream::connect(paths.socket()) {
        Ok(s) => s,
        Err(_) => {
            crate::client::start_hub(paths, exe, root).map_err(|e| format!("cannot start its hub: {e}"))?;
            let t0 = Instant::now();
            loop {
                if let Ok(s) = UnixStream::connect(paths.socket()) {
                    break s;
                }
                if t0.elapsed() > START {
                    return Err(format!("its hub did not start in {} s", START.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    let _ = s.set_read_timeout(Some(ANSWER));
    writeln!(s, "{req}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(&s).read_line(&mut line).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(line.trim()).map_err(|_| format!("no answer ({})", line.trim()))?;
    match v.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(v),
        _ => Err(v.get("error").and_then(Value::as_str).unwrap_or("refused").to_string()),
    }
}
