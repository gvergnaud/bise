//! The pages' cards (docs/ambient-pages.md §2.4-§2.6): the links from
//! cards to pages, a page's questions and step cards and their answers,
//! drafts batches and ticks, and the hint main's page replies carry.
//! Moved out of daemon/pages.rs unchanged (architect's 1,000-line rule).

use super::*;

/// What a page's path did with a line he sent ([`Shell::page_first`]).
pub(super) enum Page {
    /// taken there (a step ticked, a drafts or replaced batch card)
    Done,
    /// a page question's answer: the line to step, its digit as words
    Reply(String),
    /// not a page's card
    Not,
}

impl Shell {
    /// The snapshot's pages (docs/ambient-pages.md §2.3), newest first, and
    /// each card's page link. Moved out of Shell::snapshot unchanged
    /// (architect m_12280).
    pub(super) fn snapshot_pages(&mut self, snap: &mut Value) {
        if let Some(p) = &self.pg.pages {
            snap["pages"] = json!(p.list_json());
            snap["pages_url"] = json!(p.base());
            // his late promises (checklist rows of his past data-due): a
            // count for the morning page and the menu bar, never a card
            snap["overdue"] = json!(p.overdue());
            // a page question's card links its block (fn + ⏎ opens it)
            let p = p.clone();
            let qs = self.page_cards().clone();
            let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
            let n = self.pg.card_links.len();
            self.pg.card_links.retain(|c, _| open.contains(c));
            if self.pg.card_links.len() != n {
                self.save_card_links();
            }
            if let Some(cards) = snap["cards"].as_array_mut() {
                for c in cards {
                    // an agent's card linked with `sb card --page`
                    if let Some(link) = c["id"].as_u64().and_then(|n| self.pg.card_links.get(&n)) {
                        c["page"] = link.clone();
                    }
                    let Some((id, block)) = c["id"].as_u64().and_then(|n| qs.get(&n)) else { continue };
                    // drafts batched in one card: the page opens at the
                    // first (ambient-lead m_5977)
                    if let Some((b, anchor)) = block.strip_prefix("drafts:").and_then(|k| k.split_once(':')) {
                        c["page"] = json!({"id": id, "block": b, "item": anchor, "drafts": true, "url": format!("{}#{}", p.url(id), anchor)});
                        // its fields for the capsule's words (amb-web
                        // m_6061): {count, title, what, names}
                        let card = c["id"].as_u64();
                        if let Some(s) = p.store.steps(id).into_iter().find(|s| Some(s.card) == card && !s.info.is_null()) {
                            c["batch"] = s.info;
                        }
                        continue;
                    }
                    // a step of his: its row (roadmap D)
                    c["page"] = match block.strip_prefix("row:").and_then(|k| k.split_once('/')) {
                        Some((b, item)) => json!({"id": id, "block": b, "item": item, "url": format!("{}#{}", p.url(id), item)}),
                        None => json!({"id": id, "block": block, "url": format!("{}#{}", p.url(id), block)}),
                    };
                }
            }
        }
    }

    /// `sb card --page <id>[#<item>]`: a page that isn't there is refused
    /// before the card opens (pm's B m_6008). Ok(None): not a card with a
    /// page; Err: the refusal's words. Moved out of daemon.rs unchanged
    /// (architect m_12280).
    pub(super) fn card_page_link(&self, cmd: &str, v: &Value) -> Result<Option<Value>, String> {
        match (cmd, v.get("page").and_then(Value::as_str)) {
            ("card", Some(spec)) => match self.pg.pages.as_ref().map(|p| crate::card_link::link(p, spec)) {
                Some(Ok(l)) => Ok(Some(l)),
                Some(Err(e)) => Err(e),
                None => Err("no pages on this hub".into()),
            },
            _ => Ok(None),
        }
    }

    /// The card that step opened (an open card not in `before`) gets its
    /// page link, saved, and the clients see it at once.
    pub(super) fn link_new_card(&mut self, before: &[u64], l: Value) {
        let new = self.hub.st.open_cards().map(|c| c.id).filter(|c| !before.contains(c)).max();
        if let Some(card) = new {
            self.pg.card_links.insert(card, l);
            self.save_card_links();
            self.state_now();
        }
    }

