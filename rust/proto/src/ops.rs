//! The rows of the hub's answers that were untyped events of the
//! terminal's ops (client-protocol step 3, architect m_13313): `/diff`'s
//! branch picker (`branches`), `/version`'s picker (`versions`),
//! `/release-bise`'s plan and steps (`release`). Their
//! shapes are the older events' as they are, typed; the variants that
//! carry them are [`crate::hub::HubEv`]'s, each a method's result.

use crate::Project;
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

/// `/release-bise` (dev build only), [`crate::hub::HubEv::Release`]:
/// `state` "plan" (the answer to `release_plan`: `tag`, `commit`, `short`,
/// `subject`, `since` the last tag, `count` and the first `commits` as
/// [sha, subject]), "error" (no plan: `text`), then a run's "running",
/// "step", "done" or "failed" (`text`, `elapsed` s; a failure's `tail`
/// and `log`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReleaseEv {
    pub project: Project,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub dry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tail: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<String>,
}

/// `/update` in bise's source tree (dev-update), [`crate::hub::HubEv::Update`]:
/// `state` "building" (`rev` the commit, `elapsed` s since it started),
/// "built" (the switch says the rest in main's thread) or "failed"
/// (`text`, the build's last lines in `tail`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateEv {
    pub project: Project,
    pub state: String,
    pub rev: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tail: Vec<String>,
}
