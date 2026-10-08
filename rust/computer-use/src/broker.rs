//! The broker: one per machine, on `~/.bise/run/computer-use.sock` (C3).
//!
//! Three kinds of connection say who they are in their first line:
//! - an agent's MCP server (C3): `{"op":"hello","agent","session","tmpdir"}`,
//!   then `{"id","op","args"}` requests;
//! - a browser, through its native host relay (`host.rs`):
//!   `{"op":"hello","role":"browser"}`, then the C4 messages as JSON lines;
//! - a command (`bise computer-use stop ...`): `{"op":"hello","role":"ctl"}`.
//!
//! The broker connects to the helper app itself (C5), starting it when
//! its socket is missing. It routes `tab:` targets to the browser that owns
//! the tab and `app:` targets to the helper, refuses the hard-refused
//! targets (`refuse.rs`), writes screenshots to the agent's `TMPDIR`, holds
//! stop and pause, and writes C6's files.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::browsers::{self, Browser};
use crate::paths::Paths;
use crate::proto::{err, short_host, str_of, Target, TOOLS};
use crate::{image, refuse, state};

mod ctl;
mod status;
use ctl::{ctl_loop, release, stop_agent};
use status::{full_status, status, write_state};

/// The helper's bundle name and id (C5).
pub const HELPER_APP: &str = "bise Computer Use";
/// Its executable (CFBundleExecutable): what `bise computer-use off` quits.
pub const HELPER_EXE: &str = "bise-computer-use";
pub const HELPER_ID: &str = "dev.bise.computer-use";

#[derive(Clone, Debug)]
pub struct Opts {
    pub paths: Paths,
    /// exit after this long with no connection (None: never)
    pub idle_exit: Option<Duration>,
    /// an agent that did nothing for this long lets go (C4/C5 `release`:
    /// the debugger detaches, the yellow bar goes away)
    pub release_after: Duration,
    /// start the helper app when its socket is missing (never in tests)
    pub launch_helper: bool,
    /// `bise Computer Use.app` (see [`find_helper`]); None: not installed
    pub helper_app: Option<PathBuf>,
    /// on top of an action's own `timeout_ms`, the wait for its reply
    pub slack: Duration,
    /// after `open -g` of the helper, how long its socket may take
    pub helper_wait: Duration,
    /// who opened a connection (docs/issues/18): the process table in the
    /// commands, a fake in tests
    pub judge: crate::who::JudgeFn,
}

impl Opts {
    pub fn new(paths: Paths) -> Opts {
        let helper_app = find_helper(&paths.home);
        Opts {
            paths,
            idle_exit: Some(Duration::from_secs(600)),
            release_after: Duration::from_secs(20),
            launch_helper: true,
            helper_app,
            slack: Duration::from_secs(15),
            helper_wait: Duration::from_secs(8),
            judge: crate::who::JudgeFn::real(),
        }
    }
}

type Reply = Result<Value, Value>;
type Writer = Arc<Mutex<UnixStream>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn send_line(w: &Writer, v: &Value) -> bool {
    let mut line = v.to_string();
    line.push('\n');
    let mut s = lock(w);
    s.write_all(line.as_bytes()).and_then(|_| s.flush()).is_ok()
}

/// A browser (one extension in one browser profile).
struct BrowserLink {
    id: u64,
    w: Writer,
    /// from its C4 hello; None until it said hello
    browser: Option<Browser>,
    version: String,
    extension_version: String,
    /// the build its code was loaded with (hello.build): an older one than
    /// ~/.bise/computer-use/extension means Chrome didn't reload it yet
    extension_build: Option<String>,
    /// the browser's process (its relay's parent, in the hello): when the
    /// link ends and it still runs, only the extension's service worker
    /// stopped (MV3), and it comes back within 30 s
    pid: Option<i32>,
}

/// After an extension's service worker stops, calls wait this long for it
/// to say hello again (its 30 s alarm wakes it) instead of no_browser.
const WAKE_WAIT: Duration = Duration::from_secs(45);

/// A quitting browser closes its relays first and exits a moment later:
/// how long a closed link waits to tell a quit from a stopped worker.
const QUIT_GRACE: Duration = Duration::from_secs(2);

fn alive(pid: i32) -> bool {
    // SAFETY: kill(pid, 0) only checks that the process exists
    unsafe { libc::kill(pid, 0) == 0 }
}

struct HelperLink {
    id: u64,
    w: Writer,
    hello: Value,
}

struct Pending {
    agent: String,
    target: Option<String>,
    link: u64,
    tx: Sender<Reply>,
}

