//! The terminal's JSON-RPC requests (client-protocol step 3, architect
//! m_13089): its actions go to the hub as typed methods
//! (`bise_proto::rpc`'s tables), on its hello connection, one zone at a
//! time; it still reads the hub's older events until step 4.
//!
//! [`Sb::call`] sends one request (the hub's `project` added) and keeps
//! what to do with its answer ([`Then`]); [`answered`] runs it when the
//! response comes (dispatch's lines with `jsonrpc` and no `method`). A
//! refusal (`error`) shows as the hub's notice did: one info line per
//! line of its words, so the terminal looks the same. Each zone's chunk
//! adds its `Then` variants and their arms; nothing here parses a
//! command.

use super::*;
use bise_proto::rpc::{Id, Message, Request, Response};
use std::collections::BTreeMap;

/// What to do with one request's answer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Then {
    /// nothing but its refusal, shown like a notice (an action whose
    /// change comes in the hub's events)
    Shown,
    /// `/approvals`' read (approvals/set without a mode): the mode, the
    /// checker and the rules, and its screen opens
    Approvals,
    /// a rule out of approvals.toml (approvals/removeRule): the new list
    /// comes in the hub's `approvals` event; a refusal is said on the
    /// screen, or as `/approvals: <why>` when it closed
    RuleRemoved,
}

/// The requests waiting for their answer, by id.
#[derive(Default)]
pub(crate) struct Calls {
    next: u64,
    waiting: BTreeMap<u64, (&'static str, Then)>,
}

impl Calls {
    /// The next id, and `then` kept for it.
    fn start(&mut self, method: &'static str, then: Then) -> u64 {
        self.next += 1;
        self.waiting.insert(self.next, (method, then));
        self.next
    }

    /// A reconnection: the requests sent on the lost connection get no
    /// answer (the hub's events tell what happened).
    pub(crate) fn forget(&mut self) {
        self.waiting.clear();
    }
}

impl Sb {
    /// This hub's id (bise_home's, from the hello's workspace).
    pub(crate) fn project(&self) -> String {
        bise_home::hub_id(std::path::Path::new(&self.workspace))
    }

    /// One request: `method` (a row of `bise_proto::rpc::METHODS`) with
    /// `params` (an object; `project` is added), its answer to `then`.
    pub(crate) fn call(&mut self, method: &'static str, params: Value, then: Then) {
        let mut params = if params.is_object() { params } else { json!({}) };
        params["project"] = json!(self.project());
        let id = self.rpc.start(method, then);
        let req = Message::Request(Request::new(Id::Num(id), method, params));
        self.send(req.to_value());
    }
}

/// A response line from the hub: its request's [`Then`]. Not one of
/// ours (another id, a lost connection's): nothing.
pub(super) fn answered(app: &mut App, v: Value) {
    let Ok(Message::Response(r)) = Message::from_value(v) else { return };
    let Some(Id::Num(id)) = r.id.clone() else { return };
    let Some((_method, then)) = app.sb.rpc.waiting.remove(&id) else { return };
    run(app, then, r);
}

fn run(app: &mut App, then: Then, r: Response) {
    let result = match r.error {
        Some(e) => Err(e.message),
        None => Ok(r.result.unwrap_or(Value::Null)),
    };
    match (then, result) {
        (Then::RuleRemoved, Err(e)) => rule_refused(app, &e),
        (_, Err(e)) => refused(app, &e),
        (Then::Shown | Then::RuleRemoved, Ok(_)) => {}
        (Then::Approvals, Ok(mut v)) => {
            v["show"] = json!(true);
            approvals_event(app, &v);
        }
    }
}

/// The hub's refusal, as its notice showed it.
fn refused(app: &mut App, text: &str) {
    for l in text.lines() {
        push_event(&mut app.events, &mut app.cache, Ev::Info(l.to_string()));
    }
}

/// A rule the hub could not remove: why, on the screen when it is open.
fn rule_refused(app: &mut App, why: &str) {
    match app.approvals.as_mut() {
        Some(s) => s.said = Some(why.to_string()),
        None => {
            push_event(&mut app.events, &mut app.cache, Ev::Warn(format!("/approvals: {why}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_is_a_request_with_the_project_and_its_answer_runs_once() {
        let mut app = crate::sb::bench::test_app();
        app.sb.workspace = "/tmp/acme".into();
        app.sb.call("card/close", json!({"card": 3}), Then::Shown);
        assert_eq!(app.sb.rpc.waiting.len(), 1);
        let (id, (method, _)) = app.sb.rpc.waiting.iter().next().map(|(k, v)| (*k, v.clone())).unwrap();
        assert_eq!(method, "card/close");
        let n = app.events.len();
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32011, "message": "card 3 isn't open"}}));
        assert!(app.sb.rpc.waiting.is_empty());
        assert_eq!(app.events.len(), n + 1, "the refusal shows like the hub's notice");
        // a second answer to the same id, or another id: nothing
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {}}));
        answered(&mut app, json!({"jsonrpc": "2.0", "id": 999, "result": {}}));
        assert_eq!(app.events.len(), n + 1);
    }

    /// The id of the one waiting request.
    fn waiting(app: &App) -> u64 {
        *app.sb.rpc.waiting.keys().next().unwrap()
    }

    #[test]
    fn approvals_read_opens_its_screen_and_a_refused_removal_says_why_as_before() {
        let mut app = crate::sb::bench::test_app();
        app.sb.call("approvals/set", json!({}), Then::Approvals);
        let id = waiting(&app);
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {"mode": "auto", "env": false, "checker": "off",
            "checker_who": "", "repo": "/r", "rules": [{"tool": "bash", "pattern": "cargo test *", "what": "cargo test *"}]}}));
        assert!(app.approvals.is_some(), "the read opens /approvals");
        assert_eq!((app.sb.approvals.mode.as_str(), app.sb.approvals.rules.len()), ("auto", 1));
        // the screen open: the refusal is said there
        app.sb.call("approvals/removeRule", json!({"rule": {"tool": "bash"}}), Then::RuleRemoved);
        let (id, n) = (waiting(&app), app.events.len());
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32011, "message": "gone already"}}));
        assert_eq!(app.approvals.as_ref().unwrap().said.as_deref(), Some("gone already"));
        assert_eq!(app.events.len(), n);
        // closed: a warning, as the approvals event with an error showed it
        app.approvals = None;
        app.sb.call("approvals/removeRule", json!({"rule": {"tool": "bash"}}), Then::RuleRemoved);
        let id = waiting(&app);
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32011, "message": "gone already"}}));
        assert!(matches!(app.events.last(), Some(Ev::Warn(w)) if w == "/approvals: gone already"));
        // done: nothing here (the hub's approvals event has the new list)
        app.sb.call("approvals/removeRule", json!({"rule": {"tool": "bash"}}), Then::RuleRemoved);
        let (id, n) = (waiting(&app), app.events.len());
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {}}));
        assert_eq!(app.events.len(), n);
    }
}
