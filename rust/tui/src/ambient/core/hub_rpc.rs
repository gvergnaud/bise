//! The core's JSON-RPC side of one project hub's connection
//! (client-protocol P1c, architect m_13089): its first line is
//! `initialize`, the window's `HubCmd`s go as requests by id, the hub's
//! responses and notifications come back as the `HubEv`s the window reads
//! today (the window's own stdio moves in step 6). Pure: the connection
//! (`hubs.rs`) writes what [`RpcConn`] makes and acts on what it reads.
//!
//! - `initialize`'s result is the window's `welcome` (its `cmds` the
//!   HubCmd tags of the hub's methods, so hub-skew's `lacks` reads it as
//!   before) then the hub-wide state, each kind as its event;
//! - a hub-wide notification goes through the watermark
//!   (`bise_proto::rpc::Watermark::take`): a gap or a new epoch asks
//!   `hub/read` again, whose state replaces it;
//! - an error response is the window's `error` with the request's `cid`
//!   and the hub's `reason`/`kind`; `command/run`'s words its `notice`;
//! - an older hub (one release, architect m_13089 Q2) answers the
//!   `initialize` line with its `does not serve the op` refusal and
//!   closes: the connection says hello the older way from then on.

use bise_proto::hub::{ErrorKind, HubCmd, HubEv};
use bise_proto::rpc::{self, CommandRunResult, HubState, Id, Init, InitializeParams, InitializeResult, Message, Request, Take, Watermark};
use serde_json::Value;
use std::collections::BTreeMap;

/// `initialize`'s id on every connection (the window's requests start at 1).
const INIT: u64 = 0;

/// One request waiting for its response.
#[derive(Clone, Debug, PartialEq)]
struct Waiting {
    method: &'static str,
    tag: String,
    cid: Option<u64>,
}

/// What a line of the hub means for the core.
#[derive(Clone, Debug, PartialEq)]
pub enum Read {
    /// `initialize` answered: the window's welcome, then the hub-wide state
    Welcome(HubEv, Vec<HubEv>),
    /// an event for the window (a notification, a read's result, the
    /// state again after `hub/read`)
    Evs(Vec<HubEv>),
    /// a request refused: the window's `error`
    Error { tag: String, text: String, cid: Option<u64>, reason: Option<String>, kind: Option<ErrorKind> },
    /// a gap or a new epoch: send [`RpcConn::read_again`]
    Resync,
    /// the hub refused this connection (REFUSED, docs/issues/16)
    Refused(String),
    /// an older hub: it doesn't serve `initialize`
    Older,
    Nothing,
}

#[derive(Default)]
pub struct RpcConn {
    next: u64,
    waiting: BTreeMap<u64, Waiting>,
    wm: Watermark,
    /// `initialize` answered on this connection
    pub ready: bool,
}

impl RpcConn {
    /// A new connection's first line.
    pub fn initialize(&mut self, version: &str) -> Value {
        *self = RpcConn::default();
        let params = serde_json::to_value(InitializeParams::new("bise-ambient-core", version)).unwrap_or_default();
        Message::Request(Request::new(Id::Num(INIT), rpc::INITIALIZE, params)).to_value()
    }

    /// `hub/read` (a gap): its result replaces the state.
    pub fn read_again(&mut self) -> Value {
        self.next += 1;
        self.waiting.insert(self.next, Waiting { method: rpc::HUB_READ, tag: String::new(), cid: None });
        Message::Request(Request::new(Id::Num(self.next), rpc::HUB_READ, Value::Object(Default::default()))).to_value()
    }

    /// The window's command (a `HubCmd`'s JSON) as its request; none: no
    /// method for it (hello, an unknown tag).
    pub fn request(&mut self, v: &Value) -> Option<Value> {
        let cmd = HubCmd::from_value(v.clone()).ok()?;
        self.next += 1;
        let req = rpc::request(Id::Num(self.next), &cmd)?;
        let row = rpc::method_row(&req.method)?;
        let cid = v.get("cid").and_then(Value::as_u64);
        self.waiting.insert(self.next, Waiting { method: row.method, tag: row.cmd.to_string(), cid });
        Some(Message::Request(req).to_value())
    }

