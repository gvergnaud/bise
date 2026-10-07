//! `bise doctor` (BISE-167, docs/research/portable-bise.md §3.5): one line
//! per check, ✓ / ! / ✗, and how to fix what is wrong. Reads only: no
//! migration, no hub start, never a key (only where each one comes from).
//! Exit 1 when a check fails (✗); a warning (!) does not fail.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use bise_catalog::auth::{EnvFile, Keys, Store};
use bise_home::style::{hang, Style};
use switchboard::tools_env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Ok,
    Warn,
    Fail,
    /// `·`: a fact, nothing to do (another tool's login found)
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Check {
    pub(crate) mark: Mark,
    pub(crate) name: &'static str,
    pub(crate) detail: String,
    /// how to fix it (a warning or a failure)
    pub(crate) fix: Option<String>,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Check {
    Check { mark: Mark::Ok, name, detail: detail.into(), fix: None }
}

fn warn(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { mark: Mark::Warn, name, detail: detail.into(), fix: Some(fix.into()) }
}

fn fail(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { mark: Mark::Fail, name, detail: detail.into(), fix: Some(fix.into()) }
}

/// The report: one line per check, its fix on the next line under the
/// detail (BISE-285: the shared style; `?` a warning, as in the TUI).
/// Paths under `home` read `~/…`; on a terminal a long value wraps on
/// its own column.
pub(crate) fn render(checks: &[Check], st: &Style, home: Option<&Path>) -> String {
    let tilde = |t: &str| match home.map(|h| format!("{}/", h.display())).filter(|h| h.len() > 2) {
        Some(h) => tilde_paths(t, &h),
        None => t.to_string(),
    };
    let w = checks.iter().map(|c| c.name.len()).max().unwrap_or(9);
    let col = w + 4;
    let mut o = String::new();
    for c in checks {
        let name = format!("{:<w$}", c.name, w = w);
        let detail = hang(&tilde(&c.detail), col, st.width);
        let line = match c.mark {
            Mark::Ok => st.ok(&format!("{}  {}", st.dim(&name), detail)),
            Mark::Warn => st.ask(&format!("{}  {}", name, detail)),
            Mark::Fail => st.fail(&format!("{}  {}", name, detail)),
            Mark::Info => format!("{} {}  {}", st.dim("·"), st.dim(&name), st.dim(&detail)),
        };
        o.push_str(&line);
        o.push('\n');
        if let Some(f) = &c.fix {
            o.push_str(&format!("{}{} {}\n", " ".repeat(col), st.dim("fix:"), hang(&tilde(f), col + 5, st.width)));
        }
    }
    o
}

/// `t` with each path that starts with `home` (a path's start: the text's
/// start, or after a space, a paren or a quote) as `~/…`.
fn tilde_paths(t: &str, home: &str) -> String {
    let mut o = String::new();
    let mut rest = t;
    while let Some(i) = rest.find(home) {
        let starts = i == 0 || rest[..i].ends_with([' ', '(', '`', '\'', '"']) || (i == 0 && o.is_empty());
        o.push_str(&rest[..i]);
        o.push_str(if starts { "~/" } else { home });
        rest = &rest[i + home.len()..];
    }
    o.push_str(rest);
    o
}

/// The last line: what is left to do, or all good.
pub(crate) fn summary(checks: &[Check], st: &Style) -> String {
    let n = |m: Mark| checks.iter().filter(|c| c.mark == m).count();
    let plural = |n: usize, w: &str| format!("{} {}{}", n, w, if n == 1 { "" } else { "s" });
    match (n(Mark::Fail), n(Mark::Warn)) {
        (0, 0) => st.ok("all good."),
        (0, 1) => st.ask("1 thing to check. it says how."),
        (0, w) => st.ask(&format!("{} to check. each one says how.", plural(w, "thing"))),
        (1, 0) => st.fail("1 thing to fix. it says how."),
        (f, 0) => st.fail(&format!("{} to fix. each one says how.", plural(f, "thing"))),
        (f, w) => st.fail(&format!("{} to fix, {} to check. each one says how.", plural(f, "thing"), w)),
    }
}

// ---- the pure checks (tested) ----

/// "14.6.1" -> (14, 6)
fn major_minor(v: &str) -> Option<(u32, u32)> {
    let mut it = v.trim().split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

pub(crate) fn macos_check(version: Option<&str>, min: &str, arch: &str, rosetta: bool) -> Check {
    let arch = if rosetta { format!("{} under Rosetta", arch) } else { arch.to_string() };
    let Some(v) = version else {
        return warn("macOS", format!("version unknown ({})", arch), "bise supports macOS only");
    };
    let detail = format!("macOS {} {}", v, arch);
    if rosetta {
        return warn("macOS", detail, "run the arm64 build of bise (this one is x86_64, emulated)");
    }
    match (major_minor(v), major_minor(min)) {
        (Some(have), Some(need)) if have < need => {
            fail("macOS", format!("{} (bise needs {} or newer)", detail, min), format!("update macOS to {} or newer", min))
        }
        _ => ok("macOS", detail),
    }
}

/// The OS line on Linux: the distro (`/etc/os-release`), the arch, and
/// what bise does not have there (macOS-only), so nobody looks for it.
pub(crate) fn linux_check(pretty_name: Option<&str>, arch: &str, nixos: bool) -> Check {
    let name = pretty_name.unwrap_or("Linux");
    let how = if nixos { " (the Nix flake)" } else { "" };
    ok(
        "Linux",
        format!(
            "{} {}{} · macOS-only, off here: the sandbox (auto checks each command), voice, computer use, the desktop app",
            name, arch, how
        ),
    )
}

/// `PRETTY_NAME` of an os-release file.
pub(crate) fn os_release_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
}

/// The Unix socket path limit (sun_path, macOS: 104 bytes with the NUL).
pub(crate) const SOCKET_MAX: usize = 103;

/// `natural`: the socket's place in the hub dir (`<state>/hub.sock`); a
/// path over the limit is reached through its short link
/// (`bise_home::socket`).
pub(crate) fn socket_check(natural: &Path) -> Check {
    let n = natural.as_os_str().len();
    let socket = bise_home::socket::socket_path(natural);
    if n <= SOCKET_MAX {
        ok("socket", format!("{} bytes (max {}): {}", n, SOCKET_MAX, natural.display()))
    } else if socket.as_os_str().len() <= SOCKET_MAX {
        ok("socket", format!("{} bytes, over the {} max: reached as {}", n, SOCKET_MAX, socket.display()))
    } else {
        fail(
            "socket",
            format!("{} bytes, over the {} a Unix socket path allows: {}", n, SOCKET_MAX, socket.display()),
            "a shorter BISE_HOME (or SB_STATE_DIR)",
        )
    }
}

pub(crate) fn disk_check(where_: &Path, avail_kb: Option<u64>) -> Check {
    let Some(kb) = avail_kb else {
        return warn("disk", format!("free space unknown ({})", where_.display()), "check `df -h ~`");
    };
    let gb = kb as f64 / (1024.0 * 1024.0);
    let detail = format!("{:.1} GB free ({})", gb, where_.display());
    if gb < 1.0 {
        fail("disk", detail, "free some disk: hubs, worktrees and sessions need room")
    } else if gb < 5.0 {
        warn("disk", detail, "under 5 GB: free some disk soon")
    } else {
        ok("disk", detail)
    }
}

fn info(name: &'static str, detail: impl Into<String>) -> Check {
    Check { mark: Mark::Info, name, detail: detail.into(), fix: None }
}

/// The ChatGPT sign-in's line (no network, never a token): signed in,
/// who, the plan, until when the sign-in lasts unless a call renews it
/// (warned under 3 days); expired; None when it was never set up or is
/// signed out (nothing to check).
pub(crate) fn chatgpt_check(store: &Store, now: u64) -> Option<Check> {
    use bise_catalog::chatgpt::{self, State};
    let cli = bise_catalog::CLI;
    let again = format!("run {} login chatgpt", cli);
    match chatgpt::state_at(store, now) {
        State::NotSetUp | State::SignedOut { .. } => None,
        State::Expired { email } => {
            let who = if email.is_empty() { String::new() } else { format!(" ({})", email) };
            Some(warn("chatgpt", format!("the sign-in{} expired", who), again))
        }
        State::SignedIn { email, plan } => {
            let o = store.oauth(chatgpt::ID)?;
            let until = chatgpt::good_until(&o);
            let left = until.map(|t| t.saturating_sub(now) / 86_400);
            if let Some(d) = left.filter(|d| *d < 3) {
                let days = if d == 1 { "1 day".to_string() } else { format!("{} days", d) };
                let when = if d == 0 { "today".to_string() } else { format!("in {}", days) };
                return Some(warn("chatgpt", format!("the sign-in ends {}", when), again));
            }
            let mut d = format!("signed in as {}", if email.is_empty() { "your account" } else { &email });
            if let Some(p) = plan {
                d.push_str(&format!(" ({})", p));
            }
            d.push_str(" · renews by itself");
            if let Some(t) = until {
                d.push_str(&format!(" · good until {}", chatgpt::short_date(t, now)));
            }
            Some(ok("chatgpt", d))
        }
    }
}

/// The other tools' logins found (presence only): info lines.
pub(crate) fn detected_checks(d: &bise_catalog::detect::Detected) -> Vec<Check> {
    let mut v = Vec::new();
    if d.codex_chatgpt {
        v.push(info("codex", format!("signed in with ChatGPT. bise signs in on its own: {} login chatgpt", bise_catalog::CLI)));
    }
    if d.claude_plan {
        v.push(info("claude code", "signed in with a Claude plan. that plan doesn't run in bise (Anthropic's terms): use an Anthropic API key."));
    }
    v
}

/// The keys line: the providers with a key and where it comes from.
pub(crate) fn keys_check(found: &[(String, String)]) -> Check {
    if found.is_empty() {
        return fail("keys", "no provider key", format!("`{} login <provider>` (or set its env variable)", bise_catalog::CLI));
    }
    let list: Vec<String> = found.iter().map(|(p, from)| format!("{} ({})", p, from)).collect();
    ok("keys", list.join(", "))
}

/// The migration line from migrated.json (None: not moved yet).
/// `legacy`: `~/.bend-harness` or `~/.local/state/switchboard` exists.
pub(crate) fn migration_check(
    marker: Option<&serde_json::Value>,
    explicit_home: bool,
    no_migrate: bool,
    legacy: bool,
) -> Check {
    let Some(v) = marker else {
        return if explicit_home {
            ok("migration", "not needed (BISE_HOME is set: never filled from the old places)")
        } else if !legacy {
            ok("migration", "nothing to move (no ~/.bend-harness nor ~/.local/state/switchboard)")
        } else if no_migrate {
            warn("migration", "not done (BISE_NO_MIGRATE is set)", "unset BISE_NO_MIGRATE and start bise once")
        } else {
            warn("migration", "not done: state still in ~/.bend-harness and ~/.local/state/switchboard", "start `bise` once: it moves them to ~/.bise")
        };
    };
    let n = |k: &str| v.get(k).and_then(|x| x.as_array()).map_or(0, |a| a.len());
    let waiting = n("hubs_waiting");
    let errors = n("errors");
    let detail = format!(
        "done: {} copied, {} hubs moved, {} waiting (running in the old place), {} errors",
        n("copied"),
        n("hubs_moved"),
        waiting,
        errors
    );
    if errors > 0 {
        warn("migration", detail, "see `errors` in ~/.bise/migrated.json")
    } else if waiting > 0 {
        warn("migration", detail, "those hubs move at their next restart (`/restart` or `bise switchboard --stop`)")
    } else {
        ok("migration", detail)
    }
}

// ---- probes of this machine ----

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The OS line: macOS's version check, or Linux's.
fn os() -> Check {
    if !cfg!(target_os = "macos") {
        let arch = crate::version::build_target();
        let arch = arch.split_once('-').map_or(arch.as_str(), |(_, a)| a).to_string();
        let name = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| os_release_name(&t));
        return linux_check(name.as_deref(), &arch, Path::new("/etc/NIXOS").exists());
    }
    macos()
}

