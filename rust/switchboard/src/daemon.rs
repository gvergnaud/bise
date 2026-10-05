//! `sbd`: the hub of one workspace (RFC 0001 §5), the imperative shell
//! around `core::Hub`. One thread owns the hub and executes its effects;
//! the other threads only read (sockets, REPL output) and send messages.
//!
//! - one Bend REPL per live agent, spawned from the app root, with
//!   BEND_WORKDIR / BEND_EXTRA_PROMPT / BEND_CONTEXT_FILE / SB_AGENT;
//! - `hub.sock`: clients (`{"op":"hello"}` first, then JSON lines) and
//!   the agents' `sb` CLI (`{"op":"agent", ...}`, one request, one reply);
//! - `journal.jsonl`: the durable state; `agents/<dir>/transcript.log`:
//!   every line of each feed, for the views, `sb inspect` and
//!   `sb history`.

mod art;
mod features;
mod gate;
mod repl;
mod release;
mod session_log;
mod versions;

use repl::{adopt, adoptable, busy_at, kill_pid, supervise};
use versions::version_allowed;
use crate::core::{AgentReq, ClientId, Effect, Hub, Input, Token};
use crate::devflow;
use crate::model::{Agent, Lifecycle, MAIN};
use crate::paths::Paths;
use crate::prompts;
use crate::search;
use crate::transcript::{self, Anchor};
use crate::util::{now_ms, wire_escape};
use crate::worktree::{Config, GitEnv};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Lines kept in memory per feed and replayed to a new client (older
/// ones stay in the transcript: the client pages them with the
/// `history` op when the user scrolls up).
const BUFFER_LINES: usize = 1000;
/// Lines one `history` page may carry.
const PAGE_LINES: usize = 2000;

pub struct Opts {
    pub paths: Paths,
    /// Where repl-live and the runtime's relative files live.
    pub app_root: PathBuf,
    /// The sb-core of the app root (never one named by the environment).
    pub core_bin: PathBuf,
    /// The run_typescript engine the REPLs get (BEND_JSRT_BIN).
    pub jsrt_bin: Option<PathBuf>,
    /// The bise executable (the `sb` shim calls it).
    pub exe: PathBuf,
    pub repl_bin: PathBuf,
    /// Extra env of each REPL, computed at its spawn: the API keys
    /// resolved again (auth.json, .env), so a `login` since the hub
    /// started applies to the next REPL, and the models file written
    /// again (BISE_MODELS_FILE: a base_url set in config.toml or a .env
    /// file since). Also called at each input and idle (keys_changed):
    /// the live REPLs read the rewritten file at their next call.
    /// (name, None) = unset.
    pub spawn_env: Option<SpawnEnv>,
}

/// The env a REPL gets at its spawn: (name, Some(value)) sets, (name,
/// None) unsets.
pub type SpawnEnv = fn() -> Vec<(String, Option<String>)>;

enum Msg {
    In(Input),
    ReplConnected {
        dir: String,
        gen: u64,
        stream: TcpStream,
        steer: String,
        interrupt: String,
        pid: u32,
        /// A REPL left running by a previous hub, reconnected (adopt).
        adopted: bool,
        /// Adopted in the middle of a turn.
        busy: bool,
    },
    /// The process exists (before its banner): it must die with the hub.
    ReplSpawned {
        dir: String,
        gen: u64,
        pid: u32,
    },
    ReplLine {
        dir: String,
        gen: u64,
        line: String,
        /// Offset in the wire log just after this line.
        offset: u64,
    },
    ReplGone {
        dir: String,
        gen: u64,
        reason: String,
    },
    ClientNew {
        id: ClientId,
        stream: UnixStream,
    },
    ClientLine {
        id: ClientId,
        v: Value,
    },
    ClientGone {
        id: ClientId,
    },
    AgentNew {
        token: Token,
        stream: UnixStream,
        v: Value,
    },
    /// `sb version …` (one request, one answer).
    Version {
        stream: UnixStream,
        v: Value,
    },
    /// A `/version` build is over.
    BuildEnded {
        rev: String,
    },
    /// A `/release-bise` event (BISE-235): to that client, or to all.
    Release {
        client: Option<ClientId>,
        v: Value,
    },
    /// The end of a `/update` build in bise's source tree (dev-update).
    Update(Value),
    /// A line for main's thread (the version switcher).
    Notice {
        kind: String,
        text: String,
    },
    /// A role-line call is over (BISE-126).
    RoleLine {
        dir: String,
        key: String,
        line: Option<String>,
    },
    /// The checker answered a gated call (approvals-design.md §4).
    GateChecked {
        dir: String,
        n: String,
        req: Box<crate::approvals::check::CheckReq>,
        out: crate::approvals::check::CheckOut,
    },
    /// A land joined the line or ended (`sb land`): the views refresh
    /// (the lids); `line`: main's feed line, (kind, text).
    Land {
        line: Option<(String, String)>,
    },
    /// An agent's changes, computed off the loop (`daemon/art.rs`).
    Changes {
        name: String,
        v: Value,
    },
    /// An answer computed off the loop (a `diff`, the `branches`) for
    /// one client.
    ToClient {
        id: ClientId,
        v: Value,
    },
    /// `keep`: leave the REPLs running for the next hub to adopt.
    Shutdown {
        keep: bool,
    },
}

struct Repl {
    stream: TcpStream,
    steer: String,
    interrupt: String,
}

struct Shell {
    opts: Opts,
    hub: Hub,
    env: GitEnv,
    tx: Sender<Msg>,
    journal: std::fs::File,
    repls: BTreeMap<String, Repl>,
    /// The live generation of each agent's REPL: lines and exits of an
    /// older (killed) generation are ignored.
    gens: BTreeMap<String, u64>,
    next_gen: u64,
    /// Every REPL process alive, connected or not: (generation, pid).
    pids: BTreeMap<String, (u64, u32)>,
    /// REPLs asked for whose process is not spawned yet: (generation,
    /// since). One stuck there past START_LIMIT is a failed start, said
    /// in main's feed and restarted (BISE-291: it stayed `starting`
    /// forever, with no line anywhere).
    starts: BTreeMap<String, (u64, std::time::Instant)>,
    clients: BTreeMap<ClientId, UnixStream>,
    replies: BTreeMap<Token, UnixStream>,
    /// The last lines of each feed, with their transcript positions and
    /// the time the transcript wrote them (ms since the epoch).
    buffers: BTreeMap<String, VecDeque<(usize, u64, String)>>,
    /// The position of the last line of each feed's transcript.
    positions: BTreeMap<String, usize>,
    /// Every thread, indexed for `sb history` (built at the first search).
    search: search::Index,
    /// Wire-log offsets processed since the last flush to `wire.offset`.
    offsets: BTreeMap<String, u64>,
    /// True while `Input::Boot` runs: its spawns may adopt a REPL.
    booting: bool,
    /// The binary and port of each live REPL (adopted ones may run
    /// another version's binary: they switch at their next idle).
    bins: BTreeMap<String, PathBuf>,
    ports: BTreeMap<String, u16>,
    /// REPLs switching to this hub's binary (asked to reload between
    /// turns): the writes meant for them wait here until the new process
    /// is connected, on the same port and session.
    switching: BTreeMap<String, Vec<String>>,
    /// Of those, the ones whose new process is already spawned: its exit
    /// is a crash (of the new version), not the reload.
    switch_spawned: BTreeSet<String>,
    /// Restarted by a switch: their greeting (restored history) is not
    /// news for the feeds.
    restored: BTreeSet<String>,
    /// Versions being built (`/version <commit>`), by revision.
    building: BTreeSet<String>,
    /// The `/update` build in bise's source tree (dev-update): HEAD's
    /// short hash and when it started.
    updating: Option<(String, std::time::Instant)>,
    /// The `/release-bise` running (BISE-235).
    release: Option<release::ReleaseRun>,
    /// An installed bise (BISE-172): when `current` was last looked at,
    /// and the version already announced as ready.
    update_checked: Option<std::time::Instant>,
    update_told: Option<std::path::PathBuf>,
    /// update-card: when the release channel was last checked (None: not
    /// yet, the first tick checks).
    release_checked: Option<std::time::Instant>,
    /// Agents whose REPL was found dead at boot in the middle of a turn
    /// (killed by a restart, a crash): once respawned on their session,
    /// they are told to continue where they left off.
    resume_turn: BTreeSet<String>,
    /// The id of the reload that started this hub ("" when none,
    /// BISE-131): in the hello, the TUIs that knew another re-exec.
    reload_id: String,
    /// Adopted REPLs a reload relaunches at their next idle (same
    /// session, same port), like a switch to their own binary.
    reload_repls: BTreeSet<String>,
    /// The keys each live REPL was spawned with (a hash of `spawn_env`'s
    /// answer, by agent dir): a key saved since (the first run's key
    /// step, `bise login`) relaunches it at its next idle, same session
    /// (BISE-266).
    spawn_keys: BTreeMap<String, u64>,
    /// The plugins and skills each live REPL was spawned with (its
    /// workspace, whether it is main, and the fingerprints of its plugin
    /// and skill roots, by agent dir): a plugin installed, removed, enabled or
    /// edited since, or a SKILL.md added, edited or removed in a skill
    /// folder its prompt reads, relaunches it at its next idle, same
    /// session, so it gets the new tools and skills. Never the TUI: only
    /// the REPL restarts (the user: a recording or a draft in progress
    /// must survive).
    spawn_plugins: BTreeMap<String, PromptInputs>,
    /// The last plugins check (every 2 s on the tick).
    plugins_checked: Option<std::time::Instant>,
    /// The small model failed and agent_model answered: role lines use
    /// agent_model for the rest of this hub's life (BISE-126).
    small_broken: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The catalog and config.toml (BISE-135: the model and effort each
    /// agent runs with), re-read when config.toml changes.
    setup: Option<(Option<std::time::SystemTime>, bise_catalog::Setup)>,
    /// Each live REPL's session log writer (BISE-196), by agent dir.
    recorders: BTreeMap<String, bise_session::recorder::Recorder>,
    /// The archived agents (dirs): one more is a /drop, whose worktree
    /// folders the hub cleans (BISE-230, `sweep`).
    archived: BTreeSet<String>,
    /// This hub's id in the agents' process tags (BISE-243, `procs`).
    proc_hub: String,
    /// The stopped and archived agents (dirs): one more gets its
    /// processes killed (BISE-243).
    down: BTreeSet<String>,
    /// The approvals mode and gate (approvals-design.md §8-§10).
    gates: gate::Gates,
    /// `sb land`'s line: one land at a time per target ref (dev-flow §5).
    lands: crate::land::Queue,
    /// The feature branches (dev-flow §5.1): the registry, their facts,
    /// the try builds running (`daemon/features.rs`).
    features: features::Features,
    /// The PR poller (pr-design §7, `forge::poll`), on its own thread;
    /// its answers come back as `Input::Prs`.
    prs: Option<crate::forge::poll::Poller>,
    /// computer use's events.jsonl: each stop, one line in main's feed
    cu: crate::computer_use::Watch,
    /// idle-exit (`idle.rs`): no UI for the grace and nothing runs, the
    /// hub stops for good
    idle: crate::idle::Watch,
    /// holds on the hub besides the `hello` clients: a page server gets
    /// a clone, and each open event stream holds one while it is open
    holds: crate::idle::Holds,
    /// artifacts and diffs (`daemon/art.rs`, docs/artifacts.md)
    art: art::Art,
}

/// What an agent whose turn was cut by a restart receives.
/// How long a REPL may wait for its process to be spawned (the login
/// shell's PATH, its AGENTS.md: seconds at most) before its start counts
/// as failed (BISE-291).
const START_LIMIT: Duration = Duration::from_secs(20);

/// At a stop for good, how long the REPLs get to checkpoint and exit
/// before SIGTERM.
const REPL_QUIT: Duration = Duration::from_secs(5);

/// Tests only (tui_stuck_start_tmux.py): `SB_STALL_START=<file>` holds
/// the next REPL start before its spawn, forever, while that file
/// exists; the file is removed, so only one start stalls.
fn stall_for_tests() {
    let Some(f) = bise_home::env::test_setting("SB_STALL_START") else {
        return;
    };
    if std::fs::remove_file(&f).is_ok() {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
}

const RESUME_TEXT: &str = "Your turn was interrupted by a restart of Switchboard; continue where you left off.";

/// The journal's events, and the (1-based) numbers of the lines that are
/// not a JSON object (a half-written last line...): they are never dropped
/// in silence, the caller logs them. The Rust side does not decode the
/// events (hub/codec.bend does), so a kind it does not know still reaches
/// sb-core.
fn read_journal(text: &str) -> (Vec<Value>, Vec<usize>) {
    let mut events = Vec::new();
    let mut bad = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(l) {
            Ok(v) if v.is_object() => events.push(v),
            _ => bad.push(i + 1),
        }
    }
    (events, bad)
}

fn lines_list(ns: &[usize]) -> String {
    let mut s: Vec<String> = ns.iter().take(10).map(|n| n.to_string()).collect();
    if ns.len() > 10 {
        s.push("...".into());
    }
    s.join(", ")
}

fn log_line(paths: &Paths, s: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
    {
        let _ = writeln!(f, "{} {}", now_ms(), s);
    }
}

/// Write a file an agent depends on (its role, its wire log and offset):
/// a failure (a full disk, a permission) goes to hub.log, not nowhere
/// (BISE-292). True when written.
fn write_logged(paths: &Paths, file: &Path, text: &str) -> bool {
    std::fs::write(file, text)
        .map_err(|e| log_line(paths, &format!("cannot write {}: {}", file.display(), e)))
        .is_ok()
}

/// `rename`, its failure in hub.log (see `write_logged`).
fn rename_logged(paths: &Paths, from: &Path, to: &Path) {
    if let Err(e) = std::fs::rename(from, to) {
        log_line(paths, &format!("cannot rename {} to {}: {}", from.display(), to.display(), e));
    }
}

fn write_json(stream: &mut UnixStream, v: &Value) -> bool {
    write_line(stream, &v.to_string())
}

fn write_line(stream: &mut UnixStream, line: &str) -> bool {
    let mut s = String::with_capacity(line.len() + 1);
    s.push_str(line);
    s.push('\n');
    stream.write_all(s.as_bytes()).is_ok()
}

/// The lines of a transcript at positions [before - count, before)
/// (positions from 1, as `transcript.rs`), each with the time it was
/// written (ms, None when the stamp does not parse), without keeping the
/// rest of the file in memory.
fn transcript_page(path: &Path, before: usize, count: usize) -> Vec<(usize, Option<u64>, String)> {
    use std::io::BufRead;
    let Ok(f) = std::fs::File::open(path) else { return Vec::new() };
    let from = before.saturating_sub(count).max(1);
    let mut out = Vec::new();
    for (i, l) in std::io::BufReader::new(f).lines().enumerate() {
        let pos = i + 1;
        if pos >= before {
            break;
        }
        let Ok(l) = l else { break };
        if pos >= from {
            if let Some((ms, line)) = l.split_once('\t') {
                out.push((pos, ms.parse().ok(), line.to_string()));
            }
        }
    }
    out
}

/// One line of a `history` page (C2): `{pos, line}`, plus `ts` (the
/// time the transcript wrote it, ms since the epoch) when known. `ts` is
/// optional: a client reads a line without it as before.
fn history_line(pos: usize, ts: Option<u64>, line: &str) -> Value {
    let mut v = json!({"pos": pos, "line": line});
    if let Some(ts) = ts {
        v["ts"] = json!(ts);
    }
    v
}

/// A live line of a feed (C2 `line`): `ts`, the time the transcript
/// wrote it (ms since the epoch), is left out when unknown (0, a stamp
/// that did not parse); a client reads a line without it as before.
fn line_event(agent: &str, pos: usize, ts: u64, line: &str) -> Value {
    let mut v = json!({"ev": "line", "agent": agent, "line": line, "pos": pos});
    if ts > 0 {
        v["ts"] = json!(ts);
    }
    v
}

/// How many lines a transcript holds (the position of its last line).
fn transcript_len(path: &Path) -> usize {
    std::fs::read(path)
        .map(|b| b.iter().filter(|c| **c == b'\n').count())
        .unwrap_or(0)
}

fn free_port() -> std::io::Result<u16> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(l.local_addr()?.port())
}

impl Shell {
    /// Save how far each wire log was processed: the next hub adopts
    /// the REPLs from there.
    fn flush_offsets(&mut self) {
        for (dir, off) in std::mem::take(&mut self.offsets) {
            let d = self.opts.paths.agent_dir(&dir);
            if write_logged(&self.opts.paths, &d.join("wire.offset.tmp"), &off.to_string()) {
                rename_logged(&self.opts.paths, &d.join("wire.offset.tmp"), &d.join("wire.offset"));
            }
        }
    }

    fn agent_by_dir(&self, dir: &str) -> Option<&Agent> {
        self.hub.st.agents.values().find(|a| a.dir == dir)
    }

    fn dir_of(&self, name: &str) -> Option<String> {
        self.hub.st.agents.get(name).map(|a| a.dir.clone())
    }

    // ---- the model and effort of each agent (BISE-135) ----

    /// The catalog merged with config.toml, re-read when the file
    /// changes (a `/model ... default`, the user's editor).
    fn setup(&mut self) -> &bise_catalog::Setup {
        let path = bise_home::Home::from_env().config_file();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.setup.as_ref().is_none_or(|(t, _)| *t != mtime) {
            self.setup = Some((mtime, bise_catalog::Setup::load(&path)));
        }
        &self.setup.as_ref().expect("just set").1
    }

    /// The agent's own choice (`/model`, `/reasoning`): a file of its
    /// state dir, its REPL reads it before each call.
    fn choice_path(&self, dir: &str) -> PathBuf {
        self.opts.paths.agent_dir(dir).join("choice.toml")
    }

    /// What an agent runs with: the same resolution as its REPL's
    /// (rust/catalog Setup::in_use = runtime/provider-pure.bend).
    fn in_use(&mut self, dir: &str, is_main: bool) -> bise_catalog::InUse {
        let choice = bise_catalog::Choice::read(&self.choice_path(dir));
        self.setup().in_use(if is_main { "main" } else { "agent" }, &choice)
    }

    // ---- the model of a task, chosen at its spawn (issue #4) ----

    /// Whether a provider can run a turn now: its key or its sign-in
    /// (auth.json, the env, the .env files: the REPLs' own sources).
    fn key_ready() -> impl Fn(&bise_catalog::Provider) -> bool {
        let home = bise_home::Home::from_env();
        let files = bise_catalog::auth::EnvFile::read_all(&home.env_files());
        let store = bise_catalog::auth::Store::read(&home.auth_file()).unwrap_or_default();
        move |p| {
            let env = |k: &str| std::env::var(k).ok();
            bise_catalog::auth::Keys { env: &env, store: &store, files: &files }.ready(p)
        }
    }

    /// A line of the hub in an agent's thread (`sb info : ...`).
    fn info_line(&mut self, agent: &str, text: &str) {
        self.feed(agent, &format!("sb info : {}", wire_escape(text)));
    }

    /// `sb spawn ... --model/--effort/--profile`: pick what the new task
    /// runs on (the agents default when the ask cannot run), write its
    /// choice file before its first call, put the fallback's line at the
    /// top of its thread. The words of main's answer (`on gpt-9 · low`).
    fn spawn_model(&mut self, agent: &str, ask: &bise_catalog::spawn::Ask) -> String {
        let Some(dir) = self.dir_of(agent) else { return String::new() };
        let ready = Self::key_ready();
        let pick = self.setup().spawn_pick(ask, &ready);
        if let Err(e) = pick.choice.write(&self.choice_path(&dir)) {
            log_line(&self.opts.paths, &format!("{}: its model choice is not saved: {}", agent, e));
        }
        if !pick.choice.model.is_empty() {
            self.hub.model_trial.insert(agent.to_string());
        }
        if !pick.line.is_empty() {
            self.info_line(agent, &pick.line);
        }
        log_line(&self.opts.paths, &format!("{}: spawned {}", agent, pick.answer));
        self.refresh_models();
        pick.answer
    }

    /// `sb send <task> --model <id> [--effort <e>]`: the task's choice
    /// from its next turn, a line in its thread. Unlike the spawn, a model
    /// that cannot run is refused (the task keeps its model): main asked
    /// a running task to move, it hears why not.
    fn switch_model(&mut self, from: &str, to: &str, model: &str, effort: &str) -> Result<String, String> {
        let dir = self.dir_of(to).ok_or_else(|| format!("unknown agent: {}", to))?;
        let ready = Self::key_ready();
        let ask = bise_catalog::spawn::Ask { model: model.into(), effort: effort.into(), profile: String::new() };
        let mut pick = self.setup().spawn_pick(&ask, &ready);
        if !pick.choice.why.is_empty() {
            return Err(format!("@{} stays on its model: asked {}: {}", to, pick.choice.asked, pick.choice.why));
        }
        let before = bise_catalog::Choice::read(&self.choice_path(&dir));
        if model.trim().is_empty() {
            // an effort alone: on the model it runs now
            pick.choice.model = before.model.clone();
            let ask2 = bise_catalog::spawn::Ask { model: before.model.clone(), effort: effort.into(), profile: String::new() };
            if !before.model.is_empty() {
                pick = self.setup().spawn_pick(&ask2, &ready);
            }
        }
        pick.choice.by = "sb send --model".into();
        pick.choice.write(&self.choice_path(&dir)).map_err(|e| format!("@{}'s choice is not saved: {}", to, e))?;
        let used = self.in_use(&dir, false);
        let on = bise_catalog::names::with_effort(&used.model.name, &used.effort);
        let by = if from == MAIN { "main".to_string() } else { from.to_string() };
        self.info_line(to, &format!("{} moved this agent to {}, from its next turn.", by, on));
        if !pick.choice.model.is_empty() {
            self.hub.model_trial.insert(to.to_string());
        }
        self.refresh_models();
        if self.repls.contains_key(&dir) {
            // another context window: the compaction threshold follows
            self.reload_repls.insert(dir);
        }
        let note = if pick.choice.note.is_empty() { String::new() } else { format!(" ({})", pick.choice.note) };
        Ok(format!("@{} moves to {}, from its next turn{}", to, on, note))
    }

    /// The provider refused the model a task was spawned on, before any
    /// turn of it ended well: the agents default, a line in its thread,
    /// a word to main, and its turn starts again.
    fn model_refused(&mut self, agent: &str, why: &str) {
        let Some(dir) = self.dir_of(agent) else { return };
        let path = self.choice_path(&dir);
        let before = bise_catalog::Choice::read(&path);
        let asked = before.model.clone();
        let choice = bise_catalog::Choice { asked: asked.clone(), why: why.to_string(), ..Default::default() };
        if let Err(e) = choice.write(&path) {
            log_line(&self.opts.paths, &format!("{}: its model choice is not saved: {}", agent, e));
        }
        let used = self.in_use(&dir, false);
        let on = bise_catalog::names::with_effort(&used.model.name, &used.effort);
        let short = bise_catalog::names::long_name(&asked);
        let text = format!("asked for {}: {}. running on {}, the agents default.", short, why, on);
        self.info_line(agent, &text);
        log_line(&self.opts.paths, &format!("{}: {}", agent, text));
        self.refresh_models();
        // main hears it as a report of the task (its spawn answer named the
        // model it asked), and the task's turn starts again on the default
        let report = AgentReq::Report { kind: "progress".into(), summary: text.clone(), decisions: Vec::new() };
        self.step(Input::Agent { token: 0, from: agent.to_string(), req: report });
        self.step(Input::Agent {
            token: 0,
            from: MAIN.to_string(),
            req: AgentReq::Send {
                to: agent.to_string(),
                text: format!("bise: {} your turn starts again on it: carry on with your task.", text),
                expect_reply: false,
                reply_to: None,
                queued: true,
                why: String::new(),
                switch: None,
            },
        });
        let snap = self.snapshot();
        self.broadcast(&snap);
    }

    /// Each agent's model for `sb list`, `sb tasks` and the roster
    /// (issue #4): its tag and its `model:` line, from its choice file.
    fn refresh_models(&mut self) {
        let who: Vec<(String, String, bool)> =
            self.hub.st.agents.values().map(|a| (a.name.clone(), a.dir.clone(), a.is_main)).collect();
        self.setup(); // re-read when config.toml changed
        let setup = &self.setup.as_ref().expect("just set").1;
        let used: Vec<(String, bise_catalog::InUse, bise_catalog::Choice)> = who
            .iter()
            .map(|(name, dir, main)| {
                let c = bise_catalog::Choice::read(&self.choice_path(dir));
                (name.clone(), setup.in_use(if *main { "main" } else { "agent" }, &c), c)
            })
            .collect();
        let others: Vec<&str> = used.iter().map(|(_, u, _)| u.model.name.as_str()).collect();
        let models: crate::board::Models = used
            .iter()
            .map(|(n, u, c)| {
                // whole: the roster sizes its column (board::roster)
                let tag = bise_catalog::names::tag(&u.model.name, &u.effort, &others, "·", usize::MAX);
                (n.clone(), (tag, setup.model_line(c)))
            })
            .collect();
        self.hub.models = models;
    }

    /// The client snapshot with each agent's model and effort: its full
    /// id, the effort ("" when the model takes none), and the words it
    /// takes (the `/reasoning` list).
    /// `sb land` (dev-flow §5) in a thread: in line on the queue (the
    /// views say who waits), git and the check never on the hub's loop;
    /// the answer to the agent, and a line in main's feed.
    fn land(&mut self, token: Token, mut job: crate::land::Job) {
        let Some(mut stream) = self.replies.remove(&token) else {
            return;
        };
        job.flow = crate::flow::FlowConfig::load(&self.opts.paths);
        let (queue, tx) = (self.lands.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let txj = tx.clone();
            let res = crate::land::run(&job, &queue, &mut || {
                let _ = txj.send(Msg::Land { line: None });
            });
            let mut landed: Option<String> = None;
            let (body, line) = match res {
                Ok(o) => {
                    landed = art::landed_fields(&job.shared, &job.agent, &o.target, &o.sha, o.commits);
                    let mut text = crate::flow::land_line(&job.agent, o.commits, &o.target, &o.sha, o.pushed);
                    if let Some(e) = &o.push_error {
                        text = format!("{} ({})", text, e);
                    }
                    let note = crate::land::left_out_note(&o.left_out);
                    if !note.is_empty() {
                        text = format!("{}. {}", text, note);
                    }
                    (json!({"ok": true, "text": text}), ("info".to_string(), text))
                }
                Err(e) => (
                    json!({"ok": false, "error": e}),
                    ("warn".to_string(), format!("@{} can't land: {}", job.agent, e)),
                ),
            };
            write_json(&mut stream, &body);
            let _ = tx.send(Msg::Land { line: Some(line) });
            // the ± door of the land, right after its line (docs/artifacts.md)
            if let Some(f) = landed {
                let _ = tx.send(Msg::Land { line: Some(("landed".to_string(), f)) });
            }
        });
    }

    fn snapshot(&mut self) -> Value {
        // issue #4: each agent's model, for sb list / sb tasks too
        self.refresh_models();
        // the repo's flow, as config.toml says now (flow-prompts saves it)
        self.hub.flow = crate::flow::FlowConfig::load(&self.opts.paths).mode;
        // pr-news: whose bots' comments reach the agents (`[pr] trusted_bots`)
        self.hub.pr_bots =
            crate::forge::news::trusted_bots(&std::fs::read_to_string(self.opts.paths.config()).unwrap_or_default());
        // who lands, who waits in line (the boxes' lids)
        self.hub.lids = self.lands.lids();
        // the features' lids and Δ (dev-flow §5.1)
        self.feature_views();
        let mut snap = self.hub.snapshot(now_ms());
        // sb every's timers, running then a week of ended ones (/scheduled)
        snap["timers"] = json!(self.hub.timers().state(now_ms()));
        // one gate card for several agents' identical calls: it names them all
        self.gate_card_agents(&mut snap);
        let who: Vec<(String, String, bool)> =
            self.hub.st.agents.values().map(|a| (a.name.clone(), a.dir.clone(), a.is_main)).collect();
        if let Some(list) = snap["agents"].as_array_mut() {
            for v in list {
                let Some((_, dir, main)) = who.iter().find(|(n, _, _)| v["name"] == n.as_str()) else {
                    continue;
                };
                let u = self.in_use(dir, *main);
                v["model"] = json!(u.model.name);
                v["effort"] = json!(u.effort);
                v["efforts"] = json!(u.model.efforts());
                v["model_from"] = json!(u.model_from);
                // the "± 9 files so far" door (docs/artifacts.md)
                let name = v["name"].as_str().unwrap_or("").to_string();
                v["changes"] = self.art.changes.get(&name).cloned().unwrap_or(Value::Null);
            }
        }
        snap
    }

    /// `/model`, `/reasoning` for `agent`: check, write its choice (and
    /// config.toml for `default`), and say what it runs with now. A
    /// model with another context window: its REPL reloads at its next
    /// idle (same session), so the compaction threshold follows.
    fn choose(&mut self, agent: &str, model: Option<String>, effort: Option<String>, default: bool) -> String {
        let Some(a) = self.hub.st.agents.get(agent) else {
            return format!("no agent {}", agent);
        };
        let (dir, is_main) = (a.dir.clone(), a.is_main);
        let before = self.in_use(&dir, is_main);
        let short = |u: &bise_catalog::InUse| match u.effort.as_str() {
            "" => u.model.name.clone(),
            e => format!("{} · {}", u.model.name, e),
        };
        if model.is_none() && effort.is_none() {
            let words = before.model.efforts();
            let takes = if words.is_empty() {
                "no reasoning setting".to_string()
            } else {
                format!("efforts: {}", words.join(", "))
            };
            return format!(
                "{} runs {} (model from {}, {}) · /model <model>, /reasoning <effort>",
                agent,
                short(&before),
                before.model_from,
                takes
            );
        }
        let path = self.choice_path(&dir);
        let mut choice = bise_catalog::Choice::read(&path);
        if let Some(m) = &model {
            let setup = self.setup();
            let r = setup.catalog.resolve(m);
            if r.known == bise_catalog::Known::NoProvider {
                return format!(
                    "unknown provider for {}: pick a listed model, or add [providers.{}] to config.toml",
                    m, r.provider
                );
            }
            if !r.needs.is_empty() {
                return format!("{} is not usable yet (needs {})", r.name, r.needs);
            }
            choice.model = r.name.clone();
            // the user's pick: no longer the spawn's ask nor its fallback
            choice = bise_catalog::Choice { model: choice.model, effort: choice.effort, ..Default::default() };
        }
        if let Some(e) = &effort {
            let target = match &model {
                Some(_) => self.setup().catalog.resolve(&choice.model),
                None => before.model.clone(),
            };
            let words = target.efforts();
            if words.is_empty() {
                return format!("{} has no reasoning setting", target.name);
            }
            if !words.iter().any(|w| w == e) {
                return format!("{} takes: {}", target.name, words.join(", "));
            }
            choice.effort = e.clone();
        }
        if let Err(e) = choice.write(&path) {
            return format!("could not save the choice of {}: {}", agent, e);
        }
        let mut said = String::new();
        if default {
            // BISE-298: the role's line of [roles], its old key dropped
            let role = if is_main { bise_catalog::roles::MAIN } else { bise_catalog::roles::AGENTS };
            let cfg = bise_home::Home::from_env().config_file();
            let text = std::fs::read_to_string(&cfg).unwrap_or_default();
            let new = bise_catalog::roles::with_role(&text, role, &choice.model);
            let tmp = cfg.with_extension("toml.tmp");
            match std::fs::write(&tmp, new).and_then(|_| std::fs::rename(&tmp, &cfg)) {
                Ok(()) => said = format!(" · config.toml [roles] {} = {}", role, choice.model),
                Err(e) => said = format!(" · config.toml not written: {}", e),
            }
        }
        let after = self.in_use(&dir, is_main);
        if after.model.caps.context != before.model.caps.context && self.repls.contains_key(&dir) {
            // the compaction threshold is 80 % of the window: a reload
            // at the next idle takes the new one (nothing lost)
            self.reload_repls.insert(dir.clone());
        }
        log_line(&self.opts.paths, &format!("{}: now on {} (was {})", agent, short(&after), short(&before)));
        format!("✓ {} now on {} from its next call{}", agent, short(&after), said)
    }

    fn transcript(&self, dir: &str) -> PathBuf {
        self.opts.paths.agent_dir(dir).join("transcript.log")
    }

    /// A line enters a feed: memory, transcript, every client.
    fn feed(&mut self, name: &str, line: &str) {
        let Some(dir) = self.dir_of(name) else { return };
        let path = self.transcript(&dir);
        let pos = match self.positions.get(name) {
            Some(p) => p + 1,
            None => transcript_len(&path) + 1,
        };
        self.positions.insert(name.to_string(), pos);
        let ts = now_ms();
        let b = self.buffers.entry(name.to_string()).or_default();
        b.push_back((pos, ts, line.to_string()));
        while b.len() > BUFFER_LINES {
            b.pop_front();
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{}\t{}", ts, line);
        }
        self.broadcast(&line_event(name, pos, ts, line));
    }

    /// One event to every client (serialized once).
    fn broadcast(&mut self, v: &Value) {
        let line = v.to_string();
        let mut dead: Vec<ClientId> = Vec::new();
        for (id, s) in self.clients.iter_mut() {
            if !write_line(s, &line) {
                dead.push(*id);
            }
        }
        for id in dead {
            self.clients.remove(&id);
            let _ = self.tx.send(Msg::ClientGone { id });
        }
    }

    fn step(&mut self, input: Input) {
        let fx = self.hub.handle(input, &mut self.env);
        for e in fx {
            self.run(e);
        }
        // a gate card closed without an answer is a no
        self.gate_sweep();
        // a /drop: the dropped task's worktree folders (gate.sh's too)
        if !self.booting && self.hub.st.agents.values().any(|a| a.lifecycle == Lifecycle::Archived && !self.archived.contains(&a.dir)) {
            let now = self.archived_dirs();
            let names: BTreeSet<String> = self
                .hub
                .st
                .agents
                .values()
                .filter(|a| now.contains(&a.dir) && !self.archived.contains(&a.dir))
                .flat_map(|a| [a.name.clone(), a.dir.clone()].into_iter().chain(a.aliases.iter().cloned()))
                .collect();
            // their temp folders (approvals-design.md §7.1); run/ stays
            // with the rest of the agent's folder
            for d in now.difference(&self.archived) {
                crate::sweep::remove_agent_tmp(&self.opts.paths.agent_tmp(d));
            }
            self.archived = now;
            self.sweep_worktrees(Some(names));
        }
        // a stop or a /drop: the processes the agent started (BISE-243)
        if !self.booting {
            let now = self.down_dirs();
            let new: BTreeSet<String> = now.difference(&self.down).cloned().collect();
            self.down = now;
            if !new.is_empty() {
                self.reap_procs(Some(new));
            }
        }
    }

    fn archived_dirs(&self) -> BTreeSet<String> {
        self.hub.st.agents.values().filter(|a| a.lifecycle == Lifecycle::Archived).map(|a| a.dir.clone()).collect()
    }

    fn down_dirs(&self) -> BTreeSet<String> {
        self.hub
            .st
            .agents
            .values()
            .filter(|a| matches!(a.lifecycle, Lifecycle::Archived | Lifecycle::Stopped))
            .map(|a| a.dir.clone())
            .collect()
    }

    /// Kill, off the loop, the processes of the agents in `dirs` started
    /// until now (a REPL restored after this is not hit); `None`, at the
    /// start: those of every agent that is not live (stopped, archived,
    /// unknown), left by an earlier hub (BISE-243, `procs`).
    fn reap_procs(&self, dirs: Option<BTreeSet<String>>) {
        let hub = self.proc_hub.clone();
        let live: BTreeSet<String> = self
            .hub
            .st
            .agents
            .values()
            .filter(|a| !matches!(a.lifecycle, Lifecycle::Archived | Lifecycle::Stopped))
            .map(|a| a.dir.clone())
            .collect();
        let paths = self.opts.paths.clone();
        let before = crate::util::now_ms();
        // their REPLs' sessions until now, forgotten once reaped
        let reaped: Vec<String> = match &dirs {
            Some(d) => d.iter().cloned().collect(),
            None => self.hub.st.agents.values().filter(|a| !live.contains(&a.dir)).map(|a| a.dir.clone()).collect(),
        };
        let mut sessions = BTreeSet::new();
        for d in reaped {
            let f = paths.agent_dir(&d).join("repl.sids");
            sessions.extend(crate::procs::read_sids(&f));
            let _ = std::fs::remove_file(f);
        }
        std::thread::spawn(move || {
            let (want, what) = match &dirs {
                Some(d) => (
                    crate::procs::Want::Dirs { dirs: d, before },
                    d.iter().cloned().collect::<Vec<_>>().join(", "),
                ),
                None => (crate::procs::Want::NotLive(&live), "agents gone before this start".to_string()),
            };
            let hit = crate::procs::reap(&hub, &want, &sessions, Duration::from_secs(3));
            if !hit.is_empty() {
                log_line(&paths, &format!("processes of {} killed: {}", what, crate::procs::describe(&hit)));
            }
        });
    }

    /// Clean the task worktree folders (BISE-230, `sweep`) off the loop
    /// (a target is GBs of files): the dropped tasks' (`only`), or every
    /// orphan (the hub's start, the old place too). What is kept for its
    /// work goes to main's thread, for the user to decide.
    fn sweep_worktrees(&self, only: Option<BTreeSet<String>>) {
        let mut owners = crate::sweep::Owners::default();
        for a in self.hub.st.agents.values() {
            let names = [a.name.clone(), a.dir.clone()].into_iter().chain(a.aliases.iter().cloned());
            if a.lifecycle == Lifecycle::Archived {
                owners.archived.extend(names);
                continue;
            }
            owners.live.extend(names);
            owners.paths.insert(PathBuf::from(&a.ws.path));
            owners.paths.extend(a.place.iter().map(PathBuf::from));
        }
        let paths = self.opts.paths.clone();
        let prefix = self.env.config.branch_prefix.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let now = crate::util::now_ms();
            let mut out = crate::sweep::sweep(&paths.worktrees, &owners, only.as_ref(), now, &prefix);
            if only.is_none() {
                let legacy = paths.legacy_worktrees();
                if legacy != paths.worktrees {
                    out.extend(crate::sweep::sweep(&legacy, &owners, None, now, &prefix));
                    crate::sweep::drop_empty(&legacy);
                }
            }
            for o in out {
                match o {
                    crate::sweep::Outcome::Removed(d) => log_line(&paths, &format!("worktree folder removed: {}", d.display())),
                    crate::sweep::Outcome::Kept(d, why) => {
                        log_line(&paths, &format!("worktree folder kept: {}: {}", d.display(), why));
                        let _ = tx.send(Msg::Notice {
                            kind: "warn".into(),
                            text: format!(
                                "the worktree {} of task @{} is not deleted: {}. Delete it yourself when it is not needed any more",
                                d.display(),
                                crate::sweep::owner_of(&d),
                                why
                            ),
                        });
                    }
                }
            }
        });
    }

    fn run(&mut self, e: Effect) {
        match e {
            Effect::Journal(ev) => {
                let _ = writeln!(self.journal, "{}", ev);
                let _ = self.journal.flush();
                // /drop (design §7.3): its tab group closes too
                if let Some(name) = crate::computer_use::archived(&ev) {
                    crate::computer_use::drop_agent(name);
                }
            }
            Effect::Spawn {
                agent,
                resume,
                crash_note,
            } => self.spawn_on(&agent, resume, crash_note, None),
            Effect::Kill { agent } => {
                if let Some(dir) = self.dir_of(&agent) {
                    self.gens.remove(&dir);
                    if let Some(r) = self.repls.remove(&dir) {
                        let _ = r.stream.shutdown(std::net::Shutdown::Both);
                    }
                    if let Some((_, pid)) = self.pids.remove(&dir) {
                        kill_pid(pid);
                    }
                }
            }
            Effect::Say { agent, text } => {
                self.skills_before_turn(&agent);
                let line = format!("say {}\n", wire_escape(&text));
                if !self.repl_write(&agent, &line) {
                    log_line(
                        &self.opts.paths,
                        &format!("say to {} failed: not connected", agent),
                    );
                }
            }
            Effect::Steer { agent, text } => {
                if let Some(r) = self.dir_of(&agent).and_then(|d| self.repls.get(&d)) {
                    let ok = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&r.steer)
                        .and_then(|mut f| {
                            f.write_all(format!("{}\n", wire_escape(&text)).as_bytes())
                        });
                    if ok.is_err() {
                        log_line(&self.opts.paths, &format!("steer to {} failed", agent));
                    }
                }
            }
            Effect::Passthrough { agent, line } => {
                self.repl_write(&agent, &format!("{}\n", line));
            }
            Effect::Interrupt { agent, by } => {
                // the flag's content names the asker (the runtime's
                // "interrupted by main", provider-pure.bend interrupter;
                // any content is a stop request)
                if let Some(r) = self.dir_of(&agent).and_then(|d| self.repls.get(&d)) {
                    let _ = std::fs::write(&r.interrupt, &by);
                }
            }
            Effect::Context { agent, text } => {
                if let Some(dir) = self.dir_of(&agent) {
                    let d = self.opts.paths.agent_dir(&dir);
                    let _ = std::fs::create_dir_all(&d);
                    let tmp = d.join("context.txt.tmp");
                    if std::fs::write(&tmp, &text).is_ok() {
                        let _ = std::fs::rename(&tmp, d.join("context.txt"));
                    }
                }
            }
            Effect::Line { agent, line } => self.feed(&agent, &line),
            Effect::Reply { token, body } => {
                if let Some(mut s) = self.replies.remove(&token) {
                    write_json(&mut s, &body);
                }
            }
            Effect::Land { token, job } => self.land(token, *job),
            Effect::ToClient { client, body } => {
                if let Some(s) = self.clients.get_mut(&client) {
                    write_json(s, &body);
                }
            }
            Effect::Renamed { old, new } => {
                if let Some(b) = self.buffers.remove(&old) {
                    self.buffers.insert(new.clone(), b);
                }
                if let Some(p) = self.positions.remove(&old) {
                    self.positions.insert(new.clone(), p);
                }
                self.broadcast(&json!({"ev": "renamed", "old": old, "new": new}));
            }
            Effect::State => {
                let snap = self.snapshot();
                self.broadcast(&snap);
            }
            Effect::AskRole { dir, key, request } => self.ask_role(dir, key, request),
            Effect::Confirm { card, agent: _, text } => self.on_confirm(card, &text),
            Effect::Flow { client, token, set } => {
                let (ok, text) = self.flow_cmd(set);
                if let Some(s) = client.and_then(|c| self.clients.get_mut(&c)) {
                    write_json(s, &json!({"ev": "notice", "text": text}));
                }
                if let Some(mut s) = token.and_then(|t| self.replies.remove(&t)) {
                    let body = if ok {
                        json!({"ok": true, "text": text})
                    } else {
                        json!({"ok": false, "error": text})
                    };
                    write_json(&mut s, &body);
                }
                if ok && set.is_some() {
                    let snap = self.snapshot();
                    self.broadcast(&snap);
                }
            }
            Effect::Choose { client, agent, model, effort, default } => {
                let text = self.choose(&agent, model, effort, default);
                if let Some(s) = self.clients.get_mut(&client) {
                    write_json(s, &json!({"ev": "notice", "text": text}));
                }
                let snap = self.snapshot();
                self.broadcast(&snap);
                self.switch_idle_repls();
            }
            Effect::SpawnModel { token, agent, ask, mut body } => {
                body["model"] = json!(self.spawn_model(&agent, &ask));
                if let Some(mut s) = self.replies.remove(&token) {
                    write_json(&mut s, &body);
                }
                let snap = self.snapshot();
                self.broadcast(&snap);
            }
            Effect::Switch { token, from, to, model, effort } => {
                let res = self.switch_model(&from, &to, &model, &effort);
                if let Some(mut s) = token.and_then(|t| self.replies.remove(&t)) {
                    let body = match &res {
                        Ok(text) => json!({"ok": true, "cmd": "switch", "text": text}),
                        Err(e) => json!({"ok": false, "error": e}),
                    };
                    write_json(&mut s, &body);
                }
                let snap = self.snapshot();
                self.broadcast(&snap);
            }
            Effect::ModelRefused { agent, why } => self.model_refused(&agent, &why),
            Effect::Pr(e) => log_line(&self.opts.paths, &crate::forge::log_line(&e)),
            Effect::Merge { card, place, number, head, method } => self.merge_pr(card, place, number, head, method),
            Effect::Feature { token, op, name, agents } => self.feature(token, op, name, agents),
            Effect::Update { card, id, version } => self.update_to(card, id, version),
            Effect::UpdateLater { id } => versions::update_later(&id),
        }
    }

    /// pr-design §6.3: the user's `1` on a ready-to-merge item: `gh pr
    /// merge` with the user's login, on a thread (the network); the
    /// answer comes back as `Input::Merged`. The repo and gh are found
    /// as the poller finds them.
    fn merge_pr(&self, card: u64, place: String, number: u64, head: String, method: crate::place::MergeMethod) {
        let ws = self.opts.paths.workspace.clone();
        let bin = self.opts.paths.bin_dir();
        let log_paths = self.opts.paths.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            use crate::forge::Forge;
            let gh = crate::forge::github::GitHub::find(&crate::tools_env::hub_agent_path(&bin))
                .unwrap_or(crate::forge::github::GitHub { gh: "gh".into() });
            let res = crate::worktree::git(&ws, &["remote", "get-url", "origin"])
                .ok()
                .and_then(|url| crate::forge::repo_of_url(&url))
                .ok_or_else(|| "origin is not a GitHub repo".to_string())
                .and_then(|repo| gh.merge(&repo, number, &head, method).map_err(|e| e.describe()));
            log_line(
                &log_paths,
                &format!("PR #{} merge ({:?}): {}", number, method, res.as_ref().map_or_else(|e| e.as_str(), |_| "merged")),
            );
            let _ = tx.send(Msg::In(Input::Merged { card, place, number, res }));
        });
    }

    /// The PR poller (pr-design §7): started once, off the loop (it
    /// finds gh and the repo's forge on its thread); no forge, no asks.
    fn start_prs(&mut self) {
        let ws = self.opts.paths.workspace.clone();
        let bin = self.opts.paths.bin_dir();
        let log_paths = self.opts.paths.clone();
        let tx = self.tx.clone();
        let setup = Box::new(move || {
            let gh = crate::forge::github::GitHub::find(&crate::tools_env::hub_agent_path(&bin));
            let url = crate::worktree::git(&ws, &["remote", "get-url", "origin"]).ok()?;
            let Some(repo) = crate::forge::github::detect(&url, gh.as_ref()) else {
                log_line(&log_paths, "PRs: origin is not on GitHub, not followed");
                return None;
            };
            log_line(
                &log_paths,
                &format!("PRs: following {}/{} on {}{}", repo.owner, repo.name, repo.host, if gh.is_none() { " (gh not found yet)" } else { "" }),
            );
            let forge: Box<dyn crate::forge::Forge> = match gh {
                Some(g) => Box::new(g),
                None => Box::new(crate::forge::github::GitHub { gh: "gh".into() }),
            };
            Some(crate::forge::poll::Watcher::new(repo, forge, Box::new(crate::forge::poll::RepoGit { workspace: ws })))
        });
        let sink = Box::new(move |r| {
            let _ = tx.send(Msg::In(Input::Prs(r)));
        });
        self.prs = Some(crate::forge::poll::Poller::start(setup, sink));
    }

    /// Tell the poller the branches to follow (it sends nothing when
    /// they did not change).
    fn plan_prs(&mut self) {
        let plan = crate::forge::poll::Plan { watches: self.hub.pr_watches(), clients: self.hub.has_clients() };
        if let Some(p) = self.prs.as_mut() {
            p.plan(plan);
        }
    }

    /// The repo's flow now (dev-flow §2): `[flow]` of the config, and
    /// the last detection (`detect_flow`'s cache). None: neither yet.
    fn flow_now(&self) -> Option<devflow::Flow> {
        let cfg = crate::flow::FlowConfig::load(&self.opts.paths);
        devflow::resolve(&cfg, devflow::read_cache(&self.opts.paths.state).as_ref())
    }

    /// dev-flow §2: detect the repo's flow off the hub's loop (git, and
    /// gh for a GitHub remote: the network), cached next to the state
    /// for the prompts and `/flow`. At the boot, and again on `/flow`.
    fn detect_flow(&self) {
        let (repo, state) = (self.opts.paths.workspace.clone(), self.opts.paths.state.clone());
        std::thread::spawn(move || {
            let d = devflow::detect(&devflow::GitProbe { repo });
            devflow::write_cache(&state, &d);
        });
    }

    /// The commit message style of the repo, from its last 50 subjects.
    fn commit_style(&self) -> Option<String> {
        let out = Command::new("git")
            .args(["log", "-50", "--format=%s"])
            .current_dir(&self.opts.paths.workspace)
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        devflow::commit_style(&text.lines().collect::<Vec<_>>())
    }

    /// `/flow`, `sb flow` (dev-flow §2, §7): show the flow and why, or
    /// save a switch (refused to trunk while the branch is protected).
    /// (ok, the text).
    fn flow_cmd(&mut self, set: Option<crate::flow::FlowMode>) -> (bool, String) {
        let now = self.flow_now();
        let Some(to) = set else {
            // shown from the cache; a fresh detection for the next time
            self.detect_flow();
            let mut text = devflow::show(now.as_ref());
            // dev-flow §7: the open feature branches, under the flow
            let hub = &self.hub;
            let features = self.features.flow_line(&|f| hub.feature_agents(f).len());
            if !features.is_empty() {
                text.push_str(&format!("\u{a}{}", features));
            }
            if let Some(f) = now.as_ref().filter(|f| f.source == devflow::Source::Suggested) {
                text.push_str(&format!(
                    "\nnot saved, and it holds no work: ask the user once in a card that blocks nothing (`sb card`), then save the answer with `sb flow pr|trunk`. the question:\n{}",
                    devflow::question(f)
                ));
            }
            return (true, text);
        };
        match devflow::switch(now.as_ref(), to) {
            Err(e) => (false, e),
            Ok(said) => match crate::flow::save_mode(&self.opts.paths, to) {
                Ok(()) => {
                    log_line(&self.opts.paths, &format!("flow saved: {}", to.as_str()));
                    self.hub.flow = Some(to);
                    (true, said)
                }
                Err(e) => (false, format!("flow not saved: {e}")),
            },
        }
    }

    /// The role of `a` (its system prompt's end), with the repo's flow
    /// (dev-flow §6).
    fn role_of(&self, a: &Agent, tmp: &str) -> String {
        let flow = self.flow_now();
        let style = self.commit_style();
        if a.is_main {
            let section = devflow::main_section(flow.as_ref(), style.as_deref());
            return prompts::main_role(&self.hub.workspace, tmp, &section);
        }
        // a feature's agent: the feature's other agents (dev-flow §5.1)
        let id = crate::place::view_id(a);
        let others: Vec<String> = crate::place::places(&self.hub.st, &self.hub.prs)
            .into_iter()
            .find(|p| p.id == id)
            .map(|p| p.agents.into_iter().filter(|n| *n != a.name).collect())
            .unwrap_or_default();
        let branch = a.ws.branch.clone().unwrap_or_default();
        let place = devflow::TaskPlace {
            path: &a.ws.path,
            branch: (a.ws.mode == crate::model::Mode::Worktree).then_some(branch.as_str()),
            others: &others,
            feature: a.ws.feature(),
        };
        prompts::task_role(a, tmp, &devflow::task_place(flow.as_ref(), &place, style.as_deref()))
    }

    /// One role-line call in a thread (BISE-126): never waits in the
    /// hub's loop nor in the task's turn; the answer comes back as
    /// `Msg::RoleLine`.
    fn ask_role(&mut self, dir: String, key: String, request: String) {
        let adir = self.opts.paths.agent_dir(&dir);
        let req_file = adir.join("role-request.txt");
        if std::fs::create_dir_all(&adir).and_then(|_| std::fs::write(&req_file, request)).is_err() {
            let _ = self.tx.send(Msg::RoleLine { dir, key, line: None });
            return;
        }
        let setup = bise_catalog::Setup::load(&bise_home::Home::from_env().config_file());
        let broken = self.small_broken.clone();
        let (repl, root) = (self.opts.repl_bin.clone(), self.opts.app_root.clone());
        let (tx, paths, spawn_env) = (self.tx.clone(), self.opts.paths.clone(), self.opts.spawn_env);
        std::thread::spawn(move || {
            use std::sync::atomic::Ordering;
            let keys = spawn_env.map(|f| f()).unwrap_or_default();
            let small = setup.small_model.clone();
            let agent = setup.agent_model.clone();
            let first = if broken.load(Ordering::Relaxed) { agent.clone() } else { small.clone() };
            let mut got = oneshot(&repl, &root, &req_file, &first, &keys);
            if let Err(e) = &got {
                log_line(&paths, &format!("role line of {}: {} failed: {}", dir, first, e));
                if first != agent {
                    got = oneshot(&repl, &root, &req_file, &agent, &keys);
                    match &got {
                        Ok(_) => {
                            broken.store(true, Ordering::Relaxed);
                            log_line(&paths, &format!("role lines: {} failed, {} from now on", small, agent));
                        }
                        Err(e) => log_line(&paths, &format!("role line of {}: {} failed: {}", dir, agent, e)),
                    }
                }
            }
            let _ = std::fs::remove_file(&req_file);
            let line = got.ok().and_then(|t| crate::role::clean(&t));
            if let Some(l) = &line {
                write_role(&paths, &paths.agent_dir(&dir), l, &key);
            }
            let _ = tx.send(Msg::RoleLine { dir, key, line });
        });
    }

    fn repl_write(&mut self, agent: &str, line: &str) -> bool {
        let Some(dir) = self.dir_of(agent) else {
            return false;
        };
        if let Some(q) = self.switching.get_mut(&dir) {
            q.push(line.to_string());
            return true;
        }
        match self.repls.get_mut(&dir) {
            Some(r) => r.stream.write_all(line.as_bytes()).is_ok(),
            None => false,
        }
    }

    fn same_bin(a: &Path, b: &Path) -> bool {
        let c = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        c(a) == c(b)
    }

    /// The keys changed since a live REPL was spawned (a key saved in
    /// auth.json, BISE-266): it relaunches at its next idle on the same
    /// session and port, like a reload, and gets them; the writes meant
    /// for it wait meanwhile. Busy ones wait for the end of their turn.
    fn keys_changed(&mut self) {
        let Some(f) = self.opts.spawn_env else { return };
        let now = hash_keys(&f());
        let stale: Vec<String> = self
            .spawn_keys
            .iter()
            .filter(|(d, h)| **h != now && self.repls.contains_key(*d) && !self.switching.contains_key(*d))
            .map(|(d, _)| d.clone())
            .collect();
        if stale.is_empty() {
            return;
        }
        for d in stale {
            log_line(&self.opts.paths, &format!("keys changed: the REPL of {} relaunches at its next idle", d));
            self.reload_repls.insert(d);
        }
        self.switch_idle_repls();
    }

    /// The plugins or skills of a live REPL changed since it was spawned
    /// (design: docs/plugins.md "Reload on change"): it relaunches at its
    /// next idle, same session and port, like a key change; a busy one
    /// finishes its turn first. On the tick (`now` false, at most every
    /// 2 s) only the plugins are compared; when an agent goes idle (`now`)
    /// the skill folders too (no timer for them: they are checked again
    /// right before a turn, `skills_before_turn`).
    fn plugins_changed(&mut self, now: bool) {
        if !now && self.plugins_checked.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2)) {
            return;
        }
        self.plugins_checked = Some(std::time::Instant::now());
        let mut plugins_by_ws: BTreeMap<PathBuf, u64> = BTreeMap::new();
        let mut skills_by_ws: BTreeMap<(PathBuf, bool), u64> = BTreeMap::new();
        let mut stale = Vec::new();
        for (d, fp) in &self.spawn_plugins {
            if !self.repls.contains_key(d) || self.switching.contains_key(d) || self.reload_repls.contains(d) {
                continue;
            }
            let plugins = *plugins_by_ws.entry(fp.ws.clone()).or_insert_with(|| plugins_fingerprint(&fp.ws));
            let skills_moved = now && {
                let cur = *skills_by_ws
                    .entry((fp.ws.clone(), fp.main))
                    .or_insert_with(|| skills_fingerprint(&skill_roots(&fp.ws, &self.opts.app_root, fp.main)));
                cur != fp.skills
            };
            if plugins != fp.plugins || skills_moved {
                stale.push(d.clone());
            }
        }
        for d in stale {
            log_line(
                &self.opts.paths,
                &format!("plugins or skills changed: the REPL of {} relaunches at its next idle", d),
            );
            self.reload_repls.insert(d);
        }
        if !self.reload_repls.is_empty() {
            self.switch_idle_repls();
        }
    }

    /// A turn is about to start on `agent`'s idle REPL (a `say`): when a
    /// skill folder its prompt reads moved since its spawn (a SKILL.md
    /// added, edited or removed while it sat idle), it relaunches first,
    /// same session, and the `say` waits in the switch queue for the new
    /// REPL, so this very turn has the fresh skills. Stats only, once per
    /// turn: no timer.
    fn skills_before_turn(&mut self, agent: &str) {
        let Some(dir) = self.dir_of(agent) else {
            return;
        };
        if self.switching.contains_key(&dir) {
            return;
        }
        let Some(fp) = self.spawn_plugins.get(&dir) else {
            return;
        };
        let cur = skills_fingerprint(&skill_roots(&fp.ws, &self.opts.app_root, fp.main));
        if cur == fp.skills && !self.reload_repls.contains(&dir) {
            return;
        }
        let Some(r) = self.repls.get_mut(&dir) else {
            return;
        };
        if r.stream.write_all(b"reload\n").is_ok() {
            self.reload_repls.remove(&dir);
            log_line(
                &self.opts.paths,
                &format!("plugins or skills changed: the REPL of {} relaunches before its turn", dir),
            );
            self.switching.insert(dir, Vec::new());
        }
    }

    /// Every idle REPL still on another version's binary, or adopted by
    /// a reload (BISE-131), is asked to reload (a turn boundary: it
    /// checkpoints and exits); the hub then restarts it on its own
    /// binary, same port, same session. Busy ones wait for the end of
    /// their turn: an agent never loses a turn.
    fn switch_idle_repls(&mut self) {
        let stale: Vec<(String, String)> = self
            .hub
            .st
            .agents
            .values()
            .filter(|a| a.run == crate::model::Run::Idle)
            .filter(|a| !self.switching.contains_key(&a.dir) && self.repls.contains_key(&a.dir))
            .filter(|a| {
                self.reload_repls.contains(&a.dir)
                    || self
                        .bins
                        .get(&a.dir)
                        .is_some_and(|b| !Self::same_bin(b, &self.opts.repl_bin))
            })
            .map(|a| (a.name.clone(), a.dir.clone()))
            .collect();
        for (name, dir) in stale {
            let Some(r) = self.repls.get_mut(&dir) else {
                continue;
            };
            if r.stream.write_all(b"reload\n").is_ok() {
                self.reload_repls.remove(&dir);
                log_line(
                    &self.opts.paths,
                    &format!(
                        "switching the REPL of {} to {}",
                        name,
                        self.opts.repl_bin.display()
                    ),
                );
                self.switching.insert(dir, Vec::new());
            }
        }
    }

    /// What keeps a hub with no UI up (idle-exit rule 2): an agent
    /// mid-turn (or waiting in `sb wait`), a REPL starting or switching,
    /// a background job of an agent, a build of the hub's own.
    fn idle_busy(&self) -> Vec<String> {
        let mut v = Vec::new();
        for name in &self.hub.st.order {
            let Some(a) = self.hub.st.agents.get(name) else { continue };
            if a.lifecycle != Lifecycle::Active {
                continue;
            }
            if a.run == crate::model::Run::Busy {
                v.push(format!("{} mid-turn", a.name));
            }
            let jobs = crate::idle::bg_jobs(&self.opts.paths.agent_tmp(&a.dir).join("bg"), crate::procs::alive);
            if !jobs.is_empty() {
                v.push(format!("{}'s background job {}", a.name, jobs.join(" ")));
            }
        }
        if !self.starts.is_empty() {
            v.push("a REPL starting".into());
        }
        if !self.switching.is_empty() {
            v.push("a REPL switching".into());
        }
        if !self.building.is_empty() {
            v.push("a version build".into());
        }
        if self.updating.is_some() {
            v.push("the /update build".into());
        }
        if self.release.is_some() {
            v.push("/release-bise".into());
        }
        v
    }

    /// idle-exit, at each tick: no UI for the grace and nothing runs,
    /// the hub stops for good (the stop of `bise --stop`).
    fn idle_check(&mut self) {
        let uis = self.clients.len() + self.holds.count();
        // the look reads self: the watch is taken out for it
        let mut w = std::mem::replace(&mut self.idle, crate::idle::Watch::new(None, std::time::Instant::now()));
        let step = w.step(std::time::Instant::now(), uis, || self.idle_busy());
        self.idle = w;
        match step {
            crate::idle::Step::Stay => {}
            crate::idle::Step::Say(s) => log_line(&self.opts.paths, &s),
            crate::idle::Step::Stop(s) => {
                log_line(&self.opts.paths, &s);
                let _ = self.tx.send(Msg::Shutdown { keep: false });
            }
        }
    }

    /// Start the REPL of `name` on a supervisor thread. `port`: the port of the process it replaces (a switch keeps the
    /// port: background commands and steer files are keyed by it).
    fn spawn_on(
        &mut self,
        name: &str,
        resume: bool,
        crash_note: Option<String>,
        port: Option<u16>,
    ) {
        let Some(a) = self.hub.st.agents.get(name).cloned() else {
            return;
        };
        let dir = a.dir.clone();
        let adir = self.opts.paths.agent_dir(&dir);
        let _ = std::fs::create_dir_all(&adir);
        // its temp folder and the harness's own files (approvals-design.md
        // §7.1): nothing in /tmp
        let (tmp, run) = (self.opts.paths.agent_tmp(&dir), self.opts.paths.agent_run(&dir));
        if let Err(e) = crate::tools_env::make_agent_dirs(&tmp, &run) {
            log_line(&self.opts.paths, &format!("{}: cannot create {}: {}", a.name, tmp.display(), e));
        }
        // the approvals mode, read by the runtime before each gated call
        self.write_mode_file(&dir);
        let tmp_s = tmp.to_string_lossy().into_owned();
        let role = self.role_of(&a, &tmp_s);
        write_logged(&self.opts.paths, &adir.join("role.md"), &role);
        if !adir.join("context.txt").exists() {
            let _ = std::fs::write(adir.join("context.txt"), "");
        }
        let gen = self.next_gen;
        self.next_gen += 1;
        self.gens.insert(dir.clone(), gen);
        if self.booting && resume {
            if let Some(r) = adoptable(&adir) {
                log_line(
                    &self.opts.paths,
                    &format!(
                        "adopting the REPL of {} (pid {}, port {})",
                        a.name, r.pid, r.port
                    ),
                );
                self.pids.insert(dir.clone(), (gen, r.pid));
                if !self.reload_id.is_empty() {
                    self.reload_repls.insert(dir.clone());
                }
                self.bins.insert(dir.clone(), r.bin.clone());
                self.ports.insert(dir.clone(), r.port);
                self.attach_session(&a, &dir, &adir);
                let tx = self.tx.clone();
                let paths = self.opts.paths.clone();
                std::thread::spawn(move || adopt(r, dir, gen, adir, tx, paths));
                return;
            }
            // not adoptable: dead. Was it in the middle of a turn?
            let wire = adir.join("wire.log");
            let len = std::fs::metadata(&wire).map(|m| m.len()).unwrap_or(0);
            if len > 0 && busy_at(&wire, len) {
                log_line(
                    &self.opts.paths,
                    &format!("{}: its REPL died mid-turn, the turn resumes", a.name),
                );
                self.resume_turn.insert(dir.clone());
            }
        }
        // a fresh process: a fresh wire log
        write_logged(&self.opts.paths, &adir.join("wire.log"), "");
        write_logged(&self.opts.paths, &adir.join("wire.offset"), "0");
        let _ = std::fs::remove_file(adir.join("repl.json"));
        let port = match port.map(Ok).unwrap_or_else(free_port) {
            Ok(p) => p,
            Err(e) => {
                let _ = self.tx.send(Msg::ReplGone {
                    dir,
                    gen,
                    reason: format!("no free port: {}", e),
                });
                return;
            }
        };
        let (session, cont) = self.prepare_session(&a, &dir, &adir, resume);
        let mut cmd = Command::new(&self.opts.repl_bin);
        cmd.current_dir(&self.opts.app_root);
        // its whole environment: the hub's minus every internal and test
        // variable (bise_home::env), plus what names this agent
        let mut env = bise_home::env::for_child(
            bise_home::env::Child::Repl,
            [
                ("BEND_REPL_PORT", std::ffi::OsString::from(port.to_string())),
                ("BEND_SESSION_FILE", session.clone().into()),
                ("BEND_EXTRA_PROMPT", adir.join("role.md").into()),
                ("BEND_CONTEXT_FILE", adir.join("context.txt").into()),
                ("BEND_WORKDIR", a.ws.path.clone().into()),
                // the REPL starts its plugins bridge with this binary
                // (`bise plugins serve`, docs/plugins.md)
                ("BEND_HARNESS_BIN", self.opts.exe.clone().into()),
                ("SB_SOCKET", self.opts.paths.socket().into()),
                ("BEND_WIRE_LOG", adir.join("wire.log").into()),
                ("SB_AGENT", a.name.clone().into()),
                // which model: config.toml `model`, or `agent_model` (BISE-142)
                ("BISE_ROLE", (if a.is_main { "main" } else { "agent" }).into()),
                // its own model and effort, over both (BISE-135: /model)
                (bise_catalog::CHOICE_ENV, adir.join("choice.toml").into()),
                // RFC 0002 §9: two dev servers must not fight for one port
                ("SB_TASK", a.name.clone().into()),
                // what it starts is its: killed at its stop or /drop (BISE-243)
                (
                    crate::procs::ENV,
                    crate::procs::for_repl(
                        std::env::var(crate::procs::ENV).ok().as_deref(),
                        &self.proc_hub,
                        &dir,
                        crate::util::now_ms(),
                    )
                    .into(),
                ),
                (
                    "SB_PORT_OFFSET",
                    self.hub
                        .st
                        .order
                        .iter()
                        .position(|n| *n == a.name)
                        .unwrap_or(0)
                        .to_string()
                        .into(),
                ),
            ],
        );
        if let Some(j) = &self.opts.jsrt_bin {
            env.set("BEND_JSRT_BIN", j);
        }
        // dev: the exact body of every model request, one file each in
        // agents/<a>/requests/ (the TUI's /log shows them), when the hub
        // runs with BISE_DEBUG_REQUESTS=1 or <hub>/debug-requests exists
        // (read at each REPL start: no hub restart). Bodies only, no
        // headers: the keys go in headers.
        if debug_requests(&self.opts.paths.state) {
            let req_dir = adir.join("requests");
            if std::fs::create_dir_all(&req_dir).is_ok() {
                env.set("BEND_WIRE_DUMP", format!("{}/", req_dir.display()));
            }
        }
        for (k, v) in crate::tools_env::temp_env(&tmp, &run) {
            env.set(k, v);
        }
        let keys = self.opts.spawn_env.map(|f| f()).unwrap_or_default();
        self.spawn_keys.insert(dir.clone(), hash_keys(&keys));
        let ws = PathBuf::from(&a.ws.path);
        let spawned = PromptInputs {
            plugins: plugins_fingerprint(&ws),
            skills: skills_fingerprint(&skill_roots(&ws, &self.opts.app_root, a.is_main)),
            ws,
            main: a.is_main,
        };
        let fp = spawned.combined();
        self.spawn_plugins.insert(dir.clone(), spawned);
        for (k, v) in keys {
            match v {
                Some(v) => env.set(k, v),
                None => env.unset(k),
            };
        }
        // its own session: what it starts is found by session id too,
        // even a macOS binary whose environment is hidden (BISE-243)
        crate::procs::own_session(&mut cmd);
        if resume && cont && session.exists() {
            env.set("BEND_CONTINUE", "1");
            // its plugins changed since its prompt was built: the restored
            // session takes this start's prompt (runtime/persist.bend
            // with_cfg), else it never learns of a new plugin
            if prompt_is_stale(&adir, fp) {
                env.set("BEND_FRESH_PROMPT", "1");
            }
        }
        let _ = std::fs::write(adir.join(PROMPT_PLUGINS_FILE), fp.to_string());
        if let Some(n) = crash_note {
            env.set("BEND_CRASH_NOTE", n);
        }
        self.bins.insert(dir.clone(), self.opts.repl_bin.clone());
        self.ports.insert(dir.clone(), port);
        self.starts.insert(dir.clone(), (gen, std::time::Instant::now()));
        let tx = self.tx.clone();
        let paths = self.opts.paths.clone();
        let workdir = std::path::PathBuf::from(&a.ws.path);
        std::thread::spawn(move || {
            stall_for_tests();
            // sb, the hub's PATH, the user's login-shell PATH, the
            // standard dirs; the model is told once whether rg and git
            // are there (BISE-166), then which plugins it has and what
            // for. Here, off the hub's loop: the first spawn may wait for
            // the login shell (read once, at most 3 s)
            let agent_path = crate::tools_env::hub_agent_path(&paths.bin_dir());
            env.set("BEND_TOOLS_NOTE", crate::tools_env::session_note(&agent_path, &workdir))
                .set("PATH", agent_path);
            // the AGENTS.md files of its working folder (a task: its
            // worktree's), read again at each start and /reload (BISE-232)
            env.set(
                crate::agents_md::ENV,
                crate::agents_md::write_for(
                    &workdir,
                    &bise_home::Home::from_env(),
                    &adir.join("agents-md.md"),
                ),
            );
            env.apply(&mut cmd);
            supervise(cmd, dir, gen, adir, port, tx, paths)
        });
    }

    /// A REPL whose process is still not spawned START_LIMIT after it
    /// was asked for: its start failed. It goes as a crash (a line in
    /// main's feed, a restart, a card after MAX_CRASHES); what its stuck
    /// thread sends later belongs to an old generation and is dropped.
    fn check_starts(&mut self) {
        let late: Vec<(String, u64)> = self
            .starts
            .iter()
            .filter(|(_, (_, since))| since.elapsed() > START_LIMIT)
            .map(|(dir, (gen, _))| (dir.clone(), *gen))
            .collect();
        for (dir, gen) in late {
            self.starts.remove(&dir);
            let reason = format!(
                "its REPL did not start in {} s: the hub never got to run it; `bise doctor` shows where the hub's log is",
                START_LIMIT.as_secs()
            );
            log_line(&self.opts.paths, &format!("repl {} not started: {}", dir, reason));
            let _ = self.tx.send(Msg::ReplGone { dir, gen, reason });
        }
    }

    fn on_repl_line(&mut self, dir: &str, line: &str) {
        let Some(name) = self.agent_by_dir(dir).map(|a| a.name.clone()) else {
            return;
        };
        if self.switching.contains_key(dir) {
            // the reload acknowledgement of a switch: not the agent's news
            return;
        }
        if self.restored.contains(dir) {
            if line.contains("obs: session_restored") {
                self.restored.remove(dir);
            }
            if line.starts_with("history ") || line.contains("obs: session_restored") {
                return;
            }
            self.restored.remove(dir);
        }
        // the approvals gate (spec §3): the hub's, not the feed's
        if let Some(rest) = line.strip_prefix("gate ") {
            return self.on_gate(dir, &name, rest);
        }
        if let Some(rest) = line.strip_prefix("gate-done ") {
            return self.on_gate_done(dir, &name, rest);
        }
        if line.starts_with("history ") && self.buffers.get(&name).is_some_and(|b| !b.is_empty()) {
            // a restored session replays its history: the feed has it
            return;
        }
        self.feed(&name, line);
        self.art_on_line(&name, line);
        if line == "--- idle" {
            let leftover = self
                .repls
                .get(dir)
                .map(|r| {
                    let c = std::fs::read_to_string(&r.steer).unwrap_or_default();
                    if !c.trim().is_empty() {
                        let _ = std::fs::write(&r.steer, "");
                    }
                    !c.trim().is_empty()
                })
                .unwrap_or(false);
            self.step(Input::ReplIdle {
                agent: name,
                leftover,
            });
            self.keys_changed();
            self.plugins_changed(true);
        } else {
            self.step(Input::ReplLine {
                agent: name,
                line: line.to_string(),
            });
        }
    }

    /// A new client: hello, snapshot, the buffered lines of every feed,
    /// `ready`, the versions, in one write (thousands of lines: one
    /// syscall, not one per line).
    fn client_hello(&mut self, id: ClientId, mut stream: UnixStream) {
        let art_ev = self.artifacts_ev();
        let mut out = String::new();
        let mut push = |v: &Value| {
            out.push_str(&v.to_string());
            out.push('\n');
        };
        push(&json!({
            "ev": "hello",
            "workspace": self.hub.workspace,
            "state_dir": self.opts.paths.state.to_string_lossy(),
            "exe": self.opts.exe.to_string_lossy(),
            "version": crate::switch::version_info(&self.opts.app_root),
            "reload": self.reload_id,
        }));
        push(&self.snapshot());
        for name in &self.hub.st.order {
            for (pos, ts, l) in self.buffers.get(name).into_iter().flatten() {
                push(&line_event(name, *pos, *ts, l));
            }
        }
        push(&self.approvals_ev(false));
        push(&json!({"ev": "ready"}));
        push(&art_ev);
        push(&self.version_items());
        if let Some(r) = self.release_hello() {
            push(&r);
        }
        if let Some(u) = self.update_hello() {
            push(&u);
        }
        crate::util::timing(&format!("client hello built ({} bytes)", out.len()));
        if stream.write_all(out.as_bytes()).is_err() {
            return;
        }
        crate::util::timing("client hello written");
        self.clients.insert(id, stream);
        self.step(Input::ClientHello { client: id });
    }

    fn client_line(&mut self, id: ClientId, v: Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match s("op").as_str() {
            "version" if s("do") == "items" => {
                let items = self.version_items();
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &items);
                }
            }
            "version" if s("do") == "update" => self.update_op(id),
            "version" => {
                let text = self.version_op(&v);
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &json!({"ev": "notice", "text": text}));
                }
            }
            "input" => {
                // BISE-266: a key saved since a REPL started reaches it
                // before this message does
                self.keys_changed();
                self.step(Input::ClientInput {
                    client: id,
                    focus: s("focus"),
                    text: s("text"),
                })
            }
            // older lines of a feed, before a position (the TUI scrolled
            // to the top of what it holds)
            "history" => {
                let agent = s("agent");
                let before = v.get("before").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let count = v
                    .get("count")
                    .and_then(|x| x.as_u64())
                    .map_or(PAGE_LINES, |c| (c as usize).min(PAGE_LINES));
                let lines: Vec<Value> = match self.dir_of(&agent) {
                    Some(dir) => transcript_page(&self.transcript(&dir), before, count)
                        .into_iter()
                        .map(|(pos, ts, line)| history_line(pos, ts, &line))
                        .collect(),
                    None => Vec::new(),
                };
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(
                        c,
                        &json!({"ev": "history", "agent": agent, "before": before, "lines": lines}),
                    );
                }
            }
            "focus" => self.step(Input::ClientFocus {
                client: id,
                focus: s("focus"),
            }),
            "release" => self.release_op(id, &v),
            // artifacts and diffs (docs/artifacts.md)
            "artifacts" => self.artifacts_op(id, &v),
            "diff" => self.diff_op(id, &v),
            "branches" => self.branches_op(id),
            // shift+tab, `/approvals [yolo|auto]` (approvals-design.md §8)
            "approvals" => {
                let m = match s("mode").as_str() {
                    "toggle" => Some(self.gates.mode.other()),
                    w => crate::approvals::Mode::parse(w),
                };
                if let Some(m) = m {
                    self.set_mode(m);
                    let ev = self.approvals_ev(true);
                    self.broadcast(&ev);
                } else {
                    let mut ev = self.approvals_ev(false);
                    ev["show"] = json!(true);
                    if let Some(c) = self.clients.get_mut(&id) {
                        write_json(c, &ev);
                    }
                }
            }
            // `/approvals`, backspace on a rule: the user removes it
            "remove_rule" => {
                if let Err(e) = self.remove_rule(v.get("rule").unwrap_or(&Value::Null)) {
                    let mut ev = self.approvals_ev(false);
                    ev["error"] = json!(e);
                    if let Some(c) = self.clients.get_mut(&id) {
                        write_json(c, &ev);
                    }
                }
            }
            "confirm" => self.step(Input::ClientConfirm {
                client: id,
                id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
                yes: v.get("yes").and_then(|x| x.as_bool()).unwrap_or(false),
            }),
            "interrupt" => self.step(Input::ClientInterrupt {
                client: id,
                agent: s("agent"),
            }),
            // `/scheduled`'s x (stop) and r (run now)
            "every_stop" => self.step(Input::EveryStop {
                id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
                why: String::new(),
            }),
            "every_run" => self.step(Input::EveryRun {
                id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
            }),
            "stop_hub" => {
                let keep = v
                    .get("keep_agents")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                let _ = self.tx.send(Msg::Shutdown { keep });
            }
            other => {
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(
                        c,
                        &json!({"ev": "notice", "text": format!("unknown op: {}", other)}),
                    );
                }
            }
        }
    }

    /// `sb inspect`: a bounded page of an agent's thread, with positions
    /// and cursors, or the origin of the caller (RFC 0001 §7.5).
    fn inspect(&self, from: &str, v: &Value) -> Value {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let target = s("agent");
        let Some(name) = self.hub.st.resolve(&target) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let Some(dir) = self.dir_of(&name) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let raw = transcript::read(&self.transcript(&dir));
        let all = transcript::entries(&raw);
        let now = now_ms();
        if v.get("origin") == Some(&json!(true)) {
            let Some(me) = self.hub.st.agents.get(from) else {
                return json!({"ok": false, "error": "--origin: unknown calling agent"});
            };
            return match transcript::origin(&raw, &me.dir, me.created_ms) {
                Some(o) => {
                    json!({"ok": true, "text": transcript::render_origin(&name, from, &all, &o, now)})
                }
                None => {
                    json!({"ok": false, "error": format!("no creation of {} in the thread of {}", from, name)})
                }
            };
        }
        let pos = |k: &str| transcript::parse_pos(&s(k));
        let anchor = if let Some(p) = pos("at") {
            Anchor::At(p)
        } else if let Some(p) = pos("around") {
            Anchor::Around(p)
        } else if let Some(p) = pos("before") {
            Anchor::Before(p)
        } else if let Some(p) = pos("after") {
            Anchor::After(p)
        } else {
            Anchor::Tail
        };
        let limit = v
            .get("last")
            .and_then(|x| x.as_u64())
            .map(|n| n as usize)
            .unwrap_or(transcript::DEFAULT_LIMIT);
        let query = s("query");
        let words = transcript::words_of(&query);
        let page = transcript::window(&all, &words, anchor, limit, transcript::BUDGET);
        json!({"ok": true, "text": transcript::render_page(&name, &query, &page, now)})
    }

    /// `sb history` and `sb show` (BISE-233): the index reads what the
    /// transcripts got since the last search, then answers.
    fn search(&mut self, cmd: &str, v: &Value) -> Value {
        let t0 = std::time::Instant::now();
        self.search.refresh(&self.opts.paths.state.join("agents"));
        let who: Vec<search::Who> = self
            .hub
            .st
            .agents
            .values()
            .map(|a| search::Who {
                name: a.name.clone(),
                dir: a.dir.clone(),
                aliases: a.aliases.clone(),
                archived: a.status() == crate::model::Status::Archived,
            })
            .collect();
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let n = |k: &str, d: usize| v.get(k).and_then(|x| x.as_u64()).map_or(d, |x| x as usize);
        let strs = |k: &str| -> Vec<String> {
            v.get(k)
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default()
        };
        let r = if cmd == "show" {
            self.search.show(&who, &s("agent"), n("pos", 0), n("context", search::DEFAULT_CONTEXT), now_ms())
        } else {
            let q = search::Query {
                text: s("query"),
                agents: strs("agents"),
                roles: strs("roles").iter().filter_map(|r| search::Role::parse(r)).collect(),
                since: v.get("since").and_then(|x| x.as_u64()),
                until: v.get("until").and_then(|x| x.as_u64()),
                archived: match s("archived").as_str() {
                    "only" => search::Archived::Only,
                    "no" => search::Archived::No,
                    _ => search::Archived::Any,
                },
                limit: n("limit", search::DEFAULT_HITS),
                page: n("page", 1),
            };
            self.search.search(&who, &q, now_ms())
        };
        let ms = t0.elapsed().as_millis();
        if ms > 200 {
            let st = self.search.stats();
            eprintln!("sb {}: {} ms ({} threads, {} entries)", cmd, ms, st.threads, st.docs);
        }
        match r {
            Ok(text) => json!({"ok": true, "text": text}),
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    /// `sb inspect` and `sb history` read files; the rest goes to the core.
    fn agent_request(&mut self, token: Token, mut stream: UnixStream, v: Value) {
        let from = v
            .get("from")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        match cmd {
            "inspect" => {
                let body = self.inspect(&from, &v);
                write_json(&mut stream, &body);
            }
            "history" | "show" => {
                let body = self.search(cmd, &v);
                write_json(&mut stream, &body);
            }
            "artifact" => {
                let body = self.artifact_cmd(&from, &v);
                write_json(&mut stream, &body);
            }
            _ => match AgentReq::from_json(&v) {
                Ok(req) => {
                    self.replies.insert(token, stream);
                    self.step(Input::Agent { token, from, req });
                }
                Err(e) => {
                    write_json(&mut stream, &json!({"ok": false, "error": e}));
                }
            },
        }
    }
}

fn accept_loop(listener: UnixListener, tx: Sender<Msg>) {
    let mut next: u64 = 1;
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let id = next;
        next += 1;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let Ok(read_half) = stream.try_clone() else {
                return;
            };
            let mut r = BufReader::new(read_half);
            let mut first = String::new();
            if r.read_line(&mut first).unwrap_or(0) == 0 {
                return;
            }
            let Ok(v) = serde_json::from_str::<Value>(first.trim()) else {
                return;
            };
            match v.get("op").and_then(|x| x.as_str()) {
                Some("hello") => {
                    let _ = tx.send(Msg::ClientNew { id, stream });
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match r.read_line(&mut line) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                                    let _ = tx.send(Msg::ClientLine { id, v });
                                }
                            }
                        }
                    }
                    let _ = tx.send(Msg::ClientGone { id });
                }
                Some("agent") => {
                    let _ = tx.send(Msg::AgentNew {
                        token: id,
                        stream,
                        v,
                    });
                }
                Some("version") => {
                    let _ = tx.send(Msg::Version { stream, v });
                }
                Some("notice") => {
                    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let _ = tx.send(Msg::Notice {
                        kind: s("kind"),
                        text: s("text"),
                    });
                }
                Some("ping") => {
                    let mut s = stream;
                    let _ = write_json(&mut s, &json!({"ok": true, "pid": std::process::id()}));
                }
                _ => {}
            }
        });
    }
}

