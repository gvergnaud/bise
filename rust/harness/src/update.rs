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
/// The desktop app's own release channel for its update check (bar S.6,
/// the window's core: `SetupPorts::manifest`): `(desktop.json or why
/// not, the channel's base URL, this Mac's target)`. Never `bise update`'s
/// channel: the two cycles are apart (desktop-on-main), so this reads
/// [`release::desktop_dist_url`] for the channel he follows (stable:
/// `BISE_DIST_URL_DESKTOP`, else the rolling `desktop-latest` tag; beta:
/// `BISE_DIST_URL_DESKTOP_BETA`, else `desktop-beta`).
pub(crate) fn desktop_manifest(channel: release::DesktopChannel) -> (Result<String, String>, String, String) {
    let base = release::desktop_dist_url(&env, channel);
    let text = fetch_text(&format!("{}/{}", base, release::DESKTOP_MANIFEST));
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
    let id = release::read_version(&app).get("id").cloned().unwrap_or_default();
    if REQUIRED.iter().all(|f| app.join(f).exists()) && id != rel.id {
        return Err(format!("the tarball holds version {}, the manifest says {}", id, rel.id));
    }
    place(&inst.versions_dir(), &rel.id, &app, false).map_err(|e| format!("incomplete release: {}", e))?;
    debug_assert!(dir.join("bise").exists());
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

/// Put the app root `from` at `<versions>/<id>`, the one way a version
/// dir is made from a release (a tarball's `app/`, moved) or from the
/// desktop app's bundle (copied, it stays): REQUIRED checked, then
/// through `.<id>.<pid>.tmp` and one rename, so a half copy never shows
/// and two at once leave one. Its dir; already there: kept as it is.
fn place(versions: &Path, id: &str, from: &Path, copy: bool) -> Result<PathBuf, String> {
    if id.is_empty() || id.starts_with('.') || id.contains('/') {
        return Err(format!("{}: no usable VERSION id ('{}')", from.display(), id));
    }
    if let Some(f) = REQUIRED.iter().find(|f| !from.join(f).exists()) {
        return Err(format!("{} is missing from {}", f, from.display()));
    }
    let dir = versions.join(id);
    if dir.join("bise").exists() {
        if same_build(&dir, from) {
            return Ok(dir);
        }
        // that id names another build (or a half one): never handed out,
        // never written over (a hub may run from it). One naming rule: a
        // VERSION id names one build (releases: the version; desktop_try:
        // root-<head8>[-<changes8>]-<recipe6>, root-<sha12> for a candidate),
        // so a clash is an error that names both builds (the bundles packed
        // before 0d546db5 said id=root and went to root-<commit12> until
        // issue 31: every desktop release has 0d546db5)
        return Err(format!("{} holds another build ({}) than {} ({}): one VERSION id, two builds", dir.display(), build_of(&dir), from.display(), build_of(from)));
    }
    std::fs::create_dir_all(versions).map_err(|e| format!("{}: {}", versions.display(), e))?;
    let tmp = versions.join(format!(".{}.{}.tmp", id, std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let made = if copy {
        // a clone on APFS (`cp -c`: no second 100 MB), else a plain copy
        let cp = |clone: bool| {
            let mut c = Command::new("/bin/cp");
            c.arg(if clone { "-Rc" } else { "-R" }).arg(from).arg(&tmp).stderr(Stdio::piped());
            c.output().map_err(|e| e.to_string()).and_then(|o| match o.status.success() {
                true => Ok(()),
                false => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
            })
        };
        cp(true).or_else(|_| {
            let _ = std::fs::remove_dir_all(&tmp);
            cp(false)
        })
    } else {
        std::fs::rename(from, &tmp).map_err(|e| e.to_string())
    };
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("{}: {}", tmp.display(), e));
    }
    match std::fs::rename(&tmp, &dir) {
        Ok(()) => Ok(dir),
        // another one placed it first: theirs stays
        Err(_) if dir.join("bise").exists() => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(dir)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(format!("{}: {}", dir.display(), e))
        }
    }
}

