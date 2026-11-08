//! Feature branches, the hub's side (dev-flow §5.1): a step's end
//! (`feature_done`: its items, the agents a merge or a drop archives, the
//! agents a retire moves to the trunk through sb-core's `feature_retire`),
//! the user's numbers on a feature item (refused or failed: the item
//! closes `handed to main`, and main gets one message with a ready
//! brief, `feature::handoff`; nothing opens again), the live agents of a feature,
//! the worktree of `sb spawn --feature`, and where an agent lands
//! ([`onto`]: the land job's target, read again by a land that waited
//! behind a retire). Git runs in daemon/features.rs, never here.

use super::*;

/// What a feature step did (dev-flow §5.1), for the hub: a card to open
/// or close, agents to archive, a line for main's feed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureDone {
    pub name: String,
    /// (kind, text): open this item (place `feature:<name>`), its others closed first.
    pub open: Option<(String, String)>,
    /// Close the feature's open items (with this result).
    pub close: Option<String>,
    pub archive: Vec<String>,
    /// `sb feature retire`: (branch's new name, trunk); sb-core moves its agents to the trunk.
    pub retire: Option<(String, String)>,
    /// Told once the hub ran this step (the retire's thread waits for it).
    pub ack: Option<feature::Ack>,
    /// a record for main's feed (`info`, `warn`).
    pub line: Option<Feed>,
    /// His answer didn't go through: its open items close as
    /// [`feature::HANDED`], and main (their agent) gets this brief
    /// ([`feature::handoff`]); nothing opens again.
    pub handoff: Option<String>,
}

/// Where `sb land` puts a worktree agent's commits: its feature's branch,
/// or None (the trunk). The land job takes it at the request and again
/// once in line on a feature (a retire may have ended the feature
/// meanwhile).
pub fn onto(ws: &Workspace) -> Option<String> {
    ws.feature().map(|f| format!("refs/heads/{}", f))
}

/// The step thread's wait for the hub (a retire holds the feature's land
/// turn until sb-core's step is applied). Equal to any other: a step's
/// result, not its content.
#[derive(Clone, Debug)]
pub struct Ack(pub std::sync::mpsc::Sender<()>);

impl PartialEq for Ack {
    fn eq(&self, _: &Ack) -> bool {
        true
    }
}

impl Hub {
    /// A feature step ended (dev-flow §5.1): its items replaced or
    /// closed, its agents archived (a merge, a drop) or moved to the trunk
    /// (a retire: sb-core decides who and tells them), main's feed told.
    pub(super) fn feature_done(&mut self, fx: &mut Fx, env: &mut dyn Env, d: FeatureDone) {
        let place = crate::feature::place_id(&d.name);
        // his answer's step (if any) ended: its card runs no more
        self.feature_answers.remove(&d.name);
        if d.open.is_some() || d.close.is_some() {
            let open: Vec<u64> = self
                .st
                .open_cards()
                .filter(|c| c.place.as_deref() == Some(place.as_str()))
                .map(|c| c.id)
                .collect();
            let res = d.close.clone().unwrap_or_else(|| "replaced".into());
            for id in open {
                self.close_card(fx, env, id, &res);
            }
        }
        // his answer did not go through: his item closes, handed over in
        // one message to its agent (main), which decides what follows;
        // nothing asks him the same thing again (#780 -> #782 -> #789)
        if let Some(brief) = &d.handoff {
            let open: Vec<(u64, String)> = self
                .st
                .open_cards()
                .filter(|c| c.place.as_deref() == Some(place.as_str()))
                .map(|c| (c.id, c.agent.clone()))
                .collect();
            let to = open.first().map(|(_, a)| a.clone()).unwrap_or_else(|| MAIN.to_string());
            for (id, _) in open {
                self.close_card(fx, env, id, crate::feature::HANDED);
            }
            self.core(fx, env, None, In::HubMsg(ci::HubMsg { to, text: brief.clone() }));
        }
        if let Some((kind, text)) = &d.open {
            self.open_card(
                fx,
                env,
                merge::HubItem { kind, agent: MAIN, text, place: &place, pr: None },
            );
        }
        for a in &d.archive {
            self.core(fx, env, None, In::Drop(ci::Drop { name: a.to_string(), force: true }));
        }
        if let Some((kept, main)) = &d.retire {
            let (name, kept, main) = (d.name.clone(), kept.clone(), main.clone());
            self.core(fx, env, None, In::FeatureRetire(ci::FeatureRetire { name, kept, main }));
        }
        if let Some(rec) = d.line.clone() {
            fx.push(feed(MAIN, rec));
        }
        // the merge reached the trunk: the changes tab's merged today again
        if d.close.as_deref() == Some("merged") {
            fx.push(Effect::MergedChanged);
        }
        self.dirty = true;
        // the state is sb-core's answer now: the waiting thread goes on
        if let Some(Ack(tx)) = d.ack {
            let _ = tx.send(());
        }
    }

