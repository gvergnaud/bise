//! The agents' tools (BISE-166, docs/research/portable-bise.md §3.4):
//! the PATH their shells get, whether `git` really works, whether `rg`
//! is there, and the note that tells the model so once per session.
//!
//! - The PATH is built on purpose: the hub's `bin` dir (`sb`), the
//!   hub's own PATH (the TUI that started it), the user's login-shell
//!   PATH, then the standard dirs; deduplicated. A hub started from a
//!   thin environment (an IDE task, launchd, `env -i`) still finds
//!   Homebrew tools. The last login PATH read is kept in the hub's state
//!   (`Paths::login_path_file`): a probe that fails (too slow under
//!   load) gives the agents the kept one.
//! - `git` on a fresh Mac is `/usr/bin/git`, a shim that opens the
//!   "install the Command Line Tools" dialog and fails. It is never run
//!   while `xcode-select -p` names no developer dir: the hub would pop
//!   the dialog at every git call.
//! - `rg` is not shipped: agents use it when it is on PATH, else grep.
//! - The hub's `bin` dir holds `sb` and `bise`, links to the hub's own
//!   executable (`write_agent_links`): the office skills' `bise office
//!   run` is the hub's version, with or without the CLI installed.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Appended to every agent PATH (after the user's own dirs).
const STD_DIRS: [&str; 6] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// The standard dirs of an OS, joined: [`STD_DIRS`], and on Linux the
/// NixOS ones too (the system profile, the user's profile, nix-env's):
/// a dir that does not exist costs nothing in a PATH.
pub fn std_dirs(macos: bool, home: &str, user: &str) -> String {
    let mut d: Vec<String> = STD_DIRS.iter().map(|s| s.to_string()).collect();
    if !macos {
        d.push("/run/current-system/sw/bin".into());
        if !user.is_empty() {
            d.push(format!("/etc/profiles/per-user/{}/bin", user));
        }
        if !home.is_empty() {
            d.push(format!("{}/.nix-profile/bin", home));
        }
    }
    d.join(":")
}

/// [`std_dirs`] of this host.
pub fn host_std_dirs() -> String {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    std_dirs(cfg!(target_os = "macos"), &var("HOME"), &var("USER"))
}

/// The login shell when `$SHELL` is unset: the OS's default.
pub fn default_shell(macos: bool) -> &'static str {
    if macos {
        "/bin/zsh"
    } else {
        "/bin/sh"
    }
}

/// How the login shell prints its PATH: `/usr/bin/printenv` when it
/// exists (macOS, most Linux), else `printenv` from the login PATH
/// (NixOS has no /usr/bin/printenv).
pub fn printenv_cmd(usr_bin_has_it: bool) -> &'static str {
    if usr_bin_has_it {
        "/usr/bin/printenv"
    } else {
        "printenv"
    }
}

/// The macOS git shim (xcrun): real git only with a developer dir.
pub const MACOS_GIT_SHIM: &str = "/usr/bin/git";

/// Join PATH strings in order: empty entries dropped, the first copy
/// of each dir kept.
pub fn join_path(parts: &[&str]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for d in parts.iter().flat_map(|p| p.split(':')) {
        if !d.is_empty() && !out.contains(&d) {
            out.push(d);
        }
    }
    out.join(":")
}

/// The agents' PATH: `bin_dir` (sb) : the hub's PATH : the login
/// shell's PATH : the standard dirs.
pub fn agent_path(bin_dir: &Path, inherited: &str, login: Option<&str>) -> String {
    let bin = bin_dir.to_string_lossy();
    join_path(&[&bin, inherited, login.unwrap_or(""), &host_std_dirs()])
}

/// The agents' PATH for this hub process (the login shell is asked
/// once per process; its last good answer is kept in the hub's state).
pub fn hub_agent_path(paths: &crate::paths::Paths) -> String {
    agent_path_for(paths, &std::env::var("PATH").unwrap_or_default(), login_path())
}

/// [`agent_path`] with the login PATH [`remembered_login`] gives.
pub fn agent_path_for(paths: &crate::paths::Paths, inherited: &str, probed: Option<&str>) -> String {
    let login = remembered_login(probed, &paths.login_path_file());
    agent_path(&paths.bin_dir(), inherited, login.as_deref())
}

/// The login PATH the agents get: this probe's when it read one (kept
/// in `saved` for the next hub), else the last one kept there. A probe
/// too slow under load (a switch restarts every hub at once; 3 s cap)
/// left a hub's agents without the user's node, pnpm, volta... for the
/// hub's whole life (the 2026-10-27 switch to ac98b858b).
pub fn remembered_login(probed: Option<&str>, saved: &Path) -> Option<String> {
    let kept = std::fs::read_to_string(saved).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let Some(p) = probed else { return kept };
    if kept.as_deref() != Some(p) {
        let tmp = saved.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&tmp, p).and_then(|_| std::fs::rename(&tmp, saved)).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    Some(p.to_string())
}

/// The PATH the hub itself finds git on: its own PATH + the standard
/// dirs (no login shell: git lives in a standard dir).
fn hub_tools_path() -> String {
    join_path(&[&std::env::var("PATH").unwrap_or_default(), &host_std_dirs()])
}

