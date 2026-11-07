//! A plan's own card (plan-ok, Gabriel's pick A; pages/plan.rs): a page
//! with a plan block gets one card, asked by the plan's agent (sb-core's
//! page_card: the first live of the plan's agent and the page's, else
//! main), no batch card beside it (one card per page). It opens for a
//! version not started yet; a newer version not started replaces it (the
//! same key); a start from either door (the card's option, the page's
//! button) closes it once and tells the plan's agent. Its open option
//! leaves it open; his other words go to the plan's agent.

use super::*;
use crate::pages::plan::{Plan, PlanAnswer};

/// The card's kind (sb-core's model.bend page_kind).
const KIND: &str = "plan";

impl Shell {
    /// The agent a plan's start and words go to: its own when it is still
    /// here, else the page's, else main (the asker chain of page_card).
    fn plan_to(&self, plan: &Plan, page_agent: &str) -> String {
        [plan.agent.as_str(), page_agent].into_iter().find(|a| !a.is_empty() && !self.page_agent_gone(a)).unwrap_or(MAIN).to_string()
    }

    /// Page `id`'s plan card after any change (a publish, a note, a
    /// start): opened, kept, replaced or closed. True: page `id` has a
    /// plan (no batch card then).
    pub(super) fn page_plan(&mut self, id: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let plan = crate::pages::plan::of_fragment(&html);
        let mut steps = pages.store.steps(id);
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let version = format!("v{}", meta.version());
        // started: the fragment says so, or a start note of this version
        let started = plan.as_ref().is_some_and(|p| {
            p.is_started() || pages.store.notes(id).iter().any(|n| n.kind == "start" && n.version == meta.version() && p.start_of(&n.block, n.extra.get("item").and_then(|v| v.as_str())).is_some())
        });
        let text = plan.as_ref().map(|p| p.card_text(&meta.title)).unwrap_or_default();
        let mut changed = false;
        let mut have = false;
        for s in steps.iter_mut().filter(|s| s.block.starts_with("plan:") && s.reply.is_none()) {
            let same = plan.as_ref().is_some_and(|p| s.block == p.key()) && s.item == version && s.text == text;
            if same && !started && open.contains(&s.card) {
                have = true;
                continue;
            }
            let res = if started { "started" } else { "replaced" };
            s.reply = Some(res.into());
            self.page_cards().remove(&s.card);
            if open.contains(&s.card) {
                self.step(Input::ConfirmClose { card: s.card, res: res.into() });
            }
            changed = true;
        }
        if let (Some(p), false, false) = (&plan, have, started) {
            // closed by him for this version: not asked again until the next
            let dismissed = steps.iter().any(|s| s.block == p.key() && s.item == version && s.reply.as_deref() == Some(""));
            if !dismissed {
                let before = open.clone();
                let key = p.key();
                let about = self.page_about(id, key.strip_prefix("plan:").unwrap_or(&key), None);
                self.step(Input::PageCard { agents: vec![p.agent.clone(), meta.agent.clone()], kind: KIND.into(), text: text.clone(), key, about });
                match self.hub.st.open_cards().filter(|c| !before.contains(&c.id) && c.kind == KIND).map(|c| c.id).max() {
                    Some(card) => {
                        self.page_cards().insert(card, (id.to_string(), p.key()));
                        steps.push(crate::pages::checklist::Step { block: p.key(), item: version, text, card, ..Default::default() });
                    }
                    None => log_line(&self.opts.paths, &format!("page {id}: no card opened for its plan")),
                }
                changed = true;
            }
        }
        if changed {
            if let Err(e) = pages.locked(|st| st.save_steps(id, &steps)) {
                log_line(&self.opts.paths, &format!("page {id}: steps not saved: {e}"));
            }
            pages.push("", "pages", &json!({"pages": pages.list_json()}));
        }
        plan.is_some()
    }

    /// His answer on a plan's card: a start (the plan or one of its other
    /// ways) is a start note on the page, to the plan's agent, and the
    /// card closes; open: the client hears where the page is, the card
    /// stays; other words go to the plan's agent as his, the card stays
    /// (its next version replaces it).
    pub(super) fn page_plan_answer(&mut self, client: ClientId, id: &str, card: u64, reply: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let Some(plan) = crate::pages::plan::of_fragment(&html) else { return false };
        let to = self.plan_to(&plan, &meta.agent);
        let (block, item) = match crate::pages::plan::answer(reply, &plan) {
            PlanAnswer::Empty => return true,
            PlanAnswer::Open => {
                self.notice_out(client, Some("answer"), &format!("the plan and its other ways are on its page: {}", pages.url(id)));
                return true;
            }
            PlanAnswer::Words(words) => {
                let line = format!("start the plan {} recommends?", plan.agent);
                let asked = self.hub.st.open_cards().find(|c| c.id == card).map_or_else(|| line.clone(), |c| c.text.clone());
                let (text, part) = crate::pages::store::batch_words(&meta, &pages.url(id), card, &line, &asked, &to, words);
                let text = text.replacen("on the drafts notification", "on the plan's notification", 1);
                let parts = serde_json::to_string(&[part]).unwrap_or_default();
                log_line(&self.opts.paths, &format!("page {id}: words on plan card #{card} → {to}: {words:?}"));
                self.step(Input::ClientInput { client: 0, focus: to.clone(), text, queued: false, parts, from: String::new() });
                self.notice_out(client, Some("answer"), &format!("sent to {to}: \"{words}\". it publishes the next version of the plan."));
                return true;
            }
            PlanAnswer::Start => (plan.block.clone(), None),
            PlanAnswer::Alt(i) => match (&plan.ways, plan.alts.get(i)) {
                (Some(w), Some(a)) => (w.clone(), Some(a.item.clone())),
                _ => return false,
            },
        };
        let label = reply.trim().to_string();
        if let Err(e) = pages.locked(|s| s.add_sent(id, &block, item.as_deref(), "start", &label, &[], meta.version(), now_ms())) {
            log_line(&self.opts.paths, &format!("page {id}: a plan's start not saved: {e}"));
            return false;
        }
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        self.page_plan_starts(id);
        true
    }

    /// The plan's start notes just sent (from its card or its page's
    /// button): routed to the plan's agent with what to do, once; its
    /// card closes (started).
    pub(in crate::daemon) fn page_plan_starts(&mut self, id: &str) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(meta) = pages.store.meta(id) else { return };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let Some(plan) = crate::pages::plan::of_fragment(&html) else { return };
        let to = self.plan_to(&plan, &meta.agent);
        let ours = |n: &crate::pages::store::Note| plan.start_of(&n.block, n.extra.get("item").and_then(|v| v.as_str())).is_some();
        let starts = pages.locked(|s| s.route_starts(id, &ours, &to)).unwrap_or_default();
        // one start: the first (a second click or option is the same go)
        let Some(first) = starts.first() else { return };
        let alt = match plan.start_of(&first.block, first.extra.get("item").and_then(|v| v.as_str())) {
            Some(PlanAnswer::Alt(i)) => Some(i),
            _ => None,
        };
        let at = crate::every::hh_mm(now_ms());
        let text = plan.start_message(&meta.title, id, meta.version(), &pages.url(id), alt, &at);
        let text = Self::page_hint(&to, text);
        log_line(&self.opts.paths, &format!("page {id}: plan started → {to}"));
        self.step(Input::ClientInput { client: 0, focus: to, text, queued: false, parts: String::new(), from: String::new() });
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        self.page_plan(id);
    }
}
