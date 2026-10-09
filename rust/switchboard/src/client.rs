//! How a client reaches the hub of a workspace: connect to `hub.sock`,
//! starting the hub first when none runs.

use crate::paths::Paths;
use serde_json::{json, Value};
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
    cmd.arg("sbd")
        .arg("--workspace")
        .arg(&paths.workspace)
        .current_dir(app_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .process_group(0)
        .spawn()?;
    Ok(())
}

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
            let err_file = paths.state.join("hub.err");
            let err0 = std::fs::metadata(&err_file).map_or(0, |m| m.len());
            start_hub(paths, exe, app_root)?;
            let t0 = Instant::now();
            loop {
                if let Ok(s) = UnixStream::connect(paths.socket()) {
                    break s;
                }
                if t0.elapsed() > Duration::from_secs(15) {
                    let cause = crate::switch::last_line(&crate::switch::read_from(&err_file, err0), |_| true);
                    return Err(std::io::Error::other(not_started(&cause, &err_file)));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    Ok(s)
}

/// Stop the hub of a workspace and every agent REPL. `keep_agents`:
/// the REPLs keep running (their turns too) for the next hub to adopt -
/// a version switch, a hub restart. `initialize`, then `hub/stop`; an
/// older hub (before client-protocol) gets its hello and the `stop_hub`
/// op instead.
pub fn stop(paths: &Paths, keep_agents: bool) -> std::io::Result<bool> {
    stop_at(&paths.socket(), keep_agents)
}

fn stop_at(socket: &Path, keep_agents: bool) -> std::io::Result<bool> {
    use bise_proto::hub::HubCmd;
    use bise_proto::rpc::{self, Id, Init, InitializeParams, Message, Request};
    let Ok(mut s) = UnixStream::connect(socket) else { return Ok(false) };
    let params = serde_json::to_value(InitializeParams::new("bise", env!("CARGO_PKG_VERSION"))).unwrap_or_default();
    writeln!(s, "{}", Message::Request(Request::new(Id::Num(0), rpc::INITIALIZE, params)).to_value())?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut lines = BufReader::new(s.try_clone()?).lines();
    let project = loop {
        let Some(Ok(line)) = lines.next() else { return Ok(true) };
        match rpc::init_answer(&serde_json::from_str(&line).unwrap_or(Value::Null), &Id::Num(0)) {
            Init::Ready(res) => break Some(res.project),
            // refused (it runs in an agent's process): the hub stays
            Init::Refused(_) => return Ok(true),
            Init::Older => break None,
            Init::Other => {}
        }
    };
    drop(lines);
    let mut s = match project {
        Some(project) => {
            if let Some(req) = rpc::request(Id::Num(1), &HubCmd::StopHub { project, keep_agents }) {
                writeln!(s, "{}", Message::Request(req).to_value())?;
            }
            s
        }
        None => older_stop(socket, keep_agents)?,
    };
    // wait for the socket to go away, reading what the hub writes
    // (notifications; an older hub's hello is tens of KB): a full socket
    // buffer would block the hub, which would never stop
    let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
    let mut sink = [0u8; 65536];
    let t0 = Instant::now();
    while socket.exists() && t0.elapsed() < Duration::from_secs(5) {
        if let Ok(0) = std::io::Read::read(&mut s, &mut sink) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(true)
}

/// An older hub (before client-protocol): its hello, then the op.
// TODO(client-protocol, the plan's 'after the release' step, with
// switch/ask.rs older_switch): goes the release after client-protocol's.
fn older_stop(socket: &Path, keep_agents: bool) -> std::io::Result<UnixStream> {
    let mut s = UnixStream::connect(socket)?;
    let req = json!({"op": "stop_hub", "keep_agents": keep_agents});
    s.write_all(format!("{{\"op\":\"hello\"}}\n{req}\n").as_bytes())?;
    Ok(s)
}

/// The start of `request`'s error when the hub did not answer in time.
pub const NO_ANSWER: &str = "the hub did not answer (busy) in";

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
        assert!(stop_at(&p, true).unwrap());
        let req = hub.join().unwrap();
        assert_eq!((req["method"].as_str(), req["params"]["project"].as_str(), req["params"]["keep_agents"].as_bool()), (Some("hub/stop"), Some("p1"), Some(true)));
    }

    /// An older hub (before client-protocol): its hello and the op.
    #[test]
    fn an_older_hub_gets_hello_and_stop_hub() {
        let (p, l) = sock(2);
        let path = p.clone();
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let _ = read(&mut BufReader::new(w.try_clone().unwrap()));
            writeln!(w, "{}", json!({"ok": false, "error": "hub.sock does not serve the op \"\""})).unwrap();
            drop(w);
            let (w, _) = l.accept().unwrap();
            let mut r = BufReader::new(w);
            assert_eq!(read(&mut r)["op"], "hello");
            let op = read(&mut r);
            std::fs::remove_file(&path).unwrap();
            op
        });
        assert!(stop_at(&p, false).unwrap());
        assert_eq!(hub.join().unwrap(), json!({"op": "stop_hub", "keep_agents": false}));
    }
}