/// The user's login-shell PATH, asked once per process ($SHELL, else
/// /bin/zsh on macOS, /bin/sh on Linux; 3 s at most); None when the shell fails or is too slow.
/// The whole ask is bounded, the start of the shell included: a spawn
/// that never returned held every REPL start of the hub (BISE-291).
pub fn login_path() -> Option<&'static str> {
    static LOGIN: OnceLock<Option<String>> = OnceLock::new();
    LOGIN
        .get_or_init(|| {
            let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty());
            let shell = shell.unwrap_or_else(|| default_shell(cfg!(target_os = "macos")).into());
            bounded(Duration::from_secs(4), move || shell_path(&shell, Duration::from_secs(3))).flatten()
        })
        .as_deref()
}

/// `f`'s value if it comes within `limit`, else None (`f` goes on alone,
/// on its own thread).
fn bounded<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(limit).ok()
}

const MARK: &str = "__BISE_PATH__";

/// PATH as `shell -i -l` sets it (-l: .zprofile/.bash_profile, -i:
/// .zshrc/.bashrc, where nvm & co add their dirs). The shell runs in
/// its own session, so an interactive shell cannot reach the terminal
/// of the TUI that started the hub; stdin is /dev/null. The PATH is
/// the line after a marker (rc files may print banners). Works for
/// sh, bash, zsh and fish (printenv prints the joined form).
pub fn shell_path(shell: &str, timeout: Duration) -> Option<String> {
    let printenv = printenv_cmd(Path::new("/usr/bin/printenv").exists());
    let mut cmd = Command::new(shell);
    cmd.args(["-i", "-l", "-c", &format!("echo {}; {} PATH", MARK, printenv)])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // its own session, and no pipe of a concurrent spawn: a daemon its
    // rc files start must not hold one open (BISE-291)
    crate::procs::own_session(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        let _ = tx.send(s);
    });
    let got = rx.recv_timeout(timeout).ok();
    if got.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    parse_marked(&got?)
}

fn parse_marked(out: &str) -> Option<String> {
    let mut lines = out.lines().skip_while(|l| l.trim() != MARK);
    lines.next()?;
    let p = lines.next()?.trim();
    (!p.is_empty()).then(|| p.to_string())
}

