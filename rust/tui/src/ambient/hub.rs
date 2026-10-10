//! The hub connection of the ambient core: one reader thread that
//! connects (`hello` sent by the connector), forwards the hub's lines,
//! says when the hub went away, and reconnects (the socket path stays the
//! same across hub restarts and version switches, like the TUI's
//! `sb/client.rs hub_reader`). The writer half is shared with the core.

use std::io::{self, BufRead, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// lost, or not reachable yet: the thread keeps trying
    Down,
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
    /// [`Hub::close`] said: the reader thread ends at its next turn
    closed: Arc<AtomicBool>,
    /// [`Hub::hold`] said: the reader doesn't connect until released
    held: Arc<(Mutex<bool>, Condvar)>,
}

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
        let closed = hub.closed.clone();
        let held = hub.held.clone();
        std::thread::spawn(move || {
            // Down is said once per loss, not at each failed attempt
            let mut down_said = false;
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
                    Err(_) => {
                        if !down_said {
                            down_said = true;
                            if tx.send(wrap(HubIn::Down)).is_err() {
                                return;
                            }
                        }
                        std::thread::sleep(retry);
                        continue;
                    }
                };
                let Ok(w) = stream.try_clone() else {
                    std::thread::sleep(retry);
                    continue;
                };
                if let Ok(mut slot) = writer.lock() {
                    if closed.load(Ordering::SeqCst) {
                        return;
                    }
                    *slot = Some(w);
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
                if tx.send(wrap(HubIn::Down)).is_err() {
                    return;
                }
                std::thread::sleep(retry);
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
}