/// A hub that died abruptly (killed, crashed) left its REPLs running:
/// they still hold their sessions. Their pids are in `repl.pid`.
/// The longest a role-line call may take.
const ONESHOT_TIMEOUT: Duration = Duration::from_secs(60);

/// One provider call through `repl-live`'s one-shot mode (BISE_ONESHOT,
/// runtime/oneshot.bend) with `model`: the reply's text, or why not.
/// `keys`: the REPLs' spawn env (the keys, BISE_MODELS_FILE). The
/// provider's error text never holds a key.
fn oneshot(repl: &Path, root: &Path, req_file: &Path, model: &str, keys: &[(String, Option<String>)]) -> Result<String, String> {
    let mut cmd = Command::new(repl);
    crate::approvals::check::oneshot_env(req_file, model, keys).apply(&mut cmd);
    cmd.current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|e| format!("cannot start {}: {}", repl.display(), e))?;
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < ONESHOT_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("no answer in {} s", ONESHOT_TIMEOUT.as_secs()));
            }
        }
    }
    crate::role::oneshot_reply(&reader.join().unwrap_or_default())
}

/// `<agent dir>/role.json`: the last role line and its key (BISE-126).
fn read_role(adir: &Path) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(adir.join("role.json")).ok()?).ok()?;
    let line = v["line"].as_str().filter(|l| !l.is_empty())?.to_string();
    Some((line, v["key"].as_str().unwrap_or("").to_string()))
}