/// The first executable file named `name` in a PATH string.
pub fn which(name: &str, path: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join(name)).find(|p| {
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

/// What `git` is on a PATH.
#[derive(Clone, Debug, PartialEq)]
pub enum Git {
    Works { bin: PathBuf, version: String },
    /// no git on the PATH
    Missing,
    /// the macOS shim without Command Line Tools (never run)
    Stub,
    /// `git --version` failed (its first error line)
    Broken(String),
}

const INSTALL: &str = "run `xcode-select --install` (macOS Command Line Tools) or install git, then restart the hub";

impl Git {
    /// Why git cannot be used, in a sentence for the user; None when it
    /// works.
    pub fn problem(&self) -> Option<String> {
        match self {
            Git::Works { .. } => None,
            Git::Missing => Some(format!("git is not installed (not found on PATH): {}", INSTALL)),
            Git::Stub => Some(format!(
                "git is not installed: {} is only the macOS installer stub (no Command Line Tools); {}",
                MACOS_GIT_SHIM, INSTALL
            )),
            Git::Broken(e) => Some(format!("git does not work (`git --version`: {})", e)),
        }
    }

    /// One line for the hub log.
    pub fn describe(&self) -> String {
        match self {
            Git::Works { bin, version } => format!("{} ({})", version, bin.display()),
            _ => self.problem().unwrap_or_default(),
        }
    }
}

/// Probe git on `path`. `shim`: the path that is only real git when
/// `developer_dir_ok()` (macOS: `/usr/bin/git` and `xcode-select -p`);
/// that one is never run otherwise. Else `git --version` must succeed.
pub fn probe_git_in(path: &str, shim: Option<&Path>, developer_dir_ok: &dyn Fn() -> bool) -> Git {
    let Some(bin) = which("git", path) else {
        return Git::Missing;
    };
    if shim.is_some_and(|s| bin == s) && !developer_dir_ok() {
        return Git::Stub;
    }
    match Command::new(&bin).arg("--version").stdin(Stdio::null()).output() {
        Ok(o) if o.status.success() => Git::Works {
            bin,
            version: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        Ok(o) => Git::Broken(first_line(&String::from_utf8_lossy(&o.stderr), &o.status.to_string())),
        Err(e) => Git::Broken(e.to_string()),
    }
}

fn first_line(s: &str, default: &str) -> String {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(default).to_string()
}

/// `xcode-select -p` names an existing developer dir (Command Line
/// Tools or Xcode). It never opens a dialog.
pub fn developer_dir_ok() -> bool {
    Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| Path::new(String::from_utf8_lossy(&o.stdout).trim()).is_dir())
}

fn macos_shim() -> Option<&'static Path> {
    cfg!(target_os = "macos").then(|| Path::new(MACOS_GIT_SHIM))
}

/// git for this process: a working git is kept for good; a missing or
/// stub one is probed again at most every 10 s (the user may install
/// the tools meanwhile; the probe never runs the stub).
pub fn git() -> Git {
    static CACHE: Mutex<Option<(Instant, Git)>> = Mutex::new(None);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match &*c {
        Some((_, g @ Git::Works { .. })) => return g.clone(),
        Some((t, g)) if t.elapsed() < Duration::from_secs(10) => return g.clone(),
        _ => {}
    }
    let g = unshim(probe_git_in(&hub_tools_path(), macos_shim(), &developer_dir_ok), macos_shim(), &xcrun_git);
    *c = Some((Instant::now(), g.clone()));
    g
}

/// The git the macOS shim runs, `xcrun --find git` once (boot-speed,
/// architect m_54396): every call is then one exec, not the shim's two
/// (at load ~100 the shim took 1 to 212 s a call, the real git 0.04 to
/// 13 s). Only when PATH's git is the shim: a git he put first (Homebrew)
/// stays his. xcrun failing, or naming no file: the shim, as before.
pub fn unshim(g: Git, shim: Option<&Path>, find: &dyn Fn() -> Option<PathBuf>) -> Git {
    match g {
        Git::Works { bin, version } if shim.is_some_and(|s| bin == s) => match find().filter(|p| p.is_file()) {
            Some(real) => Git::Works { bin: real, version },
            None => Git::Works { bin, version },
        },
        g => g,
    }
}

/// `xcrun --find git`: the shim's git (Command Line Tools or Xcode).
fn xcrun_git() -> Option<PathBuf> {
    let o = Command::new("/usr/bin/xcrun").args(["--find", "git"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (o.status.success() && !p.is_empty()).then(|| PathBuf::from(p))
}

/// A `git` command on the probed binary, or why git cannot be used: the
/// hub's one way to run git (law `the_hub_runs_git_only_here`).
/// `GIT_OPTIONAL_LOCKS=0`: git takes no lock it doesn't need, so the
/// hub's reads (a `status` refreshes the index under `index.lock`) never
/// make a land's rebase or commit in that worktree fail on the lock
/// (feature_retire_e2e under load); a write still takes its own lock.
pub fn git_command() -> Result<Command, String> {
    match git() {
        Git::Works { bin, .. } => {
            let mut c = Command::new(bin);
            c.env("GIT_OPTIONAL_LOCKS", "0");
            Ok(c)
        }
        g => Err(g.problem().unwrap_or_default()),
    }
}

/// The other CLIs the note names when they are on PATH: the ones an
/// agent otherwise probes with `which` (or guesses) before using them.
/// Probed with `which` only (a PATH scan, no process started).
pub const OTHER_CLIS: [&str; 13] = [
    "gh", "node", "npm", "pnpm", "bun", "python3", "uv", "cargo", "go", "jq", "tmux", "docker", "make",
];

/// The note's last line: which of OTHER_CLIS are on PATH (None: none).
pub fn other_clis_line(found: &[&str]) -> Option<String> {
    (!found.is_empty()).then(|| {
        let names: Vec<String> = found.iter().map(|n| format!("`{}`", n)).collect();
        format!("- Also installed: {}.", names.join(", "))
    })
}

/// What the model is told once per session (the end of its system
/// prompt, through BEND_TOOLS_NOTE): whether `rg` is there, whether
/// `git` works.
pub fn tools_note(rg: Option<&Path>, git: &Git) -> String {
    let rg_line = match rg {
        Some(_) => "- `rg` (ripgrep) is installed: use it to search files and file contents.".to_string(),
        None => "- `rg` (ripgrep) is NOT installed: search with `grep -rn` (and `find`) instead; do not call `rg`.".to_string(),
    };
    let git_line = match git {
        Git::Works { .. } => "- `git` works.".to_string(),
        Git::Stub => format!(
            "- `git` is NOT installed: {} is the macOS installer stub and each call opens a dialog on the user's screen. Never run `git`; when a task needs it, ask the user to run `xcode-select --install`.",
            MACOS_GIT_SHIM
        ),
        Git::Missing => "- `git` is NOT installed. When a task needs it, ask the user to install it (macOS: `xcode-select --install`).".to_string(),
        Git::Broken(e) => format!("- `git` does not work here (`git --version`: {}).", e),
    };
    format!("## Shell tools on this machine\n\n{}\n{}", rg_line, git_line)
}

/// The note for a shell whose PATH is `path`: rg, git, then the other
/// CLIs found on it.
pub fn tools_note_for(path: &str) -> String {
    let found: Vec<&str> = OTHER_CLIS.iter().copied().filter(|n| which(n, path).is_some()).collect();
    let note = tools_note(which("rg", path).as_deref(), &git());
    match other_clis_line(&found) {
        Some(line) => format!("{}\n{}", note, line),
        None => note,
    }
}

/// The loaded plugins of a workspace, as (name, description, skills)
/// for [`plugins_note`]: built-in, user and workspace ones, the enable
/// state applied (`bend_plugins::resolve`).
pub fn loaded_plugins(workspace: &Path) -> Vec<(String, String, Vec<String>)> {
    loaded_plugins_in(&bend_plugins::resolve::Roots::standard(Some(workspace)))
}

fn loaded_plugins_in(roots: &bend_plugins::resolve::Roots) -> Vec<(String, String, Vec<String>)> {
    loaded_plugins_of(&bend_plugins::resolve::resolve(roots))
}

/// The plugins on in a resolution, each with its skills on (the
/// resolver leaves a plugin off or a skill off out of `loaded()` and
/// `skills`): what [`plugins_note`] lists. A server, a tool or a Mistral
/// connector is never in it, so turning one off leaves the note as is.
pub fn loaded_plugins_of(res: &bend_plugins::resolve::Resolution) -> Vec<(String, String, Vec<String>)> {
    res.loaded()
        .map(|p| {
            // a plugin skill's name is already `<namespace>:<name>`, the
            // one `<available-skills>` and the skill tool take
            let skills = p.skills.iter().map(|s| s.name.clone()).collect();
            (p.name.clone(), p.description.clone().unwrap_or_default(), skills)
        })
        .collect()
}

/// The prompt section naming the session's plugins and what each one
/// is for (None: no plugin). The tool groups line only says `computer
/// (7 functions)` and a skill is one entry among many: without this an
/// agent with computer use answered "i can't see your browser" and ran
/// `open -a` (cu-try, 2026-10-01).
pub fn plugins_note(plugins: &[(String, String, Vec<String>)]) -> Option<String> {
    if plugins.is_empty() {
        return None;
    }
    let lines: Vec<String> = plugins
        .iter()
        .map(|(name, desc, skills)| {
            let mut line = format!("- `{}`", name);
            let desc = desc.trim();
            if !desc.is_empty() {
                line.push_str(&format!(": {}", desc));
            }
            if !skills.is_empty() {
                let names: Vec<String> = skills.iter().map(|s| format!("`{}`", s)).collect();
                line.push_str(&format!(" (skills: {})", names.join(", ")));
            }
            line
        })
        .collect();
    Some(format!(
        "## Plugins

What this session can do besides the shell and the connectors. Their tools are in `tools.<group>`, their skills in `<available-skills>`. When a request matches a plugin, it is an option: load its skill and use it.

{}",
        lines.join("\n")
    ))
}

/// The whole BEND_TOOLS_NOTE: the shell tools, then the plugins.
pub fn session_note(path: &str, workspace: &Path) -> String {
    let note = tools_note_for(path);
    match plugins_note(&loaded_plugins(workspace)) {
        Some(p) => format!("{}\n\n{}", note, p),
        None => note,
    }
}

/// The agent's folders in its tool env (approvals-design.md §7.1):
/// `TMPDIR`, `TMP`, `TEMP` and `TMUX_TMPDIR` are its `tmp/` (mktemp,
/// python tempfile, tmux sockets land there, never in /tmp); its
/// background jobs write in `tmp/bg`; the harness's own files go to
/// `run/` (`BEND_AGENT_RUN`, bend/runtime/persist.bend `side_dir`); the
/// full results of its calls with a view go to `views/` next to `run/`
/// (`BEND_VIEWS_DIR`, `Paths::agent_views_dir`; its plugins bridge writes
/// there, docs/mcp-apps.md C2).
///
/// `TMUX_TMPDIR` only when a tmux socket fits there: tmux puts it at
/// `<realpath>/tmux-<uid>/<name>` and a unix socket path is at most 103
/// bytes on macOS; a folder too deep leaves tmux on /tmp, as before.
/// The four names are `bise_home::env::TEMP_NAMES`, in its
/// `PER_CHILD_NAMES` (the plugins' tools cache leaves them out of its key).
pub fn temp_env(tmp: &Path, run: &Path) -> Vec<(&'static str, PathBuf)> {
    let fits = tmux_fits(tmp);
    let mut v: Vec<(&'static str, PathBuf)> = bise_home::env::TEMP_NAMES
        .iter()
        .filter(|k| fits || **k != "TMUX_TMPDIR")
        .map(|k| (*k, tmp.to_path_buf()))
        .collect();
    v.push(("BEND_BG_DIR", bise_home::hub_state::bg_dir_of_tmp(tmp)));
    v.push(("BEND_AGENT_RUN", run.to_path_buf()));
    if let Some(agent) = run.parent() {
        v.push(("BEND_VIEWS_DIR", bise_home::hub_state::views_of_agent_dir(agent)));
    }
    v
}

/// macOS's `/usr/bin/mktemp` ignores `TMPDIR` (it uses the user's
/// `/var/folders/…/T`, `_CS_DARWIN_USER_TEMP_DIR`) unless given `-p`: this
/// shim, first on the agents' PATH (the hub's `bin/`, next to `sb`), adds
/// `-p "$TMPDIR"` when the call names no folder and no template, so an
/// agent's `mktemp` lands in its temp folder like python's tempfile.
pub const MKTEMP_SHIM: &str = r#"#!/bin/sh
# bise (approvals-design.md 7.1): macOS mktemp ignores TMPDIR; an agent's
# lands in its temp folder. A folder or a template given: as written.
skip=
for a in "$@"; do
  if [ -n "$skip" ]; then skip=; continue; fi
  case "$a" in
    -p*|--tmpdir*) exec /usr/bin/mktemp "$@" ;;
    -t|-*t) skip=1 ;;
    -*) ;;
    *) exec /usr/bin/mktemp "$@" ;;
  esac
