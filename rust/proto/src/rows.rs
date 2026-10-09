//! The rows a hub lists: its agents and its open cards (the `agents` and
//! `cards` events; S1's `view.json` reuses them), and the split of a
//! question's trailing numbered options.

use crate::Project;
use serde::{Deserialize, Serialize};

/// An agent's status as released (v2026.10.2-28, the wire rule in
/// lib.rs: its six values and their meanings never change): starting is
/// working, stopped is done; archived is a flag. The hub's exact word
/// when it is one of those is [`Agent::phase`] (P4b-fix, architect
/// m_14382).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Working,
    Idle,
    Waiting,
    Blocked,
    Done,
    Failed,
}

impl Status {
    /// In a turn (or starting one): what a view draws as working.
    pub fn working(self) -> bool {
        self == Status::Working
    }

    /// The hub's status word (`model::Status::as_str`): the status and
    /// whether the agent is archived (an archived one shows done).
    pub fn of_hub(word: &str) -> (Status, bool) {
        match word {
            "starting" | "working" => (Status::Working, false),
            "waiting" => (Status::Waiting, false),
            "blocked" => (Status::Blocked, false),
            "failed" => (Status::Failed, false),
            "done" | "stopped" => (Status::Done, false),
            "archived" => (Status::Done, true),
            _ => (Status::Idle, false),
        }
    }
}

/// The hub's word when [`Status`] folds it into a released value: its
/// REPL is starting (status working), or the user stopped it (status
/// done). New in P4b-fix, with `Unknown` from the start so it may grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// its REPL is starting (its first turn not begun)
    Starting,
    /// stopped by the user (`sb stop`, a drop on its way)
    Stopped,
    /// a phase this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

