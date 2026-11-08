//! One sb-core process and its line (moved out of core.rs, hub-loop):
//! started on a free port, one JSON line in, one out, within a time
//! limit ([`Limits::of`]: by input kind, scaled by the machine's load);
//! killed with it. Every failure is a [`CoreError`], never a panic:
//! `Hub::call` (core_link.rs) restarts sb-core on a death or a late
//! answer (audit-hub finding 1: a Tick held the hub 38 min).

use crate::util::clip;
use serde_json::Value;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// One sb-core process and its connection.
pub struct CoreLink {
    pub(crate) child: Child,
    _out: BufReader<ChildStdout>,
    w: TcpStream,
    r: BufReader<TcpStream>,
    /// how long an input may take (tests shorten it)
    pub(crate) limits: Limits,
}

/// Why a call to sb-core failed: each is a death of sb-core for the hub
/// (it is killed and restarted), but `GaveUp`, the hub's stop.
#[derive(Debug)]
pub enum CoreError {
    /// it could not start (its binary, its banner, its port)
    Start(std::io::Error),
    /// its connection broke or closed
    Gone(std::io::Error),
    /// no whole answer within the input's limit
    Late { what: String, limit: Duration },
    /// an answer that is not one JSON line
    BadLine(String),
    /// restarted too often (REVIVE_LIMIT in REVIVE_WINDOW_MS): the hub stops
    GaveUp(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Start(e) => write!(f, "it could not start: {}", e),
            CoreError::Gone(e) => write!(f, "{}", e),
            CoreError::Late { what, limit } => write!(f, "no answer to {} within {} s", what, limit.as_secs_f64().round()),
            CoreError::BadLine(l) => write!(f, "a bad answer {}", l),
            CoreError::GaveUp(why) => write!(f, "{}", why),
        }
    }
}

impl From<CoreError> for std::io::Error {
    fn from(e: CoreError) -> std::io::Error {
        std::io::Error::other(format!("sb-core: {}", e))
    }
}

/// How long sb-core may take on one input, before the load: a step (a
/// Tick, a message) `step`; a batch (`replay_many`, `restore_photo`)
/// `step` plus its share by size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub step: Duration,
    /// per event of a `replay_many` (a 250 batch: +5 s)
    pub per_event: Duration,
    /// per MB of a `restore_photo` line
    pub per_mb: Duration,
    /// the load factor the limits scale by; None: the machine's, read at
    /// each input (crate::load::factor). A test fixes it: a limit that
    /// moves with the box's load makes a deadline test a flake.
    pub load: Option<f64>,
}

impl Default for Limits {
    /// His slowest healthy step is well under a second; a 112k-event
    /// replay batch of 250 under 0.1 s.
    fn default() -> Limits {
        Limits { step: Duration::from_secs(30), per_event: Duration::from_millis(20), per_mb: Duration::from_secs(1), load: None }
    }
}

impl Limits {
    /// The limit of input `v` (its line `bytes` long) on a machine at
    /// load `factor` (crate::load: 1 to 8): a loaded Mac is slow, not
    /// sb-core stuck. Pure.
    pub fn of(&self, v: &Value, bytes: usize, factor: f64) -> Duration {
        let base = match v["t"].as_str() {
            Some("replay_many") => self.step + self.per_event * v["evs"].as_array().map_or(0, |a| a.len()) as u32,
            Some("restore_photo") => self.step + self.per_mb.mul_f64(bytes as f64 / 1e6),
            _ => self.step,
        };
        base.mul_f64(factor.clamp(1.0, crate::load::MAX_FACTOR))
    }
}

impl CoreLink {
    /// Start sb-core on a free port. A port taken meanwhile (parallel
    /// hubs) is retried on another one.
    pub fn start(bin: &Path) -> Result<CoreLink, CoreError> {
        let mut last = CoreLink::try_start(bin);
        for _ in 1..5 {
            if last.is_ok() {
                break;
            }
            last = CoreLink::try_start(bin);
        }
        last.map_err(CoreError::Start)
    }

