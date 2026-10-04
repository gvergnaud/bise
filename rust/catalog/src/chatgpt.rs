//! The ChatGPT plan's sign-in ("Sign in with ChatGPT" for open-source,
//! locally run tools: developers.openai.com/siwc/token-sharing-open-source;
//! docs/subscriptions-design.md).
//!
//! - **Host id.** `~/.bise/host-id` (`urn:uuid:<v4>`, 0600) is made once
//!   before the first sign-in and never changed: OpenAI's
//!   `ext_agent_host_id`.
//! - **Sign-in.** A loopback listener on `127.0.0.1:<any port>/auth/callback`,
//!   then the browser at `<issuer>/api/accounts/authorize` with PKCE S256,
//!   a fresh `state` and `nonce`, `resource = https://api.openai.com/v1`
//!   and the plan's scopes. The first time, `client_id =
//!   dynamic_agent_client` with `agent_name_hint = bise` registers a
//!   client and the callback gives its issued id (`oaiapp_...`); later
//!   sign-ins send that id (with `id_token_hint` / `login_hint`). The code
//!   is exchanged at the OpenID configuration's token endpoint, the ID
//!   token checked against its JWKS (RS256: iss, aud = the issued client,
//!   exp, nonce), and the granted scopes must hold
//!   `chatgpt.tokens.use.direct` (else the user did not let bise use the
//!   plan: [`Poll::Denied`], the client kept).
//! - **Tokens.** auth.json's `chatgpt` entry (`"type": "oauth"`,
//!   [`crate::auth::OAuth`]). [`access_token`] (`bise auth token
//!   chatgpt`, the runtime's key_command) gives the access token when it
//!   has 5 more minutes, else takes `~/.bise/auth.json.lock`, reads the
//!   file again (another process may have refreshed), and refreshes: the
//!   refresh token rotates, so two processes never both spend it. A
//!   refused refresh signs out with the client kept (`expired`).
//! - **Sign-out.** The refresh token is revoked at the OpenID
//!   configuration's `revocation_endpoint` (RFC 7009), then the tokens are
//!   dropped, the client kept.
//! - **Models.** `GET <base_url>/models` with the token: the account's
//!   list (`visibility == "list"`, the server's order), cached in
//!   `~/.bise/cache/chatgpt-models.json`.
//!
//! No token is ever printed (but by [`access_token`]'s caller, to the
//! runtime), put in an argument, the environment or a log line; errors
//! are one short line without one. `BISE_CHATGPT_ISSUER` moves the issuer
//! (tests: a fake server on 127.0.0.1).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use bend_plugins::http::{self, Url};
use bend_plugins::oauth::{challenge_of, form, get_json, post, query, random_b64};
use serde_json::{json, Value};

use crate::auth::{OAuth, Store};
use crate::auth_cli::Paths;

/// The provider id of the ChatGPT plan.
pub const ID: &str = "chatgpt";
/// `BISE_CHATGPT_ISSUER`: where the sign-in server is (tests).
pub const ISSUER_ENV: &str = "BISE_CHATGPT_ISSUER";
pub const DEFAULT_ISSUER: &str = "https://auth.openai.com";
/// `BISE_CHATGPT_SEND_HOST_ID=on`: send the host id (`ext_agent_host_id`)
/// in the authorization request. Off by default, like OpenAI's own
/// sign-in devkit (`sendHostId: false`, "enable only when the
/// authorization provider supports ext_agent_host_id"): a sign-in that
/// sent it was refused at the code exchange (`invalid_grant`, v2026.10.2-15).
pub const HOST_ID_ENV: &str = "BISE_CHATGPT_SEND_HOST_ID";
/// The `resource` of every token request (the public API).
pub const RESOURCE: &str = "https://api.openai.com/v1";
/// Identity scopes, then the plan's.
pub const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// The scope that lets bise use the ChatGPT plan.
pub const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
/// The first-time registration entry point (never saved).
pub const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
/// The callback path (fixed; only the port may vary).
pub const CALLBACK_PATH: &str = "/auth/callback";
/// How long the browser has to come back.
pub const WAIT: Duration = Duration::from_secs(300);
/// An access token this close to its end is refreshed first.
const MIN_LEFT_MS: u64 = 5 * 60 * 1000;
/// A refresh token lasts 30 days from when it was given.
pub const REFRESH_DAYS: u64 = 30;
/// The model list cache is fresh for a day.
const MODELS_TTL: u64 = 24 * 3600;
/// Where the plan's usage and the connected apps are.
pub const USAGE_URL: &str = "https://chatgpt.com/settings/usage";

// ---- where things are ----

/// What the sign-in reads and writes. [`Ctx::of`] builds it from the
/// usual paths and the environment; tests build their own.
#[derive(Clone, Debug)]
pub struct Ctx {
    /// auth.json
    pub auth_file: PathBuf,
    /// bise's home (`~/.bise`): host-id, cache/
    pub root: PathBuf,
    /// the sign-in server, no trailing '/'
    pub issuer: String,
    /// the provider's base URL (the models list), no trailing '/'
    pub base_url: String,
    /// how long a sign-in waits for the browser
    pub wait: Duration,
    /// send `ext_agent_host_id` in the authorization request
    /// ([`HOST_ID_ENV`]; off by default)
    pub send_host_id: bool,
}

