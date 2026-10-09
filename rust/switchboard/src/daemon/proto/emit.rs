//! The typed events the hub sends on its own (architect review 10: moved
//! out of `daemon/proto.rs`, no behavior change): the rows (`agents`,
//! `cards`), `prs`, `models`, `artifacts`, the routes and followed ends,
//! and what a broadcast changes (`proto_on`: each event again only when
//! it changed, the subscribed threads' new entries, an agent's usage);
//! P4b: the repo's `flow`, and each agent's newest entry (`last_pos`,
//! proto_view/heads.rs: a live line that starts an entry sends the rows
//! again).

use super::*;
use crate::daemon::rpc::Typed;
use bise_proto::hub::JobState;
use bise_proto::rows::{AgentUsage, FlowMode};
use bise_proto::thread::words;

impl Shell {
    /// desktop S2 step 2: sb-core's `route`/`route_done` effects, to every
    /// typed connection (the window and the capsule show the held route).
    pub(in crate::daemon) fn proto_route(&mut self, ev: HubEv) {
        let ids: Vec<ClientId> = self.proto.conns.keys().copied().collect();
        self.proto_note(&ids, &ev);
    }

    /// S10: a followed task ended: `job_end` to every typed connection.
    pub(in crate::daemon) fn job_end(&mut self, agent: String, state: &str, label: String, summary: String, key: u64) {
        let state = if state == "failed" { JobState::Failed } else { JobState::Done };
        let key = Some(key).filter(|k| *k > 0);
        self.proto_route(HubEv::JobEnd { project: self.project(), agent, state, label, summary, key });
    }

    /// J, bise's home hub: a task it follows in another project ended:
    /// `followed_end` to every typed connection (its line is in main).
    pub(in crate::daemon) fn followed_end(&mut self, project: String, agent: String, key: u64, state: &str, label: String, summary: String) {
        let state = if state == "failed" { JobState::Failed } else { JobState::Done };
        self.proto_route(HubEv::FollowedEnd { project, agent, key, state, label, summary });
    }

    /// The rows of the snapshot: `agents`, `cards`.
    pub(in crate::daemon) fn proto_rows(&mut self, snap: &Value) -> (HubEv, HubEv) {
        let project = self.project();
        let now = now_ms();
        self.seed_usage(snap);
        self.seed_heads(snap);
        // K4: each agent's model read against the hub's catalog (his
        // config.toml merged, re-read when it changes), fresh each time
        self.setup();
        let cat = self.setup.as_ref().map(|(_, s)| &s.catalog);
        let vision = |m: &str| cat.and_then(|c| c.vision(m));
        // S13: each agent's context after its last call, its window from
        // the same catalog, the gauge's words the TUI's divider says
        let calls = &self.proto.usage;
        let usage = |name: &str| {
            let u = calls.get(name)?;
            let context = u.input + u.output;
            let window = cat.map(|c| c.context_window(&u.model)).filter(|w| *w > 0);
            Some(AgentUsage { model: u.model.clone(), context, window, words: words::context_words(context, window), short: words::short_words(context, window) })
        };
        let heads = &self.proto.heads;
        let head = |name: &str| (heads.last(name), heads.turns(name));
        let agents = proto_view::agents(snap, &mut self.proto.since, now, crate::model::user_kind, &vision, &usage, &head);
        let (cards, others) = proto_view::cards(snap, &project, now, crate::model::user_kind);
        let places = proto_view::places(snap);
        (HubEv::Agents { project: project.clone(), agents, places }, HubEv::Cards { project, cards, others })
    }

    /// S13 (amb-win m_10985, architect m_10999): an agent the hub has no
    /// usage for yet gets it once from its transcript's tail, by the rule
    /// the TUI uses on its feed (`lines::current_usage`: the last usage
    /// line, none when a compaction ended after it), so a hub restart no
    /// longer blanks the gauge. Once per agent, never per broadcast; live
    /// lines then move it (`proto_usage_line`). No usage, no row: never
    /// an invented 0.
    fn seed_usage(&mut self, snap: &Value) {
        let names: Vec<String> = snap
            .get("agents")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();
        for name in names {
            if !self.proto.seeded.insert(name.clone()) || self.proto.usage.contains_key(&name) {
                continue;
            }
            let Some(dir) = self.dir_of(&name) else { continue };
            let tail = transcript_tail(&self.transcript(&dir), SEED_BYTES);
            let last = pthread::lines::current_usage(tail.iter().map(|l| pthread::lines::usage_mark(l)));
            if let Some(u) = last.and_then(|t| bise_session::usage_line::parse(&t)) {
                self.proto.usage.insert(name, u);
            }
        }
    }

