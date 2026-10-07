//! The shared fixtures (`fixtures/*`) round-trip: every line decodes to a
//! known message and encodes back to the same JSON value, every known tag
//! of every type appears in a fixture, and the unknown ones stay unknown.

use bise_proto::draft::{AppCmd, CoreEv, DraftHubCmd, ProjectView};
use bise_proto::hub::{HubCmd, HubEv};
use serde_json::Value;
use std::path::Path;

fn lines(file: &str) -> Vec<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(file);
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn json(l: &str) -> Value {
    serde_json::from_str(l).unwrap()
}

/// Each line: decoded (not unknown), encoded back equal; returns its tags.
fn round_trip<T>(file: &str, key: &str, decode: impl Fn(&str) -> Result<T, String>, known: impl Fn(&T) -> bool, encode: impl Fn(&T) -> Value) -> Vec<String> {
    let mut tags = Vec::new();
    for l in lines(file) {
        let m = decode(&l).unwrap_or_else(|e| panic!("{file}: {e}\n  {l}"));
        assert!(known(&m), "{file}: decoded as unknown: {l}");
        assert_eq!(encode(&m), json(&l), "{file}: the round trip changed it");
        tags.push(json(&l)[key].as_str().unwrap().to_string());
    }
    tags
}

fn covers(tags: &[String], all: &[&str], file: &str) {
    for t in all {
        assert!(tags.iter().any(|x| x == t), "{file}: no fixture for {t}");
    }
}

#[test]
fn hub_events_round_trip_and_cover_every_tag() {
    let tags = round_trip("hub_ev.jsonl", "ev", HubEv::decode, |m| !matches!(m, HubEv::Unknown { .. }), HubEv::to_value);
    covers(&tags, HubEv::TAGS, "hub_ev.jsonl");
}

#[test]
fn hub_commands_round_trip_and_cover_every_tag() {
    let tags = round_trip("hub_cmd.jsonl", "cmd", HubCmd::decode, |m| !matches!(m, HubCmd::Unknown { .. }), HubCmd::to_value);
    covers(&tags, HubCmd::TAGS, "hub_cmd.jsonl");
}

#[test]
fn drafts_round_trip_and_cover_every_tag() {
    let enc = |v: &dyn erased::Enc| v.value();
    let t = round_trip("draft_hub_cmd.jsonl", "cmd", DraftHubCmd::decode, |m| !matches!(m, DraftHubCmd::Unknown { .. }), |m| enc(m));
    covers(&t, DraftHubCmd::TAGS, "draft_hub_cmd.jsonl");
    let t = round_trip("core_ev.jsonl", "ev", CoreEv::decode, |m| !matches!(m, CoreEv::Unknown { .. }), |m| enc(m));
    covers(&t, CoreEv::TAGS, "core_ev.jsonl");
    let t = round_trip("app_cmd.jsonl", "cmd", AppCmd::decode, |m| !matches!(m, AppCmd::Unknown { .. }), |m| enc(m));
    covers(&t, AppCmd::TAGS, "app_cmd.jsonl");
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/project_view.json");
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let v: ProjectView = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(&v).unwrap(), raw);
}

#[test]
fn the_directions_never_mix() {
    // a hub can't emit the core's events, and the app's commands are no
    // hub command: each decodes as unknown on the other side
    for l in lines("core_ev.jsonl") {
        assert!(matches!(HubEv::decode(&l).unwrap(), HubEv::Unknown { .. }), "{l}");
    }
    for l in lines("app_cmd.jsonl") {
        assert!(matches!(HubCmd::decode(&l).unwrap(), HubCmd::Unknown { .. }), "{l}");
    }
    for l in lines("hub_cmd.jsonl") {
        assert!(matches!(AppCmd::decode(&l).unwrap(), AppCmd::Unknown { .. }), "{l}");
    }
}

#[test]
fn unknown_tags_decode_whole() {
    let l = lines("unknown.jsonl");
    let e = HubEv::decode(&l[0]).unwrap();
    assert!(matches!(&e, HubEv::Unknown { tag, .. } if tag == "spotlight_hint"));
    assert_eq!(e.to_value(), json(&l[0]));
    assert!(matches!(HubCmd::decode(&l[1]).unwrap(), HubCmd::Unknown { tag, .. } if tag == "approve"));
}

/// The drafts' encode, without a method on each type.
mod erased {
    use serde_json::Value;

    pub trait Enc {
        fn value(&self) -> Value;
    }

    impl<T: serde::Serialize> Enc for T {
        fn value(&self) -> Value {
            serde_json::to_value(self).expect("encodes")
        }
    }
}

/// The fn context (decision F): each line decodes and encodes back to the
/// same JSON (absent fields stay absent), and an unknown field is ignored.
#[test]
fn the_fn_context_round_trips() {
    use bise_proto::context::FnContext;
    for l in lines("context.jsonl") {
        let c: FnContext = serde_json::from_str(&l).unwrap_or_else(|e| panic!("context.jsonl: {e}\n  {l}"));
        assert_eq!(serde_json::to_value(&c).unwrap(), json(&l), "context.jsonl: the round trip changed it");
    }
    let c: FnContext = serde_json::from_str(r#"{"app":"Mail","later":"a field of a newer app"}"#).unwrap();
    assert_eq!(c, FnContext { app: Some("Mail".into()), ..Default::default() });
}

/// bise-mac-helper's lines (architect m_9461): every line of both
/// directions round-trips, every tag has a line, an unknown tag stays
/// unknown. (Its own pipe: a helper's `error` is no hub's `error`.)
#[test]
fn the_helper_lines_round_trip_and_cover_every_tag() {
    use bise_proto::helper::{HelperCmd, HelperEv};
    let t = round_trip("helper_ev.jsonl", "ev", HelperEv::decode, |m| !matches!(m, HelperEv::Unknown { .. }), HelperEv::to_value);
    covers(&t, HelperEv::TAGS, "helper_ev.jsonl");
    let t = round_trip("helper_cmd.jsonl", "cmd", HelperCmd::decode, |m| !matches!(m, HelperCmd::Unknown { .. }), HelperCmd::to_value);
    covers(&t, HelperCmd::TAGS, "helper_cmd.jsonl");
    let e = HelperEv::decode(r#"{"ev":"pulse","n":1}"#).unwrap();
    assert!(matches!(&e, HelperEv::Unknown { tag, .. } if tag == "pulse"));
}

/// `bise pty`'s lines (architect m_10140): every line of both directions
/// round-trips, every tag has a line, an unknown tag stays unknown.
#[test]
fn the_pty_lines_round_trip_and_cover_every_tag() {
    use bise_proto::pty::{PtyCmd, PtyEv};
    let t = round_trip("pty_ev.jsonl", "ev", PtyEv::decode, |m| !matches!(m, PtyEv::Unknown { .. }), PtyEv::to_value);
    covers(&t, PtyEv::TAGS, "pty_ev.jsonl");
    let t = round_trip("pty_cmd.jsonl", "cmd", PtyCmd::decode, |m| !matches!(m, PtyCmd::Unknown { .. }), PtyCmd::to_value);
    covers(&t, PtyCmd::TAGS, "pty_cmd.jsonl");
    let c = PtyCmd::decode(r#"{"cmd":"scroll","by":3}"#).unwrap();
    assert!(matches!(&c, PtyCmd::Unknown { tag, .. } if tag == "scroll"));
}
