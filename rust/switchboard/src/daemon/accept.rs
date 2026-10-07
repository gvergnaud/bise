//! The hub's two sockets (docs/issues/16): one accept loop per socket,
//! one thread per connection; the first line's op decides what the
//! connection becomes (`crate::peer::access`). On `hub.sock`, a `hello`
//! or a `notice` from an agent's process (`crate::peer::judge`, on the
//! peer's pid and the process table) gets one `{"ev":"refused"}` line,
//! is closed, and leaves one hub.log line naming the agent.

use super::{log_line, write_json, Msg};
use crate::paths::Paths;
use crate::peer::{access, judge, Access, HubSide, Sock, Who};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// What both accept loops share: the hub's facts for the peer check and
/// the connection ids (one sequence: a client id and an agent token never
/// meet).
pub(super) struct Doors {
    pub(super) paths: Paths,
    /// The hub's pid, its tag id and its owners' tags (`peer::HubSide`;
    /// its siblings are read at each check).
    pub(super) hub: HubSide,
    pub(super) next: AtomicU64,
}

impl Doors {
    pub(super) fn new(paths: Paths, id: String, owners: &str) -> Arc<Doors> {
        let owners = owners.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
        let hub = HubSide { pid: std::process::id(), id, owners, siblings: BTreeSet::new() };
        Arc::new(Doors { paths, hub, next: AtomicU64::new(1) })
    }

    /// The ids of the hubs in this hub's `hubs/` folder (its own too).
    fn siblings(&self) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = [self.hub.id.clone()].into();
        let Some(parent) = self.paths.state.parent() else { return out };
        for e in std::fs::read_dir(parent).into_iter().flatten().flatten() {
            let natural = e.path().join("hub.sock");
            out.insert(crate::procs::hub_id(&bise_home::socket::socket_path(&natural)));
        }
        out
    }

    /// Who opened the connection of `pid` (read at its accept), and its
    /// pid and program for the log.
    fn who(&self, pid: Option<u32>) -> (Who, String) {
        let Some(pid) = pid else {
            return (Who::Gone, "pid unknown".into());
        };
        let table = crate::procs::snapshot();
        let hub = HubSide { siblings: self.siblings(), ..self.hub.clone() };
        let who = judge(&table, pid, &hub);
        let prog = table
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| Path::new(&p.command).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
            .unwrap_or_default();
        (who, format!("pid {pid} ({prog})"))
    }
}

pub(super) fn accept_loop(listener: UnixListener, tx: Sender<Msg>, doors: Arc<Doors>, sock: Sock) {
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let id = doors.next.fetch_add(1, Ordering::Relaxed);
        // at once: a peer that writes one line and closes (a `notice`)
        // has no pid to read once it is gone
        let pid = crate::peer_os::peer_pid(&stream);
        let (tx, doors) = (tx.clone(), doors.clone());
        std::thread::spawn(move || serve(stream, pid, id, &tx, &doors, sock));
    }
}

fn serve(stream: UnixStream, pid: Option<u32>, id: u64, tx: &Sender<Msg>, doors: &Doors, sock: Sock) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut r = BufReader::new(read_half);
    let mut first = String::new();
    if r.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(first.trim()) else {
        return;
    };
    let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut stream = stream;
    match access(sock, &op) {
        Access::No => {
            let name = if sock == Sock::Agent { "agent.sock" } else { "hub.sock" };
            write_json(&mut stream, &json!({"ok": false, "error": format!("{name} does not serve the op {op:?}")}));
            return;
        }
        Access::Open | Access::Agent => {}
        Access::Shim => {
            let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("");
            log_line(
                &doors.paths,
                &format!("hub.sock shim: an {op} request from {from:?} (a REPL started by an older hub; hub.sock stops serving it in the release after v2026.10.2-25)"),
            );
        }
        Access::User => {
            let (who, peer) = doors.who(pid);
            if let Some(why) = who.refusal_of(&op) {
                log_line(&doors.paths, &format!("client refused on hub.sock ({op}): {peer}: {why}"));
                write_json(&mut stream, &json!({"ev": "refused", "ok": false, "error": why}));
                return;
            }
        }
    }
    match op.as_str() {
        "hello" => {
            let _ = tx.send(Msg::ClientNew { id, stream });
            let mut line = String::new();
            loop {
                line.clear();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                            let _ = tx.send(Msg::ClientLine { id, v });
                        }
                    }
                }
            }
            let _ = tx.send(Msg::ClientGone { id });
        }
        "agent" => {
            let _ = tx.send(Msg::AgentNew { token: id, stream, v });
        }
        "version" => {
            let _ = tx.send(Msg::Version { stream, v });
        }
        "notice" => {
            let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let _ = tx.send(Msg::Notice { kind: s("kind"), text: s("text") });
        }
        "ping" => {
            write_json(&mut stream, &json!({"ok": true, "pid": std::process::id()}));
        }
        _ => {}
    }
}
