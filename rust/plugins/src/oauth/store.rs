//! The OAuth store of remote MCP servers (split out of oauth.rs): one
//! login per server URL, read and written through `bise_secrets` (its
//! file, or the macOS keychain with a stub at the file's path), the
//! lock every bridge shares while a token changes, and the logout.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::{form, loopback, post, Config};
use crate::http::Url;

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
    /// a 403 `insufficient_scope` asked for these: the next login asks
    /// for them too (step-up)
    pub wanted_scope: Option<String>,
    /// RFC 7009: where a logout revokes the tokens
    pub revocation_endpoint: Option<String>,
}

impl std::fmt::Debug for Saved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Saved {{ resource: {:?}, issuer: {:?}, token: {} }}", self.resource, self.issuer, self.access_token.is_some())
    }
}

pub(super) fn now() -> u64 {
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
        put("wanted_scope", json!(self.wanted_scope));
        put("revocation_endpoint", json!(self.revocation_endpoint));
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
            wanted_scope: s("wanted_scope"),
            revocation_endpoint: s("revocation_endpoint"),
        })
    }

    /// A token that is still good for a minute.
    pub fn fresh(&self) -> bool {
        self.access_token.is_some() && self.expires_at.is_none_or(|e| e > now() + 60)
    }
}

/// The server's login (through [`bise_secrets`]: its file, or the macOS
/// keychain when the file is a stub). None: no login, or it can't be
/// read now (a locked keychain): [`load_now`] tells them apart.
pub fn load(dir: &Path, resource: &str) -> Option<Saved> {
    load_now(dir, resource).ok().flatten()
}

/// The server's login; Err: it can't be read now (a locked keychain),
/// which is never "no login".
pub fn load_now(dir: &Path, resource: &str) -> Result<Option<Saved>, String> {
    let text = bise_secrets::read(&file_of(dir, resource)).map_err(|e| e.to_string())?;
    let v: Option<Value> = text.and_then(|t| serde_json::from_str(&t).ok());
    Ok(v.and_then(|v| Saved::from_json(&v)).filter(|s| s.resource == resource))
}

/// Where `[secrets] store` says ([`bise_secrets::write`]): the keychain,
/// else the file, by rename, 0600, in a 0700 folder.
pub fn save(dir: &Path, s: &Saved) -> Result<(), String> {
    let f = file_of(dir, &s.resource);
    let text = s.to_json().to_string();
    bise_secrets::write(&f, &text, &|| save_file(dir, &f, &text)).map_err(|e| format!("cannot save the login: {}", e))
}

fn save_file(dir: &Path, f: &Path, text: &str) -> std::io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    std::fs::create_dir_all(dir).map_err(|e| std::io::Error::other(format!("cannot create bise's secrets folder: {}", e)))?;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    let tmp = f.with_extension(format!("tmp{}", std::process::id()));
    let res = (|| {
        let mut w = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        w.write_all(text.as_bytes())?;
        w.sync_all()?;
        std::fs::rename(&tmp, f)
    })();
    res.inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
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
            let _ = bise_secrets::remove(&file_of(dir, resource));
        }
    }
}

/// Log out: revoke the tokens at the login server when it has an RFC
/// 7009 endpoint (the refresh token, then the access token; best effort,
/// 5 s each), then forget them, the client kept. Returns whether the
/// server confirmed the revocation (None: it has no endpoint).
pub fn logout(dir: &Path, resource: &str, config: &Config) -> Option<bool> {
    let s = load(dir, resource)?;
    let endpoint = s.revocation_endpoint.clone().filter(|e| Url::parse(e).is_ok_and(|u| u.tls || loopback(&u.host)));
    let mut confirmed = None;
    if let Some(ep) = endpoint {
        let secret = config.client_secret.clone().or_else(|| s.client_secret.clone());
        for (tok, hint) in [(&s.refresh_token, "refresh_token"), (&s.access_token, "access_token")] {
            let Some(t) = tok else { continue };
            let mut pairs = vec![("token", t.as_str()), ("token_type_hint", hint), ("client_id", s.client_id.as_str())];
            if let Some(x) = &secret {
                pairs.push(("client_secret", x.as_str()));
            }
            let ok = matches!(post(&ep, "application/x-www-form-urlencoded", &form(&pairs)), Ok((200, _)));
            confirmed = Some(confirmed.unwrap_or(true) && ok);
        }
    }
    forget(dir, resource, true);
    confirmed
}

/// The store's lock for one server (every bridge of every agent shares
/// it): held while a token is refreshed.
pub struct Lock(std::fs::File);

pub(super) fn lock(dir: &Path, resource: &str) -> Option<Lock> {
    std::fs::create_dir_all(dir).ok()?;
    lock_file(&file_of(dir, resource))
}

/// The lock of the store file `file` (`bise secrets keychain` moves it
/// under it).
pub fn lock_file(file: &Path) -> Option<Lock> {
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(file.with_extension("lock")).ok()?;
    f.lock().ok()?;
    Some(Lock(f))
}

/// The store's login files (not their locks, nor a write's temp file).
pub fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    v.sort();
    v
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

