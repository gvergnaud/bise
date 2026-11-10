//! How a client reaches the hub of a workspace: connect to `hub.sock`,
//! starting the hub first when none runs. The hub's own side of a start
//! too ([`run_from`]: its `hub.root`, and the lease on its app root).

use crate::paths::Paths;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Start `exe sbd --workspace <ws>` detached from this terminal (its own
/// process group: closing the terminal does not stop the agents).
pub fn start_hub(paths: &Paths, exe: &Path, app_root: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    std::fs::create_dir_all(&paths.state)?;
    // a whole test run's jail (BISE_TEST_HOME): never a hub outside it
    jailed(paths)?;
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"))?;
    let mut cmd = Command::new(exe);
    // the TUI's environment minus every internal variable (bise_home::env),
    // with the TUI's app root: the hub never uses its own lookup's
    bise_home::env::for_child(bise_home::env::Child::Hub, [("BISE_APP_ROOT", app_root)]).apply(&mut cmd);
    let child = cmd
        .arg("sbd")
        .arg("--workspace")
        .arg(&paths.workspace)
        .current_dir(app_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .process_group(0)
        .spawn()
        // a bare 'No such file or directory' named nothing (his pruned runtime)
        .map_err(|e| std::io::Error::other(format!("can't run {}: {}", exe.display(), e)))?;
    reap(child);
    Ok(())
}

/// A hub's start, its own side: `hub.root` says where it runs from (the
/// switcher reads it: `switch::running_root`), and that root is leased
/// while the hub runs (`release::lease`: `bise update` never prunes it).
pub fn run_from(paths: &Paths, app_root: &Path) -> Option<bise_home::release::Lease> {
    let _ = std::fs::write(paths.state.join("hub.root"), app_root.to_string_lossy().as_bytes());
    bise_home::release::lease(app_root)
}

/// A hub this process started is its child until it exits: waited for
/// on its own thread, so one that exits (a second start that found the
/// hub already up, a hub killed or crashed) never stays a zombie under a
/// long-lived client, the desktop core (ambient-lead m_21848). If this
/// process ends first, the hub is reparented and launchd reaps it.
fn reap(mut child: std::process::Child) {
    let _ = std::thread::Builder::new().name("hub-reap".into()).spawn(move || {
        let _ = child.wait();
    });
}

/// One hub start at a time per workspace, across threads and processes
/// (ambient-lead m_21848): the desktop core's home connection and its
/// project connection to the same folder both found no socket at boot
/// and both started a hub; the second one said 'a hub is already running'
/// or 'Address already in use'. Held from the check to the socket being
/// up, so whoever waited connects to the hub the first one started.
/// An exclusive flock on `<state>/hub.start.lock`, released on drop (or
/// by the kernel when its holder dies). Its holder keeps it at most
/// open()'s own socket wait ([`START_WAIT`]); a waiter gives up after
/// [`LOCK_WAIT`] (LOCK_NB, polled), so a stuck start never blocks fn talk
/// or the terminal for longer (architect m_21925).
fn start_lock(paths: &Paths) -> std::io::Result<std::fs::File> {
    start_lock_within(paths, LOCK_WAIT)
}

fn start_lock_within(paths: &Paths, wait: Duration) -> std::io::Result<std::fs::File> {
    use std::os::fd::AsRawFd;
    std::fs::create_dir_all(&paths.state)?;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(paths.state.join("hub.start.lock"))?;
    let t0 = Instant::now();
    loop {
        // SAFETY: flock on a descriptor this function owns; never blocks
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(f);
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::WouldBlock {
            return Err(e);
        }
        if t0.elapsed() > wait {
            return Err(std::io::Error::other(format!("another start of this hub took over {} s", wait.as_secs())));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// How long open() waits for the hub it started to take connections.
const START_WAIT: Duration = Duration::from_secs(15);
/// How long open() waits for another client's start: that start's own
/// wait and a little more.
const LOCK_WAIT: Duration = Duration::from_secs(17);

/// Under BISE_TEST_HOME (bise_home::test_home::jail): the hub's workspace
/// and state dir must be inside it, else an error and no hub. Used by every
/// path that starts a hub (here, switch.rs, xhub through here) and by sbd.
pub fn jailed(paths: &Paths) -> std::io::Result<()> {
    for p in [&paths.workspace, &paths.state] {
        bise_home::test_home::jail(p).map_err(std::io::Error::other)?;
    }
    Ok(())
}

/// The one line a user reads when the hub did not come up: its cause (the
/// last line the hub wrote to hub.err, without Rust's `Error: Custom {..}`
/// wrapping) and where its log is.
fn not_started(cause: &str, err_file: &Path) -> String {
    let cause = cause.strip_prefix("Error: ").unwrap_or(cause);
    // `Custom { kind: Other, error: "..." }` / `Os { code: .., message: "..." }`
    let cause = match (cause.find("error: \"").or_else(|| cause.find("message: \"")), cause.rfind('"')) {
        (Some(i), Some(j)) if cause.starts_with(|c: char| c.is_ascii_uppercase()) && cause.contains(" { ") => {
            let start = cause[i..].find('"').map(|k| i + k + 1).unwrap_or(i);
            if start < j { &cause[start..j] } else { cause }
        }
        _ => cause,
    };
    if cause.is_empty() {
        format!("the hub did not start in 15 s (its log: {})", err_file.display())
    } else {
        format!("the hub did not start: {} (its log: {})", crate::util::clip(cause, 200), err_file.display())
    }
}

/// A client connection to the hub (started when none runs): the caller
/// says its first line, JSON-RPC's `initialize` (the terminal, the
/// desktop core).
pub fn open(paths: &Paths, exe: &Path, app_root: &Path) -> std::io::Result<UnixStream> {
    let s = match UnixStream::connect(paths.socket()) {
        Ok(s) => s,
        Err(_) => {
            jailed(paths)?;
            // a version switch is replacing the hub: its new hub comes up,
            // never one of this client's version beside it (update-safety:
            // an older core's start raced the switch and kept the old bise)
            if let Some(s) = wait_switch(paths, crate::switch::MOVE_BOUND) {
                return Ok(s);
            }
            let _start = start_lock(paths)?;
            // another client started it while this one waited for the lock
            if let Ok(s) = UnixStream::connect(paths.socket()) {
                return Ok(s);
            }
            let err_file = paths.state.join("hub.err");
            let err0 = std::fs::metadata(&err_file).map_or(0, |m| m.len());
            start_hub(paths, exe, app_root)?;
            let t0 = Instant::now();
            loop {
                if let Ok(s) = UnixStream::connect(paths.socket()) {
                    break s;
                }
                if t0.elapsed() > START_WAIT {
                    let cause = crate::switch::last_line(&crate::switch::read_from(&err_file, err0), |_| true);
                    return Err(std::io::Error::other(not_started(&cause, &err_file)));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    Ok(s)
}

/// While a switcher runs for `paths` (up to `bound`): its hub's connection
/// once it listens. None: no switch runs, or its hub is not up within the
/// bound (the caller starts one as before: a stale switch.pid never keeps
/// him out for long).
fn wait_switch(paths: &Paths, bound: Duration) -> Option<UnixStream> {
    let t0 = Instant::now();
    while crate::switch::switch_running(paths) && t0.elapsed() < bound {
        if let Ok(s) = UnixStream::connect(paths.socket()) {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    UnixStream::connect(paths.socket()).ok()
}

/// Stop the hub of a workspace and every agent REPL. `keep_agents`:
/// the REPLs keep running (their turns too) for the next hub to adopt -
/// a version switch, a hub restart. `initialize`, then `hub/stop`; a hub
/// from before client-protocol is not stopped: an error saying so
/// ([`older_words`]: his install's steps, else `rpc::HUB_OLDER`).
pub fn stop(paths: &Paths, keep_agents: bool) -> std::io::Result<bool> {
    stop_at(&paths.socket(), keep_agents, STOP_ANSWER, STOP_WAIT).map_err(|e| match e.to_string() == bise_proto::rpc::HUB_OLDER {
        true => std::io::Error::other(older_words(&paths.state)),
        false => e,
    })
}

/// What a client says to the hub of the state dir `state` when it is from
/// before client-protocol (architect m_24004): his install's steps
/// (`rpc::hub_older_steps`) when the hub runs from one (its `hub.root`,
/// daemon.rs), else `rpc::HUB_OLDER`. The one reading for the terminal
/// (`stop`, `switch::move_hub`) and the desktop core.
pub fn older_words(state: &Path) -> String {
    let home = bise_home::Home::from_env();
    older_words_in(state, home.user_home())
}

fn older_words_in(state: &Path, user_home: &Path) -> String {
    std::fs::read_to_string(state.join("hub.root"))
        .ok()
        .and_then(|r| bise_home::release::older_hub(Path::new(r.trim())))
        .map(|o| bise_proto::rpc::hub_older_steps(o.current, &bise_home::release::tilde(&o.exe, user_home)))
        .unwrap_or_else(|| bise_proto::rpc::HUB_OLDER.to_string())
}

/// `answer`: the wait for the hub's `initialize` answer; `wait`: for its
/// socket to go once the stop is asked. Not gone: an error, never "stopped".
fn stop_at(socket: &Path, keep_agents: bool, answer: Duration, wait: Duration) -> std::io::Result<bool> {
    let mut s = match ask_stop_at(socket, keep_agents, answer)? {
        Asked::NoHub => return Ok(false),
        Asked::Going => return Ok(true),
        Asked::Stop(s) => s,
    };
    // wait for the socket to go away, reading what the hub writes
    // (notifications): a full socket buffer would block the hub, which
    // would never stop
    let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
    let mut sink = [0u8; 65536];
    let t0 = Instant::now();
    while socket.exists() && t0.elapsed() < wait {
        if let Ok(0) = std::io::Read::read(&mut s, &mut sink) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    // never "stopped" while it still runs: a stop that lands later kills
    // the work of whoever attached meanwhile (2026-10-11, 85 s late)
    if socket.exists() {
        return Err(std::io::Error::other(format!(
            "the hub got the stop but still runs after {} s; `bise switchboard --stop` again to check",
            wait.as_secs()
        )));
    }
    Ok(true)
}

/// Ask the hub of `paths` to stop (`keep_agents`) and return at once: the
/// caller waits on its pid, not its socket (daemon.rs take_over: a hub
/// whose hub.pid is no longer its own leaves its socket in place).
pub fn ask_stop(paths: &Paths, keep_agents: bool) -> std::io::Result<()> {
    ask_stop_at(&paths.socket(), keep_agents, STOP_ANSWER).map(|_| ())
}

/// What [`ask_stop_at`] got.
enum Asked {
    /// no hub listens there
    NoHub,
    /// nothing asked: it is going already, or it refused (it runs in an
    /// agent's process)
    Going,
    /// the stop is asked: the connection, to read until it closes
    Stop(UnixStream),
}

/// `initialize`, then `hub/stop`.
fn ask_stop_at(socket: &Path, keep_agents: bool, answer: Duration) -> std::io::Result<Asked> {
    use bise_proto::hub::HubCmd;
    use bise_proto::rpc::{self, Id, Init, InitializeParams, Message, Request};
    let Ok(mut s) = UnixStream::connect(socket) else { return Ok(Asked::NoHub) };
    let params = serde_json::to_value(InitializeParams::new("bise", env!("CARGO_PKG_VERSION"))).unwrap_or_default();
    writeln!(s, "{}", Message::Request(Request::new(Id::Num(0), rpc::INITIALIZE, params)).to_value())?;
    s.set_read_timeout(Some(answer))?;
    let mut lines = BufReader::new(s.try_clone()?).lines();
    let project = loop {
        let line = match lines.next() {
            Some(Ok(line)) => line,
            // no answer in 5 s: nothing was asked, the hub runs on
            Some(Err(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Err(std::io::Error::other(format!("{} {} s: it was not stopped", NO_ANSWER, answer.as_secs())));
            }
            // it closed the connection: going already
            _ => return Ok(Asked::Going),
        };
        match rpc::init_answer(&serde_json::from_str(&line).unwrap_or(Value::Null), &Id::Num(0)) {
            Init::Ready(res) => break res.project,
            // refused (it runs in an agent's process): the hub stays
            Init::Refused(_) => return Ok(Asked::Going),
            // a hub from before client-protocol: never asked, it runs on
            Init::NotRpc => return Err(std::io::Error::other(rpc::HUB_OLDER)),
            Init::Other => {}
        }
    };
    drop(lines);
    if let Some(req) = rpc::request(Id::Num(1), &HubCmd::StopHub { project, keep_agents }) {
        writeln!(s, "{}", Message::Request(req).to_value())?;
    }
    Ok(Asked::Stop(s))
}

/// How long `stop` waits for the hub's `initialize` answer.
const STOP_ANSWER: Duration = Duration::from_secs(5);
/// How long `stop` waits for the hub's socket to go: the hub removes it
/// as its loop ends, before its REPLs checkpoint.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// The start of `request`'s error when the hub did not answer in time.
pub const NO_ANSWER: &str = "the hub did not answer (busy) in";

/// How long `sb land` waits for its land (land-wait): a land waits its
/// turn, then its check (`land_check::MAX`), off the hub's loop; the
/// agent's bash puts a long one in the background and wakes on its end.
pub const LAND_WAIT: Duration = Duration::from_secs(2 * 3600);

/// How long `sb <cmd>` waits for its answer (`timeout_s`: the request's
/// own wait, `sb ask`, `sb wait`), and whether it is a read the client
/// may retry. hub-lag: a request that changes something is in the hub's
/// queue once written: it waits longer rather than give up on a busy
/// hub and have it retried (a second message, a second task).
pub fn wait_for(cmd: &str, timeout_s: u64) -> (Duration, bool) {
    let idempotent = matches!(cmd, "list" | "tasks" | "history" | "show" | "inspect" | "wait");
    if cmd == "land" {
        return (LAND_WAIT, false);
    }
    let slack = if idempotent { 30 } else { 180 };
    (Duration::from_secs(timeout_s + slack), idempotent)
}

/// What `sb <cmd>` prints for `request`'s error `e`: a land that is
/// still on its way says its result comes as bise's message
/// (daemon/land.rs), a hub gone mid-land what to check.
pub fn said(cmd: &str, e: &str) -> String {
    if cmd != "land" {
        return e.to_string();
    }
    if e.starts_with(NO_ANSWER) {
        return format!("still landing after {} h: bise sends you its result when it ends", LAND_WAIT.as_secs() / 3600);
    }
    if e.starts_with("unreadable answer") || e.starts_with("no answer from the hub") {
        return "the hub stopped during your land: main moves only after a green check. see git log main, then sb land again if your commits are not there".into();
    }
    e.to_string()
}

/// One request, one JSON answer (the `sb` CLI).
pub fn request(socket: &Path, req: &Value, timeout: Duration) -> Result<Value, String> {
    let mut s = UnixStream::connect(socket)
        .map_err(|e| format!("hub injoignable ({}) : {}", socket.display(), e))?;
    s.set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut answer = String::new();
    r.read_line(&mut answer).map_err(|e| match e.kind() {
        // a read timeout is EAGAIN on macOS ("Resource temporarily
        // unavailable"): say what it means
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
            format!("{} {} s", NO_ANSWER, timeout.as_secs())
        }
        _ => format!("no answer from the hub: {}", e),
    })?;
    serde_json::from_str(answer.trim())
        .map_err(|e| format!("unreadable answer: {} ({})", e, answer.trim()))
}

/// `request`, through a hub restart (a version switch, a crash): a hub
/// that is not there yet is waited for (up to 20 s); a connection the hub
/// closed without answering is retried when `idempotent` (a read, a
/// wait) - never a send, which may have been done already.
pub fn request_retry(
    socket: &Path,
    req: &Value,
    timeout: Duration,
    idempotent: bool,
) -> Result<Value, String> {
    let t0 = Instant::now();
    loop {
        let connected = UnixStream::connect(socket).is_ok();
        let r = if connected {
            request(socket, req, timeout)
        } else {
            Err("hub absent".to_string())
        };
        let again = match &r {
            Ok(_) => false,
            Err(_) if !connected => true,
            Err(e) => idempotent && e.starts_with("unreadable answer"),
        };
        if !again || t0.elapsed() > Duration::from_secs(20) {
            return r;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// land-wait: `sb land` waits for its land (2 h), never 180 s; a read
    /// 30 s more than its own wait, a change 180 s.
    #[test]
    fn a_land_waits_for_its_land() {
        assert_eq!(wait_for("land", 0), (LAND_WAIT, false));
        assert_eq!(wait_for("list", 0), (Duration::from_secs(30), true));
        assert_eq!(wait_for("wait", 60), (Duration::from_secs(90), true));
        assert_eq!(wait_for("send", 0), (Duration::from_secs(180), false));
    }

    #[test]
    fn a_land_that_ran_long_or_lost_its_hub_says_what_comes() {
        let late = format!("{} 7200 s", NO_ANSWER);
        assert_eq!(said("land", &late), "still landing after 2 h: bise sends you its result when it ends");
        assert_eq!(said("send", &late), late);
        assert!(said("land", "unreadable answer: EOF while parsing ()").starts_with("the hub stopped during your land: "));
        assert_eq!(said("land", "hub injoignable (x) : no"), "hub injoignable (x) : no");
    }

    /// The user's fresh install: a raw `Error: Custom { .. }` line in
    /// hub.err becomes one human line with the cause and the log.
    #[test]
    fn a_hub_that_did_not_start_says_why_in_one_line() {
        let log = Path::new("/h/.bise/hubs/x-0123abcd/hub.err");
        let raw = r#"Error: Custom { kind: InvalidInput, error: "path must be shorter than SUN_LEN" }"#;
        assert_eq!(
            not_started(raw, log),
            "the hub did not start: path must be shorter than SUN_LEN (its log: /h/.bise/hubs/x-0123abcd/hub.err)"
        );
        let os = r#"Error: Os { code: 48, kind: AddrInUse, message: "Address already in use" }"#;
        assert!(not_started(os, log).starts_with("the hub did not start: Address already in use (its log: "));
        assert_eq!(not_started("bise: disk full", log), "the hub did not start: bise: disk full (its log: /h/.bise/hubs/x-0123abcd/hub.err)");
        assert_eq!(not_started("", log), "the hub did not start in 15 s (its log: /h/.bise/hubs/x-0123abcd/hub.err)");
        assert!(!not_started(raw, log).contains('\n'));
    }

    /// A start that hangs never blocks another client past its own wait
    /// (architect m_21925): the waiter gives up with a cause, and gets the
    /// lock as soon as the holder lets go (or dies: the kernel drops it).
    #[test]
    fn a_stuck_start_holds_the_start_lock_no_longer_than_the_wait() {
        let dir = std::env::temp_dir().join(format!("start-lock-{}", std::process::id()));
        let paths = Paths { workspace: dir.clone(), state: dir.join("state"), worktrees: dir.join("wt"), run: dir.join("run") };
        let held = start_lock(&paths).unwrap();
        let t0 = Instant::now();
        let e = start_lock_within(&paths, Duration::from_millis(300)).unwrap_err();
        assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
        assert!(e.to_string().starts_with("another start of this hub took over"), "{e}");
        drop(held);
        assert!(start_lock_within(&paths, Duration::from_millis(300)).is_ok());
        assert!(LOCK_WAIT > START_WAIT && LOCK_WAIT < START_WAIT + Duration::from_secs(5));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn read(r: &mut impl BufRead) -> Value {
        let mut l = String::new();
        r.read_line(&mut l).unwrap();
        serde_json::from_str(&l).unwrap_or(Value::Null)
    }

    /// A fake hub's socket (short: ~100 bytes at most), its path.
    fn sock(n: u32) -> (std::path::PathBuf, std::os::unix::net::UnixListener) {
        let p = std::env::temp_dir().join(format!("stop{}-{n}.sock", std::process::id()));
        let _ = std::fs::remove_file(&p);
        (p.clone(), std::os::unix::net::UnixListener::bind(&p).unwrap())
    }

    /// `bise stop`: `initialize`, then `hub/stop` with `keep_agents`;
    /// it returns once the socket is gone.
    #[test]
    fn stop_says_initialize_then_hub_stop() {
        let (p, l) = sock(1);
        let path = p.clone();
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let mut r = BufReader::new(w.try_clone().unwrap());
            assert_eq!(read(&mut r)["method"], "initialize");
            let res = json!({"project": "p1", "proto": bise_proto::PROTO, "workspace": "/w", "name": "w", "methods": ["hub/stop"],
                "notifications": [], "hub": {"watermark": {"epoch": 1, "seq": 0}, "state": []}});
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "id": 0, "result": res})).unwrap();
            let req = read(&mut r);
            std::fs::remove_file(&path).unwrap();
            req
        });
        assert!(stop_at(&p, true, STOP_ANSWER, STOP_WAIT).unwrap());
        let req = hub.join().unwrap();
        assert_eq!((req["method"].as_str(), req["params"]["project"].as_str(), req["params"]["keep_agents"].as_bool()), (Some("hub/stop"), Some("p1"), Some(true)));
    }

    /// A hub from before client-protocol (it doesn't serve `initialize`)
    /// is never asked: one error that says so, no second connection.
    #[test]
    fn an_older_hub_is_not_stopped_and_says_why() {
        let (p, l) = sock(2);
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let _ = read(&mut BufReader::new(w.try_clone().unwrap()));
            writeln!(w, "{}", json!({"ok": false, "error": "hub.sock does not serve the op \"\""})).unwrap();
            drop(w);
            l.set_nonblocking(true).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            l.accept().is_ok()
        });
        let e = stop_at(&p, false, STOP_ANSWER, STOP_WAIT).unwrap_err();
        assert_eq!(e.to_string(), bise_proto::rpc::HUB_OLDER);
        assert!(!hub.join().unwrap(), "a second connection to the older hub");
        let _ = std::fs::remove_file(&p);
    }

    /// architect m_24004: an older hub that runs from his install gets
    /// the install's steps (its own bise stops it), the same words as the
    /// desktop core's; no hub.root, or a root of no install: HUB_OLDER.
    #[test]
    fn an_older_hub_of_his_install_gets_the_install_steps() {
        let t = std::env::temp_dir().join(format!("sb-olderwords-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let (state, prefix) = (t.join("hub"), t.join(".local/share/bise"));
        let v = prefix.join("versions/v29");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(v.join("VERSION"), "id=v29\n").unwrap();
        std::os::unix::fs::symlink("versions/v29", prefix.join("current")).unwrap();
        assert_eq!(older_words_in(&state, &t), bise_proto::rpc::HUB_OLDER);
        std::fs::write(state.join("hub.root"), v.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(older_words_in(&state, &t), bise_proto::rpc::hub_older_steps(true, "~/.local/share/bise/versions/v29/bise"));
        std::fs::write(state.join("hub.root"), t.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(older_words_in(&state, &t), bise_proto::rpc::HUB_OLDER);
        let _ = std::fs::remove_dir_all(&t);
    }

    /// 2026-10-11: the hub took the stop but ran on (its stop queued 85 s
    /// behind REPL lines); `bise switchboard --stop` said "stopped" and
    /// the next bise attached to it. Still there after the wait: an error.
    #[test]
    fn a_hub_still_running_after_the_stop_is_not_stopped() {
        let (p, l) = sock(3);
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let mut r = BufReader::new(w.try_clone().unwrap());
            let _ = read(&mut r);
            let res = json!({"project": "p1", "proto": bise_proto::PROTO, "workspace": "/w", "name": "w", "methods": ["hub/stop"],
                "notifications": [], "hub": {"watermark": {"epoch": 1, "seq": 0}, "state": []}});
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "id": 0, "result": res})).unwrap();
            let req = read(&mut r);
            // the socket stays: the hub runs on
            std::thread::sleep(Duration::from_millis(600));
            req
        });
        let e = stop_at(&p, false, STOP_ANSWER, Duration::from_millis(300)).unwrap_err();
        assert!(e.to_string().starts_with("the hub got the stop but still runs"), "{e}");
        assert_eq!(hub.join().unwrap()["method"], "hub/stop");
        let _ = std::fs::remove_file(&p);
    }

    /// A hub that does not answer `initialize` in time was asked nothing:
    /// an error, never "stopped".
    #[test]
    fn a_hub_that_does_not_answer_was_not_stopped() {
        let (p, l) = sock(4);
        let hub = std::thread::spawn(move || {
            let (w, _) = l.accept().unwrap();
            std::thread::sleep(Duration::from_millis(700));
            drop(w);
        });
        let e = stop_at(&p, false, Duration::from_millis(200), STOP_WAIT).unwrap_err();
        assert!(e.to_string().contains("it was not stopped"), "{e}");
        hub.join().unwrap();
        let _ = std::fs::remove_file(&p);
    }
}
