//! bise's client protocol as JSON-RPC 2.0 (client-protocol option 1,
//! architect m_13089; modeled on Vibe's app_server, ADR 0009): one
//! JSON object per line on `hub.sock`, the same for the terminal, the
//! desktop core and any later client.
//!
//! - a client sends [`Request`]s (`id`, `method`, `params`) and gets one
//!   [`Response`] each (`result` or `error`), written before the
//!   notifications its action causes;
//! - the hub sends [`Notification`]s (`method`, `params`): the hub-wide
//!   ones carry a [`Watermark`] (`epoch`, `seq`), so a client that sees a
//!   gap or a new epoch reads the hub again (`hub/read`); a thread's carry
//!   its own numbering, the entry's `pos`;
//! - `initialize` first (`InitializeParams` -> `InitializeResult`: who the
//!   hub is, its methods and notifications, its watermark and hub-wide
//!   state), then `initialized`; nothing else before.
//!
//! The methods and notifications are tables over [`HubCmd`]'s and
//! [`HubEv`]'s tags, one owner: a method's `params` are its command's
//! fields as they are, a notification's its event's fields (no second
//! type per message; laws in `rpc_tests.rs`). [`HubCmd::Hello`] and
//! [`HubEv::Welcome`]/`Refused`/`Error` are the envelope's own business
//! (`initialize`, the error codes) and have no row.

use crate::hub::{ErrorKind, HubCmd, HubEv};
use crate::{Project, PROTO};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// JSON-RPC's version, in every message.
pub const JSONRPC: &str = "2.0";

/// A request's id, as the client chose it (a number or a string).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum Id {
    Num(u64),
    Str(String),
}

fn v2() -> String {
    JSONRPC.to_string()
}

/// client -> hub: one action or read, answered by one [`Response`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Request {
    #[serde(default = "v2")]
    pub jsonrpc: String,
    pub id: Id,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

/// hub -> client (and `initialized`, client -> hub): no answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Notification {
    #[serde(default = "v2")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

/// The answer to one [`Request`]: `result` or `error`. `id` is null only
/// when the request couldn't be read at all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Response {
    #[serde(default = "v2")]
    pub jsonrpc: String,
    pub id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC's error object; `data.kind` lets a client draw a failure by
/// kind, never by its text (architect m_13089, change 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<ErrorData>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ErrorData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ErrorKind>,
    /// a failed `turn/send`: "refused" (nothing reached the thread) or
    /// "undelivered" (the hub wrote its undelivered line in the thread)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The error codes: JSON-RPC's own, then bise's (-32000..-32099).
pub mod code {
    pub const PARSE: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// a request before `initialize`, or a second `initialize`
    pub const NOT_INITIALIZED: i64 = -32002;
    /// the hub refused this connection (docs/issues/16), then closes it
    pub const REFUSED: i64 = -32010;
    /// the hub refused this action (sb-core's or the handler's words)
    pub const HUB_REFUSED: i64 = -32011;
    /// the hub is older than the client: it doesn't know this method
    pub const HUB_OLDER: i64 = -32012;
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> RpcError {
        RpcError { code, message: message.into(), data: None }
    }

    /// The hub refused this action: its words, `reason` for a send.
    pub fn refused(message: impl Into<String>, reason: Option<&str>) -> RpcError {
        let data = reason.map(|r| ErrorData { kind: None, reason: Some(r.to_string()) });
        RpcError { code: code::HUB_REFUSED, message: message.into(), data }
    }

    pub fn with_kind(mut self, kind: ErrorKind) -> RpcError {
        self.data.get_or_insert_with(ErrorData::default).kind = Some(kind);
        self
    }
}

/// One line read from the other end.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Request(Request),
    Notification(Notification),
    Response(Response),
}

impl Message {
    /// One line. An error is the [`Response`] to write back (`id` null
    /// when even that couldn't be read).
    pub fn read(line: &str) -> Result<Message, Box<Response>> {
        let v: Value = serde_json::from_str(line).map_err(|e| Box::new(Response::err(None, RpcError::new(code::PARSE, format!("not JSON: {e}")))))?;
        Message::from_value(v)
    }

