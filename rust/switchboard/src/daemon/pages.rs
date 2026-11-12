//! The hub's side of the pages (docs/ambient-pages.md §2.2-§2.6; the store
//! and the server: crate::pages): `sb page …` and `sb taste`/`sb people`,
//! the page server's notes, the data-agent and watch pushes, page
//! questions and step cards and their answers, the first run's start-here
//! page. Moved out of daemon.rs unchanged (desktop plan S0b). The cards
//! (questions, steps, drafts, ticks, answers) are in daemon/page_cards.rs.

use super::*;

/// The pages' part of the Shell (architect's review 6, m_9155): the
/// server and what the hub keeps about pages, one field of the Shell
/// (`Shell::pg`) instead of ten.
#[derive(Default)]
pub(super) struct PageState {
    /// The pages and their server (docs/ambient-pages.md); None: the
    /// server could not start (said in the hub log).
    pub(super) pages: Option<std::sync::Arc<crate::pages::Pages>>,
    /// Agents a page's notes were sent to: the pages they update, and
    /// whether a turn of theirs has the notes yet (started or steered
    /// since). Its end without a publish puts the pages back to ready.
    pub(super) page_turns: BTreeMap<String, (Vec<String>, bool)>,
    /// Page messages sent to an agent that has not taken them yet (no
    /// turn started or steered since): agent -> (first sent at, [(page,
    /// text)]). Not taken in PAGE_WAIT_MS: they go to main; main's not
    /// taken: a warn in its feed. Never silently lost (pm's D fail 37).
    pub(super) page_waits: BTreeMap<String, (u64, Vec<(String, String)>)>,
    /// the agents each page's latest version names (`data-agent`):
    /// page id -> names (None: not read from the store yet)
    pub(super) page_agents: Option<BTreeMap<String, Vec<String>>>,
    /// the last `agent` frame pushed on each page, per agent
    pub(super) page_agent_sent: BTreeMap<(String, String), Value>,
    /// each agent's last tool intent (an `agent` frame's note when it
    /// declared none)
    pub(super) last_intent: BTreeMap<String, String>,
    /// each watched page's last `watch` (meta.watch)
    pub(super) page_watch_sent: BTreeMap<String, Value>,
    /// The open cards of the pages' question blocks (§4.2): card →
    /// (page, block); None until read from the store.
    pub(super) page_cards: Option<BTreeMap<u64, (String, String)>>,
    /// The answers given to those cards, until the hub closes them.
    pub(super) page_replies: BTreeMap<u64, String>,
    /// `sb card --page`: an agent's card → its page link (card_link.rs),
    /// shown on the card while it is open; its answer goes to the agent
    /// as usual (unlike `page_cards`, whose answers take the page's path)
    pub(super) card_links: BTreeMap<u64, crate::card_link::CardLink>,
}

impl PageState {
    /// At the hub's start: the cards survive a restart, so do their links
    /// (lead m_6039); the server starts with [`Shell::start_pages`].
    pub(super) fn load(state: &Path) -> PageState {
        PageState { card_links: crate::card_link::load(&crate::card_link::file(state)), ..Default::default() }
    }
}

impl Shell {
    /// The page server (docs/ambient-pages.md §2.4): 127.0.0.1, a stable
    /// port per workspace; its notes come back to the loop.
    pub(super) fn start_pages(&mut self, paths: &Paths, tx: &Sender<Msg>) {
        match crate::pages::start(&paths.state, &crate::paths::workspace_id(&paths.workspace), &self.opts.app_root, self.holds.clone()) {
            Ok((p, moved_from)) => {
                if let Some(old) = moved_from {
                    log_line(paths, &format!("pages: port {old} stayed busy {} s, moved to {}: links to {old} are dead", crate::pages::KEPT_WAIT.as_secs(), p.port));
                }
                let tx = std::sync::Mutex::new(tx.clone());
                p.connect_hub(Box::new(move |m| {
                    if let Ok(t) = tx.lock() {
                        let _ = t.send(Msg::Page(m));
                    }
                }));
                log_line(paths, &format!("pages at {}", p.base()));
                self.pg.pages = Some(p);
            }
            Err(e) => log_line(paths, &format!("no page server: {}", e)),
        }
    }

