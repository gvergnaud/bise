//! A stdio MCP client: one child process, newline-delimited JSON-RPC
//! on its stdin/stdout. Requests from the server (`ping`, `roots/list`,
//! ...) are answered so it never waits on us.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::resolve::StdioServer;

pub const PROTOCOL: &str = "2025-06-18";

type Pending = Arc<Mutex<HashMap<u64, Sender<Value>>>>;

pub struct Client {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    next: Mutex<u64>,
    /// the server's `initialize` result
    pub init: Value,
}

fn send(stdin: &Mutex<ChildStdin>, v: &Value) -> std::io::Result<()> {
    let mut w = stdin.lock().unwrap_or_else(|e| e.into_inner());
    let mut line = v.to_string();
    line.push('\n');
    w.write_all(line.as_bytes())?;
    w.flush()
}

impl Client {
    /// Spawn the server and run the handshake (`initialize`,
    /// `notifications/initialized`). stderr is appended to `log`.
    pub fn start(
        s: &StdioServer,
        plugin_root: &Path,
        data_root: &Path,
        log: &Path,
        timeout: Duration,
        on_change: Option<crate::remote::OnChange>,
    ) -> Result<Client, String> {
        let _ = std::fs::create_dir_all(data_root);
        let err = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map(Stdio::from)
            .unwrap_or_else(|_| Stdio::null());
        let mut cmd = Command::new(&s.command);
        cmd.args(&s.args)
            .current_dir(&s.cwd)
            .env("PLUGIN_ROOT", plugin_root)
            .env("PLUGIN_DATA", data_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(err);
        for (k, v) in &s.env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot start {:?}: {}", s.command, e))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().ok_or("no stdin")?));
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        {
            let pending = pending.clone();
            let stdin = stdin.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    let has_method = msg.get("method").is_some();
                    match (msg.get("id"), has_method) {
                        // a response to one of ours
                        (Some(id), false) => {
                            if let Some(tx) = id.as_u64().and_then(|id| {
                                pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id)
                            }) {
                                let _ = tx.send(msg);
                            }
                        }
                        // a request from the server
                        (Some(id), true) => {
                            let reply = if msg["method"] == "ping" {
                                json!({"jsonrpc": "2.0", "id": id, "result": {}})
                            } else {
                                json!({"jsonrpc": "2.0", "id": id, "error":
                                    {"code": -32601, "message": "not supported by bend-harness"}})
                            };
                            let _ = send(&stdin, &reply);
                        }
                        // a notification: list_changed is passed on, the rest
                        // (logging, progress) ignored
                        _ => {
                            if msg.get("method").and_then(Value::as_str) == Some("notifications/tools/list_changed") {
                                if let Some(f) = &on_change {
                                    f();
                                }
                            }
                        }
                    }
                }
                // EOF: wake every waiter with an error
                for (_, tx) in pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
                    let _ = tx.send(json!({"error": {"code": -32000, "message": "the MCP server exited"}}));
                }
            });
        }
        let mut c = Client {
            child,
            stdin,
            pending,
            next: Mutex::new(1),
            init: Value::Null,
        };
        let init = c.request(
            "initialize",
            json!({"protocolVersion": PROTOCOL, "capabilities": {},
                   "clientInfo": {"name": "bend-harness", "version": "1.0"}}),
            timeout,
        );
        match init {
            Ok(v) => c.init = v,
            Err(e) => {
                c.stop();
                return Err(format!("initialize: {}", e));
            }
        }
        send(&c.stdin, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .map_err(|e| e.to_string())?;
        Ok(c)
    }

    /// One request; the `result`, or the error message.
    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let raw = self.request_raw(method, params, timeout)?;
        match raw.get("result") {
            Some(r) => Ok(r.clone()),
            None => Err(raw
                .get("error")
                .map(|e| e.get("message").and_then(Value::as_str).map(String::from).unwrap_or(e.to_string()))
                .unwrap_or_else(|| "no result".into())),
        }
    }

    /// One request; the whole response message (`result` or `error`).
    pub fn request_raw(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = {
            let mut n = self.next.lock().unwrap_or_else(|e| e.into_inner());
            *n += 1;
            *n
        };
        let (tx, rx) = channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Err(e) = send(&self.stdin, &msg) {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(format!("write: {}", e));
        }
        match rx.recv_timeout(timeout) {
            Ok(v) => Ok(v),
            Err(_) => {
                self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                Err(format!("no answer to {} within {}s", method, timeout.as_secs()))
            }
        }
    }

    /// Every tool, following `nextCursor`.
    pub fn list_tools(&self, timeout: Duration) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let params = match &cursor {
                Some(c) => json!({"cursor": c}),
                None => json!({}),
            };
            let r = self.request("tools/list", params, timeout)?;
            if let Some(ts) = r.get("tools").and_then(Value::as_array) {
                out.extend(ts.iter().cloned());
            }
            cursor = r.get("nextCursor").and_then(Value::as_str).map(String::from);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Close stdin, give the server a moment, then kill it.
    pub fn stop(&mut self) {
        // dropping our stdin handle is not enough while the reader thread
        // holds a clone; kill after a short grace period
        for _ in 0..10 {
            if !self.alive() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.alive() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
