//! The snapshot's agent rows (typed [`AgentRow`]s, issue 30: the TUI's
//! and the desktop's roster, view.json), kept from one snapshot to the next: a row
//! is a pure function of its inputs ([`RowIn`]), so a row whose inputs
//! are equal to last time's is the kept one, cloned, never rebuilt.
//!
//! Why (issue 21 step 2, item 7): on his journal (514 agents, 493 of
//! them archived) a snapshot took 59-130 ms, 88-120 of them building the
//! archived rows, which never change; the state gate sends up to 10
//! snapshots a second, so under a flood the loop spent most of its time
//! there. Comparing a row's inputs costs a few memcmp; building it, its
//! objective's `one_line` twice and ~30 JSON fields.
//!
//! The rows stay on the wire exactly as before (architect m_15238 (1)):
//! every agent, archived ones included, the same fields. Like
//! contexts.rs, a stopgap until sb-core's typed hot view (issue 21 step 3,
//! target T3) says which agents a step changed.

use crate::board::Waiting;
use crate::model::{Agent, State};
use crate::util::{clip, one_line};
use bise_proto::rows::{AgentWatch, Doing};
use crate::snapshot::AgentRow;
use bise_proto::draft::Queued;
use std::collections::BTreeMap;

/// Everything a row reads: the agent, and what the hub knows of it
/// besides (its role line, its messages, its watches, its turn's age).
#[derive(Debug)]
pub struct RowIn<'a> {
    pub agent: &'a Agent,
    /// The model's role line (BISE-126; None: none yet).
    pub role: Option<&'a str>,
    /// Messages waiting for its delivery.
    pub queued: usize,
    /// The user's queued inputs ([`crate::board::Waiting::queued_inputs`]).
    pub queued_inputs: Vec<Queued>,
    /// main's inbox (0 for a task).
    pub inbox: usize,
    /// How long its turn has run (None: no turn).
    pub turn_ms: Option<u64>,
    pub watching: Vec<AgentWatch>,
    /// les bims: what it is doing now, from `Hub.doing` (its one owner,
    /// core_doing.rs); the row says it through `crate::doing::row`
    pub doing: Option<Doing>,
    /// its running processes and their lowest port, from `Hub.procs`
    /// (daemon/processes.rs)
    pub procs: (u16, Option<u16>),
}

/// A kept row and the inputs it was built from.
#[derive(Debug)]
struct Kept {
    agent: Agent,
    role: Option<String>,
    queued: usize,
    queued_inputs: Vec<Queued>,
    inbox: usize,
    turn_ms: Option<u64>,
    watching: Vec<AgentWatch>,
    doing: Option<Doing>,
    procs: (u16, Option<u16>),
    row: AgentRow,
}

impl Kept {
    fn same(&self, i: &RowIn) -> bool {
        self.queued == i.queued
            && self.inbox == i.inbox
            && self.turn_ms == i.turn_ms
            && self.role.as_deref() == i.role
            && self.queued_inputs == i.queued_inputs
            && self.watching == i.watching
            && self.doing == i.doing
            && self.procs == i.procs
            && self.agent == *i.agent
    }
}

/// What the rows read of the messages, by agent: queued count, the
/// user's queued inputs, the questions waiting for it. Built from one
/// [`Waiting`] at each snapshot: it walks the hot messages only (0.11 ms
/// on his journal copy, hot-view sha 2c), no longer worth keeping.
#[derive(Debug, Default)]
struct Mail {
    queued: BTreeMap<String, usize>,
    inputs: BTreeMap<String, Vec<Queued>>,
    unanswered: BTreeMap<String, usize>,
}

/// The kept rows, by agent name.
#[derive(Debug, Default)]
pub struct Rows {
    kept: BTreeMap<String, Kept>,
    mail: Mail,
    /// How many rows were built (not kept), for the tests and the bench.
    pub built: u64,
}

