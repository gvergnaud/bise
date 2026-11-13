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
//! - the app's own request for a command (`hub/<tag>`, its id kept in
//!   `Waiting`) is answered with that response (architect m_23524): the
//!   result as is (card/answer's `AnswerResult`, turn/send's `SendResult`),
//!   `{}` when it came as an event, the hub's error as is; the connection
//!   lost ([`GONE`]) or no answer in [`ASK_LATE`] ([`LATE`]): an error,
//!   its reason typed in `data.reason` ([`REASON_LATE`], [`REASON_GONE`]),
//!   never a request left waiting ([`RpcConn::take_replies`]);
//! - a send answered [`LATE`] is kept for its hub's own response (the hub
//!   may still make his message): that response, its connection's loss or
//!   [`SEND_KEPT`] settles it as a [`LateSend`] ([`RpcConn::take_late_sends`],
//!   the core's `send_answered`), so his row never stays 'sending…'
//!   (queue-drawer-fix, architect m_34889);
//! - a hub that doesn't serve `initialize` (from before client-protocol,
//!   v2026.10.2-30) is [`Read::NotRpc`]: the connection is refused with
//!   `bise_proto::rpc::HUB_OLDER`'s words, nothing more is said to it.

use bise_proto::hub::{ErrorKind, HubCmd, HubEv};
use bise_proto::rpc::{self, CommandRunResult, HubState, Id, Init, InitializeParams, InitializeResult, Message, Request, Take, Watermark};
use bise_proto::rpc::{Response, RpcError};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

/// `initialize`'s id on every connection (the window's requests start at 1).
const INIT: u64 = 0;

/// An app request's error when its hub's connection went before it answered.
pub const GONE: &str = "its hub went away before it answered. try again in a moment.";
/// An app request's error when its hub didn't answer in [`ASK_LATE`].
pub const LATE: &str = "its hub didn't answer. try again in a moment.";
/// How long an app request waits for its hub's response.
pub const ASK_LATE: std::time::Duration = std::time::Duration::from_secs(30);
/// How long a send answered [`LATE`] waits for its hub's own response
/// (architect m_34889): after it, the core says it gone.
pub const SEND_KEPT: std::time::Duration = std::time::Duration::from_secs(600);
/// How long a view's `view/call` waits for its hub (mcp-apps C5, d3): a
/// model-visible call may wait on his card, so it waits as long as the hub
/// can take to answer one (`view_rpc::VIEW_CALL_MAX_MS`, derived from the
/// card's one deadline); the hub always answers before.
pub const VIEW_CALL_KEPT: std::time::Duration = std::time::Duration::from_millis(bise_proto::view_rpc::VIEW_CALL_MAX_MS);

/// Why the core answered an app request with an error of its own, typed
/// in the error's `data.reason` (the window never reads the words).
pub const REASON_LATE: &str = "late";
pub const REASON_GONE: &str = "gone";

/// A send the core answered [`LATE`], settled by its hub at last
/// (queue-drawer-fix, architect m_34889): its `SendResult`, or none when
/// its connection went or [`SEND_KEPT`] passed first (gone). The core
/// says it as `CoreEv::SendAnswered`, so his row meets its echo by the
/// hub's message id and never stays 'sending…'.
#[derive(Clone, Debug, PartialEq)]
pub struct LateSend {
    pub agent: String,
    pub cid: u64,
    pub result: Option<rpc::SendResult>,
}

fn core_err(why: &str, reason: &str) -> RpcError {
    RpcError { data: Some(rpc::ErrorData { reason: Some(reason.to_string()), ..Default::default() }), ..RpcError::new(rpc::code::REFUSED, why) }
}

