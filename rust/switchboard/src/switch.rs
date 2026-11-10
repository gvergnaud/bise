//! Version switch of a workspace's hub, with a probation period and an
//! automatic rollback (the `sbswitch` subcommand).
//!
//! A version is an app root built by `versions.sh` (bise,
//! repl-live, sb-core, VERSION). A switch never stops an agent: the old
//! hub exits with `keep_agents`, the new one adopts the running REPLs
//! (each moves to the new binary at its next idle, same session). The
//! switcher runs from the OLD (known good) binary, detached from both
//! hubs, and watches the new one for the probation period:
//! - the hub process died, or does not answer `ping` for ~10 s;
//! - the hub that answers runs another app root (its `hub.root`) for
//!   15 s: a client started an older bise meanwhile (update-safety); the
//!   new hub takes the workspace back from such a start ([`claim`]), and
//!   clients wait for a running switch instead of starting one
//!   (client::open);
//! - the hub reported a failure (`probation-fail`: a REPL of the new
//!   version crashed, or could not start).
//!
//! Either one rolls back to the version it came from (same mechanism,
//! backwards) and leaves a warning in main's thread and its outcome in
//! hub.log. A starting hub gets time while its boot moves (crate::boot:
//! its steps and replay lines, never its heartbeats); a switch kills a
//! hub that stops moving, a rollback leaves it booting (it stops itself
//! when stuck). A probation without
//! failure marks the version good, with no line in main's thread: its
//! start and end are the switch row's (`notice` kind `switch`) and
//! hub.log's (designer m_23091). `versions.json` in the state dir holds
//! `current`, `good` and `previous` (version dirs).

use crate::client;
use crate::paths::Paths;
use crate::util::now_ms;
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const PROBATION: Duration = Duration::from_secs(120);

pub fn state_file(paths: &Paths) -> PathBuf {
    paths.state.join("versions.json")
}

pub fn fail_file(paths: &Paths) -> PathBuf {
    paths.state.join("probation-fail")
}

pub fn read_state(paths: &Paths) -> Value {
    state_of(std::fs::read_to_string(state_file(paths)).ok().as_deref())
}

/// The state file's text as a JSON object: `st["current"] = ...` panics
/// on an array or a number, so a hand-edited, truncated or foreign file
/// reads as empty.
fn state_of(text: Option<&str>) -> Value {
    text.and_then(|s| serde_json::from_str(s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn write_state(paths: &Paths, v: &Value) {
    let tmp = paths.state.join("versions.json.tmp");
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, state_file(paths));
    }
}

/// True while a switch is on probation (a hub then reports its
/// failures in `probation-fail`).
pub fn on_probation(paths: &Paths) -> bool {
    read_state(paths)
        .get("probation_until")
        .and_then(|x| x.as_u64())
        .is_some_and(|t| t > now_ms())
}

/// A hub on probation reports a failure (the switcher rolls back).
pub fn report_failure(paths: &Paths, reason: &str) {
    if on_probation(paths) && !fail_file(paths).exists() {
        let _ = std::fs::write(fail_file(paths), reason);
    }
}

/// The version an app root is (its `VERSION` file), as `key=value`.
pub fn version_info(root: &Path) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("dir".into(), json!(root.to_string_lossy()));
    if let Ok(t) = std::fs::read_to_string(root.join("VERSION")) {
        for l in t.lines() {
            if let Some((k, v)) = l.split_once('=') {
                m.insert(k.to_string(), json!(v));
            }
        }
    }
    Value::Object(m)
}

fn log(paths: &Paths, s: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
    {
        let _ = writeln!(f, "{} switch: {}", now_ms(), s);
    }
}

