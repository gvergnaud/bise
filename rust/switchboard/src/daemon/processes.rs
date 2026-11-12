//! Running processes on the hub's side (docs/processes.md): the
//! processes the live agents left running (bg jobs, detached trees) and
//! the ones that ended in the last 24 h, their output, his stop.
//!
//! One owner: a worker thread ([`Worker`]) holds all of it, never the
//! loop: the last running rows, the ports and files caches (by pid and
//! start), the followers, and it is the only writer of
//! `<hub>/procs/ended.jsonl` and `<hub>/procs/<agent>-<id>.out` (the
//! output kept at the end: its last 2,000 lines, 256 KB at most). Not the
//! journal, not sb-core: an observation log, pruned at 24 h. The loop
//! talks to it through a channel ([`Ask`]); it answers with
//! `Msg::Typed` (`processes`, `proc_out`) and `Msg::ProcCounts` (each
//! agent's running count and lowest port, only when one changes: the
//! `Agent.procs` / `procs_port` fields, held in `Hub.procs`).
//!
//! When it scans: every 60 s ps only (an ended row is kept even with no
//! client), every 15 s while a typed client is connected, every 2 s with
//! ports while one watches the list (`processes {watch: true}`); a
//! follower's output every 200 ms. Ports: one lsof for the pids not
//! asked yet (all of them while the list is watched); lsof missing: no
//! ports. Each port is asked once whether it answers HTTP (a no again
//! after `HTTP_RETRY`): a row's `url` only then. Never an agent's say
//! port (its `repl.json`) nor a port bise serves itself (pages, bridges):
//! a probe queued on a say port became his words at the next connection.
//!
//! How a row ends: its bg job's `.rc` (exited), his stop (stopped by
//! you: the stop marks it before its signals), proc-cleanup's reap
//! (`Ask::Reaping`, sent before the kill: stopped with its agent, never
//! exited; the channel keeps the order). The stop: same pid and start or
//! refused, a bise line in the bg `.out`, TERM to the group (bg) or the
//! tree, KILL after `STOP_GRACE_MS`; then bise's queued message to the
//! agent (`Input::HubMsg`, sb-core's IHubMsg: it doesn't wake it).

use super::rpc::Typed;
use super::*;
use crate::proto_view::processes::{self as pv, AgentIn, BgJob, Files, Found, Row, Tree, Why};
use bise_proto::hub::HubEv;
use bise_proto::rows::{OutKind, Proc, ProcKind};
use std::io::{Read, Seek, SeekFrom};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};

/// Each agent's running count and lowest port (`Hub.procs`).
pub(crate) type Counts = BTreeMap<String, (u16, Option<u16>)>;

/// A live agent as the loop knows it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Live {
    pub name: String,
    pub dir: String,
}

/// What the loop asks the worker.
pub(super) enum Ask {
    /// the live agents (sent when they change)
    Agents(Vec<Live>),
    /// whether a typed client is connected
    Clients(bool),
    /// `processes`: a scan with ports, its rows to `to`; `watch` on or
    /// off for that connection
    List { to: Typed, watch: Option<(ClientId, bool)> },
    Out { client: ClientId, agent: String, id: String, from: Option<u64> },
    Follow { client: ClientId, agent: String, id: String, on: bool },
    Stop { client: ClientId, agent: String, id: Option<String> },
    Gone(ClientId),
    /// proc-cleanup kills these agents' processes next (None: every
    /// agent, the hub quits): their running rows end as stopped with
    /// their agent
    Reaping(Option<BTreeSet<String>>),
    /// a stop's signals are done (the stop thread)
    Stopped { agent: String, id: String, cmd: String },
    Quit,
}

/// The loop's handle on the worker.
pub(super) struct Worker {
    tx: Sender<Ask>,
    join: Option<std::thread::JoinHandle<()>>,
    agents: Vec<Live>,
    clients: bool,
}