fn macos() -> Check {
    let version = run("/usr/bin/sw_vers", &["-productVersion"]);
    let rosetta = run("/usr/sbin/sysctl", &["-n", "sysctl.proc_translated"]).as_deref() == Some("1");
    let arch = crate::version::build_target();
    let arch = arch.split_once('-').map_or(arch.as_str(), |(_, a)| a).to_string();
    // the target of this version (VERSION macos=), else the build's (BISE-164)
    let min = root()
        .ok()
        .and_then(|(r, _)| std::fs::read_to_string(r.join("VERSION")).ok())
        .and_then(|v| v.lines().find_map(|l| l.strip_prefix("macos=").map(str::to_string)))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "14.0".into());
    macos_check(version.as_deref(), &min, &arch, rosetta)
}

fn root() -> Result<(PathBuf, crate::approot::Via), String> {
    crate::approot::locate("repl-live")
}

fn bise(verbose: bool) -> Check {
    let exe = std::env::current_exe().ok().map(|e| std::fs::canonicalize(&e).unwrap_or(e));
    let exe = exe.map(|e| e.display().to_string()).unwrap_or_else(|| "unknown".into());
    match root() {
        Ok((r, via)) => {
            let version = std::fs::read_to_string(r.join("VERSION")).ok();
            let line = crate::version::line(
                bise_catalog::CLI,
                version.as_deref(),
                &crate::version::build_target(),
                &r.display().to_string(),
            );
            ok("bise", format!("{} · {} · files: {} ({})", line, exe, r.display(), via.describe()))
        }
        Err(e) if verbose => fail("bise", format!("{} · no app root: {}", exe, e), "reinstall bise, or set BISE_APP_ROOT"),
        Err(_) => fail("bise", "no app root next to the executable (--verbose says more)", "reinstall bise, or set BISE_APP_ROOT"),
    }
}

