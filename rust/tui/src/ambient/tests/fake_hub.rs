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
    /// `initialize` answered
    answered: bool,
    /// each agent's thread as this hub holds it (the home connection)
    threads: std::collections::HashMap<String, Thread>,
    /// main's subscribe from the capsule (core/home.rs MAIN_PAGE),
    /// answered when the test says `ready`
    main_sub: Option<Value>,
    /// `initialize`'s page server, if the test sets one
    pub(super) pages_url: Option<String>,
}

/// One thread: its lines, the entries sent, subscribed.
#[derive(Default)]
struct Thread {
    lines: Vec<bise_proto::thread::Line>,
    sent: Vec<Entry>,
    live: bool,
}

/// The tests' home hub's project (the agents tests' target).
pub(super) const HOME: &str = "ws-0000beef";

/// The hub's fold of `lines`: no card open, no page known.
fn fold_lines(lines: &[bise_proto::thread::Line]) -> Vec<Entry> {
    let none = |_: &str| None;
    let ctx = Ctx {
        open_cards: &[],
        page: &none,
        provider: &|_: &str, k: &str| k.to_string(),
        width: &unicode_width::UnicodeWidthStr::width,
        offset: &|_| 0,
        attached: &bise_proto::thread::Attached::plain,
    };
    thread::fold(lines, &ctx)
}

/// An older state's agent as the hub's row (client-protocol step 5:
/// the tests keep the older state's words, the hub sends rows).
fn agent_row(a: &Value) -> Value {
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    let word = s("status");
    let status = match word.as_str() {
        "starting" | "working" => "working",
        "waiting" => "waiting",
        "blocked" => "blocked",
        "failed" => "failed",
        "done" | "stopped" => "done",
        "archived" => "done",
        _ => "idle",
    };
    let mut r = json!({"name": s("name"), "main": a["main"].as_bool().unwrap_or(false), "status": status,
        "archived": word == "archived", "title": "", "purpose": "", "since_ms": 0, "waits": 0,
        "objective": s("objective"), "note": s("note")});
    if !s("report").is_empty() {
        let kind = if matches!(status, "done" | "failed" | "blocked") { status } else { "progress" };
        r["report"] = json!({"kind": kind, "text": s("report"), "at_ms": a["report_ms"].as_u64().unwrap_or(0)});
    }
    for k in ["created_ms", "turn_ms"] {
        if a[k].is_u64() {
            r[k] = a[k].clone();
        }
    }
    r
}

/// An older state's card as the hub's row.
fn card_row(c: &Value) -> Value {
    let text = c["text"].as_str().unwrap_or("");
    let (question, _) = crate::sb::split_choices(text);
    let mut r = json!({"id": c["id"], "project": HOME, "kind": c["kind"], "agent": c["agent"], "question": question,
        "options": [], "urgent": false, "since_ms": 0, "text": text});
    for k in ["page", "batch"] {
        if c[k].is_object() {
            r[k] = c[k].clone();
        }
    }
    r
}

/// An older page as the hub's row.
fn page_row(p: &Value) -> Value {
    let mut r = json!({"id": p["id"], "title": p["title"].as_str().unwrap_or(""), "agent": p["agent"].as_str().unwrap_or(""), "version": p["version"].as_u64().unwrap_or(1),
        "url": p["url"].as_str().unwrap_or(""), "at_ms": p["at_ms"].as_u64().unwrap_or(0), "state": p["state"].as_str().unwrap_or("ready")});
    if let Some(o) = p.as_object() {
        for (k, v) in o {
            if r.get(k).is_none() && k != "ev" && !v.is_null() {
                r[k] = v.clone();
            }
        }
    }
    r
}

