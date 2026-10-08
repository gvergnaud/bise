//! A remote MCP client: Streamable HTTP (MCP 2025-03-26+) and the
//! legacy HTTP+SSE transport (2024-11-05), with static headers.
//!
//! - Streamable HTTP: every JSON-RPC message is a POST to the URL
//!   (`Accept: application/json, text/event-stream`); the answer is JSON
//!   or an event stream that carries it. The `Mcp-Session-Id` the server
//!   gives at `initialize` goes on every later request with
//!   `MCP-Protocol-Version`; a 404 on a session means it expired: a new
//!   handshake, then the request again. A server that announces
//!   `tools.listChanged` gets a GET event stream for its notifications
//!   (reopened when it drops; a 405 means it has none).
//! - SSE: a GET event stream whose first `endpoint` event names the URL
//!   to POST to; the answers come back on the stream. A dropped stream
//!   is reopened (and the handshake run again) at the next request.
//!
//! A request that may have reached the server is never sent twice: only
//! a connect failure or an expired session retries. Errors are one line
//! that names the host, never a header value or the URL's path.

use std::collections::HashMap;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::http::{self, Url};
use crate::oauth::Auth;
use crate::resolve::{expand_env, HttpServer, Transport};

pub const PROTOCOL: &str = crate::stdio::PROTOCOL;
/// the most an answer may weigh
const MAX_BODY: usize = 32 << 20;

/// Called when the server says its tool list changed.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// Why a request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    /// nothing reached the server: a retry is safe
    Connect(String),
    /// the server forgot our session (404): a new handshake
    SessionGone,
    /// 401: the server wants a login; the `WWW-Authenticate` header
    Auth { host: String, challenge: Option<String> },
    /// 403 `insufficient_scope`: the login must be done again with the
    /// scope the challenge names (step-up, MCP authorization 2025-11-25)
    Scope { host: String, challenge: String },
    /// a 401 while the login is in the macOS keychain and it is locked:
    /// never "needs a login" (the login is kept, it comes back unlocked)
    Locked { host: String },
    Other(String),
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Connect(s) | Fail::Other(s) => f.write_str(s),
            Fail::SessionGone => f.write_str("the server ended the session"),
            Fail::Auth { host, .. } => write!(f, "{} answered 401: it needs a login or a token in \"headers\"", host),
            Fail::Scope { host, .. } => write!(f, "{} answered 403: the login needs more access (insufficient_scope)", host),
            Fail::Locked { host } => f.write_str(&locked_line(host)),
        }
    }
}

impl From<http::Error> for Fail {
    fn from(e: http::Error) -> Fail {
        match e {
            http::Error::Connect(s) => Fail::Connect(s),
            http::Error::Io(s) => Fail::Other(s),
        }
    }
}

/// A server whose login is in a locked keychain (designer, m_13340):
/// `name` is its name, else its host.
pub fn locked_line(name: &str) -> String {
    format!("the keychain is locked, so {} can't log in. unlock your Mac and it comes back.", name)
}

/// The server's URL and headers with `${VAR}` filled from `env`. Errors
/// name the variable and the header, never a value.
pub fn target(s: &HttpServer, env: &dyn Fn(&str) -> Option<String>) -> Result<(Url, Vec<(String, String)>), String> {
    let url = expand_env(&s.url, env).map_err(|e| format!("\"url\" uses {}", e.trim_end_matches(" is not set")) + ", which is not set")?;
    let url = Url::parse(&url).map_err(|e| format!("\"url\": {}", e))?;
    let mut headers = Vec::new();
    for (k, v) in &s.headers {
        let v = expand_env(v, env).map_err(|e| format!("header {} uses {}, which is not set", k, e.trim_end_matches(" is not set")))?;
        headers.push((k.clone(), v));
    }
    Ok((url, headers))
}

