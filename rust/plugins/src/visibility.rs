//! Is this view's call allowed (docs/mcp-apps.md, risk 1 and C5): the
//! pure rule the hub runs before any `view/call` reaches a server. A view
//! is third-party code: it may call only its own server's tools that are
//! visible to apps, never a model-only one, never one turned off, and the
//! Le Chat extras (`mistral/*`, connectors) are not supported (q-vibe).
//!
//! - [`judge`]: one call, the server's tools as listed (`tools/list` with
//!   `_meta.ui`, app-only ones included) and `plugins.json` as read ->
//!   [`Verdict`]: free (an app-only tool, a resource read), ask (a tool
//!   the model sees too: the agent's approval mode decides, q-calls), or
//!   refused with its [`ViewError`].
//!
//! "Its own server" is structural: the hub routes every call to the
//! view's server (`<plugin>/<server>` on the bridge), so a name that
//! server does not list (another server's tool) is simply unknown.

use bise_proto::view_rpc::ViewError;
use serde_json::Value;

use crate::id::{PartKind, PluginId};
use crate::model::StateV2;

/// The view that asks: its plugin and server (the bridge's route).
#[derive(Clone, Copy, Debug)]
pub struct ViewOf<'a> {
    pub plugin: &'a str,
    pub server: &'a str,
    /// the plugin is opt-in (`"dev.bise": {"default": "off"}`)
    pub opt_in: bool,
}

/// What the view asks, as its JSON-RPC request says it.
#[derive(Clone, Copy, Debug)]
pub enum Ask<'a> {
    /// `tools/call {name}`
    Tool(&'a str),
    /// `resources/read {uri}`
    Resource(&'a str),
    /// any other method
    Other(&'a str),
}

impl<'a> Ask<'a> {
    /// A view's `method` and `params` (C5's `view/call`).
    pub fn of(method: &'a str, params: &'a Value) -> Ask<'a> {
        let s = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
        match method {
            "tools/call" => Ask::Tool(s("name")),
            "resources/read" => Ask::Resource(s("uri")),
            m => Ask::Other(m),
        }
    }

    /// What it names, for the error's words.
    pub fn what(&self) -> &'a str {
        match self {
            Ask::Tool(n) => n,
            Ask::Resource(u) => u,
            Ask::Other(m) => m,
        }
    }
}

/// The rule's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// runs with no card: an app-only tool, a resource of its server
    Free,
    /// a tool the model sees too: through the agent's approval mode
    Ask,
    Refused(ViewError),
}

/// A tool's `_meta.ui.visibility`: (model, app). Absent: both (the
/// spec's default); a list without either word: neither.
pub fn visibility(tool: &Value) -> (bool, bool) {
    match tool.pointer("/_meta/ui/visibility").and_then(Value::as_array) {
        None => (true, true),
        Some(l) => {
            let has = |w: &str| l.iter().any(|v| v.as_str() == Some(w));
            (has("model"), has("app"))
        }
    }
}

/// A name of Le Chat's extras: `mistral/*`, or a connector's
/// (`mistral:<connector>…`).
fn le_chat_extra(name: &str) -> bool {
    name.starts_with("mistral/") || name.starts_with("mistral:")
}