    fn try_start(bin: &Path) -> std::io::Result<CoreLink> {
        let port = std::net::TcpListener::bind("127.0.0.1:0")?
            .local_addr()?
            .port();
        let mut cmd = Command::new(bin);
        bise_home::env::for_child(bise_home::env::Child::Core, [("SB_CORE_PORT", port.to_string())]).apply(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // it lives as long as the hub: a pipe of a concurrent spawn must
        // not stay open in it (BISE-291)
        crate::procs::no_leaked_fds(&mut cmd);
        let mut child = cmd.spawn()?;
        let Some(stdout) = child.stdout.take() else { return Err(std::io::Error::other("no stdout")) };
        let mut out = BufReader::new(stdout);
        let mut banner = String::new();
        out.read_line(&mut banner)?;
        let conn = if banner.starts_with("sb-core on") {
            TcpStream::connect(("127.0.0.1", port))
        } else {
            Err(std::io::Error::other(format!("sb-core did not start: {:?}", banner)))
        };
        let w = match conn {
            Ok(w) => w,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        w.set_nodelay(true)?;
        let r = BufReader::new(w.try_clone()?);
        Ok(CoreLink {
            child,
            _out: out,
            w,
            r,
            limits: Limits::default(),
        })
    }

    /// One input line, one answer line, within the input's limit at the
    /// machine's load now. An error when sb-core is gone, late, or
    /// answers something that is not JSON: the link is dead after it
    /// (a late answer would come as the next input's).
    pub fn call(&mut self, v: &Value) -> Result<Value, CoreError> {
        let limit = self.send(v)?;
        self.answer_by(v, limit)
    }

    /// An input answered in lines (`view_all`, c6 (c)): every line up to
    /// the first without `more`. One limit for the whole answer, sized as
    /// the input's own (`Limits::of`, scaled by the load): the lines split
    /// the work of the one line they replace, they add none to it.
    pub fn call_lines(&mut self, v: &Value) -> Result<Vec<Value>, CoreError> {
        let limit = self.send(v)?;
        let mut out = vec![self.answer_by(v, limit)?];
        while out.last().is_some_and(|a| a["more"] == Value::Bool(true)) {
            out.push(self.answer_by(v, limit)?);
        }
        Ok(out)
    }

    /// Write input `v`: its limit, which runs from now to its answer's
    /// last line.
    fn send(&mut self, v: &Value) -> Result<Limit, CoreError> {
        let line = line_of(v);
        let limit = self.limits.of(v, line.len(), self.limits.load.unwrap_or_else(crate::load::factor));
        let end = Instant::now() + limit;
        self.w.set_write_timeout(Some(limit)).map_err(CoreError::Gone)?;
        match self.w.write_all(line.as_bytes()) {
            Err(e) if is_timeout(&e) => Err(late(v, limit)),
            r => r.map(|()| Limit { limit, end }).map_err(CoreError::Gone),
        }
    }

    /// One answer line to `v`, before its limit ends.
    fn answer_by(&mut self, v: &Value, l: Limit) -> Result<Value, CoreError> {
        let back = match read_line_by(&mut self.r, l.end) {
            Err(e) if is_timeout(&e) => return Err(late(v, l.limit)),
            r => r.map_err(CoreError::Gone)?,
        };
        serde_json::from_str(&back).map_err(|e| CoreError::BadLine(format!("{:?}: {}", clip(&back, 200), e)))
    }

    /// Kill sb-core (a late one before its restart; tests: under the hub).
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for CoreLink {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An input's limit and when it ends (its answer's last line by then).
#[derive(Clone, Copy)]
struct Limit {
    limit: Duration,
    end: Instant,
}

fn late(v: &Value, limit: Duration) -> CoreError {
    CoreError::Late { what: v["t"].as_str().unwrap_or("?").to_string(), limit }
}

fn is_timeout(e: &std::io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

/// One line from `r` before `end` (a TimedOut error after it), however
/// sb-core cuts it into packets; closed before its newline: an error.
fn read_line_by(r: &mut BufReader<TcpStream>, end: Instant) -> std::io::Result<String> {
    let mut buf = Vec::new();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(ErrorKind::TimedOut.into());
        }
        r.get_ref().set_read_timeout(Some(left))?;
        // read_until keeps what it read before an error in `buf`
        match r.read_until(b'\n', &mut buf) {
            Ok(_) if buf.ends_with(b"\n") => break,
            Ok(_) => return Err(std::io::Error::new(ErrorKind::UnexpectedEof, "its connection closed")),
            Err(e) if is_timeout(&e) || e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    String::from_utf8(buf).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
}

/// One input line for sb-core: its JSON with no byte >= 0x80, then a
/// newline. sb-core's reader (main.bend `conn`) decodes each recv on its
/// own: a multi-byte char cut by a recv boundary would read as U+FFFD
/// (~14 chars of his journal's message text at every boot, issue 36).
fn line_of(v: &Value) -> String {
    let mut line = bise_proto::ascii::ascii_json(v);
    line.push('\n');
    line
}

#[cfg(test)]
mod line_tests {
    use super::{line_of, Limits};
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn a_limit_is_by_kind_and_size_and_grows_with_the_load() {
        let l = Limits::default();
        let tick = json!({"t": "tick", "now": 1});
        assert_eq!(l.of(&tick, 20, 1.0), Duration::from_secs(30));
        let evs: Vec<_> = (0..250).map(|i| json!({"type": "x", "i": i})).collect();
        assert_eq!(l.of(&json!({"t": "replay_many", "evs": evs}), 110_000, 1.0), Duration::from_secs(35));
        assert_eq!(l.of(&json!({"t": "restore_photo", "st": {}}), 20_000_000, 1.0), Duration::from_secs(50));
        // a loaded Mac is slow, not sb-core stuck: the same input waits longer
        assert_eq!(l.of(&tick, 20, 4.0), Duration::from_secs(120));
        assert_eq!(l.of(&tick, 20, 100.0), Duration::from_secs(240), "at most MAX_FACTOR");
    }

    #[test]
    fn a_line_to_sb_core_never_holds_a_byte_past_ascii() {
        let text: String = "é─😀".repeat(20_000);
        let v = json!({"t": "replay_many", "evs": [{"type": "main_note", "text": text}]});
        let line = line_of(&v);
        assert!(line.bytes().all(|b| b < 0x80));
        assert!(line.ends_with('\n') && !line[..line.len() - 1].contains('\n'));
        assert_eq!(serde_json::from_str::<serde_json::Value>(line.trim_end()).unwrap(), v);
    }
}
