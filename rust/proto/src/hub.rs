//! The hub side of the protocol, v1's frozen part: [`HubEv`] (hub ->
//! client, tagged `"ev"`) and [`HubCmd`] (client -> hub, tagged `"cmd"`).
//! A hub can only emit a `HubEv`; the core's and the app's own messages
//! (projects, open, project_add) are other types ([`crate::draft`]), so
//! they can't reach a hub or come from one.

use crate::context::FnContext;
use crate::diff::{DiffFile, DiffResult, DiffView};
use crate::ops::{BranchRow, ReleaseEv, UpdateEv, VersionItem};
use crate::parts::Part;
use crate::rows::{Agent, ApprovalMode, ApprovalRule, Artifact, ArtifactMoved, Card, Folder, FolderOp, LibraryProject, CheckerKind, Feature, Merged, Model, Place, Pr, Proc, ScheduledTask, UsageRow, Worktree};
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
        /// when this hub's bise was built (its VERSION `built=`, from
        /// `initialize`'s version); none: a dev build or an older hub. The
        /// core compares it with its own (bise_home::release::built_after):
        /// a hub newer than the app reads the app's update channel at once
        /// (desktop-dist, architect m_25198)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        built: Option<String>,
    },
    /// every agent of the hub, on change; `places` (P4c-4a, none from an
    /// older hub): the project's worktrees as the terminal's panel draws
    /// them, from the same snapshot (an agent's `place_id` is one's id)
    /// agents-light (issue 36, architect m_37220): to a client that said
    /// `capabilities.archived_list` (rpc::CAP_ARCHIVED_LIST), `agents` is
    /// the live agents only and `archived` the light list of the others:
    /// `None` = unchanged since this connection's last agents event, `Some`
    /// = the whole list (always on a connection's first: initialize,
    /// hub/read). A client merges both with `rows::Archived`.
    Agents {
        project: Project,
        agents: Vec<Agent>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        places: Vec<Place>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        archived: Option<Vec<crate::rows::ArchivedAgent>>,
    },
    /// every open card of the hub that is his, on change; `others`
    /// (P4c-4a, none from an older hub): the rest, bise's own (a drop, a
    /// done, a blocked...), which the terminal's inbox draws too: both in
    /// the hub's order (merged by id: the snapshot's order)
    Cards {
        project: Project,
        cards: Vec<Card>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        others: Vec<Card>,
    },
    /// one page of a thread, newest last: the answer to `subscribe` and
    /// `page`; `before` the oldest entry's pos when there is more
    Thread { project: Project, agent: String, entries: Vec<Entry>, before: Option<Pos>, more: bool },
    /// the answer to `find` (`thread/find`): every entry of the thread up
    /// to the line `upto` that holds `needle` (`thread::find`'s rule),
    /// oldest first, at most `thread::find::MAX_HITS` (the newest kept);
    /// `rid` the find's own. Entries after `upto` the client matches itself
    /// as they come.
    ThreadHits { project: Project, agent: String, rid: u64, needle: String, hits: Vec<crate::thread::find::FindHit>, upto: Pos },
    /// a subscribed thread's entry, live: a new pos appends, a known pos
    /// replaces (a tools entry growing, a card answered)
    Entry { project: Project, agent: String, entry: Box<Entry> },
    /// a subscribed agent's current step ("" when its turn ends)
    Typing { project: Project, agent: String, text: String },
    /// a subscribed agent's answer as it streams in (at most ~4 a
    /// second): `call` its `<call>.<attempt>`, `text` what goes at `off`
    /// of the text so far, `off` in UTF-16 code units (as JS indexes a
    /// string, like VoiceLit.said; architect m_17866): the client
    /// keeps `text[..off] + text` (thread::lines::LiveText, so a repeated
    /// event is harmless), never the whole text again (architect m_17813:
    /// whole texts were quadratic in the reply). Subscribe sends the
    /// whole text once at off 0; "" at off 0 when it ends (its entry
    /// came, the turn ended, a stop, an error, a retry)
    Partial { project: Project, agent: String, call: String, off: u32, text: String },
    /// every artifact of the project, at hello and on change; `new`: made
    /// or changed since he last looked (`artifacts_seen` clears it)
    /// `seen_ms`: when he last looked (an item's `new` is after it, not
    /// his own); none from an older hub
    /// `folders`: the library's folders (every project's, the live ones),
    /// read at send time; an item's `folder` is one of them or none. A
    /// hub sends it again when the library file changed (his edit in the
    /// desktop core); none from an older hub
    Artifacts {
        project: Project,
        items: Vec<Artifact>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seen_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        folders: Vec<Folder>,
    },
    /// the answer to `artifacts_move`: each artifact moved and its folder
    /// before (his undo)
    ArtifactsMoved { project: Project, moved: Vec<ArtifactMoved> },
    /// the answer to `artifacts_folders`: the folders after his change;
    /// `made` the folder a make made (or found)
    Folders {
        project: Project,
        folders: Vec<Folder>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        made: Option<String>,
    },
    /// the answer to `artifacts_library`: the other registered projects'
    /// artifacts, read from their stores (this hub's own are in
    /// `artifacts`), and the folders; `here` this project's own name in
    /// the registry (its rows' project name)
    Library { project: Project, here: String, projects: Vec<LibraryProject>, folders: Vec<Folder> },
    /// the live scheduled tasks (`sb every`) of the project, at hello, on
    /// `scheduled` and whenever one is set, runs, stops or ends
    /// `ended` (P4c-4a, none from an older hub): the ones that ended in
    /// the last 7 days, oldest first (the scheduled screen's tab)
    Scheduled {
        project: Project,
        items: Vec<ScheduledTask>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        ended: Vec<ScheduledTask>,
    },
    /// the repo's worktrees (the agents' and his others), at hello, when
    /// the agents change (a turn ends, one comes or goes) and on
    /// `worktrees`
    Worktrees { project: Project, items: Vec<Worktree> },
    /// the processes the project's agents left running, then the ones
    /// that ended in the last 24 h (docs/processes.md): on `processes`,
    /// every 2 s while a connection watches the list, and when a running
    /// count changes
    Processes { project: Project, items: Vec<Proc> },
    /// a range of a process's output (the answer to `proc_out`, and its
    /// notifications while `proc_follow` is on, this connection only):
    /// `data` the bytes from offset `at`, base64; `end` the output's size
    /// when read (`at` + the bytes < `end`: more to read). `at` past the
    /// asked `from`: the start was no longer kept
    ProcOut {
        project: Project,
        agent: String,
        id: String,
        at: u64,
        data: String,
        end: u64,
    },
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
    /// his queued message taken back (the answer to `queued_take`, this
    /// connection only): it will never be delivered; `text` his words, for
    /// the window's "edit" (put back in the composer); `parts` his typed
    /// parts when he sent some (msg-parts: the chips come back from
    /// them), empty for an older message
    QueuedTaken {
        project: Project,
        agent: String,
        id: u64,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<Part>,
    },
    /// a page of an agent's raw session (the answer to `log`, this
    /// connection only; `/debug history`): rows oldest first, every text
    /// redacted; `more`: older rows remain (ask with `before` = the first
    /// row's pos). With `find`: `hits`, the pos of every row that holds
    /// it in the whole history, oldest first, and the page ends at the
    /// newest hit
    Log {
        project: Project,
        agent: String,
        items: Vec<crate::log::LogItem>,
        more: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        hits: Option<Vec<u64>>,
    },
    /// `/debug completion` (the answer to `completion`, this connection
    /// only): request `slot` of the agent's ring as its provider got it,
    /// cut into rows (`bise_session::completion`), or as `shown_as`'s
    /// family would get it (the runtime's own render, no call made);
    /// `slots`: every request the ring keeps, oldest first
    Completion(Box<crate::log::Completion>),
    /// a `/debug` body whole (the answer to `log_body`, this connection
    /// only): `of` echoed (the window swaps the body of that row only),
    /// `text` redacted and cut at [`crate::log::WHOLE_CAP`] (`cut`),
    /// `total` the whole body's bytes ('1 MB of 3.2 MB shown')
    LogBody {
        project: Project,
        agent: String,
        of: crate::log::BodyOf,
        text: String,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        cut: bool,
        total: u64,
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
    /// `/update`'s build in bise's source tree (dev-update): its start,
    /// its end ([`UpdateEv`], boxed as `Release`), to every client
    Update(Box<UpdateEv>),
    /// bise's home hub holds his words for a project's main (desktop S2,
    /// decision B): `to` the project he addressed (`@<project> …`, its hub
    /// id), `name` its name, `why` the reason; until `correct_until_ms` (2 s after) a
    /// `route_correct` or `route_cancel` with this `rid` still changes it;
    /// `route` comes again after a correction (same rid, same time)
    Route { project: Project, rid: u64, to: Project, name: String, text: String, correct_until_ms: u64, why: String },
    /// the route's end: `state` "sent" (forwarded: `xid` its delivery,
    /// `msg` his message it became in bise's main: his pending send there
    /// settles by it) or "cancelled" (his words stayed with bise's main)
    RouteDone {
        project: Project,
        rid: u64,
        state: String,
        to: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        xid: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        msg: Option<u64>,
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
    /// les bims: a message sent or a report made, hub-wide, as it
    /// happens (crate::activity: no body, no text)
    Activity { project: Project, item: crate::activity::ActivityEv },
    /// the repo's flow (`config.toml`'s `[flow] mode`), on change; none:
    /// not set
    Flow {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        flow: Option<crate::rows::FlowMode>,
    },
    /// the project's pause (cost-pause, `/pause`): at hello and on
    /// change. While `paused`, no agent's turn runs: every message, timer
    /// wake and event wake is held, queued in order, and delivered at
    /// `resume`. Background commands and `sb wake` watches keep running
    /// (their wake waits too); the REPLs stay up, idle. `since_ms`: when
    /// it was paused. One hub: a desktop's "pause everything" pauses each
    Paused {
        project: Project,
        paused: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since_ms: Option<u64>,
        /// the messages held so far (queued for an agent, not delivered)
        #[serde(default, skip_serializing_if = "crate::rows::is_zero_u64")]
        held: u64,
    },
    /// the hub's newest version switch (`/reload`, `/restart`,
    /// `/version`; architect m_20576): at hello and when its phase moves.
    /// None: no switch known. Every hub says it again at hello, so a
    /// window that reconnects through the switch still reads where it is
    Switch {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        switch: Option<crate::rows::SwitchRow>,
    },
    /// how much the project has done (les bims' growing office,
    /// rows::growth): at hello and when a counter moves
    Growth { project: Project, growth: crate::rows::Growth },
    /// the project's pulse (the H1 sidebar, rows::pulse): at hello, then
    /// at most once a minute and only when it changed
    Pulse { project: Project, pulse: crate::rows::Pulse },
    /// a newer bise to update to, as this hub sees it (update-flow,
    /// rows::bise_update): at hello and when it changes; None: nothing
    /// to show (on it, or no release known)
    BiseUpdate {
        project: Project,
        #[serde(default)]
        update: Option<crate::rows::BiseUpdate>,
    },
    /// what the project's model calls cost (`usage/read`, an answer only:
    /// never pushed). `counting`: the hub is still reading the
    /// transcripts (its first read): the sums are not complete yet, ask
    /// again. `all` the whole project (name ""), `agents` and `models`
    /// sorted by the week's cost. Today = since the hub's local midnight
    /// (its UTC offset), week = the last 7 local days, today included
    Usage {
        project: Project,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        counting: bool,
        /// boxed: HubEv stays small (hub::size)
        all: Box<UsageRow>,
        agents: Vec<UsageRow>,
        models: Vec<UsageRow>,
    },
    /// the hub's bise pages, newest first (docs/ambient-pages.md §2.3),
    /// and `overdue` his late promises across them (checklist rows of his
    /// past their data-due: the morning page's and the menu bar's count,
    /// never a card): at hello and when either changed
    Pages { project: Project, items: Vec<crate::rows::Page>, overdue: u32 },
    /// one page published, updated or answered (its new version, its
    /// state): to every client, before `pages` (boxed: the enum stays
    /// small, the wire is its fields next to the tag). Not `page`: the
    /// hub's older `page` line keeps its meaning for the readers that
    /// take it raw (the desktop core's `page`)
    PageChanged {
        project: Project,
        #[serde(flatten)]
        page: Box<crate::rows::Page>,
    },
    /// update-card's new release: open its item (to the connection whose
    /// `/update` found it)
    CardOpen { project: Project, id: u64 },
    /// MCP Apps C9: a terminal asked the app to open this (`app/open`); to
    /// the connections that said `desktop` only
    AppOpen {
        project: Project,
        what: crate::app_rows::OpenWhat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// one page of the inbox's history (`inbox_history`'s answer)
    InboxHistory {
        project: Project,
        #[serde(flatten)]
        page: crate::inbox::InboxPage,
    },
    /// a card closed (answered or withdrawn) or its answer's thread
    /// position came: its record, whole, to every client
    InboxRecorded { project: Project, record: Box<crate::inbox::InboxRecord> },
    /// the hub moved this connection's focus (an agent it was on is
    /// gone: back to main)
    Focused { project: Project, focus: String },
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
    /// confirm answered no: "@x stays: nothing archived"; `/flow`'s answer to
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
    pub const TAGS: &'static [&'static str] = &["welcome", "agents", "cards", "thread", "thread_hits", "entry", "typing", "partial", "artifacts", "artifacts_moved", "folders", "library", "scheduled", "worktrees", "processes", "proc_out", "merged", "features", "prs", "models", "tool_out", "queued_taken", "log", "completion", "log_body", "diff", "branches", "versions", "release", "update", "route", "route_done", "jobs", "job_end", "followed_end", "activity", "flow", "paused", "switch", "growth", "pulse", "bise_update", "usage", "pages", "page_changed", "card_open", "app_open", "inbox_history", "inbox_recorded", "focused", "approvals", "notice", "refused", "error"];

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
/// keeps it on his thread entry (his files go as `Send.parts`; the
/// older desktops' `files` list was removed in issue 31, an older line's
/// `files` is ignored); `voice` said in voice mode; `via` the client part that sent it (`capsule`,
/// `capsule-start`, `ambient`: the capsule's hint for main).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SendOpts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<FnContext>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub voice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// the thread he typed it in, when not the send's agent's (at-start:
    /// his `@agent …` from another thread): the hub leaves one route row
    /// there. Only a trace: an unknown or archived one writes none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

impl SendOpts {
    pub fn queued(&self) -> bool {
        self.mode == Some(Mode::Queued)
    }
}

/// What a client asks a hub.
// one command is decoded at a time and never stored in bulk: the size of
// its largest variant (Send, with its context and parts) costs nothing
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HubCmd {
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
    /// ⌘F over a whole thread (`thread/find`): the hub reads its
    /// transcript off its loop and answers `thread_hits`; a newer find of the
    /// same connection and agent stops an older one still running
    Find { project: Project, agent: String, needle: String, rid: u64 },
    /// his words to an agent (main included), as the TUI sends them, with
    /// how they go ([`SendOpts`]: mode, context, voice, via)
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
        /// his message as typed parts (msg-parts, [`crate::parts`]): the
        /// hub renders the model's text from them (`text` is then
        /// ignored) and keeps them on his message, so every client draws
        /// his quotes, pastes, images and files as chips. Empty: `text`
        /// alone. A part this hub doesn't know is refused (it would drop
        /// words)
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<Part>,
    },
    /// a card's answer: an option's number or words (approvals and merges
    /// included: the hub's one /answer path). What he pasted or dropped
    /// with it goes as `parts` (the older desktops' `files` list was
    /// removed in issue 31; an older line's `files` is ignored)
    Answer {
        project: Project,
        card: u64,
        reply: String,
        /// his answer as typed parts, as `Send.parts` (`reply` then ignored)
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<Part>,
    },
    /// close a card without answering it (the TUI's `/close N`): the
    /// hub's one close path (sb-core's `close`), a refusal as `error`
    Close { project: Project, card: u64 },
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
    /// comes again. `at_ms`: when he looked (a request that waited in
    /// the socket while the hub booted never swallows what came after;
    /// none: now)
    ArtifactsSeen {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at_ms: Option<u64>,
    },
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
    /// the window's `/log`: a page of `agent`'s raw session, the newest
    /// rows with a seq under `before` (none: the newest page). The user's
    /// authority: a client's only, never an agent's op (issue 16: it
    /// reads his raw sessions)
    Log {
        project: Project,
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        before: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        find: Option<String>,
    },
    /// `/debug completion`: request `slot` of `agent`'s ring (none: the
    /// newest), as sent, or as `provider` (a `provider/model` or a
    /// provider of the catalog) would get it; `raw`: each row its bytes
    /// as sent; `find`: the hits. The user's authority, a client's only
    Completion(Box<crate::log::CompletionAsk>),
    /// `/debug`'s 'show more': the body `of` names, whole (up to
    /// [`crate::log::WHOLE_CAP`]), read from the session log or the
    /// request ring; `log_body` answers this connection. The user's
    /// authority, a client's only (issue 16)
    LogBody { project: Project, agent: String, of: crate::log::BodyOf },
    /// his queued message `id` (an `Agent.queued` row) taken back before
    /// its turn ends (the window's "edit · drop"): `queued_taken` answers
    /// this connection with its words, or an error says why (delivered,
    /// not his, unknown). The user's authority: a client's only (issue 16)
    QueuedTake { project: Project, agent: String, id: u64 },
    /// his queued message `id` said now (the window's "send now"): sb-core
    /// delivers it into the agent's running turn (or a turn of its own),
    /// its id and parts kept; the agents event no longer lists it. An
    /// error says why it stayed (delivered, taken, not his, the agent
    /// down). The user's authority: a client's only (issue 16)
    QueuedSteer { project: Project, agent: String, id: u64 },
    /// `/diff`'s picker opened: `branches` answers
    Branches { project: Project },
    /// the environments screen opened: `worktrees` comes again
    Worktrees { project: Project },
    /// the processes list asked for: `processes` answers; `watch` true:
    /// it comes again every 2 s (cpu, memory, ports) until `watch` false
    /// or the connection goes; absent: once
    Processes {
        project: Project,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        watch: Option<bool>,
    },
    /// a range of process `id`'s output (`agent`'s): from byte `from`,
    /// none for its last 64 KB; `proc_out` answers, at most 64 KB. The
    /// user's authority: a client's only (issue 16)
    ProcOut {
        project: Project,
        agent: String,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<u64>,
    },
    /// follow its output: while `on`, `proc_out` comes to this
    /// connection as the output grows (every 200 ms at most, capped
    /// chunks), from its end when it starts. The user's authority
    ProcFollow { project: Project, agent: String, id: String, on: bool },
    /// stop process `id` of `agent`, or `all` its running ones (exactly
    /// one of the two): TERM, then KILL 5 s later; its row becomes
    /// `stopped_by_you` and the agent gets a queued note. The user's
    /// authority: a client's only, never an agent's op (issue 16)
    ProcStop {
        project: Project,
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        all: bool,
    },
    /// the changes tab opened: `merged` comes again
    Merged { project: Project },
    /// the features view opened: `features` comes again
    Features { project: Project },
    /// the PR list opened: `prs` comes again
    Prs { project: Project },
    /// the inbox's history (inbox-record): one page of his answered and
    /// withdrawn cards, newest first, answered by `inbox_history`
    InboxHistory {
        project: Project,
        #[serde(flatten)]
        q: crate::inbox::HistoryParams,
    },
    /// the scheduled tasks asked for: `scheduled` comes again
    Scheduled { project: Project },
    /// what the project's calls cost: answered by `usage`
    Usage { project: Project },
    /// pause the whole project (his only, never an agent's): every busy
    /// turn stops, messages, timers and wakes are held (`paused` comes
    /// to every client). Already paused: nothing changes
    Pause { project: Project },
    /// resume a paused project: the held messages are delivered in their
    /// order, the due timers fire. Not paused: nothing changes
    Resume { project: Project },
    /// MCP Apps C9: open this in the desktop app (the TUI's `app ↗`): the
    /// hub forwards it as `app_open` to the connections that said
    /// `desktop`, and answers how many ([`crate::rpc::AppOpenResult`])
    AppOpen {
        project: Project,
        what: crate::app_rows::OpenWhat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// stop scheduled task `id` (his, like the TUI's /scheduled stop: its
    /// agent hears it, its line says `stopped by you`); an unknown or
    /// ended id is an `error`
    ScheduledStop { project: Project, id: u64 },
    /// stop watch `id` (his, like `sb wake --stop` by its agent: the
    /// TUI's /scheduled x, the desktop's 'stop watching' on an alarm):
    /// its agent's thread says `stopped waiting for …`; an unknown or
    /// ended id is an `error`
    WakeStop { project: Project, id: u64 },
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
        /// his words' typed parts (msg-parts, as `Send.parts`): a line
        /// that isn't a command takes `send`'s path with them (`line` is
        /// then ignored); a command never has any
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<Part>,
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
        /// a library folder's id: the artifact goes there
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// his move of artifacts to a library folder (none: unfiled), all or
    /// none: `artifacts_moved` answers. `to`: the project they are in
    /// (a hub id or a registry name; none: this one), another is
    /// forwarded to its hub, started if needed
    ArtifactsMove {
        project: Project,
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        /// the undo slot is the caller's (the desktop core's move across
        /// projects records it whole): the hub leaves it
        #[serde(default, skip_serializing_if = "crate::is_false")]
        keep_undo: bool,
    },
    /// his change of the library's folders: `folders` answers
    ArtifactsFolders { project: Project, op: FolderOp },
    /// the other projects' artifacts, read once: `library` answers
    ArtifactsLibrary { project: Project },
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
    /// update now (update-flow): install the release the machine offers
    /// (`bise_update`'s) and switch every project to it, one at a time.
    /// His click only: a client socket's, never an agent's op (issue 16)
    BiseUpdate { project: Project },
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
    /// `bise stop`: the hub stops (its clients' connections end);
    /// `keep_agents`: the REPLs keep running (their turns too) for the
    /// next hub to adopt (a version switch, a hub restart)
    StopHub {
        project: Project,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        keep_agents: bool,
    },
    /// a note talk on page `page` (the desktop core's voice): `phase`
    /// start, heard (his words so far), end, send, cancel; `text` the
    /// words; the page shows them at its mouse (pages' `voice` push). A
    /// page the hub doesn't hold: nothing
    PageVoice {
        project: Project,
        page: String,
        phase: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        text: String,
    },
    /// mcp-apps (docs/mcp-apps.md, C5): a view's HTML, its sandbox and its
    /// call's input and result (`view/resource`); its answer is
    /// [`crate::view_rpc::ViewResource`] or a [`crate::view_rpc::ViewError`]
    ViewResource { project: Project, agent: String, call: String },
    /// a view's request to its own server (`view/call`): `method`
    /// `tools/call` or `resources/read`, `params` as the view sent them;
    /// its answer is the server's result, or a view error
    ViewCall {
        project: Project,
        agent: String,
        call: String,
        method: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
        params: Value,
    },
    /// a view's `ui/message` (`view/message`): the spec's content blocks
    /// (text only); `gesture_ms`: when Electron's main last saw his
    /// pointer-down in that view's frame (main stamps it, never the
    /// renderer); answered `{}` once the message is in the thread
    ViewMessage {
        project: Project,
        agent: String,
        call: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "Array<Record<string, unknown>>"))]
        content: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gesture_ms: Option<u64>,
    },
    /// a view's `ui/update-model-context` (`view/context`): its content
    /// replaces this view's context, which joins his next send to the
    /// agent once
    ViewContext {
        project: Project,
        agent: String,
        call: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "Array<Record<string, unknown>>"))]
        content: Value,
    },
    /// mcp-apps (C8, the add's step 2): plugin `plugin`'s views as its
    /// servers declare them, read through the hub's views bridge
    /// (`<base>/<plugin>/~ui`); its answer is the bridge's
    /// `{views: [...]}` as is ([`crate::view_rpc::PluginViewsResult`])
    PluginViews { project: Project, plugin: String },
    /// a tag this version doesn't know: answered with `error`
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl HubCmd {
    pub const TAGS: &'static [&'static str] = &["subscribe", "unsubscribe", "page", "find", "send", "answer", "close", "approvals", "remove_rule", "stop", "archive", "unarchive", "artifacts_seen", "tool_out", "log", "completion", "log_body", "queued_take", "queued_steer", "diff", "worktrees", "processes", "proc_out", "proc_follow", "proc_stop", "merged", "features", "prs", "inbox_history", "scheduled", "scheduled_stop", "wake_stop", "models", "new", "rename", "model", "effort", "route_correct", "route_cancel", "follow", "slash", "branches", "scheduled_run", "artifacts_add", "artifacts_move", "artifacts_folders", "artifacts_library", "focus", "versions", "version_info", "version_switch", "version_rollback", "version_restart", "version_update", "bise_update", "release_plan", "release_run", "stop_hub", "page_voice", "usage", "pause", "resume", "view_resource", "view_call", "view_message", "view_context", "plugin_views", "app_open"];

    pub fn decode(line: &str) -> Result<HubCmd, String> {
        Self::from_value(parse(line)?)
    }

    /// A command carrying a removed field with data ([`crate::REMOVED`])
    /// is refused by name, never read with its data dropped.
    pub fn from_value(v: Value) -> Result<HubCmd, String> {
        if let Some(r) = crate::removed_in(&v, "cmd") {
            return Err(crate::removed_refusal(r));
        }
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
            | HubCmd::Find { project, .. }
            | HubCmd::Send { project, .. }
            | HubCmd::Answer { project, .. }
            | HubCmd::Close { project, .. }
            | HubCmd::Approvals { project, .. }
            | HubCmd::RemoveRule { project, .. }
            | HubCmd::Stop { project, .. }
            | HubCmd::Archive { project, .. }
            | HubCmd::Unarchive { project, .. }
            | HubCmd::ArtifactsSeen { project, .. }
            | HubCmd::Worktrees { project }
            | HubCmd::Processes { project, .. }
            | HubCmd::ProcOut { project, .. }
            | HubCmd::ProcFollow { project, .. }
            | HubCmd::ProcStop { project, .. }
            | HubCmd::Merged { project }
            | HubCmd::Features { project }
            | HubCmd::Prs { project }
            | HubCmd::InboxHistory { project, .. }
            | HubCmd::Scheduled { project }
            | HubCmd::Usage { project }
            | HubCmd::Pause { project }
            | HubCmd::Resume { project }
            | HubCmd::AppOpen { project, .. }
            | HubCmd::ScheduledStop { project, .. }
            | HubCmd::WakeStop { project, .. }
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
            | HubCmd::ArtifactsMove { project, .. }
            | HubCmd::ArtifactsFolders { project, .. }
            | HubCmd::ArtifactsLibrary { project }
            | HubCmd::Focus { project, .. }
            | HubCmd::Versions { project }
            | HubCmd::VersionInfo { project }
            | HubCmd::VersionSwitch { project, .. }
            | HubCmd::VersionRollback { project }
            | HubCmd::VersionRestart { project, .. }
            | HubCmd::VersionUpdate { project }
            | HubCmd::BiseUpdate { project }
            | HubCmd::ReleasePlan { project, .. }
            | HubCmd::ReleaseRun { project, .. }
            | HubCmd::StopHub { project, .. }
            | HubCmd::PageVoice { project, .. }
            | HubCmd::Diff { project, .. }
            | HubCmd::ToolOut { project, .. }
            | HubCmd::Log { project, .. }
            | HubCmd::LogBody { project, .. }
            | HubCmd::QueuedTake { project, .. }
            | HubCmd::QueuedSteer { project, .. }
            | HubCmd::ViewResource { project, .. }
            | HubCmd::ViewCall { project, .. }
            | HubCmd::ViewMessage { project, .. }
            | HubCmd::ViewContext { project, .. }
            | HubCmd::PluginViews { project, .. } => project,
            HubCmd::Completion(c) => &c.project,
            HubCmd::Unknown { .. } => return None,
        };
        Some(v)
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
mod tests;
