//! What an agent is doing now (les bims, architect m_27581): the pure
//! part of its agents row's `doing`. [`next`] moves it on each REPL line
//! the hub reads ([`crate::wire::Wire`]); [`row`] is what the row says.
//! The hub keeps the value on `model::Agent::doing` (runtime only, like
//! `activity`: lost on a restart by design) and `core_doing.rs` calls
//! [`next`]; the row rides the state gate (100 ms), no push of its own.
//!
//! The kinds: `bise_proto::rows::DoingKind`, a tool call's by
//! `DoingKind::of_call` (the one mapping). "build" needs no timer: a
//! build or test command (of_call), or a pause on a backgrounded
//! command; "long" is the client's, from `since_ms`.

use crate::wire::Wire;
use bise_proto::rows::{AgentWatch, Doing, DoingKind, WatchKind, uses_of};

/// `cur` after `line` at `now`. A kind that doesn't change keeps its
/// `since_ms` (a burst of bash calls is one row change).
pub fn next(cur: Option<Doing>, line: &Wire, now: u64) -> Option<Doing> {
    let (kind, uses) = match line {
        // a new turn starts fresh (a crashed one left its step behind)
        Wire::TurnStarted => return Some(Doing { kind: DoingKind::Think, since_ms: now, uses: None }),
        Wire::ToolResult | Wire::CompactionEnded => (DoingKind::Think, None),
        Wire::Assistant(t) if !t.is_empty() => (DoingKind::Think, None),
        // the call's kind, and the names of what it uses (never its args: rows::doing::uses_of)
        Wire::Tool { name, args } => (DoingKind::of_call(name, args), uses_of(name, args)),
        Wire::CompactionStarted => (DoingKind::Compact, None),
        Wire::TurnDone(_) | Wire::Idle => return None,
        _ => return cur,
    };
    // the same kind keeps its since; a new `uses` is a new step (a new place for the bim)
    match cur {
        Some(d) if d.kind == kind && d.uses == uses => Some(d),
        _ => Some(Doing { kind, since_ms: now, uses }),
    }
}

