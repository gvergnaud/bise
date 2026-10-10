//! client-protocol step 4's bench (architect m_13977 Q4), before and
//! after the terminal reads typed notifications:
//! (a) the first frame of a 20-agent hub (state rows, the focus's first
//! page as `thread/subscribe` answers it, then one draw; (a') with every
//! thread's page, the old burst's work), (b) the first frame of one
//! 10k-line thread in view (every entry in one page: the worst case),
//! (c) the cost of one live line in view (its `thread/entry`
//! notifications dispatched + a frame, the loop's worst case).
//! In-process (the TUI's dispatch and draw, a TestBackend): the hub's
//! fold runs before the clock starts, so the numbers are the terminal's
//! own work (JSON decode included).
//!
//! `cargo test --release -p bend-tui bench_first -- --ignored --nocapture`

use crate::sb::bench::test_app;
use crate::sb::entries_for_tests::{fold_lines, place_page, Hub};
use crate::sb::{dispatch, draw_sb};
use bise_proto::thread::{Entry, Line};
use bise_proto::{hub::HubEv, rpc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;
use std::time::Instant;

/// One turn's lines, as a thread holds them (his words, a tool call, the
/// reply), `k` its number.
fn turn(k: usize) -> Vec<String> {
    vec![
        format!("sb you : step {k}: run the tests and tell me"),
        "  obs: turn_started".into(),
        format!("  obs: tool_started #{k}"),
        format!("tool #{k} bash : cargo test -q -p bend-tui {k}"),
        format!("tool_intent #{k} : running the tests"),
        format!("  obs: tool_finished #{k} ok"),
        format!("tool_result #{k} ok : test result: ok. {k} passed"),
        format!("  obs: assistant: all **{k}** pass. the change in `feed.rs` keeps the rows the same.\\nnext: the bench."),
        "  obs: turn_done: completed".into(),
    ]
}

/// `n` lines from `pos` on, timed from `ts`.
fn lines(n: usize, pos: u64, ts: u64) -> Vec<Line> {
    (0..).flat_map(turn).take(n).enumerate().map(|(i, l)| (pos + i as u64, ts + i as u64, l)).collect()
}

fn agent(name: &str) -> bise_proto::rows::Agent {
    let mut a = crate::sb::hub_reads::rows_for_tests::agent(name, "idle", &format!("{name}'s job"));
    (a.mode, a.path, a.created_ms) = (Some(bise_proto::rows::AgentMode::Worktree), format!("/ws/{name}"), Some(1_700_000_000_000));
    a
}

/// What a terminal reads before its first frame: the hub's lines
/// (initialize's answer with hub/agents and hub/cards in its state, P4e-1)
/// and the first pages as JSON (`agent`, its entries).
struct Start {
    head: Vec<String>,
    pages: Vec<(String, String)>,
    tail: Vec<String>,
}

/// A hub of `agents` agents (main first, the focus) of `each` lines, the
/// first `paged` of them subscribed.
fn start(agents: usize, each: usize, paged: usize) -> Start {
    let names: Vec<String> = (0..agents).map(|i| if i == 0 { "main".to_string() } else { format!("t{i}") }).collect();
    let state: Vec<serde_json::Value> = crate::sb::hub_reads::rows_for_tests::lines(names.iter().map(|n| agent(n)).collect(), vec![])
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let res = json!({"project": "p", "proto": bise_proto::PROTO, "workspace": "/ws", "name": "ws", "exe": "", "version": {"id": "bench"},
        "reload": "", "methods": [], "notifications": [], "hub": {"watermark": {"epoch": 1, "seq": 0}, "state": state}});
    let head = vec![json!({"jsonrpc": "2.0", "id": 0, "result": res}).to_string()];
    let entries = serde_json::to_string(&fold_lines(&lines(each, 1, 1_700_000_000_000))).unwrap();
    let pages = names.iter().take(paged).map(|n| (n.clone(), entries.clone())).collect();
    let tail = Vec::new();
    Start { head, pages, tail }
}

fn read(app: &mut crate::App, s: &Start) {
    for l in &s.head {
        dispatch(app, l);
    }
    for (agent, page) in &s.pages {
        let entries: Vec<Entry> = serde_json::from_str(page).unwrap();
        place_page(app, agent, entries, false);
    }
    for l in &s.tail {
        dispatch(app, l);
    }
}

/// Milliseconds from the first line read to the first frame drawn.
fn first_frame(s: &Start) -> f64 {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(150, 42)).unwrap();
    let t = Instant::now();
    read(&mut app, s);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    t.elapsed().as_secs_f64() * 1000.0
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

#[test]
#[ignore]
fn bench_first() {
    let s20 = start(20, 200, 1);
    let a = median((0..7).map(|_| first_frame(&s20)).collect());
    eprintln!("(a) first frame, 20 agents x 200 lines, the focus's page: {a:.1} ms (median of 7)");
    let all = start(20, 200, 20);
    let a2 = median((0..7).map(|_| first_frame(&all)).collect());
    eprintln!("(a') first frame, 20 agents x 200 lines, every thread's page: {a2:.1} ms (median of 7)");
    let s10k = start(1, 10_000, 1);
    let b = median((0..5).map(|_| first_frame(&s10k)).collect());
    eprintln!("(b) first frame, one 10k-line thread in view: {b:.1} ms (median of 5)");
    // (c) live lines in view after (a)'s start, a frame after each; the
    // hub's entries for each line folded before the clock starts
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(150, 42)).unwrap();
    read(&mut app, &s20);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let mut hub = Hub::new();
    for (pos, ts, l) in lines(200, 1, 1_700_000_000_000) {
        hub.changes("main", pos, ts, &l);
    }
    let live: Vec<Vec<String>> = lines(2_000, 201, 1_800_000_000_000)
        .into_iter()
        .map(|(pos, ts, l)| {
            hub.changes("main", pos, ts, &l)
                .into_iter()
                .map(|e| {
                    let ev = HubEv::Entry { project: "p".into(), agent: "main".into(), entry: Box::new(e) };
                    rpc::Message::Notification(rpc::note(&ev, None).expect("an entry note")).to_value().to_string()
                })
                .collect()
        })
        .collect();
    let t = Instant::now();
    for notes in &live {
        for n in notes {
            dispatch(&mut app, n);
        }
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
    }
    let c = t.elapsed().as_secs_f64() * 1e6 / live.len() as f64;
    eprintln!("(c) one live line in view (its entry notes + frame): {c:.0} us per line (2000 lines)");
}
