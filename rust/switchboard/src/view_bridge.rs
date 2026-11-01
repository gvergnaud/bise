//! The hub's side of the plugins' loopback bridges for MCP App views
//! (docs/mcp-apps.md C5; mcp-apps review d6, out of daemon/views.rs,
//! which keeps the dispatch): the one HTTP request a view's call makes to
//! a bridge route ([`request`]), a `bridge.json`'s address waited for
//! ([`wait_base`]), the hub's own views bridge (`bise plugins serve
//! --parent <hub>`, [`ViewsBridge`]: started on the first need, stopped
//! after its idle time or with the hub), an orphan one an older hub left
//! ([`kill_orphan_bridge`]), and whether an agent's REPL still runs
//! ([`repl_alive`]).
//!
//! [`request`] is a small client of the bridge's own server
//! (`bend_plugins::bridge`), no HTTP crate: one request a connection, a
//! 200 only, its body read to the end. The bridge always answers with a
//! Content-Length, never chunked: a chunked answer is refused by name, so
//! a bridge that ever starts sending one fails loudly, never as a JSON
//! error.

use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a thread waits for a bridge's address (bise_proto's, part of
/// the bound clients hold a view's call for: view_rpc::VIEW_CALL_MAX_MS).
pub(crate) const BRIDGE_WAIT: Duration = Duration::from_millis(bise_proto::view_rpc::VIEW_BRIDGE_MS);
/// One request to a view's server (likewise).
pub(crate) const CALL_TIMEOUT: Duration = Duration::from_millis(bise_proto::view_rpc::VIEW_SERVER_MS);

/// One request to a bridge route (`http://127.0.0.1:<port>/…`): a JSON-RPC
/// POST of `body`, or a GET with none; its answer's JSON. A status other
/// than 200 is its status line; a chunked answer is refused (the bridge
/// never sends one).
pub(crate) fn request(url: &str, body: Option<&Value>, timeout: Duration) -> Result<Value, String> {
    let rest = url.strip_prefix("http://").ok_or("not a loopback URL")?;
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).ok_or("no path")?;
    let mut s = std::net::TcpStream::connect(host).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let sent = match body {
        Some(b) => {
            let b = b.to_string();
            write!(s, "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
        }
        None => write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
    };
    sent.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(|e| e.to_string())?;
    answer_of(&String::from_utf8_lossy(&out))
}

/// An HTTP answer's JSON body: 200 only, never chunked.
fn answer_of(text: &str) -> Result<Value, String> {
    let (head, body) = text.split_once("\r\n\r\n").ok_or("no answer")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(head.lines().next().unwrap_or("").to_string());
    }
    let chunked = head.lines().any(|l| l.split_once(':').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked")));
    if chunked {
        return Err("a chunked answer: the bridge never sends one, so this is not the bridge".into());
    }
    serde_json::from_str(body).map_err(|e| e.to_string())
}

