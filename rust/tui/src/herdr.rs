//! herdr activity-state reporter — the vendor self-report protocol
//! (herdr's `pane.report_agent` / `pane.release_agent` over its unix
//! socket). When bise runs inside a herdr pane (`HERDR_ENV=1`, and a
//! `HERDR_PANE_ID` + `HERDR_SOCKET_PATH` are present), bise reports the
//! workspace's aggregate state (blocked > working > idle) as it changes,
//! and releases the pane when bise exits. Everywhere else this is a
//! no-op: nothing is read, nothing is sent.
//!
//! The reporter is nonblocking, bounded and failure-isolated: a
//! background worker owns the socket; callers only set the latest state
//! in a coalesced slot (a burst of changes sends only the newest); one
//! request at a time with a short socket timeout; a missing socket or a
//! broken write is dropped and never stalls bise.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The states herdr accepts for `pane.report_agent`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
    Idle,
    Working,
    Blocked,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Working => "working",
            State::Blocked => "blocked",
        }
    }
}

/// How long one socket write/read may take: a herdr outage must never
/// stall bise, so every request is bounded by this.
const SOCKET_TIMEOUT: Duration = Duration::from_millis(500);

/// The env herdr injects into every process it launches in a pane.
const ENV_HERDR: &str = "HERDR_ENV";
const ENV_PANE_ID: &str = "HERDR_PANE_ID";
const ENV_SOCKET: &str = "HERDR_SOCKET_PATH";

/// The stable, unique integration source (must not start with `herdr:`).
const SOURCE: &str = "bise";
/// The agent name the user sees (free text, normalized by herdr).
const AGENT: &str = "bise";

/// The reporter config: where the pane's report goes.
#[derive(Clone)]
struct Config {
    pane_id: String,
    socket: String,
}

impl Config {
    fn from_env() -> Option<Config> {
        Config::from(&|k| std::env::var(k).ok())
    }

    /// `get` is the environment (a closure for tests): the three herdr
    /// vars, read together.
    fn from(get: &dyn Fn(&str) -> Option<String>) -> Option<Config> {
        let env = get(ENV_HERDR);
        let pane_id = get(ENV_PANE_ID);
        let socket = get(ENV_SOCKET);
        if !enabled(env.as_deref(), pane_id.as_deref(), socket.as_deref()) {
            return None;
        }
        Some(Config {
            pane_id: pane_id.expect("enabled checked it").trim().to_string(),
            socket: socket.expect("enabled checked it").trim().to_string(),
        })
    }
}

/// True when bise runs inside a herdr pane: the env herdr injects is
/// present and complete. Anything else (a plain terminal, a script, the
/// hub started outside herdr) reports nothing.
fn enabled(herdr_env: Option<&str>, pane_id: Option<&str>, socket: Option<&str>) -> bool {
    herdr_env == Some("1")
        && pane_id.is_some_and(|p| !p.trim().is_empty())
        && socket.is_some_and(|s| !s.trim().is_empty())
}

/// What actually delivers one newline-terminated JSON request to herdr.
/// Factored out so tests can swap in a fake transport; production opens
/// one unix socket per request (like herdr's own hooks).
trait Transport: Send + Sync {
    /// Deliver one newline-terminated JSON line; true if it reached herdr.
    fn send(&self, line: &str) -> bool;
}

struct SocketTransport {
    socket: String,
}

impl Transport for SocketTransport {
    fn send(&self, line: &str) -> bool {
        let Ok(mut stream) = UnixStream::connect(&self.socket) else {
            return false;
        };
        let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));
        let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
        if stream.write_all(line.as_bytes()).is_err() {
            return false;
        }
        // Drain herdr's one-line answer (or the EOF on its close) so the
        // request is complete before the socket goes, like the hooks.
        let mut sink = [0u8; 512];
        let _ = stream.read(&mut sink);
        true
    }
}

/// The worker's shared state: one coalesced pending slot + the last sent.
#[derive(Default)]
struct Slot {
    last_sent: Option<State>,
    pending: Option<State>,
}

struct Shared {
    slot: Mutex<Slot>,
    cv: Condvar,
    /// Set when bise exits: the worker sends `release-agent` and stops.
    released: AtomicBool,
    /// Monotonic per-source sequence (herdr ignores seq <= the last).
    seq: AtomicU64,
    config: Config,
    transport: Arc<dyn Transport>,
}

/// The reporter: a background worker with one coalesced pending slot.
/// `None` (or a no-op) unless bise runs inside a herdr pane.
pub(crate) struct Reporter {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Reporter {
    /// A live reporter when bise runs inside a herdr pane; `None`
    /// everywhere else (no thread is started outside herdr).
    pub(crate) fn spawn() -> Option<Reporter> {
        // Never a real reporter in a test build: a contaminated parent
        // environment (bise's own tests running inside a herdr pane) must
        // not drive the real herdr socket. The reporter's own tests build
        // it directly (see `reporter_for_test`), so this gate loses nothing.
        if cfg!(test) {
            return None;
        }
        let config = Config::from_env()?;
        Some(Reporter::start(
            config.clone(),
            Arc::new(SocketTransport { socket: config.socket }),
        ))
    }

