//! The hub's JSON-RPC 2.0 door (client-protocol step 1, architect
//! m_13089; the envelope and its tables are `bise_proto::rpc`).
//!
//! - a connection whose first line is `initialize` (accept.rs judges it
//!   like `hello`) gets no hello burst: its answer is the hub's identity,
//!   methods and hub-wide state at the hub's [`Watermark`]; from then on
//!   it is a typed connection (`proto.rs`'s `Conn`, typed only) whose
//!   events go out as notifications;
//! - a request on any connection (also an older `hello` one: the
//!   terminal moves its actions over one zone at a time) becomes its
//!   `HubCmd` and runs the typed arm (`proto/cmds.rs`); its answer is
//!   the event the arm sends that connection (a read's), the arm's error,
//!   or `{}` once the arm is done (an action); a method that answers the
//!   hub's words (`rpc::says`: command/run, artifacts/add, version/info...)
//!   gets the arm's `notice` as its `CommandRunResult`. The thread answers
//!   (git, lsof, the release plan: [`LATER`]) stay pending until their
//!   event comes. Lines for that
//!   connection made while its request runs are held, then sent after its
//!   response (Vibe's order: the response first);
//! - hub-wide notifications are numbered: [`Shell::proto_note`] moves the
//!   watermark once per event and sends it to every typed connection.
//!   The counter is transport state, never journaled; `epoch` is this
//!   hub run's start (ms), so a restart or a reload is a new epoch.
//!
//! - sb-core's lines for one client become its One notifications
//!   (`hub/notice`, update-card's `card/open`, the hub's `client/focused`:
//!   `one_ev`); `hub/flow` is a hub-wide kind of the state (P4b).
//!
//! agent.sock is out of scope (agents' `sb` ops, unchanged).

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rpc::{self, code, HubState, Id, InitializeParams, InitializeResult, Message, Response, RpcError, Scope, Watermark};
use bise_proto::PROTO;

/// sb-core's line for one client (`Effect::ToClient`) as its typed One
/// notification: `notice`, `open_card`, `focus`; none: an older event
/// with no typed form for one client.
fn one_ev(project: &str, body: &Value) -> Option<HubEv> {
    let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let project = project.to_string();
    match body.get("ev").and_then(Value::as_str)? {
        "notice" => Some(HubEv::Notice { project, cmd: None, text: s("text"), cid: None }),
        "open_card" => Some(HubEv::CardOpen { project, id: body.get("id").and_then(Value::as_u64)? }),
        "focus" => Some(HubEv::Focused { project, focus: s("focus") }),
        _ => None,
    }
}

/// The methods answered from a thread (git, lsof), after the arm returns.
const LATER: &[&str] = &["diff/read", "worktrees/list", "devServers/list", "merged/list", "branches/list", "release/plan", "tool/output"];

/// The hub-wide kinds sent from a thread: `hub/read` gives their last.
const SCANNED: &[&str] = &["worktrees", "dev_servers", "merged"];

/// An op line of the released core's door and the typed command its
/// arm takes (`renames`: the op's field -> the command's).
struct DoorOp {
    op: &'static str,
    cmd: &'static str,
    renames: &'static [(&'static str, &'static str)],
}

/// What an older client's hello connection may still send (the stub,
/// architect m_15183): the released desktop cores' door, one release:
/// v2026.10.2-28's and the release cut from main before client-protocol
/// merges (-29, proto-lead m_16089), the union of what both send. Its typed lines (`cmd`, as that core sends them) and its
/// op lines, each as the command its typed arm takes (no second
/// implementation). Step 3 had dropped input, interrupt and every_stop
/// for that core; the table brings them back for one release. stop_hub:
/// that release's `bise stop` (switchboard client::stop then).
// TODO(client-protocol, the plan's 'after the release' step): the door
// closes in the first release after the one that ships client-protocol
// (-28 and -29 desktops get exactly one release of overlap): the table
// goes, the stub writes exe and reload then closes (with the core's
// older door, Hubs.older_door)
// The cmd tags: the released cores' HubCmd::TAGS, every one (a core
// forwards the window's typed commands, core/cmd.rs, Cmd::Typed):
// v2026.10.2-28's 31, and -29's one more, tool_out (a tool row's whole
// output, HubCmd::ToolOut; -29's answer may carry files, the same arm).
pub(super) const DOOR_CMDS: &[&str] = &[
    "hello", "subscribe", "unsubscribe", "page", "send", "answer", "close", "confirm", "approvals", "remove_rule", "stop", "archive", "unarchive", "artifacts_seen", "tool_out", "diff", "worktrees", "dev_servers", "merged", "features", "prs", "scheduled", "scheduled_stop", "models", "new", "rename", "model", "effort", "route_correct", "route_cancel", "follow", "slash",
];
const DOOR_OPS: &[DoorOp] = &[
    DoorOp { op: "input", cmd: "slash", renames: &[("focus", "agent"), ("text", "line")] },
    DoorOp { op: "interrupt", cmd: "stop", renames: &[] },
    DoorOp { op: "every_stop", cmd: "scheduled_stop", renames: &[] },
    DoorOp { op: "page_voice", cmd: "page_voice", renames: &[] },
    DoorOp { op: "stop_hub", cmd: "stop_hub", renames: &[] },
];

