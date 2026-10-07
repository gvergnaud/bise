//! The enable state: `plugins.json` (bise_home's `plugins_state()`),
//! `{"disabled": ["name", ...], "enabled": [...]}`. A missing or
//! unreadable file means every plugin is enabled. One writer,
//! [`set_enabled`], under a flock (the TUI and the desktop's core both
//! toggle).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// `$BEND_PLUGINS_STATE`, else bise's `plugins.json` (`bise_home`).
pub fn state_path() -> PathBuf {
    bise_home::Home::from_env().plugins_state()
}

fn list(path: &Path, key: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

pub fn disabled(path: &Path) -> Vec<String> {
    list(path, "disabled")
}

/// The opt-in plugins turned on (`"enabled": [...]`): a plugin whose
/// manifest says `"dev.bise": {"default": "off"}` loads only when named
/// here.
pub fn enabled(path: &Path) -> Vec<String> {
    list(path, "enabled")
}

/// Enable (`on`) or disable a plugin by name: `on` drops it from
/// `disabled` and adds it to `enabled` (an opt-in plugin needs it), off
/// does the reverse. Returns whether the file changed. Other keys in the
/// file are kept. The one writer of the file: the TUI and the desktop's
/// core may toggle at the same moment, so read, edit and write happen
/// under an exclusive flock on `plugins.json.lock` (the projects
/// registry's shape), through a tmp file only this call uses.
pub fn set_enabled(path: &Path, name: &str, on: bool) -> std::io::Result<bool> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _lock = lock(&path.with_extension("json.lock"))?;
    let mut doc: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let (mut off_list, mut on_list) = (disabled(path), enabled(path));
    let (was_off, was_on) = (off_list.iter().any(|n| n == name), on_list.iter().any(|n| n == name));
    if on && !was_off && was_on || !on && was_off && !was_on {
        return Ok(false);
    }
    let (add, drop) = if on { (&mut on_list, &mut off_list) } else { (&mut off_list, &mut on_list) };
    drop.retain(|n| n != name);
    if !add.iter().any(|n| n == name) {
        add.push(name.to_string());
        add.sort();
    }
    doc["disabled"] = json!(off_list);
    doc["enabled"] = json!(on_list);
    let tmp = path.with_extension(format!("json.tmp-{}-{}", std::process::id(), TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&tmp, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n")?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(true)
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

    #[test]
    fn toggles_and_keeps_other_keys() {
        let dir = std::env::temp_dir().join(format!("bp-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("plugins.json");
        assert!(disabled(&p).is_empty());
        assert!(set_enabled(&p, "b", false).unwrap());
        assert!(set_enabled(&p, "a", false).unwrap());
        assert!(!set_enabled(&p, "a", false).unwrap());
        assert_eq!(disabled(&p), vec!["a", "b"]);
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["other"] = json!(1);
        std::fs::write(&p, v.to_string()).unwrap();
        assert!(set_enabled(&p, "a", true).unwrap());
        assert_eq!(disabled(&p), vec!["b"]);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["other"], json!(1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_opt_in_plugin_is_enabled_by_name_and_off_again() {
        let dir = std::env::temp_dir().join(format!("bp-state-optin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("plugins.json");
        assert!(enabled(&p).is_empty());
        assert!(set_enabled(&p, "computer", true).unwrap());
        assert!(!set_enabled(&p, "computer", true).unwrap());
        assert_eq!((enabled(&p), disabled(&p)), (vec!["computer".to_string()], vec![]));
        assert!(set_enabled(&p, "computer", false).unwrap());
        assert_eq!((enabled(&p), disabled(&p)), (vec![], vec!["computer".to_string()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The TUI and the desktop's core toggling at once: no toggle is lost
    /// and no tmp file is left behind.
    #[test]
    fn racing_writers_lose_no_toggle() {
        let dir = std::env::temp_dir().join(format!("bp-state-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("plugins.json");
        let names: Vec<String> = (0..16).map(|i| format!("p{i:02}")).collect();
        std::thread::scope(|s| {
            for chunk in names.chunks(2) {
                let p = &p;
                s.spawn(move || {
                    for _ in 0..5 {
                        for n in chunk {
                            set_enabled(p, n, true).unwrap();
                            set_enabled(p, n, false).unwrap();
                        }
                    }
                });
            }
        });
        assert_eq!(disabled(&p), names);
        assert!(enabled(&p).is_empty());
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".tmp")).collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
