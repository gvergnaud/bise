//! ⌘K's index (his unified search, amb-web m_11850, architect m_11910):
//! the core holds hubs only for home + shown + subscribed, so for every
//! other registered project the window gets `index {projects}` with what
//! that project's `view.json` says (its agents, its newest artifacts and
//! their total, its live scheduled tasks), as of the view's `written_ms`, and whether its hub runs
//! (`up`). Read from files only: an index never starts or holds a hub.
//! A held project is never in it (its live events say the same, and the
//! hub writes the view with the same builders). Asked once (`index`, ⌘K
//! opened), then sent again on change, at most every [`EVERY`]; a project
//! that stops being held moves into it at the next send.

use super::*;
use bise_proto::draft::IndexRow;

/// At most one `index` per this long after the first.
pub const EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct Index {
    /// the window asked (⌘K): from then on, sent on change
    asked: bool,
    /// the last one sent and when
    last: String,
    at: Option<Instant>,
}

impl Core {
    /// `index`: the rows now, whatever was sent before.
    pub(super) fn index_cmd(&mut self, now: Instant) {
        self.index.asked = true;
        self.index.last.clear();
        self.index.at = None;
        self.tick_index(now);
    }

    /// At most every [`EVERY`] once asked: the rows, sent when they changed.
    pub(super) fn tick_index(&mut self, now: Instant) {
        if !self.index.asked || self.index.at.is_some_and(|t| now.duration_since(t) < EVERY) {
            return;
        }
        self.index.at = Some(now);
        let ev = json!({"ev": "index", "projects": self.index_rows()});
        let s = ev.to_string();
        if s != self.index.last {
            self.index.last = s;
            self.emit(ev);
        }
    }

    /// Every registered project not held now, in his order, from its
    /// `view.json` (none yet: empty lists, no `written_ms`).
    fn index_rows(&self) -> Vec<IndexRow> {
        let Some(p) = self.hubs.ports.as_ref() else { return Vec::new() };
        self.hubs
            .rows
            .iter()
            .filter(|r| !self.hubs.holds(&r.id))
            .map(|r| {
                let missing = !(p.facts.exists)(&r.path);
                let up = !missing && (p.facts.running)(&r.path);
                match (p.facts.view)(&r.id) {
                    Some(v) => IndexRow {
                        project: r.id.clone(),
                        up,
                        written_ms: Some(v.written_ms),
                        agents: v.agents,
                        artifacts: v.artifacts,
                        artifacts_total: v.artifacts_total,
                        scheduled: v.scheduled,
                    },
                    None => IndexRow { project: r.id.clone(), up, written_ms: None, agents: vec![], artifacts: vec![], artifacts_total: 0, scheduled: vec![] },
                }
            })
            .collect()
    }
}
