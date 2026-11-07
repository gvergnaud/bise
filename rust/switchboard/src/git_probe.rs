//! Whether the hub's workspace is a git repo, asked off the loop
//! (boot-speed, architect m_54396). sb-core gets `is_git` with every step
//! (core_link.rs); a `git rev-parse` on the loop took 1 to 212 s per call
//! at load ~100 (exec latency, not git's work), and held a booting hub's
//! last step for 1668 s. Here the probe runs on its own thread:
//!
//! - the first probe starts with the hub's GitEnv; the first ask waits for
//!   it only when it isn't in yet (sb-core gets the answer it gets today,
//!   never a guess);
//! - an answer is fresh for [`FRESH_MS`] from when it came (not from the
//!   ask: a slow probe no longer leaves a stale memo behind it);
//! - a stale answer still answers, and one probe (never two at once)
//!   brings the next;
//! - a probe that could not run git is unknown: not kept, the last answer
//!   stays (none: false, as before), asked again at the next ask.
//!
//! The state and its rules are pure ([`Probe`]); [`GitProbe`] adds the
//! worker thread and the wait.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

/// How long an answer is trusted, from when it came.
pub const FRESH_MS: u64 = 10_000;

/// One probe's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probed {
    Git(bool),
    /// git could not run (spawn failed, killed): nothing learned
    Unknown,
}

/// What an ask does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// this answer, nothing to start
    Answer(bool),
    /// this answer, and start a probe
    AnswerAndProbe(bool),
    /// wait for the probe on its way, then ask again
    Wait,
    /// no answer ever and none on its way: start one and wait for it
    ProbeAndWait,
}

/// The probe's state: its last answer and when it came, whether one runs,
/// whether the answer was marked stale (a worktree op of the hub's).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    pub answer: Option<(bool, u64)>,
    pub running: bool,
    pub stale: bool,
    /// a probe came back unknown with no answer before it: an ask answers
    /// false (as before) instead of waiting again
    pub unknown: bool,
}

impl Probe {
    /// An ask at `now`. Starting a probe sets `running` (single-flight).
    pub fn ask(&mut self, now: u64) -> Ask {
        match self.answer {
            Some((yes, at)) if !self.stale && now.saturating_sub(at) < FRESH_MS => Ask::Answer(yes),
            Some((yes, _)) if self.running => Ask::Answer(yes),
            Some((yes, _)) => {
                self.running = true;
                Ask::AnswerAndProbe(yes)
            }
            None if self.running => Ask::Wait,
            None if self.unknown => {
                self.running = true;
                Ask::AnswerAndProbe(false)
            }
            None => {
                self.running = true;
                Ask::ProbeAndWait
            }
        }
    }

    /// A probe came back at `now`.
    pub fn came(&mut self, p: Probed, now: u64) {
        self.running = false;
        match p {
            Probed::Git(yes) => {
                self.answer = Some((yes, now));
                self.stale = false;
                self.unknown = false;
            }
            Probed::Unknown => self.unknown = self.answer.is_none(),
        }
    }
}

type Run = Arc<dyn Fn(&Path) -> Probed + Send + Sync>;
type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The probe of one folder, its worker and its wait.
#[derive(Clone)]
pub struct GitProbe {
    shared: Arc<(Mutex<Probe>, Condvar)>,
    dir: PathBuf,
    run: Run,
    clock: Clock,
}

