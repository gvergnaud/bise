//! tasks (docs/tasks.md; architect m_64809): the user's tasks a hub lists,
//! the `works` event's rows. Facts only, as sb-core's view gives them
//! (the work's journal line and the status sb-core computed from the
//! facts, wstatus.bend): the window picks the words from the words
//! catalog's `tasks` section and formats the times at render (a relative
//! time sent as text goes stale). The pick rule lives twice, in the hub
//! (switchboard's `task_list::what`) and in the window, both pinned by
//! `fixtures/work_what.jsonl`.

use serde::{Deserialize, Serialize};

/// A task's status, computed by sb-core, never declared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    /// an open card, a page question or a step of his carries it
    NeedsYou,
    /// one of its agents is in a turn
    Working,
    /// on something not him, named in `about`
    #[default]
    Waiting,
    /// nothing moves for 2 h, or an agent of it crashed
    Stalled,
    /// its agents are done, a land, a merge or a look is left to him
    ToReview,
    Paused,
    Done,
    /// never done (a changelog, a watch)
    Ongoing,
    /// a status this version doesn't know (a newer hub): drawn as quiet
    #[serde(other)]
    Unknown,
}

impl WorkStatus {
    /// sb-core's word (`needs_you`, `to_review`...), typed.
    pub fn of_word(w: &str) -> WorkStatus {
        serde_json::from_value(serde_json::Value::String(w.to_string())).unwrap_or(WorkStatus::Unknown)
    }
}

/// One done-when item: its words, ticked, by whom (an agent's report, or
/// `user`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkGoal {
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub done: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
}

/// When a task was marked done, and by whom (`user`, or main on his word).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkDone {
    pub at_ms: u64,
    pub by: String,
}

/// What a task Log line is (roma idea 3; architect m_65317): an agent's
/// `sb report` (with its decisions), his answer to a question, a land, his
/// gesture, an agent joining. `Unknown`: a kind a newer hub knows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Report,
    Answer,
    Land,
    Gesture,
    Join,
    #[default]
    #[serde(other)]
    Unknown,
}

/// His gesture on a task, as a Log line says it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LogGesture {
    Pause,
    Resume,
    Done,
    Reopen,
    Rename,
    Merge,
    Why,
    Goal,
    Tick,
    Untick,
    Ongoing,
    /// he moved an artifact, a page or a card onto it
    MoveIn,
    /// he moved one off it
    MoveOut,
    #[default]
    #[serde(other)]
    Unknown,
}

/// One line of a task's Log, newest first in its list: facts only, the
/// window and `sb task show --log` put them in words. Built by the hub
/// when asked (switchboard's task_log.rs), from the journal's typed facts
/// and the land records, never kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LogLine {
    pub at_ms: u64,
    pub kind: LogKind,
    /// the agent, or `user` for his gestures and answers
    pub who: String,
    /// a report's summary; his answer's words or pick; a rename's new
    /// name, a goal's words, a merge's target, a moved thing's title; a
    /// land's commit range
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// a report's kind (`progress`, `done`, `blocked`, `failed`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub report: String,
    /// a report's `--decision`s
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gesture: Option<LogGesture>,
    /// a land's tip (full sha) and its commits' count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(default, skip_serializing_if = "super::is_zero_u64")]
    pub commits: u64,
    /// a land's target ref (`main`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// his answer's card
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<u64>,
}

/// One task with its status.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkRow {
    /// a slug of his name, stable on a rename
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<WorkGoal>,
    /// never done (a changelog, a watch)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ongoing: bool,
    /// his message that started it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<u64>,
    /// who made it (main, an agent, `user`)
    pub by: String,
    pub created_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_ms: Option<u64>,
    /// it resumes by itself then
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<WorkDone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<String>,
    /// he named it: an agent never renames it
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub named_by_you: bool,
    /// every agent a turn of it served, in order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// its newest fact
    pub last_ms: u64,
    pub status: WorkStatus,
    /// what the status is about: `agent` (working), `reply` (waits for
    /// `who`'s reply), `timer`, `watch` (wakes at `at_ms`), `until`
    /// (paused until `at_ms`), `crash` (`who` stopped at `at_ms`), `quiet`
    /// (nothing moves since `at_ms`); "" none
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub about: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub who: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_ms: Option<u64>,
    /// his open cards and page waits on it
    #[serde(default, skip_serializing_if = "super::is_zero_u64")]
    pub needs: u64,
    /// its open cards
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<u64>,
    /// to review: the open PR of its agents' place, its number (the hub
    /// joins it from the places at its snapshot; tasks-ux: Waiting is
    /// never blank); none when no place has one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// needs you: the page of a card of it (its title, else its id); none
    /// when no card of it is on a page
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

