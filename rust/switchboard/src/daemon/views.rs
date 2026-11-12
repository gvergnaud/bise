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
//!   views bridge (its folder `Paths::views_bridge_dir`, so a restarted
//!   hub ends an orphan); the bridges' client and the views bridge's
//!   supervisor are `crate::view_bridge` (review d6), this file is the
//!   dispatch;
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
//! - `view/context` is capped here (`view_limits`: 16 KB, one a second
//!   per view) and goes to sb-core as a rendered `Part::ViewContext`;
//!   sb-core keeps it and joins it to his next message to that agent once,
//!   whatever client sent it (review d5; lost when the hub restarts,
//!   architect m_49840).

use super::{log_line, ClientId, Msg, Shell};
use crate::core::Input;
use crate::util::now_ms;
use crate::pages::view_folder::ViewFolder;
use crate::view_call::{self, ViewFile, ViewParams};
use crate::view_limits::{Limits, Servers, ViewKey};
use bend_plugins::visibility::{self, Ask, Verdict, ViewOf};
use bise_proto::hub::HubCmd;
use bise_proto::jsonrpc::{ErrorData, Id, RpcError};
use bise_proto::parts::Part;
use bise_proto::view_rpc::{ViewError, ViewResource};
use serde_json::{json, Value};
use crate::view_bridge::{kill_orphan_bridge, repl_alive, request, wait_base, ViewsBridge, CALL_TIMEOUT};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where a view request's answer goes: a client's response, a page's
/// reply. Called once, from any thread.
pub(crate) type ViewDone = Box<dyn FnOnce(Result<Value, RpcError>) + Send>;

/// A view request's next step on the loop, from its thread.
pub(super) type ViewStep = Box<dyn FnOnce(&mut Shell) + Send>;

/// `data.reason` of a request whose server could not be reached.
pub(crate) const REASON_SERVER_GONE: &str = "server-gone";

/// The hub's views: the limits, the calls waiting for his ok, the
/// bridges their calls went through, the views bridge.
#[derive(Default)]
pub(super) struct Views {
    limits: Limits,
    /// the views' calls waiting on each card, by card id: the same ask
    /// asked again joins its open card (sb-core's join, d3)
    waits: BTreeMap<u64, Vec<Wait>>,
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

/// Whether a step's `view_context` effect kept `key`'s context (its
/// words when not); none: no such effect (an older sb-core).
fn context_said(fx: &[crate::core::Effect], key: &ViewKey) -> Option<Result<(), String>> {
    fx.iter().find_map(|e| match e {
        crate::core::Effect::ViewContext { agent, call, kept, why } if *agent == key.agent && *call == key.call => Some(if *kept { Ok(()) } else { Err(why.clone()) }),
        _ => None,
    })
}

/// A C5 command as its method and params.
fn method_of(cmd: HubCmd) -> Option<(&'static str, Value)> {
    Some(match cmd {
        HubCmd::ViewResource { agent, call, .. } => ("view/resource", json!({"agent": agent, "call": call})),
        HubCmd::ViewCall { agent, call, doc, method, params, .. } => ("view/call", json!({"agent": some(agent), "call": some(call), "doc": doc, "method": method, "params": params})),
        HubCmd::ViewMessage { agent, call, content, gesture_ms, .. } => ("view/message", json!({"agent": agent, "call": call, "content": content, "gesture_ms": gesture_ms})),
        HubCmd::ViewContext { agent, call, doc, content, .. } => ("view/context", json!({"agent": some(agent), "call": some(call), "doc": doc, "content": content})),
        HubCmd::PluginViews { plugin, .. } => ("plugin/views", json!({"plugin": plugin})),
        HubCmd::ViewClose { doc, .. } => ("view/close", json!({"doc": doc})),
        HubCmd::ViewNotes { doc, .. } => ("view/notes", json!({"doc": doc})),
        HubCmd::ViewOpen { project, target, opener } => ("view/open", json!({"project": project, "target": target, "opener": opener})),
        HubCmd::ViewCopy { doc, text, blob, .. } => ("view/copy", json!({"doc": doc, "text": text, "blob": blob})),
        HubCmd::FolderList { folder, hidden, .. } => ("folder/list", json!({"folder": folder, "hidden": hidden})),
        HubCmd::FileNew { make, library_folder, in_folder, .. } => ("file/new", json!({"make": make, "library_folder": library_folder, "in": in_folder})),
        HubCmd::FilesIdsSubscribe { .. } => ("files/ids/subscribe", json!({})),
        HubCmd::FilesMint { path, .. } => ("files/mint", json!({"path": path})),
        _ => return None,
    })
}

/// A key the command left out (`""`) is absent, never an empty name
/// ([`view_call::Key::of`] refuses a request with both or neither).
fn some(s: String) -> Option<String> {
    Some(s).filter(|s| !s.is_empty())
}

/// A server's JSON-RPC error, as the view gets it.
pub(super) fn server_error(e: &Value) -> RpcError {
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

pub(super) fn gone(why: &str) -> RpcError {
    RpcError { code: -32000, message: format!("the view's server is not reachable: {why}"), data: Some(ErrorData { reason: Some(REASON_SERVER_GONE.into()), ..Default::default() }) }
}

/// The folder a static view (`~views`) may serve its own files from
/// (docs/plugin-apps.md A5): its `index.html`'s, in its plugin as the
/// resolver finds it now, tagged with that plugin's version. A server's
/// view: none (its `ui://` resource stays inline). Off the loop: it reads
/// the plugins.
pub(super) fn static_folder(f: &ViewFile, ws: &Path) -> Option<ViewFolder> {
    if f.server != bend_plugins::views::SERVER {
        return None;
    }
    let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(ws)));
    let p = res.loaded().find(|p| p.name == f.plugin)?;
    let v = p.views.iter().find(|v| v.uri == f.uri)?;
    Some(ViewFolder { dir: v.html.parent()?.to_path_buf(), tag: crate::pages::view_folder::tag(&p.root, p.version.as_deref()) })
}

