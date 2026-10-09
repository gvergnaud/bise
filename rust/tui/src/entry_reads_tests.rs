//! The laws of the live reads of entries (client-protocol P4d-reads):
//! one per read, and a parity law: the zen count and the panel's dot
//! read from the hub's fold are the ones the TUI read from the lines.

use super::*;
use crate::sb::bench::{test_app, test_app_drained};
use crate::sb::test_view;
use bise_proto::thread::{fold, Ctx, Line};

fn entry(v: serde_json::Value) -> Entry {
    serde_json::from_value(v).expect("an entry")
}

fn mk(ls: &[&str]) -> Vec<Line> {
    ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_700_000_000_000 + i as u64 * 1000, l.to_string())).collect()
}

fn folded(lines: &[Line]) -> Vec<Entry> {
    let none = |_: &str| None;
    let name = |id: &str, _: &str| id.to_string();
    let width = |s: &str| unicode_width::UnicodeWidthStr::width(s);
    let offset = |_: u64| 0;
    fold(lines, &Ctx { open_cards: &[], page: &none, provider: &name, width: &width, offset: &offset, attached: &bise_proto::thread::Attached::plain })
}

const LIVE: At = At { live: true, in_focus: true, voice: false };

/// Every kind that asks for him, lights a dot or says words.
fn corpus() -> Vec<Line> {
    mk(&[
        "sb you : run the tests",
        "  obs: turn_started",
        "  obs: tool_started #1",
        "tool #1 bash : cargo test -q",
        "  obs: tool_finished #1 ok",
        "  obs: assistant: all 12 pass.",
        "sb card : #9 question @perf : which bench?\\n1. cold\\n2. warm",
        "sb msg-you : docs : done here",
        "sb msg-in : @docs m_4 : the numbers",
        "sb msg-in : ambient-lead m_3 : nice",
        "sb sent : main : m_9 : 1 : cart or checkout?",
        "  obs: turn_done: completed",
        "sb warn : the inbox is full",
    ])
}

/// (zen's count, perf's dot) after each prefix of [`corpus`], as the
/// terminal read them from the lines (frozen from the line path on
/// 20950432 before the switch deleted it): a card and two messages to
/// him count, his first line lights the dot out of view.
fn by_lines(n: usize, focus: &str) -> (u64, bool) {
    const CALLS: [u64; 13] = [0, 0, 0, 0, 0, 0, 1, 2, 3, 3, 3, 3, 3];
    (CALLS[n - 1], focus == "main")
}

fn by_entries(lines: &[Line], focus: &str) -> (u64, bool) {
    let mut app = test_app();
    test_view(&mut app, focus);
    for e in folded(lines) {
        on_entry(&mut app, "perf", &e, true);
        // the hub sends a changed entry again: nothing more
        on_entry(&mut app, "perf", &e, true);
    }
    (app.sb.calls(), app.sb.lit("perf"))
}

/// Parity: the zen count and the dot are the same from entries as the
/// lines gave, in view and out of view, for the whole corpus and for
/// each of its prefixes (a dot lit by a line is lit by its entry).
#[test]
fn zen_and_the_dot_read_the_same_from_entries() {
    let lines = corpus();
    for focus in ["perf", "main"] {
        for n in 1..=lines.len() {
            let part = &lines[..n];
            assert_eq!(by_entries(part, focus), by_lines(n, focus), "focus {focus}, up to {:?}", part.last());
        }
    }
    assert_eq!(by_entries(&lines, "main"), (3, true), "a card and two messages to him");
}

/// Before the burst ends (and for a page) nothing is live: no call, no dot.
#[test]
fn a_page_reads_nothing_live() {
    let mut app = test_app();
    test_view(&mut app, "main");
    for e in folded(&corpus()) {
        on_entry(&mut app, "perf", &e, false);
    }
    assert_eq!((app.sb.calls(), app.sb.lit("perf")), (0, false));
}

/// Read 1: BISE-61: a message between agents, live, in view: once per pos.
#[test]
fn level3_is_a_live_message_between_agents_in_view() {
    let m = entry(serde_json::json!({"pos": 4, "at_ms": 0, "kind": "from_agent", "text": "nice", "from": "lead"}));
    let mut s = Seen::default();
    assert!(reads_of(&mut s, LIVE, &m).level3);
    assert!(!reads_of(&mut s, LIVE, &m).level3, "the same entry again");
    let mut s = Seen::default();
    assert!(!reads_of(&mut s, At { in_focus: false, ..LIVE }, &m).level3, "out of view");
    let mut s = Seen::default();
    assert!(!reads_of(&mut s, At { live: false, ..LIVE }, &m).level3, "a page");
    let sent = entry(serde_json::json!({"pos": 5, "at_ms": 0, "kind": "to_agent", "text": "x", "to": "main"}));
    assert!(!reads_of(&mut Seen::default(), LIVE, &sent).level3, "what it sent is not a message between agents in view");
}

