//! MCP App views in the hub (docs/mcp-apps.md, C5, risks 1 and 3): the
//! shell's side of `view/resource`, `view/call`, `view/message` and
//! `view/context`, one dispatcher for hub.sock and the pages server's
//! `/api/view` ([`Shell::view_request`]). The decisions are elsewhere:
//! what a request names and what a resource holds (`crate::view_call`),
//! the limits (`crate::view_limits`), which tools a view may call
//! (`bend_plugins::visibility`), whether a call needs his ok
//! (`approvals::judge` through the gate's off-turn entry, gate.rs), who is
//! told (sb-core: the view's card, the view's message).
//!
//! - a request's I/O (the views file, the bridge) runs on a thread; it
//!   comes back to the loop as a [`ViewStep`], and its answer goes to
//!   its [`ViewDone`] (a client's response, a page's reply);
//! - a call goes to the agent's own bridge while its REPL lives
//!   (`$BEND_RUN_DIR/<port>/plugins/bridge.json`), else to the hub's
//!   views bridge ([`ViewsBridge`]: `bise plugins serve --parent <hub>`,
//!   started on the first need, stopped after its idle time or with the
//!   hub, its folder `Paths::views_bridge_dir` so a restarted hub ends an
//!   orphan);
//! - a call the model could also make waits on a `confirm` card that
//!   sb-core opens for its ask (the tool and its arguments, typed on the
//!   card) and names in the step's `view_card` effect, never the agent's
//!   own wait; his answer comes back by the card's id
//!   ([`Shell::view_card_answered`]);
//! - a server that is not the one that ran the view's call (the views
//!   bridge, a new bridge) is said once, on the call's result
//!   (`view_call::RESTARTED_META`): the call ran, its result is kept;
//! - at boot, a view's card left open has no call behind it any more:
//!   sb-core answers it no and closes it (law boot_answers_view_cards);
//!   the shell only ends a views bridge an older hub left
//!   ([`Shell::views_boot`]);
//! - `view/context` is kept per (agent, view) and joins his next send to
//!   that agent once ([`Shell::take_view_contexts`]); lost when the hub
//!   restarts (architect m_49840).

use super::{log_line, ClientId, Msg, Shell};
use crate::core::Input;
use crate::util::now_ms;
use crate::view_call::{self, ViewFile, ViewParams};
use crate::view_limits::{Limits, Servers, ViewKey};
use bend_plugins::visibility::{self, Ask, Verdict, ViewOf};
use bise_proto::hub::HubCmd;
use bise_proto::jsonrpc::{ErrorData, Id, RpcError};
use bise_proto::parts::Part;
use bise_proto::view_rpc::{ViewError, ViewResource};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where a view request's answer goes: a client's response, a page's
/// reply. Called once, from any thread.
pub(crate) type ViewDone = Box<dyn FnOnce(Result<Value, RpcError>) + Send>;

/// A view request's next step on the loop, from its thread.
pub(super) type ViewStep = Box<dyn FnOnce(&mut Shell) + Send>;

/// How long a thread waits for a bridge's address (bise_proto's, part of
/// the bound clients hold a view's call for: view_rpc::VIEW_CALL_MAX_MS).
const BRIDGE_WAIT: Duration = Duration::from_millis(bise_proto::view_rpc::VIEW_BRIDGE_MS);
/// One request to a view's server (likewise).
const CALL_TIMEOUT: Duration = Duration::from_millis(bise_proto::view_rpc::VIEW_SERVER_MS);

/// `data.reason` of a request whose server could not be reached.
pub(crate) const REASON_SERVER_GONE: &str = "server-gone";

/// The hub's views: the limits, the calls waiting for his ok, the views'
/// contexts, the bridges their calls went through, the views bridge.
#[derive(Default)]
pub(super) struct Views {
    limits: Limits,
    /// the views' calls waiting on each card, by card id: the same ask
    /// asked again joins its open card (sb-core's join, d3)
    waits: BTreeMap<u64, Vec<Wait>>,
    /// each agent's views' context for his next send: call -> part
    contexts: BTreeMap<String, BTreeMap<String, Part>>,
    /// the bridge each view's calls last went through (a restart said
    /// once per view), pruned with the limits
    servers: Servers,
    bridge: ViewsBridge,
}

impl Views {
    /// A call about to go through `route`: true once per view when its
    /// server is not the one before (another bridge, or the views bridge
    /// for its first call: the REPL that ran it is gone).
    fn server_restarted(&mut self, key: &ViewKey, route: &Route) -> bool {
        self.servers.restarted(key, &route.base, route.hubs, now_ms())
    }
}