impl Ctx {
    /// The real one: auth.json's folder is bise's home, the issuer from
    /// `BISE_CHATGPT_ISSUER`, the base URL from the catalog (config.toml
    /// can move it).
    pub fn of(paths: &Paths) -> Ctx {
        let root = paths.auth_file.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let issuer = std::env::var(ISSUER_ENV).ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_ISSUER.into());
        let text = std::fs::read_to_string(&paths.config).ok();
        let setup = crate::Setup::from_text(text.as_deref(), &|k| std::env::var(k).ok());
        let base_url = setup.catalog.provider(ID).map(|p| p.base_url.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| RESOURCE.into());
        let send_host_id = std::env::var(HOST_ID_ENV).map(|v| matches!(v.trim(), "1" | "on" | "true" | "yes")).unwrap_or(false);
        Ctx { auth_file: paths.auth_file.clone(), root, issuer: issuer.trim_end_matches('/').to_string(), base_url, wait: WAIT, send_host_id }
    }

    fn lock_file(&self) -> PathBuf {
        let mut s = self.auth_file.clone().into_os_string();
        s.push(".lock");
        PathBuf::from(s)
    }

    fn models_cache(&self) -> PathBuf {
        self.root.join("cache").join("chatgpt-models.json")
    }
}

// ---- time ----

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = (m as u64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Seconds since the epoch as `2026-10-03T13:00:00Z`.
pub fn rfc3339(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, t / 3600, t / 60 % 60, t % 60)
}

