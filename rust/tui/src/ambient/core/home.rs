//! The core's home connection (client-protocol step 5, proto-lead
//! m_15443, architect m_15476): bise's home hub spoken to in JSON-RPC as
//! a project's is (core/hubs.rs): `initialize` first, then typed
//! notifications only. What the capsule reads of it:
//!
//! - hub/agents, hub/cards, hub/pages, hub/scheduled: the window's `state`
//!   (its shape as the older state made it);
//! - main's first page (`thread/subscribe`): where main is (a turn
//!   running, his message waiting), nothing of it sent; then the core is
//!   ready and main's entries are live;
//! - page/changed: the window's `page`, in front when he waits for it;
//! - thread/entry: main's turn (his message read, a turn's start and
//!   end, its words, its tools) and any agent's words to him (`to_you`),
//!   so every live agent's thread is subscribed (as the terminal does).
//!
//! An older home hub (it doesn't serve `initialize`): home_switch.rs.

use super::agents_view::{Ask, Pending};
use super::hub_rpc::Read;
use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rows;
use bise_proto::thread::{Delivery, Entry, EntryKind};
use std::collections::{BTreeMap, BTreeSet};

/// main's first page: enough of its thread to say where its turn is.
pub const MAIN_PAGE: usize = 40;
/// another agent's: none of its words are read, only its next ones.
pub const QUIET_PAGE: usize = 1;

/// The home connection's JSON-RPC side and what the capsule keeps of it.
#[derive(Default)]
pub(super) struct Home {
    pub(super) rpc: hub_rpc::RpcConn,
    /// the threads the capsule subscribed on this connection (a panel
    /// closing never unsubscribes them)
    pub(super) subs: BTreeSet<String>,
    /// each thread's newest pos seen: an entry past it is new
    seen: BTreeMap<String, u64>,
    /// main's turn: the pos it started at, and whether its `turn_start`
    /// entry came (a turn started by his message read gets it later)
    turn_from: u64,
    flagged: bool,
    /// how main's turn ends when not completed (an interrupt, a failure)
    fail: Option<String>,
    /// main's row said working during this turn (a turn with no entry of
    /// its own ends when the row stops saying it)
    row_ran: bool,
    /// the step the orb shows (only a new one is sent)
    step: Option<String>,
    /// the hub's open cards that are his, and its scheduled tasks
    cards: Vec<rows::Card>,
    timers: Vec<rows::ScheduledTask>,
    /// a state kind came: the window's `state` goes at the next
    /// `take_out` (one for the kinds of a burst, as the older state was)
    pub(super) state_due: bool,
}

/// A row's status as the window's word, and archived.
pub(super) fn row_status(a: &rows::Agent) -> (&'static str, bool) {
    if a.archived {
        return ("done", true);
    }
    let w = match a.status {
        rows::Status::Working => "working",
        rows::Status::Waiting => "waiting",
        rows::Status::Blocked => "blocked",
        rows::Status::Failed => "failed",
        rows::Status::Done => "done",
        rows::Status::Idle => "idle",
    };
    (w, false)
}

/// A row's title line: what it says it does now (its note), else its
/// last report, else its objective (the TUI's title).
pub(super) fn row_title(a: &rows::Agent) -> String {
    let report = a.report.as_ref().map_or("", |r| r.text.as_str());
    let t = [a.note.trim(), report.trim(), a.objective.trim()].into_iter().find(|t| !t.is_empty()).unwrap_or("");
    super::super::agents::one_line(t, 120)
}

fn card_info(c: &rows::Card) -> CardInfo {
    CardInfo {
        id: c.id,
        kind: c.kind.clone(),
        agent: c.agent.clone(),
        text: if c.text.is_empty() { c.question.clone() } else { c.text.clone() },
        page: c.page.as_ref().and_then(|p| serde_json::to_value(p).ok()),
        batch: c.batch.as_ref().and_then(|b| serde_json::to_value(b).ok()),
    }
}

