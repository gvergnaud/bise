//! The wire rule's law (lib.rs, architect m_14382): what a released bise
//! wrote still means the same. `fixtures/released/` holds the hub's and
//! the core's events as v2026.10.2-28 wrote them (its own fixtures, never
//! edited) and the entries its fold made of message lines; today's types
//! read each one and write it back with every released key and value
//! unchanged (new keys may come on top), so a field, list or enum value
//! that changes meaning goes red here.

use bise_proto::draft::CoreEv;
use bise_proto::hub::HubEv;
use bise_proto::thread::{fold, Ctx};
use serde_json::Value;
use std::path::Path;

fn lines(file: &str) -> Vec<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/released").join(file);
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// Where `now` differs from the released `was` (a key gone, a value
/// changed), as a path; None: `now` keeps all of `was`.
fn lost(was: &Value, now: &Value, at: &str) -> Option<String> {
    match (was, now) {
        (Value::Object(w), Value::Object(n)) => w.iter().find_map(|(k, v)| match n.get(k) {
            Some(x) => lost(v, x, &format!("{at}.{k}")),
            None => Some(format!("{at}.{k} is gone")),
        }),
        (Value::Array(w), Value::Array(n)) if w.len() == n.len() => w.iter().zip(n).enumerate().find_map(|(i, (a, b))| lost(a, b, &format!("{at}[{i}]"))),
        _ if was == now => None,
        _ => Some(format!("{at}: {was} became {now}")),
    }
}

fn check<T>(file: &str, decode: impl Fn(&str) -> Result<T, String>, unknown: impl Fn(&T) -> bool, encode: impl Fn(&T) -> Value) {
    let mut n = 0;
    for l in lines(file) {
        let m = decode(&l).unwrap_or_else(|e| panic!("{file}: today's types refuse a released line: {e}\n  {l}"));
        assert!(!unknown(&m), "{file}: a released line reads as unknown: {l}");
        let was: Value = serde_json::from_str(&l).unwrap();
        if let Some(d) = lost(&was, &encode(&m), "") {
            panic!("{file}: a released meaning changed ({d}):\n  {l}");
        }
        n += 1;
    }
    assert!(n > 20, "{file}: {n} lines");
}

#[test]
fn released_hub_events_keep_their_meaning() {
    check("hub_ev.jsonl", HubEv::decode, |m| matches!(m, HubEv::Unknown { .. }), HubEv::to_value);
}

#[test]
fn released_core_events_keep_their_meaning() {
    check("core_ev.jsonl", CoreEv::decode, |m| matches!(m, CoreEv::Unknown { .. }), |m| serde_json::to_value(m).expect("encodes"));
}

/// A message line folds to the entry it did in the released fold (G5's
/// superset, architect m_14424: msg-you stays an agent entry), new facts
/// on top.
#[test]
fn released_message_lines_fold_to_the_same_entries() {
    let none = |_: &str| None;
    let ctx = Ctx { open_cards: &[], page: &none, provider: &|_: &str, k: &str| k.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0 };
    for l in lines("fold.jsonl") {
        let v: Value = serde_json::from_str(&l).unwrap();
        let line = v["line"].as_str().unwrap();
        let e = fold(&[(1, 1001, line.to_string())], &ctx);
        assert_eq!(e.len(), 1, "{line}: {e:?}");
        let now = serde_json::to_value(&e[0]).unwrap();
        if let Some(d) = lost(&v["entry"], &now, "") {
            panic!("{line}: the released entry changed ({d}): {now}");
        }
    }
}
