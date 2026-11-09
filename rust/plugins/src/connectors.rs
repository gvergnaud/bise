//! His Mistral connectors (docs/plugins.md, "Mistral connectors"): the
//! one owner of the catalog of `/v1/connectors/bootstrap`, for every
//! agent and for the UI. It is fetched here in Rust and kept in one cache
//! file (`bise_home::Home::connectors_cache`, written under a lock
//! through a tmp file, fresh for [`TTL_MS`] and shared by every REPL),
//! parsed into [`ConnectorEntry`]s, each ready one with his accounts on
//! it ([`with_accounts`]: one `GET …/user/credentials` per connector at
//! the same fetch, [`ACCOUNTS_AT_ONCE`] at a time, in the same cache; a
//! failed list keeps the last good accounts), and turned into the session's tool
//! index lines ([`index_lines`]: the index format `mcp.bend` wrote, with
//! bise's note on image_generation; a tool's input schema is serde_json's
//! compact text, keys sorted). A failed fetch keeps the last good
//! catalog, says why ([`Catalog::error`]) and is not tried again for
//! [`FAIL_BACKOFF_MS`]. The key comes from auth.json or the env
//! ([`key`]), never from or into the cache; a locked keychain is
//! [`Problem::Locked`], never "no key".
//!
//! [`connect`] connects one connector: Mistral's auth URL in his browser
//! (an OAuth or a credentials one), or the one-click connection of a
//! no-auth connector, then a re-fetch every [`CONNECT_POLL_MS`] until it
//! is ready, for at most [`CONNECT_MAX_MS`].
//!
//! The pure parts (parse, alias, index lines, the cache's decisions) take
//! their inputs (the body, the clock, the fetch) so the tests need no
//! network (`connectors/tests.rs`); `tests/fake_bootstrap.py` drives the
//! real HTTP against a fake Mistral.

#[cfg(test)]
mod tests;

use crate::http::{self, Url};
use crate::id::{PartKind, PluginId};
use crate::model::{AccountStatus, ConnectorAccount, ConnectorAuth, ConnectorEntry, StateV2, ToolEntry};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// The built-in connectors bise asks the bootstrap for, next to the
/// MCP-capable ones: each is a plugin he can turn off (image_generation
/// costs $0.10 an image).
pub const BUILTINS: &[&str] = &["web_search", "image_generation"];

/// How long a fetched catalog stays fresh.
pub const TTL_MS: u64 = 10 * 60 * 1000;

/// After a failed fetch, how long [`load`] answers the last good catalog
/// without asking again (the gateway's 500s come and go; every bridge
/// polls every 2 s).
pub const FAIL_BACKOFF_MS: u64 = 30 * 1000;

/// [`connect`]: how often it re-fetches, and for how long at most.
pub const CONNECT_POLL_MS: u64 = 3 * 1000;
pub const CONNECT_MAX_MS: u64 = 5 * 60 * 1000;

/// Mistral's API; `BISE_MISTRAL_URL` names a fake one in tests.
pub const DEFAULT_BASE: &str = "https://api.mistral.ai";

/// How many connectors' accounts are asked at once (each call bound
/// like the bootstrap's).
pub const ACCOUNTS_AT_ONCE: usize = 4;

/// One request's connect and read bound.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest bootstrap answer read.
const MAX_BODY: usize = 16 << 20;

/// bise's words after image_generation's own tool descriptions: its
/// answer is a JSON {url}, a signed link that expires in about an hour,
/// so the agent must save the image at once.
pub const IMAGE_NOTE: &str = " NOTE (bise): the result is a JSON {url}, a link that expires in about 1 hour: download it to a file at once (curl -sSL <url> -o <file>.jpg) and use the file; pass the url to edit_image only within that hour. Each image costs $0.10 on the user's Mistral account.";

/// What a call with no Mistral key says.
pub const NO_KEY: &str = "no Mistral key for connectors: run bise login mistral";

/// What a call says while the keychain holding the key is locked (never
/// "no key": he is not signed out).
pub const LOCKED: &str = "the keychain holding your Mistral key is locked: unlock it, then refresh";

/// The Mistral key, as [`key`] finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Key(String),
    /// no key anywhere: he has not connected his Mistral account
    NoKey,
    /// auth.json is in a locked keychain (and the env has none)
    Locked,
}

/// Why the catalog is not fresh from Mistral (the connectors are then the
/// last good ones).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// no Mistral key: [`NO_KEY`]
    NoKey,
    /// the keychain is locked: [`LOCKED`]
    Locked,
    /// the last fetch failed, in its words (no answer, a non-200, a body
    /// with no connectors list, the cache file)
    Fetch(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NoKey => f.write_str(NO_KEY),
            Problem::Locked => f.write_str(LOCKED),
            Problem::Fetch(e) => f.write_str(e),
        }
    }
}

/// The catalog as bise has it: its connectors, when it was fetched, and
/// why it is not fresh when it is not (the connectors are then the last
/// good ones).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub connectors: Vec<ConnectorEntry>,
    pub fetched_ms: u64,
    pub error: Option<Problem>,
}