/// `2026-10-03T13:00:00Z` (or with fractions / `+00:00`) as seconds
/// since the epoch; None when it is not one.
pub fn parse_rfc3339(s: &str) -> Option<u64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<u32>().ok());
    if s.len() < 19 || s.as_bytes()[4] != b'-' || s.as_bytes()[10] != b'T' {
        return None;
    }
    let (y, m, d) = (num(0, 4)? as i64, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)? as u64, num(14, 16)? as u64, num(17, 19)? as u64);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    (days >= 0).then(|| days as u64 * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// A day as the designer writes it: `3 Nov`, with the year when it is
/// not `now`'s.
pub fn short_date(secs: u64, now: u64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let (ny, _, _) = civil_from_days((now / 86_400) as i64);
    if y == ny {
        format!("{} {}", d, MONTHS[m as usize - 1])
    } else {
        format!("{} {} {}", d, MONTHS[m as usize - 1], y)
    }
}

// ---- the host id ----

fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(random_b64(16)).unwrap_or_default();
    b.copy_from_slice(&raw[..16.min(raw.len())]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{:02x}", x)).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// This machine's `ext_agent_host_id` (`<root>/host-id`): read, or made
/// once (`urn:uuid:<v4>`, 0600; two makers at once: the first one wins).
pub fn host_id(root: &Path) -> Result<String, String> {
    let f = root.join("host-id");
    let read = || std::fs::read_to_string(&f).ok().map(|s| s.trim().to_string()).filter(|s| s.starts_with("urn:uuid:") && s.len() > 20);
    if let Some(id) = read() {
        return Ok(id);
    }
    crate::auth::create_private_dir(root).map_err(|e| format!("cannot create {}: {}", root.display(), e))?;
    let id = format!("urn:uuid:{}", uuid_v4());
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    match o.open(&f) {
        Ok(mut w) => {
            w.write_all(format!("{}\n", id).as_bytes()).map_err(|e| format!("cannot write {}: {}", f.display(), e))?;
            Ok(id)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => read().ok_or_else(|| format!("{} is not a host id: delete it", f.display())),
        Err(e) => Err(format!("cannot write {}: {}", f.display(), e)),
    }
}

// ---- state (no network) ----

/// What auth.json says about the sign-in. No network: a token the
/// server revoked shows at the next call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// no `chatgpt` entry
    NotSetUp,
    /// signed out (bise logout chatgpt), its client kept
    SignedOut { email: Option<String> },
    SignedIn { email: String, plan: Option<String> },
    /// a refresh was refused, or the refresh token is past its 30 days
    Expired { email: String },
}

/// "plus" -> "Plus"; "" -> None.
pub fn plan_label(plan: &str) -> Option<String> {
    let p = plan.trim();
    let mut c = p.chars();
    let first = c.next()?;
    Some(first.to_uppercase().chain(c).collect())
}

/// When the sign-in ends unless a call refreshes it first: the refresh
/// token's 30 days from `saved_at` (seconds since the epoch).
pub fn good_until(o: &OAuth) -> Option<u64> {
    parse_rfc3339(&o.saved_at).map(|t| t + REFRESH_DAYS * 86_400)
}

/// The state at `now` (seconds since the epoch).
pub fn state_at(store: &Store, now: u64) -> State {
    let Some(o) = store.oauth(ID) else { return State::NotSetUp };
    // a client kept from a sign-in that never finished: no account yet
    if !o.expired && !o.signed_in() && o.email.is_empty() && o.subject.is_empty() {
        return State::NotSetUp;
    }
    let email = (!o.email.is_empty()).then(|| o.email.clone());
    if o.expired {
        return State::Expired { email: o.email };
    }
    if !o.signed_in() {
        return State::SignedOut { email };
    }
    if good_until(&o).is_some_and(|t| t <= now) {
        return State::Expired { email: o.email };
    }
    State::SignedIn { email: o.email.clone(), plan: plan_label(&o.plan) }
}

/// The state now.
pub fn state(store: &Store) -> State {
    state_at(store, now_secs())
}

// ---- small HTTP pieces ----

fn loopback(host: &str) -> bool {
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

/// An endpoint bise may call: https, or http on this machine (tests).
fn safe_url(what: &str, u: &str) -> Result<String, String> {
    let p = Url::parse(u).map_err(|_| format!("ChatGPT's sign-in server gives a {} that is not an http(s) URL", what))?;
    if !p.tls && !loopback(&p.host) {
        return Err(format!("ChatGPT's sign-in server gives a {} over plain http: refused", what));
    }
    Ok(u.to_string())
}

/// The OpenID configuration's endpoints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub issuer: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub revocation_endpoint: Option<String>,
}

/// `<issuer>/.well-known/openid-configuration`; its issuer must be ours.
pub fn discover(ctx: &Ctx) -> Result<Meta, String> {
    safe_url("issuer", &ctx.issuer)?;
    let url = format!("{}/.well-known/openid-configuration", ctx.issuer);
    let v = get_json(&url)
        .map_err(|e| format!("i couldn't reach ChatGPT's sign-in server: {}", e))?
        .ok_or("ChatGPT's sign-in server gave no OpenID configuration")?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let issuer = s("issuer").unwrap_or_default();
    if issuer.trim_end_matches('/') != ctx.issuer {
        return Err("ChatGPT's sign-in server names another issuer: refused".into());
    }
    let token_endpoint = safe_url("token endpoint", &s("token_endpoint").ok_or("no token endpoint in ChatGPT's OpenID configuration")?)?;
    let jwks_uri = safe_url("key set", &s("jwks_uri").ok_or("no key set (jwks_uri) in ChatGPT's OpenID configuration")?)?;
    let revocation_endpoint = s("revocation_endpoint").map(|u| safe_url("revocation endpoint", &u)).transpose()?;
    Ok(Meta { issuer: ctx.issuer.clone(), token_endpoint, jwks_uri, revocation_endpoint })
}

const FORM: &str = "application/x-www-form-urlencoded";

/// An OAuth error answer in a few words (OpenAI's `{"error": {"code":
/// ...}}` or RFC 6749's `{"error": "...", "error_description": ...}`).
fn oauth_error(v: &Value) -> String {
    let e = v.get("error");
    let code = e
        .and_then(|e| e.as_str().map(str::to_string).or_else(|| e.get("code").and_then(Value::as_str).map(str::to_string)))
        .unwrap_or_default();
    let desc = v
        .get("error_description")
        .and_then(Value::as_str)
        .or_else(|| e.and_then(|e| e.get("message")).and_then(Value::as_str))
        .unwrap_or("");
    match (code.is_empty(), desc.is_empty()) {
        (false, false) => format!("{} ({})", desc, code),
        (false, true) => code,
        (true, false) => desc.to_string(),
        _ => "no reason given".into(),
    }
}

/// The error code of an answer (either shape).
fn error_code(v: &Value) -> String {
    let e = v.get("error");
    e.and_then(|e| e.as_str().map(str::to_string).or_else(|| e.get("code").and_then(Value::as_str).map(str::to_string)))
        .unwrap_or_default()
}

// ---- the ID token ----

fn b64url(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).map_err(|_| "not base64url".to_string())
}

/// What a checked ID token says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    /// `https://api.openai.com/auth`.`chatgpt_plan_type` ("plus", "pro"),
    /// "" when absent
    pub plan: String,
}

