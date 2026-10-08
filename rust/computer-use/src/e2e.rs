//! End to end, with fakes for both ends (brief item 6): a fake extension
//! speaking C4 in native messaging frames on the relay's stdio
//! (`host::run`), a fake helper speaking C5 on its socket, agents speaking
//! C3 (and MCP through `mcp::Server`). Each test has its own folder, so
//! its own broker.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::broker::{self, Handle, Opts};
use crate::client::{Conn, Starter};
use crate::paths::Paths;
use crate::{b64, cli, host, mcp, nm, state};

static N: AtomicU64 = AtomicU64::new(0);

/// A short folder (unix socket paths stop at 104 bytes): `$TMPDIR` when
/// short (the gate's), else `~/.bise/gate` (an agent's TMPDIR is too deep).
fn paths() -> (PathBuf, Paths) {
    let tmp = std::env::temp_dir();
    let base = if tmp.as_os_str().len() <= 50 {
        tmp
    } else {
        let g = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".bise/gate");
        if g.is_dir() { g } else { tmp }
    };
    let d = base.join(format!("cu{}-{}", std::process::id() % 100000, N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let p = Paths::new(d.join("r"), d.join("b"), d.join("h"));
    assert!(p.app_socket.as_os_str().len() < 100, "{}", p.app_socket.display());
    (d, p)
}

fn opts(p: &Paths) -> Opts {
    Opts {
        paths: p.clone(),
        idle_exit: None,
        release_after: Duration::from_secs(60),
        launch_helper: false,
        helper_app: None,
        slack: Duration::from_secs(2),
        helper_wait: Duration::from_millis(300),
        judge: fake_judge(p),
    }
}

/// The peers the next connections to `p`'s broker are judged as
/// ([`next_peer`]); the user's (`Outside`) when none. A test runs inside an
/// agent: the real judge would see every connection as that agent's.
static NEXT_PEERS: Mutex<Vec<(PathBuf, crate::who::Peer)>> = Mutex::new(Vec::new());

fn fake_judge(p: &Paths) -> crate::who::JudgeFn {
    let run = p.run.clone();
    crate::who::JudgeFn(Arc::new(move |_| {
        let mut q = NEXT_PEERS.lock().unwrap();
        match q.iter().position(|(r, _)| *r == run) {
            Some(i) => q.remove(i).1,
            None => crate::who::Peer::Outside,
        }
    }))
}

/// The next connection to `p`'s broker comes from `peer`.
fn next_peer(p: &Paths, peer: crate::who::Peer) {
    NEXT_PEERS.lock().unwrap().push((p.run.clone(), peer));
}

/// The brokers a test started (to shut one down: a restart).
type Brokers = Arc<Mutex<Vec<Handle>>>;

/// A starter that runs an in-process broker, like `bise computer-use broker`.
fn starter(o: Opts, brokers: Brokers) -> Arc<Starter> {
    Arc::new(move |_p: &Paths| {
        match broker::start(o.clone()) {
            Ok(h) => brokers.lock().unwrap().push(h),
            Err(broker::StartError::Running) => {}
            Err(broker::StartError::Io(e)) => return Err(e),
        }
        Ok(())
    })
}

fn wait_until(what: &str, f: impl Fn() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(10), "timed out waiting for {}", what);
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---- the fake extension (C4) ----

#[derive(Default)]
struct ExtState {
    next_tab: u64,
    /// agent -> [(tab id, url)]
    tabs: HashMap<String, Vec<(u64, String)>>,
    /// control lines received: {"stop": a}, {"release": a}, ...
    control: Vec<Value>,
    requests: Vec<Value>,
}

struct FakeExt {
    w: Arc<Mutex<UnixStream>>,
    st: Arc<Mutex<ExtState>>,
}

impl FakeExt {
    /// Start the relay (`chrome-host`) on a socket pair and the fake
    /// extension on its other end; says hello as `browser`.
    fn start(p: &Paths, start: Arc<Starter>, browser: &str, jpeg: Arc<Vec<u8>>) -> FakeExt {
        let (ext, hostside) = UnixStream::pair().unwrap();
        let (hin, hout) = (hostside.try_clone().unwrap(), hostside);
        let p2 = p.clone();
        std::thread::spawn(move || host::run(&p2, start, hin, hout, None));
        let w = Arc::new(Mutex::new(ext.try_clone().unwrap()));
        let st = Arc::new(Mutex::new(ExtState::default()));
        nm::write(&mut *w.lock().unwrap(), &json!({"hello": {"browser": browser, "version": "154.0.1", "extension_version": "0.1.0"}})).unwrap();
        let (w2, st2) = (w.clone(), st.clone());
        let mut r = ext;
        std::thread::spawn(move || {
            while let Ok(Some(msg)) = nm::read(&mut r) {
                if msg.get("id").is_none() {
                    let mut s = st2.lock().unwrap();
                    if let Some(a) = msg.get("drop").and_then(Value::as_str) {
                        s.tabs.remove(a);
                    }
                    s.control.push(msg);
                    continue;
                }
                let (w3, st3, jpeg) = (w2.clone(), st2.clone(), jpeg.clone());
                std::thread::spawn(move || {
                    let reply = ext_reply(&st3, &msg, &jpeg);
                    let _ = nm::write(&mut *w3.lock().unwrap(), &reply);
                });
            }
        });
        FakeExt { w, st }
    }

    fn event(&self, v: Value) {
        nm::write(&mut *self.w.lock().unwrap(), &v).unwrap();
    }

    fn control(&self) -> Vec<Value> {
        self.st.lock().unwrap().control.clone()
    }
}

fn ext_reply(st: &Mutex<ExtState>, msg: &Value, jpeg: &[u8]) -> Value {
    let id = msg["id"].clone();
    let agent = msg["agent"].as_str().unwrap_or("").to_string();
    let args = &msg["args"];
    st.lock().unwrap().requests.push(msg.clone());
    let ok = |r: Value| json!({"id": id, "ok": true, "result": r});
    let fail = |code: &str, m: &str, extra: Value| {
        let mut e = json!({"code": code, "message": m});
        if let Some(o) = extra.as_object() {
            for (k, v) in o {
                e[k] = v.clone();
            }
        }
        json!({"id": id, "ok": false, "error": e})
    };
    let tab = args["target"].as_str().and_then(|t| t.strip_prefix("tab:")).and_then(|t| t.parse::<u64>().ok());
    let url_of = |t: u64| -> Option<String> {
        st.lock().unwrap().tabs.get(&agent)?.iter().find(|(i, _)| *i == t).map(|(_, u)| u.clone())
    };
    match msg["op"].as_str().unwrap_or("") {
        "open" => {
            let mut s = st.lock().unwrap();
            if s.tabs.get(&agent).is_some_and(|t| t.len() >= 5) {
                return fail("refused", "you already have 5 tabs open; close one (act close) first", json!({}));
            }
            s.next_tab += 1;
            let n = s.next_tab;
            let url = args["url"].as_str().unwrap_or("").to_string();
            s.tabs.entry(agent).or_default().push((n, url.clone()));
            ok(json!({"target": format!("tab:{}", n), "url": url, "title": "Test page"}))
        }
        "tabs" => {
            let s = st.lock().unwrap();
            let list: Vec<Value> = s.tabs.get(&agent).into_iter().flatten().map(|(n, u)| {
                json!({"target": format!("tab:{}", n), "url": u, "title": "Test page", "user_touched": false})
            }).collect();
            ok(json!(list))
        }
        op @ ("snapshot" | "screenshot" | "act") => {
            let Some(url) = tab.and_then(url_of) else {
                return fail("not_found", "no such tab", json!({}));
            };
            match op {
                "snapshot" => ok(json!({"target": args["target"], "url": url, "title": "Test page",
                    "text": "# Test page · 127.0.0.1\n- button \"Click me\" [e1]\n- textbox \"Your name\" [e2]", "refs": 2, "truncated": false})),
                "screenshot" => ok(json!({"data": b64::encode(jpeg), "mime": "image/jpeg", "width": 1600, "height": 90})),
                _ => {
                    let name = args["locator"]["name"].as_str().unwrap_or("");
                    let action = args["action"].as_str().unwrap_or("");
                    if action == "wait" && args["text"] == "slow" {
                        std::thread::sleep(Duration::from_millis(3000));
                    }
                    if name == "Missing" {
                        return fail("not_found", "no button named Missing", json!({"candidates": ["- button \"Click me\" [e1]"]}));
                    }
                    if name == "Twice" {
                        return fail("ambiguous", "2 buttons match", json!({"candidates": ["- button \"Twice\" [e3]", "- button \"Twice\" [e4]"]}));
                    }
                    if args["ref"] == "e99" {
                        return fail("stale_ref", "e99 is stale; snapshot again", json!({}));
                    }
                    if name == "Buy" {
                        return fail("timeout", "the button never got enabled", json!({"summary": "waited for \"Buy\" · 127.0.0.1"}));
                    }
                    if action == "close" {
                        let mut s = st.lock().unwrap();
                        if let Some(v) = s.tabs.get_mut(&agent) {
                            v.retain(|(i, _)| Some(*i) != tab);
                        }
                    }
                    let new_url = if action == "goto" { args["url"].as_str().unwrap_or(&url).to_string() } else { url };
                    if action == "goto" {
                        let mut s = st.lock().unwrap();
                        if let Some(v) = s.tabs.get_mut(&agent) {
                            for t in v.iter_mut().filter(|(i, _)| Some(*i) == tab) {
                                t.1 = new_url.clone();
                            }
                        }
                    }
                    ok(json!({"ok": true, "url": new_url, "title": "Test page", "changed": "", "summary": format!("{} \"{}\" · 127.0.0.1", action, name)}))
                }
            }
        }
        "show" => ok(json!({"tab_id": 77, "created": true, "url": args["url"]})),
        _ => fail("bad_args", "unknown op", json!({})),
    }
}

// ---- the fake helper (C5) ----

struct FakeHelper {
    control: Arc<Mutex<Vec<Value>>>,
    conn: Arc<Mutex<Option<UnixStream>>>,
}

impl FakeHelper {
    fn start(p: &Paths, jpeg: Arc<Vec<u8>>) -> FakeHelper {
        let l = UnixListener::bind(&p.app_socket).unwrap();
        let control = Arc::new(Mutex::new(Vec::new()));
        let conn: Arc<Mutex<Option<UnixStream>>> = Arc::new(Mutex::new(None));
        let (c2, k2) = (control.clone(), conn.clone());
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                let hello = json!({"hello": {"helper": "dev.bise.computer-use", "version": "0.1", "accessibility": true, "screen_recording": false}});
                writeln!(s, "{}", hello).unwrap();
                *k2.lock().unwrap() = Some(s.try_clone().unwrap());
                let (c3, jpeg) = (c2.clone(), jpeg.clone());
                std::thread::spawn(move || {
                    let mut w = s.try_clone().unwrap();
                    for line in BufReader::new(s).lines() {
                        let Ok(line) = line else { break };
                        let msg: Value = serde_json::from_str(&line).unwrap();
                        if msg.get("id").is_none() {
                            c3.lock().unwrap().push(msg);
                            continue;
                        }
                        let id = msg["id"].clone();
                        // granting Screen Recording: macOS quits the helper
                        // before it answers (it reopens on the same socket)
                        if msg["op"] == "request" && msg["args"]["what"] == "screen_recording" {
                            let _ = w.shutdown(std::net::Shutdown::Both);
                            break;
                        }
                        let r = match msg["op"].as_str().unwrap() {
                            "permissions" | "request" => json!({"accessibility": true, "screen_recording": false}),
                            "apps" => json!([{"target": "app:com.apple.TextEdit", "name": "TextEdit", "pid": 42, "windows": [{"title": "Untitled", "focused": true}]}]),
                            "snapshot" => json!({"target": msg["args"]["target"], "title": "Untitled", "text": "# Untitled · TextEdit\n- textbox \"\" [e1]", "refs": 1, "truncated": false}),
                            "screenshot" => json!({"data": b64::encode(&jpeg), "mime": "image/jpeg", "width": 1600, "height": 90}),
                            "act" => json!({"ok": true, "title": "Untitled", "changed": "", "summary": "typed in the text · TextEdit", "agent_seen": msg["agent"]}),
                            _ => Value::Null,
                        };
                        writeln!(w, "{}", json!({"id": id, "ok": true, "result": r})).unwrap();
                    }
                });
            }
        });
        FakeHelper { control, conn }
    }

    fn event(&self, v: Value) {
        let mut g = self.conn.lock().unwrap();
        writeln!(g.as_mut().unwrap(), "{}", v).unwrap();
    }
}

