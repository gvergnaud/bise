//! OAuth for remote MCP servers (the MCP authorization spec, 2025-06-18):
//! a server that answers 401 gets a browser login, and its tokens are
//! sent as `Authorization: Bearer` from then on.
//!
//! - **Discovery.** The 401's `WWW-Authenticate: Bearer
//!   resource_metadata="…"`, else `/.well-known/oauth-protected-resource`
//!   (with the URL's path, then without) gives the authorization server;
//!   its metadata comes from `/.well-known/oauth-authorization-server`
//!   or `openid-configuration` (RFC 8414 path rules). An older server with
//!   neither: its origin's `/authorize`, `/token`, `/register`.
//! - **Client.** `"oauth": {"clientId"}` in mcp.json (Claude Code's key;
//!   GitHub and Slack need a registered app), else the client registered
//!   before for this server, else dynamic client registration (RFC 7591)
//!   as a public client (`token_endpoint_auth_method: none`).
//! - **Login.** PKCE S256, a random `state`, `resource` = the server's
//!   URL (RFC 8707), the scopes the metadata lists (or mcp.json's), the
//!   redirect to `http://127.0.0.1:<port>/callback` (the port of the
//!   registration, so it is reused), the browser opened, 5 minutes to
//!   finish, the code exchanged.
//! - **Store.** One file per server URL in `<bise home>/secrets/mcp-oauth/`
//!   (`$BEND_MCP_SECRETS`), the folder 0700, the files 0600, written by
//!   rename. Never printed, never in a session, a report or /log.
//! - **Refresh.** A token that expires within a minute, or a 401 with a
//!   token, is refreshed under the file's lock (every agent's bridge
//!   shares the store: one refreshes, the others read its result; the
//!   refresh token rotates). A refused `resource` on refresh is tried
//!   once without it. A refresh refused for good drops the tokens (the
//!   client stays): the server needs a login again.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Map, Value};

use crate::http::{self, Url};

/// mcp.json's `"oauth"` of a remote server (Claude Code's keys).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub client_id: Option<String>,
    /// never printed; `${VAR}` allowed
    pub client_secret: Option<String>,
    pub scopes: Option<String>,
    pub callback_port: Option<u16>,
}

impl Config {
    pub fn parse(v: &Value) -> Result<Config, String> {
        let o = v.as_object().ok_or("\"oauth\" must be an object")?;
        for k in o.keys() {
            if !["clientId", "clientSecret", "scopes", "callbackPort"].contains(&k.as_str()) {
                return Err(format!("unknown field oauth.{}", k));
            }
        }
        let s = |k: &str| -> Result<Option<String>, String> {
            match o.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
                _ => Err(format!("oauth.{} must be a non-empty string", k)),
            }
        };
        let scopes = match o.get("scopes") {
            Some(Value::Array(a)) => Some(
                a.iter().map(|x| x.as_str().map(String::from)).collect::<Option<Vec<_>>>().ok_or("oauth.scopes must be strings")?.join(" "),
            ),
            _ => s("scopes")?,
        };
        let callback_port = match o.get("callbackPort") {
            None | Some(Value::Null) => None,
            Some(p) => Some(p.as_u64().filter(|p| (1..=65535).contains(p)).ok_or("oauth.callbackPort must be a port number")? as u16),
        };
        Ok(Config { client_id: s("clientId")?, client_secret: s("clientSecret")?, scopes, callback_port })
    }
}

// ---- the store ----

/// `$BEND_MCP_SECRETS`, else `<bise home>/secrets/mcp-oauth`.
pub fn store_dir() -> PathBuf {
    std::env::var("BEND_MCP_SECRETS")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| bise_home::Home::from_env().root().join("secrets").join("mcp-oauth"))
}

/// The canonical server URL: the resource the tokens are for (no
/// fragment, lowercase scheme and host, no default port).
pub fn resource_of(u: &Url) -> String {
    let mut c = u.clone();
    c.host = c.host.to_ascii_lowercase();
    c.to_url()
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// The file of one server's tokens.
pub fn file_of(dir: &Path, resource: &str) -> PathBuf {
    let host: String = resource
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    dir.join(format!("{}-{:016x}.json", host, fnv(resource)))
}

/// What the store keeps for one server.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub resource: String,
    pub issuer: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    /// the loopback port the client was registered with
    pub redirect_port: Option<u16>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    /// unix seconds
    pub expires_at: Option<u64>,
    pub scope: Option<String>,
}

