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
    /// where a window frames it: the hub's page server serving the
    /// current version's copy (`http://127.0.0.1:<port>/a/<id>/<v>/<file>`,
    /// the pages' own artifact links), only for a file the store copied;
    /// none for a link or a page (`url` is the page), or with no page
    /// server. `url` keeps its meaning: what the terminal opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_url: Option<String>,
    /// every version the store keeps, oldest first: the store's whole
    /// list, no cap (the versions popover); left out when empty (a row
    /// from before it, an older hub)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<ArtifactVersion>,
    /// the app that opens it on his click (docs/plugin-apps.md A2, A3):
    /// `bend_plugins::opens::pick`'s, set by the hub, never matched by a
    /// client; none: no app, the window says his Mac's
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opener: Option<crate::rows::Opener>,
    /// the other apps that can open it, in pick's order (Open with)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openers: Vec<crate::rows::Opener>,
    // the rest of the art store's row, as the terminal's /artifacts reads
    // it (client-protocol step 4, P4c): left out when empty, so an older
    // hub's row or an older client still reads
    /// who added it: `you`, `page` (a bise page), else the agent
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
    /// its agent is archived
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub archived: bool,
    /// the artifact itself was archived at (ms; `sb artifact archive`):
    /// out of the default list, behind its `N archived` row
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<u64>,
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
    /// why the store kept no copy of the current version (`over 50 MB`,
    /// `copy failed: …`): the window says it where it would frame the file
    /// (a video too big to copy still gets its `frame_url`, from its own file)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_copy: Option<String>,
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
    /// the side panel's live words while its maker saves it
    /// (`bise_proto::office::live_words`, the same the viewer's
    /// `/a/<id>/events` sends); none for a page, a link, or v1 at rest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<ArtifactLive>,
    /// its folder in the library (a [`Folder`]'s id), resolved by the hub
    /// (a merged folder's artifacts read as the one it went into, a
    /// deleted one's as none): the one field a reader reads. None: unfiled
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub folder: Option<String>,
    /// when the turn that made its current version started (ms), one turn
    /// of its agent: the grid stacks one agent's turn; none for his adds,
    /// a page, a row from before it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub turn: Option<u64>,
}

/// A folder of the artifacts library (`bise_home::library`, one list for
/// every project): a theme (no parent) or a task folder in one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// `you`, or the agent that made it
    pub by: String,
    /// he made or renamed it: an agent never renames it
    #[serde(default)]
    pub named_by_you: bool,
}

/// An artifact his move moved, and its folder before (`artifacts/move`'s
/// answer, for his undo).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactMoved {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<String>,
}

/// A change of the library's folders (`artifacts/folders`, the desktop
/// core's `folders`): his, so it takes the undo slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FolderOp {
    Make {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        parent: Option<String>,
    },
    Rename { id: String, name: String },
    /// a folder into a theme (none: to the top)
    Nest {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        parent: Option<String>,
    },
    Merge { from: String, into: String },
    /// its artifacts read as unfiled; none is deleted
    Delete { id: String },
    /// his last change, back
    Undo,
}

/// Another project's artifacts (`artifacts/library`'s answer).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LibraryProject {
    /// its hub id
    pub project: String,
    /// its name in the projects registry
    pub name: String,
    pub items: Vec<Artifact>,
}

/// The words a window draws over an artifact its maker saves while he
/// looks (office-artifacts plan §6, designer m_23868): one of the two at
/// a time. The window draws them as they are and decides nothing but
/// when he switched away or closed it (then `line` goes).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactLive {
    /// while the maker's turn that made the newest version runs: the
    /// 28 px strip, `∿ deck is updating · slide 2`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub strip: Option<String>,
    /// once that turn ended: `v3 · 2 slides changed`, drawn with
    /// [`ArtifactLive::back`] 16 px after it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub line: Option<String>,
    /// with `line`: the link to the version before (`← v2`, text-2,
    /// underline on hover, opens that version; designer m_24118)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub back: Option<ArtifactBack>,
}

/// The `← v2` link after an artifact's live line: its words and the
/// version it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactBack {
    pub text: String,
    pub v: u32,
}

/// An artifact that is a pull request: its repo (`owner/name`) and number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtifactPr {
    pub repo: String,
    pub number: u64,
    /// its sigil and number, `#412` (GitLab `!412`), from the hub's one
    /// read of its link (`forge::PrLink::tag`), there before the forge
    /// says anything: clients never parse a PR link (architect m_54703)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,
    /// pr-integration rules 1 and 3: the PR as the hub follows it, the
    /// one row the chip, the place and `/prs` read too
    /// (`forge::news::pr_row`), joined when the event is sent; none
    /// while the forge has not said (not polled yet, no gh): the row
    /// shows the PR as it is, never a made-up state
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<super::Pr>,
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
    /// where a window frames this version (the row's rule,
    /// pages::site::frame_href: `/a/<id>/view` for the newest office file,
    /// `/a/<id>/<v>/view` for an older one, else its copy's link); none
    /// with no copy served or no page server (architect m_23970)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_url: Option<String>,
    /// who saved it (docs/plugin-apps.md A6b's save line, `perf saved a
    /// newer version`); none for a version from before (architect m_60168)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<SavedBy>,
}

/// Who saved a version: him (his sitting in a viewer) or an agent (its
/// turn). The window words `You` from its catalog, never from this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedBy {
    You,
    Agent { name: String },
    /// a kind this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

