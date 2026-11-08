//! The hub's client snapshot (issue 30): `Hub::snapshot` builds the typed
//! [`crate::snapshot::Snapshot`] the views read by field: its agent rows
//! (kept from one snapshot to the next, snapshot_rows.rs), its open cards
//! with the notes the hub adds, its places with their lids and PRs, the
//! repo's flow. Moved out of core.rs (the 1,000-line rule).

use super::*;
use crate::snapshot::{AgentRow, CardRow, Snapshot};

impl Hub {
    /// The held lines (the land queue's first, else `no PR yet`) and the
    /// stale PRs' ages, by place id, for the views.
    fn pr_views(&self, now: u64) -> (BTreeMap<String, String>, BTreeMap<String, u64>) {
        let mut lids = crate::land_queue::lids(&self.landing);
        // trunk flow: a branch lands on main, it never waits for a PR
        let no_pr = if self.flow == Some(crate::flow::FlowMode::Trunk) { &BTreeMap::new() } else { &self.pr_lids };
        for (id, l) in no_pr {
            // a detached private worktree (BISE-136) has no branch to ask
            // the forge about: its count shows without an answer
            let asked = self.pr_ok_ms.contains_key(id) || self.detached_private(id);
            if !self.prs.contains_key(id) && asked {
                lids.entry(id.clone()).or_insert_with(|| l.clone());
            }
        }
        let stale = self
            .prs
            .keys()
            .filter_map(|id| {
                let age = now.saturating_sub(*self.pr_ok_ms.get(id)?);
                (self.pr_late.is_some() || age > PR_STALE_MS).then(|| (id.clone(), age))
            })
            .collect();
        (lids, stale)
    }

    /// pr-integration rule 1: every PR the hub follows as its typed row
    /// (`forge::news::pr_row`, the place's agents, its age when the
    /// forge is late), by its link's one form (`forge::pr_link`): the
    /// artifact rows' status (artifacts::join_pr_status).
    pub fn pr_rows_by_url(&self, now: u64) -> BTreeMap<String, bise_proto::rows::Pr> {
        let (_, stale) = self.pr_views(now);
        // the followed PRs (an agent only reviews them): no place, no agents
        let age = now.saturating_sub(self.followed_ok_ms);
        let late = (self.pr_late.is_some() || age > PR_STALE_MS).then_some(age);
        let mut out: BTreeMap<String, bise_proto::rows::Pr> = self
            .followed
            .iter()
            .map(|(url, pr)| (url.clone(), crate::forge::news::pr_row(&crate::place::PrView::followed(pr, late), &[])))
            .collect();
        // a place's PR wins over the same one followed
        for p in crate::place::places(&self.st, &self.prs) {
            let Some(pr) = p.pr.as_ref() else { continue };
            let Some(link) = crate::forge::pr_link(&pr.url) else { continue };
            let view = crate::place::PrView::of(pr, stale.get(&p.id).copied());
            out.insert(link.url, crate::forge::news::pr_row(&view, &p.agents));
        }
        out
    }

    /// The client snapshot (agents, cards) for the views.
    /// Its agent rows are kept from one snapshot to the next and rebuilt
    /// only when their inputs changed (snapshot_rows.rs).
    pub fn snapshot(&mut self, now: u64) -> Snapshot {
        self.rows.mail(&self.st);
        let mut agents: Vec<AgentRow> = Vec::with_capacity(self.st.order.len());
        for a in self.st.order.iter().filter_map(|n| self.st.agents.get(n)) {
            let role = self.roles.get(&a.dir).and_then(|r| r.line.as_deref());
            let i = self.rows.input(a, role, now, self.st.wakes.watching(&a.name), self.doing.get(&a.name).cloned(), self.procs.get(&a.name).copied().unwrap_or_default());
            agents.push(self.rows.row(i));
        }
        self.rows.keep_only(self.st.agents.keys());
        self.snapshot_with(agents, now)
    }

    /// The snapshot around its agent rows: the cards, the places, the flow.
    fn snapshot_with(&self, agents: Vec<AgentRow>, now: u64) -> Snapshot {
        let running = self.running_cards();
        let cards = self.st.open_cards().map(|c| self.card_row(c, now, &running)).collect();
        let places = crate::place::places(&self.st, &self.prs);
        let (mut lids, stale) = self.pr_views(now);
        // a feature's lid: the land queue's (`landing`) wins
        for (id, l) in &self.feature_lids {
            lids.entry(id.clone()).or_insert_with(|| l.clone());
        }
        let places = crate::place::views(&places, &lids, &self.landing, &stale, &self.trying);
        Snapshot { agents, cards, places, flow: self.flow, ..Snapshot::default() }
    }

    /// The cards whose answer the hub carries out now (inbox-reappear,
    /// architect m_44978): a PR's merge (merge.rs), a feature's merge,
    /// try or drop (core_feature.rs). The one owner of `CardRow::running`.
    pub fn running_cards(&self) -> BTreeSet<u64> {
        self.merges.running().chain(self.feature_answers.values().copied()).collect()
    }

    /// An open card as the views see it: its words and what the hub adds.
    fn card_row(&self, c: &Card, now: u64, running: &BTreeSet<u64>) -> CardRow {
        // update-dot: the update card's release, changelog and notes link
        let (release, link) = if c.kind == release_line::KIND { self.update_release(c) } else { (None, None) };
        CardRow {
            release,
            link,
            running: running.contains(&c.id),
            id: c.id,
            kind: c.kind.clone(),
            agent: c.agent.clone(),
            text: c.text.clone(),
            for_msg: c.for_msg,
            age_ms: now.saturating_sub(c.created_ms),
            note: match c.kind.as_str() {
                "merge" => self.merge_note(c),
                signin_card::KIND => self.signin_note(),
                _ => self.card_note(c),
            },
            // expired-ux: the agents that wait for the sign-in
            waiting: (c.kind == signin_card::KIND).then(|| self.signin_waiting()),
            place: c.place.clone(),
            pr: c.pr,
            ..CardRow::default()
        }
    }

    /// A question card whose asker heard from main since, without a
    /// reply to the question: maybe answered another way (the card
    /// stays open, the view says so).
    fn card_note(&self, c: &Card) -> Option<String> {
        let id = self.card_note_msg(c)?;
        Some(format!("@main wrote to @{} since then (m_{}): {}", c.agent, id, clip(&one_line(&self.st.text(id)), 120)))
    }

    /// The message a question card's note cites: main's newest to the
    /// asker since the card, not the reply to its question.
    fn card_note_msg(&self, c: &Card) -> Option<u64> {
        if c.kind != "question" || c.agent == MAIN {
            return None;
        }
        self.st
            .peeks()
            .rev()
            .take_while(|m| m.created_ms >= c.created_ms)
            .find(|m| m.from == MAIN && m.to == c.agent && m.reply_to != c.for_msg)
            .map(|m| m.id)
    }
}
