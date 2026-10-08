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
    if let Some(e) = r.error {
        refused(app, &e.message);
        return;
    }
    match then {
        Then::Shown => {}
    }
}

/// The hub's refusal, as its notice showed it.
fn refused(app: &mut App, text: &str) {
    for l in text.lines() {
        push_event(&mut app.events, &mut app.cache, Ev::Info(l.to_string()));
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
}