impl Worker {
    pub(super) fn start(paths: Paths, hub: String, project: String, out: Sender<Msg>) -> Worker {
        let (tx, rx) = channel();
        let me = tx.clone();
        let join = std::thread::Builder::new()
            .name("processes".into())
            .spawn(move || State::new(paths, hub, project, out, me).run(rx))
            .ok();
        Worker { tx, join, agents: Vec::new(), clients: false }
    }

    pub(super) fn ask(&self, a: Ask) {
        let _ = self.tx.send(a);
    }

    /// The live agents and whether a typed client is connected, sent
    /// only when they changed.
    pub(super) fn sync(&mut self, agents: Vec<Live>, clients: bool) {
        if agents != self.agents {
            self.agents = agents.clone();
            self.ask(Ask::Agents(agents));
        }
        if clients != self.clients {
            self.clients = clients;
            self.ask(Ask::Clients(clients));
        }
    }

    /// The hub quits for good: the agents' rows end as stopped with them,
    /// then the worker ends; `wait` at most.
    pub(super) fn quit(&mut self, wait: Duration) {
        self.ask(Ask::Reaping(None));
        self.ask(Ask::Quit);
        let Some(j) = self.join.take() else { return };
        let t0 = std::time::Instant::now();
        while !j.is_finished() && t0.elapsed() < wait {
            std::thread::sleep(Duration::from_millis(20));
        }
        if j.is_finished() {
            let _ = j.join();
        }
    }
}

impl Shell {
    /// After a step: the worker learns the live agents and whether a
    /// typed client is connected (sent only when changed).
    pub(super) fn procs_sync(&mut self) {
        let live: Vec<Live> = self
            .hub
            .st
            .order
            .iter()
            .filter_map(|n| self.hub.st.agents.get(n))
            .filter(|a| a.lifecycle == Lifecycle::Active)
            .map(|a| Live { name: a.name.clone(), dir: a.dir.clone() })
            .collect();
        let clients = !self.typed_ids().is_empty();
        self.procs.sync(live, clients);
    }

    /// The worker's new counts: the agents rows carry them.
    pub(super) fn procs_counts(&mut self, c: Counts) {
        if self.hub.procs != c {
            self.hub.procs = c;
            self.state_now();
        }
    }
}

/// Where a row's output is read.
#[derive(Clone, Debug, PartialEq)]
enum Src {
    File(PathBuf),
    None,
}

/// A running row and what the worker keeps about it.
#[derive(Clone, Debug)]
struct Seen {
    row: Proc,
    dir: String,
    src: Src,
    /// a bg job's folder (its `.rc`)
    bg: Option<PathBuf>,
    pids: Vec<u32>,
}

type Key = (String, String);

struct State {
    paths: Paths,
    hub: String,
    project: String,
    out: Sender<Msg>,
    me: Sender<Ask>,
    agents: Vec<Live>,
    clients: bool,
    watchers: BTreeSet<ClientId>,
    /// (client, agent, id) -> the offset its next chunk starts at
    followers: BTreeMap<(ClientId, String, String), u64>,
    /// the end of each connection's last `proc_out` answer (a follow
    /// starts there: no gap)
    last_end: BTreeMap<(ClientId, String, String), u64>,
    running: BTreeMap<Key, Seen>,
    ended: Vec<Proc>,
    stopping: BTreeSet<Key>,
    /// agents reaped (by name), skipped by the scans for a while
    reaped: BTreeMap<String, std::time::Instant>,
    ports: BTreeMap<(u32, String), Vec<u16>>,
    /// whether each listening port answered HTTP, and when it was asked
    /// (by pid and start: a no is asked again after `HTTP_RETRY`, a
    /// server still starting)
    http: BTreeMap<((u32, String), u16), (bool, std::time::Instant)>,
    asked: BTreeSet<(u32, String)>,
    files: BTreeMap<(u32, String), Files>,
    counts: Counts,
    last_sent: String,
    next_scan: std::time::Instant,
}