impl std::fmt::Debug for Saved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Saved {{ resource: {:?}, issuer: {:?}, token: {} }}", self.resource, self.issuer, self.access_token.is_some())
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Saved {
    fn to_json(&self) -> Value {
        let mut o = Map::new();
        let mut put = |k: &str, v: Value| {
            if !v.is_null() {
                o.insert(k.into(), v);
            }
        };
        put("resource", json!(self.resource));
        put("issuer", json!(self.issuer));
        put("token_endpoint", json!(self.token_endpoint));
        put("client_id", json!(self.client_id));
        put("client_secret", json!(self.client_secret));
        put("redirect_port", json!(self.redirect_port));
        put("access_token", json!(self.access_token));
        put("refresh_token", json!(self.refresh_token));
        put("expires_at", json!(self.expires_at));
        put("scope", json!(self.scope));
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Option<Saved> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        Some(Saved {
            resource: s("resource")?,
            issuer: s("issuer").unwrap_or_default(),
            token_endpoint: s("token_endpoint").unwrap_or_default(),
            client_id: s("client_id").unwrap_or_default(),
            client_secret: s("client_secret"),
            redirect_port: v.get("redirect_port").and_then(Value::as_u64).map(|p| p as u16),
            access_token: s("access_token"),
            refresh_token: s("refresh_token"),
            expires_at: v.get("expires_at").and_then(Value::as_u64),
            scope: s("scope"),
        })
    }

    /// A token that is still good for a minute.
    pub fn fresh(&self) -> bool {
        self.access_token.is_some() && self.expires_at.is_none_or(|e| e > now() + 60)
    }
}

pub fn load(dir: &Path, resource: &str) -> Option<Saved> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file_of(dir, resource)).ok()?).ok()?;
    Saved::from_json(&v).filter(|s| s.resource == resource)
}

/// Write by rename, 0600, in a 0700 folder.
pub fn save(dir: &Path, s: &Saved) -> Result<(), String> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create bise's secrets folder: {}", e))?;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    let f = file_of(dir, &s.resource);
    let tmp = f.with_extension(format!("tmp{}", std::process::id()));
    let res = (|| {
        let mut w = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        w.write_all(s.to_json().to_string().as_bytes())?;
        w.sync_all()?;
        std::fs::rename(&tmp, &f)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot save the login: {}", e)
    })
}

/// Forget a server's tokens (`keep_client`: its registration stays).
pub fn forget(dir: &Path, resource: &str, keep_client: bool) {
    match load(dir, resource) {
        Some(mut s) if keep_client => {
            s.access_token = None;
            s.refresh_token = None;
            s.expires_at = None;
            let _ = save(dir, &s);
        }
        _ => {
            let _ = std::fs::remove_file(file_of(dir, resource));
        }
    }
}

/// The store's lock for one server (every bridge of every agent shares
/// it): held while a token is refreshed.
struct Lock(std::fs::File);