fn write_role(paths: &Paths, adir: &Path, line: &str, key: &str) {
    let tmp = adir.join("role.json.tmp");
    if write_logged(paths, &tmp, &json!({"line": line, "key": key}).to_string()) {
        rename_logged(paths, &tmp, &adir.join("role.json"));
    }
}

fn kill_stale_repls(sh: &Shell) {
    for a in sh.hub.st.agents.values() {
        if sh.pids.contains_key(&a.dir) {
            // adopted at boot, or just spawned
            continue;
        }
        let f = sh.opts.paths.agent_dir(&a.dir).join("repl.pid");
        let Ok(pid) = std::fs::read_to_string(&f) else {
            continue;
        };
        let pid = pid.trim().to_string();
        let cmdline = Command::new("ps")
            .args(["-p", &pid, "-o", "command="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if cmdline.contains("repl-live") {
            log_line(
                &sh.opts.paths,
                &format!("killing a stale REPL of {} (pid {})", a.name, pid),
            );
            if let Ok(pid) = pid.parse() {
                crate::procs::terminate(pid);
            }
        }
        let _ = std::fs::remove_file(&f);
    }
}

/// `sb` for the agents' bash tool: `bin/sb`, a link to this hub's
/// executable, which runs `sb` when called by that name (busybox style).
/// Replaced in one rename: an agent of the previous hub calling `sb`
/// meanwhile finds the old one or the new one, never none.
pub fn write_sb_link(bin_dir: &Path, exe: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(bin_dir)?;
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let tmp = bin_dir.join(format!(".sb.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(&exe, &tmp)?;
    std::fs::rename(&tmp, bin_dir.join("sb")).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Before a hub of another version starts: a hub older than the link
/// writes its `sb` script with a plain write, which would go through the
/// link into this executable. Without `bin/sb`, it writes a new file.
pub fn drop_sb_link(bin_dir: &Path) {
    let p = bin_dir.join("sb");
    if p.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        let _ = std::fs::remove_file(p);
    }
}

/// One step of the hub's start, in hub.log (`boot: ...`): the switcher
/// waits while they come and names the last one when the hub never
/// answers (switch.rs replace_hub). SB_TIMING gets it too.
fn boot_step(paths: &Paths, what: &str) {
    log_line(paths, &format!("boot: {}", what));
    crate::util::timing(what);
}

pub fn run(opts: Opts) -> std::io::Result<()> {
    let paths = opts.paths.clone();
    std::fs::create_dir_all(&paths.state)?;
    // a state path too long for a unix socket: the short link first
    paths.prepare_socket()?;
    // one hub per workspace
    if UnixStream::connect(paths.socket()).is_ok() {
        eprintln!("a hub is already running for {}", paths.workspace.display());
        return Ok(());
    }
    wait_previous_hub(&paths);
    if UnixStream::connect(paths.socket()).is_ok() {
        eprintln!("a hub is already running for {}", paths.workspace.display());
        return Ok(());
    }
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    std::fs::write(paths.pid_file(), std::process::id().to_string())?;
    // a hub one of its own agents relaunched is not that agent's: its
    // sb-core, builds and REPLs do not carry that tag (BISE-243)
    let proc_hub = crate::procs::hub_id(&paths.socket());
    if let Ok(l) = std::env::var(crate::procs::ENV) {
        std::env::set_var(crate::procs::ENV, crate::procs::without_hub(&l, &proc_hub));
    }
    write_sb_link(&paths.bin_dir(), &opts.exe)?;
    // the agents' mktemp reads their TMPDIR (macOS's does not)
    if let Err(e) = crate::tools_env::write_mktemp_shim(&paths.bin_dir()) {
        log_line(&paths, &format!("mktemp shim not written: {}", e));
    }
    // the switcher reads where this hub runs from (to come back to it)
    let _ = std::fs::write(paths.state.join("hub.root"), opts.app_root.to_string_lossy().as_bytes());
    log_line(
        &paths,
        &format!(
            "hub start pid={} workspace={}",
            std::process::id(),
            paths.workspace.display()
        ),
    );

    // the agents' tools, once, off the start path (the login shell may
    // take a moment; the first REPL spawn waits for it)
    let tools_paths = paths.clone();
    std::thread::spawn(move || {
        let p = crate::tools_env::hub_agent_path(&tools_paths.bin_dir());
        let rg = crate::tools_env::which("rg", &p);
        log_line(
            &tools_paths,
            &format!(
                "agents' tools: git {}; rg {}; login-shell PATH {}; PATH={}",
                crate::tools_env::git().describe(),
                rg.map(|r| r.display().to_string()).unwrap_or_else(|| "not installed (agents use grep)".into()),
                if crate::tools_env::login_path().is_some() { "read" } else { "unavailable" },
                p
            ),
        );
    });
    crate::util::timing("start (socket bound)");
    let workspace = paths.workspace.to_string_lossy().to_string();
    let mut hub = Hub::with_core(&workspace, opts.core_bin.clone());
    let (mut events, unreadable) = read_journal(&std::fs::read_to_string(paths.journal()).unwrap_or_default());
    // BISE-230: the task worktrees of the old place (<state>/worktrees/)
    // move to <home>/worktrees/<id>/<task>/, and the journal follows
    let legacy = paths.legacy_worktrees();
    let (moved, errors) = crate::sweep::migrate(&legacy, &paths.worktrees, &crate::sweep::repo_name(&paths.workspace));
    for (old, new) in &moved {
        log_line(&paths, &format!("worktree moved: {} -> {}", old.display(), new.display()));
    }
    for e in &errors {
        log_line(&paths, &format!("worktree not moved: {}", e));
    }
    crate::sweep::follow_moves(&mut events, &paths.worktrees);
    boot_step(&paths, &format!("journal read ({} events)", events.len()));
    if !unreadable.is_empty() {
        log_line(&paths, &format!("journal: {} unreadable lines (not replayed), at line {}", unreadable.len(), lines_list(&unreadable)));
    }
    let skipped = hub.replay(&events);
    if !skipped.is_empty() {
        let mut kinds: Vec<String> = skipped.iter().map(|e| e["type"].to_string()).collect();
        kinds.dedup();
        log_line(
            &paths,
            &format!("journal: {} events of a kind this hub does not know (not applied; a newer hub wrote them?): {}", skipped.len(), kinds.join(", ")),
        );
    }
    boot_step(&paths, "journal replayed");
    // sb-core dies (an OOM, a runtime error, killed): the hub restarts it
    // on the journal instead of dying with it (BISE-292)
    {
        let (jp, lp) = (paths.clone(), paths.clone());
        hub.set_revive(crate::core::Revive::new(
            Box::new(move || {
                let (mut events, _) = read_journal(&std::fs::read_to_string(jp.journal()).unwrap_or_default());
                crate::sweep::follow_moves(&mut events, &jp.worktrees);
                events
            }),
            Box::new(move |s| log_line(&lp, s)),
        ));
    }
    let journal = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.journal())?;

    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let log_paths = paths.clone();
    let env = GitEnv {
        paths: paths.clone(),
        config: Config::load(&paths),
        log: Box::new(move |s| log_line(&log_paths, s)),
        merged: BTreeMap::new(),
    };
    // the checker (approvals-design.md §4): a chat model in the role runs
    // through repl-live's one-shot, like the role lines
    let runner = crate::approvals::check::Runner::new(&bise_home::Home::from_env())
        .with_oneshot(opts.repl_bin.clone(), opts.app_root.clone(), opts.spawn_env);
    let mut sh = Shell {
        opts,
        hub,
        env,
        tx: tx.clone(),
        journal,
        repls: BTreeMap::new(),
        gens: BTreeMap::new(),
        next_gen: 1,
        pids: BTreeMap::new(),
        starts: BTreeMap::new(),
        clients: BTreeMap::new(),
        replies: BTreeMap::new(),
        buffers: BTreeMap::new(),
        positions: BTreeMap::new(),
        search: search::Index::default(),
        offsets: BTreeMap::new(),
        booting: false,
        bins: BTreeMap::new(),
        ports: BTreeMap::new(),
        switching: BTreeMap::new(),
        switch_spawned: BTreeSet::new(),
        restored: BTreeSet::new(),
        building: BTreeSet::new(),
        updating: None,
        release: None,
        update_checked: None,
        update_told: None,
        release_checked: None,
        resume_turn: BTreeSet::new(),
        recorders: BTreeMap::new(),
        reload_id: String::new(),
        reload_repls: BTreeSet::new(),
        spawn_keys: BTreeMap::new(),
        spawn_plugins: BTreeMap::new(),
        plugins_checked: None,
        small_broken: Default::default(),
        setup: None,
        archived: BTreeSet::new(),
        proc_hub,
        down: BTreeSet::new(),
        gates: gate::Gates::new(
            &std::fs::read_to_string(bise_home::Home::from_env().config_file()).unwrap_or_default(),
            std::env::var(crate::approvals::mode::ENV).ok().as_deref(),
            runner,
        ),
        lands: crate::land::Queue::default(),
        features: features::Features::load(&paths.state),
        prs: None,
        cu: crate::computer_use::Watch::new(),
        idle: crate::idle::Watch::new(
            crate::idle::grace(
                std::env::var(crate::idle::ENV).ok().as_deref(),
                &std::fs::read_to_string(bise_home::Home::from_env().config_file()).unwrap_or_default(),
            ),
            std::time::Instant::now(),
        ),
        holds: crate::idle::Holds::default(),
        art: art::Art::default(),
    };
    match sh.idle.grace() {
        Some(g) => log_line(&paths, &format!("idle exit: after {} s without a UI, once nothing runs", g.as_secs())),
        None => log_line(&paths, "idle exit: off (the hub runs until stopped)"),
    }
    // the features' facts (dev-flow §5.1), off the loop
    sh.refresh_features();
    // the role lines of an earlier hub (BISE-126)
    let dirs: Vec<String> = sh.hub.st.agents.values().map(|a| a.dir.clone()).collect();
    for dir in dirs {
        if let Some((line, key)) = read_role(&sh.opts.paths.agent_dir(&dir)) {
            sh.hub.load_role(&dir, line, key);
        }
    }
    // the feeds survive a hub restart through their transcripts
    for a in sh.hub.st.agents.values() {
        let all = transcript::read(&sh.transcript(&a.dir));
        sh.positions.insert(a.name.clone(), all.last().map_or(0, |r| r.0));
        let skip = all.len().saturating_sub(BUFFER_LINES);
        let tail: VecDeque<(usize, u64, String)> = all.into_iter().skip(skip).collect();
        if !tail.is_empty() {
            sh.buffers.insert(a.name.clone(), tail);
        }
    }
    boot_step(
        &paths,
        &format!("transcripts read ({} buffered lines)", sh.buffers.values().map(|b| b.len()).sum::<usize>()),
    );

    {
        let tx = tx.clone();
        std::thread::spawn(move || accept_loop(listener, tx));
    }
    // hub-lag: one tick in the queue at most. A loop slower than the
    // clock (a big state, a loaded machine) queued a tick every 500 ms
    // anyway, and the user's lines waited behind the backlog.
    let tick_queued = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let tx = tx.clone();
        let queued = tick_queued.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            if queued.swap(true, std::sync::atomic::Ordering::AcqRel) {
                continue;
            }
            if tx.send(Msg::In(Input::Tick)).is_err() {
                break;
            }
        });
    }
    // expired-ux: the ChatGPT sign-in coming back (the TUI's sign-in,
    // `bise login chatgpt`): auth.json read again when it changed, once a
    // second (a stat); signed in again after not being: Input::SignedIn
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let path = bise_home::Home::from_env().auth_file();
            let mut seen: Option<std::time::SystemTime> = None;
            let mut was_in: Option<bool> = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let m = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                if m == seen && was_in.is_some() {
                    continue;
                }
                seen = m;
                let store = bise_catalog::auth::Store::read(&path).unwrap_or_default();
                let now_in = matches!(bise_catalog::chatgpt::state(&store), bise_catalog::chatgpt::State::SignedIn { .. });
                let back = now_in && was_in == Some(false);
                was_in = Some(now_in);
                if back && tx.send(Msg::In(Input::SignedIn)).is_err() {
                    break;
                }
            }
        });
    }
    // a rollback's warning, left by the switcher for this hub
    let notice = paths.state.join("switch-notice");
    if let Ok(t) = std::fs::read_to_string(&notice) {
        let _ = std::fs::remove_file(&notice);
        sh.feed(MAIN, &format!("sb warn : {}", wire_escape(&t)));
    }
    // a reload started this hub: every REPL it adopts is relaunched
    sh.reload_id = crate::switch::take_reload(&paths).unwrap_or_default();
    if !sh.reload_id.is_empty() {
        log_line(&paths, &format!("reload {}: every REPL relaunches at its next idle", sh.reload_id));
    }
    // dev-flow §2: the repo's flow, detected off the loop (gh may be slow)
    sh.detect_flow();
    sh.booting = true;
    sh.step(Input::Boot);
    sh.booting = false;
    kill_stale_repls(&sh);
    sh.migrate_the_rest();
    sh.archived = sh.archived_dirs();
    sh.sweep_worktrees(None);
    // the temp folders of agents that are gone (dropped while no hub ran,
    // or unknown): never a live agent's
    let live: BTreeSet<String> = sh
        .hub
        .st
        .agents
        .values()
        .filter(|a| a.lifecycle != Lifecycle::Archived)
        .map(|a| a.dir.clone())
        .collect();
    for d in crate::sweep::sweep_agent_tmps(&paths.state.join("agents"), &live) {
        log_line(&paths, &format!("temp folder removed: {}", d.display()));
    }
    sh.down = sh.down_dirs();
    sh.reap_procs(None);
    sh.start_prs();
    boot_step(&paths, "boot done (REPLs spawned)");

    let mut keep_agents = false;
    while let Ok(m) = rx.recv() {
        match m {
            Msg::In(i) => {
                let tick = matches!(i, Input::Tick);
                if tick {
                    tick_queued.store(false, std::sync::atomic::Ordering::Release);
                    sh.flush_offsets();
                }
                sh.step(i);
                if tick {
                    sh.check_starts();
                    sh.plugins_changed(false);
                    sh.switch_idle_repls();
                    sh.announce_update();
                    sh.release_check(None);
                    sh.plan_prs();
                    sh.idle_check();
                    // computer use (design §7.3): each stop, one line in main's feed
                    for l in sh.cu.poll() {
                        sh.feed(crate::model::MAIN, &format!("sb computer : {}", crate::util::wire_escape(&l)));
                    }
                }
            }
            Msg::ReplConnected {
                dir,
                gen,
                stream,
                steer,
                interrupt,
                pid,
                adopted,
                busy,
            } => {
                if sh.gens.get(&dir) != Some(&gen) {
                    // killed while it was starting
                    kill_pid(pid);
                    continue;
                }
                if !adopted {
                    let _ = std::fs::write(&steer, "");
                    let _ = std::fs::write(&interrupt, "");
                }
                sh.repls.insert(
                    dir.clone(),
                    Repl {
                        stream,
                        steer,
                        interrupt,
                    },
                );
                sh.switch_spawned.remove(&dir);
                crate::util::timing(&format!("repl connected {} (adopted {})", dir, adopted));
                if let Some(q) = sh.switching.remove(&dir) {
                    // a switched REPL: same session, the core never saw
                    // it go; the writes it missed go now
                    log_line(&sh.opts.paths, &format!("switched the REPL of {}", dir));
                    if let Some(r) = sh.repls.get_mut(&dir) {
                        for l in &q {
                            let _ = r.stream.write_all(l.as_bytes());
                        }
                    }
                    if !q.is_empty() {
                        continue;
                    }
                }
                if !adopted && sh.resume_turn.remove(&dir) {
                    // its turn was cut: the first turn of the new process
                    // continues it (queued writes of the core come after)
                    if let Some(r) = sh.repls.get_mut(&dir) {
                        let _ = r
                            .stream
                            .write_all(format!("say {}\n", wire_escape(RESUME_TEXT)).as_bytes());
                    }
                    if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        sh.feed(
                            &name,
                            "sb info : its turn was interrupted by a restart — it continues where it left off",
                        );
                    }
                }
                if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    if busy {
                        // adopted mid-turn: busy until its `--- idle`
                        sh.step(Input::ReplLine {
                            agent: name.clone(),
                            line: "  obs: turn_started".into(),
                        });
                    } else {
                        sh.step(Input::ReplReady { agent: name.clone() });
                    }
                    // design §10: a restart keeps the card the REPL waits on
                    if adopted {
                        sh.gate_restore(&dir, &name);
                    } else {
                        sh.gate_fresh(&name);
                    }
                }
            }
            Msg::ReplSpawned { dir, gen, pid } => {
                if sh.starts.get(&dir).is_some_and(|(g, _)| *g == gen) {
                    sh.starts.remove(&dir);
                }
                let _ = std::fs::write(
                    sh.opts.paths.agent_dir(&dir).join("repl.pid"),
                    pid.to_string(),
                );
                crate::procs::add_sid(&sh.opts.paths.agent_dir(&dir).join("repl.sids"), pid);
                if sh.gens.get(&dir) == Some(&gen) {
                    sh.pids.insert(dir, (gen, pid));
                } else {
                    kill_pid(pid);
                }
            }
            Msg::ReplLine {
                dir,
                gen,
                line,
                offset,
            } => {
                if sh.gens.get(&dir) == Some(&gen) {
                    if !sh.on_ev_line(&dir, &line, offset) {
                        sh.on_repl_line(&dir, &line);
                    }
                    sh.offsets.insert(dir, offset);
                }
            }
            Msg::ReplGone {
                dir,
                gen,
                reason,
            } => {
                // a killed generation is not live anymore: its exit is
                // expected; any exit of the live one is a crash (the hub
                // never asks a REPL to quit)
                if sh.gens.get(&dir) != Some(&gen) {
                    continue;
                }
                sh.starts.remove(&dir);
                sh.gens.remove(&dir);
                sh.repls.remove(&dir);
                sh.gate_forget(&dir);
                sh.pids.remove(&dir);
                // its writer's lock goes with it (a respawn resumes the log)
                sh.recorders.remove(&dir);
                if sh.switching.contains_key(&dir) && !sh.switch_spawned.contains(&dir) {
                    // the reload a switch asked for: the same session on
                    // this hub's binary, the same port
                    let port = sh.ports.get(&dir).copied();
                    if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        sh.restored.insert(dir.clone());
                        sh.switch_spawned.insert(dir.clone());
                        sh.spawn_on(&name, true, None, port);
                        continue;
                    }
                }
                // the new process of a switch died: a crash like any other
                sh.switching.remove(&dir);
                sh.switch_spawned.remove(&dir);
                sh.restored.remove(&dir);
                // a new version on probation: a REPL that dies is a
                // reason to roll back
                crate::switch::report_failure(
                    &sh.opts.paths,
                    &format!("the REPL of {} stopped: {}", dir, reason),
                );
                if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    sh.step(Input::ReplExited {
                        agent: name,
                        crashed: true,
                        reason,
                    });
                }
            }
            Msg::ClientNew { id, stream } => sh.client_hello(id, stream),
            Msg::ClientLine { id, v } => sh.client_line(id, v),
            Msg::ClientGone { id } => {
                if sh.clients.remove(&id).is_some() {
                    sh.step(Input::ClientGone { client: id });
                }
            }
            Msg::AgentNew { token, stream, v } => sh.agent_request(token, stream, v),
            Msg::Version { mut stream, v } => {
                let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("");
                let what = v.get("do").and_then(|x| x.as_str()).unwrap_or("");
                match version_allowed(from, what) {
                    Ok(()) => {
                        let text = sh.version_op(&v);
                        write_json(&mut stream, &json!({"ok": true, "text": text}));
                    }
                    Err(e) => {
                        write_json(&mut stream, &json!({"ok": false, "error": e}));
                    }
                }
            }
            Msg::Notice { kind, text } => {
                let kind = if kind == "warn" { "warn" } else { "info" };
                sh.feed(MAIN, &format!("sb {} : {}", kind, wire_escape(&text)));
                sh.broadcast_versions();
            }
            Msg::Release { client, v } => sh.release_event(client, v),
            Msg::Update(v) => sh.update_event(v),
            Msg::Changes { name, v } => sh.on_changes(name, v),
            Msg::ToClient { id, v } => {
                if let Some(c) = sh.clients.get_mut(&id) {
                    write_json(c, &v);
                }
            }
            Msg::RoleLine { dir, key, line } => sh.step(Input::RoleLine { dir, key, line }),
            Msg::GateChecked { dir, n, req, out } => sh.on_checked(&dir, &n, *req, out),
            Msg::BuildEnded { rev } => {
                sh.building.remove(&rev);
                sh.broadcast_versions();
            }
            Msg::Land { line } => {
                if let Some((kind, text)) = line {
                    sh.feed(MAIN, &format!("sb {} : {}", kind, wire_escape(&text)));
                    // a land moved main or a feature: the features' facts again
                    sh.refresh_features();
                }
                let snap = sh.snapshot();
                sh.broadcast(&snap);
            }
            Msg::Shutdown { keep } => {
                keep_agents = keep;
                break;
            }
        }
    }
    // for good: no new client reaches a hub that is going (it would wait
    // for a hello that never comes); a `bise` meanwhile starts the next
    // hub, which waits for this one to be gone (`wait_previous_hub`)
    if !keep_agents {
        let _ = std::fs::remove_file(paths.socket());
    }
    sh.flush_offsets();
    if keep_agents {
        log_line(&paths, "hub stop (REPLs kept for the next hub)");
    } else {
        log_line(&paths, "hub stop");
        // idle-exit: each idle REPL checkpoints its session and exits (the
        // `reload` of a switch: a turn boundary, nothing lost); a busy
        // one refuses and gets SIGTERM after, as before
        let pids: Vec<u32> = sh.pids.values().map(|(_, p)| *p).collect();
        for r in sh.repls.values_mut() {
            let _ = r.stream.write_all(b"reload\n");
        }
        let t0 = std::time::Instant::now();
        while pids.iter().any(|p| crate::procs::alive(*p)) && t0.elapsed() < REPL_QUIT {
            std::thread::sleep(Duration::from_millis(50));
        }
        let left: Vec<u32> = pids.iter().copied().filter(|p| crate::procs::alive(*p)).collect();
        log_line(
            &paths,
            &format!("REPLs saved and gone: {} of {}", pids.len() - left.len(), pids.len()),
        );
        for pid in left {
            kill_pid(pid);
        }
        // for good: what every agent started goes with it (BISE-243)
        let none = BTreeSet::new();
        let mut sessions = BTreeSet::new();
        for a in sh.hub.st.agents.values() {
            let f = paths.agent_dir(&a.dir).join("repl.sids");
            sessions.extend(crate::procs::read_sids(&f));
            let _ = std::fs::remove_file(f);
        }
        let hit = crate::procs::reap(&sh.proc_hub, &crate::procs::Want::NotLive(&none), &sessions, Duration::from_secs(3));
        if !hit.is_empty() {
            log_line(&paths, &format!("processes of the agents killed: {}", crate::procs::describe(&hit)));
        }
    }
    // the next hub's socket and pid are not ours to remove
    let ours = std::fs::read_to_string(paths.pid_file()).is_ok_and(|p| p.trim() == std::process::id().to_string());
    if ours {
        if keep_agents {
            let _ = std::fs::remove_file(paths.socket());
        }
        let _ = std::fs::remove_file(paths.pid_file());
    }
    Ok(())
}

