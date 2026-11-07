//! The record's laws, on fake journals.

use super::journal::{CardLine, TAGS};
use super::words::{day_head, reason, second_line, where_words};
use super::*;
use crate::core::journal::JOURNAL;
use serde_json::{json, Value};

fn opened(id: u64, kind: &str, agent: &str, text: &str, at: u64) -> Value {
    json!({"type": "card_opened", "ts": at, "card": {"id": id, "kind": kind, "agent": agent, "text": text, "for_msg": null, "created_ms": at, "place": null, "pr": null}})
}

fn closed(id: u64, res: &str, ans: Value, at: u64) -> Value {
    let mut v = json!({"type": "card_closed", "id": id, "resolution": res, "ts": at});
    if let (Some(o), Some(a)) = (v.as_object_mut(), ans.as_object()) {
        o.extend(a.clone());
    }
    v
}

fn pos(card: u64, agent: &str, p: u64) -> Value {
    json!({"type": "card_answer_pos", "card": card, "agent": agent, "pos": p, "ts": 9})
}

const Q: &str = "webp or avif for the hero?\n1. webp\n2. avif";

fn journal() -> Vec<Value> {
    vec![
        opened(1, "question", "perf", Q, 100),
        json!({"type": "message_sent", "msg": {}}),
        closed(1, "answered", json!({"said": "2", "pick_n": 2, "pick": "avif", "via": "inbox"}), 200),
        pos(1, "perf", 41),
        opened(2, "question", "docs", "ship the readme today?", 300),
        closed(2, "withdrawn by main: moot, docs shipped", json!({}), 400),
        opened(3, "drop", "old", "drop @old?", 500),
        closed(3, "accepted", json!({"said": "yes", "via": "thread"}), 600),
        opened(4, "question", "design", "which hero?", 700),
    ]
}

fn index() -> Index {
    let mut x = Index::new("harness-1".into());
    x.read_all(&journal());
    x
}

#[test]
fn a_picked_digit_reads_as_the_options_words() {
    let r = index().get(1).cloned().unwrap();
    assert_eq!(r.state, InboxState::Answered);
    assert_eq!(r.said.as_deref(), Some("avif"));
    assert_eq!(r.pick_n, Some(2));
    assert_eq!(r.question, "webp or avif for the hero?");
    assert_eq!(r.options, vec!["webp", "avif"]);
    assert_eq!(r.at, Some(Where::Inbox));
    assert_eq!(r.thread, Some(AnswerAt { agent: "perf".into(), pos: 41 }));
    assert_eq!((r.asked_ms, r.at_ms), (100, 200));
}

#[test]
fn the_link_a_card_opened_with_stays_in_its_record_after_a_re_fold() {
    let mut open = opened(5, "question", "perf", "run the bench?", 100);
    open["card"]["about"] = json!({"id": "signup-bench", "kind": "page", "title": "signup, before / after", "v": 3, "url": null, "at": null});
    let lines = vec![open, closed(5, "answered", json!({"said": "yes", "via": "inbox"}), 200)];
    // the journal folded again (a restart) gives it back: the card's link isn't a side file's
    let mut x = Index::new("harness-1".into());
    x.read_all(&lines);
    let about = x.get(5).and_then(|r| r.about.clone()).unwrap();
    assert_eq!((about.id.as_str(), about.kind.as_str(), about.v), ("signup-bench", "page", Some(3)));
    // a line from before cards kept it reads none
    assert_eq!(index().get(1).and_then(|r| r.about.clone()), None);
}

#[test]
fn a_close_without_his_answer_is_withdrawn_with_its_reason() {
    let r = index().get(2).cloned().unwrap();
    assert_eq!(r.state, InboxState::Withdrawn);
    assert_eq!(r.said, None);
    assert_eq!(r.at, None);
    assert_eq!(second_line(&r), "withdrawn: moot, docs shipped");
}

