//! The core as a client of every project's hub (bise desktop S3b step 2):
//! one JSON-RPC connection (`initialize` first, client-protocol P1c,
//! `hub_rpc.rs`; a hub that doesn't serve it is refused with
//! `bise_proto::rpc::HUB_OLDER`'s words, or his install's steps when it
//! runs from one: `ProjectFacts::older`) per held project, the
//! window's commands with a `project` routed to it, its typed events out
//! to the app as they are (they carry `project`), and `projects` (the
//! sidebar's rows) from the registry, the held hubs' own `agents`/`cards`
//! and the other hubs' `view.json`.
//!
//! Held (one rule, `projects::wanted`): bise's home hub, the projects the
//! window shows (`shown`), the ones with a subscribed thread. Before the
//! window's first `shown`, only bise's home hub is held, and only when the
//! core runs for a window (with `ProjectPorts`; the capsule alone opens no
//! connection of this kind): the registry is read once at start (no poll,
//! no `projects` out) and home's hub is connected, started if needed, so
//! his first send right after the window shows goes at once instead of
//! waiting ~2 s for its welcome (bar ⛔3, architect m_9864). Starting it is
//! allowed: that window exists to talk to it, this is not starting a hub
//! just to read. A command to home before its welcome waits for it and
//! goes once; any other project still waits for `shown`. A project not held is listed from its files,
//! never started to be listed; the files are read at most every 2 s, and
//! only while the window has said what it shows.
//!
//! The capsule and the voice keep their own connection to the voice
//! target's hub (core.rs: its legacy feed and `agents_view.rs`); it goes
//! when they move to these typed events (S8), and with it the second
//! reader of that hub's cards.
//!
//! Never buffered (architect m_8433 D): a command to a hub that isn't
//! connected yet gets `error {project, cmd, text}`; only subscriptions
//! are kept (state, not commands) and sent again at each `welcome`.
//!
//! One request, one answer (architect m_23524): the app's `hub/<tag>`
//! request is answered once (`Core::take_replies`), with its hub's own
//! response (`hub_rpc.rs`), the core's refusal (the same words as its
//! `error`), `{}` when the core keeps it, or an error when its hub goes or
//! never answers. Not covered: the capsule's `answer` (core.rs, the home
//! connection's `home_call`, whose answers are not read).

use super::hub_rpc::{Read, RpcConn};
use super::leases::{Leases, SCREEN};
use super::*;
use crate::ambient::projects::{self, Checkout, Facts};
use bise_home::projects::Row;
use bise_proto::app_rows::ProjectRow;
use bise_proto::draft::ProjectView;
use bise_proto::hub::{ErrorKind, HubEv};
use bise_proto::rpc::{Id, Response, RpcError};
use bise_proto::rows::{Agent, Card};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The registry's files are read at most this often.
pub const POLL: Duration = Duration::from_secs(2);

/// A connection to a project's hub (its reader thread tags what it reads
/// with the project id).
pub type Spawn = Box<dyn FnMut(&Path, &str) -> Hub>;
/// A hub's `view.json` by hub id.
pub type ViewOf = Box<dyn Fn(&str) -> Option<ProjectView>>;
/// What to do with a hub id's hub from before client-protocol, when it
/// runs from an install (`ProjectFacts::older`).
pub type OlderOf = Box<dyn Fn(&str) -> Option<String>>;
/// A checkout's branch and whether it is a git repo.
pub type GitOf = Box<dyn Fn(&Path) -> Checkout>;
/// A hub's journal.jsonl by hub id (none: no file).
pub type JournalOf = Box<dyn Fn(&str) -> Option<String>>;

/// What the core reads of the projects, real in `bise ambient-core`,
/// fakes in tests.
pub struct ProjectFacts {
    /// the registry's rows, home first (`bise_home::projects::list`)
    pub rows: Box<dyn Fn() -> Vec<Row>>,
    /// a hub's `view.json` by hub id (`projects::read_view`)
    pub view: ViewOf,
    /// its hub runs (its pid lives)
    pub running: Box<dyn Fn(&Path) -> bool>,
    pub exists: Box<dyn Fn(&Path) -> bool>,
    /// its checkout's branch and whether it is a git repo
    pub git: GitOf,
    /// what to do with this hub id's hub from before client-protocol when
    /// it runs from an install (`switchboard::client::older_words`: the
    /// terminal's same words); None: `rpc::HUB_OLDER`
    pub older: OlderOf,
    /// a hub's journal by hub id, read-only (inbox-record: the history of
    /// a project whose hub isn't connected); none: no file
    pub journal: JournalOf,
}

pub struct ProjectPorts {
    pub spawn: Spawn,
    pub facts: ProjectFacts,
}

impl Hubs {
    /// Whether the core holds this project's hub now (⌘K's index leaves it out).
    pub(super) fn holds(&self, id: &str) -> bool {
        self.conns.contains_key(id)
    }

    /// The live agents of project `id` as its hub last sent them (none:
    /// not connected yet).
    pub(super) fn live_agents(&self, id: &str) -> Option<Vec<String>> {
        let (agents, _) = self.conns.get(id)?.rows.as_ref()?;
        Some(agents.iter().filter(|a| !a.archived).map(|a| a.name.clone()).collect())
    }
}

/// One held project's connection.
struct Conn {
    hub: Hub,
    /// its `welcome` came since the last connection: commands go
    welcomed: bool,
    /// the connection its `initialize` went on ([`Hub::epoch`]): its
    /// commands go on that one only. The reader connects again on its
    /// own, before the core has read its Down/Up: a command typed then
    /// went on the new connection ahead of its `initialize`, neither
    /// written nor errored (fast-loop, architect m_53375)
    epoch: u64,
    /// the reader's Ups read so far: one a connect, in order, so this Up
    /// is connection `ups`; behind [`Hub::epoch`], a newer connection's
    /// own Up waits behind it and this one says nothing (two reconnects
    /// before the core reads either: one `initialize`, architect m_53385)
    ups: u64,
    /// its own rows, as its hub sent them (fresher than its view.json)
    rows: Option<(Vec<Agent>, Vec<Card>)>,
    /// agents-light: its hub's archived light list as last sent (merged
    /// into `rows`)
    archived: bise_proto::rows::Archived,
    /// commands for a hub not welcomed yet, each with its deadline and
    /// the app's request (sent at the welcome, else an error at the
    /// deadline)
    pending: Vec<(Value, Instant, Option<Id>)>,
    /// held for a command only (a project not shown, not subscribed):
    /// closed once this passes with nothing pending
    until: Option<Instant>,
    /// hub-skew: the commands its welcome listed (empty: not welcomed yet)
    cmds: Vec<String>,
    /// its JSON-RPC side (P1c, `hub_rpc.rs`)
    rpc: RpcConn,
    /// why its hub couldn't start (`HubIn::Down`'s cause), until it's up:
    /// the row's `down`
    down: Option<String>,
    /// its hub's last `hub/pulse` (the row's `pulse` while it is held)
    pulse: Option<bise_proto::rows::Pulse>,
    /// update-flow: its hub's last `hub/bise_update` (None: on it)
    bise_update: Option<bise_proto::rows::BiseUpdate>,
}