const SLOW: Duration = Duration::from_secs(60);
const CLIENT: Duration = Duration::from_secs(15);
const WATCHED: Duration = Duration::from_secs(2);
const FOLLOW: Duration = Duration::from_millis(200);
/// A reaped agent's rows are not scanned again for this long (its
/// processes are being killed).
const REAPED_FOR: Duration = Duration::from_secs(30);
/// A port that did not answer HTTP is asked again after this long.
const HTTP_RETRY: Duration = Duration::from_secs(10);

impl State {
    fn new(paths: Paths, hub: String, project: String, out: Sender<Msg>, me: Sender<Ask>) -> State {
        let ended = read_ended(&ended_file(&paths));
        State {
            paths,
            hub,
            project,
            out,
            me,
            agents: Vec::new(),
            clients: false,
            watchers: BTreeSet::new(),
            followers: BTreeMap::new(),
            last_end: BTreeMap::new(),
            running: BTreeMap::new(),
            ended,
            stopping: BTreeSet::new(),
            reaped: BTreeMap::new(),
            ports: BTreeMap::new(),
            http: BTreeMap::new(),
            asked: BTreeSet::new(),
            files: BTreeMap::new(),
            counts: Counts::new(),
            last_sent: String::new(),
            next_scan: std::time::Instant::now() + Duration::from_secs(2),
        }
    }

    fn run(mut self, rx: Receiver<Ask>) {
        loop {
            let now = std::time::Instant::now();
            let wait = if self.followers.is_empty() { self.next_scan.saturating_duration_since(now) } else { FOLLOW.min(self.next_scan.saturating_duration_since(now)) };
            match rx.recv_timeout(wait) {
                Ok(Ask::Quit) => return,
                Ok(a) => self.on(a),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if std::time::Instant::now() >= self.next_scan {
                let fast = !self.watchers.is_empty();
                self.scan(fast);
                self.send_list(if fast { Some(Typed::All) } else { None });
            }
            if !self.followers.is_empty() {
                self.follow_tick();
            }
        }
    }

    fn period(&self) -> Duration {
        if !self.watchers.is_empty() {
            WATCHED
        } else if self.clients {
            CLIENT
        } else {
            SLOW
        }
    }

    fn on(&mut self, a: Ask) {
        match a {
            Ask::Agents(v) => {
                self.agents = v;
                self.scan(false);
                self.send_list(None);
            }
            Ask::Clients(c) => {
                self.clients = c;
                self.next_scan = self.next_scan.min(std::time::Instant::now() + self.period());
            }
            Ask::List { to, watch } => {
                if let Some((id, on)) = watch {
                    if on {
                        self.watchers.insert(id);
                    } else {
                        self.watchers.remove(&id);
                    }
                }
                self.scan(true);
                self.send_list(Some(to));
            }
            Ask::Out { client, agent, id, from } => self.out_range(client, agent, id, from),
            Ask::Follow { client, agent, id, on } => {
                let k = (client, agent.clone(), id.clone());
                if !on {
                    self.followers.remove(&k);
                    return;
                }
                let start = match self.last_end.get(&k) {
                    Some(e) => *e,
                    None => self.src_of(&agent, &id).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0),
                };
                self.followers.insert(k, start);
            }
            Ask::Stop { client, agent, id } => self.stop(client, &agent, id.as_deref()),
            Ask::Gone(c) => {
                self.watchers.remove(&c);
                self.followers.retain(|k, _| k.0 != c);
                self.last_end.retain(|k, _| k.0 != c);
            }
            Ask::Reaping(dirs) => self.reaping(dirs.as_ref()),
            Ask::Stopped { agent, id, cmd } => {
                self.scan(false);
                self.send_list(None);
                let _ = self.out.send(Msg::input(Input::HubMsg { to: agent, text: pv::stopped_msg(&cmd, &id) }));
            }
            Ask::Quit => {}
        }
    }

