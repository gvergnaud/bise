//! REPL starts (pure): how many at once, in what order, and when one
//! has failed. The hub's side is `daemon/repl_starts.rs`.
//!
//! Every start goes through one limiter: a fresh spawn (a boot, a crash
//! restart), a switch's reload (a new version, changed keys or skills)
//! and a recycle. At most [`MAX_IN_FLIGHT`] at once: after a switch to
//! 7fc455f7 the hub reloaded 22 REPLs in one tick, their starts took
//! 22-38 s at load 16, and a flat 20 s limit failed the switch.
//!
//! - Order ([`order`]): a REPL the user's input waits on (main, his
//!   focus, queued messages) first, then fresh starts before switches,
//!   oldest first; recycles always last (they may wait as long as needed).
//! - Failure: a start in flight fails after [`START_STILL_MS`] without
//!   progress (its phases: the thread runs, the tools note, AGENTS.md,
//!   the process spawned, its banner; crate::watch, the boot's rule too),
//!   never after a fixed time in all.
//! - A stall is not a death: a slow start on a loaded machine does not
//!   fail a version's probation. But a version whose REPLs never start
//!   must roll back: a stall reports to probation ([`Stall::reports`])
//!   when the same REPL stalls twice in a row, or when more than half of
//!   the REPLs this hub started (3 or more) stalled.

use crate::watch::Watch;
use std::collections::{BTreeMap, BTreeSet};

/// REPL starts in flight at once.
pub const MAX_IN_FLIGHT: usize = 4;
/// A start without progress for this long has failed.
pub const START_STILL_MS: u64 = 45_000;

/// What a start is (its place in the order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A fresh process: a boot, a crash restart, a new agent.
    Start,
    /// A live REPL moved to another binary or reloaded.
    Switch,
    /// A live REPL restarted because it aged (crate::recycle).
    Recycle,
}

/// A start waiting for a slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    pub dir: String,
    pub kind: Kind,
    /// The user's input waits on it.
    pub urgent: bool,
    pub asked_ms: u64,
}

/// The order slots go in: recycles last; then the urgent ones; then
/// fresh starts before switches; then the oldest.
pub fn order(mut c: Vec<Cand>) -> Vec<Cand> {
    c.sort_by(|a, b| {
        let key = |x: &Cand| (x.kind == Kind::Recycle, !x.urgent, x.kind, x.asked_ms);
        key(a).cmp(&key(b)).then_with(|| a.dir.cmp(&b.dir))
    });
    c
}

/// A start that stalled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stall {
    pub dir: String,
    pub kind: Kind,
    pub secs: u64,
    /// A reason for a probation to roll back (see the module header).
    pub reports: bool,
}

/// The starts in flight, and the stalls of this hub.
#[derive(Debug, Default)]
pub struct Starts {
    flight: BTreeMap<String, (Kind, Watch)>,
    /// stalls in a row, by REPL (a start that connects resets it)
    row: BTreeMap<String, u32>,
    started: BTreeSet<String>,
    stalled: BTreeSet<String>,
}

/// A stall reports to probation (see the module header).
pub fn stall_reports(in_a_row: u32, stalled: usize, started: usize) -> bool {
    in_a_row >= 2 || (started >= 3 && stalled * 2 > started)
}

impl Starts {
    /// Slots free now.
    pub fn free(&self) -> usize {
        MAX_IN_FLIGHT.saturating_sub(self.flight.len())
    }

    pub fn in_flight(&self, dir: &str) -> bool {
        self.flight.contains_key(dir)
    }

    pub fn len(&self) -> usize {
        self.flight.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flight.is_empty()
    }

    /// A start takes a slot (admitted by [`order`] and [`Starts::free`]).
    pub fn begin(&mut self, dir: &str, kind: Kind, now_ms: u64) {
        self.started.insert(dir.to_string());
        self.flight.insert(dir.to_string(), (kind, Watch::new(now_ms, START_STILL_MS)));
    }

    /// One of its phases is done.
    pub fn progress(&mut self, dir: &str, now_ms: u64) {
        if let Some((_, w)) = self.flight.get_mut(dir) {
            w.moved(now_ms);
        }
    }

    /// It connected: started, its slot free, its stalls forgotten.
    pub fn connected(&mut self, dir: &str) {
        self.flight.remove(dir);
        self.row.remove(dir);
    }

    /// It ended otherwise (died, killed, dropped): its slot free.
    pub fn ended(&mut self, dir: &str) {
        self.flight.remove(dir);
    }