    /// The typed `prs` (bar A.7): the open PRs of this hub's places, the
    /// rows the TUI's `/prs` draws (forge::news::pr_rows).
    /// ⌘K and the scheduled screen: the live timers as rows, built from
    /// the hub's typed timer state (proto_view::scheduled, the one builder),
    /// and the ones that ended this week (P4c-4a: the screen's tab).
    pub(in crate::daemon) fn scheduled_ev(&self) -> HubEv {
        let timers = self.hub.timers();
        HubEv::Scheduled { project: self.project(), items: proto_view::scheduled(timers), ended: proto_view::scheduled_ended(timers, now_ms()) }
    }

    pub(in crate::daemon) fn prs_ev(&self) -> HubEv {
        let places = crate::place::places(&self.hub.st, &self.hub.prs);
        crate::forge::news::prs_ev(self.project(), &places)
    }

    /// The typed `models` (bar A.5): the TUI's `/model` list on this hub
    /// (bise_catalog::picks), with the providers ready for this hub's own
    /// environment: what its REPLs will have (architect m_10427).
    pub(in crate::daemon) fn models_ev(&mut self) -> HubEv {
        self.proto.models_at = Some(models_mtimes());
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let home = bise_home::Home::from_env();
        let setup = self.setup().clone();
        let ready = bise_catalog::picks::ready_in(&setup, &env, &home);
        // L5: every model, each row saying whether its provider is ready
        // (the window offers setup for the others); the TUI's /model
        // popup still lists the ready ones only
        let picks = bise_catalog::picks::picks(&setup, &|_| true);
        HubEv::Models { project: self.project(), items: proto_view::models(&setup, &picks, &|id| ready.iter().any(|r| r == id)) }
    }

    /// config.toml or auth.json changed since the last `models` (a stat).
    fn models_moved(&self) -> bool {
        self.proto.models_at.is_some_and(|at| at != models_mtimes())
    }

    /// The hub's `approvals` event (gate.rs `approvals_ev`, the TUI's) as
    /// the typed one (bar V8/W21); paths in the words as the TUI says them.
    pub(in crate::daemon) fn proto_approvals(&self, ev: &Value) -> HubEv {
        let home = std::env::var("HOME").ok();
        proto_view::approvals::approvals(&self.project(), ev, home.as_deref())
    }

    /// The art store's event as the typed `artifacts`.
    pub(in crate::daemon) fn proto_artifacts(&self, ev: &Value) -> HubEv {
        HubEv::Artifacts { project: self.project(), items: self.artifact_rows(ev), seen_ms: ev.get("seen_ms").and_then(Value::as_u64) }
    }

    /// The art store's event (`artifacts_ev`) as rows: the one builder of
    /// the typed `artifacts` event and of `view.json`'s artifacts (⌘K's
    /// index, architect m_11910), so the two never disagree.
    pub(in crate::daemon) fn artifact_rows(&self, ev: &Value) -> Vec<bise_proto::rows::Artifact> {
        let pages = self.pg.pages.clone();
        let page_url = |id: &str| pages.as_ref().filter(|p| p.store.meta(id).is_some()).map(|p| p.url(id));
        proto_view::artifacts(ev, page_url)
    }

