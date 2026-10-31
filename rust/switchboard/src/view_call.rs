//! The pure parts of the hub's view requests (docs/mcp-apps.md, C2, C5):
//! what a view request names, the views file the bridge wrote, the
//! bridge's address, a resource's HTML and declared sandbox, a view's
//! message text. The shell (`daemon/views.rs`) reads the files and the
//! network; this module decides what they say.

use bise_proto::view_rpc::ViewError;
use bise_proto::view_rules;
use serde::Deserialize;
use serde_json::{json, Value};

/// A view request's `agent` and `call` (every C5 method).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewParams {
    pub agent: String,
    pub call: String,
}

impl ViewParams {
    /// From a request's params; a bad call id is no file: expired.
    pub fn of(params: &Value) -> Result<ViewParams, ViewError> {
        let s = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let (agent, call) = (s("agent"), s("call"));
        if agent.is_empty() || !view_rules::call_ok(&call) {
            return Err(ViewError::Expired);
        }
        Ok(ViewParams { agent, call })
    }
}

/// The views file of a call (C2), as the bridge wrote it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ViewFile {
    #[serde(default)]
    pub plugin: String,
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub result: Value,
}

impl ViewFile {
    /// Its text read; unreadable or naming no view: expired.
    pub fn parse(text: &str) -> Result<ViewFile, ViewError> {
        let f: ViewFile = serde_json::from_str(text).map_err(|_| ViewError::Expired)?;
        if f.plugin.is_empty() || f.server.is_empty() || !f.uri.starts_with("ui://") {
            return Err(ViewError::Expired);
        }
        Ok(f)
    }
}

/// `D/bridge.json`'s base (`http://127.0.0.1:<port>/<token>`): a server's
/// route is `<base>/<plugin>/<server>` (C1).
pub fn bridge_base(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let base = v.get("base")?.as_str()?;
    base.starts_with("http://127.0.0.1:").then(|| base.trim_end_matches('/').to_string())
}

/// A route's URL: the bridge's base, the plugin, the server.
pub fn route_url(base: &str, plugin: &str, server: &str) -> String {
    format!("{base}/{plugin}/{server}")
}

/// The JSON-RPC request the hub sends a view's server.
pub fn request(method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
}

/// A server's JSON-RPC answer: its `result`, or its `error` as it gave it.
pub fn answer(v: &Value) -> Result<Value, Value> {
    match (v.get("result"), v.get("error")) {
        (_, Some(e)) => Err(e.clone()),
        (Some(r), None) => Ok(r.clone()),
        _ => Err(json!({"code": -32603, "message": "the server's answer had no result"})),
    }
}

/// The `_meta` key a `view/call` result carries when it ran on a server
/// that is not the one before (a new bridge, the hub's views bridge):
/// the call ran, but what that server kept in memory is gone. Said once
/// per view; the result itself is never dropped (the tool did run, maybe
/// after his ok: a view that ran it again would run it twice).
pub const RESTARTED_META: &str = "dev.bise/server-restarted";

/// `result` as the view gets it: [`RESTARTED_META`] in its `_meta` when
/// `restarted`, and never otherwise. The hub alone says it: a server's
/// own key is overwritten either way (it can't fake nor hide a restart).
/// A result that is not an object stays as it is.
pub fn mark_restarted(mut result: Value, restarted: bool) -> Value {
    let Some(o) = result.as_object_mut() else { return result };
    match o.get_mut("_meta").and_then(Value::as_object_mut) {
        Some(meta) if restarted => {
            meta.insert(RESTARTED_META.into(), Value::Bool(true));
        }
        Some(meta) => {
            meta.remove(RESTARTED_META);
        }
        None if restarted => {
            o.insert("_meta".into(), json!({ RESTARTED_META: true }));
        }
        None => {}
    }
    result
}

/// What a view resource declares and holds (`resources/read`'s first
/// content): its HTML (text, or base64 `blob`), its `_meta.ui`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResourceRead {
    pub html: String,
    pub csp: Value,
    pub declared: Vec<String>,
    pub prefers_border: bool,
}