    /// The testable core: a reporter over any transport.
    fn start(config: Config, transport: Arc<dyn Transport>) -> Reporter {
        let shared = Arc::new(Shared {
            slot: Mutex::new(Slot::default()),
            cv: Condvar::new(),
            released: AtomicBool::new(false),
            seq: AtomicU64::new(now_nanos()),
            config,
            transport,
        });
        let worker = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("herdr-report".into())
                .spawn(move || worker(&shared))
                .ok()
        };
        Reporter { shared, worker }
    }

    /// A reporter over a real unix socket, for tests that stand up a fake
    /// herdr server (never the real one): the dispatch-level regression
    /// observes report/release over this socket.
    #[cfg(test)]
    pub(crate) fn reporter_for_test(socket: &std::path::Path, pane_id: &str) -> Reporter {
        let config = Config {
            pane_id: pane_id.to_string(),
            socket: socket.to_string_lossy().into_owned(),
        };
        Reporter::start(
            config.clone(),
            Arc::new(SocketTransport { socket: config.socket }),
        )
    }

    /// Report the aggregate state. Nonblocking and coalesced: callers
    /// only set the latest state; a burst of changes sends the newest.
    /// Never panics and never does socket I/O on the caller's thread.
    pub(crate) fn report(&self, state: State) {
        let Ok(mut slot) = self.shared.slot.lock() else {
            return;
        };
        if slot.pending == Some(state) {
            return;
        }
        if slot.pending.is_none() && slot.last_sent == Some(state) {
            return;
        }
        slot.pending = Some(state);
        drop(slot);
        self.shared.cv.notify_one();
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        self.shared.released.store(true, Ordering::SeqCst);
        self.shared.cv.notify_all();
        if let Some(worker) = self.worker.take() {
            // The worker is bounded by SOCKET_TIMEOUT per request; join so
            // the release-agent lands before the pane's process goes.
            let _ = worker.join();
        }
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn next_seq(shared: &Shared) -> u64 {
    shared.seq.fetch_add(1, Ordering::SeqCst)
}

/// One request's newline-terminated JSON line (the exact herdr shape).
fn request_line(shared: &Shared, method: &str, params: serde_json::Value) -> String {
    let id = format!("{SOURCE}:{:x}", next_seq(shared));
    let request = serde_json::json!({
        "id": id,
        "method": method,
        "params": params,
    });
    let mut line = request.to_string();
    line.push('\n');
    line
}

fn state_params(shared: &Shared, state: State) -> serde_json::Value {
    serde_json::json!({
        "pane_id": shared.config.pane_id,
        "source": SOURCE,
        "agent": AGENT,
        "state": state.as_str(),
        "seq": next_seq(shared),
    })
}

fn release_params(shared: &Shared) -> serde_json::Value {
    serde_json::json!({
        "pane_id": shared.config.pane_id,
        "source": SOURCE,
        "agent": AGENT,
        "seq": next_seq(shared),
    })
}

fn worker(shared: &Arc<Shared>) {
    loop {
        let state = {
            let mut slot = shared.slot.lock().unwrap();
            while slot.pending.is_none() && !shared.released.load(Ordering::SeqCst) {
                slot = shared.cv.wait(slot).unwrap();
            }
            if shared.released.load(Ordering::SeqCst) {
                None
            } else {
                slot.pending.take()
            }
        };
        match state {
            None => {
                // bise exited: clear the pane's state, then stop.
                let line = request_line(shared, "pane.release_agent", release_params(shared));
                let _ = shared.transport.send(&line);
                return;
            }
            Some(state) => {
                // A duplicate that raced the caller's dedup check (the
                // caller set it before `last_sent` caught up): skip, never
                // resend a state the pane already has.
                if shared.slot.lock().unwrap().last_sent == Some(state) {
                    continue;
                }
                let line = request_line(shared, "pane.report_agent", state_params(shared, state));
                if shared.transport.send(&line) {
                    let mut slot = shared.slot.lock().unwrap();
                    slot.last_sent = Some(state);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, RecvTimeoutError};

    /// A fake transport: every sent line lands in a channel. `gated`
    /// holds a send until the test opens the gate (to observe
    /// coalescing); `fail` drops every send (to observe failure
    /// isolation). All knobs are shared so the test controls them after
    /// the transport has moved into the reporter.
    struct FakeTransport {
        tx: mpsc::Sender<String>,
        fail: Arc<AtomicBool>,
        gated: Arc<AtomicBool>,
        gate: Arc<(Mutex<bool>, Condvar)>,
        entered: mpsc::Sender<()>,
    }

    struct FakeHarness {
        rx: mpsc::Receiver<String>,
        fail: Arc<AtomicBool>,
        gated: Arc<AtomicBool>,
        gate: Arc<(Mutex<bool>, Condvar)>,
        entered_rx: mpsc::Receiver<()>,
    }

    impl FakeHarness {
        fn new() -> (FakeHarness, Arc<FakeTransport>) {
            let (tx, rx) = mpsc::channel();
            let (entered, entered_rx) = mpsc::channel();
            let fail = Arc::new(AtomicBool::new(false));
            let gated = Arc::new(AtomicBool::new(false));
            let gate: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
            let transport = Arc::new(FakeTransport {
                tx,
                fail: fail.clone(),
                gated: gated.clone(),
                gate: gate.clone(),
                entered,
            });
            (FakeHarness { rx, fail, gated, gate, entered_rx }, transport)
        }

        /// The next line, or panic on a 2 s timeout.
        fn recv(&self) -> String {
            self.rx
                .recv_timeout(Duration::from_secs(2))
                .expect("expected a sent line")
        }

        /// True when nothing arrives within `wait` (a dedup probe).
        fn quiet(&self, wait: Duration) -> bool {
            matches!(self.rx.recv_timeout(wait), Err(RecvTimeoutError::Timeout))
        }

        /// Wait until a send is blocked in the gate (so a burst can land
        /// while it is in flight — deterministic coalescing).
        fn wait_gate_entered(&self) {
            self.entered_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("expected a send to enter the gate");
        }

        fn open_gate(&self) {
            let (lock, cv) = &*self.gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cv.notify_all();
        }

        fn set_gated(&self, gated: bool) {
            self.gated.store(gated, Ordering::SeqCst);
        }

        fn set_fail(&self, fail: bool) {
            self.fail.store(fail, Ordering::SeqCst);
        }
    }

    impl Transport for FakeTransport {
        fn send(&self, line: &str) -> bool {
            if self.fail.load(Ordering::SeqCst) {
                return false;
            }
            if self.gated.load(Ordering::SeqCst) {
                // tell the test a send is now blocked here
                let _ = self.entered.send(());
                let (lock, cv) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cv.wait(open).unwrap();
                }
            }
            self.tx.send(line.to_string()).is_ok()
        }
    }

    fn reporter(config: Config, transport: Arc<dyn Transport>) -> Reporter {
        Reporter::start(config, transport)
    }

    fn cfg() -> Config {
        Config { pane_id: "w1:p1".into(), socket: "/tmp/herdr-test.sock".into() }
    }

    fn method_of(line: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        v["method"].as_str().unwrap().to_string()
    }

    fn params_of(line: &str) -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        v["params"].clone()
    }

    fn seq_of(line: &str) -> u64 {
        params_of(line)["seq"].as_u64().unwrap()
    }

    #[test]
    fn gating_requires_the_herdr_env_and_a_pane_and_socket() {
        assert!(!enabled(None, Some("w1:p1"), Some("/sock")));
        assert!(!enabled(Some("0"), Some("w1:p1"), Some("/sock")));
        assert!(!enabled(Some("1"), None, Some("/sock")));
        assert!(!enabled(Some("1"), Some("w1:p1"), None));
        assert!(!enabled(Some("1"), Some("  "), Some("/sock")));
        assert!(!enabled(Some("1"), Some("w1:p1"), Some("  ")));
        assert!(enabled(Some("1"), Some("w1:p1"), Some("/sock")));
    }

    #[test]
    fn config_reads_the_three_env_vars_together() {
        let env = |k: &str| match k {
            ENV_HERDR => Some("1".to_string()),
            ENV_PANE_ID => Some("w2:p3".to_string()),
            ENV_SOCKET => Some(" sock ".to_string()),
            _ => None,
        };
        let c = Config::from(&env).unwrap();
        assert_eq!(c.pane_id, "w2:p3");
        assert_eq!(c.socket, "sock");
        // any one missing: no config
        assert!(Config::from(&|k| if k == ENV_HERDR { Some("1".into()) } else { None }).is_none());
    }

    /// The emitted request is exactly herdr's `pane.report_agent` shape:
    /// newline-terminated JSON with the required params, a non-empty id,
    /// a valid state and a monotonic seq.
    #[test]
    fn an_emitted_report_conforms_to_the_herdr_protocol() {
        let (harness, transport) = FakeHarness::new();
        let r = reporter(cfg(), transport);
        r.report(State::Working);
        let line = harness.recv();
        assert!(line.ends_with('\n'), "newline-terminated");
        let p = params_of(&line);
        assert_eq!(method_of(&line), "pane.report_agent");
        assert_eq!(p["pane_id"], "w1:p1");
        assert_eq!(p["source"], "bise");
        assert_eq!(p["agent"], "bise");
        assert_eq!(p["state"], "working");
        assert!(p["seq"].as_u64().is_some());
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert!(!v["id"].as_str().unwrap().is_empty());
        drop(r);
    }

    /// States are emitted in order with strictly increasing seq; the
    /// source never starts with the reserved `herdr:` prefix.
    #[test]
    fn seq_increases_and_source_is_never_herdr_owned() {
        let (harness, transport) = FakeHarness::new();
        let r = reporter(cfg(), transport);
        let mut seqs = Vec::new();
        for state in [State::Working, State::Blocked, State::Idle] {
            r.report(state);
            let line = harness.recv();
            assert_eq!(method_of(&line), "pane.report_agent");
            let p = params_of(&line);
            assert!(!p["source"].as_str().unwrap().starts_with("herdr:"));
            seqs.push(seq_of(&line));
        }
        assert!(seqs.windows(2).all(|w| w[1] > w[0]), "{seqs:?}");
        drop(r);
    }

    /// Reporting the same state twice sends it once (dedup on the last
    /// sent, not just the pending slot).
    #[test]
    fn repeated_states_are_deduped() {
        let (harness, transport) = FakeHarness::new();
        let r = reporter(cfg(), transport);
        r.report(State::Working);
        assert_eq!(params_of(&harness.recv())["state"], "working");
        r.report(State::Working);
        assert!(harness.quiet(Duration::from_millis(150)), "a repeated state must not resend");
        r.report(State::Idle);
        assert_eq!(params_of(&harness.recv())["state"], "idle");
        drop(r);
    }

    /// A burst while a send is in flight keeps only the newest state.
    #[test]
    fn a_burst_of_changes_is_coalesced_to_the_newest() {
        let (harness, transport) = FakeHarness::new();
        harness.set_gated(true);
        let r = reporter(cfg(), transport);
        r.report(State::Working);
        // wait until that first send is actually blocked in the gate
        harness.wait_gate_entered();
        // a burst lands while the first is still in flight
        r.report(State::Blocked);
        r.report(State::Idle);
        harness.open_gate(); // the first (working) goes through
        assert_eq!(params_of(&harness.recv())["state"], "working");
        // the pending slot coalesced blocked->idle: only idle follows
        assert_eq!(params_of(&harness.recv())["state"], "idle");
        assert!(harness.quiet(Duration::from_millis(150)), "blocked was coalesced away");
        drop(r);
    }

    /// A broken transport (missing socket, failed write) is dropped: the
    /// reporter does not panic and a later state still sends.
    #[test]
    fn a_failed_send_does_not_break_later_reports() {
        let (harness, transport) = FakeHarness::new();
        harness.set_fail(true);
        let r = reporter(cfg(), transport);
        r.report(State::Working);
        // the send failed and was dropped; the reporter is still alive
        assert!(harness.quiet(Duration::from_millis(150)));
        harness.set_fail(false);
        r.report(State::Idle);
        assert_eq!(params_of(&harness.recv())["state"], "idle");
        drop(r);
    }

    /// On drop (bise exiting) the worker sends `pane.release_agent`.
    #[test]
    fn drop_releases_the_pane() {
        let (harness, transport) = FakeHarness::new();
        {
            let r = reporter(cfg(), transport);
            r.report(State::Working);
            assert_eq!(method_of(&harness.recv()), "pane.report_agent");
        } // dropped here
        let line = harness.recv();
        assert_eq!(method_of(&line), "pane.release_agent");
        let p = params_of(&line);
        assert_eq!(p["pane_id"], "w1:p1");
        assert_eq!(p["source"], "bise");
        assert_eq!(p["agent"], "bise");
    }

    /// A real unix socket as the fake herdr: the production transport
    /// delivers the exact line (protocol proof without a fake transport).
    #[test]
    fn the_socket_transport_delivers_the_json_line() {
        use std::io::BufRead;
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("herdr-sock-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("herdr.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let transport: Arc<dyn Transport> = Arc::new(SocketTransport { socket: path.to_string_lossy().into_owned() });
        let r = reporter(cfg(), transport);
        r.report(State::Blocked);
        let (stream, _) = listener.accept().unwrap();
        let mut buf = String::new();
        let _ = std::io::BufReader::new(stream.try_clone().unwrap()).read_line(&mut buf);
        assert_eq!(method_of(&buf), "pane.report_agent");
        assert_eq!(params_of(&buf)["state"], "blocked");
        drop(r);
        let (stream2, _) = listener.accept().unwrap();
        let mut buf2 = String::new();
        let _ = std::io::BufReader::new(stream2.try_clone().unwrap()).read_line(&mut buf2);
        assert_eq!(method_of(&buf2), "pane.release_agent");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
