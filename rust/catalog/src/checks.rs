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
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A day: a result older than that is not a fact any more.
pub const STALE_MS: u64 = 24 * 60 * 60 * 1000;

/// One model's last test, as checks.json keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    /// None: it worked (a why a newer bise wrote reads as Unknown)
    why: Option<CheckWhy>,
    /// the provider's words when it failed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    said: String,
    #[serde(default)]
    ms: u64,
    at_ms: u64,
}

/// checks.json by model (sorted: stable diffs); a file that isn't one is empty.
type Checks = BTreeMap<String, Entry>;

fn parse(text: &str) -> Checks {
    serde_json::from_str(text).unwrap_or_default()
}

/// The results in `text` (checks.json; none when it isn't one), each on
/// generation 0, the ones older than [`STALE_MS`] at `now_ms` left out.
/// Pure.
pub fn read(text: &str, now_ms: u64) -> Vec<Test> {
    parse(text).into_iter().filter(|(_, e)| fresh(e, now_ms)).map(|(model, e)| test_of(model, e)).collect()
}

/// `text` with `t` recorded (that model's older result replaced) and the
/// stale ones gone. Pure.
pub fn with(text: &str, t: &Test, now_ms: u64) -> String {
    let mut all = parse(text);
    all.retain(|_, e| fresh(e, now_ms));
    let (why, said) = match &t.failed {
        Some((w, s)) => (Some(*w), s.clone()),
        None => (None, String::new()),
    };
    all.insert(t.model.clone(), Entry { why, said, ms: t.ms, at_ms: t.at_ms });
    let mut out = serde_json::to_string_pretty(&all).unwrap_or_default();
    out.push('\n');
    out
}

/// `file` (checks.json) with `t` recorded, under its lock (tmp + rename).
pub fn record(file: &std::path::Path, t: &Test, now_ms: u64) -> Result<(), String> {
    crate::locked::rewrite(file, |text| with(text, t, now_ms))
}

fn fresh(e: &Entry, now_ms: u64) -> bool {
    now_ms.saturating_sub(e.at_ms) < STALE_MS
}

fn test_of(model: String, e: Entry) -> Test {
    Test { model, generation: 0, failed: e.why.map(|w| (w, e.said)), ms: e.ms, at_ms: e.at_ms }
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
        assert!(read(r#"{"a/b": {"why": null}}"#, NOW).is_empty(), "no at_ms: not a file of checks");
        let later = read(&format!(r#"{{"a/b": {{"why": "a_why_from_a_newer_bise", "at_ms": {NOW}}}}}"#), NOW);
        assert_eq!(later[0].failed, Some((CheckWhy::Unknown, String::new())), "a newer writer's why reads Unknown");
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