/// The first content of a `resources/read` result.
pub fn resource_read(result: &Value) -> Option<ResourceRead> {
    let c = result.get("contents")?.as_array()?.first()?;
    let html = match (c.get("text").and_then(Value::as_str), c.get("blob").and_then(Value::as_str)) {
        (Some(t), _) => t.to_string(),
        (None, Some(b)) => String::from_utf8(base64_decode(b)?).ok()?,
        _ => return None,
    };
    let meta = c.get("_meta");
    let ui = meta.and_then(|m| m.get("ui"));
    Some(ResourceRead {
        html,
        csp: ui.and_then(|u| u.get("csp")).cloned().filter(Value::is_object).unwrap_or_else(|| json!({})),
        declared: view_rules::names(&view_rules::declared(meta.unwrap_or(&Value::Null))),
        prefers_border: ui.and_then(|u| u.get("prefersBorder")).and_then(Value::as_bool).unwrap_or(false),
    })
}

/// What a view may use: what it declared and he granted.
pub fn granted(declared: &[String], grant: &[String]) -> Vec<String> {
    declared.iter().filter(|p| grant.contains(p)).cloned().collect()
}

/// The tool named `name` in a `tools/list` result.
pub fn tool_of(list: &Value, name: &str) -> Value {
    list.get("tools").and_then(Value::as_array).and_then(|t| t.iter().find(|t| t.get("name").and_then(Value::as_str) == Some(name))).cloned().unwrap_or(Value::Null)
}

/// A `tools/list` result's tools.
pub fn tools(list: &Value) -> Vec<Value> {
    list.get("tools").and_then(Value::as_array).cloned().unwrap_or_default()
}

/// The text of a view's `ui/message` or `ui/update-model-context`: its
/// text blocks, in order (the others are not supported yet: none, an
/// error).
pub fn text_of(content: &Value) -> Result<String, ViewError> {
    let blocks = content.as_array().ok_or(ViewError::NotSupported)?;
    let texts: Vec<&str> = blocks.iter().filter(|b| b.get("type").and_then(Value::as_str) == Some("text")).filter_map(|b| b.get("text").and_then(Value::as_str)).collect();
    if texts.is_empty() || texts.len() != blocks.len() {
        return Err(ViewError::NotSupported);
    }
    Ok(texts.join("\n\n"))
}

/// The card's words for a view's call he must allow.
pub fn card_text(plugin: &str, tool: &str, args: &Value) -> String {
    let mut out = vec![format!("{plugin} view wants to run {tool}")];
    let body = serde_json::to_string_pretty(args).unwrap_or_default();
    out.extend(body.lines().take(40).map(|l| format!("| {l}")));
    out.push("reason: a view called a tool the model can call too".into());
    out.join("\n")
}

/// What a view's card asks, typed on it (sb-core's `Card.view.ask`): the
/// tool and its arguments as canonical JSON (keys sorted), so the same
/// call asked twice is the same ask.
pub fn ask(tool: &str, args: &Value) -> String {
    serde_json::json!({ "tool": tool, "args": args }).to_string()
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    use base64_lite::decode;
    decode(s)
}

