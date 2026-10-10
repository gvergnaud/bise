//! The parity law of client-protocol step 4 (architect m_13977 Q1): for
//! the same thread lines, the feed drawn from the hub's fold
//! (`bise_proto::thread::fold` then [`ev_of`]) is the feed the TUI draws
//! from the lines (its own fold, `run::ingest_line` of each line). Compared
//! row by row as text at one width. A corpus that differs must be in
//! [`GAPS`] with its first differing row; a gap that closes must leave
//! the list (it only shrinks). The feed switches to entries (P4d) when it
//! is empty.

use super::*;
use crate::feed::{event_rows, push_event};
use crate::sb::bench::test_app;
use bise_proto::thread::{fold, Ctx, Line};

const WIDTH: usize = 100;

/// The corpora that still differ, by name (none since the feed switched
/// to entries, P4d)
const GAPS: &[&str] = &[];

/// The corpora: a name and its lines (`(pos, ts, line)`).
fn corpora() -> Vec<(&'static str, Vec<Line>)> {
    let mk = |ls: &[&str]| ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_700_000_000_000 + i as u64 * 1000, l.to_string())).collect::<Vec<Line>>();
    let at_once = |ls: &[&str]| ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_700_000_000_000, l.to_string())).collect::<Vec<Line>>();
    vec![
        // the lines wire_agree_tests reads (every kind both readers know)
        (
            "agree",
            mk(&[
                "sb you : make it fast\\nplease",
                "  obs: turn_started",
                "  obs: assistant: <think>hmm</think>\\non it",
                "tool #1 bash : cargo test -q",
                "tool_intent #1 : running the tests",
                "sb msg-in : ambient-lead m_3 : nice : really",
                "sb sent : main : m_9 : 1 : cart \\: or checkout?",
                "sb msg-you : docs : done here",
                "sb stopped : stopped by you",
                "  obs: turn_done: failed: 500",
                "  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s",
                "  obs: turn_done: failed: interrupted by main",
                "  obs: turn_stalled: budget",
                "core rejected: no pending completion",
                "sb warn : the inbox is full",
                "sb spawn : docs started",
                "sb undelivered : perf : then \\: the warm run",
            ]),
        ),
        // a plain turn: his words, a tool call that ends, the reply (one
        // time: the entries' durations are the lines' times, the line
        // path's its own clock, 0.0s in a test)
        (
            "turn",
            at_once(&[
                "sb you : run the tests",
                "  obs: turn_started",
                "  obs: tool_started #1",
                "tool #1 bash : cargo test -q",
                "tool_intent #1 : running the tests",
                "  obs: tool_finished #1 ok",
                "tool_result #1 ok : 12 passed",
                "  obs: assistant: all 12 pass.",
                "  obs: turn_done: completed",
            ]),
        ),
        // words only
        ("words", mk(&["sb you : hi", "  obs: turn_started", "  obs: assistant: hello.", "  obs: turn_done: completed"])),
        // cards (architect m_15013): a question with its options, a blocker,
        // a done and a failed one (one line each), all still open on the hub
        (
            "cards",
            mk(&[
                "sb card : #9 question @perf : which bench?\\n1. cold\\n2. warm",
                "sb card : #4 blocked @api-v2 : the schema file isn't in the repo",
                "sb card : #3 done @bench : p95 at 180 ms",
                "sb card : #5 failed @ci : the e2e run",
                "sb card : #7 drop @mig-db : drop the v1 tables now?",
                // BISE-31: closed cards fade with the hub's word
                "sb card-closed : #9 answered via @docs",
                "sb card-closed : #7 accepted",
                "sb card-closed : #77 closed",
            ]),
        ),
        // F2 (tui-parity m_15345): the approvals gate holds a push, its
        // card waits on him (the row says `? waiting for you`; the
        // confirm card itself is the inbox's). One time for every line:
        // the line path's durations are its own clock (0.0s in a test)
        (
            "approval-wait",
            at_once(&[
                "sb you : push it",
                "  obs: turn_started",
                "  obs: tool_started #1",
                "tool #1 bash : git push origin main --force",
                "sb gate : check 1",
                "sb gate : card 12",
                "sb card : #12 confirm @main : git push origin main --force",
            ]),
        ),
        // … he allows it, the push fails: one row, its exit code and its
        // output line, never a second `#1 ✗` row
        (
            "approval-done",
            at_once(&[
                "sb you : push it",
                "  obs: turn_started",
                "  obs: tool_started #1",
                "tool #1 bash : git push origin main --force",
                "sb gate : check 1",
                "sb gate : card 12",
                "sb card : #12 confirm @main : git push origin main --force",
                "sb approval : allowed : main : git push origin main --force : ",
                "sb gate : done 1",
                "tool_result #1 fail : exit 128: fatal: 'origin' does not appear to be a git repository",
                "  obs: tool_finished #1 fail",
                "  obs: assistant: the push failed.",
                "  obs: turn_done: completed",
            ]),
        ),
        // `$ sb card …` rows: the result line comes before the finish
        (
            "sb-card-rows",
            at_once(&[
                "  obs: turn_started",
                "  obs: tool_started #1",
                "tool #1 bash : sb card \"which bench?\" --option cold --option warm",
                "tool_result #1 ok : #4",
                "  obs: tool_finished #1 ok",
                "  obs: tool_started #2",
                "tool #2 bash : sb card \"ship it?\"",
                "tool_result #2 ok : #5",
                "  obs: tool_finished #2 ok",
                "  obs: turn_done: completed",
            ]),
        ),
        // a reloaded transcript's call, its tool_code included
        (
            "reload",
            at_once(&[
                "sb you : bash ls",
                "  obs: turn_started",
                "  obs: tool_started #3",
                "tool #3 bash : ls",
                "tool_code #3 : ls",
                "tool_result #3 ok : a.rs\\Nb.rs",
                "  obs: tool_finished #3 ok",
                "  obs: assistant: two files.",
                "  obs: turn_done: completed",
            ]),
        ),
        // a retried turn: the provider's retry notice, then the reply
        (
            "retry",
            mk(&[
                "sb you : again",
                "  obs: turn_started",
                "  obs: provider_retry: 1/10 · provider 529 (transient) · retry in 2s",
                "  obs: assistant: done this time.",
                "  obs: turn_done: completed",
            ]),
        ),
    ]
}

