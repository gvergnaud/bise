//! UTC days and ISO weeks: the only dates the usage data holds (no time
//! zone, no hour). A [`Day`] is a count of days since 1970-01-01 UTC,
//! written `YYYY-MM-DD`; an [`IsoWeek`] is written `YYYY-Www`.

use std::fmt;

/// A UTC day: days since 1970-01-01.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub u32);

/// An ISO 8601 week: its year and its number (1 to 53).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IsoWeek {
    pub year: i32,
    pub week: u8,
}

const SECS_PER_DAY: u64 = 86_400;

impl Day {
    /// The UTC day of a unix time in seconds.
    pub fn from_unix_secs(secs: u64) -> Day {
        Day((secs / SECS_PER_DAY) as u32)
    }

    /// The unix time of its midnight UTC.
    pub fn start_secs(self) -> u64 {
        u64::from(self.0) * SECS_PER_DAY
    }

    /// The day of a civil date, None for an impossible one or one before 1970.
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Option<Day> {
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        let n = days_from_civil(y, m, d);
        u32::try_from(n).ok().map(Day)
    }

    /// Its civil date (year, month 1-12, day 1-31).
    pub fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(i64::from(self.0))
    }

    /// Monday 0 to Sunday 6 (1970-01-01 was a Thursday).
    pub fn weekday(self) -> u32 {
        (self.0 + 3) % 7
    }

    /// `n` days later.
    pub fn plus(self, n: u32) -> Day {
        Day(self.0 + n)
    }

    /// The same day of the month `n` months later; a day the month lacks
    /// (Jan 31 + 1 month) becomes its last day.
    pub fn plus_months(self, n: u32) -> Day {
        let (y, m, d) = self.ymd();
        let total = (y * 12 + (m as i32 - 1)) + n as i32;
        let (y2, m2) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
        Day::from_ymd(y2, m2, d.min(days_in_month(y2, m2))).expect("a later civil date is a day")
    }

    /// Its ISO week.
    pub fn iso_week(self) -> IsoWeek {
        // the week's Thursday decides its year
        let thursday = Day(self.0 + 3 - self.weekday());
        let (year, _, _) = thursday.ymd();
        let jan1 = Day::from_ymd(year, 1, 1).expect("Jan 1 of a year after 1970");
        IsoWeek { year, week: ((thursday.0 - jan1.0) / 7 + 1) as u8 }
    }

    /// Parse `YYYY-MM-DD`.
    pub fn parse(s: &str) -> Option<Day> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let num = |r: std::ops::Range<usize>| s.get(r).filter(|p| p.bytes().all(|c| c.is_ascii_digit()))?.parse().ok();
        Day::from_ymd(num(0..4)? as i32, num(5..7)?, num(8..10)?)
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl fmt::Display for IsoWeek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-W{:02}", self.year, self.week)
    }
}

impl IsoWeek {
    /// Parse `YYYY-Www`.
    pub fn parse(s: &str) -> Option<IsoWeek> {
        let (y, w) = s.split_once("-W")?;
        let all_digits = |p: &str| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit());
        if y.len() != 4 || w.len() != 2 || !all_digits(y) || !all_digits(w) {
            return None;
        }
        let week: u8 = w.parse().ok()?;
        (1..=53).contains(&week).then_some(IsoWeek { year: y.parse().ok()?, week })
    }
}

macro_rules! serde_as_text {
    ($t:ty, $what:literal) => {
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$t, D::Error> {
                let s = String::deserialize(d)?;
                <$t>::parse(&s).ok_or_else(|| serde::de::Error::custom($what))
            }
        }
    };
}
serde_as_text!(Day, "not a YYYY-MM-DD day");
serde_as_text!(IsoWeek, "not a YYYY-Www week");

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant's days_from_civil / civil_from_days (public domain).
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(if m <= 2 { y - 1 } else { y });
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn days_print_and_parse_as_civil_dates() {
        assert_eq!(Day(0).to_string(), "1970-01-01");
        assert_eq!(Day::from_unix_secs(1_793_000_000).to_string(), "2026-10-26");
        assert_eq!(day("2028-02-29").to_string(), "2028-02-29");
        for bad in ["2027-02-29", "2026-13-01", "2026-1-01", "2026/01/01", "1969-12-31", "-026-01-01"] {
            assert_eq!(Day::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn iso_weeks_follow_their_thursday() {
        assert_eq!(day("2026-11-05").iso_week().to_string(), "2026-W45");
        // Jan 1 2027 is a Friday: still the last week of 2026
        assert_eq!(day("2027-01-01").iso_week().to_string(), "2026-W53");
        // Dec 29 2025 is a Monday whose Thursday is in 2026
        assert_eq!(day("2025-12-29").iso_week().to_string(), "2026-W01");
        assert_eq!(IsoWeek::parse("2026-W45"), Some(IsoWeek { year: 2026, week: 45 }));
        assert_eq!(IsoWeek::parse("2026-W54"), None);
    }

    #[test]
    fn months_clamp_to_the_last_day() {
        assert_eq!(day("2026-01-31").plus_months(1), day("2026-02-28"));
        assert_eq!(day("2026-01-15").plus_months(13), day("2027-02-15"));
        assert_eq!(day("2026-11-30").plus_months(3), day("2027-02-28"));
    }
}