#[derive(Default)]
struct Inner {
    browsers: Vec<BrowserLink>,
    helper: Option<HelperLink>,
    pending: HashMap<u64, Pending>,
    next: u64,
    agents: BTreeMap<String, state::Agent>,
    last_action: HashMap<String, Instant>,
    /// (agent, tab target) -> its browser link. Tab ids are unique in one
    /// browser only: Chrome's tab 17 and Edge's tab 17 are two tabs.
    owners: HashMap<(String, String), u64>,
    /// target -> (url, title) last seen
    seen: HashMap<String, (String, String)>,
    /// app target -> its name (from `apps`)
    app_names: HashMap<String, String>,
    /// live agent connections per agent
    sessions: HashMap<String, usize>,
    conns: usize,
    quiet_since: Option<Instant>,
    streams: Vec<UnixStream>,
    /// a browser whose extension's worker stopped: since when, its pid
    /// (see [`WAKE_WAIT`]); None again at the next hello
    asleep: Option<(Instant, i32)>,
    /// the last browser that said hello: once one did, a closed one is
    /// no_browser ("Chrome is closed"), never not_set_up
    last_browser: Option<&'static str>,
}

struct Shared {
    opts: Opts,
    inner: Mutex<Inner>,
    stop: AtomicBool,
    started: Instant,
    /// one helper connection attempt at a time
    helper_gate: Mutex<()>,
    /// the last `open -g` of the helper for a `permissions` poll (at most
    /// one per [`PERMISSIONS_LAUNCH_EVERY`])
    last_launch: Mutex<Option<Instant>>,
    /// how many times this broker ran `open -g` on the helper (tests)
    launches: AtomicUsize,
}

/// `permissions` (the /computer-use rows' 1 s poll) starts the helper at
/// most this often.
const PERMISSIONS_LAUNCH_EVERY: Duration = Duration::from_secs(20);

/// A running broker (tests stop it to restart it).
pub struct Handle {
    shared: Arc<Shared>,
    accept: Vec<std::thread::JoinHandle<()>>,
    _lock: std::fs::File,
}

impl Handle {
    /// How many times this broker started the helper app (`open -g`).
    pub fn helper_launches(&self) -> usize {
        self.shared.launches.load(Ordering::SeqCst)
    }

    /// Close the socket and every connection, as if the process died.
    pub fn shutdown(mut self) {
        self.close();
    }

    fn close(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(self.shared.opts.paths.socket());
        let _ = UnixStream::connect(self.shared.opts.paths.ctl_socket());
        for t in self.accept.drain(..) {
            let _ = t.join();
        }
        let inner = lock(&self.shared.inner);
        for s in &inner.streams {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        drop(inner);
        let _ = std::fs::remove_file(self.shared.opts.paths.socket());
        let _ = std::fs::remove_file(self.shared.opts.paths.ctl_socket());
    }

    /// Block until the broker stops (idle exit).
    pub fn wait(mut self) {
        while !self.shared.stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
        }
        self.close();
    }
}

#[derive(Debug)]
pub enum StartError {
    /// another broker holds the lock
    Running,
    Io(std::io::Error),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::Running => write!(f, "a broker already runs"),
            StartError::Io(e) => write!(f, "{}", e),
        }
    }
}

fn flock(paths: &Paths) -> Result<std::fs::File, StartError> {
    use std::os::unix::io::AsRawFd;
    paths.ensure().map_err(StartError::Io)?;
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.lock_file())
        .map_err(StartError::Io)?;
    // SAFETY: flock on a descriptor this function owns
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r != 0 {
        return Err(StartError::Running);
    }
    Ok(f)
}

/// Start a broker: take the lock, bind the socket (0600), serve in threads.
pub fn start(opts: Opts) -> Result<Handle, StartError> {
    let lockf = flock(&opts.paths)?;
    opts.paths.prepare_sockets().map_err(StartError::Io)?;
    let sock = opts.paths.socket();
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).map_err(StartError::Io)?;
    crate::paths::private(&sock, 0o600).map_err(StartError::Io)?;
    let ctl_sock = opts.paths.ctl_socket();
    let _ = std::fs::remove_file(&ctl_sock);
    let ctl_listener = UnixListener::bind(&ctl_sock).map_err(StartError::Io)?;
    crate::paths::private(&ctl_sock, 0o600).map_err(StartError::Io)?;
    let inner = Inner { agents: state::restore(&opts.paths), quiet_since: Some(Instant::now()), ..Inner::default() };
    let shared = Arc::new(Shared { opts, inner: Mutex::new(inner), stop: AtomicBool::new(false), started: Instant::now(), helper_gate: Mutex::new(()), last_launch: Mutex::new(None), launches: AtomicUsize::new(0) });
    write_state(&shared);
    {
        let sh = shared.clone();
        std::thread::spawn(move || ticker(sh));
    }
    let accept = |l: UnixListener, ctl: bool| {
        let sh = shared.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                if sh.stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(s) = s else { continue };
                let sh = sh.clone();
                std::thread::spawn(move || serve(sh, s, ctl));
            }
        })
    };
    let threads = vec![accept(listener, false), accept(ctl_listener, true)];
    Ok(Handle { shared, accept: threads, _lock: lockf })
}