fn hub_pid(paths: &Paths) -> Option<u32> {
    std::fs::read_to_string(paths.pid_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

use crate::procs::alive;

mod ask;

// who keeps the workspace when a starting hub finds another one there
// during a switch (update-safety: an older core's restart races the switch)
pub mod claim;

fn ping(paths: &Paths) -> bool {
    client::request(
        &paths.socket(),
        &json!({"op": "ping"}),
        Duration::from_secs(3),
    )
    .is_ok()
}

/// The switch's phase moved in versions.json: the hub sends its switch
/// row to every client, and writes no line in main's thread (a `notice`
/// of kind `switch`, daemon::dispatch).
fn phase_moved(paths: &Paths) {
    notice(paths, SWITCH_KIND, "");
}

/// The notice kind that only marks a switch phase (no thread line).
pub const SWITCH_KIND: &str = "switch";

/// A line in main's thread (and every client), through the hub.
pub fn notice(paths: &Paths, kind: &str, text: &str) {
    if let Ok(mut s) = UnixStream::connect(paths.socket()) {
        let v = json!({"op": "notice", "kind": kind, "text": text});
        let _ = s.write_all(format!("{}\n", v).as_bytes());
    }
}

/// The app root the running hub was started from (its executable's dir).
pub fn running_root(paths: &Paths) -> Option<PathBuf> {
    let pid = hub_pid(paths)?;
    // the hub writes its app root at start (`ps` may truncate the path)
    if alive(pid) {
        if let Ok(r) = std::fs::read_to_string(paths.state.join("hub.root")) {
            let r = PathBuf::from(r.trim());
            if r.join("repl-live").exists() {
                return Some(r);
            }
        }
    }
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    let exe = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let root = Path::new(&exe).parent()?.to_path_buf();
    if !root.is_absolute() {
        return None;
    }
    // the dev tree runs rust/target/debug/bise from the repo
    if root.join("repl-live").exists() {
        Some(root)
    } else {
        root.ancestors()
            .skip(1)
            .take(3)
            .find(|d| d.join("repl-live").exists())
            .map(|d| d.to_path_buf())
    }
}

/// The command's file in a version dir (BISE-165; was `bend-harness`).
pub const EXE: &str = "bise";

/// The executable of an app root: a version dir's `bise`, the dev tree's
/// debug build. `bend-harness` is not a link kept for a release: it is the
/// only executable of a version dir built from a commit before BISE-165
/// (v2026.9.29), which a switch back to that version still runs.
pub fn exe_of(root: &Path) -> Option<PathBuf> {
    [EXE, "bend-harness", "rust/target/debug/bise", "rust/target/debug/bend-harness"]
        .iter()
        .map(|f| root.join(f))
        .find(|p| p.exists())
}

/// `root/bise sbd`, detached (its own process group), like
/// `client::start_hub`; the child handle tells a hub that died at once.
/// `same`: the version running now (a restart, a reload): when `root`
/// has no binary (a dev tree built in a CARGO_TARGET_DIR), the running
/// hub's own, this switcher's; else the root's, as before (a dev tree's
/// fresh `rust/target/debug/bise`).
fn start_hub(paths: &Paths, root: &Path, same: bool) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    crate::client::jailed(paths)?;
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"))?;
    let mine = || std::env::current_exe().ok().filter(|_| same);
    let exe = exe_of(root).or_else(mine).unwrap_or_else(|| root.join(EXE));
    let mut cmd = Command::new(exe);
    // the new version's own files, not the old hub's inherited root; no
    // other internal variable of the old hub's (bise_home::env)
    bise_home::env::for_child(bise_home::env::Child::Hub, [("BISE_APP_ROOT", root)]).apply(&mut cmd);
    cmd.arg("sbd")
        .arg("--workspace")
        .arg(&paths.workspace)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .process_group(0)
        .spawn()
}

/// What `replace_hub` does with a new hub still booting at the end of
/// its wait.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Late {
    /// A switch: stop it (the rollback starts the known good one).
    Kill,
    /// A rollback: leave it booting. Killing it left no hub at all for
    /// 68 min (its journal replay was slow on a loaded machine), and it
    /// stops itself when its boot is stuck (crate::boot).
    Leave,
}

