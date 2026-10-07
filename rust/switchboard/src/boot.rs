//! The hub's boot watch (pure): a boot never hangs silently.
//!
//! A boot is a few steps (`boot: <step>` lines in hub.log) and one long
//! one, the journal replay, that reports its progress (`journal replay
//! n/N events`, at most every [`REPLAY_LINE_EVERY`]). [`BootWatch`]
//! decides on progress, never on the time spent in a step: a slow
//! replay that moves (37k events under a load of 40) is never stuck.
//! - no progress for [`STILL_EVERY`]: a heartbeat line (`boot: still in
//!   <step> ...`), again every [`STILL_EVERY`];
//! - no progress for [`STUCK_AFTER`]: stuck. The hub says where in
//!   hub.log and hub.err and exits, unless it already did so within
//!   [`STUCK_AGAIN_MS`] (then it stays up, said once: a stuck boot does
//!   not become a crash loop of the hub its clients restart).
//!
//! A heartbeat is not progress: the switcher (switch.rs) waits for a
//! starting hub while lines other than heartbeats come.
//! The thread and the effects are `daemon/boot.rs`.

use std::time::Duration;

/// No progress for this long: one heartbeat line, then one every period.
pub const STILL_EVERY: Duration = Duration::from_secs(30);
/// No progress for this long: the boot is stuck.
pub const STUCK_AFTER: Duration = Duration::from_secs(600);
/// A stuck exit this recent (ms): the next stuck boot stays up.
pub const STUCK_AGAIN_MS: u64 = 3_600_000;
/// The journal replay's progress line, at most this often.
pub const REPLAY_LINE_EVERY: Duration = Duration::from_secs(5);

const HEARTBEAT: &str = "boot: still in ";

/// What the watch says at a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Quiet,
    /// No progress for `secs` in `step`: a heartbeat line.
    Still { step: String, secs: u64 },
    /// No progress for `secs` (>= STUCK_AFTER) in `step`.
    Stuck { step: String, secs: u64 },
}

/// What a stuck hub does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnStuck {
    /// Say where, then exit (its client starts a new hub).
    Exit,
    /// It already exited stuck within the hour: stay up, said once.
    StayUp,
}

/// The current step of a boot and when it last moved (ms).
#[derive(Debug, Clone)]
pub struct BootWatch {
    step: String,
    moved_ms: u64,
    said_ms: u64,
    stuck_said: bool,
}

impl BootWatch {
    pub fn new(step: &str, now_ms: u64) -> BootWatch {
        BootWatch { step: step.to_string(), moved_ms: now_ms, said_ms: now_ms, stuck_said: false }
    }

    /// A new boot step: progress.
    pub fn step(&mut self, what: &str, now_ms: u64) {
        self.step = what.to_string();
        self.progress(now_ms);
    }

    /// The step moved (the replay applied a batch).
    pub fn progress(&mut self, now_ms: u64) {
        self.moved_ms = now_ms;
        self.said_ms = now_ms;
        self.stuck_said = false;
    }

    /// The verdict at `now_ms`: each heartbeat once per STILL_EVERY,
    /// stuck once per stall.
    pub fn tick(&mut self, now_ms: u64) -> Verdict {
        let still = now_ms.saturating_sub(self.moved_ms);
        if still >= STUCK_AFTER.as_millis() as u64 && !self.stuck_said {
            self.stuck_said = true;
            self.said_ms = now_ms;
            return Verdict::Stuck { step: self.step.clone(), secs: still / 1000 };
        }
        if now_ms.saturating_sub(self.said_ms) >= STILL_EVERY.as_millis() as u64 {
            self.said_ms = now_ms;
            return Verdict::Still { step: self.step.clone(), secs: still / 1000 };
        }
        Verdict::Quiet
    }
}

/// A stuck boot exits, unless the last stuck exit (`last_ms`, from its
/// marker) is less than STUCK_AGAIN_MS old.
pub fn on_stuck(last_ms: Option<u64>, now_ms: u64) -> OnStuck {
    match last_ms {
        Some(t) if now_ms.saturating_sub(t) < STUCK_AGAIN_MS => OnStuck::StayUp,
        _ => OnStuck::Exit,
    }
}

