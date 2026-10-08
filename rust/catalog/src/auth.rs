//! API keys (BISE-143): the key store `auth.json` and where each
//! provider's key comes from.
//!
//! Resolution, per provider (its `key_env`, from the catalog):
//!   1. `auth.json` (`bise login <provider>`, the first run's key step):
//!      what you give bise is what bise uses (BISE-269); `bise logout
//!      <provider>` goes back to the environment;
//!   2. the environment: `key_env`, then its aliases ([`ALIASES`]);
//!   3. the old `.env` files (`~/.bend-harness/.env`, `~/.vibe/.env`),
//!      `key_env` then its aliases, first file first.
//!
//! The Bend runtime reads one variable per provider (`getenv(key_env)`,
//! BISE-141), so at start the harness sets `key_env` in its own process
//! ([`Resolution::exports`]) and every REPL it starts inherits it. A key
//! is never written anywhere but `auth.json`, and no message of this
//! module holds one.
//!
//! `auth.json`, the OpenCode layout: `{"<provider>": {"type": "api",
//! "key": "..."}}`. Other entries (a future OAuth type) are kept as they
//! are. The file is 0600 in a 0700 directory, replaced atomically; with
//! `[secrets] store = "keychain"` (macOS, `bise secrets keychain on`:
//! [`crate::secrets_cli`]) its text is in the keychain and the file is a
//! stub (`bise_secrets` reads and writes it; a locked keychain is
//! [`Unreadable::Locked`], never an empty store). Its lock,
//! `auth.json.lock`, is here too ([`lock`]).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Catalog, Provider};

/// Other names a provider's key goes by in the environment and the
/// `.env` files: `key_env` -> the aliases, in order. The runtime only
/// reads `key_env`, so a key found under an alias is exported as it.
pub const ALIASES: &[(&str, &[&str])] = &[("GEMINI_API_KEY", &["GOOGLE_API_KEY"])];

/// The env names of a provider's key: `key_env`, then its aliases.
pub fn env_names(key_env: &str) -> Vec<&str> {
    let mut v = vec![key_env];
    for (k, alias) in ALIASES {
        if *k == key_env {
            v.extend(alias.iter().copied());
        }
    }
    v
}

// ---- auth.json ----

/// Why auth.json can't be read now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// it is in the macOS keychain, and the keychain is locked
    Locked,
    /// one line saying why (no content of the file)
    Other(String),
}

/// The key store. Reading never fails on a missing file (no keys);
/// a file that is not a JSON object is an error (never overwritten).
#[derive(Clone, Debug, Default)]
pub struct Store {
    entries: serde_json::Map<String, serde_json::Value>,
}

