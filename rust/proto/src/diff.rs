//! An agent's change as the review shows it: its files, their hunks, and
//! what it measured (the `diff` event, desktop S7).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Ctx,
    Add,
    Del,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiffLine {
    pub kind: LineKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
    /// git's function context after the `@@` (the terminal's `@@ head @@`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiffFile {
    pub path: String,
    /// added, modified, deleted, renamed
    pub status: String,
    pub add: u32,
    pub del: u32,
    pub hunks: Vec<Hunk>,
    /// its hunks were cut (past the size cap, or binary): the review says
    /// so, never shows it as complete
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub truncated: bool,
    /// a renamed file's old path ("renamed from decode.rs")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// a binary file: no lines (`truncated` too), its words are its own
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub binary: bool,
    /// a generated file (a lock file, a build output): a display flag,
    /// the client folds it (its lines are sent, under the same size cap)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub generated: bool,
    /// an image (by its extension): the terminal says so and opens it
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub image: bool,
    /// its size in bytes after the change, for a binary file still there
    /// ("a binary file, 2.1 MB")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// the agent's word on this file ("new: 4 workers"), from its summary
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// its absolute path in the checkout the diff was read in (the
    /// terminal's /diff opens it), none when that folder is gone
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abs: Option<String>,
}

/// What a change measured, before and after (the review's result line,
/// amb-web m_8304): "worst frame, PS5 replay · forest level 40 → 15 ms".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiffResult {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    pub before: f64,
    pub after: f64,
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Series>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Series {
    pub before: Vec<f64>,
    pub after: Vec<f64>,
}

/// The terminal's `/diff` fields of a `diff` (client-protocol step 3),
/// next to the review's on the wire: its title ("perf vs main", "range
/// a..b"), the `req` it answers, the commits ahead of the base, the agent
/// still working, edits not committed yet, when it landed, its folder
/// gone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiffView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits: Option<u64>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub working: bool,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub uncommitted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub gone: bool,
}