/// The older events the released cores (v2026.10.2-28, -29: the same
/// hub_line) read on their home
/// connection (its hub_line: state, ready, page, line), nothing more:
/// they go to that connection only ([`Shell::door_events`]), from
/// today's writers (the snapshot, line_event, the pages' page line).
/// Those writers live this one release more for it.
// TODO(client-protocol, the plan's 'after the release' step): in the
// first release after the one that ships client-protocol, the table goes
// with DOOR_OPS and DOOR_CMDS (no older event to anyone)
pub(super) const DOOR_EVENTS: &[&str] = &["state", "ready", "page", "line"];

/// Line `v` of the released door as its typed command (`project` this
/// hub's); Err: what it was, refused.
fn door_cmd(v: &Value, project: &str) -> Result<Value, String> {
    if let Some(tag) = v.get("cmd").and_then(Value::as_str) {
        return if DOOR_CMDS.contains(&tag) { Ok(v.clone()) } else { Err(format!("the cmd {tag:?}")) };
    }
    let Some(op) = v.get("op").and_then(Value::as_str) else { return Err("a line with no op".into()) };
    let Some(row) = DOOR_OPS.iter().find(|r| r.op == op) else { return Err(format!("the op {op:?}")) };
    let mut c = v.as_object().cloned().unwrap_or_default();
    c.remove("op");
    for (from, to) in row.renames {
        if let Some(x) = c.remove(*from) {
            c.insert(to.to_string(), x);
        }
    }
    c.insert("cmd".into(), json!(row.cmd));
    c.insert("project".into(), json!(project));
    Ok(Value::Object(c))
}

/// The JSON-RPC side of every connection that used it.
#[derive(Default)]
pub(super) struct Rpcs {
    conns: BTreeMap<ClientId, Conn>,
    wm: Watermark,
    /// the last hub-wide event of each [`SCANNED`] kind
    scanned: BTreeMap<String, HubEv>,
}

#[derive(Default)]
struct Conn {
    /// said `initialize`: notifications go to it
    init: bool,
    /// an older client's hello (the stub): only [`DOOR`]'s lines
    older: bool,
    pending: Vec<Pending>,
    /// its lines held while one of its requests runs
    hold: Option<Vec<String>>,
}

struct Pending {
    id: Id,
    cmd: &'static str,
    result: Option<&'static str>,
}

/// Where a thread's typed event goes: the answer to one connection's
/// request (or hello), or a hub-wide notification to these connections.
pub(super) enum Typed {
    Answer(ClientId),
    All(Vec<ClientId>),
}

impl Typed {
    pub(super) fn is_empty(&self) -> bool {
        matches!(self, Typed::All(ids) if ids.is_empty())
    }
}

impl Rpcs {
    /// The watermark moved by one hub-wide notification.
    fn bump(&mut self) -> Watermark {
        if self.wm.epoch == 0 {
            self.wm.epoch = now_ms();
        }
        self.wm.seq += 1;
        self.wm
    }

    fn now(&mut self) -> Watermark {
        if self.wm.epoch == 0 {
            self.wm.epoch = now_ms();
        }
        self.wm
    }

    /// A connection that said `initialize`.
    pub(super) fn init(&self, id: ClientId) -> bool {
        self.conns.get(&id).is_some_and(|c| c.init)
    }

}

impl Shell {
    /// A connection whose first line is `initialize` (accept.rs): its
    /// stream joins the clients, sb-core hears it like a hello, then the
    /// request is answered. No hello burst.
    pub(super) fn rpc_new(&mut self, id: ClientId, stream: UnixStream, v: Value) {
        self.clients.insert(id, stream);
        self.step(Input::ClientHello { client: id });
        self.rpc_line(id, v);
    }

