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
use std::time::{Duration, Instant};

/// What a hub needs to reconnect to a running REPL (`repl.json`).
pub(super) struct ReplInfo {
    pub(super) pid: u32,
    pub(super) port: u16,
    pub(super) bin: PathBuf,
    steer: String,
    interrupt: String,
}

pub(super) use crate::procs::terminate as kill_pid;
use crate::procs::alive as pid_alive;

fn is_repl(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("repl-"))
        .unwrap_or(false)
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
    };
    (adir.join("wire.log").exists() && pid_alive(r.pid) && is_repl(r.pid)).then_some(r)
}

/// Was a turn running at `offset` of the wire log? (the last
/// `turn_started` comes after the last `--- idle`)
pub(super) fn busy_at(wire: &Path, offset: u64) -> bool {
    let Ok(bytes) = std::fs::read(wire) else {
        return false;
    };
    let text = String::from_utf8_lossy(&bytes[..(offset as usize).min(bytes.len())]).to_string();
    let mut busy = false;
    for l in text.lines() {
        if l == "--- idle" {
            busy = false;
        } else if l.trim_start().starts_with("obs: turn_started") {
            busy = true;
        }
    }
    busy
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
    port: u16,
    tx: Sender<Msg>,
    paths: Paths,
) {
    let log_path = adir.join("repl.log");
    let err_path = adir.join("repl.err");
    let bin = PathBuf::from(cmd.get_program());
    let gone = |reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            reason,
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
    let start = Instant::now();
    let info = loop {
        let content = std::fs::read_to_string(&log_path).unwrap_or_default();
        if content.contains("REPL on") {
            break content;
        }
        if let Ok(Some(st)) = child.try_wait() {
            return gone(format!("the REPL died at startup ({})", st));
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return gone("the REPL did not start within 30 s".into());
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
    let writer = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => return gone(e.to_string()),
    };
    // what the next hub needs to adopt this REPL; `sock`: its SB_SOCKET is
    // agent.sock, so its sandbox may close hub.sock (docs/issues/16)
    let _ = std::fs::write(
        adir.join("repl.json"),
        json!({"pid": child.id(), "port": port, "steer": steer, "interrupt": interrupt,
               "bin": bin.to_string_lossy(), "sock": "agent"})
        .to_string(),
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
    gone(reason);
}

#[cfg(test)]
mod tests {
    use super::*;

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
