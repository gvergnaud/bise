//! The Chromium family (design §4.1b): where each browser lives, where its
//! native host manifests go, and the shim they point at (C4).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::paths::Paths;

/// C4: the native host's name.
pub const HOST_NAME: &str = "dev.bise.computer_use";
/// The extension id, pinned with `key` in computer-use/extension/manifest.json.
pub const EXTENSION_ID: &str = "bogffepmbkbmbfejcadaipgphgkocgob";
/// The oldest Chrome major the extension supports (MV3, tab groups,
/// `chrome.debugger` focus emulation, a native port keeping the worker alive).
pub const MIN_MAJOR: u32 = 116;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Browser {
    /// the C4 `hello.browser` key
    pub key: &'static str,
    /// the name bise shows ("↖ driving Edge · github.com")
    pub name: &'static str,
    /// its folder under `~/Library/Application Support`
    pub support: &'static str,
    /// the app bundle under /Applications
    pub app: &'static str,
    pub bundle_id: &'static str,
}

impl Browser {
    /// `~/Library/Application Support/<support>/NativeMessagingHosts`
    pub fn hosts_dir(&self, paths: &Paths) -> PathBuf {
        paths.app_support().join(self.support).join("NativeMessagingHosts")
    }

    pub fn manifest_path(&self, paths: &Paths) -> PathBuf {
        self.hosts_dir(paths).join(format!("{}.json", HOST_NAME))
    }

    /// Installed: its profile folder exists (it ran once for this user).
    pub fn installed(&self, paths: &Paths) -> bool {
        paths.app_support().join(self.support).is_dir()
    }
}

/// In the order setup proposes them. Arc reads `Arc/User Data` (checked on
/// a real install); its tabs open without a group (no real tab groups).
pub const ALL: [Browser; 6] = [
    Browser { key: "chrome", name: "Chrome", support: "Google/Chrome", app: "Google Chrome.app", bundle_id: "com.google.Chrome" },
    Browser { key: "edge", name: "Edge", support: "Microsoft Edge", app: "Microsoft Edge.app", bundle_id: "com.microsoft.edgemac" },
    Browser { key: "brave", name: "Brave", support: "BraveSoftware/Brave-Browser", app: "Brave Browser.app", bundle_id: "com.brave.Browser" },
    Browser { key: "vivaldi", name: "Vivaldi", support: "Vivaldi", app: "Vivaldi.app", bundle_id: "com.vivaldi.Vivaldi" },
    Browser { key: "opera", name: "Opera", support: "com.operasoftware.Opera", app: "Opera.app", bundle_id: "com.operasoftware.Opera" },
    Browser { key: "arc", name: "Arc", support: "Arc/User Data", app: "Arc.app", bundle_id: "company.thebrowser.Browser" },
];

pub fn by_key(key: &str) -> Option<Browser> {
    ALL.iter().copied().find(|b| b.key.eq_ignore_ascii_case(key) || b.name.eq_ignore_ascii_case(key))
}

/// The browser whose app bundle holds `exe` (the native host's parent
/// process): it names Vivaldi and Arc, which say `chrome` (C4).
pub fn from_exe(exe: &str) -> Option<Browser> {
    ALL.iter().copied().find(|b| exe.contains(&format!("/{}/", b.app)))
}

/// The C4 manifest for one browser.
pub fn manifest(shim: &Path) -> Value {
    json!({
        "name": HOST_NAME,
        "description": "bise computer use",
        "path": shim,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{}/", EXTENSION_ID)],
    })
}

/// `ok`, `missing`, or `stale` (another path or extension id).
pub fn manifest_state(b: &Browser, paths: &Paths) -> &'static str {
    match std::fs::read_to_string(b.manifest_path(paths)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        None => "missing",
        Some(v) if v == manifest(&paths.shim()) && paths.shim().exists() => "ok",
        Some(_) => "stale",
    }
}

/// The shim: `~/.bise/bin/bise-chrome-host` runs the install's `current`
/// bise (it follows version switches), else the bise that wrote it.
pub fn shim_text(current: Option<&Path>, fallback: &Path) -> String {
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    let mut s = String::from("#!/bin/sh\n# bise computer use: the browsers' native host (C4). Written by `bise computer-use repair`.\n");
    if let Some(c) = current {
        s.push_str(&format!("exe={}/bise\n[ -x \"$exe\" ] || exe={}\n", q(c), q(fallback)));
    } else {
        s.push_str(&format!("exe={}\n", q(fallback)));
    }
    // a version folder gets pruned: the shim then pointed at nothing and
    // Chrome never connected (launch, 22ba932 gone). Fall back to the newest
    // bise of the same versions folder, then to `bise` on PATH.
    if let Some(versions) = fallback.parent().and_then(Path::parent).filter(|v| v.file_name().is_some_and(|n| n == "versions")) {
        s.push_str(&format!("[ -x \"$exe\" ] || exe=$(ls -t {}/*/bise 2>/dev/null | head -n 1)\n", q(versions)));
    }
    s.push_str("[ -x \"$exe\" ] || exe=$(command -v bise)\n");
    s.push_str("exec \"$exe\" computer-use chrome-host \"$@\"\n");
    s
}