    /// The live agents' bg jobs and REPL sessions.
    fn agents_in(&self) -> Vec<AgentIn> {
        self.agents
            .iter()
            .filter(|a| self.reaped.get(&a.name).is_none_or(|t| t.elapsed() > REAPED_FOR))
            .map(|a| {
                let bg = self.paths.agent_bg(&a.dir);
                let jobs = crate::idle::bg_jobs(&bg, crate::procs::alive)
                    .into_iter()
                    .filter_map(|n| {
                        let pgid = std::fs::read_to_string(bg.join(format!("{n}.pid"))).ok()?.trim().parse().ok()?;
                        let cmd = std::fs::read_to_string(bg.join(format!("{n}.cmd"))).unwrap_or_default();
                        Some(BgJob { n: n.parse().ok()?, pgid, cmd })
                    })
                    .collect();
                let adir = self.paths.agent_dir(&a.dir);
                AgentIn { name: a.name.clone(), dir: a.dir.clone(), sessions: crate::procs::read_sids(&adir.join("repl.sids")), repl: repl_of(&adir), bg: jobs }
            })
            .collect()
    }

    /// The ports bise serves itself: its pages server, its views bridge
    /// and each live REPL's plugins bridge (never probed).
    fn own_ports(&self, agents: &[AgentIn]) -> Vec<u16> {
        let base = |p: &Path| std::fs::read_to_string(p).ok().and_then(|t| crate::view_call::bridge_base(&t)).and_then(|b| pv::base_port(&b));
        let pages = std::fs::read_to_string(self.paths.state.join("pages.port")).ok().and_then(|t| t.trim().parse().ok());
        let bridges = agents.iter().filter_map(|a| a.repl).filter_map(|r| base(&self.paths.agent_bridge_json(r.port)));
        pages.into_iter().chain(base(&self.paths.views_bridge_json())).chain(bridges).collect()
    }

    /// One scan: the rows now, what ended since the last one, the counts.
    fn scan(&mut self, ports: bool) {
        self.next_scan = std::time::Instant::now() + self.period();
        let agents = self.agents_in();
        let rows = if agents.is_empty() { Vec::new() } else { table() };
        let mut trees = pv::trees(&rows, &self.hub, &agents, std::process::id());
        // a detached root's command with its quoting (its argv; ps's line
        // when the argv can't be read)
        for t in trees.iter_mut().filter(|t| t.kind == ProcKind::Detached) {
            if let Some(a) = bise_peer::table::argv(t.root) {
                t.cmd = pv::shell_join(&a);
            }
        }
        let by: BTreeMap<u32, &Row> = rows.iter().map(|r| (r.pid, r)).collect();
        let key = |p: u32| by.get(&p).map(|r| (p, r.start.clone()));
        // ports: the pids never asked (all of them while watched)
        let ask: Vec<u32> = trees
            .iter()
            .flat_map(|t| t.pids.iter().copied())
            .filter(|p| key(*p).is_some_and(|k| if ports { !self.ports.get(&k).is_some_and(|v| !v.is_empty()) } else { !self.asked.contains(&k) }))
            .collect();
        if !ask.is_empty() {
            let got = pv::lsof_ports(&lsof(&ask, &["-iTCP", "-sTCP:LISTEN", "-Fpn"]));
            for p in &ask {
                if let Some(k) = key(*p) {
                    self.asked.insert(k.clone());
                    self.ports.insert(k, got.get(p).cloned().unwrap_or_default());
                }
            }
        }
        let roots: Vec<u32> = trees.iter().map(|t| t.root).filter(|p| key(*p).is_some_and(|k| !self.files.contains_key(&k))).collect();
        if !roots.is_empty() {
            let got = pv::lsof_files(&lsof(&roots, &["-d", "cwd,1", "-Fpftn"]));
            for p in &roots {
                if let Some(k) = key(*p) {
                    self.files.insert(k, got.get(p).cloned().unwrap_or_default());
                }
            }
        }
        let alive: BTreeSet<(u32, String)> = rows.iter().map(|r| (r.pid, r.start.clone())).collect();
        self.ports.retain(|k, _| alive.contains(k));
        self.asked.retain(|k| alive.contains(k));
        self.files.retain(|k, _| alive.contains(k));
        self.probe_http(&pv::never_probed(&agents, &self.own_ports(&agents)));
        let http: BTreeSet<u16> = self.http.iter().filter(|(_, (yes, _))| *yes).map(|((_, port), _)| *port).collect();
        let ports_by: BTreeMap<u32, Vec<u16>> = self.ports.iter().map(|((p, _), v)| (*p, v.clone())).collect();
        let files_by: BTreeMap<u32, Files> = self.files.iter().map(|((p, _), v)| (*p, v.clone())).collect();
        let found = Found { ports: &ports_by, files: &files_by, http: &http };
        let now: Vec<Seen> = trees.iter().filter_map(|t| self.seen(t, &by, &found)).collect();
        let now_rows: Vec<Proc> = now.iter().map(|s| s.row.clone()).collect();
        let was: BTreeMap<Key, Proc> = self.running.iter().map(|(k, s)| (k.clone(), s.row.clone())).collect();
        let gone: Vec<Key> = pv::gone(&was, &now_rows).into_iter().map(|p| (p.agent.clone(), p.id.clone())).collect();
        let at = now_ms();
        for k in gone {
            let Some(s) = self.running.remove(&k) else { continue };
            // a reaped agent's rows were ended by the reap already; one
            // whose agent is no longer live went with it
            let why = if self.stopping.remove(&k) {
                Why::You
            } else if !self.agents.iter().any(|a| a.name == k.0) {
                Why::Agent
            } else {
                Why::Exit(s.bg.as_ref().and_then(|bg| rc_of(bg, &s.row.kind)))
            };
            self.end(&s, why, at);
        }
        self.running = now.into_iter().map(|s| ((s.row.agent.clone(), s.row.id.clone()), s)).collect();
        self.prune(at);
        let counts = pv::counts(&self.rows());
        if counts != self.counts {
            self.counts = counts.clone();
            let _ = self.out.send(Msg::ProcCounts(counts));
        }
    }

