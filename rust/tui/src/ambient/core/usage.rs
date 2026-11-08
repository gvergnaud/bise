//! The app's usage counts in its core (page usage-metrics-plan, step 3;
//! architect m_53623 a, d.3): each app command by its name, never its
//! arguments; the app's own time and gestures (`usage_time`, from
//! Electron's main); the setup's provider check by its class of failure.
//!
//! - [`of_cmd`]: every [`AppCmd`] goes through one exhaustive match, no
//!   `_ =>`: a new command must say here, at compile time, "counted under
//!   that key" or "not counted" (a test reads this file for a wildcard);
//! - [`of_gesture`], [`of_check`]: the same for the app's gestures and the
//!   provider check's end;
//! - [`Usage`]: the day's [`Tally`] in memory, written whole to its sink
//!   every 5 minutes, at a new UTC day and at a clean exit.
//!
//! The core counts only with a [`Usage`], made by its binary only when
//! `bise_usage::spool::sink` gave a sink (sharing allowed; it asks the
//! permit again at each write). No Usage: nothing is counted at all. The
//! tests give a recording sink.

use std::time::{Duration, Instant};

use bise_proto::app_cmd::{AppCmd, AppGesture};
use bise_proto::app_rows::CheckWhy;
use bise_usage::{CountKey, Day, MinutesKey, Tally};

use super::Cmd;

/// A day's tally written whole where it goes: production's is
/// `bise_usage::spool::sink` (made only with a Permit), a test's records.
pub type UsageSink = bise_usage::spool::Sink;

/// How often the tally is written: a crash loses at most this much.
pub(crate) const WRITE_EVERY: Duration = Duration::from_secs(5 * 60);

/// What an app command counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    Under(CountKey),
    NotCounted,
}

use Counted::{NotCounted, Under};

/// An app command's count, by its name and closed fields only. The ones
/// his hub already counts (his messages, answers, cards) are not counted
/// here: the hub sees them in its journal (step 2).
pub(crate) fn of_cmd(c: &AppCmd) -> Counted {
    match c {
        // the palette (⌘K)
        AppCmd::Index => Under(CountKey::Search),
        AppCmd::PluginSet { on: true, .. } => Under(CountKey::PluginEnabled),
        AppCmd::PluginSet { on: false, .. } => NotCounted,
        // fn's talk to main, and voice mode
        AppCmd::TalkStart { .. } => Under(CountKey::VoiceStarted),
        AppCmd::VoiceMode { on: true, .. } => Under(CountKey::VoiceStarted),
        AppCmd::VoiceMode { on: false, .. } => NotCounted,
        AppCmd::DictateStart { .. } => Under(CountKey::DictationStarted),
        // its seconds and gestures are counted by Usage::took
        AppCmd::UsageTime { .. } => NotCounted,
        // his choice about the counts themselves: never counted
        AppCmd::Telemetry { .. } => NotCounted,
        // main's own (never reaches the core): artifacts are counted by kind by the window
        AppCmd::Open { .. } => NotCounted,
        // his words and answers: the hub counts them (user_message, card_answered)
        AppCmd::Send { .. } | AppCmd::Start { .. } | AppCmd::Answer { .. } | AppCmd::AgentSend { .. } => NotCounted,
        // the setup: its provider check is counted at its end (of_check)
        AppCmd::ProviderUse { .. }
        | AppCmd::SetupCheck
        | AppCmd::SetupTest { .. }
        | AppCmd::SetupFirstCancel
        | AppCmd::SignIn { .. }
        | AppCmd::SignInCancel { .. }
        | AppCmd::KeyRemove { .. }
        | AppCmd::Keychain { .. }
        | AppCmd::RoleSet { .. }
        | AppCmd::HelperPerms { .. }
        | AppCmd::PrefsSet { .. } => NotCounted,
        // his projects
        AppCmd::ProjectAdd { .. }
        | AppCmd::ProjectRemove { .. }
        | AppCmd::ProjectMove { .. }
        | AppCmd::ProjectRename { .. }
        | AppCmd::FoundScan { .. }
        | AppCmd::HubRetry { .. }
        | AppCmd::Shown { .. } => NotCounted,
        // the rest of plugins
        AppCmd::Plugins { .. }
        | AppCmd::PluginGrant { .. }
        | AppCmd::PluginSetting { .. }
        | AppCmd::PluginConnect { .. }
        | AppCmd::PluginsRefresh { .. }
        | AppCmd::PluginAdd { .. }
        | AppCmd::PluginAddConfirm { .. }
        | AppCmd::PluginAddCancel { .. }
        | AppCmd::PluginLogin { .. }
        | AppCmd::PluginLogout { .. }
        | AppCmd::PluginRemove { .. } => NotCounted,
        // the rest of voice and the mic
        AppCmd::TalkEnd
        | AppCmd::TalkCancel
        | AppCmd::ShotDrop { .. }
        | AppCmd::Shot { .. }
        | AppCmd::DictateStop { .. }
        | AppCmd::DictateCancel { .. }
        | AppCmd::DictateRetry { .. }
        | AppCmd::VoiceMute { .. }
        | AppCmd::VoiceType { .. }
        | AppCmd::VoiceSend
        | AppCmd::VoiceCut
        | AppCmd::Hush
        | AppCmd::CutIn
        | AppCmd::Quiet { .. }
        | AppCmd::Voice { .. }
        | AppCmd::FakeWords { .. } => NotCounted,
        // computer use: the hub counts its calls (step 10)
        AppCmd::ComputerUse { .. } | AppCmd::Computer { .. } => NotCounted,
        // the window's reads and its agents' panels
        AppCmd::CardInView { .. }
        | AppCmd::Commands
        | AppCmd::Skills { .. }
        | AppCmd::Files { .. }
        | AppCmd::FilePicked { .. }
        | AppCmd::Folders { .. }
        | AppCmd::FoldersMove { .. }
        | AppCmd::ProcessesAll { .. }
        | AppCmd::AgentPreview { .. }
        | AppCmd::AgentHistory { .. }
        | AppCmd::AgentUnwatch { .. }
        | AppCmd::Follow { .. }
        | AppCmd::Stop { .. }
        | AppCmd::Archive { .. }
        | AppCmd::Unarchive { .. }
        | AppCmd::EveryStop { .. } => NotCounted,
        // the app itself
        AppCmd::AwayBack { .. } | AppCmd::AppVersion { .. } | AppCmd::BiseUpdate {} => NotCounted,
        // a newer app's command: its name is not a key
        AppCmd::Unknown { .. } => NotCounted,
    }
}

