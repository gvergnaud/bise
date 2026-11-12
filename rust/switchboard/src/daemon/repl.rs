//! The REPL processes of the agents: spawn one and supervise it, adopt
//! one a previous hub left running, and pump its wire log to the shell
//! (as `Msg`s). Each runs on its own thread; only messages go back.

use super::{log_line, Msg};
use crate::paths::Paths;
use crate::util::clip;
use serde_json::{json, Value};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Duration;

/// What a hub needs to reconnect to a running REPL (`repl.json`).
pub(super) struct ReplInfo {
    pub(super) pid: u32,
    pub(super) port: u16,
    pub(super) bin: PathBuf,
    steer: String,
    interrupt: String,
    /// its say port's token (`BISE_REPL_TOKEN`); none: a REPL an older
    /// hub started, which takes no hello
    token: Option<String>,
}

/// A REPL's say port and the token its connections open with.
pub(super) struct SayPort {
    pub(super) port: u16,
    pub(super) token: String,
}

/// The first line the hub writes on a connection to a REPL it started
/// with `token` (repl-core's serve_gate: anything else is refused, so a
/// probe, a local tool or a browser's request never speaks as him).
pub(super) fn hello(token: &str) -> String {
    format!("hello {token}\n")
}

/// A connection to a REPL is the hub's once its hello is written.
fn say_hello(mut stream: &TcpStream, token: Option<&str>) -> std::io::Result<()> {
    use std::io::Write;
    match token {
        Some(t) => stream.write_all(hello(t).as_bytes()),
        None => Ok(()),
    }
}

/// `repl.json`, the private file an adopting hub reads (its token).
fn write_repl_json(path: &Path, v: &Value) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    if let Ok(mut f) = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
        let _ = f.write_all(v.to_string().as_bytes());
    }
}

pub(super) use crate::procs::terminate as kill_pid;
use crate::procs::alive as pid_alive;

/// `pid` runs a REPL binary (`repl-live`, `repl-scripted`): its path says
/// so, as its `ps` command line did; read from the kernel
/// (bise_peer::table::exe), no `ps` process on the hub's loop.
fn is_repl(pid: u32) -> bool {
    bise_peer::table::exe(pid).is_some_and(|p| p.to_string_lossy().contains("repl-"))
}

/// A REPL a previous hub left running, when it can be adopted: its
/// `repl.json` names a live REPL process, and it writes a wire log.
pub(super) fn adoptable(adir: &Path) -> Option<ReplInfo> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(adir.join("repl.json")).ok()?).ok()?;
    let r = ReplInfo {
        pid: v.get("pid")?.as_u64()? as u32,
        port: v.get("port")?.as_u64()? as u16,
        bin: PathBuf::from(v.get("bin").and_then(|b| b.as_str()).unwrap_or("")),
        steer: v.get("steer")?.as_str()?.to_string(),
        interrupt: v.get("interrupt")?.as_str()?.to_string(),
        token: v.get("token").and_then(|t| t.as_str()).map(str::to_string),
    };
    (adir.join("wire.log").exists() && pid_alive(r.pid) && is_repl(r.pid)).then_some(r)
}

/// Was a turn running at `offset` of the wire log? (the last
/// `turn_started` comes after the last `--- idle`)
pub(super) fn busy_at(wire: &Path, offset: u64) -> bool {
    // its last turn line before `offset`, read from the end (wire::find_back)
    crate::wire::find_back(wire, offset, |l| {
        if l == "--- idle" {
            Some(false)
        } else if crate::wire::parse(l) == crate::wire::Wire::TurnStarted {
            Some(true)
        } else {
            None
        }
    })
    .unwrap_or(false)
}

/// How a REPL process ended, as a fact for sb-core (hub/stall.bend
/// decides what it does to the crash count): by SIGTERM, SIGINT or
/// SIGHUP, someone asked it to stop (the hub's stop, `sb restart`, a
/// teardown, a `kill`): `Stopped`. Any other end (a crash signal, a
/// SIGKILL, an exit code) is `Died`.
pub(super) fn gone_of(status: &std::process::ExitStatus) -> super::repl_starts::Gone {
    use std::os::unix::process::ExitStatusExt;
    match status.signal() {
        Some(libc::SIGTERM | libc::SIGINT | libc::SIGHUP) => super::repl_starts::Gone::Stopped,
        _ => super::repl_starts::Gone::Died,
    }
}

fn err_tail(err_path: &Path) -> String {
    std::fs::read_to_string(err_path)
        .ok()
        .and_then(|t| {
            t.lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| clip(l.trim(), 200))
        })
        .unwrap_or_default()
}