fn signature() -> Check {
    let Ok(exe) = std::env::current_exe() else {
        return warn("signature", "executable unknown", "reinstall bise");
    };
    let out = Command::new("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(&exe)
        .stdin(Stdio::null())
        .output();
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stderr).to_string(),
        Err(e) => return warn("signature", format!("codesign: {}", e), "none needed to run; macOS only"),
    };
    if let Some(a) = text.lines().find_map(|l| l.strip_prefix("Authority=")) {
        ok("signature", a.to_string())
    } else if text.contains("Signature=adhoc") {
        ok("signature", "ad-hoc (enough for a curl or brew install: no quarantine flag)")
    } else {
        warn("signature", "not signed", "reinstall bise (an arm64 Mac runs no unsigned binary)")
    }
}

fn home_check(home: &bise_home::Home) -> Check {
    let root = home.root();
    let how = if std::env::var_os(bise_home::BISE_HOME).is_some_and(|v| !v.is_empty()) {
        "BISE_HOME"
    } else {
        "default"
    };
    match home.layout() {
        // a fresh HOME: nothing in the old places either (qa C)
        bise_home::Layout::Legacy if !legacy_found(home) => warn(
            "home",
            format!("{} does not exist yet", home.user_home().join(".bise").display()),
            "start `bise` once",
        ),
        bise_home::Layout::Legacy => warn(
            "home",
            format!("old layout: {} + ~/.local/state/switchboard", root.display()),
            "start `bise` once: it moves the state to ~/.bise",
        ),
        bise_home::Layout::Bise if !root.is_dir() => {
            warn("home", format!("{} ({}) does not exist yet", root.display(), how), "start `bise` once")
        }
        bise_home::Layout::Bise => {
            let has = |p: PathBuf| if p.exists() { "✓" } else { "-" };
            ok(
                "home",
                format!(
                    "{} ({}): config.toml {}, auth.json {}, hubs/ {}, sessions/ {}",
                    root.display(),
                    how,
                    has(home.config_file()),
                    has(home.auth_file()),
                    has(home.hubs_dir()),
                    has(home.sessions_dir())
                ),
            )
        }
    }
}

/// The old places of this HOME exist (what the migration would read).
fn legacy_found(home: &bise_home::Home) -> bool {
    let u = home.user_home();
    u.join(".bend-harness").exists() || u.join(".local/state/switchboard").exists()
}

fn migration(home: &bise_home::Home) -> Check {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let marker = std::fs::read_to_string(home.user_home().join(".bise").join(bise_home::MIGRATED))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    migration_check(
        marker.as_ref(),
        env(bise_home::BISE_HOME).is_some(),
        env(bise_home::migrate::NO_MIGRATE).is_some(),
        legacy_found(home),
    )
}

/// The PATH the agents' tools are looked up on (the hub's, plus the usual
/// dirs; the hub also adds the login shell's, BISE-166).
fn tools_path() -> String {
    tools_env::join_path(&[&std::env::var("PATH").unwrap_or_default(), &tools_env::host_std_dirs()])
}

fn git() -> Check {
    match tools_env::git() {
        tools_env::Git::Works { bin, version } => ok("git", format!("{} ({})", version, bin.display())),
        g => fail("git", "no working git (needed for -w worktrees and /version)", g.problem().unwrap_or_default()),
    }
}

fn rg() -> Check {
    match tools_env::which("rg", &tools_path()) {
        Some(p) => ok("rg", p.display().to_string()),
        None => warn("rg", "not found: the agents search with grep", "`brew install ripgrep` (faster searches)"),
    }
}

/// pr-design §8: the hub follows PRs through gh (its login, never a
/// token of bise's). `gh`: where it is; `logged_in`: `gh auth status`'s
/// answer for the repo's host; `origin`: this folder's remote URL.
pub(crate) fn github_check(gh: Option<&Path>, logged_in: Option<bool>, origin: Option<&str>) -> Check {
    let repo = origin.and_then(switchboard::forge::repo_of_url);
    let on_github = repo.as_ref().is_some_and(|r| r.host == "github.com") || (repo.is_some() && logged_in == Some(true));
    let this = match (&repo, on_github) {
        (Some(r), true) => format!("this repo's PRs: {}/{}/{}", r.host, r.owner, r.name),
        _ => "this folder is not a GitHub repo".to_string(),
    };
    match (gh, logged_in) {
        (None, _) if on_github => warn("github", format!("gh not found: {} are not followed", this.replace("this repo's PRs: ", "the PRs of ")), "`brew install gh`, then `gh auth login`"),
        (None, _) => ok("github", "gh not found (only needed to follow PRs on GitHub)"),
        (Some(p), Some(false)) if on_github => warn("github", format!("gh is not logged in ({}): PRs are not followed", p.display()), "`gh auth login` once"),
        (Some(p), Some(false)) => ok("github", format!("gh not logged in ({}); {}", p.display(), this)),
        (Some(p), _) => ok("github", format!("gh logged in ({}); {}", p.display(), this)),
    }
}

fn github() -> Check {
    let gh = tools_env::which("gh", &tools_path());
    let origin = run("git", &["remote", "get-url", "origin"]);
    let host = origin.as_deref().and_then(switchboard::forge::repo_of_url).map_or("github.com".to_string(), |r| r.host);
    // the exit code only: gh's output names the token's scopes
    let logged_in = gh.as_ref().map(|g| {
        Command::new(g)
            .args(["auth", "status", "--hostname", &host])
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    });
    github_check(gh.as_deref(), logged_in, origin.as_deref())
}

/// BISE-302: inside tmux, ctrl+1-9 (open inbox item N) pass only with
/// `extended-keys always`; elsewhere (or tmux already set): nothing.
/// `tmux`: inside tmux, its `extended-keys` value when it answered.
pub(crate) fn tmux_check(tmux: Option<Option<&str>>) -> Option<Check> {
    match tmux? {
        Some("always") => None,
        _ => Some(warn("tmux", "tmux eats ctrl+1-9", "add \"set -s extended-keys always\" to ~/.tmux.conf")),
    }
}

fn tmux() -> Option<Check> {
    std::env::var_os("TMUX").filter(|v| !v.is_empty())?;
    let out = std::process::Command::new("tmux").args(["show-options", "-sv", "extended-keys"]).stderr(std::process::Stdio::null()).output().ok();
    let v = out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    tmux_check(Some(v.as_deref()))
}