    pub fn from_value(v: Value) -> Result<Message, Box<Response>> {
        let id = v.get("id").and_then(|i| serde_json::from_value::<Id>(i.clone()).ok());
        let bad = |why: String| Box::new(Response::err(id.clone(), RpcError::new(code::INVALID_REQUEST, why)));
        if !is_rpc(&v) {
            return Err(bad(format!("not JSON-RPC {JSONRPC}")));
        }
        let has = |k: &str| v.get(k).is_some();
        if has("method") {
            if has("id") {
                return serde_json::from_value(v).map(Message::Request).map_err(|e| bad(e.to_string()));
            }
            return serde_json::from_value(v).map(Message::Notification).map_err(|e| bad(e.to_string()));
        }
        if has("result") || has("error") {
            return serde_json::from_value(v).map(Message::Response).map_err(|e| bad(e.to_string()));
        }
        Err(bad("neither a request, a notification nor a response".into()))
    }

    pub fn to_value(&self) -> Value {
        match self {
            Message::Request(m) => serde_json::to_value(m),
            Message::Notification(m) => serde_json::to_value(m),
            Message::Response(m) => serde_json::to_value(m),
        }
        .unwrap_or(Value::Null)
    }

    /// One line, without its newline.
    pub fn encode(&self) -> String {
        self.to_value().to_string()
    }
}

/// A JSON line of this protocol (`"jsonrpc": "2.0"`), not one of the
/// hub's older ops (`op`) or typed commands (`cmd`).
pub fn is_rpc(v: &Value) -> bool {
    v.get("jsonrpc").and_then(Value::as_str) == Some(JSONRPC)
}

impl Request {
    pub fn new(id: Id, method: &str, params: Value) -> Request {
        Request { jsonrpc: v2(), id, method: method.to_string(), params }
    }
}

impl Notification {
    pub fn new(method: &str, params: Value) -> Notification {
        Notification { jsonrpc: v2(), method: method.to_string(), params }
    }
}

impl Response {
    pub fn ok(id: Id, result: Value) -> Response {
        Response { jsonrpc: v2(), id: Some(id), result: Some(result), error: None }
    }

    pub fn err(id: Option<Id>, error: RpcError) -> Response {
        Response { jsonrpc: v2(), id, result: None, error: Some(error) }
    }
}

// ---- the tables ----

/// One method: its name, the [`HubCmd`] tag whose fields are its params,
/// and the [`HubEv`] tag whose fields are its result when it has one (a
/// read); none: an action, its result is `{}` and its change comes in
/// the notifications.
#[derive(Clone, Copy, Debug)]
pub struct MethodRow {
    pub method: &'static str,
    pub cmd: &'static str,
    pub result: Option<&'static str>,
}

const fn m(method: &'static str, cmd: &'static str, result: Option<&'static str>) -> MethodRow {
    MethodRow { method, cmd, result }
}

/// Every method that is a [`HubCmd`].
pub const METHODS: &[MethodRow] = &[
    m("thread/subscribe", "subscribe", Some("thread")),
    m("thread/unsubscribe", "unsubscribe", None),
    m("thread/page", "page", Some("thread")),
    m("turn/send", "send", None),
    m("turn/interrupt", "stop", None),
    m("card/answer", "answer", None),
    m("card/close", "close", None),
    m("confirm/answer", "confirm", None),
    // no mode: a read (the current mode and rules); a mode: an action
    m("approvals/set", "approvals", Some("approvals")),
    m("approvals/removeRule", "remove_rule", None),
    m("agent/new", "new", None),
    m("agent/archive", "archive", None),
    m("agent/unarchive", "unarchive", None),
    m("agent/rename", "rename", None),
    m("agent/model", "model", None),
    m("agent/effort", "effort", None),
    m("agent/follow", "follow", None),
    m("artifacts/seen", "artifacts_seen", None),
    m("diff/read", "diff", Some("diff")),
    m("worktrees/list", "worktrees", Some("worktrees")),
    m("devServers/list", "dev_servers", Some("dev_servers")),
    m("merged/list", "merged", Some("merged")),
    m("features/list", "features", Some("features")),
    m("prs/list", "prs", Some("prs")),
    m("scheduled/list", "scheduled", Some("scheduled")),
    m("scheduled/stop", "scheduled_stop", None),
    m("models/list", "models", Some("models")),
    m("route/correct", "route_correct", None),
    m("route/cancel", "route_cancel", None),
    // the line he typed (say, @route, a hub command): the hub's one
    // parser, then the same handlers as the typed methods; its result is
    // a [`CommandRunResult`] (OWN_RESULTS)
    m("command/run", "slash", None),
    // client-protocol step 3: the terminal's ops, typed (architect m_13313)
    m("scheduled/run", "scheduled_run", None),
    m("artifacts/add", "artifacts_add", None),
    m("branches/list", "branches", Some("branches")),
    m("client/focus", "focus", None),
    m("versions/list", "versions", Some("versions")),
    m("version/info", "version_info", None),
    m("version/switch", "version_switch", None),
    m("version/rollback", "version_rollback", None),
    m("version/restart", "version_restart", None),
    // its words come as hub/notice (the release check answers later)
    m("version/update", "version_update", None),
    m("release/plan", "release_plan", Some("release")),
    // its steps go to every client as the hub's older `release` lines
    m("release/run", "release_run", None),
];

