//! The rows of the hub's answers that were untyped events of the
//! terminal's ops (client-protocol step 3, architect m_13313): `/diff`'s
//! branch picker (`branches`), `/version`'s picker (`versions`). Their
//! shapes are the older events' as they are, typed; the variants that
//! carry them are [`crate::hub::HubEv`]'s, each a method's result.

use serde::{Deserialize, Serialize};

/// One row of `/diff`'s branch picker (the `branches` answer): a local
/// branch ahead of the trunk, who works on it, its size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BranchRow {
    pub branch: String,
    /// the agents (not archived) whose branch it is
    #[serde(default)]
    pub agents: Vec<String>,
    /// its commits ahead of the trunk
    pub commits: u64,
    #[serde(default)]
    pub uncommitted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed_ms: Option<u64>,
    pub add: u64,
    pub del: u64,
}

/// One row of `/version`'s picker: `back` (roll back), `tree` (the
/// working tree, in bise's source tree), an installed release or a
/// recent commit; `marks` its state words (current, good, latest, built,
/// building, failed, trial).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionItem {
    pub rev: String,
    pub subject: String,
    #[serde(default)]
    pub marks: Vec<String>,
}
