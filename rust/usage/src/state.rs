//! `state.json` (`Home::usage_state`): the machine's own memory of its use,
//! for the packet's retention fields. Its first launch (the cohort week
//! and the milestones' delays), which UTC days were active, and the
//! milestones reached and sent. Never sent as it is: a packet carries only
//! its [`DayView`] of one day and the unsent milestones.
//!
//! The active days are a bit set of the 64 days up to `last_day` (bit `i`
//! = `last_day - i`): 28 days for the packet's window, plus the 30 days a
//! spooled day may wait to be sent.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::day::{Day, IsoWeek};
use crate::key::{Delay, Milestone};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalState {
    /// When bise first counted here (unix seconds).
    pub first_launch_secs: u64,
    /// The ISO week of that first launch.
    pub cohort_week: IsoWeek,
    /// The newest day of [`LocalState::active`].
    pub last_day: Day,
    /// Bit `i`: the day `last_day - i` was active.
    pub active: u64,
    /// Each milestone reached, with its delay since the first launch.
    #[serde(default)]
    pub milestones: BTreeMap<Milestone, Delay>,
    /// The milestones a packet the server took carried.
    #[serde(default)]
    pub sent: BTreeSet<Milestone>,
}

/// The local state's view of one day, as a packet carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayView {
    pub cohort_week: IsoWeek,
    /// Active days among the 7 that end on that day (0 to 7).
    pub active_days_7: u8,
    /// Active days among the 28 that end on that day (0 to 28).
    pub active_days_28: u8,
    /// That day was active and the first active one of its ISO week.
    pub first_today_of_week: bool,
}

const BITS: u32 = u64::BITS;

impl LocalState {
    /// The state of a first launch at `now_secs`.
    pub fn new(now_secs: u64) -> LocalState {
        let today = Day::from_unix_secs(now_secs);
        LocalState {
            first_launch_secs: now_secs,
            cohort_week: today.iso_week(),
            last_day: today,
            active: 0,
            milestones: BTreeMap::new(),
            sent: BTreeSet::new(),
        }
    }

    /// `day` was active. A day older than the bit set's reach is not kept.
    pub fn mark_active(&mut self, day: Day) {
        if day > self.last_day {
            let shift = day.0 - self.last_day.0;
            self.active = if shift >= BITS { 0 } else { self.active << shift };
            self.last_day = day;
        }
        let back = self.last_day.0 - day.0;
        if back < BITS {
            self.active |= 1 << back;
        }
    }

    /// Whether `day` is known active.
    pub fn is_active(&self, day: Day) -> bool {
        day <= self.last_day && self.last_day.0 - day.0 < BITS && self.active & (1 << (self.last_day.0 - day.0)) != 0
    }

    fn active_in(&self, last: Day, n: u32) -> u8 {
        (0..n).filter(|i| last.0 >= *i && self.is_active(Day(last.0 - i))).count() as u8
    }

    /// The packet's view of `day`.
    pub fn view(&self, day: Day) -> DayView {
        let earlier_this_week = (1..=day.weekday()).any(|i| self.is_active(Day(day.0 - i)));
        DayView {
            cohort_week: self.cohort_week,
            active_days_7: self.active_in(day, 7),
            active_days_28: self.active_in(day, 28),
            first_today_of_week: self.is_active(day) && !earlier_this_week,
        }
    }

    /// A milestone reached at `now_secs`: kept with its delay, the first time only.
    pub fn reach(&mut self, m: Milestone, now_secs: u64) {
        let delay = Delay::of_secs(now_secs.saturating_sub(self.first_launch_secs));
        self.milestones.entry(m).or_insert(delay);
    }

    /// The milestones reached and not sent yet: the next packet's.
    pub fn unsent(&self) -> BTreeMap<Milestone, Delay> {
        self.milestones.iter().filter(|(m, _)| !self.sent.contains(m)).map(|(m, d)| (*m, *d)).collect()
    }

    /// The server took a packet with these milestones: never sent again.
    pub fn mark_sent<'a>(&mut self, ms: impl IntoIterator<Item = &'a Milestone>) {
        self.sent.extend(ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn active_days_count_in_their_window() {
        // 2026-11-02 is a Monday
        let mut s = LocalState::new(day("2026-10-01").start_secs());
        for d in ["2026-10-20", "2026-11-02", "2026-11-04", "2026-11-05"] {
            s.mark_active(day(d));
        }
        let v = s.view(day("2026-11-05"));
        assert_eq!((v.active_days_7, v.active_days_28), (3, 4));
        assert!(!v.first_today_of_week, "Monday was active");
        assert!(s.view(day("2026-11-02")).first_today_of_week);
        assert!(!s.view(day("2026-11-03")).first_today_of_week, "not active that day");
        assert!(!s.view(day("2026-10-19")).first_today_of_week, "a Monday not active, nothing before it that week");
        assert_eq!(s.view(day("2026-11-01")).active_days_7, 0);
        assert_eq!(v.cohort_week.to_string(), "2026-W40");
    }

    #[test]
    fn a_past_day_marked_late_still_counts() {
        let mut s = LocalState::new(0);
        s.mark_active(day("2026-11-05"));
        s.mark_active(day("2026-11-03"));
        assert!(s.is_active(day("2026-11-03")));
        assert_eq!(s.last_day, day("2026-11-05"));
        s.mark_active(day("2027-03-01"));
        assert!(!s.is_active(day("2026-11-05")), "past the bit set's reach");
        assert!(s.is_active(day("2027-03-01")));
    }

    #[test]
    fn a_milestone_is_kept_once_and_sent_once() {
        let mut s = LocalState::new(1_000_000);
        s.reach(Milestone::FirstAgent, 1_000_000 + 600);
        s.reach(Milestone::FirstAgent, 1_000_000 + 900_000);
        assert_eq!(s.unsent(), BTreeMap::from([(Milestone::FirstAgent, Delay::UnderHour)]));
        let sent: Vec<_> = s.unsent().into_keys().collect();
        s.mark_sent(&sent);
        assert!(s.unsent().is_empty());
        let back: LocalState = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
