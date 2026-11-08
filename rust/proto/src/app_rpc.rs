//! The desktop app <-> its core on stdio as JSON-RPC 2.0 (client-protocol
//! step 6; envelope [`crate::jsonrpc`]): `bise ambient-core`.
//!
//! - the app sends `initialize` ([`AppInit`]) first, gets [`AppInitResult`]
//!   (the methods and notifications this core serves), then sends
//!   `initialized`; the core writes nothing before that result and holds
//!   its events until `initialized`;
//! - every command is a [`Request`]: method `core/<cmd tag>` (an
//!   [`AppCmd`]) or `hub/<cmd tag>`
//!   (a [`HubCmd`] for a project's hub, `project` in its params); params
//!   are the command's fields as they are, without `cmd`. Its [`Response`]
//!   is `{}` once the core took it, an error when it couldn't read it;
//! - every event is a [`Notification`]: `hub/<ev tag>` for a [`HubEv`]
//!   (relayed with its `project`, or the core's own error/notice/refused),
//!   `core/<ev tag>` for a [`CoreEv`]; params are its fields without
//!   `ev`.
//!
//! One owner for the names: the tags of [`AppCmd`], [`CoreEv`], [`HubCmd`]
//! and [`HubEv`] (docs/issues/24: no untyped line left);
//! laws in `app_rpc_tests.rs` (and bend-tui's ambient tests: every line the
//! core writes or parses is in a list). A computer-use still
//! (`core/computer_peek`) is never logged, by the core or the app.

use crate::app_cmd::AppCmd;
use crate::core_ev::CoreEv;
use crate::hub::{HubCmd, HubEv};
use crate::jsonrpc::{code, Notification, Request, RpcError};
use crate::PROTO;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The methods and notifications whose params never reach a log, the
/// core's or the app's (a key, a screenshot's place, a computer-use
/// still, the usage counts' install id): a broken or refused one is logged
/// as its name and size only.
pub const NO_LOG: &[&str] = &["core/provider_use", "core/shot", "core/computer_peek", "core/telemetry"];

/// The log's words for a line of this protocol: the line, or its name and
/// size when it is a [`NO_LOG`] one (or might be: it names one).
pub fn loggable(line: &str) -> Option<String> {
    // a whole name first: a still's line holds "shot" too
    let secret = NO_LOG.iter().find(|n| line.contains(*n)).or_else(|| NO_LOG.iter().find(|n| line.contains(n.trim_start_matches("core/"))))?;
    Some(format!("a {secret} line ({} bytes, not logged)", line.len()))
}

/// The protocol's own methods.
pub const INITIALIZE: &str = "initialize";
/// app -> core notification, after `initialize`'s result
pub const INITIALIZED: &str = "initialized";

/// The two families of names.
pub const CORE: &str = "core";
pub const HUB: &str = "hub";

/// The [`AppCmd`] tags no core reads yet (draft: `open` is the core's
/// event to the window, its command has no producer): no method until one
/// does (bend-tui's law: exactly these are unknown to the core's parser).
pub const NOT_SERVED: &[&str] = &["open"];

/// Every command tag the core takes from the app.
pub fn core_cmds() -> impl Iterator<Item = &'static str> {
    AppCmd::TAGS.iter().filter(|t| !NOT_SERVED.contains(t)).copied()
}

/// Every command tag the app sends a project's hub through the core.
// TODO(client-protocol step 7, architect m_15060): when ambient-app and
// client-protocol merge, the hub half's names (hub/<cmd tag>, hub/<ev tag>)
// become rpc::METHODS/NOTIFICATIONS' names (one owner); this module keeps
// only core/<tag>.
pub fn hub_cmds() -> impl Iterator<Item = &'static str> {
    HubCmd::TAGS.iter().copied()
}

/// Every method this core serves, for `initialize`'s result.
pub fn methods() -> Vec<String> {
    let core = core_cmds().map(|t| format!("{CORE}/{t}"));
    let hub = hub_cmds().map(|t| format!("{HUB}/{t}"));
    std::iter::once(INITIALIZE.to_string()).chain(core).chain(hub).collect()
}

/// Every notification this core sends.
pub fn notifications() -> Vec<String> {
    let core = CoreEv::TAGS.iter().map(|t| format!("{CORE}/{t}"));
    let hub = HubEv::TAGS.iter().map(|t| format!("{HUB}/{t}"));
    core.chain(hub).collect()
}