impl HubEnd {
    pub(super) fn new(w: UnixStream, r: BufReader<UnixStream>) -> HubEnd {
        HubEnd { w, r, rpc: None, ahead: Default::default(), init: None, pending: Vec::new(), seq: 0, answered: false, threads: Default::default(), main_sub: None, pages_url: None }
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
        // the home connection: `initialize` answered before anything else
        if self.init.is_some() && !self.answered && tag != "refused" {
            let project = if tag == "welcome" { v["project"].as_str().unwrap_or(HOME).to_string() } else { HOME.into() };
            self.initialized(&project, None);
        }
        // the older home events the tests still say, as the hub sends them now
        match tag.as_str() {
            "welcome" => return,
            "ready" => return self.ready(),
            "line" => return self.feed(v["agent"].as_str().unwrap_or(""), v["line"].as_str().unwrap_or("")),
            "state" => return self.state(&v),
            "page" => {
                let mut p = page_row(&v);
                p["ev"] = json!("page_changed");
                p["project"] = json!(HOME);
                return self.say(p);
            }
            _ => {}
        }
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

    /// One line of `agent`'s thread: what it makes or changes goes as
    /// `thread/entry` when the core subscribed it.
    pub(super) fn line(&mut self, agent: &str, line: &str) {
        self.say(json!({"ev": "line", "agent": agent, "line": line}));
    }

    fn feed(&mut self, agent: &str, line: &str) {
        let t = self.threads.entry(agent.to_string()).or_default();
        let pos = t.lines.last().map_or(1, |l| l.0 + 1);
        t.lines.push((pos, 1_000 + pos, line.to_string()));
        let now = fold_lines(&t.lines);
        let changed: Vec<Entry> = now.iter().filter(|e| !t.sent.contains(e)).cloned().collect();
        let live = t.live;
        if live {
            t.sent = now;
            for e in changed {
                self.say(json!({"ev": "entry", "project": HOME, "agent": agent, "entry": e}));
            }
        }
    }

    /// `agent`'s thread as the hub answers `thread/subscribe` (its last
    /// `limit` entries), live from now.
    fn page_of(&mut self, agent: &str, limit: usize) -> Value {
        let t = self.threads.entry(agent.to_string()).or_default();
        t.sent = fold_lines(&t.lines);
        t.live = true;
        let skip = t.sent.len().saturating_sub(limit);
        let entries: Vec<Entry> = t.sent[skip..].to_vec();
        json!({"project": HOME, "agent": agent, "entries": entries, "before": null, "more": skip > 0})
    }

    /// `initialize` answered when the core sent it and nothing answered
    /// it yet (bise's home hub: [`HOME`]).
    pub(super) fn answer_init(&mut self) {
        self.read_ahead();
        if self.init.is_some() && !self.answered {
            self.initialized(HOME, None);
        }
    }

    /// The core subscribed main's thread (its `initialize` read).
    pub(super) fn main_subscribed(&mut self) -> bool {
        self.read_ahead();
        self.main_sub.is_some()
    }

    /// main's first page (the old `ready`: what main said before is a
    /// replay).
    fn ready(&mut self) {
        self.read_ahead();
        let Some(id) = self.main_sub.take() else { return };
        let page = self.page_of("main", super::super::core::MAIN_PAGE);
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": page}));
    }

    /// An older state as the hub's rows, each kind as its notification.
    fn state(&mut self, v: &Value) {
        let list = |k: &str| v[k].as_array().cloned().unwrap_or_default();
        let agents: Vec<Value> = list("agents").iter().map(agent_row).collect();
        let cards: Vec<Value> = list("cards").iter().map(card_row).collect();
        self.say(json!({"ev": "agents", "project": HOME, "agents": agents}));
        self.say(json!({"ev": "cards", "project": HOME, "cards": cards}));
        if v.get("pages").is_some() {
            let pages: Vec<Value> = list("pages").iter().map(page_row).collect();
            self.say(json!({"ev": "pages", "project": HOME, "items": pages, "overdue": 0}));
        }
        if let Some(t) = v.get("timers") {
            self.say(json!({"ev": "scheduled", "project": HOME, "items": t}));
        }
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
        // the capsule's threads (core/home.rs): main's answered at
        // `ready`, another's at once; never a window command
        if v["method"] == "thread/subscribe" {
            let agent = v["params"]["agent"].as_str().unwrap_or("").to_string();
            match v["params"]["limit"].as_u64().map(|l| l as usize) {
                Some(super::super::core::MAIN_PAGE) if agent == "main" => return self.main_sub = Some(v["id"].clone()),
                Some(super::super::core::QUIET_PAGE) => {
                    let page = self.page_of(&agent, super::super::core::QUIET_PAGE);
                    return self.write(json!({"jsonrpc": "2.0", "id": v["id"], "result": page}));
                }
                _ => {}
            }
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
            self.ahead.push_back(cmd.to_value());
            // his words in main's thread, as the hub writes them
            if m == "turn/send" && v["params"]["agent"] == "main" {
                let text = v["params"]["text"].as_str().unwrap_or("").replace('\n', "\\n");
                self.feed("main", &format!("sb you : {text}"));
            }
            return;
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
        self.answered = true;
        let result = bise_proto::rpc::InitializeResult {
            project: project.into(),
            proto: bise_proto::PROTO,
            workspace: format!("/p/{project}"),
            name: project.into(),
            exe: String::new(),
            state_dir: String::new(),
            version: Value::Null,
            reload: String::new(),
            pages_url: self.pages_url.clone(),
            methods: methods.unwrap_or_else(bise_proto::rpc::methods),
            notifications: bise_proto::rpc::notifications(),
            hub: bise_proto::rpc::HubState { watermark: bise_proto::rpc::Watermark { epoch: 1, seq: 0 }, state: vec![] },
        };
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }
}