impl Catalog {
    /// The connector whose id is `mistral:<name>`.
    pub fn find(&self, id: &PluginId) -> Option<&ConnectorEntry> {
        match id.owner() {
            PluginId::Mistral { name } => self.connectors.iter().find(|c| &c.name == name),
            _ => None,
        }
    }
}

// ---- the address ----

/// Mistral's API base: `BISE_MISTRAL_URL`, else [`DEFAULT_BASE`].
pub fn base_url() -> String {
    std::env::var("BISE_MISTRAL_URL").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_BASE.into()).trim_end_matches('/').to_string()
}

/// The bootstrap's address, as the vibe CLI calls it: auth-actionable
/// (not yet connected) connectors included, and each of [`BUILTINS`] its
/// own `builtin_connectors` param (a comma-joined one reads as one name).
pub fn bootstrap_url(base: &str) -> String {
    let mut u = format!("{}/v1/connectors/bootstrap?supports_mcp=true&include_auth_actionable_connectors=true", base.trim_end_matches('/'));
    for b in BUILTINS {
        u.push_str("&builtin_connectors=");
        u.push_str(b);
    }
    u
}

/// The Mistral key: auth.json's `mistral` API key (it wins over the env,
/// BISE-269), else `MISTRAL_API_KEY`. The harness exports the same key
/// into its REPLs' env.
pub fn key() -> Key {
    key_of(bise_secrets::read(&bise_home::Home::from_env().auth_file()), std::env::var("MISTRAL_API_KEY").ok())
}

/// [`key`] from auth.json's read and the env's value. A locked keychain
/// is [`Key::Locked`], never [`Key::NoKey`]; the env's key still serves
/// then (the harness exported auth.json's key there while it was open).
pub fn key_of(stored: Result<Option<String>, bise_secrets::ReadError>, env: Option<String>) -> Key {
    let env = env.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    match stored {
        Ok(text) => match text.and_then(|t| key_of_auth(&t)).or(env) {
            Some(k) => Key::Key(k),
            None => Key::NoKey,
        },
        Err(bise_secrets::ReadError::Locked) => env.map_or(Key::Locked, Key::Key),
        // an unreadable auth.json is not a locked one: the env, or no key
        Err(_) => env.map_or(Key::NoKey, Key::Key),
    }
}

/// auth.json's `mistral` entry's key, when it is an API key.
pub fn key_of_auth(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let e = v.get("mistral")?;
    if e.get("type").and_then(Value::as_str) != Some("api") {
        return None;
    }
    e.get("key").and_then(Value::as_str).map(str::trim).filter(|k| !k.is_empty()).map(String::from)
}

// ---- parse ----

/// The published alias of a connector name (vibe's
/// `normalize_connector_alias`): anything outside `[a-zA-Z0-9_-]` becomes
/// `_`, the edges lose their `_` and `-`, at most 256 characters, and an
/// empty result is `unnamed`. `tools.<alias>.<tool>` needs a clean name.
pub fn normalize_name(s: &str) -> String {
    let flat: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    let cut: String = flat.trim_matches(['_', '-']).chars().take(256).collect();
    if cut.is_empty() {
        "unnamed".into()
    } else {
        cut
    }
}

/// `alias`, or `alias_2`, `alias_3`... the first one not in `used`
/// (vibe's `_unique_alias`: it fits in 256 characters too).
pub fn dedupe(alias: &str, used: &[String]) -> String {
    let mut out = alias.to_string();
    let mut n = 2;
    while used.contains(&out) {
        let suffix = format!("_{n}");
        let room = 256usize.saturating_sub(suffix.len());
        out = format!("{}{}", alias.chars().take(room).collect::<String>(), suffix);
        n += 1;
    }
    out
}

/// The connectors of a bootstrap answer's body, in its order. An answer
/// that is not JSON or has no `connectors` array is an error (the last
/// good catalog is kept).
pub fn parse(body: &str) -> Result<Vec<ConnectorEntry>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("the answer is not JSON (line {}, column {})", e.line(), e.column()))?;
    let Some(cs) = v.get("connectors").and_then(Value::as_array) else {
        return Err("the answer has no connectors list".into());
    };
    let s = |v: &Value, k: &str, dflt: &str| v.get(k).and_then(Value::as_str).unwrap_or(dflt).to_string();
    let mut used: Vec<String> = Vec::new();
    let mut out = Vec::with_capacity(cs.len());
    for c in cs {
        let raw = s(c, "name", "");
        let name = dedupe(&normalize_name(&raw), &used);
        used.push(name.clone());
        let ready = c.get("status").and_then(|st| st.get("is_ready")) == Some(&Value::Bool(true));
        let action = c.get("auth_action").map(|a| s(a, "type", "")).unwrap_or_default();
        let auth = match (ready, action.as_str()) {
            (true, _) => ConnectorAuth::Ready,
            (false, "oauth") => ConnectorAuth::OAuth,
            (false, "no_auth_connect") => ConnectorAuth::NoAuthConnect,
            (false, "credentials_setup") => ConnectorAuth::CredentialsSetup,
            // none, or a kind bise does not know: not connectable from bise
            (false, _) => ConnectorAuth::Unknown,
        };
        let display = Some(s(c, "display_name", "")).filter(|d| !d.is_empty()).unwrap_or_else(|| if raw.is_empty() { name.clone() } else { raw.clone() });
        let tools = c
            .get("tools")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|t| ToolEntry {
                name: s(t, "name", "?"),
                description: s(t, "description", ""),
                // compact, keys sorted (serde_json's map)
                input_schema: t.get("inputSchema").map(Value::to_string).unwrap_or_default(),
            })
            .collect();
        out.push(ConnectorEntry { name, display_name: display, id: s(c, "id", ""), auth, tools, accounts: Vec::new() });
    }
    Ok(out)
}

