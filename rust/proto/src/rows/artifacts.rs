//! A project's artifacts and their versions.

use serde::{Deserialize, Serialize};

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

