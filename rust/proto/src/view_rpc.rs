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

/// How long a view's approval card waits for his answer (C5): the one
/// deadline of a view's call (mcp-apps review d3). sb-core decides with
/// it (generated into `bend/hub/gen/kit.bend` as `K.VIEW_CARD_MS`, never
/// typed again); the bounds below derive from it.
pub const VIEW_CARD_MS: u64 = 600_000;
/// One request to a view's server through a bridge route (the bridge's
/// own timeout is its mcp.json's).
pub const VIEW_SERVER_MS: u64 = 300_000;
/// How long the hub waits for a bridge's address (a views bridge that
/// just started lists its servers first).
pub const VIEW_BRIDGE_MS: u64 = 15_000;
/// The longest the hub can take to answer a `view/call`, and so how long
/// a client holds one (the web host's `/api/view`, the ambient core):
/// its bridge found and its tools listed, twice at most (its agent's
/// bridge gone: once more on the views bridge), its card's whole wait,
/// then the call. The hub always answers before it (its card's due, the
/// server's timeout), so a client never gives up on a card still open.
pub const VIEW_CALL_MAX_MS: u64 = 2 * (VIEW_BRIDGE_MS + VIEW_SERVER_MS) + VIEW_CARD_MS + VIEW_SERVER_MS;
// it outlasts the card and its call, whatever the numbers become
const _: () = assert!(VIEW_CALL_MAX_MS >= VIEW_CARD_MS + VIEW_SERVER_MS);

/// `view/resource`'s answer (C5): the view's HTML and what its sandbox
/// takes, with its call's input and result for `ui/notifications/tool-*`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ViewResource {
    /// `http://v-<16 hex>.localhost:<pages port>` (C6)
    pub origin: String,
    /// the sandbox proxy's URL for this load, the one the host frames:
    /// `<origin>/mcp-app?s=<32 hex>`, a session the hub made for this
    /// `view/resource` (C6: a page that never asked the hub, a `.html`
    /// file opened in his browser, can't frame the view's origin)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub proxy: Option<String>,
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
    /// a `ui/update-model-context` over the hub's 16 KB (review d5); a
    /// whole `resources/read` past [`MAX_READ`] (read it by ranges)
    TooLarge,
    /// a `view-file://` handle the hub refuses (docs/plugin-apps.md A4):
    /// made up, another session's, another artifact's, a path. One answer
    /// whatever the reason: nothing tells a view what exists
    BadHandle,
    /// a write without the view's `write` grant, or on an older version
    /// (its [`NoWrite`] in `data.reason`, [`ViewError::no_write`])
    NoWrite,
    /// the handle's file or version is no more (deleted, moved, renamed)
    Gone,
}

/// Why a view may not write its file: `data.reason` of a
/// [`ViewError::NoWrite`] (architect m_59962 #3: the host says "there's a
/// newer version" on the second, never "not allowed").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum NoWrite {
    NoGrant,
    OlderVersion,
}

impl NoWrite {
    pub fn reason(self) -> &'static str {
        match self {
            NoWrite::NoGrant => "no_grant",
            NoWrite::OlderVersion => "older_version",
        }
    }
}

/// The most bytes one `resources/read` answers (A4): a bigger file is read
/// by ranges.
pub const MAX_READ: u64 = 24 << 20;
/// The most bytes one `ai.mistral.vibe/resources/write` takes.
pub const MAX_WRITE: u64 = 24 << 20;

/// A `resources/read`'s `_meta` (A4): what a view needs to save it back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReadMeta {
    /// what a write sends back as `ifMatch`: an artifact's `<v>.<rev>`, a
    /// root file's content hash
    pub etag: String,
    /// its `write` grant, on the newest version
    pub writable: bool,
    /// the whole file's bytes (a range reads part of it)
    pub size: u64,
}

/// An `ai.mistral.vibe/resources/write`'s answer (A4). A refusal (no
/// grant, an older version, a bad handle) is an error, never an outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum WriteOutcome {
    /// written; `etag` the next write's `ifMatch`
    Saved { etag: String },
    /// it changed since `ifMatch` (another version, or on disk): `etag`
    /// what it is now
    Conflict { etag: String },
    /// over [`MAX_WRITE`]
    TooLarge {
        #[serde(rename = "maxBytes")]
        max_bytes: u64,
    },
}