fn on_path() -> Check {
    let me = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok());
    let path = std::env::var("PATH").unwrap_or_default();
    let Some(p) = tools_env::which(bise_catalog::CLI, &path) else {
        return warn("PATH", "`bise` is not on PATH", "add ~/.local/bin to PATH (the installer does it; open a new terminal)");
    };
    let real = std::fs::canonicalize(&p).ok();
    // a launcher (install.sh, install.sh --dev) names the version it runs
    let launched = real.as_deref().filter(|r| is_launcher(r)).and_then(|r| {
        Command::new(r)
            .arg("--launcher-root")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
    });
    let mine = root().ok().map(|(r, _)| std::fs::canonicalize(&r).unwrap_or(r));
    path_check(&p, real.as_deref(), me.as_deref(), launched.as_deref(), mine.as_deref())
}

/// A shell script written by install.sh (a launcher), not a binary.
fn is_launcher(p: &Path) -> bool {
    let mut head = [0u8; 256];
    let n = std::fs::File::open(p).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    let head = String::from_utf8_lossy(&head[..n]);
    head.starts_with("#!") && head.contains("launcher")
}

/// The PATH line: `found` is `bise` on PATH (`real`: its target); `me` this
/// executable; `launched`: the app root the launcher runs, when `found` is
/// one; `mine`: this process's app root.
pub(crate) fn path_check(found: &Path, real: Option<&Path>, me: Option<&Path>, launched: Option<&Path>, mine: Option<&Path>) -> Check {
    if real.is_some() && real == me {
        return ok("PATH", format!("{} is this bise", found.display()));
    }
    match launched {
        Some(l) if Some(l) == mine => ok("PATH", format!("{} is a launcher that runs this bise ({})", found.display(), l.display())),
        Some(l) => warn(
            "PATH",
            format!("{} is a launcher that runs {}, not this bise", found.display(), l.display()),
            "fine for a version you run by hand; `bise doctor` checks the one the launcher runs",
        ),
        None => warn(
            "PATH",
            format!("{} is another bise ({})", found.display(), real.map(|r| r.display().to_string()).unwrap_or_default()),
            "fine in the dev tree; else put ~/.local/bin first in PATH",
        ),
    }
}

/// The voice input's model (`[voice]`, BISE-130): optional, so a problem
/// is a warning (ctrl+r fails, nothing else does). `key`: its provider's
/// key is set (None: the provider needs none).
fn voice_check(v: &bise_catalog::voice::VoiceSetup, r: &bise_catalog::voice::SttResolved, key: Option<bool>) -> Check {
    // BISE-298: the origin only when not config.toml
    let what = match v.from {
        "config" => r.name.clone(),
        "default" => format!("auto · {}", r.name),
        env => format!("{} ({})", r.name, env),
    };
    let fix_model = format!("pick one with /voice in bise, or `{} config set voice <provider/model>` (`{} models voice` lists them)", bise_catalog::CLI, bise_catalog::CLI);
    if r.known == bise_catalog::Known::NoProvider {
        return warn("voice", format!("{}: unknown provider '{}'", what, r.provider), fix_model);
    }
    if r.api.is_empty() {
        return warn("voice", format!("{}: {} does not transcribe", what, r.provider), fix_model);
    }
    if !r.needs.is_empty() {
        return warn("voice", format!("{}: not usable yet ({})", what, r.needs), fix_model);
    }
    if key == Some(false) {
        return warn(
            "voice",
            format!("{}: no {} key", what, r.provider),
            format!("`{} login {}` (or set {})", bise_catalog::CLI, r.provider, r.key_env),
        );
    }
    ok("voice", format!("{} · language {} · listens when you talk", what, v.language.as_deref().unwrap_or("auto")))
}

/// config.toml's warnings (what `bise models` prints under its list).
fn config_check(path: &Path, warnings: &[String]) -> Check {
    match warnings {
        [] => ok("config", if path.exists() { path.display().to_string() } else { format!("{} (none: the defaults)", path.display()) }),
        ws => warn("config", ws.join(" · "), format!("fix {} (`{} models` shows the same)", path.display(), bise_catalog::CLI)),
    }
}

/// The fix of a model whose provider has no key (BISE-266): a provider
/// with a key and a default model → that model; a hidden provider (a
/// private proxy: nobody else can log in to it) → pick one from the
/// start; else its login.
fn no_key_fix(setup: &bise_catalog::Setup, r: &bise_catalog::Resolved, found: &[(String, String)]) -> String {
    let cli = bise_catalog::CLI;
    let keyed = setup
        .catalog
        .providers
        .iter()
        .find(|p| !p.model.is_empty() && p.chats() && p.needs.is_empty() && found.iter().any(|(id, _)| *id == p.id));
    if let Some(p) = keyed {
        return format!("you have a {} key: set model = \"{}/{}\" in config.toml", p.id, p.id, p.model);
    }
    if setup.catalog.provider(&r.provider).is_some_and(|p| p.hidden) {
        return format!("run `{cli}` and pick a provider, or `{cli} login anthropic` (any provider: `{cli} models`) and set model in config.toml");
    }
    format!("`{} login {}` (or set {})", cli, r.provider, r.key_env)
}

/// A base URL as doctor shows it: no user:password, no query (a token
/// may sit there).
pub(crate) fn shown_url(u: &str) -> String {
    let (scheme, rest) = u.split_once("://").unwrap_or(("", u));
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let rest = match rest.split_once('/') {
        Some((auth, path)) => format!("{}/{}", auth.rsplit('@').next().unwrap_or(auth), path),
        None => rest.rsplit('@').next().unwrap_or(rest).to_string(),
    };
    if scheme.is_empty() { rest } else { format!("{}://{}", scheme, rest) }
}

