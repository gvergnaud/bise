//! The agents row of the hub: its status, phase, reports, mode, changes,
//! usage and whom it waits on.

use super::{is_zero, is_zero_u64};
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
    /// how many of its turns ended since sb-core started (sb-core's own
    /// count: +1 each time its run leaves busy, in the same view as its
    /// status, issue 22), so a client never misses a turn that started
    /// and ended between two rows (the queue's next message goes at a
    /// turn's end): it compares it with what it saw (proto-lead m_14731).
    /// Back to 0 when sb-core restarts: a lower count is a new baseline
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub turns: u64,
    /// event-wake (designer's waiting state): the events it waits for
    /// now (`sb wake`'s live watches, a backgrounded bash command's
    /// included), oldest first. A list of the snapshot: always written,
    /// empty included, so a client tells "no watch now" from an older
    /// hub (none)
    #[serde(default)]
    pub watching: Vec<AgentWatch>,
}

/// What a watch looks at (event-wake).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    /// a backgrounded bash command
    Bg,
    /// a process (`sb wake --on-exit`)
    Pid,
    /// a file that appears (`--on-file`)
    File,
    /// a launchd job (`--on-job`)
    Job,
    /// a kind this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// One event an agent waits for (event-wake): the panel's `…`, the live
/// line `… waiting for cargo test · 2m`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AgentWatch {
    /// its id (`sb wake --stop <id>`)
    pub id: u64,
    pub kind: WatchKind,
    /// its name in designer's words (`cargo test`, `the build`, `pid
    /// 4242`, `build.rc`, `launchd job dev.x`): the hub's
    /// `wake::Spec::name`; [`crate::thread::words::waiting_for`] says it
    pub what: String,
    /// when it was set (ms)
    pub since_ms: u64,
}

impl Agent {
    /// Its turn runs: sb-core's run is busy (status working past its
    /// start, or waiting on a reply inside the turn). The half of the row
    /// `turns` agrees with (issue 22): false iff `turns` counts the turn
    /// that just ended.
    pub fn turn_running(&self) -> bool {
        match self.status {
            Status::Working => self.phase != Some(Phase::Starting),
            Status::Waiting => true,
            _ => false,
        }
    }
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

impl AgentUsage {
    /// The row of a call's usage (`input`/`output` its tokens), `window`
    /// from the reader's catalog: the hub's one way to fill it.
    pub fn of(model: &str, input: u64, output: u64, window: Option<u64>) -> AgentUsage {
        let context = input.saturating_add(output);
        AgentUsage {
            model: model.to_string(),
            context,
            window,
            words: crate::thread::words::context_words(context, window),
            short: crate::thread::words::short_words(context, window),
        }
    }

    /// The gauge in full, "42k / 200k tokens · 21%" (the TUI's divider
    /// while a turn holds; `words::context_label`).
    pub fn label(&self) -> String {
        crate::thread::words::context_label(self.context, self.window)
    }
}

