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

/// A client connection (the TUI, a test): `hello` already sent.
pub fn connect(paths: &Paths, exe: &Path, app_root: &Path) -> std::io::Result<UnixStream> {
    let mut s = match UnixStream::connect(paths.socket()) {
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
    s.write_all(b"{\"op\":\"hello\"}\n")?;
    Ok(s)
}

/// Stop the hub of a workspace and every agent REPL. `keep_agents`:
/// the REPLs keep running (their turns too) for the next hub to adopt -
/// a version switch, a hub restart.
pub fn stop(paths: &Paths, keep_agents: bool) -> std::io::Result<bool> {
    match UnixStream::connect(paths.socket()) {
        Ok(mut s) => {
            let req = json!({"op": "stop_hub", "keep_agents": keep_agents});
            s.write_all(format!("{{\"op\":\"hello\"}}\n{}\n", req).as_bytes())?;
            // wait for the socket to go away, reading what the hub
            // writes: its hello (the versions list alone is tens of KB
            // with long commit subjects) must not fill the socket
            // buffer, or the hub blocks on it and never stops
            let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
            let mut sink = [0u8; 65536];
            let t0 = Instant::now();
            while paths.socket().exists() && t0.elapsed() < Duration::from_secs(5) {
                match std::io::Read::read(&mut s, &mut sink) {
                    Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
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
}