/// `initialize`'s params: who the app is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppInit {
    pub proto: u32,
    pub client: AppClient,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppClient {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// `initialize`'s result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppInitResult {
    pub proto: u32,
    pub methods: Vec<String>,
    pub notifications: Vec<String>,
    /// where the app keeps its artifact card pictures
    /// (`bise_home::Home::thumbs_cache`); '' from an older core: the app
    /// makes none
    #[serde(default)]
    pub thumbs_dir: String,
}

impl AppInitResult {
    /// This core's answer; `thumbs_dir` its home's thumbs cache.
    pub fn now(thumbs_dir: &str) -> AppInitResult {
        AppInitResult { proto: PROTO, methods: methods(), notifications: notifications(), thumbs_dir: thumbs_dir.to_string() }
    }
}

/// A request's command, as the core's parser reads it: `{cmd: tag,
/// ..params}`; Err: the error to answer.
pub fn cmd_of(req: &Request) -> Result<Value, RpcError> {
    let (family, tag) = req.method.split_once('/').ok_or_else(|| unknown(&req.method))?;
    let known = match family {
        CORE => core_cmds().any(|t| t == tag),
        HUB => hub_cmds().any(|t| t == tag),
        _ => false,
    };
    if !known {
        return Err(unknown(&req.method));
    }
    let mut fields = match &req.params {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => return Err(RpcError::new(code::INVALID_PARAMS, format!("{}: params must be an object", req.method))),
    };
    if fields.contains_key("cmd") {
        return Err(RpcError::new(code::INVALID_PARAMS, format!("{}: params carry no cmd (the method says it)", req.method)));
    }
    let has_project = fields.get("project").is_some_and(|p| !p.is_null());
    if family == HUB && !has_project {
        return Err(RpcError::new(code::INVALID_PARAMS, format!("{}: a hub's command needs its project", req.method)));
    }
    // a core command never reads as a project hub's (cmd.rs routes a
    // HubCmd tag with a project to that hub)
    if family == CORE && has_project && HubCmd::TAGS.contains(&tag) {
        return Err(RpcError::new(code::INVALID_PARAMS, format!("{}: with a project it is hub/{tag}", req.method)));
    }
    fields.insert("cmd".into(), Value::String(tag.to_string()));
    Ok(Value::Object(fields))
}

fn unknown(method: &str) -> RpcError {
    RpcError::new(code::METHOD_NOT_FOUND, format!("unknown method {method:?}"))
}

/// The method of a command line `{cmd, ..}` (the app's side): `hub/<tag>`
/// for a hub's command with its project, else `core/<tag>`; None: not a
/// command.
pub fn method_of(cmd: &Value) -> Option<String> {
    let tag = cmd.get("cmd")?.as_str()?;
    let project = cmd.get("project").is_some_and(|p| !p.is_null());
    let family = if project && hub_cmds().any(|t| t == tag) { HUB } else { CORE };
    Some(format!("{family}/{tag}"))
}

/// The request of a command line (the app's side).
pub fn request_of(id: crate::jsonrpc::Id, cmd: &Value) -> Option<Request> {
    let method = method_of(cmd)?;
    let mut params = cmd.as_object()?.clone();
    params.remove("cmd");
    Some(Request::new(id, &method, Value::Object(params)))
}

/// An event's notification: `hub/<tag>` for a [`HubEv`] tag, else
/// `core/<tag>`; None: not an event (no string `ev`).
pub fn note(ev: &Value) -> Option<Notification> {
    let tag = ev.get("ev")?.as_str()?;
    let family = if HubEv::TAGS.contains(&tag) { HUB } else { CORE };
    let mut params = ev.as_object()?.clone();
    params.remove("ev");
    Some(Notification::new(&format!("{family}/{tag}"), Value::Object(params)))
}

/// A notification's event `{ev: tag, ..params}` (the app's side); None:
/// not one of this protocol's families.
pub fn ev_of(n: &Notification) -> Option<Value> {
    let (family, tag) = n.method.split_once('/')?;
    if family != CORE && family != HUB {
        return None;
    }
    let mut fields = match &n.params {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => return None,
    };
    fields.insert("ev".into(), Value::String(tag.to_string()));
    Some(Value::Object(fields))
}

#[cfg(test)]
#[path = "app_rpc_tests.rs"]
mod tests;
