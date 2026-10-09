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

use crate::hub::{HubCmd, HubEv};
use crate::{Project, PROTO};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

// the envelope (Id, Request, Notification, Response, RpcError, Message,
// the codes): one file shared with the app <-> core wire (step 6)
pub use crate::jsonrpc::*;

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
    // its steps go to every client as `release/progress`
    m("release/run", "release_run", None),
    // `bise stop` (switchboard client::stop): its `{}` may not come, the
    // hub ends
    m("hub/stop", "stop_hub", None),
    // a note talk's words to its page (the desktop core's voice)
    m("page/voice", "page_voice", None),
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
    n("hub/flow", "flow", Scope::Hub),
    n("job/end", "job_end", Scope::Hub),
    n("job/followedEnd", "followed_end", Scope::Hub),
    n("thread/entry", "entry", Scope::Thread),
    n("thread/typing", "typing", Scope::Thread),
    n("route/held", "route", Scope::Hub),
    n("route/done", "route_done", Scope::Hub),
    n("confirm/ask", "confirm", Scope::One),
    n("card/open", "card_open", Scope::One),
    n("client/focused", "focused", Scope::One),
    n("hub/notice", "notice", Scope::One),
    // P4c-5: `/version`'s picker at hello and while one builds, a
    // `/release-bise` run's steps, `/update`'s build in the source tree
    n("hub/versions", "versions", Scope::Hub),
    n("release/progress", "release", Scope::Hub),
    n("update/progress", "update", Scope::Hub),
    // the pages and his late promises (the older state's pages,
    // overdue), one page that changed (the older `page` line)
    n("hub/pages", "pages", Scope::Hub),
    n("page/changed", "page_changed", Scope::Hub),
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

// ---- client-protocol step 4's glue (architect m_13977) ----
// TODO(client-protocol step 4's end, P4e): this table, its functions and
// the hello's `reads` go when the terminal connects with `initialize`.

/// An older event of the terminal's hello connection and the
/// notifications that replace it. A hello that lists every one of them in
/// its `reads` gets them as notifications, never the older event: the
/// terminal swaps one kind at a time and reads each kind once.
#[derive(Clone, Copy, Debug)]
pub struct Older {
    pub ev: &'static str,
    pub methods: &'static [&'static str],
}

/// The kinds the terminal can read typed so far. A row is added (or
/// grows) in the chunk that sends its notifications on every path the
/// older event took (P4b: `state` gets its `flow`). Never in the TS
/// generation: the window has no older events.
// TODO(client-protocol step 4's end, P4e): delete this table with the
// glue (Older, reads_of, older_sent, the hello's reads, the law).
pub const OLDER: &[Older] = &[
    Older { ev: "state", methods: &["hub/agents", "hub/cards", "hub/scheduled", "hub/flow"] },
    Older { ev: "artifacts", methods: &["hub/artifacts"] },
    Older { ev: "approvals", methods: &["hub/approvals"] },
    Older { ev: "confirm", methods: &["confirm/ask"] },
    // P4c-5: the hub's words, update-card's item, the focus it moved
    Older { ev: "notice", methods: &["hub/notice"] },
    Older { ev: "open_card", methods: &["card/open"] },
    Older { ev: "focus", methods: &["client/focused"] },
    Older { ev: "versions", methods: &["hub/versions"] },
    Older { ev: "release", methods: &["release/progress"] },
    Older { ev: "update", methods: &["update/progress"] },
    // P4d-feed f-c: a subscribed thread's entries and step (line mode
    // first; the terminal's feed with zone-b's switch)
    Older { ev: "line", methods: &["thread/entry", "thread/typing"] },
];

/// What a hello's `reads` stands for: the methods of the [`OLDER`] rows
/// it lists whole (a row half listed is read the older way).
pub fn reads_of(listed: &[String]) -> BTreeSet<&'static str> {
    OLDER.iter().filter(|o| o.methods.iter().all(|m| listed.iter().any(|l| l == m))).flat_map(|o| o.methods.iter().copied()).collect()
}

/// How a typed event reaches an older hello connection that reads some
/// kinds typed ([`hello_way`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelloWay {
    /// its notification (the connection reads that kind)
    Note,
    /// its older line (the connection reads that kind the older way)
    Older,
    /// not typed: a hub-wide event's older line reaches it on its own
    /// path (the broadcast, the hello burst)
    Elsewhere,
}

/// How typed event `tag` reaches a hello connection that reads `reads`;
/// `alone`: it is for that connection only (a request's answer, a
/// notice, a One event), so it never vanishes (architect m_14727: every
/// event reaches the connection exactly one way).
pub fn hello_way(tag: &str, alone: bool, reads: &BTreeSet<&'static str>) -> HelloWay {
    if note_of_ev(tag).is_some_and(|r| reads.contains(r.method)) {
        HelloWay::Note
    } else if alone {
        HelloWay::Older
    } else {
        HelloWay::Elsewhere
    }
}

/// Older event `ev` still goes to a hello connection that reads `reads`.
pub fn older_sent(ev: &str, reads: &BTreeSet<&'static str>) -> bool {
    OLDER.iter().find(|o| o.ev == ev).is_none_or(|o| !o.methods.iter().all(|m| reads.contains(m)))
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

/// What a hub's line says to a connection's `initialize` (its request
/// id `init`): the one reading of an older hub's door, for every client
/// that may meet one (the desktop core, `bise`'s install follow).
#[derive(Clone, Debug, PartialEq)]
pub enum Init {
    /// the hub serves JSON-RPC: who it is, what it serves
    Ready(Box<InitializeResult>),
    /// the hub refused this client (REFUSED, docs/issues/16)
    Refused(String),
    /// an older hub (one release, architect m_13089 Q2): it doesn't
    /// serve `initialize` (accept.rs's `does not serve the op` before
    /// client-protocol, or an error to the request); speak the older
    /// door on a new connection
    Older,
    /// another line: not the answer to `initialize`
    Other,
}

/// [`Init`] of one line, `init` the `initialize` request's id.
// TODO(client-protocol, the plan's 'after the release' step): Older goes
// with the older door it detects, the release after client-protocol's.
pub fn init_answer(v: &Value, init: &Id) -> Init {
    if !is_rpc(v) {
        let older = v.get("ok") == Some(&Value::Bool(false)) && v.get("error").and_then(Value::as_str).is_some_and(|e| e.contains("does not serve"));
        return if older { Init::Older } else { Init::Other };
    }
    let Ok(Message::Response(r)) = Message::from_value(v.clone()) else { return Init::Other };
    if r.id.as_ref() != Some(init) {
        return Init::Other;
    }
    if let Some(e) = r.error {
        return if e.code == code::REFUSED { Init::Refused(e.message) } else { Init::Older };
    }
    match r.result.and_then(|v| serde_json::from_value::<InitializeResult>(v).ok()) {
        Some(res) => Init::Ready(Box::new(res)),
        None => Init::Older,
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