#[test]
fn a_close_without_ts_takes_the_journals_last_time_never_0() {
    // sb-core writes card_closed with no time (the 'jan 1 1970' group in his inbox shot)
    let mut x = Index::new("harness-1".into());
    x.read_all(&[
        json!({"type": "card_opened", "card": {"id": 7, "kind": "question", "agent": "shots", "text": "go?", "for_msg": null, "created_ms": 1_000}}),
        json!({"type": "wake_end", "id": 1, "why": "hit", "at": 5_000}),
        json!({"type": "reported", "name": "shots", "report": {"at_ms": 6_000, "kind": "turn"}}),
        json!({"type": "card_closed", "id": 7, "resolution": "withdrawn by main: not needed"}),
        json!({"type": "card_opened", "card": {"id": 8, "kind": "question", "agent": "shots", "text": "and now?", "for_msg": null, "created_ms": 9_000}}),
        json!({"type": "card_closed", "id": 8, "resolution": "withdrawn by main"}),
    ]);
    assert_eq!(x.get(7).map(|r| (r.asked_ms, r.at_ms)), Some((1_000, 6_000)));
    // nothing timed after it opened: its close is when it opened
    assert_eq!(x.get(8).map(|r| r.at_ms), Some(9_000));
}

#[test]
fn an_open_card_has_no_record() {
    assert!(index().get(4).is_none());
    assert_eq!(index().len(), 3);
}

#[test]
fn a_position_counts_once_and_only_for_an_answer() {
    let mut x = index();
    assert_eq!(x.read(&pos(1, "main", 99)), None);
    assert_eq!(x.get(1).unwrap().thread.as_ref().unwrap().pos, 41);
    assert_eq!(x.read(&pos(2, "docs", 7)), None);
    assert!(x.get(2).unwrap().thread.is_none());
}

#[test]
fn an_older_journal_reads_closed_cards_as_withdrawn() {
    // a line written before the answer's keys: no answer, no error
    let mut x = Index::new("p".into());
    x.read(&opened(5, "question", "a", "q?", 1));
    x.read(&json!({"type": "card_closed", "id": 5, "resolution": "answered"}));
    assert_eq!(x.get(5).unwrap().state, InboxState::Withdrawn);
}

#[test]
fn a_torn_or_foreign_line_is_skipped() {
    let mut x = index();
    assert_eq!(x.read(&json!({"type": "card_closed", "id": "x"})), None);
    assert_eq!(x.read(&json!("garbage")), None);
    assert!(CardLine::of_text("{\"type\":\"card_clo").is_none());
    assert_eq!(x.len(), 3);
}

#[test]
fn replay_and_live_give_the_same_records() {
    // the hub's replay (read_all) and its live reads (one at a time,
    // as each line is journaled) fold to the same records
    let all = index();
    let mut live = Index::new("harness-1".into());
    for l in journal() {
        live.read(&l);
    }
    let a: Vec<_> = (1..=4).map(|i| all.get(i).cloned()).collect();
    let b: Vec<_> = (1..=4).map(|i| live.get(i).cloned()).collect();
    assert_eq!(a, b);
}

#[test]
fn a_page_is_newest_first_and_pages_by_card() {
    let x = index();
    let p = x.page(&Query { limit: 2, ..Query::default() });
    assert_eq!(p.items.iter().map(|r| r.card).collect::<Vec<_>>(), vec![3, 2]);
    assert!(p.more);
    assert_eq!((p.answered, p.withdrawn), (2, 1));
    let next = x.page(&Query { limit: 2, before: Some(2), ..Query::default() });
    assert_eq!(next.items.iter().map(|r| r.card).collect::<Vec<_>>(), vec![1]);
    assert!(!next.more);
}

