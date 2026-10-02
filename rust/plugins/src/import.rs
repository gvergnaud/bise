//! `bise plugins import-mcp NAME [--dry-run] < servers.json` (BISE-273):
//! the MCP servers of another agent (Claude Code's `mcpServers`, Codex's
//! `mcp_servers` as JSON) become one Agent Plugin in the user root,
//! `~/.agents/plugins/NAME/` (plugin.json + mcp.json, 0600: the servers'
//! env may hold tokens). Read on stdin so the tokens go through a pipe,
//! never through an agent's screen; the output names servers, never
//! their env values.
//!
//! stdio servers, and remote ones: Claude Code's `type` http/sse with
//! `url` and `headers`, Codex's `url` with `http_headers`,
//! `env_http_headers` and `bearer_token_env_var` (those two become
//! `${VAR}` references, filled from bise's environment when the server
//! connects). Header values are written, never printed. A command given as an absolute path runs through
//! `sh -c 'exec "$0" "$@"'` (the plugin schema takes a bare executable
//! or a path inside the plugin). Run again, it rewrites its own plugin
//! (marked in plugin.json's description); another plugin of that name is
//! never touched.

use std::path::{Path, PathBuf};

use bise_home::style::Style;
use serde_json::{json, Map, Value};

use crate::resolve::{valid_name, MCP_SCHEMA, PLUGIN_SCHEMA};

/// Codex's limit keys ([`limits`]).
const LIMITS: [&str; 5] = ["startup_timeout_sec", "startup_timeout_ms", "tool_timeout_sec", "enabled_tools", "disabled_tools"];

/// The mark in plugin.json's description: the folder is import-mcp's.
pub const MARK: &str = "Imported by bise plugins import-mcp";

/// What the import makes of the input: the servers kept (the mcp.json
/// value) and one line per server (kept or skipped, why).
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub servers: Map<String, Value>,
    pub lines: Vec<String>,
}

/// Read `{"mcpServers": {...}}` or `{"mcp_servers": {...}}` or the bare
/// server map.
pub fn plan(input: &Value) -> Result<Plan, String> {
    let map = input
        .get("mcpServers")
        .or_else(|| input.get("mcp_servers"))
        .unwrap_or(input)
        .as_object()
        .ok_or("expected a JSON object of MCP servers ({\"mcpServers\": {...}})")?;
    let mut servers = Map::new();
    let mut lines = Vec::new();
    for (id, v) in map {
        match server(v) {
            Ok((s, notes)) => {
                let extra = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
                lines.push(format!("+ {}{}", id, extra));
                servers.insert(id.clone(), s);
            }
            Err(why) => lines.push(format!("- {}: skipped, {}", id, why)),
        }
    }
    Ok(Plan { servers, lines })
}

