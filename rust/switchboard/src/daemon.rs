//! `sbd`: the hub of one workspace (RFC 0001 §5), the imperative shell
//! around `core::Hub`. One thread owns the hub and executes its effects;
//! the other threads only read (sockets, REPL output) and send messages.
//!
//! - one Bend REPL per live agent, spawned from the app root, with
//!   BEND_WORKDIR / BEND_EXTRA_PROMPT / BEND_CONTEXT_FILE / SB_AGENT;
//! - `hub.sock`: clients (`{"op":"hello"}` or JSON-RPC's `initialize`
//!   first, then JSON lines: `daemon/rpc.rs`), never from an agent's
//!   process; `agent.sock`: the agents' `sb` CLI
//!   (`{"op":"agent", ...}`, one request, one reply), their
//!   `SB_SOCKET` (`daemon/accept`, `crate::peer`, docs/issues/16);
//! - `journal.jsonl`: the durable state; `agents/<dir>/transcript.log`:
//!   every line of each feed, for the views, `sb inspect` and
//!   `sb history`.
//!
//! The loop (`run`): every thread sends a `Msg` on one channel; the
//! loop reads it through `daemon/inbox` (the clients' messages ahead of
//! the rest, so a hello is served while REPL lines wait) and runs each
//! one with `daemon/dispatch`.

mod accept;
mod agent_req;
mod answers;
mod art;
mod art_folders;
mod attach;
mod boot;
mod capsule;
mod choose;
mod features;
mod fn_context;
pub(crate) mod gate;
mod history;
mod lines;
mod names;
mod page_cards;
mod pages;
mod projects;
mod prompt_inputs;
use prompt_inputs::{plugins_fingerprint, prompt_is_stale, PromptInputs, PROMPT_PLUGINS_FILE};
mod proto;
mod pulse;
mod repl;
mod repl_starts;
mod rpc;
mod recycling;
mod routing;
mod release;
mod session_log;
mod skills;
mod small_ask;
mod versions;
mod dispatch;
mod drops;
mod inbox;
mod journal_file;
mod keys;
mod land;
use keys::hash_keys;
mod loop_watch;
mod merged;
mod outbox;
mod photo_take;
mod processes;
mod revive;
mod state_gate;
mod contexts;
mod tindex;
mod usage;
mod views;
mod wake_look;
mod place_work;
mod worktrees;
mod xhub;
mod xread;

use history::transcript_page;
use lines::transcript_len;
use repl::{adopt, adoptable, busy_at, kill_pid, supervise};
use skills::{skill_roots, skills_fingerprint};
use versions::version_allowed;
use crate::core::{AgentReq, ClientId, Effect, Hub, Input, Token};
use bise_proto::core::Feed;
use bise_proto::hub::HubEv;
use crate::devflow;
use crate::model::{Agent, Lifecycle, MAIN};
use crate::paths::Paths;
use crate::prompts;
use crate::search;
use crate::transcript;
use crate::util::{now_ms, wire_escape};
use crate::worktree::GitEnv;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Write;
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
/// The line a page's input to main ends with (ambient-lead m_5724):
/// ambient-core keys on "[from the page:" to keep that turn off the
/// capsule; main's rule (prompts.rs, amb-kit 83c1a8af) reads it.
const PAGE_HINT: &str = "[from the page: answer on the page; one short line at most]";

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
    /// Boxed: an Input is far bigger than every other message (clippy's
    /// large_enum_variant); `Msg::input` builds it.
    In(Box<Input>),
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
    /// A phase of a start is done (repl_starts: progress).
    ReplStartStep {
        dir: String,
        gen: u64,
    },
    ReplGone {
        dir: String,
        gen: u64,
        reason: String,
        cause: repl_starts::Gone,
    },
    /// a client from before client-protocol said `{"op":"hello"}`
    /// (accept.rs): exe and reload, then the connection closes
    OlderHello {
        stream: UnixStream,
        /// its pid and program, for hub.log
        peer: String,
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
        asker: accept::Asker,
    },
    /// bise's cross-hub `xin` / `xreply` (daemon/xhub.rs).
    XHub {
        token: Token,
        stream: UnixStream,
        v: Value,
    },
    /// `sb version …` (one request, one answer).
    Version {
        stream: UnixStream,
        v: Value,
        asker: accept::Asker,
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
    Notice { kind: String, text: String },
    /// A watch's event, seen off the loop (daemon/wake_look.rs).
    WakeSeen(wake_look::WakeSeen),
    /// A worktree made off the loop (daemon/place_work.rs).
    Placed { name: String, ws: Value, err: String },
    /// A role-line call is over (BISE-126).
    RoleLine {
        dir: String,
        key: String,
        line: Option<String>,
    },
    /// A timer-name call is over (every_name.rs).
    /// The week's transcripts read (daemon/usage.rs's fill): the usage
    /// ledger and the pulse's week (daemon/pulse.rs).
    UsageCounted(Box<(crate::usage_ledger::Ledger, pulse::Week)>),
    TimerName {
        id: u64,
        reply: Option<String>,
    },
    /// mcp-apps (C5): a view request's next step on the loop, from its
    /// thread (daemon/views.rs).
    View(views::ViewStep),
    /// The checker answered a gated call (approvals-design.md §4).
    GateChecked {
        dir: String,
        n: String,
        req: Box<crate::approvals::check::CheckReq>,
        out: crate::approvals::check::CheckOut,
    },
    /// A land joined the line or ended (`sb land`): the views refresh
    /// (the lids); `line`: main's feed record (info, warn, landed).
    Land {
        line: Option<Feed>,
    },
    /// The page server (docs/ambient-pages.md §2.4): notes sent from a
    /// page, or its drafts changed.
    Page(crate::pages::PageMsg),
    /// An agent's changes, computed off the loop (`daemon/art.rs`).
    Changes {
        name: String,
        v: Option<bise_proto::rows::Changes>,
    },
    /// A local site's recipe, found off the loop after its add
    /// (`daemon/art.rs`, artifacts_probe): `root` the folder a static
    /// site serves, which bise copies.
    Serve {
        id: String,
        v: u64,
        serve: crate::artifacts_serve::Serve,
        root: Option<PathBuf>,
    },
    /// a thread's typed event: an answer or a hub-wide notification
    Typed {
        to: rpc::Typed,
        v: Value,
    },
    /// the processes worker's counts changed (daemon/processes.rs)
    ProcCounts(processes::Counts),
    /// a connection whose first line is JSON-RPC's `initialize`
    RpcNew {
        id: ClientId,
        stream: UnixStream,
        v: Value,
        /// its pid and program, for hub.log (accept.rs `Doors::who`)
        peer: String,
    },
    /// `keep`: leave the REPLs running for the next hub to adopt.
    Shutdown {
        keep: bool,
    },
}