/// A gesture's key; a newer app's gesture is not counted.
pub(crate) fn of_gesture(g: AppGesture) -> Option<CountKey> {
    match g {
        AppGesture::InboxOpened => Some(CountKey::InboxOpened),
        AppGesture::ArtifactPage => Some(CountKey::ArtifactOpenedPage),
        AppGesture::ArtifactDoc => Some(CountKey::ArtifactOpenedDoc),
        AppGesture::ArtifactSheet => Some(CountKey::ArtifactOpenedSheet),
        AppGesture::ArtifactSlides => Some(CountKey::ArtifactOpenedSlides),
        AppGesture::ArtifactImage => Some(CountKey::ArtifactOpenedImage),
        AppGesture::ArtifactLink => Some(CountKey::ArtifactOpenedLink),
        AppGesture::ArtifactCode => Some(CountKey::ArtifactOpenedCode),
        AppGesture::ArtifactOther => Some(CountKey::ArtifactOpenedOther),
        AppGesture::SetupStarted => Some(CountKey::SetupStarted),
        AppGesture::CrashRenderer => Some(CountKey::CrashAppRenderer),
        AppGesture::CrashChild => Some(CountKey::CrashAppChild),
        AppGesture::CrashCore => Some(CountKey::CrashCore),
        AppGesture::Unknown => None,
    }
}

/// The end of the setup's provider check: it works, or its failure's class.
pub(crate) fn of_check(end: Result<(), CheckWhy>) -> CountKey {
    match end {
        Ok(()) => CountKey::SetupDone,
        Err(CheckWhy::WrongKey | CheckWhy::NoKey | CheckWhy::NoAccess | CheckWhy::NoCredit) => CountKey::SetupFailedAuth,
        Err(CheckWhy::Unreachable) => CountKey::SetupFailedNetwork,
        Err(CheckWhy::Model | CheckWhy::Config | CheckWhy::Unknown) => CountKey::SetupFailedOther,
    }
}

/// `usage_time`'s numbers and gestures (bise_proto's `AppCmd::UsageTime`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTime {
    pub open_s: u32,
    pub focused_s: u32,
    pub engaged_s: u32,
    pub gestures: Vec<AppGesture>,
}

/// The core's counts of one UTC day, and where they go.
pub struct Usage {
    sink: UsageSink,
    day: Day,
    tally: Tally,
    /// counted since the last write
    dirty: bool,
    written: Instant,
}

impl Usage {
    /// Counting starts at zero now (`unix_s`: the clock's seconds).
    pub fn new(sink: UsageSink, unix_s: u64, now: Instant) -> Usage {
        Usage { sink, day: Day::from_unix_secs(unix_s), tally: Tally::default(), dirty: false, written: now }
    }

    pub(crate) fn count(&mut self, key: CountKey) {
        self.tally.count(key);
        self.dirty = true;
    }

    /// An app command the core took ([`Cmd::parse_counted`]): its count,
    /// and `usage_time`'s seconds and gestures.
    pub(crate) fn took(&mut self, c: &Cmd, counted: Counted) {
        if let Under(key) = counted {
            self.count(key);
        }
        if let Cmd::UsageTime(t) = c {
            self.usage_time(t);
        }
    }

    fn usage_time(&mut self, t: &AppTime) {
        for (key, secs) in [(MinutesKey::AppOpen, t.open_s), (MinutesKey::AppFocused, t.focused_s), (MinutesKey::AppEngaged, t.engaged_s)] {
            self.tally.time(key, u64::from(secs));
        }
        for key in t.gestures.iter().filter_map(|g| of_gesture(*g)) {
            self.tally.count(key);
        }
        self.dirty = true;
    }

    /// The clock's tick: a new UTC day writes the old one and starts at
    /// zero; else the tally is written every [`WRITE_EVERY`].
    pub(crate) fn tick(&mut self, now: Instant, unix_s: u64) {
        let today = Day::from_unix_secs(unix_s);
        if today != self.day {
            self.write(now);
            self.day = today;
            self.tally = Tally::default();
        } else if now.duration_since(self.written) >= WRITE_EVERY {
            self.write(now);
        }
    }

    /// Writes the day's tally when something was counted since the last write.
    pub(crate) fn write(&mut self, now: Instant) {
        self.written = now;
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        if let Err(e) = (self.sink)(self.day, &self.tally) {
            eprintln!("ambient-core: usage counts not written: {e}");
        }
    }
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