/// A root's build as an error names it: `id=<id> commit=<commit12>`.
fn build_of(dir: &Path) -> String {
    let v = release::read_version(dir);
    let commit = v.get("commit").map(String::as_str).unwrap_or("?");
    format!("id={} commit={}", v.get("id").map(String::as_str).unwrap_or("?"), commit.get(..12).unwrap_or(commit))
}

/// `dir` is the build `from` holds, whole: every REQUIRED file there and
/// the same VERSION id and commit.
fn same_build(dir: &Path, from: &Path) -> bool {
    let (a, b) = (release::read_version(dir), release::read_version(from));
    REQUIRED.iter().all(|f| dir.join(f).exists()) && a.get("id") == b.get("id") && a.get("commit") == b.get("commit")
}

/// The app root hubs run from (architect m_18354): the desktop app's
/// bise finds its runtime in `<x>.app/Contents/Resources/bin` (a version
/// dir the app carries), but a hub never runs from inside a `.app` (its
/// update renames the bundle to `.old`, then deletes it; probation and
/// rollback want immutable dirs): that runtime is installed once as
/// `<prefix>/versions/<its VERSION id>` ([`place`], copied), and that dir
/// is the root. Any other root is returned as it is.
pub(crate) fn hub_root(root: &Path) -> Result<PathBuf, String> {
    if !in_app(root) {
        return Ok(root.to_path_buf());
    }
    let id = release::read_version(root).get("id").cloned().unwrap_or_default();
    let home = bise_home::Home::from_env();
    let versions = release::default_prefix(&env, home.user_home()).join("versions");
    place(&versions, &id, root, true).map_err(|e| format!("bise couldn't install its runtime: {}. reinstall bise", e))
}

/// The bise a hub runs (architect m_21731, ambient-lead m_21726), the one
/// owner of where a hub runs from: every `sbd` this binary starts (the
/// desktop core, `bise ambient`, the terminal) gets its executable here,
/// with `root` from [`hub_root`]. A bise inside a `.app` runs its hubs
/// from the runtime [`hub_root`] installed (`versions/<id>/bise`), so
/// neither the hub nor its REPLs and their plugin bridges (they get the
/// hub's own executable) run from the bundle an update swaps; they outlive
/// the app. Anywhere else (a dev tree, a version dir): `current`, as
/// before. Never a silent fallback to the bundle: a root still in an app,
/// or one without its bise, is an error naming the path. Pure.
pub(crate) fn hub_exe(root: &Path, current: &Path, has: &dyn Fn(&Path) -> bool) -> Result<PathBuf, String> {
    if !in_app(current) {
        return Ok(current.to_path_buf());
    }
    if in_app(root) {
        return Err(format!("a hub never runs from inside an app ({}). reinstall bise", root.display()));
    }
    // the whole runtime, or no hub: a hub on a dir without its REPL or
    // its core can't start an agent (ambient-lead m_21839)
    if let Some(f) = [switchboard::switch::EXE, "repl-live", "sb-core"].iter().map(|f| root.join(f)).find(|p| !has(p)) {
        return Err(format!("bise couldn't install its runtime: no {}. reinstall bise", f.display()));
    }
    Ok(root.join(switchboard::switch::EXE))
}

/// [`hub_root`] then [`hub_exe`] for this process: (the hub's exe, its
/// app root).
pub(crate) fn hub_of(root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let root = hub_root(root)?;
    let me = std::env::current_exe().map_err(|e| format!("bise can't find itself: {e}"))?;
    Ok((hub_exe(&root, &me, &|p| p.exists())?, root))
}

/// A path inside an app bundle (`….app/Contents/…`).
pub(crate) fn in_app(p: &Path) -> bool {
    p.to_string_lossy().contains(".app/Contents/")
}

