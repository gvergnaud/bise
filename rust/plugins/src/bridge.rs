//! The per-session bridge (`bise plugins serve`): starts the
//! stdio MCP servers of the enabled plugins, writes the index files the
//! Bend REPL reads, then serves each server over loopback HTTP (the
//! JSON-response subset of MCP Streamable HTTP) until the REPL is gone.
//! Design: docs/plugins.md.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use std::sync::mpsc::{channel, Receiver, Sender};

use crate::remote::{Fail, OnChange, Remote};
use crate::report::{self, ServerStatus};
use crate::resolve::{self, Diagnostic, HttpServer, Plugin, Resolution, Severity, StdioServer};
use crate::status;
use crate::stdio::Client;

pub const START_TIMEOUT: Duration = Duration::from_secs(10);
/// a remote server's connect, handshake and tools/list (TLS, the
/// network): longer than a local process's
pub const REMOTE_START_TIMEOUT: Duration = Duration::from_secs(20);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Opts {
    /// the session's plugin dir (index files, logs, ready marker)
    pub dir: PathBuf,
    /// exit when this pid is gone (the REPL)
    pub parent: Option<u32>,
    pub roots: resolve::Roots,
    /// where each remote server's last state goes for `/plugins`
    /// (`status::dir()`); None: nowhere
    pub status_dir: Option<PathBuf>,
    /// the OAuth store (`oauth::store_dir()`); None: no login
    pub secrets_dir: Option<PathBuf>,
}

/// A server of a plugin, as mcp.json declares it.
#[derive(Clone)]
enum Spec {
    Stdio(StdioServer),
    Http(HttpServer),
}

impl Spec {
    fn id(&self) -> &str {
        match self {
            Spec::Stdio(s) => &s.id,
            Spec::Http(s) => &s.id,
        }
    }
}

/// A started server.
enum Conn {
    Stdio(Client),
    Remote(Remote),
}

impl Conn {
    fn init(&self) -> Value {
        match self {
            Conn::Stdio(c) => c.init.clone(),
            Conn::Remote(r) => r.init(),
        }
    }

    /// A local process can die; a remote client reconnects by itself.
    fn alive(&mut self) -> bool {
        match self {
            Conn::Stdio(c) => c.alive(),
            Conn::Remote(_) => true,
        }
    }

    fn request_raw(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        match self {
            Conn::Stdio(c) => c.request_raw(method, params, timeout).map_err(Fail::Other),
            Conn::Remote(r) => r.request_raw(method, params, timeout),
        }
    }

    fn list_tools(&self, timeout: Duration) -> Result<Vec<Value>, String> {
        match self {
            Conn::Stdio(c) => c.list_tools(timeout),
            Conn::Remote(r) => r.list_tools(timeout).map_err(|e| e.to_string()),
        }
    }

    fn stop(&mut self) {
        match self {
            Conn::Stdio(c) => c.stop(),
            Conn::Remote(r) => r.stop(),
        }
    }
}

/// One server of one plugin, as the bridge holds it.
struct Entry {
    /// `<plugin>/<server>`
    key: String,
    plugin: usize,
    plugin_name: String,
    namespace: String,
    spec: Spec,
    plugin_root: PathBuf,
    data_root: PathBuf,
    log: PathBuf,
    on_change: OnChange,
    secrets: Option<PathBuf>,
    /// a remote server that wants a login: its store file and the
    /// mtime last seen (a change means a login happened)
    login: Mutex<Option<(PathBuf, Option<std::time::SystemTime>)>>,
    client: Mutex<Option<Conn>>,
    /// the server's tools as it listed them
    tools: Mutex<Vec<Value>>,
    /// published tool name -> the server's own name
    names: Mutex<HashMap<String, String>>,
}