/// One request waiting for its response.
#[derive(Clone, Debug, PartialEq)]
struct Waiting {
    method: &'static str,
    tag: String,
    cid: Option<u64>,
    /// a send's agent (none for any other request)
    agent: Option<String>,
    /// a send answered [`LATE`] already, kept for its hub's own response
    late: bool,
    /// the app's request it answers (none: the core's own, a subscribe
    /// at a welcome, voice mode's)
    app: Option<Id>,
    at: Instant,
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
    Error { tag: String, text: String, cid: Option<u64>, reason: Option<String>, kind: Option<ErrorKind>, tip: Option<String> },
    /// a gap or a new epoch: send [`RpcConn::read_again`]
    Resync,
    /// the hub refused this connection (REFUSED, docs/issues/16)
    Refused(String),
    /// this hub doesn't serve `initialize` (`bise_proto::rpc::Init::NotRpc`)
    NotRpc,
    Nothing,
}

#[derive(Default)]
pub struct RpcConn {
    next: u64,
    waiting: BTreeMap<u64, Waiting>,
    /// the app's requests answered, for the core to write ([`RpcConn::take_replies`])
    replies: Vec<Response>,
    /// sends answered late, settled since ([`RpcConn::take_late_sends`])
    late_sends: Vec<LateSend>,
    wm: Watermark,
    /// `initialize` answered on this connection
    pub ready: bool,
    /// the hub's page server (`initialize`'s), if it serves pages
    pub pages_url: Option<String>,
}

impl RpcConn {
    /// A new connection's first line.
    pub fn initialize(&mut self, version: &str) -> Value {
        self.lost(GONE);
        let replies = std::mem::take(&mut self.replies);
        let late_sends = std::mem::take(&mut self.late_sends);
        *self = RpcConn { replies, late_sends, ..RpcConn::default() };
        let params = serde_json::to_value(InitializeParams::new("bise-ambient-core", version).of_kind(rpc::ClientKind::Desktop)).unwrap_or_default();
        Message::Request(Request::new(Id::Num(INIT), rpc::INITIALIZE, params)).to_value()
    }

    /// `hub/read` (a gap): its result replaces the state.
    pub fn read_again(&mut self) -> Value {
        self.next += 1;
        self.waiting.insert(self.next, Waiting { method: rpc::HUB_READ, tag: String::new(), cid: None, agent: None, late: false, app: None, at: Instant::now() });
        Message::Request(Request::new(Id::Num(self.next), rpc::HUB_READ, Value::Object(Default::default()))).to_value()
    }

    /// The window's command (a `HubCmd`'s JSON) as its request; none: no
    /// method for it (an unknown tag). `app`: the app's own request for
    /// it, answered with the hub's response ([`RpcConn::take_replies`]).
    pub fn request(&mut self, v: &Value, app: Option<Id>, now: Instant) -> Option<Value> {
        let cmd = HubCmd::from_value(v.clone()).ok()?;
        self.next += 1;
        let req = rpc::request(Id::Num(self.next), &cmd)?;
        let row = rpc::method_row(&req.method)?;
        let cid = v.get("cid").and_then(Value::as_u64);
        let agent = if row.cmd == "send" { v.get("agent").and_then(Value::as_str).map(String::from) } else { None };
        self.waiting.insert(self.next, Waiting { method: row.method, tag: row.cmd.to_string(), cid, agent, late: false, app, at: now });
        Some(Message::Request(req).to_value())
    }

    /// The app's requests answered since the last call, in order.
    pub fn take_replies(&mut self) -> Vec<Response> {
        std::mem::take(&mut self.replies)
    }

    /// The sends answered late that their hub settled since the last call.
    pub fn take_late_sends(&mut self) -> Vec<LateSend> {
        std::mem::take(&mut self.late_sends)
    }

    /// A late send kept: its end, `result` none for gone.
    fn settle_late(&mut self, w: Waiting, result: Option<rpc::SendResult>) {
        if let (Some(agent), Some(cid)) = (w.agent, w.cid) {
            self.late_sends.push(LateSend { agent, cid, result });
        }
    }

    /// The connection is lost (its hub went, a new `initialize`, the core
    /// lets it go): every app request still waiting on it is answered with
    /// `why` (the hub never said; it may or may not have acted).
    pub fn lost(&mut self, why: &str) {
        for (_, w) in std::mem::take(&mut self.waiting) {
            if w.late {
                self.settle_late(w, None);
            } else if let Some(app) = w.app {
                self.replies.push(Response::err(Some(app), core_err(why, REASON_GONE)));
            }
        }
    }