    /// Asks each listening port whether it serves HTTP: once, a no again
    /// after `HTTP_RETRY`; the ports of processes gone are forgotten.
    /// Never one of `never` (`pv::never_probed`: a say port, bise's own).
    fn probe_http(&mut self, never: &BTreeSet<u16>) {
        let listening: BTreeSet<((u32, String), u16)> =
            self.ports.iter().flat_map(|(k, v)| v.iter().filter(|p| !never.contains(p)).map(move |p| (k.clone(), *p))).collect();
        self.http.retain(|k, _| listening.contains(k));
        let now = std::time::Instant::now();
        for k in listening {
            if self.http.get(&k).is_some_and(|(yes, at)| *yes || now.duration_since(*at) < HTTP_RETRY) {
                continue;
            }
            let yes = http_answers(k.1);
            self.http.insert(k, (yes, now));
        }
    }

    fn seen(&self, t: &Tree, by: &BTreeMap<u32, &Row>, found: &Found) -> Option<Seen> {
        let row = pv::row(t, by, found)?;
        let bg = matches!(t.kind, ProcKind::Bg { .. }).then(|| self.paths.agent_bg(&t.dir));
        let src = match (t.kind, row.out) {
            (ProcKind::Bg { n }, _) => Src::File(self.paths.agent_bg(&t.dir).join(format!("{n}.out"))),
            (_, OutKind::File) => found.files.get(&t.root).and_then(|f| f.out.clone()).map(|p| Src::File(PathBuf::from(p))).unwrap_or(Src::None),
            _ => Src::None,
        };
        Some(Seen { row, dir: t.dir.clone(), src, bg, pids: t.pids.clone() })
    }

    /// The running rows, then the ended ones.
    fn rows(&self) -> Vec<Proc> {
        self.running.values().map(|s| s.row.clone()).chain(self.ended.iter().rev().cloned()).collect()
    }

