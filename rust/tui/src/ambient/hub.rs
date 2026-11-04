//! The hub connection of the ambient core: one reader thread that
//! connects (`hello` sent by the connector), forwards the hub's lines,
//! says when the hub went away, and reconnects (the socket path stays the
//! same across hub restarts and version switches, like the TUI's
//! `sb/client.rs hub_reader`). The writer half is shared with the core.

use std::io::{self, BufRead, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Opens a connection to the hub, its first line already sent when the
/// connector says one (`{"op":"hello"}`; a project's connection says
/// none: the core's first line is JSON-RPC's `initialize`). Called again
/// after each loss (the first call may start the hub).
pub type Connect = Box<dyn FnMut() -> io::Result<UnixStream> + Send>;

/// What the reader thread tells the core.
#[derive(Debug, PartialEq)]
pub enum HubIn {
    /// one JSON line of the hub
    Line(String),
    /// connected (the hello replay follows, then `ready`)
    Up,
    /// lost, or not reachable yet: the thread keeps trying. `Some`: the
    /// connector's error, in its words (a hub that didn't start:
    /// `client::open`'s cause and log; a core without its runtime: why),
    /// said once per spell, then the pauses back off (1/2/5/10 s)
    Down(Option<String>),
    /// the hub refused this connection (bise_proto HubEv::Refused, its
    /// words): the thread has ended, it never connects again (the hub's
    /// verdict is about this process: retrying alone wouldn't change it)
    Refused(String),
}

/// The hub's refusal of this connection (docs/issues/16): the older
/// typed `refused`, or JSON-RPC's REFUSED error to `initialize`.
fn refusal(l: &str) -> Option<String> {
    if l.contains("\"refused\"") {
        if let Ok(bise_proto::hub::HubEv::Refused { error }) = bise_proto::hub::HubEv::decode(l) {
            return Some(error);
        }
    }
    if l.contains(&bise_proto::rpc::code::REFUSED.to_string()) {
        if let Ok(bise_proto::rpc::Message::Response(r)) = bise_proto::rpc::Message::read(l) {
            return r.error.filter(|e| e.code == bise_proto::rpc::code::REFUSED).map(|e| e.message);
        }
    }
    None
}

/// The writer half: None while the hub is away.
#[derive(Clone, Default)]
pub struct Hub {
    writer: Arc<Mutex<Option<UnixStream>>>,
    /// the connections made so far: bumped with the writer, under its
    /// lock, at each connect ([`Hub::epoch`], [`Hub::send_on`])
    epoch: Arc<AtomicU64>,
    /// [`Hub::close`] said: the reader thread ends at its next turn
    closed: Arc<AtomicBool>,
    /// [`Hub::hold`] said: the reader doesn't connect until released
    held: Arc<(Mutex<bool>, Condvar)>,
    /// [`Hub::backoff`] said: the pauses between attempts grow
    slow: Arc<AtomicBool>,
}

/// The pauses of a reader that backs off ([`Hub::backoff`]): 1 s, 2 s,
/// 5 s, then every 10 s.
const BACKOFF: [Duration; 4] = [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10)];

