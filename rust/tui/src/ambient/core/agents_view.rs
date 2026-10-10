//! Round 10's agents view (identity10 #data) on the hub's typed threads
//! (bise-proto, desktop S3b step 1): an agent's preview and its history
//! panel come from `subscribe`/`page` and the live `entry`/`typing`
//! events of the voice target's hub; the hub folds its transcript
//! (bise_proto::thread), the core no more (its fold copy is gone). The
//! panel's wire is unchanged: `agent_history`, `agent_entry`,
//! `agent_typing`, `agent_preview`.

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::thread::Entry;
use std::collections::{BTreeMap, HashMap};

/// An agent shown on the app: its history panel open, its preview, or
/// both. Holds its newest entries (live ones replace by pos).
pub(super) struct Feed {
    pub(super) agent: String,
    history: bool,
    preview: bool,
    /// its first page came (or nothing can come: the hub away)
    loaded: bool,
    /// its newest entries by pos, the panel's shape
    entries: BTreeMap<u64, Value>,
    /// what was sent of each entry (pos → its JSON): only changes go
    emitted: HashMap<u64, String>,
    /// the open cards sent as entries of their own (not in the thread)
    cards: Vec<u64>,
    /// the last preview sent
    shown: String,
}

/// What a `thread` answers: a feed's first page (`resync`: again after a
/// reconnection, only its changes go) or an older page of a panel.
pub(super) enum Ask {
    First { resync: bool },
    Older,
    /// the capsule's: main's first page (core/home.rs), another agent's
    Main,
    Quiet,
}

/// A `subscribe` or `page` sent; the hub answers them in order.
pub(super) struct Pending {
    pub(super) agent: String,
    pub(super) ask: Ask,
}

/// The entries a feed keeps (the preview's last actions, a live entry
/// growing).
const KEEP: usize = 8;

/// An entry as the panel shows it (its `from-agent` kind is the panel's
/// older word for bise-proto's `from_agent`).
fn panel_entry(e: &Entry) -> Value {
    let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
    if v["kind"] == "from_agent" {
        v["kind"] = json!("from-agent");
    }
    v
}

fn pos_of(e: &Value) -> u64 {
    e["pos"].as_u64().unwrap_or(0)
}

impl Core {
    fn hub_agent(&self, name: &str) -> Option<&bise_proto::rows::Agent> {
        self.hub_agents.iter().find(|a| a.name == name)
    }

    /// The agent is in a turn (or starting one): a queued message waits
    /// in the hub until it ends.
    fn busy(&self, name: &str) -> bool {
        self.hub_agent(name).is_some_and(|a| matches!(a.status, bise_proto::rows::Status::Working | bise_proto::rows::Status::Waiting))
    }

