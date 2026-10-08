//! The fake hub's side of one connection (moved out of ambient/tests.rs,
//! architect m_13367: a pure move, no test changed).

use super::*;

/// The hub's side of one connection. A connection whose first line is
/// JSON-RPC's `initialize` (a project's, client-protocol P1c) is spoken
/// to in JSON-RPC: [`HubEnd::next`] gives its requests back as the
/// `HubCmd`s they carry, [`HubEnd::say`] turns a `HubEv` into the answer
/// of the request waiting for it (a read's result, an error by its tag
/// and cid, command/run's notice) or into its notification, so the tests
/// read as the window's commands and the hub's events.
pub(super) struct HubEnd {
    pub(super) w: UnixStream,
    pub(super) r: BufReader<UnixStream>,
    /// JSON-RPC (its first line was `initialize`); None: nothing read yet
    pub(super) rpc: Option<bool>,
    /// lines read ahead (by `say`), for `next`
    ahead: std::collections::VecDeque<Value>,
    /// `initialize`'s id once read
    init: Option<Value>,
    /// the requests read and not answered: (id, method, cmd tag, cid)
    pending: Vec<(Value, String, String, Option<u64>)>,
    seq: u64,
}

impl HubEnd {
    pub(super) fn new(w: UnixStream, r: BufReader<UnixStream>) -> HubEnd {
        HubEnd { w, r, rpc: None, ahead: Default::default(), init: None, pending: Vec::new(), seq: 0 }
    }

    pub(super) fn write(&mut self, v: Value) {
        writeln!(self.w, "{v}").unwrap();
    }

    pub(super) fn say(&mut self, v: Value) {
        if self.rpc.is_none() || self.rpc == Some(true) {
            self.read_ahead();
        }
        if self.rpc != Some(true) {
            return self.write(v);
        }
        let tag = v["ev"].as_str().unwrap_or("").to_string();
        if tag == "refused" {
            let id = self.init.clone().unwrap_or(Value::Null);
            let msg = v["error"].as_str().unwrap_or("").to_string();
            return self.write(json!({"jsonrpc": "2.0", "id": id, "error": {"code": bise_proto::rpc::code::REFUSED, "message": msg}}));
        }
        let Ok(ev) = bise_proto::hub::HubEv::from_value(v.clone()) else { return self.write(v) };
        if tag == "error" {
            let (cmd, cid) = (v["cmd"].as_str().unwrap_or("").to_string(), v["cid"].as_u64());
            if let Some(i) = self.pending.iter().position(|p| p.2 == cmd && (cid.is_none() || p.3 == cid)) {
                let (id, ..) = self.pending.remove(i);
                let mut data = json!({});
                for k in ["reason", "kind"] {
                    if !v[k].is_null() {
                        data[k] = v[k].clone();
                    }
                }
                let msg = v["text"].clone();
                return self.write(json!({"jsonrpc": "2.0", "id": id, "error": {"code": bise_proto::rpc::code::HUB_REFUSED, "message": msg, "data": data}}));
            }
        }
        if tag == "notice" && v["cmd"] == "slash" {
            if let Some(i) = self.pending.iter().position(|p| p.2 == "slash") {
                let (id, ..) = self.pending.remove(i);
                return self.write(json!({"jsonrpc": "2.0", "id": id, "result": {"notice": v["text"]}}));
            }
        }
        let answers = |p: &(Value, String, String, Option<u64>)| bise_proto::rpc::method_row(&p.1).and_then(|r| r.result) == Some(tag.as_str());
        if let Some(i) = self.pending.iter().position(answers) {
            let (id, ..) = self.pending.remove(i);
            return self.write(json!({"jsonrpc": "2.0", "id": id, "result": bise_proto::rpc::result(&ev)}));
        }
        let hub = bise_proto::rpc::note_of_ev(&tag).is_some_and(|r| r.scope == bise_proto::rpc::Scope::Hub);
        let w = hub.then(|| {
            self.seq += 1;
            bise_proto::rpc::Watermark { epoch: 1, seq: self.seq }
        });
        match bise_proto::rpc::note(&ev, w) {
            Some(n) => self.write(serde_json::to_value(n).unwrap()),
            None => self.write(v),
        }
    }