impl Store {
    /// Parse the text of `auth.json`. The error holds no content of the
    /// file (only the syntax position).
    pub fn parse(text: &str) -> Result<Store, String> {
        if text.trim().is_empty() {
            return Ok(Store::default());
        }
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(serde_json::Value::Object(entries)) => Ok(Store { entries }),
            Ok(_) => Err("not a JSON object".into()),
            Err(e) => Err(format!("not valid JSON (line {}, column {})", e.line(), e.column())),
        }
    }

    /// Read `path` (through [`bise_secrets`]: its file, or the macOS
    /// keychain when the file is a stub); a missing file is an empty
    /// store. A locked keychain is an error, never an empty store.
    pub fn read(path: &Path) -> Result<Store, String> {
        Store::read_or_locked(path).map_err(|e| match e {
            Unreadable::Locked => format!("{}: {}", path.display(), bise_secrets::ReadError::Locked),
            Unreadable::Other(e) => e,
        })
    }

    /// [`Store::read`], a locked keychain told apart.
    pub fn read_or_locked(path: &Path) -> Result<Store, Unreadable> {
        match bise_secrets::read(path) {
            Ok(Some(text)) => Store::parse(&text).map_err(|e| Unreadable::Other(format!("{}: {}", path.display(), e))),
            Ok(None) => Ok(Store::default()),
            Err(bise_secrets::ReadError::Locked) => Err(Unreadable::Locked),
            Err(e) => Err(Unreadable::Other(format!("{}: {}", path.display(), e))),
        }
    }

    /// The API key stored for `provider` (type "api", a non-empty key).
    pub fn key(&self, provider: &str) -> Option<&str> {
        let e = self.entries.get(provider)?.as_object()?;
        if e.get("type").and_then(|t| t.as_str()) != Some("api") {
            return None;
        }
        e.get("key").and_then(|k| k.as_str()).filter(|k| !k.trim().is_empty())
    }

    /// The `via` of `provider`'s API key (`openrouter-login`: a sign-in
    /// made it).
    pub fn via(&self, provider: &str) -> Option<&str> {
        self.entries.get(provider)?.get("via")?.as_str()
    }

    /// The providers with an entry, sorted.
    pub fn providers(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        v.sort();
        v
    }

    pub fn set(&mut self, provider: &str, key: &str) {
        self.entries.insert(
            provider.to_string(),
            serde_json::json!({ "type": "api", "key": key }),
        );
    }

    /// An API key with where it came from (`"via": "openrouter-login"`).
    pub fn set_via(&mut self, provider: &str, key: &str, via: &str) {
        self.entries.insert(
            provider.to_string(),
            serde_json::json!({ "type": "api", "key": key, "via": via }),
        );
    }

    /// The `"type": "oauth"` entry of `provider` (a sign-in: signed in,
    /// or signed out with its client kept).
    pub fn oauth(&self, provider: &str) -> Option<OAuth> {
        let e = self.entries.get(provider)?.as_object()?;
        if e.get("type").and_then(|t| t.as_str()) != Some("oauth") {
            return None;
        }
        Some(OAuth::from_json(e))
    }

    /// Write `provider`'s oauth entry; keys of the old entry this module
    /// does not know stay as they were.
    pub fn set_oauth(&mut self, provider: &str, o: &OAuth) {
        let mut e = match self.entries.get(provider) {
            Some(serde_json::Value::Object(old)) if old.get("type").and_then(|t| t.as_str()) == Some("oauth") => old.clone(),
            _ => serde_json::Map::new(),
        };
        for k in OAuth::KEYS {
            e.remove(*k);
        }
        e.extend(o.to_json());
        self.entries.insert(provider.to_string(), serde_json::Value::Object(e));
    }

    /// Sign `provider` out: its tokens dropped, its client kept (OpenAI
    /// asks to reuse the issued client: `client_id`, `email`, `subject`
    /// stay). `expired`: the sign-in ended by itself (a refresh refused),
    /// so screens say "sign-in expired" rather than "signed out".
    /// Whether there was a sign-in to end.
    pub fn sign_out(&mut self, provider: &str, expired: bool) -> bool {
        let Some(o) = self.oauth(provider) else { return false };
        let had = o.signed_in();
        let kept = OAuth { client_id: o.client_id, email: o.email, subject: o.subject, expired, ..OAuth::default() };
        self.set_oauth(provider, &kept);
        had
    }

    /// Whether there was an entry.
    pub fn remove(&mut self, provider: &str) -> bool {
        self.entries.remove(provider).is_some()
    }

    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.entries).unwrap_or_else(|_| "{}".into());
        s.push('\n');
        s
    }

    /// Write to `path`: its directory created 0700 when missing (an
    /// existing one is left as it is), the file 0600, atomic (a temp file
    /// in the same directory, renamed over it).
    /// Where it goes follows `[secrets] store` ([`bise_secrets::write`]):
    /// the keychain, else this file.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        bise_secrets::write(path, &self.to_json(), &|| self.write_file(path))
    }

    fn write_file(&self, path: &Path) -> std::io::Result<()> {
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        create_private_dir(dir)?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("auth.json");
        let tmp = dir.join(format!(".{}.{}.tmp", name, std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let res = (|| {
            let mut f = open_private(&tmp)?;
            f.write_all(self.to_json().as_bytes())?;
            f.sync_all()?;
            std::fs::rename(&tmp, path)
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        res
    }
}

// ---- the lock (moved from chatgpt.rs) ----

/// `auth.json.lock`, held while the tokens change (a refresh, a sign-in's
/// save, a sign-out): every bise process shares it.
pub(crate) struct Lock(std::fs::File);

pub(crate) fn lock(path: &Path) -> Result<Lock, String> {
    if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        create_private_dir(d).map_err(|e| format!("cannot create {}: {}", d.display(), e))?;
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

/// An `"type": "oauth"` entry of auth.json (OpenCode's layout, the
/// fields of OpenAI's credential record): a sign-in's account, client and
/// tokens. `expires` is in ms since the epoch (OpenCode's field). Debug
/// never shows a token.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct OAuth {
    /// the issued client (`oaiapp_...`), kept on sign-out
    pub client_id: String,
    pub email: String,
    /// the ID token's validated `sub`
    pub subject: String,
    /// the ChatGPT plan ("plus", "pro"), when the ID token says it
    pub plan: String,
    pub access: String,
    pub refresh: String,
    /// ms since the epoch; 0 = unknown
    pub expires: u64,
    /// kept for the next sign-in's `id_token_hint`
    pub id_token: String,
    pub scopes: Vec<String>,
    /// when the refresh token was last given (UTC, RFC 3339): the
    /// sign-in lasts 30 days from there
    pub saved_at: String,
    /// signed out by a refused refresh (the screens' "sign-in expired")
    pub expired: bool,
}

impl std::fmt::Debug for OAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OAuth {{ client_id: {:?}, email: {:?}, plan: {:?}, signed_in: {}, expires: {}, expired: {} }}",
            self.client_id,
            self.email,
            self.plan,
            self.signed_in(),
            self.expires,
            self.expired
        )
    }
}

