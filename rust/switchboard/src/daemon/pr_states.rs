//! His pick on page tasks-fresh (architect m_66713 point 3): each task's
//! linked PRs, open and merged, to sb-core's `pr_states` as the whole set,
//! at boot before any client and when the artifact store changes (an add,
//! a Move, a PR's state kept by the forge sync). The count is
//! `forge::task_prs` (pure); this file reads the store, merged and
//! archived PRs included, once per store generation.

use super::*;

impl Shell {
    pub(super) fn pr_states_send(&mut self) {
        let gen = crate::store_gen::disk_now();
        // the whole set on each store generation, never a copy of what was
        // sent: sb-core narrows to the works whose merged flipped (architect m_67577)
        if self.art.pr_states_gen == Some(gen) {
            return;
        }
        self.art.pr_states_gen = Some(gen);
        let metas: Vec<_> = self.art_store().stored().into_iter().filter(|m| m.kind == "pr").collect();
        let c = crate::forge::task_prs::counts(metas.iter().filter_map(|m| Some((m.works.works(), m.pr_last?.state))));
        self.step(Input::PrStates { works: c.works, open: c.open, merged: c.merged });
    }
}