fn lock(dir: &Path, resource: &str) -> Option<Lock> {
    std::fs::create_dir_all(dir).ok()?;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(file_of(dir, resource).with_extension("lock")).ok()?;
    f.lock().ok()?;
    Some(Lock(f))
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

// ---- small HTTP helpers ----

fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(x) => {
                        out.push(x);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            x => out.push(x),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a=1&b=2` -> pairs, decoded.
pub fn query(q: &str) -> Vec<(String, String)> {
    q.split('&').filter(|p| !p.is_empty()).map(|p| p.split_once('=').unwrap_or((p, ""))).map(|(k, v)| (dec(k), dec(v))).collect()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

const T: Duration = Duration::from_secs(15);

fn get_json(url: &str) -> Result<Option<Value>, String> {
    let u = Url::parse(url)?;
    let h = vec![("Accept".to_string(), "application/json".to_string())];
    let r = http::send(&http::Request { method: "GET", url: &u, headers: &h, body: b"", timeout: T }).map_err(|e| e.to_string())?;
    if !(200..300).contains(&r.status) {
        return Ok(None);
    }
    let b = r.read_all(1 << 20).map_err(|e| e.to_string())?;
    Ok(serde_json::from_slice(&b).ok().filter(Value::is_object))
}

fn post(url: &str, ctype: &str, body: &str) -> Result<(u16, Value), String> {
    let u = Url::parse(url)?;
    let h = vec![("Content-Type".to_string(), ctype.to_string()), ("Accept".to_string(), "application/json".to_string())];
    let r = http::send(&http::Request { method: "POST", url: &u, headers: &h, body: body.as_bytes(), timeout: T }).map_err(|e| e.to_string())?;
    let status = r.status;
    let b = r.read_all(1 << 20).map_err(|e| e.to_string())?;
    let v = serde_json::from_slice(&b).unwrap_or_else(|_| json!({"error": http::short_body(&b)}));
    Ok((status, v))
}

fn oauth_error(v: &Value) -> String {
    let e = v.get("error").and_then(Value::as_str).unwrap_or("");
    let d = v.get("error_description").and_then(Value::as_str).unwrap_or("");
    match (e.is_empty(), d.is_empty()) {
        (false, false) => format!("{} ({})", d, e),
        (false, true) => e.to_string(),
        (true, false) => d.to_string(),
        _ => "no reason given".into(),
    }
}

// ---- discovery ----

/// Where to log in, found from the server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    /// space-separated
    pub scopes: Option<String>,
}

/// One parameter of a `WWW-Authenticate: Bearer k="v", …` challenge.
pub fn challenge_param(challenge: &str, key: &str) -> Option<String> {
    let mut rest = challenge;
    while let Some(i) = rest.find(key) {
        let before = rest[..i].chars().last();
        let after = &rest[i + key.len()..];
        if before.is_none_or(|c| c == ' ' || c == ',') {
            if let Some(v) = after.trim_start().strip_prefix('=') {
                let v = v.trim_start();
                return Some(match v.strip_prefix('"') {
                    Some(q) => q.split('"').next().unwrap_or("").to_string(),
                    None => v.split([',', ' ']).next().unwrap_or("").to_string(),
                });
            }
        }
        rest = after;
    }
    None
}

/// RFC 8414 §3.1 / OIDC: the metadata URLs to try for an issuer.
fn as_metadata_urls(issuer: &Url) -> Vec<String> {
    let o = issuer.origin();
    let p = issuer.path.split('?').next().unwrap_or("/").trim_end_matches('/');
    if p.is_empty() {
        vec![format!("{}/.well-known/oauth-authorization-server", o), format!("{}/.well-known/openid-configuration", o)]
    } else {
        vec![
            format!("{}/.well-known/oauth-authorization-server{}", o, p),
            format!("{}/.well-known/openid-configuration{}", o, p),
            format!("{}{}/.well-known/openid-configuration", o, p),
        ]
    }
}

/// Discovery from the server's URL and its 401 challenge.
pub fn discover(server: &Url, challenge: Option<&str>) -> Result<Meta, String> {
    let host = server.shown();
    let mut prm_urls = Vec::new();
    if let Some(u) = challenge.and_then(|c| challenge_param(c, "resource_metadata")) {
        prm_urls.push(u);
    }
    let path = server.path.split('?').next().unwrap_or("/").trim_end_matches('/');
    if !path.is_empty() {
        prm_urls.push(format!("{}/.well-known/oauth-protected-resource{}", server.origin(), path));
    }
    prm_urls.push(format!("{}/.well-known/oauth-protected-resource", server.origin()));
    let mut prm = None;
    for u in &prm_urls {
        if let Ok(Some(v)) = get_json(u) {
            prm = Some(v);
            break;
        }
    }
    let issuer = prm
        .as_ref()
        .and_then(|p| p.get("authorization_servers").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str).map(String::from))
        .unwrap_or_else(|| server.origin());
    let issuer_url = Url::parse(&issuer).map_err(|_| format!("{} names an authorization server that is not a URL", host))?;
    let mut scopes = challenge.and_then(|c| challenge_param(c, "scope")).filter(|s| !s.is_empty());
    if scopes.is_none() {
        scopes = prm.as_ref().and_then(|p| p.get("scopes_supported")).and_then(Value::as_array).map(|a| {
            a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")
        }).filter(|s| !s.is_empty());
    }
    for u in as_metadata_urls(&issuer_url) {
        let Ok(Some(m)) = get_json(&u) else { continue };
        let s = |k: &str| m.get(k).and_then(Value::as_str).map(String::from);
        let (Some(a), Some(t)) = (s("authorization_endpoint"), s("token_endpoint")) else { continue };
        if let Some(methods) = m.get("code_challenge_methods_supported").and_then(Value::as_array) {
            if !methods.iter().any(|x| x == "S256") {
                return Err(format!("{}'s login server does not support PKCE (S256): bise can't log in safely", host));
            }
        }
        return Ok(Meta { issuer: s("issuer").unwrap_or(issuer), authorization_endpoint: a, token_endpoint: t, registration_endpoint: s("registration_endpoint"), scopes });
    }
    if prm.is_none() && challenge.is_none() {
        return Err(format!("{} gives no login metadata", host));
    }
    // 2025-03-26 servers without metadata: the default paths
    let o = issuer_url.origin();
    Ok(Meta {
        issuer: o.clone(),
        authorization_endpoint: format!("{}/authorize", o),
        token_endpoint: format!("{}/token", o),
        registration_endpoint: Some(format!("{}/register", o)),
        scopes,
    })
}

// ---- login ----

fn random_b64(n: usize) -> String {
    let mut b = vec![0u8; n];
    let ok = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b).is_ok();
    if !ok {
        let _ = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b));
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&b)
}

fn challenge_of(verifier: &str) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(d.as_ref())
}

/// What a login needs besides the server.
pub struct Login<'a> {
    /// the server's URL, `${VAR}` filled in
    pub server: &'a Url,
    /// for messages: the server's name in its plugin (`linear`)
    pub name: &'a str,
    pub config: &'a Config,
    pub dir: &'a Path,
    /// the 401's `WWW-Authenticate`, when there was one
    pub challenge: Option<&'a str>,
    /// opens the URL in the user's browser
    pub open: &'a dyn Fn(&str) -> Result<(), String>,
    /// how long the user has to finish (5 min)
    pub wait: Duration,
    /// set to true to give up early
    pub cancel: Option<&'a std::sync::atomic::AtomicBool>,
}

/// The user's browser: `open` on macOS, `xdg-open` elsewhere. Never
/// waited for: an opener that only returns once the page loaded would
/// hold the login before it listens for the redirect. A quick failure
/// (within a second) is an error; the rest is reaped in the background.
pub fn open_browser(url: &str) -> Result<(), String> {
    let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut child = std::process::Command::new(cmd)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot open the browser: {}", e))?;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(1) {
        match child.try_wait() {
            Ok(Some(s)) if !s.success() => return Err("cannot open the browser".into()),
            Ok(Some(_)) => return Ok(()),
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn register(meta: &Meta, redirect: &str, name: &str) -> Result<(String, Option<String>), String> {
    let Some(reg) = &meta.registration_endpoint else {
        return Err(format!("{}'s login server takes no app registration: add \"oauth\": {{\"clientId\": \"…\"}} to its mcp.json entry, or a token in \"headers\"", name));
    };
    let body = json!({
        "client_name": "bise",
        "redirect_uris": [redirect],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let (status, v) = post(reg, "application/json", &body.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("the app registration was refused: {}", oauth_error(&v)));
    }
    let id = v.get("client_id").and_then(Value::as_str).ok_or("the app registration gave no client_id")?;
    Ok((id.to_string(), v.get("client_secret").and_then(Value::as_str).map(String::from)))
}

/// The page the browser shows after the redirect.
pub fn page(ok: bool, name: &str, reason: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = if ok {
        format!("<p>bise <b>:*</b> is logged in to {}.</p><p class=d>you can close this tab.</p>", esc(name))
    } else {
        format!("<p>the login didn't go through: {}.</p><p class=d>/plugins login in bise tries again.</p>", esc(reason))
    };
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>bise</title>\
<style>:root{{color-scheme:light dark}}body{{margin:0;min-height:100vh;display:grid;place-items:center;\
font:16px/1.6 ui-monospace,SFMono-Regular,Menlo,monospace;background:#f4efe6;color:#1d1b19}}\
b{{color:#b8416b;font-weight:600}}.d{{opacity:.6}}main{{max-width:34em;padding:2em}}\
@media(prefers-color-scheme:dark){{body{{background:#141312;color:#ece6dc}}b{{color:#f4a6b0}}}}</style><main>{}</main>",
        body
    )
}

fn respond(s: &mut TcpStream, status: &str, html: &str) {
    let _ = write!(s, "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, html.len(), html);
    let _ = s.flush();
}

/// Wait for the browser's redirect: the code, or why not.
fn wait_callback(l: &TcpListener, state: &str, name: &str, wait: Duration, cancel: Option<&std::sync::atomic::AtomicBool>) -> Result<String, String> {
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    loop {
        if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::SeqCst)) {
            return Err("the login was cancelled".into());
        }
        if t0.elapsed() > wait {
            return Err(format!("the browser login wasn't finished in {} min", wait.as_secs().div_ceil(60)));
        }
        let (mut s, _) = match l.accept() {
            Ok(x) => x,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let _ = s.set_nonblocking(false);
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        let mut r = BufReader::new(match s.try_clone() {
            Ok(c) => c,
            Err(_) => continue,
        });
        if r.read_line(&mut line).is_err() {
            continue;
        }
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let (path, q) = target.split_once('?').unwrap_or((target, ""));
        if path != "/callback" {
            respond(&mut s, "404 Not Found", "");
            continue;
        }
        let q = query(q);
        let get = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        if get("state").as_deref() != Some(state) {
            respond(&mut s, "400 Bad Request", &page(false, name, "the answer was not for this login"));
            continue;
        }
        if let Some(e) = get("error") {
            let why = get("error_description").filter(|d| !d.is_empty()).unwrap_or(e);
            let why = if why == "access_denied" { "access was denied".to_string() } else { why };
            respond(&mut s, "200 OK", &page(false, name, &why));
            return Err(why);
        }
        match get("code") {
            Some(c) if !c.is_empty() => {
                respond(&mut s, "200 OK", &page(true, name, ""));
                return Ok(c);
            }
            _ => {
                respond(&mut s, "400 Bad Request", &page(false, name, "no code in the answer"));
                return Err("the login server sent no code".into());
            }
        }
    }
}

fn store_tokens(saved: &mut Saved, v: &Value) -> Result<(), String> {
    let at = v.get("access_token").and_then(Value::as_str).ok_or("the login server sent no access token")?;
    saved.access_token = Some(at.to_string());
    if let Some(rt) = v.get("refresh_token").and_then(Value::as_str) {
        saved.refresh_token = Some(rt.to_string());
    }
    saved.expires_at = v.get("expires_in").and_then(Value::as_u64).map(|s| now() + s);
    if let Some(sc) = v.get("scope").and_then(Value::as_str) {
        saved.scope = Some(sc.to_string());
    }
    Ok(())
}

/// The whole browser login; the tokens are saved. Errors are one short
/// reason (the caller says "couldn't log in to <name>: <reason>").
pub fn login(l: &Login<'_>) -> Result<Saved, String> {
    let resource = resource_of(l.server);
    let meta = discover(l.server, l.challenge)?;
    let before = load(l.dir, &resource).filter(|s| s.issuer == meta.issuer);
    // the registered port first, so the registration still matches
    let want = l.config.callback_port.or_else(|| before.as_ref().filter(|_| l.config.client_id.is_none()).and_then(|s| s.redirect_port));
    let listener = match want.map(|p| TcpListener::bind(("127.0.0.1", p))) {
        Some(Ok(x)) => x,
        Some(Err(_)) if l.config.callback_port.is_some() => {
            return Err(format!("port {} (oauth.callbackPort) is busy", l.config.callback_port.unwrap_or(0)))
        }
        _ => TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("cannot listen on 127.0.0.1: {}", e))?,
    };
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{}/callback", port);
    let (client_id, client_secret) = match (&l.config.client_id, &before) {
        (Some(id), _) => (id.clone(), l.config.client_secret.clone()),
        (None, Some(b)) if !b.client_id.is_empty() && b.redirect_port == Some(port) => (b.client_id.clone(), b.client_secret.clone()),
        _ => register(&meta, &redirect, l.name)?,
    };
    let verifier = random_b64(48);
    let state = random_b64(24);
    let mut q = vec![
        ("response_type", "code".to_string()),
        ("client_id", client_id.clone()),
        ("redirect_uri", redirect.clone()),
        ("code_challenge", challenge_of(&verifier)),
        ("code_challenge_method", "S256".to_string()),
        ("state", state.clone()),
        ("resource", resource.clone()),
    ];
    if let Some(s) = l.config.scopes.clone().or_else(|| meta.scopes.clone()) {
        q.push(("scope", s));
    }
    let qs: Vec<(&str, &str)> = q.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let sep = if meta.authorization_endpoint.contains('?') { '&' } else { '?' };
    let auth_url = format!("{}{}{}", meta.authorization_endpoint, sep, form(&qs));
    (l.open)(&auth_url)?;
    let code = wait_callback(&listener, &state, l.name, l.wait, l.cancel)?;
    let mut pairs = vec![
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("client_id", client_id.as_str()),
        ("code_verifier", verifier.as_str()),
        ("resource", resource.as_str()),
    ];
    if let Some(s) = &client_secret {
        pairs.push(("client_secret", s.as_str()));
    }
    let (status, v) = post(&meta.token_endpoint, "application/x-www-form-urlencoded", &form(&pairs))?;
    if !(200..300).contains(&status) {
        return Err(format!("the login server refused the code: {}", oauth_error(&v)));
    }
    let mut saved = Saved {
        resource: resource.clone(),
        issuer: meta.issuer.clone(),
        token_endpoint: meta.token_endpoint.clone(),
        client_id,
        client_secret: client_secret.filter(|_| l.config.client_id.is_none()),
        redirect_port: Some(port),
        ..Saved::default()
    };
    store_tokens(&mut saved, &v)?;
    save(l.dir, &saved)?;
    Ok(saved)
}

/// One refresh: the new tokens, or why not (`fatal`: the refresh token
/// is no good, a login is needed).
fn refresh(s: &Saved, config: &Config) -> Result<Saved, (bool, String)> {
    let Some(rt) = s.refresh_token.clone() else { return Err((true, "no refresh token".into())) };
    let secret = config.client_secret.clone().or_else(|| s.client_secret.clone());
    let attempt = |with_resource: bool| {
        let mut pairs = vec![("grant_type", "refresh_token"), ("refresh_token", rt.as_str()), ("client_id", s.client_id.as_str())];
        if with_resource {
            pairs.push(("resource", s.resource.as_str()));
        }
        if let Some(x) = &secret {
            pairs.push(("client_secret", x.as_str()));
        }
        post(&s.token_endpoint, "application/x-www-form-urlencoded", &form(&pairs))
    };
    let (mut status, mut v) = attempt(true).map_err(|e| (false, e))?;
    if !(200..300).contains(&status) && v.get("error").and_then(Value::as_str) != Some("invalid_grant") {
        // some servers refuse `resource` on refresh
        (status, v) = attempt(false).map_err(|e| (false, e))?;
    }
    if !(200..300).contains(&status) {
        let fatal = (400..500).contains(&status);
        return Err((fatal, oauth_error(&v)));
    }
    let mut n = s.clone();
    store_tokens(&mut n, &v).map_err(|e| (false, e))?;
    Ok(n)
}

/// A remote server's tokens, as its client uses them.
pub struct Auth {
    dir: PathBuf,
    resource: String,
    config: Config,
    /// the token last sent
    used: Mutex<Option<String>>,
}

impl Auth {
    pub fn new(dir: PathBuf, server: &Url, config: Config) -> Auth {
        Auth { dir, resource: resource_of(server), config, used: Mutex::new(None) }
    }

    /// The bearer token to send now (refreshed when it is about to
    /// expire); None: no login yet.
    pub fn bearer(&self) -> Option<String> {
        let s = load(&self.dir, &self.resource)?;
        let tok = if s.fresh() || s.refresh_token.is_none() {
            s.access_token.clone()
        } else {
            self.refreshed(s.access_token.as_deref()).ok().flatten()
        };
        *self.used.lock().unwrap_or_else(|e| e.into_inner()) = tok.clone();
        tok
    }

    /// The server answered 401: a newer token (another bridge refreshed
    /// it, or a refresh now). False: a login is needed.
    pub fn after_401(&self) -> bool {
        let used = self.used.lock().unwrap_or_else(|e| e.into_inner()).clone();
        matches!(self.refreshed(used.as_deref()), Ok(Some(t)) if Some(&t) != used.as_ref())
    }

    /// Under the lock: the store's token when another process already
    /// replaced `stale`, else a refresh.
    fn refreshed(&self, stale: Option<&str>) -> Result<Option<String>, String> {
        let _l = lock(&self.dir, &self.resource);
        let Some(s) = load(&self.dir, &self.resource) else { return Ok(None) };
        if s.access_token.is_some() && s.access_token.as_deref() != stale && s.fresh() {
            return Ok(s.access_token);
        }
        match refresh(&s, &self.config) {
            Ok(n) => {
                save(&self.dir, &n)?;
                Ok(n.access_token)
            }
            Err((true, _)) => {
                forget(&self.dir, &self.resource, true);
                Ok(None)
            }
            Err((false, e)) => Err(e),
        }
    }

    /// The store file (its changes mean a login happened).
    pub fn file(&self) -> PathBuf {
        file_of(&self.dir, &self.resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenges_queries_and_pkce() {
        let c = r#"Bearer error="invalid_token", resource_metadata="https://x.test/.well-known/oauth-protected-resource/mcp", scope="read write""#;
        assert_eq!(challenge_param(c, "resource_metadata").as_deref(), Some("https://x.test/.well-known/oauth-protected-resource/mcp"));
        assert_eq!(challenge_param(c, "scope").as_deref(), Some("read write"));
        assert_eq!(challenge_param("Bearer realm=x", "resource_metadata"), None);
        assert_eq!(query("code=a%2Fb+c&state=s"), vec![("code".into(), "a/b c".into()), ("state".into(), "s".into())]);
        assert_eq!(query("x=%zz%"), vec![("x".into(), "%zz%".into())]);
        assert_eq!(form(&[("redirect_uri", "http://127.0.0.1:5/cb")]), "redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2Fcb");
        // base64url(sha256(verifier)), no padding (as python's hashlib computes it)
        assert_eq!(challenge_of("dBjftJeZ4CVP-mJ92K9fGBzlDeQOeAy1b4ltO9WjXkE"), "GqV8OYYYVlzY-omeJL1CJG8siDdyIPnP0ZUPp74J0rE");
        assert_ne!(random_b64(32), random_b64(32));
    }

    #[test]
    fn metadata_urls_follow_rfc8414() {
        let u = Url::parse("https://auth.x.test/tenant1").unwrap();
        assert_eq!(as_metadata_urls(&u)[0], "https://auth.x.test/.well-known/oauth-authorization-server/tenant1");
        assert_eq!(as_metadata_urls(&Url::parse("https://a.test").unwrap())[1], "https://a.test/.well-known/openid-configuration");
    }

    #[test]
    fn the_store_is_private_and_forgets_tokens_but_keeps_the_client() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("bp-oauth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = Saved { resource: "https://mcp.x.test/mcp".into(), client_id: "c1".into(), access_token: Some("at".into()), refresh_token: Some("rt".into()), expires_at: Some(now() + 3600), ..Saved::default() };
        save(&d, &s).unwrap();
        let f = file_of(&d, &s.resource);
        assert!(f.file_name().unwrap().to_str().unwrap().starts_with("mcp.x.test-"));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&d).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(load(&d, &s.resource), Some(s.clone()));
        assert!(load(&d, &s.resource).unwrap().fresh());
        assert!(!format!("{:?}", s).contains("at\""), "Debug never shows a token");
        forget(&d, &s.resource, true);
        let k = load(&d, &s.resource).unwrap();
        assert_eq!((k.client_id.as_str(), k.access_token.clone()), ("c1", None));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn oauth_config_parses_claude_codes_keys() {
        let c = Config::parse(&json!({"clientId": "abc", "callbackPort": 8765, "scopes": ["a", "b"]})).unwrap();
        assert_eq!((c.client_id.as_deref(), c.callback_port, c.scopes.as_deref()), (Some("abc"), Some(8765), Some("a b")));
        assert!(Config::parse(&json!({"clientID": "x"})).is_err());
        assert!(Config::parse(&json!({"callbackPort": 0})).is_err());
    }

    #[test]
    fn the_pages_say_the_designers_words() {
        assert!(page(true, "linear", "").contains("bise <b>:*</b> is logged in to linear."));
        assert!(page(false, "linear", "access was <denied>").contains("the login didn't go through: access was &lt;denied&gt;."));
        // the TUI accent (designer): #b8416b on paper, #f4a6b0 on the dark ground
        let p = page(true, "linear", "");
        assert!(p.contains("b{color:#b8416b") && p.contains("b{color:#f4a6b0}"), "{}", p);
    }
}