/// `bridge.json`'s base, waited for up to [`BRIDGE_WAIT`].
pub(crate) fn wait_base(path: &Path) -> Option<String> {
    let until = std::time::Instant::now() + BRIDGE_WAIT;
    loop {
        if let Some(b) = std::fs::read_to_string(path).ok().as_deref().and_then(crate::view_call::bridge_base) {
            return Some(b);
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The hub's own bridge, for the views whose agent has no REPL now.
#[derive(Default)]
pub(crate) struct ViewsBridge {
    child: Option<std::process::Child>,
    last_use: u64,
}

impl ViewsBridge {
    /// Its `bridge.json` (`json`, in `dir`), the bridge started when it
    /// isn't running.
    pub(crate) fn ensure(&mut self, exe: &Path, dir: &Path, json: &Path, workspace: &Path, now: u64) -> Result<PathBuf, String> {
        self.last_use = now;
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return Ok(json.to_path_buf());
            }
            self.child = None;
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(json);
        let log = std::fs::File::create(dir.join("bridge.log")).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["plugins", "serve", "--dir"])
            .arg(dir)
            .arg("--workspace")
            .arg(workspace)
            .args(["--parent", &std::process::id().to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log);
        bise_home::env::for_child::<&str, &str>(bise_home::env::Child::Repl, []).apply(&mut cmd);
        self.child = Some(cmd.spawn().map_err(|e| e.to_string())?);
        Ok(json.to_path_buf())
    }

    /// Stopped after its idle time with no view call.
    pub(crate) fn stop_idle(&mut self, now: u64) {
        let idle = bend_plugins::tools_cache::IDLE_STOP.as_millis() as u64;
        if self.child.is_some() && now.saturating_sub(self.last_use) >= idle {
            self.stop();
        }
    }

    pub(crate) fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// A views bridge an older hub left (its `bridge.json`'s pid, when that
/// pid is still a `plugins serve`): ended. Its own parent watch ends it
/// once its hub is gone; this covers a pid that watch missed.
pub(crate) fn kill_orphan_bridge(file: &Path) {
    let pid = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("pid").and_then(Value::as_u64));
    if let Some(pid) = pid.filter(|p| *p > 1 && *p < i32::MAX as u64) {
        let cmd = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().ok();
        if cmd.is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("plugins serve")) {
            // SAFETY: kill(2) on a pid that runs our `plugins serve`, checked above
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
    }
    let _ = std::fs::remove_file(file);
}

/// An agent's REPL runs: its `repl.pid` names a live process, never a
/// zombie (SIGKILLed, not reaped yet: it answers `kill -0` while its
/// bridge retires; that bridge still lists the tools it held, so a call
/// judged there went on to a server that was gone).
pub(crate) fn repl_alive(pid_file: &Path) -> bool {
    std::fs::read_to_string(pid_file)
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .is_some_and(|p| p > 1 && bise_peer::table::one(p).is_some_and(|r| !r.zombie))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// A bridge on loopback serving fake-app's tools/list (the fixture's
    /// shapes, tests/fixtures/mcp-apps/fake-app): its `bridge.json` in
    /// `dir`, and the methods it was asked, in order.
    pub(crate) fn fake_bridge(dir: &Path) -> (PathBuf, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = asked.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { continue };
                let mut r = std::io::BufReader::new(s.try_clone().unwrap());
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if std::io::BufRead::read_line(&mut r, &mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                let _ = r.read_exact(&mut body);
                let req: Value = serde_json::from_slice(&body).unwrap_or_default();
                let method = req["method"].as_str().unwrap_or("").to_string();
                if body.is_empty() {
                    // a GET: the plugin's views (C8, `~ui`)
                    let out = json!({"views": [{"server": "fake-app", "tool": "show_counter", "uri": "ui://fake-app/counter", "ready": true}]}).to_string();
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
                    continue;
                }
                log.lock().unwrap().push(method.clone());
                let result = match method.as_str() {
                    "tools/list" => json!({"tools": [
                        {"name": "show_counter", "_meta": {"ui": {"resourceUri": "ui://fake-app/counter"}}},
                        {"name": "bump", "_meta": {"ui": {"visibility": ["app"]}}},
                        {"name": "whisper", "_meta": {"ui": {"visibility": ["model"]}}},
                    ]}),
                    _ => json!({"content": [{"type": "text", "text": "ok"}]}),
                };
                let out = json!({"jsonrpc": "2.0", "id": 1, "result": result}).to_string();
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            }
        });
        let json_path = dir.join("bridge.json");
        std::fs::write(&json_path, json!({"v": 1, "base": format!("http://127.0.0.1:{port}/tok"), "pid": 1}).to_string()).unwrap();
        (json_path, asked)
    }

    pub(crate) fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-views-{name}-{}-{}", std::process::id(), crate::util::now_ms()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// C8: a plugin's views come back as the bridge gave them.
    #[test]
    fn a_plugins_views_are_the_bridges_answer_as_is() {
        let d = tmp("ui");
        let (bridge, _) = fake_bridge(&d);
        let base = wait_base(&bridge).unwrap();
        let got = request(&format!("{base}/fake-app{}", bend_plugins::bridge::UI_ROUTE), None, Duration::from_secs(5)).unwrap();
        assert_eq!(got["views"][0]["uri"], "ui://fake-app/counter");
        assert!(request("http://127.0.0.1:1/x", None, Duration::from_secs(1)).is_err(), "nobody there");
        let _ = std::fs::remove_dir_all(&d);
    }
    /// A REPL killed leaves its port known a moment: its own bridge is
    /// used only while its pid lives.
    #[test]
    fn a_repl_is_alive_only_when_its_pid_runs() {
        let d = tmp("repl");
        let f = d.join("repl.pid");
        assert!(!repl_alive(&f), "no file");
        std::fs::write(&f, std::process::id().to_string()).unwrap();
        assert!(repl_alive(&f));
        let mut c = std::process::Command::new("true").spawn().unwrap();
        let pid = c.id();
        c.wait().unwrap();
        std::fs::write(&f, pid.to_string()).unwrap();
        assert!(!repl_alive(&f), "a pid gone");
        // a REPL SIGKILLed, not reaped yet: a zombie, its bridge retiring
        // (mcp_apps_e2e's REPL-gone step: the call went to that bridge)
        let mut z = std::process::Command::new("true").spawn().unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while bise_peer::table::one(z.id()).is_some_and(|p| !p.zombie) && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(&f, z.id().to_string()).unwrap();
        assert!(!repl_alive(&f), "a zombie is gone");
        z.wait().unwrap();
        std::fs::write(&f, "1").unwrap();
        assert!(!repl_alive(&f), "never launchd");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_answer_is_a_200_with_its_json_never_chunked() {
        assert_eq!(answer_of("HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n{\"a\": 1}"), Ok(json!({"a": 1})));
        assert_eq!(answer_of("HTTP/1.1 404 Not Found\r\n\r\n"), Err("HTTP/1.1 404 Not Found".to_string()));
        let e = answer_of("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n{\"a\": 1}\r\n0\r\n\r\n").unwrap_err();
        assert!(e.contains("chunked"), "{e}");
        assert!(answer_of("garbage").is_err());
    }
}