impl Phase {
    /// The phase the hub's status word says, if any.
    pub fn of_hub(word: &str) -> Option<Phase> {
        match word {
            "starting" => Some(Phase::Starting),
            "stopped" => Some(Phase::Stopped),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Progress,
    Done,
    Failed,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Report {
    pub kind: ReportKind,
    pub text: String,
    pub at_ms: u64,
}

/// One agent of a hub.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Agent {
    pub name: String,
    pub main: bool,
    pub status: Status,
    /// the hub's exact word when `status` folds it (starting: working,
    /// stopped: done)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
    pub archived: bool,
    /// what it says it does now, else its last report, else its
    /// objective: one line (the TUI's title)
    pub title: String,
    /// its objective's first line
    pub purpose: String,
    /// since when it has this status
    pub since_ms: u64,
    /// his open cards from it
    pub waits: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// its worktree's branch (none: it works in the shared checkout), as
    /// released; the shared checkout's branch is `checkout_branch`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// the branch checked out in the shared checkout it works in (none in
    /// a worktree: that is `branch`), P4b-fix
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// how long its turn has run (working only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<Report>,
    /// draft (not frozen until the hub sends it, amb-hub's queued inputs):
    /// his messages waiting for its turn to end, oldest first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<crate::draft::Queued>,
    /// its folder in the hub's `agents/` (its name but after a rename):
    /// where `sb history --project` reads its thread on disk (S2 C), and
    /// the agent's identity across a rename (a client keys agents by it:
    /// same dir, new name = renamed). Always set by a live hub; '' only
    /// in a view written before this field (read as the name)
    #[serde(default)]
    pub dir: String,
    /// its former names (a rename), which `sb history --agent` takes too
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// draft (bise desktop K4): the model it runs with, the full id as
    /// `sb list` shows it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// draft (K4): whether that model reads images, from bise's model
    /// catalog (`bise_catalog::Catalog::vision`): false only for a listed
    /// model without vision, none when the catalog doesn't know it (the
    /// window never refuses on a guess)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
    /// draft (bar A.5): its reasoning effort (`low`, `high`...), none when
    /// its model has no reasoning setting or the hub didn't say
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// bar S13/S40: its context after its last model call, from the
    /// hub's live usage lines (none after a hub start until its next
    /// call, and after a compaction)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AgentUsage>,
    /// R9/S3: whom it waits on, the hub's own fact (the TUI's panel reads
    /// the same field): you (its card or question), or another agent's
    /// reply; none when it waits on nobody
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<WaitingOn>,
    /// client-protocol step 4 (P4b, architect m_13999): the facts the
    /// terminal reads, each from the hub's one source. Where it works:
    /// the shared checkout (`checkout_branch`) or its own worktree
    /// (`branch`, `worktree` its path)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<AgentMode>,
    /// its folder (the shared checkout's or its worktree's)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// its whole objective (`purpose` is its first line)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub objective: String,
    /// what it says it does now (`sb status --note`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// what it is doing now, one line (BISE-126; none for main)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role: String,
    /// the agents' messages waiting for its turn to end (`queued` is his)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub msgs_queued: u32,
    /// main's inbox: the agents' questions waiting for main (BISE-299)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub inbox: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_ms: Option<u64>,
    /// its private worktree (BISE-136), none in the shared checkout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// the id of the place it is in (dev-flow §3.1, the `places` rows)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place_id: String,
    /// the reasoning efforts its model takes (the catalog's words)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<String>,
    /// its changes against its base (the `± 9 files so far` door), none
    /// when nothing changed or not measured yet
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<Changes>,
    /// the pos of its thread's newest entry (`bise_proto::thread::fold`,
    /// the hub's one fold): a client lights an agent out of view when it
    /// moves, without subscribing its thread
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pos: Option<crate::Pos>,
    /// how many of its turns ended since the hub started (its
    /// `turn_done` lines, counted live), so a client never misses a turn
    /// that started and ended between two rows (the queue's next message
    /// goes at a turn's end): it compares it with what it saw
    /// (proto-lead m_14731)
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub turns: u64,
}

/// Where an agent works (named apart from `hub::Mode`, a send's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// the workspace's checkout, shared with main
    Shared,
    /// its own git worktree
    Worktree,
    /// a mode this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// An agent's changes against its base: files, lines added, removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Changes {
    pub files: u64,
    pub add: u64,
    pub del: u64,
}

/// How a repo's agents land their work (`config.toml`'s `[flow] mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FlowMode {
    /// every change through a branch and a pull request
    Pr,
    /// tested commits straight on the default branch
    Trunk,
    /// a flow this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Whom an agent waits on (R9/S3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "who", rename_all = "snake_case")]
pub enum WaitingOn {
    /// the user: its card or question
    You,
    /// another agent's reply
    Agent { name: String },
    /// a kind this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

impl WaitingOn {
    /// The hub model's word (`"you"` or an agent's name), typed.
    pub fn of_word(w: &str) -> Option<WaitingOn> {
        match w {
            "" => None,
            "you" => Some(WaitingOn::You),
            name => Some(WaitingOn::Agent { name: name.to_string() }),
        }
    }
}

/// An agent's context after its last model call (bar S13, the divider's
/// gauge): the tokens the model saw plus its reply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AgentUsage {
    /// the call's full `provider/model` id
    pub model: String,
    /// tokens in its context
    pub context: u64,
    /// its model's context window, when the catalog knows it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    /// the gauge's words, as the TUI's divider says them: "42k · 21%"
    /// (bise-proto's `words::context_words`; the window never rebuilds
    /// them)
    pub words: String,
    /// the compact form the TUI's task list shows: "21%", or "42k"
    /// without a known window (`words::short_words`, amb-win m_10985)
    #[serde(default)]
    pub short: String,
}

/// How tool calls are approved on this machine (bar V8/W21,
/// approvals-design.md §8): `yolo` runs everything, `auto` asks the
/// checker first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Yolo,
    Auto,
    /// a mode this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// Who checks a gated call in `auto` (approvals-design.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CheckerKind {
    /// Jev (TypeSafe's checker, or its open route)
    Jev,
    /// a chat model in the checker role
    Model,
    /// none: every gated call asks him
    Off,
    /// a checker this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// One saved rule of `~/.bise/approvals.toml` (bar V8/W21): its fields as
/// the file has them, which are also its identity (`remove_rule` sends
/// them back, words ignored), and the words the TUI's `/approvals` list
/// shows (`approvals::what`/`note`). Its age is not words here: the
/// window shows `added_ms` with its own date helper (architect m_10951).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ApprovalRule {
    /// `bash`, an edit tool, or a connector tool (`gmail.send_email`)
    pub tool: String,
    /// bash: an arity pattern (`cargo test *`) or a command's text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// edit tools: a folder or file the edits may touch
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// the repo it applies to; none: every project
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// as the file says it (sent back as is to remove it)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<String>,
    /// `card #12, api-v2`, or the file's own words
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// `false`: its commands run outside the sandbox
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// when it was saved (ms), when `added` says it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_ms: Option<u64>,
    /// what it allows: `cargo test *`, `edits to ~/notes` (none in a
    /// `remove_rule`: the hub reads the fields only)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub what: String,
    /// its source and where it applies: `from api · every project`
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A numbered option of a card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Opt {
    pub n: u32,
    pub label: String,
}

/// Where a card points on a page (a question block, a step of a plan).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardPage {
    pub id: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}