/// Mistral's answer to `GET /v1/connectors/{id}/user/credentials`, as it
/// is on the wire (only what bise reads).
#[derive(serde::Deserialize)]
struct Credentials {
    credentials: Vec<Cred>,
}

#[derive(serde::Deserialize)]
struct Cred {
    #[serde(default)]
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    status: Option<CredStatus>,
}

#[derive(serde::Deserialize)]
struct CredStatus {
    #[serde(default)]
    status_type: AccountStatus,
}

/// His accounts in an answer of `GET /v1/connectors/{id}/user/credentials`,
/// in its order: a connection with no name is left out, a blank title is
/// none, a status bise does not know is `Unknown`. An answer with no
/// `credentials` array is an error.
pub fn parse_accounts(body: &str) -> Result<Vec<ConnectorAccount>, String> {
    let c: Credentials = serde_json::from_str(body).map_err(|e| format!("the accounts answer is not a credentials list (line {}, column {})", e.line(), e.column()))?;
    Ok(c.credentials
        .into_iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|c| ConnectorAccount {
            name: c.name.trim().to_string(),
            title: c.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
            default: c.is_default,
            status: c.status.map(|s| s.status_type).unwrap_or_default(),
        })
        .collect())
}

/// Whether bise asks Mistral for a connector's accounts: a ready one that
/// is not a built-in (those have no connection of his).
pub fn has_accounts(c: &ConnectorEntry) -> bool {
    c.auth == ConnectorAuth::Ready && !BUILTINS.contains(&c.name.as_str())
}

/// The connectors with their accounts: each one that [`has_accounts`]
/// gets the list `fetch` reads for its Mistral id ([`ACCOUNTS_AT_ONCE`]
/// at a time); a
/// list that fails or does not parse keeps that connector's accounts in
/// `old` (none when it had none), never failing the catalog.
pub fn with_accounts(mut cs: Vec<ConnectorEntry>, old: &Catalog, fetch: &(dyn Fn(&str) -> Result<String, String> + Sync)) -> Vec<ConnectorEntry> {
    let ids: Vec<(usize, String)> = cs.iter().enumerate().filter(|(_, c)| has_accounts(c)).map(|(i, c)| (i, c.id.clone())).collect();
    for batch in ids.chunks(ACCOUNTS_AT_ONCE) {
        let got: Vec<Option<Vec<ConnectorAccount>>> = std::thread::scope(|s| {
            let hs: Vec<_> = batch.iter().map(|(_, id)| s.spawn(move || fetch(id).and_then(|b| parse_accounts(&b)).ok())).collect();
            hs.into_iter().map(|h| h.join().ok().flatten()).collect()
        });
        for ((i, _), got) in batch.iter().zip(got) {
            let c = &mut cs[*i];
            c.accounts = match got {
                Some(a) => a,
                None => old.connectors.iter().find(|o| o.name == c.name).map(|o| o.accounts.clone()).unwrap_or_default(),
            };
        }
    }
    cs
}

// ---- the index lines ----

/// The index is line-oriented: a newline or carriage return becomes a
/// space.
fn flat(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

/// One tool's index line: `<cid> <cname> <tool> : #<desc>[ | input: <schema>]`.
fn tool_line(c: &ConnectorEntry, t: &ToolEntry) -> String {
    let note = if c.name == "image_generation" { IMAGE_NOTE } else { "" };
    let schema = if t.input_schema.is_empty() { String::new() } else { format!(" | input: {}", flat(&t.input_schema)) };
    format!("{} {} {} : #{}{}{}", c.id, c.name, flat(&t.name), flat(&t.description), note, schema)
}

/// The session's tool index lines for the connectors on in `state` (the
/// bridge merges them with the plugins' tools into `D/mcp-index.txt`):
/// the ready connectors' tools, minus an off connector and an off tool.
pub fn index_lines(catalog: &Catalog, state: &StateV2) -> Vec<String> {
    index_lines_with(catalog, &|id| state.is_on(id, false))
}

/// [`index_lines`] with the on/off rule given.
pub fn index_lines_with(catalog: &Catalog, on: &dyn Fn(&PluginId) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    for c in catalog.connectors.iter().filter(|c| c.auth == ConnectorAuth::Ready) {
        let id = PluginId::mistral(&c.name);
        if !on(&id) {
            continue;
        }
        for t in &c.tools {
            if on(&id.part(PartKind::Tool, None, t.name.as_str())) {
                out.push(tool_line(c, t));
            }
        }
    }
    out
}

// ---- the accounts file ----

/// The session's accounts file, beside the index in the bridge's folder
/// (`bend/runtime/plugins.bend`'s `mcp_accounts_path` reads this name).
pub const ACCOUNTS_FILE: &str = "mcp-accounts.json";

/// The accounts file's text: for each ready connector on in the index
/// that has accounts, `{"cid", "group", "accounts": [{name, title?,
/// default?}]}` (the index's cid and alias; no status: a call resolves
/// its `account` there, `bend/runtime/mcp-pure.bend`'s `account_pick`).
pub fn accounts_file(catalog: &Catalog, state: &StateV2) -> String {
    let cs: Vec<Value> = catalog
        .connectors
        .iter()
        .filter(|c| c.auth == ConnectorAuth::Ready && !c.accounts.is_empty() && state.is_on(&PluginId::mistral(&c.name), false))
        .map(|c| {
            let accts: Vec<Value> = c
                .accounts
                .iter()
                .map(|a| {
                    let mut v = json!({"name": a.name});
                    if let Some(t) = &a.title {
                        v["title"] = json!(t);
                    }
                    if a.default {
                        v["default"] = json!(true);
                    }
                    v
                })
                .collect();
            json!({"cid": c.id, "group": c.name, "accounts": accts})
        })
        .collect();
    Value::Array(cs).to_string() + "\n"
}

// ---- the cache ----

/// The cache file as read: the catalog and when a fetch last failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cached {
    pub catalog: Catalog,
    pub failed_ms: u64,
}