    /// The list to `to`; with none, to every connection when it changed
    /// (a row came, went, or its ports or state moved).
    fn send_list(&mut self, to: Option<Typed>) {
        let items = self.rows();
        let sig: String = items.iter().map(|p| format!("{}/{}/{:?}/{:?};", p.agent, p.id, p.state, p.ports)).collect();
        let changed = sig != self.last_sent;
        self.last_sent = sig;
        let to = match to {
            Some(t) => t,
            None if changed => Typed::All,
            None => return,
        };
        let v = HubEv::Processes { project: self.project.clone(), items }.to_value();
        let _ = self.out.send(Msg::Typed { to, v });
    }

    /// `s` ended: its row in the log, its output copied.
    fn end(&mut self, s: &Seen, why: Why, at: u64) {
        let mut row = pv::ended(&s.row, why, at);
        let copy = procs_dir(&self.paths).join(out_name(&row.agent, &row.id));
        let kept = match &s.src {
            Src::File(p) => std::fs::read(p).ok().and_then(|b| {
                let _ = std::fs::create_dir_all(procs_dir(&self.paths));
                std::fs::write(&copy, pv::copy_tail(&b)).ok()
            }),
            Src::None => None,
        };
        if kept.is_none() {
            row.out = OutKind::None;
        }
        append_ended(&ended_file(&self.paths), &row);
        self.ended.push(row);
    }

    /// Ended rows older than a day go, with their output copies.
    fn prune(&mut self, now: u64) {
        let before = self.ended.len();
        let kept = pv::prune(std::mem::take(&mut self.ended), now);
        if kept.len() == before {
            self.ended = kept;
            return;
        }
        let keep: BTreeSet<String> = kept.iter().map(|p| out_name(&p.agent, &p.id)).collect();
        if let Ok(rd) = std::fs::read_dir(procs_dir(&self.paths)) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.ends_with(".out") && !keep.contains(&n) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        write_ended(&ended_file(&self.paths), &kept);
        self.ended = kept;
    }

    /// proc-cleanup kills these agents' processes next: their running
    /// rows end now as stopped with their agent.
    fn reaping(&mut self, dirs: Option<&BTreeSet<String>>) {
        let at = now_ms();
        let hit: Vec<Key> = self.running.iter().filter(|(_, s)| dirs.is_none_or(|d| d.contains(&s.dir))).map(|(k, _)| k.clone()).collect();
        for k in hit {
            if let Some(s) = self.running.remove(&k) {
                self.reaped.insert(s.row.agent.clone(), std::time::Instant::now());
                self.stopping.remove(&k);
                self.end(&s, Why::Agent, at);
            }
        }
        for a in self.agents.iter().filter(|a| dirs.is_none_or(|d| d.contains(&a.dir))) {
            self.reaped.insert(a.name.clone(), std::time::Instant::now());
        }
        let counts = pv::counts(&self.rows());
        if counts != self.counts {
            self.counts = counts.clone();
            let _ = self.out.send(Msg::ProcCounts(counts));
        }
        self.send_list(None);
    }

    /// Where row (agent, id)'s output is: running, its file; ended, the
    /// copy the hub kept.
    fn src_of(&self, agent: &str, id: &str) -> Option<PathBuf> {
        if let Some(s) = self.running.get(&(agent.to_string(), id.to_string())) {
            return match &s.src {
                Src::File(p) => Some(p.clone()),
                Src::None => None,
            };
        }
        self.ended
            .iter()
            .any(|p| p.agent == agent && p.id == id && p.out != OutKind::None)
            .then(|| procs_dir(&self.paths).join(out_name(agent, id)))
    }

    fn known(&self, agent: &str, id: &str) -> bool {
        self.running.contains_key(&(agent.to_string(), id.to_string())) || self.ended.iter().any(|p| p.agent == agent && p.id == id)
    }

    fn error(&self, client: ClientId, cmd: &str, text: String) {
        let v = HubEv::Error { project: Some(self.project.clone()), cmd: Some(cmd.into()), text, cid: None, reason: None, kind: None }.to_value();
        let _ = self.out.send(Msg::Typed { to: Typed::Answer(client), v });
    }