/// Check an ID token (RS256) against the key set `jwks`: its signature,
/// `iss`, `aud` (the issued client), `exp` (a minute of skew) and, when
/// given, `nonce`. Err = why, in a few words.
pub fn check_id_token(tok: &str, jwks: &Value, issuer: &str, client_id: &str, nonce: Option<&str>, now: u64) -> Result<Claims, String> {
    let parts: Vec<&str> = tok.split('.').collect();
    let [h, p, sig] = parts.as_slice() else { return Err("not a JWT".into()) };
    let header: Value = serde_json::from_slice(&b64url(h)?).map_err(|_| "a bad header".to_string())?;
    if header.get("alg").and_then(Value::as_str) != Some("RS256") {
        return Err("not signed with RS256".into());
    }
    let kid = header.get("kid").and_then(Value::as_str);
    let keys = jwks.get("keys").and_then(Value::as_array).ok_or("no keys in the key set")?;
    let rsa: Vec<&Value> = keys.iter().filter(|k| k.get("kty").and_then(Value::as_str) == Some("RSA")).collect();
    let key = match kid {
        Some(id) => rsa.iter().find(|k| k.get("kid").and_then(Value::as_str) == Some(id)).copied(),
        None if rsa.len() == 1 => Some(rsa[0]),
        None => None,
    }
    .ok_or("its key is not in the key set")?;
    let n = b64url(key.get("n").and_then(Value::as_str).ok_or("a key without n")?)?;
    let e = b64url(key.get("e").and_then(Value::as_str).ok_or("a key without e")?)?;
    let signed = format!("{}.{}", h, p);
    ring::signature::RsaPublicKeyComponents { n: &n, e: &e }
        .verify(&ring::signature::RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &b64url(sig)?)
        .map_err(|_| "a bad signature".to_string())?;
    let c: Value = serde_json::from_slice(&b64url(p)?).map_err(|_| "bad claims".to_string())?;
    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if s("iss").trim_end_matches('/') != issuer.trim_end_matches('/') {
        return Err("another issuer".into());
    }
    let aud_ok = match c.get("aud") {
        Some(Value::String(a)) => a == client_id,
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str() == Some(client_id)),
        _ => false,
    };
    if !aud_ok {
        return Err("made for another client".into());
    }
    match c.get("exp").and_then(Value::as_u64) {
        Some(exp) if exp + 60 > now => {}
        _ => return Err("expired".into()),
    }
    if let Some(want) = nonce {
        if c.get("nonce").and_then(Value::as_str) != Some(want) {
            return Err("not from this sign-in (nonce)".into());
        }
    }
    let sub = s("sub");
    if sub.is_empty() {
        return Err("no subject".into());
    }
    let plan = c
        .get("https://api.openai.com/auth")
        .and_then(|a| a.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(Claims { sub, email: s("email"), plan })
}

// ---- the lock ----

/// `auth.json.lock`, held while the tokens change (a refresh, a sign-in's
/// save, a sign-out): every bise process shares it.
pub(crate) struct Lock(std::fs::File);

pub(crate) fn lock(path: &Path) -> Result<Lock, String> {
    if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        crate::auth::create_private_dir(d).map_err(|e| format!("cannot create {}: {}", d.display(), e))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    f.lock().map_err(|e| format!("cannot lock {}: {}", path.display(), e))?;
    Ok(Lock(f))
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn write_store(ctx: &Ctx, store: &Store) -> Result<(), String> {
    store.write(&ctx.auth_file).map_err(|e| format!("cannot write {}: {}", ctx.auth_file.display(), e))
}

/// Save a newly issued client alone (no account, no token yet): the
/// next sign-in reuses it. An entry that already holds a sign-in is left
/// as it is.
fn keep_client(ctx: &Ctx, client: &str) -> Result<(), String> {
    let _l = lock(&ctx.lock_file())?;
    let mut store = Store::read(&ctx.auth_file)?;
    if store.oauth(ID).is_some_and(|o| !o.access.is_empty() || !o.refresh.is_empty()) {
        return Ok(());
    }
    store.set_oauth(ID, &OAuth { client_id: client.to_string(), ..OAuth::default() });
    write_store(ctx, &store)
}

// ---- the token (bise auth token chatgpt) ----

/// Why there is no token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenError {
    /// no sign-in (never, or signed out)
    SignedOut,
    /// ChatGPT refused the refresh: signed out now, the client kept
    Expired,
    /// the refresh could not be made now (network, the server's trouble)
    Failed(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::SignedOut => write!(f, "bise isn't signed in to ChatGPT: run bise login chatgpt"),
            TokenError::Expired => write!(f, "your ChatGPT sign-in expired: run bise login chatgpt"),
            TokenError::Failed(e) => write!(f, "couldn't renew the ChatGPT sign-in: {}", e),
        }
    }
}

fn fresh(o: &OAuth, now_ms: u64) -> bool {
    !o.access.is_empty() && o.expires > now_ms + MIN_LEFT_MS
}

/// A refresh answer that means the refresh token is no good for good
/// (RFC 6749's 400 `invalid_grant`, OpenAI's 401 `refresh_token_reused`,
/// a revoked session): a 429, a 5xx or "try later" is not.
fn refused_for_good(status: u16, v: &Value) -> bool {
    matches!(status, 400 | 401) && !matches!(error_code(v).as_str(), "temporarily_unavailable" | "server_error" | "slow_down")
}

