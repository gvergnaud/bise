//! When something happened, in words (BISE-271): the end of a turn on
//! hover (`12:41 · 1h ago`) and the pause marks of the feed
//! (`· yesterday 18:02 ·`). Pure functions of the time, now and the
//! local UTC offsets (tests inject them); `now_ms` and `offset_at` are
//! the real clock.

use std::cell::RefCell;
use std::collections::HashMap;

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// A moment in local time: (year, month 1..=12, day, hour, minute).
type Civil = (i64, u32, u32, u32, u32);

/// `ms` (since the epoch) at UTC offset `off` (seconds east).
fn civil(ms: u64, off: i32) -> Civil {
    let secs = (ms / 1000) as i64 + off as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // days since 1970-01-01 to a civil date (H. Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32)
}

/// The local date of `ms` is the day before the local date of `now`.
fn is_yesterday(ms: u64, off: i32, now: u64, now_off: i32) -> bool {
    let (a, b) = (civil(ms, off), civil(now.saturating_sub(DAY), now_off));
    (a.0, a.1, a.2) == (b.0, b.1, b.2)
}

/// `sep 28 18:02`, `dec 30 2024 18:02` when not this year, `yesterday
/// 18:02`; `18:02` on the same day.
fn day_and_time(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    let (y, m, d, h, mi) = civil(ms, off);
    let (ny, nm, nd, ..) = civil(now, now_off);
    let hm = format!("{:02}:{:02}", h, mi);
    if (y, m, d) == (ny, nm, nd) {
        hm
    } else if is_yesterday(ms, off, now, now_off) {
        format!("yesterday {}", hm)
    } else if y == ny {
        format!("{} {} {}", MONTHS[(m - 1) as usize], d, hm)
    } else {
        format!("{} {} {} {}", MONTHS[(m - 1) as usize], d, y, hm)
    }
}

/// A time to come (a scheduled task's next run): `14:22` today,
/// `tomorrow 07:30`, else the day and the time (`oct 6 07:30`).
pub(crate) fn ahead(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    let (y, m, d, h, mi) = civil(ms, off);
    let (ty, tm, td, ..) = civil(now + DAY, now_off);
    if (y, m, d) == (ty, tm, td) {
        return format!("tomorrow {:02}:{:02}", h, mi);
    }
    day_and_time(ms, off, now, now_off)
}

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

thread_local! {
    /// The UTC offset of each hour asked (a hover asks every frame).
    static OFFSETS: RefCell<HashMap<u64, i32>> = RefCell::new(HashMap::new());
}

/// The local UTC offset at `ms`, in seconds: std has no time zone,
/// `date` has (`-r` on macOS, `-d @` on GNU); 0 (UTC) when it fails.
/// One `date` per hour of time asked.
pub(crate) fn offset_at(ms: u64) -> i32 {
    let hour = ms / HOUR;
    if let Some(o) = OFFSETS.with(|m| m.borrow().get(&hour).copied()) {
        return o;
    }
    let secs = (hour * HOUR / 1000).to_string();
    let at = |args: &[&str]| {
        std::process::Command::new("date")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| parse_offset(s.trim()))
    };
    let o = at(&["-r", &secs, "+%z"]).or_else(|| at(&["-d", &format!("@{}", secs), "+%z"])).unwrap_or(0);
    OFFSETS.with(|m| m.borrow_mut().insert(hour, o));
    o
}

/// `+0200` / `-0530` in seconds.
fn parse_offset(s: &str) -> Option<i32> {
    let (sign, d) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if d.len() != 4 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m): (i32, i32) = (d[..2].parse().ok()?, d[2..].parse().ok()?);
    Some(sign * (h * 3600 + m * 60))
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
    fn the_civil_date_of_a_moment() {
        assert_eq!(civil(0, 0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(T, 0), (2025, 9, 30, 12, 41));
        assert_eq!(civil(T, PARIS), (2025, 9, 30, 14, 41));
        // a leap day, and an offset that crosses midnight backwards
        assert_eq!(civil(1_709_164_800_000, 0), (2024, 2, 29, 0, 0));
        assert_eq!(civil(1_709_164_800_000, -3600), (2024, 2, 28, 23, 0));
    }

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

    #[test]
    fn utc_offsets_parse() {
        assert_eq!(parse_offset("+0200"), Some(7200));
        assert_eq!(parse_offset("-0530"), Some(-19_800));
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("CEST"), None);
        assert_eq!(parse_offset("+02"), None);
    }
}