    /// An older client's hello connection (the stub): [`DOOR_OPS`] and
    /// [`DOOR_CMDS`] only.
    pub(super) fn rpc_older(&mut self, id: ClientId) {
        self.rpc.conns.insert(id, Conn { older: true, ..Default::default() });
    }

    /// Line `v` (not JSON-RPC) of client `id` as its typed command: only
    /// on an older hello's connection, only the released door's lines.
    pub(super) fn rpc_door(&self, id: ClientId, v: &Value) -> Result<Value, String> {
        let what = || v.get("op").or_else(|| v.get("cmd")).map_or_else(|| "a line".to_string(), |x| format!("the line {x}"));
        if !self.rpc.conns.get(&id).is_some_and(|c| c.older) {
            return Err(what());
        }
        door_cmd(v, &self.project())
    }

    /// The one test of the released core's home connection (architect
    /// m_15390): an older hello (the stub, after the peer judge in
    /// accept.rs), then a typed hello without typed_only. Nothing else
    /// gets an older event.
    pub(super) fn door_events(&self, id: ClientId) -> bool {
        self.rpc.conns.get(&id).is_some_and(|c| c.older) && self.proto.has(id) && !self.proto.typed_only(id)
    }

    /// Older event `v` to the released core's home connections, when it
    /// is a kind of [`DOOR_EVENTS`].
    pub(super) fn door_write(&mut self, v: &Value) {
        if !v.get("ev").and_then(Value::as_str).is_some_and(|e| DOOR_EVENTS.contains(&e)) {
            return;
        }
        let ids: Vec<ClientId> = self.clients.keys().copied().filter(|id| self.door_events(*id)).collect();
        if ids.is_empty() {
            return;
        }
        let line = v.to_string();
        for id in ids {
            let alive = self.clients.get_mut(&id).is_some_and(|c| write_line(c, &line));
            if !alive {
                self.clients.remove(&id);
                let _ = self.tx.send(Msg::ClientGone { id });
            }
        }
    }

    /// What the released core's home connection read at its hello, of
    /// [`DOOR_EVENTS`]' kinds: the state, each feed's buffered lines,
    /// then ready (the older hello burst's own order and writers).
    pub(super) fn door_burst(&mut self, id: ClientId) {
        let mut out = format!("{}\n", self.snapshot());
        for name in &self.hub.st.order {
            for (pos, ts, l) in self.buffers.get(name).into_iter().flatten() {
                out.push_str(&super::line_event(name, *pos, *ts, l).to_string());
                out.push('\n');
            }
        }
        out.push_str(&json!({"ev": "ready"}).to_string());
        out.push('\n');
        if let Some(c) = self.clients.get_mut(&id) {
            let _ = std::io::Write::write_all(c, out.as_bytes());
        }
    }

    /// One JSON-RPC line from client `id`.
    pub(super) fn rpc_line(&mut self, id: ClientId, v: Value) {
        match Message::from_value(v) {
            Err(resp) => self.rpc_write(id, &Message::Response(*resp).encode(), true),
            Ok(Message::Request(r)) => self.rpc_request(id, r.id, &r.method, r.params),
            // `initialized`, and no hub -> client request yet
            Ok(Message::Notification(_) | Message::Response(_)) => {}
        }
    }

    fn rpc_respond(&mut self, id: ClientId, resp: Response) {
        self.rpc_write(id, &Message::Response(resp).encode(), true);
    }