impl WorkRow {
    /// Its `<status> · <what>` line, never blank (words::tasks): `past`
    /// formats a past time (`14:20`, `yesterday 14:20`), `next` a time to
    /// come (`tomorrow 07:30`). The one pick rule: the window's TS has its
    /// copy, both pinned by fixtures/work_what.jsonl.
    pub fn what(&self, now: u64, past: impl Fn(u64) -> String, next: impl Fn(u64) -> String) -> String {
        use crate::words::tasks;
        let past = || self.at_ms.map(&past).unwrap_or_default();
        let next = || self.at_ms.map(&next).unwrap_or_default();
        let who = self.who.as_str();
        match (self.status, self.about.as_str()) {
            (WorkStatus::NeedsYou, _) if self.page.is_some() => tasks::needs_page(self.page.as_deref().unwrap_or_default()),
            (WorkStatus::NeedsYou, _) if self.needs > 0 => tasks::needs_questions(self.needs),
            (WorkStatus::NeedsYou, _) => tasks::status_needs_you.into(),
            (WorkStatus::Working, _) if !who.is_empty() => tasks::working_who(who),
            (WorkStatus::Working, _) => tasks::status_working.into(),
            (WorkStatus::Waiting, "reply") => tasks::waiting_reply(who),
            (WorkStatus::Waiting, "timer") => tasks::waiting_timer(next()),
            (WorkStatus::Waiting, "watch") => tasks::waiting_watch(who),
            (WorkStatus::Waiting, _) if self.agents.is_empty() => tasks::waiting_no_agent.into(),
            (WorkStatus::Waiting, _) if self.at_ms.is_some() => tasks::waiting_quiet(past()),
            (WorkStatus::Waiting, _) => tasks::status_waiting.into(),
            (WorkStatus::Stalled, "crash") => tasks::stalled_crash(who, past()),
            (WorkStatus::Stalled, _) if self.at_ms.is_some() => tasks::stalled_quiet(past()),
            (WorkStatus::Stalled, _) => tasks::status_stalled.into(),
            (WorkStatus::ToReview, _) if self.pr.is_some() => tasks::review_pr(self.pr.unwrap_or_default()),
            (WorkStatus::ToReview, _) => tasks::status_to_review.into(),
            (WorkStatus::Paused, "until") if self.at_ms.is_some() => tasks::paused_until(next()),
            (WorkStatus::Paused, _) => tasks::paused_by_you.into(),
            (WorkStatus::Done, _) => match &self.done {
                Some(d) => tasks::done_when(days_ago(d.at_ms, now)),
                None => tasks::status_done.into(),
            },
            (WorkStatus::Ongoing, _) => tasks::status_ongoing.into(),
            (WorkStatus::Unknown, _) => tasks::status_unknown.into(),
        }
    }
}

/// `today`, `yesterday`, `3 days ago`: whole days since `at`.
fn days_ago(at: u64, now: u64) -> String {
    use crate::words::tasks;
    match now.saturating_sub(at) / (24 * 3_600_000) {
        0 => tasks::today.into(),
        1 => tasks::yesterday.into(),
        n => tasks::days_ago(n),
    }
}

/// A task done more than 7 days ago, folded: its id stays taken and its
/// links resolve (a merged one's to the task it went into).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkTomb {
    pub id: String,
    pub name: String,
    pub done_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// architect m_64809: the pick rule, pinned by the fixture the
    /// window's vitest reads too (times as `<past>`/`<next>`: the format
    /// is each side's own fn, the pick is shared).
    #[test]
    fn each_rows_line_is_the_fixtures() {
        let text = include_str!("../../fixtures/work_what.jsonl");
        let mut n = 0;
        for l in text.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            let row: WorkRow = serde_json::from_value(v["row"].clone()).unwrap_or_else(|e| panic!("{l}: {e}"));
            let got = row.what(v["now"].as_u64().unwrap(), |_| "<past>".into(), |_| "<next>".into());
            assert_eq!(got, v["want"].as_str().unwrap(), "{l}");
            n += 1;
        }
        assert!(n >= 20, "{n}");
    }

    #[test]
    fn an_unknown_status_reads_unknown_and_a_known_one_its_word() {
        assert_eq!(WorkStatus::of_word("to_review"), WorkStatus::ToReview);
        assert_eq!(WorkStatus::of_word("a_newer_one"), WorkStatus::Unknown);
    }
}
