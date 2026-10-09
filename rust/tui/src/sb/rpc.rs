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
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// What to do with one request's answer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Then {
    /// the hub's words (a `CommandRunResult`'s notice: version/info,
    /// switch, rollback, restart; `/artifacts add`'s artifacts/add),
    /// shown as its notice was
    Said,
    /// `versions/list`: the `/version` picker's rows (sb/versions.rs)
    Versions,
    /// `release/plan`: the plan and its y/n, or why there is none
    /// (sb/release.rs)
    Release,
    /// `prs/list` (`/prs`): its head and rows in the feed, or its words
    /// when no PR is open
    Prs,
    /// `diff/read` for the panel's ask `req`: its diff, or its refusal
    /// as the panel's failure line (diffwire.rs)
    Diff(u64),
    /// `branches/list`: the `/diff` picker's rows (diffbranches.rs)
    Branches,
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
    /// what he typed or sent (command/run, turn/send, card/*): the
    /// hub's words (its refusal, or a CommandRunResult's notice) show
    /// as its notice did, and the queue moves on as on a notice (a
    /// refused queued message starts no turn: the 766a28a6 guard)
    Line,
}

/// The requests waiting for their answer, by id (cells: a popup asks
/// for its list while it draws, from a shared borrow).
#[derive(Default)]
pub(crate) struct Calls {
    next: Cell<u64>,
    waiting: RefCell<BTreeMap<u64, (&'static str, Then)>>,
}

impl Calls {
    /// The next id, and `then` kept for it.
    fn start(&self, method: &'static str, then: Then) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.waiting.borrow_mut().insert(id, (method, then));
        id
    }

    /// A reconnection: the requests sent on the lost connection get no
    /// answer (the hub's events tell what happened).
    pub(crate) fn forget(&mut self) {
        self.waiting.get_mut().clear();
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
        self.call_shared(method, params, then);
    }

