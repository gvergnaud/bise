//! The user's preferences: one `prefs.json`. A [`Slot`] is a JSON value
//! in a file: the whole file (a test store) or one key of its object.

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

/// `shots_keep_days`: the days an image stays in bise's image store
/// (bend_images::prune). Unset: [`KEEP_DAYS_DEFAULT`], what settings ›
/// privacy shows.
pub const KEEP_DAYS: &str = "shots_keep_days";
pub const KEEP_DAYS_DEFAULT: u64 = 7;

/// `auto_update`: bise updates itself when a release is out, at a quiet
/// moment (update-flow; settings › update, `/update auto on|off`). Off
/// unless he turned it on.
pub const AUTO_UPDATE: &str = "auto_update";

/// Whether bise updates itself, from `prefs.json`'s object.
pub fn auto_update(prefs: Option<&Value>) -> bool {
    prefs.and_then(|p| p.get(AUTO_UPDATE)).and_then(Value::as_bool).unwrap_or(false)
}

/// The image store's retention in days, from `prefs.json`'s object: his
/// number when it is one of at least 1, else the default.
pub fn keep_days(prefs: Option<&Value>) -> u64 {
    prefs.and_then(|p| p.get(KEEP_DAYS)).and_then(Value::as_u64).filter(|d| *d >= 1).unwrap_or(KEEP_DAYS_DEFAULT)
}

/// The top keys of `prefs.json` the window may set (`prefs_set`,
/// architect m_9130): anything else is refused, never written.
pub const WINDOW_KEYS: &[&str] = &["onboarded", "theme", "voice", "context", "excluded_apps", "quiet", "shots_keep_days", "desktop_channel", "bims", "bim", "seen", AUTO_UPDATE, "pins", "pins_art", "pins_sched", "pins_proc"];

/// The pins' top keys, one per kind (the desktop's app/pins.ts ROOTS:
/// agents, artifacts, scheduled, processes).
const PIN_ROOTS: &[&str] = &["pins", "pins_art", "pins_sched", "pins_proc"];

/// `<pin root>.<hub id>`: the ids he pinned in that project, in pin order
/// (the desktop's app/pins.ts pinThing; his m_50763: every pin was
/// refused, 'couldn't pin', the roots were not in WINDOW_KEYS). Only that
/// path, the id with [`crate::hub_id`]'s shape, the value a list of
/// non-empty strings (an agent's name, an artifact id, a timer or watch id,
/// '<agent>/<process id>'), or null. Set whole, never one id.
fn pins_key(parts: &[&str], v: &Value) -> Result<(), String> {
    match parts {
        [root, id] if crate::hub_id::is_hub_id(id) => {
            let ids = |a: &Vec<Value>| a.len() <= 500 && a.iter().all(|x| x.as_str().is_some_and(|s| (1..=256).contains(&s.len())));
            if v.is_null() || v.as_array().is_some_and(ids) {
                Ok(())
            } else {
                Err(format!("{root}.{id} can't be {v}"))
            }
        }
        _ => Err(format!("{} is not a pins preference", parts.join("."))),
    }
}

/// `seen.<hub id>`: when he last looked at that project, in ms (the
/// desktop's app/seen.ts markSeen; bise_proto::rows::seen_key builds the
/// key, its SEEN_PREFIX is this top key and a dot). The sidebar's unread
/// dot and new counts are what came after it. Only that path, the id with
/// [`crate::hub_id`]'s shape, the value a whole number of ms (null: never).
fn seen_key(parts: &[&str], v: &Value) -> Result<(), String> {
    match parts {
        ["seen", id] if crate::hub_id::is_hub_id(id) => {
            if v.is_null() || v.as_u64().is_some() {
                Ok(())
            } else {
                Err(format!("seen.{id} can't be {v}"))
            }
        }
        _ => Err(format!("{} is not a seen preference", parts.join("."))),
    }
}

