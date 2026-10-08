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
//!   or `{}` once the arm is done (an action). The thread answers (git,
//!   lsof: [`LATER`]) stay pending until their event comes. Lines for that
//!   connection made while its request runs are held, then sent after its
//!   response (Vibe's order: the response first);
//! - hub-wide notifications are numbered: [`Shell::proto_note`] moves the
//!   watermark once per event and sends it to every typed connection.
//!   The counter is transport state, never journaled; `epoch` is this
//!   hub run's start (ms), so a restart or a reload is a new epoch.
//!
//! agent.sock is out of scope (agents' `sb` ops, unchanged).

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rpc::{self, code, HubState, Id, InitializeParams, InitializeResult, Message, Response, RpcError, Scope, Watermark};
use bise_proto::PROTO;

/// The methods answered from a thread (git, lsof), after the arm returns.
const LATER: &[&str] = &["diff/read", "worktrees/list", "devServers/list", "merged/list"];

/// The hub-wide kinds sent from a thread: `hub/read` gives their last.
const SCANNED: &[&str] = &["worktrees", "dev_servers", "merged"];

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
    /// said `initialize`: notifications go to it. False: an older hello
    /// connection that sends requests (only their answers go to it)
    init: bool,
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
        let mut evs = vec![agents, cards, jobs, arts, self.features_ev(), self.prs_ev(), self.scheduled_ev(), self.models_ev(), appr];
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
            // command/run's words (`/help`, `/flow`, `/artifacts add`):
            // its typed result
            if let HubEv::Notice { cmd: Some(c), text, .. } = ev {
                if let Some(p) = self.rpc_pending(id, |p| p.cmd == c.as_str() && p.cmd == "slash") {
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
    /// the rest (the terminal's older events) not at all. False: not one.
    pub(super) fn rpc_effect(&mut self, id: ClientId, body: &Value) -> bool {
        if !self.rpc.init(id) {
            return false;
        }
        if body.get("ev").and_then(Value::as_str) == Some("notice") {
            let text = body.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let ev = HubEv::Notice { project: self.project(), cmd: None, text, cid: None };
            self.rpc_out(id, &ev, false);
        }
        true
    }

    /// A connection gone.
    pub(super) fn rpc_gone(&mut self, id: ClientId) {
        self.rpc.conns.remove(&id);
    }
}