/// A view's call that waits for his answer on its card.
struct Wait {
    key: ViewKey,
    route: Route,
    method: String,
    params: Value,
    summary: String,
    done: ViewDone,
}

/// Where a view's server is: its URL on a bridge, and whether that bridge
/// is the hub's views bridge.
#[derive(Clone, Debug)]
struct Route {
    base: String,
    url: String,
    hubs: bool,
}

/// The waiters of one card as one call: the first's ask (theirs are the
/// same: sb-core joins only the same ask) and one answer for them all,
/// each getting it once.
fn joined(waits: Vec<Wait>) -> Option<(Wait, ViewDone)> {
    let mut it = waits.into_iter();
    let mut first = it.next()?;
    let mut dones: Vec<ViewDone> = it.map(|w| w.done).collect();
    if dones.is_empty() {
        let done = std::mem::replace(&mut first.done, Box::new(|_| {}));
        return Some((first, done));
    }
    dones.insert(0, std::mem::replace(&mut first.done, Box::new(|_| {})));
    Some((first, Box::new(move |r: Result<Value, RpcError>| dones.into_iter().for_each(|d| d(r.clone())))))
}

/// The card a step's `view_card` effect names for `key`'s call.
fn card_said(fx: &[crate::core::Effect], key: &ViewKey) -> Option<u64> {
    fx.iter().find_map(|e| match e {
        crate::core::Effect::ViewCard { agent, call, card } if *agent == key.agent && *call == key.call => *card,
        _ => None,
    })
}

/// A C5 command as its method and params.
fn method_of(cmd: HubCmd) -> Option<(&'static str, Value)> {
    Some(match cmd {
        HubCmd::ViewResource { agent, call, .. } => ("view/resource", json!({"agent": agent, "call": call})),
        HubCmd::ViewCall { agent, call, method, params, .. } => ("view/call", json!({"agent": agent, "call": call, "method": method, "params": params})),
        HubCmd::ViewMessage { agent, call, content, gesture_ms, .. } => ("view/message", json!({"agent": agent, "call": call, "content": content, "gesture_ms": gesture_ms})),
        HubCmd::ViewContext { agent, call, content, .. } => ("view/context", json!({"agent": agent, "call": call, "content": content})),
        HubCmd::PluginViews { plugin, .. } => ("plugin/views", json!({"plugin": plugin})),
        _ => return None,
    })
}

/// A server's JSON-RPC error, as the view gets it.
fn server_error(e: &Value) -> RpcError {
    RpcError {
        code: e.get("code").and_then(Value::as_i64).unwrap_or(-32603),
        message: e.get("message").and_then(Value::as_str).unwrap_or("the server failed").to_string(),
        data: None,
    }
}

/// A request's server could not be reached ([`gone`]'s).
fn is_gone(e: &RpcError) -> bool {
    e.data.as_ref().and_then(|d| d.reason.as_deref()) == Some(REASON_SERVER_GONE)
}

fn gone(why: &str) -> RpcError {
    RpcError { code: -32000, message: format!("the view's server is not reachable: {why}"), data: Some(ErrorData { reason: Some(REASON_SERVER_GONE.into()), ..Default::default() }) }
}

/// One JSON-RPC request to a bridge route (`http://127.0.0.1:<port>/…`),
/// its answer's JSON.
fn post(url: &str, body: &Value, timeout: Duration) -> Result<Value, String> {
    let rest = url.strip_prefix("http://").ok_or("not a loopback URL")?;
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).ok_or("no path")?;
    let mut s = std::net::TcpStream::connect(host).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let body = body.to_string();
    write!(s, "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out);
    let (head, body) = text.split_once("\r\n\r\n").ok_or("no answer")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(head.lines().next().unwrap_or("").to_string());
    }
    serde_json::from_str(body).map_err(|e| e.to_string())
}

/// A view's call judged off the loop: its views file (`file`), its
/// bridge (`bridge.json` and whether it is the views bridge), the server's
/// tools for a `tools/call`, the plugins' state: the view, where its
/// server is, and the verdict (`bend_plugins::visibility`).
fn judge_call(file: &Path, bridge: Result<(PathBuf, bool), String>, method: &str, inner: &Value, state: &bend_plugins::model::StateV2) -> Result<(ViewFile, Route, Verdict), RpcError> {
    let what = Ask::of(method, inner).what().to_string();
    let text = std::fs::read_to_string(file).map_err(|_| ViewError::Expired.rpc(&what))?;
    let f = ViewFile::parse(&text).map_err(|e| e.rpc(&what))?;
    let (json_path, hubs) = bridge.map_err(|e| gone(&e))?;
    let base = wait_base(&json_path).ok_or_else(|| gone("no bridge"))?;
    let url = view_call::route_url(&base, &f.plugin, &f.server);
    let tools = if method == "tools/call" {
        let got = post(&url, &view_call::request("tools/list", &json!({})), CALL_TIMEOUT).map_err(|e| gone(&e))?;
        view_call::tools(&view_call::answer(&got).map_err(|e| server_error(&e))?)
    } else {
        Vec::new()
    };
    let view = ViewOf { plugin: &f.plugin, server: &f.server, opt_in: false };
    let verdict = visibility::judge(view, Ask::of(method, inner), &tools, state);
    Ok((f, Route { base, url, hubs }, verdict))
}