/// The protocol's own methods (no [`HubCmd`]).
pub const INITIALIZE: &str = "initialize";
/// client -> hub notification, after `initialize`'s result
pub const INITIALIZED: &str = "initialized";
/// the hub-wide state and its watermark again (a gap, a new epoch)
pub const HUB_READ: &str = "hub/read";
/// the slash commands' catalog ([`CommandsList`])
pub const COMMANDS_LIST: &str = "commands/list";
pub const OWN_METHODS: &[&str] = &[INITIALIZE, HUB_READ, COMMANDS_LIST];
pub const COMMAND_RUN: &str = "command/run";

/// The methods whose result is a type of this module, not a [`HubEv`]'s
/// fields: method -> its result's type name (for the TypeScript map).
pub const OWN_RESULTS: &[(&str, &str)] = &[
    (INITIALIZE, "InitializeResult"),
    (HUB_READ, "HubState"),
    (COMMANDS_LIST, "CommandsList"),
    (COMMAND_RUN, "CommandRunResult"),
    // the hub's words (architect m_13313: one owned type for them)
    ("artifacts/add", "CommandRunResult"),
    ("version/info", "CommandRunResult"),
    ("version/switch", "CommandRunResult"),
    ("version/rollback", "CommandRunResult"),
    ("version/restart", "CommandRunResult"),
];

/// A command whose method answers the hub's words ([`CommandRunResult`]):
/// its arm's `notice` is its result.
pub fn says(cmd: &str) -> bool {
    method_of_cmd(cmd).is_some_and(|r| OWN_RESULTS.iter().any(|(m, t)| *m == r.method && *t == "CommandRunResult"))
}

/// Who gets a notification, and so how it is numbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// every initialized connection, numbered by the hub's [`Watermark`]
    /// (a gap means a loss: read the hub again)
    Hub,
    /// the connections subscribed to a thread; the entry's `pos` is its
    /// number (`thread/subscribe {after}` resumes)
    Thread,
    /// the one connection it is for (a confirm, a notice): not numbered
    One,
}

#[derive(Clone, Copy, Debug)]
pub struct NoteRow {
    pub method: &'static str,
    pub ev: &'static str,
    pub scope: Scope,
}

const fn n(method: &'static str, ev: &'static str, scope: Scope) -> NoteRow {
    NoteRow { method, ev, scope }
}

/// Every notification that is a [`HubEv`]. `thread` and `diff` are only
/// results (they answer a request, never pushed).
pub const NOTIFICATIONS: &[NoteRow] = &[
    n("hub/agents", "agents", Scope::Hub),
    n("hub/cards", "cards", Scope::Hub),
    n("hub/jobs", "jobs", Scope::Hub),
    n("hub/artifacts", "artifacts", Scope::Hub),
    n("hub/scheduled", "scheduled", Scope::Hub),
    n("hub/worktrees", "worktrees", Scope::Hub),
    n("hub/devServers", "dev_servers", Scope::Hub),
    n("hub/merged", "merged", Scope::Hub),
    n("hub/features", "features", Scope::Hub),
    n("hub/prs", "prs", Scope::Hub),
    n("hub/models", "models", Scope::Hub),
    n("hub/approvals", "approvals", Scope::Hub),
    n("job/end", "job_end", Scope::Hub),
    n("job/followedEnd", "followed_end", Scope::Hub),
    n("thread/entry", "entry", Scope::Thread),
    n("thread/typing", "typing", Scope::Thread),
    n("route/held", "route", Scope::Hub),
    n("route/done", "route_done", Scope::Hub),
    n("confirm/ask", "confirm", Scope::One),
    n("hub/notice", "notice", Scope::One),
];

