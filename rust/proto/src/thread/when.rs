//! When something happens, in words, pure (batch 3b, architect m_11122):
//! moved from the TUI's when.rs so the hub's thread fold and the TUI say
//! a scheduled task's times the same way. The caller passes the UTC
//! offsets (seconds east) at each moment: this crate has no clock and no
//! time zone; `now` is the line's own time in the fold, so a replay never
//! moves it.

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// A moment in local time: (year, month 1..=12, day, hour, minute).
pub type Civil = (i64, u32, u32, u32, u32);

/// `ms` (since the epoch) at UTC offset `off` (seconds east).
pub fn civil(ms: u64, off: i32) -> Civil {
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
pub fn day_and_time(ms: u64, off: i32, now: u64, now_off: i32) -> String {
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
pub fn ahead(ms: u64, off: i32, now: u64, now_off: i32) -> String {
    let (y, m, d, h, mi) = civil(ms, off);
    let (ty, tm, td, ..) = civil(now + DAY, now_off);
    if (y, m, d) == (ty, tm, td) {
        return format!("tomorrow {:02}:{:02}", h, mi);
    }
    day_and_time(ms, off, now, now_off)
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
    fn a_time_to_come_says_today_tomorrow_or_its_day() {
        assert_eq!(ahead(T + HOUR, 0, T, 0), "13:41");
        assert_eq!(ahead(T + DAY, 0, T, 0), "tomorrow 12:41");
        assert_eq!(ahead(T + 6 * DAY, 0, T, 0), "oct 6 12:41");
        assert_eq!(ahead(T + 10 * HOUR, PARIS, T, PARIS), "tomorrow 00:41", "the local zone decides the day");
        assert_eq!(day_and_time(T - 2 * DAY, 0, T, 0), "sep 28 12:41");
        assert_eq!(day_and_time(T - 300 * DAY, 0, T, 0), "dec 4 2024 12:41");
    }
}
