//! The on/off state of every plugin and part: `plugins.json` (bise_home's
//! `plugins_state()`, `$BEND_PLUGINS_STATE`), v2 (docs/plugins.md
//! "Enable state"):
//!
//! ```json
//! {"disabled": ["docs", "mistral:gmail"], "enabled": ["computer"],
//!  "parts_off": ["linear#tool:linear/delete_issue"], "parts_on": ["docs#skill:docs"]}
//! ```
//!
//! `disabled`/`enabled` hold plugins (any [`PluginId`] that is not a
//! part: today's bare names stay valid), `parts_off`/`parts_on` parts.
//! A missing or unreadable file: everything on but the opt-in plugins.
//!
//! `view_grants` (MCP Apps, docs/mcp-apps.md C8) holds what he granted
//! each view at the plugin's add or on its page, by its view part
//! (`<plugin>#view:<uri>`): `{"fake-app#view:ui%3A…": ["midi"]}`. A view
//! with no entry was never asked (it runs with no permission, its card
//! says so); an empty list: asked, nothing granted.
//!
//! - [`grants`], [`view_grant`]: the grants as read, and one view's, pure;
//! - [`grant`]: sets one view's grant, under the same lock.
//!
//! - [`read`]: the four lists as a [`StateV2`];
//! - [`StateV2::is_on`]: the rule, pure;
//! - [`set`]: the one writer of the file, for any [`PluginId`]. The TUI,
//!   the desktop's core and `bise plugins on|off` may toggle at once, so
//!   it reads, edits and writes under an exclusive flock on
//!   `plugins.json.lock`, through a tmp file and a rename; other keys and
//!   entries it can't parse are kept.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::id::{PartKind, PluginId, SkillsScope};
use crate::model::StateV2;

/// `plugins.json` as read: its four lists (a missing or unreadable file:
/// all empty, so everything is on but the opt-in plugins). A part id
/// that does not parse is skipped (and kept in the file by [`set`]).
pub fn read(path: &Path) -> StateV2 {
    let doc = doc(path);
    let ids = |key: &str| list(&doc, key).iter().filter_map(|s| PluginId::parse(s).ok()).filter(PluginId::is_part).collect();
    StateV2 { disabled: list(&doc, "disabled"), enabled: list(&doc, "enabled"), parts_off: ids("parts_off"), parts_on: ids("parts_on") }
}

