//! les bims (architect m_27581): the hub's side of an agent's `doing`
//! and of `hub/activity`. Each REPL line moves the agent's entry in
//! `Hub.doing`, the fact's one owner (runtime only, like `activity`;
//! `crate::doing::next`, pure); `Hub::snapshot` hands it to the row
//! (snapshot_rows.rs, `RowIn.doing`), nothing else keeps a copy. A
//! change marks the state dirty, so the row goes with the next state
//! push (the state gate, 100 ms). sb-core's
//! journal effects become activity events (`crate::activity`, pure).
//! Views only: no decision, no journal line.

use super::{Effect, Fx, Hub};
use crate::wire::Wire;
use serde_json::Value;

impl Hub {
    /// `line` from `agent`'s REPL at `now`: its `doing` moves.
    pub(super) fn doing_line(&mut self, agent: &str, line: &Wire, now: u64) {
        if !self.st.agents.contains_key(agent) {
            return;
        }
        let cur = self.doing.get(agent).cloned();
        let next = crate::doing::next(cur.clone(), line, now);
        if next == cur {
            return;
        }
        // `Hub.doing` is its one home (the row reads it: Hub::snapshot)
        match next {
            Some(d) => self.doing.insert(agent.to_string(), d),
            None => self.doing.remove(agent),
        };
        self.dirty = true;
    }

    /// sb-core journaled `ev` (live, never a replay): its activity
    /// event, if it is a message sent or a report.
    pub(super) fn activity_of_journal(fx: &mut Fx, ev: &Value) {
        if let Some(a) = crate::activity::of_journal(ev) {
            fx.push(Effect::Activity(a));
        }
    }
}
