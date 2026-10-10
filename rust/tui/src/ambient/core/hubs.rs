//! The core as a client of every project's hub (bise desktop S3b step 2):
//! one JSON-RPC connection (`initialize` first, client-protocol P1c,
//! `hub_rpc.rs`; an older hub: `{cmd: hello, proto: 1}` after the older
//! hello, one release) per held project, the
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

use super::hub_rpc::{Read, RpcConn};
use super::*;
use crate::ambient::projects::{self, Checkout, Facts};
use bise_home::projects::Row;
use bise_proto::draft::{ProjectRow, ProjectView};
use bise_proto::hub::HubEv;
use bise_proto::rows::{Agent, Card};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The registry's files are read at most this often.
pub const POLL: Duration = Duration::from_secs(2);

/// A connection to a project's hub (its reader thread tags what it reads
/// with the project id).
pub type Spawn = Box<dyn FnMut(&Path, &str) -> Hub>;
/// A hub's `view.json` by hub id.
pub type ViewOf = Box<dyn Fn(&str) -> Option<ProjectView>>;
/// A checkout's branch and whether it is a git repo.
pub type GitOf = Box<dyn Fn(&Path) -> Checkout>;

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
}

/// One held project's connection.
struct Conn {
    hub: Hub,
    /// its `welcome` came since the last connection: commands go
    welcomed: bool,
    /// its own rows, as its hub sent them (fresher than its view.json)
    rows: Option<(Vec<Agent>, Vec<Card>)>,
    /// commands for a hub not welcomed yet, each with its deadline (sent
    /// at the welcome, else an error at the deadline)
    pending: Vec<(Value, Instant)>,
    /// held for a command only (a project not shown, not subscribed):
    /// closed once this passes with nothing pending
    until: Option<Instant>,
    /// hub-skew: the commands its welcome listed (empty: a hub older
    /// than the list, or not welcomed yet)
    cmds: Vec<String>,
    /// its JSON-RPC side (P1c, `hub_rpc.rs`)
    rpc: RpcConn,
}

impl Conn {
    fn new(hub: Hub, until: Option<Instant>) -> Conn {
        Conn { hub, welcomed: false, rows: None, pending: Vec::new(), until, cmds: Vec::new(), rpc: RpcConn::default() }
    }

