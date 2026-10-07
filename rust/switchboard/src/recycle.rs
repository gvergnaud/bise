//! Recycling an aged REPL (repl-cpu-3): when an agent's REPL has read
//! enough context since it started, the hub restarts it at its next idle
//! on the same session and port (the switch path: `reload`, respawn,
//! `restored`), so its next model calls cost what a fresh process pays.
//!
//! Why: a WORKAROUND for the Bend runtime's allocator. Its LIFO
//! per-class free chains scatter the cons cells of the big strings each
//! model call builds and drops, and every later walk of them gets
//! slower: on a 340k-token session the CPU of one call went from 0.55 s
//! (call 1) to 3.4 s (call 60), and a 20-line Bend program with none of
//! bise's code shows the same curve (reported to the Bend team, main
//! m_9899). A restart resets it. Remove this module once the runtime
//! keeps its heap compact.
//!
//! The aging follows the bytes a process churned, so the budget counts
//! the context each call read (`in=` of the usage line, cached tokens
//! included: bise_session::usage_line), not the calls: a small-context
//! agent recycles rarely. Pure: the hub (daemon/recycling.rs) feeds it
//! its REPLs' lines and asks it which idle REPL is due.
//!
//! The budget: a call's extra CPU grows like a·n (n calls since the
//! start, a ~ c², c the context: 0.048 s a call at 340k tokens) and a
//! restart costs R (~2.4 cpu-s: 1.2 to load a 1.37 MB session, 1.1 for
//! the plugin servers the new REPL starts, 0.1 for the connectors
//! bootstrap), so the cheapest restart period is n = sqrt(2R/a) calls,
//! i.e. n·c = c·sqrt(2R/a) ~ 3.4M tokens whatever c is.
use std::collections::{BTreeMap, BTreeSet};

/// The default budget, in context tokens read since the REPL started:
/// ~9 calls of a 340k-token session, ~150 of a 20k one.
pub const DEFAULT_BUDGET: u64 = 3_000_000;

/// The budget of `BISE_RECYCLE_TOKENS`: unset or not a number = the
/// default, 0 = never recycle.
pub fn budget(setting: Option<&str>) -> u64 {
    setting.and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(DEFAULT_BUDGET)
}

/// What each live REPL has read since it started, and the ones due.
#[derive(Debug, Default)]
pub struct Churn {
    read: BTreeMap<String, u64>,
    due: BTreeSet<String>,
}

impl Churn {
    /// A new process for `dir` (spawned, adopted, or switched): it starts
    /// from nothing.
    pub fn started(&mut self, dir: &str) {
        self.read.remove(dir);
        self.due.remove(dir);
    }

    /// A wire line of `dir`'s live REPL: a usage line adds its context.
    pub fn line(&mut self, dir: &str, line: &str) {
        if let Some(u) = bise_session::usage_line::of_line(line) {
            let n = self.read.entry(dir.to_string()).or_default();
            *n = n.saturating_add(u.input);
        }
    }

    /// `dir`'s REPL is idle: true when this makes it due (the first time
    /// only, so the hub says it once per recycle).
    pub fn idle(&mut self, dir: &str, budget: u64) -> bool {
        let over = budget > 0 && self.read.get(dir).copied().unwrap_or(0) >= budget;
        over && self.due.insert(dir.to_string())
    }

    /// Whether `dir`'s REPL waits for a recycle.
    pub fn is_due(&self, dir: &str) -> bool {
        self.due.contains(dir)
    }

    /// The recycle was asked (the REPL got `reload`): not due anymore;
    /// its new process starts from nothing ([`Churn::started`]).
    pub fn asked(&mut self, dir: &str) {
        self.due.remove(dir);
    }

    /// The context `dir`'s REPL read since it started.
    pub fn read(&self, dir: &str) -> u64 {
        self.read.get(dir).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(n: u64) -> String {
        format!("  obs: usage: model=fake/claude-x in={n} out=10 cache_read={n} cache_write=0")
    }

    #[test]
    fn the_budget_comes_from_the_setting() {
        assert_eq!(budget(None), DEFAULT_BUDGET);
        assert_eq!(budget(Some("")), DEFAULT_BUDGET);
        assert_eq!(budget(Some("abc")), DEFAULT_BUDGET);
        assert_eq!(budget(Some(" 5000 ")), 5000);
        assert_eq!(budget(Some("0")), 0);
    }

    #[test]
    fn a_repl_is_due_once_it_read_the_budget() {
        let mut c = Churn::default();
        c.line("a", &usage(400));
        c.line("a", "  obs: turn_done: completed");
        c.line("a", "you : obs: usage: model=m in=99999");
        assert_eq!(c.read("a"), 400, "in= only, of usage lines only (cache_read is inside in=)");
        assert!(!c.idle("a", 1000));
        c.line("a", &usage(600));
        assert!(c.idle("a", 1000));
        assert!(c.is_due("a"));
        // said once: a second idle while due is not news
        assert!(!c.idle("a", 1000));
        assert!(c.is_due("a"));
        // another REPL is counted apart
        assert!(!c.is_due("b"));
        assert!(!c.idle("b", 1000));
    }

    #[test]
    fn zero_is_off() {
        let mut c = Churn::default();
        c.line("a", &usage(u64::MAX));
        c.line("a", &usage(5));
        assert_eq!(c.read("a"), u64::MAX, "saturates");
        assert!(!c.idle("a", 0));
        assert!(!c.is_due("a"));
    }

    #[test]
    fn a_new_process_starts_from_nothing() {
        let mut c = Churn::default();
        c.line("a", &usage(2000));
        assert!(c.idle("a", 1000));
        c.asked("a");
        assert!(!c.is_due("a"));
        // the old process's lines until it exits still count, then the
        // new one starts at zero
        c.started("a");
        assert_eq!(c.read("a"), 0);
        assert!(!c.idle("a", 1000));
        // a due REPL that is replaced another way (a crash, a switch) is
        // not due anymore
        c.line("a", &usage(2000));
        assert!(c.idle("a", 1000));
        c.started("a");
        assert!(!c.is_due("a"));
    }
}
