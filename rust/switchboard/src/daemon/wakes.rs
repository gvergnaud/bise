//! The look of `sb wake`'s watches (event-wake): on the tick, at most once
//! a second, each live watch of an active agent is looked at through the
//! real world (files, kill(0), `ps`, `launchctl`); a watch whose event
//! happened goes to sb-core as `Input::WakeHit` with its words (wake.rs),
//! and sb-core ends it and wakes its agent once.
//!
//! Battery (idle-cpu): no live watch, no work at all. `stat` and kill(0)
//! once a second; a pid's start time (`ps`, against a reused pid) and
//! `launchctl list` (a process each) at most every [`COSTLY_MS`]; a
//! launchd job's pid, once seen, is watched with kill(0) and launchctl is
//! asked again only when it is gone (for its LastExitStatus).
//! Owns: the cadence and the jobs' pids seen (view state, lost on restart
//! by design: the next costly look finds them again). Not here: the
//! decision (sb-core), the words and the look's logic (wake.rs).

use super::*;
use crate::wake::{self, Costly, JobState, Probe, Seen};
use std::io::{Read, Seek, SeekFrom};

/// A look at most this often.
const LOOK_MS: u64 = 1000;
/// `ps` and `launchctl` at most this often.
const COSTLY_MS: u64 = 10_000;
/// What a look reads of a file: its head, its tail.
const READ_MAX: u64 = 8192;

#[derive(Default)]
pub(super) struct Look {
    last_ms: u64,
    costly_ms: u64,
    /// a launchd job's pid, by watch id, once a look saw it
    job_pids: BTreeMap<u64, u32>,
}

/// The real world.
struct Real;

impl Probe for Real {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn head(&self, path: &str) -> Option<String> {
        let mut buf = Vec::new();
        std::fs::File::open(path).ok()?.take(READ_MAX).read_to_end(&mut buf).ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
    }
    fn tail(&self, path: &str) -> Option<String> {
        let mut f = std::fs::File::open(path).ok()?;
        let len = f.metadata().ok()?.len();
        f.seek(SeekFrom::Start(len.saturating_sub(READ_MAX))).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
    }
    fn alive(&self, pid: u32) -> bool {
        crate::procs::alive(pid)
    }
    fn start(&self, pid: u32) -> Option<String> {
        crate::procs::start_time(pid)
    }
    fn job(&self, label: &str) -> Option<JobState> {
        let out = std::process::Command::new("launchctl")
            .args(["list", label])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        out.status.success().then(|| wake::parse_job(&String::from_utf8_lossy(&out.stdout)))
    }
}

impl Shell {
    /// On the tick: look at the live watches; each one whose event
    /// happened goes to sb-core, once (it ends there).
    pub(super) fn wakes_look(&mut self) {
        if self.hub.wakes().live.is_empty() {
            self.wake_look.job_pids.clear();
            return;
        }
        let now = now_ms();
        if now < self.wake_look.last_ms + LOOK_MS {
            return;
        }
        self.wake_look.last_ms = now;
        let costly = now >= self.wake_look.costly_ms + COSTLY_MS;
        if costly {
            self.wake_look.costly_ms = now;
        }
        let active = |a: &str| self.hub.st.agents.get(a).is_some_and(|a| a.lifecycle == Lifecycle::Active);
        let watches: Vec<wake::Watch> = self.hub.wakes().live.values().filter(|w| active(&w.agent)).cloned().collect();
        let live: BTreeSet<u64> = self.hub.wakes().live.keys().copied().collect();
        self.wake_look.job_pids.retain(|id, _| live.contains(id));
        for w in watches {
            let (seen, pid) = wake::look(&w.spec, &Real, Costly { start: costly, job: costly }, self.wake_look.job_pids.get(&w.id).copied());
            match pid {
                Some(p) => self.wake_look.job_pids.insert(w.id, p),
                None => self.wake_look.job_pids.remove(&w.id),
            };
            if let Seen::Ended { rc } = seen {
                let tail = w.spec.tail.as_deref().and_then(|t| Real.tail(t));
                let after_ms = now.saturating_sub(w.set_at);
                let text = wake::hit_text(&w.spec, rc.as_deref(), tail.as_deref(), after_ms);
                let hit = wake::Hit::of(rc.as_deref(), tail.as_deref(), after_ms);
                self.step(Input::WakeHit { id: w.id, text, rc: hit.rc, tail: hit.tail, after_ms });
            }
        }
    }
}