fn auth_word(a: ConnectorAuth) -> &'static str {
    match a {
        ConnectorAuth::Ready => "ready",
        ConnectorAuth::OAuth => "oauth",
        ConnectorAuth::NoAuthConnect => "no_auth_connect",
        ConnectorAuth::CredentialsSetup => "credentials_setup",
        ConnectorAuth::Unknown => "unknown",
    }
}

fn auth_of(w: &str) -> ConnectorAuth {
    match w {
        "ready" => ConnectorAuth::Ready,
        "oauth" => ConnectorAuth::OAuth,
        "no_auth_connect" => ConnectorAuth::NoAuthConnect,
        "credentials_setup" => ConnectorAuth::CredentialsSetup,
        _ => ConnectorAuth::Unknown,
    }
}

/// The cache's text (no key in it, ever).
pub fn to_cache_text(c: &Cached) -> String {
    let cs: Vec<Value> = c
        .catalog
        .connectors
        .iter()
        .map(|e| {
            let ts: Vec<Value> = e.tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema})).collect();
            let mut v = json!({"name": e.name, "display_name": e.display_name, "id": e.id, "auth": auth_word(e.auth), "tools": ts});
            if !e.accounts.is_empty() {
                v["accounts"] = serde_json::to_value(&e.accounts).unwrap_or_default();
            }
            v
        })
        .collect();
    // only a fetch's failure is the cache's: no key and a locked keychain
    // are this process's, never written
    let error = match &c.catalog.error {
        Some(Problem::Fetch(e)) => Some(e.as_str()),
        _ => None,
    };
    let v = json!({"version": 1, "fetched_ms": c.catalog.fetched_ms, "failed_ms": c.failed_ms, "error": error, "connectors": cs});
    serde_json::to_string(&v).unwrap_or_default() + "\n"
}