#[test]
fn filters_by_state_agent_and_words() {
    let x = index();
    let ids = |q: Query| x.page(&q).items.iter().map(|r| r.card).collect::<Vec<_>>();
    assert_eq!(ids(Query { state: StateFilter::Withdrawn, ..Query::default() }), vec![2]);
    assert_eq!(ids(Query { state: StateFilter::Answered, ..Query::default() }), vec![3, 1]);
    assert_eq!(ids(Query { agent: Some("perf".into()), ..Query::default() }), vec![1]);
    assert_eq!(ids(Query { words: Some("AVIF hero".into()), ..Query::default() }), vec![1]);
    assert_eq!(ids(Query { words: Some("nothing-like-it".into()), ..Query::default() }), Vec::<u64>::new());
}

#[test]
fn the_row_words() {
    let x = index();
    // the word by its key (words::inbox): a word designer changes moves this test (issue 29)
    assert_eq!(second_line(x.get(1).unwrap()), crate::words::inbox::answered_at("avif", crate::words::inbox::where_inbox));
    assert_eq!(second_line(x.get(3).unwrap()), "you: yes · in old's thread");
    assert_eq!(where_words(&Where::Page { page: "p".into(), block: "q1".into() }), "on a page");
    assert_eq!(reason("agent stopped"), "its agent stopped");
    assert_eq!(reason("closed"), "you closed it");
}

#[test]
fn day_heads_at_the_callers_offset() {
    // 2025-09-30 12:41 UTC
    let now = 1_759_236_060_000;
    assert_eq!(day_head(now, now, 0), "today");
    assert_eq!(day_head(now - 86_400_000, now, 0), "yesterday");
    assert_eq!(day_head(now - 3 * 86_400_000, now, 0), "sep 27");
    // 23:30 UTC the day before is today at UTC+2
    let late = now - 13 * 3_600_000 - 11 * 60_000;
    assert_eq!(day_head(late, now, 0), "yesterday");
    assert_eq!(day_head(late, now, 2 * 3600), "today");
}

/// A row field's keys in its line: its own, or an inline declared record's
/// (card_closed's `ans`: `SC Ans.journal`, its keys in the line itself).
fn line_keys(f: &crate::core::Field) -> Vec<&'static str> {
    match f.kind {
        crate::core::FieldKind::Inline(c) if c.module == "SC" => {
            let (rec, shape) = c.name.split_once('.').expect("<T>.<shape>");
            let r = crate::core::state::STATE.record(rec).expect("a declared record");
            r.shape(shape).expect("its shape").keys.iter().map(|k| k.key).collect()
        }
        _ => vec![f.name],
    }
}

#[test]
fn the_card_lines_are_the_tables() {
    // the typed reader's keys are the JOURNAL rows' fields
    for (tag, keys) in [
        ("card_closed", vec!["id", "resolution", "said", "pick_n", "pick", "via", "page", "block"]),
        ("card_answer_pos", vec!["card", "agent", "pos"]),
        ("card_opened", vec!["card"]),
    ] {
        assert!(TAGS.contains(&tag));
        let row = JOURNAL.kind(tag).unwrap_or_else(|| panic!("{tag} in JOURNAL"));
        assert_eq!(row.fields.iter().flat_map(|f| line_keys(f)).collect::<Vec<_>>(), keys, "{tag}");
    }
    // a full line of each reads, every key taken
    let c = CardLine::of(&closed(7, "answered", json!({"said": "s", "pick_n": 1, "pick": "p", "via": "page", "page": "pg", "block": "q"}), 1)).unwrap();
    let CardLine::Closed(c) = c else { panic!("closed") };
    assert_eq!((c.said.as_str(), c.pick_n, c.pick.as_str(), c.via.as_str(), c.page.as_str(), c.block.as_str()), ("s", Some(1), "p", "page", "pg", "q"));
}

#[test]
fn the_released_fixture_reads() {
    // rust/proto/fixtures/core_journal.jsonl: its card lines read typed
    let text = include_str!("../../fixtures/core_journal.jsonl");
    let n = text.lines().filter_map(CardLine::of_text).count();
    assert_eq!(n, 3);
}