    pub(super) fn line(&mut self, agent: &str, line: &str) {
        self.say(json!({"ev": "line", "agent": agent, "line": line, "pos": 1}));
    }

    /// One raw line of the core (`timeout`: none read meanwhile).
    pub(super) fn raw(&mut self, timeout: Duration) -> Option<Value> {
        self.r.get_ref().set_read_timeout(Some(timeout)).unwrap();
        let mut l = String::new();
        let got = self.r.read_line(&mut l);
        self.r.get_ref().set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        match got {
            Ok(n) if n > 0 => serde_json::from_str(l.trim()).ok(),
            _ => None,
        }
    }

    /// The core's lines written so far, kept for `next` (what `say`
    /// answers must have been read).
    pub(super) fn read_ahead(&mut self) {
        while let Some(v) = self.raw(Duration::from_millis(30)) {
            self.take(v);
        }
    }

    /// One line of the core: `initialize` noted, a request kept for its
    /// answer and queued as its `HubCmd`, the hellos dropped.
    pub(super) fn take(&mut self, v: Value) {
        if self.rpc.is_none() {
            self.rpc = Some(v["method"] == "initialize");
        }
        if v["method"] == "initialize" {
            self.init = Some(v["id"].clone());
            return;
        }
        // the connector's hello, then the core's typed hello (S3b), and
        // JSON-RPC's own (initialized, hub/read): never a window command
        if v["op"] == "hello" || v["cmd"] == "hello" || v["method"] == "initialized" || v["method"] == "hub/read" {
            return;
        }
        if let (Some(m), Some(id)) = (v["method"].as_str(), v.get("id")) {
            let Ok(cmd) = bise_proto::rpc::cmd(m, v["params"].clone()) else { return self.ahead.push_back(v) };
            let tag = bise_proto::rpc::method_row(m).map(|r| r.cmd.to_string()).unwrap_or_default();
            self.pending.push((id.clone(), m.to_string(), tag, v["params"]["cid"].as_u64()));
            return self.ahead.push_back(cmd.to_value());
        }
        self.ahead.push_back(v);
    }

    /// The next request the core wrote (the hellos and `initialize`
    /// skipped; a JSON-RPC request as its `HubCmd`).
    pub(super) fn next(&mut self) -> Value {
        loop {
            if let Some(v) = self.ahead.pop_front() {
                return v;
            }
            let mut l = String::new();
            self.r.read_line(&mut l).expect("the core wrote nothing");
            let v: Value = serde_json::from_str(l.trim()).unwrap();
            self.take(v);
        }
    }

    /// [`next`](Self::next) without its `project` (the home connection's
    /// requests carry the test workspace's hub id).
    pub(super) fn sent(&mut self) -> Value {
        let mut v = self.next();
        if let Some(o) = v.as_object_mut() {
            o.remove("project");
        }
        v
    }

    /// `initialize` answered (a JSON-RPC connection): `methods` the
    /// hub's (none: all of this version's).
    pub(super) fn initialized(&mut self, project: &str, methods: Option<Vec<String>>) {
        while self.init.is_none() {
            let mut l = String::new();
            self.r.read_line(&mut l).expect("the core's initialize");
            let v: Value = serde_json::from_str(l.trim()).unwrap();
            self.take(v);
        }
        let id = self.init.clone().unwrap();
        let result = bise_proto::rpc::InitializeResult {
            project: project.into(),
            proto: bise_proto::PROTO,
            workspace: format!("/p/{project}"),
            name: project.into(),
            exe: String::new(),
            state_dir: String::new(),
            version: Value::Null,
            reload: String::new(),
            pages_url: None,
            methods: methods.unwrap_or_else(bise_proto::rpc::methods),
            notifications: bise_proto::rpc::notifications(),
            hub: bise_proto::rpc::HubState { watermark: bise_proto::rpc::Watermark { epoch: 1, seq: 0 }, state: vec![] },
        };
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }
}