/// A small standard base64 decoder (no dependency for one blob field).
mod base64_lite {
    pub fn decode(s: &str) -> Option<Vec<u8>> {
        let val = |c: u8| -> Option<u32> {
            Some(match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' | b'-' => 62,
                b'/' | b'_' => 63,
                _ => return None,
            } as u32)
        };
        let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=').collect();
        let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            let mut n = 0u32;
            for (i, c) in chunk.iter().enumerate() {
                n |= val(*c)? << (18 - 6 * i);
            }
            let k = chunk.len();
            if k < 2 {
                return None;
            }
            out.push((n >> 16) as u8);
            if k > 2 {
                out.push((n >> 8) as u8);
            }
            if k > 3 {
                out.push(n as u8);
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_call_id_names_no_file() {
        assert_eq!(ViewParams::of(&json!({"agent": "docs", "call": "../../x"})), Err(ViewError::Expired));
        assert_eq!(ViewParams::of(&json!({"agent": "docs", "call": "1-2"})), Ok(ViewParams { agent: "docs".into(), call: "1-2".into() }));
    }

    #[test]
    fn the_views_file_names_its_view_or_it_is_expired() {
        let f = ViewFile::parse(r#"{"v":1,"plugin":"fake-app","server":"fake-app","uri":"ui://fake-app/counter","tool":"show_counter","args":{"start":3},"result":{"structuredContent":{"count":3}},"at_ms":1}"#).unwrap();
        assert_eq!((f.plugin.as_str(), f.tool.as_str(), f.args["start"].as_i64()), ("fake-app", "show_counter", Some(3)));
        assert_eq!(ViewFile::parse("{}"), Err(ViewError::Expired));
        assert_eq!(ViewFile::parse("not json"), Err(ViewError::Expired));
        assert_eq!(ViewFile::parse(r#"{"plugin":"p","server":"s","uri":"https://x"}"#), Err(ViewError::Expired));
    }

    #[test]
    fn the_bridge_base_is_loopback_only() {
        assert_eq!(bridge_base(r#"{"v":1,"base":"http://127.0.0.1:4100/tok/","pid":9}"#).as_deref(), Some("http://127.0.0.1:4100/tok"));
        assert_eq!(bridge_base(r#"{"base":"http://evil.example/tok"}"#), None);
        assert_eq!(route_url("http://127.0.0.1:4100/tok", "fake-app", "~views"), "http://127.0.0.1:4100/tok/fake-app/~views");
    }

    #[test]
    fn a_resource_says_its_html_csp_and_declared_permissions() {
        let r = json!({"contents": [{"uri": "ui://x", "mimeType": "text/html;profile=mcp-app", "text": "<p>hi</p>",
            "_meta": {"ui": {"csp": {"connectDomains": ["https://api.open-meteo.com"]}, "permissions": {"microphone": {}, "geolocation": {}}, "prefersBorder": true},
                      "dev.bise/ui": {"permissions": {"midi": {}}}}}]});
        let got = resource_read(&r).unwrap();
        assert_eq!(got.html, "<p>hi</p>");
        assert_eq!(got.csp["connectDomains"][0], "https://api.open-meteo.com");
        assert_eq!(got.declared, vec!["microphone", "midi"], "geolocation is not one bise grants");
        assert!(got.prefers_border);
        let blob = json!({"contents": [{"uri": "ui://x", "blob": "PHA+aGk8L3A+"}]});
        assert_eq!(resource_read(&blob).unwrap().html, "<p>hi</p>");
        assert_eq!(resource_read(&json!({"contents": []})), None);
        assert_eq!(granted(&["microphone".into(), "midi".into()], &["midi".into()]), vec!["midi"]);
    }

    /// The hub reads a resource's declared permissions as the shared
    /// fixture says (bise_proto::view_rules::declared, mcp-apps review d1).
    #[test]
    fn a_resource_declares_as_the_shared_fixture_says() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/mcp-apps/declared.json");
        let fx: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for row in fx["rows"].as_array().unwrap() {
            let r = json!({"contents": [{"uri": "ui://x", "text": "", "_meta": row["meta"]}]});
            let want: Vec<String> = serde_json::from_value(row["declared"].clone()).unwrap();
            assert_eq!(resource_read(&r).unwrap().declared, want, "{}", row["meta"]);
        }
    }

    #[test]
    fn a_message_is_its_text_blocks_only() {
        assert_eq!(text_of(&json!([{"type": "text", "text": "make it major"}])), Ok("make it major".into()));
        assert_eq!(text_of(&json!([{"type": "image", "data": "x"}])), Err(ViewError::NotSupported));
        assert_eq!(text_of(&json!([])), Err(ViewError::NotSupported));
        assert_eq!(text_of(&json!("hi")), Err(ViewError::NotSupported));
    }

    #[test]
    fn a_result_after_a_restart_is_kept_and_says_so() {
        let r = json!({"content": [{"type": "text", "text": "created #12"}], "_meta": {"x": 1}});
        let got = mark_restarted(r.clone(), true);
        assert_eq!(got["content"], r["content"], "the result is never dropped");
        assert_eq!(got["_meta"]["x"], 1);
        assert_eq!(got["_meta"][RESTARTED_META], true);
        assert_eq!(mark_restarted(json!({}), true)["_meta"][RESTARTED_META], true);
        assert_eq!(mark_restarted(r.clone(), false), r, "the same server: as it gave it");
        assert_eq!(mark_restarted(json!("x"), true), json!("x"));
    }

    #[test]
    fn a_server_can_neither_fake_nor_hide_a_restart() {
        let fake = json!({"content": [], "_meta": {RESTARTED_META: true, "x": 1}});
        assert_eq!(mark_restarted(fake, false), json!({"content": [], "_meta": {"x": 1}}));
        let hide = json!({"content": [], "_meta": {RESTARTED_META: false}});
        assert_eq!(mark_restarted(hide, true)["_meta"][RESTARTED_META], true);
    }

    #[test]
    fn a_servers_answer_is_its_result_or_its_error() {
        assert_eq!(answer(&json!({"id": 1, "result": {"a": 1}})), Ok(json!({"a": 1})));
        assert_eq!(answer(&json!({"id": 1, "error": {"code": -32602, "message": "no"}})), Err(json!({"code": -32602, "message": "no"})));
    }
}
