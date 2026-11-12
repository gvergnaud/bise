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
//! type per message; laws in `rpc_tests.rs`).
//! [`HubEv::Welcome`]/`Refused`/`Error` are the envelope's own business
//! (`initialize`, the error codes) and have no row.

use crate::hub::{HubCmd, HubEv};
use crate::{Project, PROTO};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    m("thread/find", "find", Some("thread_hits")),
    m("turn/send", "send", None),
    m("turn/interrupt", "stop", None),
    m("card/answer", "answer", None),
    m("card/close", "close", None),
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
    // a diff's guide (page review-guided): kept, small, being written, or
    // its call; a written or failed one comes to every connection too
    m("guide/get", "guide", Some("guide")),
    // a tool row opened: its whole output from the session log (main's
    // 5a9cc5f2), answered later from a thread
    m("tool/output", "tool_out", Some("tool_out")),
    // the window's /log: a page of an agent's raw session (TP-N2)
    m("log/read", "log", Some("log")),
    // /debug completion: a request of the agent's ring, as rows
    m("log/completion", "completion", Some("completion")),
    // /debug's 'show more': one cut body whole (debug-more)
    m("log/body", "log_body", Some("log_body")),
    // his queued message taken back (the window's "edit · drop")
    m("turn/unqueue", "queued_take", Some("queued_taken")),
    // his queued message said now (the window's "send now"): sb-core
    // steers it in, its id kept; the agents event drops it from the queue
    m("turn/steerQueued", "queued_steer", None),
    m("worktrees/list", "worktrees", Some("worktrees")),
    // running processes (docs/processes.md): the list, an output range,
    // following it, stop (the user's authority)
    m("processes/list", "processes", Some("processes")),
    m("process/output", "proc_out", Some("proc_out")),
    m("process/follow", "proc_follow", None),
    m("process/stop", "proc_stop", None),
    m("merged/list", "merged", Some("merged")),
    m("features/list", "features", Some("features")),
    m("prs/list", "prs", Some("prs")),
    // his Merge or Close on a PR's tab: answered once the forge said
    m("pr/act", "pr_act", Some("pr_acted")),
    // a PR's tab: its detail and a check's log, from the forge on a thread
    m("pr/detail", "pr_detail", Some("pr_detail")),
    m("pr/checkLog", "pr_check_log", Some("pr_check_log")),
    // inbox-record: his answered and withdrawn cards, one page
    m("inbox/history", "inbox_history", Some("inbox_history")),
    m("scheduled/list", "scheduled", Some("scheduled")),
    m("scheduled/stop", "scheduled_stop", None),
    m("wake/stop", "wake_stop", None),
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
    // artifacts in folders (docs/artifacts.md, "Folders")
    m("artifacts/move", "artifacts_move", Some("artifacts_moved")),
    m("artifacts/folders", "artifacts_folders", Some("folders")),
    m("artifacts/library", "artifacts_library", Some("library")),
    m("branches/list", "branches", Some("branches")),
    m("client/focus", "focus", None),
    m("versions/list", "versions", Some("versions")),
    m("version/info", "version_info", None),
    m("version/switch", "version_switch", None),
    m("version/rollback", "version_rollback", None),
    m("version/restart", "version_restart", None),
    // its words come as hub/notice (the release check answers later)
    m("version/update", "version_update", None),
    // update now (update-flow): his click on the status pill or line;
    // client sockets only (issue 16), never an agent's op
    m("bise/update", "bise_update", None),
    m("release/plan", "release_plan", Some("release")),
    // its steps go to every client as `release/progress`
    m("release/run", "release_run", None),
    // `bise stop` (client::stop): its answer may never come, the hub ends
    m("hub/stop", "stop_hub", None),
    // a note talk's words to its page (the desktop core's voice)
    m("page/voice", "page_voice", None),
    // cost-pause: the project's cost, its pause and resume (his only)
    m("usage/read", "usage", Some("usage")),
    m("project/pause", "pause", None),
    // MCP Apps C9: open this in the desktop app (the TUI's `app ↗`)
    m("app/open", "app_open", None),
    m("project/resume", "resume", None),
    // mcp-apps (docs/mcp-apps.md, C5): a view's requests, client sockets
    // only (never an agent's op); answered later, by the server or his
    // card (view_rpc's codes on a refusal)
    m("view/resource", "view_resource", None),
    m("view/call", "view_call", None),
    m("view/message", "view_message", None),
    m("view/context", "view_context", None),
    // C8: a plugin's views, through the hub's views bridge
    m("plugin/views", "plugin_views", None),
    // plugin-apps (docs/plugin-apps.md A4): a document view's session ends
    // (his tab); client sockets only, as view/*
    m("view/close", "view_close", None),
    // A5: a document view's notes ({notes, detached}), client sockets only
    m("view/notes", "view_notes", None),
    // his click opens a document in its app (A4): client sockets only
    m("view/open", "view_open", None),
    // plugin-apps A6b: Keep mine as a copy, the host's own (architect m_60222)
    m("view/copy", "view_copy", None),
];

