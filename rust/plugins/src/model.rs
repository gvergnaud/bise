//! The plain data the plugins' modules share (docs/plugins.md, "Contract"):
//! a Mistral connector and its tools ([`crate::connectors`]), a skill of a
//! skills folder or a plugin (the skills scanner), the on/off file as read
//! ([`crate::state`]). No logic here: each type's owner fills it.

use crate::id::PluginId;
use std::path::PathBuf;

/// A connector of his Mistral account, from `/v1/connectors/bootstrap`
/// (and its cache, `bise_home::Home::connectors_cache`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectorEntry {
    /// its name in tool calls and in its id (`gmail`: `mistral:gmail`)
    pub name: String,
    /// what he reads (`Gmail`)
    pub display_name: String,
    /// Mistral's own id of the connector
    pub id: String,
    pub auth: ConnectorAuth,
    /// its tools (none until it is connected)
    pub tools: Vec<ToolEntry>,
    /// his accounts on it, read after the bootstrap
    /// (`/v1/connectors/{id}/user/credentials`): none until it is
    /// connected, nor for a built-in or when Mistral did not list them
    pub accounts: Vec<ConnectorAccount>,
}

/// One of his accounts on a connector: the wire's own type, so the
/// catalog and the rows say the same.
pub use bise_proto::app_rows::{AccountStatus, ConnectorAccount};

/// Whether a connector is ready, or what connecting it takes (the
/// catalog's `auth_action`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorAuth {
    /// connected: its tools work
    Ready,
    /// a browser login (oauth)
    OAuth,
    /// connected in one click, no login (`no_auth_connect`)
    NoAuthConnect,
    /// credentials typed on Mistral's page (`credentials_setup`)
    CredentialsSetup,
    /// not ready, and no `auth_action` bise knows: not connectable from
    /// bise
    Unknown,
}

/// A tool of a connector or an MCP server, as the session's tool index
/// lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    /// its JSON schema, as text
    pub input_schema: String,
}

/// A skill found on disk: a `<dir>/SKILL.md` of a skills folder or of a
/// plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    /// its `SKILL.md`
    pub path: PathBuf,
    /// the plugin it belongs to (`skills:personal`, `docs`)
    pub owner: PluginId,
}

/// `plugins.json` as read (`bise_home::Home::plugins_state`): the plugins
/// turned off, the opt-in plugins turned on, and the same two for parts.
/// Read by [`crate::state::read`], written only by [`crate::state::set`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateV2 {
    /// `"disabled"`: plugins (any id that is not a part) turned off
    pub disabled: Vec<String>,
    /// `"enabled"`: opt-in plugins turned on
    pub enabled: Vec<String>,
    /// `"parts_off"`: parts turned off
    pub parts_off: Vec<PluginId>,
    /// `"parts_on"`: parts turned on again
    pub parts_on: Vec<PluginId>,
}
