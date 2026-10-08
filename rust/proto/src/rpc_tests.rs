//! The JSON-RPC envelope's laws (architect m_13089, change 6): the
//! tables cover every typed command and event once, every fixture goes
//! through the envelope and back unchanged, and a client's reducer that
//! follows `Watermark::take` ends equal to `hub/read` whatever it lost.

use super::*;
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
        let want = usize::from(!ENVELOPE_CMDS.contains(tag));
        assert_eq!(n, want, "HubCmd {tag}: {n} methods");
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
        let Some(req) = request(Id::Num(i as u64), &c) else {
            assert!(ENVELOPE_CMDS.contains(&v["cmd"].as_str().unwrap()), "no request for {v}");
            continue;
        };
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
fn initialize_lists_every_method_and_notification() {
    let ms = methods();
    assert!(ms.iter().any(|m| m == INITIALIZE) && ms.iter().any(|m| m == HUB_READ));
    assert_eq!(ms.len(), OWN_METHODS.len() + METHODS.len());
    assert_eq!(notifications().len(), NOTIFICATIONS.len());
    let p = InitializeParams::new("bise-tui", "v2026.10.8");
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v, serde_json::json!({"proto": PROTO, "client": {"name": "bise-tui", "version": "v2026.10.8"}, "capabilities": {}}));
    assert_eq!(serde_json::from_value::<InitializeParams>(serde_json::json!({"proto": 1, "client": {"name": "x"}})).unwrap().capabilities, Value::Null);
}
