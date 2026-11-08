//! The hub's usage counts, the pure part (page usage-metrics-plan, step 2;
//! architect m_53623 b, m_55989): what the hub counts in a UTC day, from
//! facts already decided elsewhere. A projection, like `activity`: no
//! decision, and nothing read but a kind and its enums (never a text, a
//! name or a path), so the keys are `bise_usage`'s closed ones.
//!
//! - [`of_journal`]: a line sb-core journals (its `journal` effect, live:
//!   a boot replay runs no effect, so nothing counts twice), by an
//!   exhaustive match on its [`JournalKind`];
//! - [`of_obs`]: an observation of an agent's live REPL (the turns, a
//!   compaction, a provider retry), exhaustive on [`Obs`];
//! - [`Counter`]: the day's [`Tally`], the agents' turn time (agent
//!   work: the time between an agent's lines inside a turn, a silence
//!   longer than [`SILENCE_MS`] taken out) and the day's peak of agents
//!   in a turn; at a new UTC day the old day's tally waits to be written.
//!
//! The shell (`daemon/usage_spool.rs`) feeds it and writes its tallies
//! to the spool; the time comes in as an argument.

use std::collections::BTreeMap;

use bise_proto::activity::Party;
use bise_proto::core::JournalKind;
use bise_proto::thread::lines::{Obs, TurnEnd};
use bise_usage::{CountKey, Day, HistKey, MinutesKey, Tally};
use serde_json::Value;

/// A silence inside a turn longer than this is not agent work (a wait on
/// a long command, a stuck provider): 15 min.
pub const SILENCE_MS: u64 = 15 * 60_000;

/// The count of a journal line, if it is one: by its kind and its enums.
pub fn of_journal(ev: &Value) -> Option<CountKey> {
    let s = |k: &str| ev.get(k).and_then(Value::as_str).unwrap_or("");
    match JournalKind::of_line(ev) {
        JournalKind::TaskCreated => Some(created_by(s("parent"))),
        JournalKind::MsgSent => {
            let from = ev.get("msg").and_then(|m| m.get("from")).and_then(Value::as_str).unwrap_or("");
            (Party::of_word(from) == Party::You).then_some(CountKey::UserMessage)
        }
        JournalKind::CardOpened => Some(CountKey::CardOpened),
        // his answer has its way in (`via`); a close without him has none
        JournalKind::CardClosed => (!s("via").is_empty()).then_some(CountKey::CardAnswered),
        JournalKind::EverySet => Some(CountKey::ScheduleSet),
        // an `undelivered` line is a wake that did not reach the agent
        JournalKind::EveryFired => (ev.get("undelivered") != Some(&Value::Bool(true))).then_some(CountKey::ScheduleFired),
        JournalKind::WakeSet => Some(CountKey::TriggerSet),
        // its event came (`hit`); not its max, a stop or its agent gone
        JournalKind::WakeEnd => (s("why") == "hit").then_some(CountKey::TriggerFired),
        JournalKind::Placed => s("err").is_empty().then_some(CountKey::WorktreePlaced),
        // not counted: the hub's own bookkeeping, or counted elsewhere
        // (a land and a page publish by the shell where they happen)
        JournalKind::Lifecycle
        | JournalKind::Workspace
        | JournalKind::Snapshot
        | JournalKind::Dropped
        | JournalKind::Declared
        | JournalKind::Reported
        | JournalKind::FileTouched
        | JournalKind::ClaimDropped
        | JournalKind::Renamed
        | JournalKind::MsgsMoved
        | JournalKind::MsgState
        | JournalKind::Settled
        | JournalKind::CardAnswerPos
        | JournalKind::CardWithdrawn
        | JournalKind::CardLate
        | JournalKind::MainNote
        | JournalKind::NotesFlushed
        | JournalKind::EveryRun
        | JournalKind::EveryStop
        | JournalKind::EveryName
        | JournalKind::XOut
        | JournalKind::XAcked
        | JournalKind::XFailed
        | JournalKind::XSettled
        | JournalKind::XIn
        | JournalKind::XReplied
        | JournalKind::RouteHeld
        | JournalKind::RouteSet
        | JournalKind::RouteSent
        | JournalKind::RouteCancelled
        | JournalKind::ActSet
        | JournalKind::ActDone
        | JournalKind::Follow
        | JournalKind::JobEnd
        | JournalKind::Paused
        | JournalKind::Resumed
        | JournalKind::Unknown => None,
    }
}

/// Who created an agent, by its `parent`: him, main, or another agent.
fn created_by(parent: &str) -> CountKey {
    if parent == "user" {
        CountKey::AgentCreatedByUser
    } else if parent == crate::model::MAIN {
        CountKey::AgentCreatedByMain
    } else {
        CountKey::AgentCreatedByAgent
    }
}

/// What an observation of an agent's REPL does to its turn and the counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObsCount {
    /// a turn starts (true) or ends (false); none: no change
    pub turn: Option<bool>,
    pub count: Option<CountKey>,
}