/// The protocol's own methods (no [`HubCmd`]).
pub const INITIALIZE: &str = "initialize";
/// client -> hub notification, after `initialize`'s result
pub const INITIALIZED: &str = "initialized";
/// the hub-wide state and its watermark again (a gap, a new epoch)
pub const HUB_READ: &str = "hub/read";
/// the slash commands' catalog ([`CommandsList`])
pub const COMMANDS_LIST: &str = "commands/list";
/// agents-light (architect m_37220): one agent's full row, archived or
/// not ([`AgentGetParams`] -> [`AgentGetResult`]): what an archived
/// agent's light row doesn't carry
pub const AGENT_GET: &str = "agent/get";
pub const OWN_METHODS: &[&str] = &[INITIALIZE, HUB_READ, COMMANDS_LIST, AGENT_GET];
pub const COMMAND_RUN: &str = "command/run";

/// The methods whose result is a type of this module, not a [`HubEv`]'s
/// fields: method -> its result's type name (for the TypeScript map).
pub const OWN_RESULTS: &[(&str, &str)] = &[
    (INITIALIZE, "InitializeResult"),
    (HUB_READ, "HubState"),
    (COMMANDS_LIST, "CommandsList"),
    (AGENT_GET, "AgentGetResult"),
    (COMMAND_RUN, "CommandRunResult"),
    // the hub's words (architect m_13313: one owned type for them)
    ("artifacts/add", "CommandRunResult"),
    ("version/info", "CommandRunResult"),
    ("version/switch", "CommandRunResult"),
    ("version/rollback", "CommandRunResult"),
    ("version/restart", "CommandRunResult"),
    // cost-pause: what the pause or resume did ('paused: 2 turns stopped…')
    ("project/pause", "CommandRunResult"),
    ("project/resume", "CommandRunResult"),
    // A1 (architect m_21450): his message's id, for his pending row
    ("turn/send", "SendResult"),
    // inbox picks (3) (architect m_22636): his answer as sb-core wrote it
    ("card/answer", "AnswerResult"),
    // mcp-apps (C5): a view's resource; a call answers its server's result
    ("view/resource", "ViewResource"),
    ("view/call", "ViewCallResult"),
    ("view/open", "ViewOpened"),
    ("plugin/views", "PluginViewsResult"),
    // plugin-apps A6b
    ("view/copy", "ViewCopyResult"),
    // MCP Apps C9: how many desktop connections got it
    ("app/open", "AppOpenResult"),
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

/// Every notification that is a [`HubEv`]. `thread`, `diff` and
/// `tool_out` are only results (they answer a request, never pushed).
pub const NOTIFICATIONS: &[NoteRow] = &[
    n("hub/agents", "agents", Scope::Hub),
    n("hub/guide", "guide", Scope::Hub),
    n("hub/cards", "cards", Scope::Hub),
    n("hub/jobs", "jobs", Scope::Hub),
    n("hub/artifacts", "artifacts", Scope::Hub),
    n("hub/scheduled", "scheduled", Scope::Hub),
    n("hub/worktrees", "worktrees", Scope::Hub),
    n("hub/processes", "processes", Scope::Hub),
    // a followed process's output as it grows (`process/follow`)
    n("process/output", "proc_out", Scope::One),
    n("hub/merged", "merged", Scope::Hub),
    n("hub/features", "features", Scope::Hub),
    n("hub/prs", "prs", Scope::Hub),
    n("hub/models", "models", Scope::Hub),
    n("hub/approvals", "approvals", Scope::Hub),
    n("hub/flow", "flow", Scope::Hub),
    n("hub/paused", "paused", Scope::Hub),
    n("hub/switch", "switch", Scope::Hub),
    // les bims: how much the project has done (rows::growth)
    n("hub/growth", "growth", Scope::Hub),
    // the H1 sidebar: the project's pulse (rows::pulse)
    n("hub/pulse", "pulse", Scope::Hub),
    // a newer bise, as this hub sees it (rows::bise_update, update-flow)
    n("hub/bise_update", "bise_update", Scope::Hub),
    n("job/end", "job_end", Scope::Hub),
    n("job/followedEnd", "followed_end", Scope::Hub),
    // les bims: a message sent, a report made (no body, no text)
    n("hub/activity", "activity", Scope::Hub),
    n("thread/entry", "entry", Scope::Thread),
    n("thread/typing", "typing", Scope::Thread),
    // the answer as it streams in (whole text; "" when it ended)
    n("thread/partial", "partial", Scope::Thread),
    n("route/held", "route", Scope::Hub),
    n("route/done", "route_done", Scope::Hub),
    n("card/open", "card_open", Scope::One),
    // MCP Apps C9: a terminal's `app ↗`, to the desktop connections only
    n("app/open", "app_open", Scope::One),
    // plugin-apps A5: an artifact's notes were written (ids only): its
    // document views ask view/notes again
    n("artifact/notes", "artifact_notes", Scope::Hub),
    // inbox-record: a card closed, or its answer's position came
    n("inbox/recorded", "inbox_recorded", Scope::Hub),
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

/// A request's method and params as its [`HubCmd`]. A retired method
/// (crate::RETIRED): refused by name; not a command's method:
/// METHOD_NOT_FOUND (the protocol's own methods are the caller's); bad
/// params: INVALID_PARAMS.
pub fn cmd(method: &str, params: Value) -> Result<HubCmd, RpcError> {
    if let Some(r) = crate::retired(crate::RetiredKind::RpcMethod, method) {
        return Err(RpcError::retired(r));
    }
    let row = method_row(method).ok_or_else(|| RpcError::new(code::METHOD_NOT_FOUND, format!("unknown method: {method}")))?;
    let v = tagged(params, "cmd", row.cmd).map_err(|e| RpcError::new(code::INVALID_PARAMS, format!("{method}: {e}")))?;
    // an older app's removed field with data: refused by name (REMOVED)
    if let Some(r) = crate::removed_in(&v, "cmd") {
        return Err(RpcError::removed(r));
    }
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

/// `turn/send`'s result (A1, architect m_21450): `msg_id` the id sb-core
/// gave his message when it arrived (queued too), the `msg_id` of the
/// 'you' entry it becomes, so the sender matches his pending row by id.
/// `reason` (send-reason, architect m_30519): whether a message was made,
/// and why not; with no message there is no 'you' entry to wait for, so
/// the sender's row settles on this result. Both absent (`{}`): an older
/// hub's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SendResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<SendReason>,
}

/// What a `turn/send` did with his words ([`SendResult::reason`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SendReason {
    /// a message of his (`msg_id` its id): its 'you' entry comes
    Made,
    /// a page's question answered whole (a step ticked, a batch card):
    /// the page took it, no message
    Page,
    /// words for another project (`@<project> …` on bise's home hub):
    /// held for that project's hub, no message here
    Routed,
    /// a command line the hub ran (`/help`, a `/model`): its words come
    /// as notices, no message
    Command,
    /// a newer hub's reason
    #[serde(other)]
    Unknown,
}

/// `card/answer`'s result (inbox picks (3), architect m_22636): his answer
/// as sb-core wrote its route line in the step that closed the card
/// (`AStep.ans_*`): `agent` who asked, `question` the card's text, `said`
/// what he said, `pick` the option's words when he picked one by its
/// number. `notice`, `note`: the words of 'you answered' for that answer
/// (`words::answered_split`, the thread row's). `answer` absent (`{}`)
/// from an older hub or an answer a page took: the window keeps its own
/// words.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AnswerResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<CardAnswer>,
}