    /// His message into the agent's thread, as the TUI sends it: now (a
    /// busy agent gets it steered into its turn) or queued (sb-core holds
    /// it until the turn ends, amb-hub 7dcb56ab; never the core).
    pub(super) fn agent_send(&mut self, agent: String, text: String, queued: bool) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if self.hub_agent(&agent).is_none() {
            return self.error(&format!("there's no agent named {agent}."));
        }
        let mut v = json!({"agent": agent, "text": text, "via": "ambient"});
        if queued {
            v["mode"] = json!("queued");
        }
        if !self.home_call("turn/send", v) {
            return self.error(&format!("{agent} didn't get it: bise isn't reachable. try again in a moment."));
        }
        let mode = if queued { "queued" } else { "now" };
        self.emit(json!({"ev": "sent", "agent": agent, "mode": mode}));
        if queued && self.busy(&agent) {
            self.emit(json!({"ev": "agent_queued", "agent": agent, "text": text}));
        }
    }

    fn feed(&mut self, agent: &str) -> &mut Feed {
        if let Some(i) = self.feeds.iter().position(|f| f.agent == agent) {
            return &mut self.feeds[i];
        }
        self.feeds.push(Feed {
            agent: agent.to_string(),
            history: false,
            preview: false,
            loaded: false,
            entries: BTreeMap::new(),
            emitted: HashMap::new(),
            cards: Vec::new(),
            shown: String::new(),
        });
        self.feeds.last_mut().expect("pushed")
    }

    /// A typed command to the voice target's hub (its project added), as
    /// its JSON-RPC request; false: not sent (`initialize` not answered
    /// yet, the hub away). Never held for later: a feed is subscribed
    /// again at the next `initialize`.
    pub(super) fn typed(&mut self, mut v: Value, p: Option<Pending>) -> bool {
        let Some(project) = self.target.clone() else { return false };
        v["project"] = json!(project);
        let Some(req) = self.home.rpc.request(&v) else { return false };
        let ok = self.hub.send(&req);
        if let (true, Some(p)) = (ok, p) {
            self.pending.push_back(p);
        }
        ok
    }

    /// Its thread's newest page, then its live entries; when that can't
    /// go now (an agent the hub doesn't know, the hub away) the feed shows
    /// what the core has, at once, so the app never waits.
    fn subscribe(&mut self, agent: &str, limit: usize, resync: bool) {
        let known = self.hub_agent(agent).is_some();
        let ask = Pending { agent: agent.to_string(), ask: Ask::First { resync } };
        if known && self.typed(json!({"cmd": "subscribe", "agent": agent, "limit": limit.max(KEEP)}), Some(ask)) {
            return;
        }
        self.feed(agent).loaded = true;
        if self.feed(agent).history && !resync {
            self.first_page(agent, Vec::new(), None, false);
        } else {
            self.refresh(agent);
        }
    }

    /// A panel's thread left (never one the capsule reads: its words to
    /// him, core/home.rs).
    fn unsubscribe(&mut self, agent: &str) {
        if self.home.subs.contains(agent) {
            return;
        }
        self.typed(json!({"cmd": "unsubscribe", "agent": agent}), None);
    }

    /// Feeds with neither a panel nor a preview go (and their thread).
    fn drop_unshown(&mut self) {
        let gone: Vec<String> = self.feeds.iter().filter(|f| !f.history && !f.preview).map(|f| f.agent.clone()).collect();
        self.feeds.retain(|f| f.history || f.preview);
        for a in gone {
            self.unsubscribe(&a);
        }
    }

    pub(super) fn agent_preview(&mut self, agent: String) {
        for f in &mut self.feeds {
            f.preview = false;
            f.shown.clear();
        }
        let f = self.feed(&agent);
        f.preview = true;
        let loaded = f.loaded;
        let asked = self.pending.iter().any(|p| p.agent == agent && matches!(p.ask, Ask::First { .. }));
        self.drop_unshown();
        if loaded {
            self.refresh(&agent);
        } else if !asked {
            self.subscribe(&agent, KEEP, false);
        }
    }

    pub(super) fn agent_history(&mut self, agent: String, before: Option<usize>, limit: usize) {
        if let Some(b) = before {
            let p = Pending { agent: agent.clone(), ask: Ask::Older };
            let sent = b > 1 && self.typed(json!({"cmd": "page", "agent": agent, "before": b, "limit": limit}), Some(p));
            if !sent {
                self.emit(json!({"ev": "agent_history", "agent": agent, "entries": [], "before": null, "more": false}));
            }
            return;
        }
        let f = self.feed(&agent);
        f.history = true;
        f.emitted.clear();
        f.cards.clear();
        self.subscribe(&agent, limit, false);
    }

    pub(super) fn agent_unwatch(&mut self, agent: &str) {
        if let Some(f) = self.feeds.iter_mut().find(|f| f.agent == agent) {
            f.history = false;
            f.cards.clear();
        }
        self.drop_unshown();
    }

    /// A typed event of the voice target's hub.
    pub(super) fn typed_ev(&mut self, ev: HubEv) {
        match ev {
            HubEv::Welcome { project, .. } => {
                self.target = Some(project);
                self.pending.clear();
                // every feed shown again from the hub (a reconnection)
                let shown: Vec<(String, bool)> = self.feeds.iter().map(|f| (f.agent.clone(), f.history)).collect();
                for (a, history) in shown {
                    self.subscribe(&a, if history { 60 } else { KEEP }, true);
                }
            }
            HubEv::Thread { agent, entries, before, more, .. } => {
                let Some(i) = self.pending.iter().position(|p| p.agent == agent) else { return };
                let p = self.pending.remove(i).expect("found");
                if matches!(p.ask, Ask::Main | Ask::Quiet) {
                    return self.capsule_page(&agent, &entries, matches!(p.ask, Ask::Main));
                }
                let entries: Vec<Value> = entries.iter().map(panel_entry).collect();
                match p.ask {
                    Ask::Main | Ask::Quiet => {}
                    Ask::Older => self.emit(json!({"ev": "agent_history", "agent": agent, "entries": entries, "before": before, "more": more})),
                    Ask::First { resync, .. } => {
                        let Some(f) = self.feeds.iter_mut().find(|f| f.agent == agent) else { return };
                        f.loaded = true;
                        if f.history && !resync {
                            return self.first_page(&agent, entries, before, more);
                        }
                        for e in entries {
                            self.entry(&agent, e);
                        }
                        self.refresh(&agent);
                    }
                }
            }
            HubEv::Entry { agent, entry, .. } => {
                if self.feeds.iter().any(|f| f.agent == agent && f.loaded) {
                    self.entry(&agent, panel_entry(&entry));
                    self.refresh(&agent);
                }
            }
            HubEv::Typing { agent, text, .. } => {
                if self.typing.get(&agent) != Some(&text) {
                    self.typing.insert(agent.clone(), text.clone());
                    if self.feeds.iter().any(|f| f.agent == agent && f.history) {
                        self.emit(json!({"ev": "agent_typing", "agent": agent, "text": text}));
                    }
                    self.refresh(&agent);
                }
            }
            // a refused subscribe or page: its pending ends, empty
            HubEv::Error { cmd: Some(c), .. } if c == "subscribe" || c == "page" => {
                let Some(p) = self.pending.pop_front() else { return };
                match p.ask {
                    Ask::Main => self.home_ready(),
                    Ask::Quiet => {}
                    Ask::Older => self.emit(json!({"ev": "agent_history", "agent": p.agent, "entries": [], "before": null, "more": false})),
                    Ask::First { resync, .. } => {
                        if let Some(f) = self.feeds.iter_mut().find(|f| f.agent == p.agent) {
                            f.loaded = true;
                            if f.history && !resync {
                                return self.first_page(&p.agent, Vec::new(), None, false);
                            }
                        }
                        self.refresh(&p.agent);
                    }
                }
            }
            // the hub's rows: the window's (S3b step 2), the capsule reads `state`
            _ => {}
        }
    }

    /// One entry into its feed; a panel gets it when it changed.
    fn entry(&mut self, agent: &str, e: Value) {
        let Some(f) = self.feeds.iter_mut().find(|f| f.agent == agent) else { return };
        let pos = pos_of(&e);
        if f.history {
            let s = e.to_string();
            if f.emitted.get(&pos) != Some(&s) {
                f.emitted.insert(pos, s);
                self.out.push(json!({"ev": "agent_entry", "agent": agent, "entry": e}));
            }
        }
        f.entries.insert(pos, e);
        while f.entries.len() > KEEP {
            let first = *f.entries.keys().next().expect("not empty");
            f.entries.remove(&first);
            f.emitted.remove(&first);
        }
    }

    fn first_page(&mut self, agent: &str, mut entries: Vec<Value>, before: Option<u64>, more: bool) {
        let cards = self.agent_cards(agent, &entries);
        let Some(f) = self.feeds.iter_mut().find(|f| f.agent == agent) else { return };
        f.emitted = entries.iter().map(|e| (pos_of(e), e.to_string())).collect();
        f.cards = cards.iter().filter_map(|c| c["card"]["id"].as_u64()).collect();
        let skip = entries.len().saturating_sub(KEEP);
        f.entries = entries.iter().skip(skip).map(|e| (pos_of(e), e.clone())).collect();
        entries.extend(cards);
        self.emit(json!({"ev": "agent_history", "agent": agent, "entries": entries, "before": before, "more": more}));
        self.refresh(agent);
    }

    /// The agent's open cards not in its thread, as entries after it.
    fn agent_cards(&self, agent: &str, entries: &[Value]) -> Vec<Value> {
        let shown: Vec<u64> = entries.iter().filter_map(|e| e["card"]["id"].as_u64()).collect();
        self.cards
            .iter()
            .filter(|c| c.agent == agent && !shown.contains(&c.id))
            .map(|c| {
                let card = super::super::agents::card_ref(c.id, &c.kind, &c.agent, &c.text, true);
                json!({"pos": super::super::agents::CARD_POS + c.id, "at_ms": 0, "kind": "card",
                       "text": card["question"], "card": card})
            })
            .collect()
    }

    /// Every agent shown, after the state changed (its status, cards,
    /// pages).
    pub(super) fn refresh_shown(&mut self) {
        let shown: Vec<String> = self.feeds.iter().map(|f| f.agent.clone()).collect();
        for a in shown {
            self.refresh(&a);
        }
    }

    /// A feed changed (an entry, a step, the state): its cards to the
    /// panel, its preview if it differs.
    fn refresh(&mut self, agent: &str) {
        let Some(i) = self.feeds.iter().position(|f| f.agent == agent && f.loaded) else { return };
        let entries: Vec<Value> = self.feeds[i].entries.values().cloned().collect();
        let preview = self.feeds[i].preview.then(|| self.preview(agent, &entries));
        let history = self.feeds[i].history;
        let cards = if history { self.agent_cards(agent, &entries) } else { Vec::new() };
        let f = &mut self.feeds[i];
        let mut out = Vec::new();
        if history {
            // its own cards: a new one comes, an answered one says so
            let now: Vec<u64> = cards.iter().filter_map(|c| c["card"]["id"].as_u64()).collect();
            for c in &cards {
                if !f.cards.contains(&c["card"]["id"].as_u64().unwrap_or(0)) {
                    out.push(json!({"ev": "agent_entry", "agent": agent, "entry": c}));
                }
            }
            for id in f.cards.iter().filter(|id| !now.contains(id)) {
                let mut e = json!({"pos": super::super::agents::CARD_POS + *id, "at_ms": 0, "kind": "card", "text": ""});
                e["card"] = json!({"id": id, "answered": true});
                out.push(json!({"ev": "agent_entry", "agent": agent, "entry": e}));
            }
            f.cards = now;
        }
        if let Some(p) = preview {
            let s = p.to_string();
            if f.shown != s {
                f.shown = s;
                out.push(p);
            }
        }
        self.out.extend(out);
    }

    /// The preview (identity10 #data): what it does now, its last
    /// actions, what waits on him, its last report, its pages.
    fn preview(&self, agent: &str, entries: &[Value]) -> Value {
        use super::super::agents;
        let a = self.hub_agent(agent);
        let status = a.map_or("idle", |a| super::home::row_status(a).0);
        let now = match self.typing.get(agent) {
            Some(t) if status == "working" && !t.is_empty() => t.clone(),
            _ => a.map(super::home::row_title).unwrap_or_default(),
        };
        let report = a.and_then(|a| a.report.as_ref()).filter(|r| !r.text.is_empty());
        let waiting: Vec<Value> = self
            .cards
            .iter()
            .filter(|c| c.agent == agent)
            .map(|c| json!({"card_id": c.id, "question": agents::card_ref(c.id, &c.kind, &c.agent, &c.text, true)["question"]}))
            .collect();
        let last_report = match entries.iter().rev().find(|e| e["kind"] == "report") {
            Some(e) => json!({"kind": e["report"]["kind"], "text": e["text"], "at_ms": e["at_ms"]}),
            None if report.is_some() => {
                let kind = match status {
                    "done" | "failed" | "blocked" => status,
                    _ => "progress",
                };
                let r = report.expect("some");
                json!({"kind": kind, "text": r.text, "at_ms": r.at_ms})
            }
            None => Value::Null,
        };
        let mut pages: Vec<&bise_proto::rows::Page> = self.hub_pages.iter().filter(|p| p.agent == agent).collect();
        pages.sort_by_key(|p| std::cmp::Reverse(p.at_ms));
        let pages: Vec<Value> = pages.into_iter().take(3).map(|p| agents::page_ref(&p.id, &self.hub_pages)).collect();
        json!({"ev": "agent_preview", "agent": agent, "now": now, "actions": agents::actions(entries, 5),
               "waiting": waiting, "last_report": last_report, "pages": pages})
    }
}