impl Core {
    /// Connected: `initialize` first, on every connection.
    pub(super) fn home_up(&mut self) {
        self.ready = false;
        self.target = None;
        self.pending.clear();
        self.home.subs.clear();
        let init = self.home.rpc.initialize(env!("CARGO_PKG_VERSION"));
        self.hub.send(&init);
        self.hub_up = Some(true);
        let ws = self.workspace.clone();
        self.emit(json!({"ev": "hub", "up": true, "workspace": ws}));
    }

    /// One line of the home hub.
    pub(super) fn home_line(&mut self, raw: &str) {
        let Ok(v) = serde_json::from_str::<Value>(raw) else { return };
        let project = self.target.clone().unwrap_or_default();
        match self.home.rpc.read(&project, v) {
            Read::Welcome(welcome, state) => {
                if let Some(u) = self.home.rpc.pages_url.clone().filter(|u| !u.is_empty()) {
                    self.pages_url = Some(u);
                }
                // the target and the panels' threads, then main's (its
                // first page makes the core ready), then the state
                self.home_ev(welcome);
                self.capsule_subscribe("main", MAIN_PAGE, Ask::Main);
                self.home_evs(state);
            }
            Read::Evs(evs) => self.home_evs(evs),
            Read::Error { tag, text, cid, reason, kind } => {
                self.home_ev(HubEv::Error { project: Some(project), cmd: Some(tag), text, cid, reason, kind });
            }
            Read::Resync => {
                let req = self.home.rpc.read_again();
                self.hub.send(&req);
            }
            Read::Refused(why) => self.hub(HubIn::Refused(why)),
            Read::Older => self.home_older(),
            Read::Nothing => {}
        }
    }

    /// The hub's events in order; the state kinds kept for one `state`.
    fn home_evs(&mut self, evs: Vec<HubEv>) {
        let mut state = false;
        for ev in evs {
            match ev {
                HubEv::Agents { agents, .. } => {
                    self.hub_agents = agents;
                    self.main_row();
                    state = true;
                }
                HubEv::Cards { cards, .. } => {
                    self.home.cards = cards;
                    state = true;
                }
                HubEv::Pages { items, .. } => {
                    self.hub_pages = items.iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
                    state = true;
                }
                HubEv::Scheduled { items, ended, .. } => {
                    self.home.timers = items.into_iter().chain(ended).collect();
                    state = true;
                }
                ev => self.home_ev(ev),
            }
        }
        if state {
            self.home.state_due = true;
            self.capsule_subscribe_live();
        }
    }

    /// One typed event of the home hub (not a state kind).
    fn home_ev(&mut self, ev: HubEv) {
        // J: a job's end once, from whichever hub said it first
        if !self.end_once(&ev) {
            return;
        }
        match &ev {
            HubEv::PageChanged { page, .. } => {
                if self.ready {
                    self.page_changed(page);
                }
                return;
            }
            HubEv::Entry { agent, entry, .. } => {
                let (agent, entry) = (agent.clone(), entry.clone());
                self.capsule_entry(&agent, &entry);
            }
            _ => {}
        }
        self.typed_ev(ev);
    }

    /// A thread the capsule reads (main's, or an agent's words to him).
    fn capsule_subscribe(&mut self, agent: &str, limit: usize, ask: Ask) {
        let p = Pending { agent: agent.to_string(), ask };
        if self.typed(json!({"cmd": "subscribe", "agent": agent, "limit": limit}), Some(p)) {
            self.home.subs.insert(agent.to_string());
        }
    }

    /// Every live agent's thread, for its words to him (one a panel
    /// holds already is subscribed: it stays so when the panel goes).
    fn capsule_subscribe_live(&mut self) {
        let new: Vec<String> = self.hub_agents.iter().filter(|a| !a.main && !a.archived && !self.home.subs.contains(&a.name)).map(|a| a.name.clone()).collect();
        for a in new {
            if self.feeds.iter().any(|f| f.agent == a) {
                self.home.subs.insert(a);
            } else {
                self.capsule_subscribe(&a, QUIET_PAGE, Ask::Quiet);
            }
        }
    }

