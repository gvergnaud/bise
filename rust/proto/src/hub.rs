//! The hub side of the protocol, v1's frozen part: [`HubEv`] (hub ->
//! client, tagged `"ev"`) and [`HubCmd`] (client -> hub, tagged `"cmd"`).
//! A hub can only emit a `HubEv`; the core's and the app's own messages
//! (projects, open, project_add) are other types ([`crate::draft`]), so
//! they can't reach a hub or come from one.

use crate::context::FnContext;
use crate::diff::{DiffFile, DiffResult, DiffView};
use crate::ops::{BranchRow, ReleaseEv, VersionItem};
use crate::rows::{Agent, ApprovalMode, ApprovalRule, Artifact, Card, CheckerKind, DevServer, Feature, Merged, Model, Pr, ScheduledTask, Worktree};
use crate::thread::Entry;
use crate::{decode, parse, Pos, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Done,
    Failed,
}

/// A followed task (S10): its job's name (its last progress, else its
/// objective), `step` of `of` when it reports them (`sb report progress
/// --step n/m`), since when it is at this step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Job {
    pub agent: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u32>,
    pub state: JobState,
    pub since_ms: u64,
}

/// What a hub sends a client that said `hello {proto: 1}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum HubEv {
    /// the answer to `hello`: who this hub is (its tag is `welcome`: the
    /// hub's older `hello` event stays the TUI's)
    Welcome {
        project: Project,
        proto: u32,
        workspace: String,
        name: String,
        /// hub-skew (architect m_11314): the commands this hub knows, its
        /// `HubCmd::TAGS` (one owner). Empty: a hub older than the list;
        /// the core then compares `proto` with its own.
        #[serde(default)]
        cmds: Vec<String>,
    },
    /// every agent of the hub, on change
    Agents { project: Project, agents: Vec<Agent> },
    /// every open card of the hub, on change
    Cards { project: Project, cards: Vec<Card> },
    /// one page of a thread, newest last: the answer to `subscribe` and
    /// `page`; `before` the oldest entry's pos when there is more
    Thread { project: Project, agent: String, entries: Vec<Entry>, before: Option<Pos>, more: bool },
    /// a subscribed thread's entry, live: a new pos appends, a known pos
    /// replaces (a tools entry growing, a card answered)
    Entry { project: Project, agent: String, entry: Box<Entry> },
    /// a subscribed agent's current step ("" when its turn ends)
    Typing { project: Project, agent: String, text: String },
    /// every artifact of the project, at hello and on change; `new`: made
    /// or changed since he last looked (`artifacts_seen` clears it)
    Artifacts { project: Project, items: Vec<Artifact> },
    /// the live scheduled tasks (`sb every`) of the project, at hello, on
    /// `scheduled` and whenever one is set, runs, stops or ends
    Scheduled { project: Project, items: Vec<ScheduledTask> },
    /// the repo's worktrees (the agents' and his others), at hello, when
    /// the agents change (a turn ends, one comes or goes) and on
    /// `worktrees`
    Worktrees { project: Project, items: Vec<Worktree> },
    /// the agents' background servers, with `worktrees` (same moments)
    /// and on `dev_servers`
    DevServers { project: Project, items: Vec<DevServer> },
    /// the commits that reached the trunk today, newest first: at hello,
    /// after each land and on `merged`
    Merged { project: Project, items: Vec<Merged> },
    /// the project's feature branches (dev-flow §5.1): at hello, on
    /// `features`, and when one changes (made, synced, built, merged,
    /// dropped, its card opened or closed, its agents)
    Features { project: Project, items: Vec<Feature> },
    /// the open pull requests of the project's places (the TUI's `/prs`),
    /// by number: at hello, on `prs`, and when one changes; `head` the
    /// hub's head line (`2 PRs open`, "" when none), `none` its words when
    /// there is no open PR
    Prs {
        project: Project,
        head: String,
        items: Vec<Pr>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        none: Option<String>,
    },
    /// the models the TUI's `/model` offers on this hub (bar A.5): at
    /// hello, on `models`, and when config.toml or auth.json changed
    Models { project: Project, items: Vec<Model> },
    /// a tool row's output (the answer to `tool_out`): the call's whole
    /// result from the agent's session log, capped at
    /// `thread::TOOL_TEXT_CAP`; when the log hasn't it (an older REPL's
    /// call, no log) the transcript's preview with `cut`
    ToolOut {
        project: Project,
        agent: String,
        pos: Pos,
        out: String,
        cut: bool,
        /// the whole output's size in bytes (the unit of the cap: '4 KB
        /// of 18 KB shown'), when the session log answered; none for the
        /// preview (the size isn't known)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        total: Option<u64>,
    },
    /// an agent's change (the answer to `diff`): its checkout or branch
    /// vs `base`; `head` its branch (none: the shared folder)
    Diff {
        project: Project,
        agent: String,
        base: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        head: Option<String>,
        files: Vec<DiffFile>,
        /// what the change measured (none: nothing measured; boxed: the
        /// enum stays small, the wire is the same)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Box<DiffResult>>,
        /// its land on the trunk, once merged: the full sha (the merge
        /// path sets it; the review shows 7 and opens it on GitHub)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        merged: Option<String>,
        /// the commit asked (`diff {commit}`): this is that commit alone,
        /// not the agent's live change (amb-web m_8974)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
        /// why there are no files to show (its folder is gone...)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// the terminal's `/diff` (client-protocol step 3): its fields
        /// next to these on the wire, boxed (the enum stays small)
        #[serde(flatten)]
        view: Box<DiffView>,
    },
    /// `/diff`'s branch picker (the answer to `branches`): the local
    /// branches ahead of `base`, the trunk
    Branches { project: Project, base: String, rows: Vec<BranchRow> },
    /// `/version`'s picker (the answer to `versions`): `current` the
    /// running version's id, `dev` this is bise's source tree (commits
    /// can be built), `installed` an installed bise (releases listed)
    Versions {
        project: Project,
        current: String,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        dev: bool,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        installed: bool,
        items: Vec<VersionItem>,
    },
    /// `/release-bise` (dev build only): a plan or a run's step
    /// ([`ReleaseEv`], boxed: the enum stays small, the wire is its
    /// fields next to the tag)
    Release(Box<ReleaseEv>),
    /// bise's home hub holds his words for a project's main (desktop S2,
    /// decision B): `to` the project picked (its hub id), `name` its name,
    /// `why` the guess's reason; until `correct_until_ms` (2 s after) a
    /// `route_correct` or `route_cancel` with this `rid` still changes it;
    /// `route` comes again after a correction (same rid, same time)
    Route { project: Project, rid: u64, to: Project, name: String, text: String, correct_until_ms: u64, why: String },
    /// the route's end: `state` "sent" (forwarded: `xid` its delivery) or
    /// "cancelled" (his words stayed with bise's main)
    RouteDone {
        project: Project,
        rid: u64,
        state: String,
        to: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        xid: Option<u64>,
    },
    /// the followed tasks (S10), at hello and on change: a follow ends
    /// with its job (`job_end`), so every row runs
    Jobs { project: Project, items: Vec<Job> },
    /// a followed task ended (done or failed), once: his notification
    JobEnd {
        project: Project,
        agent: String,
        state: JobState,
        label: String,
        summary: String,
        /// draft (J): the job's key (its end's time on that hub), the same
        /// as in bise's followed_end of it: a window shows one line per
        /// (project, agent, key)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<u64>,
    },
    /// draft (J, architect m_10223): bise's home hub heard that a task it
    /// follows in another project ended (`project`: that project's hub
    /// id); bise's main has its line. The window shows one line per
    /// (project, agent, key), whichever of this and that project's own
    /// job_end comes first
    FollowedEnd { project: Project, agent: String, key: u64, state: JobState, label: String, summary: String },
    /// a yes/no question from the hub to the window that asked (bar
    /// I9: archive a working agent...), answered by `confirm` with its
    /// `id`; only that connection gets it. Left unanswered it holds
    /// nothing in the hub: a later command just asks again
    Confirm { project: Project, id: u64, text: String },
    /// how tool calls are approved on this machine and the saved rules of
    /// this repo (bar V8/W21: the TUI's `/approvals`, from the same JSON):
    /// at hello, to every connection when the mode or the rules change,
    /// to one connection on `approvals` with no mode. `env`: the mode is
    /// set for this session by BISE_APPROVALS; `flash`: it just switched
    Approvals {
        project: Project,
        mode: ApprovalMode,
        env: bool,
        checker: CheckerKind,
        /// who checks: `TypeSafe`, `OpenRouter`, a chat model's id
        checker_who: String,
        /// Jev's model id (`jev-1.13`)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checker_model: Option<String>,
        /// the workspace's git root: the rules below are its own and
        /// every project's
        repo: String,
        rules: Vec<ApprovalRule>,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        flash: bool,
    },
    /// a plain line from the hub to the window that sent `cmd` (a
    /// confirm answered no: "drop of @x cancelled"; `/flow`'s answer to
    /// a `slash`), not a failure. `cid`: the slash command's, when it
    /// had one (architect m_10911)
    Notice {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cmd: Option<String>,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cid: Option<u64>,
    },
    /// the hub refused this connection (docs/issues/16: a client in an
    /// agent's process), then closes it: socket-auth's accept.rs writes
    /// it, the TUI and the desktop core read it (never reconnecting)
    Refused { error: String },
    /// an unknown or refused command, never silence
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cmd: Option<String>,
        text: String,
        /// draft (G): the failed send's cid (HubCmd send's), to the
        /// connection that sent it: the window fails exactly that row
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cid: Option<u64>,
        /// draft (G), with a cid: "refused" (nothing reached the thread:
        /// his row shows not delivered, with retry) or "undelivered" (the
        /// hub wrote its undelivered line in the thread: that entry
        /// stands in for his row)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// draft: what kind of failure, for the window to draw (never its
        /// text): the hub is older than this core (hub_older, amb-feed's
        /// hub skew) or refused this core's connection (hub_refused)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<ErrorKind>,
    },
    /// a tag this version doesn't know (a newer hub): kept whole, ignored
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl HubEv {
    pub const TAGS: &'static [&'static str] = &["welcome", "agents", "cards", "thread", "entry", "typing", "artifacts", "scheduled", "worktrees", "dev_servers", "merged", "features", "prs", "models", "tool_out", "diff", "branches", "versions", "release", "route", "route_done", "jobs", "job_end", "followed_end", "confirm", "approvals", "notice", "refused", "error"];

    pub fn decode(line: &str) -> Result<HubEv, String> {
        Self::from_value(parse(line)?)
    }

    pub fn from_value(v: Value) -> Result<HubEv, String> {
        decode(v, "ev", Self::TAGS, |tag, raw| HubEv::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            HubEv::Unknown { raw, .. } => raw.clone(),
            e => serde_json::to_value(e).expect("a HubEv encodes"),
        }
    }

    /// One line, without its newline.
    pub fn encode(&self) -> String {
        self.to_value().to_string()
    }

    pub fn tag(&self) -> &str {
        match self {
            HubEv::Unknown { tag, .. } => tag,
            e => match serde_json::to_value(e).ok().and_then(|v| v.get("ev").and_then(Value::as_str).map(str::to_string)) {
                Some(t) => Self::TAGS.iter().find(|x| **x == t).copied().unwrap_or("?"),
                None => "?",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// steered into its running turn (or a turn of its own)
    Now,
    /// after its turn
    Queued,
}

/// How his words go, the same for `send` and `slash` (architect m_13313:
/// one struct, flattened, so the wire keeps `send`'s names): `mode` none
/// is now; `context` what was on his screen at fn (S9): the hub frames it
/// after his words for the model (switchboard's fn_context::render) and
/// keeps it on his thread entry; `files` what he attached or dropped
/// (absolute paths): the hub renders them once after his words
/// (switchboard's attached::render), an image as an image-store marker
/// the model sees, any other file by its path, a path that isn't absolute
/// or doesn't exist left out, never on a slash command; `voice` said in
/// voice mode; `via` the client part that sent it (`capsule`,
/// `capsule-start`, `ambient`: the capsule's hint for main).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SendOpts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<FnContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub voice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

impl SendOpts {
    pub fn queued(&self) -> bool {
        self.mode == Some(Mode::Queued)
    }
}

/// What a client asks a hub.
// one command is decoded at a time and never stored in bulk: the size of
// its largest variant (Send, with its context and files) costs nothing
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HubCmd {
    /// this connection speaks v`proto`: the hub answers `welcome`, then
    /// sends `agents` and `cards` now and on each change
    Hello {
        proto: u32,
        /// typed events only on this connection: the hub's older events
        /// (`state`, `line`, its own `artifacts`...) stop after this
        /// hello (a window's connection; the capsule's mixed one says
        /// false until S8). When S8 moves the capsule to typed events,
        /// every typed connection is typed_only: S8 removes the mixed
        /// case (and this flag's false)
        #[serde(default, skip_serializing_if = "crate::is_false")]
        typed_only: bool,
    },
    /// a thread's newest page (`thread`), then its `entry`/`typing` live
    Subscribe {
        project: Project,
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    Unsubscribe { project: Project, agent: String },
    /// the entries before `before` (a pos of `thread`): one more `thread`
    Page {
        project: Project,
        agent: String,
        before: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    /// his words to an agent (main included), as the TUI sends them, with
    /// how they go ([`SendOpts`]: mode, context, files, voice, via)
    Send {
        project: Project,
        agent: String,
        text: String,
        #[serde(flatten)]
        opts: SendOpts,
        /// draft (G): the window's id for this send, echoed on its error
        /// to this connection only (never journaled: view state)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cid: Option<u64>,
    },
    /// a card's answer: an option's number or words (approvals and merges
    /// included: the hub's one /answer path). `files`: what he pasted or
    /// dropped with it, the same field as `Send.files`, rendered after his
    /// reply by the same `attached::render` (an image as its image-store
    /// marker, so the asking agent's model sees it; R41, architect m_13737)
    Answer {
        project: Project,
        card: u64,
        reply: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
    },
    /// close a card without answering it (the TUI's `/close N`): the
    /// hub's one close path (sb-core's `close`), a refusal as `error`
    Close { project: Project, card: u64 },
    /// his answer to the hub's `confirm` `id`: yes or no (a no comes
    /// back as `notice`, never as `error`)
    Confirm { project: Project, id: u64, yes: bool },
    /// the approvals mode (bar V8/W21, the TUI's shift+tab and
    /// `/approvals yolo|auto`): `yolo`, `auto` or `toggle` sets it and
    /// every connection gets `approvals` with `flash`; no mode: this
    /// connection gets `approvals` (the screen opened); another word is
    /// an `error`. His command only: docs/issues/16 is what keeps an
    /// agent's connection from sending it
    Approvals {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
    },
    /// a saved rule out of approvals.toml: `rule` is the row `approvals`
    /// gave (its fields are its identity, its words are ignored); one
    /// already gone is an `error`. His command only, as `approvals`
    RemoveRule { project: Project, rule: ApprovalRule },
    /// stop its turn
    Stop { project: Project, agent: String },
    /// out of his list (`force`: a working one is stopped first)
    Archive { project: Project, agent: String, force: bool },
    Unarchive { project: Project, agent: String },
    /// he opened the artifacts screen: their `new` clears, `artifacts`
    /// comes again
    ArtifactsSeen { project: Project },
    /// a change, once (`diff` answers): exactly one of `agent` (its
    /// change; `commit`: one commit of it instead, a landed one: the
    /// review's "merged · e0f3df5"), `branch` (a local branch vs the
    /// trunk), `pr` (an open pull request) or `range` (`a..b`, with
    /// `agent` naming it in the title); `req` the client's number for
    /// this ask, echoed in the answer (the terminal's /diff drops older
    /// answers). Anything else is an `error`
    Diff {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pr: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        req: Option<u64>,
    },
    /// he opened a tool row: its whole output (`tool_out` answers this
    /// connection), the tool item's `pos` in `agent`'s thread
    ToolOut { project: Project, agent: String, pos: Pos },
    /// `/diff`'s picker opened: `branches` answers
    Branches { project: Project },
    /// the environments screen opened: `worktrees` comes again
    Worktrees { project: Project },
    /// the environments screen opened: `dev_servers` comes again
    DevServers { project: Project },
    /// the changes tab opened: `merged` comes again
    Merged { project: Project },
    /// the features view opened: `features` comes again
    Features { project: Project },
    /// the PR list opened: `prs` comes again
    Prs { project: Project },
    /// the scheduled tasks asked for: `scheduled` comes again
    Scheduled { project: Project },
    /// stop scheduled task `id` (his, like the TUI's /scheduled stop: its
    /// agent hears it, its line says `stopped by you`); an unknown or
    /// ended id is an `error`
    ScheduledStop { project: Project, id: u64 },
    /// the model picker opened: `models` comes again (after a login made
    /// elsewhere, the keys are read again)
    Models { project: Project },
    /// a new agent (`/new`, bar A.1): `brief` its objective as he wrote
    /// it (never read for flags), `name` its name (none: one is picked),
    /// `worktree` its own worktree (`-w`), `with_changes` it takes his
    /// uncommitted changes there (worktree only). The hub's own /new: a
    /// refusal is an `error`, the agent shows in the next `agents`
    New {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        brief: String,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        worktree: bool,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        with_changes: bool,
    },
    /// `/rename` (bar A.5): `agent` is called `to` from now on (the old
    /// name still works); an invalid or taken name is an `error`
    Rename { project: Project, agent: String, to: String },
    /// `/model` of one agent (bar A.5): its model from its next call;
    /// `default` also makes it config.toml's for its role. An unknown or
    /// unusable model is an `error`; the change shows in `agents`
    Model {
        project: Project,
        agent: String,
        model: String,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        default: bool,
    },
    /// `/reasoning` of one agent (bar A.5): its reasoning effort; one its
    /// model doesn't take is an `error`; the change shows in `agents`
    Effort { project: Project, agent: String, effort: String },
    /// a held route goes to project `to` instead (a hub id of the
    /// registry, else an error; "bise": his words stay with bise's main,
    /// a cancel); a route that ended already: an error
    RouteCorrect { project: Project, rid: u64, to: String },
    /// a held route doesn't go: his words reach bise's main as a plain
    /// message (a route that ended already: an error)
    RouteCancel { project: Project, rid: u64 },
    /// tell me when it's done (S10, decision E): a flag on the task, one
    /// job (off again at its `job_end`); `on: false` stops following
    Follow { project: Project, agent: String, on: bool },
    /// draft (architect m_10724, m_10789): a slash line he typed in
    /// `agent`'s view (the TUI's focus), parsed by the TUI's own router
    /// (`/rename`, `/answer`, `/close`, `/archive`, `/restore`,
    /// `/isolate`, `/stop`, `/compact`, `/flow`...) and run by the same
    /// handlers. A bad line: `error` with the router's words and `cid`;
    /// a refusal: `error` with sb-core's words and `cid`; a success
    /// shows in the events it moves (`agents`, `prs`, an entry), `/flow`'s
    /// answer as a `notice`. Plain text is refused: that's `send`
    Slash {
        project: Project,
        agent: String,
        line: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cid: Option<u64>,
        /// client-protocol step 3 (architect m_13089 change 3): any line
        /// he typed, with how it goes; a line that isn't a slash command
        /// (his words, an `@route`) takes `send`'s path, a queued slash
        /// command is refused (a command never waits)
        #[serde(flatten)]
        opts: SendOpts,
    },
    /// run scheduled task `id` now (the TUI's /scheduled `r`); an unknown
    /// or ended id is an `error`
    ScheduledRun { project: Project, id: u64 },
    /// `/artifacts add <path or link>` in `agent`'s view (`title` his):
    /// the hub's words come back as the result, a refusal as an `error`
    ArtifactsAdd {
        project: Project,
        agent: String,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// the TUI's focus moved to `focus` (an agent's name): the hub's
    /// "who is he looking at" (sb-core's ClientFocus)
    Focus { project: Project, focus: String },
    /// `/version`'s picker opened: `versions` answers
    Versions { project: Project },
    /// `/version` or `/version list`: the hub's words on its versions
    VersionInfo { project: Project },
    /// `/version <v>`: switch to it (built first when needed)
    VersionSwitch { project: Project, to: String },
    /// `/version back`: the version before
    VersionRollback { project: Project },
    /// `/restart [<v>]`: hub, REPLs and clients restart (on `to`, else on
    /// the running version)
    VersionRestart {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
    /// `/update`: look for a new release now (bise's source tree: build
    /// its HEAD); its words come as a `notice`
    VersionUpdate { project: Project },
    /// `/release-bise` (dev build only): what it would release (`release`
    /// answers, state "plan" or "error"); `dry`: a dry run's plan
    ReleasePlan {
        project: Project,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        dry: bool,
    },
    /// the plan's go: release `tag` at `commit`; its steps come as
    /// `release` to every client; one at a time, a refusal is an `error`
    ReleaseRun {
        project: Project,
        tag: String,
        commit: String,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        dry: bool,
    },
    /// a tag this version doesn't know: answered with `error`
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl HubCmd {
    pub const TAGS: &'static [&'static str] = &["hello", "subscribe", "unsubscribe", "page", "send", "answer", "close", "confirm", "approvals", "remove_rule", "stop", "archive", "unarchive", "artifacts_seen", "tool_out", "diff", "worktrees", "dev_servers", "merged", "features", "prs", "scheduled", "scheduled_stop", "models", "new", "rename", "model", "effort", "route_correct", "route_cancel", "follow", "slash", "branches", "scheduled_run", "artifacts_add", "focus", "versions", "version_info", "version_switch", "version_rollback", "version_restart", "version_update", "release_plan", "release_run"];

    pub fn decode(line: &str) -> Result<HubCmd, String> {
        Self::from_value(parse(line)?)
    }

    pub fn from_value(v: Value) -> Result<HubCmd, String> {
        decode(v, "cmd", Self::TAGS, |tag, raw| HubCmd::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            HubCmd::Unknown { raw, .. } => raw.clone(),
            c => serde_json::to_value(c).expect("a HubCmd encodes"),
        }
    }

    pub fn encode(&self) -> String {
        self.to_value().to_string()
    }

    /// The project it is for (none: `hello`, an unknown tag).
    pub fn project(&self) -> Option<&str> {
        let v = match self {
            HubCmd::Subscribe { project, .. }
            | HubCmd::Unsubscribe { project, .. }
            | HubCmd::Page { project, .. }
            | HubCmd::Send { project, .. }
            | HubCmd::Answer { project, .. }
            | HubCmd::Close { project, .. }
            | HubCmd::Confirm { project, .. }
            | HubCmd::Approvals { project, .. }
            | HubCmd::RemoveRule { project, .. }
            | HubCmd::Stop { project, .. }
            | HubCmd::Archive { project, .. }
            | HubCmd::Unarchive { project, .. }
            | HubCmd::ArtifactsSeen { project }
            | HubCmd::Worktrees { project }
            | HubCmd::DevServers { project }
            | HubCmd::Merged { project }
            | HubCmd::Features { project }
            | HubCmd::Prs { project }
            | HubCmd::Scheduled { project }
            | HubCmd::ScheduledStop { project, .. }
            | HubCmd::Models { project }
            | HubCmd::New { project, .. }
            | HubCmd::Rename { project, .. }
            | HubCmd::Model { project, .. }
            | HubCmd::Effort { project, .. }
            | HubCmd::RouteCorrect { project, .. }
            | HubCmd::RouteCancel { project, .. }
            | HubCmd::Follow { project, .. }
            | HubCmd::Slash { project, .. }
            | HubCmd::Branches { project }
            | HubCmd::ScheduledRun { project, .. }
            | HubCmd::ArtifactsAdd { project, .. }
            | HubCmd::Focus { project, .. }
            | HubCmd::Versions { project }
            | HubCmd::VersionInfo { project }
            | HubCmd::VersionSwitch { project, .. }
            | HubCmd::VersionRollback { project }
            | HubCmd::VersionRestart { project, .. }
            | HubCmd::VersionUpdate { project }
            | HubCmd::ReleasePlan { project, .. }
            | HubCmd::ReleaseRun { project, .. }
            | HubCmd::Diff { project, .. } => project,
            HubCmd::Hello { .. } | HubCmd::Unknown { .. } => return None,
        };
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tags_are_kept_and_bad_fields_are_errors() {
        let e = HubEv::decode(r#"{"ev":"weather","project":"p","to":"q"}"#).unwrap();
        assert!(matches!(&e, HubEv::Unknown { tag, .. } if tag == "weather"));
        assert_eq!(e.encode(), r#"{"ev":"weather","project":"p","to":"q"}"#);
        assert!(HubEv::decode(r#"{"ev":"typing","project":"p"}"#).is_err(), "a known tag without its fields");
        assert!(HubEv::decode(r#"{"project":"p"}"#).is_err());
        let c = HubCmd::decode(r#"{"cmd":"approve","project":"p","card":3}"#).unwrap();
        assert!(matches!(c, HubCmd::Unknown { .. }), "approve is answer's path: not a command");
        // unknown fields are ignored
        let t = HubEv::decode(r#"{"ev":"typing","project":"p","agent":"a","text":"x","new":1}"#).unwrap();
        assert_eq!(t, HubEv::Typing { project: "p".into(), agent: "a".into(), text: "x".into() });
        assert_eq!(t.tag(), "typing");
    }

    /// hub-skew (architect m_11314): a hub older than `cmds` says none, a
    /// newer one lists its tags.
    #[test]
    fn a_welcome_without_cmds_is_an_older_hub() {
        let old = HubEv::decode(r#"{"ev":"welcome","project":"p","proto":1,"workspace":"/w","name":"w"}"#).unwrap();
        assert!(matches!(&old, HubEv::Welcome { cmds, .. } if cmds.is_empty()));
        let new = HubEv::decode(r#"{"ev":"welcome","project":"p","proto":1,"workspace":"/w","name":"w","cmds":["hello","slash"]}"#).unwrap();
        assert!(matches!(&new, HubEv::Welcome { cmds, .. } if cmds == &["hello", "slash"]));
    }
}

/// draft: the kind of an [`HubEv::Error`], one enum for every typed
/// failure the window draws by kind (architect m_11379): a kind this
/// version doesn't know reads as Unknown. Only the desktop core sets
/// these, about a hub (a hub never says it is older or refused: its own
/// errors carry none); the window draws by kind, and falls back to the
/// text for Unknown or none, never matching the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// the project's hub is older than this core: the command is one it
    /// doesn't know (amb-feed's hub skew, Welcome.cmds)
    HubOlder,
    /// the project's hub refused this core's connection (issue 16: a
    /// process an agent started); nothing is sent until he retries
    HubRefused,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod size {
    /// A client keeps events in enums of its own (the core's `Read`):
    /// one big variant would weigh on all of them (clippy's
    /// large_enum_variant); box the rare big ones.
    #[test]
    fn a_hub_event_stays_small() {
        let n = std::mem::size_of::<super::HubEv>();
        assert!(n <= 264, "HubEv is {n} bytes");
    }
}