fn log(msg: &str) {
    eprintln!("[{}] {}", crate::now_ms(), msg);
}

// ---- connections ----

/// One connection: judged first (docs/issues/18, `who.rs`), then served
/// by what its hello says. `ctl`: it came in on the commands' socket.
fn serve(sh: Arc<Shared>, s: UnixStream, ctl: bool) {
    let peer = (sh.opts.judge.0)(&s);
    let Ok(rd) = s.try_clone() else { return };
    let Ok(w) = s.try_clone() else { return };
    // the end of this connection: closed here, also when it was refused
    // (`streams` keeps a handle for the broker's own shutdown)
    let Ok(end) = s.try_clone() else { return };
    {
        let mut inner = lock(&sh.inner);
        inner.conns += 1;
        inner.quiet_since = None;
        inner.streams.push(s);
    }
    let w: Writer = Arc::new(Mutex::new(w));
    let mut lines = BufReader::new(rd).lines();
    let hello: Value = match lines.next() {
        Some(Ok(l)) => serde_json::from_str(&l).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    if hello.get("op").and_then(Value::as_str) == Some("hello") {
        let outside = peer == crate::who::Peer::Outside;
        match (ctl, str_of(&hello, "role"), str_of(&hello, "agent")) {
            (true, Some("ctl"), _) => match crate::who::ctl_refusal(&peer) {
                None => ctl_loop(&sh, w, lines),
                Some(why) => {
                    log(&format!("command refused: {:?}", peer));
                    refuse(&w, lines, why)
                }
            },
            (true, _, _) => refuse(&w, lines, "this socket takes commands only: {\"op\":\"hello\",\"role\":\"ctl\"}"),
            (false, Some("ctl"), _) => refuse(&w, lines, "commands go to computer-use-ctl.sock"),
            (false, Some("browser"), _) if outside => browser_loop(&sh, w, lines),
            (false, Some("browser"), _) => log(&format!("browser link refused: {:?}", peer)),
            (false, _, Some(name)) => match crate::who::agent_key(&peer, name) {
                Some(key) => {
                    let tmpdir = str_of(&hello, "tmpdir").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
                    agent_loop(&sh, key, tmpdir, w, lines)
                }
                None => refuse(&w, lines, "the broker could not read which process opened this connection"),
            },
            _ => {}
        }
    }
    let _ = end.shutdown(std::net::Shutdown::Both);
    let mut inner = lock(&sh.inner);
    inner.conns -= 1;
    if inner.conns == 0 {
        inner.quiet_since = Some(Instant::now());
    }
}

type Lines = std::io::Lines<BufReader<UnixStream>>;

/// A refused connection: its first request gets `refused` with `why`
/// (the line the caller prints), then the connection ends.
fn refuse(w: &Writer, mut lines: Lines, why: &str) {
    let id = lines.next().and_then(Result::ok).and_then(|l| serde_json::from_str::<Value>(&l).ok()).and_then(|r| r.get("id").cloned());
    send_line(w, &json!({"id": id, "ok": false, "error": err("refused", why)}));
}

fn agent_loop(sh: &Arc<Shared>, agent: String, tmpdir: PathBuf, w: Writer, lines: Lines) {
    *lock(&sh.inner).sessions.entry(agent.clone()).or_default() += 1;
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let (sh, agent, tmpdir, w) = (sh.clone(), agent.clone(), tmpdir.clone(), w.clone());
        std::thread::spawn(move || {
            let id = req.get("id").cloned().unwrap_or(Value::Null);
            let op = str_of(&req, "op").unwrap_or("").to_string();
            let args = req.get("args").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
            let out = match handle(&sh, &agent, &tmpdir, &op, &args) {
                Ok(r) => json!({"id": id, "ok": true, "result": r}),
                Err(e) => json!({"id": id, "ok": false, "error": e}),
            };
            send_line(&w, &out);
        });
    }
    let last = {
        let mut inner = lock(&sh.inner);
        let n = inner.sessions.entry(agent.clone()).or_default();
        *n = n.saturating_sub(1);
        *n == 0
    };
    if last {
        release(sh, &agent);
    }
}