    /// The app requests waiting since before `before` get `why` as their
    /// error (reason [`REASON_LATE`]). Their hub's late response is
    /// dropped, but a send's: the hub may still make his message, so it is
    /// kept until that response ([`LateSend`]), its connection's loss, or
    /// [`SEND_KEPT`] after it was asked (then gone).
    pub fn late(&mut self, before: Instant, why: &str) {
        // a view's call: its own wait (VIEW_CALL_KEPT, not ASK_LATE)
        let view_before = before.checked_sub(VIEW_CALL_KEPT.saturating_sub(ASK_LATE));
        let past = |w: &Waiting| match w.method {
            "view/call" => view_before.is_some_and(|b| w.at < b),
            _ => w.at < before,
        };
        let late: Vec<u64> = self.waiting.iter().filter(|(_, w)| w.app.is_some() && past(w)).map(|(id, _)| *id).collect();
        for id in late {
            let Some(mut w) = self.waiting.remove(&id) else { continue };
            if let Some(app) = w.app.take() {
                self.replies.push(Response::err(Some(app), core_err(why, REASON_LATE)));
            }
            if w.agent.is_some() && w.cid.is_some() {
                w.late = true;
                self.waiting.insert(id, w);
            }
        }
        // the kept sends past their bound: gone
        let Some(bound) = before.checked_sub(SEND_KEPT.saturating_sub(ASK_LATE)) else { return };
        let gone: Vec<u64> = self.waiting.iter().filter(|(_, w)| w.late && w.at < bound).map(|(id, _)| *id).collect();
        for id in gone {
            if let Some(w) = self.waiting.remove(&id) {
                self.settle_late(w, None);
            }
        }
    }

