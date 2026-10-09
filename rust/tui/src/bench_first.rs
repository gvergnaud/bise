//! client-protocol step 4's bench (architect m_13977 Q4), before and
//! after the terminal reads typed notifications:
//! (a) the first frame of a 20-agent hub (the hello burst: state, every
//! feed's lines, ready, then one draw), (b) the first frame of one 10k-line
//! thread in view, (c) the cost of one live line in view (dispatch + a
//! frame, the loop's worst case). In-process (the TUI's dispatch and draw,
//! a TestBackend), so the numbers are the terminal's own work.
//!
//! `cargo test --release -p bend-tui bench_first -- --ignored --nocapture`

use crate::sb::bench::test_app;
use crate::sb::{dispatch, draw_sb};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;
use std::time::Instant;

/// One turn's lines, as a feed holds them (his words, a tool call, the
/// reply), `k` its number.
fn turn(k: usize) -> Vec<String> {
    vec![
        format!("sb you : step {k}: run the tests and tell me"),
        "  obs: turn_started".into(),
        format!("  obs: tool_started: {k}"),
        format!("tool #{k} bash : cargo test -q -p bend-tui {k}"),
        format!("tool_intent #{k} : running the tests"),
        format!("  obs: tool_finished: {k} ok"),
        format!("tool_result #{k} ok : test result: ok. {k} passed"),
        format!("  obs: assistant: all **{k}** pass. the change in `feed.rs` keeps the rows the same.\\nnext: the bench."),
        "  obs: turn_done: completed".into(),
    ]
}

fn lines(n: usize) -> Vec<String> {
    (0..).flat_map(turn).take(n).collect()
}

fn agent(name: &str) -> bise_proto::rows::Agent {
    let mut a = crate::sb::hub_reads::rows_for_tests::agent(name, "idle", &format!("{name}'s job"));
    (a.mode, a.path, a.created_ms) = (Some(bise_proto::rows::AgentMode::Worktree), format!("/ws/{name}"), Some(1_700_000_000_000));
    a
}

/// The hello burst of a hub with `agents` agents (main first) of `each`
/// lines, as `client_hello` writes it.
fn burst(agents: usize, each: usize) -> Vec<String> {
    let names: Vec<String> = (0..agents).map(|i| if i == 0 { "main".to_string() } else { format!("t{i}") }).collect();
    let mut out = vec![json!({"ev": "hello", "workspace": "/ws", "exe": "", "version": {"id": "bench"}, "reload": ""}).to_string()];
    // hub/agents and hub/cards, typed (P4c-4b: the older state line split by kind)
    out.extend(crate::sb::hub_reads::rows_for_tests::lines(names.iter().map(|n| agent(n)).collect(), vec![]));
    for n in &names {
        for (i, l) in lines(each).iter().enumerate() {
            out.push(json!({"ev": "line", "agent": n, "line": l, "pos": i + 1, "ts": 1_700_000_000_000u64 + i as u64}).to_string());
        }
    }
    out.push(json!({"ev": "approvals", "mode": "auto", "rules": []}).to_string());
    out.push(json!({"ev": "ready"}).to_string());
    out
}

/// Milliseconds from the burst's first line to its first frame drawn.
fn first_frame(burst: &[String]) -> f64 {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(150, 42)).unwrap();
    let t = Instant::now();
    for l in burst {
        dispatch(&mut app, l);
    }
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
    let b20 = burst(20, 200);
    let a = median((0..7).map(|_| first_frame(&b20)).collect());
    eprintln!("(a) first frame, 20 agents x 200 lines ({} burst lines): {a:.1} ms (median of 7)", b20.len());
    let b10k = burst(1, 10_000);
    let b = median((0..5).map(|_| first_frame(&b10k)).collect());
    eprintln!("(b) first frame, one 10k-line thread in view: {b:.1} ms (median of 5)");
    // (c) live lines in view after (a)'s burst, a frame after each
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(150, 42)).unwrap();
    for l in &b20 {
        dispatch(&mut app, l);
    }
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let live: Vec<String> = lines(2_000).iter().enumerate().map(|(i, l)| json!({"ev": "line", "agent": "main", "line": l, "pos": 201 + i, "ts": 1_800_000_000_000u64 + i as u64}).to_string()).collect();
    let t = Instant::now();
    for l in &live {
        dispatch(&mut app, l);
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
    }
    let c = t.elapsed().as_secs_f64() * 1e6 / live.len() as f64;
    eprintln!("(c) one live line in view (dispatch + frame): {c:.0} us per line (2000 lines)");
}