    fn out_range(&mut self, client: ClientId, agent: String, id: String, from: Option<u64>) {
        if !self.known(&agent, &id) {
            return self.error(client, "proc_out", format!("no process {id} of {agent}"));
        }
        let (at, data, end) = match self.src_of(&agent, &id) {
            Some(p) => read_range(&p, from),
            None => (0, Vec::new(), 0),
        };
        self.last_end.insert((client, agent.clone(), id.clone()), end);
        let v = HubEv::ProcOut { project: self.project.clone(), agent, id, at, data: b64(&data), end }.to_value();
        let _ = self.out.send(Msg::Typed { to: Typed::Answer(client), v });
    }

    /// Each follower's new bytes, a chunk at most per tick.
    fn follow_tick(&mut self) {
        let ks: Vec<(ClientId, String, String)> = self.followers.keys().cloned().collect();
        for k in ks {
            let Some(p) = self.src_of(&k.1, &k.2) else { continue };
            let off = self.followers[&k];
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            // a file that shrank (an ended row's copy): from its start
            let off = if size < off { 0 } else { off };
            if size == off {
                continue;
            }
            let (at, data, end) = read_range(&p, Some(off));
            self.followers.insert(k.clone(), at + data.len() as u64);
            let v = HubEv::ProcOut { project: self.project.clone(), agent: k.1.clone(), id: k.2.clone(), at, data: b64(&data), end }.to_value();
            let _ = self.out.send(Msg::Typed { to: Typed::Answer(k.0), v });
        }
    }

    /// His stop: the rows marked, the signals on a thread of their own.
    fn stop(&mut self, client: ClientId, agent: &str, id: Option<&str>) {
        let hit: Vec<Seen> = self.running.values().filter(|s| s.row.agent == agent && id.is_none_or(|i| s.row.id == i)).cloned().collect();
        if hit.is_empty() {
            let what = id.map(|i| format!("{i} of {agent} doesn't run")).unwrap_or_else(|| format!("{agent} has no process running"));
            return self.error(client, "proc_stop", what);
        }
        for s in hit {
            let k = (s.row.agent.clone(), s.row.id.clone());
            if !self.stopping.insert(k) {
                continue;
            }
            let me = self.me.clone();
            std::thread::spawn(move || {
                signal_stop(&s);
                let _ = me.send(Ask::Stopped { agent: s.row.agent.clone(), id: s.row.id.clone(), cmd: s.row.cmd.clone() });
            });
        }
    }
}

/// The stop's signals: the process the row saw (same pid and start) or
/// nothing; a bg job gets bise's line in its output first, then TERM to
/// its group; a detached tree TERM to each; KILL to what is left after
/// the grace.
fn signal_stop(s: &Seen) {
    let rows = table();
    if !pv::same_process(&rows, s.row.pid, s.row.started_ms) {
        return;
    }
    let starts: BTreeMap<u32, String> = rows.iter().filter(|r| s.pids.contains(&r.pid)).map(|r| (r.pid, r.start.clone())).collect();
    let group = matches!(s.row.kind, ProcKind::Bg { .. });
    if group {
        if let Src::File(p) = &s.src {
            note_line(p, pv::STOPPED_NOTE);
        }
        crate::procs::signal_group(s.row.pid, false);
    } else {
        for p in starts.keys() {
            crate::procs::terminate(*p);
        }
    }
    let t0 = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let left: Vec<u32> = table().into_iter().filter(|r| !r.zombie && starts.get(&r.pid) == Some(&r.start)).map(|r| r.pid).collect();
        if left.is_empty() {
            return;
        }
        if t0.elapsed() >= crate::procs::STOP_GRACE {
            if group {
                crate::procs::signal_group(s.row.pid, true);
            }
            for p in left {
                crate::procs::kill_now(p);
            }
            return;
        }
    }
}

/// A line of bise's at the end of an output file, on a line of its own.
fn note_line(p: &Path, line: &str) {
    use std::io::Write;
    let last_nl = std::fs::read(p).ok().is_none_or(|b| b.last().is_none_or(|c| *c == b'\n'));
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(p) {
        let _ = writeln!(f, "{}{}", if last_nl { "" } else { "\n" }, line);
    }
}