fn has(headers: &[(String, String)], name: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

/// The configured headers, and the login's bearer token when there is
/// one (never over a header the config sets itself).
fn with_auth(headers: &[(String, String)], auth: &Option<Arc<Auth>>) -> Vec<(String, String)> {
    let mut h = headers.to_vec();
    if let Some(a) = auth {
        if !has(&h, "authorization") {
            if let Some(t) = a.bearer() {
                h.push(("Authorization".into(), format!("Bearer {}", t)));
            }
        }
    }
    h
}

/// `f` again once when it failed with a 401 and the login has a newer
/// token (another bridge refreshed it, or a refresh now). A 403
/// `insufficient_scope` leaves the scope it asks for in the store: the
/// next login asks for it with the ones granted before.
fn retry_401<T>(auth: &Option<Arc<Auth>>, f: impl Fn() -> Result<T, Fail>) -> Result<T, Fail> {
    match f() {
        Err(Fail::Auth { host, challenge }) => match auth {
            Some(a) if a.after_401() => f(),
            Some(a) if a.locked() => Err(Fail::Locked { host }),
            _ => Err(Fail::Auth { host, challenge }),
        },
        Err(Fail::Scope { host, challenge }) => {
            if let (Some(a), Some(scope)) = (auth, crate::oauth::challenge_param(&challenge, "scope")) {
                a.want_scope(&scope);
            }
            Err(Fail::Scope { host, challenge })
        }
        r => r,
    }
}

fn status_fail(status: u16, host: &str, resp: http::Response) -> Fail {
    if status == 401 {
        let challenge = resp.header("www-authenticate").map(String::from);
        return Fail::Auth { host: host.to_string(), challenge };
    }
    if status == 403 {
        if let Some(c) = resp.header("www-authenticate").filter(|c| crate::oauth::challenge_param(c, "error").as_deref() == Some("insufficient_scope")) {
            return Fail::Scope { host: host.to_string(), challenge: c.to_string() };
        }
    }
    let body = resp.read_all(64 << 10).map(|b| http::short_body(&b)).unwrap_or_default();
    let what = match status {
        403 => "forbidden".to_string(),
        404 => "not found (check the URL)".to_string(),
        405 => "method not allowed (is it the right transport?)".to_string(),
        _ => String::new(),
    };
    let mut s = format!("HTTP {} from {}", status, host);
    for part in [what, body] {
        if !part.is_empty() && !s.contains(&part) {
            s.push_str(": ");
            s.push_str(&part);
        }
    }
    Fail::Other(s)
}

/// The most redirects one request follows (Codex's limit).
const MAX_REDIRECTS: usize = 10;

/// One request, following the server's redirects on its own origin (a
/// `/mcp` that moved to `/mcp/`): 307 and 308 keep the method and the
/// body; a GET also follows 301, 302 and 303. Another origin is refused,
/// so the headers (a token) never leave it; at most 10.
fn send(method: &str, url: &Url, headers: &[(String, String)], body: &[u8], timeout: Duration) -> Result<http::Response, Fail> {
    let mut at = url.clone();
    for _ in 0..=MAX_REDIRECTS {
        let resp = http::send(&http::Request { method, url: &at, headers, body, timeout })?;
        let follows = match resp.status {
            307 | 308 => true,
            301..=303 => method == "GET",
            _ => false,
        };
        let Some(next) = resp.header("location").filter(|_| follows).map(String::from) else {
            return Ok(resp);
        };
        let next = at.join(&next).map_err(|e| Fail::Other(format!("{} redirected to a bad address: {}", url.shown(), e)))?;
        if next.origin() != url.origin() {
            return Err(Fail::Other(format!("{} redirected to another site ({}): refused", url.shown(), next.shown())));
        }
        at = next;
    }
    Err(Fail::Other(format!("{} redirected more than {} times", url.shown(), MAX_REDIRECTS)))
}

/// A JSON-RPC response to `id`?
fn answers(m: &Value, id: u64) -> bool {
    m.get("method").is_none() && m.get("id").and_then(Value::as_u64) == Some(id)
}

/// A server's message that is not an answer to us: a notification or a
/// request. Returns the reply a request needs.
fn incoming(m: &Value, on_change: &OnChange) -> Option<Value> {
    let method = m.get("method").and_then(Value::as_str)?;
    match m.get("id") {
        None => {
            if method == "notifications/tools/list_changed" {
                on_change();
            }
            None
        }
        Some(id) => Some(if method == "ping" {
            json!({"jsonrpc": "2.0", "id": id, "result": {}})
        } else {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "not supported by bise"}})
        }),
    }
}