    /// A note talk's words (docs/ambient-pages.md §4.1): to the page's
    /// frame, never an input. Moved out of daemon.rs's page_voice arm
    /// unchanged (architect m_12280).
    pub(super) fn page_voice(&self, v: &Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        if let Some(p) = &self.pg.pages {
            let page = s("page");
            if p.store.meta(&page).is_some() {
                p.push(&page, "voice", &json!({"phase": s("phase"), "text": s("text")}));
            }
        }
    }

    /// `card_links` to the hub's state (card_link.rs), after each change.
    pub(super) fn save_card_links(&self) {
        if let Err(e) = crate::card_link::save(&crate::card_link::file(&self.opts.paths.state), &self.pg.card_links) {
            log_line(&self.opts.paths, &format!("card links not saved: {e}"));
        }
    }

    /// The open cards of the pages' questions: card → (page, block),
    /// read from the store once.
    pub(super) fn page_cards(&mut self) -> &mut BTreeMap<u64, (String, String)> {
        let pages = self.pg.pages.clone();
        self.pg.page_cards.get_or_insert_with(|| {
            let mut m = BTreeMap::new();
            if let Some(p) = pages {
                for meta in p.store.list() {
                    for q in p.store.questions(&meta.id) {
                        if q.card != 0 && q.reply.is_none() {
                            m.insert(q.card, (meta.id.clone(), q.block.clone()));
                        }
                    }
                    // the open steps' cards too: after a hub restart a
                    // step's card lost its page link and its page path
                    // (pm's D fail 31: card #2 with page null)
                    for s in p.store.steps(&meta.id) {
                        if s.card != 0 && s.reply.is_none() {
                            m.insert(s.card, (meta.id.clone(), Self::step_key(&s.block, &s.item)));
                        }
                    }
                }
            }
            m
        })
    }

