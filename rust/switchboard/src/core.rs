//! The hub's decisions now run in Bend (`hub/*.bend`, the `sb-core`
//! process): this module is the Rust side of the link. `Hub::handle`
//! turns one input into a `bise_proto::core::In` for sb-core (issue 25:
//! the seam is declared once there); `Hub::core` (core_link.rs) sends it,
//! answers its git queries through `Env`, and reads the effects it returns
//! as `bise_proto::core::Fx` into `Effect`s (core_fx.rs, pure). It
//! keeps a read-only mirror of the durable state (for the views: board,
//! snapshot, contexts, prompts), fed only by the journal events and the
//! runtime changes sb-core emits.
//!
//! `sb every`'s timers are sb-core's too (bend/hub/timers.bend): their
//! journal lines replay with the rest, the tick decides the wakes. Here
//! only the request's checks and words, the first wake's time
//! (`every_set`), the `every_wake` need (wake texts, a daily timer's next
//! time on the local clock, every.rs), the ◷ lines of a set or ended
//! timer, and the mirror `st.timers` from the view.

use crate::board;
use crate::core_turn::{failed_turn_report, model_refusal};
use crate::model::*;
use crate::prompts;
use crate::role;
use crate::router::{self, UserCmd};
use crate::util::{clip, one_line};
use crate::wire::{self, Wire};
use bise_proto::core::{input as ci, link as cl, ColdMsg, In, Link, View};
use bise_proto::core::feed as feed_rec;
use bise_proto::core::Feed;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;


pub type ClientId = u64;
pub type Token = u64;

/// The side of the world the core may query synchronously.
pub trait Env {
    fn now(&self) -> u64;
    /// The workspace is a git work tree (may be remembered a few seconds).
    fn is_git(&mut self) -> bool;
    /// RFC 0002 §4.1: a worktree on `sb/<name>` for this task.
    fn worktree_create(&mut self, name: &str, with_changes: bool) -> Result<Workspace, String>;
    fn worktree_loss(&mut self, ws: &Workspace) -> Loss;
    /// RFC 0002 §5.2-5.3: save if needed, then remove. Answers the ref.
    fn worktree_drop(
        &mut self,
        name: &str,
        ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String>;
    /// RFC 0002 §5.6.
    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        snapshot: Option<&str>,
    ) -> Result<Workspace, String>;
    /// pr-design §6.4: the PR of `branch` is merged with `head` its head:
    /// a tip at `head` loses nothing at a drop (no backup, RFC 0002
    /// §5.1's squash case).
    fn pr_merged(&mut self, _branch: &str, _head: &str) {}
    /// dev-flow §5.1: a worktree on `sb/<name>` from the tip of feature
    /// `feature` (a registered one), landing on it.
    fn worktree_feature(&mut self, _name: &str, feature: &str) -> Result<Workspace, String> {
        Err(format!("no feature {}", feature))
    }
    /// bise desktop S2 (bise's home hub): the hub id of the registered
    /// project `name` (its name or its id), never this hub's own.
    fn project_hub(&self, name: &str) -> Result<String, String> {
        Err(format!("no project {}", name))
    }
}

/// A request of the `sb` CLI (RFC 0003 §4, RFC 0001 §7.2, §7.5), typed
/// once in `bise_proto::sb` (issue 27): the CLI builds it, the daemon
/// decodes it (`Req::decode`), [`Hub::agent_req`] and the daemon's file
/// ops match on it.
pub use bise_proto::sb::{parse_msg_id, Req as AgentReq};
pub use bise_proto::sb::{Every as EveryReq, Wake as WakeReq};