/// Read 2: BISE-15: his message in view goes from received to read (the
/// model read the steering): once; a turn that reads it from sent is not
/// steering.
#[test]
fn steered_is_received_then_read() {
    let you = |d: &str| entry(serde_json::json!({"pos": 7, "at_ms": 0, "kind": "you", "text": "and the logs", "delivery": d}));
    let mut s = Seen::default();
    assert!(!reads_of(&mut s, LIVE, &you("sent")).steered);
    assert!(!reads_of(&mut s, LIVE, &you("received")).steered);
    assert!(reads_of(&mut s, LIVE, &you("read")).steered);
    assert!(!reads_of(&mut s, LIVE, &you("read")).steered, "once");
    let mut s = Seen::default();
    reads_of(&mut s, LIVE, &you("sent"));
    assert!(!reads_of(&mut s, LIVE, &you("read")).steered, "read at a turn's start");
    let mut s = Seen::default();
    reads_of(&mut s, LIVE, &you("received"));
    assert!(!reads_of(&mut s, At { in_focus: false, ..LIVE }, &you("read")).steered, "out of view");
}

/// The steering corpus of the hub's fold: received, then read.
#[test]
fn the_folds_steering_lines_say_steered() {
    let lines = mk(&["sb you : a", "  obs: turn_started", "sb you : and the logs", "  obs: steering_received: and the logs"]);
    let mut s = Seen::default();
    let mut steered = false;
    for e in folded(&lines) {
        steered |= reads_of(&mut s, LIVE, &e).steered;
    }
    assert!(!steered, "received only");
    let mut more = lines.clone();
    more.push((5, 1_700_000_005_000, "  obs: steered: and the logs".into()));
    for e in folded(&more) {
        steered |= reads_of(&mut s, LIVE, &e).steered;
    }
    assert!(steered, "then read");
}

/// Read 3: zen: a live card, or an agent writing to him, in any feed, once.
#[test]
fn a_call_is_a_card_or_a_message_to_him() {
    let card = entry(serde_json::json!({"pos": 3, "at_ms": 0, "kind": "card", "text": "#9 question", "card": {"id": 9, "question": "?", "options": [], "answered": false}}));
    let mut s = Seen::default();
    assert!(reads_of(&mut s, At { in_focus: false, ..LIVE }, &card).call);
    let mut answered = card.clone();
    answered.card.as_mut().unwrap().answered = true;
    assert!(!reads_of(&mut s, LIVE, &answered).call, "the card again, answered");
    let to_you = entry(serde_json::json!({"pos": 4, "at_ms": 0, "kind": "agent", "text": "done", "from": "docs", "to_you": true}));
    assert!(reads_of(&mut s, LIVE, &to_you).call);
    let reply = entry(serde_json::json!({"pos": 5, "at_ms": 0, "kind": "agent", "text": "ok"}));
    assert!(!reads_of(&mut s, LIVE, &reply).call);
}

/// Read 4: The dot: a new live entry he would see, out of view; never its
/// tools or reasoning.
#[test]
fn the_dot_is_words_out_of_view() {
    let out = At { in_focus: false, ..LIVE };
    let e = |pos: u64, kind: &str| entry(serde_json::json!({"pos": pos, "at_ms": 0, "kind": kind, "text": "x"}));
    let mut s = Seen::default();
    assert!(reads_of(&mut s, out, &e(1, "agent")).activity);
    assert!(!reads_of(&mut s, LIVE, &e(2, "agent")).activity, "in view");
    assert!(!reads_of(&mut s, out, &e(3, "compacting")).activity);
    assert!(!lights(EntryKind::Tools) && !lights(EntryKind::Thinking));
    assert!(lights(EntryKind::Card) && lights(EntryKind::FromAgent) && lights(EntryKind::Notice));
}