    fn rpc_request(&mut self, id: ClientId, rid: Id, method: &str, params: Value) {
        let init = self.rpc.conns.get(&id).map(|c| c.init);
        if method == rpc::INITIALIZE {
            if init.is_some() || self.proto.has(id) {
                return self.rpc_respond(id, Response::err(Some(rid), RpcError::new(code::NOT_INITIALIZED, "initialize comes once, first")));
            }
            return self.rpc_initialize(id, rid, params);
        }
        if method == rpc::COMMANDS_LIST {
            return self.rpc_respond(id, Response::ok(rid, serde_json::to_value(rpc::CommandsList::now()).unwrap_or_default()));
        }
        if method == rpc::HUB_READ {
            let state = self.hub_state();
            return self.rpc_respond(id, Response::ok(rid, serde_json::to_value(state).unwrap_or_default()));
        }
        let cmd = match rpc::cmd(method, params) {
            Ok(c) => c,
            Err(e) => return self.rpc_respond(id, Response::err(Some(rid), e)),
        };
        let Some(row) = rpc::method_row(method) else { return };
        if let Some(p) = cmd.project().filter(|p| *p != self.project()) {
            let e = RpcError::refused(format!("this hub is {}, not {p}", self.project()), None);
            return self.rpc_respond(id, Response::err(Some(rid), e));
        }
        // an older hello connection (the terminal) sends requests too
        let c = self.rpc.conns.entry(id).or_default();
        c.pending.push(Pending { id: rid.clone(), cmd: row.cmd, result: row.result });
        c.hold = Some(Vec::new());
        let project = self.project();
        self.proto_run(id, row.cmd, project, cmd);
        // done: an action's `{}` (a read answered already, a thread's
        // answer comes later), then what was held
        if !LATER.contains(&row.method) {
            if let Some(p) = self.rpc_pending(id, |p| p.id == rid) {
                self.rpc_respond(id, Response::ok(p.id, Value::Object(Default::default())));
            }
        }
        let held = self.rpc.conns.get_mut(&id).and_then(|c| c.hold.take()).unwrap_or_default();
        for line in held {
            self.rpc_write(id, &line, true);
        }
    }