/// One open card: what waits on him.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Card {
    pub id: u64,
    pub project: Project,
    /// the hub's kind (`question`, `confirm`, `merge`, …): an open set
    pub kind: String,
    pub agent: String,
    /// the question without its options
    pub question: String,
    pub options: Vec<Opt>,
    /// it holds running work now (a tool call waiting for a yes)
    pub urgent: bool,
    pub since_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<CardPage>,
    /// it acts or leaves (a tool call's yes, a merge, a release...): only
    /// one of its options answers it, never typed words (the composer
    /// rule, lead m_8777); false: a question, words answer it
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub approval: bool,
    /// its place in reading order, [`card_rank`] of its kind (0 = the most
    /// blocking): the inbox sorts on (rank, id) inside one hub, as the
    /// TUI's, and has no table of its own (architect m_9549). Absent from
    /// an older hub: a reader ranks it as `card_rank("")`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u8>,
    /// the `signin` card only (bar V14): the agents the expired ChatGPT
    /// sign-in stopped, which go on once he signs in
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Vec<String>>,
    /// client-protocol step 4 (P4c-4a): the card's words as the hub wrote
    /// them, options included (`question` and `options` are parsed from
    /// them); "" from an older hub
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// the hub's second line under it (a merge's `approved · checks
    /// pass`, a question answered another way...)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// the place it is about (a `places` row's id: a merge, a feature)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// the number of the PR it is about
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// the page its link opens (the update card's release page)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// the message it answers (`sb card --for`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_msg: Option<u64>,
}

/// A card kind's rank in reading order, what blocks an agent first: an
/// approval, then what only he decides (a question, a merge, a sign-in),
/// blocked, failed, drop, overlap, done (and any kind nobody taught us),
/// the setup offers last. The one table: the TUI's inbox (`kind_look`)
/// and the hub's `Card.rank` read it.
pub fn card_rank(kind: &str) -> u8 {
    match kind {
        "approval" | "confirm" => 0,
        "question" | "merge" | "feature_try" | "feature_merge" | "signin" => 1,
        "blocked" => 2,
        "failed" | "restart" => 3,
        "drop" => 4,
        "overlap" => 5,
        // the setup card and its offers (BISE-245): they block nothing
        "setup" => 7,
        _ => 6,
    }
}

impl Card {
    /// A kind answered only by one of its options: every kind but
    /// `question` and `drop` (sb-core keeps yes/no words there, and a drop
    /// never reaches a window: bise's bookkeeping): the hub's `confirm`,
    /// `merge`, `feature_try`, `feature_merge`, `update`, `signin`, and any
    /// newer one (a kind nobody taught the clients is safer picked than
    /// typed). sb-core's refusal of words says the same (amb-hub's
    /// agreement test, architect m_8848).
    pub fn approval_kind(kind: &str) -> bool {
        !matches!(kind, "question" | "drop")
    }
}

/// A commit that reached the project's trunk today (the changes tab's
/// "merged today", amb-web m_8974).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Merged {
    /// the full sha
    pub sha: String,
    /// its subject line
    pub title: String,
    /// the agent whose land brought it (sb land, the merge path); absent:
    /// not landed through bise (his own commit), or not known
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// its commit time
    pub at_ms: u64,
}