fn result_of(raw: Value) -> Result<Value, Fail> {
    match raw.get("result") {
        Some(r) => Ok(r.clone()),
        None => Err(Fail::Other(
            raw.get("error")
                .map(|e| e.get("message").and_then(Value::as_str).map(String::from).unwrap_or(e.to_string()))
                .unwrap_or_else(|| "no result".into()),
        )),
    }
}

fn init_params() -> Value {
    json!({"protocolVersion": PROTOCOL, "capabilities": {},
           "clientInfo": {"name": "bise", "version": "1.0"}})
}

// ---- Streamable HTTP ----

struct Streamable {
    url: Url,
    headers: Vec<(String, String)>,
    auth: Option<Arc<Auth>>,
    host: String,
    session: Mutex<Option<String>>,
    protocol: Mutex<Option<String>>,
    next: AtomicU64,
    on_change: OnChange,
    init: Mutex<Value>,
    stop: AtomicBool,
    /// the GET stream's socket, to unblock it at the end
    listening: Mutex<Option<TcpStream>>,
}

impl Streamable {
    fn request_headers(&self, accept: &str, json_body: bool) -> Vec<(String, String)> {
        let mut h = with_auth(&self.headers, &self.auth);
        if !has(&h, "accept") {
            h.push(("Accept".into(), accept.into()));
        }
        if json_body {
            h.push(("Content-Type".into(), "application/json".into()));
        }
        if let Some(s) = self.session.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            h.push(("Mcp-Session-Id".into(), s));
        }
        if let Some(p) = self.protocol.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            h.push(("MCP-Protocol-Version".into(), p));
        }
        h
    }

    /// POST one message; the answer to `want` when there is one.
    fn post(&self, msg: &Value, want: Option<u64>, timeout: Duration) -> Result<Option<Value>, Fail> {
        let body = msg.to_string();
        let headers = self.request_headers("application/json, text/event-stream", true);
        let resp = send("POST", &self.url, &headers, body.as_bytes(), timeout)?;
        let status = resp.status;
        if status == 404 && self.session.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            return Err(Fail::SessionGone);
        }
        if !(200..300).contains(&status) {
            return Err(status_fail(status, &self.host, resp));
        }
        if let Some(s) = resp.header("mcp-session-id") {
            *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.to_string());
        }
        let Some(id) = want else { return Ok(None) };
        if status == 202 || status == 204 {
            return Err(Fail::Other(format!("{} accepted the request but sent no answer", self.host)));
        }
        if resp.content_type() == "text/event-stream" {
            let mut ev = resp.events();
            loop {
                match ev.next_event() {
                    Ok(Some(e)) => {
                        let Ok(m) = serde_json::from_str::<Value>(&e.data) else { continue };
                        for m in if m.is_array() { m.as_array().cloned().unwrap_or_default() } else { vec![m] } {
                            if answers(&m, id) {
                                return Ok(Some(m));
                            }
                            self.handle(&m, timeout);
                        }
                    }
                    Ok(None) => return Err(Fail::Other(format!("{} ended the event stream before the answer", self.host))),
                    Err(e) => {
                        return Err(Fail::Other(match e.kind() {
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                                format!("{} did not answer within {}s", self.host, timeout.as_secs())
                            }
                            _ => format!("the stream from {} dropped: {}", self.host, e),
                        }))
                    }
                }
            }
        }
        let b = resp.read_all(MAX_BODY)?;
        let m: Value = serde_json::from_slice(&b).map_err(|_| Fail::Other(format!("{} answered something that is not JSON", self.host)))?;
        let all = if m.is_array() { m.as_array().cloned().unwrap_or_default() } else { vec![m] };
        let mut found = None;
        for m in all {
            if answers(&m, id) {
                found = Some(m);
            } else {
                self.handle(&m, timeout);
            }
        }
        found.map(Some).ok_or_else(|| Fail::Other(format!("{}'s answer has no response to the request", self.host)))
    }

    /// A message from the server that is not our answer.
    fn handle(&self, m: &Value, timeout: Duration) {
        if let Some(reply) = incoming(m, &self.on_change) {
            let _ = self.post(&reply, None, timeout);
        }
    }

    fn rpc(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.post(&msg, Some(id), timeout)?.ok_or_else(|| Fail::Other("no answer".into()))
    }

    fn handshake(&self, timeout: Duration) -> Result<Value, Fail> {
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.protocol.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let init = result_of(self.rpc("initialize", init_params(), timeout)?)?;
        let version = init.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL).to_string();
        *self.protocol.lock().unwrap_or_else(|e| e.into_inner()) = Some(version);
        self.post(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), None, timeout)?;
        *self.init.lock().unwrap_or_else(|e| e.into_inner()) = init.clone();
        Ok(init)
    }

    fn request_raw(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        match self.rpc(method, params.clone(), timeout) {
            Err(Fail::SessionGone) => {
                self.handshake(timeout)?;
                self.rpc(method, params, timeout)
            }
            Err(Fail::Connect(_)) => {
                // the server restarted or the network blinked: once more
                std::thread::sleep(Duration::from_millis(300));
                match self.rpc(method, params.clone(), timeout) {
                    Err(Fail::SessionGone) => {
                        self.handshake(timeout)?;
                        self.rpc(method, params, timeout)
                    }
                    r => r,
                }
            }
            r => r,
        }
    }

    /// The GET stream for notifications, reopened when it drops.
    fn listen(self: Arc<Self>) {
        let mut wait = 1u64;
        while !self.stop.load(Ordering::SeqCst) {
            let headers = self.request_headers("text/event-stream", false);
            let r = send("GET", &self.url, &headers, b"", Duration::from_secs(30));
            match r {
                Ok(resp) if resp.status == 405 => return,
                Ok(resp) if resp.status == 200 && resp.content_type() == "text/event-stream" => {
                    wait = 1;
                    let mut ev = resp.events();
                    ev.set_timeout(None);
                    *self.listening.lock().unwrap_or_else(|e| e.into_inner()) = ev.closer();
                    if self.stop.load(Ordering::SeqCst) {
                        return;
                    }
                    while let Ok(Some(e)) = ev.next_event() {
                        if let Ok(m) = serde_json::from_str::<Value>(&e.data) {
                            self.handle(&m, Duration::from_secs(30));
                        }
                    }
                }
                Ok(_) | Err(_) => {}
            }
            for _ in 0..wait * 10 {
                if self.stop.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            wait = (wait * 2).min(30);
        }
    }

    fn close(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.listening.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = s.shutdown(Shutdown::Both);
        }
        // the spec's way to end a session; best effort
        if self.session.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            let headers = self.request_headers("application/json", false);
            let _ = send("DELETE", &self.url, &headers, b"", Duration::from_secs(2));
        }
    }
}