/// `current -> versions/<id>`, atomically (a symlink renamed over it).
fn flip(inst: &Install, id: &str) -> Result<(), String> {
    let tmp = inst.prefix.join(".current.tmp");
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(format!("versions/{}", id), &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, inst.current_link()).map_err(|e| e.to_string())
}

/// The versions to delete: past the newest `keep`, never `current`, nor
/// one a process runs (a hub, an agent's REPL: `ps` names its folder),
/// nor one `leased` (`release::in_use`: the desktop core runs from its
/// .app but its hubs start from that dir; a ps line never names it).
fn to_prune(installed: &[release::Installed], keep: usize, current: Option<&Path>, ps: &str, leased: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    installed
        .iter()
        .skip(keep)
        .filter(|i| Some(i.dir.as_path()) != current)
        .filter(|i| !ps.contains(&format!("{}/", i.dir.display())))
        .filter(|i| !leased(&i.dir))
        .map(|i| i.dir.clone())
        .collect()
}

/// What a prune removes: nothing under an explicit root (a try's throwaway
/// `BISE_HOME`): its install shares his `~/.local/share/bise/versions`,
/// write-once dirs it may add to, but its `current` is not his, so it never
/// judges what his hubs still need (architect m_62738). Else [`to_prune`].
/// `ps`: the process list, read only when a prune is decided (an explicit
/// home never forks `ps`, architect m_62746).
fn prune_dirs(explicit: bool, inst: &Install, ps: &dyn Fn() -> String, leased: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    if explicit {
        return Vec::new();
    }
    to_prune(&inst.installed(), KEEP, inst.current().as_deref(), &ps(), leased)
}