/// The bise to run: `<prefix>/current` of an install, and this executable.
pub fn exe_and_current() -> (PathBuf, Option<PathBuf>) {
    let exe = std::env::current_exe().ok().map(|p| p.canonicalize().unwrap_or(p)).unwrap_or_else(|| PathBuf::from("bise"));
    let current = exe
        .parent()
        .and_then(bise_home::release::Install::of_root)
        .map(|i| i.current_link());
    (exe, current)
}

/// Write the shim and every installed browser's manifest. Returns what it wrote.
pub fn repair(paths: &Paths, exe: &Path, current: Option<&Path>) -> Result<Value, String> {
    let shim = paths.shim();
    if let Some(d) = shim.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d.display(), e))?;
    }
    std::fs::write(&shim, shim_text(current, exe)).map_err(|e| format!("{}: {}", shim.display(), e))?;
    crate::paths::private(&shim, 0o755).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for b in ALL.iter().filter(|b| b.installed(paths)) {
        let dir = b.hosts_dir(paths);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
        let f = b.manifest_path(paths);
        let text = serde_json::to_string_pretty(&manifest(&shim)).unwrap_or_default() + "\n";
        std::fs::write(&f, text).map_err(|e| format!("{}: {}", f.display(), e))?;
        written.push(json!({"browser": b.name, "manifest": f}));
    }
    Ok(json!({"shim": shim, "manifests": written}))
}

/// The reverse of [`repair`] (`bise computer-use uninstall`): every
/// browser's manifest of our host (ours by name) and the shim. Returns
/// what it removed.
pub fn unrepair(paths: &Paths) -> Value {
    let mut removed = Vec::new();
    for b in ALL.iter() {
        let f = b.manifest_path(paths);
        if std::fs::remove_file(&f).is_ok() {
            removed.push(json!({"browser": b.name, "manifest": f}));
        }
    }
    let shim = paths.shim();
    let shim_removed = std::fs::remove_file(&shim).is_ok();
    json!({"manifests": removed, "shim": shim_removed.then_some(shim)})
}

/// The extension's files (relative path, bytes), sorted, without its tests
/// and its build stamp.
fn extension_files(src: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().to_string();
            if rel == "test" || rel == "build.js" || rel == "build.json" || rel.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                walk(root, &p, out);
            } else if let Ok(b) = std::fs::read(&p) {
                out.push((rel, b));
            }
        }
    }
    let mut out = Vec::new();
    walk(src, src, &mut out);
    out.sort();
    out
}

