//! Which browser profile an extension runs in (several Chrome profiles
//! can each have the bise extension; each opens its own native port).
//!
//! The extension has no API that names its profile, so it proves it: at
//! each start it writes a fresh nonce in its `chrome.storage.local` and
//! says it in its hello (`hello.profile_nonce`, C4). Chrome keeps that
//! storage in `<user data dir>/<profile dir>/Local Extension Settings/<id>/`
//! (LevelDB: a fresh write lands in its `.log` as it is; a compacted
//! `.ldb` block may be compressed), so the profile dir whose files hold
//! the nonce is the one that spoke. Its name ("pro") and account email
//! come from the user data dir's `Local State` (`profile.info_cache`).
//! Not found: the profile is unknown, never guessed. The native host
//! (`host.rs`) does this once per hello; the broker routes by it.
//!
//! [`read_user_data`] is the one effect (the file reads); the rest is pure.

use std::path::Path;

use serde_json::{json, Value};

/// One browser profile, as bise names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// its folder in the user data dir ("Default", "Profile 1"): the key
    pub dir: String,
    /// the name Chrome shows ("pro"); its dir when Local State has none
    pub name: String,
    /// the Google account signed in to it, when one is: a match key for
    /// `open`'s `profile` and a line in Settings, never in tool results
    pub email: Option<String>,
}

impl Profile {
    /// As the hello and state.json carry it (with the email).
    pub fn to_json(&self) -> Value {
        let mut v = json!({"dir": self.dir, "name": self.name});
        if let Some(e) = &self.email {
            v["email"] = json!(e);
        }
        v
    }

    pub fn from_json(v: &Value) -> Option<Profile> {
        let dir = v.get("dir")?.as_str().filter(|d| !d.is_empty())?.to_string();
        let name = v.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(&dir).to_string();
        let email = v.get("email").and_then(Value::as_str).filter(|e| !e.is_empty()).map(String::from);
        Some(Profile { dir, name, email })
    }

    /// `wanted` names it: its name, its account email or its dir, any case.
    pub fn named(&self, wanted: &str) -> bool {
        let w = wanted.trim();
        !w.is_empty()
            && (self.name.eq_ignore_ascii_case(w)
                || self.dir.eq_ignore_ascii_case(w)
                || self.email.as_deref().is_some_and(|e| e.eq_ignore_ascii_case(w)))
    }
}

/// A nonce the extension may send: `bise-` then 16 to 64 of `[0-9a-z]`
/// (anything else is never searched for).
pub fn valid_nonce(n: &str) -> bool {
    n.strip_prefix("bise-").is_some_and(|r| (16..=64).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()))
}

/// One profile dir's extension storage: its LevelDB files' names and bytes.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub dir: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// What [`find`] reads in a user data dir.
#[derive(Clone, Debug, Default)]
pub struct UserData {
    /// the profiles that have the extension's storage, by dir name
    pub stores: Vec<Store>,
    /// `Local State`'s bytes
    pub local_state: Option<Vec<u8>>,
}

/// The largest storage file read (an extension's own settings are small).
const MAX_FILE: u64 = 16 * 1024 * 1024;

/// The effect: each profile's `Local Extension Settings/<extension_id>`
/// `.log` and `.ldb` files, and `Local State`, under `user_data`.
pub fn read_user_data(user_data: &Path, extension_id: &str) -> UserData {
    let mut stores = Vec::new();
    for d in std::fs::read_dir(user_data).into_iter().flatten().flatten() {
        let Some(dir) = d.file_name().to_str().map(String::from) else { continue };
        let Ok(files) = std::fs::read_dir(d.path().join("Local Extension Settings").join(extension_id)) else { continue };
        let mut store = Store { dir, files: Vec::new() };
        for f in files.flatten() {
            let Some(name) = f.file_name().to_str().map(String::from) else { continue };
            let leveldb = name.ends_with(".log") || name.ends_with(".ldb");
            if leveldb && f.metadata().is_ok_and(|m| m.len() <= MAX_FILE) {
                if let Ok(b) = std::fs::read(f.path()) {
                    store.files.push((name, b));
                }
            }
        }
        stores.push(store);
    }
    stores.sort_by(|a, b| a.dir.cmp(&b.dir));
    UserData { stores, local_state: std::fs::read(user_data.join("Local State")).ok() }
}

/// The profile dir whose storage holds `nonce`: the `.log` files first
/// (where a fresh write is, as it is), then the `.ldb` ones.
pub fn dir_with_nonce(data: &UserData, nonce: &str) -> Option<String> {
    if !valid_nonce(nonce) {
        return None;
    }
    let needle = nonce.as_bytes();
    let holds = |ext: &str| {
        data.stores.iter().find(|s| s.files.iter().any(|(n, b)| n.ends_with(ext) && b.windows(needle.len()).any(|w| w == needle))).map(|s| s.dir.clone())
    };
    holds(".log").or_else(|| holds(".ldb"))
}