impl Rows {
    /// Bring the messages' part up to `st` (one pass over the hot ones).
    pub fn mail(&mut self, st: &State) {
        let w = Waiting::of(st);
        let mut m = Mail::default();
        for a in st.agents.keys() {
            let (q, i, u) = (w.queued_count(a), w.queued_inputs(a), w.unanswered(a).len());
            if q > 0 {
                m.queued.insert(a.clone(), q);
            }
            if !i.is_empty() {
                m.inputs.insert(a.clone(), i);
            }
            if u > 0 {
                m.unanswered.insert(a.clone(), u);
            }
        }
        self.mail = m;
    }

    /// `a`'s row inputs, its messages' part from the last [`Rows::mail`].
    pub fn input<'a>(&self, a: &'a Agent, role: Option<&'a str>, now: u64, watching: Vec<AgentWatch>, doing: Option<Doing>, procs: (u16, Option<u16>)) -> RowIn<'a> {
        RowIn {
            agent: a,
            role,
            queued: self.mail.queued.get(&a.name).copied().unwrap_or(0),
            queued_inputs: self.mail.inputs.get(&a.name).cloned().unwrap_or_default(),
            inbox: if a.is_main { self.mail.unanswered.get(&a.name).copied().unwrap_or(0) } else { 0 },
            turn_ms: a.turn_ms.map(|t| now.saturating_sub(t)),
            watching,
            doing,
            procs,
        }
    }

    /// `i`'s row: the kept one when its inputs are the same, else built
    /// (and kept).
    pub fn row(&mut self, i: RowIn) -> AgentRow {
        if let Some(k) = self.kept.get(&i.agent.name).filter(|k| k.same(&i)) {
            return k.row.clone();
        }
        let row = build(&i);
        self.built += 1;
        let k = Kept {
            agent: i.agent.clone(),
            role: i.role.map(str::to_string),
            queued: i.queued,
            queued_inputs: i.queued_inputs,
            inbox: i.inbox,
            turn_ms: i.turn_ms,
            watching: i.watching,
            doing: i.doing,
            procs: i.procs,
            row: row.clone(),
        };
        self.kept.insert(k.agent.name.clone(), k);
        row
    }

    /// Forget the rows of the agents no longer in `names` (a rename).
    pub fn keep_only<'a>(&mut self, names: impl Iterator<Item = &'a String>) {
        let names: std::collections::BTreeSet<&String> = names.collect();
        self.kept.retain(|n, _| names.contains(n));
    }
}

/// The role line the views show (BISE-126): the model's, else the
/// objective's first sentence; none for main.
pub fn role_of(a: &Agent, line: Option<&str>) -> String {
    if a.is_main {
        return String::new();
    }
    match line {
        Some(l) => l.to_string(),
        None => crate::role::default_line(&a.brief.objective),
    }
}

/// A row from its inputs.
pub fn build(i: &RowIn) -> AgentRow {
    let a = i.agent;
    let report = a.last_report.as_ref();
    AgentRow {
        name: a.name.clone(),
        main: a.is_main,
        status: a.status(),
        objective: a.description(),
        role: role_of(a, i.role),
        parent: a.parent.clone(),
        mode: a.ws.mode.clone(),
        path: a.ws.path.clone(),
        branch: a.ws.branch.clone(),
        place: a.place.clone(),
        place_id: crate::place::view_id(a),
        created_ms: a.created_ms,
        dir: a.dir.clone(),
        aliases: a.aliases.clone(),
        note: a.declared().map(|d| d.note.clone()).unwrap_or_default(),
        declared: a.declared().map(|d| d.status),
        report: report.map(|r| clip(&one_line(&r.summary), 200)),
        report_ms: report.map(|r| r.at_ms),
        follow: a.follow,
        report_kind: report.map(|r| r.kind.clone()),
        step: report.and_then(|r| r.step),
        of: report.and_then(|r| r.of),
        queued: i.queued,
        queued_inputs: i.queued_inputs.clone(),
        inbox: i.inbox,
        turn_ms: i.turn_ms,
        turns: a.turns,
        waiting_on: a.waiting_on.clone().filter(|_| a.waiting),
        watching: i.watching.clone(),
        doing: crate::doing::row(a.run == crate::model::Run::Busy, i.doing.clone(), &i.watching),
        procs: i.procs,
        model: None,
    }
}

#[cfg(test)]
#[path = "snapshot_rows_tests.rs"]
mod tests;