#[derive(Clone, Debug)]
pub enum Input {
    /// The daemon started: spawn every live agent.
    Boot,
    /// The REPL of `agent` is connected and idle.
    ReplReady {
        agent: String,
    },
    ReplLine {
        agent: String,
        line: String,
    },
    /// `--- idle`. `leftover`: steering written during the turn was still
    /// in the file (never read by the runtime).
    ReplIdle {
        agent: String,
        leftover: bool,
    },
    /// `stalled`/`loaded`/`row`: a start that made no progress, the
    /// machine at load x2+ then, that REPL's stalls in a row (sb-core's
    /// hub/stall.bend decides what they do to the crash count);
    /// `stopped`: it ended by TERM, INT or HUP, a request to stop
    /// (daemon::repl::gone_of), `row` then its stops with no connect between
    ReplExited {
        agent: String,
        crashed: bool,
        reason: String,
        stalled: bool,
        loaded: bool,
        row: u64,
        stopped: bool,
    },
    ClientHello {
        client: ClientId,
    },
    ClientInput {
        client: ClientId,
        focus: String,
        text: String,
        /// The window's "send queued": a plain text or `@agent` text waits
        /// for the end of the agent's turn (sb-core's queued-mode path).
        queued: bool,
        /// his message's parts as JSON (msg-parts, `parts_in::said`; ""
        /// none): kept on sb-core's message, written at its delivery
        parts: String,
        /// the thread he typed it in when not `focus`'s (at-start: his
        /// `@agent …` from another thread; "" none): sb-core leaves one
        /// route row there (says.trace)
        from: String,
    },
    ClientFocus {
        client: ClientId,
        focus: String,
    },
    ClientGone {
        client: ClientId,
    },
    ClientInterrupt {
        client: ClientId,
        agent: String,
    },
    /// his queued message `id` taken back (HubCmd queued_take; a client's
    /// only, issue 16): sb-core's unqueue, Rejected{"taken_back"} or a
    /// notice saying why to that client
    ClientUnqueue {
        client: ClientId,
        id: u64,
    },
    /// his queued message `id` said now (HubCmd queued_steer; a client's
    /// only): sb-core's steer_queued, delivered with its id or a notice
    ClientSteerQueued {
        client: ClientId,
        id: u64,
    },
    /// `by`: its sender, sb-core refuses it first when it may not speak as `from` (issue 37 #2)
    Agent { token: Token, from: String, req: AgentReq, by: crate::peer::Sender },
    /// A request the shell serves itself: may its sender speak as `from`? Answered by fx spoke.
    Speaks { token: Token, from: String, cmd: String, by: crate::peer::Sender },
    Tick,
    /// `/scheduled`'s stop: the user stopped timer `id`; its agent hears
    /// it from bise, once (`why`: words after the id, may be empty).
    EveryStop {
        id: u64,
        why: String,
    },
    /// `/scheduled`'s run now: timer `id` wakes its agent at once,
    /// outside its count.
    EveryRun {
        id: u64,
    },
    /// event-wake: the user stopped watch `id` (the window's wake_stop)
    WakeStop { id: u64 },
    /// event-wake: the look (daemon/wake_look.rs) saw watch `id`'s event;
    /// `text` is the wake (wake.rs `hit_text`); `rc`, `tail` (clipped
    /// once, wake.rs `tail_clip`) and `after_ms` the same facts, typed,
    /// for its thread's `wake` line (wake-ui).
    WakeHit {
        id: u64,
        text: String,
        rc: Option<i64>,
        tail: Vec<String>,
        after_ms: u64,
    },
    /// hub-loop: the shell made task `name`'s worktree (`ws`), or could
    /// not (`err`); daemon/place_work.rs, sb-core's hub/place.bend
    Placed { name: String, ws: Value, err: String },
    /// issue 32 3b: the shell removed task `name`'s worktree (`snapshot`:
    /// its saved work's ref), or could not (`err`); daemon/place_work.rs
    Unplaced { name: String, snapshot: Option<String>, err: String },
    /// The approvals gate (approvals-design.md §9): a `confirm` card for
    /// `agent`'s waiting call, in the user's inbox.
    ConfirmOpen { agent: String, text: String },
    /// Its calls are gone (interrupted): the card closes unanswered.
    ConfirmClose { card: u64, res: String },
    /// mcp-apps (docs/mcp-apps.md, C5): a view's call that needs his ok,
    /// outside `agent`'s turn: a `confirm` card for that view's `ask` (its
    /// tool and arguments, canonical JSON) that sb-core closes as timed out
    /// at its due, at a stop, at boot (never the agent's wait); its id
    /// comes back as [`Effect::ViewCard`].
    ViewConfirmOpen { agent: String, call: String, text: String, ask: String },
    /// mcp-apps: a view's `ui/message`, queued to `agent` from the sender
    /// `view` with its plugin typed.
    ViewMessage { agent: String, plugin: String, text: String },
    /// mcp-apps (d5): a view's context for `agent`'s next message from him
    /// (sb-core keeps it, joins it once, drops it at a stop): `text` the
    /// model text of `part`, its `Part::ViewContext` as JSON.
    ViewContext { agent: String, call: String, text: String, part: serde_json::Value },
    /// A page question's card answered on its page: sb-core's page_answered (its asker told unless it is `agent`).
    PageAnswered { card: u64, agent: String, page: String, text: String, block: String },
    /// A page's own card (plan-ok): sb-core's page_card opens a `kind`
    /// card asked by the first live agent of `agents`, else main; `key`:
    /// what it asks on its page, for the record; `about`: the page as an
    /// artifact (an ArtLink's JSON), on its card_opened.
    PageCard { agents: Vec<String>, kind: String, text: String, key: String, about: Option<serde_json::Value> },
    /// inbox-record: his answer's route line went into `agent`'s transcript at `pos` (daemon/answers.rs).
    AnswerPos { card: u64, agent: String, pos: u64 },
    /// The end of a role-line call (BISE-126) for the task in `dir`,
    /// asked with `key`: the new line, or None (failed: the old one stays).
    RoleLine {
        dir: String,
        key: String,
        line: Option<String>,
    },
    /// The end of a timer-name call (every_name.rs): the model's reply,
    /// or None (no model, or it failed: the plain fallback names it).
    TimerName {
        id: u64,
        reply: Option<String>,
    },
    /// An answer of the PR poller (`forge::poll`, its own thread).
    Prs(crate::forge::poll::Report),
    /// `gh pr merge`'s answer for the ready-to-merge item `card`
    /// (`Effect::Merge`, the daemon's thread): Err is gh's reason.
    Merged {
        card: u64,
        place: String,
        number: u64,
        res: Result<(), String>,
    },
    /// pr-integration: his Merge or Close on a PR's tab (`HubCmd::PrAct`,
    /// his click only): planned by `merge::plan`, run as `Effect::PrAct`.
    PrAct {
        client: ClientId,
        req: u64,
        url: String,
        head: String,
        act: bise_proto::hub::PrAct,
    },
    /// The forge's answer to `Effect::PrAct` (the daemon's thread): Err
    /// is gh's reason.
    PrActed {
        client: ClientId,
        req: u64,
        url: String,
        number: u64,
        act: bise_proto::hub::PrAct,
        res: Result<(), String>,
    },
    /// A feature step ended (dev-flow §5.1, the daemon's thread).
    Feature(FeatureDone),
    /// A land in line on a feature asks where its agent lands now
    /// (`feature::onto`): the daemon answers from the state on its loop.
    Onto { agent: String, reply: std::sync::mpsc::Sender<Option<String>> },
    /// update-flow: a look at the machine's release offer (the daemon's,
    /// every 30 s, or `/update`).
    Release(release_line::ReleaseCheck),
    /// expired-ux: the ChatGPT sign-in is back (the daemon reads
    /// auth.json): the `signin` item closes, its agents go on.
    SignedIn,
    /// bise's message to `to` (sb-core's IHubMsg): a land whose sb was gone (daemon/land.rs).
    HubMsg { to: String, text: String },
    /// land-claims: the claims of `place`'s agents whose file has no
    /// change there now (sb-core's IClaimsClean drops them): after a land
    /// in the shared folder and at boot (daemon/land.rs).
    ClaimsClean { place: String, paths: Vec<String> },
    /// desktop S2, a project hub: bise's message (`xin` on hub.sock) from
    /// bise's hub `hub`; `token` answers the caller (ack or error).
    XIn {
        token: Token,
        hub: String,
        xid: u64,
        kind: String,
        text: String,
        /// the sender's project name (J: a followed task's end, kind
        /// job_end, is said from `@<name>`; "" from bise's home hub)
        name: String,
        /// his routed message's parts (msg-parts JSON, completed in this
        /// hub's image store by `parts_in::restored`; "" none, R41 (6))
        parts: String,
    },
    /// desktop S2, bise's home hub: the project hub has outbox entry `xid`.
    XAck {
        xid: u64,
    },
    /// Its delivery failed (the hub away, a refusal): a retry later.
    XFail {
        xid: u64,
    },
    /// The project `name` answered it (`xreply` on hub.sock).
    XReply {
        xid: u64,
        name: String,
        text: String,
    },
    /// His words (issue 37 #1): held for the project he addressed (xhub.bend `addressed`),
    /// answered by [`Effect::RouteAsked`]; `text` goes there whole. `projects`: (id, name)
    /// of the registered ones (bise's home hub only, else empty).
    RouteAsk {
        focus: String,
        text: String,
        via: String,
        queued: bool,
        /// his fn context (FnContext JSON): carried as text, rendered by the project
        context: Option<Value>,
        /// his message's parts (msg-parts JSON, "" none), forwarded (R41 (6))
        parts: String,
        projects: Vec<(String, String)>,
    },
    /// A held route goes to `to` instead (the shell checked it in the
    /// registry; `bise`: a cancel). `client`: the typed connection that
    /// asked, which gets sb-core's notice when the route ended already.
    RouteCorrect {
        client: Option<ClientId>,
        rid: u64,
        to: String,
        name: String,
    },
    /// A held route doesn't go: his words reach main as today.
    RouteCancel {
        client: Option<ClientId>,
        rid: u64,
    },
    /// S10: follow a task (the typed `follow`; `client` gets the refusal).
    Follow {
        client: Option<ClientId>,
        agent: String,
        on: bool,
    },
    /// A typed command of a window (bise-proto `new`, `rename`, `model`,
    /// `effort`): the same handlers as the TUI's line, its fields never
    /// parsed again (core_user.rs `user_cmd`); `focus` the agent it's for.
    UserCmd {
        client: ClientId,
        focus: String,
        cmd: UserCmd,
    },
    /// update-flow: the end of `update now` (`Effect::Update`): Ok, the
    /// release is installed; Err, why it failed (nothing changed).
    Updated {
        version: String,
        res: Result<(), String>,
    },
    /// update-flow: `update now` (his click: `bise/update`): the release
    /// the machine offers, installed once, every project switched to it
    UpdateNow,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // short-lived, one list per input
pub enum Effect {
    /// Append to the journal (already applied to the state). The event is
    /// sb-core's JSON as is: the Rust side never decodes it (hub/gen/journal.bend
    /// does), so a new event kind needs no Rust change.
    Journal(Value),
    Spawn {
        agent: String,
        resume: bool,
        crash_note: Option<String>,
        /// admitted only at a tick under load x2 (hub/stall.bend)
        wait_load: bool,
    },
    Kill {
        agent: String,
    },
    /// hub-loop: a worktree to make or remove off the hub's loop
    /// (daemon/place_work.rs); a made one comes back as `Input::Placed`.
    Place(crate::place_job::Job),
    /// A new turn: `say <text>` on the REPL socket (the agent is idle).
    Say {
        agent: String,
        text: String,
    },
    /// Steering: appended to the steering file (the agent is busy).
    Steer {
        agent: String,
        text: String,
    },
    /// A raw REPL command line (`/compact`), the agent is idle.
    Passthrough {
        agent: String,
        line: String,
    },
    /// `by`: who asked, written in the interrupt flag so the runtime's
    /// text names them ("user": the TUI; "main" or a task: `sb
    /// interrupt`; "bise": the hub itself).
    Interrupt {
        agent: String,
        by: String,
    },
    /// Rewrite the agent's BEND_CONTEXT_FILE.
    Context {
        agent: String,
        text: String,
    },
    /// A synthetic line in the agent's feed (`sb <kind> : <text>`).
    Line {
        agent: String,
        line: String,
    },
    /// The answer to an `sb` request.
    Reply {
        token: Token,
        body: Value,
    },
    /// `sb land` (dev-flow §5): the daemon runs it in a thread, in line
    /// on its land queue, then answers `token`.
    Land {
        token: Token,
        job: Box<crate::land::Job>,
    },
    ToClient {
        client: ClientId,
        body: Value,
    },
    Renamed { old: String, new: String },
    /// name-free: another agent takes the old name `from`: pages and
    /// artifacts that still say it are `to`'s (daemon/names.rs)
    Retarget { from: String, to: String },
    /// The state changed: broadcast a snapshot.
    State,
    /// Ask a small model for a task's role line (BISE-126), off the hub's
    /// loop; the answer comes back as `Input::RoleLine`.
    AskRole {
        dir: String,
        key: String,
        request: String,
    },
    /// Ask a small model for timer `id`'s name (every_name.rs), off the
    /// hub's loop; the answer comes back as `Input::TimerName`.
    AskTimerName {
        id: u64,
        request: String,
    },
    /// The user answered a `confirm` card (approvals-design.md §9): the
    /// daemon writes the verdict to the waiting calls' gates.
    Confirm {
        card: u64,
        agent: String,
        text: String,
    },
    /// mcp-apps (C5): the card a view's call waits on, the answer to
    /// [`Input::ViewConfirmOpen`] (none: its agent is not active, no card).
    /// Read by the shell from that step's effects (daemon/views.rs).
    ViewCard {
        agent: String,
        call: String,
        card: Option<u64>,
    },
    /// mcp-apps (d5): sb-core kept a view's context or not (`why`, its
    /// words: its agent is not active), the answer to
    /// [`Input::ViewContext`]; read by its step's caller (daemon/views.rs).
    ViewContext {
        agent: String,
        call: String,
        kept: bool,
        why: String,
    },
    /// Issue #4: the answer to a spawn that asked for a model (`--model`,
    /// `--effort`, `--profile`): the daemon picks what runs (the agents
    /// default when the ask cannot), writes the task's choice file before
    /// its first call, says it in the answer `body` (`model`) and, for a
    /// fallback, in a line of the task's thread.
    SpawnModel {
        token: Token,
        agent: String,
        ask: bise_catalog::spawn::Ask,
        body: Value,
    },
    /// Issue #4: `sb send <task> --model <id> [--effort <e>]`: the task's
    /// model from its next turn (a line in its thread says who moved it);
    /// `token`: the request to answer (None: a message goes with it, its
    /// own answer is the reply).
    Switch {
        token: Option<Token>,
        from: String,
        to: String,
        model: String,
        effort: String,
    },
    /// Issue #4: the provider refused the model a task was spawned on
    /// before it ever answered (`why`: the runtime's refusal line): the
    /// daemon moves it to the agents default, says so in its thread and
    /// to main, and starts its turn again.
    ModelRefused {
        agent: String,
        why: String,
    },
    /// `/model`, `/reasoning` (BISE-135): the daemon checks the words
    /// against the catalog, writes the agent's choice (and config.toml
    /// for `default`), answers the client and broadcasts the state. No
    /// model and no effort: it says what the agent runs with.
    Choose {
        client: ClientId,
        agent: String,
        model: Option<String>,
        effort: Option<String>,
        default: bool,
    },
    /// `/flow` and `sb flow` (dev-flow §2, §7): the daemon reads the
    /// config and the detection, saves a switch, and answers the client
    /// (`client`) or the agent's request (`token`).
    Flow {
        client: Option<ClientId>,
        token: Option<Token>,
        set: Option<crate::flow::FlowMode>,
    },
    /// A PR event (pr-design §10): the daemon logs it (pr-news routes
    /// them, wave 3). The journaled ones also come as `Journal`.
    Pr(crate::forge::PrEvent),
    /// pr-integration rules 2 and 4: the hub's snapshot of a PR changed
    /// (or came first after a start): the daemon keeps its `pr` artifact
    /// in step (`forge::artifact::sync`, daemon/pr_arts.rs). `agent`: its
    /// place's first agent.
    PrArtifact { pr: Box<crate::place::PrSnapshot>, agent: Option<String> },
    /// pr-design §6.3: the user's `1` on a ready-to-merge item. The
    /// daemon runs `gh pr merge` on a thread (the head pinned) and
    /// answers with `Input::Merged`.
    Merge {
        card: u64,
        place: String,
        number: u64,
        head: String,
        method: crate::place::MergeMethod,
    },
    /// pr-integration: his click on a PR's tab, planned (`merge::plan`):
    /// the daemon runs it on a thread with his forge login (a merge with
    /// the head pinned) and answers with `Input::PrActed`.
    PrAct {
        client: ClientId,
        req: u64,
        url: String,
        number: u64,
        head: String,
        action: merge::Action,
    },
    /// The answer to his click, to that client (`HubEv::PrActed`).
    PrActed {
        client: ClientId,
        req: u64,
        url: String,
        act: bise_proto::hub::PrAct,
        err: Option<bise_proto::hub::PrActErr>,
        why: Option<String>,
    },
    /// dev-flow §5.1: a feature step (`sb feature <op> <name>`, or the
    /// user's answer on a feature item: `try`, `diff`, `later`, `keep`,
    /// `merge`, `drop`), run by the daemon in a thread; it answers
    /// `token` when an agent asked, then comes back as
    /// `Input::Feature`.
    Feature {
        token: Option<Token>,
        op: String,
        name: String,
        /// The feature's agents (live), each with its worktree.
        agents: Vec<(String, String)>,
    },
    /// update-flow: `update now`: the daemon installs release `id` once
    /// for the machine (`bise update` when needed), then writes
    /// `update-go`, and answers with `Input::Updated` (`version`: its
    /// name, for the words).
    Update {
        id: String,
        version: String,
    },
    /// update-dot: the update card's `2 later`: the daemon writes
    /// `update-later`, no card for release `id` again in any project.
    UpdateLater {
        id: String,
    },
    /// update-flow: what the clients show changed (`hub/bise_update`, the
    /// TUI's status line); None: nothing to show.
    BiseUpdate {
        update: Option<bise_proto::rows::BiseUpdate>,
    },
    /// update-flow: the machine's `update now` is installed and this hub
    /// still runs an older version: the daemon switches it to release
    /// `id` when no turn runs and no other hub is switching.
    ReleaseSwitch {
        id: String,
    },
    /// desktop S2, bise's home hub: deliver outbox entry `xid` to the
    /// project hub `project` (a hub id; started when stopped), on a
    /// thread; it comes back as `Input::XAck` or `Input::XFail`.
    XDeliver {
        xid: u64,
        project: String,
        kind: String,
        text: String,
        /// a routed message's fn context (FnContext JSON text, "" none)
        context: String,
        /// a routed message's parts (msg-parts JSON, "" none, R41 (6))
        parts: String,
    },
    /// desktop S2, a project hub: main's answer to bise's message `xid`
    /// goes back to bise's hub `hub` (`xreply` on its hub.sock).
    XReply {
        hub: String,
        xid: u64,
        text: String,
    },
    /// desktop S2 step 2: a route held (or corrected: same `rid`), for
    /// the typed clients (`route`).
    Route {
        rid: u64,
        to: String,
        name: String,
        text: String,
        due_ms: u64,
        why: String,
    },
    /// The answer to [`Input::RouteAsk`]: his words were held ([`Effect::Route`] first).
    RouteAsked { routed: bool },
    /// sb-core's word on a request's sender: `speaks`' answer, or a refused `req` alone.
    Spoke { token: Token, ok: bool, why: String },
    /// Its end: `state` "sent" (`xid` its delivery, `msg` his message
    /// it became in bise's main) or "cancelled".
    RouteDone {
        rid: u64,
        state: String,
        to: String,
        xid: Option<u64>,
        msg: Option<u64>,
    },
    /// A feature merged into the trunk (T1 run 5 step 9): today's merged
    /// list goes again to the typed connections, as after a land (a
    /// feature merge writes no `landed` line).
    MergedChanged,
    /// les bims: a message sent or a report made (crate::activity), to
    /// every typed connection.
    Activity(bise_proto::activity::ActivityEv),
    /// S10: a followed task ended (done or failed), once: his notification.
    /// `key`: the job's (its end's time here), the same in what goes back
    /// to a hub that asked (J), so a window hearing both shows one line.
    JobEnd {
        agent: String,
        state: String,
        label: String,
        summary: String,
        key: u64,
    },
    /// J, bise's home hub: a task it follows in another project ended
    /// (`project`: that hub's id); bise's main has its line already.
    FollowedEnd {
        project: String,
        agent: String,
        key: u64,
        state: String,
        label: String,
        summary: String,
    },
}


/// The sb-core executable when the app root has none: `sb-core` next to
/// the executable; a debug build (the tests) also tries the root of the
/// repository it was built from. No build-machine path in a release build
/// (BISE-163, packaging.md C4). The hub gets its sb-core from the harness
/// (`daemon::Opts::core_bin`: the app root's), never from the environment:
/// an inherited `SB_CORE_BIN` once ran a stale sb-core on every version
/// after it (5d136677).
pub use crate::core_proc::{CoreError, CoreLink};

pub fn default_core_bin() -> std::path::PathBuf {
    let next_to_exe = std::env::current_exe()
        .ok()
        .map(|e| std::fs::canonicalize(&e).unwrap_or(e))
        .and_then(|e| e.parent().map(|d| bise_home::exe_home_dir(d).join("sb-core")));
    #[cfg(debug_assertions)]
    {
        let dev = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sb-core");
        if !next_to_exe.as_ref().is_some_and(|p| p.exists()) {
            return dev;
        }
    }
    next_to_exe.unwrap_or_else(|| "sb-core".into())
}

#[derive(Clone, Debug)]
struct ClientView {
    focus: String,
    since_ms: u64,
    sent: Vec<String>,
}

pub struct Hub {
    /// The mirror of sb-core's durable state (plus the runtime fields it
    /// reports), for the views.
    pub st: State,
    pub workspace: String,
    clients: BTreeMap<ClientId, ClientView>,
    /// (time, text) of recent assistant messages, for direct-exchange excerpts.
    recent: BTreeMap<String, VecDeque<(u64, String)>>,
    /// The agents' contexts, rebuilt where their inputs changed (contexts.rs).
    contexts: crate::contexts::Contexts,
    /// The snapshot's agent rows, rebuilt where their inputs changed
    /// (snapshot_rows.rs).
    rows: crate::snapshot_rows::Rows,
    /// The last thing each agent did (from its REPL lines, for the views).
    activity: BTreeMap<String, (u64, String)>,
    /// les bims: what each agent is doing now (doing.rs, core_doing.rs);
    /// runtime only, like `activity`.
    doing: BTreeMap<String, bise_proto::rows::Doing>,
    /// Each agent's running processes: how many and their lowest port
    /// (daemon/processes.rs's worker, the one source); runtime only, like
    /// `doing`. The agents rows read it (`Agent.procs`, `procs_port`).
    pub procs: BTreeMap<String, (u16, Option<u16>)>,
    /// The role line of each task, by dir (BISE-126).
    roles: BTreeMap<String, Role>,
    /// The timers' name calls (every_name.rs); runtime only.
    timer_names: crate::every_name::Asker,
    /// BISE-136: the private worktree each agent works in, by dir (a
    /// rename keeps it); runtime only, like `activity`.
    places: BTreeMap<String, Place>,
    /// Agents whose call waits on a `confirm` card (approvals-design.md
    /// §10): shown `waiting` on `you`; runtime only, like `activity`.
    on_you: BTreeSet<String>,
    /// dev-flow §3.1: the PR of each place, by place id (pr-hub, wave 2;
    /// empty until then) and its rank in the land queue's line (set by the
    /// daemon; its lid `waits to land · 2nd` is `land_queue::lid` of it).
    pub prs: BTreeMap<String, crate::place::PrSnapshot>,
    pub landing: BTreeMap<String, u32>,
    /// The repo's `[flow] mode` (None: not set), for the views' held
    /// header (`lands via PRs` / `lands on main`); the daemon reads it.
    pub flow: Option<crate::flow::FlowMode>,
    /// dev-flow §5.1, set by the daemon: each feature's held lid by place
    /// id (`feature · 14 commits · not tried`), and the ones whose try
    /// build builds or is on trial (the `Δ`). Runtime only.
    pub feature_lids: BTreeMap<String, String>,
    pub trying: BTreeSet<String>,
    /// inbox-reappear: his answer's feature step running, by feature: the
    /// card he answered (feature_choice's merge, try, drop), until its
    /// `Input::Feature` ends it. Runtime only (`running_cards`).
    feature_answers: BTreeMap<String, u64>,
    /// pr-hub (pr-design §7-§10), runtime but `known`: what the journal
    /// says of each place's PR (its number; `pr_*` lines, read back by
    /// `replay`); `pr_lids`: the held line of a worktree with no PR yet
    /// (`no PR yet · 2 commits`; the land queue's `lids` win);
    /// `pr_ok_ms`: when the forge last answered for a place; `pr_late`:
    /// its last ask failed (the boxes go faint); `pr_done`: places whose
    /// merged PR was cleaned up (or kept, said once).
    pr_known: BTreeMap<String, crate::forge::Known>,
    pr_lids: BTreeMap<String, String>,
    pr_ok_ms: BTreeMap<String, u64>,
    pub pr_late: Option<crate::forge::ForgeError>,
    pr_done: BTreeSet<String>,
    /// pr-integration rule 2: the PRs followed by number (an agent only
    /// reviews them: their artifact, no place), by their link's one form,
    /// as the last answer said; `followed_ok_ms` its time. Runtime only:
    /// no news, no cleanup, only their artifact's status and archive.
    followed: BTreeMap<String, crate::place::PrSnapshot>,
    followed_ok_ms: u64,
    /// pr-news (pr-design §6): who owns each PR, what was passed on, the
    /// checks' tries (forge/news.rs); `pr_bots`: the repo's `[pr]
    /// trusted_bots` (the daemon reads config.toml).
    pr_news: crate::forge::news::News,
    pub pr_bots: Vec<String>,
    /// pr-merge (pr-design §6.3): the ready-to-merge items' runtime side
    /// (merge.rs).
    merges: merge::Merges,
    /// pr-integration rule 6: the "your review" items (core_review.rs).
    reviews: review::Reviews,
    /// update-flow: a newer bise, this hub's side (release_line.rs).
    updates: release_line::Updates,
    /// expired-ux: the agents waiting for the ChatGPT sign-in
    /// (signin_card.rs).
    signin: signin_card::SignIn,
    dirty: bool,
    link: CoreLink,
    /// hub-loop: where the loop's message spends its time (spent.rs)
    pub spent: crate::spent::Spent,
    /// The sb-core it runs (and starts again after a death).
    core_bin: PathBuf,
    /// How to bring sb-core back when it dies (the daemon's; none: a
    /// death stops the hub, `gave_up`, as at boot and in most tests).
    revive: Option<Revive>,
    /// Lines for main's feed from a revival, out at the next input.
    revived: Vec<String>,
    /// sb-core could not be brought back (no revival, or REVIVE_LIMIT
    /// restarts in REVIVE_WINDOW_MS): why. The daemon's loop stops on it.
    gave_up: Option<String>,
    /// main's feed said an sb-core `{error}` already; a step that works
    /// clears it (one warning per run of errors, never one per Tick).
    error_told: bool,
    /// From a step's answer: his message's id (A1, `msg`) for `turn/send`;
    /// his answer to a card (inbox picks (3), `ans_*`) for `card/answer`
    pub said: Option<u64>,
    pub answered: Option<bise_proto::rpc::CardAnswer>,
    /// Who asked for sb-core's interrupt; none: the hub's (`Effect::Interrupt::by`).
    interrupt_by: Option<String>,
    /// Issue #4, set by the daemon from the choice files: each agent's
    /// model for `sb list`, `sb tasks` and the roster.
    pub models: board::Models,
    /// Issue #4: the tasks on a model asked at their spawn (or by `sb
    /// send --model`) that has not answered a turn yet: a provider that
    /// refuses it moves them to the agents default (`Effect::ModelRefused`).
    /// Set by the daemon, cleared at the agent's first turn end.
    pub model_trial: BTreeSet<String>,
    /// The spawns that asked for a model, by request token: their answer
    /// waits for the daemon's pick (`Effect::SpawnModel`).
    spawn_asks: BTreeMap<Token, bise_catalog::spawn::Ask>,
}

/// More restarts than this in `REVIVE_WINDOW_MS`: sb-core dies on its
/// state, not by accident; the hub stops (as before BISE-292).
/// A PR the forge has not answered about for this long shows as stale
/// (pr-design §7; pr-hub sets it from its cadence).
pub const PR_STALE_MS: u64 = 10 * 60 * 1000;

const REVIVE_LIMIT: usize = 3;
/// Journal events per `replay_many` line at boot (~110 KB). On a 112k-event
/// journal (retired instructions, main.bend `cut`): 100 per line 118G, 250
/// 114G, 500 112G; cycles flat from 100 to 500, rising past 1000.
const REPLAY_BATCH: usize = 250;
const REVIVE_WINDOW_MS: u64 = 60_000;

type Fx = Vec<Effect>;

/// Where an agent said it works (BISE-136), or where its bash went.
#[derive(Clone, Debug, Default)]
struct Place {
    /// The private worktree; "" = the agent's own workspace.
    path: String,
    /// It came from `sb worktree` (gate.sh): the bash fallback no
    /// longer guesses for this agent.
    told: bool,
    /// The branch checked out there, as the PR poller last read it
    /// (None: detached, as `gate.sh new` makes it, or not read yet).
    branch: Option<String>,
}

/// A task's role line (BISE-126) and its calls.
#[derive(Clone, Debug, Default)]
struct Role {
    /// The last line a model gave; None: the objective's first sentence.
    line: Option<String>,
    /// What `line` was made from (role::key).
    key: String,
    /// The key of the call in flight.
    asking: Option<String>,
    /// A turn ended during the call: look again when it is over.
    pending: bool,
    /// The last failed call: no new one for ROLE_RETRY_MS.
    failed_ms: Option<u64>,
}

/// After a failed role-line call, the next one waits this long.
const ROLE_RETRY_MS: u64 = 5 * 60 * 1000;

/// A record of an agent's feed, as its line (`sb <kind> :: <json>`,
/// issue 25): every line the hub writes itself.
pub fn feed(agent: &str, rec: bise_proto::core::Feed) -> Effect {
    Effect::Line { agent: agent.to_string(), line: rec.line() }
}

fn notice(client: ClientId, text: &str) -> Effect {
    Effect::ToClient {
        client,
        body: json!({"ev": "notice", "text": text}),
    }
}

fn parse<T: serde::de::DeserializeOwned>(x: &Value) -> Option<T> {
    T::deserialize(x).ok()
}

impl Hub {
    /// The tests' hub: their sb-core is the test setting `SB_CORE_BIN`
    /// (the gate's cache file), else [`default_core_bin`].
    #[cfg(test)]
    pub fn new(workspace: &str) -> Hub {
        let bin = bise_home::env::test_setting("SB_CORE_BIN").map(PathBuf::from).unwrap_or_else(default_core_bin);
        // a test on an sb-core of other sources lies both ways (core_fresh.rs)
        crate::core_fresh::check(&bin);
        Hub::with_core(workspace, bin).unwrap_or_else(|e| panic!("sb-core: {}", e))
    }