    /// One line of the hub.
    pub fn read(&mut self, project: &str, v: Value) -> Read {
        if !self.ready {
            // initialize's answer, or a hub that doesn't serve it
            // (bise_proto::rpc::init_answer, the one reading)
            // anything else is a line of the connection before it (the
            // core let it go, its lines were on their way; hub-older-race):
            // never read here, it would ask `hub/read` before `initialize`
            // and the hub would answer 'does not serve the op' (NotRpc)
            return match rpc::init_answer(&v, &Id::Num(INIT)) {
                Init::Ready(res) => self.welcome(*res),
                Init::Refused(e) => Read::Refused(e),
                Init::NotRpc => Read::NotRpc,
                Init::Other => Read::Nothing,
            };
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
        self.pages_url = res.pages_url.clone();
        let cmds: Vec<String> = res.methods.iter().filter_map(|m| rpc::method_row(m)).map(|r| r.cmd.to_string()).collect();
        let built = res.version.get("built").and_then(Value::as_str).map(str::trim).filter(|b| !b.is_empty()).map(String::from);
        let welcome = HubEv::Welcome { project: res.project, proto: res.proto, workspace: res.workspace, name: res.name, cmds, built };
        Read::Welcome(welcome, state(&res.hub))
    }

    fn response(&mut self, project: &str, r: rpc::Response) -> Read {
        let Some(Id::Num(id)) = r.id else { return Read::Nothing };
        let Some(w) = self.waiting.remove(&id) else { return Read::Nothing };
        if let Some(e) = r.error {
            // the app's request gets the hub's refusal as is (its words, reason, kind)
            if let Some(app) = w.app.clone() {
                self.replies.push(Response::err(Some(app), e.clone()));
            }
            let data = e.data.unwrap_or_default();
            return Read::Error { tag: w.tag, text: e.message, cid: w.cid, reason: data.reason, kind: data.kind, tip: data.tip };
        }
        let result = r.result.unwrap_or(Value::Null);
        let read = self.result(project, &w, result.clone());
        if w.late {
            // a send answered late: its hub's result at last (an older hub's
            // {} reads as a SendResult with neither field, as the window's 'sent')
            let sr = serde_json::from_value::<rpc::SendResult>(result).unwrap_or(rpc::SendResult { msg_id: None, reason: None });
            self.settle_late(w, Some(sr));
            return read;
        }
        // the app's request gets the hub's result as is (card/answer's
        // AnswerResult, turn/send's SendResult), or `{}` when it came as an
        // event already (a thread's page, command/run's words): never twice
        if let Some(app) = w.app {
            let r = if matches!(read, Read::Nothing) && w.method != rpc::COMMAND_RUN { result } else { Value::Object(Default::default()) };
            self.replies.push(Response::ok(app, r));
        }
        read
    }

    fn result(&mut self, project: &str, w: &Waiting, result: Value) -> Read {
        if w.method == rpc::HUB_READ {
            let Ok(st) = serde_json::from_value::<HubState>(result) else { return Read::Nothing };
            self.wm = st.watermark;
            return Read::Evs(state(&st));
        }
        if w.method == rpc::COMMAND_RUN {
            let notice = serde_json::from_value::<CommandRunResult>(result).ok().and_then(|r| r.notice);
            return match notice {
                Some(text) => Read::Evs(vec![HubEv::Notice { project: project.to_string(), cmd: Some(w.tag.clone()), text, cid: w.cid }]),
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
        let agents = rpc::note(&HubEv::Agents { project: "shop".into(), agents: vec![], places: vec![], archived: Some(vec![]) }, None).unwrap();
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
        assert!(cmds.contains(&"send".to_string()) && cmds.contains(&"slash".to_string()) && !cmds.contains(&"hello".to_string()));
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
        let req = c.request(&json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": 12}), None, Instant::now()).unwrap();
        assert_eq!(req["method"], "turn/send");
        let id = req["id"].as_u64().unwrap();
        let mut e = RpcError::refused("no agent main", Some("undelivered"));
        e = e.with_kind(ErrorKind::HubOlder).with_tip(Some("looked in /usr/bin"));
        let resp = serde_json::to_value(Response::err(Some(Id::Num(id)), e)).unwrap();
        assert_eq!(
            c.read("shop", resp),
            Read::Error { tag: "send".into(), text: "no agent main".into(), cid: Some(12), reason: Some("undelivered".into()), kind: Some(ErrorKind::HubOlder), tip: Some("looked in /usr/bin".into()) }
        );
        let req = c.request(&json!({"cmd": "slash", "project": "shop", "agent": "main", "line": "/help", "cid": 3}), None, Instant::now()).unwrap();
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({"notice": "the help"}))).unwrap();
        assert_eq!(c.read("shop", resp), Read::Evs(vec![HubEv::Notice { project: "shop".into(), cmd: Some("slash".into()), text: "the help".into(), cid: Some(3) }]));
        let req = c.request(&json!({"cmd": "scheduled", "project": "shop"}), None, Instant::now()).unwrap();
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({"project": "shop", "items": []}))).unwrap();
        assert_eq!(c.read("shop", resp), Read::Evs(vec![HubEv::Scheduled { project: "shop".into(), items: vec![], ended: vec![] }]));
        assert_eq!(c.request(&json!({"cmd": "hello", "proto": 1}), None, Instant::now()), None);
    }

