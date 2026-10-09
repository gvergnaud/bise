//! The feed from entries: a first page replaces the feed, a new pos
//! appends, a known pos (a changed entry) replaces its events in place
//! and leaves the others where they were, an older page goes in front,
//! and the window's marks follow every move.

use super::*;
use crate::sb::bench::test_app;
use bise_proto::thread::{fold, Ctx, Line};

fn entries(ls: &[&str]) -> Vec<Entry> {
    let lines: Vec<Line> = ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_700_000_000_000 + i as u64 * 1000, l.to_string())).collect();
    let none = |_: &str| None;
    let ctx = Ctx { open_cards: &[], page: &none, provider: &|_: &str, k: &str| k.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &bise_proto::thread::Attached::plain };
    fold(&lines, &ctx)
}

fn subscribed(agent: &str) -> App {
    let mut app = test_app();
    app.sb.subscribed.insert(agent.to_string());
    app
}

/// The feed's events by kind; the turn edges (hidden rows, their own
/// test) left out.
fn kinds(app: &App) -> Vec<&'static str> {
    app.events
        .iter()
        .filter_map(|e| match e {
            Ev::You(..) => Some("you"),
            Ev::Assistant(_) => Some("agent"),
            Ev::Tool(_) => Some("tool"),
            Ev::Thinking { .. } => Some("thinking"),
            Ev::AgentMsg { .. } => Some("msg"),
            Ev::TimeMark(_) => Some("time"),
            Ev::Turn | Ev::TurnDone | Ev::Ended(_) => None,
            _ => Some("other"),
        })
        .collect()
}

const TURN: &[&str] = &[
    "sb you : run the tests",
    "  obs: turn_started",
    "  obs: tool_started #1",
    "tool #1 bash : cargo test -q",
    "  obs: tool_finished #1 ok",
    "  obs: assistant: all pass.",
];

#[test]
fn a_first_page_replaces_the_feed_and_marks_its_entries() {
    let mut app = subscribed("main");
    app.events.push(Ev::Info("stale".into()));
    app.cache.push(None);
    let es = entries(TURN);
    page(&mut app, "main", None, es.clone(), false);
    assert_eq!(kinds(&app), ["you", "tool", "agent"]);
    let marks: Vec<(usize, usize)> = app.win.marks.iter().copied().collect();
    // a turn starts at pos 3: its (hidden) turn row is that entry's first event
    assert_eq!(marks, [(0, 1), (1, 3), (3, 6)], "(event, entry pos)");
    assert_eq!(app.win.first_pos, Some(1), "nothing before it: no page to ask");
    page(&mut app, "main", None, es[1..].to_vec(), true);
    assert_eq!(app.win.first_pos, Some(3), "more before it: its first pos");
}

#[test]
fn a_changed_entry_replaces_its_events_in_place() {
    let mut app = subscribed("main");
    let es = entries(TURN);
    page(&mut app, "main", None, es[..1].to_vec(), false);
    // the tools entry comes running, then again with a second call
    let mut more = TURN[..4].to_vec();
    let running = entries(&more);
    entry(&mut app, "main", &running[1]);
    assert_eq!(kinds(&app), ["you", "tool"]);
    more.extend(["  obs: tool_finished #1 ok", "  obs: tool_started #2", "tool #2 read_file : a.rs"]);
    let grown = entries(&more);
    entry(&mut app, "main", &grown[1]);
    assert_eq!(kinds(&app), ["you", "tool", "tool"], "the same entry, two calls now");
    // a newer entry after it, then the you entry changes (its mark): the
    // others stay where they are, the marks follow
    let reply = entries(&[TURN, &["  obs: assistant: done."]].concat());
    entry(&mut app, "main", reply.last().unwrap());
    assert_eq!(kinds(&app), ["you", "tool", "tool", "agent"]);
    let mut you = grown[0].clone();
    you.delivery = Some(Mark::Read);
    entry(&mut app, "main", &you);
    assert!(matches!(&app.events[0], Ev::You(_, Mark::Read, ..)), "his mark moved");
    assert_eq!(kinds(&app), ["you", "tool", "tool", "agent"]);
    let marks: Vec<(usize, usize)> = app.win.marks.iter().copied().collect();
    assert_eq!(marks, [(0, 1), (1, 3), (4, 7)]);
    assert_eq!(app.events.len(), app.cache.len());
}

#[test]
fn an_entry_of_a_thread_not_subscribed_does_nothing() {
    let mut app = test_app();
    entry(&mut app, "main", &entries(TURN)[0]);
    assert!(app.events.is_empty());
}