fn browser_loop(sh: &Arc<Shared>, w: Writer, lines: Lines) {
    let id = {
        let mut inner = lock(&sh.inner);
        inner.next += 1;
        let id = inner.next;
        inner.browsers.push(BrowserLink { id, w, browser: None, version: String::new(), extension_version: String::new(), extension_build: None, pid: None });
        id
    };
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(h) = msg.get("hello") {
            let b = str_of(h, "browser").and_then(browsers::by_key).unwrap_or(browsers::ALL[0]);
            {
                let mut inner = lock(&sh.inner);
                if let Some(l) = inner.browsers.iter_mut().find(|l| l.id == id) {
                    l.browser = Some(b);
                    l.version = str_of(h, "version").unwrap_or("").to_string();
                    l.extension_version = str_of(h, "extension_version").unwrap_or("").to_string();
                    l.extension_build = str_of(h, "build").map(String::from);
                    l.pid = h.get("pid").and_then(Value::as_i64).and_then(|p| i32::try_from(p).ok()).filter(|p| *p > 1);
                }
                // the most recent browser comes first for `open`
                if let Some(i) = inner.browsers.iter().position(|l| l.id == id) {
                    let l = inner.browsers.remove(i);
                    inner.browsers.insert(0, l);
                }
                inner.asleep = None;
                inner.last_browser = Some(b.name);
            }
            log(&format!("browser {} connected", b.name));
            write_state(sh);
            continue;
        }
        incoming(sh, id, &msg);
    }
    let (name, pid) = {
        let mut inner = lock(&sh.inner);
        let link = inner.browsers.iter().find(|l| l.id == id);
        let name = link.and_then(|l| l.browser).map(|b| b.name).unwrap_or("the browser");
        let pid = link.filter(|l| l.browser.is_some()).and_then(|l| l.pid).filter(|p| alive(*p));
        // until we know: calls wait for its hello (live_links)
        if let Some(p) = pid {
            inner.asleep = Some((Instant::now(), p));
        }
        inner.browsers.retain(|l| l.id != id);
        inner.owners.retain(|_, l| *l != id);
        (name, pid)
    };
    write_state(sh);
    let asleep = pid.is_some_and(|p| {
        let t0 = Instant::now();
        while t0.elapsed() < QUIT_GRACE {
            if !alive(p) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        true
    });
    if asleep {
        // only the extension's worker stopped: the browser runs on, the next call waits for it
        fail_link(sh, id, err("timeout", format!("the bise extension in {} restarted during the action; try again", name)));
        log(&format!("browser {} asleep (its extension's service worker stopped)", name));
    } else {
        {
            let mut inner = lock(&sh.inner);
            if inner.asleep.is_some_and(|(_, p)| Some(p) == pid) {
                inner.asleep = None;
            }
        }
        fail_link(sh, id, err("no_browser", format!("{} closed during the action; ask the user to open it again", name)));
        log(&format!("browser {} gone", name));
    }
}

/// A reply or an event from a browser or the helper.
fn incoming(sh: &Arc<Shared>, link: u64, msg: &Value) {
    if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        let p = lock(&sh.inner).pending.remove(&id);
        if let Some(p) = p {
            let r = if msg.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
            } else {
                let mut e = msg.get("error").cloned().unwrap_or_else(|| json!({}));
                if !e.is_object() || str_of(&e, "code").is_none() {
                    let m = e.as_str().map(String::from).unwrap_or_else(|| e.to_string());
                    e = err("timeout", format!("the action failed: {}", m));
                    // a broken reply, not a C1 error: the MCP server says isError
                    e["transport"] = json!(true);
                }
                Err(e)
            };
            let _ = p.tx.send(r);
        }
        return;
    }
    let Some(ev) = str_of(msg, "event") else { return };
    let Some(agent) = str_of(msg, "agent").map(String::from) else { return };
    let _ = link;
    match ev {
        "stopped" => {
            let by = match str_of(msg, "reason") {
                Some("group_closed") => "group_closed",
                Some("cancel_bar") => "cancel_bar",
                _ => "you",
            };
            stop_agent(sh, &agent, by);
        }
        "paused" => {
            let target = str_of(msg, "target").unwrap_or("").to_string();
            let driving = {
                let mut inner = lock(&sh.inner);
                let a = inner.agents.entry(agent.clone()).or_default();
                if !a.paused.contains(&target) {
                    a.paused.push(target.clone());
                }
                a.driving.clone()
            };
            let _ = state::event_driving(&sh.opts.paths, &agent, "paused", "you", driving.as_deref());
            fail_where(sh, |p| p.agent == agent && p.target.as_deref() == Some(target.as_str()), || {
                err("paused", "the user took over this tab or app; wait until he gives it back, or ask him")
            });
            write_state(sh);
        }
        "resumed" => {
            if let Some(a) = lock(&sh.inner).agents.get_mut(&agent) {
                a.paused.clear();
            }
            let _ = state::event(&sh.opts.paths, &agent, "resumed", "you");
            write_state(sh);
        }
        _ => {}
    }
}

