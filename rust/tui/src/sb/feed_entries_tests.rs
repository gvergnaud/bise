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

fn kinds(app: &App) -> Vec<&'static str> {
    app.events
        .iter()
        .map(|e| match e {
            Ev::You(..) => "you",
            Ev::Assistant(_) => "agent",
            Ev::Tool(_) => "tool",
            Ev::Thinking { .. } => "thinking",
            Ev::AgentMsg { .. } => "msg",
            Ev::TimeMark(_) => "time",
            _ => "other",
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
    assert_eq!(marks, [(0, 1), (1, 3), (2, 6)], "(event, entry pos)");
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
    assert_eq!(marks, [(0, 1), (1, 3), (3, 7)]);
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
    assert_eq!(marks, [(0, 1), (1, 3), (2, 6)]);
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
    app.pending = true;
    turn_edge(&mut app, "main", false);
    assert!(!app.pending);
    assert!(matches!(app.events.last(), Some(Ev::Ended(_))));
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
    // through the feed: the first row fires nothing, then a pair per turn
    let mut app = subscribed("main");
    agent_row(&mut app, "main", false, 2);
    assert!(app.events.is_empty());
    agent_row(&mut app, "main", false, 4);
    let edges: Vec<&str> = app.events.iter().filter_map(|e| match e {
        Ev::Turn => Some("turn"),
        Ev::TurnDone => Some("done"),
        _ => None,
    }).collect();
    assert_eq!(edges, ["turn", "done", "turn", "done"]);
}