/// Whether `ask` from `view` may run, given its server's `tools` as
/// listed and `state`. The order: not supported (never, whatever is
/// on), then off (the plugin, its server, the tool), then visibility.
pub fn judge(view: ViewOf, ask: Ask, tools: &[Value], state: &StateV2) -> Verdict {
    let refused = Verdict::Refused;
    let name = match ask {
        Ask::Other(_) => return refused(ViewError::NotSupported),
        Ask::Tool(n) if le_chat_extra(n) => return refused(ViewError::NotSupported),
        Ask::Resource(u) if le_chat_extra(u) => return refused(ViewError::NotSupported),
        Ask::Tool(n) => Some(n),
        Ask::Resource(_) => None,
    };
    let owner = PluginId::plugin(view.plugin);
    let server = owner.part(PartKind::Mcp, None, view.server);
    if !state.is_on(&server, view.opt_in) {
        return refused(ViewError::PluginOff);
    }
    let Some(name) = name else { return Verdict::Free };
    if name.is_empty() {
        return refused(ViewError::NotVisible);
    }
    let Some(tool) = tools.iter().find(|t| t.get("name").and_then(Value::as_str) == Some(name)) else {
        return refused(ViewError::NotVisible);
    };
    if !state.is_on(&owner.part(PartKind::Tool, Some(view.server), name), view.opt_in) {
        return refused(ViewError::PluginOff);
    }
    match visibility(tool) {
        (_, false) => refused(ViewError::NotVisible),
        (false, true) => Verdict::Free,
        (true, true) => Verdict::Ask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const VIEW: ViewOf = ViewOf { plugin: "fake-app", server: "fake-app", opt_in: false };

    /// fake-app's tools/list as a UI client gets it (the fixture's shapes)
    fn tools() -> Vec<Value> {
        vec![
            json!({"name": "show_counter", "_meta": {"ui": {"resourceUri": "ui://fake-app/counter"}}}),
            json!({"name": "bump", "_meta": {"ui": {"visibility": ["app"]}}}),
            json!({"name": "whisper", "_meta": {"ui": {"visibility": ["model"]}}}),
            json!({"name": "slow"}),
            json!({"name": "both", "_meta": {"ui": {"visibility": ["model", "app"]}}}),
        ]
    }

    fn call(name: &str, state: &StateV2) -> Verdict {
        judge(VIEW, Ask::Tool(name), &tools(), state)
    }

    fn off(parts: &[&str], plugins: &[&str]) -> StateV2 {
        StateV2 {
            disabled: plugins.iter().map(|s| s.to_string()).collect(),
            parts_off: parts.iter().map(|s| PluginId::parse(s).unwrap()).collect(),
            ..StateV2::default()
        }
    }

    #[test]
    fn an_app_only_tool_is_free_and_a_shared_one_asks() {
        let s = StateV2::default();
        assert_eq!(call("bump", &s), Verdict::Free);
        assert_eq!(call("show_counter", &s), Verdict::Ask);
        assert_eq!(call("slow", &s), Verdict::Ask, "no visibility: model and app (the spec's default)");
        assert_eq!(call("both", &s), Verdict::Ask);
    }

    #[test]
    fn a_model_only_tool_another_servers_or_an_unknown_one_is_not_visible() {
        let s = StateV2::default();
        assert_eq!(call("whisper", &s), Verdict::Refused(ViewError::NotVisible));
        // another server's tool: the view's server does not list it
        assert_eq!(call("create_issue", &s), Verdict::Refused(ViewError::NotVisible));
        assert_eq!(call("", &s), Verdict::Refused(ViewError::NotVisible));
        let none = json!({"name": "x", "_meta": {"ui": {"visibility": []}}});
        assert_eq!(judge(VIEW, Ask::Tool("x"), &[none], &s), Verdict::Refused(ViewError::NotVisible));
    }

    #[test]
    fn the_plugin_its_server_or_the_tool_off_refuses() {
        assert_eq!(call("bump", &off(&[], &["fake-app"])), Verdict::Refused(ViewError::PluginOff));
        assert_eq!(call("bump", &off(&["fake-app#mcp:fake-app"], &[])), Verdict::Refused(ViewError::PluginOff));
        assert_eq!(call("bump", &off(&["fake-app#tool:fake-app/bump"], &[])), Verdict::Refused(ViewError::PluginOff));
        assert_eq!(call("show_counter", &off(&["fake-app#tool:fake-app/bump"], &[])), Verdict::Ask, "only that tool");
        let r = judge(VIEW, Ask::Resource("ui://fake-app/counter"), &tools(), &off(&[], &["fake-app"]));
        assert_eq!(r, Verdict::Refused(ViewError::PluginOff));
        let opt_in = ViewOf { opt_in: true, ..VIEW };
        assert_eq!(judge(opt_in, Ask::Tool("bump"), &tools(), &StateV2::default()), Verdict::Refused(ViewError::PluginOff));
    }

    #[test]
    fn le_chat_extras_and_other_methods_are_not_supported_even_when_off() {
        for s in [StateV2::default(), off(&[], &["fake-app"])] {
            assert_eq!(call("mistral/complete", &s), Verdict::Refused(ViewError::NotSupported));
            assert_eq!(call("mistral:gmail/send_email", &s), Verdict::Refused(ViewError::NotSupported));
            let other = judge(VIEW, Ask::of("sampling/createMessage", &json!({})), &tools(), &s);
            assert_eq!(other, Verdict::Refused(ViewError::NotSupported));
        }
    }

    #[test]
    fn a_resource_of_its_server_is_free() {
        let p = json!({"uri": "ui://fake-app/counter"});
        assert_eq!(judge(VIEW, Ask::of("resources/read", &p), &tools(), &StateV2::default()), Verdict::Free);
    }

    #[test]
    fn ask_reads_the_request() {
        let p = json!({"name": "bump", "arguments": {"by": 1}});
        assert!(matches!(Ask::of("tools/call", &p), Ask::Tool("bump")));
        assert_eq!(Ask::of("tools/call", &p).what(), "bump");
    }
}
