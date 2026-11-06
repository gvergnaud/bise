//! The agents' contexts: the `<bise_state>` block appended to each model
//! request of an active agent (main's task board, a task's roster and
//! questions; board.rs builds them), pushed as `Effect::Context` when
//! its text changes (the REPL re-reads `BEND_CONTEXT_FILE` before every
//! model call).
//!
//! The file is current before each input reaches a REPL, and within
//! [`CONTEXT_MS`] otherwise (send-fast, architect m_35501): an input marks
//! the contexts pending ([`Contexts::pend`]); they are built when a step
//! delivers to a REPL (their effects go before the delivery,
//! core_contexts.rs), before any line the shell writes to a REPL, a spawn
//! or a reload (daemon/contexts.rs), and else by the loop's gate at most
//! every [`CONTEXT_MS`]. Built at each input, they were ~25% of the hub's
//! loop with 30 agents (60% at 5 lines/s), so his sends waited behind them.
//!
//! Every active agent's context is built fresh; only the texts that
//! changed are pushed. Nothing else is kept: what made a fresh
//! build slow (main's `<agent_threads>` and everyone's questions walked
//! every message) is now O(hot) or kept by State's writer (issue 21,
//! hot-view sha 2a-2c), and on his journal copy (529 agents, 34 active,
//! 19,474 messages) a fresh build of every context is 5.34 ms (median of
//! 30 tool lines) against 5.56 ms for the per-task cache that was here
//! (sha 2d, rule 7).

use crate::board::{self, Group, Models, Waiting};
use crate::model::{Lifecycle, State, MAIN};
use std::collections::BTreeMap;

/// The longest a context waits for its build when no input reaches a
/// REPL (the loop's gate, daemon/contexts.rs).
pub const CONTEXT_MS: u64 = 200;

/// How many threads main's `<agent_threads>` shows.
pub const THREADS: usize = 8;

#[derive(Debug, Default)]
pub struct Contexts {
    /// What each agent's REPL has: the last text pushed.
    texts: BTreeMap<String, String>,
    /// An input came since the last build.
    pending: bool,
}

impl Contexts {
    /// An input came: the contexts are built at the next flush.
    pub fn pend(&mut self) {
        self.pending = true;
    }

    pub fn pending(&self) -> bool {
        self.pending
    }

    /// The pending build, now: the changed texts ([`Contexts::refresh`]);
    /// nothing when none is pending.
    pub fn flush(&mut self, st: &State, now: u64, models: &Models) -> Vec<(String, String)> {
        if !std::mem::take(&mut self.pending) {
            return Vec::new();
        }
        self.refresh(st, now, models)
    }

    /// The contexts this state changes, (agent, its new text) each, in
    /// the agents' name order: every context built fresh, only the texts
    /// that differ from what the REPLs have are pushed (and kept).
    pub fn refresh(&mut self, st: &State, now: u64, models: &Models) -> Vec<(String, String)> {
        let mut changed = Vec::new();
        for (name, text) in fresh(st, now, models) {
            if self.texts.get(&name) != Some(&text) {
                self.texts.insert(name.clone(), text.clone());
                changed.push((name, text));
            }
        }
        changed
    }

    /// The text `name`'s REPL has (None: none pushed yet).
    pub fn text(&self, name: &str) -> Option<&str> {
        self.texts.get(name).map(String::as_str)
    }
}

/// Every active agent's context built from nothing, by name.
pub fn fresh(st: &State, now: u64, models: &Models) -> BTreeMap<String, String> {
    let waiting = Waiting::of(st);
    let group = Group::of(st, now, models);
    let threads = board::agent_threads(st, THREADS);
    st.agents
        .values()
        .filter(|a| a.lifecycle == Lifecycle::Active)
        .map(|a| {
            let questions = board::questions_for(waiting.unanswered(&a.name));
            let text = if a.name == MAIN {
                board::main_context(st, now, &threads, &questions)
            } else {
                board::task_context(&a.name, &group, &questions)
            };
            (a.name.clone(), text)
        })
        .collect()
}

#[cfg(test)]
#[path = "contexts_tests.rs"]
mod tests;