/// The keys, model and voice lines.
fn keys_and_model(home: &bise_home::Home) -> (Check, Vec<Check>, Check) {
    let store = match Store::read(&home.auth_file()) {
        Ok(s) => s,
        Err(e) => {
            let f = fail("keys", format!("auth.json unreadable: {}", e), format!("fix or remove {}", home.auth_file().display()));
            return (f.clone(), vec![fail("main", "keys unknown", "fix auth.json first")], warn("voice", "keys unknown", "fix auth.json first"));
        }
    };
    let files = EnvFile::read_all(&home.env_files());
    let setup = bise_catalog::Setup::load(&home.config_file());
    let env = |k: &str| std::env::var(k).ok();
    let keys = Keys { env: &env, store: &store, files: &files };
    let user = Some(home.user_home());
    let found: Vec<(String, String)> = setup
        .catalog
        .providers
        .iter()
        .filter_map(|p| keys.source(p, user).map(|from| (p.id.clone(), from)))
        .collect();
    let keys_line = keys_check(&found);
    let model = |label: &str, name: &str, from: &str| -> Result<String, (String, String)> {
        // BISE-266: no built-in model: none until a key is checked
        if name.trim().is_empty() {
            return Err((
                format!("{}: no model yet", label),
                format!("run `{}` and pick a provider (it checks the key), or set model in config.toml", bise_catalog::CLI),
            ));
        }
        let r = setup.catalog.resolve(name);
        let what = format!("{} {} ({})", label, r.name, from);
        if r.known == bise_catalog::Known::NoProvider {
            return Err((
                format!("{}: unknown provider '{}'", what, r.provider),
                format!("add [providers.{}] to config.toml, or pick a listed model (`{} models`)", r.provider, bise_catalog::CLI),
            ));
        }
        if !r.needs.is_empty() {
            return Err((format!("{}: not usable yet ({})", what, r.needs), format!("pick another model (`{} models`)", bise_catalog::CLI)));
        }
        if r.base_url.is_empty() {
            let fix = setup.catalog.no_base_url(&r.provider);
            let fix = fix.split_once(": ").map(|(_, f)| f.to_string()).unwrap_or(fix);
            return Err((format!("{}: {} has no base URL", what, r.provider), fix));
        }
        if setup.catalog.provider(&r.provider).is_some_and(|p| p.signs_in() && !keys.signed_in(p)) {
            return Err((format!("{}: not signed in to {}", what, r.provider), format!("`{} login {}`", bise_catalog::CLI, r.provider)));
        }
        if r.caps.key_command.is_empty() && !r.key_env.is_empty() && keys.find(&r.provider, &r.key_env).is_none() {
            return Err((format!("{}: no {} key", what, r.provider), no_key_fix(&setup, &r, &found)));
        }
        Ok(what)
    };
    // BISE-298: one line per role, by its name: what runs (a fallback:
    // its rule word), its effort, what it is for; the origin only when
    // not config.toml
    let roles: [(&'static str, &str, &'static str, &str); 3] = [
        ("main", &setup.model, setup.model_from, ""),
        ("agents", &setup.agent_model, setup.agent_model_from, ""),
        ("small jobs", &setup.small_model, setup.small_model_from, "titles, summaries"),
    ];
    let model_lines: Vec<Check> = roles
        .iter()
        .map(|(name, m, from, about)| match model(name, m, from) {
            Err((detail, fix)) => fail(name, detail, fix),
            Ok(_) => {
                let r = setup.catalog.resolve(m);
                let mut parts = vec![match *from {
                    "model" => "same as main".to_string(),
                    "agent_model" => "same as agents".to_string(),
                    "provider" | "default" => format!("auto · {}", r.name),
                    "config" => r.name.clone(),
                    env => format!("{} ({})", r.name, env),
                }];
                let asked = match *name {
                    "main" => setup.effort.as_str(),
                    "agents" if *from != "model" => setup.agent_effort.as_str(),
                    _ => "",
                };
                let e = r.effort_for(asked);
                if (*name == "main" || (*name == "agents" && *from != "model")) && !e.is_empty() {
                    parts.push(e);
                }
                if !about.is_empty() {
                    parts.push(about.to_string());
                }
                // the URL in use when it is not the built-in one (config.toml,
                // ANTHROPIC_FOUNDRY_BASE_URL): what the agents call
                if let Some(p) = setup.catalog.provider(&r.provider).filter(|p| p.base_url_from != "built-in") {
                    parts.push(format!("{} ({})", shown_url(&p.base_url), p.base_url_from));
                }
                ok(name, parts.join(" · "))
            }
        })
        .collect();
    let stt = setup.catalog.resolve_stt(&setup.voice.model);
    let stt_key = (!stt.key_env.is_empty()).then(|| keys.find(&stt.provider, &stt.key_env).is_some());
    (keys_line, model_lines, voice_check(&setup.voice, &stt, stt_key))
}

/// When the conversations compact (BISE-300): BEND_THRESHOLD (`env`),
/// else config `compaction_threshold`, else 80 % of the window; tokens
/// or a share of the window, never above 80 % of it. One figure per
/// model that runs (main, the agents when theirs differs).
pub(crate) fn compaction_check(setup: &bise_catalog::Setup, env: Option<&str>) -> Check {
    let env = env.map(str::trim).filter(|v| !v.is_empty());
    let (written, from) = match env {
        Some(v) => (Some(v), "BEND_THRESHOLD"),
        None => (setup.compaction_threshold.as_deref(), "compaction_threshold"),
    };
    let short = |n: u64| {
        if n >= 1_000_000 && n.is_multiple_of(1_000_000) {
            format!("{}M", n / 1_000_000)
        } else if n.is_multiple_of(1000) {
            format!("{}k", n / 1000)
        } else {
            n.to_string()
        }
    };
    let mut who: Vec<(&str, &str)> = vec![("main", &setup.model)];
    if !setup.agent_model.is_empty() && setup.agent_model != setup.model {
        who.push(("agents", &setup.agent_model));
    }
    let figures: Vec<String> = who
        .iter()
        .filter(|(_, m)| !m.trim().is_empty())
        .map(|(name, m)| {
            let w = setup.catalog.context_window(m);
            let t = bise_catalog::compaction_threshold(written, w);
            let capped = written.and_then(|v| bise_catalog::threshold_written(v, w)).is_some_and(|n| n > t);
            let note = if capped { format!(" (capped: 80% of its {} window)", short(w)) } else { String::new() };
            format!("{} {} tokens{}", name, t, note)
        })
        .collect();
    let figures = if figures.is_empty() { String::new() } else { format!(" · {}", figures.join(" · ")) };
    match written {
        None => ok("compaction", format!("at 80% of the model's window{}", figures)),
        Some(v) if bise_catalog::threshold_written(v, 1_000_000).is_none() => warn(
            "compaction",
            format!("{} = {} is not a threshold: 80% of the window applies{}", from, v, figures),
            "set compaction_threshold = 450000 (tokens) or \"45%\" (of the window) in config.toml",
        ),
        Some(v) => ok("compaction", format!("{} = {}{}", from, v, figures)),
    }
}