/// His answer to a card, as sb-core wrote its route line ([`AnswerResult`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardAnswer {
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<String>,
    #[serde(default)]
    pub notice: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl CardAnswer {
    /// A step's answer to a card, copied as is; none when the step
    /// answered no card (an older sb-core's step too).
    pub fn of_step(s: &crate::core::link::AStep) -> Option<CardAnswer> {
        let (agent, question, said) = (s.ans_who.clone()?, s.ans_ask.clone()?, s.ans_said.clone()?);
        s.ans_card?;
        let words = s.ans_pick.clone().unwrap_or_else(|| said.clone());
        let (notice, note) = crate::thread::words::answered_split(&agent, &words, &|w: &str| w.chars().count());
        Some(CardAnswer { agent, question, said, pick: s.ans_pick.clone(), notice, note })
    }
}

/// `app/open`'s result (docs/mcp-apps.md C9): how many connections that
/// said `desktop` got its `app/open` notification; 0: no app on this hub.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppOpenResult {
    pub delivered: u32,
}

// ---- initialize ----

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClientInfo {
    pub name: String,
    #[serde(default)]
    pub version: String,
    /// what it is (docs/mcp-apps.md C9): the hub sends `app/open` to the
    /// `desktop` ones only; absent: unknown, an older client is never
    /// taken for the app
    #[serde(default, skip_serializing_if = "ClientKind::is_unknown")]
    pub kind: ClientKind,
}

