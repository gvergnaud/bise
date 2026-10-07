//! `bise update` (BISE-171) and `bise uninstall` of an installed bise
//! (install.sh, BISE-170), and the daily background check.
//!
//! `bise update` reads `<channel>/latest.json` (bise_home::release), and
//! when it names another, newer build for this Mac: downloads the
//! tarball (curl: https or file://), checks its sha256, unpacks its
//! `app/` into `<prefix>/versions/<id>` (immutable) and flips `current`.
//! Running hubs keep their version: the hub says an update is ready;
//! `/restart` (or `/restart latest`) switches it (BISE-172), and so does
//! launching `bise` again in its folder (BISE-255). The last manifest read is
//! kept in `~/.bise/cache/latest.json` for the hub.
//!
//! The background check: at most once a day (`BISE_UPDATE_INTERVAL`
//! seconds), a detached `bise update --background` when an installed bise
//! opens Switchboard or a session; never blocks the start; off with
//! `BISE_NO_UPDATE=1`.
//!
//! `bise update --manifest` (update-card): the hub's check, at its start
//! and every hour: fetch `latest.json` into the cache, nothing else (no
//! stamp, no download, no output); the hub shows the new-release item.
//!
//! A private GitHub repo (BISE-217): a plain download of a release asset
//! gets a 404, so [`fetch_file`] asks `gh release download` (the GitHub
//! CLI, logged in), then the API with `GH_TOKEN`/`GITHUB_TOKEN`; neither:
//! the error says to run `gh auth login`. A public repo needs neither.

use bise_home::release::{self, Install, Release};
use std::path::{Path, PathBuf};
use bise_home::style::Style;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The files an app root must have (install.sh checks the same).
const REQUIRED: [&str; 4] = ["bise", "repl-live", "sb-core", "VERSION"];
/// Installed versions kept (with the current one and any a hub runs).
const KEEP: usize = 3;
const DAY: u64 = 24 * 3600;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// The tarball this Mac wants: `darwin-arm64` also for an x86_64 bise
/// under Rosetta (an M-series Mac runs the native build).
pub(crate) fn host_target() -> String {
    let t = crate::version::build_target();
    let rosetta = Command::new("/usr/sbin/sysctl")
        .args(["-n", "sysctl.proc_translated"])
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "1");
    rosetta_target(&t, rosetta)
}

fn rosetta_target(target: &str, rosetta: bool) -> String {
    match (target, rosetta) {
        ("darwin-x86_64", true) => "darwin-arm64".into(),
        (t, _) => t.into(),
    }
}

/// Where this bise runs from, in the words of the update: an install,
/// else why there is nothing to update.
/// bise's release channel for the desktop app's update check (bar S.6,
/// the window's core: `SetupPorts::manifest`): `(latest.json or why not,
/// the channel's base URL, this Mac's target)`. The channel is the same
/// as `bise update`'s: `BISE_DIST_URL`, else the install's, else
/// [`release::DIST_URL`] (a bise in an app bundle isn't an install).
pub(crate) fn desktop_manifest() -> (Result<String, String>, String, String) {
    let base = this_install()
        .ok()
        .and_then(|i| i.dist_url(&env))
        .or_else(|| env(release::DIST_URL_ENV))
        .unwrap_or_else(|| release::DIST_URL.to_string());
    let base = base.trim_end_matches('/').to_string();
    let text = fetch_text(&format!("{}/{}", base, release::MANIFEST));
    (text, base, host_target())
}

fn this_install() -> Result<Install, String> {
    let root = crate::approot::locate("repl-live").map(|(r, _)| r)?;
    let kind = release::root_kind(&root);
    match kind {
        release::RootKind::Install(i) => Ok(i),
        release::RootKind::Dev => Err(format!(
            "this bise is a dev build ({}), not an install: pull the repo, then /restart latest in its hub",
            root.display()
        )),
        k => Err(k.update_hint().unwrap_or_default().into()),
    }
}

