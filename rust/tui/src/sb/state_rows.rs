//! The hub's typed state rows as the terminal's own (client-protocol
//! step 4, P4c-4b): hub/agents' agents and places, hub/cards' cards and
//! others, hub/scheduled's live and ended tasks, hub/flow's flow, each
//! mapped onto what the older `state` reader made of the snapshot, so the
//! panel, the inbox, the scheduled screen and `/flow` read the same
//! values (P4c-4a's law, switchboard state_rows_tests.rs, holds the rows
//! to the older keys; the tests here hold these maps to the words the
//! terminal draws). Pure: the readers (hub_reads.rs) apply them.

use super::cards::Card;
use super::places::{Place, Pr};
use super::Agent;
use bise_proto::rows::{self, AgentMode, FlowMode, Phase, PrChecks, PrReview, PrState, ScheduledTask, Status, WaitingOn};
use bise_proto::thread::scheduled::Task;

/// The hub's status word (`model::Status::as_str`) from a typed row: the
/// one inverse of `Status::of_hub` + `Phase::of_hub` (the law below:
/// every hub word round-trips).
pub(super) fn status_word(status: Status, phase: Option<Phase>, archived: bool) -> &'static str {
    match (archived, phase, status) {
        (true, ..) => "archived",
        (_, Some(Phase::Starting), _) => "starting",
        (_, Some(Phase::Stopped), _) => "stopped",
        (_, _, Status::Working) => "working",
        (_, _, Status::Waiting) => "waiting",
        (_, _, Status::Blocked) => "blocked",
        (_, _, Status::Failed) => "failed",
        (_, _, Status::Done) => "done",
        (_, _, Status::Idle) => "idle",
    }
}

/// Whom it waits on, the hub model's word (`you`, an agent's name, "").
fn waiting_word(w: Option<&WaitingOn>) -> String {
    match w {
        Some(WaitingOn::You) => "you".into(),
        Some(WaitingOn::Agent { name }) => name.clone(),
        Some(WaitingOn::Unknown) | None => String::new(),
    }
}

impl Agent {
    /// A hub/agents row. `branch`: the branch checked out where it works
    /// (its worktree's, else the shared checkout's), as the snapshot's.
    pub(super) fn of_row(a: &rows::Agent) -> Agent {
        Agent {
            name: a.name.clone(),
            main: a.main,
            status: status_word(a.status, a.phase, a.archived).into(),
            objective: a.objective.clone(),
            mode: match a.mode {
                Some(AgentMode::Shared) => "shared".into(),
                Some(AgentMode::Worktree) => "worktree".into(),
                Some(AgentMode::Unknown) | None => String::new(),
            },
            branch: a.branch.clone().or_else(|| a.checkout_branch.clone()),
            path: a.path.clone(),
            note: a.note.clone(),
            queued: u64::from(a.msgs_queued),
            inbox: u64::from(a.inbox),
            turn_ms: a.turn_ms,
            turn_seen: Some(std::time::Instant::now()),
            report: a.report.as_ref().map(|r| r.text.clone()).unwrap_or_default(),
            report_ms: a.report.as_ref().map(|r| r.at_ms).filter(|ms| *ms > 0),
            role: a.role.clone(),
            created_ms: a.created_ms.unwrap_or(0),
            waiting_on: waiting_word(a.waiting_on.as_ref()),
            dir: if a.dir.is_empty() { a.name.clone() } else { a.dir.clone() },
            place: a.place.clone().unwrap_or_default(),
            place_id: a.place_id.clone(),
            model: a.model.clone().unwrap_or_default(),
            effort: a.effort.clone().unwrap_or_default(),
            efforts: a.efforts.clone(),
            changes: a.changes.map(|c| (c.files, c.add, c.del)),
            usage: a.usage.clone(),
        }
    }
}

/// The agents whose name changed since `old` (architect Q3: same `dir`,
/// new name), as (old name, new name).
pub(super) fn renamed(old: &[Agent], new: &[Agent]) -> Vec<(String, String)> {
    new.iter()
        .filter_map(|n| old.iter().find(|o| !o.dir.is_empty() && o.dir == n.dir && o.name != n.name).map(|o| (o.name.clone(), n.name.clone())))
        .collect()
}