/// A remote server (Claude Code's `{"type": "http"|"sse", "url",
/// "headers"}`, Codex's `{"url", "bearer_token_env_var", "http_headers",
/// "env_http_headers"}`) in the plugin schema's shape. Header values are
/// copied, never printed: `${VAR}` references stay as they are and are
/// filled from bise's environment when the server connects.
fn remote(o: &Map<String, Value>, ty: &str) -> Result<(Value, Vec<String>), String> {
    let url = o.get("url").and_then(Value::as_str).filter(|u| !u.is_empty()).ok_or("no url")?;
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("${")) {
        return Err("the url is not http(s)".into());
    }
    let ty = match ty {
        "sse" => "sse",
        "http" | "streamable-http" | "streamable_http" | "streamableHttp" | "stdio" => "http",
        t => return Err(format!("type {:?} is not a transport bise knows", t)),
    };
    let mut headers = Map::new();
    let add = |k: &str, v: String, headers: &mut Map<String, Value>| -> Result<(), String> {
        if k.is_empty() || k.contains([':', ' ', '\r', '\n']) || v.contains(['\r', '\n']) {
            return Err(format!("header {:?} is not a valid header", k));
        }
        headers.insert(k.to_string(), json!(v));
        Ok(())
    };
    for key in ["headers", "http_headers"] {
        if let Some(h) = o.get(key) {
            let h = h.as_object().ok_or(format!("{} is not an object", key))?;
            for (k, v) in h {
                let v = v.as_str().ok_or(format!("header {} is not a string", k))?;
                add(k, v.to_string(), &mut headers)?;
            }
        }
    }
    // Codex: a header from an env var, and the bearer token's env var
    if let Some(h) = o.get("env_http_headers") {
        let h = h.as_object().ok_or("env_http_headers is not an object")?;
        for (k, v) in h {
            let var = v.as_str().filter(|v| valid_var(v)).ok_or(format!("env_http_headers {} is not an env var name", k))?;
            add(k, format!("${{{}}}", var), &mut headers)?;
        }
    }
    if let Some(v) = o.get("bearer_token_env_var") {
        let var = v.as_str().filter(|v| valid_var(v)).ok_or("bearer_token_env_var is not an env var name")?;
        add("Authorization", format!("Bearer ${{{}}}", var), &mut headers)?;
    }
    let mut notes = vec![format!("{} {}", ty, host_of(url))];
    if !headers.is_empty() {
        let names: Vec<&str> = headers.keys().map(String::as_str).collect();
        notes.push(format!("header{} {}", if names.len() == 1 { "" } else { "s" }, names.join(", ")));
    }
    // the login's client: Claude Code's "oauth" {clientId, clientSecret,
    // callbackPort, scopes}, Codex's oauth {client_id, client_secret,
    // callback_port} and "scopes" list, in mcp.json's (Claude's) keys
    let mut oauth = Map::new();
    let mut unused = Vec::new();
    if let Some(a) = o.get("oauth").and_then(Value::as_object) {
        for (k, v) in a {
            let to = match k.as_str() {
                "clientId" | "client_id" => "clientId",
                "clientSecret" | "client_secret" => "clientSecret",
                "callbackPort" | "callback_port" => "callbackPort",
                "scopes" => "scopes",
                _ => {
                    unused.push(format!("oauth.{}", k));
                    continue;
                }
            };
            oauth.insert(to.into(), v.clone());
        }
    }
    if let Some(s) = o.get("scopes").filter(|_| !oauth.contains_key("scopes")) {
        oauth.insert("scopes".into(), s.clone());
    }
    let oauth = match crate::oauth::Config::parse(&Value::Object(oauth.clone())) {
        Ok(_) if oauth.is_empty() => None,
        Ok(_) => {
            let mut what: Vec<&str> = oauth.keys().filter(|k| *k != "clientSecret").map(String::as_str).collect();
            what.sort();
            notes.push(format!("oauth {}", what.join(", ")));
            Some(oauth)
        }
        Err(e) => {
            unused.push(format!("oauth ({})", e));
            None
        }
    };
    let mut out = Map::new();
    out.insert("type".into(), json!(ty));
    out.insert("url".into(), json!(url));
    if !headers.is_empty() {
        out.insert("headers".into(), Value::Object(headers));
    }
    if let Some(a) = oauth {
        out.insert("oauth".into(), Value::Object(a));
    }
    limits(o, &mut out, &mut notes, &mut unused);
    let known = ["type", "url", "headers", "http_headers", "env_http_headers", "bearer_token_env_var", "enabled", "disabled", "transport", "oauth", "scopes"];
    let mut dropped: Vec<String> = o.keys().filter(|k| !known.contains(&k.as_str()) && !LIMITS.contains(&k.as_str())).cloned().collect();
    dropped.extend(unused);
    if !dropped.is_empty() {
        notes.push(format!("ignored: {}", dropped.join(", ")));
    }
    Ok((Value::Object(out), notes))
}

/// Codex's (and Vibe's) per-server limits, copied under the same keys
/// when they are valid ([`crate::resolve::Limits`]); a bad one is said.
fn limits(o: &Map<String, Value>, out: &mut Map<String, Value>, notes: &mut Vec<String>, unused: &mut Vec<String>) {
    let mut kept = Vec::new();
    for k in ["startup_timeout_sec", "startup_timeout_ms", "tool_timeout_sec", "enabled_tools", "disabled_tools"] {
        let Some(v) = o.get(k).filter(|v| !v.is_null()) else { continue };
        let one: Map<String, Value> = [(k.to_string(), v.clone())].into_iter().collect();
        match crate::resolve::Limits::parse(&one) {
            Ok(_) => {
                out.insert(k.into(), v.clone());
                kept.push(k);
            }
            Err(e) => unused.push(format!("{} ({})", k, e.replace('"', ""))),
        }
    }
    if !kept.is_empty() {
        notes.push(kept.join(", "));
    }
}

fn valid_var(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !v.starts_with(|c: char| c.is_ascii_digit())
}

/// The URL's host, for the lines printed (a path may hold a key).
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let hp = rest.split(['/', '?', '#']).next().unwrap_or("");
    hp.rsplit_once('@').map(|(_, h)| h).unwrap_or(hp).to_string()
}

