//! Plugin apps (docs/plugin-apps.md A2, A3): the app that opens a file,
//! as `bend_plugins::opens::pick` answers it and the hub puts it on an
//! artifact row, a folder row and Settings › Plugins › File types; what a
//! plugin opens and makes, on its plugins-page row and its add sheet.

use serde::{Deserialize, Serialize};

/// An app that opens a file: one opener entry that matched (A3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Opener {
    /// the plugin's name (`sheets`)
    pub plugin: String,
    /// its opener tool (`open_sheet`; a static view's tool)
    pub tool: String,
    /// the app's title, what he reads (`Sheets`)
    pub app: String,
    /// the app's own id, the `app:` filter's key (`sheets`): declared
    /// beside its title (A1 `dev.bise/ui.app_id`), else `<plugin>-<tool>`;
    /// never a word (architect m_63530)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    /// the canonical type key of the entry that matched (`*.xlsx`):
    /// his pick's key in `opener_set`
    #[serde(rename = "type")]
    pub type_key: String,
    /// the type's name (`Excel workbook`)
    pub type_title: String,
    /// its icon's name or address (none: the window's own)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// one of bise's own types not migrated yet (`opens::BUILTIN`): for
    /// the icon only, the window never branches on it (A3)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub builtin: bool,
}

/// How strongly an entry claims its type (A1): `option` is never picked
/// by itself, only listed in Open with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OpenRank {
    Owner,
    #[default]
    Default,
    Option,
}

/// One type a plugin opens, on its plugins-page row (A3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginOpens {
    /// the canonical type key: an extension's glob (`*.xlsx`), a folder
    /// rule (a folder holding an index.html), a path under the plugin's
    /// documents (`@documents/boards/…`)
    #[serde(rename = "type")]
    pub type_key: String,
    /// the type's name (`Excel workbook`)
    pub title: String,
    pub rank: OpenRank,
    /// the app that opens it (`Sheets`)
    pub app: String,
}

/// One kind of file a plugin makes (A1's `makes`): a row of the
/// launcher's New ›.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginMake {
    /// `.xlsx`, lowercase, with its dot
    pub ending: String,
    /// the New › row's word (`Sheet`)
    pub title: String,
    /// the app that makes it (`Sheets`)
    pub app: String,
}

/// Settings › Plugins › File types: one type some app opens (A3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileType {
    /// the canonical type key (`*.xlsx`)
    #[serde(rename = "type")]
    pub type_key: String,
    /// the type's name (`Excel workbook`)
    pub title: String,
    /// the app that opens it now (none: only `option` apps, he picks)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<Opener>,
    /// the other apps that can, in A2's order (Open with)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub others: Vec<Opener>,
    /// `app` is his pick (`opener_set`), not A2's order: the row resets
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub his: bool,
}

/// The add sheet's line for one type the new plugin opens (A3), one per
/// type it opens (its `opens` row: `.xlsx and .csv files`): `now` when
/// another app opens it now (`.xlsx opens in Sheets now · Keep Sheets /
/// Use Tables`, keep by default), `wide` for a wide claim (`*.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OpenClaim {
    /// the canonical type key it claims
    #[serde(rename = "type")]
    pub type_key: String,
    /// the type's name, as the new plugin says it
    pub title: String,
    /// the new plugin's app (`Tables`)
    pub app: String,
    /// its opener tool: `Use Tables` sends `opener_set {type, plugin,
    /// tool}`
    pub tool: String,
    /// the app that opens it now, when another does (`Sheets`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
    /// a wide claim: a bare extension many apps' files share (`*.json`,
    /// `*.txt`, `*.html`), said on the sheet
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub wide: bool,
}