/// The profile `dir` as `Local State` names it.
pub fn info(local_state: Option<&[u8]>, dir: &str) -> Profile {
    let state = local_state.and_then(|b| serde_json::from_slice::<Value>(b).ok());
    let entry = state.as_ref().and_then(|s| s.get("profile")?.get("info_cache")?.get(dir));
    let get = |k: &str| entry.and_then(|e| e.get(k)).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Profile { dir: dir.to_string(), name: get("name").unwrap_or_else(|| dir.to_string()), email: get("user_name") }
}

/// The profile that wrote `nonce`; None: unknown (not written yet, or
/// another user data dir).
pub fn find(data: &UserData, nonce: &str) -> Option<Profile> {
    dir_with_nonce(data, nonce).map(|d| info(data.local_state.as_deref(), &d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A real throwaway Chrome user data dir's files (headless Chrome 157,
    /// the extension loaded, its worker's hello written; see
    /// tests/fixtures/chrome-profiles/README.md): `Default` wrote
    /// [`NONCE_DEFAULT`], `Profile 1` (another run's storage)
    /// [`NONCE_P1`].
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chrome-profiles")
    }
    const NONCE_DEFAULT: &str = include_str!("../tests/fixtures/chrome-profiles/nonce-default.txt");
    const NONCE_P1: &str = include_str!("../tests/fixtures/chrome-profiles/nonce-profile1.txt");

    #[test]
    fn finds_the_profile_that_wrote_the_nonce_in_real_files() {
        let data = read_user_data(&fixture(), crate::browsers::EXTENSION_ID);
        assert_eq!(data.stores.iter().map(|s| s.dir.as_str()).collect::<Vec<_>>(), ["Default", "Profile 1"]);
        let d = find(&data, NONCE_DEFAULT.trim()).unwrap();
        assert_eq!(d, info(data.local_state.as_deref(), "Default"));
        assert_ne!(d.name, "Default", "Local State names it");
        let p1 = find(&data, NONCE_P1.trim()).unwrap();
        assert_eq!(p1.dir, "Profile 1");
        // not in Local State's cache: named by its dir, never guessed
        assert_eq!((p1.name.as_str(), p1.email.as_deref()), ("Profile 1", None));
        // a nonce nobody wrote, not a nonce, another extension: unknown
        assert_eq!(find(&data, "bise-99999999999999999999"), None);
        assert_eq!(find(&data, "Default"), None);
        assert!(read_user_data(&fixture(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").stores.is_empty());
    }

    fn store(dir: &str, files: &[(&str, &str)]) -> Store {
        Store { dir: dir.into(), files: files.iter().map(|(n, b)| (n.to_string(), b.as_bytes().to_vec())).collect() }
    }

    #[test]
    fn the_log_wins_over_an_old_ldb_and_other_files_never_count() {
        let n = "bise-0123456789abcdef0123";
        let data = UserData {
            stores: vec![
                // an old copy in a compacted table (another profile restored from a backup)
                store("Default", &[("000005.ldb", n), ("LOCK", n)]),
                store("Profile 2", &[("000003.log", &format!("\0\x01bise_profile\"{}\"", n))]),
            ],
            local_state: Some(br#"{"profile":{"info_cache":{"Profile 2":{"name":" pro ","user_name":"gabriel@work.com"}}}}"#.to_vec()),
        };
        let p = find(&data, n).unwrap();
        assert_eq!(p, Profile { dir: "Profile 2".into(), name: "pro".into(), email: Some("gabriel@work.com".into()) });
        let only_lock = UserData { stores: vec![store("Default", &[("LOCK", n), ("MANIFEST-000001", n)])], local_state: None };
        assert_eq!(find(&only_lock, n), None);
        let ldb = UserData { stores: vec![store("Default", &[("000005.ldb", n)])], local_state: None };
        assert_eq!(find(&ldb, n).map(|p| p.name), Some("Default".into()));
    }

    #[test]
    fn named_by_name_email_or_dir_any_case() {
        let p = Profile { dir: "Profile 1".into(), name: "pro".into(), email: Some("gabriel@work.com".into()) };
        for w in ["pro", "PRO", " pro ", "Gabriel@Work.com", "profile 1"] {
            assert!(p.named(w), "{}", w);
        }
        for w in ["", "perso", "gab@perso.fr", "pr"] {
            assert!(!p.named(w), "{}", w);
        }
        assert_eq!(Profile::from_json(&p.to_json()), Some(p));
        assert_eq!(Profile::from_json(&json!({"dir": "Default"})).map(|p| p.name), Some("Default".into()));
        assert_eq!(Profile::from_json(&json!({"name": "x"})), None);
    }

    #[test]
    fn nonces() {
        assert!(valid_nonce("bise-0123456789abcdef"));
        for bad in ["bise-short", "bise-0123456789ABCDEF", "0123456789abcdef0123", "bise-0123456789abcdef/.."] {
            assert!(!valid_nonce(bad), "{}", bad);
        }
    }
}