impl Conn {
    fn new(hub: Hub, until: Option<Instant>) -> Conn {
        Conn { hub, welcomed: false, epoch: 0, ups: 0, rows: None, archived: Default::default(), pending: Vec::new(), until, cmds: Vec::new(), rpc: RpcConn::default(), down: None, pulse: None, bise_update: None }
    }

    /// One window command (a `HubCmd`'s JSON) to its hub, as a request
    /// once `initialize` answered, on the connection that answered it;
    /// false when nothing was written (the command takes the pending path).
    fn send(&mut self, v: &Value, app: Option<Id>) -> bool {
        if !self.rpc.ready || self.hub.epoch() != self.epoch {
            return false;
        }
        match self.rpc.request(v, app, Instant::now()) {
            Some(req) => self.hub.send_on(self.epoch, &req),
            None => false,
        }
    }

    /// Kept though no rule holds it: a command waits or was just sent.
    fn lingers(&self, now: Instant) -> bool {
        !self.pending.is_empty() || self.until.is_some_and(|u| u > now)
    }
}

/// How long a command waits for its hub's welcome (it may start).
pub const START: Duration = Duration::from_secs(20);
/// How long a hub held for a command stays held after it went (its
/// answer, an error, the echo of a send).
pub const LINGER: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct Hubs {
    pub(super) ports: Option<ProjectPorts>,
    pub(super) rows: Vec<Row>,
    /// the projects the window shows; None: no window yet
    shown: Option<Vec<String>>,
    conns: BTreeMap<String, Conn>,
    /// each project's subscribed agents (kept across reconnections)
    subs: BTreeMap<String, BTreeSet<String>>,
    /// each row's (branch, git), read at each poll
    git: BTreeMap<String, Checkout>,
    last: String,
    polled: Option<Instant>,
    /// J: the job ends the window got, (project, agent, key), newest last
    ends: VecDeque<(String, String, u64)>,
    /// voice mode's thread (project, agent): followed by the core only
    /// while the window doesn't follow it itself (core/voice_mode.rs)
    voice: Option<(String, String)>,
    /// the projects whose hub refused this core's connection, with its
    /// words (HubEv::Refused, docs/issues/16; or `bise_proto::rpc::HUB_OLDER`
    /// for a hub that doesn't serve `initialize`): never connected again until
    /// he retries (`hub_retry`) or a new core starts. The hub's verdict is
    /// about this core's process (a core started by an agent, e.g. an
    /// ambient-qa run on a real workspace, is refused by design), so
    /// retrying on our own would never change it
    refused: BTreeMap<String, String>,
    /// why bise's own (home) hub couldn't start, until it's up: the home
    /// row's `down` (its connection is the core's `hub`, not a `Conn`)
    pub(super) home_down: Option<String>,
    /// who watches the processes lists (`processes_all`): the running
    /// screen holds every running hub, a process's panel its project's;
    /// leases their windows renew, so no hold outlives its screen, its
    /// window or this core (architect m_38378, m_47406; core/leases.rs)
    lists: Leases,
    /// update-flow: the machine's line the window last got (`bise_update`)
    bise_sent: Option<Option<bise_proto::rows::BiseUpdate>>,
    /// boot-speed: each project's copy of main's last page, shown before
    /// its hub's page (core/thread_cache.rs)
    threads: super::thread_cache::Threads,
}

/// The rows the running screen holds: the projects whose hub runs, never
/// one that would have to start (a stopped hub killed its processes).
pub fn listed(rows: &[Row], running: impl Fn(&Path) -> bool) -> BTreeSet<String> {
    rows.iter().filter(|r| running(&r.path)).map(|r| r.id.clone()).collect()
}

/// What a command to a refused project's hub answers (kind hub_refused).
fn refused_text(name: &str, why: &str) -> String {
    format!("{name}'s hub refused this connection: {why}")
}

/// hub-skew (architect m_11314): what the window says when this
/// project's hub doesn't know a command, or is too old to say which.
/// (A hub that doesn't serve `initialize` at all is another fact, with
/// its own words: `bise_proto::rpc::HUB_OLDER`.)
pub const HUB_LACKS: &str = "this project's hub is older: update bise";

/// hub-skew: a hub that listed its commands (`welcome.cmds`) and lacks
/// `tag`, the typed command's own tag (never its text: architect
/// m_11487). An empty list (not welcomed yet) lacks nothing we can know.
/// A command's error when its hub never welcomed it: the start's cause
/// when the connector said one (`HubIn::Down`: client::open's 'the hub
/// did not start: <why> (its log: ..)', a missing runtime), never a bare
/// 'try again' that a retry wouldn't fix.
pub fn not_started(down: Option<&str>) -> String {
    match down.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => {
            let why = d.strip_prefix("the hub did not start: ").or_else(|| d.strip_prefix("the hub ")).unwrap_or(d);
            format!("its hub didn't start: {}. nothing sent.", why.trim_end_matches('.'))
        }
        None => format!("its hub didn't start in {} s: nothing sent.", START.as_secs()),
    }
}

pub fn lacks(cmds: &[String], tag: &str) -> bool {
    !cmds.is_empty() && !cmds.iter().any(|c| c == tag)
}

/// J: how many job ends the core remembers to drop a second copy.
const ENDS_KEPT: usize = 64;

/// J (architect m_10223): one bar line per (project, agent, key): the
/// first of a project's own `job_end` and bise's `followed_end` of the same
/// job goes to the window, the other is dropped. A `job_end` without a key
/// (an older hub) always goes; anything else is not an end.
pub fn first_end(seen: &mut VecDeque<(String, String, u64)>, ev: &HubEv) -> bool {
    let k = match ev {
        HubEv::JobEnd { project, agent, key: Some(key), .. } | HubEv::FollowedEnd { project, agent, key, .. } => (project.clone(), agent.clone(), *key),
        _ => return true,
    };
    if seen.contains(&k) {
        return false;
    }
    seen.push_back(k);
    if seen.len() > ENDS_KEPT {
        seen.pop_front();
    }
    true
}

