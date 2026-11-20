//! The hub's own git polls, paced (git-calm, architect m_67753): the
//! worktrees scan (daemon/worktrees.rs) and an agent's Changes
//! (daemon/art.rs). At load 124 on 12 cores the dashboard hub ran a scan
//! thread per agent status flip, each walking every worktree, so the
//! same worktree got many `git status` at once, 30-40 s each.
//!
//! - [`Scans`] (pure, the hub's instance owned by the loop): one scan in
//!   flight per hub; an ask while one runs, or inside the gap after the
//!   last one, waits, the asks merged into one rerun. It keeps each
//!   worktree's last facts, so a row the scan skipped never reads clean.
//! - [`Slots`]: the worktrees a poll's git runs in now, shared by the
//!   scan and the Changes threads; a poll whose worktree is held skips
//!   it. Lands, drops, the sweep and the PR poller never claim: they
//!   never wait or skip.
//! - [`scan_gap`] and [`changes_gap`]: the pause between two polls, on
//!   the machine's load ([`crate::load::factor`]), longer from
//!   [`crate::load::LOADED`].
//!
//! Agents' own temp worktrees are never polled
//! (`bise_home::hub_state::is_agent_tmp`, the scan drops them).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The gap after a scan when the machine is not loaded: a burst of
/// status flips makes one scan, not one each.
pub const SCAN_GAP: Duration = Duration::from_secs(2);
/// The gap after a scan per load factor when the machine is loaded (30 s
/// at 2 per core, 2 min at the cap).
pub const SCAN_GAP_LOADED: Duration = Duration::from_secs(15);
/// The gap between two Changes of one agent after its edits (at its
/// idle: none), times the load factor when loaded.
pub const CHANGES_GAP: Duration = Duration::from_secs(5);

/// The gap after a worktrees scan at load `factor`.
pub fn scan_gap(factor: f64) -> Duration {
    if crate::load::is_loaded(factor) {
        SCAN_GAP_LOADED.mul_f64(factor)
    } else {
        SCAN_GAP
    }
}

/// The gap between two Changes of one agent after its edits at load
/// `factor`.
pub fn changes_gap(factor: f64) -> Duration {
    if crate::load::is_loaded(factor) {
        CHANGES_GAP.mul_f64(factor)
    } else {
        CHANGES_GAP
    }
}

/// A worktree's facts: (dirty, ahead, behind).
pub type Facts = (bool, u32, u32);

/// The hub's worktrees scans: one in flight, the asks merged, the last
/// facts of each worktree. `T` is who gets the scan's event (a client's
/// answer, or every client).
#[derive(Debug)]
pub struct Scans<T> {
    running: bool,
    /// who asked since the scan in flight (or the gap) started
    pending: Vec<T>,
    /// when the last scan ended (ms)
    ended_ms: Option<u64>,
    /// each listed worktree's last facts, by its path
    last: HashMap<String, Facts>,
}

impl<T> Default for Scans<T> {
    fn default() -> Self {
        Scans { running: false, pending: Vec::new(), ended_ms: None, last: HashMap::new() }
    }
}

impl<T: PartialEq> Scans<T> {
    /// `to` asks for a scan at `now_ms`: the targets to scan for now, or
    /// None (it waits for the scan in flight or the gap; [`Scans::due`]
    /// starts it).
    pub fn ask(&mut self, to: T, now_ms: u64, gap: Duration) -> Option<Vec<T>> {
        if !self.pending.contains(&to) {
            self.pending.push(to);
        }
        self.due(now_ms, gap)
    }

    /// The pending targets when a scan may start now (none in flight, the
    /// gap since the last one passed); the scan is then in flight.
    pub fn due(&mut self, now_ms: u64, gap: Duration) -> Option<Vec<T>> {
        let rested = self.ended_ms.is_none_or(|e| now_ms >= e.saturating_add(gap.as_millis() as u64));
        if self.running || self.pending.is_empty() || !rested {
            return None;
        }
        self.running = true;
        Some(std::mem::take(&mut self.pending))
    }