impl GitProbe {
    /// The probe of `dir`, its first probe started now.
    pub fn start(dir: &Path, run: Run, clock: Clock) -> GitProbe {
        let p = GitProbe { shared: Arc::new((Mutex::new(Probe::default()), Condvar::new())), dir: dir.to_path_buf(), run, clock };
        let first = p.lock().ask((p.clock)());
        if first == Ask::ProbeAndWait {
            p.spawn();
        }
        p
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Probe> {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn spawn(&self) {
        let me = self.clone();
        std::thread::spawn(move || {
            let got = (me.run)(&me.dir);
            let now = (me.clock)();
            me.lock().came(got, now);
            me.shared.1.notify_all();
        });
    }

    /// Whether `dir` is a git repo, as [`Probe::ask`] says.
    pub fn is_git(&self) -> bool {
        let mut p = self.lock();
        loop {
            match p.ask((self.clock)()) {
                Ask::Answer(yes) => return yes,
                Ask::AnswerAndProbe(yes) => {
                    drop(p);
                    self.spawn();
                    return yes;
                }
                Ask::ProbeAndWait => {
                    drop(p);
                    self.spawn();
                    p = self.lock();
                }
                Ask::Wait => p = self.shared.1.wait(p).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    /// The hub changed worktrees: the answer is asked again, off the loop.
    pub fn mark_stale(&self) {
        self.lock().stale = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    #[test]
    fn fresh_from_when_it_came_then_stale_answers_and_probes_once() {
        let mut p = Probe::default();
        assert_eq!(p.ask(0), Ask::ProbeAndWait);
        assert_eq!(p.ask(1), Ask::Wait, "one probe at a time");
        // a slow probe: 39 s; fresh from its answer, not from the ask
        p.came(Probed::Git(true), 39_000);
        assert_eq!(p.ask(40_000), Ask::Answer(true));
        assert_eq!(p.ask(39_000 + FRESH_MS), Ask::AnswerAndProbe(true));
        assert_eq!(p.ask(39_000 + FRESH_MS + 1), Ask::Answer(true), "single-flight: the stale answer, no second probe");
        p.came(Probed::Git(false), 60_000);
        assert_eq!(p.ask(60_001), Ask::Answer(false));
    }

    #[test]
    fn an_unknown_probe_is_never_kept_as_not_git() {
        let mut p = Probe::default();
        p.ask(0);
        p.came(Probed::Git(true), 10);
        p.ask(10 + FRESH_MS);
        p.came(Probed::Unknown, 20_000);
        assert_eq!(p.answer, Some((true, 10)), "the last answer stays");
        assert_eq!(p.ask(20_001), Ask::AnswerAndProbe(true), "asked again");
        // never an answer: false as before, and asked again, never a wait on nothing
        let mut q = Probe::default();
        q.ask(0);
        q.came(Probed::Unknown, 5);
        assert_eq!(q.ask(6), Ask::AnswerAndProbe(false));
        q.came(Probed::Git(true), 7);
        assert_eq!(q.ask(8), Ask::Answer(true));
    }

    #[test]
    fn a_worktree_op_makes_it_stale_not_unknown() {
        let mut p = Probe::default();
        p.ask(0);
        p.came(Probed::Git(true), 1);
        p.stale = true;
        assert_eq!(p.ask(2), Ask::AnswerAndProbe(true));
        p.came(Probed::Git(true), 3);
        assert_eq!(p.ask(4), Ask::Answer(true));
    }

    #[test]
    fn the_first_ask_waits_for_the_first_probe_and_later_asks_never_wait() {
        let runs = Arc::new(AtomicUsize::new(0));
        let clock = Arc::new(AtomicU64::new(0));
        let (r, c) = (runs.clone(), clock.clone());
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let g = gate.clone();
        let run: Run = Arc::new(move |_| {
            r.fetch_add(1, Ordering::SeqCst);
            // the probe waits until the test lets it go
            let (m, cv) = &*g;
            let mut go = m.lock().unwrap();
            while !*go {
                go = cv.wait(go).unwrap();
            }
            Probed::Git(true)
        });
        let p = GitProbe::start(Path::new("/w"), run, Arc::new(move || c.load(Ordering::SeqCst)));
        let asker = {
            let p = p.clone();
            std::thread::spawn(move || p.is_git())
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!asker.is_finished(), "the first ask waits for the first answer");
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        assert!(asker.join().unwrap());
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        // stale: answers at once with the last answer, one probe started
        clock.store(FRESH_MS + 5, Ordering::SeqCst);
        assert!(p.is_git());
        assert!(p.is_git());
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(runs.load(Ordering::SeqCst), 2, "one probe for both stale asks");
    }
}