/// A server an agent runs in the background (`pnpm dev`, a local
/// grafana): its job listening on a port, or a known server command not
/// listening yet (no port, no url).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DevServer {
    pub agent: String,
    /// what it is ("dev server", "http server")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `http://localhost:<port>` once it listens
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// the job's command line
    pub cmd: String,
    /// its job runs
    pub up: bool,
}

/// A worktree of the project's repo: an agent's, or one of his own (the
/// main checkout included).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Worktree {
    pub path: String,
    /// its branch ("" when detached)
    pub branch: String,
    /// the full sha it sits at ("detached at e0f3df5", amb-web m_8951)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// the agent working there (none: his own)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// changes not committed, untracked files included
    pub dirty: bool,
    /// commits ahead of and behind the trunk
    pub ahead: u32,
    pub behind: u32,
}

/// A feature's last try build (`sb feature` / the try card's "try it").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeatureTry {
    /// the tip built (short)
    pub sha: String,
    /// what to run, in his words (`~/.bise/dev/versions/fd25c45/bise`,
    /// or `git checkout <name>` when the repo has no try command)
    pub run: String,
    pub at_ms: u64,
}

/// A feature branch (dev-flow §5.1): several agents land on it, he tries
/// it, it merges into the trunk only on his answer. Its actions are the
/// answers of its open card (`card`, kind `feature_try` or
/// `feature_merge`) through `answer`, as in the TUI: no command of its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Feature {
    pub name: String,
    /// its local branch (never pushed)
    pub branch: String,
    /// the trunk it leaves and merges into (`main`)
    pub base: String,
    /// its live agents, in the hub's order
    pub agents: Vec<String>,
    /// commits it has that the trunk lacks, and the other way
    pub ahead: u32,
    pub behind: u32,
    /// lines added and removed against the trunk
    pub adds: u32,
    pub dels: u32,
    /// its tip (full sha; "" before git was read)
    pub tip: String,
    /// the repo's check passed on this tip
    pub checked: bool,
    /// its last try build
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tried: Option<FeatureTry>,
    /// on trial: built, and he hasn't said merge, keep working or drop yet
    pub trial: bool,
    /// a try build runs now
    pub building: bool,
    /// an existing local branch `sb feature new` took as it was
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub adopted: bool,
    pub created_ms: u64,
    /// its open card (`feature_try`: try it / show the diff / not yet;
    /// `feature_merge`: merge / keep working / drop the branch, the drop
    /// asked once more as the TUI does), the id `answer` takes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<u64>,
}

/// An open pull request's state (merged and closed ones aren't listed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    Draft,
    /// P4c-4a: a place's PR only (`/prs` lists open and draft ones): a
    /// merged place goes a tick later, a closed one keeps its box
    Merged,
    Closed,
    /// a state this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// Its checks on the head commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrChecks {
    Pass,
    Fail,
    Running,
    /// none reported yet
    None,
    #[serde(other)]
    Unknown,
}

/// Its review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrReview {
    Approved,
    Changes,
    None,
    #[serde(other)]
    Unknown,
}

/// An open pull request of the project's places (the TUI's `/prs` row):
/// what it means, and the hub's own words for it (never rebuilt by a
/// client).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Pr {
    pub number: u64,
    pub url: String,
    pub branch: String,
    /// the agents of its place, in the hub's order
    pub agents: Vec<String>,
    pub state: PrState,
    pub checks: PrChecks,
    /// the failing checks' names, when `checks` is `fail`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing: Vec<String>,
    pub review: PrReview,
    /// its state in words: `changes asked · checks pass`, `draft · checks running`
    pub words: String,
    /// its `/prs` line after `#<number>`: `sb/x · x, y · <words>`
    pub text: String,
    /// P4c-4a: a review was asked and none came yet (`review` stays
    /// `none` then, as it always was: the wire rule)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub in_review: bool,
    /// a place's PR (`HubEv::Agents.places`): how old the forge's last
    /// answer is, when it is late (offline, rate limit); none when fresh,
    /// and always none in `/prs`'s rows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_ms: Option<u64>,
}