/// A view's resource read on its server at `url` (the one read path of
/// `view/resource` and `view/open`, architect m_60316 #5): its HTML and
/// sandbox, its server's `tools/list` answer (none: `{}`), and what he
/// granted the view (C8; never asked: nothing). Off the loop.
pub(super) fn read_view(url: &str, plugin: &str, uri: &str) -> Result<(view_call::ResourceRead, Value, Vec<String>), RpcError> {
    let got = request(url, Some(&view_call::request("resources/read", &json!({"uri": uri}))), CALL_TIMEOUT).map_err(|e| gone(&e))?;
    let res = view_call::answer(&got).map_err(|e| server_error(&e))?;
    let r = view_call::resource_read(&res).ok_or_else(|| ViewError::Expired.rpc(uri))?;
    let list = request(url, Some(&view_call::request("tools/list", &json!({}))), CALL_TIMEOUT).ok().and_then(|v| view_call::answer(&v).ok()).unwrap_or_default();
    let grant = bend_plugins::state::view_grant(&bend_plugins::state::grants(&bend_plugins::state::state_path()), plugin, uri).unwrap_or_default();
    Ok((r, list, grant))
}

/// A view's call judged off the loop: its views file (`file`), its
/// bridge (`bridge.json` and whether it is the views bridge), the server's
/// tools for a `tools/call`, the plugins' state: the view, where its
/// server is, and the verdict (`bend_plugins::visibility`).
fn judge_call(file: &Path, bridge: Result<(PathBuf, bool), String>, method: &str, inner: &Value, state: &bend_plugins::model::StateV2) -> Result<(ViewFile, Route, Verdict), RpcError> {
    let what = Ask::of(method, inner).what().to_string();
    let text = std::fs::read_to_string(file).map_err(|_| ViewError::Expired.rpc(&what))?;
    let f = ViewFile::parse(&text).map_err(|e| e.rpc(&what))?;
    judge_on(f, bridge, method, inner, state)
}