/// The [`HubCmd`] tags the envelope replaces (`initialize`).
pub const ENVELOPE_CMDS: &[&str] = &["hello"];
/// The [`HubEv`] tags the envelope replaces (`initialize`'s result, the
/// error codes).
pub const ENVELOPE_EVS: &[&str] = &["welcome", "refused", "error"];

pub fn method_row(method: &str) -> Option<&'static MethodRow> {
    METHODS.iter().find(|r| r.method == method)
}

pub fn method_of_cmd(tag: &str) -> Option<&'static MethodRow> {
    METHODS.iter().find(|r| r.cmd == tag)
}

pub fn note_row(method: &str) -> Option<&'static NoteRow> {
    NOTIFICATIONS.iter().find(|r| r.method == method)
}

pub fn note_of_ev(tag: &str) -> Option<&'static NoteRow> {
    NOTIFICATIONS.iter().find(|r| r.ev == tag)
}

/// Every method this version serves, for `initialize`'s result.
pub fn methods() -> Vec<String> {
    OWN_METHODS.iter().copied().chain(METHODS.iter().map(|r| r.method)).map(str::to_string).collect()
}

/// Every notification this version sends.
pub fn notifications() -> Vec<String> {
    NOTIFICATIONS.iter().map(|r| r.method.to_string()).collect()
}

// ---- messages <-> the typed commands and events ----

/// A tagged value's fields as an object, without its tag.
fn fields(mut v: Value, key: &str) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove(key);
    }
    v
}

/// An object's fields with `key: tag` added.
fn tagged(params: Value, key: &str, tag: &str) -> Result<Value, String> {
    let mut o = match params {
        Value::Object(o) => o,
        Value::Null => Map::new(),
        _ => return Err("params must be an object".into()),
    };
    o.insert(key.to_string(), Value::String(tag.to_string()));
    Ok(Value::Object(o))
}

/// A request's method and params as its [`HubCmd`]. Not a command's
/// method: METHOD_NOT_FOUND (the protocol's own methods are the caller's);
/// bad params: INVALID_PARAMS.
pub fn cmd(method: &str, params: Value) -> Result<HubCmd, RpcError> {
    let row = method_row(method).ok_or_else(|| RpcError::new(code::METHOD_NOT_FOUND, format!("unknown method: {method}")))?;
    let v = tagged(params, "cmd", row.cmd).map_err(|e| RpcError::new(code::INVALID_PARAMS, format!("{method}: {e}")))?;
    // row.cmd is a known tag (law): never `Unknown`
    HubCmd::from_value(v).map_err(|e| RpcError::new(code::INVALID_PARAMS, format!("{method}: {e}")))
}

/// A [`HubCmd`] as the request a client sends (none: `hello` and an
/// unknown tag have no method).
pub fn request(id: Id, c: &HubCmd) -> Option<Request> {
    let v = c.to_value();
    let row = method_of_cmd(v.get("cmd")?.as_str()?)?;
    Some(Request::new(id, row.method, fields(v, "cmd")))
}

/// Where a hub-wide notification stands: the hub's `epoch` (one per hub
/// run: a restart or a reload starts another) and its `seq` in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Watermark {
    pub epoch: u64,
    pub seq: u64,
}

/// What a client does with a numbered notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    /// the next one: apply it, the watermark moves
    Apply,
    /// at or below the watermark: seen already
    Skip,
    /// a gap or another epoch: read the hub again (`hub/read`), whose
    /// watermark replaces this one
    Resync,
}

impl Watermark {
    /// The client's reducer rule (Vibe's): skip what is seen, apply the
    /// next, resync on a gap or a new epoch. Apply moves `self`.
    pub fn take(&mut self, w: Watermark) -> Take {
        if w.epoch != self.epoch {
            return Take::Resync;
        }
        if w.seq <= self.seq {
            return Take::Skip;
        }
        if w.seq != self.seq + 1 {
            return Take::Resync;
        }
        self.seq = w.seq;
        Take::Apply
    }
}