    /// An event the hub broadcasts, for the typed connections: `state` →
    /// `agents`/`cards` when they changed (and the subscribed threads
    /// refolded: a card answered), `line` → its thread's changed entries
    /// and the agent's step.
    pub(in crate::daemon) fn proto_on(&mut self, v: &Value) {
        let live: BTreeSet<ClientId> = self.clients.keys().copied().collect();
        self.proto.conns.retain(|id, _| live.contains(id));
        if self.proto.conns.is_empty() {
            return;
        }
        let ids: Vec<ClientId> = self.proto.conns.keys().copied().collect();
        match v.get("ev").and_then(Value::as_str) {
            Some("state") => {
                let (agents, cards) = self.proto_rows(v);
                let (a, c) = (agents.encode(), cards.encode());
                let new = (a != self.proto.last.0, c != self.proto.last.1);
                self.proto.last = (a, c);
                if new.0 {
                    self.proto_note(&ids, &agents);
                }
                if new.1 {
                    self.proto_note(&ids, &cards);
                }
                if new.1 {
                    self.proto_live(|l, ctx| l.refold(ctx));
                }
                // P4b: the repo's flow, when it changed
                let flow = self.flow_ev();
                let f = flow.encode();
                if f != self.proto.flow {
                    self.proto.flow = f;
                    self.proto_note(&ids, &flow);
                }
                // S10: the followed jobs, when they changed
                let jobs = HubEv::Jobs { project: self.project(), items: proto_view::jobs(v) };
                let j = jobs.encode();
                if j != self.proto.jobs {
                    self.proto.jobs = j;
                    self.proto_note(&ids, &jobs);
                }
                // bar A.6: a feature made, synced, built, merged or
                // dropped, its card or its agents (the steps' ends and
                // the facts' refresh come back as a snapshot)
                let feats = self.features_ev();
                let f = feats.encode();
                if f != self.proto.features {
                    self.proto.features = f;
                    self.proto_note(&ids, &feats);
                }
                // bar A.7: a PR opened, closed, reviewed or checked (the
                // forge poll's report lands as a state change)
                let prs = self.prs_ev();
                let p = prs.encode();
                if p != self.proto.prs {
                    self.proto.prs = p;
                    self.proto_note(&ids, &prs);
                }
                // ⌘K: a timer set, run, stopped or ended (each comes back
                // as a state change)
                let sched = self.scheduled_ev();
                let s = sched.encode();
                if s != self.proto.scheduled {
                    self.proto.scheduled = s;
                    self.proto_note(&ids, &sched);
                }
                // bar A.5: config.toml or auth.json moved (a stat): the
                // models again when their rows changed
                if self.models_moved() {
                    let models = self.models_ev();
                    let m = models.encode();
                    if m != self.proto.models {
                        self.proto.models = m;
                        self.proto_note(&ids, &models);
                    }
                }
                // an agent came, went, started or ended a turn: its
                // worktree may have changed (commits, files)
                let key = self.worktrees_key();
                if key != self.proto.worktrees {
                    self.proto.worktrees = key;
                    self.worktrees_typed(Typed::All(ids.clone()));
                    self.dev_servers_typed(Typed::All(ids.clone()));
                }
            }
            // bar V8/W21: the mode switched or a rule left (set_mode,
            // remove_rule broadcast the TUI's event): the typed one to all
            Some("approvals") => {
                let ev = self.proto_approvals(v);
                self.proto_note(&ids, &ev);
            }
            // the art store changed (artifacts_refresh broadcasts it)
            Some("artifacts") => {
                let arts = self.proto_artifacts(v);
                self.proto_note(&ids, &arts);
            }
            Some("line") => {
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let (agent, line) = (s("agent"), s("line"));
                let pos = v.get("pos").and_then(Value::as_u64).unwrap_or(0);
                let ts = v.get("ts").and_then(Value::as_u64).unwrap_or(0);
                let step = proto_view::step_of(&line);
                let sent = self.proto_live_of(&agent, |l, ctx| l.push((pos, ts, line.clone()), ctx));
                let moved = self.proto_head_line(&agent, (pos, ts, line.clone()));
                if self.proto_usage_line(&agent, &line) || moved {
                    self.proto_agents_again(&ids);
                }
                if let Some(text) = step {
                    let project = self.project();
                    for id in sent {
                        self.proto_send(id, &HubEv::Typing { project: project.clone(), agent: agent.clone(), text: text.clone() });
                    }
                }
            }
            _ => {}
        }
    }

    /// S13: a live line of `agent` that moves its context (its usage
    /// line, a compaction's end): true when it did. The line is read by
    /// the one parser of its kind (`thread::lines`), the usage by
    /// bise_session's.
    fn proto_usage_line(&mut self, agent: &str, line: &str) -> bool {
        match pthread::lines::read(line) {
            pthread::lines::Rec::Obs(pthread::lines::Obs::Usage(t)) => match bise_session::usage_line::parse(&t) {
                Some(u) => {
                    self.proto.usage.insert(agent.to_string(), u);
                    true
                }
                None => false,
            },
            pthread::lines::Rec::Obs(pthread::lines::Obs::CompactionDone(_)) => self.proto.usage.remove(agent).is_some(),
            _ => false,
        }
    }