/// The cache's text read back; anything unreadable is an empty cache.
pub fn of_cache_text(text: &str) -> Cached {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Cached::default() };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let connectors = v
        .get("connectors")
        .and_then(Value::as_array)
        .map(|cs| {
            cs.iter()
                .map(|e| ConnectorEntry {
                    name: s(e, "name"),
                    display_name: s(e, "display_name"),
                    id: s(e, "id"),
                    auth: auth_of(&s(e, "auth")),
                    tools: e
                        .get("tools")
                        .and_then(Value::as_array)
                        .map(|ts| ts.iter().map(|t| ToolEntry { name: s(t, "name"), description: s(t, "description"), input_schema: s(t, "input_schema") }).collect())
                        .unwrap_or_default(),
                    // a cache written before accounts: none, read at the next fetch
                    accounts: e.get("accounts").cloned().and_then(|a| serde_json::from_value(a).ok()).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Cached { catalog: Catalog { connectors, fetched_ms: n("fetched_ms"), error: v.get("error").and_then(Value::as_str).map(|e| Problem::Fetch(e.into())) }, failed_ms: n("failed_ms") }
}

/// The catalog in `cache` as it is, stale or not (empty when none): no
/// network, for a list that opens now (the refresh runs off-thread).
pub fn cached(cache: &Path) -> Catalog {
    read_cache(cache).catalog
}

pub fn read_cache(cache: &Path) -> Cached {
    std::fs::read_to_string(cache).map(|t| of_cache_text(&t)).unwrap_or_default()
}

/// Each write's own tmp name in this process (with the pid: across them).
static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_cache(cache: &Path, c: &Cached) -> std::io::Result<()> {
    let tmp = cache.with_extension(format!("json.tmp-{}-{}", std::process::id(), TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&tmp, to_cache_text(c))?;
    std::fs::rename(&tmp, cache).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

/// What [`load`] does with the cache it read at `now_ms`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// answer the cache as it is
    Keep,
    /// fetch
    Fetch,
}

/// Fresh, or failed a moment ago: keep; else (or `force`) fetch.
pub fn plan(c: &Cached, now_ms: u64, force: bool) -> Plan {
    let fresh = c.catalog.fetched_ms > 0 && now_ms.saturating_sub(c.catalog.fetched_ms) < TTL_MS && c.catalog.error.is_none();
    let backing_off = c.failed_ms > 0 && now_ms.saturating_sub(c.failed_ms) < FAIL_BACKOFF_MS;
    if !force && (fresh || backing_off) {
        Plan::Keep
    } else {
        Plan::Fetch
    }
}

/// The cache after a fetch at `now_ms`: the new catalog, or the last good
/// one with the error.
pub fn after_fetch(old: Cached, got: Result<Vec<ConnectorEntry>, String>, now_ms: u64) -> Cached {
    match got {
        Ok(connectors) => Cached { catalog: Catalog { connectors, fetched_ms: now_ms, error: None }, failed_ms: 0 },
        Err(e) => Cached { catalog: Catalog { error: Some(Problem::Fetch(e)), ..old.catalog }, failed_ms: now_ms },
    }
}

/// The catalog from `cache` when fresh at `now_ms` (and `force` is
/// false), else fetched with `key` (the bootstrap, then each ready
/// connector's accounts) and written to `cache` under its lock.
/// No key or a locked keychain: the cache as it is (nothing written),
/// with [`Problem::NoKey`] or [`Problem::Locked`].
pub fn load(cache: &Path, key: &Key, now_ms: u64, force: bool) -> Catalog {
    let problem = match key {
        Key::Key(k) => {
            let base = base_url();
            return load_with(cache, now_ms, force, &Fetch { bootstrap: &|| fetch(&base, k), accounts: &|raw| fetch_accounts(&base, k, raw) });
        }
        Key::NoKey => Problem::NoKey,
        Key::Locked => Problem::Locked,
    };
    Catalog { error: Some(problem), ..read_cache(cache).catalog }
}

/// What [`load_with`] fetches with: the bootstrap's body, and a
/// connector's accounts' body by its Mistral id (each the body of a 200,
/// or why not).
pub struct Fetch<'a> {
    pub bootstrap: &'a dyn Fn() -> Result<String, String>,
    pub accounts: &'a (dyn Fn(&str) -> Result<String, String> + Sync),
}

/// [`load`] with the fetch given.
pub fn load_with(cache: &Path, now_ms: u64, force: bool, fetch: &Fetch<'_>) -> Catalog {
    let first = read_cache(cache);
    if plan(&first, now_ms, force) == Plan::Keep {
        return first.catalog;
    }
    if let Some(dir) = cache.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _lock = match lock(&cache.with_extension("json.lock")) {
        Ok(l) => l,
        Err(e) => return Catalog { error: Some(Problem::Fetch(format!("{}: {}", cache.display(), e))), ..first.catalog },
    };
    // another process may have fetched while this one waited
    let now = read_cache(cache);
    if now != first && (plan(&now, now_ms, false) == Plan::Keep || now.catalog.fetched_ms >= now_ms || now.failed_ms >= now_ms) {
        return now.catalog;
    }
    let got = (fetch.bootstrap)().and_then(|b| parse(&b)).map(|cs| with_accounts(cs, &now.catalog, fetch.accounts));
    let next = after_fetch(now, got, now_ms);
    if let Err(e) = write_cache(cache, &next) {
        return Catalog { error: Some(Problem::Fetch(format!("{}: {}", cache.display(), e))), ..next.catalog };
    }
    next.catalog
}

// ---- HTTP ----

fn bearer(key: &str) -> Vec<(String, String)> {
    vec![("Authorization".into(), format!("Bearer {key}")), ("Accept".into(), "application/json".into())]
}

/// One request; the body of a 2xx, else why not (the status and the
/// start of the answer, never the key).
fn request(method: &str, url: &str, key: &str, body: &[u8]) -> Result<String, String> {
    let u = Url::parse(url)?;
    let mut h = bearer(key);
    if !body.is_empty() {
        h.push(("Content-Type".into(), "application/json".into()));
    }
    let r = http::send(&http::Request { method, url: &u, headers: &h, body, timeout: FETCH_TIMEOUT }).map_err(|e| e.to_string())?;
    let status = r.status;
    let b = r.read_all(MAX_BODY).map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {} from {}: {}", status, u.shown(), http::short_body(&b)));
    }
    String::from_utf8(b).map_err(|_| format!("the answer of {} is not UTF-8", u.shown()))
}

/// GET the bootstrap: the body of a 200.
pub fn fetch(base: &str, key: &str) -> Result<String, String> {
    request("GET", &bootstrap_url(base), key, b"")
}

/// GET a connector's accounts (his connections), by its Mistral id: the
/// body of a 200.
pub fn fetch_accounts(base: &str, key: &str, raw: &str) -> Result<String, String> {
    request("GET", &format!("{}/v1/connectors/{}/user/credentials", base.trim_end_matches('/'), path_part(raw)), key, b"")
}

/// A connector's name or id in a URL path.
fn path_part(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The raw Mistral id of connector `id` from the cache (its alias when
/// the cache does not know it: the API takes an id or a name).
fn raw_id(cache: &Path, id: &PluginId) -> Result<String, String> {
    let PluginId::Mistral { name } = id.owner() else { return Err(format!("{id} is not a Mistral connector")) };
    Ok(read_cache(cache).catalog.find(id).map(|c| c.id.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| name.clone()))
}

/// The address that connects connector `id` (a `mistral:` id) in his
/// browser: Mistral's `GET /v1/connectors/{id}/auth_url` (the SDK's
/// `beta.connectors.get_auth_url`).
pub fn connect_url(key: &str, id: &PluginId) -> Result<String, String> {
    auth_url(&base_url(), key, &raw_id(&bise_home::Home::from_env().connectors_cache(), id)?, None)
}

/// [`connect_url`] for the raw id, at `base`; `account`: the connection
/// it logs in (`credentials_name`: a new one for another account, an
/// existing one to reconnect it), none for his default.
pub fn auth_url(base: &str, key: &str, raw: &str, account: Option<&str>) -> Result<String, String> {
    let q = account.map(|a| format!("?credentials_name={}", path_part(a))).unwrap_or_default();
    let body = request("GET", &format!("{}/v1/connectors/{}/auth_url{}", base, path_part(raw), q), key, b"")?;
    let v: Value = serde_json::from_str(&body).map_err(|_| "Mistral's answer has no auth_url".to_string())?;
    v.get("auth_url").and_then(Value::as_str).filter(|u| u.starts_with("https://") || u.starts_with("http://")).map(String::from).ok_or_else(|| "Mistral's answer has no auth_url".into())
}

/// A no-auth connector's one-click connection: his default connection,
/// with no secret (what Mistral's dashboard and vibe do).
pub fn connect_no_auth(base: &str, key: &str, raw: &str) -> Result<(), String> {
    let body = json!({"name": "default", "is_default": true, "credentials": {"headers": {}}}).to_string();
    request("POST", &format!("{}/v1/connectors/{}/user/credentials", base, path_part(raw)), key, body.as_bytes()).map(|_| ())
}

/// His browser on `url`: `BISE_OPEN` when set (the TUI's links do the
/// same), else the system's opener.
pub fn open(url: &str) -> Result<(), String> {
    match std::env::var("BISE_OPEN").ok().filter(|c| !c.trim().is_empty()) {
        Some(cmd) => std::process::Command::new(cmd)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|mut c| {
                std::thread::spawn(move || c.wait());
            })
            .map_err(|e| format!("cannot open the browser: {e}")),
        None => crate::oauth::open_browser(url),
    }
}

/// What [`connect`] needs from the world, so a test gives its own.
pub struct World<'a> {
    pub cache: &'a Path,
    pub key: &'a str,
    pub base: &'a str,
    pub now_ms: &'a dyn Fn() -> u64,
    pub sleep_ms: &'a dyn Fn(u64),
    pub open: &'a dyn Fn(&str) -> Result<(), String>,
    /// one line for him as it goes ("opened Gmail's login in your browser")
    pub say: &'a dyn Fn(&str),
}

/// How [`connect`] ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connected {
    /// ready now
    Ready,
    /// it was already
    Already,
}

/// Connect connector `id` (his click): an OAuth or a credentials one opens
/// Mistral's page in his browser, a no-auth one is connected at once; then
/// the catalog is fetched again every [`CONNECT_POLL_MS`] until the
/// connector is ready, for at most [`CONNECT_MAX_MS`].
pub fn connect(w: &World<'_>, id: &PluginId) -> Result<Connected, String> {
    let fetch = Fetch { bootstrap: &|| fetch(w.base, w.key), accounts: &|raw| fetch_accounts(w.base, w.key, raw) };
    let cat = load_with(w.cache, (w.now_ms)(), false, &fetch);
    let Some(c) = cat.find(id).cloned() else {
        let cat = load_with(w.cache, (w.now_ms)(), true, &fetch);
        return Err(match &cat.error {
            Some(e) => format!("no connector {id} in the catalog ({e})"),
            None => format!("no connector {id} in your Mistral catalog"),
        });
    };
    match c.auth {
        ConnectorAuth::Ready => return Ok(Connected::Already),
        ConnectorAuth::NoAuthConnect => {
            connect_no_auth(w.base, w.key, &c.id)?;
            (w.say)(&format!("connected {} on Mistral; waiting for it to be ready", c.display_name));
        }
        ConnectorAuth::Unknown => {
            return Err(format!("{} can't be connected from bise: see it on console.mistral.ai, Build › Connectors", c.display_name));
        }
        ConnectorAuth::OAuth | ConnectorAuth::CredentialsSetup => {
            let url = auth_url(w.base, w.key, &c.id, None).map_err(|e| format!("{}: no login page from Mistral ({e}); set it up on console.mistral.ai, Build › Connectors", c.display_name))?;
            (w.open)(&url)?;
            (w.say)(&format!("opened {}'s page in your browser; waiting for you to finish there", c.display_name));
        }
    }
    poll(w, &fetch, &|e| e.auth == ConnectorAuth::Ready, id)
        .then_some(Connected::Ready)
        .ok_or_else(|| format!("{} is not connected after {} minutes: try again, or check it on console.mistral.ai", c.display_name, CONNECT_MAX_MS / 60_000))
}

/// A forced fetch every [`CONNECT_POLL_MS`] until connector `id` is in
/// the catalog and `done` holds for it (true), or [`CONNECT_MAX_MS`]
/// passed (false): [`connect`]'s and [`connect_account`]'s one wait.
pub fn poll(w: &World<'_>, fetch: &Fetch<'_>, done: &dyn Fn(&ConnectorEntry) -> bool, id: &PluginId) -> bool {
    let t0 = (w.now_ms)();
    loop {
        (w.sleep_ms)(CONNECT_POLL_MS);
        let now = (w.now_ms)();
        if load_with(w.cache, now, true, fetch).find(id).is_some_and(done) {
            return true;
        }
        if now.saturating_sub(t0) >= CONNECT_MAX_MS {
            return false;
        }
    }
}

// ---- his accounts on a connector ----

/// The first free name of a new account (`account-2`, `account-3`...)
/// among `accounts`: what "connect another account" asks Mistral for
/// (the title he gives after the login is his name for it).
pub fn next_account(accounts: &[ConnectorAccount]) -> String {
    (2..).map(|n| format!("account-{n}")).find(|n| !accounts.iter().any(|a| &a.name == n)).unwrap_or_default()
}

/// Whether `name` is listed on `c` and its login works or is not checked
/// yet (an invalid one waits for its reconnect).
pub fn account_works(c: &ConnectorEntry, name: &str) -> bool {
    c.accounts.iter().any(|a| a.name == name && a.status != AccountStatus::Invalid)
}

/// Log in account `name` of ready connector `id` (his click: another
/// account, or one to reconnect): Mistral's auth URL for that connection
/// in his browser, then [`poll`] until it is listed and works.
pub fn connect_account(w: &World<'_>, id: &PluginId, name: &str) -> Result<(), String> {
    let fetch = Fetch { bootstrap: &|| fetch(w.base, w.key), accounts: &|raw| fetch_accounts(w.base, w.key, raw) };
    let cat = load_with(w.cache, (w.now_ms)(), false, &fetch);
    let Some(c) = cat.find(id).filter(|c| c.auth == ConnectorAuth::Ready).cloned() else {
        return Err(format!("{id} is not connected: connect it first"));
    };
    let url = auth_url(w.base, w.key, &c.id, Some(name)).map_err(|e| format!("{}: no login page from Mistral ({e})", c.display_name))?;
    (w.open)(&url)?;
    (w.say)(&format!("opened {}'s login in your browser; waiting for you to finish there", c.display_name));
    poll(w, &fetch, &|e| account_works(e, name), id)
        .then_some(())
        .ok_or_else(|| format!("{}: the account is not connected after {} minutes: try again", c.display_name, CONNECT_MAX_MS / 60_000))
}

/// What he does to one of his accounts, besides logging it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountChange {
    /// his name for it
    Rename(String),
    /// the account a call takes when it names none
    MakeDefault,
    /// gone from Mistral (final: the clients ask him first)
    Remove,
}

