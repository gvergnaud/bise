//! A bise page of this hub.

use super::is_zero;
use serde::{Deserialize, Serialize};

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

