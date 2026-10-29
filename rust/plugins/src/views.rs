//! MCP Apps views on the bridge's side (docs/mcp-apps.md, piece 1, C1):
//!
//! - the UI extension bise announces at `initialize` ([`client_capabilities`]):
//!   a server lists `_meta.ui` only to a client that announces it;
//! - a tool's `_meta.ui` read once ([`tool_ui`]): its view's `ui://` URI
//!   and its visibility (an `app`-only tool is never in the index);
//! - the index line's last field ` | ui: <uri>` ([`ui_field`]);
//! - a plugin's Vibe static views (`ai.mistral.vibe/views/<name>/`:
//!   `view.json` + `index.html`, no server), loaded by [`load`] and
//!   served by [`Virtual`], the plugin's virtual server [`SERVER`]
//!   (`<base>/<plugin>/~views`): each view is a tool that answers its
//!   arguments as `structuredContent`, its HTML a `ui://` resource;
//! - the `ui://` resource cache of one server ([`ResourceCache`]): a
//!   view's HTML read once per server start.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use crate::resolve::{identifier, valid_name, Diagnostic, Severity};

/// The MCP Apps extension's id (SEP-1865).
pub const EXT: &str = "io.modelcontextprotocol/ui";
/// The one view MIME type the spec defines.
pub const MIME: &str = "text/html;profile=mcp-app";
/// The id of a plugin's virtual server of static views (its route:
/// `<base>/<plugin>/~views`). No mcp.json server may start with `~`.
pub const SERVER: &str = "~views";
/// Vibe's manifest extension (its folder holds the views).
pub const VIBE_EXTENSION: &str = "ai.mistral.vibe";
/// At most this many static views per plugin (more: a warning, the
/// rest dropped).
pub const MAX_VIEWS: usize = 100;
/// A `view.json` bigger than this is refused (Vibe's registry cap).
pub const MAX_VIEW_JSON: u64 = 64 * 1024;
/// A view's HTML bigger than this is refused, and a resource answer
/// bigger than this is never cached.
pub const MAX_HTML: u64 = 4 * 1024 * 1024;
/// At most this many `ui://` resources cached per server.
pub const MAX_CACHED: usize = 32;

/// The `capabilities` of our `initialize`: the UI extension with its
/// one MIME type.
pub fn client_capabilities() -> Value {
    json!({"extensions": {EXT: {"mimeTypes": [MIME]}}})
}

/// A tool's `_meta.ui`, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUi {
    /// its view (`_meta.ui.resourceUri`, or the older flat
    /// `_meta["ui/resourceUri"]`): a `ui://` URI only
    pub uri: Option<String>,
    /// the model may see and call it (in the index)
    pub model: bool,
    /// a view of its server may call it
    pub app: bool,
}

/// Read `tool`'s `_meta.ui`. No `visibility` (or not a list): both, the
/// spec's default `["model", "app"]`.
pub fn tool_ui(tool: &Value) -> ToolUi {
    let meta = tool.get("_meta");
    let ui = meta.and_then(|m| m.get("ui"));
    let uri = ui
        .and_then(|u| u.get("resourceUri"))
        .or_else(|| meta.and_then(|m| m.get("ui/resourceUri")))
        .and_then(Value::as_str)
        .filter(|u| u.starts_with("ui://") && u.len() > 5)
        .map(String::from);
    let (model, app) = match ui.and_then(|u| u.get("visibility")).and_then(Value::as_array) {
        Some(v) => {
            let has = |w: &str| v.iter().any(|x| x.as_str() == Some(w));
            (has("model"), has("app"))
        }
        None => (true, true),
    };
    ToolUi { uri, model, app }
}