impl Hub {
    /// Start the reader thread; every [`HubIn`] goes through `wrap` into
    /// `tx`. `retry`: the pause between two connection attempts.
    pub fn start<T: Send + 'static>(
        mut connect: Connect,
        tx: Sender<T>,
        wrap: impl Fn(HubIn) -> T + Send + 'static,
        retry: Duration,
    ) -> Hub {
        let hub = Hub::default();
        let writer = hub.writer.clone();
        let epoch = hub.epoch.clone();
        let closed = hub.closed.clone();
        let held = hub.held.clone();
        let slow = hub.slow.clone();
        std::thread::spawn(move || {
            // Down is said once per loss, not at each failed attempt
            let mut down_said = false;
            // the pause before the next attempt: `retry`, or the backoff's
            // next step while it backs off (its count reset when it doesn't)
            let mut misses = 0usize;
            // a connector that failed with a cause (a start that failed):
            // backs off like `slow` until it connects
            let mut failed = false;
            let mut pause = move |failed: bool| {
                if failed || slow.load(Ordering::SeqCst) {
                    std::thread::sleep(BACKOFF[misses.min(BACKOFF.len() - 1)]);
                    misses += 1;
                } else {
                    misses = 0;
                    std::thread::sleep(retry);
                }
            };
            loop {
                // held: no attempt (no reconnect loop) until released
                {
                    let (on, cv) = &*held;
                    let mut h = on.lock().unwrap_or_else(|e| e.into_inner());
                    while *h && !closed.load(Ordering::SeqCst) {
                        h = cv.wait(h).unwrap_or_else(|e| e.into_inner());
                    }
                }
                if closed.load(Ordering::SeqCst) {
                    return;
                }
                let stream = match connect() {
                    Ok(s) => s,
                    Err(e) => {
                        // a cause the user can read: an error the connector
                        // made (`Other`), never a bare refused or missing socket
                        let cause = (e.kind() == io::ErrorKind::Other).then(|| e.to_string());
                        if !down_said || (cause.is_some() && !failed) {
                            down_said = true;
                            failed = cause.is_some();
                            if tx.send(wrap(HubIn::Down(cause))).is_err() {
                                return;
                            }
                        }
                        pause(failed);
                        continue;
                    }
                };
                failed = false;
                let Ok(w) = stream.try_clone() else {
                    pause(false);
                    continue;
                };
                if let Ok(mut slot) = writer.lock() {
                    if closed.load(Ordering::SeqCst) {
                        return;
                    }
                    *slot = Some(w);
                    epoch.fetch_add(1, Ordering::SeqCst);
                }
                if tx.send(wrap(HubIn::Up)).is_err() {
                    return;
                }
                let mut r = io::BufReader::new(stream);
                let mut line = String::new();
                loop {
                    line.clear();
                    match r.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let l = line.trim_end();
                            // the typed refusal (docs/issues/16): said once,
                            // then this reader ends without reconnecting
                            if let Some(error) = refusal(l) {
                                if let Ok(mut slot) = writer.lock() {
                                    *slot = None;
                                }
                                let _ = tx.send(wrap(HubIn::Refused(error)));
                                return;
                            }
                            if tx.send(wrap(HubIn::Line(l.to_string()))).is_err() {
                                return;
                            }
                        }
                    }
                }
                if let Ok(mut slot) = writer.lock() {
                    *slot = None;
                }
                if closed.load(Ordering::SeqCst) {
                    return;
                }
                down_said = true;
                if tx.send(wrap(HubIn::Down(None))).is_err() {
                    return;
                }
                pause(false);
            }
        });
        hub
    }

    /// No more of this hub: the connection closes (the hub's idle-exit
    /// counts one client less) and the reader thread ends, saying nothing.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.held.1.notify_all();
        if let Ok(mut slot) = self.writer.lock() {
            if let Some(s) = slot.take() {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    /// Back off (`on`): the reader's pauses between attempts grow (1 s,
    /// 2 s, 5 s, then every 10 s) instead of `retry` (the home hub whose
    /// move said Late: it may still come up, it is not polled every
    /// 500 ms; core/home_switch.rs). Off: `retry` again.
    pub fn backoff(&self, on: bool) {
        self.slow.store(on, Ordering::SeqCst);
    }

    /// Hold the connection (`on`): the one open closes and the reader
    /// makes no attempt until released (the home hub whose move failed:
    /// no reconnect every `retry`, core/home_switch.rs); released, it
    /// connects again at once.
    pub fn hold(&self, on: bool) {
        let (flag, cv) = &*self.held;
        *flag.lock().unwrap_or_else(|e| e.into_inner()) = on;
        cv.notify_all();
        if on {
            if let Ok(slot) = self.writer.lock() {
                if let Some(s) = slot.as_ref() {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                }
            }
        }
    }

    /// One request line to the hub; false when it is away or the write
    /// failed (nothing was sent).
    pub fn send(&self, v: &serde_json::Value) -> bool {
        let Ok(mut slot) = self.writer.lock() else { return false };
        let Some(s) = slot.as_mut() else { return false };
        let mut line = v.to_string();
        line.push('\n');
        s.write_all(line.as_bytes()).is_ok()
    }

    /// The connection the writer is on now (0: none made yet): a
    /// connection's `initialize` is written on it, by [`Hub::send`].
    pub fn epoch(&self) -> u64 {
        // read under the writer's lock: the reader bumps it there
        let _slot = self.writer.lock();
        self.epoch.load(Ordering::SeqCst)
    }

    /// [`Hub::send`] on connection `epoch` only: false, nothing written,
    /// when the reader has connected again since (the core welcomed the
    /// older one: a command must not reach a hub before its `initialize`,
    /// fast-loop, architect m_53375).
    pub fn send_on(&self, epoch: u64, v: &serde_json::Value) -> bool {
        let Ok(mut slot) = self.writer.lock() else { return false };
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return false;
        }
        let Some(s) = slot.as_mut() else { return false };
        let mut line = v.to_string();
        line.push('\n');
        s.write_all(line.as_bytes()).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    /// A typed refusal then EOF: Up, Refused with the hub's words, and
    /// the reader ends: it never connects again (connect called once).
    #[test]
    fn a_refusal_ends_the_reader_without_reconnecting() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let connect: Connect = Box::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
            let (core_end, mut hub_end) = UnixStream::pair()?;
            writeln!(hub_end, "{}", serde_json::json!({"ev": "refused", "error": "an agent's process"}))?;
            drop(hub_end);
            Ok(core_end)
        });
        let (tx, rx) = mpsc::channel();
        let hub = Hub::start(connect, tx, |h| h, Duration::from_millis(5));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), HubIn::Up);
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), HubIn::Refused("an agent's process".into()));
        // the thread ended: its sender is gone, nothing else comes
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "connected once");
        assert!(!hub.send(&serde_json::json!({"cmd": "x"})), "nothing goes to a hub that refused");
    }

    /// A start that fails says why, once (architect m_18354): the cause
    /// rides on `Down`, a bare missing socket says none, and the reader
    /// backs off (1 s first) instead of retrying every `retry`.
    #[test]
    fn a_failed_start_says_its_cause_once_then_backs_off() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let connect: Connect = Box::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::other("the hub did not start: sb-core: no such file (its log: /h/hub.err)"))
        });
        let (tx, rx) = mpsc::channel();
        let hub = Hub::start(connect, tx, |h| h, Duration::from_millis(5));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            HubIn::Down(Some("the hub did not start: sb-core: no such file (its log: /h/hub.err)".into()))
        );
        // said once; with a 5 ms retry it would try ~100 times in 500 ms
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "said once");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "backed off: no second attempt within 1 s");
        hub.close();

        // no socket yet (a plain connect's NotFound): Down with no cause
        let connect: Connect = Box::new(|| Err(io::Error::from(io::ErrorKind::NotFound)));
        let (tx, rx) = mpsc::channel();
        let hub = Hub::start(connect, tx, |h| h, Duration::from_millis(5));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), HubIn::Down(None));
        hub.close();
    }
}
