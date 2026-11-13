//! The JSON-RPC envelope's laws (architect m_13089, change 6): the
//! tables cover every typed command and event once, every fixture goes
//! through the envelope and back unchanged, and a client's reducer that
//! follows `Watermark::take` ends equal to `hub/read` whatever it lost.

use super::*;
use crate::hub::ErrorKind;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn fixture(file: &str) -> Vec<Value> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(file);
    std::fs::read_to_string(&p).unwrap().lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn every_command_has_exactly_one_method() {
    let mut methods = BTreeSet::new();
    for r in METHODS {
        assert!(methods.insert(r.method), "method {} twice", r.method);
        assert!(HubCmd::TAGS.contains(&r.cmd), "{}: no HubCmd tag {}", r.method, r.cmd);
        assert!(!OWN_METHODS.contains(&r.method), "{} is also the protocol's own", r.method);
    }
    for tag in HubCmd::TAGS {
        let n = METHODS.iter().filter(|r| r.cmd == *tag).count();
        assert_eq!(n, 1, "HubCmd {tag}: {n} methods");
    }
}

#[test]
fn every_event_is_one_notification_or_a_result() {
    let mut methods = BTreeSet::new();
    for r in NOTIFICATIONS {
        assert!(methods.insert(r.method), "notification {} twice", r.method);
        assert!(HubEv::TAGS.contains(&r.ev), "{}: no HubEv tag {}", r.method, r.ev);
    }
    for r in METHODS {
        if let Some(t) = r.result {
            assert!(HubEv::TAGS.contains(&t), "{}: result {t} is no HubEv tag", r.method);
        }
    }
    for tag in HubEv::TAGS {
        let notes = NOTIFICATIONS.iter().filter(|r| r.ev == *tag).count();
        let result = METHODS.iter().any(|r| r.result == Some(tag));
        if ENVELOPE_EVS.contains(tag) {
            assert!(notes == 0 && !result, "{tag} is the envelope's");
        } else {
            assert!(notes <= 1, "HubEv {tag}: {notes} notifications");
            assert!(notes == 1 || result, "HubEv {tag}: neither a notification nor a result");
        }
    }
}

#[test]
fn every_command_fixture_goes_through_a_request_unchanged() {
    for (i, v) in fixture("hub_cmd.jsonl").into_iter().enumerate() {
        let c = HubCmd::from_value(v.clone()).unwrap();
        let req = request(Id::Num(i as u64), &c).unwrap_or_else(|| panic!("no request for {v}"));
        let line = Message::Request(req).encode();
        let Ok(Message::Request(back)) = Message::read(&line) else { panic!("{line}") };
        assert_eq!(back.id, Id::Num(i as u64));
        assert!(back.params.get("cmd").is_none(), "{line}: the tag leaked into params");
        assert_eq!(cmd(&back.method, back.params).unwrap().to_value(), v, "{line}");
    }
}

#[test]
fn every_event_fixture_goes_through_a_notification_or_a_result_unchanged() {
    let w = Watermark { epoch: 7, seq: 42 };
    for v in fixture("hub_ev.jsonl") {
        let e = HubEv::from_value(v.clone()).unwrap();
        let tag = v["ev"].as_str().unwrap();
        if let Some(n) = note(&e, Some(w)) {
            let line = Message::Notification(n).encode();
            let Ok(Message::Notification(back)) = Message::read(&line) else { panic!("{line}") };
            let (got, gw) = ev(&back).unwrap();
            assert_eq!(got.to_value(), v, "{line}");
            let hub = note_of_ev(tag).unwrap().scope == Scope::Hub;
            assert_eq!(gw, hub.then_some(w), "{line}: watermark");
        }
        for r in METHODS.iter().filter(|r| r.result == Some(tag)) {
            let got = ev_of_result(r.method, result(&e)).unwrap().unwrap();
            assert_eq!(got.to_value(), v, "{}: result", r.method);
        }
    }
}

