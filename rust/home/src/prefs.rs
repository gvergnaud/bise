//! The user's preferences: one `prefs.json` in the bise layout, the
//! old files in the legacy one (`tui.json`, `hints.json`, `tip`,
//! `onboarded`). A [`Slot`] hides which: a JSON value in a file, the
//! whole file or one key of its object.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pref {
    /// `voice_mode_enabled`: bool.
    Voice,
    /// `theme`: "auto" | "light" | "dark".
    Theme,
    /// `hints`: `{hint key: true}`, the one-time hints seen.
    Hints,
    /// `tip`: the index of the last tip shown.
    Tip,
    /// `onboarded`: present once the onboarding was seen.
    Onboarded,
    /// `setup`: `{"asked": true, "repos": [root, …]}`, the setup card's
    /// answers (BISE-245: once per user, the repo part once per repo).
    Setup,
    /// `excluded_apps`: `[bundle id, …]`, the apps the Mac app never reads
    /// at fn (no title, URL, file, selection, text or shot; bise desktop
    /// S9, architect F). Unset: [`EXCLUDED_APPS_SEED`] ([`excluded_apps`]).
    ExcludedApps,
}

/// The apps never read at fn until he changes the list (bundle id, the
/// name the window shows): password managers, Keychain, Passwords,
/// System Settings, Messages. An id not here shows its last part.
pub const EXCLUDED_APPS_SEED: &[(&str, &str)] = &[
    ("com.agilebits.onepassword7", "1Password 7"),
    ("com.1password.1password", "1Password"),
    ("com.bitwarden.desktop", "Bitwarden"),
    ("com.dashlane.dashlanephone", "Dashlane"),
    ("com.apple.keychainaccess", "Keychain Access"),
    ("com.apple.Passwords", "Passwords"),
    ("com.apple.systempreferences", "System Settings"),
    ("com.apple.MobileSMS", "Messages"),
];

/// The prefs event's names of the seed's apps (`for_window`).
pub const APP_NAMES: &str = "app_names";

/// The `excluded_apps` value as read from its slot: his list when set
/// (an empty list included: he cleared it), else the seed.
pub fn excluded_apps(v: Option<Value>) -> Vec<String> {
    match v.as_ref().and_then(|v| v.as_array()) {
        Some(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        None => EXCLUDED_APPS_SEED.iter().map(|(id, _)| id.to_string()).collect(),
    }
}

/// The top keys of `prefs.json` the window may set (`prefs_set`,
/// architect m_9130): anything else is refused, never written.
pub const WINDOW_KEYS: &[&str] = &["onboarded", "theme", "voice", "context", "excluded_apps", "quiet", "shots_keep_days"];

/// The one-time hints the window may mark seen (`prefs_set {key:
/// "hints.<name>", value: true}`, S36): the TUI's own keys (tui hints.rs
/// `Hint::key`), in the same `hints` object, so a hint seen in either
/// never shows again. Only marking seen: never `hints` whole, never
/// another value (unseeing one is not the window's).
pub const WINDOW_HINTS: &[&str] = &["first_agent", "first_level3", "first_card", "first_steer", "first_yolo", "first_auto"];

/// The type a typed key keeps (a wrong one is refused).
fn typed(key: &str, v: &Value) -> Result<(), String> {
    let ok = match key {
        "onboarded" => v.is_boolean(),
        "theme" => v.as_str().is_some_and(|t| ["auto", "light", "dark", "system"].contains(&t)),
        "shots_keep_days" => v.as_u64().is_some(),
        "excluded_apps" => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("{key} can't be {v}"))
    }
}