/// A worktree of the project as the terminal's panel draws it (dev-flow
/// §3.1, the `places` of the hub's snapshot, P4c-4a): a worktree its
/// agents share, a feature branch, a solo agent's own worktree. Never
/// the shared checkout. In its first agent's order; an agent's
/// `place_id` is its `id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Place {
    /// `wt:<branch>`, `pt:<path>` (a private worktree), `feature:<name>`
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// its agents, not archived, in the hub's order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// the PR of its branch (merged and closed ones too, until it goes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<Pr>,
    /// the held line the hub writes (`waits to land · 2nd`, `no PR yet ·
    /// 2 commits`): it wins over the PR's words
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,
    /// a feature branch's place (dev-flow §5.1): never a PR
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub feature: bool,
    /// its try build builds or is on trial
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub trying: bool,
}

/// A bise page of this hub (docs/ambient-pages.md §2.3): `hub/pages`'
/// rows (newest first) and `page/changed`'s page. `state`: `ready`,
/// `updating` (its agent answers his notes)...; the counts and marks are
/// left out of `page/changed` (its older `page` line has none).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Page {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub version: u32,
    pub url: String,
    pub at_ms: u64,
    pub state: String,
    /// his notes not answered yet
    #[serde(default, skip_serializing_if = "is_zero")]
    pub open_notes: u32,
    /// the version he last opened
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_version: Option<u32>,
    /// what on it waits for him (`sb page waiting`'s lines of this page)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub waiting: u32,
    /// the meta line of its first heading block
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kicker: Option<String>,
    /// it holds a question whose card is open (§4.2)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub asking: bool,
}

/// The role a model is config.toml's default for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Main,
    Agents,
    #[serde(other)]
    Unknown,
}

/// A model the TUI's `/model` list offers (bise_catalog::picks: a chat
/// model whose provider can run a turn on this hub now, or an alias):
/// the facts its row is made from, and the hub's words for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Model {
    /// what `model` takes: a full `provider/id`, or an alias
    pub id: String,
    /// its name for people (`opus 5.5`)
    pub label: String,
    /// its provider's name, the header it goes under ("" for an alias)
    pub provider: String,
    /// its row's words under that header: `1M`, `128k · config.toml`,
    /// `= anthropic/claude-opus-5-5`
    pub short: String,
    /// its context window in tokens (none for an alias)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    /// set in config.toml
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub config: bool,
    /// an alias: the model it names
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    /// whether it reads images (the catalog's K4 rule; none: not listed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
    /// the reasoning efforts it takes (`/reasoning`), none: no setting
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<String>,
    /// the effort it gets by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    /// the roles whose default model it is (config.toml's [roles])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_for: Vec<ModelRole>,
    /// draft (L5): its provider's catalog id (`anthropic`, `chatgpt`),
    /// the `provider` of the account that sets it up; an alias: its
    /// target's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_id: String,
    /// draft (L5): its provider has a key or a sign-in this hub finds
    /// (env, auth.json, the old .env files), or needs none; false: a pick
    /// needs setup first. Absent: true (an older hub listed only these)
    #[serde(default = "crate::yes", skip_serializing_if = "crate::is_true")]
    pub key_ready: bool,
}

/// A question and its options: the trailing numbered lines (`1. v1`,
/// `1) v1`, `1 - v1`, `1 v1`, two to nine, numbered from 1) or a list
/// inline at the end of its last line (`Que fais-tu ? 1. a 2. b`). None
/// found: the text as is and no options. A copy of the TUI's
/// `sb/cards.rs split_choices` (bend-tui tests that they agree); the
/// TUI takes this one at the ambient merge.
pub fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))
            .or_else(|| rest.strip_prefix(' ').filter(|r| !r.starts_with(['-', '–', ' '])))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return split_inline(text).unwrap_or_else(|| (text.to_string(), Vec::new()));
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}