struct Repl {
    /// its writer (daemon/outbox.rs): the loop only queues
    stream: outbox::Outbox,
    steer: String,
    interrupt: String,
}

struct Shell {
    opts: Opts,
    hub: Hub,
    env: GitEnv,
    tx: Sender<Msg>,
    journal: journal_file::Out,
    repls: BTreeMap<String, Repl>,
    /// The live generation of each agent's REPL: lines and exits of an
    /// older (killed) generation are ignored.
    gens: BTreeMap<String, u64>,
    next_gen: u64,
    /// Every REPL process alive, connected or not: (generation, pid).
    pids: BTreeMap<String, (u64, u32)>,
    /// REPL starts in flight, at most repl_start::MAX_IN_FLIGHT; one
    /// without progress is a failed start (daemon/repl_starts.rs;
    /// BISE-291: it stayed `starting` forever, with no line anywhere).
    starts: crate::repl_start::Starts,
    /// fresh spawns waiting for a slot
    start_queue: BTreeMap<String, repl_starts::Queued>,
    /// each client's writer (daemon/outbox.rs): the loop only queues
    clients: BTreeMap<ClientId, outbox::Outbox>,
    /// what the loop runs, said in hub.log when it holds (daemon/loop_watch.rs)
    loop_watch: loop_watch::LoopWatch,
    replies: BTreeMap<Token, UnixStream>,
    /// The last lines of each feed, with their transcript positions and
    /// the time the transcript wrote them (ms since the epoch).
    buffers: BTreeMap<String, VecDeque<(usize, u64, String)>>,
    /// The position of the last line of each feed's transcript.
    positions: BTreeMap<String, usize>,
    /// Every thread, indexed for `sb history` (built at the first search).
    search: search::Index,
    /// The other projects' threads (and bise's under `bise/`), for `sb
    /// history --project|--all` (xread.rs; refreshed only by a query).
    xsearch: search::Index,
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
    /// Each agent's answer as it streams in (its `partial` lines, by
    /// name): memory only, never journaled (daemon/lines.rs).
    partials: BTreeMap<String, bise_proto::thread::lines::Partial>,
    /// Each transcript's offset index (thread pages and `thread/find`,
    /// daemon/tindex.rs): built by a worker the first time it is paged.
    tix: tindex::Indexes,
    /// Each connection's newest `thread/find` per agent: an older search
    /// still running stops when it sees it moved (daemon/proto/find.rs).
    finds: BTreeMap<(ClientId, String), std::sync::Arc<std::sync::atomic::AtomicU64>>,
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
    /// update-card: when this hub last looked at the machine's release
    /// offer (None: not yet, the first tick looks).
    release_checked: Option<std::time::Instant>,
    /// update-flow: when this hub last started its switch to the
    /// machine's `update now` release (no second one while it starts).
    release_switched: Option<std::time::Instant>,
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
    /// What each live REPL read since it started: an aged one restarts
    /// at idle (daemon/recycling.rs, a workaround for the Bend allocator).
    recycle: recycling::Recycle,
    /// What the project's calls cost, per agent and model (daemon/usage.rs).
    usage: usage::Usage,
    /// The project's pulse: its turns, main's entries for him, its news
    /// (daemon/pulse.rs).
    pulse: pulse::PulseState,
    /// The last plugins check (every 2 s on the tick).
    plugins_checked: Option<std::time::Instant>,
    /// The small model failed and agent_model answered: role lines use
    /// agent_model for the rest of this hub's life (BISE-126).
    small_broken: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The catalog and config.toml (BISE-135: the model and effort each
    /// agent runs with), re-read when config.toml changes.
    setup: Option<(Option<std::time::SystemTime>, bise_catalog::Setup)>,
    /// config.toml's path, found once (the env a hub runs with doesn't
    /// change): `setup()` runs per REPL line, `Home::from_env` each time
    /// was 8% of a flooded loop (hub-lag-7); only the stat is per call.
    config_path: Option<PathBuf>,
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
    /// mcp-apps (C5): the views' limits, waiting calls, contexts and the
    /// hub's views bridge (daemon/views.rs).
    views: views::Views,
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
    /// the pages: their server and what the hub keeps about them (daemon/pages.rs)
    pg: pages::PageState,
    /// the typed protocol's connections (bise desktop S3a, daemon/proto.rs)
    proto: proto::Proto,
    /// the inbox's history, from the journal (inbox-record, daemon/answers.rs)
    answers: answers::Answers,
    /// the JSON-RPC connections and the hub-wide watermark (daemon/rpc.rs)
    rpc: rpc::Rpcs,
    /// view.json's writer (bise desktop S1, daemon/projects.rs)
    projects: projects::Projects,
    /// when the state goes to the clients (daemon/state_gate.rs)
    state_gate: state_gate::StateGate,
    /// when the pending contexts are built (daemon/contexts.rs)
    context_gate: state_gate::StateGate,
    /// S9: the fn context of the input being stepped: (agent, its JSON).
    /// Its 'you' line goes to the transcript without the rendered block,
    /// with an `sb context : <json>` line after it in the same append
    /// (feed). Set and cleared around one `step`.
    fn_ctx: Option<(String, String)>,
    /// idle-exit (`idle.rs`): no UI for the grace and nothing runs, the
    /// hub stops for good
    idle: crate::idle::Watch,
    /// holds on the hub besides the `hello` clients: a page server gets
    /// a clone, and each open event stream holds one while it is open
    holds: crate::idle::Holds,
    /// artifacts and diffs (`daemon/art.rs`, docs/artifacts.md)
    art: art::Art,
    /// the local sites' servers it started again (`daemon/revive.rs`)
    revive: revive::Revive,
    /// `sb wake`'s look, on its worker (`daemon/wake_look.rs`)
    wake_look: wake_look::WakeLook,
    /// sb-core's worktrees, made off the loop (`daemon/place_work.rs`)
    place_work: place_work::PlaceWork,
    /// the running processes' worker: scans, ended log, output, stop
    /// (`daemon/processes.rs`, docs/processes.md)
    procs: processes::Worker,
    /// the boot photo's bookkeeping (`daemon/photo_take.rs`, issue 36)
    photos: photo_take::Photos,
}

/// What an agent whose turn was cut by a restart receives.
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

/// One reply to a one-request peer, off the loop (daemon/outbox.rs: a
/// peer that reads nothing never holds the loop).
fn write_line(stream: &mut UnixStream, line: &str) -> bool {
    outbox::reply(stream, line)
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
        let path = self.config_path.get_or_insert_with(|| bise_home::Home::from_env().config_file()).clone();
        let mtime = bise_catalog::live_fetch::stamp(&path, &bise_home::Home::from_env().cache_dir());
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

    /// A line of the hub in an agent's thread (an `info` record).
    fn info_line(&mut self, agent: &str, text: &str) {
        self.feed_rec(agent, Feed::info(text));
    }

    /// A record of the hub's into a feed (issue 25: every line the hub
    /// writes is a `Feed` record).
    fn feed_rec(&mut self, name: &str, rec: Feed) {
        self.feed(name, &rec.line());
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
        let report = AgentReq::Report { kind: "progress".into(), summary: text.clone(), decisions: Vec::new(), step: None, result: None };
        self.step(Input::Agent { token: 0, from: agent.to_string(), req: report, by: crate::peer::Sender::hub() });
        self.step(Input::Agent {
            token: 0,
            from: MAIN.to_string(),
            by: crate::peer::Sender::hub(),
            req: AgentReq::Send {
                to: agent.to_string(),
                text: format!("bise: {} your turn starts again on it: carry on with your task.", text),
                expect_reply: false,
                reply_to: None,
                queued: true,
                why: String::new(),
                switch: None,
                attach: Vec::new(),
                parts: String::new(),
                fwd: None,
            },
        });
        self.state_now();
    }

    /// Each agent's model for `sb list`, `sb tasks` and the roster
    /// (issue #4): its tag and its `model:` line, from its choice file.
    /// What each agent runs with, by name (the snapshot's model fields).
    fn refresh_models(&mut self) -> BTreeMap<String, bise_catalog::InUse> {
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
        used.into_iter().map(|(n, u, _)| (n, u)).collect()
    }

    /// The client snapshot with each agent's model and effort: its full
    /// id, the effort ("" when the model takes none), and the words it
    /// takes (the `/reasoning` list).
    /// The hub's snapshot (core_snapshot.rs), the shell's parts each from its one writer.
    fn snapshot(&mut self) -> crate::snapshot::Snapshot {
        // issue #4: each agent's model, for sb list / sb tasks too
        let used = self.refresh_models();
        // the repo's flow, as config.toml says now (flow-prompts saves it)
        self.hub.flow = crate::flow::FlowConfig::load(&self.opts.paths).mode;
        // pr-news: whose bots' comments reach the agents (`[pr] trusted_bots`)
        self.hub.pr_bots =
            crate::forge::news::trusted_bots(&std::fs::read_to_string(self.opts.paths.config()).unwrap_or_default());
        // who lands, who waits in line (the boxes' lids are said from it)
        self.hub.landing = self.lands.ranks();
        // the features' lids and Δ (dev-flow §5.1)
        self.feature_views();
        let mut snap = self.hub.snapshot(now_ms());
        // the pages (docs/ambient-pages.md §2.3), newest first, and the
        // cards' page links (daemon/page_cards.rs)
        self.snapshot_pages(&mut snap);
        self.snapshot_card_links(&mut snap);
        // one gate card for several agents' identical calls: it names them all
        self.gate_card_agents(&mut snap);
        self.snapshot_models(&mut snap, &used);
        snap
    }

    /// Each agent's model and its "± 9 files so far" door (docs/artifacts.md).
    fn snapshot_models(&self, snap: &mut crate::snapshot::Snapshot, used: &BTreeMap<String, bise_catalog::InUse>) {
        for a in &mut snap.agents {
            let Some(u) = used.get(&a.name) else { continue };
            a.model = Some(crate::snapshot::Model {
                name: u.model.name.clone(),
                effort: u.effort.clone(),
                efforts: u.model.efforts(),
                changes: self.art.changes.get(&a.name).cloned().flatten(),
            });
        }
    }

    /// The state to the clients, with its page hooks and view.json: what
    /// sb-core's `state` effect did at once, now when the loop's gate
    /// allows (daemon/state_gate.rs).
    fn push_state(&mut self) {
        self.page_answers();
        self.page_agent_push();
        self.page_watch_push();
        let snap = self.snapshot();
        self.proto_state(&snap);
        self.view_changed(&snap);
        self.state_gate.sent(now_ms());
    }

    /// A user action's snapshot, to every client at once; a throttled
    /// push still pending waits its period from now (state_gate `shown`).
    fn state_now(&mut self) {
        let snap = self.snapshot();
        self.proto_state(&snap);
        self.state_gate.shown(now_ms());
    }

    fn transcript(&self, dir: &str) -> PathBuf {
        self.opts.paths.agent_dir(dir).join("transcript.log")
    }

    /// A line enters a feed: memory, transcript, every client.
    fn feed(&mut self, name: &str, line: &str) {
        // a failed turn keeps what streamed in, right above its failure
        if let Some(kept) = self.partial_kept(name, line) {
            self.feed(name, &kept);
        }
        if self.pg.page_waits.contains_key(name) && crate::pages::took_input(line) {
            self.pg.page_waits.remove(name);
        }
        self.page_turn(name, line);
        self.page_agent_line(name, line);
        let Some(dir) = self.dir_of(name) else { return };
        let path = self.transcript(&dir);
        let lines = self.fn_ctx_lines(name, line);
        // positions, the memory buffer, the transcript text, the events
        // (daemon/lines.rs)
        let (out, events) = self.buffer_lines(name, &path, lines, now_ms());
        // one append: a 'you' line and its context line never part
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = f.write_all(out.as_bytes());
        }
        for e in events {
            self.broadcast(&e);
        }
        // after the line: a client that draws the final entry and the
        // cleared live text in one frame never shows a gap (no jump)
        self.partial_clear(name, line);
    }

