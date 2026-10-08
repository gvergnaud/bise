//! `follow_install`'s question to the folder's hub (BISE-255): switch to
//! the launched version, and the hub's words back. A hub that serves
//! JSON-RPC (client-protocol) gets `initialize` then `version/switch`; an
//! older one (`bise_proto::rpc::init_answer`'s `Older`, the one reading
//! of its door) the older hello and `version` op on a new connection.

use bise_proto::hub::HubCmd;
use bise_proto::rpc::{self, Id, Init, InitializeParams, Message, Request};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const INIT: u64 = 0;
const SWITCH: u64 = 1;

/// The hub's words to `/version <to>` (its notice, or its refusal);
/// empty when it said nothing in time.
pub(super) fn ask_switch(socket: &Path, to: &str) -> std::io::Result<String> {
    let mut s = UnixStream::connect(socket)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    let params = serde_json::to_value(InitializeParams::new("bise", env!("CARGO_PKG_VERSION"))).unwrap_or_default();
    writeln!(s, "{}", Message::Request(Request::new(Id::Num(INIT), rpc::INITIALIZE, params)).to_value())?;
    let mut lines = BufReader::new(s.try_clone()?).lines();
    let project = loop {
        let Some(line) = lines.next() else { return Ok(String::new()) };
        match rpc::init_answer(&serde_json::from_str(&line?).unwrap_or(Value::Null), &Id::Num(INIT)) {
            Init::Ready(res) => break res.project,
            Init::Refused(e) => return Ok(e),
            Init::Older => return older_switch(socket, to),
            Init::Other => {}
        }
    };
    let cmd = HubCmd::VersionSwitch { project, to: to.to_string() };
    let Some(req) = rpc::request(Id::Num(SWITCH), &cmd) else { return Ok(String::new()) };
    writeln!(s, "{}", Message::Request(req).to_value())?;
    // the hub's notifications may come first: its response by id
    for line in lines {
        let Ok(Message::Response(r)) = Message::from_value(serde_json::from_str(&line?).unwrap_or(Value::Null)) else { continue };
        if r.id != Some(Id::Num(SWITCH)) {
            continue;
        }
        return Ok(match r.error {
            Some(e) => e.message,
            None => r.result.as_ref().and_then(|v| v.get("notice")).and_then(Value::as_str).unwrap_or("").to_string(),
        });
    }
    Ok(String::new())
}

/// An older hub (before client-protocol): hello, then the `version` op;
/// its answer is a `notice` after the hello's lines.
// TODO(client-protocol, the plan's 'after the release' step, with
// Hubs.older_door): goes the release after client-protocol's.
fn older_switch(socket: &Path, to: &str) -> std::io::Result<String> {
    let mut s = UnixStream::connect(socket)?;
    let req = json!({"op": "version", "do": "switch", "to": to});
    s.write_all(format!("{{\"op\":\"hello\"}}\n{}\n", req).as_bytes())?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    // the hub's hello first (tens of KB), then the answer: a notice
    for line in BufReader::new(s).lines() {
        let v: Value = serde_json::from_str(&line?).unwrap_or(Value::Null);
        if v.get("ev").and_then(|x| x.as_str()) == Some("notice") {
            return Ok(v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string());
        }
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn read(r: &mut impl BufRead) -> Value {
        let mut l = String::new();
        r.read_line(&mut l).unwrap();
        serde_json::from_str(&l).unwrap_or(Value::Null)
    }

    /// A fake hub's socket (short: a socket path has ~100 bytes), gone
    /// when the guard drops.
    struct Gone(std::path::PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn sock(n: u32) -> (Gone, std::path::PathBuf, UnixListener) {
        let p = std::env::temp_dir().join(format!("ask{}-{n}.sock", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let l = UnixListener::bind(&p).unwrap();
        (Gone(p.clone()), p, l)
    }

    #[test]
    fn a_new_hub_gets_initialize_then_version_switch() {
        let (_d, p, l) = sock(1);
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let mut r = BufReader::new(w.try_clone().unwrap());
            let init = read(&mut r);
            assert_eq!(init["method"], "initialize");
            let res = json!({"project": "p1", "proto": bise_proto::PROTO, "workspace": "/w", "name": "w", "methods": ["version/switch"],
                "notifications": [], "hub": {"watermark": {"epoch": 1, "seq": 0}, "state": []}});
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "id": 0, "result": res})).unwrap();
            let req = read(&mut r);
            // a notification before the answer is skipped
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "method": "hub/notice", "params": {"project": "p1", "text": "x"}})).unwrap();
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "id": req["id"], "result": {"notice": "switching to version v2"}})).unwrap();
            req
        });
        assert_eq!(ask_switch(&p, "/apps/v2").unwrap(), "switching to version v2");
        let req = hub.join().unwrap();
        assert_eq!((req["method"].as_str(), req["params"]["project"].as_str(), req["params"]["to"].as_str()), (Some("version/switch"), Some("p1"), Some("/apps/v2")));
    }

    #[test]
    fn an_older_hub_refuses_initialize_and_gets_the_older_op() {
        let (_d, p, l) = sock(2);
        let hub = std::thread::spawn(move || {
            // accept.rs before client-protocol: initialize isn't an op it serves
            let (mut w, _) = l.accept().unwrap();
            let _ = read(&mut BufReader::new(w.try_clone().unwrap()));
            writeln!(w, "{}", json!({"ok": false, "error": "hub.sock does not serve the op \"\""})).unwrap();
            drop(w);
            let (mut w, _) = l.accept().unwrap();
            let mut r = BufReader::new(w.try_clone().unwrap());
            assert_eq!(read(&mut r)["op"], "hello");
            let op = read(&mut r);
            writeln!(w, "{}", json!({"ev": "hello", "workspace": "/w"})).unwrap();
            writeln!(w, "{}", json!({"ev": "notice", "text": "switching to version v2"})).unwrap();
            op
        });
        assert_eq!(ask_switch(&p, "/apps/v2").unwrap(), "switching to version v2");
        assert_eq!(hub.join().unwrap(), json!({"op": "version", "do": "switch", "to": "/apps/v2"}));
    }

    #[test]
    fn a_refusal_is_the_answer() {
        let (_d, p, l) = sock(3);
        let hub = std::thread::spawn(move || {
            let (mut w, _) = l.accept().unwrap();
            let _ = read(&mut BufReader::new(w.try_clone().unwrap()));
            writeln!(w, "{}", json!({"jsonrpc": "2.0", "id": 0, "error": {"code": rpc::code::REFUSED, "message": "an agent's process"}})).unwrap();
        });
        assert_eq!(ask_switch(&p, "/apps/v2").unwrap(), "an agent's process");
        hub.join().unwrap();
    }
}