/// A place's PR with the words places.rs reads (`draft`, `changes_requested`,
/// `pending`, `fail`...): the snapshot's `PrView` values.
fn pr_of_row(p: &rows::Pr) -> Pr {
    Pr {
        number: p.number,
        url: p.url.clone(),
        state: match p.state {
            PrState::Open => "open",
            PrState::Draft => "draft",
            PrState::Merged => "merged",
            PrState::Closed => "closed",
            PrState::Unknown => "",
        }
        .into(),
        review: match p.review {
            PrReview::Approved => "approved",
            PrReview::Changes => "changes_requested",
            PrReview::None if p.in_review => "pending",
            PrReview::None => "none",
            PrReview::Unknown => "",
        }
        .into(),
        checks: match p.checks {
            PrChecks::Pass => "pass",
            PrChecks::Fail => "fail",
            PrChecks::Running => "running",
            PrChecks::None => "none",
            PrChecks::Unknown => "",
        }
        .into(),
        failing: p.failing.clone(),
        stale_ms: p.stale_ms,
    }
}

impl Place {
    /// A hub/agents `places` row.
    pub(super) fn of_row(p: &rows::Place) -> Place {
        Place {
            id: p.id.clone(),
            branch: p.branch.clone(),
            agents: p.agents.clone(),
            pr: p.pr.as_ref().map(pr_of_row),
            lid: p.lid.clone().filter(|l| !l.is_empty()),
            feature: p.feature,
            trying: p.trying,
        }
    }
}

/// hub/cards' his cards and the others (bise's own: a drop, a done...)
/// in one list, by id: the snapshot's order (the hub's open cards are
/// kept by id). `asking`: the card whose drop asks once more (the
/// terminal's, kept across rows); `now`: the clock their age is from.
pub(super) fn cards_of_rows(his: &[rows::Card], others: &[rows::Card], asking: Option<u64>, now: u64) -> Vec<Card> {
    let mut all: Vec<&rows::Card> = his.iter().chain(others).collect();
    all.sort_by_key(|c| c.id);
    all.into_iter()
        .map(|c| Card {
            id: c.id,
            kind: c.kind.clone(),
            agent: c.agent.clone(),
            // an older hub's row has no text: its question
            text: if c.text.is_empty() { c.question.clone() } else { c.text.clone() },
            age_ms: now.saturating_sub(c.since_ms),
            seen_at: std::time::Instant::now(),
            note: c.note.clone().unwrap_or_default(),
            look: None,
            place: c.place.clone(),
            pr: c.pr,
            link: c.link.clone(),
            asking: asking == Some(c.id),
            waiting: c.waiting.clone().unwrap_or_default(),
        })
        .collect()
}

/// A hub/scheduled row as the scheduled screen's task (`stopped_by`:
/// `user` or the agent's name, as the hub's state wrote it).
pub(super) fn task_of_row(t: &ScheduledTask) -> Task {
    Task {
        id: t.id,
        agent: t.agent.clone(),
        by: t.by.clone(),
        label: t.label.clone(),
        text: t.words.clone(),
        name: t.name.clone(),
        next_ms: t.next_ms.unwrap_or(0),
        last_ms: t.last_ms.unwrap_or(0),
        fired: t.done,
        times: t.times,
        until_ms: t.until_ms,
        page: t.page.clone(),
        runs: t.runs.clone(),
        ended_ms: t.ended_ms,
        end: t.end.map(|e| e.word().to_string()).unwrap_or_default(),
        stopped_by: match &t.stopped_by {
            Some(WaitingOn::You) => "user".into(),
            Some(WaitingOn::Agent { name }) => name.clone(),
            Some(WaitingOn::Unknown) | None => String::new(),
        },
    }
}

/// hub/flow's mode as `/flow` and the header read it (`pr`, `trunk`; ""
/// when not set).
pub(super) fn flow_word(f: Option<FlowMode>) -> String {
    match f {
        Some(FlowMode::Pr) => "pr".into(),
        Some(FlowMode::Trunk) => "trunk".into(),
        Some(FlowMode::Unknown) | None => String::new(),
    }
}

#[cfg(test)]
#[path = "state_rows_tests.rs"]
mod tests;