/// One server in the plugin schema's shape; Err = why it can't be.
fn server(v: &Value) -> Result<(Value, Vec<String>), String> {
    let o = v.as_object().ok_or("not an object")?;
    let ty = o.get("type").and_then(Value::as_str).unwrap_or("stdio");
    if o.get("enabled").and_then(Value::as_bool) == Some(false) || o.get("disabled").and_then(Value::as_bool) == Some(true) {
        return Err("disabled there".into());
    }
    let Some(cmd) = o.get("command").and_then(Value::as_str).filter(|c| !c.is_empty()) else {
        return match o.get("url") {
            Some(_) => remote(o, ty),
            None => Err("no command and no url".into()),
        };
    };
    if ty != "stdio" {
        return Err(format!("type {:?} with a command: not a transport bise knows", ty));
    }
    let mut args: Vec<Value> = Vec::new();
    match o.get("args") {
        None | Some(Value::Null) => {}
        Some(Value::Array(a)) if a.iter().all(Value::is_string) => args = a.clone(),
        Some(_) => return Err("args is not a list of strings".into()),
    }
    let mut notes = Vec::new();
    // Codex's cwd: an absolute folder (mcp.json's cwd stays in the
    // plugin), so sh goes there first
    let cwd = match o.get("cwd") {
        None | Some(Value::Null) => None,
        Some(Value::String(c)) if c.starts_with('/') => Some(c.clone()),
        Some(_) => return Err("cwd is not an absolute path".into()),
    };
    if cmd.contains('/') && !cmd.starts_with('/') {
        return Err(format!("command {:?} is a relative path: give its absolute path", cmd));
    }
    let command = if let Some(c) = &cwd {
        notes.push("runs in its cwd, through sh".to_string());
        let mut a = vec![json!("-c"), json!("cd \"$0\" && exec \"$@\""), json!(c), json!(cmd)];
        a.append(&mut args);
        args = a;
        "sh".to_string()
    } else if cmd.starts_with('/') {
        notes.push("absolute command, run through sh".to_string());
        let mut a = vec![json!("-c"), json!("exec \"$0\" \"$@\""), json!(cmd)];
        a.append(&mut args);
        args = a;
        "sh".to_string()
    } else {
        cmd.to_string()
    };
    let mut out = Map::new();
    out.insert("type".into(), json!("stdio"));
    out.insert("command".into(), json!(command));
    if !args.is_empty() {
        out.insert("args".into(), Value::Array(args));
    }
    if let Some(env) = o.get("env").and_then(Value::as_object).filter(|e| !e.is_empty()) {
        let mut e = Map::new();
        for (k, v) in env {
            match v {
                Value::String(_) if k != "PLUGIN_ROOT" && k != "PLUGIN_DATA" => {
                    e.insert(k.clone(), v.clone());
                }
                Value::Number(_) | Value::Bool(_) => {
                    e.insert(k.clone(), json!(v.to_string()));
                }
                _ => return Err(format!("env {} is not a string", k)),
            }
        }
        notes.push(format!("{} env var(s)", e.len()));
        out.insert("env".into(), Value::Object(e));
    }
    let mut unused = Vec::new();
    limits(o, &mut out, &mut notes, &mut unused);
    // Codex's env_vars: names passed from its environment; bise's
    // servers get the whole environment, so nothing to write
    let known = ["type", "command", "args", "env", "enabled", "disabled", "cwd", "env_vars"];
    let mut dropped: Vec<String> = o.keys().filter(|k| !known.contains(&k.as_str()) && !LIMITS.contains(&k.as_str())).cloned().collect();
    dropped.extend(unused);
    if !dropped.is_empty() {
        notes.push(format!("ignored: {}", dropped.join(", ")));
    }
    Ok((Value::Object(out), notes))
}

/// Write the plan as plugin `name` under `root` (the user plugins root).
/// Refuses a folder that holds another plugin. Returns the folder.
pub fn write(root: &Path, name: &str, p: &Plan) -> Result<PathBuf, String> {
    let dir = root.join(name);
    let manifest = dir.join("plugin.json");
    if dir.exists() {
        let ours = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("description").and_then(Value::as_str).map(|d| d.starts_with(MARK)))
            .unwrap_or(false);
        if !ours {
            return Err(format!("{} exists and is not an import: pick another name", dir.display()));
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;
    let m = json!({
        "$schema": PLUGIN_SCHEMA,
        "name": name,
        "version": "0.1.0",
        "description": format!("{}: {} MCP server(s).", MARK, p.servers.len()),
    });
    let mcp = json!({ "$schema": MCP_SCHEMA, "mcpServers": Value::Object(p.servers.clone()) });
    write_private(&manifest, &(serde_json::to_string_pretty(&m).unwrap_or_default() + "\n"))?;
    write_private(&dir.join("mcp.json"), &(serde_json::to_string_pretty(&mcp).unwrap_or_default() + "\n"))?;
    Ok(dir)
}

fn write_private(file: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = file.with_extension("tmp-import");
    let res = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        std::fs::rename(&tmp, file)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot write {}: {}", file.display(), e)
    })
}