fn split_inline(text: &str) -> Option<(String, Vec<String>)> {
    let t = text.trim_end();
    let line_at = t.rfind('\n').map_or(0, |i| i + 1);
    let line = &t[line_at..];
    for sep in [". ", ") "] {
        let mut at: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        for n in 1..=9 {
            let m = format!("{n}{sep}");
            let found = line[from..].match_indices(&m).map(|(i, _)| from + i).find(|&i| i == 0 || line[..i].ends_with(' '));
            match found {
                Some(i) => {
                    at.push((i, i + m.len()));
                    from = i + m.len();
                }
                None => break,
            }
        }
        if at.len() < 2 {
            continue;
        }
        let options: Vec<String> = at
            .iter()
            .enumerate()
            .map(|(k, &(_, start))| {
                let end = at.get(k + 1).map_or(line.len(), |&(i, _)| i);
                line[start..end].trim().trim_end_matches([',', ';']).trim().to_string()
            })
            .collect();
        if options.iter().any(|o| o.is_empty()) {
            continue;
        }
        let body = format!("{}{}", &t[..line_at], line[..at[0].0].trim_end()).trim_end().to_string();
        return Some((body, options));
    }
    None
}

/// A question's text as a card shows it: the body and its options.
pub fn question(text: &str) -> (String, Vec<Opt>) {
    let (body, options) = split_choices(text);
    let opts = options.into_iter().enumerate().map(|(i, label)| Opt { n: i as u32 + 1, label }).collect();
    (body.trim().to_string(), opts)
}

/// An artifact of a project (what an agent made for him), the
/// `artifacts` event's row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Artifact {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub agent: String,
    pub version: u32,
    pub at_ms: u64,
    pub url: String,
    /// made or changed since he last looked
    pub new: bool,
    /// the file on disk (absolute), only for a file target, never for a
    /// link or a page: the window shows it in the Finder or opens it.
    /// Sent on the client socket only, never to an agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// every version the store keeps, oldest first: the store's whole
    /// list, no cap (the versions popover); left out when empty (a row
    /// from before it, an older hub)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<ArtifactVersion>,
    // the rest of the art store's row, as the terminal's /artifacts reads
    // it (client-protocol step 4, P4c): left out when empty, so an older
    // hub's row or an older client still reads
    /// who added it: `you`, `page` (a bise page), else the agent
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
    /// its agent is archived
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub archived: bool,
    /// when it was first added (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_ms: Option<u64>,
    /// the current version's target as the store keeps it: a path or a
    /// link (a page's: its file)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// the store's copy of the current version (absolute), when it kept one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
    /// its target is a file that is no longer there
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub gone: bool,
    /// the row's dim words: a page's notes (`2 notes open`), a site's bare
    /// link
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// a pull request's link
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<ArtifactPr>,
    /// the words a search matches (its links, bare and full; its paths,
    /// absolute and from the workspace)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
}

/// An artifact that is a pull request: its repo (`owner/name`) and number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactPr {
    pub repo: String,
    pub number: u64,
}

/// One version of an artifact: its number, when it was made, and the
/// TUI's /artifacts words for it (a page's 'n notes open', 'no copy: …'),
/// printed as they are; its target and the store's copy of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactVersion {
    pub v: u32,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

/// A live scheduled task of a project (`sb every`), the `scheduled`
/// event's row (⌘K's source, the scheduled screen): data, the window
/// formats the times; `every` the task's how-often words (`every 2m`,
/// `every day 07:30`, `once`: bise_proto::thread::scheduled::every_words,
/// the TUI's), `done` its runs so far.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScheduledTask {
    pub id: u64,
    /// the agent it wakes
    pub agent: String,
    /// who set it
    pub by: String,
    /// its words, what the agent reads at each run
    pub words: String,
    /// its name, what the lists show (sched-names): the hub's model's or
    /// `--name`, else the plain fallback of its words; always filled by
    /// the hub (empty only from an older one)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub every: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<u64>,
    pub done: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<u64>,
    /// the page it keeps fresh (`--page`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// P4c-4a: the hub's how-often label (`every 2m`, `every day
    /// 07:30`), before `every`'s `once` and times words
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// when it last woke its agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ms: Option<u64>,
    /// its last runs, oldest first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<u64>,
    /// an ended one (`HubEv::Scheduled.ended` only): when, how, by whom
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<ScheduledEnd>,
    /// who stopped it (`end` is `stopped`): you, or an agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_by: Option<WaitingOn>,
}

