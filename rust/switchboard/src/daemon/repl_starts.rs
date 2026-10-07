//! The hub's side of REPL starts (the decisions: crate::repl_start).
//! Every start takes a slot: a fresh spawn waits in `start_queue` until
//! one is free (`admit_starts`, at each request and tick), a switch or a
//! recycle is admitted by `switch_admitted` from what is left. Each phase
//! of a start is progress (`Msg::ReplStartStep`, `ReplSpawned`, the old
//! process's exit in a switch); `check_starts` ends a start without
//! progress for START_STILL_MS: its process killed, a `ReplGone` that
//! says whether the stall reports to probation (a death always does).

use super::{kill_pid, log_line, Msg, Shell};
use crate::repl_start::{order, Cand, Kind};
use crate::util::now_ms;

/// A fresh spawn waiting for a slot.
#[derive(Debug)]
pub(super) struct Queued {
    pub(super) name: String,
    pub(super) gen: u64,
    pub(super) resume: bool,
    pub(super) crash_note: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) asked_ms: u64,
}

/// Why a REPL is gone (probation reads it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gone {
    /// It exited, crashed or could not be run.
    Died,
    /// Its start made no progress (crate::repl_start).
    Stalled { reports: bool },
}

impl Gone {
    /// A reason for a version on probation to roll back.
    pub(super) fn reports(self) -> bool {
        match self {
            Gone::Died => true,
            Gone::Stalled { reports } => reports,
        }
    }
}

impl Shell {
    /// The user's input waits on this agent: main, a client's focus, a
    /// message queued for it, a stall restarted (it goes first).
    fn urgent(&self, name: &str, dir: &str) -> bool {
        name == crate::model::MAIN
            || self.hub.focused().contains(name)
            || crate::board::queued_count(&self.hub.st, name) > 0
            || self.restart_first.contains(dir)
    }

    /// A fresh spawn of `name` waits for a slot (generation `gen`).
    pub(super) fn queue_start(&mut self, dir: &str, q: Queued) {
        self.start_queue.insert(dir.to_string(), q);
        self.admit_starts();
    }

    /// The queued spawns that get a free slot, in order.
    pub(super) fn admit_starts(&mut self) {
        if self.start_queue.is_empty() || self.starts.free() == 0 {
            return;
        }
        let cands: Vec<Cand> = self
            .start_queue
            .iter()
            .map(|(dir, q)| Cand { dir: dir.clone(), kind: Kind::Start, urgent: self.urgent(&q.name, dir), asked_ms: q.asked_ms })
            .collect();
        let free = self.starts.free();
        for c in order(cands).into_iter().take(free) {
            let Some(q) = self.start_queue.remove(&c.dir) else { continue };
            // killed or asked again meanwhile: not this start
            if self.gens.get(&c.dir) != Some(&q.gen) {
                continue;
            }
            self.restart_first.remove(&c.dir);
            self.starts.begin(&c.dir, Kind::Start, now_ms());
            self.spawn_fresh(&q.name, q.gen, q.resume, q.crash_note, q.port);
        }
    }

    /// Of the idle REPLs to switch or recycle (name, dir), the ones that
    /// get a slot now (the others: a later tick), each taking it.
    pub(super) fn switch_admitted(&mut self, stale: Vec<(String, String)>) -> Vec<(String, String)> {
        let cands: Vec<Cand> = stale
            .iter()
            .map(|(name, dir)| {
                let recycle = self.recycle.is_due(dir) && !self.reload_repls.contains(dir);
                let kind = if recycle { Kind::Recycle } else { Kind::Switch };
                Cand { dir: dir.clone(), kind, urgent: self.urgent(name, dir), asked_ms: 0 }
            })
            .collect();
        let free = self.starts.free();
        let now = now_ms();
        let mut out = Vec::new();
        for c in order(cands).into_iter().take(free) {
            if let Some(n) = stale.iter().find(|(_, d)| *d == c.dir) {
                self.starts.begin(&c.dir, c.kind, now);
                out.push(n.clone());
            }
        }
        out
    }

    /// A phase of `dir`'s start is done.
    pub(super) fn start_progress(&mut self, dir: &str) {
        self.starts.progress(dir, now_ms());
    }

    /// The starts without progress: killed and gone (a crash, restarted
    /// first); then the queue gets the freed slots.
    pub(super) fn check_starts(&mut self) {
        for s in self.starts.stalled(now_ms()) {
            let reason = format!(
                "its REPL made no progress in {} s while starting ({} at once at most, the rest wait)",
                s.secs,
                crate::repl_start::MAX_IN_FLIGHT
            );
            log_line(&self.opts.paths, &format!("repl {} not started: {}", s.dir, reason));
            if let Some((_, pid)) = self.pids.get(&s.dir) {
                kill_pid(*pid);
            }
            self.restart_first.insert(s.dir.clone());
            if let Some(gen) = self.gens.get(&s.dir).copied() {
                let _ = self.tx.send(Msg::ReplGone { dir: s.dir, gen, reason, cause: Gone::Stalled { reports: s.reports } });
            }
        }
        self.admit_starts();
    }
}

/// Tests only: `SB_SLOW_SPAWN=<file>` holding a number of ms: each REPL
/// start of this hub waits that long before its process is spawned, with
/// no progress (a slow machine; a very long one: a version whose REPLs
/// never start). Read at each start: the test writes it at the switch.
pub(super) fn slow_spawn_for_tests() {
    let Some(f) = bise_home::env::test_setting("SB_SLOW_SPAWN") else {
        return;
    };
    if let Some(ms) = std::fs::read_to_string(f).ok().and_then(|s| s.trim().parse::<u64>().ok()) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