// ---- legacy HTTP+SSE ----

type Pending = Mutex<HashMap<u64, Sender<Value>>>;

struct Sse {
    url: Url,
    headers: Vec<(String, String)>,
    auth: Option<Arc<Auth>>,
    host: String,
    /// the POST endpoint of the open stream; None: no stream
    endpoint: Mutex<Option<(Url, u64)>>,
    generation: AtomicU64,
    pending: Pending,
    next: AtomicU64,
    on_change: OnChange,
    init: Mutex<Value>,
    stop: AtomicBool,
    stream: Mutex<Option<TcpStream>>,
    /// one (re)connect at a time
    connecting: Mutex<()>,
}

impl Sse {
    /// Open the stream, wait for its endpoint, run the handshake.
    fn connect(self: &Arc<Self>, timeout: Duration) -> Result<Value, Fail> {
        let mut headers = with_auth(&self.headers, &self.auth);
        if !has(&headers, "accept") {
            headers.push(("Accept".into(), "text/event-stream".into()));
        }
        let resp = send("GET", &self.url, &headers, b"", timeout)?;
        if !(200..300).contains(&resp.status) {
            return Err(status_fail(resp.status, &self.host, resp));
        }
        if resp.content_type() != "text/event-stream" {
            return Err(Fail::Other(format!(
                "{} did not open an event stream: a Streamable HTTP server? use \"type\": \"http\"",
                self.host
            )));
        }
        let mut ev = resp.events();
        let endpoint = loop {
            match ev.next_event() {
                Ok(Some(e)) if e.event == "endpoint" => {
                    break self.url.join(&e.data).map_err(|e| Fail::Other(format!("{}'s endpoint event: {}", self.host, e)))?;
                }
                Ok(Some(_)) => continue,
                Ok(None) => return Err(Fail::Other(format!("{} closed the event stream before its endpoint event", self.host))),
                Err(_) => return Err(Fail::Other(format!("{} sent no endpoint event within {}s", self.host, timeout.as_secs()))),
            }
        };
        if endpoint.origin() != self.url.origin() {
            return Err(Fail::Other(format!("{}'s endpoint is on another origin ({}): refused", self.host, endpoint.shown())));
        }
        ev.set_timeout(None);
        *self.stream.lock().unwrap_or_else(|e| e.into_inner()) = ev.closer();
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.endpoint.lock().unwrap_or_else(|e| e.into_inner()) = Some((endpoint, gen));
        let me = self.clone();
        std::thread::spawn(move || {
            while let Ok(Some(e)) = ev.next_event() {
                if e.event != "message" {
                    continue;
                }
                let Ok(m) = serde_json::from_str::<Value>(&e.data) else { continue };
                let all = if m.is_array() { m.as_array().cloned().unwrap_or_default() } else { vec![m] };
                for m in all {
                    let ours = m.get("method").is_none().then(|| m.get("id").and_then(Value::as_u64)).flatten();
                    if let Some(id) = ours {
                        if let Some(tx) = me.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                            let _ = tx.send(m);
                        }
                    } else if let Some(reply) = incoming(&m, &me.on_change) {
                        let me = me.clone();
                        std::thread::spawn(move || {
                            let _ = me.post(&reply, Duration::from_secs(30));
                        });
                    }
                }
            }
            // the stream is gone: the next request reconnects
            let mut ep = me.endpoint.lock().unwrap_or_else(|e| e.into_inner());
            if ep.as_ref().is_some_and(|(_, g)| *g == gen) {
                *ep = None;
            }
            drop(ep);
            for (_, tx) in me.pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
                let _ = tx.send(json!({"error": {"code": -32000, "message": format!("the event stream from {} dropped", me.host)}}));
            }
        });
        let init = result_of(self.rpc("initialize", init_params(), timeout)?)?;
        self.post(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), timeout)?;
        *self.init.lock().unwrap_or_else(|e| e.into_inner()) = init.clone();
        Ok(init)
    }

    fn post(&self, msg: &Value, timeout: Duration) -> Result<(), Fail> {
        let Some((endpoint, _)) = self.endpoint.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Err(Fail::Connect(format!("no event stream from {}", self.host)));
        };
        let mut headers = with_auth(&self.headers, &self.auth);
        headers.push(("Content-Type".into(), "application/json".into()));
        let body = msg.to_string();
        let resp = send("POST", &endpoint, &headers, body.as_bytes(), timeout)?;
        match resp.status {
            200..=299 => Ok(()),
            404 => Err(Fail::SessionGone),
            s => Err(status_fail(s, &self.host, resp)),
        }
    }

    fn rpc(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Err(e) = self.post(&msg, timeout) {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(e);
        }
        let got = rx.recv_timeout(timeout);
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        got.map_err(|_| Fail::Other(format!("{} did not answer {} within {}s", self.host, method, timeout.as_secs())))
    }

    fn ensure(self: &Arc<Self>, timeout: Duration) -> Result<(), Fail> {
        let _one = self.connecting.lock().unwrap_or_else(|e| e.into_inner());
        if self.endpoint.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            self.connect(timeout)?;
        }
        Ok(())
    }

    fn reset(&self) {
        *self.endpoint.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if let Some(s) = self.stream.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    fn request_raw(self: &Arc<Self>, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        self.ensure(timeout)?;
        match self.rpc(method, params.clone(), timeout) {
            Err(Fail::Connect(_)) | Err(Fail::SessionGone) => {
                self.reset();
                std::thread::sleep(Duration::from_millis(300));
                self.ensure(timeout)?;
                self.rpc(method, params, timeout)
            }
            r => r,
        }
    }

    fn close(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.reset();
    }
}