    /// P4b: the repo's flow as the typed `flow` (`hub/flow`).
    pub(in crate::daemon) fn flow_ev(&self) -> HubEv {
        let flow = self.hub.flow.map(|f| match f {
            crate::flow::FlowMode::Pr => FlowMode::Pr,
            crate::flow::FlowMode::Trunk => FlowMode::Trunk,
        });
        HubEv::Flow { project: self.project(), flow }
    }

    /// P4b: an agent's live line for its head (`last_pos`, heads.rs): a
    /// subscribed thread's live fold gives its newest pos, else the
    /// head's tail is folded. True when its newest entry moved.
    fn proto_head_line(&mut self, agent: &str, line: pthread::Line) -> bool {
        let known = self.proto.conns.values().filter_map(|c| c.subs.get(agent)).filter_map(|l| l.last_pos()).max();
        let facts = self.facts();
        self.setup();
        let setup = &self.setup;
        let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
        self.proto.heads.push(agent, line, known, &ctx)
    }

    /// P4b: each agent the heads don't know yet, from the lines the hub
    /// buffered of it (its transcript's tail at start), folded once.
    fn seed_heads(&mut self, snap: &Value) {
        let names: Vec<String> = snap["agents"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str().map(String::from)).filter(|n| !self.proto.heads.has(n)).collect();
        if names.is_empty() {
            return;
        }
        let facts = self.facts();
        self.setup();
        let setup = &self.setup;
        let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
        for n in names {
            let lines: Vec<pthread::Line> = self.buffers.get(&n).into_iter().flatten().map(|(p, ts, l)| (*p as u64, *ts, l.clone())).collect();
            self.proto.heads.seed(&n, lines, &ctx);
        }
    }

    /// P4b: an agent renamed: its head follows it.
    pub(in crate::daemon) fn proto_renamed(&mut self, old: &str, new: &str) {
        self.proto.heads.rename(old, new);
    }

    /// The `agents` rows again, to `ids` when they changed (an agent's
    /// context moved between two state broadcasts).
    fn proto_agents_again(&mut self, ids: &[ClientId]) {
        let snap = self.snapshot();
        let (agents, _) = self.proto_rows(&snap);
        let a = agents.encode();
        if a == self.proto.last.0 {
            return;
        }
        self.proto.last.0 = a;
        self.proto_note(ids, &agents);
    }

    /// Every subscription refolded: the entries that changed go out.
    fn proto_live(&mut self, f: impl Fn(&mut Live, &Ctx) -> Vec<pthread::Entry>) {
        let agents: BTreeSet<String> = self.proto.conns.values().flat_map(|c| c.subs.keys().cloned()).collect();
        for a in agents {
            self.proto_live_of(&a, &f);
        }
    }

    /// One agent's subscriptions through `f`; the clients subscribed.
    fn proto_live_of(&mut self, agent: &str, f: impl Fn(&mut Live, &Ctx) -> Vec<pthread::Entry>) -> Vec<ClientId> {
        let facts = self.facts();
        self.setup();
        let setup = &self.setup;
        let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
        let mut out: Vec<(ClientId, Vec<pthread::Entry>)> = Vec::new();
        for (id, c) in self.proto.conns.iter_mut() {
            if let Some(l) = c.subs.get_mut(agent) {
                out.push((*id, f(l, &ctx)));
            }
        }
        let project = self.project();
        let mut ids = Vec::new();
        for (id, entries) in out {
            for e in entries {
                self.proto_send(id, &HubEv::Entry { project: project.clone(), agent: agent.to_string(), entry: Box::new(e) });
            }
            ids.push(id);
        }
        ids
    }
}

/// The bytes of a transcript's end read to seed an agent's usage: enough
/// for its last usage line.
const SEED_BYTES: u64 = 64 * 1024;

/// The lines in the last `bytes` of a transcript, without their `<ms>\t`
/// stamp (a line cut by the start is dropped).
fn transcript_tail(path: &std::path::Path, bytes: u64) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return Vec::new() };
    let from = f.metadata().map(|m| m.len()).unwrap_or(0).saturating_sub(bytes);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(from)).is_err() || f.take(bytes).read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    if from > 0 {
        lines.next();
    }
    lines.map(|l| l.split_once('\t').map_or(l, |(_, line)| line).to_string()).collect()
}

/// The mtimes of config.toml and auth.json (none: no file).
fn models_mtimes() -> (Option<std::time::SystemTime>, Option<std::time::SystemTime>) {
    let home = bise_home::Home::from_env();
    let at = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    (at(home.config_file()), at(home.auth_file()))
}