/// The access token to send now: auth.json's when it has 5 more minutes,
/// else refreshed under the lock (the file read again once the lock is
/// held: another process may have refreshed it). The new tokens are
/// written together, atomically. A refused refresh signs out, the client
/// kept, `expired` set.
pub fn access_token_ctx(ctx: &Ctx) -> Result<String, TokenError> {
    let read = || Store::read(&ctx.auth_file).map_err(TokenError::Failed);
    let o = read()?.oauth(ID).filter(|o| o.signed_in()).ok_or(TokenError::SignedOut)?;
    if fresh(&o, now_ms()) {
        return Ok(o.access);
    }
    let _l = lock(&ctx.lock_file()).map_err(TokenError::Failed)?;
    let mut store = read()?;
    let o = store.oauth(ID).filter(|o| o.signed_in()).ok_or(TokenError::SignedOut)?;
    if fresh(&o, now_ms()) {
        return Ok(o.access);
    }
    if o.refresh.is_empty() || o.client_id.is_empty() {
        store.sign_out(ID, true);
        let _ = write_store(ctx, &store);
        return Err(TokenError::Expired);
    }
    let meta = discover(ctx).map_err(TokenError::Failed)?;
    let body = form(&[
        ("grant_type", "refresh_token"),
        ("client_id", &o.client_id),
        ("refresh_token", &o.refresh),
        ("resource", RESOURCE),
    ]);
    let (status, v) = post(&meta.token_endpoint, FORM, &body).map_err(TokenError::Failed)?;
    if !(200..300).contains(&status) {
        if refused_for_good(status, &v) {
            store.sign_out(ID, true);
            write_store(ctx, &store).map_err(TokenError::Failed)?;
            return Err(TokenError::Expired);
        }
        return Err(TokenError::Failed(format!("ChatGPT answered {}: {}", status, oauth_error(&v))));
    }
    let n = renewed(&o, &v, now_ms()).map_err(TokenError::Failed)?;
    store.set_oauth(ID, &n);
    write_store(ctx, &store).map_err(TokenError::Failed)?;
    Ok(n.access)
}

/// The entry after a refresh answer: the access token, its expiry, the
/// rotated refresh token (and its 30 days from now), the scopes and the
/// ID token when given.
fn renewed(o: &OAuth, v: &Value, now_ms: u64) -> Result<OAuth, String> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let mut n = o.clone();
    n.access = s("access_token").ok_or("ChatGPT sent no access token")?;
    n.expires = now_ms + v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600) * 1000;
    if let Some(r) = s("refresh_token") {
        n.refresh = r;
        n.saved_at = rfc3339(now_ms / 1000);
    }
    if let Some(sc) = s("scope") {
        n.scopes = sorted_scopes(&sc);
    }
    if let Some(id) = s("id_token") {
        n.id_token = id;
    }
    n.expired = false;
    Ok(n)
}

fn sorted_scopes(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.split_whitespace().map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

/// [`access_token_ctx`] with the real paths (the TUI's "checking your
/// plan…" call; `bise auth token chatgpt`).
pub fn access_token(paths: &Paths) -> Result<String, TokenError> {
    access_token_ctx(&Ctx::of(paths))
}

// ---- sign-out ----

/// Revoke the refresh token (form POST to the revocation endpoint, an
/// empty 200 = done; a network failure or a 5xx is tried twice more),
/// then drop the tokens, the client kept. Ok(true): ChatGPT confirmed.
pub fn sign_out_ctx(ctx: &Ctx) -> Result<bool, String> {
    let _l = lock(&ctx.lock_file())?;
    let mut store = Store::read(&ctx.auth_file)?;
    let o = store.oauth(ID).filter(|o| o.signed_in()).ok_or("bise isn't signed in to ChatGPT.")?;
    let mut confirmed = false;
    if !o.refresh.is_empty() {
        if let Ok(Some(ep)) = discover(ctx).map(|m| m.revocation_endpoint) {
            let body = form(&[("token", &o.refresh), ("token_type_hint", "refresh_token"), ("client_id", &o.client_id)]);
            for wait in [0u64, 500, 1500] {
                std::thread::sleep(Duration::from_millis(wait));
                match post(&ep, FORM, &body) {
                    Ok((200, _)) => {
                        confirmed = true;
                        break;
                    }
                    Ok((s, _)) if s < 500 => break,
                    _ => {}
                }
            }
        }
    }
    store.sign_out(ID, false);
    write_store(ctx, &store)?;
    Ok(confirmed)
}

pub fn sign_out(paths: &Paths) -> Result<bool, String> {
    sign_out_ctx(&Ctx::of(paths))
}

// ---- the account's models ----

/// A model of the account's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanModel {
    /// the id to send (`gpt-6.1-sol`)
    pub slug: String,
    /// the name to show
    pub display_name: String,
}