    /// One hub event to every client, in its typed form (`proto_on`):
    /// no client reads the older line.
    fn broadcast(&mut self, v: &Value) {
        self.proto_on(v);
    }

    fn run(&mut self, e: Effect) {
        match e {
            Effect::Journal(ev) => {
                if let Some(l) = self.journal.append(&ev.to_string()) {
                    log_line(&self.env.paths, &l);
                }
                self.photos.journaled(&ev);
                self.answers_journaled(&ev);
                // /drop (design §7.3): its tab group closes too
                if let Some(name) = crate::computer_use::archived(&ev) {
                    crate::computer_use::drop_agent(&self.proc_hub, name);
                }
            }
            Effect::Spawn { agent, resume, crash_note, wait_load } => self.spawn_on(&agent, resume, crash_note, None, wait_load),
            Effect::Place(j) => self.place_work.run(j),
            Effect::Kill { agent } => {
                if let Some(dir) = self.dir_of(&agent) {
                    self.gens.remove(&dir);
                    self.starts.ended(&dir);
                    self.start_queue.remove(&dir);
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
                self.contexts_now();
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
            Effect::Context { agent, text } => self.context_write(&agent, &text),
            Effect::Line { agent, line } => {
                self.typed_line(&line);
                self.feed(&agent, &line)
            }
            Effect::Reply { token, body } => {
                if let Some(mut s) = self.replies.remove(&token) {
                    write_json(&mut s, &body);
                }
            }
            Effect::Land { token, job } => self.land(token, *job),
            Effect::ToClient { client, body } => {
                // typed, or nothing: no client reads the older line
                let _ = self.typed_notice(client, &body) || self.rpc_effect(client, &body);
            }
            Effect::Renamed { old, new } => self.renamed(old, new),
            Effect::Retarget { from, to } => self.retarget(&from, &to),
            // sent by the loop when the gate allows (daemon/state_gate.rs)
            Effect::State => self.state_gate.changed(),
            Effect::AskRole { dir, key, request } => self.ask_role(dir, key, request),
            Effect::AskTimerName { id, request } => self.ask_timer_name(id, request),
            Effect::Confirm { card, agent: _, text } => self.on_confirm(card, &text),
            Effect::Flow { client, token, set } => {
                let (ok, text) = self.flow_cmd(set);
                if let Some(c) = client {
                    self.notice_out(c, None, &text);
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
                    self.state_now();
                }
            }
            Effect::Choose { client, agent, model, effort, default } => {
                let r = self.choose(&agent, model, effort, default);
                // a typed `model`/`effort`: an error only when refused, the
                // change shows in `agents` (architect m_10331)
                if !self.typed_outcome(client, &r) {
                    let text = r.unwrap_or_else(|e| e);
                    self.notice_out(client, None, &text);
                }
                self.state_now();
                self.switch_idle_repls();
            }
            Effect::SpawnModel { token, agent, ask, mut body } => {
                body["model"] = json!(self.spawn_model(&agent, &ask));
                if let Some(mut s) = self.replies.remove(&token) {
                    write_json(&mut s, &body);
                }
                self.state_now();
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
                self.state_now();
            }
            Effect::ModelRefused { agent, why } => self.model_refused(&agent, &why),
            Effect::Pr(e) => log_line(&self.opts.paths, &crate::forge::log_line(&e)),
            Effect::Merge { card, place, number, head, method } => self.merge_pr(card, place, number, head, method),
            Effect::Feature { token, op, name, agents } => self.feature(token, op, name, agents),
            Effect::Update { id, version } => self.update_to(id, version),
            Effect::UpdateLater { id } => versions::update_later(&id),
            Effect::BiseUpdate { update } => self.proto_note_all(&HubEv::BiseUpdate { project: self.project(), update }),
            Effect::ReleaseSwitch { id } => self.release_switch(&id),
            Effect::XDeliver { xid, project, kind, text, context, parts } => self.xdeliver(xid, project, kind, text, context, parts),
            Effect::XReply { hub, xid, text } => self.xreply_back(hub, xid, text),
            Effect::Route { rid, to, name, text, due_ms, why } => self.proto_route(HubEv::Route { project: self.project(), rid, to, name, text, correct_until_ms: due_ms, why }),
            Effect::JobEnd { agent, state, label, summary, key } => self.job_end(agent, &state, label, summary, key),
            Effect::FollowedEnd { project, agent, key, state, label, summary } => self.followed_end(project, agent, key, &state, label, summary),
            Effect::MergedChanged => self.merged_all(),
            Effect::Activity(item) => self.proto_route(HubEv::Activity { project: self.project(), item }),
            // read before the effects run (daemon/routing.rs, daemon/agent_req.rs)
            Effect::RouteAsked { .. } | Effect::Spoke { .. } => {}
            Effect::RouteDone { rid, state, to, xid, msg } => self.proto_route(HubEv::RouteDone { project: self.project(), rid, state, to, xid, msg }),
        }
    }

    /// pr-design §6.3: the user's `1` on a ready-to-merge item: `gh pr
    /// merge` with the user's login, on a thread (the network); the
    /// answer comes back as `Input::Merged`. The repo and gh are found
    /// as the poller finds them.
    fn merge_pr(&self, card: u64, place: String, number: u64, head: String, method: crate::place::MergeMethod) {
        let ws = self.opts.paths.workspace.clone();
        let log_paths = self.opts.paths.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            use crate::forge::Forge;
            let gh = crate::forge::github::GitHub::find(&crate::tools_env::hub_agent_path(&log_paths))
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
            let _ = tx.send(Msg::input(Input::Merged { card, place, number, res }));
        });
    }