/// The counts of an observation, by its kind (never its text).
pub fn of_obs(o: &Obs) -> ObsCount {
    let (turn, count) = match o {
        Obs::TurnStarted => (Some(true), None),
        Obs::TurnDone(end) => (
            Some(false),
            match end {
                TurnEnd::Completed => Some(CountKey::TurnDone),
                TurnEnd::Failed(_) => Some(CountKey::TurnFailed),
                // a stop someone asked for, or a word this build does not know
                TurnEnd::Interrupted { .. } | TurnEnd::Other(_) => None,
            },
        ),
        Obs::TurnStalled(_) => (None, Some(CountKey::TurnStalled)),
        Obs::CompactionStarted => (None, Some(CountKey::Compaction)),
        Obs::ProviderRetry(_) => (None, Some(CountKey::ProviderRetry)),
        Obs::Assistant(_)
        | Obs::ToolStarted(_)
        | Obs::ToolFinished { .. }
        | Obs::Plumbing
        | Obs::SteeringReceived(_)
        | Obs::Steered(_)
        | Obs::NotificationReceived(_)
        | Obs::NotificationDelivered(_)
        | Obs::NetworkWait { .. }
        | Obs::HarnessRestarted(_)
        | Obs::CandidateDiscarded(_)
        | Obs::CompactionFailed(_)
        | Obs::SessionRestored(_)
        | Obs::ModelSwapped { .. }
        | Obs::CompactionDone(_)
        | Obs::Usage(_)
        | Obs::NullIteration
        | Obs::Other(_) => (None, None),
    };
    ObsCount { turn, count }
}

/// The UTC day of a time in ms.
pub fn day_of(ms: u64) -> Day {
    Day::from_unix_secs(ms / 1000)
}

/// What the hub counted today, and the day before while it waits to be
/// written.
#[derive(Clone, Debug)]
pub struct Counter {
    day: Day,
    tally: Tally,
    /// the most agents in a turn at once today
    peak: u32,
    /// the agents in a turn (by folder), and their last line's time
    in_turn: BTreeMap<String, u64>,
    /// closed days not written yet
    closed: Vec<(Day, Tally)>,
}

impl Counter {
    pub fn new(now_ms: u64) -> Counter {
        Counter { day: day_of(now_ms), tally: Tally::default(), peak: 0, in_turn: BTreeMap::new(), closed: Vec::new() }
    }

    /// The day of `now_ms` is a new one: today's tally waits to be
    /// written, the new day starts at zero (its peak at the agents in a
    /// turn now).
    fn roll(&mut self, now_ms: u64) {
        let day = day_of(now_ms);
        if day == self.day {
            return;
        }
        let done = self.snapshot();
        if !done.is_empty() {
            self.closed.push((self.day, done));
        }
        self.day = day;
        self.tally = Tally::default();
        self.peak = self.in_turn.len() as u32;
    }

    /// Count `key` once at `now_ms`.
    pub fn count(&mut self, key: CountKey, now_ms: u64) {
        self.roll(now_ms);
        self.tally.count(key);
    }

    /// A journal line sb-core wrote at `now_ms`.
    pub fn journal(&mut self, ev: &Value, now_ms: u64) {
        if let Some(k) = of_journal(ev) {
            self.count(k, now_ms);
        }
    }

    /// A line of agent `dir`'s live REPL at `now_ms` (`obs`: its
    /// observation, if it is one). Inside a turn, the time since its last
    /// line is agent work, unless longer than [`SILENCE_MS`].
    pub fn repl_line(&mut self, dir: &str, obs: Option<&Obs>, now_ms: u64) {
        self.roll(now_ms);
        if let Some(last) = self.in_turn.get_mut(dir) {
            let gap = now_ms.saturating_sub(*last);
            if gap <= SILENCE_MS {
                self.tally.time(MinutesKey::AgentWork, gap / 1000);
            }
            *last = now_ms;
        }
        let Some(o) = obs.map(of_obs) else { return };
        if let Some(k) = o.count {
            self.tally.count(k);
        }
        match o.turn {
            Some(true) => {
                self.in_turn.insert(dir.to_string(), now_ms);
                self.peak = self.peak.max(self.in_turn.len() as u32);
            }
            Some(false) => {
                self.in_turn.remove(dir);
            }
            None => {}
        }
    }

    /// Agent `dir`'s REPL is gone: its turn, if any, ends without a line.
    pub fn gone(&mut self, dir: &str) {
        self.in_turn.remove(dir);
    }

    /// Today's tally as written: the counts and times, and the day's
    /// peak as its one observation.
    fn snapshot(&self) -> Tally {
        let mut t = self.tally.clone();
        t.observe(HistKey::PeakAgents, self.peak);
        t
    }

    /// What to write at `now_ms`: the closed days waiting, then today's
    /// (an empty day: nothing).
    pub fn due(&mut self, now_ms: u64) -> Vec<(Day, Tally)> {
        self.roll(now_ms);
        let mut out = std::mem::take(&mut self.closed);
        let today = self.snapshot();
        if !today.is_empty() {
            out.push((self.day, today));
        }
        out
    }

    /// Sharing went off: everything counted is dropped.
    pub fn clear(&mut self, now_ms: u64) {
        *self = Counter { in_turn: std::mem::take(&mut self.in_turn), ..Counter::new(now_ms) };
    }
}

#[cfg(test)]
#[path = "usage_count_tests.rs"]
mod tests;
