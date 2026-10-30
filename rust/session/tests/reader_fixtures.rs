//! BISE-191: the reader part of every fixture's expect.json.
use bise_session::{read_dir, Open};
use serde_json::Value;
use std::path::PathBuf;

pub fn fixtures() -> Vec<(String, PathBuf, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session");
    let mut out: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|e| e.path().join("expect.json").exists())
        .map(|e| {
            let exp: Value = serde_json::from_str(&std::fs::read_to_string(e.path().join("expect.json")).unwrap()).unwrap();
            (e.file_name().to_string_lossy().to_string(), e.path(), exp)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(out.len(), 16, "the 15 cases of §13.3 and 16-hollow-assistant");
    out
}

fn lines(v: &Value) -> Vec<usize> {
    v.as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap() as usize).collect()).unwrap_or_default()
}

#[test]
fn every_fixture_reads_as_expected() {
    for (name, dir, exp) in fixtures() {
        if name.starts_with("14-") {
            continue; // the writer's case (input.jsonl → events.jsonl), BISE-193
        }
        let log = read_dir(&dir).unwrap();
        let open = match &log.open {
            Open::Ok => "ok",
            Open::ReadOnly => "read_only",
            Open::Refused(_) => "refused",
        };
        assert_eq!(open, exp["open"].as_str().unwrap(), "{name}: open ({:?})", log.open);
        if open == "refused" {
            continue;
        }
        let cur = |v: &Vec<bise_session::Loc>| v.iter().filter(|l| l.file == "events.jsonl").map(|l| l.line).collect::<Vec<_>>();
        assert_eq!(cur(&log.bad_lines), lines(&exp["bad_lines"]), "{name}: bad lines");
        assert_eq!(cur(&log.unknown), lines(&exp["unknown"]), "{name}: unknown");
        assert_eq!(cur(&log.seq_errors), lines(&exp["seq_errors"]), "{name}: seq errors");
        assert!(log.malformed.is_empty(), "{name}: malformed {:?}", log.malformed);
        let others: Vec<(usize, String)> = log.others.iter().map(|(l, f)| (l.line, f.clone())).collect();
        let want: Vec<(usize, String)> = exp["others"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_u64().unwrap() as usize, p[1].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(others, want, "{name}: others");
        let torn = log.torn.as_ref().map(|t| String::from_utf8_lossy(t).to_string());
        assert_eq!(torn.as_deref(), exp["torn"].as_str(), "{name}: torn");
        // every known event of the fixtures has a payload
        for e in &log.events {
            let known = bise_session::types::TYPES.iter().any(|(t, _)| *t == e.typ);
            assert_eq!(e.payload.is_some(), known, "{name}: seq {} ({})", e.seq, e.typ);
            assert_eq!(e.must, bise_session::types::must_of(&e.typ) || (!known && e.must), "{name}: must of {}", e.typ);
        }
    }
}

#[test]
fn an_unknown_enum_value_reads_as_other() {
    let log = read_dir(&fixtures().into_iter().find(|f| f.0.starts_with("08-")).unwrap().1).unwrap();
    let ts = log.events.iter().find(|e| e.typ == "turn_started").unwrap();
    assert!(matches!(ts.payload, Some(bise_session::Payload::TurnStarted { cause: bise_session::types::Cause::Other })));
}

#[test]
fn unknown_fields_stay_in_the_raw_line() {
    let log = read_dir(&fixtures().into_iter().find(|f| f.0.starts_with("07-")).unwrap().1).unwrap();
    let um = log.events.iter().find(|e| e.typ == "user_message").unwrap();
    assert!(um.raw.contains("\"mood\":\"curious\"") && um.raw.contains("\"host\":\"ada-mbp\""));
    assert!(um.payload.is_some());
}

#[test]
fn a_known_type_with_the_wrong_shape_is_skipped_and_reported() {
    let log = bise_session::read_bytes(&[(
        "events.jsonl".into(),
        b"{\"seq\":1,\"type\":\"session_start\",\"v\":1,\"data\":{\"session\":\"s\",\"format\":1,\"created_by\":\"x\",\"cwd\":\"/\"}}\n{\"seq\":2,\"type\":\"user_message\",\"v\":1,\"must\":true,\"data\":{\"content\":3}}\n".to_vec(),
    )]);
    assert_eq!(log.malformed.len(), 1);
    assert_eq!(log.open, Open::ReadOnly, "a must event we cannot read: no resume");
}