fn prune(inst: &Install, home: &bise_home::Home) {
    let ps = || {
        Command::new("ps")
            .args(["-axo", "command="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    if home.explicit() {
        eprintln!("{} update: no prune of {} under an explicit BISE_HOME: its versions may be another home's", now(), inst.versions_dir().display());
    }
    for d in prune_dirs(home.explicit(), inst, &ps, &release::in_use) {
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
    prune(&inst, &home);
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

    /// `to_prune` once a dropped lease is seen released, 10 s at most. A
    /// child any test forks (codesign, cp, git...) holds a copy of every
    /// open fd between its fork and its exec, the lease's too, so `in_use`
    /// can still see it held for a moment (cli-sign, release-40: 2 reds;
    /// a lock around this file's own forks was not enough).
    fn pruned_once_released(all: &[release::Installed], current: &Path, want: usize) -> Vec<PathBuf> {
        let t0 = std::time::Instant::now();
        loop {
            let gone = to_prune(all, 2, Some(current), "", &release::in_use);
            if gone.len() == want || t0.elapsed() > Duration::from_secs(10) {
                return gone;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

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

    /// The desktop app's runtime (architect m_18354): copied once out of
    /// the .app into `versions/<id>` (the bundle stays whole), a second
    /// launch copies nothing, a broken bundle says what it lacks.
    #[test]
    fn a_bundles_runtime_is_placed_once_as_a_version_dir() {
        let t = std::env::temp_dir().join(format!("bise-place-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let bin = t.join("bise.app/Contents/Resources/bin");
        std::fs::create_dir_all(bin.join("prompts")).unwrap();
        for f in REQUIRED {
            std::fs::write(bin.join(f), "x").unwrap();
        }
        std::fs::write(bin.join("VERSION"), "id=root-abc
").unwrap();
        std::fs::write(bin.join("prompts/p.txt"), "p").unwrap();
        assert!(in_app(&bin) && !in_app(&t.join("versions/root-abc")));
        let versions = t.join("prefix/versions");
        let dir = place(&versions, "root-abc", &bin, true).unwrap();
        assert_eq!(dir, versions.join("root-abc"));
        assert_eq!(std::fs::read_to_string(dir.join("prompts/p.txt")).unwrap(), "p");
        assert!(bin.join("repl-live").exists(), "copied: the bundle stays whole");
        // the second launch: kept as it is, nothing copied over it
        std::fs::write(dir.join("marker"), "first").unwrap();
        std::fs::write(bin.join("prompts/p.txt"), "changed").unwrap();
        assert_eq!(place(&versions, "root-abc", &bin, true).unwrap(), dir);
        assert_eq!(std::fs::read_to_string(dir.join("prompts/p.txt")).unwrap(), "p");
        assert!(dir.join("marker").exists());
        // no temp dir left
        let left: Vec<String> = std::fs::read_dir(&versions).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into()).collect();
        assert_eq!(left, ["root-abc"]);
        // a bundle without its runtime, or a bad id: why, nothing placed
        std::fs::remove_file(bin.join("sb-core")).unwrap();
        let e = place(&versions, "root-new", &bin, true).unwrap_err();
        assert!(e.starts_with("sb-core is missing from ") && e.ends_with("bise.app/Contents/Resources/bin"), "{e}");
        assert!(place(&versions, "../x", &bin, true).unwrap_err().contains("no usable VERSION id"));
        assert!(!versions.join("root-new").exists());
        let _ = std::fs::remove_dir_all(&t);
    }

    /// A release's (and the desktop runtime's) bise is a signed bise.app
    /// with `bise` a link into it (cli-sign): placing it, copied or moved,
    /// keeps the bundle byte for byte and the link a link, so its signature
    /// (what TCC keys his folder grant on) still verifies.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_placed_bundle_keeps_its_signature() {
        let t = std::env::temp_dir().join(format!("bise-place-sig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let versions = t.join("prefix/versions");
        for (id, copy) in [("v-copied", true), ("v-moved", false)] {
            let root = t.join(id);
            let macos = root.join("bise.app/Contents/MacOS");
            std::fs::create_dir_all(&macos).unwrap();
            std::fs::copy("/usr/bin/true", macos.join("bise")).unwrap();
            std::fs::write(root.join("bise.app/Contents/Info.plist"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>dev.bise.cli</string><key>CFBundleExecutable</key><string>bise</string></dict></plist>").unwrap();
            let signed = Command::new("codesign").args(["--force", "-s", "-"]).arg(root.join("bise.app")).status().unwrap();
            assert!(signed.success());
            std::os::unix::fs::symlink("bise.app/Contents/MacOS/bise", root.join("bise")).unwrap();
            for f in ["repl-live", "sb-core"] {
                std::fs::write(root.join(f), "x").unwrap();
            }
            std::fs::write(root.join("VERSION"), format!("id={id}\n")).unwrap();
            let dir = place(&versions, id, &root, copy).unwrap();
            assert_eq!(std::fs::read_link(dir.join("bise")).unwrap(), Path::new("bise.app/Contents/MacOS/bise"), "{id}: the link stays a link");
            let v = Command::new("codesign").args(["--verify", "--strict"]).arg(dir.join("bise.app")).output().unwrap();
            assert!(v.status.success(), "{id}: {}", String::from_utf8_lossy(&v.stderr));
        }
        let _ = std::fs::remove_dir_all(&t);
    }

    /// Where a hub runs from (architect m_21731, ambient-lead m_21726): a
    /// bise inside a .app hands its hubs the installed runtime's bise,
    /// never its own; a dev tree or a version dir keeps its own; nothing
    /// falls back to the bundle silently.
    #[test]
    fn a_hub_never_runs_from_inside_an_app() {
        let app = Path::new("/Applications/bise try.app/Contents/Resources/bin/bise");
        let root = Path::new("/u/.bise/versions/root-1c5659b8-abcdef");
        let whole = |p: &Path| ["bise", "repl-live", "sb-core"].iter().any(|f| p == root.join(f));
        assert_eq!(hub_exe(root, app, &whole).unwrap(), root.join("bise"));
        // its runtime not installed: why, with the path; never the bundle's bise
        let e = hub_exe(root, app, &|_| false).unwrap_err();
        assert!(e.contains("/u/.bise/versions/root-1c5659b8-abcdef/bise"), "{e}");
        // a half runtime (ambient-lead m_21839): no REPL or no core, no hub
        for gone in ["repl-live", "sb-core"] {
            let e = hub_exe(root, app, &|p: &Path| whole(p) && p != root.join(gone)).unwrap_err();
            assert!(e.contains(&format!("root-1c5659b8-abcdef/{gone}")), "{e}");
        }
        let inside = Path::new("/Applications/bise try.app/Contents/Resources/bin");
        let e = hub_exe(inside, app, &|_| true).unwrap_err();
        assert!(e.contains("bise try.app/Contents/Resources/bin"), "{e}");
        // a dev tree, a version dir: this bise, as before
        for me in ["/src/bise/rust/target/debug/bise", "/u/.bise/versions/x/bise"] {
            assert_eq!(hub_exe(Path::new("/src/bise"), Path::new(me), &|_| true).unwrap(), PathBuf::from(me));
            assert_eq!(hub_exe(inside, Path::new(me), &|_| false).unwrap(), PathBuf::from(me));
        }
    }

    /// A newer app never hands its hubs an older build's dir (ambient-lead
    /// m_21844: every promoted candidate said id=root, so his 8b00ed2b app
    /// ran its hubs on b4a25acb's versions/root). The same id with another
    /// commit, or a half dir: the bundle's runtime goes whole to
    /// <id>-<commit12>, and the dir there stays as it is (a hub may run it).
    #[test]
    fn an_app_never_hands_its_hubs_another_builds_dir() {
        let t = std::env::temp_dir().join(format!("bise-other-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let versions = t.join("prefix/versions");
        let app = |name: &str, commit: &str| {
            let bin = t.join(format!("{name}.app/Contents/Resources/bin"));
            std::fs::create_dir_all(&bin).unwrap();
            for f in REQUIRED {
                std::fs::write(bin.join(f), commit).unwrap();
            }
            std::fs::write(bin.join("VERSION"), format!("id=root{}commit={commit}{}", '\n', '\n')).unwrap();
            bin
        };
        let (a, b) = ("b4a25acbc155ff9315c299f62ad9089142565abf", "8b00ed2b0123456789abcdef0123456789abcdef");
        let dir_a = place(&versions, "root", &app("a", a), true).unwrap();
        assert_eq!(dir_a, versions.join("root"));
        // the same id, another commit (a bundle packed before 0d546db5; its
        // root-<commit12> second dir went in issue 31): an error naming both
        let e = place(&versions, "root", &app("b", b), true).unwrap_err();
        assert!(e.contains("holds another build (id=root commit=b4a25acbc155)") && e.contains("(id=root commit=8b00ed2b0123)"), "{e}");
        assert!(!versions.join("root-8b00ed2b0123").exists(), "no second name");
        assert_eq!(std::fs::read_to_string(dir_a.join("bise")).unwrap(), a, "A's dir untouched");
        assert_eq!(place(&versions, "root", &app("a", a), true).unwrap(), dir_a, "A again: its dir");
        // one naming rule (architect m_21930): any id that names two
        // builds, or a half dir under it, is an error naming both, never a
        // second name
        std::fs::create_dir_all(versions.join("v2")).unwrap();
        std::fs::write(versions.join("v2/bise"), "x").unwrap();
        let c = app("c", b);
        std::fs::write(c.join("VERSION"), format!("id=v2{}commit={b}{}", '\n', '\n')).unwrap();
        let e = place(&versions, "v2", &c, true).unwrap_err();
        assert!(e.contains("holds another build (id=? commit=?)") && e.contains("(id=v2 commit=8b00ed2b0123)"), "{e}");
        assert!(!versions.join("v2-8b00ed2b0123").exists(), "no second name");
        // a release (moved, its dir is current's target) never goes beside either
        let e = place(&versions, "root", &app("r", "ffffffffffffffffffffffff"), false).unwrap_err();
        assert!(e.contains("holds another build (id=root commit=b4a25acbc155)"), "{e}");
        let _ = std::fs::remove_dir_all(&t);
    }

    /// A legacy root's second dir (root-<commit12>, made for the bundles
    /// packed before 0d546db5 until issue 31; one may still sit in a
    /// versions dir) is a version like the others: installed() lists it,
    /// and prune keeps it only while it is among the newest, current or run
    /// from (architect m_21930: ~100 MB each).
    #[test]
    fn a_legacy_roots_second_dir_is_listed_and_pruned_like_the_others() {
        let t = std::env::temp_dir().join(format!("bise-prune-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let versions = t.join("prefix/versions");
        let mk = |dir: &str, id: &str, day: u32| {
            let d = versions.join(dir);
            std::fs::create_dir_all(&d).unwrap();
            for f in REQUIRED {
                std::fs::write(d.join(f), "x").unwrap();
            }
            let nl = char::from(10u8);
            std::fs::write(d.join("VERSION"), format!("id={id}{nl}built=2026-10-{day:02}T00:00:00Z{nl}")).unwrap();
        };
        mk("root", "root", 1);
        mk("root-8b00ed2b0123", "root", 2);
        mk("v3", "v3", 3);
        mk("v4", "v4", 4);
        std::os::unix::fs::symlink("versions/v4", t.join("prefix/current")).unwrap();
        let versions = versions.canonicalize().unwrap();
        let inst = Install::of_root(&versions.join("v4")).unwrap();
        let all = inst.installed();
        let dirs: Vec<PathBuf> = all.iter().map(|i| i.dir.clone()).collect();
        assert!(dirs.contains(&versions.join("root-8b00ed2b0123")) && dirs.contains(&versions.join("root")), "{dirs:?}");
        // an explicit BISE_HOME (a try's) prunes nothing in the shared versions; the default home
        // prunes as today (architect m_62738)
        let today = to_prune(&all, KEEP, Some(&versions.join("v4")), "", &release::in_use);
        assert!(!today.is_empty(), "the fixture has something to prune past KEEP");
        let never = || -> String { panic!("an explicit home read the process list") };
        assert_eq!(prune_dirs(true, &inst, &never, &release::in_use), Vec::<PathBuf>::new());
        assert_eq!(prune_dirs(false, &inst, &String::new, &release::in_use), today);
        // past KEEP=2: both roots go
        let gone = to_prune(&all, 2, Some(&versions.join("v4")), "", &release::in_use);
        assert_eq!(gone.len(), 2, "{gone:?}");
        assert!(gone.contains(&versions.join("root-8b00ed2b0123")) && gone.contains(&versions.join("root")), "{gone:?}");
        // a hub runs from it: kept
        let ps = format!("{}/bise sbd --workspace /w{}", versions.join("root-8b00ed2b0123").display(), char::from(10u8));
        let gone = to_prune(&all, 2, Some(&versions.join("v4")), &ps, &release::in_use);
        assert_eq!(gone, [versions.join("root")]);
        // current: kept
        let gone = to_prune(&all, 2, Some(&versions.join("root-8b00ed2b0123")), "", &release::in_use);
        assert_eq!(gone, [versions.join("root")]);
        // leased (a desktop core's runtime, no ps line names it): kept
        // while held, gone once released (his pruned app-form runtime)
        let lease = release::lease(&versions.join("root-8b00ed2b0123")).expect("a versions dir is leased");
        let gone = to_prune(&all, 2, Some(&versions.join("v4")), "", &release::in_use);
        assert_eq!(gone, [versions.join("root")]);
        drop(lease);
        let gone = pruned_once_released(&all, &versions.join("v4"), 2);
        assert_eq!(gone.len(), 2, "{gone:?}");
        let _ = std::fs::remove_dir_all(&t);
    }

    /// The same VERSION id, installed by the desktop app (its bundle's
    /// runtime, copied) and by the terminal's `bise update` (a release's
    /// app/, moved), is one versions/<id> dir, whichever came first.
    #[test]
    fn the_app_and_the_terminal_share_one_version_dir() {
        for app_first in [true, false] {
            let t = std::env::temp_dir().join(format!("bise-share-{}-{}", std::process::id(), app_first));
            let _ = std::fs::remove_dir_all(&t);
            let bin = t.join("bise.app/Contents/Resources/bin");
            let rel = t.join("release/app");
            for d in [&bin, &rel] {
                std::fs::create_dir_all(d).unwrap();
                for f in REQUIRED {
                    std::fs::write(d.join(f), "x").unwrap();
                }
            }
            let versions = t.join("prefix/versions");
            let (a, b) = if app_first {
                (place(&versions, "v1", &bin, true).unwrap(), place(&versions, "v1", &rel, false).unwrap())
            } else {
                (place(&versions, "v1", &rel, false).unwrap(), place(&versions, "v1", &bin, true).unwrap())
            };
            assert_eq!(a, versions.join("v1"));
            assert_eq!(b, a);
            let left: Vec<String> = std::fs::read_dir(&versions).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into()).collect();
            assert_eq!(left, ["v1"], "one dir, no temp left (app first: {app_first})");
            let _ = std::fs::remove_dir_all(&t);
        }
    }

    /// No path hands this process's own executable to an `sbd` spawn but
    /// through [`hub_exe`] (architect m_21731): every use of it in the
    /// harness and switchboard sources is either on a [`hub_of`] /
    /// [`hub_exe`] line or one of these, each never a hub start from a
    /// bise that may be inside an app. A new one fails until it's sorted.
    #[test]
    fn every_own_exe_use_is_known() {
        let pat = concat!("current", "_exe()");
        let allowed: [(&str, &str); 10] = [
            // the hub itself: its own exe (it runs from versions/<id>) for its switch and its peers' hubs
            ("harness/src/main.rs", "exe: std::env::"),
            // the hub's REPLs start their plugin bridge with the hub's own exe
            ("harness/src/main.rs", "BEND_HARNESS_BIN"),
            ("harness/src/update.rs", "let me = std::env::"),
            ("harness/src/update.rs", "let Ok(exe) = std::env::"),
            ("harness/src/approot.rs", "exe: std::env::"),
            ("harness/src/doctor.rs", ""),
            ("switchboard/src/switch.rs", "let mine = || std::env::"),
            ("switchboard/src/core.rs", "let next_to_exe = std::env::"),
            ("switchboard/src/computer_use.rs", "let exe = std::env::"),
            ("harness/src/update.rs", "let pat = concat!"),
        ];
        let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut stack = vec![rust.join("harness/src"), rust.join("switchboard/src")];
        let mut unknown = vec![];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let rel = p.strip_prefix(&rust).unwrap().to_string_lossy().into_owned();
                for (i, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
                    if !l.contains(pat) || l.contains("hub_of(") || l.contains("hub_exe(") {
                        continue;
                    }
                    if !allowed.iter().any(|(f, frag)| rel == *f && l.contains(frag)) {
                        unknown.push(format!("{}:{}: {}", rel, i + 1, l.trim()));
                    }
                }
            }
        }
        assert!(unknown.is_empty(), "an own exe that may start a hub from inside an app: go through update::hub_of\n{}", unknown.join("\n"));
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
        let gone = to_prune(&all, 3, Some(Path::new("/p/versions/a")), ps, &|_| false);
        assert_eq!(gone, Vec::<PathBuf>::new());
        let gone = to_prune(&all, 2, None, ps, &|_| false);
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