// ---- the client ----

enum Kind {
    Streamable(Arc<Streamable>),
    Sse(Arc<Sse>),
}

/// One remote MCP server, connected.
pub struct Remote {
    kind: Kind,
    auth: Option<Arc<Auth>>,
    /// the URL's host, for messages
    pub host: String,
}

impl Remote {
    /// Connect and run the handshake (`initialize`, `initialized`).
    pub fn start(s: &HttpServer, env: &dyn Fn(&str) -> Option<String>, on_change: OnChange, timeout: Duration) -> Result<Remote, Fail> {
        Remote::start_with(s, env, None, on_change, timeout)
    }

    /// The same, with the login's tokens: `secrets` is the OAuth store;
    /// None, or an `Authorization` header in mcp.json: no login.
    pub fn start_with(
        s: &HttpServer,
        env: &dyn Fn(&str) -> Option<String>,
        secrets: Option<&std::path::Path>,
        on_change: OnChange,
        timeout: Duration,
    ) -> Result<Remote, Fail> {
        let (url, headers) = target(s, env).map_err(Fail::Other)?;
        let auth = secrets
            .filter(|_| s.may_login())
            .map(|d| Arc::new(Auth::new(d.to_path_buf(), &url, s.oauth.clone().unwrap_or_default())));
        Remote::connect(s.transport, url, headers, auth, on_change, timeout)
    }

