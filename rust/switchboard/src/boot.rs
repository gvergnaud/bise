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
//! - time the hub's process did not run is not time without progress: a
//!   tick [`FROZEN_GAP`] or more after the last one (the Mac froze the
//!   process) moves the count by the gap.
//!
//! A heartbeat is not progress: the switcher (switch.rs) waits for a
//! starting hub while lines other than heartbeats come.
//! The thread and the effects are `daemon/boot.rs`.

use crate::watch::Watch;
use std::time::Duration;

/// No progress for this long: one heartbeat line, then one every period.
pub const STILL_EVERY: Duration = Duration::from_secs(30);
/// No progress for this long: the boot is stuck.
pub const STUCK_AFTER: Duration = Duration::from_secs(600);
/// A stuck exit this recent (ms): the next stuck boot stays up.
pub const STUCK_AGAIN_MS: u64 = 3_600_000;
/// The journal replay's progress line, at most this often.
pub const REPLAY_LINE_EVERY: Duration = Duration::from_secs(5);
/// The watch ticks every second: a tick this late or later means the Mac
/// froze the hub's process (swap full, replay-stall: 981 s between two
/// ticks), and the gap is not counted as no progress.
pub const FROZEN_GAP: Duration = Duration::from_secs(5);

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
    /// stuck: STUCK_AFTER without progress (crate::watch, the rule REPL
    /// starts follow too)
    watch: Watch,
    said_ms: u64,
    stuck_said: bool,
    /// the last tick (ms): a late one is a frozen process (FROZEN_GAP)
    ticked_ms: u64,
}

impl BootWatch {
    pub fn new(step: &str, now_ms: u64) -> BootWatch {
        let watch = Watch::new(now_ms, STUCK_AFTER.as_millis() as u64);
        BootWatch { step: step.to_string(), watch, said_ms: now_ms, stuck_said: false, ticked_ms: now_ms }
    }

    /// A new boot step: progress.
    pub fn step(&mut self, what: &str, now_ms: u64) {
        self.step = what.to_string();
        self.progress(now_ms);
    }

    /// The step moved (the replay applied a batch).
    pub fn progress(&mut self, now_ms: u64) {
        self.watch.moved(now_ms);
        self.said_ms = now_ms;
        self.stuck_said = false;
    }

    /// The verdict at `now_ms`: each heartbeat once per STILL_EVERY,
    /// stuck once per stall. A tick FROZEN_GAP or more after the last:
    /// the process was frozen, the gap does not count.
    pub fn tick(&mut self, now_ms: u64) -> Verdict {
        let gap = now_ms.saturating_sub(self.ticked_ms);
        self.ticked_ms = now_ms;
        if gap >= FROZEN_GAP.as_millis() as u64 {
            self.watch.froze(gap, now_ms);
            self.said_ms = self.said_ms.saturating_add(gap).min(now_ms);
        }
        let still = self.watch.still_ms(now_ms);
        if self.watch.stalled(now_ms) && !self.stuck_said {
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
        let step = "journal read (37585 events)";
        let mut w = BootWatch::new(step, 0);
        // a heartbeat every 30 s, stuck once at 600 s, then heartbeats again
        let want: Vec<(u64, Verdict)> = (1..=21u64)
            .map(|k| {
                let secs = k * 30;
                let v = if secs == 600 { Verdict::Stuck { step: step.into(), secs } } else { Verdict::Still { step: step.into(), secs } };
                (secs, v)
            })
            .collect();
        assert_eq!(ticks(&mut w, 1, 630), want);
    }

    #[test]
    fn a_slow_replay_that_moves_is_never_stuck() {
        // 37k events at one batch per 20 s: hours, never stuck, never a heartbeat
        let mut w = BootWatch::new("journal read", 0);
        for i in 1..=400u64 {
            assert_eq!(ticks(&mut w, (i - 1) * 20 + 1, i * 20 - 1), vec![], "batch {}", i);
            w.progress(i * 20 * S);
        }
        // a new step is progress too
        w.step("journal replayed", 8_000 * S);
        assert_eq!(ticks(&mut w, 8_000, 8_029), vec![]);
    }

    /// Its 1 s ticks from `from` to `to` (s): the verdicts that are not Quiet.
    fn ticks(w: &mut BootWatch, from: u64, to: u64) -> Vec<(u64, Verdict)> {
        (from..=to).map(|s| (s, w.tick(s * S))).filter(|(_, v)| *v != Verdict::Quiet).collect()
    }

    #[test]
    fn a_frozen_mac_is_not_a_stuck_boot() {
        // his harness hub (replay-stall): the replay moved until 298 s, then
        // the Mac froze the whole process (swap full) and the watch's next
        // tick came at 1279 s; it said stuck there and the switch failed
        let mut w = BootWatch::new("journal read", 0);
        for s in 1..=298 {
            w.tick(s * S);
            w.progress(s * S);
        }
        assert_eq!(w.tick(1_279 * S), Verdict::Quiet, "the 981 s the process did not run are not 981 s without progress");
        // the replay still does not move once the Mac runs again: stuck
        // after STUCK_AFTER of ticks, counted from the thaw
        let said = ticks(&mut w, 1_280, 1_279 + 600);
        assert_eq!(said.last(), Some(&(1_879, Verdict::Stuck { step: "journal read".into(), secs: 600 })));
        assert!(said.iter().all(|(s, v)| *s == 1_879 || matches!(v, Verdict::Still { .. })), "{:?}", said);
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