    /// architect m_23524: the app's request is answered once, with its
    /// hub's own response: the result as is, the refusal with its words
    /// and reason, an error when the hub goes or never answers.
    #[test]
    fn an_app_request_gets_its_hubs_own_answer_once() {
        let mut c = RpcConn::default();
        c.initialize("v1");
        c.read("shop", init_result(0));
        let t0 = Instant::now();
        let answer = json!({"cmd": "answer", "project": "shop", "card": 384, "reply": "1"});
        let req = c.request(&answer, Some(Id::Num(7)), t0).unwrap();
        assert_eq!(req["method"], "card/answer");
        assert!(c.take_replies().is_empty(), "nothing before its hub said");
        let got = json!({"answer": {"agent": "perf", "question": "merge?", "said": "1", "pick": "yes", "notice": "you answered perf"}});
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), got.clone())).unwrap();
        assert_eq!(c.read("shop", resp), Read::Nothing);
        assert_eq!(c.take_replies(), vec![Response::ok(Id::Num(7), got)], "card/answer's AnswerResult as is");
        // refused: the hub's words and reason to that very request, and the window's error event
        let req = c.request(&json!({"cmd": "close", "project": "shop", "card": 12}), Some(Id::Num(8)), t0).unwrap();
        let e = RpcError::refused("no card 12", Some("closed"));
        let resp = serde_json::to_value(Response::err(Some(Id::Num(req["id"].as_u64().unwrap())), e.clone())).unwrap();
        assert!(matches!(c.read("shop", resp), Read::Error { .. }));
        assert_eq!(c.take_replies(), vec![Response::err(Some(Id::Num(8)), e)]);
        // a result that came as an event: `{}`, never the page twice
        let req = c.request(&json!({"cmd": "scheduled", "project": "shop"}), Some(Id::Num(9)), t0).unwrap();
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({"project": "shop", "items": []}))).unwrap();
        assert!(matches!(c.read("shop", resp), Read::Evs(_)));
        assert_eq!(c.take_replies(), vec![Response::ok(Id::Num(9), json!({}))]);
        // the core's own request (a subscribe at a welcome): no reply
        let req = c.request(&json!({"cmd": "close", "project": "shop", "card": 1}), None, t0).unwrap();
        c.read("shop", serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({}))).unwrap());
        assert!(c.take_replies().is_empty());
        // its hub never answers: an error at ASK_LATE, and the late response is dropped
        let req = c.request(&answer, Some(Id::Num(10)), t0).unwrap();
        c.late(t0, LATE);
        assert!(c.take_replies().is_empty(), "not late yet");
        c.late(t0 + ASK_LATE + std::time::Duration::from_millis(1), LATE);
        let r = c.take_replies();
        assert_eq!((r.len(), r[0].id.clone(), r[0].error.as_ref().map(|e| e.message.as_str())), (1, Some(Id::Num(10)), Some(LATE)));
        let resp = serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({}))).unwrap();
        assert_eq!(c.read("shop", resp), Read::Nothing);
        assert!(c.take_replies().is_empty(), "answered once");
        // its hub goes (or a new initialize): every waiting app request gets GONE
        c.request(&answer, Some(Id::Num(11)), t0).unwrap();
        c.initialize("v1");
        let r = c.take_replies();
        assert_eq!((r.len(), r[0].id.clone(), r[0].error.as_ref().map(|e| e.message.as_str())), (1, Some(Id::Num(11)), Some(GONE)));
    }

    #[test]
    fn a_hub_without_initialize_and_a_refusal_say_so() {
        let mut c = RpcConn::default();
        c.initialize("v1");
        assert_eq!(c.read("shop", json!({"ok": false, "error": "hub.sock does not serve the op \"\""})), Read::NotRpc);
        let refused = serde_json::to_value(Response::err(Some(Id::Num(0)), RpcError::new(rpc::code::REFUSED, "an agent's process"))).unwrap();
        assert_eq!(c.read("shop", refused), Read::Refused("an agent's process".into()));
    }

    fn send_req(c: &mut RpcConn, cid: u64, app: u64, t: Instant) -> u64 {
        let req = c.request(&json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": cid}), Some(Id::Num(app)), t).unwrap();
        req["id"].as_u64().unwrap()
    }

    #[test]
    fn a_send_answered_late_is_kept_for_its_hubs_answer_then_gone_at_the_bound() {
        let ms = std::time::Duration::from_millis;
        let t0 = Instant::now();
        let mut c = RpcConn::default();
        c.initialize("v1");
        c.read("shop", init_result(4));
        // the hub answers the send 40 s later: LATE (typed) at 30 s, then its msg_id, once
        let id = send_req(&mut c, 12, 5, t0);
        c.late(t0 + ASK_LATE + ms(1), LATE);
        let r = c.take_replies();
        let e = r[0].error.clone().unwrap();
        assert_eq!((r.len(), r[0].id.clone(), e.data.and_then(|d| d.reason)), (1, Some(Id::Num(5)), Some(REASON_LATE.to_string())), "the reason typed, never the words");
        assert!(c.take_late_sends().is_empty(), "not settled yet");
        let resp = serde_json::to_value(Response::ok(Id::Num(id), json!({"msg_id": 41, "reason": "made"}))).unwrap();
        c.read("shop", resp.clone());
        assert!(c.take_replies().is_empty(), "the app's request was answered once");
        let made = rpc::SendResult { msg_id: Some(41), reason: Some(rpc::SendReason::Made) };
        assert_eq!(c.take_late_sends(), vec![LateSend { agent: "main".into(), cid: 12, result: Some(made) }]);
        c.read("shop", resp);
        assert!(c.take_late_sends().is_empty(), "settled once");
        // a hub that never answers: nothing kept after SEND_KEPT, said gone once
        send_req(&mut c, 13, 6, t0);
        c.late(t0 + ASK_LATE + ms(1), LATE);
        c.take_replies();
        // (`late`'s `before` is the core's now less ASK_LATE, hubs.rs)
        c.late(t0 + SEND_KEPT - ASK_LATE - ms(1), LATE);
        assert!(c.take_late_sends().is_empty(), "within the bound");
        c.late(t0 + SEND_KEPT - ASK_LATE + ms(1), LATE);
        assert_eq!(c.take_late_sends(), vec![LateSend { agent: "main".into(), cid: 13, result: None }]);
        assert!(c.waiting.is_empty(), "nothing left in the map after the bound");
        // its connection goes first: gone; a request still waiting gets GONE, typed gone
        send_req(&mut c, 14, 7, t0);
        c.late(t0 + ASK_LATE + ms(1), LATE);
        c.take_replies();
        send_req(&mut c, 15, 8, t0 + ASK_LATE);
        c.lost(GONE);
        assert_eq!(c.take_late_sends(), vec![LateSend { agent: "main".into(), cid: 14, result: None }]);
        let r = c.take_replies();
        assert_eq!((r.len(), r[0].id.clone(), r[0].error.clone().and_then(|e| e.data).and_then(|d| d.reason)), (1, Some(Id::Num(8)), Some(REASON_GONE.to_string())));
        assert!(c.waiting.is_empty());
        // a late card answer is still dropped, never kept
        let req = c.request(&json!({"cmd": "answer", "project": "shop", "card": 3, "reply": "1"}), Some(Id::Num(9)), t0).unwrap();
        c.late(t0 + ASK_LATE + ms(1), LATE);
        c.take_replies();
        assert!(c.waiting.is_empty());
        c.read("shop", serde_json::to_value(Response::ok(Id::Num(req["id"].as_u64().unwrap()), json!({}))).unwrap());
        assert!(c.take_late_sends().is_empty() && c.take_replies().is_empty());
    }

    #[test]
    fn a_views_call_waits_for_his_card_not_ask_late() {
        let ms = std::time::Duration::from_millis;
        let t0 = Instant::now();
        let mut c = RpcConn::default();
        c.initialize("v1");
        c.read("shop", init_result(4));
        let call = json!({"cmd": "view_call", "project": "shop", "agent": "main", "call": "1-1", "method": "tools/call", "params": {"name": "bump"}});
        let req = c.request(&call, Some(Id::Num(5)), t0).unwrap();
        assert_eq!(req["method"], "view/call");
        // a card answered at 5 min: still waiting past ASK_LATE
        c.late(t0 + ASK_LATE + ms(1), LATE);
        assert!(c.take_replies().is_empty(), "a view's call outlives ASK_LATE");
        // (`late`'s `before` is the core's now less ASK_LATE, hubs.rs)
        c.late(t0 + VIEW_CALL_KEPT - ASK_LATE - ms(1), LATE);
        assert!(c.take_replies().is_empty(), "within VIEW_CALL_KEPT");
        c.late(t0 + VIEW_CALL_KEPT - ASK_LATE + ms(1), LATE);
        let r = c.take_replies();
        assert_eq!((r.len(), r[0].id.clone()), (1, Some(Id::Num(5))), "LATE past VIEW_CALL_KEPT");
        assert!(c.waiting.is_empty());
    }
}
