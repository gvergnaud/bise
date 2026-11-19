//! His pick on page tasks-fresh (architect m_66713 point 3): each task's
//! linked PRs, open and merged, to sb-core's `pr_states` as the whole set,
//! at boot before any client and when the artifact store changes (an add,
//! a Move, a PR's state kept by the forge sync). The count is
//! `forge::task_prs` (pure); this file reads the store, merged and
//! archived PRs included, and sends only when the store moved.

use super::*;

impl Shell {
    pub(super) fn pr_states_send(&mut self) {
        let gen = crate::store_gen::disk_now();
        if self.art.pr_states.as_ref().is_some_and(|(g, _)| *g == gen) {
            return;
        }
        let metas: Vec<_> = self.art_store().stored().into_iter().filter(|m| m.kind == "pr").collect();
        let c = crate::forge::task_prs::counts(metas.iter().filter_map(|m| Some((m.works.works(), m.pr_last?.state))));
        let same = self.art.pr_states.as_ref().is_some_and(|(_, sent)| *sent == c);
        self.art.pr_states = Some((gen, c.clone()));
        if !same {
            self.step(Input::PrStates { works: c.works, open: c.open, merged: c.merged });
        }
    }
}