fn start_conn(spec: &Spec, root: &Path, data: &Path, log: &Path, on_change: OnChange, secrets: Option<&Path>) -> Result<Conn, Fail> {
    match spec {
        Spec::Stdio(s) => Client::start(s, root, data, log, START_TIMEOUT, Some(on_change)).map(Conn::Stdio).map_err(Fail::Other),
        Spec::Http(s) => {
            let env = |k: &str| std::env::var(k).ok();
            Remote::start_with(s, &env, secrets, on_change, REMOTE_START_TIMEOUT).map(Conn::Remote)
        }
    }
}

/// The store file of a remote server that may log in.
fn login_file(spec: &Spec, secrets: Option<&Path>) -> Option<PathBuf> {
    let (Spec::Http(s), Some(d)) = (spec, secrets) else { return None };
    let env = |k: &str| std::env::var(k).ok();
    let (url, _) = crate::remote::target(s, &env).ok()?;
    s.may_login().then(|| crate::oauth::file_of(d, &crate::oauth::resource_of(&url)))
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// What an agent reads when a server wants a login (the designer's words).
pub fn login_error(server: &str) -> String {
    format!("{} needs the user to log in (/plugins login). tell them, or go on without it.", server)
}

impl Entry {
    /// Forward one JSON-RPC request; a dead local server is restarted
    /// once.
    fn forward(&self, method: &str, mut params: Value) -> Result<Value, String> {
        if method == "tools/call" {
            let published = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(src) = self.names.lock().unwrap_or_else(|e| e.into_inner()).get(&published) {
                params["name"] = json!(src);
            }
        }
        let mut guard = self.client.lock().unwrap_or_else(|e| e.into_inner());
        let dead = match guard.as_mut() {
            Some(c) => !c.alive(),
            None => true,
        };
        let id = self.spec.id().to_string();
        let said = |e: Fail| match e {
            Fail::Auth { .. } => login_error(&id),
            e => e.to_string(),
        };
        if dead {
            *guard = Some(start_conn(&self.spec, &self.plugin_root, &self.data_root, &self.log, self.on_change.clone(), self.secrets.as_deref()).map_err(said)?);
        }
        let c = guard.as_ref().ok_or("no server")?;
        c.request_raw(method, params, CALL_TIMEOUT).map_err(said)
    }
}

pub struct Session {
    pub index: String,
    pub skills: String,
    pub report: String,
}

fn token() -> String {
    let mut b = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_ok();
    if !ok {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            ^ (std::process::id() as u128) << 64;
        b = t.to_le_bytes();
    }
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// Descriptions and schemas ride on one index line.
fn flat(s: &str) -> String {
    s.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

/// `name\tdescription\tpath`, tabs and quotes stripped (the scan's rule)
pub fn skill_line(name: &str, desc: &str, path: &Path) -> String {
    let clean = |s: &str| -> String { flat(s).chars().filter(|c| *c != '\t' && *c != '"').collect() };
    format!("{}\t{}\t{}\n", clean(name), clean(desc), path.display())
}

/// The connector index format: `<cid> <group> <tool> : #<desc> | input: <schema>`
pub fn index_line(cid: &str, group: &str, tool: &str, desc: &str, schema: Option<&Value>) -> String {
    let schema = schema.map(|s| format!(" | input: {}", flat(&s.to_string()))).unwrap_or_default();
    format!("{} {} {} : #{}{}\n", cid, group, tool, flat(desc), schema)
}

struct Started {
    plugin: usize,
    spec: Spec,
    log: PathBuf,
    on_change: OnChange,
    /// Err((why, wants a login))
    result: Result<(Conn, Vec<Value>), (String, bool)>,
}

/// Start every server of every loaded plugin, in parallel. `changed`
/// gets `<plugin>/<server>` when a server says its tools changed.
fn start_all(res: &Resolution, dir: &Path, changed: &Sender<String>, secrets: Option<&Path>) -> Vec<Started> {
    let mut handles = Vec::new();
    for (i, p) in res.plugins.iter().enumerate() {
        if p.state != resolve::State::Loaded {
            continue;
        }
        let specs = p.servers.iter().cloned().map(Spec::Stdio).chain(p.remotes.iter().cloned().map(Spec::Http));
        for spec in specs {
            let (root, data) = (p.root.clone(), p.data_root.clone());
            let log = dir.join(format!("{}.{}.log", p.name, spec.id()));
            let (tx, key) = (Mutex::new(changed.clone()), format!("{}/{}", p.name, spec.id()));
            let on_change: OnChange = Arc::new(move || {
                let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(key.clone());
            });
            let secrets = secrets.map(Path::to_path_buf);
            handles.push(std::thread::spawn(move || {
                let timeout = if matches!(spec, Spec::Http(_)) { REMOTE_START_TIMEOUT } else { START_TIMEOUT };
                let result = start_conn(&spec, &root, &data, &log, on_change.clone(), secrets.as_deref())
                    .map_err(|e| {
                        let login = matches!(e, Fail::Auth { .. }) && login_file(&spec, secrets.as_deref()).is_some();
                        (e.to_string(), login)
                    })
                    .and_then(|c| match c.list_tools(timeout) {
                        Ok(ts) => Ok((c, ts)),
                        Err(e) => Err((format!("tools/list: {}", e), false)),
                    });
                let result = result.map_err(|(e, login)| {
                    if matches!(spec, Spec::Http(_)) {
                        return (e, login);
                    }
                    let tail = std::fs::read_to_string(&log).unwrap_or_default();
                    let tail: Vec<&str> = tail.lines().rev().take(3).collect();
                    if tail.is_empty() {
                        (e, login)
                    } else {
                        let t: Vec<&str> = tail.into_iter().rev().collect();
                        (format!("{} (stderr: {})", e, t.join(" | ")), login)
                    }
                });
                Started { plugin: i, spec, log, on_change, result }
            }));
        }
    }
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// The index of every entry's tools, in order; sets each entry's
/// published names. Two tools of one plugin with one name: the later
/// is dropped (a diagnostic).
fn index_of(entries: &[Arc<Entry>], base: &str) -> (String, Vec<Diagnostic>, HashMap<String, usize>) {
    let mut index = String::new();
    let mut diags = Vec::new();
    let mut counts = HashMap::new();
    let mut taken: HashMap<usize, Vec<String>> = HashMap::new();
    for e in entries {
        let cid = format!("{}/{}", base, e.key);
        let mut names = HashMap::new();
        let used = taken.entry(e.plugin).or_default();
        for t in e.tools.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            let Some(src) = t.get("name").and_then(Value::as_str) else { continue };
            let published = resolve::identifier(src);
            if used.contains(&published) {
                diags.push(Diagnostic {
                    code: "plugin.tool.name_collision",
                    severity: Severity::Warning,
                    plugin: e.plugin_name.clone(),
                    message: format!("tool {}.{} (server {:?}) is already taken; dropped", e.namespace, published, e.spec.id()),
                });
                continue;
            }
            used.push(published.clone());
            let desc = t.get("description").and_then(Value::as_str).unwrap_or("");
            index.push_str(&index_line(&cid, &e.namespace, &published, desc, t.get("inputSchema")));
            names.insert(published, src.to_string());
        }
        counts.insert(e.key.clone(), names.len());
        *e.names.lock().unwrap_or_else(|e| e.into_inner()) = names;
    }
    (index, diags, counts)
}

/// A remote server's state for `/plugins`.
fn remote_status(status_dir: Option<&Path>, plugin: &str, spec: &Spec, tools: &Result<usize, String>) {
    if let (Some(d), Spec::Http(s)) = (status_dir, spec) {
        status::write(d, plugin, &s.id, &status::Status::now(s.transport.as_str(), &s.host(), tools.clone()));
    }
}

fn remote_login_status(status_dir: Option<&Path>, plugin: &str, spec: &Spec) {
    if let (Some(d), Spec::Http(s)) = (status_dir, spec) {
        status::write(d, plugin, &s.id, &status::Status::login_needed(s.transport.as_str(), &s.host()));
    }
}

/// Start the servers and build the index files. The entries are in
/// plugin and server order.
fn build(
    res: &mut Resolution,
    dir: &Path,
    base: &str,
    changed: &Sender<String>,
    status_dir: Option<&Path>,
    secrets: Option<&Path>,
) -> (Vec<Arc<Entry>>, Session) {
    let started = start_all(res, dir, changed, secrets);
    let mut entries = Vec::new();
    let mut status: Vec<ServerStatus> = Vec::new();
    let mut extra: Vec<Diagnostic> = Vec::new();
    for st in started {
        let p: &Plugin = &res.plugins[st.plugin];
        let new_entry = |client: Option<Conn>, tools: Vec<Value>, spec: Spec, log: PathBuf, on_change: OnChange, login| {
            Arc::new(Entry {
                key: format!("{}/{}", p.name, spec.id()),
                plugin: st.plugin,
                plugin_name: p.name.clone(),
                namespace: p.namespace.clone(),
                log,
                spec,
                plugin_root: p.root.clone(),
                data_root: p.data_root.clone(),
                on_change,
                secrets: secrets.map(Path::to_path_buf),
                login: Mutex::new(login),
                client: Mutex::new(client),
                tools: Mutex::new(tools),
                names: Mutex::new(HashMap::new()),
            })
        };
        let (client, tools) = match st.result {
            Ok(ct) => ct,
            Err((_, true)) => {
                // kept, without tools: a login (any session's) brings it up
                extra.push(Diagnostic {
                    code: "plugin.mcp.login_needed",
                    severity: Severity::Info,
                    plugin: p.name.clone(),
                    message: format!("MCP server {:?} needs a login: /plugins login", st.spec.id()),
                });
                remote_login_status(status_dir, &p.name, &st.spec);
                status.push(ServerStatus { plugin: p.name.clone(), server: st.spec.id().to_string(), tools: Err("needs a login".into()) });
                let file = login_file(&st.spec, secrets).map(|f| {
                    let m = mtime(&f);
                    (f, m)
                });
                entries.push(new_entry(None, Vec::new(), st.spec, st.log, st.on_change, file));
                continue;
            }
            Err((e, false)) => {
                extra.push(Diagnostic {
                    code: "plugin.mcp.connection_failed",
                    severity: Severity::Warning,
                    plugin: p.name.clone(),
                    message: format!("MCP server {:?}: {}", st.spec.id(), e),
                });
                remote_status(status_dir, &p.name, &st.spec, &Err(e.clone()));
                status.push(ServerStatus { plugin: p.name.clone(), server: st.spec.id().to_string(), tools: Err(e) });
                continue;
            }
        };
        entries.push(new_entry(Some(client), tools, st.spec, st.log, st.on_change, None));
    }
    let (index, diags, counts) = index_of(&entries, base);
    for e in entries.iter().filter(|e| e.login.lock().unwrap_or_else(|e| e.into_inner()).is_none()) {
        let n = Ok(counts.get(&e.key).copied().unwrap_or(0));
        remote_status(status_dir, &e.plugin_name, &e.spec, &n);
        status.push(ServerStatus { plugin: e.plugin_name.clone(), server: e.spec.id().to_string(), tools: n });
    }
    res.diagnostics.extend(extra);
    res.diagnostics.extend(diags);
    let mut skills = String::new();
    for p in res.loaded() {
        for s in &p.skills {
            skills.push_str(&skill_line(&s.name, &s.description, &s.path));
        }
    }
    status.sort_by(|a, b| (&a.plugin, &a.server).cmp(&(&b.plugin, &b.server)));
    let report = report::text(res, Some(&status));
    (entries, Session { index, skills, report })
}

/// `notifications/tools/list_changed`: list that server's tools again
/// and rewrite the index (the REPL reads it at each search and call).
fn refresh_loop(rx: Receiver<String>, entries: Arc<Vec<Arc<Entry>>>, dir: PathBuf, base: String, status_dir: Option<PathBuf>) {
    while let Ok(first) = rx.recv() {
        // a burst of notifications is one refresh
        std::thread::sleep(Duration::from_millis(100));
        let mut keys = vec![first];
        keys.extend(rx.try_iter());
        keys.sort();
        keys.dedup();
        for k in keys {
            let Some(e) = entries.iter().find(|e| e.key == k) else { continue };
            if e.client.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                // a login happened: connect now
                match start_conn(&e.spec, &e.plugin_root, &e.data_root, &e.log, e.on_change.clone(), e.secrets.as_deref()) {
                    Ok(c) => {
                        *e.client.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
                        *e.login.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    }
                    Err(Fail::Auth { .. }) => continue,
                    Err(err) => {
                        remote_status(status_dir.as_deref(), &e.plugin_name, &e.spec, &Err(err.to_string()));
                        continue;
                    }
                }
            }
            let listed = {
                let guard = e.client.lock().unwrap_or_else(|e| e.into_inner());
                match guard.as_ref() {
                    Some(c) => c.list_tools(CALL_TIMEOUT),
                    None => continue,
                }
            };
            match listed {
                Ok(ts) => *e.tools.lock().unwrap_or_else(|e| e.into_inner()) = ts,
                Err(err) => remote_status(status_dir.as_deref(), &e.plugin_name, &e.spec, &Err(format!("tools/list: {}", err))),
            }
        }
        let (index, _, counts) = index_of(&entries, &base);
        let _ = write_atomic(&dir.join("mcp-index.txt"), &index);
        for e in entries.iter().filter(|e| e.client.lock().unwrap_or_else(|e| e.into_inner()).is_some()) {
            remote_status(status_dir.as_deref(), &e.plugin_name, &e.spec, &Ok(counts.get(&e.key).copied().unwrap_or(0)));
        }
    }
}

// ---- HTTP ----

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
    close: bool,
}

fn read_request(r: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    if r.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut len = 0usize;
    let mut close = false;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            if k == "content-length" {
                len = v.trim().parse().ok()?;
            } else if k == "connection" && v.trim().eq_ignore_ascii_case("close") {
                close = true;
            }
        }
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    Some(Request { method, path, body, close })
}