fn hubs(home: &bise_home::Home) -> Check {
    let ws = crate::sb_workspace(&[]);
    let paths = switchboard::paths::Paths::for_workspace(&ws);
    let here = if switchboard::switch::hub_busy(&paths.state) {
        // where a hub says what went wrong: hub.log (its REPL starts and
        // exits), hub.err, each agent's repl.err (BISE-291)
        format!(
            "running for {} ({}; logs: hub.log, hub.err, agents/*/repl.err)",
            paths.workspace.display(),
            paths.state.display()
        )
    } else {
        format!("none for {} (`{}` starts one)", paths.workspace.display(), bise_catalog::CLI)
    };
    let count = |dir: &Path| -> usize {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| bise_home::migrate::is_hub_id(&e.file_name().to_string_lossy()))
                    .filter(|e| e.path().is_dir() && switchboard::switch::hub_busy(&e.path()))
                    .count()
            })
            .unwrap_or(0)
    };
    let new = count(&home.hubs_dir());
    let old_dir = home.user_home().join(".local/state/switchboard");
    let old = if old_dir == home.hubs_dir() { 0 } else { count(&old_dir) };
    let detail = format!("{}; {} running in {}, {} in the old place", here, new, home.hubs_dir().display(), old);
    let socket = socket_check(&paths.natural_socket());
    if socket.mark == Mark::Fail {
        return socket;
    }
    if old > 0 && home.layout() == bise_home::Layout::Bise {
        warn("hubs", detail, "they move to ~/.bise at their next restart")
    } else {
        ok("hubs", detail)
    }
}

fn disk(home: &bise_home::Home) -> Check {
    let at = if home.root().exists() { home.root().to_path_buf() } else { home.user_home().to_path_buf() };
    let kb = Command::new("/bin/df")
        .arg("-Pk")
        .arg(&at)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .nth(1)
                .and_then(|l| l.split_whitespace().nth(3).and_then(|v| v.parse().ok()))
        });
    disk_check(&at, kb)
}