/// Turn a plugin or a part on or off. `on` drops the id from its off list
/// (`disabled`, `parts_off`) and adds it to its on list (`enabled`,
/// `parts_on`: an opt-in plugin needs it), off the reverse. Returns
/// whether the file changed.
pub fn set(path: &Path, id: &PluginId, on: bool) -> std::io::Result<bool> {
    if StateV2::locked(id) {
        return if on { Ok(false) } else { Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{} is locked on", id.owner()))) };
    }
    let (off_key, on_key) = if id.is_part() { ("parts_off", "parts_on") } else { ("disabled", "enabled") };
    let name = id.to_string();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _lock = lock(&path.with_extension("json.lock"))?;
    let mut doc = doc(path);
    let (mut off_list, mut on_list) = (list(&doc, off_key), list(&doc, on_key));
    let (was_off, was_on) = (off_list.contains(&name), on_list.contains(&name));
    if on && !was_off && was_on || !on && was_off && !was_on {
        return Ok(false);
    }
    let (add, drop) = if on { (&mut on_list, &mut off_list) } else { (&mut off_list, &mut on_list) };
    drop.retain(|n| *n != name);
    if !add.contains(&name) {
        add.push(name);
        add.sort();
    }
    doc[off_key] = json!(off_list);
    doc[on_key] = json!(on_list);
    save(path, &doc)?;
    Ok(true)
}

/// Drop `plugin` (its owner when a part is given) and every part under
/// it from the four lists: a removed plugin leaves no state behind, and
/// one of the same name added later starts fresh. Returns whether the
/// file changed.
pub fn forget(path: &Path, plugin: &PluginId) -> std::io::Result<bool> {
    let owner = plugin.owner().clone();
    let name = owner.to_string();
    if !path.exists() {
        return Ok(false);
    }
    let _lock = lock(&path.with_extension("json.lock"))?;
    let mut doc = doc(path);
    let mut changed = false;
    for key in ["disabled", "enabled", "parts_off", "parts_on"] {
        let before = list(&doc, key);
        let gone = |s: &String| *s == name || PluginId::parse(s).is_ok_and(|id| id.is_part() && *id.owner() == owner);
        if before.iter().any(gone) {
            doc[key] = json!(before.into_iter().filter(|s| !gone(s)).collect::<Vec<_>>());
            changed = true;
        }
    }
    if let Some(g) = doc.get_mut("view_grants").and_then(Value::as_object_mut) {
        let n = g.len();
        g.retain(|k, _| !PluginId::parse(k).is_ok_and(|id| *id.owner() == owner));
        changed |= g.len() != n;
    }
    if changed {
        save(path, &doc)?;
    }
    Ok(changed)
}

/// The permissions a view may be granted (the spec's `_meta.ui.permissions`
/// keys, and bise's `midi`; `apps/desktop/src/shared/viewperms.ts`).
pub const VIEW_PERMS: [&str; 4] = ["camera", "microphone", "midi", "clipboardWrite"];

/// Each view's grant, by its view part id (the file's `view_grants`).
pub type Grants = std::collections::BTreeMap<PluginId, Vec<String>>;

/// `plugins.json`'s `view_grants` (missing, unreadable, an id that does
/// not parse or is not a view: left out; unknown permissions dropped).
pub fn grants(path: &Path) -> Grants {
    let doc = doc(path);
    let Some(map) = doc.get("view_grants").and_then(Value::as_object) else { return Grants::new() };
    map.iter()
        .filter_map(|(k, v)| {
            let id = PluginId::parse(k).ok().filter(is_view)?;
            let names: Vec<String> = v.as_array()?.iter().filter_map(Value::as_str).map(String::from).collect();
            Some((id, known(&names)))
        })
        .collect()
}

fn is_view(id: &PluginId) -> bool {
    matches!(id, PluginId::Part { kind: PartKind::View, .. })
}

/// The known permissions of `names`, each once, in [`VIEW_PERMS`]' order.
fn known(names: &[String]) -> Vec<String> {
    VIEW_PERMS.iter().filter(|p| names.iter().any(|n| n == *p)).map(|p| p.to_string()).collect()
}

/// What view `uri` of plugin `plugin` was granted: `None` when never
/// asked, else its permissions (maybe none). Pure: the hub's
/// `view/resource` gives `declared ∩ granted` from it.
pub fn view_grant(grants: &Grants, plugin: &str, uri: &str) -> Option<Vec<String>> {
    grants.get(&PluginId::plugin(plugin).part(PartKind::View, None, uri)).cloned()
}

/// Set view `id`'s grant to `perms` (unknown ones dropped). Returns
/// whether the file changed.
pub fn grant(path: &Path, id: &PluginId, perms: &[String]) -> std::io::Result<bool> {
    if !is_view(id) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{id} is not a view")));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _lock = lock(&path.with_extension("json.lock"))?;
    let mut doc = doc(path);
    let key = id.to_string();
    let want = json!(known(perms));
    if doc.get("view_grants").and_then(|g| g.get(&key)) == Some(&want) {
        return Ok(false);
    }
    if !doc.get("view_grants").is_some_and(Value::is_object) {
        doc["view_grants"] = json!({});
    }
    doc["view_grants"][key] = want;
    save(path, &doc)?;
    Ok(true)
}

/// `doc` written to `path` through a tmp file and a rename (under the
/// caller's lock).
fn save(path: &Path, doc: &Value) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("json.tmp-{}-{}", std::process::id(), TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&tmp, serde_json::to_string_pretty(doc).unwrap_or_default() + "\n")?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

impl StateV2 {
    /// Whether `id` can't be turned off: bise's own skills
    /// (`skills:bise`) and their parts. The one owner of "locked": the
    /// catalog and the skills scanner ask it, [`set`] refuses to turn it
    /// off, [`is_on`](StateV2::is_on) says on whatever the file says.
    pub fn locked(id: &PluginId) -> bool {
        matches!(id.owner(), PluginId::Skills { scope: SkillsScope::Bise })
    }

    /// Whether `id` is on. A locked id ([`StateV2::locked`]): always. A
    /// plugin: off when named in `disabled` (it
    /// wins), on when named in `enabled`, else on unless it is opt-in
    /// (`opt_in`: its manifest's `"dev.bise": {"default": "off"}`). A
    /// part: off when its owner is off (`opt_in` is the owner's), when
    /// named in `parts_off`, or, for a plugin's tool, when its server's
    /// part (`<owner>#mcp:<server>`) is; `parts_on` never turns on a part
    /// under an off owner or server.
    pub fn is_on(&self, id: &PluginId, opt_in: bool) -> bool {
        if StateV2::locked(id) {
            return true;
        }
        let PluginId::Part { owner, kind, server, .. } = id else {
            let name = id.to_string();
            return !self.disabled.contains(&name) && (self.enabled.contains(&name) || !opt_in);
        };
        let server_off = || match (kind, server) {
            (PartKind::Tool, Some(sv)) => self.parts_off.contains(&owner.part(PartKind::Mcp, None, sv.as_str())),
            _ => false,
        };
        self.is_on(owner, opt_in) && !self.parts_off.contains(id) && !server_off()
    }
}

/// `$BEND_PLUGINS_STATE`, else bise's `plugins.json` (`bise_home`).
pub fn state_path() -> PathBuf {
    bise_home::Home::from_env().plugins_state()
}

/// The file as a JSON object (missing, unreadable or not an object: `{}`).
fn doc(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// The strings of `doc[key]` (not a list: none).
fn list(doc: &Value, key: &str) -> Vec<String> {
    doc.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Each write's own tmp name in this process (with the pid: across them).
static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

#[cfg(test)]
mod tests {
    use super::*;



    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bp-state-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn id(s: &str) -> PluginId {
        PluginId::parse(s).unwrap()
    }

    fn state(disabled: &[&str], enabled: &[&str], parts_off: &[&str], parts_on: &[&str]) -> StateV2 {
        let s = |l: &[&str]| l.iter().map(|x| x.to_string()).collect();
        let p = |l: &[&str]| l.iter().map(|x| id(x)).collect();
        StateV2 { disabled: s(disabled), enabled: s(enabled), parts_off: p(parts_off), parts_on: p(parts_on) }
    }

    #[test]
    fn everything_is_on_by_default_but_an_opt_in_plugin() {
        let s = StateV2::default();
        for x in ["docs", "mistral:gmail", "skills:personal", "docs#skill:docs", "linear#mcp:linear", "linear#tool:linear/x", "mistral:gmail#tool:send"] {
            assert!(s.is_on(&id(x), false), "{x}");
        }
        assert!(!s.is_on(&id("computer"), true), "opt-in: off until named");
        assert!(!s.is_on(&id("computer#mcp:computer"), true), "its parts too");
        let s = state(&[], &["computer"], &[], &[]);
        assert!(s.is_on(&id("computer"), true) && s.is_on(&id("computer#mcp:computer"), true));
        let s = state(&["computer"], &["computer"], &[], &[]);
        assert!(!s.is_on(&id("computer"), true), "disabled wins");
        let s = state(&["mistral:gmail", "skills:vibe"], &[], &[], &[]);
        assert!(!s.is_on(&id("mistral:gmail"), false) && !s.is_on(&id("skills:vibe"), false));
        assert!(s.is_on(&PluginId::Skills { scope: SkillsScope::Personal }, false));
    }

    #[test]
    fn a_part_is_on_only_under_an_on_owner_and_server() {
        let s = state(&["docs"], &[], &[], &["docs#skill:docs"]);
        assert!(!s.is_on(&id("docs#skill:docs"), false), "owner off: parts_on does not turn it on");
        assert!(!s.is_on(&id("mistral:x#tool:t"), false) || s.is_on(&id("mistral:x"), false));
        let s = state(&["mistral:gmail"], &[], &[], &[]);
        assert!(!s.is_on(&id("mistral:gmail#tool:send"), false));
        let s = state(&[], &[], &["linear#mcp:linear", "docs#skill:docs"], &["linear#tool:linear/create"]);
        assert!(!s.is_on(&id("linear#mcp:linear"), false));
        assert!(!s.is_on(&id("linear#tool:linear/create"), false), "server off wins over parts_on");
        assert!(s.is_on(&id("linear#tool:other/create"), false), "another server's tool");
        assert!(!s.is_on(&id("docs#skill:docs"), false));
        assert!(s.is_on(&id("docs#skill:other"), false));
        assert!(s.is_on(&id("linear"), false), "a part off leaves its plugin on");
        let s = state(&[], &[], &["mistral:gmail#tool:send"], &["mistral:gmail#tool:send"]);
        assert!(!s.is_on(&id("mistral:gmail#tool:send"), false), "off wins");
    }

    /// Today's file (written by the released app) reads unchanged: the
    /// built-in `computer` opt-in on, `ambient-qa` off.
    #[test]
    fn todays_plugins_json_reads_unchanged() {
        let dir = tmp("v1");
        let p = dir.join("plugins.json");
        std::fs::create_dir_all(&dir).unwrap();
        let v1 = "{\n  \"disabled\": [\n    \"ambient-qa\"\n  ],\n  \"enabled\": [\n    \"computer\"\n  ]\n}\n";
        std::fs::write(&p, v1).unwrap();
        let s = read(&p);
        assert_eq!(s, state(&["ambient-qa"], &["computer"], &[], &[]));
        assert!(s.is_on(&id("computer"), true) && !s.is_on(&id("ambient-qa"), false) && s.is_on(&id("docs"), false));
        // an already-set toggle writes nothing; a part adds its own keys only
        assert!(!set(&p, &id("computer"), true).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), v1);
        assert!(set(&p, &id("docs#skill:docs"), false).unwrap());
        let s = read(&p);
        assert_eq!(s, state(&["ambient-qa"], &["computer"], &["docs#skill:docs"], &[]));
        assert_eq!(read(&dir.join("missing.json")), StateV2::default());
        std::fs::write(&p, "not json").unwrap();
        assert_eq!(read(&p), StateV2::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_views_grant_is_written_read_and_forgotten() {
        let dir = tmp("grants");
        let p = dir.join("plugins.json");
        let counter = PluginId::plugin("fake-app").part(PartKind::View, None, "ui://fake-app/counter");
        assert_eq!(view_grant(&grants(&p), "fake-app", "ui://fake-app/counter"), None, "never asked");
        assert!(set(&p, &id("docs#skill:docs"), false).unwrap());
        let perms = |l: &[&str]| l.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(grant(&p, &counter, &perms(&["midi", "geolocation", "camera", "midi"])).unwrap());
        assert!(!grant(&p, &counter, &perms(&["camera", "midi"])).unwrap(), "the same grant writes nothing");
        let g = grants(&p);
        assert_eq!(view_grant(&g, "fake-app", "ui://fake-app/counter"), Some(perms(&["camera", "midi"])));
        assert_eq!(view_grant(&g, "fake-app", "ui://fake-app/other"), None);
        assert_eq!(view_grant(&g, "other", "ui://fake-app/counter"), None, "another plugin's view");
        // asked, nothing granted: an empty list, not 'never asked'
        assert!(grant(&p, &counter, &[]).unwrap());
        assert_eq!(view_grant(&grants(&p), "fake-app", "ui://fake-app/counter"), Some(vec![]));
        assert_eq!(read(&p).parts_off, vec![id("docs#skill:docs")], "the other lists kept");
        assert!(grant(&p, &id("docs#skill:docs"), &[]).is_err(), "a grant is a view's");
        // the plugin removed: its grants go with it
        assert!(forget(&p, &PluginId::plugin("fake-app")).unwrap());
        assert_eq!(grants(&p), Grants::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bise_skills_are_locked_on() {
        let dir = tmp("locked");
        let p = dir.join("plugins.json");
        for x in ["skills:bise", "skills:bise#skill:bise-pages"] {
            assert!(StateV2::locked(&id(x)), "{x}");
            assert!(set(&p, &id(x), false).is_err(), "{x}");
            assert!(!set(&p, &id(x), true).unwrap(), "{x}");
        }
        assert!(!p.exists(), "nothing written");
        assert!(!StateV2::locked(&id("skills:personal")) && !StateV2::locked(&id("docs")));
        let s = state(&["skills:bise"], &[], &["skills:bise#skill:bise-pages"], &[]);
        assert!(s.is_on(&id("skills:bise"), false) && s.is_on(&id("skills:bise#skill:bise-pages"), false), "whatever the file says");
    }

    #[test]
    fn toggles_and_keeps_other_keys() {
        let dir = tmp("keys");
        let p = dir.join("plugins.json");
        assert!(set(&p, &id("b"), false).unwrap());
        assert!(set(&p, &id("a"), false).unwrap());
        assert!(!set(&p, &id("a"), false).unwrap());
        assert_eq!(read(&p).disabled, vec!["a", "b"]);
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["other"] = json!(1);
        v["parts_off"] = json!(["not an id#", "x#skill:y"]);
        std::fs::write(&p, v.to_string()).unwrap();
        assert_eq!(read(&p).parts_off, vec![id("x#skill:y")], "a bad id is skipped");
        assert!(set(&p, &id("a"), true).unwrap());
        assert!(set(&p, &id("x#skill:y"), true).unwrap());
        let s = read(&p);
        assert_eq!((s.disabled, s.enabled, s.parts_on), (vec!["b".to_string()], vec!["a".to_string()], vec![id("x#skill:y")]));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["other"], json!(1));
        assert_eq!(v["parts_off"], json!(["not an id#"]), "kept, never lost");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn forget_drops_a_plugin_and_its_parts_only() {
        let dir = tmp("forget");
        let p = dir.join("plugins.json");
        assert!(!forget(&p, &id("pay")).unwrap(), "no file: nothing written");
        assert!(!p.exists());
        std::fs::create_dir_all(&dir).unwrap();
        let v = json!({"disabled": ["pay", "docs"], "enabled": ["pay"], "parts_off": ["pay#mcp:pay", "docs#skill:docs", "payx#skill:s"],
                       "parts_on": ["pay#tool:pay/send"], "other": 1});
        std::fs::write(&p, v.to_string()).unwrap();
        assert!(forget(&p, &id("pay#skill:x")).unwrap(), "a part names its owner");
        let s = read(&p);
        assert_eq!(s, state(&["docs"], &[], &["docs#skill:docs", "payx#skill:s"], &[]));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["other"], json!(1));
        assert!(!forget(&p, &id("pay")).unwrap(), "already gone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_opt_in_plugin_is_enabled_by_name_and_off_again() {
        let dir = tmp("optin");
        let p = dir.join("plugins.json");
        let cu = id("computer");
        assert!(!read(&p).is_on(&cu, true));
        assert!(set(&p, &cu, true).unwrap());
        assert!(!set(&p, &cu, true).unwrap());
        assert_eq!(read(&p), state(&[], &["computer"], &[], &[]));
        assert!(read(&p).is_on(&cu, true));
        assert!(set(&p, &cu, false).unwrap());
        assert_eq!(read(&p), state(&["computer"], &[], &[], &[]));
        assert!(!read(&p).is_on(&cu, true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The TUI and the desktop's core toggling at once, plugins and parts:
    /// no toggle is lost and no tmp file is left behind.
    #[test]
    fn racing_writers_lose_no_toggle() {
        let dir = tmp("race");
        let p = dir.join("plugins.json");
        let names: Vec<String> = (0..16).map(|i| format!("p{i:02}")).collect();
        let ids: Vec<PluginId> = names.iter().flat_map(|n| [id(n), id(&format!("{n}#skill:s"))]).collect();
        std::thread::scope(|s| {
            for chunk in ids.chunks(4) {
                let p = &p;
                s.spawn(move || {
                    for _ in 0..5 {
                        for n in chunk {
                            set(p, n, true).unwrap();
                            set(p, n, false).unwrap();
                        }
                    }
                });
            }
        });
        let s = read(&p);
        assert_eq!(s.disabled, names);
        assert_eq!(s.parts_off.len(), names.len());
        assert!(s.enabled.is_empty() && s.parts_on.is_empty());
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".tmp")).collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
