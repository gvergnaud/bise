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
use crate::snapshot::Snapshot;

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

    /// The rows of the snapshot: `agents` (the live ones, agents-light;
    /// `archived` none: [`Shell::agents_for`] gives each connection its
    /// shape), `cards`.
    pub(in crate::daemon) fn proto_rows(&mut self, snap: &Snapshot) -> (HubEv, HubEv) {
        let project = self.project();
        let agents = self.agent_rows(snap, proto_view::Which::Live);
        let (cards, others) = proto_view::cards(snap, &project, now_ms(), crate::model::user_kind);
        let places = proto_view::places(snap);
        (HubEv::Agents { project: project.clone(), agents, places, archived: None }, HubEv::Cards { project, cards, others })
    }

    /// The agents event for connection `id`: a light one gets the live
    /// rows and the archived list when it hasn't got this one yet
    /// (always its first).
    pub(in crate::daemon) fn agents_for(&mut self, id: ClientId, snap: &Snapshot, live: &HubEv) -> HubEv {
        let (list, hash) = self.archived_now(snap);
        with_archived(live, self.proto.archived_for(id, &list, hash))
    }

    /// agents-light: the archived agents' light rows now, and their hash
    /// (what a connection got last is compared by it).
    fn archived_now(&mut self, snap: &Snapshot) -> (Vec<bise_proto::rows::ArchivedAgent>, u64) {
        use std::hash::{Hash, Hasher};
        let list = proto_view::archived(snap, &mut self.proto.since, now_ms());
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&list).unwrap_or_default().hash(&mut h);
        (list, h.finish())
    }

    /// `agent/get`: one agent's full row, archived or not (the same
    /// builder as the agents event's rows).
    pub(in crate::daemon) fn agent_row(&mut self, snap: &Snapshot, name: &str) -> Option<bise_proto::rows::Agent> {
        self.agent_rows_with(snap, |snap, since, now, vision, usage, head| proto_view::agent_named(snap, name, since, now, crate::model::user_kind, vision, usage, head))
    }

    /// The snapshot's agents as rows, `which` of them.
    fn agent_rows(&mut self, snap: &Snapshot, which: proto_view::Which) -> Vec<bise_proto::rows::Agent> {
        let mut rows = self.agent_rows_with(snap, |snap, since, now, vision, usage, head| proto_view::agents(snap, since, now, crate::model::user_kind, vision, usage, head, which));
        // agent-rank: the rows' `rank`, a pure function of them (proto_view/rank.rs)
        proto_view::rank::apply(&mut rows);
        rows
    }

    /// The row builder's inputs (the live agents seeded, the catalog,
    /// their usage and heads), given to `make`.
    #[allow(clippy::type_complexity)]
    fn agent_rows_with<T>(
        &mut self,
        snap: &Snapshot,
        make: impl FnOnce(&Snapshot, &mut proto_view::Since, u64, &dyn Fn(&str) -> Option<bool>, &dyn Fn(&str) -> Option<AgentUsage>, &dyn Fn(&str) -> Option<bise_proto::Pos>) -> T,
    ) -> T {
        let now = now_ms();
        self.seed_usage(snap);
        self.seed_heads(snap);
        // K4: each agent's model read against the hub's catalog (his
        // config.toml merged, re-read when it changes), fresh each time
        let cat = Some(&self.config.setup().catalog);
        let vision = |m: &str| cat.and_then(|c| c.vision(m));
        // S13: each agent's context after its last call, its window from
        // the same catalog, the gauge's words the TUI's divider says
        let calls = &self.proto.usage;
        let usage = |name: &str| {
            let u = calls.get(name)?;
            let window = cat.map(|c| c.context_window(&u.model)).filter(|w| *w > 0);
            Some(AgentUsage::of(&u.model, u.input, u.output, window))
        };
        let heads = &self.proto.heads;
        let head = |name: &str| heads.last(name);
        make(snap, &mut self.proto.since, now, &vision, &usage, &head)
    }

    /// S13 (amb-win m_10985, architect m_10999): an agent the hub has no
    /// usage for yet gets it once from its transcript's tail, by the rule
    /// the TUI uses on its feed (`lines::current_usage`: the last usage
    /// line, none when a compaction ended after it), so a hub restart no
    /// longer blanks the gauge. Once per agent, never per broadcast; live
    /// lines then move it (`proto_usage_line`). No usage, no row: never
    /// an invented 0. Live agents only (`proto_view::seeds`): his 888
    /// archived tails were 18 % of `initialize`.
    fn seed_usage(&mut self, snap: &Snapshot) {
        for name in crate::proto_view::seeds(snap).map(str::to_string) {
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
        self.proto.models_at = Some(models_mtimes(&self.opts.home));
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let home = self.opts.home.clone();
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
        self.proto.models_at.is_some_and(|at| at != models_mtimes(&self.opts.home))
    }

    /// The hub's `approvals` event (gate.rs `approvals_ev`, the TUI's) as
    /// the typed one (bar V8/W21); paths in the words as the TUI says them.
    pub(in crate::daemon) fn proto_approvals(&self, ev: &Value) -> HubEv {
        let home = std::env::var("HOME").ok();
        proto_view::approvals::approvals(&self.project(), ev, home.as_deref())
    }

    /// The typed `pages` of a snapshot (its `pages` rows and `overdue`,
    /// `snapshot_pages`).
    pub(in crate::daemon) fn pages_ev(&self, snap: &Snapshot) -> HubEv {
        HubEv::Pages { project: self.project(), items: snap.pages.clone(), overdue: snap.overdue }
    }

    /// A hub line whose fields are its typed event's (`versions`,
    /// `release`, `update`): that event, with this hub's project.
    pub(in crate::daemon) fn typed_of(&self, v: &Value) -> Option<HubEv> {
        let mut v = v.clone();
        v["project"] = json!(self.project());
        match HubEv::from_value(v) {
            Ok(HubEv::Unknown { .. }) => None,
            Ok(ev) => Some(ev),
            Err(e) => {
                crate::daemon::log_line(&self.opts.paths, &format!("not typed: {e}"));
                None
            }
        }
    }

    /// The art store's event as the typed `artifacts`.
    pub(in crate::daemon) fn proto_artifacts(&self, ev: &Value) -> HubEv {
        HubEv::Artifacts {
            project: self.project(),
            items: self.artifact_rows(ev),
            seen_ms: ev.get("seen_ms").and_then(Value::as_u64),
            folders: ev.get("folders").and_then(|f| serde_json::from_value(f.clone()).ok()).unwrap_or_default(),
        }
    }

    /// The art store's event (`artifacts_ev`) as rows: the one builder of
    /// the typed `artifacts` event and of `view.json`'s artifacts (⌘K's
    /// index, architect m_11910), so the two never disagree.
    pub(in crate::daemon) fn artifact_rows(&self, ev: &Value) -> Vec<bise_proto::rows::Artifact> {
        let pages = self.pg.pages.clone();
        let page_url = |id: &str| pages.as_ref().filter(|p| p.store.meta(id).is_some()).map(|p| p.url(id));
        // a copied file framed by the page server (ambient-lead m_19544,
        // architect m_19561): the window never frames file://
        let base = pages.as_ref().map(|p| p.base());
        proto_view::artifacts(ev, page_url, base.as_deref())
    }

    /// The hub's state changed (push_state, state_now): each typed event
    /// it feeds again, only when it changed.
    pub(in crate::daemon) fn proto_state(&mut self, snap: &Snapshot) {
        let live: BTreeSet<ClientId> = self.clients.keys().copied().collect();
        self.proto.conns.retain(|id, _| live.contains(id));
        // the processes worker: the live agents, a typed client or none
        // (daemon/processes.rs; sent only when changed)
        self.procs_sync();
        if self.proto.conns.is_empty() {
            return;
        }
        let ids: Vec<ClientId> = self.proto.conns.keys().copied().collect();
        let (agents, cards) = self.proto_rows(snap);
        let (a, c) = (agents.encode(), cards.encode());
        let new = (a != self.proto.last.0, c != self.proto.last.1);
        self.proto.last = (a, c);
        // agents-light (architect m_37000 2.): the live rows when they
        // changed, the archived list only to a connection that hasn't got
        // this one (a usage tick never resends it)
        let (list, hash) = self.archived_now(snap);
        let mut outs = Vec::new();
        for id in &ids {
            let arch = self.proto.archived_for(*id, &list, hash);
            if new.0 || arch.is_some() {
                outs.push((*id, with_archived(&agents, arch)));
            }
        }
        self.proto_note_each(&outs);
        // les bims: the project's growth, when it changed
        self.growth_changed(&ids, Some(snap), None);
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
        // cost-pause: the project's pause, when it changed
        let paused = self.paused_ev();
        let p = paused.encode();
        if p != self.proto.paused {
            self.proto.paused = p;
            self.proto_note(&ids, &paused);
        }
        // S10: the followed jobs, when they changed
        let jobs = HubEv::Jobs { project: self.project(), items: proto_view::jobs(snap) };
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
        // P4c-5: the pages and his late promises (the snapshot's)
        let pages = self.pages_ev(snap);
        let p = pages.encode();
        if p != self.proto.pages {
            self.proto.pages = p;
            self.proto_note(&ids, &pages);
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
            self.worktrees_typed(Typed::All);
        }
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
            // bar V8/W21: the mode switched or a rule left (set_mode,
            // remove_rule broadcast the TUI's event): the typed one to all
            Some("approvals") => {
                let ev = self.proto_approvals(v);
                self.proto_note(&ids, &ev);
            }
            // P4c-5: `/version`'s picker (at a build's start and end, a
            // switch), a release run's steps, `/update`'s build: the
            // older line's fields are the typed event's
            // and one page that changed (before the state that lists it)
            Some("versions" | "release" | "update") => {
                if let Some(ev) = self.typed_of(v) {
                    self.proto_note(&ids, &ev);
                }
            }
            // its typed tag is page_changed (the older `page` line keeps
            // its own meaning)
            Some("page") => {
                let mut changed = v.clone();
                changed["ev"] = json!("page_changed");
                if let Some(ev) = self.typed_of(&changed) {
                    self.proto_note(&ids, &ev);
                }
            }
            // the art store changed (artifacts_refresh broadcasts it)
            Some("artifacts") => {
                let arts = self.proto_artifacts(v);
                self.proto_note(&ids, &arts);
                if let HubEv::Artifacts { items, .. } = &arts {
                    self.growth_changed(&ids, None, Some(items.len() as u32));
                    self.pulse_artifacts(items);
                }
            }
            // the answer as it streams in: to the connections subscribed
            Some("partial") => {
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let (agent, call, text) = (s("agent"), s("call"), s("text"));
                let off = v.get("off").and_then(Value::as_u64).map_or(0, |o| u32::try_from(o).unwrap_or(u32::MAX));
                let project = self.project();
                let subs: Vec<ClientId> = self.proto.conns.iter().filter(|(_, c)| c.subs.contains_key(&agent)).map(|(id, _)| *id).collect();
                for id in subs {
                    self.proto_send(id, &HubEv::Partial { project: project.clone(), agent: agent.clone(), call: call.clone(), off, text: text.clone() });
                }
            }
            Some("line") => {
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let (agent, line) = (s("agent"), s("line"));
                let pos = v.get("pos").and_then(Value::as_u64).unwrap_or(0);
                let ts = v.get("ts").and_then(Value::as_u64).unwrap_or(0);
                let step = proto_view::step_of(&line);
                let sent = self.proto_live_of(&agent, |l, ctx| l.push((pos, ts, line.clone()), ctx));
                let moved = self.proto_head_line(&agent, (pos, ts, line.clone()));
                // its head or its usage moved: the agents rows go with the
                // next gated state push (at most EVERY_MS away), never a
                // whole snapshot per line (hub-lag-7: one per tool line,
                // with git subprocesses, was 40% of a flooded loop)
                if self.proto_usage_line(&agent, &line) || moved {
                    self.state_gate.changed();
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

    /// cost-pause: the project's pause as the typed `paused`
    /// (`hub/paused`), from sb-core's view; `held` its queued messages.
    pub(in crate::daemon) fn paused_ev(&self) -> HubEv {
        let st = &self.hub.st;
        let held = match st.paused {
            Some(_) => st.msg_state().values().filter(|s| matches!(s, crate::model::MsgState::Queued { .. })).count() as u64,
            None => 0,
        };
        HubEv::Paused { project: self.project(), paused: st.paused.is_some(), since_ms: st.paused, held }
    }

    /// The hub's newest version switch as the typed `switch`
    /// (`hub/switch`), from switch.rs's own facts in versions.json
    /// (switch::row: a 'reloading' to this hub's own version reads 'up').
    pub(in crate::daemon) fn switch_ev(&self) -> HubEv {
        let st = crate::switch::read_state(&self.opts.paths);
        let mine = crate::switch::id_of(&self.opts.app_root);
        HubEv::Switch { project: self.project(), switch: crate::switch::row(&st, &mine) }
    }

    /// The switch row to every connection, when it moved (the switcher's
    /// notices: its start, its probation passed, its rollback).
    pub(in crate::daemon) fn switch_moved(&mut self) {
        let ev = self.switch_ev();
        let e = ev.encode();
        if e != self.proto.switch {
            self.proto.switch = e;
            let ids: Vec<ClientId> = self.proto.conns.keys().copied().collect();
            self.proto_note(&ids, &ev);
        }
    }

    /// P4b: an agent's live line for its head (`last_pos`, heads.rs): a
    /// subscribed thread's live fold gives its newest pos, else the
    /// head's tail is folded. True when its newest entry moved.
    fn proto_head_line(&mut self, agent: &str, line: pthread::Line) -> bool {
        let known = self.proto.conns.values().filter_map(|c| c.subs.get(agent)).filter_map(|l| l.last_pos()).max();
        let facts = self.facts();
        let setup = self.config.setup();
        let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
        self.proto.heads.push(agent, line, known, &ctx)
    }

    /// P4b: each agent the heads don't know yet, from the lines the hub
    /// buffered of it (its transcript's tail at start), folded once. Live
    /// agents only (`proto_view::seeds`): folding every archived agent's
    /// lines was 72 % of `initialize` on his 888 agents.
    fn seed_heads(&mut self, snap: &Snapshot) {
        let names: Vec<String> = crate::proto_view::seeds(snap).filter(|n| !self.proto.heads.has(n)).map(str::to_string).collect();
        if names.is_empty() {
            return;
        }
        for n in &names {
            // an agent archived at boot, back now: its feed from the file
            if let Some(dir) = self.dir_of(n) {
                self.load_feed(n, &dir);
            }
        }
        let facts = self.facts();
        let setup = self.config.setup();
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
        let setup = self.config.setup();
        let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
        let mut out: Vec<(ClientId, Vec<pthread::Entry>)> = Vec::new();
        for (id, c) in self.proto.conns.iter_mut() {
            if let Some(l) = c.subs.get_mut(agent) {
                out.push((*id, f(l, &ctx)));
            }
        }
        let project = self.project();
        let dir = self.dir_of(agent);
        let mut ids = Vec::new();
        for (id, mut entries) in out {
            if let Some(d) = &dir {
                self.expire_views(d, &mut entries);
            }
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
fn models_mtimes(home: &bise_home::Home) -> (Option<std::time::SystemTime>, Option<std::time::SystemTime>) {
    let at = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    (at(home.config_file()), at(home.auth_file()))
}

/// The live agents event with `archived` set (agents-light).
fn with_archived(live: &HubEv, archived: Option<Vec<bise_proto::rows::ArchivedAgent>>) -> HubEv {
    match live {
        HubEv::Agents { project, agents, places, .. } => HubEv::Agents { project: project.clone(), agents: agents.clone(), places: places.clone(), archived },
        other => other.clone(),
    }
}