    /// [`Sb::call`] from a shared borrow (a popup asking the hub for its
    /// list while it draws: `/version`'s, `/diff`'s branches).
    pub(crate) fn call_shared(&self, method: &'static str, params: Value, then: Then) {
        let mut params = if params.is_object() { params } else { json!({}) };
        params["project"] = json!(self.project());
        let id = self.rpc.start(method, then);
        // G: a send's own id, echoed on its error with why (undelivered:
        // the hub wrote its line in the thread), as the window's cid
        if matches!(method, "command/run" | "turn/send") {
            params["cid"] = json!(id);
        }
        let req = Message::Request(Request::new(Id::Num(id), method, params));
        self.send_shared(req.to_value());
    }
}

/// A response line from the hub: its request's [`Then`]. Not one of
/// ours (another id, a lost connection's): nothing.
pub(super) fn answered(app: &mut App, v: Value) {
    let Ok(Message::Response(r)) = Message::from_value(v) else { return };
    let Some(Id::Num(id)) = r.id.clone() else { return };
    let Some((_method, then)) = app.sb.rpc.waiting.get_mut().remove(&id) else { return };
    run(app, then, r);
}

fn run(app: &mut App, then: Then, r: Response) {
    // BISE-86: refused because the hub wrote its undelivered line: its
    // words go above his line ([`above_undelivered`])
    let undelivered = r.error.as_ref().and_then(|e| e.data.as_ref()).and_then(|d| d.reason.as_deref()) == Some("undelivered");
    let result = match r.error {
        Some(e) => Err(e.message),
        None => Ok(r.result.unwrap_or(Value::Null)),
    };
    match (then, result) {
        (Then::Said, Ok(v)) => said(app, &v),
        (Then::Versions, Ok(v)) => versions::answered(app, &v),
        (Then::Release, Ok(v)) => release::event(app, &v),
        (Then::Prs, Ok(v)) => prs(app, &v),
        (Then::Diff(req), Ok(v)) => crate::diffview::answered(app, req, crate::diffwire::of(&v)),
        (Then::Diff(req), Err(e)) => crate::diffview::answered(app, req, crate::diffwire::refused(&e)),
        (Then::Branches, Ok(v)) => crate::diffbranches::branches_event(&v),
        (Then::RuleRemoved, Err(e)) => rule_refused(app, &e),
        (Then::Line, Err(e)) => {
            crate::queue::seen(app);
            match undelivered.then(|| above_undelivered(app)).flatten() {
                Some(at) => {
                    for (k, l) in e.lines().enumerate() {
                        app.events.insert(at + k, Ev::Info(l.to_string()));
                        app.cache.insert(at + k, None);
                    }
                }
                None => refused(app, &e),
            }
        }
        (_, Err(e)) => refused(app, &e),
        (Then::Shown | Then::RuleRemoved, Ok(_)) => {}
        (Then::Approvals, Ok(v)) => {
            if let Ok(Some(ev)) = bise_proto::rpc::ev_of_result("approvals/set", v) {
                hub_reads::approvals(app, &ev, true);
            }
        }
        (Then::Line, Ok(v)) => {
            let said = serde_json::from_value::<bise_proto::rpc::CommandRunResult>(v).ok().and_then(|c| c.notice);
            if let Some(text) = said {
                crate::queue::seen(app);
                refused(app, &text);
            }
        }
    }
}

/// `prs/list`'s answer (`HubEv::Prs`'s fields): no PR, its words as the
/// hub's notice showed them; else the dim head and a PR line per row.
fn prs(app: &mut App, result: &Value) {
    if let Some(none) = result.get("none").and_then(Value::as_str) {
        crate::queue::seen(app);
        return refused(app, none);
    }
    for e in prs_events(result) {
        push_event(&mut app.events, &mut app.cache, e);
    }
}

/// A `CommandRunResult`'s words, as the hub's `notice` event showed
/// them (sb.rs's `notice` arm, the queue's mark included).
fn said(app: &mut App, result: &Value) {
    let Some(text) = result.get("notice").and_then(Value::as_str) else { return };
    crate::queue::seen(app);
    refused(app, text);
}

/// Where an undelivered send's refusal goes (BISE-86): above his line
/// that the open `not delivered` question is about, where the hub's
/// notice showed before (it came before the undelivered line; ⏎ sends
/// again only while the question is the last row). None: no question.
fn above_undelivered(app: &App) -> Option<usize> {
    let ask = app.events.iter().rposition(|x| matches!(x, Ev::Undelivered { open: true, .. }))?;
    Some(app.events[..ask].iter().rposition(|x| matches!(x, Ev::You(..))).unwrap_or(ask))
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
        assert_eq!(app.sb.rpc.waiting.borrow().len(), 1);
        let (id, (method, _)) = app.sb.rpc.waiting.borrow().iter().next().map(|(k, v)| (*k, v.clone())).unwrap();
        assert_eq!(method, "card/close");
        let n = app.events.len();
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32011, "message": "card 3 isn't open"}}));
        assert!(app.sb.rpc.waiting.borrow().is_empty());
        assert_eq!(app.events.len(), n + 1, "the refusal shows like the hub's notice");
        // a second answer to the same id, or another id: nothing
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {}}));
        answered(&mut app, json!({"jsonrpc": "2.0", "id": 999, "result": {}}));
        assert_eq!(app.events.len(), n + 1);
    }

    /// The id of the one waiting request.
    fn waiting(app: &App) -> u64 {
        *app.sb.rpc.waiting.borrow().keys().next().unwrap()
    }

    #[test]
    fn the_hubs_words_show_as_its_notice_did() {
        let mut app = crate::sb::bench::test_app();
        app.sb.call("artifacts/add", json!({"agent": "main", "target": "https://x.dev/6"}), Then::Said);
        let (id, n) = (waiting(&app), app.events.len());
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {"notice": "↗ added: PR #6"}}));
        assert_eq!(app.events.len(), n + 1);
        assert!(matches!(app.events.last(), Some(Ev::Info(t)) if t == "↗ added: PR #6"));
        app.sb.call("artifacts/add", json!({"agent": "main", "target": "nothing.md"}), Then::Said);
        let id = waiting(&app);
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32011, "message": "no file or link at nothing.md."}}));
        assert!(matches!(app.events.last(), Some(Ev::Info(t)) if t == "no file or link at nothing.md."));
    }

    /// `/prs` asks prs/list (it was a command/run whose One `prs` event a
    /// hello with reads swallowed, P4c-1): its head and a PR line per row
    /// as the older prs event drew them; no PR, its words.
    #[test]
    fn prs_shows_its_rows_or_its_words() {
        let mut app = crate::sb::bench::test_app();
        app.sb.workspace = "/tmp/acme".into();
        let _ = handle_input(&mut app, "/prs");
        let (id, (method, then)) = app.sb.rpc.waiting.borrow().iter().next().map(|(k, v)| (*k, v.clone())).unwrap();
        assert_eq!((method, then), ("prs/list", Then::Prs));
        let n = app.events.len();
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {"project": "acme", "head": "1 PR open", "items": [
            {"number": 12, "url": "https://github.com/a/b/pull/12", "branch": "sb/login", "agents": ["login"], "state": "open",
             "checks": "fail", "review": "none", "words": "checks fail", "text": "sb/login · login · checks fail"}]}}));
        assert!(matches!(app.events.get(n), Some(Ev::Fold { head, .. }) if head == " 1 PR open"));
        assert!(matches!(app.events.get(n + 1), Some(Ev::Pr { number: 12, tone, .. }) if tone == "red"));
        let _ = handle_input(&mut app, "/prs");
        let id = waiting(&app);
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {"project": "acme", "head": "", "items": [], "none": "no open PR."}}));
        assert!(matches!(app.events.last(), Some(Ev::Info(t)) if t == "no open PR."));
    }

    #[test]
    fn approvals_read_opens_its_screen_and_a_refused_removal_says_why_as_before() {
        let mut app = crate::sb::bench::test_app();
        app.sb.call("approvals/set", json!({}), Then::Approvals);
        let id = waiting(&app);
        answered(&mut app, json!({"jsonrpc": "2.0", "id": id, "result": {"project": "p", "mode": "auto", "env": false, "checker": "off",
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

    /// Law (architect m_13450, the 766a28a6 queue guard): a queued
    /// message's command/run refused by the hub, through the real
    /// dispatch, lets the queue move on as the hub's notice did; its
    /// words show; a bare `{}` keeps the mark (the turn will start).
    #[test]
    fn a_refused_queued_line_moves_the_queue_on() {
        let mut app = crate::sb::bench::test_app();
        app.sb.workspace = "/tmp/acme".into();
        let send = |app: &mut App| {
            app.queue_out = Some(std::time::Instant::now());
            app.sb.call("command/run", json!({"agent": "main", "line": "/stop x"}), Then::Line);
            *app.sb.rpc.waiting.borrow().keys().last().unwrap()
        };
        let id = send(&mut app);
        let n = app.events.len();
        dispatch(&mut app, &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32010, "message": "commands run now, not queued"}}).to_string());
        assert_eq!(app.queue_out, None, "refused: the queue moves on");
        assert!(matches!(app.events.get(n), Some(Ev::Info(t)) if t == "commands run now, not queued"));
        // the hub's words as the result: shown, and the queue moves on
        let id = send(&mut app);
        dispatch(&mut app, &json!({"jsonrpc": "2.0", "id": id, "result": {"notice": "no agent x"}}).to_string());
        assert_eq!(app.queue_out, None);
        // done, nothing said: the turn it starts clears the mark
        let id = send(&mut app);
        dispatch(&mut app, &json!({"jsonrpc": "2.0", "id": id, "result": {}}).to_string());
        assert!(app.queue_out.is_some());
    }
}