/// Read 5: The answer fold: his answer given here is not placed twice; the
/// hub's fold opens on what the item asked (BISE-307), every time it is
/// placed.
#[test]
fn his_answer_folded_here_is_skipped_and_the_hubs_asks() {
    let lines = mk(&["sb card : #4 question @perf : which bench?\\n1. cold\\n2. warm", "sb route : you → @perf (answer to card #4) : both"]);
    let es = folded(&lines);
    let fold = es.iter().find(|e| e.kind == EntryKind::Approval).expect("the route's fold");
    assert_eq!(answers(fold), Some(4));
    let mut app = test_app();
    test_view(&mut app, "perf");
    assert!(!skip(&app, "perf", fold), "answered elsewhere: placed");
    app.sb.fold_here(4, "perf");
    assert!(skip(&app, "perf", fold) && skip(&app, "perf", fold), "answered here: never placed, same answer each time");
    assert!(!skip(&app, "main", fold), "another feed's");
    // placed (answered in another view): the card's question in the feed
    let mut app = test_app();
    test_view(&mut app, "perf");
    for e in &es {
        for ev in crate::entry_ev::ev_of(e) {
            crate::feed::push_event(&mut app.events, &mut app.cache, ev);
        }
        on_entry(&mut app, "perf", e, true);
    }
    let asked = app.events.iter().find_map(|e| match e {
        Ev::Approval { asked, .. } => Some(asked.clone()),
        _ => None,
    });
    assert_eq!(asked.as_deref(), Some("which bench?"));
}

/// Read 6: The queue: at a turn's end the oldest queued message goes, once
/// (the next one waits for the turn it starts).
#[test]
fn a_turns_end_sends_the_oldest_queued() {
    let mut app = test_app_drained();
    test_view(&mut app, "perf");
    app.pending = true;
    for t in ["one", "two"] {
        app.ed.insert(t);
        crate::queue::push(&mut app);
    }
    on_turn(&mut app, "perf", true);
    assert_eq!(app.queued.len(), 2, "a turn that starts sends nothing");
    app.pending = false;
    on_turn(&mut app, "perf", false);
    assert_eq!(app.queued.len(), 1, "the oldest went");
    assert!(app.pending);
    let e = entry(serde_json::json!({"pos": 9, "at_ms": 0, "kind": "agent", "text": "x"}));
    on_entry(&mut app, "perf", &e, true);
    on_turn(&mut app, "perf", false);
    assert_eq!(app.queued.len(), 1, "the next one waits for its turn");
}

/// Read 7: Voice mode: a live reply of his agent, once; never a message to
/// him, a page or his own words.
#[test]
fn voice_says_a_live_reply_once() {
    let v = At { voice: true, ..LIVE };
    let reply = entry(serde_json::json!({"pos": 2, "at_ms": 0, "kind": "agent", "text": "all 12 pass."}));
    let mut s = Seen::default();
    assert_eq!(reads_of(&mut s, v, &reply).said.as_deref(), Some("all 12 pass."));
    assert_eq!(reads_of(&mut s, v, &reply).said, None, "the same entry again");
    assert_eq!(reads_of(&mut Seen::default(), LIVE, &reply).said, None, "voice off");
    assert_eq!(reads_of(&mut Seen::default(), At { live: false, ..v }, &reply).said, None, "a page");
    let to_you = entry(serde_json::json!({"pos": 3, "at_ms": 0, "kind": "agent", "text": "done", "from": "docs", "to_you": true}));
    assert_eq!(reads_of(&mut Seen::default(), v, &to_you).said, None);
}

/// What line mode printed for [`line_mode_prints_the_same_from_entries`]'s
/// lines from `line` events (frozen from the line path on 20950432
/// before the switch deleted it).
const PRINTED_BY_LINES: &[&str] = &[
    "[perf] you : run the tests ⏎ please",
    "[perf] tool 1 bash : cargo test -q",
    "[perf] assistant: all 12 pass.",
    "[perf] msg-in : ambient-lead m_3 : nice",
    "[perf] msg-in : @docs m_4 : the numbers",
    "[perf] msg : perf → docs m_7 : the numbers are in",
    "[perf] msg-you : docs : done here",
    "[perf] sent : main : m_9 : 1 : cart or checkout?",
];

/// What line mode prints for `lines` of perf's thread from the hub's fold
/// (a `thread/entry` notification each, sent twice: a changed entry
/// prints only what it gained).
fn printed_by_entries(lines: &[Line]) -> Vec<String> {
    use bise_proto::{hub::HubEv, rpc};
    let mut printed = crate::sb::Printed::new();
    let mut out = Vec::new();
    for e in folded(lines) {
        let ev = HubEv::Entry { project: "p".into(), agent: "perf".into(), entry: Box::new(e) };
        let j = rpc::Message::Notification(rpc::note(&ev, None).expect("a notification")).to_value().to_string();
        out.extend(crate::sb::hub_event_lines(&j, &mut printed));
        out.extend(crate::sb::hub_event_lines(&j, &mut printed));
    }
    out
}