// ---- helpers ----

fn agent(p: &Paths, name: &str, tmp: &std::path::Path) -> Conn {
    Conn::open(p, None, &json!({"op": "hello", "agent": name, "session": "s", "tmpdir": tmp})).unwrap()
}

fn ok(c: &mut Conn, op: &str, args: Value) -> Value {
    match c.call(op, &args).unwrap() {
        Ok(v) => v,
        Err(e) => panic!("{} {}: {}", op, args, e),
    }
}

fn code(c: &mut Conn, op: &str, args: Value) -> Value {
    match c.call(op, &args).unwrap() {
        Ok(v) => panic!("{} {} should fail: {}", op, args, v),
        Err(e) => e,
    }
}

fn jpeg(d: &std::path::Path) -> Arc<Vec<u8>> {
    Arc::new(crate::image::tests::jpeg(&d.join("img"), 1600, 90))
}

fn connected(p: &Paths, n: usize) {
    wait_until("the browsers", || {
        Conn::ctl(p)
            .ok()
            .and_then(|mut c| c.call("status", &json!({})).ok())
            .and_then(Result::ok)
            .is_some_and(|s| s["browsers"].as_array().unwrap().iter().filter(|b| b["connected"] == true).count() == n)
    });
}

// ---- the tests ----

#[test]
fn every_web_op_and_error() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let ext = FakeExt::start(&p, starter(o, brokers.clone()), "chrome", jpeg(&d));
    connected(&p, 1);
    let tmp = d.join("agent-tmp");
    let mut a = agent(&p, "api-v2", &tmp);

    let st = ok(&mut a, "status", json!({}));
    assert_eq!(st["browsers"][0], json!({"name": "Chrome", "version": "154.0.1", "connected": true, "extension_version": "0.1.0"}));
    assert_eq!(st["me"], json!({"stopped": false, "paused": []}));
    assert_eq!(st["apps"]["helper"], "absent");

    let tab = ok(&mut a, "open", json!({"url": "https://www.amazon.fr/x"}));
    assert_eq!(tab["target"], "tab:1");
    let t = tab["target"].as_str().unwrap().to_string();
    let s = state::read(&p);
    assert_eq!(s["agents"]["api-v2"]["driving"], "Chrome");
    assert_eq!(s["agents"]["api-v2"]["where"], "amazon.fr");
    assert!(s["agents"]["api-v2"]["since_ms"].as_u64().unwrap() > 0);
    assert_eq!(s["browsers"][0]["connected"], true);

    assert_eq!(ok(&mut a, "tabs", json!({})).as_array().unwrap().len(), 1);
    let snap = ok(&mut a, "snapshot", json!({"target": t}));
    assert!(snap["text"].as_str().unwrap().contains("[e1]"));
    // the extension got the agent with each request
    assert_eq!(ext.st.lock().unwrap().requests.last().unwrap()["agent"], "api-v2");

    let shot = ok(&mut a, "screenshot", json!({"target": t}));
    assert_eq!((shot["width"].as_u64(), shot["height"].as_u64(), shot["mime"].as_str()), (Some(1280), Some(72), Some("image/jpeg")));
    let path = PathBuf::from(shot["path"].as_str().unwrap());
    assert!(path.starts_with(&tmp) && path.exists(), "{}", path.display());
    let small = ok(&mut a, "screenshot", json!({"target": t, "max_width": 800}));
    assert_eq!(small["width"], 800);

    let click = ok(&mut a, "act", json!({"target": t, "action": "click", "locator": {"role": "button", "name": "Click me"}}));
    assert_eq!(click["summary"], "click \"Click me\" · 127.0.0.1");

    // errors from the extension pass through, candidates and summary included
    let e = code(&mut a, "act", json!({"target": t, "action": "click", "locator": {"name": "Missing"}}));
    assert_eq!((e["code"].as_str(), e["candidates"].as_array().map(Vec::len)), (Some("not_found"), Some(1)));
    let e = code(&mut a, "act", json!({"target": t, "action": "click", "locator": {"name": "Twice"}}));
    assert_eq!((e["code"].as_str(), e["candidates"].as_array().map(Vec::len)), (Some("ambiguous"), Some(2)));
    assert_eq!(code(&mut a, "act", json!({"target": t, "action": "click", "ref": "e99"}))["code"], "stale_ref");
    let e = code(&mut a, "act", json!({"target": t, "action": "click", "locator": {"name": "Buy"}}));
    assert_eq!(e["summary"], "waited for \"Buy\" · 127.0.0.1");

    // the broker's own checks
    assert_eq!(code(&mut a, "nope", json!({}))["code"], "bad_args");
    assert_eq!(code(&mut a, "snapshot", json!({"target": "17"}))["code"], "bad_args");
    assert_eq!(code(&mut a, "act", json!({"target": t}))["code"], "bad_args");
    assert_eq!(code(&mut a, "open", json!({}))["code"], "bad_args");
    assert_eq!(code(&mut a, "snapshot", json!({"target": "tab:999"}))["code"], "not_found");
    for u in ["chrome://settings", "chrome-extension://x/y.html", "https://chromewebstore.google.com/detail/x"] {
        assert_eq!(code(&mut a, "open", json!({"url": u}))["code"], "refused", "{}", u);
        assert_eq!(code(&mut a, "act", json!({"target": t, "action": "goto", "url": u}))["code"], "refused", "{}", u);
    }
    assert_eq!(code(&mut a, "open", json!({"url": "https://x.org", "browser": "edge"}))["code"], "no_browser");
    // a page that became refused: no more actions, closing still works
    ok(&mut a, "act", json!({"target": t, "action": "goto", "url": "https://accounts.google.com/v3/signin/challenge/pwd"}));
    assert_eq!(code(&mut a, "act", json!({"target": t, "action": "fill", "text": "x", "locator": {"role": "textbox"}}))["code"], "refused");
    ok(&mut a, "act", json!({"target": t, "action": "click", "locator": {"name": "Next"}}));
    // a 6th tab: refused by the extension
    for i in 0..4 {
        ok(&mut a, "open", json!({"url": format!("http://127.0.0.1/{}", i)}));
    }
    assert_eq!(code(&mut a, "open", json!({"url": "http://127.0.0.1/6"}))["code"], "refused");
    ok(&mut a, "act", json!({"target": t, "action": "close"}));
    assert_eq!(code(&mut a, "snapshot", json!({"target": t}))["code"], "not_found");
    assert_eq!(ok(&mut a, "tabs", json!({})).as_array().unwrap().len(), 4);

    // the agent's session ends: it lets go
    drop(a);
    wait_until("release", || ext.control().contains(&json!({"release": "api-v2"})));
    wait_until("idle state", || state::read(&p)["agents"].get("api-v2").is_none());
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn timeout_and_idle_release() {
    let (d, p) = paths();
    let mut o = opts(&p);
    o.slack = Duration::from_millis(200);
    o.release_after = Duration::from_millis(400);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let ext = FakeExt::start(&p, starter(o, brokers.clone()), "edge", jpeg(&d));
    connected(&p, 1);
    let mut a = agent(&p, "w", &d);
    let t = ok(&mut a, "open", json!({"url": "http://127.0.0.1/"}))["target"].as_str().unwrap().to_string();
    assert_eq!(state::read(&p)["agents"]["w"]["driving"], "Edge");
    let t0 = Instant::now();
    let e = code(&mut a, "act", json!({"target": t, "action": "wait", "text": "slow", "timeout_ms": 100}));
    assert_eq!(e["code"], "timeout");
    assert!(t0.elapsed() < Duration::from_millis(2000), "{:?}", t0.elapsed());
    // idle: released while the session stays
    wait_until("idle release", || ext.control().contains(&json!({"release": "w"})));
    assert!(state::read(&p)["agents"].get("w").is_none());
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn apps_through_the_helper() {
    let (d, p) = paths();
    let o = opts(&p);
    let h = broker::start(o).unwrap();
    let tmp = d.join("t");
    let mut a = agent(&p, "figma-bot", &tmp);
    assert_eq!(code(&mut a, "apps", json!({}))["code"], "no_helper");
    assert_eq!(ok(&mut a, "status", json!({}))["apps"]["helper"], "absent");
    let helper = FakeHelper::start(&p, jpeg(&d));
    let apps = ok(&mut a, "apps", json!({}));
    assert_eq!(apps[0]["target"], "app:com.apple.TextEdit");
    let st = ok(&mut a, "status", json!({}));
    assert_eq!(st["apps"], json!({"helper": "running", "accessibility": true, "screen_recording": false}));
    let t = "app:com.apple.TextEdit";
    assert!(ok(&mut a, "snapshot", json!({"target": t}))["text"].as_str().unwrap().contains("TextEdit"));
    let shot = ok(&mut a, "screenshot", json!({"target": t, "window": "Untitled"}));
    assert_eq!(shot["width"], 1280);
    assert!(PathBuf::from(shot["path"].as_str().unwrap()).starts_with(&tmp));
    let r = ok(&mut a, "act", json!({"target": t, "action": "type", "text": "hi"}));
    assert_eq!(r["agent_seen"], "figma-bot");
    let s = state::read(&p);
    assert_eq!((s["agents"]["figma-bot"]["driving"].as_str(), s["agents"]["figma-bot"]["where"].as_str()), (Some("TextEdit"), Some("TextEdit")));
    assert_eq!(s["apps"]["helper"], "running");
    assert_eq!(code(&mut a, "act", json!({"target": t, "action": "goto", "url": "https://x.org"}))["code"], "bad_args");
    for b in ["com.apple.Terminal", "com.mitchellh.ghostty", "com.1password.1password", "com.apple.loginwindow"] {
        assert_eq!(code(&mut a, "snapshot", json!({"target": format!("app:{}", b)}))["code"], "refused", "{}", b);
    }
    assert_eq!(code(&mut a, "act", json!({"target": "app:com.apple.systempreferences", "window": "Privacy & Security", "action": "click"}))["code"], "refused");
    // the user types in the driven app: paused, then resumed
    helper.event(json!({"event": "paused", "agent": "figma-bot", "target": t}));
    wait_until("paused", || state::read(&p)["agents"]["figma-bot"]["paused"] == true);
    assert_eq!(code(&mut a, "act", json!({"target": t, "action": "type", "text": "x"}))["code"], "paused");
    assert_eq!(ok(&mut a, "status", json!({}))["me"]["paused"], json!([t]));
    helper.event(json!({"event": "resumed", "agent": "figma-bot"}));
    wait_until("resumed", || state::read(&p)["agents"]["figma-bot"]["paused"] == false);
    ok(&mut a, "act", json!({"target": t, "action": "type", "text": "x"}));
    // stop reaches the helper too
    cli::control(&p, "stop", &json!({"agent": "figma-bot"})).unwrap();
    wait_until("helper stop", || helper.control.lock().unwrap().contains(&json!({"stop": "figma-bot"})));
    h.shutdown();
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn screen_recording_relaunch_is_expected() {
    let (d, p) = paths();
    let h = broker::start(opts(&p)).unwrap();
    let _helper = FakeHelper::start(&p, jpeg(&d));
    let req = |what: &str| Conn::ctl(&p).unwrap().call("request", &json!({"what": what})).unwrap();
    // accessibility: the helper answers with its permissions
    assert_eq!(req("accessibility").unwrap(), json!({"accessibility": true, "screen_recording": false}));
    // screen recording: macOS quits the helper mid-request; not an error
    assert_eq!(req("screen_recording").unwrap(), json!({"what": "screen_recording", "relaunching": true}));
    // the reopened helper (same default socket) is picked up by the next call
    let mut a = agent(&p, "figma-bot", &d.join("t"));
    assert_eq!(ok(&mut a, "apps", json!({}))[0]["target"], "app:com.apple.TextEdit");
    assert_eq!(req("nope").unwrap_err()["code"], "bad_args");
    h.shutdown();
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn stop_pause_resume_and_drop() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let ext = FakeExt::start(&p, starter(o, brokers.clone()), "chrome", jpeg(&d));
    connected(&p, 1);
    let mut a = agent(&p, "api-v2", &d);
    let t = ok(&mut a, "open", json!({"url": "https://github.com/"}))["target"].as_str().unwrap().to_string();

    // the user clicks in the agent's tab: that tab pauses
    ext.event(json!({"event": "paused", "agent": "api-v2", "target": t}));
    wait_until("paused", || state::read(&p)["agents"]["api-v2"]["paused"] == true);
    assert_eq!(code(&mut a, "act", json!({"target": t, "action": "click"}))["code"], "paused");
    ext.event(json!({"event": "resumed", "agent": "api-v2"}));
    wait_until("resumed", || state::read(&p)["agents"]["api-v2"]["paused"] == false);
    ok(&mut a, "act", json!({"target": t, "action": "click"}));

    // `bise computer-use stop`: a running action fails at once, the next ones too
    let p2 = p.clone();
    let t2 = t.clone();
    let slow = std::thread::spawn(move || {
        let mut b = agent(&p2, "api-v2", std::path::Path::new("/nonexistent"));
        b.call("act", &json!({"target": t2, "action": "wait", "text": "slow", "timeout_ms": 10000})).unwrap()
    });
    wait_until("the slow action", || ext.st.lock().unwrap().requests.iter().any(|r| r["args"]["text"] == "slow"));
    let t0 = Instant::now();
    assert_eq!(cli::control(&p, "stop", &json!({"agent": "api-v2"})).unwrap(), json!({"stopped": ["api-v2"]}));
    assert_eq!(slow.join().unwrap().unwrap_err()["code"], "stopped");
    assert!(t0.elapsed() < Duration::from_millis(2500));
    assert!(ext.control().contains(&json!({"stop": "api-v2"})));
    assert_eq!(code(&mut a, "snapshot", json!({"target": t}))["code"], "stopped");
    assert_eq!(code(&mut a, "open", json!({"url": "https://x.org"}))["code"], "stopped");
    assert_eq!(ok(&mut a, "status", json!({}))["me"]["stopped"], true);
    assert_eq!(state::read(&p)["agents"]["api-v2"]["stopped"], true);
    cli::control(&p, "resume", &json!({"agent": "api-v2"})).unwrap();
    assert!(ext.control().contains(&json!({"resume": "api-v2"})) || {
        wait_until("resume", || ext.control().contains(&json!({"resume": "api-v2"})));
        true
    });
    ok(&mut a, "snapshot", json!({"target": t}));

    // the yellow bar's Cancel, then the group closed
    ext.event(json!({"event": "stopped", "agent": "api-v2", "reason": "cancel_bar"}));
    wait_until("stopped", || state::read(&p)["agents"]["api-v2"]["stopped"] == true);
    assert_eq!(code(&mut a, "snapshot", json!({"target": t}))["code"], "stopped");
    cli::control(&p, "resume", &json!({"agent": "api-v2"})).unwrap();
    ext.event(json!({"event": "stopped", "agent": "api-v2", "reason": "group_closed"}));
    wait_until("stopped again", || state::read(&p)["agents"]["api-v2"]["stopped"] == true);
    let ev: Vec<(String, String)> = state::events(&p)
        .iter()
        .map(|e| (e["event"].as_str().unwrap().to_string(), e["by"].as_str().unwrap().to_string()))
        .collect();
    let want = [("paused", "you"), ("resumed", "you"), ("stopped", "you"), ("resumed", "you"), ("stopped", "cancel_bar"), ("resumed", "you"), ("stopped", "group_closed")];
    assert_eq!(ev, want.map(|(a, b)| (a.to_string(), b.to_string())));
    assert!(state::events(&p).iter().all(|e| e["agent"] == "api-v2" && e["t"].as_u64().is_some()));

    // stop --all, then drop
    cli::control(&p, "resume", &json!({"agent": "api-v2"})).unwrap();
    let mut b = agent(&p, "other", &d);
    ok(&mut b, "open", json!({"url": "https://example.org/"}));
    let all = cli::control(&p, "stop", &json!({"all": true})).unwrap();
    assert_eq!(all, json!({"stopped": ["api-v2", "other"]}));
    cli::control(&p, "drop", &json!({"agent": "other"})).unwrap();
    wait_until("drop", || ext.control().contains(&json!({"drop": "other"})));
    assert!(state::read(&p)["agents"].get("other").is_none());
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn two_agents_two_browsers_at_once() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let img = jpeg(&d);
    let chrome = FakeExt::start(&p, starter(o.clone(), brokers.clone()), "chrome", img.clone());
    connected(&p, 1);
    let edge = FakeExt::start(&p, starter(o, brokers.clone()), "edge", img);
    connected(&p, 2);
    let threads: Vec<_> = ["ana", "bob"]
        .into_iter()
        .map(|name| {
            let (p, d) = (p.clone(), d.clone());
            std::thread::spawn(move || {
                let mut c = agent(&p, name, &d.join(name));
                let browser = if name == "ana" { "chrome" } else { "edge" };
                let mut mine = Vec::new();
                for i in 0..3 {
                    let t = ok(&mut c, "open", json!({"url": format!("https://{}.example/{}", name, i), "browser": browser}));
                    mine.push(t["target"].as_str().unwrap().to_string());
                    let s = ok(&mut c, "screenshot", json!({"target": mine[i]}));
                    assert!(PathBuf::from(s["path"].as_str().unwrap()).starts_with(d.join(name)));
                    ok(&mut c, "act", json!({"target": mine[i], "action": "click", "ref": "e1"}));
                }
                let tabs = ok(&mut c, "tabs", json!({}));
                let got: Vec<String> = tabs.as_array().unwrap().iter().map(|t| t["target"].as_str().unwrap().to_string()).collect();
                (c, mine, got)
            })
        })
        .collect();
    let mut res: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let (_, bob_tabs, bob_got) = res.pop().unwrap();
    let (mut ana, ana_tabs, ana_got) = res.pop().unwrap();
    assert_eq!(ana_got.len(), 3);
    assert_eq!(bob_got.len(), 3);
    // each browser counts its own tab ids: ana's and bob's may share one;
    // the broker routes by agent
    assert!(chrome.st.lock().unwrap().tabs.contains_key("ana") && !chrome.st.lock().unwrap().tabs.contains_key("bob"));
    assert!(edge.st.lock().unwrap().tabs.contains_key("bob"));
    let only_bob: Vec<&String> = bob_tabs.iter().filter(|t| !ana_tabs.contains(t)).collect();
    if let Some(t) = only_bob.first() {
        assert_eq!(code(&mut ana, "snapshot", json!({"target": t}))["code"], "not_found");
    }
    let s = state::read(&p);
    assert_eq!(s["agents"]["ana"]["driving"], "Chrome");
    assert_eq!(s["agents"]["bob"]["driving"], "Edge");
    assert_eq!(s["agents"]["bob"]["where"], "bob.example");
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_broker_restart_through_mcp() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    let start = starter(o, brokers.clone());
    // nobody runs a broker: the relay starts one, like Chrome's first connectNative
    let ext = FakeExt::start(&p, start.clone(), "chrome", jpeg(&d));
    connected(&p, 1);
    let me = mcp::Me { agent: "api-v2".into(), session: "s1".into(), tmpdir: d.join("t") };
    let mut server = mcp::Server::new(p.clone(), start, me);
    let call = |s: &mut mcp::Server, name: &str, args: Value| -> (bool, Value) {
        let r = s.message(&json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        (r["result"]["isError"] == true, serde_json::from_str(text).unwrap())
    };
    let (e, tab) = call(&mut server, "open", json!({"url": "https://www.amazon.fr/"}));
    assert!(!e, "{}", tab);
    let t = tab["target"].as_str().unwrap().to_string();
    // the broker dies
    let old = brokers.lock().unwrap().pop().unwrap();
    old.shutdown();
    // a read is sent again on a fresh broker; the relay came back with its hello
    let (e, snap) = call(&mut server, "snapshot", json!({"target": t}));
    assert!(!e, "{}", snap);
    assert_eq!(brokers.lock().unwrap().len(), 1);
    // an action in flight when the broker dies is not sent twice
    let (e, r) = call(&mut server, "act", json!({"target": t, "action": "click", "ref": "e1"}));
    assert!(!e, "{}", r);
    cli::control(&p, "stop", &json!({"agent": "api-v2"})).unwrap();
    brokers.lock().unwrap().pop().unwrap().shutdown();
    let clicks = || ext.st.lock().unwrap().requests.iter().filter(|r| r["args"]["action"] == "click").count();
    let before = clicks();
    let (e, r) = call(&mut server, "act", json!({"target": t, "action": "click", "ref": "e1"}));
    assert!(e, "the broker gone mid-action is a transport failure: isError");
    assert_eq!(r["error"]["code"], "timeout", "{}", r);
    assert!(r["error"].get("transport").is_none());
    assert_eq!(clicks(), before);
    // the stop outlived the restart; a C1 error is a normal result {"error": {...}}
    let (e, r) = call(&mut server, "snapshot", json!({"target": t}));
    assert!(!e);
    assert_eq!(r["error"]["code"], "stopped");
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mcp_protocol_and_not_set_up() {
    let (d, p) = paths();
    let brokers: Brokers = Default::default();
    let me = mcp::Me { agent: "a".into(), session: "s".into(), tmpdir: d.clone() };
    let server = mcp::Server::new(p.clone(), starter(opts(&p), brokers.clone()), me);
    let input = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "open", "arguments": {"url": "https://x.org"}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "nope"}),
    ]
    .iter()
    .map(|v| v.to_string() + "\n")
    .collect::<String>();
    let mut out = Vec::new();
    assert_eq!(mcp::run(server, input.as_bytes(), &mut out), 0);
    let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "bise-computer-use");
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 7);
    assert_eq!(lines[2]["result"]["isError"], false);
    let e: Value = serde_json::from_str(lines[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(e["error"]["code"], "not_set_up");
    assert_eq!(e["error"]["message"], "computer use isn't set up: ask the user to run /computer-use");
    assert_eq!(lines[3]["error"]["code"], -32601);
    // set up (manifests written), no browser open: no_browser
    std::fs::create_dir_all(p.app_support().join("Google/Chrome")).unwrap();
    crate::browsers::repair(&p, std::path::Path::new("/x/bise"), None).unwrap();
    let mut a = agent(&p, "a", &d);
    assert_eq!(code(&mut a, "open", json!({"url": "https://x.org"}))["code"], "no_browser");
    assert_eq!(code(&mut a, "tabs", json!({}))["code"], "no_browser");
    let st = ok(&mut a, "status", json!({}));
    assert_eq!(st["browsers"], json!([{"name": "Chrome", "version": null, "connected": false, "extension_version": null}]));
    // off/uninstall: the manifest and the shim go, other files stay
    let other = p.app_support().join("Google/Chrome/NativeMessagingHosts/com.other.host.json");
    std::fs::write(&other, "{}").unwrap();
    let gone = crate::browsers::unrepair(&p);
    assert_eq!(gone["manifests"].as_array().unwrap().len(), 1, "{gone}");
    assert!(!crate::browsers::ALL[0].manifest_path(&p).exists() && !p.shim().exists());
    assert!(other.exists(), "never another program's host");
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// ctl `show` (bise ambient, docs/ambient-pages.md §2.8): a command
/// connection's, never an agent's tool; no browser answers at once (no
/// 3 s wait for relays: the caller falls back to its own open); the
/// extension gets url and url_prefix with no agent, and no agent appears.
#[test]
fn show_for_the_user_on_command_connections_only() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let show = |args: Value| Conn::ctl(&p).unwrap().call("show", &args).unwrap();
    let t0 = Instant::now();
    let none = |e: Value| e["code"] == "no_browser" || e["code"] == "not_set_up";
    assert!(none(show(json!({"url": "http://127.0.0.1:47123/p/weekly"})).unwrap_err()));
    assert!(t0.elapsed() < Duration::from_secs(1), "show waited {:?} for a browser", t0.elapsed());

    let ext = FakeExt::start(&p, starter(o, brokers.clone()), "chrome", jpeg(&d));
    connected(&p, 1);
    let r = show(json!({"url": "http://127.0.0.1:47123/p/weekly?v=2", "url_prefix": "http://127.0.0.1:47123/p/weekly"})).unwrap();
    assert_eq!(r, json!({"tab_id": 77, "created": true, "url": "http://127.0.0.1:47123/p/weekly?v=2", "browser": "chrome"}));
    let req = ext.st.lock().unwrap().requests.iter().find(|m| m["op"] == "show").cloned().unwrap();
    assert_eq!(req["args"], json!({"url": "http://127.0.0.1:47123/p/weekly?v=2", "url_prefix": "http://127.0.0.1:47123/p/weekly"}));
    assert!(req.get("agent").is_none());
    assert_eq!(show(json!({"url": "file:///etc/passwd"})).unwrap_err()["code"], "bad_args");
    assert!(none(show(json!({"url": "https://x.org", "browser": "edge"})).unwrap_err()));

    // an agent can't show
    let mut a = agent(&p, "api-v2", &d);
    assert_eq!(code(&mut a, "show", json!({"url": "https://x.org"}))["code"], "bad_args");
    assert!(state::read(&p)["agents"].get("ambient").is_none());
    let _ = std::fs::remove_dir_all(&d);
}

/// docs/issues/18: the commands' socket is the user's only; an agent's MCP
/// server is keyed by its process's tag, not its hello; two projects'
/// "perf" are two agents, and a stop by key reaches one; a browser link
/// from an agent's process is refused; the agents' socket takes no command.
#[test]
fn who_connects_decides() {
    use crate::who::Peer;
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    brokers.lock().unwrap().push(broker::start(o.clone()).unwrap());
    let _ext = FakeExt::start(&p, starter(o, brokers.clone()), "chrome", jpeg(&d));
    connected(&p, 1);
    let tag = |t: &str| Peer::Agent(bise_peer::tags::parse_tag(t).unwrap());
    let (aa, bb) = ("00000000000000aa", "00000000000000bb");

    // an agent's process can't run the user's commands
    for peer in [tag(&format!("{aa}.perf.1")), Peer::Gone] {
        next_peer(&p, peer);
        let e = Conn::ctl(&p).unwrap().call("status", &json!({})).unwrap().unwrap_err();
        assert_eq!((e["code"].as_str(), e["message"].as_str()), (Some("refused"), Some(crate::who::CTL_REFUSED)));
    }
    // the agents' socket takes no command
    let e = Conn::open(&p, None, &json!({"op": "hello", "role": "ctl"})).unwrap().call("status", &json!({})).unwrap().unwrap_err();
    assert_eq!(e["code"], "refused");

    // two projects' perf, each saying it is "main": keyed by their tags
    next_peer(&p, tag(&format!("{aa}.perf.1")));
    let mut a = agent(&p, "main", &d);
    next_peer(&p, tag(&format!("{bb}.perf.7")));
    let mut b = agent(&p, "main", &d);
    ok(&mut a, "open", json!({"url": "https://a.org"}));
    ok(&mut b, "open", json!({"url": "https://b.org"}));
    let st = state::read(&p);
    let ka = format!("{aa}.perf");
    let kb = format!("{bb}.perf");
    assert_eq!((&st["agents"][&ka]["name"], &st["agents"][&ka]["hub"]), (&json!("perf"), &json!(aa)));
    assert_eq!(st["agents"][&kb]["where"], "b.org");
    assert!(st["agents"].get("main").is_none());
    // the extension got the key to route and the name to show
    let req = _ext.st.lock().unwrap().requests.iter().find(|m| m["op"] == "open").cloned().unwrap();
    assert_eq!((req["agent"].as_str(), req["name"].as_str()), (Some(ka.as_str()), Some("perf")));
    // a bare name two projects share is refused; the key stops one
    assert!(cli::control(&p, "stop", &json!({"agent": "perf"})).unwrap_err().contains("2 agents are named perf"));
    cli::control(&p, "stop", &json!({"agent": ka})).unwrap();
    assert_eq!(code(&mut a, "open", json!({"url": "https://a.org"}))["code"], "stopped");
    ok(&mut b, "tabs", json!({}));

    // a browser link from an agent's process is not a browser
    next_peer(&p, tag(&format!("{aa}.perf.1")));
    let mut s = UnixStream::connect(p.socket()).unwrap();
    writeln!(s, "{}", json!({"op": "hello", "role": "browser"})).unwrap();
    let mut line = String::new();
    assert_eq!(BufReader::new(s).read_line(&mut line).unwrap(), 0, "closed at once");
    connected(&p, 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn commands_without_a_broker_and_one_broker_only() {
    let (d, p) = paths();
    // stop with no broker: written in state.json; the next broker keeps it
    cli::control(&p, "stop", &json!({"agent": "x"})).unwrap();
    assert_eq!(state::read(&p)["agents"]["x"]["stopped"], true);
    let h = broker::start(opts(&p)).unwrap();
    assert!(matches!(broker::start(opts(&p)), Err(broker::StartError::Running)));
    let mut a = agent(&p, "x", &d);
    assert_eq!(code(&mut a, "open", json!({"url": "https://x.org"}))["code"], "stopped");
    let check = cli::setup_check(&p, None, None);
    assert_eq!(check["broker"]["running"], true);
    // no helper app: the apps rows say so, nothing asks the broker
    let ids: Vec<&str> = check["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["browser", "extension", "live_test", "accessibility", "screen_recording"]);
    assert_eq!(check["rows"][3]["fix"], "install_helper");
    assert_eq!(check["rows"][4]["state"], "not_yet");
    assert_eq!(check["extension"]["id"], crate::browsers::EXTENSION_ID);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(p.socket()).unwrap().permissions().mode() & 0o777, 0o600);
    h.shutdown();
    // resume with no broker clears it
    cli::control(&p, "resume", &json!({"agent": "x"})).unwrap();
    assert!(state::read(&p)["agents"].get("x").is_none());
    let _ = std::fs::remove_dir_all(&d);
}

/// setup-check's apps rows (m_3897): a 1 s poll never relaunches the
/// helper (one `open -g` per 20 s at most), sees a helper that comes back
/// by itself at once (a plain connect each poll), and reads its grants.
#[test]
fn setup_check_polls_permissions_without_relaunching() {
    let (d, p) = paths();
    // a path that doesn't exist: `open -g` fails at once, nothing opens
    // (a folder would open in Finder, in front of the user)
    let app = d.join("no such helper.app");
    let o = Opts { launch_helper: true, helper_app: Some(app.clone()), ..opts(&p) };
    let brokers: Brokers = Default::default();
    let start = starter(o, brokers.clone());
    for _ in 0..5 {
        let v = cli::setup_check(&p, Some(&app), Some(&*start));
        assert_eq!(v["rows"][3]["state"], "checking", "{v}");
        std::thread::sleep(Duration::from_millis(1000));
    }
    assert_eq!(brokers.lock().unwrap()[0].helper_launches(), 1);
    // the helper comes back (macOS reopened it): the next poll sees it
    let _helper = FakeHelper::start(&p, jpeg(&d));
    let v = cli::setup_check(&p, Some(&app), Some(&*start));
    assert_eq!(v["rows"][3]["id"], "accessibility");
    assert_ne!(v["rows"][3]["state"], "checking", "{v}");
    assert_eq!(brokers.lock().unwrap()[0].helper_launches(), 1);
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn live_test_against_the_fake() {
    let (d, p) = paths();
    let o = opts(&p);
    let brokers: Brokers = Default::default();
    let start = starter(o, brokers.clone());
    let ext = FakeExt::start(&p, start.clone(), "chrome", jpeg(&d));
    connected(&p, 1);
    // the fake doesn't run the page: "read" says what the real page would
    let v = cli::live_test(&p, start);
    // the fake's read returns no text, so the check step fails: the steps before ran
    let names: Vec<&str> = v["steps"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(names, ["open", "snapshot", "type", "click", "check"]);
    assert_eq!(v["ok"], false);
    wait_until("drop setup", || ext.control().contains(&json!({"drop": "setup"})));
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(p.live_test_file()).unwrap()).unwrap();
    assert_eq!(saved["error"], "the click didn't land");
    for h in brokers.lock().unwrap().drain(..) {
        h.shutdown();
    }
    let _ = std::fs::remove_dir_all(&d);
}