#[test]
fn an_older_page_goes_in_front() {
    let mut app = subscribed("main");
    let es = entries(TURN);
    page(&mut app, "main", None, es[2..].to_vec(), true);
    assert_eq!(app.win.first_pos, Some(6));
    // a stale answer (the feed moved since the ask): nothing
    page(&mut app, "main", Some(99), es[..2].to_vec(), false);
    assert_eq!(kinds(&app), ["agent"]);
    page(&mut app, "main", Some(6), es[..2].to_vec(), false);
    assert_eq!(kinds(&app), ["you", "tool", "agent"]);
    let marks: Vec<(usize, usize)> = app.win.marks.iter().copied().collect();
    assert_eq!(marks, [(0, 1), (1, 3), (3, 6)]);
    assert_eq!((app.win.first_pos, app.win.loading), (Some(1), false));
}

#[test]
fn a_pause_between_entries_gets_its_time_mark() {
    let mut app = subscribed("main");
    let mut es = entries(&["sb you : a", "sb you : b"]);
    es[1].at_ms = es[0].at_ms + 6 * 60_000;
    page(&mut app, "main", None, es, false);
    assert_eq!(kinds(&app), ["you", "time", "you"]);
}

#[test]
fn a_turn_edge_ends_the_turn_in_its_feed() {
    let mut app = subscribed("main");
    page(&mut app, "main", None, entries(TURN), false);
    let n = app.events.len();
    app.pending = true;
    app.interrupt_requested = true;
    turn_edge(&mut app, "main", false);
    assert!(!app.pending && !app.interrupt_requested);
    // the rows come with the entries (turn_start, turn_end_ms), not the
    // agents rows: one source for the edges drawn
    assert_eq!(app.events.len(), n);
}

/// BISE-271 from entries: a turn's start before its first entry, its end
/// and the end's time (the hub's, not this clock) after its last, so the
/// hover of a reply reads its own turn's end, never the next one's.
#[test]
fn a_turns_edges_and_end_time_come_with_its_entries() {
    let mut app = subscribed("main");
    let mut es = vec![
        json!({"pos": 1, "at_ms": 1000, "kind": "agent", "text": "first", "turn_start": true}),
        json!({"pos": 3, "at_ms": 3000, "kind": "agent", "text": "second", "turn_start": true, "turn_end_ms": 4000}),
    ];
    page(&mut app, "main", None, es.drain(..).map(|v| serde_json::from_value(v).unwrap()).collect(), false);
    let edges: Vec<String> = app.events.iter().filter_map(|e| match e {
        Ev::Turn => Some("turn".to_string()),
        Ev::TurnDone => Some("done".to_string()),
        Ev::Ended(t) => Some(format!("ended {t}")),
        _ => None,
    }).collect();
    assert_eq!(edges, ["turn", "turn", "done", "ended 4000"]);
    let end_of = |app: &App, text: &str| {
        let i = app.events.iter().position(|e| matches!(e, Ev::Assistant(t) if t == text)).unwrap();
        crate::feed::turn_end_of(&app.events, i)
    };
    assert_eq!(end_of(&app, "first"), None, "the first turn's end had no time");
    assert_eq!(end_of(&app, "second"), Some(4000));
    // live: the end comes as the newest entry again, at its pos
    entry(&mut app, "main", &serde_json::from_value(json!({"pos": 5, "at_ms": 5000, "kind": "agent", "text": "third", "turn_start": true})).unwrap());
    assert_eq!(end_of(&app, "third"), None, "still running");
    entry(&mut app, "main", &serde_json::from_value(json!({"pos": 5, "at_ms": 5000, "kind": "agent", "text": "third", "turn_start": true, "turn_end_ms": 6000})).unwrap());
    assert_eq!(end_of(&app, "third"), Some(6000));
}

/// proto-lead m_14731: a turn is never missed. A flip alone is its edge;
/// two quick turns between two rows are two start/end pairs; an end and a
/// start together come end first.
#[test]
fn every_turn_gets_its_edges() {
    assert_eq!(turn_edges((false, 0), (true, 0)), [true]);
    assert_eq!(turn_edges((true, 0), (false, 1)), [false]);
    assert_eq!(turn_edges((false, 3), (false, 5)), [true, false, true, false], "two quick turns");
    assert_eq!(turn_edges((true, 3), (true, 4)), [false, true], "one ended, the next one runs");
    assert_eq!(turn_edges((true, 3), (true, 3)), Vec::<bool>::new());
    // through the feed: the first row fires nothing, then its ends
    let mut app = subscribed("main");
    app.pending = true;
    agent_row(&mut app, "main", false, 2);
    assert!(app.pending, "the first row fires nothing");
    agent_row(&mut app, "main", false, 4);
    assert!(!app.pending);
}