/// `bise doctor`: the report on stdout; 1 when a check failed.
pub(crate) fn main(args: &[String]) -> i32 {
    let verbose = match args {
        [] => false,
        [a] if a == "--verbose" || a == "-v" => true,
        _ => {
            eprintln!("{}", Style::stderr().fail(&format!("usage: {} doctor [--verbose]", bise_catalog::CLI)));
            return 2;
        }
    };
    let home = bise_home::Home::from_env();
    let (keys, models, voice) = keys_and_model(&home);
    let setup = bise_catalog::Setup::load(&home.config_file());
    let config = config_check(&home.config_file(), &setup.catalog.warnings);
    let compaction = compaction_check(&setup, std::env::var("BEND_THRESHOLD").ok().as_deref());
    let mut checks = vec![os(), bise(verbose)];
    // a code signature is a macOS thing
    if cfg!(target_os = "macos") {
        checks.push(signature());
    }
    checks.extend([
        on_path(),
        home_check(&home),
        migration(&home),
        git(),
        rg(),
        github(),
        config,
        keys,
    ]);
    // the ChatGPT sign-in, then the other tools' logins (presence only)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(store) = Store::read(&home.auth_file()) {
        checks.extend(chatgpt_check(&store, now));
    }
    let env = |k: &str| std::env::var(k).ok();
    checks.extend(detected_checks(&bise_catalog::detect::detect(home.user_home(), &env)));
    checks.extend(models);
    checks.extend([compaction, voice, hubs(&home), disk(&home)]);
    checks.extend(tmux());
    let st = Style::stdout();
    println!("{}", st.title("checking your setup"));
    println!();
    print!("{}", render(&checks, &st, Some(home.user_home())));
    println!();
    println!("{}", summary(&checks, &st));
    if checks.iter().any(|c| c.mark == Mark::Fail) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_says_its_distro_and_what_is_macos_only() {
        let t = "NAME=NixOS
PRETTY_NAME=\"NixOS 25.11 (Xantusia)\"
ID=nixos
";
        assert_eq!(os_release_name(t).as_deref(), Some("NixOS 25.11 (Xantusia)"));
        assert_eq!(os_release_name("ID=x
"), None);
        let c = linux_check(Some("NixOS 25.11 (Xantusia)"), "arm64", true);
        assert_eq!(c.mark, Mark::Ok);
        assert!(c.detail.starts_with("NixOS 25.11 (Xantusia) arm64 (the Nix flake) · macOS-only"), "{}", c.detail);
        assert!(c.detail.contains("the sandbox (auto checks each command)"), "{}", c.detail);
        assert_eq!(linux_check(None, "x86_64", false).detail.split(" · ").next(), Some("Linux x86_64"));
    }

    #[test]
    fn macos_versions() {
        assert_eq!(macos_check(Some("26.6"), "14.0", "arm64", false).mark, Mark::Ok);
        assert_eq!(macos_check(Some("14.0"), "14.0", "arm64", false).mark, Mark::Ok);
        let old = macos_check(Some("13.6.1"), "14.0", "x86_64", false);
        assert_eq!(old.mark, Mark::Fail);
        assert!(old.fix.unwrap().contains("14.0"));
        assert_eq!(macos_check(Some("15.1"), "14.0", "x86_64", true).mark, Mark::Warn);
        assert_eq!(macos_check(None, "14.0", "arm64", false).mark, Mark::Warn);
    }

    #[test]
    fn github_through_gh() {
        let gh = Path::new("/opt/homebrew/bin/gh");
        let gh_url = Some("https://github.com/o/r.git");
        let c = github_check(Some(gh), Some(true), gh_url);
        assert_eq!((c.mark, c.detail.as_str()), (Mark::Ok, "gh logged in (/opt/homebrew/bin/gh); this repo's PRs: github.com/o/r"));
        let c = github_check(Some(gh), Some(false), gh_url);
        assert_eq!(c.mark, Mark::Warn);
        assert_eq!(c.fix.as_deref(), Some("`gh auth login` once"));
        let c = github_check(None, None, gh_url);
        assert_eq!((c.mark, c.detail.as_str()), (Mark::Warn, "gh not found: the PRs of github.com/o/r are not followed"));
        // not a GitHub repo: nothing to fix
        assert_eq!(github_check(None, None, None).mark, Mark::Ok);
        let c = github_check(Some(gh), Some(false), Some("git@gitlab.com:o/r.git"));
        assert_eq!((c.mark, c.detail.as_str()), (Mark::Ok, "gh not logged in (/opt/homebrew/bin/gh); this folder is not a GitHub repo"));
        // GitHub Enterprise: a host gh is logged in to
        let c = github_check(Some(gh), Some(true), Some("https://ghe.corp/o/r"));
        assert!(c.detail.ends_with("this repo's PRs: ghe.corp/o/r"), "{}", c.detail);
    }

    #[test]
    fn tmux_needs_extended_keys_for_ctrl_digits() {
        assert_eq!(tmux_check(None), None, "not in tmux");
        assert_eq!(tmux_check(Some(Some("always"))), None);
        for v in [Some("off"), Some("on"), None] {
            let c = tmux_check(Some(v)).unwrap();
            assert_eq!((c.mark, c.detail.as_str()), (Mark::Warn, "tmux eats ctrl+1-9"));
            assert_eq!(c.fix.as_deref(), Some("add \"set -s extended-keys always\" to ~/.tmux.conf"));
        }
    }

    #[test]
    fn socket_length() {
        let short = format!("/Users/me/.bise/hubs/{}-0123abcd/hub.sock", "a".repeat(32));
        assert_eq!(socket_check(Path::new(&short)).mark, Mark::Ok);
        let long = format!("/Users/me/{}/hubs/x-0123abcd/hub.sock", "d".repeat(80));
        // over the limit: fine, reached through /tmp/bise-<uid>/<hash>/
        let c = socket_check(Path::new(&long));
        assert_eq!(c.mark, Mark::Ok);
        assert!(c.detail.contains("reached as /tmp/bise-"), "{}", c.detail);
    }

    #[test]
    fn disk_thresholds() {
        let gb = 1024 * 1024;
        assert_eq!(disk_check(Path::new("/"), Some(20 * gb)).mark, Mark::Ok);
        assert_eq!(disk_check(Path::new("/"), Some(3 * gb)).mark, Mark::Warn);
        assert_eq!(disk_check(Path::new("/"), Some(gb / 2)).mark, Mark::Fail);
        assert_eq!(disk_check(Path::new("/"), None).mark, Mark::Warn);
    }

    #[test]
    fn keys_never_print_a_key_only_the_source() {
        let c = keys_check(&[("mistral".into(), "auth.json".into()), ("openai".into(), "env OPENAI_API_KEY".into())]);
        assert_eq!(c.mark, Mark::Ok);
        assert_eq!(c.detail, "mistral (auth.json), openai (env OPENAI_API_KEY)");
        let none = keys_check(&[]);
        assert_eq!(none.mark, Mark::Fail);
        assert!(none.fix.unwrap().contains("bise login"));
    }

    /// The ChatGPT line and the other tools' logins: never a token.
    #[test]
    fn the_chatgpt_sign_in_and_other_logins_get_their_lines() {
        use bise_catalog::auth::OAuth;
        use bise_catalog::chatgpt::{parse_rfc3339, rfc3339};
        let now = parse_rfc3339("2026-10-04T12:00:00Z").unwrap();
        let mut store = Store::default();
        assert_eq!(chatgpt_check(&store, now), None);
        let o = OAuth {
            client_id: "oaiapp_x".into(),
            email: "you@example.com".into(),
            plan: "plus".into(),
            access: "at-secret".into(),
            refresh: "rt-secret".into(),
            saved_at: rfc3339(now - 4 * 86_400),
            ..OAuth::default()
        };
        store.set_oauth("chatgpt", &o);
        let c = chatgpt_check(&store, now).unwrap();
        assert_eq!(c.mark, Mark::Ok);
        assert_eq!(c.detail, "signed in as you@example.com (Plus) · renews by itself · good until 30 Oct");
        let rendered = render(&[c], &Style::PLAIN, None);
        assert!(!rendered.contains("secret"), "{rendered}");
        // under 3 days left: a warning
        store.set_oauth("chatgpt", &OAuth { saved_at: rfc3339(now - 28 * 86_400), ..o.clone() });
        let c = chatgpt_check(&store, now).unwrap();
        assert_eq!((c.mark, c.detail.as_str(), c.fix.as_deref()), (Mark::Warn, "the sign-in ends in 2 days", Some("run bise login chatgpt")));
        store.set_oauth("chatgpt", &OAuth { saved_at: rfc3339(now - 29 * 86_400), ..o.clone() });
        assert_eq!(chatgpt_check(&store, now).unwrap().detail, "the sign-in ends in 1 day");
        // a refused refresh: expired; signed out by the user: nothing
        store.sign_out("chatgpt", true);
        assert_eq!(chatgpt_check(&store, now).unwrap().detail, "the sign-in (you@example.com) expired");
        store.sign_out("chatgpt", false);
        assert_eq!(chatgpt_check(&store, now), None);
        // detection: info lines, nothing to fix
        let d = bise_catalog::detect::Detected { codex_chatgpt: true, claude_plan: true };
        let lines = detected_checks(&d);
        assert_eq!(lines.iter().map(|c| (c.mark, c.name)).collect::<Vec<_>>(), [(Mark::Info, "codex"), (Mark::Info, "claude code")]);
        assert_eq!(lines[0].detail, "signed in with ChatGPT. bise signs in on its own: bise login chatgpt");
        let text = render(&lines, &Style::PLAIN, None);
        assert!(text.starts_with("· codex "), "{text}");
        assert_eq!(summary(&lines, &Style::PLAIN), "✓ all good.");
        assert!(detected_checks(&Default::default()).is_empty());
        // a sign-in alone passes the keys check
        assert_eq!(keys_check(&[("chatgpt".into(), "ChatGPT sign-in (you@example.com)".into())]).mark, Mark::Ok);
    }

    /// A base URL in doctor's lines: never its user:password nor its query.
    #[test]
    fn shown_url_drops_credentials_and_query() {
        assert_eq!(shown_url("https://proxy.example/anthropic/v1"), "https://proxy.example/anthropic/v1");
        assert_eq!(shown_url("https://u:tok@proxy.example/a/v1?key=tok#x"), "https://proxy.example/a/v1");
        assert_eq!(shown_url("http://tok@h:8080"), "http://h:8080");
    }

    /// qa G: doctor reads `[voice]` and shows config.toml's warnings.
    #[test]
    fn voice_and_config_lines() {
        let d = std::env::temp_dir().join(format!("doctor-voice-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let cfg = d.join("config.toml");
        std::fs::write(&cfg, "[voice]\nmodel = \"nosuch/whisper\"\nlanguage = 42\nbogus = true\n").unwrap();
        let s = bise_catalog::Setup::load(&cfg);
        let r = s.catalog.resolve_stt(&s.voice.model);
        let v = voice_check(&s.voice, &r, None);
        assert_eq!(v.mark, Mark::Warn);
        assert!(v.detail.contains("unknown provider 'nosuch'"), "{v:?}");
        let c = config_check(&cfg, &s.catalog.warnings);
        assert_eq!(c.mark, Mark::Warn);
        assert!(c.detail.contains("voice."), "{c:?}");
        std::fs::write(&cfg, "").unwrap();
        let s = bise_catalog::Setup::load(&cfg);
        let r = s.catalog.resolve_stt(&s.voice.model);
        assert_eq!(voice_check(&s.voice, &r, Some(true)).mark, Mark::Ok);
        assert_eq!(voice_check(&s.voice, &r, Some(false)).mark, Mark::Warn, "no key");
        assert_eq!(config_check(&cfg, &s.catalog.warnings).mark, Mark::Ok);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// BISE-300: the compaction line: the default, a number capped on a
    /// smaller agent model, a share, a bad value, BEND_THRESHOLD first.
    #[test]
    fn compaction_line() {
        let cfg = |extra: &str| {
            bise_catalog::Setup::from_text(
                Some(&format!("{}[roles]\nmain = \"foundry/claude-opus-5-5\"\nagents = \"anthropic/claude-haiku-4-5\"\n", extra)),
                &|_| None,
            )
        };
        let c = compaction_check(&cfg(""), None);
        assert_eq!(c.mark, Mark::Ok);
        assert_eq!(c.detail, "at 80% of the model's window · main 800000 tokens · agents 160000 tokens");
        let c = compaction_check(&cfg("compaction_threshold = 450000\n"), None);
        assert_eq!(c.detail, "compaction_threshold = 450000 · main 450000 tokens · agents 160000 tokens (capped: 80% of its 200k window)");
        let c = compaction_check(&cfg("compaction_threshold = \"45%\"\n"), None);
        assert_eq!(c.detail, "compaction_threshold = 45% · main 450000 tokens · agents 90000 tokens");
        let c = compaction_check(&cfg("compaction_threshold = \"lots\"\n"), None);
        assert_eq!(c.mark, Mark::Warn);
        assert!(c.detail.starts_with("compaction_threshold = lots is not a threshold"), "{c:?}");
        let c = compaction_check(&cfg("compaction_threshold = 450000\n"), Some("10%"));
        assert_eq!(c.detail, "BEND_THRESHOLD = 10% · main 100000 tokens · agents 20000 tokens");
        // the old key: not read, the config line says so
        let s = cfg("threshold = 450000\n");
        assert_eq!(compaction_check(&s, None).detail, "at 80% of the model's window · main 800000 tokens · agents 160000 tokens");
        assert!(s.catalog.warnings.iter().any(|w| w.contains("rename it compaction_threshold")), "{:?}", s.catalog.warnings);
    }

    /// qa C: a fresh HOME is not an "old layout".
    #[test]
    fn a_fresh_home_is_not_an_old_layout() {
        let d = std::env::temp_dir().join(format!("doctor-fresh-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let hs = d.to_string_lossy().into_owned();
        let home = bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| hs.clone()));
        let c = home_check(&home);
        assert!(c.detail.contains(".bise does not exist yet") && !c.detail.contains("old layout"), "{c:?}");
        std::fs::create_dir_all(d.join(".bend-harness")).unwrap();
        assert!(home_check(&home).detail.contains("old layout"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn migration_states() {
        let done = serde_json::json!({"copied": ["a", "b"], "hubs_moved": ["h"], "hubs_waiting": [], "errors": []});
        let c = migration_check(Some(&done), false, false, true);
        assert_eq!(c.mark, Mark::Ok);
        assert!(c.detail.contains("2 copied, 1 hubs moved, 0 waiting"), "{}", c.detail);
        let waiting = serde_json::json!({"copied": [], "hubs_moved": [], "hubs_waiting": ["x"], "errors": []});
        assert_eq!(migration_check(Some(&waiting), false, false, true).mark, Mark::Warn);
        assert_eq!(migration_check(None, true, false, true).mark, Mark::Ok);
        assert_eq!(migration_check(None, false, false, true).mark, Mark::Warn);
        // qa C: a fresh HOME has nothing to move
        let fresh = migration_check(None, false, false, false);
        assert_eq!(fresh.mark, Mark::Ok);
        assert!(fresh.detail.contains("nothing to move"), "{}", fresh.detail);
    }

    #[test]
    fn a_launcher_on_path_that_runs_this_bise_is_fine() {
        let (found, launcher) = (Path::new("/h/.local/bin/bise"), Path::new("/h/.bise/dev/bin/bise"));
        let (exe, root) = (Path::new("/v/abc/bise"), Path::new("/v/abc"));
        // the binary itself on PATH
        assert_eq!(path_check(found, Some(exe), Some(exe), None, Some(root)).mark, Mark::Ok);
        // a launcher that runs this version (the dev channel, an install)
        let c = path_check(found, Some(launcher), Some(exe), Some(root), Some(root));
        assert_eq!(c.mark, Mark::Ok, "{c:?}");
        assert!(c.detail.contains("launcher that runs this bise"), "{c:?}");
        // a launcher that runs another version: a warning, never a failure
        let c = path_check(found, Some(launcher), Some(exe), Some(Path::new("/v/old")), Some(root));
        assert_eq!(c.mark, Mark::Warn);
        assert!(c.detail.contains("/v/old"), "{c:?}");
        // another binary
        assert_eq!(path_check(found, Some(Path::new("/x/bise")), Some(exe), None, Some(root)).mark, Mark::Warn);
    }

    #[test]
    fn one_line_per_check_with_its_fix() {
        let checks = [ok("git", "git version 2.50"), fail("keys", "no provider key", "`bise login <provider>`")];
        let out = render(&checks, &Style::PLAIN, None);
        assert_eq!(out, "✓ git   git version 2.50\n✗ keys  no provider key\n        fix: `bise login <provider>`\n");
        assert_eq!(summary(&checks, &Style::PLAIN), "✗ 1 thing to fix. it says how.");
        // a warning is the TUI's '?'; paths under the home read ~/
        let w = [warn("PATH", "/h/.local/bin/bise is another bise", "put /h/.local/bin first")];
        assert_eq!(render(&w, &Style::PLAIN, Some(Path::new("/h"))), "? PATH  ~/.local/bin/bise is another bise\n        fix: put ~/.local/bin first\n");
        assert_eq!(tilde_paths("/h/x (/h/y) /private/h/z", "/h/"), "~/x (~/y) /private/h/z");
        // colors only on a terminal: the same words once stripped
        let tty = Style { color: true, light: false, width: 0 };
        assert_eq!(bise_home::style::strip(&render(&w, &tty, None)), render(&w, &Style::PLAIN, None));
        // a terminal wraps a long value on its column, never at column 0
        let long = [ok("hubs", "none for /a/very/long/workspace/path (`bise` starts one); 0 running in /h/.bise/hubs, 0 in the old place")];
        let narrow = Style { color: false, light: false, width: 50 };
        let out = render(&long, &narrow, None);
        assert!(out.lines().count() > 1 && out.lines().all(|l| l.chars().count() <= 50), "{out}");
        assert!(out.lines().skip(1).all(|l| l.starts_with("        ") && !l.starts_with("         ")), "{out}");
        let all = [ok("git", "x"), warn("PATH", "y", "z"), warn("voice", "y", "z"), fail("keys", "n", "f")];
        assert_eq!(summary(&all[..1], &Style::PLAIN), "✓ all good.");
        assert_eq!(summary(&all[..3], &Style::PLAIN), "? 2 things to check. each one says how.");
        assert_eq!(summary(&all, &Style::PLAIN), "✗ 1 thing to fix, 2 to check. each one says how.");
    }
}