/// The process table with its ps stats (one `ps -E` read by the peer
/// table, one `ps -o` for groups, starts, cpu, memory, command lines).
fn table() -> Vec<Row> {
    let stats = std::process::Command::new("ps")
        .args(pv::PS_STATS)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| pv::parse_stats(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    pv::join(&crate::procs::snapshot(), &stats, |s| {
        let (y, mo, d, h, mi, sec) = pv::lstart_parts(s)?;
        crate::every::mktime_s(y, mo, d, h, mi, sec)
    })
}

/// Whether 127.0.0.1:`port` answers HTTP (`pv::HTTP_PROBE`, its first
/// bytes read by `pv::answers_http`); a short wait each way, so a port
/// that takes the bytes and says nothing costs half a second, once.
fn http_answers(port: u16) -> bool {
    use std::io::Write as _;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_millis(300)));
    let _ = s.set_write_timeout(Some(Duration::from_millis(200)));
    if s.write_all(pv::HTTP_PROBE).is_err() {
        return false;
    }
    let mut buf = [0u8; 8];
    let mut got = 0;
    while got < 5 {
        match s.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    pv::answers_http(&buf[..got])
}

/// The agent's REPL as `repl.json` names it (`daemon/repl.rs` writes it
/// at the spawn, an adopting hub trusts it).
fn repl_of(adir: &Path) -> Option<pv::Repl> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(adir.join("repl.json")).ok()?).ok()?;
    Some(pv::Repl { pid: v.get("pid")?.as_u64()?.try_into().ok()?, port: v.get("port")?.as_u64()?.try_into().ok()? })
}

/// One lsof on `pids` with `what` (no lsof: nothing).
fn lsof(pids: &[u32], what: &[&str]) -> String {
    let list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    std::process::Command::new("lsof")
        .args(["-nP", "-a", "-p", &list])
        .args(what)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// A bg job's exit code, from its slot's `.rc` while it is there.
fn rc_of(bg: &Path, kind: &ProcKind) -> Option<i32> {
    let ProcKind::Bg { n } = kind else { return None };
    std::fs::read_to_string(bg.join(format!("{n}.rc"))).ok()?.trim().parse().ok()
}

/// Bytes of `p` from `from` (none: its last chunk): (at, bytes, size).
fn read_range(p: &Path, from: Option<u64>) -> (u64, Vec<u8>, u64) {
    let Ok(mut f) = std::fs::File::open(p) else { return (0, Vec::new(), 0) };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let (at, len) = pv::out_range(size, from);
    let mut buf = vec![0u8; len as usize];
    if f.seek(SeekFrom::Start(at)).is_err() || f.read_exact(&mut buf).is_err() {
        return (at, Vec::new(), size);
    }
    (at, buf, size)
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(A[(n >> 18) as usize & 63] as char);
        s.push(A[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    s
}

fn procs_dir(paths: &Paths) -> PathBuf {
    paths.procs_dir()
}

fn ended_file(paths: &Paths) -> PathBuf {
    procs_dir(paths).join("ended.jsonl")
}

/// An ended row's output copy: `<agent>-<id>.out`.
fn out_name(agent: &str, id: &str) -> String {
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '_' }).collect::<String>();
    format!("{}-{}.out", safe(agent), safe(id))
}

fn read_ended(f: &Path) -> Vec<Proc> {
    std::fs::read_to_string(f).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn append_ended(f: &Path, row: &Proc) {
    use std::io::Write;
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let (Ok(mut h), Ok(l)) = (std::fs::OpenOptions::new().create(true).append(true).open(f), serde_json::to_string(row)) {
        let _ = writeln!(h, "{l}");
    }
}

fn write_ended(f: &Path, rows: &[Proc]) {
    let text: String = rows.iter().filter_map(|r| serde_json::to_string(r).ok()).map(|l| l + "\n").collect();
    let tmp = f.with_extension("jsonl.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(tmp, f);
    }
}

#[cfg(test)]
#[path = "processes_tests.rs"]
mod tests;