/// The feed's rows of `events`, as text (the feed's own path, its tool
/// box included).
fn rows(events: &[Ev]) -> Vec<String> {
    (0..events.len())
        .flat_map(|i| event_rows(events, i, false, WIDTH, 0).rows)
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string())
        .collect()
}

/// The feed the TUI's own fold of `lines` draws (`run::ingest_line`,
/// wire.rs rec_ev: the line path, without the hub's `line` arm the
/// switch deleted).
fn from_lines(lines: &[Line]) -> Vec<String> {
    let mut app = test_app();
    for (_, ts, l) in lines {
        crate::run::ingest_line(&mut app, l.clone(), Some(*ts));
    }
    rows(&app.events)
}

/// The feed of the hub's fold of `lines`.
fn from_entries(lines: &[Line]) -> Vec<String> {
    let none = |_: &str| None;
    let name = |id: &str, _: &str| id.to_string();
    let width = |s: &str| unicode_width::UnicodeWidthStr::width(s);
    let offset = |_: u64| 0;
    // the corpora's cards are open on the hub (the line path draws them open)
    let ctx = Ctx { open_cards: &[9, 4, 3, 5, 7], page: &none, provider: &name, width: &width, offset: &offset, attached: &bise_proto::thread::Attached::plain };
    let (mut events, mut cache) = (Vec::new(), Vec::new());
    for e in fold(lines, &ctx) {
        for ev in ev_of(&e) {
            push_event(&mut events, &mut cache, ev);
        }
    }
    rows(&events)
}

/// The first row where `a` and `b` differ, said for a person.
fn first_diff(a: &[String], b: &[String]) -> Option<String> {
    let n = a.len().max(b.len());
    (0..n).find(|i| a.get(*i) != b.get(*i)).map(|i| format!("row {i}: lines {:?} vs entries {:?}", a.get(i), b.get(i)))
}

#[test]
fn the_hubs_fold_draws_the_feed_the_lines_draw() {
    let mut report = Vec::new();
    for (name, lines) in corpora() {
        let (a, b) = (from_lines(&lines), from_entries(&lines));
        let diff = first_diff(&a, &b);
        match (diff, GAPS.contains(&name)) {
            (Some(d), false) => report.push(format!("{name}: differs and isn't a named gap: {d}")),
            (None, true) => report.push(format!("{name}: the same now, take it out of GAPS")),
            (Some(d), true) => {
                eprintln!("gap {name}: {d}");
                // both feeds whole, to read the gap (--nocapture shows them)
                {
                    eprintln!("-- lines:\n{}\n-- entries:\n{}", a.join("\n"), b.join("\n"));
                }
            }
            (None, false) => {}
        }
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}
