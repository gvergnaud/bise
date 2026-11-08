//! A REPL asked to stop (TERM, INT, HUP: the hub's stop, `sb restart`, a
//! teardown, a `kill`) is not a crash (hub/stall.bend, architect
//! m_48209): the shell's `stopped` fact in, sb-core's crash count and
//! words out.

use super::*;

/// A stop of `agent`'s REPL, its `row`-th with no connect between.
fn stopped(agent: &str, row: u64) -> Input {
    Input::ReplExited { agent: agent.into(), crashed: true, reason: "signal: 15 (SIGTERM)".into(), stalled: false, loaded: false, row, stopped: true }
}

/// What the hub said to `to` so far.
fn told(t: &T, to: &str) -> Vec<String> {
    t.hub.st.msgs().values().filter(|m| m.to == to && m.from == "switchboard").map(|m| t.hub.st.text(m.id)).collect()
}

fn spawns(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "a"))
}

/// keep-current's 22:32 (m_48183): an idle or done agent stopped by a
/// TERM starts again on its checkpoint and nobody hears of it; its stops
/// never count, so a crash after five of them is still attempt 1/5.
#[test]
fn an_idle_agent_stopped_starts_again_and_main_hears_nothing() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    let before = told(&t, MAIN).len();
    for _ in 0..5 {
        assert!(spawns(&t.go(stopped("a", 1))), "it starts again");
        t.go(Input::ReplReady { agent: "a".into() });
    }
    assert_eq!(told(&t, MAIN).len(), before, "main hears nothing of a stop: {:?}", told(&t, MAIN));
    assert!(told(&t, "a").iter().all(|m| !m.contains("stopped")), "an idle agent is told nothing");
    assert_ne!(t.status("a"), Status::Failed);
    t.turn(MAIN, "vu");
    t.go(exited("a", "boom"));
    let said = told(&t, MAIN);
    assert!(said.iter().any(|m| m.contains("agent @a crashed (boom)") && m.contains("attempt 1/5")), "{:?}", said);
}

/// An agent cut mid-turn by a stop gets one line that says what
/// happened; main hears nothing.
#[test]
fn an_agent_stopped_mid_turn_gets_one_line_and_main_nothing() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    t.go(Input::ReplLine { agent: "a".into(), line: "  obs: turn_started".into() });
    let before = told(&t, MAIN).len();
    assert!(spawns(&t.go(stopped("a", 1))));
    let lines: Vec<String> = told(&t, "a").into_iter().filter(|m| m.contains("stopped mid-turn")).collect();
    assert_eq!(lines.len(), 1, "{:?}", told(&t, "a"));
    assert!(lines[0].contains("SIGTERM") && lines[0].contains("checkpoint"), "{}", lines[0]);
    assert_eq!(told(&t, MAIN).len(), before, "{:?}", told(&t, MAIN));
}

/// Something that keeps killing a REPL before it is ready is a fault:
/// the K()-th stop in a row (3) counts as a crash does, and main hears it.
#[test]
fn the_kth_stop_with_no_ready_between_counts() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    for row in 1..=2 {
        t.go(stopped("a", row));
    }
    assert!(told(&t, MAIN).iter().all(|m| !m.contains("agent @a crashed")), "{:?}", told(&t, MAIN));
    t.turn(MAIN, "vu");
    t.go(stopped("a", 3));
    let said = told(&t, MAIN);
    assert!(said.iter().any(|m| m.contains("agent @a crashed") && m.contains("attempt 1/5")), "{:?}", said);
}