/// `bise plugins import-mcp NAME [--dry-run]`, the JSON on stdin; the
/// exit code.
pub fn main(args: &[String], root: &Path) -> i32 {
    let usage = "usage: bise plugins import-mcp NAME [--dry-run] < servers.json
  servers.json: {\"mcpServers\": {...}} (Claude Code's shape; Codex's mcp_servers as JSON works too).
  Writes ~/.agents/plugins/NAME/ (plugin.json, mcp.json 0600): stdio, http and sse servers.";
    let dry = args.iter().any(|a| a == "--dry-run");
    let (out, err) = (Style::stdout(), Style::stderr());
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--dry-run").collect();
    let name = match rest.as_slice() {
        [n] if valid_name(n) => n.as_str(),
        [n] if !n.starts_with('-') => {
            eprintln!("{}", err.fail(&format!("{:?} is not a plugin name: [a-z0-9.-], 1-64 chars, alphanumeric at both ends", n)));
            return 2;
        }
        _ => {
            eprintln!("{}", usage);
            return 2;
        }
    };
    let mut text = String::new();
    if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
        eprintln!("{}", err.fail(&format!("cannot read stdin: {}", e)));
        return 1;
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", err.fail(&format!("stdin is not JSON ({}): nothing written", e.classify_name())));
            return 1;
        }
    };
    let p = match plan(&v) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            return 1;
        }
    };
    for l in &p.lines {
        println!("{}", out.dim(l));
    }
    if p.servers.is_empty() {
        println!("{}", out.ask("no server bise can run: nothing written"));
        return 0;
    }
    let home = crate::resolve::home();
    let shown = |d: &Path| match d.strip_prefix(&home) {
        Ok(r) => format!("~/{}", r.display()),
        Err(_) => d.display().to_string(),
    };
    if dry {
        println!("{}", out.dim(&format!("dry run: would write {}", shown(&root.join(name)))));
        return 0;
    }
    match write(root, name, &p) {
        Ok(dir) => {
            let n = p.servers.len();
            println!("{}", out.ok(&format!("wrote {} ({} server{})", shown(&dir), n, if n == 1 { "" } else { "s" })));
            println!("{}", out.dim("they start with the next session, or /reload in one."));
            0
        }
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            1
        }
    }
}

trait Classify {
    fn classify_name(&self) -> &'static str;
}

impl Classify for serde_json::Error {
    /// The kind of error without the text (it may quote the input).
    fn classify_name(&self) -> &'static str {
        match self.classify() {
            serde_json::error::Category::Io => "io",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "bad data",
            serde_json::error::Category::Eof => "cut short",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, Roots};

    const TOKEN: &str = "ghp_SECRET_123";

    fn input() -> Value {
        json!({ "mcpServers": {
            "github": { "type": "stdio", "command": "npx", "args": ["-y", "@mcp/github"], "env": { "GITHUB_TOKEN": TOKEN } },
            "local": { "command": "/opt/tools/mcp-local", "args": ["--quiet"], "startup_timeout_sec": 20 },
            "linear": { "type": "http", "url": "https://mcp.linear.app/mcp", "headers": { "Authorization": format!("Bearer {}", TOKEN) } },
            "old": { "type": "sse", "url": "https://old.test/k3y/sse", "headers": { "X-Key": "${OLD_KEY}" },
                     "oauth": { "clientId": "abc", "callbackPort": 8765, "bogus": 1 } },
            "codex-remote": { "url": "https://x.test/mcp", "bearer_token_env_var": "X_TOKEN",
                              "http_headers": { "X-Region": "eu" }, "env_http_headers": { "X-Org": "X_ORG" },
                              "scopes": ["mcp.read"], "oauth": { "client_id": "cx", "client_secret": TOKEN } },
            "ftp": { "url": "ftp://x.test/" },
            "off": { "command": "uvx", "enabled": false },
            "rel": { "command": "./bin/x" },
            "codex-local": { "command": "uvx", "args": ["mcp-x"], "cwd": "/srv/x", "env_vars": ["X_KEY"],
                             "tool_timeout_sec": 600, "enabled_tools": ["a", "b"], "disabled_tools": ["b"],
                             "startup_timeout_ms": "soon", "required": true }
        }})
    }