    /// The same with the URL and headers already filled in.
    pub fn connect(
        transport: Transport,
        url: Url,
        headers: Vec<(String, String)>,
        auth: Option<Arc<Auth>>,
        on_change: OnChange,
        timeout: Duration,
    ) -> Result<Remote, Fail> {
        let host = url.shown();
        let kind = match transport {
            Transport::Streamable => {
                let c = Arc::new(Streamable {
                    url,
                    headers,
                    auth: auth.clone(),
                    host: host.clone(),
                    session: Mutex::new(None),
                    protocol: Mutex::new(None),
                    next: AtomicU64::new(1),
                    on_change,
                    init: Mutex::new(Value::Null),
                    stop: AtomicBool::new(false),
                    listening: Mutex::new(None),
                });
                let init = match retry_401(&auth, || c.handshake(timeout)) {
                    Err(Fail::Connect(_)) => {
                        std::thread::sleep(Duration::from_millis(300));
                        retry_401(&auth, || c.handshake(timeout))?
                    }
                    r => r?,
                };
                if init.pointer("/capabilities/tools/listChanged") == Some(&json!(true)) {
                    let l = c.clone();
                    std::thread::spawn(move || l.listen());
                }
                Kind::Streamable(c)
            }
            Transport::Sse => {
                let c = Arc::new(Sse {
                    url,
                    headers,
                    auth: auth.clone(),
                    host: host.clone(),
                    endpoint: Mutex::new(None),
                    generation: AtomicU64::new(0),
                    pending: Mutex::new(HashMap::new()),
                    next: AtomicU64::new(1),
                    on_change,
                    init: Mutex::new(Value::Null),
                    stop: AtomicBool::new(false),
                    stream: Mutex::new(None),
                    connecting: Mutex::new(()),
                });
                retry_401(&auth, || c.ensure(timeout))?;
                Kind::Sse(c)
            }
        };
        Ok(Remote { kind, host, auth })
    }