/// A client's kind, as it says it at `initialize`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    /// the TUI
    Terminal,
    /// the desktop app's core (`bise ambient-core`)
    Desktop,
    /// a browser page of the hub's pages server
    Web,
    /// said nothing, or a kind this version doesn't know
    #[default]
    #[serde(other)]
    Unknown,
}

impl ClientKind {
    pub fn is_unknown(&self) -> bool {
        *self == ClientKind::Unknown
    }
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

/// agents-light (architect m_37220): the client reads `HubEv::Agents`'
/// `archived` light list (and merges it with `rows::Archived`); without
/// it, a hub sends the older shape (every archived row in full inside
/// `agents`).
pub const CAP_ARCHIVED_LIST: &str = "archived_list";

impl InitializeParams {
    /// Every Rust client of this version reads the archived light list
    /// ([`CAP_ARCHIVED_LIST`]): the TUI and the desktop's core merge it
    /// (`rows::Archived`), `bise stop` and `/version` never read agents.
    pub fn new(name: &str, version: &str) -> InitializeParams {
        let caps = Map::from_iter([(CAP_ARCHIVED_LIST.to_string(), Value::Bool(true))]);
        InitializeParams { proto: PROTO, client: ClientInfo { name: name.into(), version: version.into(), kind: ClientKind::Unknown }, capabilities: Value::Object(caps) }
    }

    /// The same, saying what the client is ([`ClientKind`]).
    pub fn of_kind(mut self, kind: ClientKind) -> InitializeParams {
        self.client.kind = kind;
        self
    }

