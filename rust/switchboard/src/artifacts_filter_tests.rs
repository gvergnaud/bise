//! The filter against the table the desktop's filters.ts reads too
//! (tests/fixtures/artifacts-filter.json): the two sides cannot drift.

use super::*;
use serde_json::Value;

const TABLE: &str = include_str!("../../../tests/fixtures/artifacts-filter.json");

fn when(v: &Value) -> Option<When> {
    v.as_str().map(|s| serde_json::from_value(Value::String(s.into())).unwrap())
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()
}

#[test]
fn law_every_case_of_the_shared_table() {
    let table: Value = serde_json::from_str(TABLE).unwrap();
    let cases = table["cases"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for c in cases {
        let f = Filter { since: when(&c["filter"]["since"]), by: strings(&c["filter"]["by"]), words: strings(&c["filter"]["words"]), apps: strings(&c["filter"]["apps"]) };
        let now = c["now"].as_u64().unwrap();
        // `tz`: [from_ms, offset] steps (a DST change); else one offset
        let steps: Vec<(u64, i32)> = match c["tz"].as_array() {
            Some(t) => t.iter().map(|s| (s[0].as_u64().unwrap(), s[1].as_i64().unwrap() as i32)).collect(),
            None => vec![(0, c["offset_min"].as_i64().unwrap() as i32)],
        };
        let tz = |ms: u64| steps.iter().rev().find(|(from, _)| ms >= *from).map_or(0, |s| s.1);
        let kept: Vec<&str> = c["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                let s = |k: &str| r[k].as_str().unwrap_or_default();
                // the row's app as the hub reads it (app_of): `app` an opener's app id with
                // its `app_word`, or `prs` / `links`; none: his Mac
                let (app, app_word) = match s("app") {
                    "" | APP_MAC => app_of(false, None, false),
                    APP_PRS => app_of(true, None, false),
                    APP_LINKS => app_of(false, None, true),
                    plugin => app_of(false, Some((plugin, s("app_word"))), false),
                };
                let row = Row { id: s("id"), title: s("title"), agent: s("agent"), kind: s("kind"), by: s("by"), new: r["new"].as_bool().unwrap(), touched_ms: r["touched_ms"].as_u64().unwrap(), app: &app, app_word: &app_word };
                keeps(&f, &row, now, &tz)
            })
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(kept, strings(&c["want"]), "{}", c["name"]);
    }
}

#[test]
fn an_unknown_since_keeps_nothing_never_everything() {
    let row = Row { id: "a", touched_ms: 1, ..Default::default() };
    assert!(!keeps(&Filter { since: Some(When::Unknown), ..Default::default() }, &row, 2, &|_| 0));
    assert_eq!(of_list("", vec![], vec![], Some(When::Unknown)), Err(SINCE_WORDS.to_string()), "sb artifact list refuses it");
    assert_eq!(serde_json::from_str::<When>("\"next-month\"").unwrap(), When::Unknown);
}

#[test]
fn a_local_day_is_the_offsets_never_the_machines() {
    // 2026-11-10 22:30 UTC is 23:30 at +60: the 10th both ways; 23:30 UTC is 00:30 the 11th at +60
    let late = 1_794_349_800_000;
    assert_eq!(day_of(late, 60), day_of(late, 0));
    let early = late + 3_600_000;
    assert_eq!(day_of(early, 60), day_of(early, 0) + 1);
    assert_eq!(day_of(0, -60), -1, "before 1970 at a west offset: the day before");
}

#[test]
fn an_app_reads_by_its_key_or_its_word_never_a_word_alone() {
    use bise_proto::words::{common, library};
    let fixed = [app_of(true, None, false), app_of(false, None, true), app_of(false, None, false)];
    let keys: Vec<&str> = fixed.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, [APP_PRS, APP_LINKS, APP_MAC], "the fixed keys, the desktop's apps.ts FIXED_TAGS");
    let tags: Vec<(&str, &str)> = fixed.iter().map(|(k, w)| (k.as_str(), w.as_str())).chain([("office-grid", "Sheets")]).collect();
    assert_eq!(app_key_for(common::your_mac, &tags), APP_MAC);
    assert_eq!(app_key_for(&library::kind_prs.to_uppercase(), &tags), APP_PRS);
    assert_eq!(app_key_for(" mac ", &tags), APP_MAC);
    assert_eq!(app_key_for("sheets", &tags), "office-grid", "a word reads as its app's key");
    assert_eq!(app_key_for("Office-Grid", &tags), "office-grid");
    assert_eq!(app_key_for("Writer", &tags), "writer", "unknown: what he typed, lowercased");
    // a PR wins over its opener, like the window's
    assert_eq!(app_of(true, Some(("office-grid", "Sheets")), false).0, APP_PRS);
}