/// How long a starting hub waits for the previous one of its workspace
/// to finish stopping (its REPLs checkpoint, its agents' processes go).
const PREVIOUS_HUB_WAIT: Duration = Duration::from_secs(15);

/// A hub that stops for good removes its socket first: a `bise` launched
/// meanwhile starts a new hub at once. That one waits here until the
/// previous hub's process (`hub.pid`, still a `sbd`) is gone, so the two
/// never share a REPL, a session or the reap of the agents' processes.
fn wait_previous_hub(paths: &Paths) {
    let Some(pid) = std::fs::read_to_string(paths.pid_file()).ok().and_then(|p| p.trim().parse::<u32>().ok()) else {
        return;
    };
    if pid == std::process::id() || !crate::procs::alive(pid) {
        return;
    }
    let cmd = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if !cmd.contains(" sbd") {
        return;
    }
    let t0 = std::time::Instant::now();
    while crate::procs::alive(pid) && t0.elapsed() < PREVIOUS_HUB_WAIT {
        std::thread::sleep(Duration::from_millis(50));
    }
    log_line(paths, &format!("waited {} ms for the previous hub (pid {}) to stop", t0.elapsed().as_millis(), pid));
}

/// A hash of a REPL's spawn keys (never the keys themselves, kept).
/// The plugins fingerprint of a workspace's roots (built-in, user,
/// `<ws>/.agents/plugins`, the enable state).
/// In an agent's dir: the plugins fingerprint its REPL's prompt was
/// last built with.
const PROMPT_PLUGINS_FILE: &str = "prompt-plugins.fp";

