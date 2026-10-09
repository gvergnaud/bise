//! The typed state rows give the terminal the words its older `state`
//! reader took from the snapshot (state_rows.rs).

use super::*;
use bise_proto::rows::{Card as RowCard, Changes, Opt, Pr as RowPr, Report, ReportKind, ScheduledEnd};
use serde_json::json;

#[test]
fn every_hub_status_word_round_trips() {
    // model::Status::as_str's words
    for w in ["starting", "working", "waiting", "idle", "done", "blocked", "failed", "stopped", "archived"] {
        let (status, archived) = Status::of_hub(w);
        assert_eq!(status_word(status, Phase::of_hub(w), archived), w);
    }
}

fn agent_row() -> rows::Agent {
    serde_json::from_value(json!({
        "name": "perf", "main": false, "status": "working", "phase": "starting", "archived": false,
        "title": "", "purpose": "", "since_ms": 1, "waits": 0, "dir": "perf-0",
        "mode": "shared", "checkout_branch": "main", "path": "/w", "objective": "make it fast\nmore",
        "note": "profiling", "role": "the hub", "msgs_queued": 2, "inbox": 0, "created_ms": 7,
        "turn_ms": 4000, "report": {"kind": "progress", "text": "half way", "at_ms": 9},
        "waiting_on": {"who": "agent", "name": "main"}, "place": "/w-wt/perf", "place_id": "pt:/w-wt/perf",
        "model": "mistral/devstral", "effort": "high", "efforts": ["low", "high"],
        "changes": {"files": 3, "add": 40, "del": 2}
    }))
    .unwrap()
}

#[test]
fn an_agent_row_reads_as_the_snapshot_did() {
    let a = Agent::of_row(&agent_row());
    assert_eq!((a.name.as_str(), a.status.as_str(), a.mode.as_str(), a.branch.as_deref()), ("perf", "starting", "shared", Some("main")));
    assert_eq!((a.objective.as_str(), a.note.as_str(), a.role.as_str(), a.path.as_str()), ("make it fast\nmore", "profiling", "the hub", "/w"));
    assert_eq!((a.queued, a.inbox, a.turn_ms, a.created_ms), (2, 0, Some(4000), 7));
    assert_eq!((a.report.as_str(), a.report_ms, a.waiting_on.as_str()), ("half way", Some(9), "main"));
    assert_eq!((a.dir.as_str(), a.place.as_str(), a.place_id.as_str()), ("perf-0", "/w-wt/perf", "pt:/w-wt/perf"));
    assert_eq!((a.model.as_str(), a.effort.as_str(), a.efforts.len(), a.changes), ("mistral/devstral", "high", 2, Some((3, 40, 2))));
    // a worktree's own branch wins; no dir: its name
    let mut r = agent_row();
    r.branch = Some("sb/perf".into());
    r.dir = String::new();
    r.report = Some(Report { kind: ReportKind::Done, text: "done".into(), at_ms: 0 });
    r.changes = Some(Changes { files: 0, add: 0, del: 0 });
    let a = Agent::of_row(&r);
    assert_eq!((a.branch.as_deref(), a.dir.as_str(), a.report_ms), (Some("sb/perf"), "perf", None));
}

#[test]
fn a_rename_is_the_same_dir_with_a_new_name() {
    let old = vec![Agent::of_row(&agent_row())];
    let mut r = agent_row();
    r.name = "speed".into();
    assert_eq!(renamed(&old, &[Agent::of_row(&r)]), [("perf".to_string(), "speed".to_string())]);
    assert!(renamed(&old, &old).is_empty());
}

fn pr(review: PrReview, in_review: bool) -> RowPr {
    RowPr {
        number: 412,
        url: "u".into(),
        branch: "sb/dark".into(),
        agents: vec!["dark".into()],
        state: PrState::Draft,
        checks: PrChecks::Fail,
        failing: vec!["ci/test".into()],
        review,
        words: String::new(),
        text: String::new(),
        in_review,
        stale_ms: Some(720_000),
    }
}