    #[test]
    fn stdio_servers_are_kept_the_rest_said_and_no_token_shows() {
        let p = plan(&input()).unwrap();
        let ids: Vec<&str> = p.servers.keys().map(String::as_str).collect();
        assert_eq!(ids, ["codex-local", "codex-remote", "github", "linear", "local", "old"]);
        assert_eq!(
            p.servers["codex-local"],
            json!({"type": "stdio", "command": "sh", "args": ["-c", "cd \"$0\" && exec \"$@\"", "/srv/x", "uvx", "mcp-x"],
                   "tool_timeout_sec": 600, "enabled_tools": ["a", "b"], "disabled_tools": ["b"]})
        );
        assert_eq!(p.servers["local"]["startup_timeout_sec"], 20);
        assert_eq!(p.servers["linear"], json!({"type": "http", "url": "https://mcp.linear.app/mcp", "headers": {"Authorization": format!("Bearer {}", TOKEN)}}));
        assert_eq!(p.servers["old"]["type"], "sse");
        assert_eq!(p.servers["old"]["headers"]["X-Key"], "${OLD_KEY}");
        assert_eq!(p.servers["old"]["oauth"], json!({"clientId": "abc", "callbackPort": 8765}));
        assert_eq!(
            p.servers["codex-remote"],
            json!({"type": "http", "url": "https://x.test/mcp",
                   "headers": {"X-Region": "eu", "X-Org": "${X_ORG}", "Authorization": "Bearer ${X_TOKEN}"},
                   "oauth": {"clientId": "cx", "clientSecret": TOKEN, "scopes": ["mcp.read"]}})
        );
        assert_eq!(p.servers["local"]["command"], "sh");
        assert_eq!(p.servers["local"]["args"], json!(["-c", "exec \"$0\" \"$@\"", "/opt/tools/mcp-local", "--quiet"]));
        let text = p.lines.join("\n");
        assert!(text.contains("+ linear (http mcp.linear.app; header Authorization)"), "{text}");
        assert!(text.contains("+ old (sse old.test; header X-Key; oauth callbackPort, clientId; ignored: oauth.bogus)"), "{text}");
        assert!(text.contains("+ codex-remote (http x.test; headers Authorization, X-Org, X-Region; oauth clientId, scopes)"), "{text}");
        assert!(text.contains("- ftp: skipped, the url is not http(s)"), "{text}");
        assert!(!text.contains("k3y"), "a URL path may hold a key: {text}");
        assert!(text.contains("- off: skipped, disabled there"), "{text}");
        assert!(text.contains("- rel: skipped"), "{text}");
        assert!(text.contains("+ local (absolute command, run through sh; startup_timeout_sec)"), "{text}");
        assert!(
            text.contains("+ codex-local (runs in its cwd, through sh; tool_timeout_sec, enabled_tools, disabled_tools; ignored: required, startup_timeout_ms (startup_timeout_ms must be a whole number of milliseconds above 0))"),
            "{text}"
        );
        assert!(!text.contains(TOKEN));
        // Codex's key and the bare map work too
        assert_eq!(plan(&json!({ "mcp_servers": { "a": { "command": "x" } } })).unwrap().servers.len(), 1);
        assert_eq!(plan(&json!({ "a": { "command": "x" } })).unwrap().servers.len(), 1);
    }

    #[test]
    fn the_plugin_written_resolves_and_a_rerun_replaces_only_its_own() {
        let d = std::env::temp_dir().join(format!("bise-import-mcp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let root = d.join("plugins");
        let p = plan(&input()).unwrap();
        let dir = write(&root, "from-claude-code", &p).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("mcp.json")).unwrap().permissions().mode() & 0o777, 0o600);
        let roots = Roots { builtin: None, user: Some(root.clone()), workspace: None, data: d.join("data"), disabled: vec![], enabled: vec![] };
        let r = resolve(&roots);
        assert_eq!(r.plugins.len(), 1, "{:?}", r.diagnostics);
        let ids: Vec<&str> = r.plugins[0].servers.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["codex-local", "github", "local"], "{:?}", r.diagnostics);
        let cl = &r.plugins[0].servers[0];
        assert_eq!(cl.limits.tool_timeout, Some(std::time::Duration::from_secs(600)));
        assert!(cl.limits.allows("a") && !cl.limits.allows("b") && !cl.limits.allows("c"));
        let remotes: Vec<&str> = r.plugins[0].remotes.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(remotes, ["codex-remote", "linear", "old"], "{:?}", r.diagnostics);
        assert!(write(&root, "from-claude-code", &p).is_ok(), "a rerun rewrites its own import");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(root.join("mine/plugin.json"), "{\"name\":\"mine\"}").unwrap();
        assert!(write(&root, "mine", &p).unwrap_err().contains("not an import"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