/// The projects to hold: bise's home alone before the window says what it
/// shows; then `projects::wanted` (home, shown, subscribed), as ids of
/// known rows.
pub fn held(rows: &[Row], shown: Option<&[String]>, subs: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let Some(home) = rows.iter().find(|r| r.home) else { return BTreeSet::new() };
    let Some(shown) = shown else { return BTreeSet::from([home.id.clone()]) };
    let path = |id: &String| rows.iter().find(|r| &r.id == id).map(|r| r.path.clone());
    let shown: Vec<PathBuf> = shown.iter().filter_map(path).collect();
    let subscribed: Vec<PathBuf> = subs.iter().filter(|(_, a)| !a.is_empty()).filter_map(|(id, _)| path(id)).collect();
    let want = projects::wanted(&home.path, &shown, &[], &subscribed);
    rows.iter().filter(|r| want.contains(&r.path)).map(|r| r.id.clone()).collect()
}

/// A subscribe or unsubscribe of the window, kept: true when it changed.
pub fn track(subs: &mut BTreeMap<String, BTreeSet<String>>, cmd: &str, project: &str, agent: &str) -> bool {
    let s = subs.entry(project.to_string()).or_default();
    let changed = match cmd {
        "subscribe" => s.insert(agent.to_string()),
        "unsubscribe" => s.remove(agent),
        _ => false,
    };
    if s.is_empty() {
        subs.remove(project);
    }
    changed
}

/// The branch a `.git/HEAD` names (`ref: refs/heads/<b>`); None when
/// detached.
pub fn branch_of_head(head: &str) -> Option<String> {
    head.trim().strip_prefix("ref: refs/heads/").map(str::to_string).filter(|b| !b.is_empty())
}

/// A project's inbox: hub/cards' his cards and bise's own (`others`: a
/// drop asking him, a done, a merge...) in one list by id, as the TUI's
/// (state_rows::cards_of_rows). The window lists them all (ambient-lead
/// m_20217); the capsule keeps its own no-drop filter (home.rs).
fn inbox_cards(cards: Vec<Card>, others: Vec<Card>) -> Vec<Card> {
    if others.is_empty() {
        return cards;
    }
    let mut all: Vec<Card> = cards.into_iter().chain(others).collect();
    all.sort_by_key(|c| c.id);
    all
}

/// A held hub's own rows as a view (the sidebar's counts).
fn live_view(project: &str, agents: &[Agent], cards: &[Card]) -> ProjectView {
    ProjectView { v: 1, project: project.into(), written_ms: 0, stopped_ms: None, last_activity_ms: 0, agents: agents.to_vec(), cards: cards.to_vec(), artifacts: vec![], artifacts_total: 0, scheduled: vec![], growth: None, pulse: None }
}

impl Core {
    /// One JSON-RPC request on the home connection (P3b: his words,
    /// his command line, a card's answer, a stop, an archive); false:
    /// bise isn't reachable.
    pub(super) fn home_call(&self, method: &str, params: Value) -> bool {
        let project = bise_home::hub_id(std::path::Path::new(&self.workspace));
        self.hub.send(&super::hub_rpc::home(&project, method, params))
    }

    /// The projects' ports (`bise ambient-core`, a test): without them a
    /// command with a project is refused.
    /// For a window: the registry read once and bise's home hub held now
    /// (see the header), before the window loads.
    pub fn set_projects(&mut self, p: ProjectPorts) {
        self.hubs.rows = (p.facts.rows)();
        self.hubs.ports = Some(p);
        self.hold();
    }

    /// `cid` (G): the failed send's, echoed so the window fails exactly
    /// that row (nothing reached a thread: reason refused). `kind`: what
    /// failed, for the window to draw (bise_proto::hub::ErrorKind). `app`:
    /// the app's request for it, answered with the same words.
    fn project_error(&mut self, project: &str, cmd: &str, text: &str, cid: Option<u64>, kind: Option<bise_proto::hub::ErrorKind>, app: Option<Id>) {
        if let Some(app) = app {
            let mut e = RpcError::refused(text, Some("refused"));
            if let Some(k) = kind {
                e = e.with_kind(k);
            }
            self.replies.push(Response::err(Some(app), e));
        }
        let mut ev = json!({"ev": "error", "project": project, "cmd": cmd, "text": text});
        if let Some(cid) = cid {
            ev["cid"] = json!(cid);
            ev["reason"] = json!("refused");
        }
        if let Some(k) = kind {
            ev["kind"] = json!(k);
        }
        self.emit(ev);
    }

    /// `shown {projects}`: the projects the window shows now.
    /// J: whether bise's home hub's typed line goes on (not the second
    /// copy of a job end the window already has, [`first_end`]).
    pub(super) fn end_once(&mut self, ev: &HubEv) -> bool {
        first_end(&mut self.hubs.ends, ev)
    }

    pub(super) fn shown(&mut self, projects: Vec<String>) {
        self.hubs.shown = Some(projects);
        self.poll_projects(true);
    }

    /// `hub_retry {project}`: his action on a refused project, its hub
    /// held again (a new refusal says hub_refused again); a project that
    /// isn't refused: nothing.
    pub(super) fn hub_retry(&mut self, project: &str) {
        // the home hub's failed move to this version: move it again
        if self.home_move_retry(project) {
            return;
        }
        if self.hubs.refused.remove(project).is_some() {
            self.hold();
            self.emit_projects();
        }
    }

    /// A command of the core's own for a project's hub (voice mode, a
    /// held `/model`): no app request waits on it.
    pub(super) fn typed_cmd(&mut self, v: Value) {
        self.typed_req(v, None);
    }