/// One change to account `name` of connector `raw` at `base`: a PATCH of
/// `/v1/connectors/{id}/user/credentials` (its title, or `is_default`),
/// or the DELETE of `…/user/credentials/{name}`.
pub fn change_account(base: &str, key: &str, raw: &str, name: &str, change: &AccountChange) -> Result<(), String> {
    let at = format!("{}/v1/connectors/{}/user/credentials", base.trim_end_matches('/'), path_part(raw));
    match change {
        AccountChange::Rename(title) => request("PATCH", &at, key, json!({"name": name, "title": title.trim()}).to_string().as_bytes()),
        AccountChange::MakeDefault => request("PATCH", &at, key, json!({"name": name, "is_default": true}).to_string().as_bytes()),
        AccountChange::Remove => request("DELETE", &format!("{at}/{}", path_part(name)), key, b""),
    }
    .map(|_| ())
}

/// [`change_account`] in the real world (his key, Mistral or
/// `BISE_MISTRAL_URL`), then a forced fetch so the cache and the rows
/// follow. Blocks up to two requests' timeouts: run it on a thread.
pub fn change_account_now(id: &PluginId, name: &str, change: &AccountChange) -> Result<(), String> {
    let k = real_key()?;
    let cache = bise_home::Home::from_env().connectors_cache();
    change_account(&base_url(), &k, &raw_id(&cache, id)?, name, change)?;
    match load(&cache, &Key::Key(k), now_ms(), true).error {
        Some(e) => Err(format!("changed on Mistral, but the list was not read again: {e}")),
        None => Ok(()),
    }
}