    /// The PR poller (pr-design §7): started once, off the loop (it
    /// finds gh and the repo's forge on its thread); no forge, no asks.
    fn start_prs(&mut self) {
        let ws = self.opts.paths.workspace.clone();
        let log_paths = self.opts.paths.clone();
        let tx = self.tx.clone();
        let setup = Box::new(move || {
            let gh = crate::forge::github::GitHub::find(&crate::tools_env::hub_agent_path(&log_paths));
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
            let _ = tx.send(Msg::input(Input::Prs(r)));
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
        let out = crate::tools_env::git_command()
            .ok()?
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
                    "\nnot saved, and it holds no work: ask the user once in a card that blocks nothing (`sb notify`), then save the answer with `sb flow pr|trunk`. the question:\n{}",
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
        // the home workspace (docs/ambient-pages.md §5.1): no git, no flow
        let git = crate::worktree::is_git_dir(&self.opts.paths.workspace);
        let flow = self.flow_now().filter(|_| git);
        let style = self.commit_style();
        let ws = &self.opts.paths.workspace;
        let desktop = prompts::desktop_on(crate::paths::is_home(ws), crate::paths::is_registered(ws));
        if a.is_main && crate::paths::is_home(ws) {
            return prompts::bise_role(&self.hub.workspace, tmp);
        }
        if a.is_main && !git {
            return prompts::main_role(&self.hub.workspace, tmp, devflow::NO_GIT_SECTION, desktop);
        }
        if a.is_main {
            let section = devflow::main_section(flow.as_ref(), style.as_deref());
            return prompts::main_role(&self.hub.workspace, tmp, &section, desktop);
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
        prompts::task_role(a, tmp, &devflow::task_place(flow.as_ref(), &place, style.as_deref()), desktop)
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
        let (small, tx) = (small_ask::Small::of(self), self.tx.clone());
        std::thread::spawn(move || {
            let got = small.call(&req_file, &format!("role line of {}", dir));
            let line = got.ok().and_then(|t| crate::role::clean(&t));
            if let Some(l) = &line {
                write_role(small.paths(), &small.paths().agent_dir(&dir), l, &key);
            }
            let _ = tx.send(Msg::RoleLine { dir, key, line });
        });
    }

    /// One timer-name call in a thread (every_name.rs): the request goes
    /// in main's agent dir; the answer comes back as `Msg::TimerName`
    /// (None: no model, or it failed: the hub's plain fallback names it).
    fn ask_timer_name(&mut self, id: u64, request: String) {
        let adir = self.opts.paths.agent_dir(&self.dir_of(MAIN).unwrap_or_else(|| MAIN.to_string()));
        let req_file = adir.join(format!("timer-name-{id}.txt"));
        if std::fs::create_dir_all(&adir).and_then(|_| std::fs::write(&req_file, request)).is_err() {
            let _ = self.tx.send(Msg::TimerName { id, reply: None });
            return;
        }
        let (small, tx) = (small_ask::Small::of(self), self.tx.clone());
        std::thread::spawn(move || {
            let reply = small.call(&req_file, &format!("timer #{id}'s name")).ok();
            let _ = tx.send(Msg::TimerName { id, reply });
        });
    }

    fn repl_write(&mut self, agent: &str, line: &str) -> bool {
        self.contexts_now();
        let Some(dir) = self.dir_of(agent) else {
            return false;
        };
        if let Some(q) = self.switching.get_mut(&dir) {
            q.push(line.to_string());
            return true;
        }
        match self.repls.get(&dir).map(|r| r.stream.write_all(line.as_bytes())) {
            Some(Ok(())) => true,
            Some(Err(e)) => {
                log_line(&self.opts.paths, &format!("the REPL of {dir} takes no line: {e}"));
                false
            }
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
        let skip = |d: &str| !self.repls.contains_key(d) || self.switching.contains_key(d) || self.reload_repls.contains(d);
        let (app_root, paths) = (self.opts.app_root.clone(), self.opts.paths.clone());
        let mut skills = |fp: &PromptInputs| skills_fingerprint(&paths, &skill_roots(&fp.ws, &app_root, fp.main, fp.desktop));
        let skills_now: Option<&mut dyn FnMut(&PromptInputs) -> u64> = if now { Some(&mut skills) } else { None };
        let stale = prompt_inputs::stale(&self.spawn_plugins, &skip, &mut |ws| plugins_fingerprint(ws), skills_now);
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
        self.contexts_now();
        let Some(dir) = self.dir_of(agent) else {
            return;
        };
        if self.switching.contains_key(&dir) {
            return;
        }
        let Some(fp) = self.spawn_plugins.get(&dir) else {
            return;
        };
        let cur = skills_fingerprint(&self.opts.paths, &skill_roots(&fp.ws, &self.opts.app_root, fp.main, fp.desktop));
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
    /// their turn: an agent never loses a turn. Main's own recycle waits
    /// while a message is queued for it (`recycle::main_holds`); its
    /// explicit relaunches (/reload, plugins, skills, keys) do not.
    fn switch_idle_repls(&mut self) {
        self.contexts_now();
        let stale: Vec<(String, String)> = self
            .hub
            .st
            .agents
            .values()
            .filter(|a| a.run == crate::model::Run::Idle)
            .filter(|a| !self.switching.contains_key(&a.dir) && self.repls.contains_key(&a.dir))
            .filter(|a| {
                let explicit = self.reload_repls.contains(&a.dir)
                    || self
                        .bins
                        .get(&a.dir)
                        .is_some_and(|b| !Self::same_bin(b, &self.opts.repl_bin));
                let due = explicit || self.recycle.is_due(&a.dir);
                // main's for a message queued for it, any for the load
                due && !(a.is_main && self.main_holds(explicit)) && !self.load_holds(&a.dir, explicit)
            })
            .map(|a| (a.name.clone(), a.dir.clone()))
            .collect();
        // a few at a time (repl_starts): 22 at once failed a switch
        let stale = self.switch_admitted(stale);
        for (name, dir) in stale {
            let Some(r) = self.repls.get_mut(&dir) else {
                self.starts.ended(&dir);
                continue;
            };
            if r.stream.write_all(b"reload\n").is_ok() {
                self.reload_repls.remove(&dir);
                self.recycle_asked(&dir);
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

    /// What runs, for idle-exit rule 2 (`idle::busy` decides what holds
    /// the hub): the hub's own operations, and the agents' work (who is
    /// mid-turn or in `sb wait`, their background jobs, `sb every`).
    fn idle_running(&self) -> (crate::idle::Own, crate::idle::Work) {
        let mut work = crate::idle::Work {
            timers: self.hub.timers().map.len(),
            ..Default::default()
        };
        for name in &self.hub.st.order {
            let Some(a) = self.hub.st.agents.get(name) else { continue };
            if a.lifecycle != Lifecycle::Active {
                continue;
            }
            if a.run == crate::model::Run::Busy {
                work.mid_turn.push(a.name.clone());
            }
            let bg = self.opts.paths.agent_tmp(&a.dir).join("bg");
            let jobs = crate::idle::bg_jobs(&bg, crate::procs::alive)
                .into_iter()
                .map(|id| {
                    let cmd = std::fs::read_to_string(bg.join(format!("{}.cmd", id))).unwrap_or_default();
                    crate::idle::Job { name: crate::idle::cmd_name(&cmd), id }
                })
                .collect();
            work.jobs.push((a.name.clone(), jobs));
        }
        let own = crate::idle::Own {
            starting: !self.starts.is_empty(),
            switching: !self.switching.is_empty(),
            probation: crate::switch::on_probation(&self.opts.paths),
            building: !self.building.is_empty(),
            updating: self.updating.is_some(),
            release: self.release.is_some(),
        };
        (own, work)
    }

    /// idle-exit, at each tick: no UI for the grace and nothing runs,
    /// the hub stops for good (the stop of `bise --stop`).
    fn idle_check(&mut self) {
        // view.json's debounced write rides the same tick (daemon/projects.rs)
        self.view_tick();
        let uis = self.clients.len() + self.holds.count();
        // the look reads self: the watch is taken out for it
        let mut w = std::mem::replace(&mut self.idle, crate::idle::Watch::new(None, false, std::time::Instant::now()));
        let step = w.step(std::time::Instant::now(), uis, || self.idle_running());
        self.idle = w;
        match step {
            crate::idle::Step::Stay => {}
            crate::idle::Step::Say(s) => log_line(&self.opts.paths, &s),
            crate::idle::Step::Stop { log, main } => {
                log_line(&self.opts.paths, &log);
                // what the stop cut, in main's thread: he reads it at the next start
                if let Some(m) = main {
                    self.feed_rec(MAIN, Feed::info(m));
                }
                self.view_stopped();
                let _ = self.tx.send(Msg::Shutdown { keep: false });
            }
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
        self.recycle_line(dir, line);
        self.usage_line(&name, line);
        // the answer as it streams in: memory and clients, never the feed
        if line.starts_with("partial #") {
            return self.on_partial(&name, line);
        }
        if self.restored.contains(dir) {
            let restored = matches!(bise_proto::thread::lines::read(line), bise_proto::thread::lines::Rec::Obs(bise_proto::thread::lines::Obs::SessionRestored(_)));
            if restored || !line.starts_with("history ") {
                self.restored.remove(dir);
            }
            if restored || line.starts_with("history ") {
                return;
            }
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

    /// A client from before client-protocol (v2026.10.2-30) said the older
    /// `{"op":"hello"}`: the hub's exe and reload id, so an older terminal
    /// re-executes as this version (its `hello` arm follows `exe`), then
    /// the connection closes (dropping `stream`, the last handle:
    /// accept.rs reads nothing more from it). That is its refusal: no
    /// older event, no older line is ever served. Every client of this
    /// version says `initialize` (daemon/rpc.rs).
    fn older_hello(&mut self, mut stream: UnixStream, peer: &str) {
        log_line(&self.opts.paths, &format!("an older client said hello on hub.sock (before client-protocol, v2026.10.2-30): {peer}: exe and reload, then closed"));
        write_json(&mut stream, &json!({"ev": "hello", "exe": self.opts.exe.to_string_lossy(), "reload": self.reload_id}));
    }

    /// A line on a client connection: JSON-RPC (an initialized one);
    /// anything else is refused (one hub.log line naming the peer, never
    /// the line's body; a typed notice).
    fn client_line(&mut self, id: ClientId, v: Value) {
        if bise_proto::rpc::is_rpc(&v) {
            return self.rpc_line(id, v);
        }
        log_line(&self.opts.paths, &format!("hub.sock refused a line that is not JSON-RPC: {}", self.rpc_peer(id)));
        self.notice_out(id, None, "not JSON-RPC: refused");
    }
}

/// `<agent dir>/role.json`: the last role line and its key (BISE-126).
fn read_role(adir: &Path) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(adir.join("role.json")).ok()?).ok()?;
    let line = v["line"].as_str().and_then(crate::role::clean)?;
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

pub fn run(opts: Opts) -> std::io::Result<()> {
    let paths = opts.paths.clone();
    std::fs::create_dir_all(&paths.state)?;
    // under a test run's jail (BISE_TEST_HOME), a hub outside it stops here
    crate::client::jailed(&paths)?;
    // a state path too long for a unix socket: the short link first
    paths.prepare_socket()?;
    let Some(Claimed { listener, agent_listener, lease: _lease }) = claim(&paths, &opts.app_root)? else {
        eprintln!("a hub is already running for {}", paths.workspace.display());
        return Ok(());
    };
    // a hub one of its own agents relaunched is not that agent's: its
    // sb-core, builds and REPLs do not carry that tag (BISE-243)
    let proc_hub = paths.proc_hub();
    if let Ok(l) = std::env::var(crate::procs::ENV) {
        std::env::set_var(crate::procs::ENV, crate::procs::without_hub(&l, &proc_hub));
    }
    // who may say hello (docs/issues/16): the tags left are its owners'
    let doors = accept::Doors::new(paths.clone(), proc_hub.clone(), &std::env::var(crate::procs::ENV).unwrap_or_default());
    crate::tools_env::write_agent_links(&paths.bin_dir(), &opts.exe)?;
    // the agents' mktemp reads their TMPDIR (macOS's does not)
    if let Err(e) = crate::tools_env::write_mktemp_shim(&paths.bin_dir()) {
        log_line(&paths, &format!("mktemp shim not written: {}", e));
    }
    log_line(&paths, &format!("hub start pid={} workspace={}", std::process::id(), paths.workspace.display()));
    // no boot hangs silently: its steps and the replay's batches are its progress
    let boot = boot::Boot::start(&paths, "hub start (sb-core starting)");

    // the agents' tools, once, off the start path (the login shell may
    // take a moment; the first REPL spawn waits for it)
    let tools_paths = paths.clone();
    std::thread::spawn(move || {
        let p = crate::tools_env::hub_agent_path(&tools_paths);
        let rg = crate::tools_env::which("rg", &p);
        let login = match (crate::tools_env::login_path(), tools_paths.login_path_file().exists()) {
            (Some(_), _) => "read",
            (None, true) => "unavailable, the last one read kept",
            (None, false) => "unavailable",
        };
        log_line(
            &tools_paths,
            &format!(
                "agents' tools: git {}; rg {}; login-shell PATH {}; PATH={}",
                crate::tools_env::git().describe(),
                rg.map(|r| r.display().to_string()).unwrap_or_else(|| "not installed (agents use grep)".into()),
                login,
                p
            ),
        );
    });
    crate::util::timing("start (socket bound)");
    let workspace = paths.workspace.to_string_lossy().to_string();
    let mut hub = Hub::with_core(&workspace, opts.core_bin.clone());
    boot.core(hub.core_pid());
    let (mut events, unreadable) = journal_file::read(&paths.journal());
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
    boot.step(&format!("journal read ({} events)", events.len()));
    if !unreadable.is_empty() {
        log_line(&paths, &format!("journal: {} unreadable lines (not replayed), at line {}", unreadable.len(), journal_file::lines_list(&unreadable)));
    }
    // from the boot photo when one fits the journal (issue 36), else all of it
    let photos = photo_take::boot(&mut hub, &paths, &opts.core_bin, &events, !moved.is_empty(), &boot);
    boot.step("journal replayed");
    // sb-core dies (an OOM, a runtime error, killed): the hub restarts it
    // on the journal instead of dying with it (BISE-292)
    let jp = paths.clone();
    hub.set_revive(photo_take::revive(
        &paths,
        &photos,
        Box::new(move || {
            let (mut events, _) = journal_file::read(&jp.journal());
            crate::sweep::follow_moves(&mut events, &jp.worktrees);
            events
        }),
    ));
    let journal = journal_file::Out::new(std::fs::OpenOptions::new().create(true).append(true).open(paths.journal())?);

    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let (log_paths, trash_paths) = (paths.clone(), paths.clone());
    // a drop's folder rm on the trash's worker, off this loop (trash.rs)
    let trash = crate::trash::Trash::worker(Box::new(move |s| log_line(&trash_paths, s)));
    let env = GitEnv::new(&paths, Box::new(move |s| log_line(&log_paths, s)), trash);
    // the checker (approvals-design.md §4): a chat model in the role runs
    // through repl-live's one-shot, like the role lines
    let runner = crate::approvals::check::Runner::new(&bise_home::Home::from_env())
        .with_oneshot(opts.repl_bin.clone(), opts.app_root.clone(), opts.spawn_env);
    let mut sh = Shell {
        opts,
        // before `hub` moves: its split of each message (crate::spent)
        loop_watch: loop_watch::LoopWatch::start(&paths, hub.spent.clone()),
        hub,
        env,
        tx: tx.clone(),
        journal,
        repls: BTreeMap::new(),
        gens: BTreeMap::new(),
        next_gen: 1,
        pids: BTreeMap::new(),
        starts: repl_starts::new_starts(),
        start_queue: BTreeMap::new(),
        clients: BTreeMap::new(),
        replies: BTreeMap::new(),
        buffers: BTreeMap::new(),
        positions: BTreeMap::new(),
        proto: proto::Proto::default(),
        answers: answers::Answers::new(crate::paths::workspace_id(&paths.workspace)),
        rpc: rpc::Rpcs::default(),
        projects: projects::Projects::boot(&paths),
        state_gate: state_gate::StateGate::default(),
        context_gate: state_gate::StateGate::every(crate::contexts::CONTEXT_MS),
        search: search::Index::default(),
        xsearch: search::Index::default(),
        offsets: BTreeMap::new(),
        booting: false,
        bins: BTreeMap::new(),
        ports: BTreeMap::new(),
        switching: BTreeMap::new(),
        switch_spawned: BTreeSet::new(),
        restored: BTreeSet::new(),
        partials: BTreeMap::new(),
        tix: tindex::Indexes::default(),
        finds: BTreeMap::new(),
        building: BTreeSet::new(),
        updating: None,
        release: None,
        update_checked: None,
        update_told: None,
        release_checked: None,
        release_switched: None,
        resume_turn: BTreeSet::new(),
        recorders: BTreeMap::new(),
        reload_id: String::new(),
        reload_repls: BTreeSet::new(),
        spawn_keys: BTreeMap::new(),
        spawn_plugins: BTreeMap::new(),
        plugins_checked: None,
        recycle: recycling::Recycle::from_env(),
        usage: Default::default(),
        pulse: Default::default(),
        small_broken: Default::default(),
        setup: None,
        config_path: None,
        archived: BTreeSet::new(),
        proc_hub: proc_hub.clone(),
        down: BTreeSet::new(),
        gates: gate::Gates::new(
            &std::fs::read_to_string(bise_home::Home::from_env().config_file()).unwrap_or_default(),
            std::env::var(crate::approvals::mode::ENV).ok().as_deref(),
            runner,
        ),
        views: views::Views::default(),
        lands: crate::land::Queue::default(),
        features: features::Features::load(&paths.state),
        prs: None,
        cu: crate::computer_use::Watch::new(&proc_hub),
        pg: pages::PageState::load(&paths.state),
        fn_ctx: None,
        idle: {
            let config = std::fs::read_to_string(bise_home::Home::from_env().config_file()).unwrap_or_default();
            crate::idle::Watch::new(
                crate::idle::grace(std::env::var(crate::idle::ENV).ok().as_deref(), &config),
                crate::idle::waits(&config),
                std::time::Instant::now(),
            )
        },
        holds: crate::idle::Holds::default(),
        art: art::Art::default(),
        revive: revive::Revive::default(),
        wake_look: wake_look::WakeLook::start(tx.clone()),
        place_work: place_work::PlaceWork::start(&paths, tx.clone()),
        procs: processes::Worker::start(paths.clone(), proc_hub.clone(), crate::paths::workspace_id(&paths.workspace), tx.clone()),
        photos,
    };
    sh.answers_replayed(&events);
    sh.start_pages(&paths, &tx);
    sh.start_here(&paths);
    match sh.idle.grace() {
        Some(g) => log_line(&paths, &sh.idle.boot_line(g)),
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
    boot.step(&format!("transcripts read ({} buffered lines)", sh.buffers.values().map(|b| b.len()).sum::<usize>()));

    for (l, sock) in [(listener, crate::peer::Sock::Client), (agent_listener, crate::peer::Sock::Agent)] {
        let (tx, doors) = (tx.clone(), doors.clone());
        std::thread::spawn(move || accept::accept_loop(l, tx, doors, sock));
    }
    // hub-lag: one tick in the queue at most. A loop slower than the
    // clock (a big state, a loaded machine) queued a tick every 500 ms
    // anyway, and the user's lines waited behind the backlog.
    let tick_queued = std::sync::Arc::new(dispatch::TickFlag::default());
    {
        let tx = tx.clone();
        let queued = tick_queued.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            if !queued.take() {
                continue;
            }
            if tx.send(Msg::input(Input::Tick)).is_err() {
                break;
            }
        });
    }
    // expired-ux: the ChatGPT sign-in coming back (the TUI's sign-in,
    // `bise login chatgpt`): auth.json read again when it changed, once a
    // second (a stat); changed and signed in: Input::SignedIn
    // (signin_card::AuthWatch)
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let path = bise_home::Home::from_env().auth_file();
            let mut watch = crate::core::signin_card::AuthWatch::default();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let m = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                let back = watch.poll(m, || {
                    let store = bise_catalog::auth::Store::read(&path).unwrap_or_default();
                    matches!(bise_catalog::chatgpt::state(&store), bise_catalog::chatgpt::State::SignedIn { .. })
                });
                if back && tx.send(Msg::input(Input::SignedIn)).is_err() {
                    break;
                }
            }
        });
    }
    // a rollback's warning, left by the switcher for this hub
    let notice = paths.state.join("switch-notice");
    if let Ok(t) = std::fs::read_to_string(&notice) {
        let _ = std::fs::remove_file(&notice);
        sh.feed_rec(MAIN, Feed::warn(t));
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
    // mcp-apps: a view's card has no call behind it after a restart
    sh.close_view_cards();
    // land-claims: the shared folder's claims whose file has no change
    // left (stale from before, or landed while no hub ran) end, off the loop
    {
        let (tx, (dir, claims)) = (sh.tx.clone(), sh.hub.shared_claims());
        std::thread::spawn(move || land::claims_clean(&tx, &dir, &claims));
    }
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
    boot.step("boot done (REPLs spawned)");
    boot.done();

    let keep_agents = sh.serve(rx, &tick_queued);
    sh.photo_stop();
    sh.views_stop();
    // for good: no new client reaches a hub that is going (it would wait
    // for a hello that never comes); a `bise` meanwhile starts the next
    // hub, which waits for this one to be gone (`wait_previous_hub`)
    if !keep_agents {
        let _ = std::fs::remove_file(paths.socket());
        let _ = std::fs::remove_file(paths.agent_socket());
    }
    sh.flush_offsets();
    sh.contexts_now();
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
        // the processes list's rows end as stopped with their agents first
        sh.procs.quit(Duration::from_secs(2));
        // the local sites it started again are the hub's: they go with it
        sh.revive.stop_all(crate::procs::STOP_GRACE);
        let hit = crate::procs::reap(&sh.proc_hub, &crate::procs::Want::NotLive(&none), &sessions, crate::procs::STOP_GRACE);
        if !hit.is_empty() {
            log_line(&paths, &format!("processes of the agents killed: {}", crate::procs::describe(&hit)));
        }
    }
    // the next hub's socket and pid are not ours to remove
    let ours = std::fs::read_to_string(paths.pid_file()).is_ok_and(|p| p.trim() == std::process::id().to_string());
    if ours {
        if keep_agents {
            let _ = std::fs::remove_file(paths.socket());
            let _ = std::fs::remove_file(paths.agent_socket());
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

/// What a hub that won its workspace holds: its two sockets and the lease
/// on its app root.
struct Claimed {
    listener: UnixListener,
    agent_listener: UnixListener,
    lease: Option<bise_home::release::Lease>,
}

/// One hub per workspace: None when one already runs. Else this hub says
/// where it runs from (`hub.root`, read by the switcher and the clients
/// once the socket is up) and leases that root, then binds its sockets
/// and writes its pid. All of it under an exclusive flock on
/// `<state>/hub.bind.lock`, so two hubs that start at once never both
/// write `hub.root`: the second one finds the first one's socket.
fn claim(paths: &Paths, app_root: &Path) -> std::io::Result<Option<Claimed>> {
    use std::os::fd::AsRawFd;
    if UnixStream::connect(paths.socket()).is_ok() {
        return Ok(None);
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.state.join("hub.bind.lock"))?;
    // SAFETY: flock on a descriptor this function owns, released when `lock` drops
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    wait_previous_hub(paths);
    if UnixStream::connect(paths.socket()).is_ok() {
        return Ok(None);
    }
    let lease = crate::client::run_from(paths, app_root);
    let _ = std::fs::remove_file(paths.socket());
    let _ = std::fs::remove_file(paths.agent_socket());
    let listener = UnixListener::bind(paths.socket())?;
    let agent_listener = UnixListener::bind(paths.agent_socket())?;
    std::fs::write(paths.pid_file(), std::process::id().to_string())?;
    drop(lock);
    Ok(Some(Claimed { listener, agent_listener, lease }))
}

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