    fn rpc_initialize(&mut self, id: ClientId, rid: Id, params: Value) {
        let p: InitializeParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return self.rpc_respond(id, Response::err(Some(rid), RpcError::new(code::INVALID_PARAMS, format!("initialize: {e}")))),
        };
        if p.proto != PROTO {
            let e = RpcError::new(code::INVALID_PARAMS, format!("this hub speaks proto {PROTO}, not {}", p.proto));
            return self.rpc_respond(id, Response::err(Some(rid), e));
        }
        crate::util::timing(&format!("rpc initialize from {} {}", p.client.name, p.client.version));
        self.rpc.conns.insert(id, Conn { init: true, ..Default::default() });
        self.proto.typed(id);
        let ws = self.opts.paths.workspace.clone();
        let result = InitializeResult {
            project: self.project(),
            proto: PROTO,
            workspace: ws.to_string_lossy().to_string(),
            name: ws.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            exe: self.opts.exe.to_string_lossy().to_string(),
            state_dir: self.opts.paths.state.to_string_lossy().to_string(),
            version: crate::switch::version_info(&self.opts.app_root),
            reload: self.reload_id.clone(),
            pages_url: self.pg.pages.as_ref().map(|p| p.base()),
            methods: rpc::methods(),
            notifications: rpc::notifications(),
            hub: self.hub_state(),
        };
        self.rpc_respond(id, Response::ok(rid, serde_json::to_value(result).unwrap_or_default()));
    }

    /// The hub-wide state now, at the current watermark (`initialize`,
    /// `hub/read`): every hub-wide kind, the scanned ones from their last
    /// scan (none yet: a scan starts, its notification follows).
    fn hub_state(&mut self) -> HubState {
        let snap = self.snapshot();
        let (agents, cards) = self.proto_rows(&snap);
        let jobs = HubEv::Jobs { project: self.project(), items: crate::proto_view::jobs(&snap) };
        let arts = self.artifacts_ev();
        let arts = self.proto_artifacts(&arts);
        let appr = self.approvals_ev(false);
        let appr = self.proto_approvals(&appr);
        let pages = self.pages_ev(&snap);
        let mut evs = vec![agents, cards, jobs, arts, self.features_ev(), self.prs_ev(), self.scheduled_ev(), self.models_ev(), appr, self.flow_ev(), pages];
        let mut missing = Vec::new();
        for kind in SCANNED {
            match self.rpc.scanned.get(*kind) {
                Some(ev) => evs.push(ev.clone()),
                None => missing.push(*kind),
            }
        }
        let ids = self.typed_ids();
        for kind in missing {
            match kind {
                "worktrees" => self.worktrees_typed(Typed::All(ids.clone())),
                "dev_servers" => self.dev_servers_typed(Typed::All(ids.clone())),
                _ => self.merged_typed(Typed::All(ids.clone())),
            }
        }
        let state = evs.iter().filter_map(|e| rpc::note(e, None)).collect();
        HubState { watermark: self.rpc.now(), state }
    }

    /// Connection `id` waits for the answer to a request of command `cmd`
    /// (a thread's answer that has an older line too: `release_plan`).
    pub(super) fn rpc_waits(&self, id: ClientId, cmd: &str) -> bool {
        self.rpc.conns.get(&id).is_some_and(|c| c.pending.iter().any(|p| p.cmd == cmd))
    }

    /// Removes and returns the first pending request of `id` that `f` picks.
    fn rpc_pending(&mut self, id: ClientId, f: impl Fn(&Pending) -> bool) -> Option<Pending> {
        let c = self.rpc.conns.get_mut(&id)?;
        let i = c.pending.iter().position(f)?;
        Some(c.pending.remove(i))
    }

    /// One line to `id`; `now`: past its hold (a response).
    fn rpc_write(&mut self, id: ClientId, line: &str, now: bool) {
        if !now {
            if let Some(h) = self.rpc.conns.get_mut(&id).and_then(|c| c.hold.as_mut()) {
                h.push(line.to_string());
                return;
            }
        }
        if let Some(c) = self.clients.get_mut(&id) {
            write_line(c, line);
        }
    }

    /// A typed event for connection `id`, when it speaks JSON-RPC: the
    /// answer to its pending request (`answer`: never for a hub-wide
    /// notification) or, for an initialized one, its notification (a
    /// hub-wide one at the current watermark). True: done with it.
    pub(super) fn rpc_out(&mut self, id: ClientId, ev: &HubEv, answer: bool) -> bool {
        let Some(init) = self.rpc.conns.get(&id).map(|c| c.init) else { return false };
        let tag = ev.tag().to_string();
        if answer {
            if let HubEv::Error { cmd, text, reason, .. } = ev {
                let cmd = cmd.clone().unwrap_or_default();
                if let Some(p) = self.rpc_pending(id, |p| p.cmd == cmd) {
                    let e = RpcError::refused(text.clone(), reason.as_deref());
                    self.rpc_respond(id, Response::err(Some(p.id), e));
                    return true;
                }
            }
            // the hub's words for a method that answers them (command/run's
            // `/help`, `/flow`; artifacts/add; version/info...): its typed
            // result
            if let HubEv::Notice { cmd: Some(c), text, .. } = ev {
                if let Some(p) = self.rpc_pending(id, |p| p.cmd == c.as_str() && rpc::says(p.cmd)) {
                    let r = rpc::CommandRunResult { notice: Some(text.clone()) };
                    self.rpc_respond(id, Response::ok(p.id, serde_json::to_value(r).unwrap_or_default()));
                    return true;
                }
            }
            let tag_cmd = match ev {
                HubEv::Notice { cmd, .. } => cmd.clone(),
                _ => None,
            };
            if let Some(p) = self.rpc_pending(id, |p| p.result == Some(tag.as_str()) && tag_cmd.as_deref().is_none_or(|c| c == p.cmd)) {
                self.rpc_respond(id, Response::ok(p.id, rpc::result(ev)));
                return true;
            }
        }
        if !init {
            // the released core's door (its typed line, proto_send_old)
            return false;
        }
        let ev = match ev {
            // a refusal outside a request: a notice, never silence
            HubEv::Error { project, text, .. } => HubEv::Notice { project: project.clone().unwrap_or_else(|| self.project()), cmd: None, text: text.clone(), cid: None },
            e => e.clone(),
        };
        let hub = rpc::note_of_ev(ev.tag()).is_some_and(|r| r.scope == Scope::Hub);
        let w = hub.then(|| self.rpc.now());
        if let Some(n) = rpc::note(&ev, w) {
            self.rpc_write(id, &Message::Notification(n).encode(), false);
        }
        true
    }

    /// A hub-wide event to every connection of `ids`: the watermark moves
    /// once, every JSON-RPC connection gets it numbered, the older typed
    /// ones as today.
    pub(super) fn proto_note(&mut self, ids: &[ClientId], ev: &HubEv) {
        self.rpc.bump();
        if SCANNED.contains(&ev.tag()) {
            self.rpc.scanned.insert(ev.tag().to_string(), ev.clone());
        }
        for id in ids {
            if !self.rpc_out(*id, ev, false) {
                self.proto_send_old(*id, ev);
            }
        }
    }

    /// A thread's typed event reached the loop (`Msg::Typed`).
    pub(super) fn typed_msg(&mut self, to: Typed, v: Value) {
        let Ok(ev) = HubEv::from_value(v) else { return };
        match to {
            Typed::Answer(id) => self.proto_send(id, &ev),
            Typed::All(ids) => self.proto_note(&ids, &ev),
        }
    }

    /// sb-core's line for client `id` (`Effect::ToClient`) when it is an
    /// initialized JSON-RPC connection: a notice goes as `hub/notice`,
    /// update-card's `open_card` as `card/open`, the hub's `focus` as
    /// `client/focused` (P4b), the rest (the terminal's older events) not
    /// at all. False: not one.
    pub(super) fn rpc_effect(&mut self, id: ClientId, body: &Value) -> bool {
        if !self.rpc.init(id) {
            return false;
        }
        if let Some(ev) = one_ev(&self.project(), body) {
            self.rpc_out(id, &ev, false);
        }
        true
    }

    /// A connection gone.
    pub(super) fn rpc_gone(&mut self, id: ClientId) {
        self.rpc.conns.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Client-protocol step 5: no older notice line is written at all
    /// (architect m_14727's one writer is gone with it): a connection
    /// gets the typed notice, or nothing.
    #[test]
    fn no_older_notice_line_is_written() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![root.join("daemon.rs")];
        let mut dirs = vec![root.join("daemon")];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.to_string_lossy().ends_with("_tests.rs") {
                    files.push(p);
                }
            }
        }
        let needle = concat!("\"ev\": ", "\"notice\"");
        let mut at = Vec::new();
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            // the shell's code, not its tests
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            for (i, l) in code.lines().enumerate() {
                if l.contains(needle) {
                    at.push(format!("{}:{}", f.strip_prefix(&root).unwrap().display(), i + 1));
                }
            }
        }
        assert!(at.is_empty(), "an older notice line is written: {at:?}");
    }

    use bise_proto::hub::HubCmd;
    use std::collections::{BTreeMap, BTreeSet};

    fn door(v: Value) -> Result<Value, String> {
        door_cmd(&v, "acme")
    }

    /// The released cores' lines (rust/proto/fixtures/released/
    /// core_door.jsonl for v2026.10.2-28, core_door_v2026.10.2-29.jsonl for -29,
    /// their sources' own shapes) map to the typed command its arm takes;
    /// their cmd lines pass as they are; anything else is refused.
    #[test]
    fn the_released_doors_lines_map_to_their_typed_commands() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../proto/fixtures/released");
        for f in ["core_door.jsonl", "core_door_v2026.10.2-29.jsonl"] {
            door_lines(&std::fs::read_to_string(dir.join(f)).unwrap());
        }
        // -29's one more tag: a tool row's output, and its answer's files
        assert_eq!(door(json!({"cmd": "tool_out", "project": "acme", "agent": "main", "pos": 12})), Ok(json!({"cmd": "tool_out", "project": "acme", "agent": "main", "pos": 12})));
        // outside the table: refused, and what it was
        assert_eq!(door(json!({"op": "history", "agent": "main"})), Err("the op \"history\"".to_string()));
        assert_eq!(door(json!({"cmd": "release_run", "project": "acme"})), Err("the cmd \"release_run\"".to_string()));
        assert_eq!(door(json!({"x": 1})), Err("a line with no op".to_string()));
    }

    /// One released core's lines: each maps to its typed command, every
    /// op of the table is among them, with its exact command.
    fn door_lines(text: &str) {
        let mut ops = BTreeMap::new();
        for l in text.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(l).unwrap();
            if v.get("op") == Some(&json!("hello")) {
                continue;
            }
            let c = door(v.clone()).unwrap_or_else(|e| panic!("{l}: {e}"));
            let cmd = HubCmd::from_value(c.clone()).unwrap_or_else(|e| panic!("{l}: {e}"));
            assert!(!matches!(cmd, HubCmd::Unknown { .. }), "{l}");
            if let Some(op) = v.get("op").and_then(Value::as_str) {
                ops.insert(op.to_string(), c);
            }
        }
        // every op of the table is in the released lines
        assert_eq!(ops.keys().map(String::as_str).collect::<Vec<_>>(), DOOR_OPS.iter().map(|o| o.op).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>());
        assert_eq!(ops["input"], json!({"cmd": "slash", "project": "acme", "agent": "docs", "line": "thanks", "via": "ambient"}));
        assert_eq!(ops["stop_hub"], json!({"cmd": "stop_hub", "project": "acme", "keep_agents": true}));
        assert_eq!(ops["interrupt"], json!({"cmd": "stop", "project": "acme", "agent": "docs"}));
        assert_eq!(ops["every_stop"], json!({"cmd": "scheduled_stop", "project": "acme", "id": 3}));
        assert_eq!(ops["page_voice"], json!({"cmd": "page_voice", "project": "acme", "page": "weekly-update", "phase": "heard", "text": "shorter"}));
    }
}