fn models_of(v: &Value) -> Vec<PlanModel> {
    v.get("models")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|m| m.get("visibility").and_then(Value::as_str).is_none_or(|x| x == "list"))
                .filter_map(|m| {
                    let slug = m.get("slug").and_then(Value::as_str)?.to_string();
                    let display_name = m.get("display_name").and_then(Value::as_str).unwrap_or(&slug).to_string();
                    Some(PlanModel { slug, display_name })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `GET <base_url>/models` with the token: the account's list (only
/// `visibility == "list"`, the server's order), written to the cache.
pub fn fetch_models_ctx(ctx: &Ctx) -> Result<Vec<PlanModel>, String> {
    let tok = access_token_ctx(ctx).map_err(|e| e.to_string())?;
    let url = Url::parse(&format!("{}/models", ctx.base_url))?;
    let h = vec![("Authorization".to_string(), format!("Bearer {}", tok)), ("Accept".to_string(), "application/json".to_string())];
    let r = http::send(&http::Request { method: "GET", url: &url, headers: &h, body: b"", timeout: Duration::from_secs(15) })
        .map_err(|e| e.to_string())?;
    let status = r.status;
    let b = r.read_all(4 << 20).map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("ChatGPT answered {} to the model list", status));
    }
    let v: Value = serde_json::from_slice(&b).map_err(|_| "the model list is not JSON".to_string())?;
    let models = models_of(&v);
    let cache = json!({
        "fetched_at": now_secs(),
        "models": models.iter().map(|m| json!({"slug": m.slug, "display_name": m.display_name})).collect::<Vec<_>>(),
    });
    let path = ctx.models_cache();
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, cache.to_string()).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(models)
}

pub fn fetch_models(paths: &Paths) -> Result<Vec<PlanModel>, String> {
    fetch_models_ctx(&Ctx::of(paths))
}

fn read_cache(ctx: &Ctx) -> Option<(u64, Vec<PlanModel>)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(ctx.models_cache()).ok()?).ok()?;
    Some((v.get("fetched_at").and_then(Value::as_u64).unwrap_or(0), models_of(&v)))
}

/// The cached list, any age (no network); empty: none yet (use the
/// catalog's chatgpt entries).
pub fn cached_models_ctx(ctx: &Ctx) -> Vec<PlanModel> {
    read_cache(ctx).map(|(_, m)| m).unwrap_or_default()
}

pub fn cached_models(paths: &Paths) -> Vec<PlanModel> {
    cached_models_ctx(&Ctx::of(paths))
}

/// The cache is younger than a day (else fetch again, off the UI thread).
pub fn models_fresh(paths: &Paths) -> bool {
    read_cache(&Ctx::of(paths)).is_some_and(|(t, _)| t + MODELS_TTL > now_secs())
}

// ---- the loopback listener (shared with openrouter_login) ----

/// How a sign-in is going.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll<T> {
    Waiting,
    Done(T),
    /// the user did not allow it (ChatGPT: `access_denied`, or no plan scope)
    Denied,
    /// cancelled, or no answer in 5 minutes
    Unfinished,
    /// one line, never a token
    Failed(String),
}

/// Who signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub email: String,
    /// "Plus", "Pro" (ready to show); None when the ID token does not say
    pub plan: Option<String>,
}

