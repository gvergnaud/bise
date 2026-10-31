//! The views' client methods on `hub.sock` (docs/mcp-apps.md, C5): their
//! errors, one owner of each code, reason and the words a view gets.
//!
//! `view/resource`, `view/call`, `view/message`, `view/context` are
//! `HubCmd`s (`hub::HubCmd::View*`, rows in `rpc::METHODS`); a refusal
//! is a JSON-RPC error whose `data.reason` is [`ViewError::reason`], so
//! the window draws it by reason, never by its words.

use crate::jsonrpc::{ErrorData, RpcError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `view/resource`'s answer (C5): the view's HTML and what its sandbox
/// takes, with its call's input and result for `ui/notifications/tool-*`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ViewResource {
    /// `http://v-<16 hex>.localhost:<pages port>` (C6)
    pub origin: String,
    pub html: String,
    /// the resource's `_meta.ui.csp` as it declared it (the pages server
    /// makes the header from it)
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub csp: Value,
    /// what it may use: declared and granted at the plugin's add (C8)
    pub permissions: Vec<String>,
    /// what it declared (`_meta.ui.permissions`, MIDI from
    /// `_meta["dev.bise/ui"].permissions`): the card says what runs
    /// without its grant
    pub declared: Vec<String>,
    #[serde(rename = "prefersBorder")]
    pub prefers_border: bool,
    /// the call's arguments; none: no views file kept them
    #[serde(rename = "toolInput")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub tool_input: Value,
    /// the call's whole result (`{"truncated": true}` past 4 MiB)
    #[serde(rename = "toolResult")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub tool_result: Value,
    /// the tool as its server lists it (`toolInfo`)
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub tool: Value,
}

/// `view/call`'s answer: its server's result as it gave it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct ViewCallResult(#[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))] pub Value);

/// `plugin/views`' answer (C8): the bridge's `~ui` answer as it gave it,
/// `{views: [{server, tool, uri, csp, permissions, bise_permissions,
/// prefersBorder, ready, error?}]}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct PluginViewsResult(#[cfg_attr(feature = "ts", ts(type = "{ views: Array<Record<string, unknown>> }"))] pub Value);

/// Why the hub refused a view's request (C5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewError {
    /// the call's views file is gone (pruned, or never written)
    Expired,
    /// a model-only tool, a tool of another server, an unknown tool
    NotVisible,
    /// the plugin, its server or the tool is turned off
    PluginOff,
    /// he refused it in the approval card (or let it lapse)
    Denied,
    /// `mistral/*`, a connector's tool, a method bise does not serve to
    /// views (decision q-vibe)
    NotSupported,
    /// past the view's limits (30 calls a minute, 1 message every 10 s)
    RateLimited,
    /// a `ui/message` without his fresh click in the view (risk 1; the
    /// web host never has one: no Electron main sees his clicks)
    NoGesture,
}

impl ViewError {
    pub const ALL: [ViewError; 7] = [
        ViewError::Expired,
        ViewError::NotVisible,
        ViewError::PluginOff,
        ViewError::Denied,
        ViewError::NotSupported,
        ViewError::RateLimited,
        ViewError::NoGesture,
    ];

    /// Its JSON-RPC code, -32030..-32035 and -32037 (-32036 was `server-restarted`, now a
    /// result's `_meta`: never reuse it).
    pub fn code(self) -> i64 {
        match self {
            ViewError::Expired => -32030,
            ViewError::NotVisible => -32031,
            ViewError::PluginOff => -32032,
            ViewError::Denied => -32033,
            ViewError::NotSupported => -32034,
            ViewError::RateLimited => -32035,
            ViewError::NoGesture => -32037,
        }
    }

    /// Its `data.reason`.
    pub fn reason(self) -> &'static str {
        match self {
            ViewError::Expired => "expired",
            ViewError::NotVisible => "not-visible",
            ViewError::PluginOff => "plugin-off",
            ViewError::Denied => "denied",
            ViewError::NotSupported => "not-supported",
            ViewError::RateLimited => "rate-limited",
            ViewError::NoGesture => "no-gesture",
        }
    }

    pub fn from_reason(reason: &str) -> Option<ViewError> {
        ViewError::ALL.into_iter().find(|e| e.reason() == reason)
    }

    /// The words the view (and its developer) reads; `what` names the
    /// tool or method asked.
    pub fn words(self, what: &str) -> String {
        match self {
            ViewError::Expired => "this view's result is gone: run the tool again".into(),
            ViewError::NotVisible => format!("{what} is not a tool this view can call (only its own server's tools that are visible to apps)"),
            ViewError::PluginOff => format!("{what} is turned off in plugins"),
            ViewError::Denied => format!("{what} was not allowed"),
            ViewError::NotSupported => format!("{what} is not supported in bise"),
            ViewError::RateLimited => format!("{what}: too many requests from this view"),
            ViewError::NoGesture => format!("{what} needs a click in the view first (and a browser tab never has one)"),
        }
    }

    /// The JSON-RPC error a view's request gets.
    pub fn rpc(self, what: &str) -> RpcError {
        let data = ErrorData { kind: None, reason: Some(self.reason().into()), field: None };
        RpcError { code: self.code(), message: self.words(what), data: Some(data) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_reasons_are_the_contracts() {
        let got: Vec<(i64, &str)> = ViewError::ALL.iter().map(|e| (e.code(), e.reason())).collect();
        assert_eq!(
            got,
            vec![
                (-32030, "expired"),
                (-32031, "not-visible"),
                (-32032, "plugin-off"),
                (-32033, "denied"),
                (-32034, "not-supported"),
                (-32035, "rate-limited"),
                (-32037, "no-gesture"),
            ]
        );
        for e in ViewError::ALL {
            assert_eq!(ViewError::from_reason(e.reason()), Some(e));
        }
    }

    #[test]
    fn the_error_carries_its_reason_and_says_not_supported_in_bise() {
        let e = ViewError::NotSupported.rpc("mistral/complete");
        assert_eq!(e.code, -32034);
        assert_eq!(e.data.and_then(|d| d.reason).as_deref(), Some("not-supported"));
        assert!(e.message.contains("not supported in bise"));
    }
}