/// Stop the hub, agents kept; then start the hub of `root`. Err: why
/// the new hub is not up.
fn replace_hub(paths: &Paths, root: &Path, same: bool, late: Late) -> Result<(), String> {
    let old = hub_pid(paths);
    // Err (no answer, or still there after its wait): the old pid is
    // killed just below before a new hub starts, never two at once
    let _ = client::stop(paths, true);
    if let Some(p) = old {
        let t0 = Instant::now();
        while alive(p) && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if alive(p) {
            // not answering: it cannot stop cleanly; its REPLs survive it
            crate::procs::kill_now(p);
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    // a stale socket goes; one that answers is another bise a client
    // started meanwhile: left for the new hub to take back (switch::claim),
    // never unlinked under it (two hubs then ran, update-safety)
    if UnixStream::connect(paths.socket()).is_err() {
        let _ = std::fs::remove_file(paths.socket());
    }
    if !same {
        // an older version's hub would write its sb script through the link
        crate::tools_env::drop_agent_links(&paths.bin_dir());
    }
    // what the new hub writes: only its own lines name why it failed
    // (an older line of hub.err once passed for the cause)
    let (err_file, log_file) = (paths.state.join("hub.err"), paths.log());
    let (err0, log0) = (file_len(&err_file), file_len(&log_file));
    let mut child = start_hub(paths, root, same).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let (mut seen, mut moved) = (log0, Instant::now());
    let exited = loop {
        if ping(paths) {
            return Ok(());
        }
        if let Ok(Some(s)) = child.try_wait() {
            break Some(s);
        }
        // progress: a new hub.log line, but a heartbeat (crate::boot)
        let len = file_len(&log_file);
        if len != seen {
            if crate::boot::moved(&read_from(&log_file, seen)) {
                moved = Instant::now();
            }
            seen = len;
        }
        if !start_waits(t0.elapsed(), moved.elapsed()) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let err = last_line(&read_from(&err_file, err0), |_| true);
    let step = last_line(&read_from(&log_file, log0), |l| !l.contains(" switch: ") && !crate::boot::is_heartbeat(l));
    let why = start_failure(exited.as_ref().map(|s| s.to_string()), t0.elapsed(), &step, &err);
    if exited.is_none() && late == Late::Leave {
        return Err(format!("{}; it is left booting", why));
    }
    let _ = child.kill();
    Err(why)
}

/// Starting hub: wait while it boots. Up to START_MAX in all, and never
/// START_STILL without progress in hub.log: its boot steps and the
/// journal replay's `n/N events` lines, every 5 s (a 37.6k-event replay
/// took >25 s under a load of 40, and failed a 20 s wait for a new line).
const START_MAX: Duration = Duration::from_secs(600);
const START_STILL: Duration = Duration::from_secs(60);

fn start_waits(total: Duration, since_progress: Duration) -> bool {
    total < START_MAX && since_progress < START_STILL
}

/// Why the new hub is not up, from its own lines: its last boot step
/// (hub.log) and its last error line (hub.err).
fn start_failure(exited: Option<String>, waited: Duration, step: &str, err: &str) -> String {
    let mut s = match exited {
        Some(status) => format!("the hub did not start: it exited ({})", status),
        None => format!("the hub did not start: no answer after {} s", waited.as_secs()),
    };
    if !step.is_empty() {
        s.push_str(&format!("; last boot step: {}", crate::util::clip(step, 160)));
    }
    if !err.is_empty() {
        s.push_str(&format!("; hub.err: {}", crate::util::clip(err, 200)));
    }
    s
}

fn file_len(p: &Path) -> u64 {
    std::fs::metadata(p).map_or(0, |m| m.len())
}

/// The text of `p` after its first `from` bytes.
pub(crate) fn read_from(p: &Path, from: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let mut out = Vec::new();
    if let Ok(mut f) = std::fs::File::open(p) {
        if f.seek(SeekFrom::Start(from)).is_ok() {
            let _ = f.read_to_end(&mut out);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The last non-empty line of `text` that `keep` keeps, without a
/// hub.log time stamp.
pub(crate) fn last_line(text: &str, keep: impl Fn(&str) -> bool) -> String {
    let l = text.lines().rev().map(str::trim).find(|l| !l.is_empty() && keep(l)).unwrap_or("");
    match l.split_once(' ') {
        Some((ms, rest)) if !ms.is_empty() && ms.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
        _ => l.to_string(),
    }
}

/// The id of the version at `root` (from its `VERSION` file); None for
/// a dev tree.
pub fn version_id(root: &Path) -> Option<String> {
    version_info(root)
        .get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
}

/// The version id, else the path (for messages).
pub fn id_of(root: &Path) -> String {
    version_id(root).unwrap_or_else(|| root.to_string_lossy().to_string())
}

/// Watch the new hub for the probation period. Err: the reason to roll back.
fn probation(paths: &Paths, to: &Path, period: Duration, marks: Marks) -> Result<(), String> {
    // the hub reads the deadline (on_probation: it reports its failures
    // and does not idle-exit): from now, not from before the backup and
    // the start, else the hub thought it over and idle-exited while this
    // loop still watched it, and its exit read as a crash
    let mut st = read_state(paths);
    st["probation_until"] = json!(now_ms() + period.as_millis() as u64 + 5_000);
    write_state(paths, &st);
    let t0 = Instant::now();
    let (mut misses, mut elsewhere) = (0, 0);
    while t0.elapsed() < period {
        std::thread::sleep(Duration::from_secs(1));
        if let Ok(r) = std::fs::read_to_string(fail_file(paths)) {
            return Err(r);
        }
        match hub_pid(paths) {
            Some(p) if alive(p) => {}
            _ => return Err(crash_cause(paths, marks)),
        }
        // the hub that answers runs `to`, never another bise that took the
        // workspace meanwhile (an older client's start: a pass then left
        // him on the old version, update-safety); a few seconds of grace
        // while a new hub takes it back (switch::claim)
        if runs_elsewhere(paths, to) {
            elsewhere += 1;
            if elsewhere >= TAKEOVER_GRACE {
                let root = std::fs::read_to_string(paths.state.join("hub.root")).unwrap_or_default();
                return Err(format!("another bise took the hub: {}", root.trim()));
            }
        } else {
            elsewhere = 0;
        }
        if ping(paths) {
            misses = 0;
        } else {
            misses += 1;
            if misses >= 4 {
                return Err("the hub no longer answers".into());
            }
        }
    }
    Ok(())
}

/// Where hub.err and hub.log ended when the switch started the new hub:
/// only the lines after them are the new hub's (an older line once
/// passed for the cause).
#[derive(Clone, Copy)]
struct Marks {
    err: u64,
    log: u64,
}

impl Marks {
    fn now(paths: &Paths) -> Marks {
        Marks { err: file_len(&paths.state.join("hub.err")), log: file_len(&paths.log()) }
    }
}

/// Why the new hub stopped during its probation, from its own lines
/// (update-safety, B: Gauthier's card said only 'the hub stopped (crash)'):
/// its last hub.err line (a panic says where), else its last hub.log
/// step; the reason the update card shows ('couldn't update to …').
fn crash_cause(paths: &Paths, marks: Marks) -> String {
    let err = last_line(&read_from(&paths.state.join("hub.err"), marks.err), |_| true);
    let step = last_line(&read_from(&paths.log(), marks.log), |l| !l.contains(" switch: ") && !crate::boot::is_heartbeat(l));
    cause_of(&err, &step)
}

/// [`crash_cause`]'s words from the new hub's last error line and last
/// log step (either may be empty). Pure.
fn cause_of(err: &str, step: &str) -> String {
    // it goes in versions.json and on the card: no control character (a
    // colored panic's ANSI escapes, a stray \r), architect m_59049
    let plain = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string();
    let (err, step) = (plain(err), plain(step));
    match (err.is_empty(), step.is_empty()) {
        (false, _) => format!("the hub stopped (crash): {}", crate::util::clip(&err, 200)),
        (true, false) => format!("the hub stopped (crash) after: {}", crate::util::clip(&step, 160)),
        (true, true) => "the hub stopped (crash)".into(),
    }
}

/// Probation seconds in a row the hub may run another root than the
/// switch's before it fails: the time a new hub takes the workspace back
/// from an older client's start (switch::claim: its stop is bounded at 5 s,
/// then a kill).
const TAKEOVER_GRACE: u32 = 15;

/// The live hub of `paths` says it runs another app root than `to`
/// (its `hub.root`); unreadable: no.
fn runs_elsewhere(paths: &Paths, to: &Path) -> bool {
    let Ok(r) = std::fs::read_to_string(paths.state.join("hub.root")) else { return false };
    let (r, to) = (Path::new(r.trim()), to.canonicalize().unwrap_or_else(|_| to.to_path_buf()));
    r.canonicalize().is_ok_and(|r| r != to)
}

/// The hub whose state is `state` runs (a live socket or pid) or is being
/// switched: the migration to ~/.bise leaves it in place (BISE-161).
pub fn hub_busy(state: &Path) -> bool {
    let paths = Paths { workspace: PathBuf::new(), state: state.to_path_buf(), worktrees: PathBuf::new(), run: PathBuf::new() };
    UnixStream::connect(paths.socket()).is_ok() || hub_pid(&paths).is_some_and(alive) || switch_running(&paths)
}

/// A switcher is running (a switch on probation).
pub fn switch_running(paths: &Paths) -> bool {
    std::fs::read_to_string(paths.state.join("switch.pid"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .is_some_and(alive)
}

/// Roll back the switch on probation now (the user asked).
pub fn abort_probation(paths: &Paths) {
    let _ = std::fs::write(fail_file(paths), "rollback requested");
}

/// `sbswitch --to <version dir> [--probation <s>]`: the whole switch.
// the state's backup before a switch or a restart (/tmp/sb-backup-*)
mod backup;

/// The workspace is bise's own source tree (dev mode, BISE-131): there
/// `/restart` builds the latest commit, then switches to it; anywhere
/// else it reloads the running version (nothing built).
pub fn dev_workspace(ws: &Path) -> bool {
    versions_script(ws).is_some() && ws.join("rust/switchboard/Cargo.toml").is_file()
}

/// The script that builds a version of the source tree `repo`:
/// scripts/versions.sh, or versions.sh at the top of a tree from before
/// the root cleanup.
pub fn versions_script(repo: &Path) -> Option<PathBuf> {
    ["scripts/versions.sh", "versions.sh"].iter().map(|p| repo.join(p)).find(|p| p.is_file())
}

// the newest switch's facts (versions.json `switch`) and its row
// (`hub/switch`, architect m_20576)
mod row;
pub use row::row;
use row::set_phase;

/// Left by a reload's switcher for the hub it starts (BISE-131): that hub
/// reloads every REPL it adopts and tells the TUIs to re-exec. It holds
/// the reload's id (ms), the hub takes it at boot.
pub fn reload_file(paths: &Paths) -> PathBuf {
    paths.state.join("reload")
}

/// At boot: the id of the reload that started this hub, if one did.
pub fn take_reload(paths: &Paths) -> Option<String> {
    let f = reload_file(paths);
    let id = std::fs::read_to_string(&f).ok()?.trim().to_string();
    let _ = std::fs::remove_file(&f);
    (!id.is_empty()).then_some(id)
}

/// `restart`: `to` may be the running version (a plain restart of the
/// hub, e.g. a stuck one); the agents are kept the same way. `reload`
/// (BISE-131, implies `restart`): the new hub also relaunches every
/// agent's REPL (at its next idle, same session) and the TUIs.
/// `release` (update-flow): a switch to the machine's `update now`
/// release: it holds the machine's switch lock (bise_home::release_offer)
/// through the switch and its probation, so one hub switches at a time;
/// another holder: nothing done (the hub asks again at its next look).
pub fn run(paths: &Paths, to: &Path, period: Duration, restart: bool, reload: bool, release: bool) -> i32 {
    let _machine = match release {
        true => match bise_home::release_offer::try_lock(&bise_home::Home::from_env().switch_lock()) {
            Some(l) => Some(l),
            None => {
                log(paths, "another project is switching to the new release: this one waits its turn");
                return 1;
            }
        },
        false => None,
    };
    let lock = paths.state.join("switch.pid");
    if let Some(p) = std::fs::read_to_string(&lock)
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        if alive(p) && p != std::process::id() {
            notice(paths, "warn", "a version switch is already in progress");
            return 1;
        }
    }
    let _ = std::fs::write(&lock, std::process::id().to_string());
    let code = run_locked(paths, to, period, restart || reload, reload);
    let _ = std::fs::remove_file(&lock);
    code
}

fn run_locked(paths: &Paths, to: &Path, period: Duration, restart: bool, reload: bool) -> i32 {
    let to = to.canonicalize().unwrap_or_else(|_| to.to_path_buf());
    if !to.join("repl-live").exists() {
        notice(
            paths,
            "warn",
            &format!("version not found: {}", to.display()),
        );
        return 1;
    }
    let mut st = read_state(paths);
    let from = running_root(paths).or_else(|| {
        st.get("current")
            .and_then(|x| x.as_str())
            .map(PathBuf::from)
    });
    let Some(from) = from.map(|p| p.canonicalize().unwrap_or(p)) else {
        notice(
            paths,
            "warn",
            "current version unknown: no rollback possible, switch cancelled",
        );
        return 1;
    };
    if from == to && !restart {
        notice(
            paths,
            "info",
            &format!("already on version {}", id_of(&to)),
        );
        return 0;
    }
    let (from_id, to_id) = (id_of(&from), id_of(&to));
    log(paths, &format!("{} -> {}", from.display(), to.display()));
    let saved = match backup::backup(&paths.state, now_ms()) {
        Ok((d, how)) => {
            log(paths, &format!("state backup ({}): {}", how.word(), d.display()));
            format!(" · state backed up: {}", d.display())
        }
        Err(e) => {
            log(paths, &format!("state backup failed: {}", e));
            format!(" · the state backup failed ({}): nothing is backed up", e)
        }
    };
    let what = if reload {
        format!("reloading bise on version {}: the hub, every agent and the TUI restart, nothing lost", to_id)
    } else if from == to {
        format!("restarting the hub on version {}", to_id)
    } else if restart {
        format!("restarting the hub on version {} (from {})", to_id, from_id)
    } else {
        format!("switching to version {} (from {})", to_id, from_id)
    };
    // in hub.log, not main's thread: the switch row says it while it runs,
    // and a line in the thread would stay after the hub is back (designer
    // m_22843: a restart that went well leaves no line)
    log(paths, &format!("{}{}{}", what, if reload { "" } else { " · the agents keep running" }, saved));
    let _ = std::fs::remove_file(fail_file(paths));
    if from != to {
        st["previous"] = json!(from.to_string_lossy());
    }
    st["current"] = json!(to.to_string_lossy());
    if st.get("good").is_none() {
        st["good"] = json!(from.to_string_lossy());
    }
    st["probation_until"] = json!(now_ms() + period.as_millis() as u64);
    set_phase(&mut st, "reloading", &to_id, &from_id, None);
    write_state(paths, &st);
    if reload {
        let _ = std::fs::write(reload_file(paths), now_ms().to_string());
    }
    // the phase is written: the hub sends its switch row ('reloading')
    phase_moved(paths);

    let marks = Marks::now(paths);
    let outcome = replace_hub(paths, &to, from == to, Late::Kill).and_then(|_| probation(paths, &to, period, marks));
    let mut st = read_state(paths);
    st["probation_until"] = json!(0);
    match outcome {
        Ok(()) => {
            st["good"] = json!(to.to_string_lossy());
            set_phase(&mut st, "passed", &to_id, &from_id, None);
            write_state(paths, &st);
            log(paths, &format!("{} good", to_id));
            // no thread line either: the switch row says '✓ reloaded'
            phase_moved(paths);
            0
        }
        Err(reason) => {
            log(
                paths,
                &format!("{} failed: {} — rollback to {}", to_id, reason, from_id),
            );
            // a restart on the same version goes back to the last good
            // one when there is another, else it tries that version again
            let from = if from == to {
                st.get("good")
                    .and_then(|x| x.as_str())
                    .map(PathBuf::from)
                    .filter(|g| g.canonicalize().map(|c| c != to).unwrap_or(false))
                    .unwrap_or_else(|| from.clone())
            } else {
                from
            };
            let from_id = id_of(&from);
            st["current"] = json!(from.to_string_lossy());
            if from != to {
                st["previous"] = json!(to.to_string_lossy());
            }
            st["failed"] =
                json!({"version": to.to_string_lossy(), "reason": reason, "at": now_ms()});
            set_phase(&mut st, "rolled_back", &to_id, &from_id, Some(&reason));
            write_state(paths, &st);
            let _ = std::fs::remove_file(fail_file(paths));
            let back = replace_hub(paths, &from, from == to, Late::Leave);
            // in hub.log too: the notice reaches no hub while none is up
            log(paths, &match &back {
                Ok(()) => format!("rollback to {}: up", from_id),
                Err(e) => format!("rollback to {} failed: {}", from_id, e),
            });
            let text = format!(
                "version {} failed ({}) — rollback to version {}{}",
                to_id,
                reason,
                from_id,
                match &back {
                    Ok(()) => String::new(),
                    Err(e) => format!(": ROLLBACK FAILED, {}", e),
                }
            );
            notice(paths, "warn", &text);
            // the hub may still be booting: the notice is also left for it
            let _ = std::fs::write(paths.state.join("switch-notice"), &text);
            2
        }
    }
}

/// A `bise` launch (BISE-255): the hub of the workspace runs `hub`, the
/// launched bise is `me`. True when the hub should move to `me`: `me` is
/// the installed `current` (what `bise update` or the daily check
/// installed), `hub` an older build of the same install, or both are Nix
/// store paths and `me` is a later build (`nix profile upgrade`), and a
/// switch to `me` did not fail in this workspace (its probation rolled back: the
/// hub stays where it is, `/restart` tries again).
pub fn should_follow_install(me: &Path, hub: &Path, failed: Option<&Path>) -> bool {
    use bise_home::release::{read_version, root_kind, RootKind};
    let canon = |p: &Path| p.canonicalize().ok();
    let (Some(me), Some(hub)) = (canon(me), canon(hub)) else { return false };
    if me == hub || failed.and_then(canon).as_ref() == Some(&me) {
        return false;
    }
    match (root_kind(&me), root_kind(&hub)) {
        // install.sh: the launched bise is its prefix's current
        (RootKind::Install(mine), RootKind::Install(theirs)) => {
            if mine.prefix != theirs.prefix || mine.current().as_ref() != Some(&me) {
                return false;
            }
        }
        // Nix: `nix profile upgrade` made a new store path; the launched
        // one is what the profile runs now
        (RootKind::Nix, RootKind::Nix) => {}
        _ => return false,
    }
    let built = |r: &Path| read_version(r).get("built").cloned().unwrap_or_default();
    bise_home::release::built_after(&built(&me), &built(&hub))
}

/// Before a `bise` TUI attaches to the hub of its workspace (BISE-255):
/// a hub on an older installed version moves to the launched one (the
/// hub's own `/version <dir>`: the switcher, probation, agents kept, an
/// agent in a turn moves at its next idle), and this waits for the new
/// hub (<= 30 s) so the TUI attaches to it instead of following the old
/// one. Every hub since BISE-131 takes the request. `say`: one line for
/// the user each (the terminal is not the TUI's yet).
pub fn follow_install(paths: &Paths, me: &Path, say: &dyn Fn(&str)) {
    if switch_running(paths) {
        return;
    }
    let Some(hub) = running_root(paths) else { return };
    let failed = read_state(paths)
        .pointer("/failed/version")
        .and_then(|x| x.as_str())
        .map(PathBuf::from);
    if !should_follow_install(me, &hub, failed.as_deref()) {
        return;
    }
    let me = me.canonicalize().unwrap_or_else(|_| me.to_path_buf());
    let (from_id, to_id) = (id_of(&hub), id_of(&me));
    say(&format!("this folder's hub runs bise {}: moving it to {} (the agents keep running)…", from_id, to_id));
    match move_hub(paths, &me, MOVE_BOUND) {
        Moved::There => say(&format!("the hub runs bise {} now", to_id)),
        Moved::Refused(answer) => say(&format!("the hub did not switch ({}): /restart in it switches to {}", clip_answer(&answer), to_id)),
        Moved::Late => say(&format!("the hub is still switching to {} (probation): the TUI follows it once it is up", to_id)),
    }
}

/// How long a client waits for a moved hub to listen again (the
/// switcher's start, then the new hub's).
pub const MOVE_BOUND: Duration = Duration::from_secs(30);

/// What [`move_hub`] got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Moved {
    /// the hub listens again, as `me`'s version
    There,
    /// the hub didn't take the switch: its words (a refusal, `""` when it
    /// said nothing in time)
    Refused(String),
    /// it took the switch but isn't up as `me` within the bound
    Late,
}

/// Move the folder's running hub to the app root `me` (the hub's own
/// `/version <me>`: the switcher, probation, agents kept), then wait up
/// to `bound` for it to listen again as `me`. The one move a client
/// makes of its hub: `bise` at launch (`follow_install`, BISE-255) and
/// the desktop core when its home hub is older than it (an older hub's
/// `initialize` refusal, architect m_15476). A hub that serves JSON-RPC
/// gets `initialize` then `version/switch`, an older one the older hello
/// and `version` op (switch/ask.rs).
pub fn move_hub(paths: &Paths, me: &Path, bound: Duration) -> Moved {
    let me = me.canonicalize().unwrap_or_else(|_| me.to_path_buf());
    let mut answer = ask::ask_switch(&paths.socket(), &me.to_string_lossy()).unwrap_or_default();
    // an older hub: his install's steps (architect m_24004)
    if answer == bise_proto::rpc::HUB_OLDER {
        answer = crate::client::older_words(&paths.state);
    }
    if !answer.starts_with("switching to version") {
        return Moved::Refused(answer);
    }
    let t0 = Instant::now();
    while t0.elapsed() < bound {
        std::thread::sleep(Duration::from_millis(200));
        let there = running_root(paths).and_then(|r| r.canonicalize().ok()).as_ref() == Some(&me);
        if there && ping(paths) {
            return Moved::There;
        }
    }
    Moved::Late
}

fn clip_answer(a: &str) -> String {
    if a.trim().is_empty() {
        "no answer".into()
    } else {
        crate::util::clip(a.trim(), 160)
    }
}

#[cfg(test)]
mod state_tests {
    /// The probation fails a switch whose answering hub runs another root
    /// (update-safety: an older client's start passed for the new version).
    #[test]
    fn the_probation_sees_a_hub_of_another_root() {
        use super::runs_elsewhere;
        let d = std::env::temp_dir().join(format!("sb-elsewhere-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (old, new) = (d.join("v/old"), d.join("v/new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        let paths = crate::paths::Paths { workspace: d.clone(), state: d.clone(), worktrees: d.join("w"), run: d.join("r") };
        assert!(!runs_elsewhere(&paths, &new), "no hub.root: nothing known, no failure");
        std::fs::write(d.join("hub.root"), new.to_string_lossy().as_bytes()).unwrap();
        assert!(!runs_elsewhere(&paths, &new), "the switch's own root");
        std::fs::write(d.join("hub.root"), old.to_string_lossy().as_bytes()).unwrap();
        assert!(runs_elsewhere(&paths, &new), "another root answers");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A crash during probation says the new hub's own last words (B of
    /// page update-safety): the card shows them, never a bare 'crash'.
    #[test]
    fn a_crash_in_probation_says_its_cause() {
        use super::cause_of;
        assert_eq!(cause_of("thread 'main' panicked at daemon.rs:12: index out of bounds", "boot: journal read"), "the hub stopped (crash): thread 'main' panicked at daemon.rs:12: index out of bounds");
        assert_eq!(cause_of("", "boot: journal read (21 events)"), "the hub stopped (crash) after: boot: journal read (21 events)");
        assert_eq!(cause_of("", ""), "the hub stopped (crash)");
        assert_eq!(cause_of("\u{1b}[31mpanicked\u{1b}[0m at x\r", ""), "the hub stopped (crash): [31mpanicked[0m at x", "no control character reaches the card");
        assert_eq!(cause_of("\u{7}\r", "boot done"), "the hub stopped (crash) after: boot done", "a line of control characters only is no line");
    }

    #[test]
    fn a_launch_moves_the_hub_to_a_newer_installed_current_only() {
        use super::should_follow_install as follow;
        use std::path::Path;
        let t = std::env::temp_dir().join(format!("sb-follow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let mk = |prefix: &Path, id: &str, extra: &str| {
            let d = prefix.join("versions").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("VERSION"), format!("id={}\n{}", id, extra)).unwrap();
            d
        };
        let p = t.join("prefix");
        let old = mk(&p, "old", "built=2026-09-29T14:00:00Z\n");
        let new = mk(&p, "new", "built=2026-09-30T01:00:00Z\n");
        std::os::unix::fs::symlink("versions/new", p.join("current")).unwrap();
        assert!(follow(&new, &old, None), "the installed current, newer: the hub moves");
        assert!(!follow(&new, &new, None), "already there");
        assert!(!follow(&old, &new, None), "an older bise launched never moves the hub back");
        assert!(!follow(&new, &old, Some(&new)), "a switch to it failed here: stay");
        assert!(follow(&new, &old, Some(&old)), "another version failed: no matter");
        // not the installed current (a hand-run versions/<id>/bise): no move
        let newer = mk(&p, "newer", "built=2026-10-01T00:00:00Z\n");
        assert!(!follow(&newer, &old, None));
        // a dev hub (repl=) or another install: never touched
        let dev = mk(&p, "dev", "built=2026-01-01T00:00:00Z\nrepo=/src\n");
        assert!(!follow(&new, &dev, None));
        let q = t.join("other");
        let elsewhere = mk(&q, "x", "built=2026-01-01T00:00:00Z\n");
        std::os::unix::fs::symlink("versions/x", q.join("current")).unwrap();
        assert!(!follow(&new, &elsewhere, None));
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn dev_mode_is_bise_source_tree() {
        let d = std::env::temp_dir().join(format!("sb-devws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("rust/switchboard")).unwrap();
        assert!(!super::dev_workspace(&d), "any workspace: a reload");
        std::fs::create_dir_all(d.join("scripts")).unwrap();
        std::fs::write(d.join("scripts/versions.sh"), "").unwrap();
        assert!(!super::dev_workspace(&d));
        std::fs::write(d.join("rust/switchboard/Cargo.toml"), "").unwrap();
        assert!(super::dev_workspace(&d), "bise's sources: build then switch");
        // a tree from before the root cleanup: versions.sh at the top
        std::fs::remove_dir_all(d.join("scripts")).unwrap();
        assert!(!super::dev_workspace(&d));
        std::fs::write(d.join("versions.sh"), "").unwrap();
        assert!(super::dev_workspace(&d), "old layout");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_reload_file_is_taken_once() {
        let d = std::env::temp_dir().join(format!("sb-reload-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let paths = crate::paths::Paths { workspace: d.clone(), state: d.clone(), worktrees: d.join("worktrees"), run: d.join("run") };
        assert_eq!(super::take_reload(&paths), None);
        std::fs::write(super::reload_file(&paths), "123\n").unwrap();
        assert_eq!(super::take_reload(&paths).as_deref(), Some("123"));
        assert_eq!(super::take_reload(&paths), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_non_object_state_file_reads_as_empty() {
        for t in [None, Some(""), Some("[1]"), Some("3"), Some("null"), Some("{\"good\": \"x\"")] {
            let mut st = super::state_of(t);
            st["current"] = serde_json::json!("v"); // never panics
            assert_eq!(st["current"], "v");
        }
        assert_eq!(super::state_of(Some("{\"good\": \"x\"}"))["good"], "x");
    }

    #[test]
    fn a_starting_hub_gets_time_while_its_boot_steps_move() {
        use super::start_waits;
        use std::time::Duration;
        let s = Duration::from_secs;
        // the rolled-back hub of 1791355103: 28 s, no line for 22 s in its replay
        assert!(start_waits(s(28), s(22)), "a replay on a loaded machine is not a failed start");
        assert!(start_waits(s(300), s(3)), "a slow boot that moves");
        assert!(!start_waits(s(65), s(60)), "no progress for 60 s");
        assert!(!start_waits(s(600), s(1)), "600 s at most");
    }

    #[test]
    fn a_start_failure_names_the_new_hubs_last_step_and_error_only() {
        use super::{last_line, start_failure};
        use std::time::Duration;
        let log = "1790975537485 hub start pid=1 workspace=/w\n1790975538124 boot: journal read (21056 events)\n1790975540000 switch: x -> y\n1790975568124 boot: still in journal read (21056 events): no progress for 30 s\n";
        let step = last_line(log, |l| !l.contains(" switch: ") && !crate::boot::is_heartbeat(l));
        assert_eq!(step, "boot: journal read (21056 events)");
        assert_eq!(
            start_failure(None, Duration::from_secs(41), &step, ""),
            "the hub did not start: no answer after 41 s; last boot step: boot: journal read (21056 events)"
        );
        assert_eq!(
            start_failure(Some("exit status: 1".into()), Duration::from_secs(1), "", "Error: Address in use"),
            "the hub did not start: it exited (exit status: 1); hub.err: Error: Address in use"
        );
        assert_eq!(last_line("\n  \n", |_| true), "");
    }
}
