//! The hub's usage counts on the shell's side (page usage-metrics-plan,
//! step 2; architect m_53623 b, m_55989). The pure part is
//! `crate::usage_count` ([`Counter`]); this module feeds it and writes
//! its tallies to the spool (`bise_usage::spool`, one file per hub process
//! and UTC day, under `Home::usage_day_dir`). It observes, decides
//! nothing, wakes nobody.
//!
//! Fed by: sb-core's `journal` effects as the shell runs them
//! ([`Shell::usage_journal`]; a boot replay runs none, so a restart counts
//! nothing twice); the live REPL lines ([`Shell::usage_repl_line`]; never
//! the week's transcripts); a REPL gone; a land that moved a ref and a
//! page published ([`Shell::usage_count`]).
//!
//! Written: every [`WRITE_MS`] from the tick, and at a clean stop. The
//! permit (`bise_usage::sharing_allowed`) is read again at each write:
//! without one nothing is counted, and what was counted is dropped.

use super::*;
use crate::usage_count::Counter;
use bise_usage::packet::Channel;
use bise_usage::spool::{self, Source};
use bise_usage::{sharing_allowed, CountKey, Facts, Permit};

/// How often the day's tally is written: 5 min (a crash loses at most that).
pub(super) const WRITE_MS: u64 = 5 * 60_000;

/// The hub's counter, its spool source and whether sharing is on.
pub(super) struct UsageSpool {
    counter: Counter,
    /// this hub process in the spool (none: a workspace without a hub id)
    source: Option<Source>,
    /// sharing allowed at the last read (the start, then each write)
    on: bool,
    written_ms: u64,
}

impl UsageSpool {
    /// At the hub's start: nothing counted yet, sharing read once.
    pub(super) fn new(home: &bise_home::Home, workspace: &Path, now: u64) -> UsageSpool {
        let source = Source::hub(&bise_home::hub_id(workspace), std::process::id());
        UsageSpool { counter: Counter::new(now), on: permit(home).is_some() && source.is_some(), source, written_ms: now }
    }
}

/// The permit of this process, read now. The hub runs as part of a
/// release or of a dev build; until a build says its channel (plan step
/// 9), it reads as `Dev`, which never shares.
fn permit(home: &bise_home::Home) -> Option<Permit> {
    let env = |k: &str| std::env::var(k).ok();
    sharing_allowed(&Facts::read(home, &env, Channel::Dev, false)).ok()
}

impl Shell {
    /// sb-core journaled `ev` (live: Shell::run's journal effect).
    pub(super) fn usage_journal(&mut self, ev: &Value) {
        if self.usage_spool.on {
            self.usage_spool.counter.journal(ev, now_ms());
        }
    }

    /// A line of agent `dir`'s live REPL.
    pub(super) fn usage_repl_line(&mut self, dir: &str, line: &str) {
        if !self.usage_spool.on {
            return;
        }
        let obs = match bise_proto::thread::lines::read(line) {
            bise_proto::thread::lines::Rec::Obs(o) => Some(o),
            _ => None,
        };
        self.usage_spool.counter.repl_line(dir, obs.as_ref(), now_ms());
    }

    /// Agent `dir`'s REPL is gone: a turn it was in ends.
    pub(super) fn usage_gone(&mut self, dir: &str) {
        self.usage_spool.counter.gone(dir);
    }

    /// Something the shell itself saw happen (a land, a page published).
    pub(super) fn usage_count(&mut self, key: CountKey) {
        if self.usage_spool.on {
            self.usage_spool.counter.count(key, now_ms());
        }
    }

    /// At each tick: the day's tally written every [`WRITE_MS`].
    pub(super) fn usage_check(&mut self) {
        let now = now_ms();
        if now >= self.usage_spool.written_ms + WRITE_MS {
            self.usage_write(now);
        }
    }

    /// Write what is due (the closed days, then today's), sharing read
    /// again first: off, the counts are dropped and nothing is written.
    pub(super) fn usage_write(&mut self, now: u64) {
        let u = &mut self.usage_spool;
        u.written_ms = now;
        let (Some(permit), Some(source)) = (permit(&self.opts.home), u.source.as_ref()) else {
            u.on = false;
            u.counter.clear(now);
            return;
        };
        u.on = true;
        for (day, tally) in u.counter.due(now) {
            let dir = self.opts.home.usage_day_dir(&day.to_string());
            if let Err(e) = spool::write_source(&permit, &dir, source, &tally) {
                log_line(&self.opts.paths, &format!("usage: the day's counts not written: {e}"));
            }
        }
    }
}