/// A call of view `f` (an agent's call's views file, or a document's
/// opener) judged on its server's tools through `bridge`.
fn judge_on(f: ViewFile, bridge: Result<(PathBuf, bool), String>, method: &str, inner: &Value, state: &bend_plugins::model::StateV2) -> Result<(ViewFile, Route, Verdict), RpcError> {
    let (json_path, hubs) = bridge.map_err(|e| gone(&e))?;
    let base = wait_base(&json_path).ok_or_else(|| gone("no bridge"))?;
    let url = view_call::route_url(&base, &f.plugin, &f.server);
    let tools = if method == "tools/call" {
        let got = request(&url, Some(&view_call::request("tools/list", &json!({}))), CALL_TIMEOUT).map_err(|e| gone(&e))?;
        view_call::tools(&view_call::answer(&got).map_err(|e| server_error(&e))?)
    } else {
        Vec::new()
    };
    let view = ViewOf { plugin: &f.plugin, server: &f.server, opt_in: false };
    let verdict = visibility::judge(view, Ask::of(method, inner), &tools, state);
    Ok((f, Route { base, url, hubs }, verdict))
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
        match method {
            "plugin/views" => return self.plugin_views(params, done),
            "view/close" => return self.view_close(params, done),
            "view/open" => return self.view_open(params, done),
            "view/copy" => {
                let doc = params.get("doc").and_then(Value::as_str).unwrap_or("").to_string();
                return self.view_copy(doc, params, done);
            }
            // the wire is in (kb-fileprovider builds on it); the home hub's
            // table, folder/list and file/new serve them in the next commit
            "folder/list" | "file/new" | "files/ids/subscribe" | "files/mint" => return done(Err(ViewError::NotSupported.rpc(method))),
            _ => {}
        }
        let p = match view_call::Key::of(&params) {
            Ok(view_call::Key::Call(p)) => p,
            Ok(view_call::Key::Doc(doc)) => return self.doc_request(method, doc, params, done),
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
    pub(super) fn view_step(&self) -> impl Fn(ViewStep) + Send + 'static {
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
    /// hub's views bridge. A view moved to the views bridge stays there
    /// (`Servers::moved`): a fresh REPL's servers never ran it, and its
    /// server there keeps the state its calls built.
    fn bridge_json(&mut self, dir: &str, key: &ViewKey, own: bool) -> Result<(PathBuf, bool), String> {
        // its REPL alive, never only its port known: a REPL just killed
        // leaves a bridge that retires its servers ("turned off")
        let own = own && !self.views.servers.moved(key);
        if let Some(port) = self.ports.get(dir).filter(|_| own && repl_alive(&self.opts.paths.agent_dir(dir).join("repl.pid"))) {
            let own = self.opts.paths.agent_bridge_json(*port);
            if own.exists() {
                return Ok((own, false));
            }
        }
        let (exe, d, json, ws) = (self.opts.exe.clone(), self.opts.paths.views_bridge_dir(), self.opts.paths.views_bridge_json(), self.opts.paths.workspace.clone());
        self.views.bridge.ensure(&exe, &d, &json, &ws, now_ms()).map(|p| (p, true))
    }

    /// The hub's views bridge, started when it is not running: its
    /// `bridge.json` (a document's opener and its calls go there, never
    /// to an agent's bridge).
    pub(super) fn views_bridge(&mut self) -> Result<PathBuf, String> {
        let (exe, d, json, ws) = (self.opts.exe.clone(), self.opts.paths.views_bridge_dir(), self.opts.paths.views_bridge_json(), self.opts.paths.workspace.clone());
        self.views.bridge.ensure(&exe, &d, &json, &ws, now_ms())
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
                request(&url, None, CALL_TIMEOUT).map_err(|e| gone(&e))
            })();
            done(r);
        });
    }

    // ---- view/resource ----

    /// `own`: its agent's bridge may be tried (a REPL that just died
    /// leaves one that resets: then once more on the views bridge).
    fn view_resource(&mut self, p: ViewParams, dir: String, done: ViewDone, own: bool) {
        let file = self.view_file(&dir, &p.call);
        let bridge = self.bridge_json(&dir, &ViewKey::new(&p.agent, &p.call), own);
        let via_own = matches!(bridge, Ok((_, false)));
        let step = self.view_step();
        let ws = self.opts.paths.workspace.clone();
        std::thread::spawn(move || {
            let read = || -> Result<(ViewFile, view_call::ResourceRead, Value, Vec<String>, Value), RpcError> {
                let text = std::fs::read_to_string(&file).map_err(|_| ViewError::Expired.rpc("view/resource"))?;
                let f = ViewFile::parse(&text).map_err(|e| e.rpc("view/resource"))?;
                let (json_path, _) = bridge.map_err(|e| gone(&e))?;
                let base = wait_base(&json_path).ok_or_else(|| gone("no bridge"))?;
                let url = view_call::route_url(&base, &f.plugin, &f.server);
                let (r, list, grant) = read_view(&url, &f.plugin, &f.uri)?;
                let tool = view_call::tool_of(&list, &f.tool);
                Ok((f, r, tool, grant, list))
            };
            match read() {
                Err(e) if via_own && is_gone(&e) => step(Box::new(move |sh: &mut Shell| sh.view_resource(p, dir, done, false))),
                Err(e) => done(Err(e)),
                Ok((f, r, tool, grant, list)) => {
                    let folder = static_folder(&f, &ws);
                    step(Box::new(move |sh: &mut Shell| {
                        // its server's live tools: the rows' apps (A3)
                        sh.apps_seen(&f.plugin, &f.server, &list);
                        sh.view_resource_read(f, r, tool, grant, folder, done)
                    }))
                }
            }
        });
    }

    /// The resource is read: its sandbox registered on the pages server,
    /// then its answer.
    #[allow(clippy::too_many_arguments)]
    fn view_resource_read(&mut self, f: ViewFile, r: view_call::ResourceRead, tool: Value, grant: Vec<String>, folder: Option<ViewFolder>, done: ViewDone) {
        let Some(pages) = self.pg.pages.clone() else {
            return done(Err(RpcError::refused("this hub serves no pages: views need its pages server", None)));
        };
        let permissions = view_call::granted(&r.declared, &grant);
        let hash = crate::pages::views::hash(&f.plugin, &f.server, &f.uri);
        let session = pages.view_sandbox(&hash, crate::pages::views::ViewSandbox { csp: r.csp.clone(), perms: permissions.clone(), folder });
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
        let bridge = self.bridge_json(&dir, &key, own);
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
            let r = match request(&route.url, Some(&view_call::request(&method, &params)), CALL_TIMEOUT) {
                Err(e) => Err(gone(&e)),
                Ok(v) => match view_call::answer(&v) {
                    Ok(r) => Ok(view_call::mark_restarted(r, restarted)),
                    Err(e) => Err(server_error(&e)),
                },
            };
            done(r);
        });
    }

    // ---- document views (docs/plugin-apps.md A4) ----

    /// A request of a document view (`{doc}`): its calls to its opener's
    /// server; it has no agent, so no message nor context.
    fn doc_request(&mut self, method: &str, doc: String, params: Value, done: ViewDone) {
        match method {
            "view/call" => self.doc_call(doc, params, done),
            "view/notes" => self.doc_notes(doc, done),
            m => done(Err(ViewError::NotSupported.rpc(m))),
        }
    }

    /// `view/notes {doc}` (A5, architect m_59942): the notes of the
    /// artifact the session opened, each by the view's own key, the keyless
    /// ones `detached` (pages/artifact/notes.rs `view_notes`). The session
    /// decides the artifact, never the view; a file of ~/bise has none.
    /// The target is copied out of the files table, then the store is read
    /// off the loop (the lock is never held across I/O).
    fn doc_notes(&mut self, doc: String, done: ViewDone) {
        let Some(pages) = self.pg.pages.clone() else { return done(Err(ViewError::Expired.rpc("view/notes"))) };
        let Some(target) = pages.doc_target(&doc) else { return done(Err(ViewError::Expired.rpc("view/notes"))) };
        std::thread::spawn(move || {
            done(Ok(match target {
                crate::view_files::FileTarget::Artifact { id, .. } => crate::pages::artifact::notes::for_view(&pages, &id),
                crate::view_files::FileTarget::RootFile { .. } => json!({"notes": [], "detached": []}),
            }))
        });
    }

    /// A document view's `tools/call` to its opener's server, on the
    /// views bridge (never an agent's), judged as C5's: an app-only tool
    /// runs; one the model sees too would need an agent's card, which a
    /// document view has none of: refused.
    fn doc_call(&mut self, doc: String, params: Value, done: ViewDone) {
        let method = params.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let inner = params.get("params").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
        let what = Ask::of(&method, &inner).what().to_string();
        // the session's opener, copied out of the pages' table (its end is
        // the pages server's, with the session)
        let Some(d) = self.pg.pages.as_ref().and_then(|p| p.doc_opener(&doc)) else { return done(Err(ViewError::Expired.rpc(&what))) };
        // its file: the hub's own, never its server's (daemon/view_files.rs)
        match super::view_files::doc_route(&method) {
            super::view_files::DocRoute::File => return self.doc_file(doc, &method, inner, done),
            super::view_files::DocRoute::Unknown => return done(Err(ViewError::NotSupported.rpc(&what))),
            super::view_files::DocRoute::Tool => {}
        }
        let key = ViewKey::new("", &doc);
        if !self.views.limits.call(&key, now_ms()) {
            return done(Err(ViewError::RateLimited.rpc(&what)));
        }
        let f = ViewFile { plugin: d.plugin, server: d.server, uri: d.uri, ..Default::default() };
        let bridge = self.views_bridge().map(|p| (p, true));
        let state_path = bend_plugins::state::state_path();
        let step = self.view_step();
        std::thread::spawn(move || {
            let state = bend_plugins::state::read(&state_path);
            match judge_on(f, bridge, &method, &inner, &state) {
                Err(e) => done(Err(e)),
                Ok((_, _, Verdict::Refused(e))) => done(Err(e.rpc(&what))),
                Ok((_, _, Verdict::Ask)) => done(Err(ViewError::NotVisible.rpc(&what))),
                Ok((_, route, Verdict::Free)) => step(Box::new(move |sh: &mut Shell| sh.view_forward(key, route, method, inner, done))),
            }
        });
    }

    /// `view/close {doc}`: his tab closed, unloaded or swapped app: the
    /// session and its handles end, in the pages server's one place.
    fn view_close(&mut self, params: Value, done: ViewDone) {
        match view_call::Key::of(&params) {
            Ok(view_call::Key::Doc(doc)) => {
                if let Some(pages) = self.pg.pages.clone() {
                    pages.view_session_end(&doc);
                }
                done(Ok(json!({})))
            }
            _ => done(Err(ViewError::Expired.rpc("view/close"))),
        }
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
        if let Err(why) = self.views.limits.context(&ViewKey::new(&p.agent, &p.call), now_ms(), text.len()) {
            return done(Err(why.error().rpc(WHAT)));
        }
        // sb-core keeps it (an active agent's only) and joins it (d5); its
        // model text is rendered here, the answer is the step's view_context
        let key = ViewKey::new(&p.agent, &p.call);
        let part = Part::ViewContext { plugin: f.plugin, uri: f.uri, text };
        let text = bise_proto::parts::render(std::slice::from_ref(&part));
        let part = serde_json::to_value(&part).unwrap_or_default();
        let fx = self.hub.handle(Input::ViewContext { agent: p.agent, call: p.call, text, part }, &mut self.env);
        let kept = context_said(&fx, &key);
        self.step_fx(fx);
        match kept {
            Some(Ok(())) => done(Ok(json!({}))),
            Some(Err(why)) => done(Err(RpcError::refused(why, None))),
            None => done(Err(RpcError::refused("bise did not keep the view's context", None))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_bridge::tests::{fake_bridge, tmp};

    /// A document view's methods: its file's three go to the hub, a tool
    /// call to its opener; the host's own copy (A6b) and anything else is
    /// unknown (architect m_60137).
    #[test]
    fn a_doc_views_methods_route_and_the_hosts_copy_is_unknown() {
        use super::super::view_files::{doc_route, DocRoute};
        for m in ["resources/read", "resources/url", "ai.mistral.vibe/resources/write"] {
            assert_eq!(doc_route(m), DocRoute::File, "{m}");
        }
        assert_eq!(doc_route("tools/call"), DocRoute::Tool);
        for m in ["dev.bise/resources/copy", "view/copy", "resources/write", "resources/list", ""] {
            assert_eq!(doc_route(m), DocRoute::Unknown, "{m}");
        }
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
    /// bridge, or a call through a new bridge, is said restarted once per
    /// server; a moved view stays on the views bridge.
    #[test]
    fn a_new_server_is_said_once_per_server() {
        let mut v = Views::default();
        let route = |base: &str, hubs: bool| Route { base: base.into(), url: format!("{base}/p/s"), hubs };
        let (a, b) = (ViewKey::new("docs", "1-1"), ViewKey::new("docs", "1-2"));
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:1/x", false)), "its own bridge: the server that ran it");
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:1/x", false)));
        assert!(v.server_restarted(&a, &route("http://127.0.0.1:2/y", true)), "another bridge");
        assert!(!v.server_restarted(&a, &route("http://127.0.0.1:2/y", true)), "once per server");
        assert!(v.servers.moved(&a) && !v.servers.moved(&b), "a moved view stays on the views bridge");
        assert!(v.server_restarted(&a, &route("http://127.0.0.1:3/z", true)), "a views bridge started again: said again");
        assert!(v.server_restarted(&b, &route("http://127.0.0.1:2/y", true)), "first call on the views bridge");
        assert!(!v.server_restarted(&b, &route("http://127.0.0.1:2/y", true)));
    }

    #[test]
    fn a_views_context_is_answered_by_its_effect() {
        use crate::core::Effect;
        let k = ViewKey::new("docs", "1-2");
        let said = |agent: &str, kept: bool, why: &str| Effect::ViewContext { agent: agent.into(), call: "1-2".into(), kept, why: why.into() };
        assert_eq!(context_said(&[Effect::State, said("docs", true, "")], &k), Some(Ok(())));
        assert_eq!(context_said(&[said("docs", false, "not active")], &k), Some(Err("not active".into())));
        assert_eq!(context_said(&[said("main", true, "")], &k), None, "another view's");
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