    /// The server's `initialize` result (the latest handshake's).
    pub fn init(&self) -> Value {
        match &self.kind {
            Kind::Streamable(c) => c.init.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            Kind::Sse(c) => c.init.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        }
    }

    /// One request; the whole response message (`result` or `error`).
    pub fn request_raw(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        retry_401(&self.auth, || match &self.kind {
            Kind::Streamable(c) => c.request_raw(method, params.clone(), timeout),
            Kind::Sse(c) => c.request_raw(method, params.clone(), timeout),
        })
    }

    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Fail> {
        result_of(self.request_raw(method, params, timeout)?)
    }

    /// Every tool, following `nextCursor`.
    pub fn list_tools(&self, timeout: Duration) -> Result<Vec<Value>, Fail> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let params = match &cursor {
                Some(c) => json!({"cursor": c}),
                None => json!({}),
            };
            let r = self.request("tools/list", params, timeout)?;
            if let Some(ts) = r.get("tools").and_then(Value::as_array) {
                out.extend(ts.iter().cloned());
            }
            cursor = r.get("nextCursor").and_then(Value::as_str).filter(|c| !c.is_empty()).map(String::from);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    pub fn stop(&mut self) {
        match &self.kind {
            Kind::Streamable(c) => c.close(),
            Kind::Sse(c) => c.close(),
        }
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        match &self.kind {
            Kind::Streamable(c) => c.stop.store(true, Ordering::SeqCst),
            Kind::Sse(c) => c.close(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_fill_env_and_never_say_a_value() {
        let s = HttpServer {
            id: "x".into(),
            transport: Transport::Streamable,
            url: "https://${HOST:-mcp.example.com}/mcp".into(),
            headers: vec![("Authorization".into(), "Bearer ${TOKEN}".into()), ("X-Org".into(), "${ORG:-acme}".into())],
            oauth: None,
            limits: Default::default(),
        };
        let env = |k: &str| (k == "TOKEN").then(|| "s3cret".to_string());
        let (u, h) = target(&s, &env).unwrap();
        assert_eq!(u.host, "mcp.example.com");
        assert_eq!(h, vec![("Authorization".into(), "Bearer s3cret".into()), ("X-Org".into(), "acme".into())]);
        let none = |_: &str| None;
        let e = target(&s, &none).unwrap_err();
        assert_eq!(e, "header Authorization uses ${TOKEN}, which is not set");
        assert!(!format!("{:?}", s).contains("TOKEN"), "Debug shows header names only");
    }

    #[test]
    fn server_messages_are_answered_or_noticed() {
        let hit = Arc::new(AtomicBool::new(false));
        let h = hit.clone();
        let on: OnChange = Arc::new(move || h.store(true, Ordering::SeqCst));
        assert_eq!(incoming(&json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}), &on), None);
        assert!(hit.load(Ordering::SeqCst));
        assert_eq!(incoming(&json!({"jsonrpc": "2.0", "id": 4, "method": "ping"}), &on).unwrap()["result"], json!({}));
        assert!(incoming(&json!({"jsonrpc": "2.0", "id": 5, "method": "sampling/createMessage"}), &on).unwrap()["error"].is_object());
        assert!(answers(&json!({"id": 3, "result": {}}), 3) && !answers(&json!({"id": 3, "method": "x"}), 3));
    }
}