/// A stable id of the extension's content (FNV-1a 64 over paths and bytes).
pub fn extension_build(src: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (rel, bytes) in extension_files(src) {
        for b in rel.bytes().chain([0u8]).chain(bytes.iter().copied()).chain([0u8]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{:016x}", h)
}

/// Copy this version's extension into `dst` (`~/.bise/computer-use/extension`,
/// the folder the user loads unpacked once) when its content changed, with
/// build.js and build.json stamped: the running extension sees the new
/// build.json and reloads itself (sw.js checkBuild). A Chrome that loaded
/// the version's own folder never got a new build (launch's re-test).
/// Returns whether it copied.
pub fn sync_extension(src: &Path, dst: &Path) -> std::io::Result<bool> {
    let build = extension_build(src);
    let stamp = dst.join("build.json");
    let current = std::fs::read_to_string(&stamp).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    if current.as_ref().and_then(|v| v["build"].as_str()) == Some(build.as_str()) && dst.join("manifest.json").is_file() {
        return Ok(false);
    }
    // the code first, the stamp last: a worker never sees a new build.json
    // next to old code
    let old: Vec<String> = extension_files(dst).into_iter().map(|(r, _)| r).collect();
    let new = extension_files(src);
    for (rel, bytes) in &new {
        let p = dst.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, bytes)?;
    }
    for rel in old.iter().filter(|r| !new.iter().any(|(n, _)| n == *r)) {
        let _ = std::fs::remove_file(dst.join(rel));
    }
    std::fs::write(dst.join("build.js"), format!("// written by bise computer-use (sync_extension)\nexport const BUILD = \"{}\";\n", build))?;
    std::fs::write(&stamp, json!({"build": build}).to_string() + "\n")?;
    Ok(true)
}

/// `CFBundleShortVersionString` of an app bundle.
pub fn app_version(app: &Path) -> Option<String> {
    let out = std::process::Command::new("plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Where the app is: /Applications, then ~/Applications.
pub fn app_path(b: &Browser, paths: &Paths) -> Option<PathBuf> {
    [PathBuf::from("/Applications"), paths.home.join("Applications")]
        .into_iter()
        .map(|d| d.join(b.app))
        .find(|p| p.is_dir())
}

/// Whether the browser runs (its main executable, by bundle path).
pub fn running(b: &Browser) -> bool {
    let out = std::process::Command::new("pgrep").args(["-f", &format!("/{}/Contents/MacOS/", b.app)]).output();
    out.map(|o| o.status.success()).unwrap_or(false)
}

pub fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// launch's re-test: Chrome kept running the first build of an
    /// unpacked extension. Each version syncs into one folder with a build
    /// stamp; same content, nothing written.
    #[test]
    fn the_extension_syncs_into_one_folder_with_a_build_stamp() {
        let d = std::env::temp_dir().join(format!("cu-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (src, dst) = (d.join("v1"), d.join("stable"));
        std::fs::create_dir_all(src.join("lib")).unwrap();
        std::fs::create_dir_all(src.join("test")).unwrap();
        std::fs::write(src.join("manifest.json"), "{}").unwrap();
        std::fs::write(src.join("lib/a.js"), "1").unwrap();
        std::fs::write(src.join("test/t.mjs"), "x").unwrap();
        std::fs::write(src.join("build.js"), "export const BUILD = \"dev\";").unwrap();
        assert!(sync_extension(&src, &dst).unwrap());
        let b1 = extension_build(&src);
        let stamp: Value = serde_json::from_str(&std::fs::read_to_string(dst.join("build.json")).unwrap()).unwrap();
        assert_eq!(stamp["build"], json!(b1));
        assert!(std::fs::read_to_string(dst.join("build.js")).unwrap().contains(&b1));
        assert!(!dst.join("test").exists(), "no tests in the loaded folder");
        assert!(!sync_extension(&src, &dst).unwrap(), "same content: nothing written");
        // a new version: new build, a file gone is removed
        std::fs::write(src.join("lib/a.js"), "2").unwrap();
        std::fs::write(dst.join("old.js"), "stale").unwrap();
        assert!(sync_extension(&src, &dst).unwrap());
        assert_ne!(extension_build(&src), b1);
        assert_eq!(std::fs::read_to_string(dst.join("lib/a.js")).unwrap(), "2");
        assert!(!dst.join("old.js").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn manifests_and_shim() {
        let d = std::env::temp_dir().join(format!("cu-br-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = Paths::new(d.join("run"), d.join("bise"), d.join("home"));
        for s in ["Google/Chrome", "Arc/User Data"] {
            std::fs::create_dir_all(p.app_support().join(s)).unwrap();
        }
        let chrome = by_key("chrome").unwrap();
        assert_eq!(manifest_state(&chrome, &p), "missing");
        let out = repair(&p, Path::new("/opt/bise/bise"), Some(Path::new("/opt/x/current"))).unwrap();
        assert_eq!(out["manifests"].as_array().unwrap().len(), 2);
        assert_eq!(manifest_state(&chrome, &p), "ok");
        assert_eq!(manifest_state(&by_key("edge").unwrap(), &p), "missing");
        let arc = by_key("Arc").unwrap();
        assert!(arc.manifest_path(&p).ends_with("Arc/User Data/NativeMessagingHosts/dev.bise.computer_use.json"));
        let m: Value = serde_json::from_str(&std::fs::read_to_string(chrome.manifest_path(&p)).unwrap()).unwrap();
        assert_eq!(m["allowed_origins"][0], "chrome-extension://bogffepmbkbmbfejcadaipgphgkocgob/");
        assert_eq!(m["path"], p.shim().to_string_lossy().as_ref());
        let shim = std::fs::read_to_string(p.shim()).unwrap();
        assert!(shim.contains("exe='/opt/x/current'/bise"), "{}", shim);
        assert!(shim.contains("computer-use chrome-host"));
        std::fs::write(chrome.manifest_path(&p), "{}").unwrap();
        assert_eq!(manifest_state(&chrome, &p), "stale");
        // launch: the shim named a dev version that was pruned since; it
        // falls back to the newest bise of the same versions folder
        let versions = d.join("versions");
        std::fs::create_dir_all(versions.join("new")).unwrap();
        let newer = versions.join("new").join("bise");
        std::fs::write(&newer, "#!/bin/sh\necho \"newer $*\"\n").unwrap();
        crate::paths::private(&newer, 0o755).unwrap();
        let gone = versions.join("pruned").join("bise");
        let text = shim_text(None, &gone);
        assert!(text.contains("ls -t") && text.contains("command -v bise"), "{}", text);
        let sh = d.join("shim.sh");
        std::fs::write(&sh, &text).unwrap();
        let out = std::process::Command::new("/bin/sh").arg(&sh).arg("x").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "newer computer-use chrome-host x");
        assert_eq!(from_exe("/Applications/Vivaldi.app/Contents/MacOS/Vivaldi").map(|b| b.name), Some("Vivaldi"));
        assert_eq!(from_exe("/usr/bin/true"), None);
        assert_eq!(major("154.0.7000.1"), Some(154));
        let _ = std::fs::remove_dir_all(&d);
    }
}