    /// Whether the client said [`CAP_ARCHIVED_LIST`].
    pub fn reads_archived_list(&self) -> bool {
        self.capabilities.get(CAP_ARCHIVED_LIST).and_then(Value::as_bool) == Some(true)
    }
}

/// `agent/get`'s params: the agent, by its name now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AgentGetParams {
    pub name: String,
}

/// `agent/get`'s result: its full row (the agents event's builder), none
/// when the hub has no agent of that name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AgentGetResult {
    #[serde(default)]
    pub agent: Option<crate::rows::Agent>,
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
/// id `init`): the one reading, for every client (the desktop core,
/// `bise stop`, the version switch at launch, the terminal).
#[derive(Clone, Debug, PartialEq)]
pub enum Init {
    /// the hub serves JSON-RPC: who it is, what it serves
    Ready(Box<InitializeResult>),
    /// the hub refused this client (REFUSED, docs/issues/16)
    Refused(String),
    /// this hub doesn't serve `initialize`: a hub from before
    /// client-protocol (v2026.10.2-30) answers `does not serve the op`,
    /// an error to the request or a result that isn't
    /// [`InitializeResult`]. Nothing more is said to it: the client
    /// says [`HUB_OLDER`].
    NotRpc,
    /// another line: not the answer to `initialize`
    Other,
}

/// What a client says when [`Init::NotRpc`] answers it: a hub from before
/// client-protocol (v2026.10.2-30) is no longer spoken to. The one
/// sentence, for every client (`bise stop`, the version switch at launch,
/// the desktop core).
pub const HUB_OLDER: &str = "this project's hub is older than this bise: stop it with `bise switchboard --stop`, then start bise again";

/// What a client says instead of [`HUB_OLDER`] when that hub runs from an
/// install (bise_home::release::older_hub; ambient-lead m_23937, architect
/// m_24004): the steps in a terminal, its own bise first (a newer one no
/// longer speaks to it). `current`: the install's `current` is still that
/// hub's version (its `bise` stops it, then `bise update`); else `exe`,
/// the hub's own binary as he types it (`~/…`), stops it. The one sentence
/// for every client (the desktop core, `bise switchboard --stop`, the
/// version switch at launch).
pub fn hub_older_steps(current: bool, exe: &str) -> String {
    let head = "this project's hub runs an older bise: in a terminal, in that project, run";
    if current {
        format!("{head} `bise switchboard --stop`, then `bise update`, then `bise`")
    } else {
        format!("{head} `{} switchboard --stop`, then `bise`", shell_word(exe))
    }
}

/// A path as one shell word: as is when it has only safe characters, else
/// single-quoted after a leading `~/` (kept outside the quotes so the
/// shell still expands it).
fn shell_word(p: &str) -> String {
    let safe = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+=:@%,".contains(c));
    let (tilde, rest) = p.strip_prefix("~/").map(|r| ("~/", r)).unwrap_or(("", p));
    if safe(rest) {
        return p.to_string();
    }
    format!("{tilde}'{}'", rest.replace('\'', "'\\''"))
}

/// [`Init`] of one line, `init` the `initialize` request's id.
pub fn init_answer(v: &Value, init: &Id) -> Init {
    if !is_rpc(v) {
        let older = v.get("ok") == Some(&Value::Bool(false)) && v.get("error").and_then(Value::as_str).is_some_and(|e| e.contains("does not serve"));
        return if older { Init::NotRpc } else { Init::Other };
    }
    let Ok(Message::Response(r)) = Message::from_value(v.clone()) else { return Init::Other };
    if r.id.as_ref() != Some(init) {
        return Init::Other;
    }
    if let Some(e) = r.error {
        return if e.code == code::REFUSED { Init::Refused(e.message) } else { Init::NotRpc };
    }
    match r.result.and_then(|v| serde_json::from_value::<InitializeResult>(v).ok()) {
        Some(res) => Init::Ready(Box::new(res)),
        None => Init::NotRpc,
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