fn respond(w: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    let ctype = if body.is_empty() { "" } else { "Content-Type: application/json\r\n" };
    write!(
        w,
        "HTTP/1.1 {}\r\n{}Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
        status,
        ctype,
        body.len(),
        body
    )?;
    w.flush()
}

fn rpc_error(id: &Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// One JSON-RPC message for one server: (HTTP status, body).
fn handle_rpc(entry: &Entry, body: &[u8]) -> (&'static str, String) {
    let Ok(msg) = serde_json::from_slice::<Value>(body) else {
        return ("400 Bad Request", rpc_error(&Value::Null, -32700, "parse error"));
    };
    let Some(id) = msg.get("id").cloned() else {
        // a notification: accepted, nothing to answer
        return ("202 Accepted", String::new());
    };
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    if method == "initialize" {
        let init = entry
            .client
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(Conn::init)
            .unwrap_or_else(|| json!({"protocolVersion": crate::stdio::PROTOCOL, "capabilities": {"tools": {}},
                                     "serverInfo": {"name": "bend-plugin-bridge", "version": "1"}}));
        return ("200 OK", json!({"jsonrpc": "2.0", "id": id, "result": init}).to_string());
    }
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    match entry.forward(method, params) {
        Ok(answer) => ("200 OK", stamp(answer, id)),
        Err(e) => ("200 OK", rpc_error(&id, -32000, &e)),
    }
}

/// The server's answer with the client's id: a JSON object gets `id`
/// and `jsonrpc`; anything else becomes an error answer (indexing a
/// non-object `Value` panics).
fn stamp(answer: Value, id: Value) -> String {
    match answer {
        Value::Object(mut o) => {
            o.insert("id".into(), id);
            o.insert("jsonrpc".into(), json!("2.0"));
            Value::Object(o).to_string()
        }
        other => rpc_error(&id, -32000, &format!("the plugin server answered a non-object: {}", other)),
    }
}

fn connection(stream: TcpStream, token: String, entries: Arc<Vec<Arc<Entry>>>) {
    let Ok(mut w) = stream.try_clone() else { return };
    let mut r = BufReader::new(stream);
    while let Some(req) = read_request(&mut r) {
        let prefix = format!("/{}/", token);
        let key = req.path.strip_prefix(&prefix).map(|k| k.trim_end_matches('/').to_string());
        let out = match (req.method.as_str(), key.and_then(|k| entries.iter().find(|e| e.key == k).cloned())) {
            ("POST", Some(e)) => handle_rpc(&e, &req.body),
            (_, Some(_)) => ("405 Method Not Allowed", String::new()),
            (_, None) => ("404 Not Found", String::new()),
        };
        if respond(&mut w, out.0, &out.1).is_err() || req.close {
            break;
        }
    }
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Run the bridge. Returns when the parent is gone, or at once when
/// no server is up (the index files are written either way).
pub fn serve(opts: Opts) -> std::io::Result<()> {
    std::fs::create_dir_all(&opts.dir)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = token();
    let base = format!("http://127.0.0.1:{}/{}", port, token);
    let mut res = resolve::resolve(&opts.roots);
    let (changed_tx, changed_rx) = channel();
    let (entries, session) = build(&mut res, &opts.dir, &base, &changed_tx, opts.status_dir.as_deref(), opts.secrets_dir.as_deref());
    write_atomic(&opts.dir.join("mcp-index.txt"), &session.index)?;
    write_atomic(&opts.dir.join("skills-index.txt"), &session.skills)?;
    write_atomic(&opts.dir.join("report.txt"), &session.report)?;
    write_atomic(&opts.dir.join("ready"), &format!("{}\n", std::process::id()))?;
    let entries = Arc::new(entries);
    // nothing to serve: the files say so, no process lingers
    if entries.is_empty() {
        return Ok(());
    }
    {
        let (entries, dir, base, sd) = (entries.clone(), opts.dir.clone(), base.clone(), opts.status_dir.clone());
        std::thread::spawn(move || refresh_loop(changed_rx, entries, dir, base, sd));
    }
    {
        let entries = entries.clone();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let (t, e) = (token.clone(), entries.clone());
                std::thread::spawn(move || connection(s, t, e));
            }
        });
    }
    // a login (from /plugins login, any session) shows as a change of
    // the server's store file: connect it then
    let logins = |entries: &Vec<Arc<Entry>>| {
        for e in entries {
            let mut l = e.login.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((f, seen)) = l.as_mut() {
                let now = mtime(f);
                if now.is_some() && now != *seen {
                    *seen = now;
                    let _ = changed_tx.send(e.key.clone());
                }
            }
        }
    };
    let mut tick = 0u64;
    loop {
        if let Some(pid) = opts.parent {
            if !alive(pid) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
        tick += 1;
        if tick.is_multiple_of(4) {
            logins(&entries);
        }
    }
    for e in entries.iter() {
        if let Some(mut c) = e.client.lock().unwrap_or_else(|e| e.into_inner()).take() {
            c.stop();
        }
    }
    Ok(())
}

#[cfg(test)]
mod stamp_tests {
    use super::*;

    #[test]
    fn a_non_object_answer_is_an_error_not_a_panic() {
        let ok: Value = serde_json::from_str(&stamp(json!({"result": 1}), json!(7))).unwrap_or_default();
        assert_eq!((ok["id"].clone(), ok["result"].clone()), (json!(7), json!(1)));
        for bad in [json!([1, 2]), json!(3), json!("s"), Value::Null] {
            let e: Value = serde_json::from_str(&stamp(bad, json!(8))).unwrap_or_default();
            assert_eq!(e["id"], json!(8));
            assert!(e["error"].is_object(), "{e}");
        }
    }
}
