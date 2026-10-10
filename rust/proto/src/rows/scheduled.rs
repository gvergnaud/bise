//! A project's live scheduled tasks (`sb every`) and how they end.

use super::WaitingOn;
use serde::{Deserialize, Serialize};

/// A live scheduled task of a project (`sb every`), the `scheduled`
/// event's row (⌘K's source, the scheduled screen): data, the window
/// formats the times; `every` the task's how-often words (`every 2m`,
/// `every day 07:30`, `once`: bise_proto::thread::scheduled::every_words,
/// the TUI's), `done` its runs so far.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScheduledTask {
    pub id: u64,
    /// the agent it wakes
    pub agent: String,
    /// who set it
    pub by: String,
    /// its words, what the agent reads at each run
    pub words: String,
    /// its name, what the lists show (sched-names): the hub's model's or
    /// `--name`, else the plain fallback of its words; always filled by
    /// the hub (empty only from an older one)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub every: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<u64>,
    pub done: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<u64>,
    /// the page it keeps fresh (`--page`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// P4c-4a: the hub's how-often label (`every 2m`, `every day
    /// 07:30`), before `every`'s `once` and times words
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// when it last woke its agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ms: Option<u64>,
    /// its last runs, oldest first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<u64>,
    /// an ended one (`HubEv::Scheduled.ended` only): when, how, by whom
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<ScheduledEnd>,
    /// who stopped it (`end` is `stopped`): you, or an agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_by: Option<WaitingOn>,
}

/// How a scheduled task ended (the hub's `every::end_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ScheduledEnd {
    /// it ran its times
    Times,
    /// its end time passed
    Until,
    /// its agent is gone
    Gone,
    /// someone stopped it (`stopped_by`), or an older line said nothing
    Stopped,
    /// an end this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

impl ScheduledEnd {
    /// The hub's word (`times`, `until`, `gone`, `stopped`), typed; none
    /// for "" (a live one).
    pub fn of_word(w: &str) -> Option<ScheduledEnd> {
        match w {
            "" => None,
            "times" => Some(ScheduledEnd::Times),
            "until" => Some(ScheduledEnd::Until),
            "gone" => Some(ScheduledEnd::Gone),
            "stopped" => Some(ScheduledEnd::Stopped),
            _ => Some(ScheduledEnd::Unknown),
        }
    }

    /// Its word, the one `of_word` reads ("" for an unknown one).
    pub fn word(self) -> &'static str {
        match self {
            ScheduledEnd::Times => "times",
            ScheduledEnd::Until => "until",
            ScheduledEnd::Gone => "gone",
            ScheduledEnd::Stopped => "stopped",
            ScheduledEnd::Unknown => "",
        }
    }
}

