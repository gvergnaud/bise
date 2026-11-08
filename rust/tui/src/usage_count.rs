//! The TUI's usage counts (page usage-metrics-plan, step 4; architect
//! m_53623, m_53687): how long this TUI was attached to its hub, how long
//! its terminal had the focus (focus reporting: FocusGained/FocusLost,
//! `App::focus_lost`), and its panics (`crash_tui`: the logs it wrote to
//! `crashes/`, counted by `crash::take_logged`; a log never leaves).
//! Nothing else is read: no text, no name, no path.
//!
//! - [`Clock`], the pure part: the day's seconds and panics, the time
//!   passed in as arguments; a span that crosses a UTC midnight is split
//!   between the two days, and a new day writes the old one first;
//! - [`Live`], the shell: the clock with this process's clocks, made in
//!   `run_switchboard` only when `bise_usage::spool::sink` gives a sink
//!   (sharing allowed now; none: nothing is counted), ticked once a loop
//!   ([`tick`]) and written a last time when the App goes, a panic's
//!   unwinding included, so the panic that ends a TUI is counted.
//!
//! Written every [`WRITE_EVERY`] and at the end: a kill loses at most
//! 5 minutes. One source a process (`tui-<hub hash>-<pid>.json`),
//! starting at zero.

use std::time::{Duration, Instant};

use bise_usage::spool::{Sink, Source};
use bise_usage::{CountKey, Day, MinutesKey, Tally};

/// How often the day's tally is written.
pub(crate) const WRITE_EVERY: Duration = Duration::from_secs(5 * 60);

/// The longest span one tick counts. The loop ticks every 80 ms at most;
/// a longer span is a screen that held the loop (the onboarding), and
/// a span past this is not believed whole.
pub(crate) const MAX_STEP: Duration = WRITE_EVERY;

/// One day's counts of this TUI.
#[derive(Default)]
struct DayCounts {
    attached_ms: u64,
    focused_ms: u64,
    crashes: u64,
}

/// What this TUI counted in its current UTC day, and where it goes.
pub(crate) struct Clock {
    sink: Sink,
    day: Day,
    counts: DayCounts,
    /// the focus since the last tick
    focused: bool,
    last: Instant,
    written: Instant,
}

impl Clock {
    /// Counting starts at zero at `now` (`unix_s`: the wall clock's
    /// seconds), the terminal focused (it was just opened).
    pub(crate) fn new(sink: Sink, now: Instant, unix_s: u64) -> Clock {
        Clock { sink, day: Day::from_unix_secs(unix_s), counts: DayCounts::default(), focused: true, last: now, written: now }
    }

    /// The loop's tick: the span since the last one counted as attached
    /// (and focused if it was), `crashes` new panic logs, the focus from
    /// now on; a new UTC day writes the old one and starts at zero; else
    /// the tally is written every [`WRITE_EVERY`].
    pub(crate) fn tick(&mut self, now: Instant, unix_s: u64, focused: bool, crashes: u32) {
        let span = now.saturating_duration_since(self.last).min(MAX_STEP).as_millis() as u64;
        self.last = now;
        let today = Day::from_unix_secs(unix_s);
        if today == self.day {
            self.add(span);
        } else {
            // the part of the span before today's midnight is the old day's
            let into_today = unix_s.saturating_sub(today.start_secs()).saturating_mul(1000);
            let before = span.saturating_sub(into_today);
            self.add(before);
            self.write(now);
            self.day = today;
            self.counts = DayCounts::default();
            self.add(span - before);
        }
        self.counts.crashes += u64::from(crashes);
        self.focused = focused;
        if now.saturating_duration_since(self.written) >= WRITE_EVERY {
            self.write(now);
        }
    }

    fn add(&mut self, ms: u64) {
        self.counts.attached_ms += ms;
        if self.focused {
            self.counts.focused_ms += ms;
        }
    }

    /// The day's tally: whole seconds, the panics.
    pub(crate) fn tally(&self) -> Tally {
        let c = &self.counts;
        let mut t = Tally::default();
        t.time(MinutesKey::TuiAttached, c.attached_ms / 1000);
        t.time(MinutesKey::TuiFocused, c.focused_ms / 1000);
        if c.crashes > 0 {
            t.add(CountKey::CrashTui, c.crashes);
        }
        t
    }

    /// Write the day's tally whole to the sink.
    pub(crate) fn write(&mut self, now: Instant) {
        self.written = now;
        let t = self.tally();
        if let Err(e) = (self.sink)(self.day, &t) {
            crate::crash::note(format!("the usage counts were not written: {e}"));
        }
    }

    /// The last tick, then the last write (the TUI ends).
    pub(crate) fn close(&mut self, now: Instant, unix_s: u64, crashes: u32) {
        self.tick(now, unix_s, self.focused, crashes);
        self.write(now);
    }
}

/// The clock of this process, written a last time when it goes.
pub(crate) struct Live(Clock);

impl Live {
    /// This TUI's clock, attached to the hub `hub_id`: only when sharing
    /// is allowed now (`spool::sink`), else None and nothing is counted.
    pub(crate) fn start(hub_id: &str, app_root: &std::path::Path) -> Option<Live> {
        let source = Source::tui(hub_id, std::process::id())?;
        let sink = bise_usage::spool::sink(bise_home::Home::from_env(), source, app_root, false)?;
        Some(Live(Clock::new(sink, Instant::now(), unix_now())))
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.0.close(Instant::now(), unix_now(), crate::crash::take_logged());
    }
}

/// The loop's tick: the time since the last one, the focus, new panics.
pub(crate) fn tick(app: &mut crate::app::App) {
    let focused = !app.focus_lost;
    if let Some(Live(clock)) = app.usage_count.as_mut() {
        clock.tick(Instant::now(), unix_now(), focused, crate::crash::take_logged());
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "usage_count_tests.rs"]
mod tests;