    /// A hub whose decisions run in the sb-core at `core_bin`; an error
    /// when it does not start or answer (the boot says it and exits).
    pub fn with_core(workspace: &str, core_bin: PathBuf) -> Result<Hub, CoreError> {
        let mut link = CoreLink::start(&core_bin)?;
        link.call(&Link::Init(cl::Init { workspace: workspace.into() }).to_value())?;
        let mut hub = Hub {
            st: State::new(workspace),
            workspace: workspace.to_string(),
            clients: BTreeMap::new(),
            recent: BTreeMap::new(),
            contexts: Default::default(),
            rows: Default::default(),
            activity: BTreeMap::new(),
            doing: BTreeMap::new(),
            procs: BTreeMap::new(),
            roles: BTreeMap::new(),
            timer_names: Default::default(),
            places: BTreeMap::new(),
            on_you: BTreeSet::new(),
            prs: BTreeMap::new(),
            followed: BTreeMap::new(),
            followed_ok_ms: 0,
            landing: BTreeMap::new(),
            flow: None,
            feature_lids: BTreeMap::new(),
            trying: BTreeSet::new(),
            feature_answers: BTreeMap::new(),
            pr_known: BTreeMap::new(),
            pr_lids: BTreeMap::new(),
            pr_ok_ms: BTreeMap::new(),
            pr_late: None,
            pr_done: BTreeSet::new(),
            pr_news: Default::default(),
            pr_bots: Vec::new(),
            merges: merge::Merges::default(),
            reviews: review::Reviews::default(),
            updates: release_line::Updates::default(),
            signin: signin_card::SignIn::default(),
            dirty: false,
            link,
            spent: Default::default(),
            core_bin,
            revive: None,
            revived: Vec::new(),
            gave_up: None,
            error_told: false,
            said: None,
            answered: None,
            interrupt_by: None,
            models: BTreeMap::new(),
            model_trial: BTreeSet::new(),
            spawn_asks: BTreeMap::new(),
        };
        let out = hub.try_raw(&Link::ViewAll(cl::ViewAll {}).to_value())?;
        hub.load_answer_view(&out);
        Ok(hub)
    }