/// Whether a session's prompt predates its plugins: the fingerprint of
/// its last start differs from `fp` (none recorded: a session from
/// before this file, its prompt rebuilt once).
fn prompt_is_stale(adir: &Path, fp: u64) -> bool {
    std::fs::read_to_string(adir.join(PROMPT_PLUGINS_FILE)).map(|s| s.trim() != fp.to_string()).unwrap_or(true)
}

/// What a live REPL's prompt was built from: its workspace, whether it is
/// main, the plugins fingerprint of its roots and the skills fingerprint
/// of the skill folders its startup scan reads.
struct PromptInputs {
    ws: PathBuf,
    main: bool,
    plugins: u64,
    skills: u64,
}

impl PromptInputs {
    /// The one number kept in `<agent dir>/prompt-plugins.fp`.
    fn combined(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.plugins.hash(&mut h);
        self.skills.hash(&mut h);
        h.finish()
    }
}

fn plugins_fingerprint(ws: &Path) -> u64 {
    bend_plugins::resolve::fingerprint(&bend_plugins::resolve::Roots::standard(Some(ws)))
}

/// The skill folders a REPL's startup scan reads (runtime/skills.bend
/// `scan_script`): `~/.agents/skills`, `~/.vibe/skills`,
/// `<ws>/.agents/skills` and, for main, the app root's `prompts/skills`
/// (the plugins' skills are in the plugins fingerprint).
fn skill_roots(ws: &Path, app_root: &Path, is_main: bool) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        let home = PathBuf::from(home);
        roots.push(home.join(".agents/skills"));
        roots.push(home.join(".vibe/skills"));
    }
    roots.push(ws.join(".agents/skills"));
    if is_main {
        roots.push(app_root.join("prompts/skills"));
    }
    roots
}