/// How a scheduled task ended (the hub's `every::end_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ScheduledEnd {
    /// it ran its times
    Times,
    /// its end time passed
    Until,
    /// its agent is gone
    Gone,
    /// someone stopped it (`stopped_by`), or an older line said nothing
    Stopped,
    /// an end this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

impl ScheduledEnd {
    /// The hub's word (`times`, `until`, `gone`, `stopped`), typed; none
    /// for "" (a live one).
    pub fn of_word(w: &str) -> Option<ScheduledEnd> {
        match w {
            "" => None,
            "times" => Some(ScheduledEnd::Times),
            "until" => Some(ScheduledEnd::Until),
            "gone" => Some(ScheduledEnd::Gone),
            "stopped" => Some(ScheduledEnd::Stopped),
            _ => Some(ScheduledEnd::Unknown),
        }
    }

    /// Its word, the one `of_word` reads ("" for an unknown one).
    pub fn word(self) -> &'static str {
        match self {
            ScheduledEnd::Times => "times",
            ScheduledEnd::Until => "until",
            ScheduledEnd::Gone => "gone",
            ScheduledEnd::Stopped => "stopped",
            ScheduledEnd::Unknown => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_words_map_to_statuses() {
        // the released meanings (the wire rule); the exact word is the phase
        assert_eq!(Status::of_hub("starting"), (Status::Working, false));
        assert_eq!(Status::of_hub("stopped"), (Status::Done, false));
        assert_eq!((Phase::of_hub("starting"), Phase::of_hub("stopped"), Phase::of_hub("working")), (Some(Phase::Starting), Some(Phase::Stopped), None));
        assert_eq!(Status::of_hub("archived"), (Status::Done, true));
        assert_eq!(Status::of_hub("idle"), (Status::Idle, false));
    }

    #[test]
    fn a_question_loses_its_options() {
        let (q, o) = question("keep the banner?\n1. yes\n2. no");
        assert_eq!((q.as_str(), o.len(), o[1].label.as_str(), o[1].n), ("keep the banner?", 2, "no", 2));
        let (q, o) = question("Que fais-tu ? 1. regarde le diff 2. arrête-le 3. laisse-le finir");
        assert_eq!((q.as_str(), o[2].label.as_str()), ("Que fais-tu ?", "laisse-le finir"));
        let (q, o) = question("how should agents ship?\n1 a PR per task\n2 straight to main");
        assert_eq!((q.as_str(), o[0].label.as_str()), ("how should agents ship?", "a PR per task"));
        assert_eq!(question("plain"), ("plain".to_string(), vec![]));
    }

    /// R9/S3: the hub model's waiting_on word, typed: 'you', an agent's
    /// name, none; a newer kind reads as Unknown.
    #[test]
    fn whom_an_agent_waits_on_is_typed() {
        assert_eq!(WaitingOn::of_word("you"), Some(WaitingOn::You));
        assert_eq!(WaitingOn::of_word("docs"), Some(WaitingOn::Agent { name: "docs".into() }));
        assert_eq!(WaitingOn::of_word(""), None);
        assert_eq!(serde_json::to_value(WaitingOn::You).unwrap(), serde_json::json!({"who": "you"}));
        assert_eq!(serde_json::to_value(WaitingOn::Agent { name: "docs".into() }).unwrap(), serde_json::json!({"who": "agent", "name": "docs"}));
        assert_eq!(serde_json::from_value::<WaitingOn>(serde_json::json!({"who": "a_review"})).unwrap(), WaitingOn::Unknown);
    }

    #[test]
    fn what_blocks_an_agent_ranks_first() {
        let order = ["confirm", "question", "merge", "signin", "blocked", "failed", "drop", "overlap", "done", "newer_kind", "setup"];
        let ranks: Vec<u8> = order.iter().map(|k| card_rank(k)).collect();
        assert_eq!(ranks, [0, 1, 1, 1, 2, 3, 4, 5, 6, 6, 7]);
        assert_eq!(card_rank("approval"), card_rank("confirm"));
        assert_eq!(card_rank("restart"), card_rank("failed"));
    }
}
