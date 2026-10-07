//! The user's opt-in mirror of the public pages (pages-ui, main m_7501).
//! By default nothing leaves the machine: the pages are served by the hub's
//! `/` only. When a page published with `--public` gets a new version (or
//! is taken back with `--private`), the hub writes the static export of
//! every public page (export.rs) to `<state>/public/`, then runs the
//! user's `[pages] mirror` from config.toml, if he set one:
//!
//! - a folder (`mirror = "~/sites/pages"`): the export is copied into it;
//! - a command (`mirror = "~/.bise/pages-mirror/deploy.sh"`): run with
//!   `sh -c`, in the export, with `BISE_PAGES_EXPORT=<the export>`.
//!
//! Off the hub's loop (a deploy takes a minute); one at a time; a failure
//! is one line in main's thread (`PageMsg::Mirror`), the last good copy
//! stays where it was.

use super::export::{files, Public};
use super::{PageMsg, Pages};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// One mirror at a time: a second publish waits for the first deploy.
static RUNNING: Mutex<()> = Mutex::new(());

/// `[pages] mirror` of config.toml; None: not set (local only).
pub fn setting(config: &Path) -> Option<String> {
    let text = std::fs::read_to_string(config).ok()?;
    let v: toml::Table = toml::from_str(&text).ok()?;
    let m = v.get("pages")?.get("mirror")?.as_str()?.trim().to_string();
    (!m.is_empty()).then_some(m)
}

/// `~/x` → `$HOME/x`.
fn expand(s: &str) -> String {
    match (s.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => s.to_string(),
    }
}

/// A setting that names a folder to copy into (one path, not a program).
pub fn is_folder(setting: &str) -> bool {
    let p = PathBuf::from(expand(setting));
    !setting.contains(char::is_whitespace) && (setting.ends_with('/') || p.is_dir())
}

/// Write the export's files into `dir`, replacing what was there (built
/// beside it, then swapped, so a reader never sees half of it).
pub fn write(dir: &Path, out: &[(String, Vec<u8>)]) -> Result<(), String> {
    let new = dir.with_extension("new");
    let _ = std::fs::remove_dir_all(&new);
    for (rel, bytes) in out {
        let p = new.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if out.is_empty() {
        std::fs::create_dir_all(&new).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir_all(dir);
    std::fs::rename(&new, dir).map_err(|e| format!("{}: {e}", dir.display()))
}

fn copy_into(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            // the export's folders are replaced whole: a page taken back goes
            let _ = std::fs::remove_dir_all(&dst);
            copy_into(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

/// Run the mirror on the written export.
pub fn run(setting: &str, export: &Path) -> Result<(), String> {
    if is_folder(setting) {
        return copy_into(export, Path::new(&expand(setting)));
    }
    let cmd = match setting.strip_prefix("~/") {
        Some(_) => expand(setting),
        None => setting.to_string(),
    };
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(export)
        .env("BISE_PAGES_EXPORT", export)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    Err(format!("{cmd} exited {}{}", out.status.code().unwrap_or(-1), if last.is_empty() { String::new() } else { format!(": {last}") }))
}

/// The export of every public page now, as files.
pub fn export_now(pages: &Pages) -> Vec<(String, Vec<u8>)> {
    let public: Vec<Public> = pages
        .store
        .list()
        .into_iter()
        .filter(|m| m.public)
        .filter_map(|m| pages.store.html(&m.id, m.version()).map(|html| Public { meta: m, html }))
        .collect();
    let kit = pages.kit_dir.clone();
    files(&public, &move |f| std::fs::read(kit.join(f)).ok())
}

/// Where the export is written: `<state>/public/`.
pub fn export_dir(pages: &Pages) -> PathBuf {
    pages.store.dir.parent().unwrap_or(&pages.store.dir).join("public")
}

/// After a publish that touched a public page (`id`): write the export and
/// run the user's mirror, off the caller's thread. `config`: config.toml.
pub fn spawn(pages: Arc<Pages>, id: String, config: PathBuf) {
    std::thread::spawn(move || {
        let _one = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        let dir = export_dir(&pages);
        let res = write(&dir, &export_now(&pages)).and_then(|_| match setting(&config) {
            Some(s) => run(&s, &dir),
            None => Ok(()),
        });
        if let Err(error) = res {
            pages.tell_hub(PageMsg::Mirror { id, error });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("sb-mirror-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_setting_is_read_from_config_toml() {
        let d = tmp();
        let c = d.as_path().join("config.toml");
        assert_eq!(setting(&c), None);
        std::fs::write(&c, "model = \"x\"\n[voice]\nsounds = true\n").unwrap();
        assert_eq!(setting(&c), None, "no [pages]: local only");
        std::fs::write(&c, "[pages]\nmirror = \"~/.bise/pages-mirror/deploy.sh\"\n").unwrap();
        assert_eq!(setting(&c).as_deref(), Some("~/.bise/pages-mirror/deploy.sh"));
        std::fs::write(&c, "[pages]\nmirror = \"  \"\n").unwrap();
        assert_eq!(setting(&c), None);
    }

    #[test]
    fn a_folder_gets_a_copy_and_a_command_runs_in_the_export() {
        let d = tmp();
        let export = d.as_path().join("public");
        write(&export, &[("artifacts/index.html".into(), b"i".to_vec()), ("artifacts/a/index.html".into(), b"a".to_vec())]).unwrap();
        // a folder: copied, a page taken back disappears there too
        let site = d.as_path().join("site");
        std::fs::create_dir_all(site.join("artifacts/old")).unwrap();
        std::fs::write(site.join("artifacts/old/index.html"), "gone").unwrap();
        run(&format!("{}/", site.display()), &export).unwrap();
        assert_eq!(std::fs::read_to_string(site.join("artifacts/a/index.html")).unwrap(), "a");
        assert!(!site.join("artifacts/old").exists());
        // a command: in the export, BISE_PAGES_EXPORT set
        let log = d.as_path().join("ran.txt");
        run(&format!("ls artifacts > {} && echo \"$BISE_PAGES_EXPORT\" >> {}", log.display(), log.display()), &export).unwrap();
        let ran = std::fs::read_to_string(&log).unwrap();
        assert!(ran.contains("index.html") && ran.contains(&export.display().to_string()), "{ran}");
        // a failure says the command, its code and its last words
        let e = run("echo nope >&2; exit 3", &export).unwrap_err();
        assert!(e.ends_with("exited 3: nope"), "{e}");
    }

    #[test]
    fn the_export_is_swapped_whole() {
        let d = tmp();
        let export = d.as_path().join("public");
        write(&export, &[("artifacts/a/index.html".into(), b"a".to_vec())]).unwrap();
        write(&export, &[("artifacts/b/index.html".into(), b"b".to_vec())]).unwrap();
        assert!(!export.join("artifacts/a").exists() && export.join("artifacts/b/index.html").exists());
        assert!(!d.as_path().join("public.new").exists());
    }
}