impl OAuth {
    /// The keys this module writes (the others of an entry are kept).
    const KEYS: &'static [&'static str] = &[
        "type", "client_id", "email", "subject", "plan", "access", "refresh", "expires", "id_token", "scopes", "saved_at", "expired",
    ];

    fn from_json(e: &serde_json::Map<String, serde_json::Value>) -> OAuth {
        let s = |k: &str| e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        OAuth {
            client_id: s("client_id"),
            email: s("email"),
            subject: s("subject"),
            plan: s("plan"),
            access: s("access"),
            refresh: s("refresh"),
            expires: e.get("expires").and_then(|v| v.as_u64()).unwrap_or(0),
            id_token: s("id_token"),
            scopes: e
                .get("scopes")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            saved_at: s("saved_at"),
            expired: e.get("expired").and_then(|v| v.as_bool()).unwrap_or(false),
        }
    }

    fn to_json(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut o = serde_json::Map::new();
        o.insert("type".into(), "oauth".into());
        for (k, v) in [
            ("client_id", &self.client_id),
            ("email", &self.email),
            ("subject", &self.subject),
            ("plan", &self.plan),
            ("access", &self.access),
            ("refresh", &self.refresh),
        ] {
            if !v.is_empty() {
                o.insert(k.into(), v.clone().into());
            }
        }
        if self.expires > 0 {
            o.insert("expires".into(), self.expires.into());
        }
        if !self.id_token.is_empty() {
            o.insert("id_token".into(), self.id_token.clone().into());
        }
        if !self.scopes.is_empty() {
            o.insert("scopes".into(), self.scopes.clone().into());
        }
        if !self.saved_at.is_empty() {
            o.insert("saved_at".into(), self.saved_at.clone().into());
        }
        if self.expired {
            o.insert("expired".into(), true.into());
        }
        o
    }

    /// It has a refresh token (or at least an access token): calls can
    /// get a token without the browser.
    pub fn signed_in(&self) -> bool {
        !self.refresh.is_empty() || !self.access.is_empty()
    }

    /// It was granted the ChatGPT plan's use (`chatgpt.tokens.use.direct`).
    pub fn plan_scope(&self) -> bool {
        self.scopes.iter().any(|s| s == "chatgpt.tokens.use.direct")
    }
}

#[cfg(unix)]
fn open_private(p: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)
}

#[cfg(not(unix))]
fn open_private(p: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(p)
}

/// Create `dir` and its missing parents; the ones created here are 0700
/// (an existing directory keeps its mode).
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_private_dir(parent)?;
    }
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    match b.create(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        r => r,
    }
}

