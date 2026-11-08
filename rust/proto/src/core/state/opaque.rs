//! The records sb-core keeps as opaque JSON (`J.Json` fields of its
//! Agent: `ws`, `brief`, `report`): it stores and writes them back as
//! they came, only Rust reads them. Rust-only, declared here once
//! (switchboard's model re-exports them); pinned on
//! fixtures/core_view.jsonl's agent rows (core::tests).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The id of the shared folder's place (switchboard's place.rs).
pub const SHARED: &str = "shared";

/// The id of the worktree first made for the agent of dir `dir`.
pub fn worktree_id(dir: &str) -> String {
    format!("wt:{}", dir)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Shared,
    Worktree,
}

/// Where a task works (RFC 0002). `path` is the workspace for `shared`.
/// Its default: the shared folder at "" (a view row whose ws doesn't
/// read).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub mode: Mode,
    pub path: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub base_commit: Option<String>,
    /// The worktree was removed by a drop (restore can bring it back).
    #[serde(default)]
    pub dropped: bool,
    /// The place this workspace is (dev-flow §3.1, switchboard's
    /// `place`): a worktree's id, `wt:<its first agent's dir>`, the same
    /// in every agent that shares it. None: the shared folder, or an
    /// older journal's worktree (its agent's own place,
    /// [`Workspace::place_id`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// dev-flow §5.1: the feature this worktree lands on (`sb spawn
    /// --feature`): its branch is made from the feature's tip and `sb
    /// land` moves the feature, never main. None: not a feature's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// hub-loop: `placing` while the shell makes its worktree (sb-core's
    /// hub/place.bend): no path, no branch, no REPL yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl Workspace {
    /// The shared folder at `path`.
    pub fn shared(path: &str) -> Workspace {
        Workspace {
            mode: Mode::Shared,
            path: path.to_string(),
            branch: None,
            base_commit: None,
            dropped: false,
            place: None,
            feature: None,
            state: None,
        }
    }

    /// Its worktree is still being made (no REPL yet).
    pub fn placing(&self) -> bool {
        self.state.as_deref() == Some("placing")
    }

    /// The feature an agent with this workspace lands on (a worktree's).
    pub fn feature(&self) -> Option<&str> {
        match self.mode {
            Mode::Worktree if !self.dropped => self.feature.as_deref(),
            _ => None,
        }
    }

    /// The id of the place an agent with this workspace (and this dir)
    /// is in: `shared`, or its worktree's id.
    pub fn place_id(&self, dir: &str) -> String {
        match self.mode {
            Mode::Shared => SHARED.to_string(),
            Mode::Worktree => self.place.clone().unwrap_or_else(|| worktree_id(dir)),
        }
    }
}

/// The task brief (RFC 0001 §7.1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    pub objective: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub done_when: Option<String>,
    #[serde(default)]
    pub report_format: Option<String>,
    /// `sb spawn --folder "<theme>/<task>"` (artifact-folders, architect
    /// m_46279): the library folder its artifacts go to, by name. No
    /// sb-core decision reads it: the hub resolves it at the task's first
    /// `sb artifact add` (switchboard's artifacts_filing.rs). None: no key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// An agent's last report: explicit (`sb report`) or the end of a turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub at_ms: u64,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub decisions: Vec<String>,
    /// Built by the hub at the end of a turn, not sent by the agent.
    #[serde(default)]
    pub auto: bool,
    /// S10: `sb report progress --step n/m`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u32>,
    /// emitter 5: `--result`, proto's DiffResult as JSON
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