    /// The user's number on a feature item (pr-merge's choice cards,
    /// dev-flow §5.1): `feature_try` 1 try it / 3 not yet (its `2 show
    /// the diff` is a client's act, never sent: it opens the feature's
    /// diff artifact, Shell::snapshot_card_links); `feature_merge` 1
    /// merge / 2 keep working / 3 drop the branch
    /// (the TUI asked "drop it?" once more before sending 3). The card
    /// stays open until the step ends (`feature_done` closes or replaces
    /// it). A number out of range: nothing. A merge, a try or a drop runs
    /// his answer: `card` is running until that step's `Input::Feature`
    /// (inbox-reappear: no client shows it again meanwhile).
    pub(super) fn feature_choice(&mut self, fx: &mut Fx, card: u64, kind: &str, place: Option<&str>, choice: &str) {
        let Some(name) = place.and_then(crate::feature::of_place) else { return };
        let op = match (kind, choice.trim()) {
            (crate::feature::TRY, "1") => "try",
            (crate::feature::TRY, "3") => "later",
            (crate::feature::MERGE, "1") => "merge",
            (crate::feature::MERGE, "2") => "keep",
            (crate::feature::MERGE, "3") => "drop",
            _ => return,
        };
        // the step always comes back as one Input::Feature (its end, or
        // the shell's refusal: a busy feature), which ends it
        if matches!(op, "merge" | "try" | "drop") {
            self.feature_answers.insert(name.to_string(), card);
        }
        let agents = self.feature_agents(name);
        fx.push(Effect::Feature { token: None, op: op.into(), name: name.into(), agents });
    }

    /// The live agents of feature `name` with their worktrees (dev-flow
    /// §5.1), in the hub's order.
    pub fn feature_agents(&self, name: &str) -> Vec<(String, String)> {
        self.st
            .order
            .iter()
            .filter_map(|n| self.st.agents.get(n))
            .filter(|a| a.lifecycle != Lifecycle::Archived && a.ws.feature() == Some(name))
            .map(|a| (a.name.clone(), a.ws.path.clone()))
            .collect()
    }

    /// Where agent `name` lands now ([`onto`]); None for an unknown one.
    pub fn onto_now(&self, name: &str) -> Option<Option<String>> {
        self.st.agents.get(name).map(|a| onto(&a.ws))
    }

    /// `sb spawn --feature <f>` (dev-flow §5.1): refused in a PR-flow repo;
    /// else sb-core creates the task placing and its fx `worktree_create`
    /// makes the worktree from the feature's tip, off the loop (issue 32
    /// 3c).
    pub(super) fn feature_allowed(&self) -> Result<(), String> {
        if self.flow == Some(crate::flow::FlowMode::Pr) {
            return Err("this repo ships through pull requests: a feature is a PR's branch here (`--place new`)".into());
        }
        Ok(())
    }
}
