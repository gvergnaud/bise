//! When something happened, in words (BISE-271): the end of a turn on
//! hover (`12:41 · 1h ago`) and the pause marks of the feed
//! (`· yesterday 18:02 ·`). Pure functions of the time, now and the
//! local UTC offsets (tests inject them); `now_ms` and `offset_at` are
//! the real clock.

const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

// pure, shared with the hub's thread fold (batch 3b): bise_proto::thread::when
use bise_proto::thread::when::{civil, day_and_time};

/// When a turn ended, as its hover says (the designer's words): under
/// a day the time and how long ago (`12:41 · now`, `12:41 · 5m ago`,
/// `12:41 · 1h ago`), else the day and the time (`yesterday 18:02`,
/// `sep 28 18:02`). `off` is the UTC offset at `ms`, `now_off` at `now`.
pub(crate) fn ended(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    let ago = now.saturating_sub(ms);
    if ago >= DAY {
        return day_and_time(ms, off, now, now_off);
    }
    let (_, _, _, h, mi) = civil(ms, off);
    let rel = if ago < MINUTE {
        "now".to_string()
    } else if ago < HOUR {
        format!("{}m ago", ago / MINUTE)
    } else {
        format!("{}h ago", ago / HOUR)
    };
    format!("{:02}:{:02} · {}", h, mi, rel)
}

/// The text of a pause mark `· 14:31 ·` for a line written at `ms`:
/// the time; the day too when it is not today (`yesterday 18:02`,
/// `sep 28 18:02`).
pub(crate) fn mark(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    day_and_time(ms, off, now, now_off)
}

/// How many local calendar days separate `ms` from `now` (0: today).
pub(crate) fn days_ago(ms: u64, off: i32, now: u64, now_off: i32) -> i64 {
    let day = |t: u64, o: i32| ((t / 1000) as i64 + o as i64).div_euclid(86_400);
    day(now, now_off) - day(ms, off)
}

// ---- the real clock ----

/// Now, ms since the epoch.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The local UTC offset at `ms`, in seconds (bise_home's one reader,
/// shared with the hub and `sb every`).
pub(crate) fn offset_at(ms: u64) -> i32 {
    bise_home::clock::offset_at(ms)
}

/// `ended` on the real clock.
pub(crate) fn ended_now(ms: u64) -> String {
    let now = now_ms();
    ended(ms, offset_at(ms), now, offset_at(now))
}

/// `mark` on the real clock.
pub(crate) fn mark_now(ms: u64) -> String {
    let now = now_ms();
    mark(ms, offset_at(ms), now, offset_at(now))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2025-09-30 12:41:00 UTC
    const T: u64 = 1_759_236_060_000;
    const PARIS: i32 = 2 * 3600;

    #[test]
    fn a_turn_under_a_day_ago_says_its_time_and_how_long_ago() {
        assert_eq!(ended(T, 0, T, 0), "12:41 · now");
        assert_eq!(ended(T, 0, T + 59_999, 0), "12:41 · now");
        assert_eq!(ended(T, 0, T + 5 * MINUTE + 10_000, 0), "12:41 · 5m ago");
        assert_eq!(ended(T, 0, T + 61 * MINUTE, 0), "12:41 · 1h ago");
        assert_eq!(ended(T, PARIS, T + 23 * HOUR + 59 * MINUTE, PARIS), "14:41 · 23h ago");
        // a clock that went back: now
        assert_eq!(ended(T + HOUR, 0, T, 0), "13:41 · now");
    }

    #[test]
    fn a_turn_a_day_ago_or_more_says_its_day() {
        // 18:02 the day before, now 19:00: over a day, yesterday
        let y = T - 12 * HOUR - 41 * MINUTE + 18 * HOUR + 2 * MINUTE - DAY;
        assert_eq!(ended(y, 0, y + DAY + HOUR, 0), "yesterday 18:02");
        // two days back: the date, lowercase month
        assert_eq!(ended(T - 2 * DAY, 0, T, 0), "sep 28 12:41");
        // another year: the year too
        assert_eq!(ended(T - 300 * DAY, 0, T, 0), "dec 4 2024 12:41");
        // the local zone decides the day
        assert_eq!(ended(T - 2 * DAY, PARIS, T, PARIS), "sep 28 14:41");
    }

    #[test]
    fn a_pause_mark_says_the_day_when_it_is_not_today() {
        assert_eq!(mark(T, 0, T + HOUR, 0), "12:41");
        assert_eq!(mark(T - 20 * HOUR, 0, T, 0), "yesterday 16:41");
        assert_eq!(mark(T - 3 * DAY, 0, T, 0), "sep 27 12:41");
        // 23:30 in Paris is already the next day there
        let late = T + 8 * HOUR + 49 * MINUTE; // 21:30 UTC, 23:30 Paris
        assert_eq!(mark(late, PARIS, late + 2 * HOUR, PARIS), "yesterday 23:30");
    }
}