/// What the row says: `cur` while its turn runs; else, paused on a
/// backgrounded command (a `resumes` bg watch), a build since that
/// watch; else none (a stop, a crash, a kill never leave a stale one).
pub fn row(turn_running: bool, cur: Option<Doing>, watching: &[AgentWatch]) -> Option<Doing> {
    if turn_running {
        return cur;
    }
    watching
        .iter()
        .filter(|w| w.kind == WatchKind::Bg && w.resumes)
        .map(|w| w.since_ms)
        .min()
        .map(|since_ms| Doing { kind: DoingKind::Build, since_ms, uses: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::parse;

    fn run(lines: &[(&str, u64)]) -> Vec<Option<(DoingKind, u64)>> {
        let mut cur = None;
        lines
            .iter()
            .map(|(l, t)| {
                cur = next(cur.clone(), &parse(l), *t);
                cur.as_ref().map(|d| (d.kind, d.since_ms))
            })
            .collect()
    }

    #[test]
    fn a_turn_thinks_calls_tools_and_ends() {
        use DoingKind::*;
        let got = run(&[
            ("  obs: turn_started", 10),
            ("  obs: assistant: looking", 11),
            ("tool #1 bash : ls", 12),
            ("tool_result #1 ok : a b", 13),
            ("tool #2 edit : {\"file_path\":\"a.rs\"}", 14),
            ("tool_result #2 ok : done", 15),
            ("tool #3 bash : cargo test -p x", 16),
            ("tool_intent #3 : running the tests", 17),
            ("tool_result #3 ok : ok", 18),
            ("  obs: compaction_started #4 (auto)", 19),
            ("  obs: compaction_done: ## Who I am", 20),
            ("  obs: turn_done: completed", 21),
            ("--- idle", 22),
        ]);
        assert_eq!(
            got,
            vec![
                Some((Think, 10)),
                Some((Think, 10)),
                Some((Tool, 12)),
                Some((Think, 13)),
                Some((Edit, 14)),
                Some((Think, 15)),
                Some((Build, 16)),
                Some((Build, 16)),
                Some((Think, 18)),
                Some((Compact, 19)),
                Some((Think, 20)),
                None,
                None,
            ]
        );
    }

    #[test]
    fn the_same_kind_keeps_its_since() {
        let got = run(&[("tool #1 bash : ls", 5), ("tool #2 bash : pwd", 9)]);
        assert_eq!(got[1], Some((DoingKind::Tool, 5)));
    }

    /// A plugin's call says what it uses until its result; a call to
    /// another plugin is a new step even of the same kind (the bim goes on
    /// to that object), the same plugin keeps its since (one trip).
    #[test]
    fn a_plugin_call_says_what_it_uses_until_its_result() {
        use bise_proto::rows::{Uses, UsesKind};
        let mut cur = None;
        let mut step = |l: &str, t: u64| {
            cur = next(cur.clone(), &parse(l), t);
            cur.clone().map(|d| (d.kind, d.since_ms, d.uses.map(|u| u.name)))
        };
        let gmail = Some("gmail".to_string());
        assert_eq!(step("tool #1 run_typescript : return tools.gmail.search({})", 10), Some((DoingKind::Tool, 10, gmail.clone())));
        assert_eq!(step("tool #2 run_typescript : return tools.gmail.read({})", 12), Some((DoingKind::Tool, 10, gmail)));
        assert_eq!(step("tool #3 run_typescript : return tools.slack.post({})", 14), Some((DoingKind::Tool, 14, Some("slack".to_string()))));
        assert_eq!(step("tool #4 bash : ls", 15), Some((DoingKind::Tool, 15, None)));
        assert_eq!(step("tool #5 skill : {\"name\":\"computer:computer-use\"}", 16), Some((DoingKind::Tool, 16, Some("computer-use".to_string()))));
        assert_eq!(step("tool_result #5 ok : loaded", 17), Some((DoingKind::Think, 17, None)));
        let u = next(None, &parse("tool #6 skill : {\"name\":\"computer:computer-use\"}"), 20).and_then(|d| d.uses);
        assert_eq!(u, Some(Uses { kind: UsesKind::Skill, plugin: Some("computer".into()), name: "computer-use".into() }));
    }

    #[test]
    fn a_new_turn_starts_fresh() {
        let stale = Some(Doing { kind: DoingKind::Think, since_ms: 3, uses: None });
        assert_eq!(next(stale, &parse("  obs: turn_started"), 50), Some(Doing { kind: DoingKind::Think, since_ms: 50, uses: None }));
    }

    #[test]
    fn other_lines_change_nothing() {
        let cur = Some(Doing { kind: DoingKind::Edit, since_ms: 3, uses: None });
        for l in ["history tool #1 bash : ls", "obs: steered: x", "something else", "  obs: assistant: "] {
            assert_eq!(next(cur.clone(), &parse(l), 99), cur, "{l}");
        }
    }

    fn watch(kind: WatchKind, resumes: bool, since_ms: u64) -> AgentWatch {
        AgentWatch { id: 1, kind, what: "cargo test".into(), since_ms, noted: false, resumes, until_ms: None }
    }

    /// The row's invariant: none unless its turn runs, but for a pause on
    /// a backgrounded command (a build since that watch).
    #[test]
    fn the_row_says_doing_only_while_the_turn_runs() {
        let cur = Some(Doing { kind: DoingKind::Edit, since_ms: 3, uses: None });
        assert_eq!(row(true, cur.clone(), &[]), cur);
        assert_eq!(row(false, cur.clone(), &[]), None, "a stopped, crashed or idle agent");
        assert_eq!(row(false, None, &[watch(WatchKind::File, true, 4), watch(WatchKind::Bg, false, 5)]), None, "an alarm is no build");
        assert_eq!(
            row(false, cur, &[watch(WatchKind::Bg, true, 8), watch(WatchKind::Bg, true, 6)]),
            Some(Doing { kind: DoingKind::Build, since_ms: 6, uses: None })
        );
        assert_eq!(row(true, None, &[watch(WatchKind::Bg, true, 8)]), None, "in a turn, the turn's own step");
    }
}
