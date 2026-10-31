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
    /// its `index.html`, canonical, under `root`
    pub html: PathBuf,
    /// the plugin's root, canonical: `html` must stay under it, at load
    /// and at each read ([`html_under`])
    pub root: PathBuf,
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
    let json_path = html_under(root, &real.join("view.json")).map_err(|why| format!("view.json {}", why))?;
    let html = html_under(root, &real.join("index.html")).map_err(|why| format!("index.html {}", why))?;
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
        root: root.to_path_buf(),
        ui,
    })
}

/// `path` canonical (its symlinks followed), when it is a file under
/// `root` (canonical): a view's `index.html` or `view.json` that links
/// out of its plugin (`index.html -> ~/.ssh/id_rsa`) names no file. Its
/// error ends "...is missing" or "...is outside the plugin".
pub fn html_under(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let real = path.canonicalize().map_err(|_| "is missing".to_string())?;
    if !real.starts_with(root) {
        return Err("is outside the plugin".into());
    }
    if !real.is_file() {
        return Err("is not a file".into());
    }
    Ok(real)
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
                // read again each time (an edit shows), so checked again:
                // a file swapped for a link out of the plugin since the
                // load is refused, its target never read
                let read = html_under(&v.root, &v.html)
                    .map_err(|why| format!("index.html {}", why))
                    .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()));
                match read {
                    Ok(html) => json!({"result": {"contents": [{"uri": v.uri, "mimeType": MIME, "text": html, "_meta": {"ui": v.ui}}]}}),
                    Err(e) => err(-32603, format!("view {}: {}", v.name, e)),
                }
            }
            m => err(-32601, format!("method not found: {}", m)),
        }
    }
}

/// The header the REPL sends (`x-bise-call: 1`) on a `tools/call` whose
/// index line has `ui:` (docs/mcp-apps.md C2): keep its full result.
pub const CALL_HEADER: &str = "x-bise-call";
/// The header the bridge answers when it kept a call's full result: the
/// view's ref as ASCII JSON ([`view_header`]), which the REPL puts on its
/// `tool_view #<id>` line as is (C3).
pub const VIEW_HEADER: &str = "x-bise-view";
/// A views file bigger than this keeps `{"truncated": true}` for its
/// result (the view says "result too large").
pub const MAX_FILE: usize = 4 * 1024 * 1024;

/// A call id as the bridge makes it (`[0-9]+-[0-9]+`, at most 40
/// chars): anything else names no file.
pub fn call_ok(call: &str) -> bool {
    let Some((a, b)) = call.split_once('-') else { return false };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    call.len() <= 40 && digits(a) && digits(b)
}

/// The view's ref the REPL's `tool_view` line carries:
/// `{"plugin","server","uri","call"}`, every non-ASCII character as
/// `\uXXXX` so it rides an HTTP header (no CR or LF either). Pure.
pub fn view_header(plugin: &str, server: &str, uri: &str, call: &str) -> String {
    let text = json!({"plugin": plugin, "server": server, "uri": uri, "call": call}).to_string();
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut b = [0u16; 2];
            for u in c.encode_utf16(&mut b) {
                out.push_str(&format!("\\u{:04x}", u));
            }
        }
    }
    out
}

/// One call with a view, as its views file keeps it (C2).
pub struct Record<'a> {
    pub plugin: &'a str,
    pub server: &'a str,
    /// the server's own `ui://` URI (never the index's encoded one)
    pub uri: &'a str,
    /// the tool's published name
    pub tool: &'a str,
    pub args: &'a Value,
    /// the server's whole JSON-RPC answer: its `result` is kept, or its
    /// `error` (the view gets `tool-cancelled`)
    pub answer: &'a Value,
    pub at_ms: u64,
}

/// The views file's text: `{"v":1,"plugin","server","uri","tool","args",
/// "result","at_ms"}` (`"error"` instead of `"result"` for an error
/// answer); over [`MAX_FILE`], `result` is `{"truncated": true}`. Pure.
pub fn file_text(r: &Record) -> String {
    let mut v = json!({"v": 1, "plugin": r.plugin, "server": r.server, "uri": r.uri, "tool": r.tool, "args": r.args, "at_ms": r.at_ms});
    match (r.answer.get("result"), r.answer.get("error")) {
        (Some(res), _) => v["result"] = res.clone(),
        (None, Some(e)) => v["error"] = e.clone(),
        (None, None) => v["error"] = json!({"code": -32603, "message": "no result"}),
    }
    let text = v.to_string();
    if text.len() <= MAX_FILE {
        return text;
    }
    v["result"] = json!({"truncated": true});
    if let Some(o) = v.as_object_mut() {
        o.remove("error");
    }
    let text = v.to_string();
    if text.len() <= MAX_FILE {
        return text;
    }
    // the args alone are too big: dropped too
    v["args"] = json!({"truncated": true});
    v.to_string()
}

/// Keep `r` in `dir` under a new call id `<at_ms>-<n>` (the first `n`
/// whose file does not exist yet, taken by an exclusive create, so two
/// calls of one ms or two bridges of one agent never share a file), the
/// text written whole (a temp file, then a rename). The call id.
pub fn keep(dir: &Path, r: &Record) -> std::io::Result<String> {
    std::fs::create_dir_all(dir)?;
    for n in 0..1000u32 {
        let call = format!("{}-{}", r.at_ms, n);
        let path = dir.join(format!("{}.json", call));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => {
                let tmp = dir.join(format!(".{}.json.tmp", call));
                std::fs::write(&tmp, file_text(r))?;
                std::fs::rename(&tmp, &path)?;
                return Ok(call);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free call id"))
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