/// The stderr file of a new REPL process. The previous process's goes
/// to `repl.err.1`: an exit reason reads the last line of `repl.err`,
/// and an old crash line there was reported again for every later exit
/// ("signal: 15 (SIGTERM) · bend: memory fault", BISE-122).
fn fresh_err(err_path: &Path) -> std::io::Result<std::fs::File> {
    let _ = std::fs::rename(err_path, err_path.with_extension("err.1"));
    std::fs::File::create(err_path)
}

/// The REPL's output, from its wire log (the REPL appends every batch
/// there before sending it): complete lines from `offset` on, until the
/// connection closes (the REPL died, or was killed). The socket is only
/// drained, so a full buffer never blocks the REPL.
fn pump_wire(stream: &TcpStream, wire: &Path, offset: u64, dir: &str, gen: u64, tx: &Sender<Msg>) {
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let closed = Arc::new(AtomicBool::new(false));
    if let Ok(mut s) = stream.try_clone() {
        let closed = closed.clone();
        std::thread::spawn(move || {
            let mut sink = [0u8; 8192];
            while matches!(s.read(&mut sink), Ok(n) if n > 0) {}
            closed.store(true, Ordering::SeqCst);
        });
    }
    let mut f = match std::fs::File::open(wire) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut offset = offset;
    let _ = f.seek(SeekFrom::Start(offset));
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = Vec::new();
    loop {
        // the flag BEFORE the read: the last read then sees every byte
        let done = closed.load(Ordering::SeqCst);
        chunk.clear();
        let _ = f.read_to_end(&mut chunk);
        pending.extend_from_slice(&chunk);
        while let Some(n) = pending.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = pending.drain(..=n).collect();
            offset += raw.len() as u64;
            let line = String::from_utf8_lossy(&raw)
                .trim_end_matches(['\n', '\r'])
                .to_string();
            let m = Msg::ReplLine {
                dir: dir.to_string(),
                gen,
                line,
                offset,
            };
            if tx.send(m).is_err() {
                return;
            }
        }
        if done {
            return;
        }
        if chunk.is_empty() {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Reconnect to a REPL a previous hub left running. It accepts the
/// connection when the old one is over (at once when idle, at the end of
/// its turn otherwise); its output meanwhile is in the wire log.
pub(super) fn adopt(r: ReplInfo, dir: String, gen: u64, adir: PathBuf, tx: Sender<Msg>, paths: Paths) {
    let gone = |reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            reason,
            cause: super::repl_starts::Gone::Died,
        });
    };
    let wire = adir.join("wire.log");
    let offset: u64 = std::fs::read_to_string(adir.join("wire.offset"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let stream = match TcpStream::connect(("127.0.0.1", r.port)) {
        Ok(s) => s,
        Err(e) => return gone(format!("reconnecting to the REPL: {}", e)),
    };
    let _ = stream.set_nodelay(true);
    if let Err(e) = say_hello(&stream, r.token.as_deref()) {
        return gone(format!("reconnecting to the REPL: {}", e));
    }
    let Ok(writer) = stream.try_clone() else {
        return gone("socket".into());
    };
    let _ = tx.send(Msg::ReplConnected {
        dir: dir.clone(),
        gen,
        stream: writer,
        steer: r.steer.clone(),
        interrupt: r.interrupt.clone(),
        pid: r.pid,
        adopted: true,
        busy: busy_at(&wire, offset),
    });
    pump_wire(&stream, &wire, offset, &dir, gen, &tx);
    // not our child: no exit status, only its death
    for _ in 0..50 {
        if !pid_alive(r.pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let tail = err_tail(&adir.join("repl.err"));
    let reason = if tail.is_empty() {
        "the REPL stopped".to_string()
    } else {
        format!("the REPL stopped · {}", tail)
    };
    log_line(
        &paths,
        &format!("repl {} (adopted) exited: {}", dir, reason),
    );
    gone(reason);
}

/// Spawn one REPL, wait for its banner, connect, stream its lines.
pub(super) fn supervise(
    mut cmd: Command,
    dir: String,
    gen: u64,
    adir: PathBuf,
    say: SayPort,
    tx: Sender<Msg>,
    paths: Paths,
) {
    let SayPort { port, token } = say;
    let log_path = adir.join("repl.log");
    let err_path = adir.join("repl.err");
    let bin = PathBuf::from(cmd.get_program());
    let gone = |reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            reason,
            cause: super::repl_starts::Gone::Died,
        });
    };
    let log = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => return gone(format!("log : {}", e)),
    };
    let err = fresh_err(&err_path);
    cmd.stdin(Stdio::null()).stdout(Stdio::from(log));
    match err {
        Ok(f) => cmd.stderr(Stdio::from(f)),
        Err(_) => cmd.stderr(Stdio::null()),
    };
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return gone(format!("spawn : {}", e)),
    };
    let _ = tx.send(Msg::ReplSpawned {
        dir: dir.clone(),
        gen,
        pid: child.id(),
    });
    let info = loop {
        let content = std::fs::read_to_string(&log_path).unwrap_or_default();
        if content.contains("REPL on") {
            break content;
        }
        // no time limit here: a start without progress is the hub's to
        // end (repl_starts), it kills this child
        if let Ok(Some(st)) = child.try_wait() {
            return gone(format!("the REPL died at startup ({})", st));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let field = |k: &str| -> String {
        info.lines()
            .find(|l| l.starts_with("harness-info "))
            .and_then(|l| {
                l.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(&format!("{}=", k)).map(|v| v.to_string()))
            })
            .unwrap_or_default()
    };
    let steer = field("steer");
    let interrupt = field("interrupt");
    let stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return gone(format!("connecting to the REPL: {}", e));
        }
    };
    let _ = stream.set_nodelay(true);
    if let Err(e) = say_hello(&stream, Some(&token)) {
        let _ = child.kill();
        let _ = child.wait();
        return gone(format!("connecting to the REPL: {}", e));
    }
    let writer = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => return gone(e.to_string()),
    };
    // what the next hub needs to adopt this REPL, its token too (so the
    // file is his alone); `sock`: its SB_SOCKET is agent.sock, so its
    // sandbox may close hub.sock (docs/issues/16)
    write_repl_json(
        &adir.join("repl.json"),
        &json!({"pid": child.id(), "port": port, "steer": steer, "interrupt": interrupt,
               "bin": bin.to_string_lossy(), "sock": "agent", "token": token}),
    );
    let _ = tx.send(Msg::ReplConnected {
        dir: dir.clone(),
        gen,
        stream: writer,
        steer,
        interrupt,
        pid: child.id(),
        adopted: false,
        busy: false,
    });
    pump_wire(&stream, &adir.join("wire.log"), 0, &dir, gen, &tx);
    let status = child.wait();
    let reason = match &status {
        Ok(s) => {
            let tail = err_tail(&err_path);
            format!(
                "{}{}",
                s,
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", tail)
                }
            )
        }
        Err(e) => e.to_string(),
    };
    log_line(&paths, &format!("repl {} exited: {}", dir, reason));
    let cause = status.as_ref().map_or(super::repl_starts::Gone::Died, gone_of);
    let _ = tx.send(Msg::ReplGone { dir: dir.clone(), gen, reason, cause });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// say-port: the hub's first line on a REPL it started is its hello,
    /// one whole line; repl.json, which holds the token for adoption, is
    /// his alone (0600) and an adopting hub reads the token back; an older
    /// REPL's repl.json has none, so no hello reaches it as his words.
    #[test]
    fn the_hub_opens_a_repl_with_its_token() {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(hello("ab12"), "hello ab12\n");
        let d = std::env::temp_dir().join(format!("say-port-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let f = d.join("repl.json");
        std::fs::write(&f, "old").unwrap();
        write_repl_json(&f, &json!({"pid": 1, "port": 2, "steer": "s", "interrupt": "i", "token": "ab12"}));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["token"], "ab12");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// stop-not-crash (m_48183): a TERM, INT or HUP is a request to stop;
    /// a crash signal, a KILL or an exit code is not.
    #[test]
    fn only_term_int_and_hup_are_a_stop() {
        use super::super::repl_starts::Gone;
        use std::os::unix::process::ExitStatusExt;
        // a wait status: the signal in the low 7 bits, an exit code << 8
        let by = |raw: i32| gone_of(&std::process::ExitStatus::from_raw(raw));
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            assert_eq!(by(sig), Gone::Stopped, "signal {sig}");
        }
        for sig in [libc::SIGKILL, libc::SIGSEGV, libc::SIGABRT, libc::SIGBUS] {
            assert_eq!(by(sig), Gone::Died, "signal {sig}");
        }
        assert_eq!(by(0), Gone::Died, "exit 0");
        assert_eq!(by(1 << 8), Gone::Died, "exit 1");
        assert_eq!(by(143 << 8), Gone::Died, "exit 143 is a code, not a signal");
    }

    #[test]
    fn a_new_repl_does_not_inherit_the_last_crash_line() {
        let d = std::env::temp_dir().join(format!("sb-err-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let err = d.join("repl.err");
        std::fs::write(&err, "bend: memory fault (machine stack overflow?)\n").unwrap();
        drop(fresh_err(&err).unwrap());
        assert_eq!(err_tail(&err), "");
        assert_eq!(
            std::fs::read_to_string(d.join("repl.err.1")).unwrap(),
            "bend: memory fault (machine stack overflow?)\n"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