    /// main's row moved: a turn with no entry of its own (its start and
    /// end ride on entries) ends when the row stops saying it runs.
    fn main_row(&mut self) {
        let runs = self.hub_agents.iter().any(|a| a.main && a.status == rows::Status::Working);
        if !self.in_turn || self.home.flagged {
            return;
        }
        if runs {
            self.home.row_ran = true;
        } else if self.home.row_ran {
            self.turn_end();
        }
    }

    /// main's turn ended (its end on an entry, or its row).
    fn turn_end(&mut self) {
        self.in_turn = false;
        let for_user = std::mem::take(&mut self.for_user);
        let fail = self.home.fail.take();
        if self.ready {
            self.turn_done(fail, for_user);
        }
    }

    /// A capsule thread's first page: main's says where it is, then the
    /// core is ready; another's only marks what is old.
    pub(super) fn capsule_page(&mut self, agent: &str, entries: &[Entry], main: bool) {
        for e in entries {
            self.capsule_entry(agent, e);
        }
        if main {
            self.home_ready();
        }
    }

    /// main's first page came (or can't): main's entries are live now,
    /// and the orb says where its turn is (working only on a turn that
    /// answers him).
    pub(super) fn home_ready(&mut self) {
        // a turn that started with no entry of its own yet (its start
        // rides on its first one): main's row says it runs
        let runs = self.hub_agents.iter().any(|a| a.main && a.status == rows::Status::Working);
        if runs && !self.in_turn {
            let next = self.home.seen.get("main").map_or(0, |p| p + 1);
            self.turn_start(next, false);
        }
        self.ready = true;
        let p = if self.in_turn && self.for_user { Phase::Working } else { Phase::Idle };
        if self.talk.is_none() && self.speech.is_none() {
            self.set_phase(p, None);
        }
    }

    /// One entry of a thread the capsule reads.
    fn capsule_entry(&mut self, agent: &str, e: &Entry) {
        let seen = self.home.seen.entry(agent.to_string()).or_default();
        let new = e.pos > *seen;
        if new {
            *seen = e.pos;
        }
        if e.to_you && matches!(e.kind, EntryKind::Agent | EntryKind::FromAgent) {
            if new && self.ready {
                self.msg_you(e.from.as_deref().unwrap_or(agent), &e.text);
            }
        } else if agent == "main" {
            self.main_entry(e, new);
        }
    }

    /// An agent (or main) writing to him: main's words, with who wrote
    /// them.
    fn msg_you(&mut self, from: &str, text: &str) {
        let text = super::super::plain::plain(text);
        if !text.is_empty() {
            let turn = self.turn;
            self.emit(json!({"ev": "main", "text": text, "turn": turn, "from": from}));
        }
    }

    /// One entry of main's thread (ambient's review, m_4672: only main's
    /// words FOR THE USER reach the capsule). A turn answers him when his
    /// message reached it: read when the turn started, or steered into
    /// it. Main's other turns (an agent's report, a message between
    /// agents) send nothing: no `main`, no phase. Before `ready` (main's
    /// first page) the entries only say where main is.
    fn main_entry(&mut self, e: &Entry, new: bool) {
        if e.turn_start && new {
            if self.in_turn && !self.home.flagged && e.pos >= self.home.turn_from {
                // the start of the turn his message began
                self.home.flagged = true;
            } else {
                self.turn_start(e.pos, true);
            }
        }
        match e.kind {
            EntryKind::You => self.you_entry(e, new),
            EntryKind::Stopped if new && self.in_turn => self.home.fail = Some("interrupted".into()),
            EntryKind::TurnFailed if new && self.in_turn => {
                // its line as the feed shows it ("turn failed: why")
                let why = if e.text.is_empty() { e.turn_failed.as_ref().map(|f| f.why.clone()).unwrap_or_default() } else { e.text.clone() };
                self.home.fail = Some(if why.is_empty() { "failed".into() } else { why });
            }
            EntryKind::Agent if self.ready && self.for_user && self.in_turn => {
                // kept until the turn ends: words followed by a tool call
                // are main planning, not its answer (pm's 22)
                let text = super::super::plain::plain(&crate::markdown::unescape_md(&e.text));
                if !text.is_empty() {
                    self.main_text = text;
                }
            }
            EntryKind::Tools if self.ready && self.for_user && self.in_turn => {
                self.main_text.clear();
                let step = e.tools.as_ref().and_then(|t| t.items.last()).and_then(|i| match &i.intent {
                    Some(t) if !t.is_empty() => Some(crate::render::truncate_chars(t, 80)),
                    _ => Some(i.name.clone()).filter(|n| !n.is_empty()),
                });
                // a tools entry grows: only a new step goes
                let new_step = step.is_some() && step != self.home.step;
                if new_step && self.speech.is_none() {
                    self.home.step = step.clone();
                    self.set_phase(Phase::Working, step);
                }
            }
            _ => {}
        }
        if e.turn_end_ms.is_some() && self.in_turn && e.pos >= self.home.turn_from {
            self.turn_end();
        }
    }

