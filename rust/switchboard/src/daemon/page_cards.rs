//! The pages' cards (docs/ambient-pages.md §2.4-§2.6): the links from
//! cards to pages, a page's questions and step cards and their answers,
//! drafts batches and ticks, and the hint main's page replies carry. A
//! question or a drawn pick is a page card its page's agent asks (sb-core
//! page_card, notif-artifact); its card carries the page's chip (title,
//! version) and the pick's why lines; the page hears its asks (`asks`,
//! `answered` with the door and the time).
//! Moved out of daemon/pages.rs unchanged (architect's 1,000-line rule).

use super::*;
use crate::card_link::{CardLink, PageLink};
use crate::snapshot::Snapshot;

// a plan's own card (plan-ok): its own file, beside the drafts' batch card
#[path = "page_plan.rs"]
mod page_plan;

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
    /// The snapshot's pages (docs/ambient-pages.md §2.3), newest first,
    /// and his late promises (checklist rows of his past data-due): a
    /// count for the morning page and the menu bar, never a card. Its one
    /// writer (daemon.rs `snapshot`).
    pub(super) fn snapshot_pages(&self, snap: &mut Snapshot) {
        if let Some(p) = &self.pg.pages {
            snap.pages = p.list_rows();
            snap.overdue = u32::try_from(p.overdue()).unwrap_or(u32::MAX);
        }
    }

    /// Each open card's links (its one writer): an agent's card linked
    /// with `sb notify --page`, its options' artifacts with `--open`
    /// (card_link.rs); a page question's or step's card its block (fn + ⏎
    /// opens it), a drafts batch or a plan its page; every linked card
    /// says what it is about in one link, pages included (card-links,
    /// architect m_27431).
    pub(super) fn snapshot_card_links(&mut self, snap: &mut Snapshot) {
        let Some(p) = self.pg.pages.clone() else { return };
        let qs = self.page_cards().clone();
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let n = self.pg.card_links.len();
        self.pg.card_links.retain(|c, _| open.contains(c));
        if self.pg.card_links.len() != n {
            self.save_card_links();
        }
        for c in &mut snap.cards {
            if let Some(link) = self.pg.card_links.get(&c.id) {
                if link.page.is_some() {
                    c.page = link.page.clone();
                }
                if link.artifact.is_some() {
                    c.artifact = link.artifact.clone();
                }
                if !link.opens.is_empty() {
                    c.opens = link.opens.clone();
                }
            }
            // what it is about, its card_opened's (inbox-answered): an
            // older client's `page` derived from it
            let about = self.hub.st.cards.get(&c.id).and_then(|k| k.about.clone()).and_then(|v| serde_json::from_value::<bise_proto::rows::ArtLink>(v).ok());
            if let Some(a) = about {
                if c.page.is_none() {
                    c.page = crate::card_link::page_of_art(&a);
                }
                if c.artifact.is_none() {
                    c.artifact = Some(a);
                }
            }
            let Some((id, block)) = qs.get(&c.id) else { continue };
            // drafts batched in one card: the page opens at the first
            // (ambient-lead m_5977)
            if let Some((b, anchor)) = block.strip_prefix("drafts:").and_then(|k| k.split_once(':')) {
                let url = format!("{}#{}", p.url(id), anchor);
                // its 1 review opens the page (an artifact) at the first,
                // in the app, nothing sent (notif-open)
                c.opens = BTreeMap::from([(1, bise_proto::rows::OptAct::artifact(id, Some(anchor), Some(&url)))]);
                c.page = Some(PageLink { id: id.clone(), block: Some(b.into()), item: Some(anchor.into()), drafts: true, url, ..PageLink::default() });
                // its fields for the capsule's words (amb-web m_6061):
                // {count, title, what, names}
                if let Some(s) = p.store.steps(id).into_iter().find(|s| s.card == c.id && !s.info.is_null()) {
                    c.batch = serde_json::from_value(s.info).ok();
                }
                continue;
            }
            // a plan's own card (plan-ok): the page at the plan; its last
            // option ('open the plan and its alternatives') opens it in
            // the app, nothing sent (notif-open)
            if let Some(b) = block.strip_prefix("plan:") {
                let url = format!("{}#{}", p.url(id), b);
                let n = bise_proto::rows::split_choices(&c.text).1.len() as u32;
                if n > 0 {
                    c.opens = BTreeMap::from([(n, bise_proto::rows::OptAct::artifact(id, Some(b), Some(&url)))]);
                }
                // its chip, '↗ lean installs · page · v1' (designer m_26845)
                c.page = Some(PageLink { id: id.clone(), block: Some(b.into()), url, ..Self::page_chip(&p, id) });
                continue;
            }
            // a step of his: its row (roadmap D); the card's chip, '↗
            // <title> · page · v<n>' (notif-artifact)
            c.page = Some(match block.strip_prefix("row:").and_then(|k| k.split_once('/')) {
                Some((b, item)) => PageLink { id: id.clone(), block: Some(b.into()), item: Some(item.into()), url: format!("{}#{}", p.url(id), item), ..Self::page_chip(&p, id) },
                None => PageLink { id: id.clone(), block: Some(block.clone()), url: format!("{}#{}", p.url(id), block), ..Self::page_chip(&p, id) },
            });
            // a drawn pick: its options' why lines, the agent's own; past
            // 4 options its last one opens the page for the rest
            if let Some(q) = p.store.questions(id).into_iter().find(|q| q.block == *block && q.pick.is_some()) {
                if let Some((n, _)) = q.more() {
                    let url = format!("{}#{}", p.url(id), block);
                    c.opens = BTreeMap::from([(n as u32, bise_proto::rows::OptAct::artifact(id, Some(block), Some(&url)))]);
                }
                c.pick = q.pick;
            }
        }
        for c in snap.cards.iter_mut().filter(|c| c.artifact.is_none()) {
            c.artifact = c.page.as_ref().and_then(crate::card_link::art_of_page);
        }
    }

    /// Page `id` at `block` (and `item`) as the artifact its card is
    /// about: an ArtLink's JSON, stored on its card_opened as is (none
    /// when there are no pages).
    pub(super) fn page_about(&self, id: &str, block: &str, item: Option<&str>) -> Option<serde_json::Value> {
        let p = self.pg.pages.as_deref()?;
        let at = item.unwrap_or(block);
        let page = PageLink { id: id.into(), block: Some(block.into()), item: item.map(Into::into), url: format!("{}#{}", p.url(id), at), ..Self::page_chip(p, id) };
        crate::card_link::art_of_page(&page).and_then(|a| serde_json::to_value(a).ok())
    }

    /// A page's chip on its card: its title and version (none when it is
    /// gone).
    fn page_chip(p: &crate::pages::Pages, id: &str) -> PageLink {
        let meta = p.store.meta(id);
        PageLink { title: meta.as_ref().map(|m| m.title.clone()), v: meta.as_ref().map(|m| m.version()), ..PageLink::default() }
    }

    /// `sb notify --page|--artifact <id>[#<item>]` and `--open
    /// <n>=<artifact>[#<anchor>]`: a page, an artifact or an anchor that
    /// isn't there is refused before the card opens (pm's B m_6008).
    /// Ok(None): no link; Ok(Some): the card's link `{page?, artifact?,
    /// opens?}` (card_link.rs, card-links); Err: the refusal's words.
    pub(super) fn card_page_link(&self, req: &crate::core::AgentReq) -> Result<Option<CardLink>, String> {
        let crate::core::AgentReq::Card { page, artifact, open, .. } = req else {
            return Ok(None);
        };
        let store = crate::artifacts::Store::new(&self.opts.paths.state);
        let art = |id: &str| store.get(id);
        let mut link = CardLink::default();
        if let Some(spec) = artifact.as_deref().or(page.as_deref()) {
            let (page, a) = crate::card_link::artifact(self.pg.pages.as_deref(), &art, spec)?;
            link.page = page;
            link.artifact = Some(a);
        }
        let specs: Vec<&str> = open.iter().map(String::as_str).collect();
        if !specs.is_empty() {
            link.opens = specs.iter().map(|s| crate::card_link::open_act(self.pg.pages.as_deref(), &art, s)).collect::<Result<_, _>>()?;
        }
        Ok((link != CardLink::default()).then_some(link))
    }

    /// The card that step opened (an open card not in `before`) gets its
    /// link (`{page?, opens?}`), saved, and the clients see it at once.
    pub(super) fn link_new_card(&mut self, before: &[u64], l: CardLink) {
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
    pub(super) fn page_voice(&self, page: &str, phase: &str, text: &str) {
        if let Some(p) = &self.pg.pages {
            if p.store.meta(page).is_some() {
                p.push(page, "voice", &json!({"phase": phase, "text": text}));
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
        let mut new = crate::pages::questions::of_fragment(html);
        // a drawn pick with no data-ask asks with the page's title
        let title = pages.store.meta(id).map(|m| m.title).unwrap_or_default();
        for q in new.iter_mut().filter(|q| q.text.is_empty()) {
            q.text = title.clone();
        }
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
        // a page-only question (data-notify="none") never opens one; a
        // dismissed one not again until its words change (m_6091)
        for q in qs.iter_mut().filter(|q| q.card == 0 && q.reply.is_none() && !q.page_only && !q.dismissed) {
            let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
            // a page card (notif-artifact, architect m_26569): asked by the
            // page's agent (main when it is gone: page_asker), a question or
            // a drawn pick alike; his answer is the hub's own page path
            // (page_card_answer: the pick goes to the page's agent)
            let about = self.page_about(id, &q.block, None);
            self.step(Input::PageCard { agents: vec![agent.to_string()], kind: "question".into(), text: q.card_text(), key: format!("question:{}", q.block), about });
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
        Self::push_asks(&pages, id);
        pages.push("", "pages", &json!({"pages": pages.list_json()}));
    }

    /// The page's asks as the kit draws them (notif-artifact): '? waits
    /// in your inbox' while a notification is open, its answer after.
    fn push_asks(pages: &crate::pages::Pages, id: &str) {
        let asks: Vec<Value> = pages.store.questions(id).iter().map(|q| q.ask_json(true)).collect();
        pages.push(id, "asks", &json!({"asks": asks}));
    }

    /// A page question answered with no open card (amb-kit m_5336,
    /// ambient-lead m_5343): the answer is kept, the page hears
    /// `answered`, and the pick is a `pick` note on that block, sent to
    /// the page's agent (main when it is gone) like any notes.
    /// `via`: the door that took it (`inbox` or `page`), kept with it.
    /// `parts`: his answer's typed parts (none: a plain one), kept on the
    /// pick note (card-image-parts).
    pub(super) fn page_answer_without_card(&mut self, id: &str, block: &str, reply: &str, parts: &[bise_proto::parts::Part], via: &str) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(meta) = pages.store.meta(id) else { return };
        let mut qs = pages.store.questions(id);
        let Some(q) = qs.iter_mut().find(|q| q.block == block && q.reply.is_none()) else { return };
        q.reply = Some(reply.to_string());
        q.via = Some(via.to_string());
        q.at_ms = Some(now_ms());
        let answered = q.answered_json();
        if let Err(e) = pages.locked(|s| s.save_questions(id, &qs)) {
            log_line(&self.opts.paths, &format!("page {id}: questions not saved: {e}"));
        }
        pages.push(id, "answered", &answered);
        let note = match pages.locked(|s| s.add_pick(id, block, reply, parts, meta.version(), now_ms())) {
            Ok(n) => n,
            Err(e) => return log_line(&self.opts.paths, &format!("page {id}: the pick not saved: {e}")),
        };
        pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
        let text = crate::pages::store::notes_message(&meta, &pages.url(id), std::slice::from_ref(&note));
        self.page_msg(crate::pages::PageMsg::Sent { target: crate::pages::store::Target::Page(id.to_string()), agent: meta.agent.clone(), text, notes: vec![note] });
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
        // a plan's own card (plan-ok, page_plan.rs): no batch card beside it
        let planned = self.page_plan(id);
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(meta) = pages.store.meta(id) else { return };
        let html = pages.store.html(id, meta.version()).unwrap_or_default();
        let rows = crate::pages::checklist::judged(&html, &pages.store.questions(id));
        let mut steps = pages.store.steps(id);
        let notes = pages.store.notes(id);
        // drafts batch on any page but a promises or meeting page (never a
        // card there): a feedback page's replies have no plan (pm's C fail
        // 42: 3 Slack replies and no card)
        let drafts = match crate::pages::drafts::carded(id) && !planned {
            // (a draft whose item's agent is still at work waits: m_6168)
            true => crate::pages::drafts::ready(&html, crate::pages::drafts::pending(&html, &notes, &pages.store.questions(id), &meta.went_blocks()), &|a| pages.held(id, a, meta.version())),
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
        for s in steps.iter_mut().filter(|s| s.reply.is_none() && s.block != crate::pages::checklist::BATCH && !s.block.starts_with("plan:")) {
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
                by: crate::peer::Sender::hub(),
                // (the page's title first: where this step is, with no page
                // open; notif-words)
                req: crate::core::AgentReq::card_about(crate::pages::checklist::with_title(&meta.title, &s.card_text()), self.page_about(id, &r.block, Some(&r.item))),
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
    pub(super) fn open_page_card(&mut self, id: &str, text: String, key: String, about: Option<serde_json::Value>) -> Option<u64> {
        let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        self.step(Input::Agent { token: 0, from: MAIN.to_string(), req: crate::core::AgentReq::card_about(text, about), by: crate::peer::Sender::hub() });
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
        let (text, choices) = (crate::pages::drafts::batch_text(drafts, title), crate::pages::drafts::choices(drafts));
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
                // its words no longer say what waits (the next change
                // after his go on one, a draft sent from the page): a new
                // one says it (notif-words)
                (false, true, true) if s.text != text || s.batch_choices() != choices => ("replaced", "replaced"),
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
        // (an account change of a chain he started, his go on the one
        // before, is asked alone too: the last of a plan's changes)
        let going = drafts.iter().any(|d| d.kind == "action")
            && steps.iter().any(|s| s.block == BATCH && s.reply.as_deref().is_some_and(|r| !matches!(r, "" | "done" | "replaced")));
        let new = drafts.len() >= 2 || (replaced && !drafts.is_empty()) || going;
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
            text,
            batch: keys,
            info: crate::pages::drafts::info(drafts, title),
            choices,
            ..Step::default()
        };
        let about = self.page_about(id, &first.block, Some(&first.anchor()));
        match self.open_page_card(id, s.card_text(), format!("drafts:{}:{}", first.block, first.anchor()), about) {
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
    /// (main when it is gone), the card closes. Review (1): the card
    /// stays, the answering client hears where the page is (the capsule
    /// and the window open it themselves). Other words: they go to the
    /// card's asker (the page's agent, main when it is gone) as his
    /// message, the card stays, and the client hears where they went;
    /// never silently nothing (his 'Ou sont les drafts?' on a docked
    /// card, ambient-lead m_21516).
    pub(super) fn page_drafts_answer(&mut self, client: ClientId, id: &str, card: u64, reply: &str) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        use crate::pages::drafts::BatchAnswer;
        let choices = pages.store.steps(id).into_iter().find(|s| s.card == card && s.reply.is_none()).map(|s| s.batch_choices()).unwrap_or_default();
        let pick = match crate::pages::drafts::batch_answer(reply, &choices) {
            BatchAnswer::Empty => return true,
            // option 1 does what it says: the page, never a message; the card stays
            BatchAnswer::Review => {
                let text = format!("the drafts of {} are on its page: {}", meta.title, pages.url(id));
                log_line(&self.opts.paths, &format!("page {id}: review on batch card #{card}"));
                self.notice_out(client, Some("answer"), &text);
                return true;
            }
            // words that are no option (ambient-lead m_21516: they were swallowed):
            // the asker gets them as his page note, he gets one line where he typed
            BatchAnswer::Words(words) => {
                let to = if self.page_agent_gone(&meta.agent) { MAIN.to_string() } else { meta.agent.clone() };
                let line = pages.store.steps(id).into_iter().find(|s| s.card == card).map(|s| s.text.lines().next().unwrap_or_default().to_string()).unwrap_or_default();
                let asked = self.hub.st.open_cards().find(|c| c.id == card).map_or_else(|| line.clone(), |c| c.text.clone());
                let (text, part) = crate::pages::store::batch_words(&meta, &pages.url(id), card, &line, &asked, &to, words);
                let parts = serde_json::to_string(&[part]).unwrap_or_default();
                log_line(&self.opts.paths, &format!("page {id}: words on batch card #{card} → {to}: {words:?}"));
                // client 0, as every page message: a real id would move that client's focus to `to` (focus_of)
                self.step(Input::ClientInput { client: 0, focus: to.clone(), text, queued: false, parts, from: String::new() });
                let who = if to == MAIN { "bise".to_string() } else { to };
                self.notice_out(client, Some("answer"), &format!("sent to {who}: \"{words}\". the drafts still wait for you."));
                return true;
            }
            BatchAnswer::Choice(i) => i,
        };
        // the drafts of the option he picked, only them: an account
        // change goes alone, the next one gets its own (notif-words)
        let Some(choice) = choices.get(pick) else { return false };
        let mut steps = pages.store.steps(id);
        let sent = self.page_approve_drafts(id, Some(&choice.keys));
        for s in steps.iter_mut().filter(|s| s.card == card && s.reply.is_none()) {
            s.reply = Some(choice.label.clone());
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
        let drafts = crate::pages::drafts::ready(&html, crate::pages::drafts::pending(&html, &pages.store.notes(id), &pages.store.questions(id), &meta.went_blocks()), &|a| pages.held(id, a, meta.version()));
        let mut sent = Vec::new();
        for d in drafts.iter().filter(|d| keys.is_none_or(|k| k.contains(&d.key()))) {
            match pages.locked(|s| s.add_sent(id, &d.block, d.item.as_deref(), "approve", "send", &[], meta.version(), now_ms())) {
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
        self.page_msg(crate::pages::PageMsg::Sent { target: crate::pages::store::Target::Page(id.to_string()), agent: meta.agent.clone(), text: msg, notes: sent.to_vec() });
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
        // an old one's send acts on the drafts that wait now, never on an
        // account change: those go one at a time, each on its own go
        // (notif-words)
        let closed = || format!("that notification is closed: nothing waits on {} now.", meta.title);
        let changed = |s: &crate::pages::checklist::Step| format!("that notification changed: {}", s.text.lines().next().unwrap_or_default());
        let say = match (now, crate::pages::drafts::is_send_all(reply)) {
            (Some(s), true) => match s.batch_choices().into_iter().find(|c| crate::pages::drafts::is_all_label(&c.label)) {
                Some(c) => {
                    self.page_drafts_answer(client, &id, s.card, &c.label);
                    None
                }
                None => Some(changed(&s)),
            },
            (None, true) => {
                let html = pages.store.html(&id, meta.version()).unwrap_or_default();
                let sends: Vec<String> = crate::pages::drafts::pending(&html, &pages.store.notes(&id), &pages.store.questions(&id), &meta.went_blocks())
                    .iter()
                    .filter(|d| d.kind != "action")
                    .map(|d| d.key())
                    .collect();
                let sent = if sends.is_empty() { Vec::new() } else { self.page_approve_drafts(&id, Some(&sends)) };
                if sent.is_empty() {
                    Some(closed())
                } else {
                    self.page_send_approves(&id, &meta, &sent);
                    self.page_steps(&id);
                    None
                }
            }
            (Some(s), false) => Some(changed(&s)),
            (None, false) => Some(closed()),
        };
        log_line(&self.opts.paths, &format!("page {id}: an answer on old batch card #{card}: {reply:?} → {}", say.as_deref().unwrap_or("sent the current drafts")));
        if let Some(text) = say {
            self.notice_out(client, None, &text);
        }
        true
    }

    /// A step ticked from its card or by voice (`sb page tick`): a sent
    /// tick note on its row, the page hears `ticked`, the page's agent
    /// gets the notes message (main when it is gone), its card closes and
    /// the next step's turn comes. `text`: the user's words when they are
    /// not a plain "done". False: no such row.
    /// `parts`: his words' typed parts, kept on a note (none: plain).
    pub(super) fn page_tick(&mut self, id: &str, item: &str, text: &str, parts: &[bise_proto::parts::Part]) -> bool {
        let Some(pages) = self.pg.pages.clone() else { return false };
        let Some(meta) = pages.store.meta(id) else { return false };
        let rows = pages.store.html(id, meta.version()).map(|h| crate::pages::checklist::rows_of(&h)).unwrap_or_default();
        let Some(row) = rows.iter().find(|r| r.item == item) else { return false };
        let go = crate::pages::checklist::is_done_word(text) || text.trim().eq_ignore_ascii_case("send");
        let (kind, words, parts) = match (go, row.drafted) {
            // a drafted step's "send": an approve of its draft, so the
            // page's agent sends it (pm's D fail 30)
            (true, true) => ("approve", "send", &[][..]),
            (true, false) => ("tick", "done", &[][..]),
            (false, _) => ("note", text.trim(), parts),
        };
        let (block, on) = match (kind, row.draft.as_deref()) {
            ("approve", Some(d)) => (d.to_string(), None),
            _ => (row.block.clone(), Some(item)),
        };
        let note = match pages.locked(|s| s.add_sent(id, &block, on, kind, words, parts, meta.version(), now_ms())) {
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
        let msg = crate::pages::store::notes_message(&meta, &pages.url(id), std::slice::from_ref(&note));
        self.page_msg(crate::pages::PageMsg::Sent { target: crate::pages::store::Target::Page(id.to_string()), agent: meta.agent.clone(), text: msg, notes: vec![note] });
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
    /// `parts`: his answer's typed parts (card/answer's; none: a line).
    pub(super) fn page_first(&mut self, client: ClientId, line: &str, parts: &[bise_proto::parts::Part]) -> Page {
        if self.page_step_answer(client, line, parts) {
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
    fn page_step_answer(&mut self, client: ClientId, text: &str, parts: &[bise_proto::parts::Part]) -> bool {
        let Some(rest) = text.trim().strip_prefix("/answer ") else { return false };
        let Some((n, reply)) = rest.trim().split_once(' ') else { return false };
        let Ok(card) = n.trim_start_matches('#').parse::<u64>() else { return false };
        let Some((id, key)) = self.page_cards().get(&card).cloned() else { return self.page_old_batch_answer(client, card, reply) };
        if key.starts_with("drafts:") {
            return self.page_drafts_answer(client, &id, card, reply);
        }
        if key.starts_with("plan:") {
            return self.page_plan_answer(client, &id, card, reply);
        }
        match key.strip_prefix("row:").and_then(|k| k.split_once('/')).map(|(_, i)| i.to_string()) {
            Some(item) => self.page_tick(&id, &item, reply, parts),
            None => self.page_question_answer(&id, &key, card, reply, parts, "inbox"),
        }
    }

    /// A page question's card answered (from the TUI, the capsule or the
    /// page): the card closes, and the answer takes the page's path: the
    /// page hears `answered`, the page's agent gets it as a pick note
    /// (main when it is gone). sb-core's question path would send it to
    /// main, the card's asker (pm's 25: a task's page); sb-core's
    /// `page_answered` closes it and tells the asker in one message when
    /// the pick went to another agent (wake-deliver).
    pub(super) fn page_question_answer(&mut self, id: &str, block: &str, card: u64, reply: &str, parts: &[bise_proto::parts::Part], via: &str) -> bool {
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
        // the card closes; sb-core tells its asker too when that is not
        // the agent the pick goes to (wake-deliver: main never heard #513)
        let agent = pages.store.meta(id).map(|m| m.agent).unwrap_or_default();
        let to = if self.page_agent_gone(&agent) { MAIN.to_string() } else { agent };
        self.step(Input::PageAnswered { card, agent: to, page: id.to_string(), text: words.clone(), block: block.to_string() });
        self.page_answer_without_card(id, block, &words, parts, via);
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
            if block.starts_with("row:") || block.starts_with("plan:") {
                // a step's card closed another way (the user closed it):
                // it stays closed; a tick still ticks the row (a plan's:
                // not asked again for that version, its page's button
                // still starts it)
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
                    Some(r) => {
                        q.reply = Some(r.clone());
                        q.via = Some("inbox".into());
                        q.at_ms = Some(now_ms());
                    }
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
            match (reply, qs.iter().find(|q| q.card == card)) {
                (Some(_), Some(q)) => pages.push(&id, "answered", &q.answered_json()),
                // closed unanswered: the page no longer says it waits in the inbox
                _ => Self::push_asks(&pages, &id),
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