/// `bridge.json`'s base, waited for up to [`BRIDGE_WAIT`].
fn wait_base(path: &Path) -> Option<String> {
    let until = std::time::Instant::now() + BRIDGE_WAIT;
    loop {
        if let Some(b) = std::fs::read_to_string(path).ok().as_deref().and_then(view_call::bridge_base) {
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
pub(super) struct ViewsBridge {
    child: Option<std::process::Child>,
    last_use: u64,
}

impl ViewsBridge {
    /// Its `bridge.json` (`json`, in `dir`), the bridge started when it
    /// isn't running.
    fn ensure(&mut self, exe: &Path, dir: &Path, json: &Path, workspace: &Path, now: u64) -> Result<PathBuf, String> {
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
    fn stop_idle(&mut self, now: u64) {
        let idle = bend_plugins::tools_cache::IDLE_STOP.as_millis() as u64;
        if self.child.is_some() && now.saturating_sub(self.last_use) >= idle {
            self.stop();
        }
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// A views bridge an older hub left (its `bridge.json`'s pid, when that
/// pid is still a `plugins serve`): ended. Its own parent watch ends it
/// once its hub is gone; this covers a pid that watch missed.
pub(super) fn kill_orphan_bridge(file: &Path) {
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

/// An agent's REPL runs: its `repl.pid` names a live process.
fn repl_alive(pid_file: &Path) -> bool {
    std::fs::read_to_string(pid_file).ok().and_then(|t| t.trim().parse::<u32>().ok()).is_some_and(|p| p > 1 && crate::procs::alive(p))
}

/// One GET to a bridge route, its answer's JSON.
fn get(url: &str, timeout: Duration) -> Result<Value, String> {
    let rest = url.strip_prefix("http://").ok_or("not a loopback URL")?;
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).ok_or("no path")?;
    let mut s = std::net::TcpStream::connect(host).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out);
    let (head, body) = text.split_once("\r\n\r\n").ok_or("no answer")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(head.lines().next().unwrap_or("").to_string());
    }
    serde_json::from_str(body).map_err(|e| e.to_string())
}

impl Shell {
    /// Boot: a views bridge an older hub left ends (its view cards are
    /// sb-core's to close, at its boot step).
    pub(super) fn views_boot(&mut self) {
        kill_orphan_bridge(&self.opts.paths.views_bridge_json());
    }

    /// The hub's tick: the views bridge idle long enough stops, the limits
    /// forget old views, a call whose card closed without his answer (its
    /// agent stopped) gets its no.
    pub(super) fn views_tick(&mut self) {
        let now = now_ms();
        self.views.bridge.stop_idle(now);
        self.views.limits.prune(now);
        self.views.servers.prune(now);
        let gone: Vec<u64> = self.views.waits.keys().filter(|id| !self.hub.st.cards.contains_key(id)).copied().collect();
        for id in gone {
            for w in self.views.waits.remove(&id).unwrap_or_default() {
                (w.done)(Err(ViewError::Denied.rpc(&w.summary)));
            }
        }
    }

    /// The hub stops: its views bridge with it.
    pub(super) fn views_stop(&mut self) {
        self.views.bridge.stop();
    }

    /// A client's C5 request (`hub.sock`): answered to `client` by `rid`.
    pub(super) fn view_rpc(&mut self, client: ClientId, rid: Id, cmd: HubCmd) {
        let Some((method, params)) = method_of(cmd) else { return };
        let tx = self.tx.clone();
        let done: ViewDone = Box::new(move |r| {
            let step: ViewStep = Box::new(move |sh: &mut Shell| sh.rpc_answer(client, rid, r));
            let _ = tx.send(Msg::View(step));
        });
        self.view_request(method, params, done);
    }

    /// One view request (C5's methods), from hub.sock or `/api/view`.
    pub(crate) fn view_request(&mut self, method: &str, params: Value, done: ViewDone) {
        if method == "plugin/views" {
            return self.plugin_views(params, done);
        }
        let p = match ViewParams::of(&params) {
            Ok(p) => p,
            Err(e) => return done(Err(e.rpc(method))),
        };
        let Some(dir) = self.dir_of(&p.agent) else { return done(Err(ViewError::Expired.rpc(method))) };
        match method {
            "view/resource" => self.view_resource(p, dir, done, true),
            "view/call" => self.view_call(p, dir, params, done),
            "view/message" => self.view_message(p, dir, params, done),
            "view/context" => self.view_context(p, dir, params, done),
            m => done(Err(ViewError::NotSupported.rpc(m))),
        }
    }

    /// Hands a thread's next step to the loop.
    fn view_step(&self) -> impl Fn(ViewStep) + Send + 'static {
        let tx = self.tx.clone();
        move |s: ViewStep| {
            let _ = tx.send(Msg::View(s));
        }
    }

    /// Where a view's file is (C2).
    fn view_file(&self, dir: &str, call: &str) -> PathBuf {
        self.opts.paths.agent_views_dir(dir).join(format!("{call}.json"))
    }

    /// C4: a live view whose file is gone reads `expired` before its
    /// entries leave for a client (agent folder `dir`).
    pub(in crate::daemon) fn expire_views(&self, dir: &str, entries: &mut [bise_proto::thread::Entry]) {
        bise_proto::thread::expire_views(entries, |call| !self.view_file(dir, call).exists());
    }

    /// The `bridge.json` a view's server is reached through: its agent's
    /// own bridge while its REPL lives (`own`: not tried yet), else the
    /// hub's views bridge.
    fn bridge_json(&mut self, dir: &str, own: bool) -> Result<(PathBuf, bool), String> {
        // its REPL alive, never only its port known: a REPL just killed
        // leaves a bridge that retires its servers ("turned off")
        if let Some(port) = self.ports.get(dir).filter(|_| own && repl_alive(&self.opts.paths.agent_dir(dir).join("repl.pid"))) {
            let own = self.opts.paths.agent_bridge_json(*port);
            if own.exists() {
                return Ok((own, false));
            }
        }
        let (exe, d, json, ws) = (self.opts.exe.clone(), self.opts.paths.views_bridge_dir(), self.opts.paths.views_bridge_json(), self.opts.paths.workspace.clone());
        self.views.bridge.ensure(&exe, &d, &json, &ws, now_ms()).map(|p| (p, true))
    }

    // ---- plugin/views (C8) ----

    /// A plugin's views as its servers declare them, through the hub's
    /// views bridge (`<base>/<plugin>/~ui`), its answer as is; the plugin
    /// off: `plugin-off`.
    fn plugin_views(&mut self, params: Value, done: ViewDone) {
        const WHAT: &str = "plugin/views";
        let plugin = params.get("plugin").and_then(Value::as_str).unwrap_or("").to_string();
        if plugin.is_empty() || plugin.contains('/') || plugin.starts_with('.') {
            return done(Err(RpcError { code: bise_proto::rpc::code::INVALID_PARAMS, message: "plugin/views needs a plugin name".into(), data: None }));
        }
        let state = bend_plugins::state::read(&bend_plugins::state::state_path());
        if !state.is_on(&bend_plugins::id::PluginId::plugin(&plugin), false) {
            return done(Err(ViewError::PluginOff.rpc(WHAT)));
        }
        let (exe, d, json, ws) = (self.opts.exe.clone(), self.opts.paths.views_bridge_dir(), self.opts.paths.views_bridge_json(), self.opts.paths.workspace.clone());
        let bridge = self.views.bridge.ensure(&exe, &d, &json, &ws, now_ms());
        std::thread::spawn(move || {
            let r = (|| {
                let json_path = bridge.map_err(|e| gone(&e))?;
                let base = wait_base(&json_path).ok_or_else(|| gone("no bridge"))?;
                let url = format!("{base}/{plugin}{}", bend_plugins::bridge::UI_ROUTE);
                get(&url, CALL_TIMEOUT).map_err(|e| gone(&e))
            })();
            done(r);
        });
    }

    // ---- view/resource ----

    /// `own`: its agent's bridge may be tried (a REPL that just died
    /// leaves one that resets: then once more on the views bridge).
    fn view_resource(&mut self, p: ViewParams, dir: String, done: ViewDone, own: bool) {
        let file = self.view_file(&dir, &p.call);
        let bridge = self.bridge_json(&dir, own);
        let via_own = matches!(bridge, Ok((_, false)));
        let step = self.view_step();
        std::thread::spawn(move || {
            let read = || -> Result<(ViewFile, view_call::ResourceRead, Value, Vec<String>), RpcError> {
                let text = std::fs::read_to_string(&file).map_err(|_| ViewError::Expired.rpc("view/resource"))?;
                let f = ViewFile::parse(&text).map_err(|e| e.rpc("view/resource"))?;
                let (json_path, _) = bridge.map_err(|e| gone(&e))?;
                let base = wait_base(&json_path).ok_or_else(|| gone("no bridge"))?;
                let url = view_call::route_url(&base, &f.plugin, &f.server);
                let got = post(&url, &view_call::request("resources/read", &json!({"uri": f.uri})), CALL_TIMEOUT).map_err(|e| gone(&e))?;
                let res = view_call::answer(&got).map_err(|e| server_error(&e))?;
                let r = view_call::resource_read(&res).ok_or_else(|| ViewError::Expired.rpc(&f.uri))?;
                let list = post(&url, &view_call::request("tools/list", &json!({})), CALL_TIMEOUT).ok().and_then(|v| view_call::answer(&v).ok()).unwrap_or_default();
                let tool = view_call::tool_of(&list, &f.tool);
                // C8: what he granted this view (never asked: nothing)
                let grant = bend_plugins::state::view_grant(&bend_plugins::state::grants(&bend_plugins::state::state_path()), &f.plugin, &f.uri).unwrap_or_default();
                Ok((f, r, tool, grant))
            };
            match read() {
                Err(e) if via_own && is_gone(&e) => step(Box::new(move |sh: &mut Shell| sh.view_resource(p, dir, done, false))),
                Err(e) => done(Err(e)),
                Ok((f, r, tool, grant)) => step(Box::new(move |sh: &mut Shell| sh.view_resource_read(f, r, tool, grant, done))),
            }
        });
    }

    /// The resource is read: its sandbox registered on the pages server,
    /// then its answer.
    fn view_resource_read(&mut self, f: ViewFile, r: view_call::ResourceRead, tool: Value, grant: Vec<String>, done: ViewDone) {
        let Some(pages) = self.pg.pages.clone() else {
            return done(Err(RpcError::refused("this hub serves no pages: views need its pages server", None)));
        };
        let permissions = view_call::granted(&r.declared, &grant);
        let hash = crate::pages::views::hash(&f.plugin, &f.server, &f.uri);
        let session = pages.view_sandbox(&hash, crate::pages::views::ViewSandbox { csp: r.csp.clone(), perms: permissions.clone() });
        let origin = crate::pages::views::origin(&f.plugin, &f.server, &f.uri, pages.port);
        let res = ViewResource {
            proxy: Some(crate::pages::views::proxy_url(&origin, &session)),
            origin,
            html: r.html,
            csp: r.csp,
            permissions,
            declared: r.declared,
            prefers_border: r.prefers_border,
            tool_input: f.args,
            tool_result: f.result,
            tool,
        };
        done(Ok(serde_json::to_value(res).unwrap_or_default()));
    }

    // ---- view/call ----

    fn view_call(&mut self, p: ViewParams, dir: String, params: Value, done: ViewDone) {
        let method = params.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let inner = params.get("params").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
        let what = Ask::of(&method, &inner).what().to_string();
        // piece 12 (q-vibe): Le Chat's extras need no file nor server
        let none = ViewOf { plugin: "", server: "", opt_in: false };
        if visibility::judge(none, Ask::of(&method, &inner), &[], &Default::default()) == Verdict::Refused(ViewError::NotSupported) {
            return done(Err(ViewError::NotSupported.rpc(&what)));
        }
        let key = ViewKey::new(&p.agent, &p.call);
        if !self.views.limits.call(&key, now_ms()) {
            return done(Err(ViewError::RateLimited.rpc(&what)));
        }
        self.view_call_via(key, dir, method, inner, done, true);
    }

    /// A call judged through a bridge (`own`: its agent's may be tried;
    /// it reset or refused the connection: once more on the views
    /// bridge, judging read nothing but the tools).
    fn view_call_via(&mut self, key: ViewKey, dir: String, method: String, inner: Value, done: ViewDone, own: bool) {
        let file = self.view_file(&dir, &key.call);
        let bridge = self.bridge_json(&dir, own);
        let via_own = matches!(bridge, Ok((_, false)));
        let state_path = bend_plugins::state::state_path();
        let step = self.view_step();
        std::thread::spawn(move || {
            let state = bend_plugins::state::read(&state_path);
            match judge_call(&file, bridge, &method, &inner, &state) {
                Err(e) if via_own && is_gone(&e) => step(Box::new(move |sh: &mut Shell| sh.view_call_via(key, dir, method, inner, done, false))),
                Err(e) => done(Err(e)),
                Ok((f, route, verdict)) => step(Box::new(move |sh: &mut Shell| sh.view_call_judged(key, dir, f, route, method, inner, verdict, done))),
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn view_call_judged(&mut self, key: ViewKey, dir: String, f: ViewFile, route: Route, method: String, params: Value, verdict: Verdict, done: ViewDone) {
        let what = Ask::of(&method, &params).what().to_string();
        match verdict {
            Verdict::Refused(e) => done(Err(e.rpc(&what))),
            Verdict::Free => self.view_forward(key, route, method, params, done),
            Verdict::Ask => {
                let tool = format!("{}.{}", f.plugin, what);
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match self.view_gate(&dir, &key.agent, &tool, &args) {
                    Ok(true) => self.view_forward(key, route, method, params, done),
                    Ok(false) => self.view_card(key, route, method, params, &f.plugin, &what, tool, done),
                    Err(hint) => done(Err(RpcError { message: hint, ..ViewError::Denied.rpc(&what) })),
                }
            }
        }
    }

    /// A card for a view's call (sb-core's view_confirm_open): its id is
    /// the step's `view_card` effect (none: its agent is not active); his
    /// answer comes back by it.
    #[allow(clippy::too_many_arguments)]
    fn view_card(&mut self, key: ViewKey, route: Route, method: String, params: Value, plugin: &str, what: &str, summary: String, done: ViewDone) {
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let text = view_call::card_text(plugin, what, &args);
        let ask = view_call::ask(what, &args);
        let fx = self.hub.handle(Input::ViewConfirmOpen { agent: key.agent.clone(), call: key.call.clone(), text, ask }, &mut self.env);
        let card = card_said(&fx, &key);
        self.step_fx(fx);
        match card {
            Some(id) => self.views.waits.entry(id).or_default().push(Wait { key, route, method, params, summary, done }),
            None => done(Err(ViewError::Denied.rpc(what))),
        }
    }

    /// His answer on a view's card (the gate's `confirm` effect): true
    /// when the card was a view's.
    /// Joined asks (the same tool and arguments) run once: every waiter
    /// gets that one result.
    pub(super) fn view_card_answered(&mut self, card: u64, allow: bool) -> bool {
        let Some(waits) = self.views.waits.remove(&card) else { return false };
        let Some((w, done)) = joined(waits) else { return false };
        let how = if allow { super::gate::Fold::Allowed } else { super::gate::Fold::No };
        let fold = super::gate::fold_line(how, std::slice::from_ref(&w.key.agent), &w.summary, "");
        let agent = w.key.agent.clone();
        self.feed(&agent, &fold);
        if allow {
            self.view_forward(w.key, w.route, w.method, w.params, done);
        } else {
            done(Err(ViewError::Denied.rpc(&w.summary)));
        }
        true
    }

    /// The call goes to its server; a server that is not the one before
    /// is said once, on its result (`_meta["dev.bise/server-restarted"]`).
    fn view_forward(&mut self, key: ViewKey, route: Route, method: String, params: Value, done: ViewDone) {
        let restarted = self.views.server_restarted(&key, &route);
        std::thread::spawn(move || {
            let r = match post(&route.url, &view_call::request(&method, &params), CALL_TIMEOUT) {
                Err(e) => Err(gone(&e)),
                Ok(v) => match view_call::answer(&v) {
                    Ok(r) => Ok(view_call::mark_restarted(r, restarted)),
                    Err(e) => Err(server_error(&e)),
                },
            };
            done(r);
        });
    }

    // ---- view/message, view/context ----

    /// The views file of a call, read now (a message and a context need
    /// only its plugin and uri).
    fn view_file_now(&self, dir: &str, call: &str) -> Result<ViewFile, ViewError> {
        let text = std::fs::read_to_string(self.view_file(dir, call)).map_err(|_| ViewError::Expired)?;
        ViewFile::parse(&text)
    }

    fn view_message(&mut self, p: ViewParams, dir: String, params: Value, done: ViewDone) {
        const WHAT: &str = "ui/message";
        let key = ViewKey::new(&p.agent, &p.call);
        let gesture = params.get("gesture_ms").and_then(Value::as_u64);
        let text = match view_call::text_of(params.get("content").unwrap_or(&Value::Null)) {
            Ok(t) => t,
            Err(e) => return done(Err(e.rpc(WHAT))),
        };
        let plugin = match self.view_file_now(&dir, &p.call) {
            Ok(f) => f.plugin,
            Err(e) => return done(Err(e.rpc(WHAT))),
        };
        if let Err(why) = self.views.limits.message(&key, now_ms(), gesture) {
            return done(Err(why.error().rpc(WHAT)));
        }
        if self.hub.st.agents.get(&p.agent).is_none_or(|a| a.lifecycle != crate::model::Lifecycle::Active) {
            return done(Err(RpcError::refused(format!("@{} is not active: the view's message reached nobody", p.agent), None)));
        }
        log_line(&self.opts.paths, &format!("view message from {plugin} to {} ({} chars)", p.agent, text.len()));
        self.step(Input::ViewMessage { agent: p.agent, plugin, text });
        done(Ok(json!({})));
    }

    fn view_context(&mut self, p: ViewParams, dir: String, params: Value, done: ViewDone) {
        const WHAT: &str = "ui/update-model-context";
        let text = match view_call::text_of(params.get("content").unwrap_or(&Value::Null)) {
            Ok(t) => t,
            Err(e) => return done(Err(e.rpc(WHAT))),
        };
        let f = match self.view_file_now(&dir, &p.call) {
            Ok(f) => f,
            Err(e) => return done(Err(e.rpc(WHAT))),
        };
        let part = Part::ViewContext { plugin: f.plugin, uri: f.uri, text };
        self.views.contexts.entry(p.agent).or_default().insert(p.call, part);
        done(Ok(json!({})));
    }

    /// His send to `agent` (its model text and parts' JSON, "" none):
    /// the views' contexts waiting for it join it once, before his words.
    pub(super) fn with_view_contexts(&mut self, agent: &str, text: String, parts: String) -> (String, String) {
        let Some(ctx) = self.views.contexts.remove(agent) else { return (text, parts) };
        join_contexts(ctx.into_values().collect(), text, parts)
    }
}

/// `contexts` before his send's parts (his words as a text part when he
/// sent none): the model's text rendered again from them all.
fn join_contexts(contexts: Vec<Part>, text: String, parts: String) -> (String, String) {
    if contexts.is_empty() {
        return (text, parts);
    }
    let his: Vec<Part> = match parts.is_empty() {
        true => vec![Part::Text { text }],
        false => serde_json::from_str(&parts).unwrap_or_default(),
    };
    let all: Vec<Part> = contexts.into_iter().chain(his).collect();
    (bise_proto::parts::render(&all), serde_json::to_string(&all).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_views_context_joins_his_send_before_his_words() {
        let ctx = vec![Part::ViewContext { plugin: "fake-app".into(), uri: "ui://fake-app/counter".into(), text: "count=4".into() }];
        let (text, parts) = join_contexts(ctx.clone(), "bump it".into(), String::new());
        assert!(text.starts_with("<view_context plugin=\"fake-app\" uri=\"ui://fake-app/counter\">\ncount=4\n</view_context>"), "{text}");
        assert!(text.ends_with("bump it"), "{text}");
        let typed: Vec<Part> = serde_json::from_str(&parts).unwrap();
        assert_eq!(typed.len(), 2);
        assert_eq!(join_contexts(Vec::new(), "hi".into(), String::new()), ("hi".into(), String::new()));
    }

    /// A bridge on loopback serving fake-app's tools/list (the fixture's
    /// shapes, tests/fixtures/mcp-apps/fake-app): its `bridge.json` in
    /// `dir`, and the methods it was asked, in order.
    fn fake_bridge(dir: &Path) -> (PathBuf, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
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

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-views-{name}-{}-{}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// C5 on fake-app: an app-only tool runs, a model-visible one asks,
    /// a model-only one or another server's is refused, the plugin off
    /// refuses; the call goes to the view's own server on its bridge.
    #[test]
    fn a_views_call_is_judged_on_its_own_servers_tools() {
        let d = tmp("judge");
        let (bridge, asked) = fake_bridge(&d);
        let file = d.join("1-1.json");
        std::fs::write(&file, json!({"v": 1, "plugin": "fake-app", "server": "fake-app", "uri": "ui://fake-app/counter", "tool": "show_counter", "args": {}, "result": {}, "at_ms": 1}).to_string()).unwrap();
        let on = bend_plugins::model::StateV2::default();
        let call = |name: &str, state: &bend_plugins::model::StateV2| judge_call(&file, Ok((bridge.clone(), false)), "tools/call", &json!({"name": name}), state);
        let (f, route, v) = call("bump", &on).unwrap();
        assert_eq!((f.plugin.as_str(), v), ("fake-app", Verdict::Free), "app-only: no card");
        assert!(route.url.ends_with("/tok/fake-app/fake-app") && !route.hubs, "{route:?}");
        assert_eq!(call("show_counter", &on).unwrap().2, Verdict::Ask, "the model sees it too: his ok");
        assert_eq!(call("whisper", &on).unwrap().2, Verdict::Refused(ViewError::NotVisible), "model-only");
        assert_eq!(call("create_issue", &on).unwrap().2, Verdict::Refused(ViewError::NotVisible), "another server's tool");
        let off = bend_plugins::model::StateV2 { disabled: vec!["fake-app".into()], ..Default::default() };
        assert_eq!(call("bump", &off).unwrap().2, Verdict::Refused(ViewError::PluginOff));
        assert!(asked.lock().unwrap().iter().all(|m| m == "tools/list"), "judging runs nothing");
        // no file: expired; no bridge: the server is gone, typed
        let none = judge_call(&d.join("9-9.json"), Ok((bridge.clone(), false)), "tools/call", &json!({"name": "bump"}), &on).unwrap_err();
        assert_eq!(none.code, ViewError::Expired.rpc("bump").code);
        let gone = judge_call(&file, Err("no REPL".into()), "tools/call", &json!({"name": "bump"}), &on).unwrap_err();
        assert_eq!(gone.data.and_then(|d| d.reason).as_deref(), Some(REASON_SERVER_GONE));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A view whose agent's REPL went: its first call through the views
    /// bridge, or a call through a new bridge, is said restarted once.
    #[test]
    fn a_new_server_is_said_once_per_view() {
        let mut v = Views::default();
        let route = |base: &str, hubs: bool| Route { base: base.into(), url: format!("{base}/p/s"), hubs };
        let (a, b) = (ViewKey::new("docs", "1-1"), ViewKey::new("docs", "1-2"));
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:1/x", false)), "its own bridge: the server that ran it");
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:1/x", false)));
        assert!(v.server_restarted(&a, &route("http://127.0.0.1:2/y", true)), "another bridge");
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:3/z", false)), "once per view");
        assert!(v.server_restarted(&b, &route("http://127.0.0.1:2/y", true)), "first call on the views bridge");
        assert!(!v.server_restarted(&b, &route("http://127.0.0.1:2/y", true)));
    }

    /// C8: a plugin's views come back as the bridge gave them.
    #[test]
    fn a_plugins_views_are_the_bridges_answer_as_is() {
        let d = tmp("ui");
        let (bridge, _) = fake_bridge(&d);
        let base = wait_base(&bridge).unwrap();
        let got = get(&format!("{base}/fake-app{}", bend_plugins::bridge::UI_ROUTE), Duration::from_secs(5)).unwrap();
        assert_eq!(got["views"][0]["uri"], "ui://fake-app/counter");
        assert!(get("http://127.0.0.1:1/x", Duration::from_secs(1)).is_err(), "nobody there");
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
        std::fs::write(&f, "1").unwrap();
        assert!(!repl_alive(&f), "never launchd");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// d3: two waiters joined on one card get his one answer, once each.
    #[test]
    fn a_joined_cards_answer_reaches_every_waiter_once() {
        let got = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let wait = |n: u32| {
            let got = got.clone();
            Wait {
                key: ViewKey::new("docs", "1-1"),
                route: Route { base: "b".into(), url: format!("u{n}"), hubs: false },
                method: "tools/call".into(),
                params: json!({"name": "bump"}),
                summary: "bump".into(),
                done: Box::new(move |r| got.lock().unwrap().push((n, r.is_ok()))),
            }
        };
        let (w, done) = joined(vec![wait(1), wait(2), wait(3)]).unwrap();
        assert_eq!(w.route.url, "u1", "the first's ask runs");
        done(Ok(json!({})));
        assert_eq!(*got.lock().unwrap(), vec![(1, true), (2, true), (3, true)]);
        let (_, one) = joined(vec![wait(4)]).unwrap();
        one(Err(ViewError::Denied.rpc("bump")));
        assert_eq!(got.lock().unwrap().last(), Some(&(4, false)));
        assert!(joined(Vec::new()).is_none());
    }

    #[test]
    fn a_views_card_is_the_one_its_effect_names() {
        use crate::core::Effect;
        let k = ViewKey::new("docs", "1-2");
        let said = |agent: &str, call: &str, card| Effect::ViewCard { agent: agent.into(), call: call.into(), card };
        assert_eq!(card_said(&[Effect::State, said("docs", "1-2", Some(7))], &k), Some(7));
        assert_eq!(card_said(&[said("docs", "1-3", Some(8)), said("main", "1-2", Some(9))], &k), None, "another view's");
        assert_eq!(card_said(&[said("docs", "1-2", None)], &k), None, "not active: no card");
        assert_eq!(card_said(&[], &k), None);
    }

    #[test]
    fn a_c5_command_is_its_method_and_params() {
        let (m, p) = method_of(HubCmd::ViewMessage { project: "p".into(), agent: "docs".into(), call: "1-2".into(), content: json!([{"type": "text", "text": "hi"}]), gesture_ms: Some(5) }).unwrap();
        assert_eq!(m, "view/message");
        assert_eq!(p, json!({"agent": "docs", "call": "1-2", "content": [{"type": "text", "text": "hi"}], "gesture_ms": 5}));
        assert!(method_of(HubCmd::Usage { project: "p".into() }).is_none());
    }
}