    /// His message in main's thread: waiting until a turn reads it (a
    /// slash command is the hub's, never a turn; what comes from a page
    /// is answered on the page, ambient-lead m_5724, pm's 27).
    fn you_entry(&mut self, e: &Entry, new: bool) {
        let t = e.text.trim_start();
        if t.starts_with('/') || t.contains(PAGE_MARK) {
            return;
        }
        if new {
            self.user_waiting = true;
        }
        if !matches!(e.delivery, Some(Delivery::Received | Delivery::Read)) {
            return;
        }
        if !self.in_turn {
            // the turn that read it started
            if self.user_waiting {
                self.turn_start(e.pos, false);
            }
            return;
        }
        // steered into the running turn
        if self.user_waiting {
            self.user_waiting = false;
            if !self.for_user {
                self.for_user = true;
                self.user_turn_agents = Some(self.agent_names.clone());
                if self.ready && self.speech.is_none() {
                    self.set_phase(Phase::Working, None);
                }
            }
        }
        if self.ready {
            self.reached();
        }
    }

    /// main's turn starts at `pos` (`flagged`: its turn_start entry).
    fn turn_start(&mut self, pos: u64, flagged: bool) {
        self.in_turn = true;
        self.home.turn_from = pos;
        self.home.flagged = flagged;
        self.home.fail = None;
        self.home.row_ran = self.hub_agents.iter().any(|a| a.main && a.status == rows::Status::Working);
        self.home.step = None;
        self.turn += 1;
        self.main_text.clear();
        self.for_user = std::mem::take(&mut self.user_waiting);
        if self.for_user {
            self.user_turn_agents = Some(self.agent_names.clone());
        }
        if self.ready {
            self.reached();
            if self.for_user && self.speech.is_none() {
                self.set_phase(Phase::Working, None);
            }
        }
    }

    /// A page published, updated or answered, relayed with `front`: he
    /// waits for it (main's page in his turn, or a page of an agent born
    /// since his last turn started), so the app opens it without fn + o
    /// (docs/ambient-pages.md §2.8).
    fn page_changed(&mut self, p: &rows::Page) {
        let waited = match &self.user_turn_agents {
            None => false,
            Some(_) if p.agent == "main" => self.for_user && self.in_turn,
            Some(known) => !known.contains(&p.agent),
        };
        let front = p.state == "ready" && waited;
        self.emit(json!({"ev": "page", "id": p.id, "title": p.title, "agent": p.agent, "version": p.version,
            "url": p.url, "at_ms": p.at_ms, "state": p.state, "front": front}));
    }