    /// The scan in flight ended at `now_ms` with each listed worktree's
    /// facts: they replace the last ones (a worktree no longer listed
    /// leaves).
    pub fn done(&mut self, now_ms: u64, facts: HashMap<String, Facts>) {
        self.running = false;
        self.ended_ms = Some(now_ms);
        self.last = facts;
    }

    /// The last facts, for a scan to reuse on a worktree it skips.
    pub fn last(&self) -> &HashMap<String, Facts> {
        &self.last
    }

    /// Someone waits for a scan.
    pub fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// The worktrees a poll's git runs in now. The lock is held only to
/// insert and remove, never across a git call.
#[derive(Clone, Debug, Default)]
pub struct Slots(Arc<Mutex<HashSet<PathBuf>>>);

/// A worktree claimed by a poll: free again when dropped.
#[derive(Debug)]
pub struct Held {
    slots: Slots,
    dir: PathBuf,
}

impl Slots {
    /// Claim `dir` for one poll; None: another poll's git runs there now,
    /// this poll skips it.
    pub fn claim(&self, dir: &Path) -> Option<Held> {
        let fresh = self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf());
        fresh.then(|| Held { slots: self.clone(), dir: dir.to_path_buf() })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.slots.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: Duration = Duration::from_secs(2);

    #[test]
    fn an_ask_while_a_scan_runs_waits_and_all_the_waiting_asks_make_one_rerun() {
        let mut s: Scans<&str> = Scans::default();
        assert_eq!(s.ask("all", 0, GAP), Some(vec!["all"]), "nothing in flight: it runs");
        assert_eq!(s.ask("all", 10, GAP), None, "one in flight: no second scan");
        assert_eq!(s.ask("c1", 20, GAP), None);
        assert_eq!(s.ask("all", 30, GAP), None, "the same target once");
        assert_eq!(s.due(40, GAP), None, "still in flight");
        s.done(5_000, HashMap::new());
        assert_eq!(s.due(6_000, GAP), None, "inside the gap");
        assert_eq!(s.due(7_000, GAP), Some(vec!["all", "c1"]), "one rerun for every waiting ask");
        assert_eq!(s.due(7_001, GAP), None, "and nothing more");
        s.done(8_000, HashMap::new());
        assert_eq!(s.due(20_000, GAP), None, "nobody asked: no scan");
    }

    #[test]
    fn a_done_scan_keeps_the_listed_worktrees_facts_only() {
        let mut s: Scans<()> = Scans::default();
        s.ask((), 0, GAP);
        s.done(1, HashMap::from([("/w/a".to_string(), (true, 1, 0)), ("/w/b".to_string(), (false, 0, 2))]));
        s.ask((), 10_000, GAP);
        s.done(10_001, HashMap::from([("/w/a".to_string(), (false, 2, 0))]));
        assert_eq!(s.last(), &HashMap::from([("/w/a".to_string(), (false, 2, 0))]), "b left the list: its entry too");
    }

    #[test]
    fn the_polls_space_out_when_the_machine_is_loaded() {
        assert_eq!(scan_gap(1.0), SCAN_GAP);
        assert_eq!(scan_gap(1.99), SCAN_GAP);
        assert_eq!(scan_gap(2.0), Duration::from_secs(30));
        assert_eq!(scan_gap(8.0), Duration::from_secs(120), "load 124 on 12 cores: every 2 min");
        assert_eq!(changes_gap(1.0), CHANGES_GAP);
        assert_eq!(changes_gap(2.0), Duration::from_secs(10));
        assert_eq!(changes_gap(8.0), Duration::from_secs(40));
    }

    #[test]
    fn a_held_worktree_is_skipped_until_its_poll_ends() {
        let slots = Slots::default();
        let a = slots.claim(Path::new("/w/a")).expect("free");
        assert!(slots.claim(Path::new("/w/a")).is_none(), "one poll per worktree");
        let b = slots.clone().claim(Path::new("/w/b"));
        assert!(b.is_some(), "another worktree is free");
        drop(a);
        assert!(slots.claim(Path::new("/w/a")).is_some(), "free once its poll ended");
    }
}