/// A [`HubEv`] as its notification, numbered with `w` when it is
/// hub-wide (none: an envelope event, a result-only one, an unknown tag).
pub fn note(ev: &HubEv, w: Option<Watermark>) -> Option<Notification> {
    let v = ev.to_value();
    let row = note_of_ev(v.get("ev")?.as_str()?)?;
    let mut params = fields(v, "ev");
    if let (Some(w), Scope::Hub, Some(o)) = (w, row.scope, params.as_object_mut()) {
        o.insert("epoch".into(), w.epoch.into());
        o.insert("seq".into(), w.seq.into());
    }
    Some(Notification::new(row.method, params))
}

/// A notification as its [`HubEv`] and watermark (a hub-wide one's). An
/// unknown method: `HubEv::Unknown`, never an error (a newer hub).
pub fn ev(n: &Notification) -> Result<(HubEv, Option<Watermark>), String> {
    let Some(row) = note_row(&n.method) else {
        return Ok((HubEv::Unknown { tag: n.method.clone(), raw: n.params.clone() }, None));
    };
    let mut params = n.params.clone();
    let mut w = None;
    if let (Scope::Hub, Some(o)) = (row.scope, params.as_object_mut()) {
        let epoch = o.remove("epoch").and_then(|v| v.as_u64());
        let seq = o.remove("seq").and_then(|v| v.as_u64());
        if let (Some(epoch), Some(seq)) = (epoch, seq) {
            w = Some(Watermark { epoch, seq });
        }
    }
    Ok((HubEv::from_value(tagged(params, "ev", row.ev)?)?, w))
}

/// A read's [`HubEv`] as the result of its request: its fields.
pub fn result(ev: &HubEv) -> Value {
    fields(ev.to_value(), "ev")
}

/// A method's result as its [`HubEv`] (a read's); none: an action's.
pub fn ev_of_result(method: &str, result: Value) -> Result<Option<HubEv>, String> {
    let Some(tag) = method_row(method).and_then(|r| r.result) else { return Ok(None) };
    if result.as_object().is_some_and(Map::is_empty) {
        return Ok(None);
    }
    HubEv::from_value(tagged(result, "ev", tag)?).map(Some)
}

/// `commands/list`'s result: every slash command, owned rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CommandsList {
    pub commands: Vec<crate::commands::CommandRow>,
}

impl CommandsList {
    pub fn now() -> CommandsList {
        CommandsList { commands: crate::commands::rows() }
    }
}

/// `command/run`'s result (architect m_13277), and the result of every
/// method that answers the hub's words (`artifacts/add`, `version/info`,
/// ... : OWN_RESULTS): `notice` those words (`/help`, `/flow`,
/// `/artifacts add`, the versions); none: what it did shows in the
/// notifications.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CommandRunResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

// ---- initialize ----

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClientInfo {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// `initialize`'s params: the client says who it is and which protocol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InitializeParams {
    pub proto: u32,
    pub client: ClientInfo,
    /// what the client hosts (none yet: an object, so a later field is
    /// not a break)
    #[serde(default)]
    pub capabilities: Value,
}

impl InitializeParams {
    pub fn new(name: &str, version: &str) -> InitializeParams {
        InitializeParams { proto: PROTO, client: ClientInfo { name: name.into(), version: version.into() }, capabilities: Value::Object(Map::new()) }
    }
}

/// The hub-wide state at a watermark: `initialize`'s and `hub/read`'s.
/// `state` holds one hub-wide notification of each kind, as it would be
/// sent now (unnumbered: the watermark covers them). Threads are not in
/// it: `thread/subscribe` brings each (architect m_13089, change 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HubState {
    pub watermark: Watermark,
    pub state: Vec<Notification>,
}

/// `initialize`'s result: who this hub is, what it serves, its state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InitializeResult {
    pub project: Project,
    pub proto: u32,
    /// the workspace's folder, and its name
    pub workspace: String,
    pub name: String,
    /// the hub's executable: a client of another version follows it
    #[serde(default)]
    pub exe: String,
    /// the hub's state folder
    #[serde(default)]
    pub state_dir: String,
    /// the hub's version (`switch::version_info`)
    #[serde(default)]
    pub version: Value,
    /// set when a reload started this hub (BISE-131)
    #[serde(default)]
    pub reload: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_url: Option<String>,
    pub methods: Vec<String>,
    pub notifications: Vec<String>,
    pub hub: HubState,
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