#[test]
fn a_place_row_has_the_words_places_draws() {
    let row = rows::Place { id: "wt:dark".into(), branch: Some("sb/dark".into()), agents: vec!["dark".into(), "i18n".into()], pr: Some(pr(PrReview::None, true)), lid: Some(String::new()), feature: false, trying: false };
    let p = Place::of_row(&row);
    let x = p.pr.as_ref().unwrap();
    assert_eq!((x.state.as_str(), x.review.as_str(), x.checks.as_str(), x.failing.len(), x.stale_ms), ("draft", "pending", "fail", 1, Some(720_000)));
    assert_eq!((p.id.as_str(), p.agents.len(), p.lid.as_deref()), ("wt:dark", 2, None));
    for (review, in_review, word) in [(PrReview::None, false, "none"), (PrReview::Changes, false, "changes_requested"), (PrReview::Approved, true, "approved")] {
        assert_eq!(pr_of_row(&pr(review, in_review)).review, word);
    }
}

fn card(id: u64, kind: &str, text: &str) -> RowCard {
    RowCard {
        id,
        project: "p".into(),
        kind: kind.into(),
        agent: "perf".into(),
        question: text.lines().next().unwrap_or("").into(),
        options: vec![Opt { n: 1, label: "yes".into() }],
        urgent: false,
        since_ms: 1_000,
        page: None,
        approval: false,
        rank: None,
        waiting: None,
        text: text.into(),
        note: None,
        place: None,
        pr: None,
        link: None,
        for_msg: None,
        batch: None,
    }
}

#[test]
fn his_cards_and_the_others_are_one_inbox_by_id() {
    let mut merge = card(12, "merge", "merge #412?\n1. merge\n2. not yet");
    merge.note = Some("approved · checks pass".into());
    merge.place = Some("wt:dark".into());
    merge.pr = Some(412);
    let his = [card(9, "question", "keep it?\n1. yes"), merge];
    let mut signin = card(14, "signin", "");
    signin.waiting = Some(vec!["perf".into()]);
    let others = [card(11, "drop", "archive old?"), signin];
    let c = cards_of_rows(&his, &others, Some(11), 4_000);
    assert_eq!(c.iter().map(|c| c.id).collect::<Vec<_>>(), [9, 11, 12, 14]);
    assert_eq!((c[0].text.as_str(), c[0].age_ms, c[0].asking), ("keep it?\n1. yes", 3_000, false));
    assert!(c[1].asking, "the drop asking again stays");
    assert_eq!((c[2].note.as_str(), c[2].place.as_deref(), c[2].pr), ("approved · checks pass", Some("wt:dark"), Some(412)));
    assert_eq!((c[3].text.as_str(), c[3].waiting.len()), ("", 1), "no text, no question: nothing");
}

#[test]
fn a_scheduled_row_is_the_screens_task() {
    let row: ScheduledTask = serde_json::from_value(json!({
        "id": 4, "agent": "perf", "by": "main", "words": "check HN", "every": "every 10m · 6 times", "times": 6, "done": 6,
        "label": "every 10m", "last_ms": 90, "runs": [80, 90], "ended_ms": 95, "end": "stopped", "stopped_by": {"who": "you"}
    }))
    .unwrap();
    let t = task_of_row(&row);
    assert_eq!((t.label.as_str(), t.text.as_str(), t.fired, t.next_ms, t.last_ms, t.runs.len()), ("every 10m", "check HN", 6, 0, 90, 2));
    assert_eq!((t.ended_ms, t.end.as_str(), t.stopped_by.as_str(), t.active()), (Some(95), "stopped", "user", false));
    assert_eq!(t.ended_words(), "stopped by you");
    let mut live = row.clone();
    (live.ended_ms, live.end, live.stopped_by) = (None, None, None);
    assert!(task_of_row(&live).active());
    live.end = Some(ScheduledEnd::Times);
    assert_eq!(task_of_row(&live).end, "times");
}

#[test]
fn the_flow_word() {
    assert_eq!((flow_word(Some(FlowMode::Pr)), flow_word(Some(FlowMode::Trunk)), flow_word(None)), ("pr".into(), "trunk".into(), String::new()));
}