/// `bim`: what his own bim looks like (the desktop's onboarding and
/// settings › general, page bim-pick): `{skin, hair, color, wear, hat}`,
/// skin, color and hat (the accessory's colour) small indexes, hair and
/// wear short slugs; a look without `hat` (picked before the accessory
/// colour) is one too, its hat today's sky blue. The looks are the
/// desktop's drawing table (bims/look.ts: an unknown one draws today's),
/// never listed here; only the shape is checked. Set whole, never a part.
fn bim_look(v: &Value) -> bool {
    let slug = |s: Option<&Value>| s.and_then(Value::as_str).is_some_and(|s| (1..=24).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
    let small = |n: Option<&Value>| n.and_then(Value::as_u64).is_some_and(|n| n < 32);
    v.as_object().is_some_and(|o| {
        let hat = match o.get("hat") {
            None => o.len() == 4,
            h => o.len() == 5 && small(h),
        };
        hat && small(o.get("skin")) && small(o.get("color")) && slug(o.get("hair")) && slug(o.get("wear"))
    })
}

/// `bims.map.<hub id>`: the office's map he picked for that project (the
/// desktop's /bims, architect m_28589). Only that path, the id with
/// [`crate::hub_id`]'s shape, the value a short slug: the map names are the
/// desktop's drawing table (an unknown one draws the loft), never listed
/// here. Unset reads as the loft; nothing writes a default.
fn bims_key(parts: &[&str], v: &Value) -> Result<(), String> {
    let slug = |s: &str| (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    match parts {
        ["bims", "map", id] if crate::hub_id::is_hub_id(id) => {
            if v.is_null() || v.as_str().is_some_and(slug) {
                Ok(())
            } else {
                Err(format!("bims.map.{id} can't be {v}"))
            }
        }
        _ => Err(format!("{} is not a bims preference", parts.join("."))),
    }
}

/// The one-time hints the window may mark seen (`prefs_set {key:
/// "hints.<name>", value: true}`, S36): the TUI's own keys (tui hints.rs
/// `Hint::key`), in the same `hints` object, so a hint seen in either
/// never shows again. Only marking seen: never `hints` whole, never
/// another value (unseeing one is not the window's).
pub const WINDOW_HINTS: &[&str] = &["first_agent", "first_level3", "first_card", "first_steer", "first_yolo", "first_auto"];

/// The desktop's own one-time things, in the same `hints` object, marked
/// seen the same way (only true): the TUI never shows them.
///   - `fn_perms`: the capsule's permissions sheet was shown once, at his
///     first fn hold with a permission missing (desktop onboarding v3). It
///     means shown, never granted: whether one is missing comes from the
///     helper's perms each time, so one he revokes later shows in settings ›
///     privacy and in the capsule's state, never in this sheet again.
pub const DESKTOP_HINTS: &[&str] = &["fn_perms"];

/// The type a typed key keeps (a wrong one is refused).
fn typed(key: &str, v: &Value) -> Result<(), String> {
    let ok = match key {
        "onboarded" | AUTO_UPDATE => v.is_boolean(),
        "theme" => v.as_str().is_some_and(|t| ["auto", "light", "dark", "system"].contains(&t)),
        "shots_keep_days" => v.as_u64().is_some(),
        // the desktop app's update channel (release::DesktopChannel)
        "desktop_channel" => v.as_str().is_some_and(|c| ["stable", "beta"].contains(&c)),
        "excluded_apps" => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        "bim" => bim_look(v),
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
            [_, name] if (WINDOW_HINTS.contains(&name) || DESKTOP_HINTS.contains(&name)) && value == Value::Bool(true) => {}
            [_, name] if WINDOW_HINTS.contains(&name) || DESKTOP_HINTS.contains(&name) => return Err(format!("hints.{name} can only be true")),
            _ => return Err(format!("{key} is not a hint the window marks")),
        }
    } else if !WINDOW_KEYS.contains(&top) {
        return Err(format!("{top} is not a preference the window sets"));
    }
    if top == "bims" {
        bims_key(&parts, &value)?;
    }
    if top == "seen" {
        seen_key(&parts, &value)?;
    }
    if PIN_ROOTS.contains(&top) {
        pins_key(&parts, &value)?;
    }
    if parts.len() == 1 {
        if !value.is_null() {
            typed(top, &value)?;
        }
    } else if ["onboarded", "theme", "shots_keep_days", "excluded_apps", "desktop_channel", "bim"].contains(&top) {
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