fn fail_where(sh: &Arc<Shared>, pred: impl Fn(&Pending) -> bool, e: impl Fn() -> Value) {
    let gone: Vec<Pending> = {
        let mut inner = lock(&sh.inner);
        let ids: Vec<u64> = inner.pending.iter().filter(|(_, p)| pred(p)).map(|(k, _)| *k).collect();
        ids.iter().filter_map(|k| inner.pending.remove(k)).collect()
    };
    for p in gone {
        let _ = p.tx.send(Err(e()));
    }
}

fn fail_link(sh: &Arc<Shared>, link: u64, e: Value) {
    fail_where(sh, |p| p.link == link, || e.clone());
}

/// Every browser and the helper (control lines: stop, resume, release, drop).
fn broadcast(sh: &Arc<Shared>, v: &Value) {
    let ws: Vec<Writer> = {
        let inner = lock(&sh.inner);
        inner.browsers.iter().map(|l| l.w.clone()).chain(inner.helper.as_ref().map(|h| h.w.clone())).collect()
    };
    for w in ws {
        send_line(&w, v);
    }
}

// ---- the helper (C5) ----

/// Where `bise Computer Use.app` is: `$BISE_CU_HELPER`, the app root
/// (`$BISE_APP_ROOT`, else next to this executable: the helper ships in
/// the version dir), then /Applications and ~/Applications. Opened by path
/// (`open -g <path>`): it registers the app with LaunchServices, where a
/// bundle id lookup would fail before its first launch (cu-apps).
pub fn find_helper(home: &std::path::Path) -> Option<PathBuf> {
    let app = format!("{}.app", HELPER_APP);
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.canonicalize().ok()).and_then(|e| e.parent().map(PathBuf::from));
    let mut c: Vec<PathBuf> = Vec::new();
    c.extend(bise_home::env::test_setting("BISE_CU_HELPER").map(PathBuf::from));
    c.extend(env("BISE_APP_ROOT").map(|r| r.join(&app)));
    c.extend(exe_dir.map(|d| d.join(&app)));
    c.push(PathBuf::from("/Applications").join(&app));
    c.push(home.join("Applications").join(&app));
    c.into_iter().find(|p| p.is_dir())
}

fn helper_installed(sh: &Shared) -> bool {
    sh.opts.helper_app.is_some()
}

fn connect_helper(sh: &Arc<Shared>) -> Option<()> {
    let s = UnixStream::connect(&sh.opts.paths.app_socket).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let rd = s.try_clone().ok()?;
    let mut lines = BufReader::new(rd).lines();
    let hello: Value = serde_json::from_str(&lines.next()?.ok()?).ok()?;
    let hello = hello.get("hello")?.clone();
    s.set_read_timeout(None).ok()?;
    let w: Writer = Arc::new(Mutex::new(s.try_clone().ok()?));
    let id = {
        let mut inner = lock(&sh.inner);
        inner.next += 1;
        let id = inner.next;
        inner.helper = Some(HelperLink { id, w, hello });
        inner.streams.push(s);
        id
    };
    log("helper connected");
    let sh2 = sh.clone();
    std::thread::spawn(move || {
        for line in lines {
            let Ok(line) = line else { break };
            if let Ok(msg) = serde_json::from_str::<Value>(&line) {
                incoming(&sh2, id, &msg);
            }
        }
        lock(&sh2.inner).helper.take_if(|h| h.id == id);
        fail_link(&sh2, id, err("no_helper", "the bise Computer Use app quit during the action; try again"));
        log("helper gone");
        write_state(&sh2);
    });
    write_state(sh);
    Some(())
}