/// [`connect_account`] in the real world; blocks up to [`CONNECT_MAX_MS`].
pub fn connect_account_now(id: &PluginId, name: &str, say: &dyn Fn(&str)) -> Result<(), String> {
    let k = real_key()?;
    let cache = bise_home::Home::from_env().connectors_cache();
    let base = base_url();
    let w = World { cache: &cache, key: &k, base: &base, now_ms: &now_ms, sleep_ms: &|ms| std::thread::sleep(Duration::from_millis(ms)), open: &open, say };
    connect_account(&w, id, name)
}

/// His Mistral key, or why there is none.
fn real_key() -> Result<String, String> {
    match key() {
        Key::Key(k) => Ok(k),
        Key::NoKey => Err(NO_KEY.into()),
        Key::Locked => Err(LOCKED.into()),
    }
}

/// [`connect`] in the real world: his key, bise's cache, Mistral (or
/// `BISE_MISTRAL_URL`), the wall clock, his browser. Blocks up to
/// [`CONNECT_MAX_MS`]: run it on a thread; `say` gets each step's line.
pub fn connect_now(id: &PluginId, say: &dyn Fn(&str)) -> Result<Connected, String> {
    let k = real_key()?;
    let cache = bise_home::Home::from_env().connectors_cache();
    let base = base_url();
    let w = World { cache: &cache, key: &k, base: &base, now_ms: &now_ms, sleep_ms: &|ms| std::thread::sleep(Duration::from_millis(ms)), open: &open, say };
    connect(&w, id)
}