    /// The starts without progress for START_STILL_MS: out of flight,
    /// counted, each said once.
    pub fn stalled(&mut self, now_ms: u64) -> Vec<Stall> {
        let late: Vec<(String, Kind, u64)> = self
            .flight
            .iter()
            .filter(|(_, (_, w))| w.stalled(now_ms))
            .map(|(d, (k, w))| (d.clone(), *k, w.still_ms(now_ms) / 1000))
            .collect();
        let mut out = Vec::new();
        for (dir, kind, secs) in late {
            self.flight.remove(&dir);
            self.stalled.insert(dir.clone());
            let n = self.row.entry(dir.clone()).or_insert(0);
            *n += 1;
            let reports = stall_reports(*n, self.stalled.len(), self.started.len());
            out.push(Stall { dir, kind, secs, reports });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(dir: &str, kind: Kind, urgent: bool, at: u64) -> Cand {
        Cand { dir: dir.into(), kind, urgent, asked_ms: at }
    }

    #[test]
    fn his_input_first_then_fresh_starts_oldest_first_and_recycles_last() {
        let got: Vec<String> = order(vec![
            cand("recycled-main", Kind::Recycle, true, 0),
            cand("old-switch", Kind::Switch, false, 1),
            cand("new-start", Kind::Start, false, 9),
            cand("old-start", Kind::Start, false, 2),
            cand("focused-switch", Kind::Switch, true, 8),
            cand("recycled", Kind::Recycle, false, 0),
        ])
        .into_iter()
        .map(|c| c.dir)
        .collect();
        assert_eq!(got, ["focused-switch", "old-start", "new-start", "old-switch", "recycled-main", "recycled"]);
    }

    #[test]
    fn at_most_four_in_flight_and_a_slot_frees_when_one_connects_or_ends() {
        let mut s = Starts::default();
        for (i, d) in ["a", "b", "c"].iter().enumerate() {
            s.begin(d, Kind::Switch, i as u64);
        }
        assert_eq!(s.free(), 1);
        s.begin("d", Kind::Start, 3);
        assert_eq!(s.free(), 0);
        s.connected("a");
        s.ended("b");
        assert_eq!(s.free(), 2);
        assert!(s.in_flight("c") && !s.in_flight("a"));
    }

    #[test]
    fn a_slow_start_that_moves_never_stalls() {
        // 22 starts at load 16 took 22-38 s: each phase moves the watch
        let mut s = Starts::default();
        s.begin("amb-kit", Kind::Switch, 0);
        for t in (10_000..300_000).step_by(30_000) {
            s.progress("amb-kit", t);
            assert!(s.stalled(t + 29_000).is_empty(), "at {} ms", t);
        }
        assert!(s.in_flight("amb-kit"));
    }

    #[test]
    fn one_stall_does_not_fail_a_probation_the_same_repl_twice_in_a_row_does() {
        let mut s = Starts::default();
        for d in ["a", "b", "c", "d"] {
            s.begin(d, Kind::Switch, 0);
        }
        s.progress("b", 40_000);
        s.progress("c", 40_000);
        s.progress("d", 40_000);
        let st = s.stalled(45_000);
        assert_eq!(st, [Stall { dir: "a".into(), kind: Kind::Switch, secs: 45, reports: false }]);
        assert!(!s.in_flight("a"), "a stalled start leaves its slot");
        // restarted, it stalls again: a reason to roll back
        s.begin("a", Kind::Start, 50_000);
        for d in ["b", "c", "d"] {
            s.progress(d, 90_000);
        }
        let st = s.stalled(95_000);
        assert!(st.len() == 1 && st[0].reports, "{:?}", st);
        // a start that connects forgets its stalls
        s.begin("a", Kind::Start, 100_000);
        s.connected("a");
        s.begin("a", Kind::Start, 110_000);
        s.progress("b", 150_000);
        s.progress("c", 150_000);
        s.progress("d", 150_000);
        assert!(!s.stalled(155_000)[0].reports);
    }

    #[test]
    fn more_than_half_of_the_starts_stalled_is_a_reason_to_roll_back() {
        assert!(!stall_reports(1, 1, 1), "one REPL, one stall: not yet");
        assert!(!stall_reports(1, 2, 4));
        assert!(stall_reports(1, 3, 4));
        assert!(stall_reports(2, 1, 22));
        // a version whose REPLs never start: every one stalls
        let mut s = Starts::default();
        for d in ["a", "b", "c", "d"] {
            s.begin(d, Kind::Switch, 0);
        }
        let st = s.stalled(45_000);
        assert_eq!(st.iter().map(|x| x.reports).collect::<Vec<_>>(), [false, false, true, true]);
    }
}
