//! Connecting to the broker, starting it when none runs (the MCP server
//! and the native host relay both do), and one request/reply line.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::paths::Paths;

/// How a missing broker gets started: a detached `bise computer-use
/// broker` in the commands, an in-process one in tests.
pub type Starter = dyn Fn(&Paths) -> std::io::Result<()> + Send + Sync;

/// Start `exe computer-use broker` in its own session (it outlives the
/// browser or the agent that started it), stderr to the broker's log.
pub fn spawn_broker(exe: PathBuf) -> Box<Starter> {
    Box::new(move |paths: &Paths| {
        use std::os::unix::process::CommandExt;
        paths.ensure()?;
        let log = std::fs::OpenOptions::new().create(true).append(true).open(paths.log_file())?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["computer-use", "broker"])
            // the broker is no agent's (docs/issues/18): the hub's
            // cleanup must not kill it with the agent that started it,
            // and its own chain carries no tag
            .env(bise_peer::tags::ENV, "")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(log);
        // SAFETY: setsid only, between fork and exec
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        cmd.spawn().map(|_| ())
    })
}

/// A connection, started broker included: tries the socket, starts the
/// broker once, then waits up to 5 s for it.
pub fn connect(paths: &Paths, start: Option<&Starter>) -> std::io::Result<UnixStream> {
    if let Ok(s) = UnixStream::connect(paths.socket()) {
        return Ok(s);
    }
    let Some(start) = start else {
        return UnixStream::connect(paths.socket());
    };
    start(paths)?;
    let t0 = Instant::now();
    loop {
        match UnixStream::connect(paths.socket()) {
            Ok(s) => return Ok(s),
            Err(e) if t0.elapsed() > Duration::from_secs(5) => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// A line-oriented connection (an agent's or a command's).
pub struct Conn {
    w: UnixStream,
    r: BufReader<UnixStream>,
    next: u64,
}

impl Conn {
    /// Connect and say hello.
    pub fn open(paths: &Paths, start: Option<&Starter>, hello: &Value) -> std::io::Result<Conn> {
        let s = connect(paths, start)?;
        let mut c = Conn { r: BufReader::new(s.try_clone()?), w: s, next: 0 };
        c.send(hello)?;
        Ok(c)
    }

    /// A command connection, on the commands' socket (docs/issues/18):
    /// the broker serves it to the user's processes only.
    pub fn ctl(paths: &Paths) -> std::io::Result<Conn> {
        Conn::ctl_on(UnixStream::connect(paths.ctl_socket())?)
    }

    /// [`Conn::ctl`] whose every read and write waits at most `wait` (a
    /// caller that must not hang: the hub's and the TUI's /stop).
    pub fn ctl_within(paths: &Paths, wait: std::time::Duration) -> std::io::Result<Conn> {
        let s = UnixStream::connect(paths.ctl_socket())?;
        s.set_read_timeout(Some(wait))?;
        s.set_write_timeout(Some(wait))?;
        Conn::ctl_on(s)
    }

    fn ctl_on(s: UnixStream) -> std::io::Result<Conn> {
        let mut c = Conn { r: BufReader::new(s.try_clone()?), w: s, next: 0 };
        c.send(&json!({"op": "hello", "role": "ctl"}))?;
        Ok(c)
    }

    pub fn send(&mut self, v: &Value) -> std::io::Result<()> {
        let mut line = v.to_string();
        line.push('\n');
        self.w.write_all(line.as_bytes())?;
        self.w.flush()
    }

    /// One request; `Ok(Ok(result))`, `Ok(Err(C1 error))`, or the
    /// connection's error.
    pub fn call(&mut self, op: &str, args: &Value) -> std::io::Result<Result<Value, Value>> {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"id": id, "op": op, "args": args}))?;
        loop {
            let mut line = String::new();
            if self.r.read_line(&mut line)? == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the broker closed the connection"));
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if v.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            return Ok(if v.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(v.get("result").cloned().unwrap_or(Value::Null))
            } else {
                Err(v.get("error").cloned().unwrap_or(Value::Null))
            });
        }
    }
}