impl ViewError {
    pub const ALL: [ViewError; 11] = [
        ViewError::Expired,
        ViewError::NotVisible,
        ViewError::PluginOff,
        ViewError::Denied,
        ViewError::NotSupported,
        ViewError::RateLimited,
        ViewError::NoGesture,
        ViewError::TooLarge,
        ViewError::BadHandle,
        ViewError::NoWrite,
        ViewError::Gone,
    ];

    /// Its JSON-RPC code, -32030..-32035, -32037..-32041 (-32036 was
    /// `server-restarted`, now a result's `_meta`: never reuse it).
    pub fn code(self) -> i64 {
        match self {
            ViewError::Expired => -32030,
            ViewError::NotVisible => -32031,
            ViewError::PluginOff => -32032,
            ViewError::Denied => -32033,
            ViewError::NotSupported => -32034,
            ViewError::RateLimited => -32035,
            ViewError::NoGesture => -32037,
            ViewError::TooLarge => -32038,
            ViewError::BadHandle => -32039,
            ViewError::NoWrite => -32040,
            ViewError::Gone => -32041,
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
            ViewError::TooLarge => "too-large",
            ViewError::BadHandle => "bad-handle",
            ViewError::NoWrite => "no-write",
            ViewError::Gone => "gone",
        }
    }

    /// The error a `data.reason` names (a [`NoWrite`]'s: `NoWrite`).
    pub fn from_reason(reason: &str) -> Option<ViewError> {
        if [NoWrite::NoGrant, NoWrite::OlderVersion].iter().any(|w| w.reason() == reason) {
            return Some(ViewError::NoWrite);
        }
        ViewError::ALL.into_iter().find(|e| e.reason() == reason)
    }

    /// A refused write, `data.reason` its [`NoWrite`].
    pub fn no_write(why: NoWrite, what: &str) -> RpcError {
        let mut e = ViewError::NoWrite.rpc(what);
        if let Some(d) = e.data.as_mut() {
            d.reason = Some(why.reason().into());
        }
        if why == NoWrite::OlderVersion {
            e.message = format!("{what}: this is an older version; open the newest to save");
        }
        e
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
            ViewError::TooLarge => format!("{what}: too big for bise"),
            ViewError::BadHandle => format!("{what}: not a file this view was opened on"),
            ViewError::NoWrite => format!("{what}: this view may not save its file"),
            ViewError::Gone => format!("{what}: the file is gone"),
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
                (-32038, "too-large"),
                (-32039, "bad-handle"),
                (-32040, "no-write"),
                (-32041, "gone"),
            ]
        );
        for e in ViewError::ALL {
            assert_eq!(ViewError::from_reason(e.reason()), Some(e));
        }
    }

    /// architect m_59962 #3: a refused write says why, as an enum.
    #[test]
    fn a_refused_write_says_no_grant_or_older_version() {
        for (why, reason) in [(NoWrite::NoGrant, "no_grant"), (NoWrite::OlderVersion, "older_version")] {
            let e = ViewError::no_write(why, "ai.mistral.vibe/resources/write");
            assert_eq!(e.code, -32040);
            assert_eq!(e.data.and_then(|d| d.reason).as_deref(), Some(reason));
            assert_eq!(ViewError::from_reason(reason), Some(ViewError::NoWrite));
            assert_eq!(serde_json::to_value(why).unwrap(), serde_json::json!(reason));
        }
    }

    #[test]
    fn a_write_answers_its_outcome() {
        use serde_json::json;
        let cases = [
            (WriteOutcome::Saved { etag: "3.2".into() }, json!({"outcome": "saved", "etag": "3.2"})),
            (WriteOutcome::Conflict { etag: "4.1".into() }, json!({"outcome": "conflict", "etag": "4.1"})),
            (WriteOutcome::TooLarge { max_bytes: MAX_WRITE }, json!({"outcome": "too-large", "maxBytes": MAX_WRITE})),
        ];
        for (o, want) in cases {
            assert_eq!(serde_json::to_value(&o).unwrap(), want);
            assert_eq!(serde_json::from_value::<WriteOutcome>(want).unwrap(), o);
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