    /// One line of the hub.
    pub fn read(&mut self, project: &str, v: Value) -> Read {
        if !self.ready {
            // initialize's answer, or an older hub's refusal of its line
            // (bise_proto::rpc::init_answer, the one reading)
            match rpc::init_answer(&v, &Id::Num(INIT)) {
                Init::Ready(res) => return self.welcome(*res),
                Init::Refused(e) => return Read::Refused(e),
                Init::Older => return Read::Older,
                Init::Other => {}
            }
        }
        if !rpc::is_rpc(&v) {
            return Read::Nothing;
        }
        match Message::from_value(v) {
            Ok(Message::Response(r)) => self.response(project, r),
            Ok(Message::Notification(n)) => match rpc::ev(&n) {
                Ok((HubEv::Unknown { .. }, _)) | Err(_) => Read::Nothing,
                Ok((ev, None)) => Read::Evs(vec![ev]),
                Ok((ev, Some(w))) => match self.wm.take(w) {
                    Take::Apply => Read::Evs(vec![ev]),
                    Take::Skip => Read::Nothing,
                    Take::Resync => Read::Resync,
                },
            },
            _ => Read::Nothing,
        }
    }

    /// `initialize` answered: the window's welcome and the hub's state.
    fn welcome(&mut self, res: InitializeResult) -> Read {
        self.ready = true;
        self.wm = res.hub.watermark;
        let mut cmds: Vec<String> = res.methods.iter().filter_map(|m| rpc::method_row(m)).map(|r| r.cmd.to_string()).collect();
        cmds.insert(0, "hello".into());
        let welcome = HubEv::Welcome { project: res.project, proto: res.proto, workspace: res.workspace, name: res.name, cmds };
        Read::Welcome(welcome, state(&res.hub))
    }

    fn response(&mut self, project: &str, r: rpc::Response) -> Read {
        let Some(Id::Num(id)) = r.id else { return Read::Nothing };
        let Some(w) = self.waiting.remove(&id) else { return Read::Nothing };
        if let Some(e) = r.error {
            let data = e.data.unwrap_or_default();
            return Read::Error { tag: w.tag, text: e.message, cid: w.cid, reason: data.reason, kind: data.kind };
        }
        let result = r.result.unwrap_or(Value::Null);
        if w.method == rpc::HUB_READ {
            let Ok(st) = serde_json::from_value::<HubState>(result) else { return Read::Nothing };
            self.wm = st.watermark;
            return Read::Evs(state(&st));
        }
        if w.method == rpc::COMMAND_RUN {
            let notice = serde_json::from_value::<CommandRunResult>(result).ok().and_then(|r| r.notice);
            return match notice {
                Some(text) => Read::Evs(vec![HubEv::Notice { project: project.to_string(), cmd: Some(w.tag), text, cid: w.cid }]),
                None => Read::Nothing,
            };
        }
        match rpc::ev_of_result(w.method, result) {
            Ok(Some(ev)) => Read::Evs(vec![ev]),
            _ => Read::Nothing,
        }
    }
}

/// A request on the home connection (an older hello one: its answers
/// are not read, the hub's events tell what happened): `method` with
/// `params` (an object) and this hub's `project`; the id is the method's
/// name (P3b: his words, a command line, a card's answer, a stop).
pub fn home(project: &str, method: &str, params: Value) -> Value {
    let mut params = if params.is_object() { params } else { Value::Object(Default::default()) };
    params["project"] = Value::String(project.to_string());
    Message::Request(Request::new(Id::Str(method.to_string()), method, params)).to_value()
}

