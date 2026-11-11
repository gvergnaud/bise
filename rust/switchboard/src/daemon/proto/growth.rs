//! The project's growth (les bims' growing office, architect m_28018):
//! `hub/growth` at initialize and when a counter moves, and view.json's
//! `growth`, both from one cached value. Its inputs are kept as they
//! come: the agents' growth rows at each `state` push, the artifact count
//! at each `artifacts` push (and at view.json's write). The value is
//! computed again (`bise_proto::rows::growth`, the per-row local offset)
//! only when `growth_key` moved, and sent only when it changed:
//! [`Shell::growth_changed`] is the one emit point.

use super::*;
use crate::snapshot::Snapshot;
use bise_proto::rows::{growth, growth_key, Growth, GrowthKey, GrowthRow};

/// The inputs, the key they had at the last computation, its value, and
/// the value last sent to the typed connections.
#[derive(Default)]
pub(in crate::daemon) struct GrowthCache {
    rows: Vec<GrowthRow>,
    arts: Option<u32>,
    key: Option<GrowthKey>,
    value: Growth,
    sent: Option<Growth>,
}

impl GrowthCache {
    /// The value for the inputs now (computed again only when the key moved).
    fn value(&mut self) -> Growth {
        let arts = self.arts.unwrap_or(0);
        let key = growth_key(&self.rows, arts);
        if self.key != Some(key) {
            self.value = growth(&self.rows, arts, &bise_home::clock::offset_at);
            self.key = Some(key);
        }
        self.value
    }
}

impl Shell {
    /// The growth of `snap`'s agents and `arts` artifacts (each kept for
    /// the next call when given; the artifact count read once from the
    /// store when never given yet).
    pub(in crate::daemon) fn growth_of(&mut self, snap: Option<&Snapshot>, arts: Option<u32>) -> Growth {
        if let Some(snap) = snap {
            self.proto.growth.rows = proto_view::growth_rows(snap);
        }
        if arts.is_some() {
            self.proto.growth.arts = arts;
        }
        if self.proto.growth.arts.is_none() {
            // none before the artifact rows' first build (daemon/art_rows.rs)
            self.proto.growth.arts = self.artifacts_ev().map(|ev| self.artifact_rows(&ev).len() as u32);
        }
        self.proto.growth.value()
    }

    /// `hub/growth` now (initialize's state). Not marked sent: a value
    /// the other connections haven't had yet still goes to all of them.
    pub(in crate::daemon) fn growth_ev(&mut self, snap: &Snapshot) -> HubEv {
        HubEv::Growth { project: self.project(), growth: self.growth_of(Some(snap), None) }
    }

    /// The one emit point: after the agents push (`snap`) or the
    /// artifacts push (`arts`), `hub/growth` to every typed connection
    /// when the value changed.
    pub(in crate::daemon) fn growth_changed(&mut self, ids: &[ClientId], snap: Option<&Snapshot>, arts: Option<u32>) {
        let g = self.growth_of(snap, arts);
        if self.proto.growth.sent == Some(g) {
            return;
        }
        self.proto.growth.sent = Some(g);
        let ev = HubEv::Growth { project: self.project(), growth: g };
        self.proto_note(ids, &ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_again_only_when_the_key_moves() {
        let mut c = GrowthCache { rows: vec![GrowthRow { done: true, created_ms: Some(1_790_899_200_000) }], arts: Some(2), ..Default::default() };
        let g = c.value();
        assert_eq!((g.jobs_done, g.days_used, g.artifacts_made), (1, 1, 2));
        // a stale value under the same key stays: proof nothing recomputed
        c.value.jobs_done = 99;
        assert_eq!(c.value().jobs_done, 99);
        c.arts = Some(3);
        assert_eq!((c.value().jobs_done, c.value().artifacts_made), (1, 3));
    }
}