    // ---- pages (docs/ambient-pages.md §2.2-§2.6; the store and the server: pages/) ----

    /// An agent that is gone for its page's notes: dropped, archived,
    /// stopped or failed (main gets them then).
    pub(super) fn page_agent_gone(&self, name: &str) -> bool {
        match self.hub.st.agents.get(name) {
            Some(a) => matches!(a.status().as_str(), "archived" | "stopped" | "failed"),
            None => true,
        }
    }

    /// `sb page publish|list|notes`: the answer for the CLI.
    /// `sb taste` and `sb people` (ambient-lead m_5630): the two files of
    /// the home workspace bise keeps about the user, one `- ` line each;
    /// after a change the `about-you` page, when there is one, is redrawn
    /// from them and republished by its own agent (no model turn).
    pub(super) fn keeps_op(&mut self, cmd: &str, k: &bise_proto::sb::Keep) -> Value {
        use crate::keeps;
        use bise_proto::sb::Keep;
        let kept = if cmd == "taste" { keeps::Kept::Taste } else { keeps::Kept::People };
        let path = match keeps::file(kept, &self.opts.home) {
            Ok(p) => p,
            Err(e) => return json!({"ok": false, "error": e}),
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let (what, noun) = if cmd == "taste" { ("rule", "rules") } else { ("person", "people") };
        let count = |t: &str| {
            let n = keeps::items(t).len();
            format!("{n} {}", if n == 1 { what } else { noun })
        };
        let changed = match (cmd, k) {
            (_, Keep::List) => {
                let items = keeps::items(&text);
                let body = if items.is_empty() {
                    format!("nothing kept yet ({}): sb {cmd} {}", path.display(), if cmd == "taste" { "add \"<rule>\"" } else { "set <name> \"<who>\"" })
                } else {
                    let lines: Vec<String> = items.iter().enumerate().map(|(k, i)| format!("{}. {i}", k + 1)).collect();
                    format!("{} ({}):\n{}", count(&text), path.display(), lines.join("\n"))
                };
                return json!({"ok": true, "text": body, "items": items});
            }
            ("taste", Keep::Add { rule, source }) => keeps::taste_add(&text, rule, Some(source.as_str()).filter(|f| !f.trim().is_empty())).map(|t| {
                let line = format!("kept: {}", rule.split_whitespace().collect::<Vec<_>>().join(" "));
                (t, line)
            }),
            ("taste", Keep::Remove { which, .. }) => keeps::taste_remove(&text, which).map(|(t, gone)| (t, format!("removed: {gone}"))),
            ("people", Keep::Set { name, who }) => keeps::people_set(&text, name, who).map(|t| (t, format!("kept: {}", name.trim()))),
            ("people", Keep::Remove { name, .. }) => keeps::people_remove(&text, name).map(|t| (t, format!("removed: {}", name.trim()))),
            _ => Err(format!("sb {cmd}: unknown step")),
        };
        let (new, line) = match changed {
            Ok(x) => x,
            Err(e) => return json!({"ok": false, "error": e}),
        };
        if let Err(e) = keeps::write(&path, &new) {
            return json!({"ok": false, "error": e});
        }
        let mut out = format!("{line} ({} in {})", count(&new), path.display());
        if let Some(v) = self.about_you_redraw() {
            out.push_str(&format!("; about-you is v{v} now"));
        }
        json!({"ok": true, "text": out})
    }

    /// The home workspace's first run: amb-kit's `start here` page,
    /// published once as main (first_run.rs); the marker keeps it from
    /// ever coming back.
    pub(super) fn start_here(&mut self, paths: &crate::paths::Paths) {
        use crate::first_run;
        let Some(pages) = self.pg.pages.clone() else { return };
        let Some(html) = first_run::due(&paths.state, &paths.workspace, &crate::paths::home_workspace(), &self.opts.app_root) else {
            return;
        };
        let p = crate::pages::store::Publish {
            agent: "main".into(),
            id: Some(first_run::PAGE_ID.into()),
            title: Some(first_run::TITLE.into()),
            html,
            ..Default::default()
        };
        match pages.publish(&p, now_ms(), &|_| false) {
            Ok((meta, _)) => {
                let _ = first_run::mark(&paths.state, meta.version());
                log_line(paths, &format!("first run: {} v{} published", meta.id, meta.version()));
            }
            Err(lines) => log_line(paths, &format!("first run: start-here did not pass: {}", lines.join("; "))),
        }
    }

    /// Republish the `about-you` page from the kept files, as its agent;
    /// None when there is no such page (or it did not pass).
    pub(super) fn about_you_redraw(&mut self) -> Option<u64> {
        use crate::keeps;
        let pages = self.pg.pages.clone()?;
        let meta = pages.store.meta("about-you")?;
        let home = &self.opts.home;
        let read = |k: keeps::Kept| keeps::file(k, home).ok().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let p = crate::pages::store::Publish {
            agent: meta.agent.clone(),
            id: Some("about-you".into()),
            html: keeps::about_you(&read(keeps::Kept::Taste), &read(keeps::Kept::People)),
            ..Default::default()
        };
        let gone: Vec<String> = self.hub.st.agents.keys().filter(|n| self.page_agent_gone(n)).cloned().collect();
        let is_gone = |n: &str| gone.iter().any(|g| g == n) || !self.hub.st.agents.contains_key(n);
        match pages.publish(&p, now_ms(), &is_gone) {
            Ok((meta, _)) => {
                self.broadcast(&pages.page_ev(&meta));
                self.state_now();
                Some(meta.version())
            }
            Err(lines) => {
                eprintln!("about-you: the redraw did not pass: {}", lines.join("; "));
                None
            }
        }
    }

    pub(super) fn page_op(&mut self, from: &str, req: AgentReq) -> Value {
        let Some(pages) = self.pg.pages.clone() else {
            return json!({"ok": false, "error": "the page server is not running (see the hub log)"});
        };
        match req {
            AgentReq::PageList => json!({"ok": true, "pages": pages.list_json()}),
            AgentReq::PageNotes { id } => {
                match pages.store.meta(&id) {
                    Some(_) => json!({"ok": true, "id": id, "notes": pages.store.notes(&id)}),
                    None => json!({"ok": false, "error": format!("no page {id}")}),
                }
            }
            AgentReq::PageWaiting => {
                let items = pages.waiting();
                json!({"ok": true, "waiting": items, "lines": items.iter().map(|w| w.line()).collect::<Vec<_>>()})
            }
            AgentReq::PageTick { id, item, text } => {
                // a step of his he says is done (by voice, through main):
                // the same path as its card's answer (roadmap D)
                let words = text.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "done".into());
                if pages.store.meta(&id).is_none() {
                    return json!({"ok": false, "error": format!("no page {id}")});
                }
                if self.page_tick(&id, &item, &words, &[]) {
                    json!({"ok": true, "id": id, "item": item, "ticked": crate::pages::checklist::is_done_word(&words)})
                } else {
                    json!({"ok": false, "error": format!("{id} has no checklist row {item}")})
                }
            }
            AgentReq::PageStart { id, title, agent, ask } => {
                // the page shows within seconds of the ask; the first
                // publish is v1 (ambient-lead m_5000)
                let agent = agent.filter(|a| !a.is_empty()).unwrap_or_else(|| from.to_string());
                if agent != from && !self.hub.st.agents.contains_key(&agent) {
                    return json!({"ok": false, "error": format!("no agent {agent}")});
                }
                let gone: Vec<String> = self.hub.st.agents.keys().filter(|n| self.page_agent_gone(n)).cloned().collect();
                let is_gone = |n: &str| gone.iter().any(|g| g == n) || !self.hub.st.agents.contains_key(n);
                match pages.start(&id, title.as_deref(), ask.as_deref(), &agent, now_ms(), &is_gone) {
                    Ok(meta) => {
                        // its writer's turn: the caller's runs now; another
                        // agent's is the next one (its end without a
                        // publish puts the page back to ready)
                        let e = self.pg.page_turns.entry(agent.clone()).or_insert_with(|| (Vec::new(), false));
                        if !e.0.contains(&meta.id) {
                            e.0.push(meta.id.clone());
                        }
                        if agent == from {
                            e.1 = true;
                        }
                        self.broadcast(&pages.page_ev(&meta));
                        self.state_now();
                        json!({"ok": true, "id": meta.id, "version": 0, "url": pages.url(&meta.id), "state": "writing", "agent": agent})
                    }
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
            AgentReq::PagePublish(r) => {
                // --went (docs/ambient-roadmap.md A): the CLI read them
                let mut went = Vec::new();
                for w in r.went {
                    match serde_json::from_value::<crate::pages::store::Went>(w) {
                        Ok(w) => went.push(w),
                        Err(e) => return json!({"ok": false, "error": format!("--went: {e}")}),
                    }
                }
                // --taste: the rules of ~/bise/taste.md now (the frame says
                // 'following your taste · N rules'); no file: nothing
                let taste_file = self.opts.home.taste_file();
                let taste = r.taste.then(|| std::fs::read_to_string(&taste_file).ok().map(|t| crate::pages::store::taste_rules(&t)));
                if taste == Some(None) {
                    return json!({"ok": false, "error": format!("--taste: no {} (read it, or publish without --taste)", taste_file.display())});
                }
                let p = crate::pages::store::Publish {
                    taste: taste.flatten(),
                    agent: from.to_string(),
                    id: r.id.filter(|x| !x.is_empty()),
                    title: r.title.filter(|x| !x.is_empty()),
                    html: r.html,
                    done: r.done,
                    answers: r.answers.into_iter().collect(),
                    went,
                    // --public / --private (pages-ui): the static export and the user's mirror
                    public: r.public,
                };
                let gone: Vec<String> = self.hub.st.agents.keys().filter(|n| self.page_agent_gone(n)).cloned().collect();
                let is_gone = |n: &str| gone.iter().any(|g| g == n) || !self.hub.st.agents.contains_key(n);
                let was_public = p.id.as_deref().and_then(|i| pages.store.meta(i)).is_some_and(|m| m.public);
                match pages.publish(&p, now_ms(), &is_gone) {
                    Ok((meta, unknown)) => {
                        self.usage_count(bise_usage::CountKey::PagePublished);
                        // a public page changed (or was taken back): the static export, then the
                        // user's mirror if he set one (pages/mirror.rs); nothing else ever leaves
                        if meta.public || was_public {
                            crate::pages::mirror::spawn(pages.clone(), meta.id.clone(), self.opts.home.config_file());
                        }
                        // the notes it was updating for are answered
                        if let Some((ids, _)) = self.pg.page_turns.get_mut(from) {
                            ids.retain(|i| *i != meta.id);
                            if ids.is_empty() {
                                self.pg.page_turns.remove(from);
                            }
                        }
                        self.broadcast(&pages.page_ev(&meta));
                        if !p.went.is_empty() {
                            pages.push(&meta.id, "went", &json!({"went": meta.went}));
                        }
                        pages.push(&meta.id, "taste", &json!(meta.taste));
                        // its items' agents: their status from now on
                        let names = crate::pages::store::data_agents_of(&p.html);
                        self.page_agents_now().insert(meta.id.clone(), names);
                        self.page_agent_push();
                        self.page_watch_push();
                        // its question blocks: one card each (§4.2)
                        self.page_questions(&meta.id, &meta.agent, &p.html);
                        // its checklists: the step of his whose turn came
                        self.page_steps(&meta.id);
                        self.state_now();
                        let url = pages.url(&meta.id);
                        json!({"ok": true, "id": meta.id, "version": meta.version(), "url": url, "unknown_notes": unknown})
                    }
                    Err(lines) => json!({"ok": false, "error": lines.join("\n"), "lint": lines}),
                }
            }
            r => json!({"ok": false, "error": format!("sb {}: not a page request", r.tag())}),
        }
    }

    /// The page server's news.
    pub(super) fn page_msg(&mut self, m: crate::pages::PageMsg) {
        let Some(pages) = self.pg.pages.clone() else { return };
        match m {
            crate::pages::PageMsg::Notes { id } => {
                // a tick on the page (a draft too) closes its step's card
                self.page_steps(&id);
                self.state_now();
            }
            crate::pages::PageMsg::Opened { .. } => {
                self.state_now();
            }
            // an artifact's notes were written (any door): its document
            // views ask view/notes again (A5, architect m_61664)
            crate::pages::PageMsg::ArtifactNotes { id } => {
                let ev = bise_proto::hub::HubEv::ArtifactNotes { project: self.project(), artifact: id };
                self.proto_note_all(&ev);
            }
            // his edits in the viewer's grid: the file and the version (daemon/art.rs)
            crate::pages::PageMsg::Save { id, req, reply } => {
                let (status, body) = self.art_save(&pages, &id, req);
                reply.send(status, body);
            }
            // a view's request from the web host (pages/view_web.rs, C9):
            // the views dispatcher (daemon/views.rs), answered when it can
            crate::pages::PageMsg::View { method, params, reply } => {
                self.view_request(&method, params, Box::new(move |r| reply.send(200, crate::pages::view_web::answer(r))));
            }
            // a local site's door found its port free: its recipe again (daemon/revive.rs)
            crate::pages::PageMsg::Revive { id, reply } => {
                let (status, body) = self.revive.answer(&self.art_store(), &self.opts.paths.revive_dir(), &id);
                reply.send(status, body);
            }
            crate::pages::PageMsg::Mirror { id, error } => {
                // the last good public copy stays up; main hears it once (designer m_7509)
                self.feed_rec(MAIN, Feed::warn(format!("▲ couldn't publish {id} to the public mirror: {error}")));
            }
            // notes on an artifact (office-artifacts §9b): to its maker, main
            // when it is gone, as from the user, with their anchor parts
            crate::pages::PageMsg::Sent { target: crate::pages::store::Target::Artifact(id), agent, text, notes } => {
                if !text.is_empty() {
                    let to = if self.page_agent_gone(&agent) { MAIN.to_string() } else { agent };
                    let parts = crate::pages::artifact::notes::parts_json(&pages, &id, &notes);
                    self.step(Input::ClientInput { client: 0, focus: to, text, queued: false, parts, from: String::new() });
                }
                self.state_now();
            }
            crate::pages::PageMsg::Sent { target: crate::pages::store::Target::Page(id), agent, text, notes } => {
                // start notes (an item the user wants an agent on): main's,
                // never the page's agent's (ambient-lead m_5006)
                // a stop note (the frame's 'stop' of a watched page): the hub
                // ends the page's timers itself, no model turn
                let stops = pages.locked(|s| s.take_stops(&id)).unwrap_or_default();
                if !stops.is_empty() {
                    let ids: Vec<u64> = self.hub.timers().of_page(&id).iter().map(|t| t.id).collect();
                    for t in ids {
                        self.step(Input::EveryStop { id: t, why: format!(" with the stop on the page {id}") });
                    }
                    pages.push(&id, "notes", &json!({"notes": pages.store.notes(&id)}));
                    self.page_watch_push();
                }
                // a plan's start goes to its agent (plan-ok), the others to main
                self.page_plan_starts(&id);
                let starts = pages.locked(|s| s.route_starts(&id, &|_| true, MAIN)).unwrap_or_default();
                if let (false, Some(meta)) = (starts.is_empty(), pages.store.meta(&id)) {
                    let msg = crate::pages::store::start_message(&meta, &pages.url(&id), &starts);
                    let msg = Self::page_hint(MAIN, msg);
                    let parts = Self::page_parts(&pages, &meta, &starts, MAIN, &msg);
                    self.step(Input::ClientInput { client: 0, focus: MAIN.to_string(), text: msg, queued: false, parts, from: String::new() });
                    pages.push(&id, "notes", &json!({"notes": pages.store.notes(&id)}));
                }
                // the other notes: the page's agent, main when it is gone
                // (none: only start notes were sent)
                if !text.is_empty() {
                    let to = if self.page_agent_gone(&agent) { MAIN.to_string() } else { agent };
                    self.pg.page_turns.entry(to.clone()).or_insert_with(|| (Vec::new(), false)).0.push(id.clone());
                    if let Some(meta) = pages.set_state(&id, "updating") {
                        self.broadcast(&pages.page_ev(&meta));
                    }
                    // as from the user (client 0: no view of its own);
                    // watched until a turn of `to` takes it
                    let w = self.pg.page_waits.entry(to.clone()).or_insert_with(|| (now_ms(), Vec::new()));
                    w.1.push((id.clone(), text.clone()));
                    let text = Self::page_hint(&to, text);
                    let sent: Vec<_> = notes.into_iter().filter(|n| n.kind != "start" && n.kind != "stop").collect();
                    let parts = pages.store.meta(&id).map(|m| Self::page_parts(&pages, &m, &sent, &to, &text)).unwrap_or_default();
                    self.step(Input::ClientInput { client: 0, focus: to, text, queued: false, parts, from: String::new() });
                }
                self.state_now();
            }
            crate::pages::PageMsg::Answer { id, block, reply } => {
                // the same path as the capsule's and the TUI's answer
                if let Some(card) = self.page_card(&id, &block) {
                    self.page_question_answer(&id, &block, card, &reply, &[], "page");
                } else {
                    // no open card for it (it never opened, or the store
                    // was written another way): the loop still closes,
                    // the answer to the page's agent as from the user
                    self.page_answer_without_card(&id, &block, &reply, &[], "page");
                }
                self.page_answers();
            }
        }
    }

    /// The desktop door of the notes on an artifact (office-artifacts
    /// §9b): his message's anchor parts, taken at the door, become sent
    /// notes of their artifacts; their open viewers hear it.
    pub(super) fn artifact_notes_said(&mut self, parts_json: &str) {
        let Some(pages) = self.pg.pages.clone() else { return };
        if !parts_json.contains("\"anchor\"") {
            return;
        }
        let Ok(ps) = serde_json::from_str::<Vec<bise_proto::parts::Part>>(parts_json) else { return };
        for id in crate::pages::artifact::notes::record_said(&pages, &ps, now_ms()) {
            crate::pages::artifact::notes::push(&pages, &id);
        }
    }

    /// Version `v` of artifact `id` is here (`parts_in::said`'s check of
    /// an anchor part), or his words why not; no page server: none is.
    pub(super) fn artifact_known(&self) -> impl Fn(&str, u64) -> Result<u32, String> + 'static {
        let pages = self.pg.pages.clone();
        move |id: &str, v: u64| match pages.as_ref() {
            Some(p) => crate::pages::artifact::notes::known(p.store.dir.parent().unwrap_or(&p.store.dir), id, v),
            None => Err(format!("{id} has no version {v} here, so the note can't go on it")),
        }
    }

    /// The parts' JSON of a page's message `text` to `to` (its one
    /// `Part::Page`, architect m_20833): what the clients draw his words
    /// from, while the model reads `text` (render(parts) == text).
    fn page_parts(pages: &crate::pages::Pages, meta: &crate::pages::store::Meta, sent: &[crate::pages::store::Note], to: &str, text: &str) -> String {
        let part = crate::pages::store::page_part(meta, &pages.store.questions(&meta.id), sent, to, text);
        serde_json::to_string(&[part]).unwrap_or_default()
    }

    /// The agents each page's latest version names, read from the store
    /// the first time.
    pub(super) fn page_agents_now(&mut self) -> &mut BTreeMap<String, Vec<String>> {
        if self.pg.page_agents.is_none() {
            let m: BTreeMap<String, Vec<String>> = match &self.pg.pages {
                Some(p) => p.store.list().into_iter().map(|m| (m.id.clone(), p.store.data_agents(&m.id))).filter(|(_, n)| !n.is_empty()).collect(),
                None => BTreeMap::new(),
            };
            self.pg.page_agents = Some(m);
        }
        self.pg.page_agents.get_or_insert_with(BTreeMap::new)
    }

    /// An agent's live status on a page item (kit's `agent` frame, amb-kit
    /// m_5189): working|idle|done|blocked|failed, its status note else its
    /// last tool intent.
    pub(super) fn page_agent_frame(&self, name: &str) -> Value {
        use crate::model::Status;
        let a = self.hub.st.resolve(name).and_then(|n| self.hub.st.agents.get(&n));
        let Some(a) = a else { return json!({"name": name, "status": "done"}) };
        let status = match a.status() {
            Status::Starting | Status::Working | Status::Waiting => "working",
            Status::Idle => "idle",
            Status::Done | Status::Stopped | Status::Archived => "done",
            Status::Blocked => "blocked",
            Status::Failed => "failed",
        };
        let note = a
            .declared()
            .map(|d| d.note.trim().to_string())
            .filter(|n| !n.is_empty())
            .or_else(|| self.pg.last_intent.get(&a.name).cloned());
        let mut f = json!({"name": name, "status": status});
        if let Some(n) = note {
            f["note"] = json!(n);
        }
        f
    }

    /// Push the `agent` frames that changed, on every page naming them;
    /// each page keeps its last frames for a new stream.
    pub(super) fn page_agent_push(&mut self) {
        // the same typed state moves the artifacts' live words (art.rs)
        self.art_live_watch();
        let Some(pages) = self.pg.pages.clone() else { return };
        let list: Vec<(String, Vec<String>)> = self.page_agents_now().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        // the agents at work on page items: their drafts wait; when one
        // is done, its page's drafts are judged again (ambient-lead m_6168)
        let busy: std::collections::BTreeSet<String> = list
            .iter()
            .flat_map(|(_, names)| names.iter())
            .filter(|n| matches!(self.page_agent_frame(n)["status"].as_str(), Some("working" | "idle" | "blocked")))
            .cloned()
            .collect();
        if let Some(left) = pages.set_busy(busy) {
            // done now: its replies stay held until the page is published
            // again, after the fix (ambient-lead m_6187)
            for (id, names) in &list {
                let v = pages.store.meta(id).map_or(0, |m| m.version());
                for a in names.iter().filter(|a| left.contains(a)) {
                    pages.mark_done(id, a, v);
                }
            }
            for (id, _) in &list {
                self.page_steps(id);
            }
        }
        for (id, names) in list {
            let mut changed = false;
            for name in names {
                let f = self.page_agent_frame(&name);
                let key = (id.clone(), name.clone());
                if self.pg.page_agent_sent.get(&key) == Some(&f) {
                    continue;
                }
                pages.push(&id, "agent", &f);
                self.pg.page_agent_sent.insert(key, f);
                changed = true;
            }
            if changed {
                let frames: serde_json::Map<String, Value> =
                    self.pg.page_agent_sent.iter().filter(|((p, _), _)| *p == id).map(|((_, n), f)| (n.clone(), f.clone())).collect();
                let _ = pages.locked(|s| s.save_agent_frames(&id, &frames));
            }
        }
    }

    /// The page of `name` waiting for the user's review, by title: its
    /// latest version has a review block or an unanswered question, and
    /// no word of his on it since (an approve, a send, "put it in my
    /// drafts"). The approvals gate makes a draft call a card then.
    pub(crate) fn pending_review_of(&self, name: &str) -> Option<String> {
        self.pg.pages.as_ref()?.store.pending_review(name)
    }

    /// meta.watch of each page a timer watches (`sb every --page`), and
    /// SSE `watch` when it changes (null when its last timer ends):
    /// {timer, every, checked_ms, until_ms} (amb-kit m_5454).
    pub(super) fn page_watch_push(&mut self) {
        let Some(pages) = self.pg.pages.clone() else { return };
        let mut now: BTreeMap<String, Value> = BTreeMap::new();
        for t in self.hub.timers().map.values() {
            let Some(page) = &t.page else { continue };
            if now.contains_key(page) {
                continue;
            }
            let Some(m) = pages.store.meta(page) else { continue };
            let checked = m.versions.last().map_or(0, |v| v.at_ms).max(t.last_ms);
            now.insert(page.clone(), json!({"timer": t.id, "every": t.sched.label(), "checked_ms": checked, "until_ms": t.until_ms}));
        }
        let before = std::mem::take(&mut self.pg.page_watch_sent);
        for (id, w) in &now {
            if before.get(id) != Some(w) {
                let _ = pages.locked(|s| s.set_watch(id, Some(w.clone())));
                pages.push(id, "watch", w);
            }
        }
        for id in before.keys().filter(|k| !now.contains_key(*k)) {
            let _ = pages.locked(|s| s.set_watch(id, None));
            pages.push(id, "watch", &Value::Null);
        }
        self.pg.page_watch_sent = now;
    }

    /// A tool intent of an agent some page names: its item's note.
    pub(super) fn page_agent_line(&mut self, name: &str, line: &str) {
        if !self.page_agents_now().values().any(|ns| ns.iter().any(|n| n == name)) {
            return;
        }
        if let crate::wire::Wire::Intent(text) = crate::wire::parse(line) {
            self.pg.last_intent.insert(name.to_string(), crate::util::one_line(&text));
            self.page_agent_push();
        }
    }

    /// Page messages an agent has not taken (no turn started or steered)
    /// for PAGE_WAIT_MS: a task's go to main, which acts on the page;
    /// main's own are a warn in its feed. Never silently lost (pm's D
    /// fail 37: five answers on a task's page woke no one).
    pub(super) fn page_waits_check(&mut self) {
        use crate::pages::{untaken, Untaken};
        let now = now_ms();
        let late: Vec<(String, Untaken)> = self
            .pg
            .page_waits
            .iter()
            .map(|(a, (at, _))| (a.clone(), untaken(a, now.saturating_sub(*at))))
            .filter(|(_, u)| *u != Untaken::Wait)
            .collect();
        for (agent, u) in late {
            let Some((_, items)) = self.pg.page_waits.remove(&agent) else { continue };
            let pages: Vec<String> = items.iter().map(|(p, _)| p.clone()).collect();
            match u {
                Untaken::ToMain => {
                    let body: Vec<String> = items.into_iter().map(|(_, t)| t).collect();
                    let text = format!(
                        "@{agent} did not take this in {} s (no turn of its own): the page {} is yours to act on now.\n\n{}",
                        crate::pages::PAGE_WAIT_MS / 1000,
                        pages.join(", "),
                        body.join("\n\n")
                    );
                    log_line(&self.opts.paths, &format!("pages {}: @{agent} took no turn on the notes: to main", pages.join(", ")));
                    let w = self.pg.page_waits.entry(MAIN.to_string()).or_insert_with(|| (now, Vec::new()));
                    w.1.extend(pages.iter().map(|p| (p.clone(), String::new())));
                    let text = Self::page_hint(MAIN, text);
                    self.step(Input::ClientInput { client: 0, focus: MAIN.to_string(), text, queued: false, parts: String::new(), from: String::new() });
                }
                Untaken::Warn => {
                    let text = format!("his notes on the page {} reached no one: main took no turn on them", pages.join(", "));
                    log_line(&self.opts.paths, &text);
                    self.feed_rec(MAIN, Feed::warn(text));
                }
                Untaken::Wait => {}
            }
        }
    }

    /// The turns of an agent updating pages: one that has the notes and
    /// ends without a publish puts its pages back to ready (the notes
    /// stay sent).
    pub(super) fn page_turn(&mut self, name: &str, line: &str) {
        let Some((ids, seen)) = self.pg.page_turns.get_mut(name) else { return };
        // what its writer does now, on the pages it writes or updates
        // (the model's one-line intent of each bash/run_typescript call)
        if let crate::wire::Wire::Intent(text) = crate::wire::parse(line) {
            if let Some(p) = &self.pg.pages {
                for id in ids.iter() {
                    p.push(id, "progress", &json!({"text": text}));
                }
            }
            return;
        }
        if crate::pages::took_input(line) {
            *seen = true;
            return;
        }
        use bise_proto::thread::lines::{read, Obs, Rec};
        let Rec::Obs(Obs::TurnDone(end)) = read(line) else { return };
        if !*seen {
            return;
        }
        let how = end.text().trim().to_string();
        let ids = std::mem::take(ids);
        self.pg.page_turns.remove(name);
        let Some(pages) = self.pg.pages.clone() else { return };
        for id in ids {
            // the page says why nothing came (amb-kit m_5105)
            let writing = pages.store.meta(&id).is_some_and(|m| m.state == "writing");
            let reason = match (how.as_str(), writing) {
                ("completed", true) => "the turn ended without a page".to_string(),
                ("completed", false) => "the turn ended without a new version".to_string(),
                (h, _) => format!("the turn stopped: {h}"),
            };
            if let Some(meta) = pages.set_state_why(&id, "ready", &reason) {
                self.broadcast(&pages.page_ev(&meta));
            }
        }
        self.state_now();
    }
}