/// The index line's ` | ui: <uri>` field: a space, a control character,
/// `|` and `%` percent-encoded, so the field has no space and the line
/// stays one line (the views file keeps the server's own URI).
pub fn ui_field(uri: &str) -> String {
    let mut out = String::from(" | ui: ");
    for c in uri.chars() {
        if c.is_whitespace() || c.is_control() || c == '|' || c == '%' {
            let mut b = [0u8; 4];
            for x in c.encode_utf8(&mut b).bytes() {
                out.push_str(&format!("%{:02X}", x));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// One static view of a plugin (`ai.mistral.vibe/views/<name>/`).
#[derive(Clone, Debug, PartialEq)]
pub struct StaticView {
    /// its folder's name, the same as `view.json`'s `name`
    pub name: String,
    /// its tool's name: [`identifier`] of `name` (`-` -> `_`)
    pub tool: String,
    pub description: String,
    pub input_schema: Value,
    /// `ui://<plugin>/views/<name>`
    pub uri: String,
    /// its `index.html`
    pub html: PathBuf,
    /// `view.json`'s `ui` (`csp`, `permissions`, `prefersBorder`): the
    /// resource's `_meta.ui`
    pub ui: Value,
}

fn view_diag(plugin: &str, message: String) -> Diagnostic {
    Diagnostic { code: "plugin.view.invalid", severity: Severity::Warning, plugin: plugin.to_string(), message }
}

/// The static views of the plugin `plugin` at `root`, sorted by name,
/// and a warning for each one dropped. A folder outside `root` (a
/// symlink), a `view.json` or an `index.html` too big or missing, a
/// name that differs from its folder: dropped.
pub fn load(plugin: &str, root: &Path) -> (Vec<StaticView>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let dir = root.join(VIBE_EXTENSION).join("views");
    let Ok(rd) = std::fs::read_dir(&dir) else { return (Vec::new(), diags) };
    let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let croot = root.canonicalize().unwrap_or(root.to_path_buf());
    let mut views = Vec::new();
    for d in dirs {
        if views.len() >= MAX_VIEWS {
            diags.push(view_diag(plugin, format!("more than {} views; the rest are dropped", MAX_VIEWS)));
            break;
        }
        let folder = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match load_one(plugin, &croot, &d, &folder) {
            Ok(v) => views.push(v),
            Err(why) => diags.push(view_diag(plugin, format!("view {:?}: {}; dropped", folder, why))),
        }
    }
    (views, diags)
}

fn load_one(plugin: &str, root: &Path, dir: &Path, folder: &str) -> Result<StaticView, String> {
    let real = dir.canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(root) {
        return Err("its folder is outside the plugin".into());
    }
    let json_path = real.join("view.json");
    let html = real.join("index.html");
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).map_err(|_| format!("no {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    if size(&json_path)? > MAX_VIEW_JSON {
        return Err(format!("view.json is over {} KiB", MAX_VIEW_JSON / 1024));
    }
    if size(&html)? > MAX_HTML {
        return Err(format!("index.html is over {} MiB", MAX_HTML / 1024 / 1024));
    }
    let text = std::fs::read_to_string(&json_path).map_err(|e| format!("view.json: {}", e))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("view.json is not JSON: {}", e))?;
    let o = v.as_object().ok_or("view.json must be an object")?;
    let name = o.get("name").and_then(Value::as_str).ok_or("view.json needs a \"name\"")?;
    if name != folder {
        return Err(format!("its name {:?} must be its folder's", name));
    }
    if !valid_name(name) {
        return Err(format!("its name {:?} must be [a-z0-9.-], alphanumeric at both ends", name));
    }
    let description = match o.get("description") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err("\"description\" must be a string".into()),
    };
    let input_schema = match o.get("inputSchema") {
        None => json!({"type": "object"}),
        Some(s @ (Value::Object(_) | Value::Bool(_))) => s.clone(),
        Some(_) => return Err("\"inputSchema\" must be a JSON Schema object".into()),
    };
    let ui = match o.get("ui") {
        None => Value::Object(Map::new()),
        Some(u @ Value::Object(_)) => u.clone(),
        Some(_) => return Err("\"ui\" must be an object".into()),
    };
    Ok(StaticView {
        name: name.to_string(),
        tool: identifier(name),
        description,
        input_schema,
        uri: format!("ui://{}/views/{}", plugin, name),
        html,
        ui,
    })
}

/// A plugin's virtual server of static views: answers the JSON-RPC
/// methods a client of a real server would send.
#[derive(Clone, Debug, PartialEq)]
pub struct Virtual {
    pub plugin: String,
    pub namespace: String,
    pub opt_in: bool,
    pub views: Vec<StaticView>,
}

fn err(code: i64, message: String) -> Value {
    json!({"error": {"code": code, "message": message}})
}

impl Virtual {
    /// Its route under the bridge's base: `<plugin>/~views`.
    pub fn key(&self) -> String {
        format!("{}/{}", self.plugin, SERVER)
    }

    /// Its tools as a server lists them (`_meta.ui.resourceUri` on each).
    pub fn tools(&self) -> Vec<Value> {
        self.views
            .iter()
            .map(|v| json!({"name": v.tool, "description": v.description, "inputSchema": v.input_schema, "_meta": {"ui": {"resourceUri": v.uri}}}))
            .collect()
    }

    /// The JSON-RPC answer (no `id`): `{"result"}` or `{"error"}`.
    pub fn answer(&self, method: &str, params: &Value) -> Value {
        match method {
            "initialize" => json!({"result": {"protocolVersion": crate::stdio::PROTOCOL,
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": format!("{} views", self.plugin), "version": "1"}}}),
            "ping" => json!({"result": {}}),
            "tools/list" => json!({"result": {"tools": self.tools()}}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                match self.views.iter().find(|v| v.tool == name) {
                    Some(v) => {
                        let args = params.get("arguments").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                        json!({"result": {"content": [{"type": "text", "text": format!("the {} view is shown", v.name)}], "structuredContent": args}})
                    }
                    None => err(-32602, format!("unknown tool: {}", name)),
                }
            }
            "resources/list" => json!({"result": {"resources": self.views.iter().map(|v| json!({"uri": v.uri, "name": v.name, "mimeType": MIME})).collect::<Vec<_>>()}}),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                let Some(v) = self.views.iter().find(|v| v.uri == uri) else {
                    return err(-32602, format!("unknown resource: {}", uri));
                };
                match std::fs::read_to_string(&v.html) {
                    Ok(html) => json!({"result": {"contents": [{"uri": v.uri, "mimeType": MIME, "text": html, "_meta": {"ui": v.ui}}]}}),
                    Err(e) => err(-32603, format!("{}: {}", v.html.display(), e)),
                }
            }
            m => err(-32601, format!("method not found: {}", m)),
        }
    }
}

/// The `ui://` resources one server answered, by URI: a view's HTML is
/// read from its server once per start (cleared when it starts again
/// or says its tools changed). Only a `result` is kept, at most
/// [`MAX_CACHED`] of them, none over [`MAX_HTML`].
#[derive(Default)]
pub struct ResourceCache {
    map: Mutex<HashMap<String, Value>>,
}

/// The `ui://` URI a `resources/read` asks for.
pub fn ui_read(method: &str, params: &Value) -> Option<String> {
    let uri = params.get("uri").and_then(Value::as_str)?;
    (method == "resources/read" && uri.starts_with("ui://")).then(|| uri.to_string())
}

impl ResourceCache {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Value>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The answer kept for `uri` (a whole response: `{"result"}`).
    pub fn get(&self, uri: &str) -> Option<Value> {
        self.lock().get(uri).cloned()
    }

    /// Keep `answer` for `uri` when it is a result, small enough, and
    /// there is room.
    pub fn put(&self, uri: &str, answer: &Value) {
        if answer.get("result").is_none() || answer.get("error").is_some() {
            return;
        }
        if answer.to_string().len() as u64 > MAX_HTML {
            return;
        }
        let mut m = self.lock();
        if m.len() >= MAX_CACHED && !m.contains_key(uri) {
            return;
        }
        m.insert(uri.to_string(), answer.clone());
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests;