/// curl `url` into `to`; `headers`: curl config lines given on stdin
/// (a token never shows in `ps`).
fn curl(url: &str, to: &Path, headers: &[String]) -> Result<(), String> {
    use std::io::Write;
    let mut c = Command::new("curl");
    c.args(["-fsSL", "--connect-timeout", "15", "--retry", "2", "-K", "-", "-o"])
        .arg(to)
        .arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut p = c.spawn().map_err(|e| format!("curl: {}", e))?;
    if let Some(mut i) = p.stdin.take() {
        let cfg: String = headers.iter().map(|h| format!("header = \"{}\"\n", h.replace('\\', "\\\\").replace('"', "\\\""))).collect();
        let _ = i.write_all(cfg.as_bytes());
    }
    let o = p.wait_with_output().map_err(|e| format!("curl: {}", e))?;
    if !o.status.success() {
        let _ = std::fs::remove_file(to);
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(())
}

/// `gh release download` of the asset (the CLI's own login).
fn gh_download(a: &release::GithubAsset, to: &Path) -> Result<(), String> {
    let mut c = Command::new("gh");
    c.args(["release", "download"]);
    if let Some(t) = &a.tag {
        c.arg(t);
    }
    c.args(["-R", &a.gh_repo(), "-p", &a.name, "--clobber", "-O"])
        .arg(to)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .stdin(Stdio::null());
    let o = c.output().map_err(|e| format!("gh: {}", e))?;
    if !o.status.success() {
        let _ = std::fs::remove_file(to);
        let e = String::from_utf8_lossy(&o.stderr);
        return Err(format!("gh: {}", e.lines().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim()));
    }
    Ok(())
}

/// The API with a token: the release's JSON, then the asset's file.
fn api_download(a: &release::GithubAsset, token: &str, to: &Path) -> Result<(), String> {
    let auth = format!("Authorization: Bearer {}", token);
    let json = to.with_extension("release.json");
    let r = curl(&a.release_api(&env), &json, &[auth.clone(), "Accept: application/vnd.github+json".into()])
        .and_then(|_| std::fs::read_to_string(&json).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&json);
    let asset = release::asset_api_url(&r.map_err(|e| format!("GitHub API: {}", e))?, &a.name)?;
    curl(&asset, to, &[auth, "Accept: application/octet-stream".into()]).map_err(|e| format!("GitHub API: {}", e))
}

/// Download `url` into `to`: plain curl (https, file://); a GitHub
/// release asset it cannot read (a private repo): `gh`, then a token.
fn fetch_file(url: &str, to: &Path) -> Result<(), String> {
    let plain = match curl(url, to, &[]) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    let Some(a) = release::github_asset(url) else {
        return Err(format!("cannot read {}: {}", url, plain));
    };
    let mut why = vec![plain];
    match gh_download(&a, to) {
        Ok(()) => return Ok(()),
        Err(e) => why.push(e),
    }
    if let Some(t) = env("GH_TOKEN").or_else(|| env("GITHUB_TOKEN")) {
        match api_download(&a, &t, to) {
            Ok(()) => return Ok(()),
            Err(e) => why.push(e),
        }
    }
    Err(format!(
        "cannot read {} ({}): if {} is private, install the GitHub CLI and run 'gh auth login' with an account that can read it (or set GH_TOKEN)",
        url,
        why.join("; "),
        a.repo
    ))
}

fn fetch_text(url: &str) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("bise-fetch-{}.json", std::process::id()));
    let r = fetch_file(url, &tmp).and_then(|_| std::fs::read_to_string(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    r
}

fn sha256(file: &Path) -> Result<String, String> {
    // shasum (macOS, most Linux), else sha256sum (a minimal Linux has no
    // perl, so no shasum)
    let o = Command::new("shasum")
        .args(["-a", "256"])
        .arg(file)
        .output()
        .or_else(|_| Command::new("sha256sum").arg(file).output())
        .map_err(|e| format!("sha256sum: {}", e))?;
    let out = String::from_utf8_lossy(&o.stdout);
    out.split_whitespace()
        .next()
        .filter(|_| o.status.success())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("shasum failed on {}", file.display()))
}

/// Download, check and unpack a release into `versions/<id>` (kept when
/// it is there already). The bundle's install.sh replaces the prefix's.
fn install_release(inst: &Install, rel: &Release, base: &str) -> Result<PathBuf, String> {
    let dir = inst.versions_dir().join(&rel.id);
    if dir.join("bise").exists() {
        return Ok(dir);
    }
    let work = inst.prefix.join(format!(".update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {}", work.display(), e))?;
    let r = unpack(inst, rel, base, &work, &dir);
    let _ = std::fs::remove_dir_all(&work);
    r.map(|_| dir)
}

