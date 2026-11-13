use super::*;
use serde_json::json;

fn req(v: Value) -> Req {
    Req::decode(v).unwrap()
}

#[test]
fn each_fixture_line_reads_and_writes_back_the_same() {
    let text = include_str!("../../fixtures/sb_req.jsonl");
    let mut tags = std::collections::BTreeSet::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).unwrap();
        let r = Req::decode(v.clone()).unwrap_or_else(|e| panic!("{e}: {l}"));
        assert!(!matches!(r, Req::Unknown { .. }), "{l}");
        assert_eq!(r.tag(), v["cmd"], "{l}");
        assert_eq!(r.to_value(), v, "{l}");
        tags.insert(r.tag().to_string());
    }
    // every command has its line: a variant missing from TAGS reads as
    // Unknown above
    let missing: Vec<_> = Req::TAGS.iter().filter(|t| !tags.contains(**t)).collect();
    assert!(missing.is_empty(), "no fixture line for {missing:?}");
}

#[test]
fn an_unknown_command_is_kept_whole() {
    let v = json!({"cmd": "teleport", "to": "mars", "from": "fix"});
    let r = req(v.clone());
    assert_eq!(r, Req::Unknown { tag: "teleport".into(), raw: v.clone() });
    assert_eq!((r.tag(), r.to_value()), ("teleport", v));
}

#[test]
fn the_envelope_keys_are_ignored() {
    let r = req(json!({"cmd": "wait", "msg": "m_3", "op": "agent", "from": "fix"}));
    assert_eq!(r, Req::Wait { msg: 3, timeout_s: 20 });
}

#[test]
fn a_known_command_with_bad_fields_is_the_senders_error() {
    for v in [
        json!({"cmd": "status", "status": "idle"}),
        json!({"cmd": "report", "kind": "maybe", "summary": "x"}),
        json!({"cmd": "send", "to": "a", "text": "b", "mode": "later"}),
        json!({"cmd": "wait", "msg": "soon"}),
        json!({"cmd": "close"}),
        json!({"cmd": "every", "step": "stop"}),
        json!({"cmd": "wake", "step": "nap"}),
        json!({"cmd": "every", "step": "show"}),
        json!({"cmd": "every", "step": "nap"}),
        json!({"cmd": "withdraw", "why": "x"}),
        json!({"cmd": "project_send", "project": "shop", "msg": "soon"}),
        json!({"cmd": "close", "card": u64::MAX as f64 * 2.0}),
    ] {
        assert!(Req::decode(v.clone()).is_err(), "{v}");
    }
    assert!(Req::decode(json!({"to": "a"})).is_err(), "no cmd");
    // the hub's read: an unknown command is the newer sb's
    assert_eq!(Req::decode_known(json!({"cmd": "teleport"})), Err("unknown command: teleport (sb is newer than this hub)".into()));
    assert_eq!(Req::decode_known(json!({"cmd": "list"})), Ok(Req::List));
}

#[test]
fn the_wire_shapes_read_as_their_values() {
    let r = req(json!({"cmd": "send", "to": "a", "text": "b", "reply_to": "12", "mode": "queued", "model": "m"}));
    assert_eq!(r, Req::Send { to: "a".into(), text: "b".into(), expect_reply: false, reply_to: Some(12), queued: true, why: String::new(), switch: Some(("m".into(), String::new())), attach: vec![], parts: String::new(), fwd: None, task: vec![] });
    // msg-forward: `--forward m_<n>` reads as its id
    let r = req(json!({"cmd": "send", "to": "a", "fwd": "m_7"}));
    assert!(matches!(r, Req::Send { fwd: Some(7), .. }), "{r:?}");
    let r = req(json!({"cmd": "show", "msg": "m_7"}));
    assert!(matches!(r, Req::Show { msg: Some(7), .. }), "{r:?}");
    // an id that doesn't read is none, as before
    let r = req(json!({"cmd": "card", "text": "q", "for": "later"}));
    assert_eq!(r, Req::card("q"));
    let r = req(json!({"cmd": "follow", "agent": "@fix"}));
    assert_eq!(r, Req::Follow { agent: "fix".into(), on: true, hub: String::new() });
    let r = req(json!({"cmd": "report", "kind": "done", "summary": "s", "step": 3, "result": "no"}));
    assert_eq!(r, Req::Report { kind: "done".into(), summary: "s".into(), decisions: vec![], step: None, result: None, done_when: None, task: vec![] }, "step without of; a result that isn't an object");
    let r = req(json!({"cmd": "every", "step": "add", "to": "@news", "text": "t", "every_ms": 60000, "page": " ", "name": " a b "}));
    assert_eq!(r, Req::Every(Every::Add { to: "news".into(), text: "t".into(), every_ms: Some(60000), daily_min: None, until_ms: None, times: None, page: None, name: Some("a b".into()) }));
    let r = req(json!({"cmd": "spawn", "name": "fix", "objective": "o", "done_when": "", "report_format": "short", "profile": "fast"}));
    let Req::Spawn { brief, ask, .. } = r else { panic!() };
    assert_eq!((brief.done_when, brief.report_format.as_deref(), ask.profile.as_str()), (None, Some("short"), "fast"));
    assert_eq!(brief.folder, None, "a spawn without --folder (an older sb) has none");
}

/// artifact-folders (architect m_46279): `sb spawn --folder` rides the
/// brief by name, both ways; none is no key, on the wire and in the brief.
#[test]
fn a_spawns_folder_round_trips() {
    let r = req(json!({"cmd": "spawn", "name": "fix", "objective": "o", "folder": "launch/hero film"}));
    let Req::Spawn { brief, .. } = r.clone() else { panic!() };
    assert_eq!(brief.folder.as_deref(), Some("launch/hero film"));
    assert_eq!(r.to_value()["folder"], "launch/hero film");
    assert_eq!(Req::decode(r.to_value()).unwrap(), r);
    assert_eq!(serde_json::to_value(&brief).unwrap()["folder"], "launch/hero film");
    let r = req(json!({"cmd": "spawn", "name": "fix", "objective": "o"}));
    assert!(r.to_value().get("folder").is_none(), "no folder, no key on the wire");
    let none = serde_json::to_value(crate::core::state::Brief::default()).unwrap();
    assert!(none.get("folder").is_none(), "no folder, no key in the brief: {none}");
    let old: crate::core::state::Brief = serde_json::from_value(json!({"objective": "o"})).unwrap();
    assert_eq!(old.folder, None, "an older journal's brief reads with none");
}
