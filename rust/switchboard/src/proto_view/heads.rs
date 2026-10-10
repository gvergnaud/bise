//! Each agent's newest entry (client-protocol step 4, P4b, architect
//! m_13999): `hub/agents`' `last_pos`, so a client lights an agent out of
//! view when it moves, without subscribing its thread. Pure.
//!
//! Per agent: the lines from its newest entry's start on. A live line
//! folds that tail with `bise_proto::thread::fold` (the one fold; an
//! entry's pos is its first line's) and keeps the newest entry's lines
//! again: bounded by one entry. A subscribed thread's live fold already
//! knows its newest pos ([`super::Live::last_pos`]): it is given, never
//! folded twice. The pos moves on a new entry only, never on a line
//! inside one (a tool's result) nor on one the fold hides (usage, obs).
//! An agent's ended turns are not counted here: the row's `turns` is
//! sb-core's (set_rt), from the same view as its status (issue 22).

use bise_proto::thread::{self, Ctx, Line};
use bise_proto::Pos;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Heads {
    by: BTreeMap<String, Head>,
}

#[derive(Default)]
struct Head {
    lines: Vec<Line>,
    last: Option<Pos>,
}

impl Heads {
    /// The hub's lines of `agent` it had before any live line (its
    /// buffered tail at start), folded once.
    pub fn seed(&mut self, agent: &str, lines: Vec<Line>, ctx: &Ctx) {
        if self.by.contains_key(agent) {
            return;
        }
        let mut h = Head { lines, last: None };
        h.fold(ctx);
        self.by.insert(agent.to_string(), h);
    }

    /// Whether `agent` has been seeded (or had a live line).
    pub fn has(&self, agent: &str) -> bool {
        self.by.contains_key(agent)
    }

    /// One live line of `agent`; `known`: its newest entry's pos from a
    /// subscription's live fold (then not folded here). True when its
    /// newest entry moved.
    pub fn push(&mut self, agent: &str, line: Line, known: Option<Pos>, ctx: &Ctx) -> bool {
        let h = self.by.entry(agent.to_string()).or_default();
        if h.lines.last().is_some_and(|l| l.0 >= line.0) {
            return false;
        }
        let before = h.last;
        h.lines.push(line);
        match known {
            Some(p) => h.keep(p),
            None => h.fold(ctx),
        }
        h.last != before
    }

    /// `agent`'s newest entry's pos (none: no entry seen yet).
    pub fn last(&self, agent: &str) -> Option<Pos> {
        self.by.get(agent).and_then(|h| h.last)
    }

    /// An agent renamed: its head follows it.
    pub fn rename(&mut self, old: &str, new: &str) {
        if let Some(h) = self.by.remove(old) {
            self.by.insert(new.to_string(), h);
        }
    }
}

impl Head {
    fn fold(&mut self, ctx: &Ctx) {
        if let Some(p) = thread::fold(&self.lines, ctx).last().map(|e| e.pos) {
            self.keep(p);
        }
    }

    /// `p` is the newest entry: its lines on are kept.
    fn keep(&mut self, p: Pos) {
        self.last = Some(self.last.map_or(p, |l| l.max(p)));
        self.lines.retain(|l| l.0 >= p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_moves_on_a_new_entry_only_and_its_tail_stays_one_entry() {
        let page = |_: &str| -> Option<thread::PageRef> { None };
        let c = Ctx { open_cards: &[], page: &page, provider: &|_: &str, k: &str| k.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &crate::attached::split };
        let mut h = Heads::default();
        let you = |p: Pos, t: &str| (p, p * 10, format!("sb you : {t}"));
        assert!(h.push("perf", you(1, "first"), None, &c), "a first entry");
        assert_eq!(h.last("perf"), Some(1));
        assert!(!h.push("perf", you(1, "first"), None, &c), "a line seen: nothing");
        // a line inside the entry (a hidden obs): the head stays
        assert!(!h.push("perf", (2, 20, "  obs: turn_started".into()), None, &c));
        assert!(h.push("perf", you(3, "second"), None, &c), "a new entry");
        assert_eq!(h.last("perf"), Some(3));
        assert_eq!(h.by["perf"].lines.len(), 1, "the newest entry's lines only");
        // a subscription's fold knows it: no fold here
        assert!(h.push("perf", you(4, "third"), Some(4), &c));
        assert_eq!(h.last("perf"), Some(4));
        h.rename("perf", "perf2");
        assert_eq!((h.last("perf"), h.last("perf2")), (None, Some(4)));
        // seeded from a buffered tail once
        h.seed("docs", vec![you(5, "a"), you(6, "b")], &c);
        h.seed("docs", vec![], &c);
        assert_eq!(h.last("docs"), Some(6));
    }
}