    /// A window command for a project's hub (a `HubCmd` with `project`).
    /// `app`: the app's request (`hub/<tag>`), answered exactly once: with
    /// its hub's response, the core's refusal, or `{}` when the core keeps
    /// it (a subscription, a held `/model`) (architect m_23524).
    pub(super) fn typed_req(&mut self, v: Value, app: Option<Id>) {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let (project, cmd, agent) = (s("project"), s("cmd"), s("agent"));
        let cid = v.get("cid").and_then(Value::as_u64);
        if self.hubs.ports.is_none() {
            return self.project_error(&project, &cmd, "this core has no projects", cid, None, app);
        }
        let Some(row) = self.hubs.rows.iter().find(|r| r.id == project).cloned() else {
            return self.project_error(&project, &cmd, &format!("there's no project {project}"), cid, None, app);
        };
        // before shown, home only (architect m_10133: its own words, so a
        // window that sends too early reads plainly in the logs)
        if !row.home && self.hubs.shown.is_none() {
            return self.project_error(&project, &cmd, &format!("project {project} isn't shown yet"), cid, None, app);
        }
        // the inbox's history of a project whose hub isn't connected: its
        // journal, folded here, never a hub started to read it
        if cmd == "inbox_history" && !self.hubs.conns.get(&project).is_some_and(|c| c.welcomed) {
            return self.history_from_journal(&project, &v, app);
        }
        // its hub refused this core: nothing goes until he retries
        if let Some(why) = self.hubs.refused.get(&project).cloned() {
            if cmd == "unsubscribe" {
                track(&mut self.hubs.subs, &cmd, &project, &agent);
                return self.reply_ok(app);
            }
            return self.project_error(&project, &cmd, &refused_text(&row.name, &why), cid, Some(bise_proto::hub::ErrorKind::HubRefused), app);
        }
        // never a hub for a folder that is gone (its hub would start on a
        // missing workspace)
        if cmd != "unsubscribe" && !self.folder_exists(&row.path) {
            return self.project_error(&project, &cmd, &format!("{}: its folder is gone", row.name), cid, None, app);
        }
        // R8: a /model whose provider has no key waits for its setup (setup.rs)
        if cmd == "slash" && self.hold_if_keyless(&project, &agent, &s("line"), cid) == super::setup::Hold::Held {
            return self.reply_ok(app);
        }
        if track(&mut self.hubs.subs, &cmd, &project, &agent) {
            self.hold();
        }
        // boot-speed: main's last page from this core's copy at once, while
        // its hub's page is on its way (a booting hub); replaced whole by it
        if cmd == "subscribe" {
            let path = bise_home::Home::from_env().thread_cache(&project);
            if let Some(copy) = self.hubs.threads.copy_for(&project, &agent, || super::thread_cache::read(&path)) {
                self.emit(copy.to_value());
            }
        }
        // voice mode still follows that thread: the window's unsubscribe
        // stays here (the core's own subscription goes when voice mode ends)
        if cmd == "unsubscribe" && self.hubs.voice.as_ref().is_some_and(|(p, a)| *p == project && *a == agent) {
            return self.reply_ok(app);
        }
        // his typed send while voice mode waits on his typing: its answer is said
        // the spotlight's still it names (`shot: id`) as its image part (core/shot.rs)
        let v = match cmd == "send" {
            true => {
                self.voice_mode_typed(&project, &agent);
                let v = self.send_shot(v);
                // his typed `@computer`: that plugin's part (at-plugins, core/plugin_send.rs)
                self.send_plugins(v, &project, &row.path)
            }
            false => v,
        };
        let welcomed = self.hubs.conns.get(&project).is_some_and(|c| c.welcomed);
        match (cmd.as_str(), welcomed) {
            // kept: sent at its hub's welcome
            ("subscribe" | "unsubscribe", false) => self.reply_ok(app),
            (_, true) => {
                let Some(c) = self.hubs.conns.get_mut(&project) else { return self.reply_ok(app) };
                if lacks(&c.cmds, &cmd) {
                    return self.project_error(&project, &cmd, HUB_LACKS, cid, Some(bise_proto::hub::ErrorKind::HubOlder), app);
                }
                // the write failed: its hub just went (its Down is on its
                // way); the reader connects again, the command waits
                if !c.send(&v, app.clone()) {
                    c.welcomed = false;
                    c.pending.push((v, Instant::now() + START, app));
                }
            }
            // its hub connects on demand (started if needed), the command
            // goes at its welcome, then the hold goes (decision m_8720)
            (_, false) => self.deliver_at_welcome(&project, &row.path, v, app),
        }
    }

    /// `inbox_history` for a project not connected: its journal, folded
    /// by the hub's own fold (bise_proto::inbox), read-only; a torn last
    /// line is skipped. The page goes out as its hub's would.
    fn history_from_journal(&mut self, project: &str, v: &Value, app: Option<Id>) {
        let text = self.hubs.ports.as_ref().and_then(|p| (p.facts.journal)(project)).unwrap_or_default();
        let mut index = bise_proto::inbox::Index::new(project.to_string());
        for line in text.lines() {
            if let Some((_, l)) = bise_proto::inbox::journal::CardLine::of_text(line) {
                index.read(&l);
            }
        }
        let q: bise_proto::inbox::HistoryParams = serde_json::from_value(v.clone()).unwrap_or_default();
        let ev = bise_proto::hub::HubEv::InboxHistory { project: project.to_string(), page: index.page(&q.query()) };
        if let Some(app) = app {
            self.replies.push(Response::ok(app, bise_proto::rpc::result(&ev)));
        }
        self.emit(ev.to_value());
    }

    /// The app's request done here, the core's own way (nothing for its hub to say).
    fn reply_ok(&mut self, app: Option<Id>) {
        if let Some(app) = app {
            self.replies.push(Response::ok(app, json!({})));
        }
    }

    fn folder_exists(&self, path: &Path) -> bool {
        self.hubs.ports.as_ref().is_some_and(|p| (p.facts.exists)(path))
    }

    /// `v` waits for `project`'s welcome on its connection, opened for it
    /// when no rule holds that project.
    fn deliver_at_welcome(&mut self, project: &str, path: &Path, v: Value, app: Option<Id>) {
        let now = Instant::now();
        if !self.hubs.conns.contains_key(project) {
            let Some(p) = self.hubs.ports.as_mut() else { return };
            let hub = (p.spawn)(path, project);
            self.hubs.conns.insert(project.to_string(), Conn::new(hub, Some(now + START + LINGER)));
        }
        if let Some(c) = self.hubs.conns.get_mut(project) {
            c.pending.push((v, now + START, app));
        }
    }

    /// boot-speed: main's changed copies to their files (at most every
    /// thread_cache::FLUSH_EVERY; `all`: now, at the core's end).
    pub(super) fn write_thread_copies(&mut self, now: Instant, all: bool) {
        let home = bise_home::Home::from_env();
        for (project, text) in self.hubs.threads.take_due(now, all) {
            super::thread_cache::write(&home.thread_cache(&project), text.as_deref());
        }
    }

    /// boot-speed: a removed project's copy of main goes with it.
    pub(super) fn forget_thread_copy(&mut self, project: &str) {
        self.hubs.threads.forget(project);
        super::thread_cache::write(&bise_home::Home::from_env().thread_cache(project), None);
    }

