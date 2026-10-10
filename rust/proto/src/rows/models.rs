//! The models the TUI's `/model` list offers.

use serde::{Deserialize, Serialize};

/// The role a model is config.toml's default for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Main,
    Agents,
    #[serde(other)]
    Unknown,
}

/// A model the TUI's `/model` list offers (bise_catalog::picks: a chat
/// model whose provider can run a turn on this hub now, or an alias):
/// the facts its row is made from, and the hub's words for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Model {
    /// what `model` takes: a full `provider/id`, or an alias
    pub id: String,
    /// its name for people (`opus 5.5`)
    pub label: String,
    /// its provider's name, the header it goes under ("" for an alias)
    pub provider: String,
    /// its row's words under that header: `1M`, `128k · config.toml`,
    /// `= anthropic/claude-opus-5-5`
    pub short: String,
    /// its context window in tokens (none for an alias)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    /// set in config.toml
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub config: bool,
    /// an alias: the model it names
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    /// whether it reads images (the catalog's K4 rule; none: not listed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
    /// the reasoning efforts it takes (`/reasoning`), none: no setting
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<String>,
    /// the effort it gets by default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    /// the roles whose default model it is (config.toml's [roles])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_for: Vec<ModelRole>,
    /// draft (L5): its provider's catalog id (`anthropic`, `chatgpt`),
    /// the `provider` of the account that sets it up; an alias: its
    /// target's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_id: String,
    /// draft (L5): its provider has a key or a sign-in this hub finds
    /// (env, auth.json, the old .env files), or needs none; false: a pick
    /// needs setup first. Absent: true (an older hub listed only these)
    #[serde(default = "crate::yes", skip_serializing_if = "crate::is_true")]
    pub key_ready: bool,
}

