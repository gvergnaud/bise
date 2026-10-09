//! JSON-RPC 2.0's envelope, one JSON object per line (client-protocol
//! option 1, architect m_13089; modeled on Vibe's app_server, ADR 0009),
//! shared by every JSON-RPC wire bise has: `hub.sock` ([`crate::rpc`]'s
//! tables) and the desktop app <-> its core on stdio
//! ([`crate::app_rpc`]'s tables).
//!
//! - a client sends [`Request`]s (`id`, `method`, `params`) and gets one
//!   [`Response`] each (`result` or `error`), written before the
//!   notifications its action causes;
//! - the server sends [`Notification`]s (`method`, `params`);
//! - `initialize` first, then `initialized`; nothing else before.
//!
//! Pure; the same bytes on every branch that has it.

use crate::hub::ErrorKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