    /// Commands past their deadline get their error; the hubs held for a
    /// command only go once it went.
    pub(super) fn tick_projects(&mut self, now: Instant) {
        self.write_thread_copies(now, false);
        // (project, cmd, cid, the app's request, its words)
        type Late = (String, String, Option<u64>, Option<Id>, String);
        let mut late: Vec<Late> = Vec::new();
        let mut close = false;
        // hubs that failed to start: their connection goes, so the next
        // command starts them again (a project connection starts its hub
        // at its first attempt only)
        let mut failed: Vec<String> = Vec::new();
        for (id, c) in self.hubs.conns.iter_mut() {
            let words = not_started(c.down.as_deref());
            c.pending.retain(|(v, deadline, app)| {
                let ok = *deadline > now;
                if !ok {
                    late.push((id.clone(), v.get("cmd").and_then(Value::as_str).unwrap_or("").to_string(), v.get("cid").and_then(Value::as_u64), app.clone(), words.clone()));
                }
                ok
            });
            if c.down.is_some() && !c.welcomed && late.iter().any(|l| &l.0 == id) {
                failed.push(id.clone());
            }
            // an app request its hub never answered gets the core's words
            if let Some(before) = now.checked_sub(super::hub_rpc::ASK_LATE) {
                c.rpc.late(before, super::hub_rpc::LATE);
            }
            close |= c.until.is_some_and(|u| u <= now) && c.pending.is_empty();
        }
        self.take_rpc_replies();
        for (id, cmd, cid, app, words) in late {
            self.project_error(&id, &cmd, &words, cid, None, app);
        }
        for id in failed {
            if let Some(mut c) = self.hubs.conns.remove(&id) {
                c.hub.close();
                c.rpc.lost(super::hub_rpc::GONE);
                self.replies.extend(c.rpc.take_replies());
                self.answered_late(&id, c.rpc.take_late_sends());
            }
            close = true;
        }
        if close {
            self.hold_at(now);
        }
        // a list's lease ran out (its window closed or went quiet): its
        // holds and its hubs' watches go
        if self.hubs.lists.any() {
            // what was watched: every hold, its time come or not
            let before = self.watched_now(None);
            if self.hubs.lists.expire(now) {
                self.relist(before, now);
            }
        }
    }

    /// `processes_all {on, by, project}`: a list holder (the running
    /// screen, every project; a process's panel, its project) shown
    /// (again, renewing its lease) or left.
    pub(super) fn processes_all(&mut self, on: bool, by: Option<String>, project: Option<String>) {
        self.processes_all_at(on, by.as_deref().unwrap_or(SCREEN), project, Instant::now());
    }

    fn processes_all_at(&mut self, on: bool, by: &str, project: Option<String>, now: Instant) {
        // a hold set, run out or not, was watched (its end, the tick's, still lets go)
        let before = self.watched_now(None);
        self.hubs.lists.set(by, on, project, now);
        self.relist(before, now);
    }

    /// Each welcomed hub: whether a holder watches it (`at` none: any
    /// hold set, its time come or not).
    fn watched_now(&self, at: Option<Instant>) -> BTreeMap<String, bool> {
        self.hubs.conns.iter().filter(|(_, c)| c.welcomed).map(|(id, _)| (id.clone(), self.hubs.lists.covers(id, at))).collect()
    }

    /// The holds again, then each welcomed hub whose watch changed since
    /// `before` asked (or told to stop watching); one welcomed later is
    /// asked at its welcome.
    fn relist(&mut self, before: BTreeMap<String, bool>, now: Instant) {
        self.hold_at(now);
        for (id, was) in before {
            let on = self.hubs.lists.covers(&id, Some(now));
            if on == was {
                continue;
            }
            if let Some(c) = self.hubs.conns.get_mut(&id) {
                c.send(&json!({"cmd": "processes", "project": id, "watch": on}), None);
            }
        }
    }

    /// The window's subscriptions and voice mode's thread.
    fn all_subs(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut subs = self.hubs.subs.clone();
        if let Some((p, a)) = &self.hubs.voice {
            subs.entry(p.clone()).or_default().insert(a.clone());
        }
        subs
    }

    /// Voice mode's thread (architect m_11164): the core subscribes to it
    /// only when the window doesn't, and unsubscribes when voice mode ends
    /// or moves (never a hidden subscription left behind).
    pub(super) fn voice_sub(&mut self, want: Option<(String, String)>) {
        let old = self.hubs.voice.take();
        if old == want {
            self.hubs.voice = old;
            return;
        }
        let window = |s: &Self, p: &str, a: &str| s.hubs.subs.get(p).is_some_and(|x| x.contains(a));
        if let Some((p, a)) = &old {
            if !window(self, p, a) {
                if let Some(c) = self.hubs.conns.get_mut(p).filter(|c| c.welcomed) {
                    c.send(&json!({"cmd": "unsubscribe", "project": p, "agent": a}), None);
                }
            }
        }
        if let Some((p, a)) = &want {
            if !window(self, p, a) {
                if let Some(c) = self.hubs.conns.get_mut(p).filter(|c| c.welcomed) {
                    c.send(&json!({"cmd": "subscribe", "project": p, "agent": a}), None);
                }
            }
        }
        self.hubs.voice = want;
        self.hold();
    }

    /// Open the connections to hold, close the others.
    fn hold(&mut self) {
        self.hold_at(Instant::now());
    }