/// A fetch now ([`load`] forced, his key, bise's cache): `/plugins
/// refresh`. Blocks up to the fetch's timeout: run it on a thread.
pub fn refresh() -> Catalog {
    load(&bise_home::Home::from_env().connectors_cache(), &key(), now_ms(), true)
}

/// The wall clock, in ms.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---- bise plugins refresh | connect ----

/// `bise plugins refresh`: fetch the catalog now. Exit code.
pub fn main_refresh() -> i32 {
    use bise_home::style::Style;
    let cat = refresh();
    let ready = cat.connectors.iter().filter(|c| c.auth == ConnectorAuth::Ready).count();
    let line = format!("{} Mistral connectors, {} connected", cat.connectors.len(), ready);
    match &cat.error {
        None => {
            println!("{}", Style::stdout().ok(&line));
            0
        }
        Some(e) => {
            eprintln!("{}", Style::stderr().fail(&format!("{e} (kept the last catalog: {line})")));
            1
        }
    }
}

/// `bise plugins account mistral:<name> connect|default|remove <account>
/// | rename <account> <title>`. Exit code.
pub fn main_account(args: &[String]) -> i32 {
    use bise_home::style::Style;
    let err = |e: &str| eprintln!("{}", Style::stderr().fail(e));
    let usage = "usage: bise plugins account mistral:<name> connect|default|remove <account> | rename <account> <title>";
    let (Some(id), Some(verb), Some(account)) = (args.first(), args.get(1), args.get(2)) else {
        err(usage);
        return 2;
    };
    let id = match PluginId::parse(id) {
        Ok(id @ PluginId::Mistral { .. }) => id,
        _ => {
            err(&format!("{id} is not a Mistral connector (mistral:<name>)"));
            return 2;
        }
    };
    let change = match (verb.as_str(), args.get(3)) {
        ("connect", _) => None,
        ("default", _) => Some(AccountChange::MakeDefault),
        ("remove", _) => Some(AccountChange::Remove),
        ("rename", Some(t)) if !t.trim().is_empty() => Some(AccountChange::Rename(t.clone())),
        _ => {
            err(usage);
            return 2;
        }
    };
    let r = match &change {
        None => connect_account_now(&id, account, &|s| println!("{}", Style::stdout().dim(s))),
        Some(c) => change_account_now(&id, account, c),
    };
    match r {
        Ok(()) => {
            println!("{}", Style::stdout().ok(&format!("{id}: {account} {}", match change {
                None => "connected",
                Some(AccountChange::MakeDefault) => "is the default now",
                Some(AccountChange::Remove) => "removed",
                Some(AccountChange::Rename(_)) => "renamed",
            })));
            0
        }
        Err(e) => {
            err(&e);
            1
        }
    }
}

/// `bise plugins connect mistral:<name>`. Exit code.
pub fn main_connect(arg: Option<&str>) -> i32 {
    use bise_home::style::Style;
    let err = |e: &str| {
        eprintln!("{}", Style::stderr().fail(e));
    };
    let Some(arg) = arg else {
        err("usage: bise plugins connect mistral:<name>");
        return 2;
    };
    let id = match PluginId::parse(arg) {
        Ok(id @ PluginId::Mistral { .. }) => id,
        Ok(_) => {
            err(&format!("{arg} is not a Mistral connector (mistral:<name>)"));
            return 2;
        }
        Err(e) => {
            err(&e);
            return 2;
        }
    };
    match connect_now(&id, &|s| println!("{}", Style::stdout().dim(s))) {
        Ok(Connected::Ready) => {
            println!("{}", Style::stdout().ok(&format!("{id} connected: your agents get its tools at their next call")));
            0
        }
        Ok(Connected::Already) => {
            println!("{}", Style::stdout().ok(&format!("{id} is already connected")));
            0
        }
        Err(e) => {
            err(&e);
            1
        }
    }
}