    /// One window command (a `HubCmd`'s JSON) to its hub: a request once
    /// `initialize` answered, else the typed line (an older hub); false
    /// when nothing was written.
    fn send(&mut self, v: &Value) -> bool {
        if !self.rpc.ready {
            return self.hub.send(v);
        }
        match self.rpc.request(v) {
            Some(req) => self.hub.send(&req),
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
    /// hub-skew: the projects already told their hub is older (once each)
    older: BTreeSet<String>,
    /// the projects whose hub doesn't serve JSON-RPC's `initialize` (one
    /// release, architect m_13089 Q2): their connection says the older
    /// hellos, for this core's life.
    /// TODO(client-protocol plan, "after the release": remove older_door,
    /// its Read::Older and the core's typed hello in the release after the
    /// one client-protocol ships in)
    older_door: BTreeSet<String>,
    /// the projects whose hub refused this core's connection, with its
    /// words (HubEv::Refused, docs/issues/16): never connected again until
    /// he retries (`hub_retry`) or a new core starts. The hub's verdict is
    /// about this core's process (a core started by an agent, e.g. an
    /// ambient-qa run on a real workspace, is refused by design), so
    /// retrying on our own would never change it
    refused: BTreeMap<String, String>,
}

/// What a command to a refused project's hub answers (kind hub_refused).
fn refused_text(name: &str, why: &str) -> String {
    format!("{name}'s hub refused this connection: {why}")
}

/// hub-skew (architect m_11314): what the window says when this
/// project's hub doesn't know a command, or is too old to say which.
pub const HUB_OLDER: &str = "this project's hub is older: update bise";

/// hub-skew: a hub that listed its commands (`welcome.cmds`) and lacks
/// `tag`, the typed command's own tag (never its text: architect
/// m_11487). An empty list (an older hub) lacks nothing we can know.
pub fn lacks(cmds: &[String], tag: &str) -> bool {
    !cmds.is_empty() && !cmds.iter().any(|c| c == tag)
}

/// hub-skew: a hub too old to list its commands that speaks an older
/// proto than this core (one notice; its errors pass as they are).
pub fn older_proto(cmds: &[String], proto: u32) -> bool {
    cmds.is_empty() && proto < bise_proto::PROTO
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

/// A held hub's own rows as a view (the sidebar's counts).
fn live_view(project: &str, agents: &[Agent], cards: &[Card]) -> ProjectView {
    ProjectView { v: 1, project: project.into(), written_ms: 0, stopped_ms: None, last_activity_ms: 0, agents: agents.to_vec(), cards: cards.to_vec(), artifacts: vec![], artifacts_total: 0, scheduled: vec![] }
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
    /// failed, for the window to draw (bise_proto::hub::ErrorKind).
    fn project_error(&mut self, project: &str, cmd: &str, text: &str, cid: Option<u64>, kind: Option<bise_proto::hub::ErrorKind>) {
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
        if self.hubs.refused.remove(project).is_some() {
            self.hold();
            self.emit_projects();
        }
    }

    /// A window command for a project's hub (a `HubCmd` with `project`).
    pub(super) fn typed_cmd(&mut self, v: Value) {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let (project, cmd, agent) = (s("project"), s("cmd"), s("agent"));
        let cid = v.get("cid").and_then(Value::as_u64);
        if self.hubs.ports.is_none() {
            return self.project_error(&project, &cmd, "this core has no projects", cid, None);
        }
        let Some(row) = self.hubs.rows.iter().find(|r| r.id == project).cloned() else {
            return self.project_error(&project, &cmd, &format!("there's no project {project}"), cid, None);
        };
        // before shown, home only (architect m_10133: its own words, so a
        // window that sends too early reads plainly in the logs)
        if !row.home && self.hubs.shown.is_none() {
            return self.project_error(&project, &cmd, &format!("project {project} isn't shown yet"), cid, None);
        }
        // its hub refused this core: nothing goes until he retries
        if let Some(why) = self.hubs.refused.get(&project).cloned() {
            if cmd == "unsubscribe" {
                track(&mut self.hubs.subs, &cmd, &project, &agent);
                return;
            }
            return self.project_error(&project, &cmd, &refused_text(&row.name, &why), cid, Some(bise_proto::hub::ErrorKind::HubRefused));
        }
        // never a hub for a folder that is gone (its hub would start on a
        // missing workspace)
        if cmd != "unsubscribe" && !self.folder_exists(&row.path) {
            return self.project_error(&project, &cmd, &format!("{}: its folder is gone", row.name), cid, None);
        }
        // R8: a /model whose provider has no key waits for its setup (setup.rs)
        if cmd == "slash" && self.hold_if_keyless(&project, &agent, &s("line"), cid) == super::setup::Hold::Held {
            return;
        }
        if track(&mut self.hubs.subs, &cmd, &project, &agent) {
            self.hold();
        }
        // voice mode still follows that thread: the window's unsubscribe
        // stays here (the core's own subscription goes when voice mode ends)
        if cmd == "unsubscribe" && self.hubs.voice.as_ref().is_some_and(|(p, a)| *p == project && *a == agent) {
            return;
        }
        // his typed send while voice mode waits on his typing: its answer is said
        if cmd == "send" {
            self.voice_mode_typed(&project, &agent);
        }
        let welcomed = self.hubs.conns.get(&project).is_some_and(|c| c.welcomed);
        match (cmd.as_str(), welcomed) {
            // kept: sent at its hub's welcome
            ("subscribe" | "unsubscribe", false) => {}
            (_, true) => {
                let Some(c) = self.hubs.conns.get_mut(&project) else { return };
                if lacks(&c.cmds, &cmd) {
                    return self.project_error(&project, &cmd, HUB_OLDER, cid, Some(bise_proto::hub::ErrorKind::HubOlder));
                }
                // the write failed: its hub just went (its Down is on its
                // way); the reader connects again, the command waits
                if !c.send(&v) {
                    c.welcomed = false;
                    c.pending.push((v, Instant::now() + START));
                }
            }
            // its hub connects on demand (started if needed), the command
            // goes at its welcome, then the hold goes (decision m_8720)
            (_, false) => self.deliver_at_welcome(&project, &row.path, v),
        }
    }

    fn folder_exists(&self, path: &Path) -> bool {
        self.hubs.ports.as_ref().is_some_and(|p| (p.facts.exists)(path))
    }

    /// `v` waits for `project`'s welcome on its connection, opened for it
    /// when no rule holds that project.
    fn deliver_at_welcome(&mut self, project: &str, path: &Path, v: Value) {
        let now = Instant::now();
        if !self.hubs.conns.contains_key(project) {
            let Some(p) = self.hubs.ports.as_mut() else { return };
            let hub = (p.spawn)(path, project);
            self.hubs.conns.insert(project.to_string(), Conn::new(hub, Some(now + START + LINGER)));
        }
        if let Some(c) = self.hubs.conns.get_mut(project) {
            c.pending.push((v, now + START));
        }
    }

    /// Commands past their deadline get their error; the hubs held for a
    /// command only go once it went.
    pub(super) fn tick_projects(&mut self, now: Instant) {
        let mut late: Vec<(String, String, Option<u64>)> = Vec::new();
        let mut close = false;
        for (id, c) in self.hubs.conns.iter_mut() {
            c.pending.retain(|(v, deadline)| {
                let ok = *deadline > now;
                if !ok {
                    late.push((id.clone(), v.get("cmd").and_then(Value::as_str).unwrap_or("").to_string(), v.get("cid").and_then(Value::as_u64)));
                }
                ok
            });
            close |= c.until.is_some_and(|u| u <= now) && c.pending.is_empty();
        }
        for (id, cmd, cid) in late {
            self.project_error(&id, &cmd, "its hub didn't start: nothing sent. try again in a moment.", cid, None);
        }
        if close {
            self.hold_at(now);
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
                    c.send(&json!({"cmd": "unsubscribe", "project": p, "agent": a}));
                }
            }
        }
        if let Some((p, a)) = &want {
            if !window(self, p, a) {
                if let Some(c) = self.hubs.conns.get_mut(p).filter(|c| c.welcomed) {
                    c.send(&json!({"cmd": "subscribe", "project": p, "agent": a}));
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
            if let Some(c) = self.hubs.conns.remove(&id) {
                c.hub.close();
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
        let older = self.hubs.older_door.contains(id);
        let Some(c) = self.hubs.conns.get_mut(id) else { return };
        match h {
            // JSON-RPC's `initialize` first (P1c); an older hub (one
            // release): the older hello, then the typed one
            HubIn::Up => {
                c.welcomed = false;
                if older {
                    c.hub.send(&json!({"op": "hello"}));
                    c.hub.send(&json!({"cmd": "hello", "proto": bise_proto::PROTO, "typed_only": true}));
                } else {
                    let init = c.rpc.initialize(env!("CARGO_PKG_VERSION"));
                    c.hub.send(&init);
                }
            }
            HubIn::Down => {
                c.welcomed = false;
                c.rpc.ready = false;
                c.rows = None;
                self.emit_projects();
            }
            HubIn::Refused(why) => self.project_refused(id, why),
            HubIn::Line(l) => {
                let Ok(v) = serde_json::from_str::<Value>(&l) else { return };
                if older {
                    return self.project_line(id, v);
                }
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
                        c.hub.send(&req);
                    }
                    Read::Refused(why) => self.project_refused(id, why),
                    // its connection closes: the next one says hello the older way
                    Read::Older => {
                        self.hubs.older_door.insert(id.to_string());
                    }
                    Read::Nothing => {}
                }
            }
        }
    }

    /// Its reader has ended: no reconnect, its waiting commands fail with
    /// the hub's words, nothing more goes until he retries.
    fn project_refused(&mut self, id: &str, why: String) {
        let Some(c) = self.hubs.conns.remove(id) else { return };
        c.hub.close();
        let name = self.hubs.rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.to_string());
        self.hubs.refused.insert(id.to_string(), why.clone());
        for (cmd, _) in c.pending {
            let tag = cmd.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
            let cid = cmd.get("cid").and_then(Value::as_u64);
            self.project_error(id, &tag, &refused_text(&name, &why), cid, Some(bise_proto::hub::ErrorKind::HubRefused));
        }
        self.emit(json!({"ev": "hub_refused", "project": id, "error": why}));
        self.emit_projects();
    }

    /// An older hub's typed line (its typed hello's connection).
    fn project_line(&mut self, id: &str, v: Value) {
        let ev = v.get("ev").and_then(Value::as_str).unwrap_or("");
        if !HubEv::TAGS.contains(&ev) {
            return;
        }
        // a typed tag that doesn't decode is the hub's older event of the
        // same name (`artifacts` {rows, new, seen_ms} from a hub that
        // doesn't know typed_only): never the window's (amb-win m_8886)
        let Ok(typed) = HubEv::from_value(v) else { return };
        self.project_ev(id, typed);
    }

    /// One typed event of a project's hub, for the window.
    fn project_ev(&mut self, id: &str, typed: HubEv) {
        if !first_end(&mut self.hubs.ends, &typed) {
            return;
        }
        let Some(c) = self.hubs.conns.get_mut(id) else { return };
        let v = typed.to_value();
        let tag = typed.tag().to_string();
        // voice mode hears its agent's turn and messages
        let for_voice = (self.voice_on.is_some() && matches!(typed, HubEv::Agents { .. } | HubEv::Entry { .. })).then(|| typed.clone());
        match typed {
            HubEv::Welcome { cmds, proto, .. } => {
                c.welcomed = true;
                let older = older_proto(&cmds, proto) && self.hubs.older.insert(id.to_string());
                c.cmds = cmds;
                let voice = self.hubs.voice.as_ref().filter(|(p, _)| p == id).map(|(_, a)| a);
                let subs = self.hubs.subs.get(id);
                let agents: Vec<String> = subs.into_iter().flatten().chain(voice.filter(|a| !subs.is_some_and(|s| s.contains(*a)))).cloned().collect();
                for agent in agents {
                    c.send(&json!({"cmd": "subscribe", "project": id, "agent": agent}));
                }
                // the commands that waited for it, in order; a hub
                // held for them only stays a while for their answers
                let mut failed = Vec::new();
                let mut lacked = Vec::new();
                for (cmd, _) in std::mem::take(&mut c.pending) {
                    let tag = cmd.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
                    let cid = cmd.get("cid").and_then(Value::as_u64);
                    if lacks(&c.cmds, &tag) {
                        lacked.push((tag, cid));
                    } else if !c.send(&cmd) {
                        failed.push((tag, cid));
                    }
                }
                if let Some(u) = c.until.as_mut() {
                    *u = Instant::now() + LINGER;
                }
                for (cmd, cid) in failed {
                    self.project_error(id, &cmd, "its hub went away: nothing sent. try again in a moment.", cid, None);
                }
                for (cmd, cid) in lacked {
                    self.project_error(id, &cmd, HUB_OLDER, cid, Some(bise_proto::hub::ErrorKind::HubOlder));
                }
                if older {
                    self.emit(json!({"ev": "notice", "project": id, "text": HUB_OLDER}));
                }
            }
            HubEv::Agents { agents, .. } => {
                let cards = c.rows.take().map(|r| r.1).unwrap_or_default();
                c.rows = Some((agents, cards));
            }
            HubEv::Cards { cards, .. } => {
                let agents = c.rows.take().map(|r| r.0).unwrap_or_default();
                c.rows = Some((agents, cards));
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

    /// `projects` when the rows changed.
    fn emit_projects(&mut self) {
        // no rows before the window says what it shows (home's early
        // welcome, agents or cards say nothing yet)
        if self.hubs.shown.is_none() {
            return;
        }
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
                projects::row(&f, i as u32, &c)
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
        for (_, c) in std::mem::take(&mut self.hubs.conns) {
            c.hub.close();
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
