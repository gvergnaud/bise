//! The boot watch's thread and effects (the decisions: `crate::boot`).
//! `Boot::start` at the hub's start; each boot step and each replayed
//! batch goes through it; `done` once the hub serves (the thread ends:
//! no work after the boot). Its lines go to hub.log; a stuck boot's line
//! goes to hub.err too, and the hub exits with sb-core (its marker,
//! `<state>/boot-stuck`, keeps the next stuck boot within the hour up).

use super::log_line;
use crate::boot::{self, BootWatch, OnStuck, Verdict};
use crate::paths::Paths;
use crate::util::now_ms;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(super) struct Boot {
    paths: Paths,
    watch: Arc<Mutex<Option<BootWatch>>>,
    core_pid: Arc<AtomicU32>,
    replay_line_ms: Mutex<u64>,
}

fn marker(paths: &Paths) -> std::path::PathBuf {
    paths.state.join("boot-stuck")
}

impl Boot {
    /// The watch of a boot at its first step, and its thread.
    pub(super) fn start(paths: &Paths, first: &str) -> Boot {
        let watch = Arc::new(Mutex::new(Some(BootWatch::new(first, now_ms()))));
        let core_pid = Arc::new(AtomicU32::new(0));
        let (w, pid, p) = (watch.clone(), core_pid.clone(), paths.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let verdict = match w.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                Some(bw) => bw.tick(now_ms()),
                None => return,
            };
            match verdict {
                Verdict::Quiet => {}
                Verdict::Still { step, secs } => log_line(&p, &boot::still_line(&step, secs)),
                Verdict::Stuck { step, secs } => stuck(&p, &step, secs, pid.load(Ordering::Acquire)),
            }
        });
        Boot { paths: paths.clone(), watch, core_pid, replay_line_ms: Mutex::new(now_ms()) }
    }

    /// A boot step: logged (hub.log, SB_TIMING) and progress.
    pub(super) fn step(&self, what: &str) {
        log_line(&self.paths, &format!("boot: {}", what));
        crate::util::timing(what);
        if let Some(bw) = self.watch.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            bw.step(what, now_ms());
        }
    }

    /// The journal replay applied `done` of `total` events: progress,
    /// and a line at most every REPLAY_LINE_EVERY.
    pub(super) fn replayed(&self, done: usize, total: usize) {
        let now = now_ms();
        {
            let mut last = self.replay_line_ms.lock().unwrap_or_else(|e| e.into_inner());
            if boot::replay_line_due(*last, now) {
                *last = now;
                log_line(&self.paths, &format!("boot: {}", boot::replay_step(done, total)));
            }
        }
        if let Some(bw) = self.watch.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            bw.step(&boot::replay_step(done, total), now);
        }
    }

    /// sb-core's pid: a stuck hub stops it with itself.
    pub(super) fn core(&self, pid: u32) {
        self.core_pid.store(pid, Ordering::Release);
    }

    /// The hub serves: the watch ends.
    pub(super) fn done(&self) {
        *self.watch.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Say where the boot is stuck; exit with sb-core, unless a stuck boot
/// already exited within the hour.
fn stuck(paths: &Paths, step: &str, secs: u64, core_pid: u32) {
    let last = std::fs::read_to_string(marker(paths)).ok().and_then(|s| s.trim().parse().ok());
    let on = boot::on_stuck(last, now_ms());
    let line = boot::stuck_line(step, secs, on);
    log_line(paths, &line);
    eprintln!("{}", line);
    if on == OnStuck::Exit {
        let _ = std::fs::write(marker(paths), now_ms().to_string());
        if core_pid != 0 {
            crate::procs::kill_now(core_pid);
        }
        std::process::exit(1);
    }
}
