//! Shapes with no producer yet (architect's S3a review, change A): their
//! fixtures feed the window's fake core now, but their names may still
//! move until their stream emits them, outside v1's "never rename" rule.
//! When a stream ships one, it moves to [`crate::hub`] (or stays here for
//! the core's own messages) and its fixture joins the frozen ones.
//!
//! - [`DraftHubCmd`]: a send with quoted lines (S7's artifacts, diffs,
//!   worktrees and dev servers, S2's routing "→ project" and S10's
//!   followed jobs shipped: hub.rs);
//! - [`ProjectView`]: S1's `hubs/<id>/view.json` (amb-hub writes it).
//!
//! The desktop core's own lines are in [`crate::core_ev`],
//! [`crate::app_cmd`] and their rows in [`crate::app_rows`].

use crate::app_rows::is_zero;
use crate::rows::{Agent, Artifact, Card, ScheduledTask};
use crate::{decode, parse, Pos, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Selected lines of a thread quoted in a message (❝).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Quote {
    pub agent: String,
    pub pos: Pos,
    pub text: String,
}

/// One of his messages queued for an agent until its turn ends (sb-core
/// holds it, amb-hub m_8281): an [`Agent`] row's `queued`, draft until the
/// hub's mirror sends them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Queued {
    pub id: u64,
    pub text: String,
    pub created_ms: u64,
    /// his message as typed parts (msg-parts), when he sent some: a
    /// client edits it back from them (chips), never by parsing `text`;
    /// empty for an older message (then `text` is all there is)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<crate::parts::Part>,
}

/// Hub commands whose stream hasn't shipped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum DraftHubCmd {
    /// a send with quoted lines (the window's ❝)
    SendQuote { project: Project, agent: String, text: String, quote: Quote },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl DraftHubCmd {
    pub const TAGS: &'static [&'static str] = &["send_quote"];

    pub fn decode(line: &str) -> Result<DraftHubCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| DraftHubCmd::Unknown { tag, raw })
    }
}

/// `hubs/<id>/view.json` (S1, written only by that hub): what the sidebar
/// and bise read of a project without its hub.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectView {
    pub v: u32,
    pub project: Project,
    /// `written_ms` and `last_activity_ms` move only when something a
    /// reader sees changed (the hub skips a view equal but for them)
    pub written_ms: u64,
    /// set at a clean stop, absent while it runs (readers tell a running
    /// hub by its pid or a ping, never by a flag here)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_ms: Option<u64>,
    pub last_activity_ms: u64,
    pub agents: Vec<Agent>,
    pub cards: Vec<Card>,
    /// ⌘K's index (architect m_11910): its artifacts as the typed
    /// `artifacts` event gives them (the same builder), the newest
    /// [`VIEW_ARTIFACTS`] first; `artifacts_total`: how many it has
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub artifacts_total: u32,
    /// ⌘K's index: its live scheduled tasks, as the typed `scheduled`
    /// event gives them (the same builder, proto_view::scheduled)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scheduled: Vec<ScheduledTask>,
    /// les bims: how much it has done, the typed `growth` event's value
    /// (the same cached computation); none: a view written before it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth: Option<crate::rows::Growth>,
    /// the H1 sidebar: its pulse, the typed `pulse` event's value (one
    /// computation for both); after a stop its buckets stop moving.
    /// None: a view written before it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<crate::rows::Pulse>,
}

/// How many artifacts a `view.json` keeps (it is rewritten on every
/// change, so it stays small); the newest first.
pub const VIEW_ARTIFACTS: usize = 50;