/// `obj` with the dotted `key` (`quiet.call`) set to `value` (null: taken
/// out). Its top key must be one of [`WINDOW_KEYS`]; a typed key keeps
/// its type; a path through a non-object replaces it with an object.
pub fn set_dotted(obj: &Value, key: &str, value: Value) -> Result<Value, String> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("{key:?} is not a preference"));
    }
    let top = parts[0];
    if top == "hints" {
        match parts[..] {
            [_, name] if WINDOW_HINTS.contains(&name) && value == Value::Bool(true) => {}
            [_, name] if WINDOW_HINTS.contains(&name) => return Err(format!("hints.{name} can only be true")),
            _ => return Err(format!("{key} is not a hint the window marks")),
        }
    } else if !WINDOW_KEYS.contains(&top) {
        return Err(format!("{top} is not a preference the window sets"));
    }
    if parts.len() == 1 {
        if !value.is_null() {
            typed(top, &value)?;
        }
    } else if ["onboarded", "theme", "shots_keep_days", "excluded_apps"].contains(&top) {
        return Err(format!("{top} has no {}", parts[1..].join(".")));
    }
    let mut out = if obj.is_object() { obj.clone() } else { Value::Object(Default::default()) };
    let mut at = &mut out;
    for p in &parts[..parts.len() - 1] {
        if !at.get(*p).is_some_and(Value::is_object) {
            at[*p] = Value::Object(Default::default());
        }
        at = &mut at[*p];
    }
    let last = parts[parts.len() - 1];
    match (value.is_null(), at.as_object_mut()) {
        (true, Some(m)) => {
            m.remove(last);
        }
        _ => at[last] = value,
    }
    Ok(out)
}

/// What the window reads of `prefs.json` (`ev: prefs`): its object as is,
/// with `excluded_apps` resolved ([`excluded_apps`]: the seed when unset)
/// and `app_names`, the seed's id -> name (read-only: not a key the
/// window sets), so the window names the apps from this one table.
pub fn for_window(obj: Option<Value>) -> Value {
    let mut v = obj.filter(Value::is_object).unwrap_or_else(|| Value::Object(Default::default()));
    let apps = excluded_apps(v.get(Pref::ExcludedApps.key()).cloned());
    v[Pref::ExcludedApps.key()] = Value::from(apps);
    v[APP_NAMES] = Value::Object(EXCLUDED_APPS_SEED.iter().map(|(id, name)| (id.to_string(), Value::from(*name))).collect());
    // a fresh home has no key: not onboarded yet (the window's first
    // launch shows on `false`; amb-tools m_9345)
    if v.get(Pref::Onboarded.key()).is_none_or(Value::is_null) {
        v[Pref::Onboarded.key()] = Value::Bool(false);
    }
    v
}

impl Pref {
    /// Its key in `prefs.json` (and `tui.json` for voice and theme).
    pub fn key(self) -> &'static str {
        match self {
            Pref::Voice => "voice_mode_enabled",
            Pref::Theme => "theme",
            Pref::Hints => "hints",
            Pref::Tip => "tip",
            Pref::Onboarded => "onboarded",
            Pref::Setup => "setup",
            Pref::ExcludedApps => "excluded_apps",
        }
    }
}

/// A value kept in `file`: the whole file, or its object's `key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub file: PathBuf,
    pub key: Option<&'static str>,
}

impl Slot {
    /// The whole file is the value.
    pub fn file(file: impl Into<PathBuf>) -> Slot {
        Slot { file: file.into(), key: None }
    }

    /// One key of the file's JSON object.
    pub fn key(file: impl Into<PathBuf>, key: &'static str) -> Slot {
        Slot { file: file.into(), key: Some(key) }
    }

    /// The value, if set and readable. A whole-file slot whose text is
    /// not JSON (an old flag file) reads as that text.
    pub fn get(&self) -> Option<Value> {
        let text = std::fs::read_to_string(&self.file).ok()?;
        match self.key {
            Some(k) => serde_json::from_str::<Value>(&text).ok()?.get(k).cloned().filter(|v| !v.is_null()),
            None => Some(serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.trim().to_string()))),
        }
    }

    /// Set the value (a keyed slot keeps the object's other keys). The
    /// file is replaced whole (tmp + rename): a reader never sees half.
    pub fn set(&self, v: Value) -> io::Result<()> {
        let text = match self.key {
            Some(k) => {
                let mut obj = std::fs::read_to_string(&self.file)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .filter(|o| o.is_object())
                    .unwrap_or_else(|| Value::Object(Default::default()));
                obj[k] = v;
                serde_json::to_string_pretty(&obj)?
            }
            None => serde_json::to_string_pretty(&v)?,
        };
        write_atomic(&self.file, &(text + "\n"))
    }
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
