//! task_log.rs on journal lines as sb-core writes them (rust/proto's
//! fixtures/core_journal.jsonl shapes), fed newest first like the shell.

use super::*;
use serde_json::json;

fn ids(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

/// The Log of `ids` over `journal` written oldest first (as the file is).
fn log(ids_: &[&str], journal: &[serde_json::Value], lands: &[LandRecord]) -> Vec<LogLine> {
    let lines: Vec<String> = journal.iter().map(|v| v.to_string()).collect();
    of_lines(&ids(ids_), lines.iter().rev().map(String::as_str), lands, LIMIT)
}

fn created(id: &str, by: &str, at: u64) -> serde_json::Value {
    json!({"type": "work_created", "work": {"id": id, "name": "Fix the login", "why": "", "goals": [], "kind": "task", "by": by, "created_ms": at, "agents": [by], "last": at}})
}

fn report(name: &str, at: u64, works: &[&str], decisions: &[&str]) -> serde_json::Value {
    json!({"type": "reported", "name": name, "report": {"at_ms": at, "kind": "progress", "summary": "repro on 3G", "decisions": decisions, "auto": false, "works": works}})
}

#[test]
fn a_report_counts_only_when_its_works_hold_the_task() {
    let j = [
        created("login", "main", 100),
        report("auth", 200, &["login"], &["keep the 10 s budget"]),
        report("auth", 300, &["other"], &[]),
        // an older line with no works, and a turn's end: no line
        json!({"type": "reported", "name": "auth", "report": {"at_ms": 400, "kind": "progress", "summary": "old", "decisions": []}}),
        json!({"type": "reported", "name": "auth", "report": {"at_ms": 500, "kind": "turn", "summary": "end", "decisions": [], "auto": true, "works": ["login"]}}),
    ];
    let l = log(&["login"], &j, &[]);
    let reps: Vec<&LogLine> = l.iter().filter(|x| x.kind == LogKind::Report).collect();
    assert_eq!(reps.len(), 1);
    assert_eq!(reps[0].who, "auth");
    assert_eq!(reps[0].text, "repro on 3G");
    assert_eq!(reps[0].decisions, vec!["keep the 10 s budget".to_string()]);
}

#[test]
fn his_gestures_are_his_and_an_agents_change_is_not_one() {
    let j = [
        created("login", "main", 100),
        json!({"type": "work_changed", "id": "login", "op": "tick", "n": 1, "by": "auth", "at": 150}),
        json!({"type": "work_changed", "id": "login", "op": "pause", "until": 0, "by": "user", "at": 200}),
        json!({"type": "work_changed", "id": "login", "op": "resume", "by": "user", "at": 300}),
        json!({"type": "work_changed", "id": "login", "op": "rename", "name": "Fix the login on 3G", "by": "user", "at": 400}),
        json!({"type": "work_changed", "id": "other", "op": "done", "by": "user", "at": 450}),
    ];
    let l = log(&["login"], &j, &[]);
    let g: Vec<(Option<LogGesture>, &str)> = l.iter().filter(|x| x.kind == LogKind::Gesture).map(|x| (x.gesture, x.text.as_str())).collect();
    assert_eq!(g, vec![(Some(LogGesture::Rename), "Fix the login on 3G"), (Some(LogGesture::Resume), ""), (Some(LogGesture::Pause), "")]);
}

#[test]
fn an_agent_joins_once_at_its_first_turn_and_the_maker_at_the_birth() {
    let j = [
        created("login", "main", 100),
        json!({"type": "work_turn", "turn": {"agent": "auth", "works": ["login"], "add": false, "at": 200}}),
        json!({"type": "work_turn", "turn": {"agent": "auth", "works": ["login"], "add": false, "at": 300}}),
        json!({"type": "work_turn", "turn": {"agent": "docs", "works": ["other"], "add": false, "at": 350}}),
    ];
    let l = log(&["login"], &j, &[]);
    let joins: Vec<(&str, u64)> = l.iter().filter(|x| x.kind == LogKind::Join).map(|x| (x.who.as_str(), x.at_ms)).collect();
    assert_eq!(joins, vec![("auth", 200), ("main", 100)]);
}

#[test]
fn his_answer_counts_for_a_card_of_the_task_and_takes_its_neighbours_time() {
    let j = [
        created("login", "main", 100),
        json!({"type": "card_opened", "card": {"id": 3, "kind": "question", "agent": "auth", "text": "3G or 4G?", "for_msg": 7, "created_ms": 200, "works": ["login"]}}),
        json!({"type": "card_opened", "card": {"id": 4, "kind": "question", "agent": "docs", "text": "theme?", "for_msg": 8, "created_ms": 210, "works": ["other"]}}),
        json!({"type": "card_closed", "id": 3, "resolution": "answered", "said": "3G", "via": "inbox"}),
        json!({"type": "message_sent", "msg": {"id": 9, "from": "user", "to": "auth", "created": 250, "works": ["login"]}, "text": "3G"}),
        json!({"type": "card_closed", "id": 4, "resolution": "answered", "said": "dark", "via": "inbox"}),
        // withdrawn without him: no answer
        json!({"type": "card_closed", "id": 5, "resolution": "withdrawn"}),
    ];
    let l = log(&["login"], &j, &[]);
    let a: Vec<(&str, Option<u64>, u64)> = l.iter().filter(|x| x.kind == LogKind::Answer).map(|x| (x.text.as_str(), x.card, x.at_ms)).collect();
    assert_eq!(a, vec![("3G", Some(3), 250)]);
}

#[test]
fn a_move_onto_the_task_and_off_it_are_his_gestures() {
    let j = [
        created("login", "main", 100),
        json!({"type": "work_changed", "id": "login", "op": "why", "why": "spins", "by": "user", "at": 150}),
        json!({"type": "art_moved", "moved": {"art": "repro-page", "works": ["login"]}}),
        json!({"type": "work_changed", "id": "login", "op": "goal", "text": "10 s on 3G", "by": "user", "at": 300}),
        json!({"type": "art_moved", "moved": {"art": "repro-page", "works": ["other"]}}),
    ];
    let l = log(&["login"], &j, &[]);
    let m: Vec<(Option<LogGesture>, &str)> = l.iter().filter(|x| matches!(x.gesture, Some(LogGesture::MoveIn | LogGesture::MoveOut))).map(|x| (x.gesture, x.text.as_str())).collect();
    assert_eq!(m, vec![(Some(LogGesture::MoveOut), "repro-page"), (Some(LogGesture::MoveIn), "repro-page")]);
}

#[test]
fn a_land_comes_from_its_record_works_and_the_log_is_newest_first_and_capped() {
    let rec = |at: u64, works: &[&str]| LandRecord {
        agent: "auth".into(),
        place: "wt:auth".into(),
        target: "main".into(),
        base: "b".into(),
        tip: "abcdef1234567".into(),
        commits: vec!["abcdef1234567".into(), "c2".into()],
        at_ms: at,
        works: works.iter().map(|s| s.to_string()).collect(),
    };
    let j = [created("login", "main", 100), report("auth", 200, &["login"], &[])];
    let l = log(&["login"], &j, &[rec(300, &["login"]), rec(400, &["other"]), rec(500, &[])]);
    assert_eq!(l.iter().map(|x| x.kind).collect::<Vec<_>>(), vec![LogKind::Land, LogKind::Report, LogKind::Join]);
    assert_eq!(l[0].commits, 2);
    assert_eq!(l[0].sha.as_deref(), Some("abcdef1234567"));
    let lines: Vec<String> = j.iter().map(|v| v.to_string()).collect();
    assert_eq!(of_lines(&ids(&["login"]), lines.iter().rev().map(String::as_str), &[], 1).len(), 1);
}

#[test]
fn a_merged_tasks_facts_are_in_the_log_of_the_one_it_went_into() {
    let j = [created("login", "main", 100), created("login-3g", "main", 150), report("net", 200, &["login-3g"], &[])];
    let l = log(&["login", "login-3g"], &j, &[]);
    assert!(l.iter().any(|x| x.kind == LogKind::Report && x.who == "net"));
}

#[test]
fn the_walk_stops_at_the_birth_of_every_id() {
    let lines: Vec<String> = [report("old", 50, &["login"], &[]), created("login", "main", 100), report("auth", 200, &["login"], &[])]
        .iter()
        .map(|v| v.to_string())
        .collect();
    let mut s = Scan::new(&ids(&["login"]));
    let mut fed = 0;
    for l in lines.iter().rev() {
        fed += 1;
        if !s.line(l) {
            break;
        }
    }
    assert_eq!(fed, 2, "the line older than its birth is never read");
}

#[test]
fn the_words_of_sb_task_show_log() {
    let l = vec![
        LogLine { at_ms: 2, kind: LogKind::Report, who: "auth".into(), text: "repro on 3G".into(), report: "progress".into(), decisions: vec!["keep 10 s".into()], ..LogLine::default() },
        LogLine { at_ms: 1, kind: LogKind::Gesture, who: "user".into(), gesture: Some(LogGesture::Pause), ..LogLine::default() },
    ];
    let w = words(&l, |t| format!("t{t}"));
    assert_eq!(w[0], format!("t2  auth reported progress: repro on 3G{}    decided: keep 10 s", char::from(10u8)));
    assert_eq!(w[1], "t1  you paused it");
}

/// architect m_65437 (debt D1): the Log's records are a second reader of
/// sb-core's journal; pinned by the codec's own released fixture, one line
/// per kind (core_gen's journal test reads it back through the Bend
/// codec): a renamed field goes red here, never an empty Log.
#[test]
fn every_kind_the_log_reads_parses_the_journal_codecs_fixture() {
    let text = include_str!("../../proto/fixtures/core_journal.jsonl");
    let mut seen = std::collections::BTreeSet::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let kind = serde_json::from_str::<serde_json::Value>(line).unwrap()["type"].as_str().unwrap_or_default().to_string();
        if KINDS.contains(&kind.as_str()) {
            assert_eq!(reads(line), Ok(()), "{kind}: {line}");
            seen.insert(kind);
        }
    }
    assert_eq!(seen.len(), KINDS.len(), "a kind the Log reads has no fixture line: {seen:?}");
}