fn unpack(inst: &Install, rel: &Release, base: &str, work: &Path, dir: &Path) -> Result<(), String> {
    let url = release::resolve_url(base, &rel.url);
    let tgz = work.join("dl.tar.gz");
    fetch_file(&url, &tgz)?;
    let got = sha256(&tgz)?;
    if got != rel.sha256 {
        return Err(format!("checksum mismatch for {} (want {}, got {})", url, rel.sha256, got));
    }
    let x = work.join("x");
    std::fs::create_dir_all(&x).map_err(|e| e.to_string())?;
    let ok = Command::new("tar").arg("-C").arg(&x).arg("-xzf").arg(&tgz).status().is_ok_and(|s| s.success());
    if !ok {
        return Err(format!("cannot unpack {}", url));
    }
    let bundle = std::fs::read_dir(&x)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.join("app").is_dir())
        .ok_or("the tarball has no app/ folder")?;
    let app = bundle.join("app");
    if let Some(f) = REQUIRED.iter().find(|f| !app.join(f).exists()) {
        return Err(format!("incomplete release: app/{} missing", f));
    }
    let id = release::read_version(&app).get("id").cloned().unwrap_or_default();
    if id != rel.id {
        return Err(format!("the tarball holds version {}, the manifest says {}", id, rel.id));
    }
    let tmp = inst.versions_dir().join(format!(".{}.tmp", rel.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::rename(&app, &tmp).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    // the newest installer: `bise uninstall` runs it
    let sh = bundle.join("install.sh");
    if sh.is_file() {
        let t = inst.prefix.join(".install.sh.tmp");
        if std::fs::copy(&sh, &t).is_ok() {
            let _ = std::fs::rename(&t, inst.installer());
        }
    }
    Ok(())
}

/// `current -> versions/<id>`, atomically (a symlink renamed over it).
fn flip(inst: &Install, id: &str) -> Result<(), String> {
    let tmp = inst.prefix.join(".current.tmp");
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(format!("versions/{}", id), &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, inst.current_link()).map_err(|e| e.to_string())
}

/// The versions to delete: past the newest `keep`, never `current` nor
/// one a process runs (a hub, an agent's REPL: `ps` names its folder).
fn to_prune(installed: &[release::Installed], keep: usize, current: Option<&Path>, ps: &str) -> Vec<PathBuf> {
    installed
        .iter()
        .skip(keep)
        .filter(|i| Some(i.dir.as_path()) != current)
        .filter(|i| !ps.contains(&format!("{}/", i.dir.display())))
        .map(|i| i.dir.clone())
        .collect()
}

fn prune(inst: &Install) {
    let ps = Command::new("ps")
        .args(["-axo", "command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let current = inst.current();
    for d in to_prune(&inst.installed(), KEEP, current.as_deref(), &ps) {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Install,
    Check,
    Background,
    /// the hub's check (update-card): the manifest only
    Manifest,
}

/// `bise update [--check]`; `--background`: the daily check (quiet).
pub(crate) fn main(args: &[String]) -> i32 {
    let mode = if args.iter().any(|a| a == "--manifest") {
        Mode::Manifest
    } else if args.iter().any(|a| a == "--background") {
        Mode::Background
    } else if args.iter().any(|a| a == "--check") {
        Mode::Check
    } else {
        Mode::Install
    };
    if let Some(a) = args.iter().find(|a| !matches!(a.as_str(), "--background" | "--check" | "--manifest")) {
        eprintln!("{}", Style::stderr().fail(&format!("unknown flag {}: {} update [--check]", a, crate::version::cmd_name())));
        return 2;
    }
    let out = Style::stdout();
    // the daily check writes a log: plain, stamped
    let say = |s: String| {
        if mode == Mode::Background {
            eprintln!("{} {}", now(), s);
        } else if s.ends_with('…') {
            println!("{}", out.dim(&s));
        } else if s.contains("is available") {
            println!("{}", out.ask(&s));
        } else {
            println!("{}", out.ok(&s));
        }
    };
    match run(mode, &say) {
        Ok(()) => 0,
        Err(e) => {
            if mode == Mode::Background {
                eprintln!("{} error: {}", now(), e);
            } else {
                eprintln!("{}", Style::stderr().fail(&format!("{} update: {}", crate::version::cmd_name(), e)));
            }
            1
        }
    }
}

fn now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn run(mode: Mode, say: &dyn Fn(String)) -> Result<(), String> {
    let inst = this_install()?;
    let base = inst.dist_url(&env).ok_or(
        "no release channel: nothing is published yet; install from one with BISE_DIST_URL=<url> (or set it for this command)",
    )?;
    let home = bise_home::Home::from_env();
    let _ = std::fs::create_dir_all(home.cache_dir());
    if mode == Mode::Manifest {
        // the hub's check: the manifest only, for its item (a bad one is
        // not kept: the last good one stays)
        let text = fetch_text(&format!("{}/{}", base, release::MANIFEST))?;
        release::parse_manifest(&text, &host_target())?;
        return std::fs::write(home.release_manifest(), &text).map_err(|e| format!("{}: {}", home.release_manifest().display(), e));
    }
    let _ = std::fs::write(home.update_stamp(), now());
    let text = fetch_text(&format!("{}/{}", base, release::MANIFEST))?;
    let target = host_target();
    let rel = release::parse_manifest(&text, &target)?;
    let _ = std::fs::write(home.release_manifest(), &text);
    let cur = inst.current().map(|c| release::read_version(&c)).unwrap_or_default();
    let cur_id = cur.get("id").cloned().unwrap_or_default();
    let cmd = crate::version::cmd_name();
    if !release::is_update(&rel, &cur_id, cur.get("built").map(String::as_str)) {
        if mode != Mode::Background {
            say(format!("{} {} is up to date (latest release: {} {})", cmd, cur_id, rel.version, rel.id));
        }
        return Ok(());
    }
    if mode == Mode::Check {
        say(format!("{} {} is available (installed: {}): {} update installs it", rel.version, rel.id, cur_id, cmd));
        return Ok(());
    }
    say(format!("downloading {} {} for {}…", rel.version, rel.id, target));
    install_release(&inst, &rel, &base)?;
    flip(&inst, &rel.id)?;
    prune(&inst);
    say(format!(
        "{} updated to {} ({}). a bise already running moves to it when you run {} again in its folder, or /restart in it (the agents keep running).",
        cmd, rel.id, rel.version, cmd
    ));
    Ok(())
}

/// Whether the daily check is due: the stamp is older than the interval.
fn due(stamp_age: Option<Duration>, interval: Duration) -> bool {
    stamp_age.is_none_or(|a| a >= interval)
}

/// At the start of an installed bise (Switchboard, a session): the daily
/// check, detached; never waits for it.
pub(crate) fn check_in_background() {
    if env(release::NO_UPDATE_ENV).is_some_and(|v| v != "0") {
        return;
    }
    let Ok(inst) = this_install() else { return };
    if inst.dist_url(&env).is_none() {
        return;
    }
    let home = bise_home::Home::from_env();
    let stamp = home.update_stamp();
    let age = std::fs::metadata(&stamp).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
    let interval = env("BISE_UPDATE_INTERVAL").and_then(|s| s.parse().ok()).unwrap_or(DAY);
    if !due(age, Duration::from_secs(interval)) {
        return;
    }
    // stamped now: two starts in a row spawn one check
    let _ = std::fs::create_dir_all(home.cache_dir());
    let _ = std::fs::write(&stamp, now());
    let Ok(exe) = std::env::current_exe() else { return };
    let log = std::fs::OpenOptions::new().create(true).append(true).open(home.update_log());
    use std::os::unix::process::CommandExt;
    let mut c = Command::new(exe);
    c.args(["update", "--background"]).stdin(Stdio::null()).stdout(Stdio::null()).process_group(0);
    if let Ok(f) = log {
        c.stderr(Stdio::from(f));
    } else {
        c.stderr(Stdio::null());
    }
    let _ = c.spawn();
}

/// `bise uninstall [--purge]`: the prefix's installer does it.
pub(crate) fn uninstall(args: &[String]) -> i32 {
    let inst = match this_install() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{}", Style::stderr().fail(&format!("{} uninstall: {}", crate::version::cmd_name(), e)));
            return 1;
        }
    };
    use std::os::unix::process::CommandExt;
    let e = Command::new("/bin/sh")
        .arg(inst.installer())
        .args(["--uninstall", "--prefix"])
        .arg(&inst.prefix)
        .args(args)
        .exec();
    eprintln!("{}", Style::stderr().fail(&format!("{} uninstall: {}: {}", crate::version::cmd_name(), inst.installer().display(), e)));
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rosetta_gets_the_native_build() {
        assert_eq!(rosetta_target("darwin-x86_64", true), "darwin-arm64");
        assert_eq!(rosetta_target("darwin-x86_64", false), "darwin-x86_64");
        assert_eq!(rosetta_target("darwin-arm64", false), "darwin-arm64");
    }

    #[test]
    fn the_check_runs_once_a_day() {
        let day = Duration::from_secs(DAY);
        assert!(due(None, day));
        assert!(!due(Some(Duration::from_secs(3600)), day));
        assert!(due(Some(day), day));
        assert!(due(Some(Duration::ZERO), Duration::ZERO));
    }

    #[test]
    fn prune_keeps_the_newest_the_current_and_the_running() {
        let v = |id: &str| release::Installed {
            id: id.into(),
            dir: PathBuf::from(format!("/p/versions/{}", id)),
            subject: String::new(),
            built: String::new(),
        };
        let all = [v("e"), v("d"), v("c"), v("b"), v("a")];
        let ps = "/p/versions/b/bise sbd --workspace /w\n";
        let gone = to_prune(&all, 3, Some(Path::new("/p/versions/a")), ps);
        assert_eq!(gone, Vec::<PathBuf>::new());
        let gone = to_prune(&all, 2, None, ps);
        assert_eq!(gone, [PathBuf::from("/p/versions/c"), PathBuf::from("/p/versions/a")]);
    }

    /// A whole update from a file:// channel: manifest, download, sha256,
    /// unpack, flip; then "up to date"; a bad checksum changes nothing.
    #[test]
    fn an_update_from_a_file_channel_installs_and_flips() {
        let t = std::env::temp_dir().join(format!("bise-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let prefix = t.join("prefix");
        let mk_app = |dir: &Path, id: &str| {
            std::fs::create_dir_all(dir).unwrap();
            for f in REQUIRED {
                std::fs::write(dir.join(f), "x").unwrap();
            }
            std::fs::write(dir.join("VERSION"), format!("id={}\nbuilt=2026-10-0{}T00:00:00Z\n", id, if id == "new1" { 2 } else { 1 })).unwrap();
        };
        mk_app(&prefix.join("versions/old1"), "old1");
        std::os::unix::fs::symlink("versions/old1", prefix.join("current")).unwrap();
        let inst = Install::of_root(&prefix.join("versions/old1")).unwrap();
        // the channel: a tarball of bise-new1/{install.sh,app/}
        let rel_dir = t.join("rel");
        mk_app(&t.join("stage/bise-new1/app"), "new1");
        std::fs::write(t.join("stage/bise-new1/install.sh"), "#new installer\n").unwrap();
        std::fs::create_dir_all(&rel_dir).unwrap();
        let tgz = rel_dir.join("bise-new1.tar.gz");
        assert!(Command::new("tar").arg("-C").arg(t.join("stage")).arg("-czf").arg(&tgz).arg("bise-new1").status().unwrap().success());
        let base = format!("file://{}", rel_dir.display());
        let mut rel = Release {
            version: "0.2".into(),
            id: "new1".into(),
            url: "bise-new1.tar.gz".into(),
            sha256: "0".repeat(64),
            built: Some("2026-10-02T00:00:00Z".into()),
        };
        let e = install_release(&inst, &rel, &base).unwrap_err();
        assert!(e.contains("checksum mismatch"), "{e}");
        assert!(!prefix.join("versions/new1").exists());
        rel.sha256 = sha256(&tgz).unwrap();
        let dir = install_release(&inst, &rel, &base).unwrap();
        flip(&inst, &rel.id).unwrap();
        assert_eq!(inst.current(), Some(dir.canonicalize().unwrap()));
        assert_eq!(std::fs::read_to_string(inst.installer()).unwrap(), "#new installer\n");
        assert!(!release::is_update(&rel, "new1", None));
        // no leftovers
        let names: Vec<String> = std::fs::read_dir(&prefix).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into()).collect();
        assert!(names.iter().all(|n| !n.starts_with(".update-") && n != ".current.tmp"), "{names:?}");
        // a manifest that lies about the id is refused
        let lie = Release { id: "other".into(), ..rel.clone() };
        assert!(install_release(&inst, &lie, &base).unwrap_err().contains("holds version new1"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