/// Each root, then each `<root>/<skill>/SKILL.md` (sorted) with its size
/// and mtime; a missing root or file hashes as absent.
fn skills_fingerprint(roots: &[PathBuf]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for root in roots {
        root.hash(&mut h);
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let Ok(m) = std::fs::metadata(d.join("SKILL.md")) else { continue };
            d.hash(&mut h);
            m.len().hash(&mut h);
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .hash(&mut h);
        }
    }
    h.finish()
}

/// Dev: log every model request's body (`BISE_DEBUG_REQUESTS=1` in the
/// hub's environment, or a `debug-requests` file in its state folder).
fn debug_requests(state: &std::path::Path) -> bool {
    std::env::var("BISE_DEBUG_REQUESTS").is_ok_and(|v| !v.is_empty() && v != "0") || state.join("debug-requests").exists()
}

fn hash_keys(keys: &[(String, Option<String>)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    keys.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `bin/sb` of an older hub (a script) becomes a link to the exe, in
    /// A SKILL.md added, edited or removed in a root moves the skills
    /// fingerprint; nothing changing, or a folder without SKILL.md, does not.
    #[test]
    fn the_skills_fingerprint_moves_when_a_skill_comes_goes_or_changes() {
        let d = std::env::temp_dir().join(format!("sb-skills-fp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let roots = vec![d.join("user"), d.join("ws/.agents/skills")];
        let empty = skills_fingerprint(&roots);
        std::fs::create_dir_all(d.join("ws/.agents/skills/notes")).unwrap();
        assert_eq!(skills_fingerprint(&roots), empty, "a folder without SKILL.md is no skill");
        let f = d.join("ws/.agents/skills/a/SKILL.md");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "---\nname: a\ndescription: one\n---\n").unwrap();
        let added = skills_fingerprint(&roots);
        assert_ne!(added, empty);
        assert_eq!(skills_fingerprint(&roots), added, "stable while nothing changes");
        std::fs::write(&f, "---\nname: a\ndescription: two longer\n---\n").unwrap();
        let edited = skills_fingerprint(&roots);
        assert_ne!(edited, added);
        std::fs::remove_dir_all(f.parent().unwrap()).unwrap();
        assert_eq!(skills_fingerprint(&roots), empty);
        // main reads the app root's prompts/skills too, a task does not
        let ws = d.join("ws");
        assert!(skill_roots(&ws, &d, true).contains(&d.join("prompts/skills")));
        assert!(!skill_roots(&ws, &d, false).contains(&d.join("prompts/skills")));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// one rename (no temp file left); only a link is dropped for an
    /// older version's hub.
    #[test]
    fn the_sb_link_replaces_the_script() {
        let d = std::env::temp_dir().join(format!("sb-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = d.join("bise");
        std::fs::write(&exe, "exe").unwrap();
        std::fs::write(bin.join("sb"), "#!/bin/sh\nexec old sb \"$@\"\n").unwrap();
        write_sb_link(&bin, &exe).unwrap();
        write_sb_link(&bin, &exe).unwrap();
        let sb = bin.join("sb");
        assert!(sb.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_link(&sb).unwrap(), std::fs::canonicalize(&exe).unwrap());
        let names: Vec<_> = std::fs::read_dir(&bin).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("sb")]);
        drop_sb_link(&bin);
        assert!(!sb.exists() && std::fs::read_to_string(&exe).unwrap() == "exe");
        std::fs::write(&sb, "a file").unwrap();
        drop_sb_link(&bin);
        assert!(sb.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A line that is not a JSON object (a half-written last line) is
    /// counted with its number, never dropped in silence; a kind the Rust
    /// side does not know is kept (sb-core decodes the events).
    #[test]
    fn the_journal_keeps_unknown_kinds_and_counts_unreadable_lines() {
        let text = "{\"type\":\"main_notes_flushed\"}\n{\"type\":\"from_a_newer_hub\",\"x\":1}\n\n42\n{\"type\":\"main_no";
        let (events, bad) = read_journal(text);
        assert_eq!(events.len(), 2);
        assert_eq!(events[1]["type"], "from_a_newer_hub");
        assert_eq!(bad, vec![4, 5]);
        assert_eq!(lines_list(&(1..=12).collect::<Vec<_>>()), "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, ...");
    }

    /// A `history` page carries each line's transcript time as `ts`
    /// (C2 amendment); a line whose stamp does not parse has no `ts`.
    #[test]
    fn history_lines_carry_their_time() {
        let dir = std::env::temp_dir().join(format!("sb-hist-ts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("transcript.log");
        std::fs::write(&path, "1700000000000\tyou : hi\nx\tobs: turn_started\n1700000400000\t--- idle\n").unwrap();
        let page: Vec<Value> = transcript_page(&path, 4, 10)
            .into_iter()
            .map(|(pos, ts, line)| history_line(pos, ts, &line))
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(page[0], json!({"pos": 1, "line": "you : hi", "ts": 1700000000000u64}));
        assert_eq!(page[1], json!({"pos": 2, "line": "obs: turn_started"}));
        assert_eq!(page[2]["ts"], 1700000400000u64);
    }

    /// A live `line` carries its transcript time as `ts` too (BISE-271:
    /// the turns' end times, the pause marks of a replayed feed); an
    /// unknown time (0) is left out.
    #[test]
    fn live_lines_carry_their_time() {
        assert_eq!(
            line_event("main", 3, 1700000000000, "  obs: turn_done: completed"),
            json!({"ev": "line", "agent": "main", "line": "  obs: turn_done: completed", "pos": 3, "ts": 1700000000000u64})
        );
        assert_eq!(line_event("t1", 1, 0, "x"), json!({"ev": "line", "agent": "t1", "line": "x", "pos": 1}));
    }
}