/// A browser page after the redirect.
pub(crate) fn page(ok: bool, what: &str, line: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = if ok {
        format!("<p>bise <b>:*</b> is signed in to {}.</p><p class=d>you can close this tab.</p>", esc(what))
    } else {
        format!("<p>{}</p><p class=d>you can close this tab and go back to bise.</p>", esc(line))
    };
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>bise</title><style>:root{{color-scheme:light dark}}body{{margin:0;min-height:100vh;display:grid;place-items:center;font:16px/1.6 ui-monospace,SFMono-Regular,Menlo,monospace;background:#f4efe6;color:#1d1b19}}b{{color:#b8416b;font-weight:600}}.d{{opacity:.6}}main{{max-width:34em;padding:2em}}@media(prefers-color-scheme:dark){{body{{background:#141312;color:#ece6dc}}b{{color:#f4a6b0}}}}</style><main>{}</main>",
        body
    )
}

fn respond(s: &mut TcpStream, status: &str, html: &str) {
    let head = format!("HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", status, html.len());
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(html.as_bytes());
    let _ = s.flush();
}

/// What one callback request decides: keep waiting, or the end.
pub(crate) type Answer<'a, T> = dyn FnMut(&[(String, String)]) -> Step<T> + 'a;

pub(crate) enum Step<T> {
    /// not this sign-in's answer: the status and page, then wait on
    Again(&'static str, String),
    /// the end: the page, then the result
    End(&'static str, String, Poll<T>),
}

/// Wait on `l` for requests to `path` until `cancel`, the deadline, or a
/// [`Step::End`]; each one's query goes to `answer`.
pub(crate) fn serve<T>(
    l: &TcpListener,
    path: &str,
    deadline: Instant,
    cancel: &AtomicBool,
    answer: &mut Answer<'_, T>,
) -> Poll<T> {
    if l.set_nonblocking(true).is_err() {
        return Poll::Failed("cannot listen for the browser".into());
    }
    loop {
        if cancel.load(Ordering::SeqCst) || Instant::now() > deadline {
            return Poll::Unfinished;
        }
        let mut s = match l.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Poll::Failed(format!("the browser's answer could not be read: {}", e)),
        };
        let _ = s.set_nonblocking(false);
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        let Ok(c) = s.try_clone() else { continue };
        if BufReader::new(c).read_line(&mut line).is_err() {
            continue;
        }
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let (p, q) = target.split_once('?').unwrap_or((target, ""));
        if p != path {
            respond(&mut s, "404 Not Found", "");
            continue;
        }
        match answer(&query(q)) {
            Step::Again(status, html) => respond(&mut s, status, &html),
            Step::End(status, html, r) => {
                respond(&mut s, status, &html);
                return r;
            }
        }
    }
}

/// A sign-in running in the background: [`SignIn::url`] for the
/// browser, [`SignIn::poll`] for its state. Cancelled on drop.
pub struct SignIn<T: Clone + Send + 'static> {
    url: String,
    port: u16,
    status: Arc<Mutex<Poll<T>>>,
    cancel: Arc<AtomicBool>,
}

impl<T: Clone + Send + 'static> SignIn<T> {
    /// Run `work` on a thread with the listener; its result is the end.
    pub(crate) fn spawn(url: String, port: u16, work: impl FnOnce(&AtomicBool) -> Poll<T> + Send + 'static) -> SignIn<T> {
        let status = Arc::new(Mutex::new(Poll::Waiting));
        let cancel = Arc::new(AtomicBool::new(false));
        let (st, c) = (status.clone(), cancel.clone());
        std::thread::spawn(move || {
            let r = work(&c);
            *st.lock().unwrap_or_else(|e| e.into_inner()) = r;
        });
        SignIn { url, port, status, cancel }
    }

    /// The address to open in a browser ("c copies the link").
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The loopback port (an SSH user forwards it).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Where it is now; never blocks.
    pub fn poll(&self) -> Poll<T> {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Give up: the listener closes, the next poll says Unfinished.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Block until it ends (the CLI).
    pub fn wait(&self) -> Poll<T> {
        loop {
            match self.poll() {
                Poll::Waiting => std::thread::sleep(Duration::from_millis(100)),
                r => return r,
            }
        }
    }
}

impl<T: Clone + Send + 'static> Drop for SignIn<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

// ---- the ChatGPT sign-in ----

/// Which client a sign-in uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// the saved client when there is one (same account), else a new one
    Again,
    /// a new registration (another account or workspace)
    NewAccount,
}

/// Start a sign-in (no browser opened: the caller opens [`SignIn::url`]).
pub fn start(paths: &Paths, mode: Mode) -> Result<SignIn<Account>, String> {
    start_ctx(&Ctx::of(paths), mode)
}

/// What a pending sign-in keeps.
struct Pending {
    ctx: Ctx,
    client: String,
    dynamic: bool,
    subject: String,
    redirect: String,
    state: String,
    nonce: String,
    verifier: String,
}

pub fn start_ctx(ctx: &Ctx, mode: Mode) -> Result<SignIn<Account>, String> {
    safe_url("issuer", &ctx.issuer)?;
    let store = Store::read(&ctx.auth_file)?;
    let host = host_id(&ctx.root)?;
    let saved = store.oauth(ID).filter(|o| !o.client_id.is_empty() && mode == Mode::Again);
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("cannot listen on 127.0.0.1: {}", e))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let p = Pending {
        ctx: ctx.clone(),
        client: saved.as_ref().map(|o| o.client_id.clone()).unwrap_or_else(|| DYNAMIC_CLIENT.into()),
        dynamic: saved.is_none(),
        subject: saved.as_ref().map(|o| o.subject.clone()).unwrap_or_default(),
        redirect: format!("http://127.0.0.1:{}{}", port, CALLBACK_PATH),
        state: random_b64(24),
        nonce: random_b64(24),
        verifier: random_b64(48),
    };
    let challenge = challenge_of(&p.verifier);
    let mut q: Vec<(&str, &str)> = vec![("client_id", &p.client)];
    if p.dynamic {
        q.push(("agent_name_hint", "bise"));
    }
    // OpenAI's devkit: the host id only where the deployment takes it,
    // and no id_token_hint (the browser URL goes through `open`'s
    // arguments: no token in it); a saved email as login_hint
    if ctx.send_host_id {
        q.push(("ext_agent_host_id", &host));
    }
    if let Some(o) = &saved {
        if !o.email.is_empty() {
            q.push(("login_hint", &o.email));
        }
    }
    q.extend([
        ("response_type", "code"),
        ("redirect_uri", p.redirect.as_str()),
        ("scope", SCOPES),
        ("resource", RESOURCE),
        ("state", p.state.as_str()),
        ("nonce", p.nonce.as_str()),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge.as_str()),
    ]);
    let url = format!("{}/api/accounts/authorize?{}", ctx.issuer, form(&q));
    let deadline = Instant::now() + ctx.wait;
    Ok(SignIn::spawn(url, port, move |cancel| {
        serve(&listener, CALLBACK_PATH, deadline, cancel, &mut |q| callback(&p, q))
    }))
}