done
if [ -n "${TMPDIR:-}" ] && [ -d "$TMPDIR" ]; then exec /usr/bin/mktemp -p "$TMPDIR" "$@"; fi
exec /usr/bin/mktemp "$@"
"#;

/// The links in `bin_dir` to the hub's executable: `sb` (it runs `sb`
/// when called by that name, busybox style) and `bise`, so an agent's
/// `bise office run` (the office skills) is the hub's own version, whose
/// office.js and bend-jsrt match the skills it loaded, even for a desktop
/// user with no CLI on PATH. Nothing new for agents: a client command
/// they could misuse is refused by hub.sock's peer judge (issue 17).
pub const AGENT_LINKS: [&str; 2] = ["sb", "bise"];

/// Write the [`AGENT_LINKS`]. Each replaced in one rename: an agent of
/// the previous hub calling `sb` meanwhile finds the old one or the new
/// one, never none.
pub fn write_agent_links(bin_dir: &Path, exe: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(bin_dir)?;
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    for name in AGENT_LINKS {
        let tmp = bin_dir.join(format!(".{}.{}.tmp", name, std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        std::os::unix::fs::symlink(&exe, &tmp)?;
        std::fs::rename(&tmp, bin_dir.join(name)).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
    }
    Ok(())
}

/// Before a hub of another version starts: a hub older than the links
/// writes its `sb` script with a plain write, which would go through the
/// link into this executable. Without them, it writes a new file. Only a
/// link is dropped.
pub fn drop_agent_links(bin_dir: &Path) {
    for name in AGENT_LINKS {
        let p = bin_dir.join(name);
        if p.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Write the mktemp shim in `bin_dir` (macOS only: GNU mktemp reads
/// TMPDIR), in one rename.
pub fn write_mktemp_shim(bin_dir: &Path) -> std::io::Result<()> {
    if !cfg!(target_os = "macos") || !Path::new("/usr/bin/mktemp").exists() {
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    let dst = bin_dir.join("mktemp");
    if std::fs::read_to_string(&dst).is_ok_and(|s| s == MKTEMP_SHIM) {
        return Ok(());
    }
    std::fs::create_dir_all(bin_dir)?;
    let tmp = bin_dir.join(format!(".mktemp.{}.tmp", std::process::id()));
    std::fs::write(&tmp, MKTEMP_SHIM)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&tmp, dst).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The longest unix socket path (macOS: `sun_path` is 104 bytes, NUL
/// included).
pub const SOCKET_PATH_MAX: usize = 103;

/// Room left for a tmux socket name (`default`, a `-L` name).
pub const TMUX_NAME_ROOM: usize = 16;

/// Whether `<realpath of tmp>/tmux-<uid>/` + a 16-byte socket name fits
/// in a unix socket path (the folder must exist: its realpath, its uid).
pub fn tmux_fits(tmp: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(real), Ok(meta)) = (std::fs::canonicalize(tmp), std::fs::metadata(tmp)) else {
        return false;
    };
    let dir = format!("{}/tmux-{}/", real.to_string_lossy(), meta.uid());
    dir.len() + TMUX_NAME_ROOM <= SOCKET_PATH_MAX
}

/// Create the agent's `tmp/` and `run/` (private: 0700).
pub fn make_agent_dirs(tmp: &Path, run: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    for d in [tmp, run] {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// `bin/sb` and `bin/bise`: links to the exe, the old sb script
    /// replaced, in one rename (no temp file left); only a link is
    /// dropped for an older version's hub.
    #[test]
    fn the_agent_links_replace_the_script() {
        let d = std::env::temp_dir().join(format!("sb-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = d.join("bise");
        std::fs::write(&exe, "exe").unwrap();
        std::fs::write(bin.join("sb"), "#!/bin/sh\nexec old sb \"$@\"\n").unwrap();
        write_agent_links(&bin, &exe).unwrap();
        write_agent_links(&bin, &exe).unwrap();
        let (sb, bise) = (bin.join("sb"), bin.join("bise"));
        for l in [&sb, &bise] {
            assert!(l.symlink_metadata().unwrap().file_type().is_symlink());
            assert_eq!(std::fs::read_link(l).unwrap(), std::fs::canonicalize(&exe).unwrap());
        }
        let mut names: Vec<_> = std::fs::read_dir(&bin).unwrap().map(|e| e.unwrap().file_name()).collect();
        names.sort();
        assert_eq!(names, vec![std::ffi::OsString::from("bise"), std::ffi::OsString::from("sb")]);
        drop_agent_links(&bin);
        assert!(!sb.exists() && !bise.exists() && std::fs::read_to_string(&exe).unwrap() == "exe");
        std::fs::write(&sb, "a file").unwrap();
        drop_agent_links(&bin);
        assert!(sb.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// mktemp, python's tempfile and tmux read TMPDIR/TMP/TEMP and
    /// TMUX_TMPDIR: all four name the agent's tmp/; the runtime gets
    /// run/ and tmp/bg.
    #[test]
    fn the_temp_env_points_to_the_agent_folders() {
        let d = tmp("temp-env");
        let (t, r) = (d.join("a/tmp"), d.join("a/run"));
        make_agent_dirs(&t, &r).unwrap();
        let env = temp_env(&t, &r);
        let get = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
        for k in ["TMPDIR", "TMP", "TEMP"] {
            assert_eq!(get(k).as_ref(), Some(&t), "{}", k);
        }
        assert_eq!(get("TMUX_TMPDIR"), tmux_fits(&t).then(|| t.clone()));
        assert_eq!(get("BEND_BG_DIR"), Some(t.join("bg")));
        assert_eq!(get("BEND_AGENT_RUN"), Some(r.clone()));
        assert_eq!(get("BEND_VIEWS_DIR"), Some(d.join("a/views")));
        // per agent and not bise's: only PER_CHILD_NAMES (the tools
        // cache's key leaves exactly those out); the rest are registered
        for (k, _) in &env {
            assert!(
                bise_home::env::PER_CHILD_NAMES.contains(k) || bise_home::env::kind(k) == Some(bise_home::env::Kind::Internal),
                "{k} is set per agent but is neither in PER_CHILD_NAMES nor an internal bise name"
            );
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A tmux socket under a folder too deep would fail ("File name too
    /// long"): TMUX_TMPDIR is then left out (tmux stays on /tmp).
    #[test]
    fn tmux_goes_to_tmp_only_when_its_socket_fits() {
        // $TMPDIR when short (a sandboxed gate cannot write /tmp), else /tmp
        let t = std::env::temp_dir();
        let root = if t.as_os_str().len() > 40 { std::path::PathBuf::from("/tmp") } else { t };
        let d = root.join(format!("sbtx{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let short = d.join("a");
        let deep = d.join("x".repeat(90));
        std::fs::create_dir_all(&short).unwrap();
        std::fs::create_dir_all(&deep).unwrap();
        assert!(tmux_fits(&short));
        assert!(!tmux_fits(&deep));
        assert!(!tmux_fits(&d.join("missing")));
        assert!(!temp_env(&deep, &d).iter().any(|(k, _)| *k == "TMUX_TMPDIR"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The shim: no folder, no template: in $TMPDIR (`-t` too); a
    /// template or `-p`: as written.
    #[cfg(target_os = "macos")]
    #[test]
    fn mktemp_lands_in_tmpdir() {
        let d = tmp("mktemp");
        let (bin, t, cwd) = (d.join("bin"), d.join("t"), d.join("cwd"));
        std::fs::create_dir_all(&t).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        write_mktemp_shim(&bin).unwrap();
        write_mktemp_shim(&bin).unwrap();
        let run = |args: &[&str]| {
            let o = Command::new(bin.join("mktemp")).args(args).env("TMPDIR", &t).current_dir(&cwd).output().unwrap();
            assert!(o.status.success(), "{:?}", args);
            PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())
        };
        for args in [&[][..], &["-d"], &["-t", "foo"], &["-dt", "foo"], &["-q"]] {
            assert_eq!(run(args).parent().unwrap(), t.as_path(), "{:?}", args);
        }
        let other = d.join("other");
        std::fs::create_dir_all(&other).unwrap();
        assert_eq!(run(&["-p", other.to_str().unwrap()]).parent().unwrap(), other.as_path());
        let own = run(&["x.XXXX"]);
        assert!(own.is_relative() && cwd.join(&own).exists(), "a template stays in the cwd: {:?}", own);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_agent_dirs_are_private() {
        let d = tmp("agent-dirs");
        let (t, r) = (d.join("a/tmp"), d.join("a/run"));
        make_agent_dirs(&t, &r).unwrap();
        make_agent_dirs(&t, &r).unwrap();
        for p in [&t, &r] {
            assert_eq!(std::fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-tools-env-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{}\n", body)).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn agent_path_has_sb_first_then_hub_login_and_std_dirs_once() {
        let p = agent_path(
            Path::new("/state/bin"),
            "/usr/bin:/custom/bin::/bin",
            Some("/Users/u/.nvm/bin:/opt/homebrew/bin:/custom/bin"),
        );
        assert_eq!(
            p,
            "/state/bin:/usr/bin:/custom/bin:/bin:/Users/u/.nvm/bin:/opt/homebrew/bin:/usr/local/bin:/usr/sbin:/sbin"
        );
        // a thin environment (env -i, launchd): sb + the standard dirs
        assert_eq!(
            agent_path(Path::new("/s/bin"), "", None),
            "/s/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        );
    }

    /// A hub whose login-shell probe fails (too slow under a switch's
    /// load) still gives its agents the user's dirs: the last PATH a
    /// probe read, kept in the hub's state (2026-10-27: node and pnpm
    /// gone from every agent of the switched hub).
    #[test]
    fn a_failed_login_probe_keeps_the_last_login_path() {
        let d = tmp("login-kept");
        let paths = crate::paths::Paths { workspace: d.join("ws"), state: d.join("hub"), worktrees: d.join("wt"), run: d.join("run") };
        std::fs::create_dir_all(&paths.state).unwrap();
        let volta = format!("{}/home/.volta/bin", d.display());
        let login = format!("{}:/usr/bin", volta);
        let has = |p: &str| p.split(':').any(|e| e == volta);
        // never read on this hub: the thin PATH, nothing invented
        assert!(!has(&agent_path_for(&paths, "/usr/bin:/bin", None)));
        // a probe reads it: the agents get it, and it is kept
        assert!(has(&agent_path_for(&paths, "/usr/bin:/bin", Some(&login))));
        // the next hub's probe fails: the kept one, after the hub's PATH
        let p = agent_path_for(&paths, "/usr/bin:/bin", None);
        assert!(p.starts_with(&format!("{}:/usr/bin:/bin:{}", paths.bin_dir().display(), volta)), "{}", p);
        // a later probe's answer replaces it
        assert!(!has(&agent_path_for(&paths, "", Some("/other/bin"))));
        assert!(!has(&agent_path_for(&paths, "", None)));
        assert!(agent_path_for(&paths, "", None).contains("/other/bin"));
    }

    #[test]
    fn linux_std_dirs_add_the_nixos_profiles_and_macos_is_unchanged() {
        let mac = std_dirs(true, "/Users/u", "u");
        assert_eq!(mac, "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        assert_eq!(
            std_dirs(false, "/home/u", "u"),
            format!("{}:/run/current-system/sw/bin:/etc/profiles/per-user/u/bin:/home/u/.nix-profile/bin", mac)
        );
        // no HOME, no USER (env -i): the system profile only
        assert_eq!(std_dirs(false, "", ""), format!("{}:/run/current-system/sw/bin", mac));
        assert_eq!(default_shell(true), "/bin/zsh");
        assert_eq!(default_shell(false), "/bin/sh");
        assert_eq!(printenv_cmd(true), "/usr/bin/printenv");
        assert_eq!(printenv_cmd(false), "printenv");
    }

    #[test]
    fn login_shell_path_is_read_after_the_marker() {
        let d = tmp("shell");
        // an rc file that prints a banner, then the shell's -c command
        let sh = script(&d, "fakesh", "echo 'Welcome!'\nshift 3\nPATH=/from/login:/usr/bin; export PATH\neval \"$1\"");
        assert_eq!(
            shell_path(sh.to_str().unwrap(), Duration::from_secs(5)).as_deref(),
            Some("/from/login:/usr/bin")
        );
        // too slow: None, in about the timeout
        let slow = script(&d, "slowsh", "sleep 5");
        let t = Instant::now();
        assert_eq!(shell_path(slow.to_str().unwrap(), Duration::from_millis(300)), None);
        assert!(t.elapsed() < Duration::from_secs(3));
        // no shell, no output
        assert_eq!(shell_path("/nonexistent/sh", Duration::from_secs(1)), None);
        assert_eq!(parse_marked("x\n__BISE_PATH__\n\n"), None);
        assert!(shell_path("/bin/sh", Duration::from_secs(5)).is_some());
    }

    #[test]
    fn which_wants_an_executable_file() {
        let d = tmp("which");
        let a = d.join("a");
        let b = d.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("rg"), "not executable").unwrap();
        std::fs::create_dir_all(a.join("tool")).unwrap();
        let rg = script(&b, "rg", "true");
        let path = format!("{}:{}", a.display(), b.display());
        assert_eq!(which("rg", &path), Some(rg));
        assert_eq!(which("tool", &path), None);
        assert_eq!(which("rg", ""), None);
    }

    #[test]
    fn missing_rg_tells_the_model_to_use_grep() {
        let d = tmp("rg");
        let works = Git::Works { bin: "/usr/bin/git".into(), version: "git version 2.39.5".into() };
        let note = tools_note(which("rg", d.to_str().unwrap()).as_deref(), &works);
        assert!(note.starts_with("## Shell tools on this machine\n\n"), "{}", note);
        assert!(note.contains("`rg` (ripgrep) is NOT installed: search with `grep -rn`"), "{}", note);
        assert!(note.contains("- `git` works."));
        let rg = script(&d, "rg", "true");
        let note = tools_note(which("rg", d.to_str().unwrap()).as_deref(), &works);
        assert!(note.contains("`rg` (ripgrep) is installed: use it"), "{}", note);
        assert!(!note.contains("grep -rn"));
        assert_eq!(tools_note(Some(&rg), &works), note);
    }

    #[test]
    fn the_other_clis_found_are_named_in_one_line() {
        assert_eq!(other_clis_line(&[]), None);
        assert_eq!(other_clis_line(&["gh", "jq"]).as_deref(), Some("- Also installed: `gh`, `jq`."));
        let d = tmp("clis");
        script(&d, "jq", "true");
        script(&d, "rg", "true");
        let note = tools_note_for(d.to_str().unwrap());
        assert!(note.ends_with("\n- Also installed: `jq`."), "{}", note);
        let empty = tmp("clis-none");
        assert!(!tools_note_for(empty.to_str().unwrap()).contains("Also installed"));
    }

    #[test]
    fn the_plugins_note_names_each_plugin_its_use_and_skills() {
        assert_eq!(plugins_note(&[]), None);
        let note = plugins_note(&[
            ("computer".into(), "computer use: the user's browser and Mac apps".into(), vec!["computer:computer-use".into()]),
            ("hn".into(), " ".into(), vec![]),
        ])
        .unwrap();
        assert!(note.starts_with("## Plugins\n\n"), "{}", note);
        assert!(
            note.contains("\n- `computer`: computer use: the user's browser and Mac apps (skills: `computer:computer-use`)\n- `hn`"),
            "{}",
            note
        );
        assert!(note.ends_with("- `hn`"), "{}", note);
    }

    #[test]
    fn the_built_in_computer_plugin_says_it_drives_the_browser_and_apps() {
        // the repo's own plugins/ (the built-in root of a dev build)
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/computer/plugin.json");
        let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
        let d = m["description"].as_str().unwrap().to_lowercase();
        for w in ["computer use", "browser", "tab", "mac apps", "click", "type"] {
            assert!(d.contains(w), "{} not in {}", w, d);
        }
    }

    /// The repo's built-in plugins as a fresh install loads them: the
    /// office ones on by default, each skill named as `<available-skills>`
    /// and the skill tool name it (`slides:slides`; the note said
    /// `slides:slides:slides`, a name the skill tool refuses).
    #[test]
    fn the_built_in_office_plugins_are_noted_with_their_skill_names() {
        let builtin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let roots = bend_plugins::resolve::Roots { builtin: Some(builtin), ..bend_plugins::resolve::Roots::for_test(&std::env::temp_dir()) };
        let note = plugins_note(&loaded_plugins_in(&roots)).unwrap();
        for (p, kind) in [("slides", "pptx"), ("sheets", "xlsx"), ("docs", "docx")] {
            let line = note.lines().find(|l| l.starts_with(&format!("- `{}`", p))).unwrap_or_else(|| panic!("{}", note));
            assert!(line.contains(kind) && line.ends_with(&format!("(skills: `{p}:{p}`)")), "{}", line);
        }
        // computer is opt-in: off on a fresh install
        assert!(!note.contains("`computer`"), "{}", note);
    }

    #[test]
    fn git_missing_is_said_in_english_with_the_fix() {
        let d = tmp("nogit");
        let g = probe_git_in(d.to_str().unwrap(), None, &|| true);
        assert_eq!(g, Git::Missing);
        let p = g.problem().unwrap();
        assert!(p.starts_with("git is not installed"), "{}", p);
        assert!(p.contains("xcode-select --install"), "{}", p);
        assert!(tools_note(None, &g).contains("`git` is NOT installed"));
    }

    #[test]
    fn the_macos_stub_is_never_run_without_developer_tools() {
        let d = tmp("stub");
        let ran = d.join("ran");
        // the stub: it would pop the dialog (here: leave a trace)
        let shim = script(&d, "git", &format!("touch '{}'; echo 'xcrun: error: invalid active developer path' >&2; exit 1", ran.display()));
        let path = d.to_str().unwrap();
        let asked = std::cell::Cell::new(0);
        let g = probe_git_in(path, Some(&shim), &|| {
            asked.set(asked.get() + 1);
            false
        });
        assert_eq!(g, Git::Stub);
        assert_eq!(asked.get(), 1);
        assert!(!ran.exists(), "the stub ran");
        let p = g.problem().unwrap();
        assert!(p.contains("installer stub") && p.contains("xcode-select --install"), "{}", p);
        let note = tools_note(None, &g);
        assert!(note.contains("Never run `git`"), "{}", note);
        // with the developer tools, the same path is real git: run it
        let g = probe_git_in(path, Some(&shim), &|| true);
        assert!(ran.exists());
        assert_eq!(g, Git::Broken("xcrun: error: invalid active developer path".into()));
        assert!(g.problem().unwrap().starts_with("git does not work"));
    }

    #[test]
    fn a_real_git_answers_its_version() {
        let d = tmp("git");
        let bin = script(&d, "git", "[ \"$1\" = --version ] && echo 'git version 9.9.9'");
        // a git that is not the shim never asks for the developer dir
        let g = probe_git_in(d.to_str().unwrap(), Some(Path::new(MACOS_GIT_SHIM)), &|| panic!("asked"));
        assert_eq!(g, Git::Works { bin: bin.clone(), version: "git version 9.9.9".into() });
        assert_eq!(g.problem(), None);
        assert_eq!(g.describe(), format!("git version 9.9.9 ({})", bin.display()));
    }

    /// boot-speed: PATH's macOS shim becomes the git it runs (one exec a
    /// call); a git he put first, or xcrun failing, stays as it was.
    #[test]
    fn the_shim_is_resolved_to_its_git_and_only_the_shim() {
        let d = tmp("unshim");
        let real = script(&d, "git", "echo real");
        let shim = Path::new(MACOS_GIT_SHIM);
        let works = |bin: &Path| Git::Works { bin: bin.to_path_buf(), version: "git version 2.50.1".into() };
        let found = || Some(real.clone());
        assert_eq!(unshim(works(shim), Some(shim), &found), works(&real));
        // his own git first on PATH: never asked
        let brew = Path::new("/opt/homebrew/bin/git");
        assert_eq!(unshim(works(brew), Some(shim), &|| panic!("asked")), works(brew));
        // xcrun fails, or names a file that isn't there: the shim as before
        assert_eq!(unshim(works(shim), Some(shim), &|| None), works(shim));
        assert_eq!(unshim(works(shim), Some(shim), &|| Some(d.join("gone"))), works(shim));
        // no git, or not macOS: as it was
        assert_eq!(unshim(Git::Missing, Some(shim), &|| panic!("asked")), Git::Missing);
        assert_eq!(unshim(works(shim), None, &|| panic!("asked")), works(shim));
    }

    /// The hub's git takes no optional lock: a `status` poll never holds
    /// the `index.lock` a land's rebase needs (feature_retire_e2e).
    #[test]
    fn the_hubs_git_takes_no_optional_lock() {
        let Ok(c) = git_command() else { return };
        let envs: Vec<_> = c.get_envs().collect();
        assert!(envs.contains(&(std::ffi::OsStr::new("GIT_OPTIONAL_LOCKS"), Some(std::ffi::OsStr::new("0")))), "{envs:?}");
    }

    /// One owner for the hub's git: no `Command::new("git")` in the
    /// crate's sources but here (a test module or a tests file aside).
    #[test]
    fn the_hub_runs_git_only_here() {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let needle = ["Command::new(", "\"git\")"].concat();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            if name == "tools_env.rs" || name.ends_with("_tests.rs") || f.components().any(|c| c.as_os_str() == "tests") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            assert!(!code.contains(&needle), "{}: runs git without tools_env::git_command (GIT_OPTIONAL_LOCKS=0)", f.display());
        }
    }
}