/// The permission bits of `path` when others than its owner may read or
/// write it (a warning); None when private or missing.
pub fn loose_mode(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
        (m & 0o077 != 0).then_some(m)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

// ---- the old .env files ----

/// Run an explicitly requested key check. Never include output in an error.
/// Listing providers must not call this function.
pub fn command_key(command: &str, timeout: std::time::Duration) -> Result<String, String> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let error = || "key_command failed or printed no valid key (config.toml)".to_string();
    let mut child = Command::new("/bin/sh")
        .args(["-c", command])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|_| error())?;
    let stdout = child.stdout.take().expect("piped stdout");
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            _ => break None,
        }
    };
    // Stop descendants too: they must not keep the stdout pipe open.
    // SAFETY: this child was started in its own process group.
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
    let _ = child.wait();
    let bytes = reader.join().map_err(|_| error())?.map_err(|_| error())?;
    if !status.is_some_and(|s| s.success()) || bytes.len() > 8192 {
        return Err(error());
    }
    let text = String::from_utf8(bytes).map_err(|_| error())?;
    let key = text.trim();
    if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(error());
    }
    Ok(key.to_string())
}

/// One `.env` file: its path and its `KEY=VALUE` lines.
#[derive(Clone, Debug, Default)]
pub struct EnvFile {
    pub path: PathBuf,
    pub vars: BTreeMap<String, String>,
}

impl EnvFile {
    /// `KEY=VALUE` lines: `#` comments, `export `, quotes; the first
    /// line of a key wins.
    pub fn parse(path: PathBuf, text: &str) -> EnvFile {
        let mut vars = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if !k.is_empty() {
                vars.entry(k.to_string()).or_insert_with(|| v.to_string());
            }
        }
        EnvFile { path, vars }
    }

    /// The files that exist among `paths`, in order.
    pub fn read_all(paths: &[PathBuf]) -> Vec<EnvFile> {
        paths
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok().map(|t| EnvFile::parse(p.clone(), &t)))
            .collect()
    }
}

// ---- resolution ----

/// Where a provider's key comes from (never the key itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum From {
    /// the environment, under this name
    Env(String),
    /// auth.json
    AuthFile,
    /// an old .env file, under this name
    EnvFile(PathBuf, String),
}

impl From {
    /// "env ANTHROPIC_API_KEY", "auth.json", "~/.vibe/.env (ANTHROPIC_API_KEY)"
    pub fn describe(&self, home: Option<&Path>) -> String {
        match self {
            From::Env(n) => format!("env {}", n),
            From::AuthFile => "auth.json".into(),
            From::EnvFile(p, n) => format!("{} ({})", tilde(p, home), n),
        }
    }
}

/// `path` with the home directory written `~`.
pub fn tilde(p: &Path, home: Option<&Path>) -> String {
    if let Some(rest) = home.and_then(|h| p.strip_prefix(h).ok()) {
        return format!("~/{}", rest.display());
    }
    p.display().to_string()
}

/// A provider's key, resolved. Debug never shows the key.
#[derive(Clone)]
pub struct Found {
    pub from: From,
    pub key: String,
}

impl std::fmt::Debug for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Found {{ from: {:?}, key: <{} bytes> }}", self.from, self.key.len())
    }
}