/// The heartbeat line (without `boot: ` time stamps).
pub fn still_line(step: &str, secs: u64) -> String {
    format!("{}{}: no progress for {} s", HEARTBEAT, step, secs)
}

/// The line of a stuck boot, for hub.log and hub.err.
pub fn stuck_line(step: &str, secs: u64, on: OnStuck) -> String {
    match on {
        OnStuck::Exit => format!("boot: stuck in {}: no progress for {} s, the hub exits", step, secs),
        OnStuck::StayUp => format!(
            "boot: stuck in {}: no progress for {} s; it already exited stuck within the hour: it stays up",
            step, secs
        ),
    }
}

/// A hub.log line is a heartbeat (not progress).
pub fn is_heartbeat(line: &str) -> bool {
    line.contains(HEARTBEAT)
}

/// The text has a line that is progress: any non-empty line but a
/// heartbeat (the switcher's view of a starting hub).
pub fn moved(text: &str) -> bool {
    text.lines().any(|l| !l.trim().is_empty() && !is_heartbeat(l))
}

/// The replay's progress step.
pub fn replay_step(done: usize, total: usize) -> String {
    format!("journal replay {}/{} events", done, total)
}

/// A replay progress line is due: the last one is REPLAY_LINE_EVERY old.
pub fn replay_line_due(last_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_ms) >= REPLAY_LINE_EVERY.as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1000;

    #[test]
    fn a_boot_without_progress_says_where_every_30_s_then_is_stuck_once() {
        let mut w = BootWatch::new("journal read (37585 events)", 0);
        assert_eq!(w.tick(29 * S), Verdict::Quiet);
        assert_eq!(w.tick(30 * S), Verdict::Still { step: "journal read (37585 events)".into(), secs: 30 });
        assert_eq!(w.tick(31 * S), Verdict::Quiet, "one heartbeat per period");
        assert_eq!(w.tick(60 * S), Verdict::Still { step: "journal read (37585 events)".into(), secs: 60 });
        assert_eq!(w.tick(600 * S), Verdict::Stuck { step: "journal read (37585 events)".into(), secs: 600 });
        assert_eq!(w.tick(601 * S), Verdict::Quiet, "stuck is said once per stall");
        assert!(matches!(w.tick(630 * S), Verdict::Still { .. }));
    }

    #[test]
    fn a_slow_replay_that_moves_is_never_stuck() {
        // 37k events at one batch per 20 s: hours, never stuck, never a heartbeat
        let mut w = BootWatch::new("journal read", 0);
        for i in 1..=400u64 {
            assert_eq!(w.tick(i * 20 * S - 1), Verdict::Quiet, "batch {}", i);
            w.progress(i * 20 * S);
        }
        // a new step is progress too
        w.step("journal replayed", 9_000 * S);
        assert_eq!(w.tick(9_029 * S), Verdict::Quiet);
    }

    #[test]
    fn a_second_stuck_boot_within_the_hour_stays_up() {
        assert_eq!(on_stuck(None, 10 * S), OnStuck::Exit);
        assert_eq!(on_stuck(Some(0), 3_599 * S), OnStuck::StayUp);
        assert_eq!(on_stuck(Some(0), 3_600 * S), OnStuck::Exit);
        assert!(stuck_line("journal read", 600, OnStuck::Exit).ends_with("the hub exits"));
    }

    #[test]
    fn a_heartbeat_is_not_progress_for_the_switcher() {
        let hb = format!("1791355133000 {}", still_line("journal read (37585 events)", 30));
        assert!(is_heartbeat(&hb));
        assert!(!moved(&format!("{}\n\n", hb)));
        assert!(moved(&format!("{}\n1791355134000 boot: {}\n", hb, replay_step(500, 37585))));
        assert!(!moved(""));
    }

    #[test]
    fn replay_lines_come_at_most_every_5_s() {
        assert!(!replay_line_due(0, 4_999));
        assert!(replay_line_due(0, 5_000));
    }
}
