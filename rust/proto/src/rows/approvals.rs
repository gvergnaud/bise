//! The approval mode and rules a hub reports.

use serde::{Deserialize, Serialize};

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