    /// §4.2: after a publish of page `id`, one open card per question
    /// block, `agent` asking: a changed question gets a new card (the
    /// old one closes `replaced`), a question gone closes its card
    /// (`withdrawn`), an answered one stays answered.
    pub(super) fn page_questions(&mut self, id: &str, agent: &str, html: &str) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let old = pages.store.questions(id);
        let new = crate::pages::questions::of_fragment(html);
        if old.is_empty() && new.is_empty() {
            return;
        }
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let plan = crate::pages::questions::plan(&old, new, &|c| open.contains(&c));
        for (card, res) in &plan.close {
            self.page_cards().remove(card);
            self.step(Input::ConfirmClose { card: *card, res: res.to_string() });
        }
        let mut qs = plan.questions;
        // a page-only question (data-card="none") never opens one; a
        // dismissed one not again until its words change (m_6091)
        for q in qs.iter_mut().filter(|q| q.card == 0 && q.reply.is_none() && !q.page_only && !q.dismissed) {
            let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
            // sb-core opens cards for main only (a task's request is
            // refused: pm's 25), so main asks; the hub answers it itself
            // (page_card_answer: the pick goes to the page's agent `agent`).
            // No client waits for the reply (token 0: never a connection's)
            let _ = agent;
            self.step(Input::Agent {
                token: 0,
                from: MAIN.to_string(),
                req: crate::core::AgentReq::Card { text: q.card_text(), for_msg: None },
            });
            let card = self.hub.st.open_cards().filter(|c| !before.contains(&c.id) && c.kind == "question").map(|c| c.id).max();
            match card {
                Some(card) => {
                    q.card = card;
                    self.page_cards().insert(card, (id.to_string(), q.block.clone()));
                }
                None => log_line(&self.opts.paths, &format!("page {id}: no card opened for question {}", q.block)),
            }
        }
        if let Err(e) = pages.locked(|s| s.save_questions(id, &qs)) {
            log_line(&self.opts.paths, &format!("page {id}: questions not saved: {e}"));
        }
        pages.push("", "pages", &json!({"pages": pages.list_json()}));
    }

    /// A page question answered with no open card (amb-kit m_5336,
    /// ambient-lead m_5343): the answer is kept, the page hears
    /// `answered`, and the pick is a `pick` note on that block, sent to
    /// the page's agent (main when it is gone) like any notes.
    pub(super) fn page_answer_without_card(&mut self, id: &str, block: &str, reply: &str) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(meta) = pages.store.meta(id) else { return };
        let mut qs = pages.store.questions(id);
        let Some(q) = qs.iter_mut().find(|q| q.block == block && q.reply.is_none()) else { return };
        q.reply = Some(reply.to_string());
        if let Err(e) = pages.locked(|s| s.save_questions(id, &qs)) {
            log_line(&self.opts.paths, &format!("page {id}: questions not saved: {e}"));
        }
        pages.push(id, "answered", &json!({"block": block, "reply": reply}));
        let note = match pages.locked(|s| s.add_pick(id, block, reply, meta.version(), now_ms())) {
            Ok(n) => n,
            Err(e) => return log_line(&self.opts.paths, &format!("page {id}: the pick not saved: {e}")),
        };
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        let text = crate::pages::store::notes_message(&meta, &pages.url(id), &[note]);
        self.page_msg(crate::pages::PageMsg::Sent { id: id.to_string(), agent: meta.agent.clone(), text });
        // a drafted step that waited for this answer gets its card now
        self.page_steps(id);
    }

    // ---- checklists that tick themselves (docs/ambient-roadmap.md §2 D) ----

    /// The key of a step's card in `page_cards` (a question's is its block).
    pub(super) fn step_key(block: &str, item: &str) -> String {
        format!("row:{block}/{item}")
    }

    /// Page `id`'s steps of the user's after any change (a publish, a
    /// tick, a note): the step whose turn came gets its card (one per
    /// checklist block), a step done or ticked closes its card.
    pub(super) fn page_steps(&mut self, id: &str) {
        // one mechanism (ambient-lead m_6005): only a checklist marked
        // data-plan makes step cards (the filter on `due` below). A
        // promises page or a meeting page has none, so its rows of his
        // show in sb page waiting and the state's overdue count only.
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(meta) = pages.store.meta(id) else { return };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let rows = crate::pages::checklist::judged(&html, &pages.store.questions(id));
        let mut steps = pages.store.steps(id);
        let notes = pages.store.notes(id);
        // drafts batch on any page but a promises or meeting page (never a
        // card there): a feedback page's replies have no plan (pm's C fail
        // 42: 3 Slack replies and no card)
        let drafts = match crate::pages::drafts::carded(id) {
            // (a draft whose item's agent is still at work waits: m_6168)
            true => crate::pages::drafts::ready(&html, crate::pages::drafts::pending(&html, &notes, &pages.store.questions(id)), &|a| pages.held(id, a, meta.version())),
            false => Vec::new(),
        };
        if steps.is_empty() && !rows.iter().any(|r| r.his() && r.plan) && drafts.len() < 2 {
            return;
        }
        let closed = |r: &crate::pages::checklist::Row| crate::pages::checklist::closed_by(r, &notes);
        // cards only for a plan he works through with bise (data-plan):
        // a meeting's or a promise's rows of his reach him by sb page
        // waiting and the count (ambient-lead m_5988)
        let mut due: Vec<crate::pages::checklist::Row> =
            crate::pages::checklist::due(&rows, &closed).into_iter().filter(|r| r.plan).cloned().collect();
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let mut changed = false;
        // two or more drafts waiting at once are ONE card (ambient-lead
        // m_5977): its drafted steps get no card of their own
        let batch_open = steps.iter().any(|s| s.block == crate::pages::checklist::BATCH && s.reply.is_none());
        if drafts.len() >= 2 || (batch_open && !drafts.is_empty()) {
            due.retain(|r| !r.drafted);
            let mut kept: Vec<crate::pages::checklist::Step> = Vec::new();
            for s in std::mem::take(&mut steps) {
                if s.drafted && s.reply.is_none() {
                    // a drafted step's own card: the batch card asks it now
                    self.page_cards().remove(&s.card);
                    if open.contains(&s.card) {
                        self.step(Input::ConfirmClose { card: s.card, res: "replaced".into() });
                    }
                    changed = true;
                    continue;
                }
                kept.push(s);
            }
            steps = kept;
        }
        changed |= self.page_batch(id, &meta.title, &mut steps, &drafts, &open);
        for s in steps.iter_mut().filter(|s| s.reply.is_none() && s.block != crate::pages::checklist::BATCH) {
            let still = due.iter().any(|r| r.block == s.block && r.item == s.item);
            if still && open.contains(&s.card) {
                continue;
            }
            // done (republished, ticked on the page) or closed another way
            s.reply = Some(if still { String::new() } else { "done".into() });
            self.page_cards().remove(&s.card);
            if open.contains(&s.card) {
                self.step(Input::ConfirmClose { card: s.card, res: "ticked".into() });
            }
            changed = true;
        }
        // a drafted step's card waits while the page asks him anything:
        // its draft may need that answer (legal's address), and two cards
        // at once read as one muddle (pm's D fail 31, m_5870)
        let asking = pages.store.questions(id).iter().any(|q| q.reply.is_none());
        for r in &due {
            if steps.iter().any(|s| s.block == r.block && s.item == r.item) {
                continue;
            }
            if r.drafted && asking {
                continue;
            }
            // nor while its draft cannot leave (no recipient yet): his
            // send would do nothing (pm's D fail 37)
            if r.drafted && r.draft.as_deref().is_some_and(|d| !crate::pages::checklist::draft_sendable(&html, d)) {
                continue;
            }
            let mut s = crate::pages::checklist::Step {
                block: r.block.clone(),
                item: r.item.clone(),
                text: r.text.clone(),
                card: 0,
                reply: None,
                drafted: r.drafted,
                ..Default::default()
            };
            let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
            // main asks (sb-core's cards are main's, pm's 25); its answer
            // is the hub's own path (page_card_answer → page_tick)
            self.step(Input::Agent {
                token: 0,
                from: MAIN.to_string(),
                req: crate::core::AgentReq::Card { text: s.card_text(), for_msg: None },
            });
            match self.hub.st.open_cards().filter(|c| !before.contains(&c.id) && c.kind == "question").map(|c| c.id).max() {
                Some(card) => {
                    s.card = card;
                    self.page_cards().insert(card, (id.to_string(), Self::step_key(&r.block, &r.item)));
                }
                None => {
                    log_line(&self.opts.paths, &format!("page {id}: no card opened for step {}", r.item));
                    s.reply = Some(String::new());
                }
            }
            steps.push(s);
            changed = true;
        }
        if changed {
            if let Err(e) = pages.locked(|st| st.save_steps(id, &steps)) {
                log_line(&self.opts.paths, &format!("page {id}: steps not saved: {e}"));
            }
            pages.push("", "pages", &json!({"pages": pages.list_json()}));
        }
    }

    /// Main asks a page's card (sb-core's cards are main's, pm's 25);
    /// `key` maps it back to the page's own answer path. None: no card.
    pub(super) fn open_page_card(&mut self, id: &str, text: String, key: String) -> Option<u64> {
        let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        self.step(Input::Agent { token: 0, from: MAIN.to_string(), req: crate::core::AgentReq::Card { text, for_msg: None } });
        let card = self.hub.st.open_cards().filter(|c| !before.contains(&c.id) && c.kind == "question").map(|c| c.id).max()?;
        self.page_cards().insert(card, (id.to_string(), key));
        Some(card)
    }

    /// Page `id`'s batch card (ambient-lead m_5977): open while two or
    /// more drafts wait (and until its last one is sent or skipped); a
    /// draft it lacks replaces it with one for them all. True: `steps`
    /// changed.
    pub(super) fn page_batch(&mut self, id: &str, title: &str, steps: &mut Vec<crate::pages::checklist::Step>, drafts: &[crate::pages::drafts::Draft], open: &[u64]) -> bool {
        use crate::pages::checklist::{Step, BATCH};
        let keys: Vec<String> = drafts.iter().map(|d| d.key()).collect();
        let mut changed = false;
        let mut have = false;
        let mut replaced = false;
        for s in steps.iter_mut().filter(|s| s.block == BATCH && s.reply.is_none()) {
            let covers = keys.iter().all(|k| s.batch.contains(k));
            let (res, reply) = match (drafts.is_empty(), covers, open.contains(&s.card)) {
                // all sent or skipped
                (true, _, _) => ("done", "done"),
                // a draft it lacks: one card for them all
                (false, false, _) => ("replaced", ""),
                (false, true, true) => {
                    have = true;
                    continue;
                }
                // closed another way (dismissed): not asked again for
                // the same drafts
                (false, true, false) => ("", ""),
            };
            replaced |= res == "replaced";
            s.reply = Some(reply.into());
            self.page_cards().remove(&s.card);
            if open.contains(&s.card) {
                self.step(Input::ConfirmClose { card: s.card, res: res.into() });
            }
            changed = true;
        }
        let dismissed = steps
            .iter()
            .any(|s| s.block == BATCH && s.reply.as_deref() == Some("") && keys.iter().all(|k| s.batch.contains(k)));
        let new = drafts.len() >= 2 || (replaced && !drafts.is_empty());
        if have || !new || dismissed {
            return changed;
        }
        // one batch card per page, ever (pm's C fail 43): any other open
        // one of this page closes as replaced before the new one opens
        let stale: Vec<u64> = self
            .page_cards()
            .iter()
            .filter(|(c, (p, k))| p == id && k.starts_with("drafts:") && open.contains(c))
            .map(|(c, _)| *c)
            .collect();
        for c in stale {
            self.page_cards().remove(&c);
            self.step(Input::ConfirmClose { card: c, res: "replaced".into() });
            for s in steps.iter_mut().filter(|s| s.card == c && s.reply.is_none()) {
                s.reply = Some(String::new());
            }
        }
        let first = &drafts[0];
        let mut s = Step {
            block: BATCH.into(),
            item: "drafts".into(),
            text: crate::pages::drafts::batch_text(drafts, title),
            batch: keys,
            info: crate::pages::drafts::info(drafts, title),
            ..Step::default()
        };
        match self.open_page_card(id, s.card_text(), format!("drafts:{}:{}", first.block, first.anchor())) {
            Some(card) => s.card = card,
            None => {
                log_line(&self.opts.paths, &format!("page {id}: no card opened for its {} drafts", drafts.len()));
                s.reply = Some(String::new());
            }
        }
        steps.push(s);
        true
    }

    /// His answer on a batch card: send all = an approve note per draft
    /// it holds that still waits, one notes message to the page's agent
    /// (main when it is gone), the card closes. Review (2) or other
    /// words: the card stays (the capsule opens the page at the first).
    pub(super) fn page_drafts_answer(&mut self, id: &str, card: u64, reply: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        if !crate::pages::drafts::is_send_all(reply) {
            return true;
        }
        let mut steps = pages.store.steps(id);
        let Some(keys) = steps.iter().find(|s| s.card == card && s.reply.is_none()).map(|s| s.batch.clone()) else { return false };
        let sent = self.page_approve_drafts(id, Some(&keys));
        for s in steps.iter_mut().filter(|s| s.card == card && s.reply.is_none()) {
            s.reply = Some("send all".into());
        }
        if let Err(e) = pages.locked(|st| st.save_steps(id, &steps)) {
            log_line(&self.opts.paths, &format!("page {id}: steps not saved: {e}"));
        }
        self.page_cards().remove(&card);
        if self.hub.st.open_cards().any(|c| c.id == card) {
            self.step(Input::ConfirmClose { card, res: "answered".into() });
        }
        self.page_send_approves(id, &meta, &sent);
        self.page_steps(id);
        true
    }

    /// An approve note for each draft of page `id` that waits now (only
    /// those in `keys` when given); the notes it wrote.
    pub(super) fn page_approve_drafts(&mut self, id: &str, keys: Option<&[String]>) -> Vec<crate::pages::store::Note> {
        let Some(pages) = self.pg.pages.clone() else { return Vec::new() };
        let Some(meta) = pages.store.meta(id) else { return Vec::new() };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let drafts = crate::pages::drafts::ready(&html, crate::pages::drafts::pending(&html, &pages.store.notes(id), &pages.store.questions(id)), &|a| pages.held(id, a, meta.version()));
        let mut sent = Vec::new();
        for d in drafts.iter().filter(|d| keys.is_none_or(|k| k.contains(&d.key()))) {
            match pages.locked(|s| s.add_sent(id, &d.block, d.item.as_deref(), "approve", "send", meta.version(), now_ms())) {
                Ok(n) => sent.push(n),
                Err(e) => log_line(&self.opts.paths, &format!("page {id}: an approve not saved: {e}")),
            }
        }
        sent
    }

    /// The approves written: the page hears them, its agent (main when
    /// gone) gets one notes message.
    pub(super) fn page_send_approves(&mut self, id: &str, meta: &crate::pages::store::Meta, sent: &[crate::pages::store::Note]) {
        let Some(pages) = self.pg.pages.clone() else { return };
        if sent.is_empty() {
            return;
        }
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        let msg = crate::pages::store::notes_message(meta, &pages.url(id), sent);
        self.page_msg(crate::pages::PageMsg::Sent { id: id.to_string(), agent: meta.agent.clone(), text: msg });
    }

    /// An answer on a batch card that was replaced or closed (pm's C fail
    /// 43: his 'send all 6' on a replaced card did nothing): send all acts
    /// on the page's current drafts (its open batch card, else the drafts
    /// that wait); other words say in one line what the card is now. Never
    /// silently nothing. False: no batch card of a page.
    pub(super) fn page_old_batch_answer(&mut self, client: ClientId, card: u64, reply: &str) -> bool {
        use crate::pages::checklist::BATCH;
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(id) = pages.store.list().into_iter().map(|m| m.id).find(|id| pages.store.steps(id).iter().any(|s| s.block == BATCH && s.card == card)) else {
            return false;
        };
        let Some(meta) = pages.store.meta(&id) else { return false };
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let now = pages.store.steps(&id).into_iter().find(|s| s.block == BATCH && s.reply.is_none() && open.contains(&s.card));
        let say = match (now, crate::pages::drafts::is_send_all(reply)) {
            (Some(s), true) => {
                self.page_drafts_answer(&id, s.card, reply);
                None
            }
            (None, true) => {
                let sent = self.page_approve_drafts(&id, None);
                if sent.is_empty() {
                    Some(format!("that card is closed: nothing waits on {} now.", meta.title))
                } else {
                    self.page_send_approves(&id, &meta, &sent);
                    self.page_steps(&id);
                    None
                }
            }
            (Some(s), false) => Some(format!("that card changed: {}", s.text.replace('\n', " · "))),
            (None, false) => Some(format!("that card is closed: nothing waits on {} now.", meta.title)),
        };
        log_line(&self.opts.paths, &format!("page {id}: an answer on old batch card #{card}: {reply:?} → {}", say.as_deref().unwrap_or("sent the current drafts")));
        if let (Some(text), Some(c)) = (say, self.clients.get_mut(&client)) {
            write_json(c, &json!({"ev": "notice", "text": text}));
        }
        true
    }

    /// A step ticked from its card or by voice (`sb page tick`): a sent
    /// tick note on its row, the page hears `ticked`, the page's agent
    /// gets the notes message (main when it is gone), its card closes and
    /// the next step's turn comes. `text`: the user's words when they are
    /// not a plain "done". False: no such row.
    pub(super) fn page_tick(&mut self, id: &str, item: &str, text: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        let rows = pages.store.html(id, meta.version()).map(|h| crate::pages::checklist::rows_of(&h)).unwrap_or_default();
        let Some(row) = rows.iter().find(|r| r.item == item) else { return false };
        let go = crate::pages::checklist::is_done_word(text) || text.trim().eq_ignore_ascii_case("send");
        let (kind, words) = match (go, row.drafted) {
            // a drafted step's "send": an approve of its draft, so the
            // page's agent sends it (pm's D fail 30)
            (true, true) => ("approve", "send"),
            (true, false) => ("tick", "done"),
            (false, _) => ("note", text.trim()),
        };
        let (block, on) = match (kind, row.draft.as_deref()) {
            ("approve", Some(d)) => (d.to_string(), None),
            _ => (row.block.clone(), Some(item)),
        };
        let note = match pages.locked(|s| s.add_sent(id, &block, on, kind, words, meta.version(), now_ms())) {
            Ok(n) => n,
            Err(e) => {
                log_line(&self.opts.paths, &format!("page {id}: the tick not saved: {e}"));
                return false;
            }
        };
        if kind == "tick" {
            pages.push(id, "ticked", &json!({"block": row.block, "item": item}));
        }
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        // its card: closed by this answer, never reopened for words
        let mut steps = pages.store.steps(id);
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        for s in steps.iter_mut().filter(|s| s.item == item && s.block == row.block && s.reply.is_none()) {
            s.reply = Some(words.to_string());
            self.page_cards().remove(&s.card);
            if open.contains(&s.card) {
                self.step(Input::ConfirmClose { card: s.card, res: "answered".into() });
            }
        }
        if let Err(e) = pages.locked(|st| st.save_steps(id, &steps)) {
            log_line(&self.opts.paths, &format!("page {id}: steps not saved: {e}"));
        }
        let msg = crate::pages::store::notes_message(&meta, &pages.url(id), &[note]);
        self.page_msg(crate::pages::PageMsg::Sent { id: id.to_string(), agent: meta.agent.clone(), text: msg });
        self.page_steps(id);
        true
    }

    /// A line he sent that may answer a page's card, the page's path
    /// first, in its one order for every door (the input handler,
    /// card/answer, command/run's `/answer`; architect m_13688): a step
    /// ticked or a drafts or replaced batch card answered there
    /// ([`Page::Done`]); a question's digit as its option's words
    /// ([`Page::Reply`], the line to step); else [`Page::Not`]. The
    /// caller ends with [`Self::page_answers`] (the page hears it).
    pub(super) fn page_first(&mut self, client: ClientId, line: &str) -> Page {
        if self.page_step_answer(client, line) {
            return Page::Done;
        }
        match self.page_reply_text(line) {
            Some(l) => Page::Reply(l),
            None => Page::Not,
        }
    }

    /// `/answer N reply` on a step's card: the step ticked (or the words
    /// as a note on its row), never the hub's question path. False: not
    /// a step's card.
    fn page_step_answer(&mut self, client: ClientId, text: &str) -> bool {
        let Some(rest) = text.trim().strip_prefix("/answer ") else { return false };
        let Some((n, reply)) = rest.trim().split_once(' ') else { return false };
        let Ok(card) = n.trim_start_matches('#').parse::<u64>() else { return false };
        let Some((id, key)) = self.page_cards().get(&card).cloned() else { return self.page_old_batch_answer(client, card, reply) };
        if key.starts_with("drafts:") {
            return self.page_drafts_answer(&id, card, reply);
        }
        match key.strip_prefix("row:").and_then(|k| k.split_once('/')).map(|(_, i)| i.to_string()) {
            Some(item) => self.page_tick(&id, &item, reply),
            None => self.page_question_answer(&id, &key, card, reply),
        }
    }

    /// A page question's card answered (from the TUI, the capsule or the
    /// page): the card closes, and the answer takes the page's path: the
    /// page hears `answered`, the page's agent gets it as a pick note
    /// (main when it is gone). sb-core's question path would send it to
    /// main, the card's asker (pm's 25: a task's page).
    pub(super) fn page_question_answer(&mut self, id: &str, block: &str, card: u64, reply: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let reply = reply.trim();
        let words = match reply.parse::<usize>() {
            Ok(k) => pages.store.questions(id).iter().find(|q| q.block == block).and_then(|q| q.option(k).map(String::from)),
            Err(_) => None,
        }
        .unwrap_or_else(|| reply.to_string());
        if words.is_empty() {
            return false;
        }
        self.page_cards().remove(&card);
        if self.hub.st.open_cards().any(|c| c.id == card) {
            self.step(Input::ConfirmClose { card, res: "answered".into() });
        }
        self.page_answer_without_card(id, block, &words);
        true
    }

    /// The card of question `block` on page `id`, when it is still open.
    pub(super) fn page_card(&mut self, id: &str, block: &str) -> Option<u64> {
        let card = self.page_cards().iter().find(|(_, (p, b))| p == id && b == block).map(|(c, _)| *c)?;
        self.hub.st.open_cards().any(|c| c.id == card).then_some(card)
    }

    /// `/answer N reply` on a page question's card (from the TUI, the
    /// capsule or the page): an option's number becomes its words (the
    /// agent reads words), kept until the card closes.
    fn page_reply_text(&mut self, text: &str) -> Option<String> {
        let rest = text.trim().strip_prefix("/answer ")?;
        let (n, reply) = rest.trim().split_once(' ')?;
        let card: u64 = n.trim_start_matches('#').parse().ok()?;
        let (id, block) = self.page_cards().get(&card).cloned()?;
        let pages = self.pg.pages.clone()?;
        let reply = reply.trim().to_string();
        let words = match reply.parse::<usize>() {
            Ok(k) => pages.store.questions(&id).iter().find(|q| q.block == block).and_then(|q| q.option(k).map(String::from)),
            Err(_) => None,
        }
        .unwrap_or(reply);
        self.pg.page_replies.insert(card, words.clone());
        Some(format!("/answer {card} {words}"))
    }

    /// The page questions whose cards closed: the answer goes on the page
    /// (`answered {block, reply}` on its SSE; "" when the card closed
    /// without one: withdrawn by main, closed by the user).
    pub(super) fn page_answers(&mut self) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let closed: Vec<(u64, String, String)> = self
            .page_cards()
            .iter()
            .filter(|(c, _)| !open.contains(c))
            .map(|(c, (p, b))| (*c, p.clone(), b.clone()))
            .collect();
        if closed.is_empty() {
            return;
        }
        for (card, id, block) in closed {
            self.page_cards().remove(&card);
            if block.starts_with("row:") {
                // a step's card closed another way (the user closed it):
                // it stays closed; a tick still ticks the row
                let mut steps = pages.store.steps(&id);
                for s in steps.iter_mut().filter(|s| s.card == card && s.reply.is_none()) {
                    s.reply = Some(String::new());
                }
                if let Err(e) = pages.locked(|s| s.save_steps(&id, &steps)) {
                    log_line(&self.opts.paths, &format!("page {id}: steps not saved: {e}"));
                }
                continue;
            }
            let reply = self.pg.page_replies.remove(&card);
            let mut qs = pages.store.questions(&id);
            if let Some(q) = qs.iter_mut().find(|q| q.card == card) {
                match &reply {
                    Some(r) => q.reply = Some(r.clone()),
                    // closed with no answer (withdrawn, closed by the user,
                    // a card this hub never had: a copied page, pm's 409):
                    // the question stays open on the page, a pick there
                    // still goes (as a pick note); a republish opens no
                    // card until its words or options change (m_6091)
                    None => {
                        q.card = 0;
                        q.dismissed = true;
                    }
                }
            }
            if let Err(e) = pages.locked(|s| s.save_questions(&id, &qs)) {
                log_line(&self.opts.paths, &format!("page {id}: questions not saved: {e}"));
            }
            if let Some(r) = reply {
                pages.push(&id, "answered", &json!({"block": block, "reply": r}));
            }
        }
        pages.push("", "pages", &json!({"pages": pages.list_json()}));
    }

    /// What reaches main from a page (notes, starts, picks, ticks) carries
    /// this line: main answers on the page, and the capsule never shows
    /// that turn's words (ambient-lead m_5724, pm's 27; ambient-core
    /// reads the same marker).
    pub(super) fn page_hint(to: &str, text: String) -> String {
        if to != MAIN {
            return text;
        }
        format!("{text}

{PAGE_HINT}")
    }
}