#[test]
fn an_action_answers_an_empty_result_and_a_bad_request_its_code() {
    assert_eq!(ev_of_result("turn/send", json_obj()).unwrap(), None);
    assert_eq!(ev_of_result("approvals/set", json_obj()).unwrap(), None);
    assert_eq!(cmd("turn/fly", json_obj()).unwrap_err().code, code::METHOD_NOT_FOUND);
    assert_eq!(cmd("turn/send", serde_json::json!({"project": "p"})).unwrap_err().code, code::INVALID_PARAMS);
    assert_eq!(cmd("turn/send", serde_json::json!([1])).unwrap_err().code, code::INVALID_PARAMS);
    // a newer hub's notification: unknown, never an error
    let (e, w) = ev(&Notification::new("hub/weather", serde_json::json!({"sun": true}))).unwrap();
    assert!(matches!(e, HubEv::Unknown { ref tag, .. } if tag == "hub/weather"));
    assert_eq!(w, None);
}

fn json_obj() -> Value {
    Value::Object(Map::new())
}

#[test]
fn a_line_is_read_as_what_it_is() {
    let r = |l: &str| Message::read(l);
    assert!(matches!(r(r#"{"jsonrpc":"2.0","id":1,"method":"hub/read"}"#), Ok(Message::Request(_))));
    assert!(matches!(r(r#"{"jsonrpc":"2.0","id":"a","method":"hub/read","params":{}}"#), Ok(Message::Request(Request { id: Id::Str(_), .. }))));
    assert!(matches!(r(r#"{"jsonrpc":"2.0","method":"initialized"}"#), Ok(Message::Notification(_))));
    assert!(matches!(r(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#), Ok(Message::Response(_))));
    assert!(matches!(r(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"x"}}"#), Ok(Message::Response(_))));
    let e = |l: &str| r(l).unwrap_err();
    assert_eq!(e("{nope").error.unwrap().code, code::PARSE);
    let old = e(r#"{"op":"hello"}"#);
    assert_eq!((old.id, old.error.unwrap().code), (None, code::INVALID_REQUEST));
    let odd = e(r#"{"jsonrpc":"2.0","id":3}"#);
    assert_eq!((odd.id, odd.error.unwrap().code), (Some(Id::Num(3)), code::INVALID_REQUEST));
    assert!(is_rpc(&serde_json::json!({"jsonrpc": "2.0"})) && !is_rpc(&serde_json::json!({"cmd": "hello"})));
}

#[test]
fn an_error_keeps_its_kind_and_reason() {
    let e = RpcError::refused("no agent x", Some("refused")).with_kind(ErrorKind::HubOlder);
    let v = serde_json::to_value(Response::err(Some(Id::Num(1)), e)).unwrap();
    assert_eq!(v["error"]["code"], code::HUB_REFUSED);
    assert_eq!(v["error"]["data"], serde_json::json!({"kind": "hub_older", "reason": "refused"}));
    assert!(v.get("result").is_none());
}

/// an undelivered send to a task whose worktree is being deleted: its
/// typed ws beside the reason (architect m_64338); none: no field
#[test]
fn an_undelivered_send_says_its_worktree_state() {
    let e = RpcError::refused("@t4 is archived", Some("undelivered")).with_ws(Some("dropping"));
    assert_eq!(serde_json::to_value(&e.data).unwrap(), serde_json::json!({"reason": "undelivered", "ws": "dropping"}));
    let e = RpcError::refused("@t4 is archived", Some("undelivered")).with_ws(None);
    assert_eq!(serde_json::to_value(&e.data).unwrap(), serde_json::json!({"reason": "undelivered"}));
}

/// A tiny xorshift: the law below runs the same every time.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The hub side of the law: hub-wide projections by kind, numbered.
struct Hub {
    w: Watermark,
    state: BTreeMap<u8, u64>,
}

impl Hub {
    fn change(&mut self, kind: u8, value: u64) -> (Watermark, u8, u64) {
        self.w.seq += 1;
        self.state.insert(kind, value);
        (self.w, kind, value)
    }
    fn restart(&mut self) {
        self.w = Watermark { epoch: self.w.epoch + 1, seq: 0 };
    }
}

/// Law (architect m_13089, Q1): a reducer that skips what is at or below
/// its watermark, applies the next and reads the hub again on a gap or a
/// new epoch ends equal to `hub/read`, whatever was lost, duplicated or
/// sent across a restart.
#[test]
fn a_reducer_that_follows_the_watermark_ends_equal_to_hub_read() {
    for run in 1..=200u64 {
        let mut rng = Rng(run.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut hub = Hub { w: Watermark { epoch: 1, seq: 0 }, state: BTreeMap::new() };
        let (mut w, mut mine) = (hub.w, hub.state.clone());
        let mut resyncs = 0;
        let mut last = None;
        for _ in 0..300 {
            if rng.next().is_multiple_of(97) {
                hub.restart();
            }
            let n = hub.change((rng.next() % 6) as u8, rng.next() % 1000);
            let mut sent = vec![n];
            match rng.next() % 10 {
                0 => sent.clear(), // lost
                1 => sent.push(n), // twice
                2 => sent.extend(last), // an old one, late
                _ => {}
            }
            last = Some(n);
            for (nw, kind, value) in sent {
                match w.take(nw) {
                    Take::Apply => {
                        mine.insert(kind, value);
                    }
                    Take::Skip => {}
                    Take::Resync => {
                        (w, mine) = (hub.w, hub.state.clone());
                        resyncs += 1;
                    }
                }
            }
        }
        // the client's last act: a gap shows at the next notification at
        // the latest, so one more change closes the run
        let n = hub.change(0, 1);
        match w.take(n.0) {
            Take::Apply => {
                mine.insert(n.1, n.2);
            }
            Take::Skip => {}
            Take::Resync => (w, mine) = (hub.w, hub.state.clone()),
        }
        assert_eq!(mine, hub.state, "run {run}: {resyncs} resyncs");
        assert_eq!(w, hub.w, "run {run}");
    }
}

#[test]
fn every_own_method_and_command_run_has_a_result_type() {
    for m in OWN_METHODS {
        assert!(OWN_RESULTS.iter().any(|(r, _)| r == m), "{m}: no result type");
    }
    for (m, _) in OWN_RESULTS {
        let row = method_row(m);
        assert!(OWN_METHODS.contains(m) || row.is_some_and(|r| r.result.is_none()), "{m}: two result types");
    }
}

/// The fixtures of command/run's result read back as themselves.
#[test]
fn command_run_results_round_trip() {
    for v in fixture("command_run_result.jsonl") {
        let r: CommandRunResult = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&r).unwrap(), v);
    }
    let list = serde_json::to_value(CommandsList::now()).unwrap();
    assert_eq!(serde_json::from_value::<CommandsList>(list).unwrap(), CommandsList::now());
}

/// turn/send's result (A1, send-reason): its fixtures (the released `{}`
/// and `{"msg_id":7}` too) read back as themselves, `{}` (an older hub's)
/// is no id and no reason, each reason reads as itself and a newer
/// hub's word as Unknown.
#[test]
fn send_results_round_trip() {
    for v in fixture("send_result.jsonl") {
        let r: SendResult = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&r).unwrap(), v);
    }
    assert_eq!(serde_json::from_value::<SendResult>(serde_json::json!({})).unwrap(), SendResult::default());
    let reason = |w: &str| serde_json::from_value::<SendResult>(serde_json::json!({"reason": w})).unwrap().reason;
    for (w, r) in [("made", SendReason::Made), ("page", SendReason::Page), ("routed", SendReason::Routed), ("command", SendReason::Command), ("held", SendReason::Unknown)] {
        assert_eq!(reason(w), Some(r), "{w}");
    }
    assert!(OWN_RESULTS.contains(&("turn/send", "SendResult")));
}

/// card/answer's result (inbox picks (3)): its fixtures read back as
/// themselves; a step's ans_* make it with the thread row's words, and a
/// step with none (an older sb-core's, no card answered) makes none.
#[test]
fn answer_results_come_from_the_step() {
    for v in fixture("answer_result.jsonl") {
        let r: AnswerResult = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&r).unwrap(), v);
    }
    assert!(OWN_RESULTS.contains(&("card/answer", "AnswerResult")));
    let step = |pick: Option<&str>, said: &str| crate::core::link::AStep {
        ans_card: Some(3),
        ans_who: Some("docs".into()),
        ans_said: Some(said.into()),
        ans_ask: Some("where?\n1. at every step\n2. only at checkout".into()),
        ans_pick: pick.map(|p| p.into()),
        ..Default::default()
    };
    assert_eq!(serde_json::from_value::<AnswerResult>(serde_json::json!({})).unwrap().answer, None);
    let r = CardAnswer::of_step(&step(Some("only at checkout"), "2")).unwrap();
    assert_eq!((r.agent.as_str(), r.said.as_str(), r.pick.as_deref()), ("docs", "2", Some("only at checkout")));
    assert_eq!((r.notice.as_str(), r.note.as_str()), ("you answered docs: only at checkout", ""));
    assert_eq!(r.question, "where?\n1. at every step\n2. only at checkout");
    let long = "not before the release notes are written";
    let r = CardAnswer::of_step(&step(None, long)).unwrap();
    assert_eq!((r.notice.as_str(), r.note.as_str(), r.pick), ("you answered docs", long, None));
    assert_eq!(CardAnswer::of_step(&crate::core::link::AStep::default()), None);
}

#[test]
fn initialize_lists_every_method_and_notification() {
    let ms = methods();
    assert!(ms.iter().any(|m| m == INITIALIZE) && ms.iter().any(|m| m == HUB_READ));
    assert_eq!(ms.len(), OWN_METHODS.len() + METHODS.len());
    assert_eq!(notifications().len(), NOTIFICATIONS.len());
    let p = InitializeParams::new("bise-tui", "v2026.10.8");
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v, serde_json::json!({"proto": PROTO, "client": {"name": "bise-tui", "version": "v2026.10.8"}, "capabilities": {"archived_list": true}}));
    assert!(p.reads_archived_list());
    assert_eq!(serde_json::from_value::<InitializeParams>(serde_json::json!({"proto": 1, "client": {"name": "x"}})).unwrap().capabilities, Value::Null);
    assert!(!serde_json::from_value::<InitializeParams>(serde_json::json!({"proto": 1, "client": {"name": "x"}})).unwrap().reads_archived_list(), "an older client: the full shape");
}


/// architect m_24004: the steps for an older hub of an install, both
/// cases; a path with a space or a quote stays one shell word, `~/` outside
/// the quotes.
#[test]
fn an_older_install_hubs_steps_are_one_copy_paste_line() {
    assert_eq!(
        hub_older_steps(true, "~/.local/share/bise/versions/v29/bise"),
        "this project's hub runs an older bise: in a terminal, in that project, run `bise switchboard --stop`, then `bise update`, then `bise`"
    );
    assert_eq!(
        hub_older_steps(false, "~/.local/share/bise/versions/v29/bise"),
        "this project's hub runs an older bise: in a terminal, in that project, run `~/.local/share/bise/versions/v29/bise switchboard --stop`, then `bise`"
    );
    assert!(hub_older_steps(false, "/Users/Ben Martin/bise/v29/bise").contains("run `'/Users/Ben Martin/bise/v29/bise' switchboard --stop`"));
    assert!(hub_older_steps(false, "~/my apps/bise").contains("run `~/'my apps/bise' switchboard --stop`"));
    assert!(hub_older_steps(false, "/x/it's/bise").contains(r"run `'/x/it'\''s/bise' switchboard --stop`"));
}

/// An older app's `files` list (bise_proto::REMOVED, architect m_34507):
/// with paths, `card/answer` (and `send`, `slash`) is refused by name,
/// nothing sent; an empty list reads as the command.
#[test]
fn a_removed_field_with_data_is_refused_by_name() {
    let answer = |files: serde_json::Value| {
        cmd("card/answer", serde_json::json!({"project": "p", "card": 11, "reply": "this one", "files": files}))
    };
    let e = answer(serde_json::json!(["/Users/ana/Desktop/shot.png"])).unwrap_err();
    assert_eq!(e.code, code::HUB_REFUSED);
    assert_eq!(e.message, crate::words::common::removed_field);
    let d = e.data.unwrap();
    assert_eq!((d.reason.as_deref(), d.field.as_deref()), (Some("removed_field"), Some("files")));
    assert!(matches!(answer(serde_json::json!([])), Ok(HubCmd::Answer { .. })));
    assert!(matches!(answer(serde_json::Value::Null), Ok(HubCmd::Answer { .. })));
    for r in crate::REMOVED {
        assert!(crate::hub::HubCmd::TAGS.contains(&r.cmd), "{}: not a hub command", r.cmd);
    }
}

/// A retired tag or method (bise_proto::RETIRED, architect m_48959) is
/// never reused: no retired tag is in its kind's TAGS, no retired method
/// is a method; a retired tag reads as `Unknown`. A retired method is
/// refused by name with the removed field's words, reason
/// `retired_method`; a method that never existed is still not-found.
#[test]
fn a_retired_tag_reads_as_unknown_and_a_retired_method_is_refused_by_name() {
    use crate::{RetiredKind, RETIRED};
    for r in RETIRED {
        let (kind, tag) = (r.kind, r.tag);
        match kind {
            RetiredKind::HubEv => {
                assert!(!HubEv::TAGS.contains(&tag), "{tag}: retired but a hub event");
                let e = HubEv::from_value(serde_json::json!({"ev": tag, "project": "p"})).unwrap();
                assert!(matches!(e, HubEv::Unknown { .. }), "{tag}: {e:?}");
            }
            RetiredKind::HubCmd => {
                assert!(!HubCmd::TAGS.contains(&tag), "{tag}: retired but a hub command");
                let c = HubCmd::from_value(serde_json::json!({"cmd": tag, "project": "p"})).unwrap();
                assert!(matches!(c, HubCmd::Unknown { .. }), "{tag}: {c:?}");
            }
            RetiredKind::RpcMethod => {
                assert!(method_row(tag).is_none(), "{tag}: retired but a method");
                let e = cmd(tag, serde_json::json!({"project": "p"})).unwrap_err();
                assert_eq!((e.code, e.message.as_str()), (code::HUB_REFUSED, crate::words::common::removed_field), "{tag}");
                let d = e.data.unwrap();
                assert_eq!((d.reason.as_deref(), d.field.as_deref()), (Some("retired_method"), Some(tag)));
            }
        }
    }
    assert_eq!(cmd("never/was", serde_json::json!({})).unwrap_err().code, code::METHOD_NOT_FOUND);
}

/// MCP Apps C9: a client says what it is at `initialize`; absent or a
/// kind this version doesn't know is `unknown`, never taken for the app.
#[test]
fn a_client_says_its_kind_and_an_older_one_is_unknown() {
    let p = |v: serde_json::Value| serde_json::from_value::<InitializeParams>(v).unwrap().client.kind;
    assert_eq!(p(serde_json::json!({"proto": PROTO, "client": {"name": "old"}})), ClientKind::Unknown);
    assert_eq!(p(serde_json::json!({"proto": PROTO, "client": {"name": "a", "kind": "desktop"}})), ClientKind::Desktop);
    assert_eq!(p(serde_json::json!({"proto": PROTO, "client": {"name": "a", "kind": "toaster"}})), ClientKind::Unknown);
    let v = serde_json::to_value(InitializeParams::new("x", "1")).unwrap();
    assert!(v["client"].get("kind").is_none(), "unknown is not written: {v}");
    let v = serde_json::to_value(InitializeParams::new("x", "1").of_kind(ClientKind::Terminal)).unwrap();
    assert_eq!(v["client"]["kind"], "terminal");
}

/// MCP Apps C9: `app/open` is a command with its own result, and the
/// notification of the same name goes to one connection at a time.
#[test]
fn app_open_is_a_command_and_a_one_connection_notification() {
    let c = cmd("app/open", serde_json::json!({"project": "p", "what": "view", "agent": "t1", "id": "17-3"})).unwrap();
    assert_eq!(c, HubCmd::AppOpen { project: "p".into(), what: crate::app_rows::OpenWhat::View, agent: Some("t1".into()), id: Some("17-3".into()) });
    assert!(OWN_RESULTS.contains(&("app/open", "AppOpenResult")));
    assert_eq!(note_of_ev("app_open").map(|r| r.scope), Some(Scope::One));
    let ev = HubEv::from_value(serde_json::json!({"ev": "app_open", "project": "p", "what": "view", "agent": "t1", "id": "17-3"})).unwrap();
    assert!(matches!(ev, HubEv::AppOpen { what: crate::app_rows::OpenWhat::View, .. }), "{ev:?}");
}
