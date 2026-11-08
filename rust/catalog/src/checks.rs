//! Each model's last live test, kept for both screens (architect
//! m_57509): `checks.json` in bise's home, `{"<provider/id>": {"why",
//! "said", "ms", "at_ms"}}` (`why` null: it worked). The desktop core's
//! setup_test and the terminal's key check write it after a test; both
//! read it into the gauge's facts ([`crate::features::Facts::tests`]), so
//! the terminal's /models says "key expired" when Settings' test found it.
//!
//! A result older than [`STALE_MS`] reads as not tested. A stored result
//! has no generation: it reads on generation 0, the key the reading
//! process first saw (a key changed since bumps the provider's generation,
//! and the result no longer counts until the next test).

use crate::features::Test;
use bise_proto::app_rows::CheckWhy;
use serde_json::{Map, Value, json};

/// A day: a result older than that is not a fact any more.
pub const STALE_MS: u64 = 24 * 60 * 60 * 1000;

/// The results in `text` (checks.json; none when it isn't one), each on
/// generation 0, the ones older than [`STALE_MS`] at `now_ms` left out.
/// Pure.
pub fn read(text: &str, now_ms: u64) -> Vec<Test> {
    let Ok(Value::Object(all)) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    all.iter().filter_map(|(model, e)| entry(model, e)).filter(|t| fresh(t, now_ms)).collect()
}

/// `text` with `t` recorded (that model's older result replaced) and the
/// stale ones gone. Pure.
pub fn with(text: &str, t: &Test, now_ms: u64) -> String {
    let mut all = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    all.retain(|model, e| model != &t.model && entry(model, e).is_some_and(|o| fresh(&o, now_ms)));
    let (why, said) = match &t.failed {
        Some((w, s)) => (serde_json::to_value(w).unwrap_or(Value::Null), s.clone()),
        None => (Value::Null, String::new()),
    };
    all.insert(t.model.clone(), json!({"why": why, "said": said, "ms": t.ms, "at_ms": t.at_ms}));
    let mut out = serde_json::to_string_pretty(&Value::Object(all)).unwrap_or_default();
    out.push('\n');
    out
}

/// `file` (checks.json) with `t` recorded, under its lock (tmp + rename).
pub fn record(file: &std::path::Path, t: &Test, now_ms: u64) -> Result<(), String> {
    crate::roles::rewrite_locked(file, |text| with(text, t, now_ms))
}

fn fresh(t: &Test, now_ms: u64) -> bool {
    now_ms.saturating_sub(t.at_ms) < STALE_MS
}

fn entry(model: &str, e: &Value) -> Option<Test> {
    let at_ms = e["at_ms"].as_u64()?;
    let failed = match &e["why"] {
        Value::Null => None,
        w => Some((serde_json::from_value::<CheckWhy>(w.clone()).ok()?, e["said"].as_str().unwrap_or_default().to_string())),
    };
    Some(Test { model: model.to_string(), generation: 0, failed, ms: e["ms"].as_u64().unwrap_or(0), at_ms })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_791_100_000_000;

    fn test(model: &str, failed: Option<(CheckWhy, &str)>, at_ms: u64) -> Test {
        Test { model: model.into(), generation: 0, failed: failed.map(|(w, s)| (w, s.into())), ms: 420, at_ms }
    }

    #[test]
    fn a_result_reads_back_as_it_was_written_on_generation_0() {
        let ok = test("anthropic/claude-opus-5-5", None, NOW - 1000);
        let bad = test("mistral/mistral-large-4", Some((CheckWhy::WrongKey, "401 invalid key")), NOW - 2000);
        let text = with(&with("", &ok, NOW), &bad, NOW);
        let mut got = read(&text, NOW);
        got.sort_by(|a, b| a.model.cmp(&b.model));
        assert_eq!(got.len(), 2, "{text}");
        assert_eq!((got[0].model.as_str(), got[0].failed.clone(), got[0].ms, got[0].at_ms), ("anthropic/claude-opus-5-5", None, 420, NOW - 1000));
        assert_eq!(got[1].failed, Some((CheckWhy::WrongKey, "401 invalid key".to_string())));
        assert!(got.iter().all(|t| t.generation == 0));
    }

    #[test]
    fn a_new_result_replaces_the_models_older_one() {
        let first = with("", &test("mistral/m", Some((CheckWhy::NoCredit, "402")), NOW - 5000), NOW);
        let got = read(&with(&first, &test("mistral/m", None, NOW), NOW), NOW);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].failed, None);
    }

    #[test]
    fn a_result_older_than_a_day_reads_as_not_tested_and_goes_at_the_next_write() {
        let old = with("", &test("mistral/m", Some((CheckWhy::WrongKey, "401")), NOW - STALE_MS - 1), NOW - STALE_MS);
        assert_eq!(read(&old, NOW - STALE_MS).len(), 1, "fresh then");
        assert!(read(&old, NOW).is_empty(), "stale now");
        let next = with(&old, &test("anthropic/a", None, NOW), NOW);
        assert!(!next.contains("mistral/m"), "{next}");
    }

    #[test]
    fn a_file_that_isnt_checks_reads_as_none() {
        assert!(read("", NOW).is_empty());
        assert!(read("[1, 2]", NOW).is_empty());
        assert!(read(r#"{"a/b": {"why": null}}"#, NOW).is_empty(), "no at_ms: not a result");
    }

    #[test]
    fn record_writes_the_file_under_its_lock() {
        let dir = std::env::temp_dir().join(format!("bise-checks-{}", std::process::id()));
        let file = dir.join("checks.json");
        record(&file, &test("anthropic/a", None, NOW), NOW).unwrap();
        record(&file, &test("mistral/m", Some((CheckWhy::Unreachable, "timeout")), NOW), NOW).unwrap();
        assert_eq!(read(&std::fs::read_to_string(&file).unwrap(), NOW).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