    /// Tests only: put an agent's REPL in a given state (never sent by
    /// the daemon).
    #[cfg(test)]
    pub fn force_run(&mut self, agent: &str, run: Run) {
        if let Some(out) = self.call(&force_run_input(agent, run)) {
            self.load_answer_view(&out);
        }
    }

    /// Rebuild the durable state from the journal: sb-core replays it,
    /// then sends the whole state. Returns the events sb-core did not
    /// apply (a kind it does not know: the journal was written by a newer
    /// hub before a /version rollback), for the caller to log.
    /// Batches of REPLAY_BATCH events (`replay_many`): one event per call
    /// was quadratic in the messages (a 20k-event journal took ~18 s at
    /// boot, past the switcher's wait). Tests only: the boot and a
    /// revival go through `rebuild`.
    #[cfg(test)]
    pub fn replay(&mut self, events: &[Value]) -> Vec<Value> {
        self.replay_with(events, &mut |_, _| {}).unwrap_or_else(|e| panic!("sb-core: {}", e))
    }

    /// `replay`, telling `progress(done, total)` after each batch (the
    /// boot watch, daemon/boot.rs: a long replay is progress, not a hang).
    /// sb-core failing a batch (dead, late, a bad line): the error.
    pub(super) fn replay_with(&mut self, events: &[Value], progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Value>, CoreError> {
        let mut skipped = Vec::new();
        let mut core: Vec<&Value> = Vec::new();
        for ev in events {
            // the hub's own lines (the PR numbers): never sb-core's
            if crate::forge::is_pr_line(ev) {
                crate::forge::read_line(&mut self.pr_known, ev);
                self.pr_news.read_journal(ev);
                continue;
            }
            core.push(ev);
        }
        let (mut done, total) = (0, core.len());
        for batch in core.chunks(REPLAY_BATCH) {
            let evs = batch.iter().map(|ev| (*ev).clone()).collect();
            let out = self.try_raw(&Link::ReplayMany(cl::ReplayMany { evs }).to_value())?;
            for i in core_fx::skipped_of(&core_fx::answer(&out)) {
                if let Some(ev) = batch.get(i as usize) {
                    skipped.push((*ev).clone());
                }
            }
            done += batch.len();
            progress(done, total);
        }
        let out = self.try_raw(&Link::ViewAll(cl::ViewAll {}).to_value())?;
        self.load_answer_view(&out);
        Ok(skipped)
    }

    #[cfg(test)]
    fn view_all(&mut self) {
        if let Some(out) = self.call(&Link::ViewAll(cl::ViewAll {}).to_value()) {
            self.load_answer_view(&out);
        }
    }

    /// view-all-perf: the bodies of the cold messages of `ids` (view_all
    /// sent them without), asked of sb-core (`msg_bodies`, at most
    /// MSG_BODIES_MAX ids a call) and loaded: they are whole after. A
    /// whole or unknown id costs nothing; on the loop only, like a step.
    pub fn fetch_bodies(&mut self, ids: impl IntoIterator<Item = u64>) {
        let cold = self.st.cold_of(ids);
        for batch in cold.chunks(cl::MSG_BODIES_MAX) {
            if let Some(out) = self.call(&Link::MsgBodies(cl::MsgBodies { ids: batch.to_vec() }).to_value()) {
                self.load_answer_view(&out);
            }
        }
    }

    /// The view an answer of sb-core carries (core_fx::view_of), loaded;
    /// none (a newer sb-core's shape): the mirror stays as it is.
    fn load_answer_view(&mut self, out: &Value) {
        match core_fx::view_of(&core_fx::answer(out)) {
            Some(v) => self.load_view(v),
            None => eprintln!("sb-core: an answer without a view (skipped): {}", out),
        }
    }

    /// The agents a client is looking at (a REPL start for them goes
    /// first, daemon/repl_starts.rs).
    pub fn focused(&self) -> BTreeSet<String> {
        self.clients.values().map(|c| c.focus.clone()).collect()
    }

    /// sb-core's pid (a stuck boot stops it with the hub).
    pub fn core_pid(&self) -> u32 {
        self.link.child.id()
    }

    /// Restart sb-core when it dies (see `Revive`); the daemon sets it
    /// once the journal is replayed.
    pub fn set_revive(&mut self, r: Revive) {
        self.revive = Some(r);
    }

    /// Tests only: kill sb-core under the hub.
    #[cfg(test)]
    pub fn kill_core(&mut self) {
        self.link.kill();
    }

    /// One call to sb-core, with no revival: its failure is the caller's
    /// (the boot's, a revival's).
    fn try_raw(&mut self, v: &Value) -> Result<Value, CoreError> {
        self.link.call(v)
    }

    /// Tests only: `try_raw`, its failure a panic.
    #[cfg(test)]
    fn raw(&mut self, v: &Value) -> Value {
        self.try_raw(v).unwrap_or_else(|e| panic!("sb-core: {}", e))
    }

    /// sb-core could not be brought back: why. The daemon's loop stops
    /// on it (one hub.log line, the normal stop, a non-zero exit).
    pub fn gave_up(&self) -> Option<&str> {
        self.gave_up.as_deref()
    }

    /// Tests only: how long sb-core may take (a fake that sleeps).
    #[cfg(test)]
    pub fn set_core_limits(&mut self, l: crate::core_proc::Limits) {
        self.link.limits = l;
    }

    /// The agent's last activity, for the views (not a decision).
    fn set_activity(&mut self, agent: &str, now: u64, what: String) {
        if let Some(a) = self.st.agents.get_mut(agent) {
            a.activity = Some((now, what.clone()));
            self.activity.insert(agent.to_string(), (now, what));
        }
    }
    /// The approvals gate (approvals-design.md §10): `agent`'s call waits
    /// on the user (a card), or no longer does. The views say it at once.
    pub fn set_on_you(&mut self, agent: &str, on: bool) {
        let changed = if on { self.on_you.insert(agent.to_string()) } else { self.on_you.remove(agent) };
        if let Some(a) = self.st.agents.get_mut(agent).filter(|_| changed) {
            a.waiting = on;
            a.waiting_on = on.then(|| "you".to_string());
        }
    }

    /// BISE-136: the private worktree `a` works in (None: its own
    /// workspace, shared checkout or hub worktree).
    /// `sb move <agent> <place>`: the target's ws (None: a new worktree,
    /// /isolate's path). An agent alone in its worktree does not leave
    /// it: the folder would be left behind with no one (drop it instead).
    fn move_target(&self, agent: &str, place: &str) -> Result<Option<Workspace>, String> {
        let name = self.st.resolve(agent).ok_or_else(|| format!("no agent named {}", agent))?;
        let a = &self.st.agents[&name];
        let target = match place {
            "new" => None,
            crate::place::SHARED => self.st.agents.get(MAIN).map(|m| m.ws.clone()),
            p => Some(crate::place::find_worktree(&self.st, p)?),
        };
        if a.ws.mode == Mode::Worktree && !a.ws.dropped {
            let id = a.ws.place_id(&a.dir);
            let others = crate::place::places(&self.st, &BTreeMap::new())
                .into_iter()
                .find(|p| p.id == id)
                .is_some_and(|p| p.agents.iter().any(|n| *n != name));
            let same = target.as_ref().is_some_and(|w| w.place_id(&a.dir) == id);
            if !others && !same {
                return Err(format!(
                    "@{} is alone in its worktree: moving it would leave the folder behind; drop it instead",
                    name
                ));
            }
        }
        Ok(target)
    }

    /// land-claims: the shared folder's path and its live agents' claims,
    /// as the state keeps them (the boot's look for claims that are over).
    pub fn shared_claims(&self) -> (std::path::PathBuf, Vec<String>) {
        let mut claims: Vec<String> = Vec::new();
        for a in self.st.agents.values().filter(|a| a.lifecycle != Lifecycle::Archived) {
            if a.ws.place_id(&a.dir) == crate::place::SHARED {
                claims.extend(a.files.iter().filter(|f| !claims.contains(f)).cloned().collect::<Vec<_>>());
            }
        }
        (std::path::PathBuf::from(&self.workspace), claims)
    }

    /// `sb land` (dev-flow §5): what the land needs from the state. The
    /// daemon adds the repo's flow and runs it (`land::run`).
    fn land_job(&self, from: &str, here: bool, message: &str, add: Vec<String>) -> Result<crate::land::Job, String> {
        let name = self.st.resolve(from).ok_or_else(|| format!("unknown agent: {}", from))?;
        let a = &self.st.agents[&name];
        let place = a.ws.place_id(&a.dir);
        let others = self
            .st
            .agents
            .values()
            .filter(|b| b.name != name && b.lifecycle != Lifecycle::Archived)
            .filter(|b| b.ws.place_id(&b.dir) == place)
            .map(|b| (b.name.clone(), b.files.to_vec()))
            .collect();
        Ok(crate::land::Job {
            agent: name.clone(),
            here,
            message: message.trim().to_string(),
            worktree: a.ws.mode == Mode::Worktree,
            place,
            dir: std::path::PathBuf::from(&a.ws.path),
            shared: std::path::PathBuf::from(&self.workspace),
            files: a.files.to_vec(),
            others,
            add,
            since_ms: a.created_ms,
            flow: crate::flow::FlowConfig::default(),
            onto: feature::onto(&a.ws), // a feature's agent lands on the feature (dev-flow §5.1)
            checks: std::path::PathBuf::new(),
            lands: std::path::PathBuf::new(),
        })
    }

    /// `sb every`'s timers (the state's `timers`, the pages' `watch`).
    pub fn timers(&self) -> &crate::every::Timers {
        &self.st.timers
    }

    /// `sb every` (every.rs): set, list or stop a timer; the reply. The
    /// request's checks and words are here; the timer is sb-core's
    /// (`every_set`, `every_stop` inputs, hub/timers.bend).
    fn timer_req(&mut self, fx: &mut Fx, env: &mut dyn Env, from: &str, r: EveryReq) -> Value {
        let now = env.now();
        let Some(by) = self.st.resolve(from) else {
            return json!({"ok": false, "error": format!("unknown agent: {}", from)});
        };
        match r {
            EveryReq::List => json!({"ok": true, "text": self.st.timers.list(now)}),
            EveryReq::Show { id } => match self.st.timers.map.get(&id) {
                Some(t) => json!({"ok": true, "text": crate::every::show(t, now)}),
                None => json!({"ok": false, "error": format!("no timer #{} (`sb every` lists them)", id)}),
            },
            EveryReq::Stop { id } => {
                let no = json!({"ok": false, "error": format!("no timer #{} (`sb every` lists them)", id)});
                if !self.st.timers.map.contains_key(&id) {
                    return no;
                }
                let why = format!("stopped by {}", by);
                self.core(fx, env, None, In::EveryStop(ci::EveryStop { id, why, wake: String::new() }));
                if self.st.timers.map.contains_key(&id) {
                    return no;
                }
                json!({"ok": true, "text": format!("timer #{} stopped", id)})
            },
            EveryReq::Add { to, text, every_ms, daily_min, until_ms, times, page, name } => {
                let sched = match crate::every::Sched::of_wire(every_ms, daily_min) {
                    Ok(s) => s,
                    Err(e) => return json!({"ok": false, "error": e}),
                };
                let to = if to.is_empty() { by.clone() } else { to };
                let agent = match self.st.resolve(&to) {
                    Some(a) if self.st.agents[&a].lifecycle == Lifecycle::Active => a,
                    _ => return json!({"ok": false, "error": format!("sb every: no active agent @{}", to)}),
                };
                if matches!(sched, crate::every::Sched::Every(p) if p < crate::every::min_ms(bise_home::env::test_setting("SB_EVERY_MIN_MS").as_deref())) {
                    return json!({"ok": false, "error": "sb every: at least 1m between two wakes"});
                }
                if text.trim().is_empty() {
                    return json!({"ok": false, "error": "sb every: the message is empty"});
                }
                if until_ms.is_some_and(|u| u <= now) {
                    return json!({"ok": false, "error": "sb every: --until is already past"});
                }
                if times == Some(0) {
                    return json!({"ok": false, "error": "sb every: --times is at least 1"});
                }
                let n = crate::every::New { agent, by, text: text.trim().to_string(), sched, until_ms, times, page, name };
                // sb-core gives the id: its every_set journal line's
                let k = fx.len();
                self.core(fx, env, None, n.input(now));
                let set_id = |e: &Effect| match e {
                    Effect::Journal(j) if j["type"] == "every_set" => j["id"].as_u64(),
                    _ => None,
                };
                let Some(id) = fx[k..].iter().find_map(set_id) else {
                    return json!({"ok": false, "error": format!("sb every: no active agent @{}", to)});
                };
                let line = self.st.timers.list(now).lines().find(|l| l.starts_with(&format!("#{} ", id))).unwrap_or_default().to_string();
                json!({"ok": true, "id": id, "text": format!("timer set: {} (stop it: sb every --stop {})", line, id)})
            }
        }
    }

    /// A timer's line in the feeds (the `scheduled` record, the TUI's ◷
    /// line): its agent's feed, and main's when main set it for another
    /// agent (main's thread shows the timers main sets, never another
    /// agent's wakes). `ev`: the timer's state json with `ev` set/end.
    fn timer_lines(&self, fx: &mut Fx, agent: &str, by: &str, ev: &Value) {
        let rec = |timer: Value| Feed::Scheduled(feed_rec::Scheduled { v: 1, timer });
        fx.push(feed(agent, rec(ev.clone())));
        if by == MAIN && agent != MAIN {
            // main's copy says so: its ended line names the agent
            let mut m = ev.clone();
            m["in"] = json!(MAIN);
            fx.push(feed(MAIN, rec(m)));
        }
    }

    /// The ◷ line of a timer sb-core just set or ended (its `every_set` or
    /// `every_stop`; the step's view, loaded first, has the timer).
    fn timer_journal_lines(&self, fx: &mut Fx, ev: &Value) {
        let id = ev["id"].as_u64().unwrap_or(0);
        let (t, mut v, what) = match ev["type"].as_str() {
            Some("every_set" | "every_name") if crate::every_name::set_line_now(ev) => match self.st.timers.map.get(&id) {
                Some(t) => (t, t.json(), "set"),
                None => return,
            },
            Some("every_stop") => match self.st.timers.ended.iter().rev().find(|e| e.timer.id == id) {
                Some(e) => (&e.timer, e.json(), "end"),
                None => return,
            },
            // a refused wake (fire_is_a_message): tried again in a minute
            Some("every_fired") if ev["undelivered"] == true => {
                if let Some(t) = self.st.timers.map.get(&id) {
                    let w = format!("timer #{id}: its wake did not reach @{}; tried again in a minute", t.agent);
                    fx.push(feed(MAIN, Feed::warn(w)));
                }
                return;
            }
            _ => return,
        };
        v["ev"] = json!(what);
        self.timer_lines(fx, &t.agent, &t.by, &v);
    }

    /// `/scheduled`'s stop (the user): the timer ends, its agent hears it
    /// from bise once in the same sb-core step, the ◷ line says so.
    fn timer_stop_by_user(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64, why: &str) {
        let Some(t) = self.st.timers.map.get(&id) else { return };
        let wake = crate::every::stop_note(id, &t.text, why);
        self.core(fx, env, None, In::EveryStop(ci::EveryStop { id, why: "stopped by the user".into(), wake }));
    }

    /// `/scheduled`'s run now: one wake of timer `id` at once, outside its
    /// count, through sb-core's only wake path (timer_fire: never a second
    /// wake while one from bise is queued).
    fn timer_run_now(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64) {
        if let Some(text) = self.st.timers.run_text(id, env.now()) {
            self.core(fx, env, None, In::EveryRun(ci::EveryRun { id, text }));
        }
    }

    fn place_of(&self, a: &Agent) -> Option<String> {
        let p = &self.places.get(&a.dir)?.path;
        (!p.is_empty() && *p != a.ws.path).then(|| p.clone())
    }

    /// The branch checked out in `a`'s private worktree, as the PR
    /// poller last read it (None: detached, or none read yet).
    fn place_branch_of(&self, a: &Agent) -> Option<String> {
        self.place_of(a)?;
        self.places.get(&a.dir)?.branch.clone()
    }

    /// BISE-136: the PR poller read what private worktree `path` has
    /// checked out (`branch` "": detached). A change: its agents' places
    /// carry it (the views, the next plan asks the forge about it).
    fn private_branch_in(&mut self, path: &str, branch: &str) {
        let branch = (!branch.is_empty()).then(|| branch.to_string());
        let mut changed = false;
        for p in self.places.values_mut().filter(|p| p.path == path) {
            if p.branch != branch {
                p.branch = branch.clone();
                changed = true;
            }
        }
        if !changed {
            return;
        }
        for a in self.st.agents.values_mut().filter(|a| a.place.as_deref() == Some(path)) {
            a.place_branch = branch.clone();
        }
        self.dirty = true;
    }

    /// `id` is a private worktree with no branch checked out (as far as
    /// the poller read).
    fn detached_private(&self, id: &str) -> bool {
        crate::place::private_path(id).is_some_and(|path| {
            let mut ps = self.places.values().filter(|p| p.path == path).peekable();
            ps.peek().is_some() && ps.all(|p| p.branch.is_none())
        })
    }

    /// Record where `agent` works; `told`: from `sb worktree` (else the
    /// bash fallback, ignored once the agent told).
    fn set_place(&mut self, agent: &str, path: String, told: bool) {
        let Some(dir) = self.st.agents.get(agent).map(|a| a.dir.clone()) else {
            return;
        };
        if !told && self.places.get(&dir).is_some_and(|p| p.told || p.path == path) {
            return;
        }
        // the same folder keeps the branch read; another is read anew
        let branch = self.places.get(&dir).filter(|p| p.path == path).and_then(|p| p.branch.clone());
        self.places.insert(dir, Place { path, told, branch });
        if let Some(a) = self.st.agents.get(agent).cloned() {
            let (place, branch) = (self.place_of(&a), self.place_branch_of(&a));
            if let Some(a) = self.st.agents.get_mut(agent) {
                a.place = place;
                a.place_branch = branch;
            }
        }
        self.dirty = true;
    }

    /// A client is attached (the PR poller's slow cadence when none).
    pub fn has_clients(&self) -> bool {
        !self.clients.is_empty()
    }

    /// The branches the PR poller follows: each worktree place's.
    pub fn pr_watches(&self) -> Vec<crate::forge::poll::Watch> {
        crate::place::places(&self.st, &self.prs)
            .into_iter()
            .filter(|p| p.kind == crate::place::PlaceKind::Worktree)
            .filter_map(|p| {
                // BISE-136: a private worktree is followed by what it has
                // checked out (its branch "" while detached or not read)
                let head = crate::place::private_path(&p.id).is_some();
                let branch = if head { p.branch.unwrap_or_default() } else { p.branch? };
                Some(crate::forge::poll::Watch { branch, place: p.id, path: p.path, base: p.base, head })
            })
            .collect()
    }

    /// An answer of the PR poller (pr-design §7-§10): the snapshots by
    /// place, the events (journaled when they carry the number), the
    /// held line of a place with no PR yet, a merged PR's cleanup.
    fn prs_in(&mut self, fx: &mut Fx, env: &mut dyn Env, r: crate::forge::poll::Report) {
        use crate::forge::{self, PrEvent};
        // BISE-136: what each private worktree has checked out first (its
        // place's branch, the PR below is matched against it)
        for l in &r.local {
            if let Some(path) = crate::place::private_path(&l.place) {
                self.private_branch_in(path, &l.branch);
            }
        }
        let places = crate::place::places(&self.st, &self.prs);
        let place = |id: &str| places.iter().find(|p| p.id == id && p.kind == crate::place::PlaceKind::Worktree);
        for l in &r.local {
            if place(&l.place).is_none() {
                continue;
            }
            // designer's call 8: a detached private worktree with no
            // commit of its own has nothing to say (no held line)
            let detached = crate::place::private_path(&l.place).is_some() && l.branch.is_empty();
            match l.commits {
                Some(0) if detached => {
                    if self.pr_lids.remove(&l.place).is_some() {
                        self.dirty = true;
                    }
                }
                Some(n) => {
                    let lid = forge::no_pr_lid(n);
                    if self.pr_lids.get(&l.place) != Some(&lid) {
                        self.pr_lids.insert(l.place.clone(), lid);
                        self.dirty = true;
                    }
                }
                None => {}
            }
        }
        let prs = match r.prs {
            None => return,
            Some(Err(e)) => {
                if self.pr_late.is_none() {
                    fx.push(Effect::Pr(PrEvent::Unreachable { error: e.clone() }));
                    self.dirty = true;
                }
                self.gh_off(fx, &e);
                self.pr_late = Some(e);
                return;
            }
            Some(Ok(prs)) => prs,
        };
        if self.pr_late.take().is_some() {
            self.dirty = true;
        }
        for l in &r.local {
            let Some(p) = place(&l.place) else { continue };
            let known = self.pr_known.get(&l.place).copied();
            let new = prs.iter().find(|pr| pr.branch == l.branch).filter(|pr| forge::belongs(pr, known, l.tip.as_deref()));
            for e in forge::diff(&l.place, self.prs.get(&l.place), new, known) {
                // the repo's first PR (the journal knew none): its line says more
                let first = self.pr_known.is_empty();
                if let Some(j) = forge::journal_line(&e) {
                    forge::read_line(&mut self.pr_known, &j);
                    fx.push(Effect::Journal(j));
                }
                let at = forge::news::At { place: &p.id, branch: &l.branch, agents: &p.agents };
                let outs = self.pr_news.event(&e, Some(&at), first);
                self.news_out(fx, env, outs);
                fx.push(Effect::Pr(e));
            }
            self.pr_ok_ms.insert(l.place.clone(), r.at_ms);
            match new {
                Some(pr) => {
                    if self.prs.get(&l.place) != Some(pr) {
                        self.prs.insert(l.place.clone(), pr.clone());
                        self.dirty = true;
                        fx.push(Effect::PrArtifact { pr: Box::new(pr.clone()), agent: p.agents.first().cloned() });
                    }
                }
                None => {
                    if self.prs.remove(&l.place).is_some() {
                        self.dirty = true;
                    }
                }
            }
            // a private worktree is its agent's (gate.sh done removes
            // it): a merge archives no one there
            let private = crate::place::private_path(&l.place).is_some();
            if let Some(pr) = new.filter(|pr| pr.state == crate::place::PrState::Merged && !private) {
                let agents = p.agents.clone();
                self.pr_cleanup(fx, env, l, pr, &agents);
            }
        }
        self.followed_in(fx, r.at_ms, &r.followed);
        // pr-news: the reviews, comments and failing checks of the PRs
        // that changed, to the agent that owns each
        for (branch, act) in &r.activity {
            let Some(l) = r.local.iter().find(|l| &l.branch == branch) else { continue };
            let (Some(p), Some(pr)) = (place(&l.place), self.prs.get(&l.place).cloned()) else { continue };
            let at = forge::news::At { place: &p.id, branch: &l.branch, agents: &p.agents };
            let outs = self.pr_news.activity(&at, &pr, act, &self.pr_bots);
            self.news_out(fx, env, outs);
        }
        // the places gone (dropped): their PR goes with them
        let ids: BTreeSet<&str> = places.iter().map(|p| p.id.as_str()).collect();
        self.pr_news.retain(&ids);
        self.prs.retain(|k, _| ids.contains(k.as_str()));
        self.pr_lids.retain(|k, _| ids.contains(k.as_str()));
        self.pr_ok_ms.retain(|k, _| ids.contains(k.as_str()));
        // pr-merge: the ready-to-merge items against the PRs now
        self.merge_sync(fx, env);
        self.review_sync(fx, env);
    }

    /// pr-integration rule 2: the followed PRs' answer (every one asked
    /// this round): each changed one's artifact kept in step (its status,
    /// its archive at the end), one that no longer resolves dropped. Not
    /// a place's: no news, no cleanup, no merge item.
    fn followed_in(&mut self, fx: &mut Fx, at_ms: u64, answer: &[crate::forge::poll::Followed]) {
        let mut now: BTreeMap<String, crate::place::PrSnapshot> = BTreeMap::new();
        for f in answer {
            let Some(pr) = &f.pr else { continue };
            if self.followed.get(&f.url) != Some(pr) {
                fx.push(Effect::PrArtifact { pr: Box::new(pr.clone()), agent: None });
            }
            now.insert(f.url.clone(), pr.clone());
        }
        if now != self.followed {
            self.dirty = true;
        }
        self.followed = now;
        self.followed_ok_ms = at_ms;
    }

    /// pr-news' decisions, carried out: a message from `github` (sb-core's
    /// `forge`, a pseudo sender like a peer), a `pr` record in main's
    /// feed, a question card for the user
    /// (`forge_card`: its answer goes to the agent), a journal line.
    fn news_out(&mut self, fx: &mut Fx, env: &mut dyn Env, outs: Vec<crate::forge::news::Out>) {
        use crate::forge::news::Out;
        for o in outs {
            match o {
                Out::Tell { to, text } => self.core(fx, env, None, In::Forge(ci::Forge { to, text })),
                Out::Line { tone, number, url, text } => {
                    fx.push(feed(MAIN, Feed::Pr(feed_rec::Pr { v: 1, state: tone.state().into(), number, url, text })))
                }
                Out::Card { agent, text } => {
                    self.core(fx, env, None, In::ForgeCard(ci::ForgeCard { agent, text }))
                }
                Out::Journal(j) => fx.push(Effect::Journal(j)),
            }
        }
    }

    /// pr-design §6.4: a merged PR archives its place's agents and
    /// removes the worktree, with no backup when nothing would be lost
    /// (the PR's head is the branch's tip, no changed file). Else the
    /// place stays, and main's thread says why, once.
    fn pr_cleanup(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        l: &crate::forge::poll::Local,
        pr: &crate::place::PrSnapshot,
        agents: &[String],
    ) {
        if self.pr_done.contains(&l.place) {
            return;
        }
        let Some(dirty) = l.dirty else { return };
        self.pr_done.insert(l.place.clone());
        let names = agents.iter().map(|a| format!("@{}", a)).collect::<Vec<_>>().join(", ");
        if dirty || l.tip.as_deref() != Some(pr.head_oid.as_str()) {
            if !agents.is_empty() {
                let why = if dirty { "changed files" } else { "commits after the PR's head" };
                fx.push(feed(
                    MAIN,
                    Feed::warn(format!(
                        "#{} ({}) is merged, but its worktree has {}: {} stays. /archive it when its work is not needed",
                        pr.number, l.branch, why, names
                    )),
                ));
            }
            return;
        }
        env.pr_merged(&l.branch, &pr.head_oid);
        for a in agents {
            self.core(fx, env, None, In::Drop(ci::Drop { name: a.to_string(), force: true }));
        }
    }

    /// The role line the views show (BISE-126): the model's, else the
    /// objective's first sentence; none for main.
    pub fn role_line(&self, a: &Agent) -> String {
        crate::snapshot_rows::role_of(a, self.roles.get(&a.dir).and_then(|r| r.line.as_deref()))
    }

    /// A line saved by an earlier hub (`<agent dir>/role.json`).
    pub fn load_role(&mut self, dir: &str, line: String, key: String) {
        let r = self.roles.entry(dir.to_string()).or_default();
        r.line = Some(line);
        r.key = key;
    }

    /// A task's turn ended: one call for a new line when what it is made
    /// of changed, none while one is in flight (it looks again at its
    /// end), none for a while after a failure. Never main, never a task
    /// that is not active.
    fn ask_role(&mut self, fx: &mut Fx, now: u64, agent: &str) {
        let Some(a) = self.st.agents.get(agent) else { return };
        if a.is_main || a.lifecycle != Lifecycle::Active {
            return;
        }
        let report = a.last_report.as_ref().map(|r| r.summary.clone()).unwrap_or_default();
        let note = a.declared().map(|d| d.note.clone()).unwrap_or_default();
        let key = role::key(&a.brief.objective, &report, &note);
        let current = self.role_line(a);
        let (objective, dir) = (a.brief.objective.clone(), a.dir.clone());
        let r = self.roles.entry(dir.clone()).or_default();
        if r.asking.is_some() {
            r.pending = true;
            return;
        }
        if r.key == key || r.failed_ms.is_some_and(|t| now.saturating_sub(t) < ROLE_RETRY_MS) {
            return;
        }
        r.asking = Some(key.clone());
        fx.push(Effect::AskRole {
            dir,
            key,
            request: role::request(&objective, &report, &note, &current),
        });
    }

    fn role_answer(&mut self, fx: &mut Fx, now: u64, dir: &str, key: String, line: Option<String>) {
        let r = self.roles.entry(dir.to_string()).or_default();
        r.asking = None;
        match line {
            Some(l) => {
                if r.line.as_deref() != Some(l.as_str()) {
                    self.dirty = true;
                }
                r.line = Some(l);
                r.key = key;
                r.failed_ms = None;
            }
            None => r.failed_ms = Some(now),
        }
        if std::mem::take(&mut r.pending) {
            if let Some(name) = self.st.agents.values().find(|a| a.dir == dir).map(|a| a.name.clone()) {
                self.ask_role(fx, now, &name);
            }
        }
    }

    pub fn handle(&mut self, input: Input, env: &mut dyn Env) -> Fx {
        let mut fx = Fx::new();
        match input {
            Input::Boot => self.core(&mut fx, env, None, In::Boot(ci::Boot {})),
            Input::ReplReady { agent } => self.core(&mut fx, env, None, In::Ready(ci::Ready { agent })),
            Input::ReplLine { agent, line } => self.repl_line(&mut fx, env, &agent, &line),
            Input::ReplIdle { agent, leftover } => {
                self.doing_line(&agent, &Wire::Idle, env.now());
                // expired-ux: a turn that stopped on the expired ChatGPT
                // sign-in is held, not over: no automatic reply (`(no
                // written reply…)` woke its parent, on the same plan, into
                // a turn that failed too)
                let held = self.signin.stopped.contains(&agent);
                self.core(&mut fx, env, None, In::Idle(ci::Idle { agent: agent.clone(), leftover, held }));
                self.ask_role(&mut fx, env.now(), &agent);
            }
            Input::RoleLine { dir, key, line } => self.role_answer(&mut fx, env.now(), &dir, key, line),
            Input::TimerName { id, reply } => {
                self.timer_names.done(id);
                if let Some(t) = self.st.timers.map.get(&id) {
                    let name = crate::every_name::name_of(reply.as_deref(), &t.text, &t.agent);
                    self.core(&mut fx, env, None, In::EveryName(ci::EveryName { id, name }));
                }
            }
            Input::Prs(r) => self.prs_in(&mut fx, env, r),
            Input::Feature(d) => self.feature_done(&mut fx, env, d),
            Input::Onto { agent, reply } => {
                let _ = reply.send(self.onto_now(&agent).flatten());
            }
            Input::ReplExited { agent, crashed, reason, stalled, loaded, row, stopped } => {
                self.core(&mut fx, env, None, In::Exited(ci::Exited { agent, crashed, reason, stalled, loaded, row, stopped }))
            }
            Input::ClientHello { client } => {
                self.clients.insert(
                    client,
                    ClientView {
                        focus: MAIN.to_string(),
                        since_ms: env.now(),
                        sent: Vec::new(),
                    },
                );
            }
            Input::ClientInput {
                client,
                focus,
                text,
                queued,
                parts,
                from,
            } => self.user_input(&mut fx, env, client, &focus, &text, queued, &parts, &from),
            Input::UserCmd { client, focus, cmd } => {
                let focus = self.focus_of(client, &focus);
                self.user_cmd(&mut fx, env, client, focus, cmd, false, "", "")
            }
            Input::ClientFocus { client, focus } => self.set_focus(&mut fx, env, client, &focus),
            Input::ClientGone { client } => {
                self.set_focus(&mut fx, env, client, MAIN);
                self.clients.remove(&client);
            }
            Input::ClientInterrupt { client, agent } => {
                self.interrupt_by = Some("user".into());
                self.core(&mut fx, env, Some(client), In::Interrupt(ci::Interrupt { agent }));
                self.interrupt_by = None;
            }
            Input::ClientUnqueue { client, id } => self.core(&mut fx, env, Some(client), In::Unqueue(ci::Unqueue { id })),
            Input::ClientSteerQueued { client, id } => self.core(&mut fx, env, Some(client), In::SteerQueued(ci::SteerQueued { id })),
            Input::Agent { token, from, req, by } => self.agent_step(&mut fx, env, token, &from, req, by),
            Input::Speaks { token, from, cmd, by } => self.core(&mut fx, env, None, In::Speaks(by.speaks(token, from, cmd))),
            // the waits' timeouts, then sb every's timers (sb-core decides)
            Input::Tick => self.core(&mut fx, env, None, In::Tick(ci::Tick {})),
            Input::EveryStop { id, why } => self.timer_stop_by_user(&mut fx, env, id, &why),
            Input::EveryRun { id } => self.timer_run_now(&mut fx, env, id),
            Input::WakeStop { id } => self.wake_stop_by_him(&mut fx, env, id),
            Input::WakeHit { id, text, rc, tail, after_ms } => self.wake_hit(&mut fx, env, id, &text, crate::wake::Hit { rc, tail, after_ms }),
            Input::Placed { name, ws, err } => self.core(&mut fx, env, None, In::Placed(ci::Placed { name, ws, err })),
            Input::Unplaced { name, snapshot, err } => self.core(&mut fx, env, None, In::Unplaced(ci::Unplaced { name, snapshot, err })),
            Input::ConfirmOpen { agent, text } => self.core(&mut fx, env, None, In::ConfirmOpen(ci::ConfirmOpen { agent, text })),
            Input::ConfirmClose { card, res } => self.core(&mut fx, env, None, In::ConfirmClose(ci::ConfirmClose { card, res })),
            Input::ViewConfirmOpen { agent, call, text, ask } => self.core(&mut fx, env, None, In::ViewConfirmOpen(ci::ViewConfirmOpen { agent, call, text, ask })),
            Input::ViewContext { agent, call, text, part } => self.core(&mut fx, env, None, In::ViewContext(ci::ViewContext { agent, call, text, part })),
            Input::ViewMessage { agent, plugin, text } => self.core(&mut fx, env, None, In::ViewMessage(ci::ViewMessage { agent, plugin, text })),
            Input::PageAnswered { card, agent, page, text, block } => self.core(&mut fx, env, None, In::PageAnswered(ci::PageAnswered { card, agent, page, text, block })),
            Input::PageCard { agents, kind, text, key, about } => self.core(&mut fx, env, None, In::PageCard(ci::PageCard { agents, kind, text, key, about })),
            Input::AnswerPos { card, agent, pos } => self.core(&mut fx, env, None, In::AnswerPos(ci::AnswerPos { card, agent, pos })),
            Input::Merged { card, place, number, res } => self.merged(&mut fx, env, card, &place, number, res),
            Input::PrAct { client, req, url, head, act } => self.pr_act(&mut fx, merge::Click { client, req, url, act }, &head),
            Input::PrActed { client, req, url, number, act, res } => self.pr_acted(&mut fx, env, merge::Click { client, req, url, act }, number, res),
            Input::Release(c) => self.release_in(&mut fx, env, c),
            Input::UpdateNow => self.update_now(&mut fx),
            Input::SignedIn => self.signed_in(&mut fx, env),
            Input::HubMsg { to, text } => self.core(&mut fx, env, None, In::HubMsg(ci::HubMsg { to, text })),
            Input::ClaimsClean { place, paths } => self.core(&mut fx, env, None, In::ClaimsClean(ci::ClaimsClean { place, paths })),
            Input::Updated { version, res } => self.updated(&mut fx, &version, res),
            Input::XIn { token, hub, xid, kind, text, name, parts } => {
                self.core(&mut fx, env, None, In::XIn(ci::XIn { token, hub, xid, kind, text, name, parts }))
            }
            Input::XAck { xid } => self.core(&mut fx, env, None, In::XAck(ci::XAck { xid })),
            Input::XFail { xid } => self.core(&mut fx, env, None, In::XFail(ci::XFail { xid })),
            Input::XReply { xid, name, text } => self.core(&mut fx, env, None, In::XReply(ci::XReply { xid, name, text })),
            Input::RouteAsk { focus, text, via, queued, context, parts, projects } => {
                let context = context.filter(|c| !c.is_null()).map_or(String::new(), |c| c.to_string());
                let projects = projects.into_iter().map(|(id, name)| json!({"id": id, "name": name})).collect();
                self.core(&mut fx, env, None, In::RouteAsk(ci::RouteAsk { focus, text, via, queued, context, parts, projects }))
            }
            Input::RouteCorrect { client, rid, to, name } => {
                self.core(&mut fx, env, client, In::RouteCorrect(ci::RouteCorrect { rid, to, name }))
            }
            Input::RouteCancel { client, rid } => self.core(&mut fx, env, client, In::RouteCancel(ci::RouteCancel { rid })),
            Input::Follow { client, agent, on } => self.core(&mut fx, env, client, In::Follow(ci::Follow { agent, on })),
        }
        self.refresh_contexts(&mut fx, env.now());
        if self.dirty {
            fx.push(Effect::State);
            self.dirty = false;
        }
        fx
    }

    fn repl_line(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, raw: &str) {
        let now = env.now();
        let agent_s = agent.to_string();
        let line = wire::parse(raw);
        self.doing_line(agent, &line, now);
        match line {
            Wire::TurnStarted => self.core(fx, env, None, In::TurnStarted(ci::TurnStarted { agent: agent_s })),
            Wire::SteeringReceived => self.core(fx, env, None, In::SteerRx(ci::SteerRx { agent: agent_s })),
            Wire::Steered => self.core(fx, env, None, In::Steered(ci::Steered { agent: agent_s })),
            Wire::Assistant(text) if !text.is_empty() => {
                self.set_activity(agent, now, format!("wrote: {}", clip(&one_line(&text), 160)));
                let r = self.recent.entry(agent.to_string()).or_default();
                r.push_back((now, text.clone()));
                if r.len() > 20 {
                    r.pop_front();
                }
                self.core(fx, env, None, In::Assistant(ci::Assistant { agent: agent_s, text }));
            }
            Wire::Tool { name, args } if name == "edit" || name == "write_file" => {
                // Vibe's edit tools: the runtime's feed line carries
                // {"file_path": …} (bend/core/edit.bend ann_args)
                let path = wire::edit_file(&args);
                if self.st.agents.contains_key(agent) {
                    let verb = if name == "edit" { "edit" } else { "write" };
                    self.set_activity(agent, now, format!("{} {}", verb, path.as_deref().unwrap_or("")));
                    self.dirty = true;
                }
                if let Some(path) = path {
                    self.core(fx, env, None, In::Touch(ci::Touch { agent: agent_s, path }));
                }
            }
            Wire::Tool { name, args } if name != "apply_patch" => {
                if let Some(own) = self.st.agents.get(agent).map(|a| a.ws.path.clone()) {
                    self.set_activity(agent, now, format!("{} `{}`", name, clip(&one_line(&args), 120)));
                    self.dirty = true;
                    // main works in the workspace (`sb worktree` refuses it
                    // too): its `cd <a task's worktree> && …` to look at
                    // a task's work is no move (Benjamin's 'ψ ven-bots')
                    let guess = name == "bash" && agent != MAIN;
                    if let Some(p) = guess.then(|| crate::place::bash_worktree(&args, &own)).flatten() {
                        self.set_place(agent, p, false);
                    }
                }
            }
            Wire::BgHandoff(args) => self.bg_handoff(fx, env, agent, &args),
            Wire::Intent(text) => {
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, clip(&text, 120));
                    self.dirty = true;
                }
            }
            Wire::Tool { args, .. } => {
                let files = wire::patch_files(&args);
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, format!("apply_patch {}", files.join(", ")));
                    self.dirty = true;
                }
                for path in files {
                    self.core(fx, env, None, In::Touch(ci::Touch { agent: agent_s.clone(), path }));
                }
            }
            // BR-007: a task whose turn failed (network down, provider
            // error...) must not go quiet: the failure reaches its parent
            // as a report (board + message), like a report the task wrote.
            // Main's own failures show in main's view (its turn_done line);
            // a stop the user asked for (Ctrl+C) is not news to anyone.
            Wire::TurnDone(t) => {
                // issue #4: the first turn on a model asked at the spawn;
                // refused by its provider, the task moves to the default
                if self.model_trial.remove(agent) {
                    if let Some(why) = model_refusal(&t) {
                        fx.push(Effect::ModelRefused { agent: agent.to_string(), why });
                        return;
                    }
                }
                // expired-ux: the ChatGPT sign-in expired: the user's
                // `signin` item says it, the agent waits for the sign-in
                // (its parent, on the same plan, can't help)
                if self.signin_turn(fx, env, agent, &t) {
                    return;
                }
                if let Some(summary) = failed_turn_report(agent, &t) {
                    if self.st.agents.contains_key(agent) {
                        let q = json!({"cmd": "report", "kind": "turn_failed",
                            "summary": summary, "decisions": []});
                        // token 0: no connection waits for this reply
                        // (connection tokens start at 1)
                        self.core(fx, env, None, In::Req(ci::Req { token: 0, from: agent_s, req: q, who: "hub".into(), dir: String::new(), sock: String::new() }));
                    }
                }
            }
            _ => {}
        }
    }

    fn set_focus(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, focus: &str) {
        let now = env.now();
        let Some(view) = self.clients.get_mut(&client) else {
            return;
        };
        let prev = std::mem::replace(
            view,
            ClientView {
                focus: focus.to_string(),
                since_ms: now,
                sent: Vec::new(),
            },
        );
        let mut input = ci::Focus { focus: focus.to_string(), note: None, direct: String::new() };
        if prev.focus != MAIN && prev.focus != focus && !prev.sent.is_empty() {
            // RFC 0001 §7.4: main learns what the user decided directly
            let reply: String = self
                .recent
                .get(&prev.focus)
                .map(|r| {
                    r.iter()
                        .filter(|(t, _)| *t >= prev.since_ms)
                        .map(|(_, s)| s.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let (note, direct) = direct_exchange(&prev.focus, &prev.sent, &reply);
            (input.note, input.direct) = (Some(note), direct);
        }
        self.core(fx, env, Some(client), In::Focus(input));
    }

    /// Issue #4: who `sb send --model` may move: a task, by main or its
    /// parent. Its name, or the words of the refusal.
    fn switch_target(&self, from: &str, to: &str) -> Result<String, String> {
        let to = self.st.resolve(to.trim_start_matches('@')).ok_or_else(|| format!("unknown agent: {}", to))?;
        let from = self.st.resolve(from).unwrap_or_default();
        let parent = self.st.agents.get(&to).and_then(|a| a.parent.clone()).unwrap_or_default();
        if to == MAIN {
            return Err("sb send --model moves a task; main's model is the user's (/model)".into());
        }
        if from != MAIN && from != parent {
            return Err(format!("sb send --model is main's (or the parent's): ask {}", if parent.is_empty() { MAIN } else { &parent }));
        }
        Ok(to)
    }

    fn agent_req(&mut self, fx: &mut Fx, env: &mut dyn Env, token: Token, from: &str, req: AgentReq, by: crate::peer::Sender) {
        let reply = |fx: &mut Fx, body: Value| fx.push(Effect::Reply { token, body });
        // the home workspace (docs/ambient-pages.md §5.1): no git, so no
        // land, feature or flow; one line that says why, never a git error
        if let Some(cmd) = git_only(&req).filter(|_| !env.is_git()) {
            reply(fx, json!({"ok": false, "error": not_in_a_repo(cmd)}));
            return;
        }
        let q = match req {
            AgentReq::List | AgentReq::Tasks => {
                let Some(from) = self.st.resolve(from) else {
                    reply(fx, json!({"ok": false, "error": format!("unknown agent: {}", from)}));
                    return;
                };
                let text = if req == AgentReq::List {
                    board::roster(&self.st, &from, env.now(), &self.models).join("\n")
                } else {
                    let mut t = board::tasks_detail(&self.st, env.now(), &self.models);
                    // the standing orders (sb every): what wakes whom, when
                    let timers = self.st.timers.list(env.now());
                    if !self.st.timers.map.is_empty() {
                        t.push_str(&format!("\n\n## timers (sb every)\n{}", timers));
                    }
                    t
                };
                reply(fx, json!({"ok": true, "text": text}));
                return;
            }
            AgentReq::Send {
                to,
                text,
                expect_reply,
                reply_to,
                queued,
                why,
                switch,
                parts,
                attach: _,
                fwd,
            } => {
                // issue #4: the model first, so the message's turn runs on it
                if let Some((model, effort)) = switch {
                    match self.switch_target(from, &to) {
                        Ok(to) => fx.push(Effect::Switch { token: None, from: from.to_string(), to, model, effort }),
                        Err(e) => {
                            reply(fx, json!({"ok": false, "error": e}));
                            return;
                        }
                    }
                }
                let mut q = json!({"cmd": "send", "to": to, "text": text, "expect_reply": expect_reply,
                        "reply_to": reply_to, "queued": queued, "why": why});
                // send-parts: only when it has some (an older core reads
                // the same line as before)
                if !parts.is_empty() {
                    q["parts"] = json!(parts);
                }
                crate::forward::keys(&self.st, &mut q, fwd);
                q
            }
            AgentReq::Switch { to, model, effort } => {
                match self.switch_target(from, &to) {
                    Ok(to) => fx.push(Effect::Switch { token: Some(token), from: from.to_string(), to, model, effort }),
                    Err(e) => reply(fx, json!({"ok": false, "error": e})),
                }
                return;
            }
            AgentReq::Wait { msg, timeout_s } => {
                json!({"cmd": "wait", "msg": msg, "timeout_s": timeout_s})
            }
            AgentReq::Ask {
                to,
                text,
                timeout_s,
            } => json!({"cmd": "ask", "to": to, "text": text, "timeout_s": timeout_s}),
            AgentReq::Status { status, note } => {
                json!({"cmd": "status", "status": status, "note": note})
            }
            AgentReq::Flow { set } => {
                if self.st.resolve(from).as_deref() != Some(MAIN) {
                    reply(fx, json!({"ok": false, "error": "sb flow is main's: ask main"}));
                    return;
                }
                let set = match pure::flow_set(&set) {
                    Ok(s) => s,
                    Err(e) => return reply(fx, json!({"ok": false, "error": e})),
                };
                fx.push(Effect::Flow { client: None, token: Some(token), set });
                return;
            }
            AgentReq::Feature { op, name } => {
                // merge and drop on the user's go only: main's (dev-flow §5.1)
                let is_main = self.st.resolve(from).as_deref() == Some(MAIN);
                if matches!(op.as_str(), "new" | "merge" | "drop" | "retire") && !is_main {
                    reply(fx, json!({"ok": false, "error": format!("sb feature {} is main's: ask main", op)}));
                    return;
                }
                let agents = self.feature_agents(&name);
                fx.push(Effect::Feature { token: Some(token), op, name, agents });
                return;
            }
            AgentReq::Worktree { path } => {
                let path = match pure::worktree_path(&path) {
                    Ok(p) => p,
                    Err(e) => return reply(fx, json!({"ok": false, "error": e})),
                };
                let Some(name) = self.st.resolve(from) else {
                    reply(fx, json!({"ok": false, "error": format!("unknown agent: {}", from)}));
                    return;
                };
                if name == MAIN && !path.is_empty() {
                    let e = "sb worktree: main works in the workspace, not in a worktree";
                    reply(fx, json!({"ok": false, "error": e}));
                    return;
                }
                self.set_place(&name, path.clone(), true);
                reply(fx, json!({"ok": true, "path": path}));
                return;
            }
            AgentReq::Report {
                kind,
                summary,
                decisions,
                step,
                result,
            } => {
                let mut q = json!({"cmd": "report", "kind": kind, "summary": summary, "decisions": decisions});
                if let Some((n, of)) = step {
                    q["step"] = json!(n);
                    q["of"] = json!(of);
                }
                if let Some(r) = result {
                    q["result"] = r;
                }
                q
            }
            AgentReq::Follow { agent, on, hub } => json!({"cmd": "follow", "agent": agent, "off": !on, "hub": hub}),
            AgentReq::Spawn {
                name,
                brief,
                worktree,
                with_changes,
                place,
                feature,
                ask,
                parts,
                attach: _,
                fwd,
            } => {
                // issue #4: the answer waits for the daemon's pick of the model
                if !ask.is_empty() {
                    self.spawn_asks.insert(token, ask);
                }
                let name = Some(name.as_str()).filter(|n| !n.is_empty());
                let join = match (place.as_str(), feature.as_str()) {
                    // dev-flow §5.1: its own worktree from the feature's tip,
                    // made off the loop by sb-core's fx (issue 32 3c)
                    (_, f) if !f.is_empty() => self.feature_allowed().map(|_| None),
                    ("" | "new" | crate::place::SHARED, _) => Ok(None),
                    (p, _) => crate::place::find_worktree(&self.st, p).map(Some),
                };
                match join.and_then(|j| new_task(name, &brief, &parts, worktree || place == "new", with_changes).map(|v| (j, json!(v)))) {
                    Ok((join, mut v)) => {
                        v["cmd"] = json!("spawn");
                        if !feature.is_empty() {
                            v["feature"] = json!(feature);
                        }
                        if !parts.is_empty() {
                            v["brief_parts"] = json!(parts);
                        }
                        if let Some(ws) = join {
                            v["join"] = json!(ws);
                            v["worktree"] = json!(false);
                        }
                        crate::forward::keys(&self.st, &mut v, fwd);
                        v
                    }
                    Err(e) => json!({"cmd": "spawn", "pre_err": e}),
                }
            }
            AgentReq::Move { agent, place } => match self.move_target(&agent, &place) {
                Ok(ws) => json!({"cmd": "move", "agent": agent, "ws": ws}),
                Err(e) => json!({"cmd": "move", "agent": agent, "pre_err": e}),
            },
            AgentReq::Land { here, message, add } => {
                match self.land_job(from, here, &message, add) {
                    Ok(job) => {
                        // pr-news: the PR's news go to the place's last lander
                        if job.worktree {
                            self.pr_news.landed(&job.place, &job.agent);
                        }
                        fx.push(Effect::Land { token, job: Box::new(job) })
                    }
                    Err(e) => reply(fx, json!({"ok": false, "error": e})),
                }
                return;
            }
            AgentReq::Interrupt { agent } => {
                // the flag names the asker: "interrupted by main", not
                // "by the user"
                self.interrupt_by = Some(from.to_string());
                json!({"cmd": "interrupt", "agent": agent})
            }
            AgentReq::Stop { agent, reason } => {
                json!({"cmd": "stop", "agent": agent, "reason": reason})
            }
            AgentReq::Drop { agent } => {
                // a drop the user was asked to confirm waits for his
                // answer: a second drop never acts in his place (pm's C
                // fail 36: main dropped @proxy-fix again once it was idle,
                // while its card still asked him)
                let name = self.st.resolve(&agent).unwrap_or_else(|| agent.clone());
                if let Some(c) = self.st.open_cards().find(|c| c.kind == "drop" && c.agent == name) {
                    let error = format!("@{name}: the user has not answered notification #{} (archive it?) yet: wait for his answer", c.id);
                    reply(fx, json!({"ok": false, "error": error}));
                    return;
                }
                json!({"cmd": "drop", "agent": agent})
            }
            // its page or artifact links are the daemon's (daemon/page_cards.rs);
            // `about`, the artifact it resolved, rides it to card_opened
            AgentReq::Card { text, for_msg, about, .. } => {
                let mut q = json!({"cmd": "card", "text": text, "for": for_msg});
                if let Some(a) = about {
                    q["about"] = a;
                }
                q
            }
            AgentReq::Close { card, note } => json!({"cmd": "close", "card": card, "note": note}),
            AgentReq::Withdraw { card, why } => json!({"cmd": "withdraw", "card": card, "why": why}),
            AgentReq::Rename { agent, new_name } => {
                let valid = router::valid_name(&new_name);
                json!({"cmd": "rename", "agent": agent, "new_name": new_name, "valid": valid})
            }
            AgentReq::Restore { agent } => json!({"cmd": "restore", "agent": agent}),
            AgentReq::Isolate { agent } => json!({"cmd": "isolate", "agent": agent}),
            AgentReq::Every(r) => {
                let body = self.timer_req(fx, env, from, r);
                reply(fx, body);
                return;
            }
            AgentReq::Wake(r) => {
                let body = self.wake_req(fx, env, from, r);
                reply(fx, body);
                return;
            }
            // desktop S2: the project's hub id (sb-core never reads the registry)
            AgentReq::ProjectSend { project, msg } => match env.project_hub(&project) {
                Ok(hub) => json!({"cmd": "project_send", "project": hub, "msg": msg}),
                Err(e) => json!({"cmd": "project_send", "pre_err": e}),
            },
            AgentReq::ProjectAsk { project, text } => match env.project_hub(&project) {
                Ok(hub) => json!({"cmd": "project_ask", "project": hub, "text": text}),
                Err(e) => json!({"cmd": "project_ask", "pre_err": e}),
            },
            // the daemon's own (daemon.rs `agent_request`): never here
            r => {
                reply(fx, json!({"ok": false, "error": format!("sb {}: not a request of the hub's core", r.tag())}));
                return;
            }
        };
        self.core(fx, env, None, In::Req(ci::Req { token, from: from.to_string(), req: q, who: by.who, dir: by.dir, sock: by.sock }));
    }
}

#[path = "core_pure.rs"]
mod pure;
pub(crate) use pure::not_in_a_repo;
pub use pure::Loss;
use pure::{direct_exchange, force_run_input, git_only, jstr};

#[path = "core_link.rs"]
mod link;

#[path = "core_photo.rs"]
mod photo;
pub use photo::{Rebuilt, Revive};

#[path = "core_fx.rs"]
mod core_fx;

#[path = "core_view.rs"]
mod view;

#[path = "core_user.rs"]
mod user;
use user::new_task;
pub use user::HELP;

#[path = "core_wake.rs"]
mod wake_link;

#[path = "core_doing.rs"]
mod doing_link;
#[path = "core_contexts.rs"]
mod contexts_link;

#[path = "core_feature.rs"]
pub mod feature;
pub use feature::FeatureDone;

#[path = "merge.rs"]
pub(crate) mod merge;

#[path = "core_review.rs"]
pub(crate) mod review;

#[path = "release_line.rs"]
pub mod release_line;

#[path = "signin_card.rs"]
pub mod signin_card;

#[path = "core_snapshot.rs"]
mod snapshot;

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "core_view_tests.rs"]
mod view_tests;