/// What a key lookup sees: the environment (the real one, before any
/// .env file is loaded into it), auth.json, the old .env files.
pub struct Keys<'a> {
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub store: &'a Store,
    pub files: &'a [EnvFile],
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl Keys<'_> {
    /// The key of the provider `id` whose key variable is `key_env`
    /// (None: no key found, or the provider needs none).
    pub fn find(&self, id: &str, key_env: &str) -> Option<Found> {
        if key_env.is_empty() {
            return None;
        }
        let names = env_names(key_env);
        if let Some(key) = non_empty(self.store.key(id).map(str::to_string)) {
            return Some(Found { from: From::AuthFile, key });
        }
        for n in &names {
            if let Some(key) = non_empty((self.env)(n)) {
                return Some(Found { from: From::Env(n.to_string()), key });
            }
        }
        for f in self.files {
            for n in &names {
                if let Some(key) = non_empty(f.vars.get(*n).cloned()) {
                    return Some(Found { from: From::EnvFile(f.path.clone(), n.to_string()), key });
                }
            }
        }
        None
    }

    pub fn for_provider(&self, p: &Provider) -> Option<Found> {
        self.find(&p.id, &p.key_env)
    }

    /// `p` can run a turn now (BISE-294): usable, and its key is found
    /// or it needs none (a local server). The model pickers list only
    /// these providers' models.
    pub fn ready(&self, p: &Provider) -> bool {
        if p.signs_in() {
            return p.needs.is_empty() && p.chats() && self.signed_in(p);
        }
        p.needs.is_empty() && p.chats() && (!p.key_command().is_empty() || p.key_env.is_empty() || self.for_provider(p).is_some())
    }

    /// `p` signs in (`auth = "chatgpt"`) and is signed in (auth.json has
    /// its tokens; no network: an expired one is found at the next call).
    pub fn signed_in(&self, p: &Provider) -> bool {
        p.signs_in() && self.store.oauth(&p.id).is_some_and(|o| o.signed_in())
    }

    /// The environment variable holding ANOTHER key for provider `id`
    /// than the one auth.json gives it (BISE-269: auth.json wins, so that
    /// one is unused; said once, so nobody wonders which key runs).
    pub fn shadowed(&self, id: &str, key_env: &str) -> Option<String> {
        let saved = non_empty(self.store.key(id).map(str::to_string))?;
        env_names(key_env)
            .into_iter()
            .find(|n| non_empty((self.env)(n)).is_some_and(|v| v != saved))
            .map(str::to_string)
    }

    /// Where `p`'s key comes from, for a list (never the key):
    /// "env MISTRAL_API_KEY", "auth.json", "auth.json · env MISTRAL_API_KEY
    /// holds another key, unused".
    pub fn source(&self, p: &Provider, home: Option<&Path>) -> Option<String> {
        if p.signs_in() {
            let o = self.store.oauth(&p.id).filter(|o| o.signed_in())?;
            return Some(match o.email.as_str() {
                "" => format!("{} sign-in", p.name),
                e => format!("{} sign-in ({})", p.name, e),
            });
        }
        if !p.key_command().is_empty() {
            return Some(crate::KEY_COMMAND_STATE.into());
        }
        let f = self.for_provider(p)?;
        let from = f.from.describe(home);
        Some(match self.shadowed(&p.id, &p.key_env) {
            Some(n) => format!("{} · env {} holds another key, unused", from, n),
            None => from,
        })
    }

    /// Every provider's key, in catalog order.
    pub fn resolve(&self, c: &Catalog) -> Resolution {
        let mut r = Resolution::default();
        for p in &c.providers {
            if let Some(f) = self.for_provider(p) {
                r.found.push((p.id.clone(), p.key_env.clone(), f));
            }
        }
        r
    }
}

/// The keys of all the providers.
#[derive(Clone, Debug, Default)]
pub struct Resolution {
    /// (provider id, key_env, found)
    pub found: Vec<(String, String, Found)>,
}

impl Resolution {
    /// The variables to set so the runtime's `getenv(key_env)` sees each
    /// key: every key_env whose key does not already come from the
    /// environment under that very name. Two providers sharing a
    /// key_env: the first one wins.
    pub fn exports(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (_, key_env, f) in &self.found {
            if f.from == From::Env(key_env.clone()) || out.iter().any(|(k, _)| k == key_env) {
                continue;
            }
            out.push((key_env.clone(), f.key.clone()));
        }
        out
    }

    /// The env of a REPL the hub spawns now (a `login` / `logout` since the
    /// hub started reaches the next REPL, no hub restart): `ours` = the
    /// names this process set itself at start (load_keys), each with the
    /// value the real environment had before (None: unset); the
    /// resolution that made `self` saw those values as "the environment".
    /// Each export is set; a provider's key_env we set at start and do not
    /// export now (logout) gets its real value back, or is removed. The
    /// real environment's other keys are inherited as they are.
    pub fn spawn_env(&self, c: &Catalog, ours: &[(String, Option<String>)]) -> Vec<(String, Option<String>)> {
        let ex = self.exports();
        let mut out: Vec<(String, Option<String>)> =
            ex.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect();
        for p in &c.providers {
            let k = &p.key_env;
            if k.is_empty() || out.iter().any(|(n, _)| n == k) {
                continue;
            }
            if let Some((_, before)) = ours.iter().find(|(n, _)| n == k) {
                out.push((k.clone(), before.clone()));
            }
        }
        out
    }
}