/// The hub-wide state's notifications as the window's events.
fn state(st: &HubState) -> Vec<HubEv> {
    st.state.iter().filter_map(|n| rpc::ev(n).ok()).map(|(e, _)| e).filter(|e| !matches!(e, HubEv::Unknown { .. })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_proto::rpc::{Notification, Response, RpcError};
    use serde_json::json;

    fn init_result(seq: u64) -> Value {
        let agents = rpc::note(&HubEv::Agents { project: "shop".into(), agents: vec![], places: vec![] }, None).unwrap();
        let r = InitializeResult {
            project: "shop".into(),
            proto: 1,
            workspace: "/p/shop".into(),
            name: "shop".into(),
            exe: String::new(),
            state_dir: String::new(),
            version: Value::Null,
            reload: String::new(),
            pages_url: None,
            methods: rpc::methods(),
            notifications: rpc::notifications(),
            hub: HubState { watermark: Watermark { epoch: 9, seq }, state: vec![agents] },
        };
        serde_json::to_value(Response::ok(Id::Num(0), serde_json::to_value(r).unwrap())).unwrap()
    }

    fn note(seq: u64) -> Value {
        let n = rpc::note(&HubEv::Cards { project: "shop".into(), cards: vec![], others: vec![] }, Some(Watermark { epoch: 9, seq })).unwrap();
        serde_json::to_value(n).unwrap()
    }

    #[test]
    fn initialize_is_the_welcome_with_the_methods_tags_then_the_state() {
        let mut c = RpcConn::default();
        assert_eq!(c.initialize("v1")["method"], "initialize");
        let Read::Welcome(HubEv::Welcome { cmds, project, .. }, state) = c.read("shop", init_result(4)) else { panic!() };
        assert_eq!(project, "shop");
        assert!(cmds.contains(&"send".to_string()) && cmds.contains(&"slash".to_string()) && cmds[0] == "hello");
        assert!(matches!(state.as_slice(), [HubEv::Agents { .. }]));
        assert!(c.ready);
    }

    #[test]
    fn notifications_follow_the_watermark_and_a_gap_reads_again() {
        let mut c = RpcConn::default();
        c.initialize("v1");
        c.read("shop", init_result(4));
        assert!(matches!(c.read("shop", note(5)), Read::Evs(_)));
        assert_eq!(c.read("shop", note(5)), Read::Nothing, "seen");
        assert_eq!(c.read("shop", note(7)), Read::Resync, "a gap");
        let req = c.read_again();
        assert_eq!(req["method"], "hub/read");
        let st = HubState { watermark: Watermark { epoch: 9, seq: 7 }, state: vec![] };
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), serde_json::to_value(st).unwrap())).unwrap();
        assert_eq!(c.read("shop", resp), Read::Evs(vec![]));
        assert!(matches!(c.read("shop", note(8)), Read::Evs(_)), "the next one after the read");
        // a thread's notification has no watermark: it goes
        let typing = serde_json::to_value(Notification::new("thread/typing", json!({"project": "shop", "agent": "main", "text": "runs"}))).unwrap();
        assert!(matches!(c.read("shop", typing), Read::Evs(_)));
    }

    #[test]
    fn a_requests_error_keeps_its_cid_reason_and_kind_and_a_read_its_event() {
        let mut c = RpcConn::default();
        c.initialize("v1");
        c.read("shop", init_result(0));
        let req = c.request(&json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": 12})).unwrap();
        assert_eq!(req["method"], "turn/send");
        let id = req["id"].as_u64().unwrap();
        let mut e = RpcError::refused("no agent main", Some("undelivered"));
        e = e.with_kind(ErrorKind::HubOlder);
        let resp = serde_json::to_value(Response::err(Some(Id::Num(id)), e)).unwrap();
        assert_eq!(
            c.read("shop", resp),
            Read::Error { tag: "send".into(), text: "no agent main".into(), cid: Some(12), reason: Some("undelivered".into()), kind: Some(ErrorKind::HubOlder) }
        );
        let req = c.request(&json!({"cmd": "slash", "project": "shop", "agent": "main", "line": "/help", "cid": 3})).unwrap();
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({"notice": "the help"}))).unwrap();
        assert_eq!(c.read("shop", resp), Read::Evs(vec![HubEv::Notice { project: "shop".into(), cmd: Some("slash".into()), text: "the help".into(), cid: Some(3) }]));
        let req = c.request(&json!({"cmd": "scheduled", "project": "shop"})).unwrap();
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({"project": "shop", "items": []}))).unwrap();
        assert_eq!(c.read("shop", resp), Read::Evs(vec![HubEv::Scheduled { project: "shop".into(), items: vec![], ended: vec![] }]));
        assert_eq!(c.request(&json!({"cmd": "hello", "proto": 1})), None);
    }

    #[test]
    fn an_older_hub_and_a_refusal_say_so() {
        let mut c = RpcConn::default();
        c.initialize("v1");
        assert_eq!(c.read("shop", json!({"ok": false, "error": "hub.sock does not serve the op \"\""})), Read::Older);
        let refused = serde_json::to_value(Response::err(Some(Id::Num(0)), RpcError::new(rpc::code::REFUSED, "an agent's process"))).unwrap();
        assert_eq!(c.read("shop", refused), Read::Refused("an agent's process".into()));
    }
}
