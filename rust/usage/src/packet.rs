//! The day's packet: what the machine sends for one closed UTC day, made
//! from the sum of its sources ([`Tally::sum`]), the local state's view of
//! that day and the build. No free text: every string is a [`Day`], an
//! [`IsoWeek`], the install id, a version of `[0-9a-z.-]` or a closed name.
//!
//! The caps are applied here and only here: a count or a bucket at most
//! [`COUNT_MAX`], minutes rounded to the nearest [`MINUTE_STEP`] and at
//! most [`MinutesKey::max_minutes`]; zeros are left out.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::day::{Day, IsoWeek};
use crate::install_id::InstallId;
use crate::key::{CountKey, Delay, HistKey, Milestone, MinutesKey, COUNT_MAX, MINUTE_STEP};
use crate::state::DayView;
use crate::tally::Tally;

/// The packet's schema version: raised with every change of its shape or
/// of a key (`schema.json`, `docs/telemetry.md`).
pub const SCHEMA: u32 = 1;

closed! {
    /// The release channel of the build. A dev build never shares.
    pub enum Channel {
        Stable = "stable",
        Beta = "beta",
        Dev = "dev",
    }
}

closed! {
    /// Which of his surfaces counted that day.
    pub enum Surface {
        /// The desktop app only.
        Desktop = "desktop",
        /// A terminal (the TUI) only.
        Terminal = "terminal",
        /// Both.
        Both = "both",
        /// Neither: only a hub counted.
        Headless = "headless",
    }
}

closed! {
    /// The CPU.
    pub enum Arch {
        Arm64 = "arm64",
        X86_64 = "x86_64",
        Other = "other",
    }
}

/// The build that sends: its version, channel, OS and CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// The release's version id, as the build knows it.
    pub version: String,
    pub channel: Channel,
    /// macOS's major version (26 for Tahoe).
    pub os_major: u16,
    pub arch: Arch,
}

/// A version carried only when it is short and made of `[0-9a-z.-]`;
/// anything else is sent as `unknown`.
pub fn clean_version(v: &str) -> String {
    let ok = (1..=24).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase() || c == b'.' || c == b'-');
    if ok {
        v.to_string()
    } else {
        "unknown".to_string()
    }
}

impl Arch {
    /// The CPU of this build (`std::env::consts::ARCH`'s names).
    pub fn of(arch: &str) -> Arch {
        match arch {
            "aarch64" | "arm64" => Arch::Arm64,
            "x86_64" => Arch::X86_64,
            _ => Arch::Other,
        }
    }
}

/// The most a packet's `os_major` says (a bigger number is a bad read).
pub const OS_MAJOR_MAX: u16 = 99;

/// One closed day's packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Packet {
    pub schema: u32,
    pub day: Day,
    /// Sent as its UUID alone (its creation day stays on the Mac).
    #[serde(serialize_with = "crate::install_id::serialize_uuid")]
    pub install_id: InstallId,
    pub cohort_week: IsoWeek,
    pub active_days_7: u8,
    pub active_days_28: u8,
    pub first_today_of_week: bool,
    pub version: String,
    pub channel: Channel,
    pub surface: Surface,
    pub os_major: u16,
    pub arch: Arch,
    pub counts: BTreeMap<CountKey, u16>,
    pub minutes: BTreeMap<MinutesKey, u16>,
    pub hist: BTreeMap<HistKey, Vec<u16>>,
    pub milestones: BTreeMap<Milestone, Delay>,
}

/// What a packet is made of, besides the caps.
pub struct Parts<'a> {
    pub day: Day,
    pub install_id: &'a InstallId,
    /// The local state's view of that day ([`crate::state::LocalState::view`]).
    pub view: DayView,
    pub build: &'a Build,
    pub surface: Surface,
    /// The sum of the day's sources.
    pub tally: &'a Tally,
    /// The milestones reached and not sent yet.
    pub milestones: BTreeMap<Milestone, Delay>,
}

fn cap(n: u64) -> u16 {
    n.min(u64::from(COUNT_MAX)) as u16
}

/// Seconds to minutes, rounded to the nearest step, at most `max`.
pub fn round_minutes(secs: u64, max: u16) -> u16 {
    let step = u64::from(MINUTE_STEP) * 60;
    let rounded = (secs + step / 2) / step * u64::from(MINUTE_STEP);
    rounded.min(u64::from(max - max % MINUTE_STEP)) as u16
}

impl Packet {
    /// The packet of a day: the caps, the rounding, the zeros left out.
    pub fn build(p: Parts) -> Packet {
        Packet {
            schema: SCHEMA,
            day: p.day,
            install_id: p.install_id.clone(),
            cohort_week: p.view.cohort_week,
            active_days_7: p.view.active_days_7,
            active_days_28: p.view.active_days_28,
            first_today_of_week: p.view.first_today_of_week,
            version: clean_version(&p.build.version),
            channel: p.build.channel,
            surface: p.surface,
            os_major: p.build.os_major.min(OS_MAJOR_MAX),
            arch: p.build.arch,
            counts: p.tally.counts.iter().map(|(k, n)| (*k, cap(*n))).filter(|(_, n)| *n > 0).collect(),
            minutes: p.tally.seconds.iter().map(|(k, s)| (*k, round_minutes(*s, k.max_minutes()))).filter(|(_, m)| *m > 0).collect(),
            hist: p
                .tally
                .hist
                .iter()
                .map(|(k, b)| (*k, b.iter().map(|n| cap(*n)).collect::<Vec<_>>()))
                .filter(|(_, b)| b.iter().any(|n| *n > 0))
                .collect(),
            milestones: p.milestones,
        }
    }
}

impl Surface {
    /// The surface of a day from whether the app's core and a TUI counted.
    pub fn of(desktop: bool, terminal: bool) -> Surface {
        match (desktop, terminal) {
            (true, true) => Surface::Both,
            (true, false) => Surface::Desktop,
            (false, true) => Surface::Terminal,
            (false, false) => Surface::Headless,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_round_to_five_and_stop_at_their_cap() {
        assert_eq!([0, 149, 150, 449, 450, 3600].map(|s| round_minutes(s, 1440)), [0, 0, 5, 5, 10, 60]);
        assert_eq!(round_minutes(200_000, 1440), 1440);
        assert_eq!(round_minutes(10_000_000, MinutesKey::AgentWork.max_minutes()), 14_400);
    }

    #[test]
    fn a_version_is_kept_only_when_plain() {
        assert_eq!(clean_version("V2026.11.2-3"), "unknown", "a capital");
        assert_eq!(clean_version("2026.11.2-3"), "2026.11.2-3");
        for bad in ["/Users/camille/bise", "camille@example.com", "", "1.0 beta", "aaaaaaaaaaaaaaaaaaaaaaaaa"] {
            assert_eq!(clean_version(bad), "unknown", "{bad}");
        }
    }
}