const DENIED_LINE: &str = "ChatGPT signed you in but didn't let bise use your plan. try again and allow it, or pick another way.";

/// One request on the callback path.
fn callback(p: &Pending, q: &[(String, String)]) -> Step<Account> {
    let get = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    if get("state") != Some(p.state.as_str()) {
        return Step::Again("400 Bad Request", page(false, "ChatGPT", "this answer is not for bise's sign-in."));
    }
    let fail = |line: String| Step::End("200 OK", page(false, "ChatGPT", &line), Poll::Failed(line));
    if let Some(e) = get("error") {
        if e == "access_denied" {
            return Step::End("200 OK", page(false, "ChatGPT", DENIED_LINE), Poll::Denied);
        }
        let d = get("error_description").filter(|d| !d.is_empty()).unwrap_or(e);
        return fail(format!("ChatGPT didn't sign you in: {}", d));
    }
    let client = match (p.dynamic, get("client_id").filter(|c| !c.is_empty())) {
        (true, Some(c)) if c != DYNAMIC_CLIENT => c.to_string(),
        (true, _) => return fail("ChatGPT didn't finish registering bise (no client id came back). try again.".into()),
        (false, None) => p.client.clone(),
        (false, Some(c)) if c == p.client => p.client.clone(),
        (false, Some(_)) => return fail("ChatGPT answered for another registration of bise: nothing changed. try again.".into()),
    };
    let Some(code) = get("code").filter(|c| !c.is_empty()) else {
        return fail("ChatGPT sent no code. try again.".into());
    };
    match finish(p, &client, code, get("scope")) {
        Ok(Some(a)) => Step::End("200 OK", page(true, "ChatGPT", ""), Poll::Done(a)),
        Ok(None) => Step::End("200 OK", page(false, "ChatGPT", DENIED_LINE), Poll::Denied),
        Err(line) => fail(line),
    }
}

/// Exchange the code, check the ID token and the plan scope, save.
/// Ok(None): signed in without the plan's use (the client kept).
fn finish(p: &Pending, client: &str, code: &str, cb_scope: Option<&str>) -> Result<Option<Account>, String> {
    if p.dynamic {
        // OpenAI's devkit (onRegistration): keep the issued client before
        // the one-time code exchange, so a failed exchange is retried with
        // it instead of registering bise again
        keep_client(&p.ctx, client)?;
    }
    let meta = discover(&p.ctx)?;
    let body = form(&[
        ("grant_type", "authorization_code"),
        ("client_id", client),
        ("code", code),
        ("code_verifier", &p.verifier),
        ("redirect_uri", &p.redirect),
        ("resource", RESOURCE),
    ]);
    let (status, v) = post(&meta.token_endpoint, FORM, &body).map_err(|e| format!("i couldn't reach ChatGPT's sign-in server: {}", e))?;
    if !(200..300).contains(&status) {
        return Err(format!("ChatGPT refused the sign-in: {}. try again.", oauth_error(&v)));
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let id_token = s("id_token").ok_or("ChatGPT sent no ID token. try again.")?;
    let jwks = get_json(&meta.jwks_uri)
        .map_err(|e| format!("i couldn't reach ChatGPT's key set: {}", e))?
        .ok_or("ChatGPT's key set is not there")?;
    let claims = check_id_token(&id_token, &jwks, &meta.issuer, client, Some(&p.nonce), now_secs())
        .map_err(|why| format!("the sign-in's ID token didn't check out ({}): nothing saved.", why))?;
    if !p.dynamic && !p.subject.is_empty() && claims.sub != p.subject {
        return Err("that's another ChatGPT account than the one bise had: use \"switch account\" for it.".into());
    }
    let scopes = sorted_scopes(&s("scope").or_else(|| cb_scope.map(str::to_string)).unwrap_or_default());
    let now = now_ms();
    let _l = lock(&p.ctx.lock_file())?;
    let mut store = Store::read(&p.ctx.auth_file)?;
    let base = OAuth { client_id: client.to_string(), email: claims.email.clone(), subject: claims.sub.clone(), ..OAuth::default() };
    if !scopes.iter().any(|x| x == PLAN_SCOPE) {
        // signed in without the plan: keep the client for the next try
        store.set_oauth(ID, &base);
        write_store(&p.ctx, &store)?;
        return Ok(None);
    }
    let entry = OAuth {
        plan: claims.plan.clone(),
        access: s("access_token").ok_or("ChatGPT sent no access token. try again.")?,
        refresh: s("refresh_token").unwrap_or_default(),
        expires: now + v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600) * 1000,
        id_token,
        scopes,
        saved_at: rfc3339(now / 1000),
        ..base
    };
    store.set_oauth(ID, &entry);
    write_store(&p.ctx, &store)?;
    Ok(Some(Account { email: claims.email, plan: plan_label(&claims.plan) }))
}

#[cfg(test)]
#[path = "chatgpt_tests.rs"]
pub(crate) mod tests;