    fn hold_at(&mut self, now: Instant) {
        let mut want = held(&self.hubs.rows, self.hubs.shown.as_deref(), &self.all_subs());
        // the running screen: every hub that runs; a process's panel: its
        // project's if it runs; never one to start
        if self.hubs.shown.is_some() {
            if let Some(p) = self.hubs.ports.as_ref() {
                let running = listed(&self.hubs.rows, |path| (p.facts.running)(path));
                if self.hubs.lists.all(now) {
                    want.extend(running);
                } else {
                    let one = self.hubs.lists.projects(now);
                    want.extend(running.into_iter().filter(|id| one.contains(id)));
                }
            }
        }
        // a folder gone: never its hub (it would start on a missing
        // workspace), whatever holds it
        let gone_folders: BTreeSet<String> = self.hubs.rows.iter().filter(|r| !self.folder_exists(&r.path)).map(|r| r.id.clone()).collect();
        want.retain(|id| !gone_folders.contains(id) && !self.hubs.refused.contains_key(id));
        let gone: Vec<String> = self
            .hubs
            .conns
            .iter()
            .filter(|(id, c)| gone_folders.contains(*id) || (!want.contains(*id) && !c.lingers(now)))
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            if let Some(mut c) = self.hubs.conns.remove(&id) {
                c.hub.close();
                c.rpc.lost(super::hub_rpc::GONE);
                self.replies.extend(c.rpc.take_replies());
                self.answered_late(&id, c.rpc.take_late_sends());
            }
        }
        for id in want {
            if let Some(c) = self.hubs.conns.get_mut(&id) {
                // a rule holds it now: no longer for a command only
                c.until = None;
                continue;
            }
            let Some(path) = self.hubs.rows.iter().find(|r| r.id == id).map(|r| r.path.clone()) else { continue };
            let Some(p) = self.hubs.ports.as_mut() else { return };
            let hub = (p.spawn)(&path, &id);
            self.hubs.conns.insert(id, Conn::new(hub, None));
        }
    }

    /// A project hub's connection said something.
    pub fn project_hub(&mut self, id: &str, h: HubIn) {
        self.project_hub_in(id, h);
        self.take_rpc_replies();
    }

    /// The app's requests the connections answered, for the run loop
    /// ([`Core::take_replies`]), before the events they caused.
    fn take_rpc_replies(&mut self) {
        let mut late = Vec::new();
        for (id, c) in self.hubs.conns.iter_mut() {
            self.replies.extend(c.rpc.take_replies());
            late.push((id.clone(), c.rpc.take_late_sends()));
        }
        for (id, l) in late {
            self.answered_late(&id, l);
        }
    }

    /// The sends this core answered late, settled at last: each to the
    /// app as `CoreEv::SendAnswered` (hub_rpc.rs `LateSend`), so his row
    /// meets its echo by the hub's message id, or says it went.
    fn answered_late(&mut self, project: &str, late: Vec<super::hub_rpc::LateSend>) {
        for l in late {
            if l.result.is_none() {
                eprintln!("ambient-core: hub {project}: send {} to {} never answered (its connection went, or {} s passed)", l.cid, l.agent, super::hub_rpc::SEND_KEPT.as_secs());
            }
            self.ev(CoreEv::SendAnswered { project: project.to_string(), agent: l.agent, cid: l.cid, result: l.result });
        }
    }

    fn project_hub_in(&mut self, id: &str, h: HubIn) {
        let Some(c) = self.hubs.conns.get_mut(id) else { return };
        match h {
            // JSON-RPC's `initialize` first (P1c), always
            HubIn::Up => {
                c.welcomed = false;
                let was_down = c.down.take().is_some();
                // this Up is connection `ups`; a newer one's Up waits behind it: it says initialize
                c.ups += 1;
                if c.ups == c.hub.epoch() {
                    // the one write on a connection not welcomed yet, by its own call
                    c.epoch = c.ups;
                    let init = c.rpc.initialize(env!("CARGO_PKG_VERSION"));
                    c.hub.send_on(c.epoch, &init);
                }
                if was_down {
                    self.emit_projects();
                }
            }
            HubIn::Down(cause) => {
                c.welcomed = false;
                c.rpc.ready = false;
                c.rpc.lost(super::hub_rpc::GONE);
                c.rows = None;
                // a cause stays until it's up: a later bare loss keeps it
                if cause.is_some() {
                    c.down = cause;
                }
                self.emit_projects();
            }
            HubIn::Refused(why) => self.project_refused(id, why, ErrorKind::HubRefused),
            HubIn::Line(l) => {
                let Ok(v) = serde_json::from_str::<Value>(&l) else { return };
                match c.rpc.read(id, v) {
                    Read::Welcome(welcome, state) => {
                        self.project_ev(id, welcome);
                        for ev in state {
                            self.project_ev(id, ev);
                        }
                    }
                    Read::Evs(evs) => {
                        for ev in evs {
                            self.project_ev(id, ev);
                        }
                    }
                    Read::Error { tag, text, cid, reason, kind } => {
                        let mut ev = json!({"ev": "error", "project": id, "cmd": tag, "text": text});
                        if let Some(cid) = cid {
                            ev["cid"] = json!(cid);
                        }
                        if let Some(r) = reason {
                            ev["reason"] = json!(r);
                        }
                        if let Some(k) = kind {
                            ev["kind"] = json!(k);
                        }
                        self.emit(ev);
                    }
                    Read::Resync => {
                        let req = c.rpc.read_again();
                        c.hub.send_on(c.epoch, &req);
                    }
                    Read::Refused(why) => self.project_refused(id, why, ErrorKind::HubRefused),
                    // a hub from before client-protocol: refused the same
                    // way, with what to do (nothing more is said to it):
                    // his install's steps when it runs from one
                    Read::NotRpc => {
                        let older = self.hubs.ports.as_ref().and_then(|p| (p.facts.older)(id));
                        self.project_refused(id, older.unwrap_or_else(|| bise_proto::rpc::HUB_OLDER.to_string()), ErrorKind::HubOlder)
                    }
                    Read::Nothing => {}
                }
            }
        }
    }

    /// Its reader has ended: no reconnect, its waiting commands fail with
    /// the hub's words (`kind` hub_refused) or what to do with an older hub
    /// (hub_older), nothing more goes until he retries.
    fn project_refused(&mut self, id: &str, why: String, kind: ErrorKind) {
        let Some(mut c) = self.hubs.conns.remove(id) else { return };
        c.hub.close();
        let name = self.hubs.rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.to_string());
        self.hubs.refused.insert(id.to_string(), why.clone());
        let text = match kind {
            ErrorKind::HubRefused => refused_text(&name, &why),
            _ => why.clone(),
        };
        c.rpc.lost(&text);
        self.replies.extend(c.rpc.take_replies());
        self.answered_late(id, c.rpc.take_late_sends());
        for (cmd, _, app) in c.pending {
            let tag = cmd.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
            let cid = cmd.get("cid").and_then(Value::as_u64);
            self.project_error(id, &tag, &text, cid, Some(kind), app);
        }
        self.emit(json!({"ev": "hub_refused", "project": id, "error": why}));
        self.emit_projects();
    }

    /// One typed event of a project's hub, for the window.
    fn project_ev(&mut self, id: &str, typed: HubEv) {
        if !first_end(&mut self.hubs.ends, &typed) {
            return;
        }
        let Some(c) = self.hubs.conns.get_mut(id) else { return };
        // boot-speed: main's page and entries into its copy (written at the tick)
        self.hubs.threads.seen(id, &typed);
        let v = typed.to_value();
        let tag = typed.tag().to_string();
        // voice mode hears its agent's turn and messages
        let for_voice = (self.voice_on.is_some() && matches!(typed, HubEv::Agents { .. } | HubEv::Entry { .. })).then(|| typed.clone());
        match typed {
            HubEv::Welcome { cmds, built, .. } => {
                c.welcomed = true;
                c.cmds = cmds;
                // a hub newer than this app's own runtime: the app's update channel, now
                self.hub_built(built.as_deref());
                let Some(c) = self.hubs.conns.get_mut(id) else { return };
                let voice = self.hubs.voice.as_ref().filter(|(p, _)| p == id).map(|(_, a)| a);
                let subs = self.hubs.subs.get(id);
                let agents: Vec<String> = subs.into_iter().flatten().chain(voice.filter(|a| !subs.is_some_and(|s| s.contains(*a)))).cloned().collect();
                for agent in agents {
                    c.send(&json!({"cmd": "subscribe", "project": id, "agent": agent}), None);
                }
                // the running screen (or a panel on this project) is open:
                // this hub's list too, watched
                if self.hubs.lists.covers(id, Some(Instant::now())) {
                    c.send(&json!({"cmd": "processes", "project": id, "watch": true}), None);
                }
                // the commands that waited for it, in order; a hub
                // held for them only stays a while for their answers
                let mut failed = Vec::new();
                let mut lacked = Vec::new();
                for (cmd, _, app) in std::mem::take(&mut c.pending) {
                    let tag = cmd.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
                    let cid = cmd.get("cid").and_then(Value::as_u64);
                    if lacks(&c.cmds, &tag) {
                        lacked.push((tag, cid, app));
                    } else if !c.send(&cmd, app.clone()) {
                        failed.push((tag, cid, app));
                    }
                }
                if let Some(u) = c.until.as_mut() {
                    *u = Instant::now() + LINGER;
                }
                for (cmd, cid, app) in failed {
                    self.project_error(id, &cmd, "its hub went away: nothing sent. try again in a moment.", cid, None, app);
                }
                for (cmd, cid, app) in lacked {
                    self.project_error(id, &cmd, HUB_LACKS, cid, Some(ErrorKind::HubOlder), app);
                }
            }
            HubEv::Agents { agents, archived, .. } => {
                let cards = c.rows.take().map(|r| r.1).unwrap_or_default();
                c.rows = Some((c.archived.merge(&agents, archived.as_deref()), cards));
            }
            HubEv::Cards { cards, others, .. } => {
                let agents = c.rows.take().map(|r| r.0).unwrap_or_default();
                c.rows = Some((agents, inbox_cards(cards, others)));
            }
            // the H1 sidebar: the row's pulse (emit_rows sends it when it changed)
            HubEv::Pulse { pulse, .. } => {
                c.pulse = Some(pulse);
                self.emit_projects();
            }
            // update-flow: the machine's one line, folded over every hub
            HubEv::BiseUpdate { update, .. } => {
                c.bise_update = update;
                self.emit_bise_update();
            }
            _ => {}
        }
        if let Some(t) = for_voice {
            self.voice_mode_hub(&t);
        }
        self.emit(v);
        if matches!(tag.as_str(), "welcome" | "agents" | "cards") {
            self.emit_projects();
        }
    }

    /// At most every [`POLL`] while the window shows something (`force`:
    /// now): the registry and the files again, the holds, the rows.
    pub(crate) fn poll_projects(&mut self, force: bool) {
        if self.hubs.shown.is_none() {
            return;
        }
        let now = Instant::now();
        if !force && self.hubs.polled.is_some_and(|t| now.duration_since(t) < POLL) {
            return;
        }
        self.hubs.polled = Some(now);
        let Some(p) = self.hubs.ports.as_ref() else { return };
        let rows = (p.facts.rows)();
        let git = rows.iter().map(|r| (r.id.clone(), (p.facts.git)(&r.path))).collect();
        self.hubs.rows = rows;
        self.hubs.git = git;
        self.hold();
        self.emit_projects();
    }

    /// His `project_add` answers with the rows whatever the window said:
    /// the registry read again and `projects` sent even before `shown`
    /// (ambient-lead m_18943: a core asked before any `shown`, as
    /// realshape's no-env run does, added the folder and said nothing).
    pub(super) fn projects_now(&mut self) {
        if self.hubs.shown.is_some() {
            return self.poll_projects(true);
        }
        let Some(p) = self.hubs.ports.as_ref() else { return };
        self.hubs.rows = (p.facts.rows)();
        self.hubs.git = self.hubs.rows.iter().map(|r| (r.id.clone(), (p.facts.git)(&r.path))).collect();
        self.hold();
        self.emit_rows();
    }

    /// update-flow: `machine_update`, the machine's line folded over every
    /// held hub's (rows::bise_update::machine, the projects by name), when
    /// it changed.
    pub(super) fn emit_bise_update(&mut self) {
        let rows = &self.hubs.rows;
        let name = |id: &str| rows.iter().find(|r| r.id == id).map_or_else(|| id.to_string(), |r| r.name.clone());
        let hubs: Vec<(String, Option<bise_proto::rows::BiseUpdate>)> =
            self.hubs.conns.iter().map(|(id, c)| (name(id), c.bise_update.clone())).collect();
        let now = bise_proto::rows::machine(&hubs);
        if self.hubs.bise_sent.as_ref() == Some(&now) {
            return;
        }
        self.hubs.bise_sent = Some(now.clone());
        self.emit(json!({"ev": "machine_update", "update": now}));
    }

    /// update-flow: his click on the pill's `update` (`bise_update`): to
    /// one connected hub (the machine installs it once; the install flock
    /// dedupes a second); false when none is connected.
    pub(super) fn bise_update_now(&mut self) -> bool {
        let project = self.hubs.conns.iter().find(|(_, c)| c.rpc.ready).map(|(id, _)| id.clone());
        let Some(project) = project else { return false };
        let cmd = json!({"cmd": "bise_update", "project": project});
        self.hubs.conns.get_mut(&project).is_some_and(|c| c.send(&cmd, None))
    }

    /// `projects` when the rows changed.
    pub(super) fn emit_projects(&mut self) {
        // no rows before the window says what it shows (home's early
        // welcome, agents or cards say nothing yet)
        if self.hubs.shown.is_none() {
            return;
        }
        self.emit_rows();
    }

    fn emit_rows(&mut self) {
        let Some(p) = self.hubs.ports.as_ref() else { return };
        let conns = &self.hubs.conns;
        let facts: Vec<Facts> = projects::facts(
            &self.hubs.rows,
            |id| match conns.get(id).and_then(|c| c.rows.as_ref()) {
                Some((a, c)) => Some(live_view(id, a, c)),
                None => (p.facts.view)(id),
            },
            |_| false,
            |path| (p.facts.exists)(path),
        );
        let rows: Vec<ProjectRow> = facts
            .into_iter()
            .enumerate()
            .map(|(i, mut f)| {
                let held = conns.get(&f.row.id).is_some_and(|c| c.welcomed);
                // a folder gone: missing and never running, whatever its
                // old hub process still does (lead m_8979)
                f.running = !f.missing && (held || (p.facts.running)(&f.row.path));
                let c = self.hubs.git.get(&f.row.id).cloned().unwrap_or_default();
                let mut row = projects::row(&f, i as u32, &c);
                row.down = match f.row.home {
                    true => self.hubs.home_down.clone(),
                    false => conns.get(&f.row.id).and_then(|c| c.down.clone()).filter(|_| !held),
                };
                // a held hub's own pulse, else its view.json's (projects::row)
                if let Some(p) = conns.get(&f.row.id).and_then(|c| c.pulse.clone()) {
                    row.pulse = Some(p);
                } else if row.pulse.is_none() && f.view.as_ref().is_some_and(|v| v.written_ms == 0) {
                    // a held hub with no pulse yet: its view.json's last one
                    row.pulse = (p.facts.view)(&f.row.id).and_then(|v| v.pulse);
                }
                row
            })
            .collect();
        let ev = json!({"ev": "projects", "projects": rows});
        let s = ev.to_string();
        if s != self.hubs.last {
            self.hubs.last = s;
            self.emit(ev);
        }
    }

    /// Each registered project's (id, agents, cards), in his order: a held
    /// hub's own rows, else its `view.json` (none: empty).
    pub(super) fn project_rows(&self) -> Vec<(String, Vec<Agent>, Vec<Card>)> {
        let Some(p) = self.hubs.ports.as_ref() else { return Vec::new() };
        self.hubs
            .rows
            .iter()
            .map(|r| match self.hubs.conns.get(&r.id).and_then(|c| c.rows.as_ref()) {
                Some((a, c)) => (r.id.clone(), a.clone(), c.clone()),
                None => match (p.facts.view)(&r.id) {
                    Some(v) => (r.id.clone(), v.agents, v.cards),
                    None => (r.id.clone(), Vec::new(), Vec::new()),
                },
            })
            .collect()
    }

    /// Every project connection closed (the core's end).
    pub(super) fn close_projects(&mut self) {
        for (id, mut c) in std::mem::take(&mut self.hubs.conns) {
            c.hub.close();
            c.rpc.lost(super::hub_rpc::GONE);
            self.replies.extend(c.rpc.take_replies());
            self.answered_late(&id, c.rpc.take_late_sends());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, home: bool) -> Row {
        Row { path: PathBuf::from(format!("/p/{id}")), name: id.into(), id: id.into(), home, added_ms: 0 }
    }

    #[test]
    fn a_projects_inbox_is_his_cards_and_bises_own_by_id_as_the_tuis() {
        let card = |id: u64, kind: &str| -> Card {
            serde_json::from_value(serde_json::json!({"id": id, "kind": kind, "agent": "gone", "project": "t", "question": "q", "text": "q", "options": [], "rank": 1, "since_ms": 0, "urgent": false})).unwrap()
        };
        let got = inbox_cards(vec![card(3, "question"), card(5, "confirm")], vec![card(4, "drop"), card(1, "done")]);
        assert_eq!(got.iter().map(|c| (c.id, c.kind.as_str())).collect::<Vec<_>>(), [(1, "done"), (3, "question"), (4, "drop"), (5, "confirm")]);
        assert_eq!(inbox_cards(vec![card(2, "question")], vec![]).len(), 1, "no others: his cards as they came");
    }

    #[test]
    fn the_held_hubs_are_home_shown_and_subscribed_once_a_window_shows() {
        let rows = [row("home", true), row("a", false), row("b", false), row("c", false)];
        let mut subs = BTreeMap::new();
        let ids = |s: BTreeSet<String>| s.into_iter().collect::<Vec<_>>();
        assert_eq!(ids(held(&rows, None, &subs)), ["home"], "no window yet: bise's home alone");
        assert!(held(&rows[1..], None, &subs).is_empty(), "no home row: nothing");
        assert_eq!(ids(held(&rows, Some(&[]), &subs)), ["home"]);
        assert_eq!(ids(held(&rows, Some(&["a".into(), "zz".into()]), &subs)), ["a", "home"], "an unknown id is not held");
        assert!(track(&mut subs, "subscribe", "b", "main"));
        assert!(!track(&mut subs, "subscribe", "b", "main"), "already");
        assert_eq!(ids(held(&rows, Some(&["a".into()]), &subs)), ["a", "b", "home"], "a subscribed thread holds its hub");
        assert!(track(&mut subs, "unsubscribe", "b", "main"));
        assert!(subs.is_empty());
        assert!(!track(&mut subs, "send", "b", "main"));
        assert_eq!(ids(held(&rows, Some(&["a".into()]), &subs)), ["a", "home"]);
    }

    /// J (architect m_10223): both ends of one job arrive (shop's own
    /// job_end, bise's followed_end), in either order: one line; another
    /// job of the same agent is its own line; a keyless job_end always goes.
    #[test]
    fn a_job_end_and_its_followed_end_make_one_line() {
        use bise_proto::hub::JobState;
        let job = |key: Option<u64>| HubEv::JobEnd {
            project: "shop-1".into(),
            agent: "perf".into(),
            state: JobState::Done,
            label: "l".into(),
            summary: "s".into(),
            key,
        };
        let followed = |key: u64| HubEv::FollowedEnd {
            project: "shop-1".into(),
            agent: "perf".into(),
            key,
            state: JobState::Done,
            label: "l".into(),
            summary: "s".into(),
        };
        let mut seen = VecDeque::new();
        assert!(first_end(&mut seen, &job(Some(7))));
        assert!(!first_end(&mut seen, &followed(7)), "the second copy is dropped");
        assert!(first_end(&mut seen, &followed(8)), "its next job: a line of its own");
        assert!(!first_end(&mut seen, &job(Some(8))));
        assert!(first_end(&mut seen, &job(None)) && first_end(&mut seen, &job(None)), "no key: always");
        assert!(first_end(&mut seen, &HubEv::Typing { project: "p".into(), agent: "a".into(), text: String::new() }));
        for k in 100..100 + ENDS_KEPT as u64 + 5 {
            first_end(&mut seen, &followed(k));
        }
        assert_eq!(seen.len(), ENDS_KEPT);
    }

    #[test]
    fn a_head_names_its_branch() {
        assert_eq!(branch_of_head("ref: refs/heads/main\n"), Some("main".into()));
        assert_eq!(branch_of_head("ref: refs/heads/sb/perf"), Some("sb/perf".into()));
        assert_eq!(branch_of_head("3f2a9c0d1e"), None, "detached");
    }
}
