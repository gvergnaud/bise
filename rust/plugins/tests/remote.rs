//! Remote MCP servers against tests/fake_mcp_http.py: Streamable HTTP
//! and the legacy SSE transport, headers with ${VAR}, pagination, an
//! expired session, a dropped stream, a 401, list_changed, a server
//! gone; then a plugin with an http server through the real bridge.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bend_plugins::remote::{Fail, OnChange, Remote};
use bend_plugins::resolve::{HttpServer, Transport};
use bend_plugins::{bridge, resolve};
use serde_json::{json, Value};

const T: Duration = Duration::from_secs(10);
const TOKEN: &str = "tok-SECRET-42";

struct Fake {
    child: Child,
    port: u16,
    dir: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fake(tag: &str, args: &[&str]) -> Fake {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("bp-remote-{}-{}-{}", tag, std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fake_mcp_http.py");
    let port_file = dir.join("port");
    let child = Command::new("python3").arg(&script).arg("--port-file").arg(&port_file).args(args).spawn().unwrap();
    let t0 = Instant::now();
    let port = loop {
        if let Some(p) = std::fs::read_to_string(&port_file).ok().and_then(|s| s.trim().parse().ok()) {
            break p;
        }
        assert!(t0.elapsed() < T, "the fake server never started");
        std::thread::sleep(Duration::from_millis(30));
    };
    Fake { child, port, dir }
}

impl Fake {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    fn control(&self, action: &str) -> Value {
        let u = bend_plugins::http::Url::parse(&self.url("/control")).unwrap();
        let body = json!({"action": action}).to_string();
        let r = bend_plugins::http::send(&bend_plugins::http::Request { method: "POST", url: &u, headers: &[], body: body.as_bytes(), timeout: T }).unwrap();
        serde_json::from_slice(&r.read_all(1 << 20).unwrap()).unwrap()
    }
}

fn server(transport: Transport, url: String) -> HttpServer {
    HttpServer {
        id: "fake".into(),
        transport,
        url,
        headers: vec![("Authorization".into(), "Bearer ${FAKE_TOKEN}".into()), ("X-Test".into(), "${X_TEST:-from-default}".into())],
    }
}

fn env(k: &str) -> Option<String> {
    (k == "FAKE_TOKEN").then(|| TOKEN.to_string())
}

fn counter() -> (OnChange, Arc<AtomicUsize>) {
    let n = Arc::new(AtomicUsize::new(0));
    let m = n.clone();
    (Arc::new(move || {
        m.fetch_add(1, Ordering::SeqCst);
    }), n)
}

fn text(r: &Value) -> String {
    r["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn call(c: &Remote, name: &str, args: Value) -> Result<Value, Fail> {
    c.request_raw("tools/call", json!({"name": name, "arguments": args}), T)
}

fn wait_for(what: &str, f: impl Fn() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < T, "timed out waiting for {}", what);
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn streamable_http_headers_pages_session_and_list_changed() {
    let f = fake("st", &["--mode", "streamable", "--require-header", &format!("Authorization=Bearer {}", TOKEN), "--page-size", "1"]);
    let (on, changes) = counter();
    let c = Remote::start(&server(Transport::Streamable, f.url("/mcp")), &env, on, T).unwrap();
    assert_eq!(c.init()["serverInfo"]["name"], "fake-streamable");
    // three pages of one tool
    let names: Vec<String> = c.list_tools(T).unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["echo", "add_tool", "slow"]);
    // the headers, ${VAR:-default} included
    let r = call(&c, "echo", json!({"text": "hi"})).unwrap();
    assert_eq!(text(&r), "echo:hi x-test=from-default");
    // the session id and the protocol version ride on every request after initialize
    let log = f.control("log");
    let posts: Vec<&Value> = log.as_array().unwrap().iter().filter(|e| e["method"] == "POST").collect();
    assert_eq!(posts[0]["rpc"], "initialize");
    assert!(posts[0]["session"].is_null());
    assert!(posts[1..].iter().all(|e| e["session"].is_string() && e["protocol"] == "2025-06-18" && e["auth_ok"] == true), "{:?}", posts);
    // the server forgets the session: a new handshake, the call goes through
    f.control("forget_sessions");
    let r = call(&c, "echo", json!({"text": "again"})).unwrap();
    assert_eq!(text(&r), "echo:again x-test=from-default");
    // list_changed on the GET stream (it was reopened with the new session)
    wait_for("the GET stream", || f.control("log").as_array().unwrap().iter().filter(|e| e["method"] == "GET").count() >= 1);
    std::thread::sleep(Duration::from_millis(1200));
    call(&c, "add_tool", json!({"name": "fresh"})).unwrap();
    wait_for("list_changed", || changes.load(Ordering::SeqCst) >= 1);
    assert_eq!(c.list_tools(T).unwrap().len(), 4);
    // a tool error is an answer, not a failure
    let r = call(&c, "nope", json!({})).unwrap();
    assert!(r["error"]["message"].as_str().unwrap().contains("no such tool"));
}

#[test]
fn streamable_http_answers_as_a_chunked_event_stream() {
    let f = fake("sa", &["--mode", "streamable", "--sse-answers"]);
    let (on, _) = counter();
    let c = Remote::start(&server(Transport::Streamable, f.url("/mcp")), &env, on, T).unwrap();
    assert_eq!(c.list_tools(T).unwrap().len(), 3);
    assert_eq!(text(&call(&c, "echo", json!({"text": "sse"})).unwrap()), "echo:sse x-test=from-default");
}

#[test]
fn a_401_is_a_login_failure_with_its_challenge_and_no_token_in_it() {
    let f = fake("401", &["--mode", "streamable", "--require-header", "Authorization=Bearer other"]);
    let (on, _) = counter();
    let e = Remote::start(&server(Transport::Streamable, f.url("/mcp")), &env, on, T).err().unwrap();
    match &e {
        Fail::Auth { host, challenge } => {
            assert_eq!(host, &format!("127.0.0.1:{}", f.port));
            assert!(challenge.as_deref().unwrap_or("").contains("resource_metadata="), "{:?}", challenge);
        }
        other => panic!("{:?}", other),
    }
    let line = e.to_string();
    assert!(line.contains("401") && !line.contains(TOKEN) && !line.contains('\n'), "{}", line);
    // the same over SSE
    let f = fake("401s", &["--mode", "sse", "--require-header", "Authorization=Bearer other"]);
    let (on, _) = counter();
    assert!(matches!(Remote::start(&server(Transport::Sse, f.url("/sse")), &env, on, T), Err(Fail::Auth { .. })));
}

#[test]
fn an_unset_variable_and_a_server_gone_are_one_line_each() {
    let s = HttpServer { headers: vec![("Authorization".into(), "Bearer ${NOT_SET_ANYWHERE}".into())], ..server(Transport::Streamable, "http://127.0.0.1:9/mcp".into()) };
    let (on, _) = counter();
    let e = Remote::start(&s, &env, on.clone(), T).err().unwrap().to_string();
    assert_eq!(e, "header Authorization uses ${NOT_SET_ANYWHERE}, which is not set");
    let mut f = fake("gone", &["--mode", "streamable"]);
    let c = Remote::start(&server(Transport::Streamable, f.url("/mcp")), &env, on, T).unwrap();
    let _ = f.child.kill();
    let _ = f.child.wait();
    let e = call(&c, "echo", json!({})).err().unwrap().to_string();
    assert_eq!(e, format!("127.0.0.1:{} refused the connection", f.port));
    // a wrong transport says so
    let f = fake("wrong", &["--mode", "streamable"]);
    let (on, _) = counter();
    let e = Remote::start(&server(Transport::Sse, f.url("/mcp")), &env, on, T).err().unwrap().to_string();
    assert!(e.contains("HTTP 404") || e.contains("event stream"), "{}", e);
}

#[test]
fn sse_transport_calls_reconnects_after_a_drop_and_hears_list_changed() {
    let f = fake("sse", &["--mode", "sse", "--require-header", &format!("Authorization=Bearer {}", TOKEN), "--page-size", "2"]);
    let (on, changes) = counter();
    let c = Remote::start(&server(Transport::Sse, f.url("/sse")), &env, on, T).unwrap();
    assert_eq!(c.init()["serverInfo"]["name"], "fake-sse");
    assert_eq!(c.list_tools(T).unwrap().len(), 3);
    assert_eq!(text(&call(&c, "echo", json!({"text": "a"})).unwrap()), "echo:a x-test=from-default");
    call(&c, "add_tool", json!({"name": "more"})).unwrap();
    wait_for("list_changed", || changes.load(Ordering::SeqCst) >= 1);
    // the stream drops: the next call opens a new one and runs the handshake again
    f.control("drop_streams");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(text(&call(&c, "echo", json!({"text": "b"})).unwrap()), "echo:b x-test=from-default");
    let gets = f.control("log").as_array().unwrap().iter().filter(|e| e["method"] == "GET").count();
    assert_eq!(gets, 2);
    // a stream dropped mid-call is an error for that call, never a second send
    let c2 = Arc::new(c);
    let c3 = c2.clone();
    let slow = std::thread::spawn(move || call(&c3, "slow", json!({"secs": 1.5})));
    std::thread::sleep(Duration::from_millis(400));
    f.control("drop_streams");
    let r = slow.join().unwrap();
    let msg = match r {
        Ok(v) => v["error"]["message"].as_str().unwrap_or_default().to_string(),
        Err(e) => e.to_string(),
    };
    assert!(msg.contains("dropped"), "{}", msg);
    let slows = f.control("log").as_array().unwrap().iter().filter(|e| e["rpc"] == "tools/call").count();
    assert_eq!(slows, 4, "echo a, add_tool, echo b, slow once");
}

#[test]
fn a_plugin_with_an_http_server_goes_through_the_bridge() {
    let f = fake("bridge", &["--mode", "streamable", "--require-header", &format!("Authorization=Bearer {}", TOKEN)]);
    let var = format!("BISE_TEST_FAKE_TOKEN_{}", std::process::id());
    std::env::set_var(&var, TOKEN);
    let base = f.dir.join("home");
    let p = base.join("ws/.agents/plugins/remote-one");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("plugin.json"), format!("{{\"$schema\":\"{}\",\"name\":\"remote-one\",\"version\":\"1.0.0\"}}", resolve::PLUGIN_SCHEMA)).unwrap();
    // Claude Code's shape: no $schema
    std::fs::write(
        p.join("mcp.json"),
        json!({"mcpServers": {"fake": {"type": "http", "url": f.url("/mcp"), "headers": {"Authorization": format!("Bearer ${{{}}}", var), "X-Test": "x1"}},
                              "down": {"type": "sse", "url": "http://127.0.0.1:9/sse"}}})
        .to_string(),
    )
    .unwrap();
    let dir = base.join("run/plugins");
    let status = base.join("status");
    let roots = resolve::Roots {
        builtin: None,
        user: None,
        workspace: Some(base.join("ws/.agents/plugins")),
        data: base.join("data"),
        disabled: vec![],
        enabled: vec![],
    };
    let mut parent = Command::new("sleep").arg("60").spawn().unwrap();
    let opts = bridge::Opts { dir: dir.clone(), parent: Some(parent.id()), roots, status_dir: Some(status.clone()) };
    let srv = std::thread::spawn(move || bridge::serve(opts));
    wait_for("the bridge", || dir.join("ready").exists());
    let index = std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap();
    assert_eq!(index.lines().count(), 3, "{}", index);
    assert!(index.contains(" remote_one echo : #Echo a text back."), "{}", index);
    let report = std::fs::read_to_string(dir.join("report.txt")).unwrap();
    assert!(report.contains(&format!("mcp fake: 3 tools as tools.remote_one.* (http 127.0.0.1:{})", f.port)), "{}", report);
    assert!(report.contains("mcp down: sse 127.0.0.1:9: unavailable"), "{}", report);
    assert!(report.contains("plugin.mcp.connection_failed [remote-one] MCP server \"down\": 127.0.0.1:9 refused the connection"), "{}", report);
    assert!(!report.contains(TOKEN));
    // the last state, for /plugins
    let st = bend_plugins::status::read(&status, "remote-one", "fake").unwrap();
    assert_eq!(st.tools, Ok(3));
    assert!(bend_plugins::status::read(&status, "remote-one", "down").unwrap().tools.is_err());
    // a call through the loopback bridge, as the REPL makes it
    let cid = index.lines().next().unwrap().split(' ').next().unwrap().to_string();
    let u = bend_plugins::http::Url::parse(&cid).unwrap();
    let post = |body: Value| -> Value {
        let b = body.to_string();
        let h = vec![("Content-Type".to_string(), "application/json".to_string())];
        let r = bend_plugins::http::send(&bend_plugins::http::Request { method: "POST", url: &u, headers: &h, body: b.as_bytes(), timeout: T }).unwrap();
        serde_json::from_slice(&r.read_all(1 << 20).unwrap()).unwrap_or(Value::Null)
    };
    let init = post(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "fake-streamable");
    let r = post(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "echo", "arguments": {"text": "via bridge"}}}));
    assert_eq!((r["id"].clone(), text(&r)), (json!(5), "echo:via bridge x-test=x1".to_string()));
    // list_changed: the bridge lists again and rewrites the index
    std::thread::sleep(Duration::from_millis(1200));
    post(json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "add_tool", "arguments": {"name": "late_tool"}}}));
    wait_for("the index rewrite", || std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap_or_default().contains(" remote_one late_tool : #added"));
    parent.kill().unwrap();
    parent.wait().unwrap();
    wait_for("the bridge to stop", || srv.is_finished());
    srv.join().unwrap().unwrap();
    std::env::remove_var(&var);
}