/// The helper's link id: connected, else connect, else start it and wait.
fn helper(sh: &Arc<Shared>) -> Result<u64, Value> {
    if let Some(h) = &lock(&sh.inner).helper {
        return Ok(h.id);
    }
    let _g = lock(&sh.helper_gate);
    let id = || lock(&sh.inner).helper.as_ref().map(|h| h.id);
    if let Some(i) = id() {
        return Ok(i);
    }
    if connect_helper(sh).is_none() && sh.opts.launch_helper && helper_installed(sh) {
        // -g: never in front (the user's rule: computer use never steals focus)
        sh.launches.fetch_add(1, Ordering::SeqCst);
        let _ = std::process::Command::new("open")
            .arg("-g")
            .arg(sh.opts.helper_app.as_deref().unwrap_or(std::path::Path::new("")))
            .args(["--args", "--socket"])
            .arg(&sh.opts.paths.app_socket)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let t0 = Instant::now();
        while t0.elapsed() < sh.opts.helper_wait && connect_helper(sh).is_none() {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    id().ok_or_else(|| {
        if helper_installed(sh) || !sh.opts.launch_helper {
            err("no_helper", "the bise Computer Use app doesn't answer; ask the user to run /computer-use")
        } else {
            err("no_helper", "the bise Computer Use app isn't installed; ask the user to run /computer-use")
        }
    })
}

// ---- requests ----

/// One request to a browser or the helper; waits for its reply.
fn forward(sh: &Arc<Shared>, link: u64, agent: Option<&str>, target: Option<&str>, op: &str, args: &Value, wait: Duration) -> Reply {
    let (tx, rx) = channel();
    let (id, w) = {
        let mut inner = lock(&sh.inner);
        let w = inner
            .browsers
            .iter()
            .find(|l| l.id == link)
            .map(|l| l.w.clone())
            .or_else(|| inner.helper.as_ref().filter(|h| h.id == link).map(|h| h.w.clone()));
        let Some(w) = w else {
            return Err(err("no_browser", "the browser closed; ask the user to open it again"));
        };
        inner.next += 1;
        let id = inner.next;
        inner.pending.insert(
            id,
            Pending { agent: agent.unwrap_or("").to_string(), target: target.map(String::from), link, tx },
        );
        (id, w)
    };
    let mut msg = json!({"id": id, "op": op, "args": args});
    if let Some(a) = agent {
        // the key routes, the name shows (a group's title, the cursor's pill)
        msg["agent"] = json!(a);
        let (hub, name) = crate::who::split(a);
        msg["name"] = json!(name);
        // a group title two projects' agents share names it (step 3)
        if let Some(p) = hub.and_then(|h| crate::who::project_of(&sh.opts.paths.hubs, h)) {
            msg["project"] = json!(p);
        }
    }
    if !send_line(&w, &msg) {
        lock(&sh.inner).pending.remove(&id);
        return Err(err("no_browser", "the browser closed; ask the user to open it again"));
    }
    match rx.recv_timeout(wait) {
        Ok(r) => r,
        Err(_) => {
            lock(&sh.inner).pending.remove(&id);
            Err(err("timeout", format!("no answer within {} s; check the page with snapshot, then try again", wait.as_secs())))
        }
    }
}

fn wait_for(sh: &Arc<Shared>, args: &Value) -> Duration {
    let ms = args.get("timeout_ms").and_then(Value::as_u64).unwrap_or(5000).min(120_000);
    Duration::from_millis(ms) + sh.opts.slack
}

/// The browser `open` goes to: the one asked, else the agent's, else the
/// last connected.
fn pick_browser(sh: &Arc<Shared>, agent: &str, wanted: Option<&str>) -> Result<(u64, Browser), Value> {
    live_links(sh);
    let inner = lock(&sh.inner);
    let live: Vec<(u64, Browser)> = inner.browsers.iter().filter_map(|l| l.browser.map(|b| (l.id, b))).collect();
    if live.is_empty() {
        drop(inner);
        return Err(no_browser(sh));
    }
    if let Some(w) = wanted {
        return live
            .iter()
            .find(|(_, b)| b.key.eq_ignore_ascii_case(w) || b.name.eq_ignore_ascii_case(w))
            .copied()
            .ok_or_else(|| {
                let names: Vec<&str> = live.iter().map(|(_, b)| b.name).collect();
                err("no_browser", format!("{} isn't connected (connected: {}); use one of those or ask the user", w, names.join(", ")))
            });
    }
    let mine = inner.owners.iter().find(|((a, _), _)| a == agent).map(|(_, l)| *l);
    Ok(mine.and_then(|m| live.iter().find(|(l, _)| *l == m).copied()).unwrap_or(live[0]))
}

fn no_browser(sh: &Arc<Shared>) -> Value {
    let p = &sh.opts.paths;
    if let Some(name) = lock(&sh.inner).last_browser {
        return err("no_browser", format!("{} is closed (it was connected); ask the user to open it again", name));
    }
    let set_up = p.shim().exists() && browsers::ALL.iter().any(|b| browsers::manifest_state(b, p) == "ok");
    if set_up {
        err("no_browser", "no browser with the bise extension is open; ask the user to open Chrome (/computer-use checks it)")
    } else {
        err("not_set_up", "computer use isn't set up: ask the user to run /computer-use")
    }
}

/// `tabs` of one browser for one agent; records the owners.
fn tabs_of(sh: &Arc<Shared>, link: u64, agent: &str) -> Reply {
    let r = forward(sh, link, Some(agent), None, "tabs", &json!({}), Duration::from_secs(5) + sh.opts.slack)?;
    let mut inner = lock(&sh.inner);
    for t in r.as_array().into_iter().flatten() {
        if let Some(target) = str_of(t, "target") {
            inner.owners.insert((agent.to_string(), target.to_string()), link);
            let (url, title) = (str_of(t, "url").unwrap_or(""), str_of(t, "title").unwrap_or(""));
            inner.seen.insert(target.to_string(), (url.to_string(), title.to_string()));
        }
    }
    Ok(r)
}

/// The browser that owns `target` for `agent`; asks each browser's `tabs`
/// when unknown (after a broker restart).
fn owner(sh: &Arc<Shared>, agent: &str, target: &str) -> Result<(u64, Browser), Value> {
    let key = (agent.to_string(), target.to_string());
    let name_of = |l: u64| lock(&sh.inner).browsers.iter().find(|x| x.id == l).and_then(|x| x.browser);
    let known = lock(&sh.inner).owners.get(&key).copied();
    if let Some(b) = known.and_then(|l| name_of(l).map(|b| (l, b))) {
        return Ok(b);
    }
    let links = live_links(sh);
    if links.is_empty() {
        return Err(no_browser(sh));
    }
    for l in links {
        let _ = tabs_of(sh, l, agent);
    }
    let found = lock(&sh.inner).owners.get(&key).copied();
    match found.and_then(|l| name_of(l).map(|b| (l, b))) {
        Some(b) => Ok(b),
        None => Err(err("not_found", format!("{} is not open (closed, or not yours); call tabs or open", target))),
    }
}

/// The browsers that said hello. A broker that just started waits up to
/// 3 s for them: the relays reconnect a moment after a restart.
fn live_links(sh: &Arc<Shared>) -> Vec<u64> {
    loop {
        let (links, waking) = {
            let inner = lock(&sh.inner);
            let links: Vec<u64> = inner.browsers.iter().filter(|l| l.browser.is_some()).map(|l| l.id).collect();
            // a stopped worker comes back; a browser that quit since does not
            (links, inner.asleep.is_some_and(|(t, p)| t.elapsed() < WAKE_WAIT && alive(p)))
        };
        if !links.is_empty() || (sh.started.elapsed() > Duration::from_secs(3) && !waking) || sh.stop.load(Ordering::SeqCst) {
            return links;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The agent drives `what` at `place` now (C6 state).
fn driving(sh: &Arc<Shared>, agent: &str, what: &str, place: &str) {
    {
        let mut inner = lock(&sh.inner);
        inner.last_action.insert(agent.to_string(), Instant::now());
        let a = inner.agents.entry(agent.to_string()).or_default();
        if a.driving.is_none() {
            a.since_ms = Some(crate::now_ms());
        }
        a.driving = Some(what.to_string());
        if !place.is_empty() {
            a.place = Some(place.to_string());
        }
    }
    write_state(sh);
}

fn remember(sh: &Arc<Shared>, target: &str, r: &Value) {
    let mut inner = lock(&sh.inner);
    let old = inner.seen.get(target).cloned().unwrap_or_default();
    let url = str_of(r, "url").map(String::from).unwrap_or(old.0);
    let title = str_of(r, "title").map(String::from).unwrap_or(old.1);
    inner.seen.insert(target.to_string(), (url, title));
}

/// One C1 tool call of one agent.
fn handle(sh: &Arc<Shared>, agent: &str, tmpdir: &std::path::Path, op: &str, args: &Value) -> Reply {
    if op == "status" {
        return Ok(status(sh, Some(agent)));
    }
    if !TOOLS.contains(&op) {
        return Err(err("bad_args", format!("unknown tool {:?}; the tools are {}", op, TOOLS.join(", "))));
    }
    {
        let inner = lock(&sh.inner);
        if inner.agents.get(agent).is_some_and(|a| a.stopped) {
            return Err(err("stopped", "the user stopped you; ask before you start again"));
        }
    }
    match op {
        "open" => {
            let Some(url) = str_of(args, "url").filter(|u| !u.trim().is_empty()) else {
                return Err(err("bad_args", "open needs a url"));
            };
            if let Some(why) = refuse::url(url) {
                return Err(err("refused", why));
            }
            let (link, b) = pick_browser(sh, agent, str_of(args, "browser"))?;
            let r = forward(sh, link, Some(agent), None, "open", args, wait_for(sh, args))?;
            if let Some(t) = str_of(&r, "target") {
                lock(&sh.inner).owners.insert((agent.to_string(), t.to_string()), link);
                remember(sh, t, &r);
            }
            driving(sh, agent, b.name, &short_host(str_of(&r, "url").unwrap_or(url)));
            Ok(r)
        }
        "tabs" => {
            let links = live_links(sh);
            if links.is_empty() {
                return Err(no_browser(sh));
            }
            let mut all = Vec::new();
            for l in links {
                if let Ok(Value::Array(a)) = tabs_of(sh, l, agent) {
                    all.extend(a);
                }
            }
            Ok(Value::Array(all))
        }
        "apps" => {
            let h = helper(sh)?;
            let r = forward(sh, h, Some(agent), None, "apps", args, Duration::from_secs(10) + sh.opts.slack)?;
            let mut inner = lock(&sh.inner);
            for a in r.as_array().into_iter().flatten() {
                if let (Some(t), Some(n)) = (str_of(a, "target"), str_of(a, "name")) {
                    inner.app_names.insert(t.to_string(), n.to_string());
                }
            }
            Ok(r)
        }
        _ => on_target(sh, agent, tmpdir, op, args),
    }
}

/// `snapshot`, `screenshot`, `act` on one target.
fn on_target(sh: &Arc<Shared>, agent: &str, tmpdir: &std::path::Path, op: &str, args: &Value) -> Reply {
    let ts = str_of(args, "target").unwrap_or("");
    let Some(target) = Target::parse(ts) else {
        return Err(err("bad_args", "target must be tab:<id> (from open or tabs) or app:<bundle id> (from apps)"));
    };
    let action = str_of(args, "action").unwrap_or("");
    if op == "act" && action.is_empty() {
        return Err(err("bad_args", "act needs an action: click, fill, type, press, select, check, hover, scroll, goto, close, wait or read"));
    }
    // one target the user took over, or all of them ("*": ctl `pause`)
    if lock(&sh.inner).agents.get(agent).is_some_and(|a| a.paused.iter().any(|p| p == ts || p == "*")) {
        return Err(err("paused", "the user took over this tab or app; wait until he gives it back, or ask him"));
    }
    let seen = lock(&sh.inner).seen.get(ts).cloned();
    let wait = wait_for(sh, args);
    let (link, what, place) = match &target {
        Target::Tab(_) => {
            if let Some((url, _)) = &seen {
                if action != "close" {
                    if let Some(why) = refuse::url(url).or_else(|| refuse::typing(url, action)) {
                        return Err(err("refused", why));
                    }
                }
            }
            if action == "goto" {
                let Some(u) = str_of(args, "url") else { return Err(err("bad_args", "goto needs a url")) };
                if let Some(why) = refuse::url(u) {
                    return Err(err("refused", why));
                }
            }
            let (link, b) = owner(sh, agent, ts)?;
            (link, b.name.to_string(), String::new())
        }
        Target::App(bundle) => {
            let title = seen.as_ref().map(|s| s.1.as_str()).or(str_of(args, "window"));
            if let Some(why) = refuse::app(bundle, title) {
                return Err(err("refused", why));
            }
            if action == "goto" {
                return Err(err("bad_args", "goto works on tabs only"));
            }
            let link = helper(sh)?;
            let name = lock(&sh.inner).app_names.get(ts).cloned().unwrap_or_else(|| bundle.rsplit('.').next().unwrap_or(bundle).to_string());
            (link, name.clone(), name)
        }
    };
    let r = forward(sh, link, Some(agent), Some(ts), op, args, wait)?;
    let r = if op == "screenshot" {
        let n = {
            let mut inner = lock(&sh.inner);
            inner.next += 1;
            inner.next
        };
        let mw = args.get("max_width").and_then(Value::as_u64).unwrap_or(1280) as u32;
        image::write_screenshot(tmpdir, ts, n, &r, mw)?
    } else {
        remember(sh, ts, &r);
        r
    };
    if action == "close" {
        lock(&sh.inner).owners.remove(&(agent.to_string(), ts.to_string()));
    }
    let place = if place.is_empty() {
        lock(&sh.inner).seen.get(ts).map(|(u, _)| short_host(u)).unwrap_or_default()
    } else {
        place
    };
    driving(sh, agent, &what, &place);
    Ok(r)
}

fn ticker(sh: Arc<Shared>) {
    while !sh.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        let idle: Vec<String> = {
            let inner = lock(&sh.inner);
            inner.last_action.iter().filter(|(_, t)| t.elapsed() >= sh.opts.release_after).map(|(a, _)| a.clone()).collect()
        };
        for a in idle {
            release(&sh, &a);
        }
        let quiet = lock(&sh.inner).quiet_since;
        if let (Some(q), Some(limit)) = (quiet, sh.opts.idle_exit) {
            if q.elapsed() >= limit {
                log("idle: exit");
                sh.stop.store(true, Ordering::SeqCst);
            }
        }
    }
}