/// Read 8: Line mode, in the parity law: his words, the replies, the tool
/// calls and the messages print from entries what the lines printed.
#[test]
fn line_mode_prints_the_same_from_entries() {
    let lines = mk(&[
        "sb you : run the tests\\nplease",
        "  obs: turn_started",
        "  obs: tool_started #1",
        "tool #1 bash : cargo test -q",
        "  obs: tool_finished #1 ok",
        "  obs: assistant: <think>hmm</think>\\nall 12 pass.",
        "sb msg-in : ambient-lead m_3 : nice",
        "sb msg-in : @docs m_4 : the numbers",
        "sb msg : perf → docs m_7 : the numbers are in",
        "sb msg-you : docs : done here",
        "sb sent : main : m_9 : 1 : cart or checkout?",
        "  obs: turn_done: completed",
    ]);
    assert_eq!(printed_by_entries(&lines), PRINTED_BY_LINES);
}

/// Line mode: every entry whose lines printed something (all of
/// [`corpus`]'s) still prints something (its words may be the entry's: a card's question, a hub
/// line's text); a changed entry prints only what it gained.
#[test]
fn line_mode_drops_no_entry() {
    // every entry of the corpus came from lines line mode printed
    for e in folded(&corpus()) {
        assert!(!line_of("perf", &e).is_empty(), "{e:?} printed nothing");
    }
    let a = vec!["[perf] tool a".to_string()];
    let ab = vec!["[perf] tool a".to_string(), "[perf] tool b".to_string()];
    assert_eq!(fresh(None, 3, &a), a);
    assert_eq!(fresh(Some(&(3, a.clone())), 3, &ab), vec!["[perf] tool b".to_string()]);
    assert_eq!(fresh(Some(&(4, a.clone())), 3, &ab), Vec::<String>::new());
    assert_eq!(fresh(Some(&(3, a.clone())), 4, &a), a);
}

/// Read 7: Voice mode, in the parity law: the replies said from entries are
/// the replies the lines drew in his agent's feed (frozen from the line
/// path on 20950432), in order, for every prefix (each once, though the hub sends a changed entry again).
#[test]
fn voice_says_the_replies_the_lines_drew() {
    let lines = mk(&[
        "sb you : run the tests",
        "  obs: turn_started",
        "  obs: assistant: on it.",
        "  obs: tool_started #1",
        "tool #1 bash : cargo test -q",
        "  obs: tool_finished #1 ok",
        "  obs: assistant: <think>hmm</think>\\nall 12 pass.",
        "sb msg-you : docs : done here",
        "  obs: turn_done: completed",
    ]);
    for n in 1..=lines.len() {
        let part = &lines[..n];
        // the replies the lines drew (frozen from the line path)
        let drew: Vec<String> = [(3, "on it."), (7, "all 12 pass.")].iter().filter(|(at, _)| n >= *at).map(|(_, t)| t.to_string()).collect();
        let mut s = Seen::default();
        let v = At { voice: true, ..LIVE };
        let said: Vec<String> = folded(part).iter().flat_map(|e| [reads_of(&mut s, v, e).said, reads_of(&mut s, v, e).said]).flatten().collect();
        assert_eq!(said, drew, "up to {:?}", part.last());
    }
}

/// Read 4 (P4e, plan v2): a thread this terminal doesn't follow lights its
/// dot when the hub's `last_pos` moves past what it saw, once per move,
/// never in view.
#[test]
fn a_head_that_moves_lights_an_unfollowed_thread() {
    let mut s = Seen::default();
    assert!(head_moved(&s, Some(0)) && !head_moved(&s, None));
    s.take(5);
    assert!(!head_moved(&s, Some(5)) && !head_moved(&s, Some(3)) && head_moved(&s, Some(6)));
    let mut app = test_app();
    test_view(&mut app, "main");
    on_head(&mut app, "perf", Some(6));
    assert!(app.sb.lit("perf"));
    let mut app = test_app();
    test_view(&mut app, "perf");
    on_head(&mut app, "perf", Some(6));
    assert!(!app.sb.lit("perf"), "in view");
}