    /// The window's `state`: the agents' rows, his cards with their
    /// options, the pages, the standing orders.
    pub(super) fn emit_state(&mut self) {
        self.agent_names = self.hub_agents.iter().map(|a| a.name.clone()).collect();
        let user_kind = self.ports.user_kind;
        // bise's own bookkeeping (main's archive suggestion, a `drop`
        // card) stays in the TUI: never on the glass, in the count or in
        // needs-you (pm's C fail 36)
        self.cards = self.home.cards.iter().filter(|c| user_kind(&c.kind) && c.kind != "drop").map(card_info).collect();
        for c in self.cards.iter().filter(|c| c.batch.is_some()) {
            if !self.batches_seen.contains(&c.id) {
                self.batches_seen.push(c.id);
            }
        }
        let now = now_ms();
        let mut rows = Vec::new();
        for a in self.hub_agents.iter().filter(|a| !a.main) {
            let (status, archived) = row_status(a);
            // since: when the core saw the status change; at first sight
            // its last report's time, else its birth, else now
            let first = || {
                let report = a.report.as_ref().map(|r| r.at_ms).filter(|m| *m > 0 && status != "working");
                report.or(a.created_ms.filter(|m| *m > 0)).unwrap_or(now)
            };
            let since = match self.since.get(&a.name) {
                Some((st, ms)) if st == status => *ms,
                Some(_) => now,
                None => first(),
            };
            self.since.insert(a.name.clone(), (status.to_string(), since));
            let waits = self.cards.iter().filter(|c| c.agent == a.name).count();
            let purpose = if a.objective.is_empty() { &a.purpose } else { &a.objective };
            rows.push(json!({
                "name": a.name,
                "status": status,
                "title": row_title(a),
                "purpose": super::super::agents::one_line(purpose, 200),
                "since_ms": since,
                "waits": waits,
                "followed": self.followed.contains(&a.name),
                "archived": archived,
                "note": a.note,
                "report": a.report.as_ref().map_or("", |r| r.text.as_str()),
                "working": !archived && matches!(a.status, rows::Status::Working | rows::Status::Waiting),
                "turn_ms": a.turn_ms,
            }));
        }
        let cards: Vec<Value> = self.cards.iter().map(card_ev).collect();
        let pages = self.hub_pages.clone();
        let s = |p: &Value, k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        self.page_titles = pages.iter().map(|p| (s(p, "id"), s(p, "title"))).collect();
        let mut st = json!({"ev": "state", "agents": rows, "cards": cards, "pages": pages});
        if let Some(u) = &self.pages_url {
            st["pages_url"] = json!(u);
        }
        // the standing orders (roadmap B), as the hub has them: the live
        // ones, then the ones ended lately
        if !self.home.timers.is_empty() {
            st["timers"] = serde_json::to_value(&self.home.timers).unwrap_or(Value::Null);
        }
        self.emit(st);
        // round 10: what is shown of an agent follows its status, its
        // cards, its pages
        self.refresh_shown();
    }
}

/// A card of his as the window's `state` has it.
fn card_ev(c: &CardInfo) -> Value {
    let mut options = crate::sb::ambient_options(&c.kind, &c.agent, &c.text);
    if options.is_empty() && c.page.is_some() {
        options = page_options(&c.text);
    }
    // the body does not repeat the options (ambient m_6476): a numbered
    // list at the end of the text that is the card's options goes, the
    // question stays
    let (body, listed) = crate::sb::split_choices(&c.text);
    let text = if !listed.is_empty() && listed.iter().eq(options.iter().map(|(_, l)| l)) { body } else { c.text.clone() };
    let options: Vec<Value> = options.into_iter().map(|(n, label)| json!({"n": n, "label": label})).collect();
    // urgent: it asks even on a call or in a meeting. A tool call waiting
    // for a yes holds an agent's work now (best guess, told to
    // ambient-lead)
    let urgent = c.kind == "confirm";
    let mut v = json!({"id": c.id, "kind": c.kind, "agent": c.agent, "text": text, "options": options, "urgent": urgent});
    // main's own question (no page): its label (ambient m_6476)
    if c.agent == "main" && c.kind == "question" && c.page.is_none() {
        v["label"] = json!("? main needs you");
    }
    if let Some(p) = &c.page {
        v["page"] = p.clone();
    }
    if let Some(b) = &c.batch {
        v["batch"] = b.clone();
    }
    v
}
