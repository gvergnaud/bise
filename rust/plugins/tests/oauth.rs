//! OAuth against tests/fake_mcp_http.py --oauth: discovery from the
//! 401, dynamic registration, PKCE, the loopback redirect (a "browser"
//! that follows the 302), the token exchange, the private store, a
//! refresh after a 401 and before expiry, a revoked login, a denied one,
//! a server without registration; then a bridge that keeps a server
//! needing a login and connects it once the login is in the store.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bend_plugins::http::{self, Url};
use bend_plugins::login::{self, Target};
use bend_plugins::remote::{Fail, OnChange, Remote};
use bend_plugins::resolve::{HttpServer, Transport};
use bend_plugins::{bridge, oauth, resolve, status};
use serde_json::{json, Value};

const T: Duration = Duration::from_secs(10);

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
    let dir = std::env::temp_dir().join(format!("bp-oauth-{}-{}-{}", tag, std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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
        let u = Url::parse(&self.url("/control")).unwrap();
        let body = json!({"action": action}).to_string();
        let r = http::send(&http::Request { method: "POST", url: &u, headers: &[], body: body.as_bytes(), timeout: T }).unwrap();
        serde_json::from_slice(&r.read_all(1 << 20).unwrap()).unwrap()
    }

    fn oauth_steps(&self) -> Vec<Value> {
        self.control("log").as_array().unwrap().iter().filter(|e| e.get("oauth").is_some()).cloned().collect()
    }

    fn target(&self, oauth: Option<oauth::Config>) -> Target {
        Target {
            plugin: "oauth-one".into(),
            server: HttpServer { id: "fake".into(), transport: Transport::Streamable, url: self.url("/mcp"), headers: vec![], oauth, limits: Default::default() },
            name: "fake".into(),
        }
    }
}

fn get(url: &str) -> http::Response {
    let u = Url::parse(url).unwrap();
    http::send(&http::Request { method: "GET", url: &u, headers: &[], body: b"", timeout: T }).unwrap()
}

/// A browser: follows the authorize URL's 302 to bise's callback and
/// keeps the page it shows.
fn browser(page: Arc<Mutex<String>>) -> impl Fn(&str) -> Result<(), String> {
    move |u: &str| {
        let (u, page) = (u.to_string(), page.clone());
        std::thread::spawn(move || {
            let a = get(&u);
            assert_eq!(a.status, 302, "the authorize step");
            let loc = a.header("location").unwrap().to_string();
            let b = get(&loc);
            *page.lock().unwrap() = String::from_utf8(b.read_all(1 << 20).unwrap()).unwrap();
        });
        Ok(())
    }
}

fn none() -> OnChange {
    Arc::new(|| {})
}

fn wait_for(what: &str, f: impl Fn() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(15), "timed out waiting for {}", what);
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn env(_: &str) -> Option<String> {
    None
}

#[test]
fn a_login_registers_uses_pkce_saves_privately_and_refreshes() {
    let f = fake("full", &["--mode", "streamable", "--oauth"]);
    let secrets = f.dir.join("secrets");
    let sd = f.dir.join("status");
    let t = f.target(None);
    // before: a 401
    assert!(matches!(Remote::start_with(&t.server, &env, Some(&secrets), none(), T), Err(Fail::Auth { .. })));
    let page = Arc::new(Mutex::new(String::new()));
    let open = browser(page.clone());
    let n = login::run(&t, &secrets, Some(&sd), &open, T, None, None).unwrap();
    assert_eq!(n, 3);
    assert!(page.lock().unwrap().contains("bise <b>:*</b> is logged in to fake."), "{}", page.lock().unwrap());
    assert_eq!(status::read(&sd, "oauth-one", "fake").unwrap().tools, Ok(3));
    let steps: Vec<String> = f.oauth_steps().iter().map(|e| e["oauth"].as_str().unwrap().to_string()).collect();
    assert_eq!(steps, ["prm", "as", "register", "authorize", "token"], "discovery from the challenge, then DCR");
    let log = f.oauth_steps();
    assert_eq!(log[2]["auth_method"], "none", "a public client");
    assert_eq!(log[3]["scope"], "mcp.read mcp.write");
    assert_eq!(log[3]["resource"], f.url("/mcp"));
    assert_eq!((log[4]["grant"].as_str(), log[4]["resource"].as_str()), (Some("authorization_code"), Some(f.url("/mcp").as_str())));
    // the store: 0600 in a 0700 folder, one file
    use std::os::unix::fs::PermissionsExt;
    let resource = oauth::resource_of(&Url::parse(&f.url("/mcp")).unwrap());
    let file = oauth::file_of(&secrets, &resource);
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(&secrets).unwrap().permissions().mode() & 0o777, 0o700);
    let first = oauth::load(&secrets, &resource).unwrap();
    // connected with the token
    let c = Remote::start_with(&t.server, &env, Some(&secrets), none(), T).unwrap();
    let call = || c.request_raw("tools/call", json!({"name": "echo", "arguments": {"text": "t"}}), T);
    assert!(call().unwrap()["result"].is_object());
    // the server drops every access token: a 401, a refresh, the call goes through
    f.control("expire_tokens");
    assert!(call().unwrap()["result"].is_object());
    let after = oauth::load(&secrets, &resource).unwrap();
    assert_ne!(after.access_token, first.access_token);
    assert_ne!(after.refresh_token, first.refresh_token, "the refresh token rotates");
    assert_eq!(f.oauth_steps().last().unwrap()["grant"], "refresh_token");
    // a second login reuses the registration (same port, same client)
    let open = browser(page.clone());
    login::run(&t, &secrets, Some(&sd), &open, T, None, None).unwrap();
    assert_eq!(f.oauth_steps().iter().filter(|e| e["oauth"] == "register").count(), 1);
    // refused for good: a login is needed again, the client is kept
    f.control("expire_tokens");
    f.control("revoke_refresh");
    assert!(matches!(call(), Err(Fail::Auth { .. })));
    let gone = oauth::load(&secrets, &resource).unwrap();
    assert_eq!((gone.access_token.is_none(), gone.refresh_token.is_none(), gone.client_id.is_empty()), (true, true, false));
}

#[test]
fn a_short_lived_token_is_refreshed_before_it_expires() {
    let f = fake("ttl", &["--mode", "streamable", "--oauth", "--token-ttl", "30"]);
    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    let page = Arc::new(Mutex::new(String::new()));
    login::run(&t, &secrets, None, &browser(page), T, None, None).unwrap();
    let refreshes = || f.oauth_steps().iter().filter(|e| e["grant"] == "refresh_token").count();
    let before = refreshes();
    let c = Remote::start_with(&t.server, &env, Some(&secrets), none(), T).unwrap();
    assert!(c.list_tools(T).is_ok());
    assert!(refreshes() > before, "30 s left is under the minute: refreshed without a 401");
    let unauthorized = f.control("log").as_array().unwrap().iter().filter(|e| e["auth_ok"] == false && e["method"] == "POST").count();
    assert_eq!(unauthorized, 1, "only the probe before the login was refused");
}

#[test]
fn a_token_endpoint_that_is_down_keeps_the_login() {
    let f = fake("down", &["--mode", "streamable", "--oauth", "--token-ttl", "30"]);
    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    let page = Arc::new(Mutex::new(String::new()));
    login::run(&t, &secrets, None, &browser(page), T, None, None).unwrap();
    let resource = oauth::resource_of(&Url::parse(&f.url("/mcp")).unwrap());
    // the refresh a minute before expiry gets a 503: the token still
    // works for 30 s, so it is sent, and the login stays (the login's
    // own client may still be refreshing in its notification thread)
    f.control("token_down");
    std::thread::sleep(Duration::from_millis(300));
    let first = oauth::load(&secrets, &resource).unwrap();
    assert!(first.access_token.is_some() && first.refresh_token.is_some());
    let c = Remote::start_with(&t.server, &env, Some(&secrets), none(), T).unwrap();
    assert_eq!(c.list_tools(T).unwrap().len(), 3);
    let kept = oauth::load(&secrets, &resource).unwrap();
    assert_eq!((kept.access_token.as_ref(), kept.refresh_token.as_ref()), (first.access_token.as_ref(), first.refresh_token.as_ref()));
    // up again: the next request refreshes
    f.control("token_up");
    assert_eq!(c.list_tools(T).unwrap().len(), 3);
    assert_ne!(oauth::load(&secrets, &resource).unwrap().access_token, first.access_token);
}

#[test]
fn a_server_that_sends_iss_must_send_its_own() {
    let page = Arc::new(Mutex::new(String::new()));
    let f = fake("iss", &["--mode", "streamable", "--oauth", "--iss", "good"]);
    assert_eq!(login::run(&f.target(None), &f.dir.join("secrets"), None, &browser(page.clone()), T, None, None).unwrap(), 3);
    for (tag, how) in [("issbad", "bad"), ("issnone", "missing")] {
        let f = fake(tag, &["--mode", "streamable", "--oauth", "--iss", how]);
        let e = login::run(&f.target(None), &f.dir.join("secrets"), None, &browser(page.clone()), T, None, None).unwrap_err();
        assert_eq!(e, "the answer came from another login server (iss)", "{}", how);
        assert!(!f.oauth_steps().iter().any(|s| s["oauth"] == "token"), "{}: the code is never sent", how);
    }
}

#[test]
fn a_denied_login_and_a_server_without_registration_say_why() {
    let f = fake("deny", &["--mode", "streamable", "--oauth"]);
    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    f.control("deny_next");
    let page = Arc::new(Mutex::new(String::new()));
    let e = login::run(&t, &secrets, None, &browser(page.clone()), T, None, None).unwrap_err();
    assert_eq!(e, "access was denied");
    wait_for("the error page", || !page.lock().unwrap().is_empty());
    assert!(page.lock().unwrap().contains("the login didn't go through: access was denied."));
    assert_eq!(login::done_line("fake", &Err(e)), "▲ couldn't log in to fake: access was denied. /plugins login tries again.");
    // a browser that never comes back: the wait ends with the designer's reason
    let e = login::run(&t, &secrets, None, &|_: &str| Ok(()), Duration::from_secs(1), None, None).unwrap_err();
    assert_eq!(e, "the browser login wasn't finished in 1 min");

    let f = fake("nodcr", &["--mode", "streamable", "--oauth", "--no-dcr"]);
    let secrets = f.dir.join("secrets");
    let e = login::run(&f.target(None), &secrets, None, &browser(page.clone()), T, None, None).unwrap_err();
    assert!(e.starts_with("fake's login server takes no app registration: add \"oauth\": {\"clientId\""), "{}", e);
    // a registered client in mcp.json
    let cfg = oauth::Config { client_id: Some("preregistered".into()), ..Default::default() };
    assert_eq!(login::run(&f.target(Some(cfg)), &secrets, None, &browser(page), T, None, None).unwrap(), 3);
}

#[test]
fn the_bridge_keeps_a_server_that_needs_a_login_and_connects_it_after() {
    let f = fake("bridge", &["--mode", "streamable", "--oauth"]);
    let base = f.dir.join("home");
    let p = base.join("ws/.agents/plugins/oauth-one");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("plugin.json"), format!("{{\"$schema\":\"{}\",\"name\":\"oauth-one\",\"version\":\"1.0.0\"}}", resolve::PLUGIN_SCHEMA)).unwrap();
    std::fs::write(p.join("mcp.json"), json!({"mcpServers": {"fake": {"type": "http", "url": f.url("/mcp")}}}).to_string()).unwrap();
    let (dir, sd, secrets) = (base.join("run/plugins"), base.join("status"), base.join("secrets"));
    let roots = resolve::Roots {
        builtin: None,
        user: None,
        workspace: Some(base.join("ws/.agents/plugins")),
        data: base.join("data"),
        disabled: vec![],
        enabled: vec![],
    };
    let mut parent = Command::new("sleep").arg("60").spawn().unwrap();
    let opts = bridge::Opts { dir: dir.clone(), parent: Some(parent.id()), roots, status_dir: Some(sd.clone()), secrets_dir: Some(secrets.clone()), ready_wait: bridge::READY_WAIT };
    let srv = std::thread::spawn(move || bridge::serve(opts));
    wait_for("the bridge", || dir.join("ready").exists());
    let report = std::fs::read_to_string(dir.join("report.txt")).unwrap();
    assert!(report.contains("plugin.mcp.login_needed [oauth-one] MCP server \"fake\" needs a login: /plugins login"), "{}", report);
    assert_eq!(std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap(), "", "no tools before the login");
    let st = status::read(&sd, "oauth-one", "fake").unwrap();
    assert!(st.login);
    // the static listing says so, the designer's words
    let res = resolve::resolve(&resolve::Roots {
        builtin: None,
        user: None,
        workspace: Some(base.join("ws/.agents/plugins")),
        data: base.join("data"),
        disabled: vec![],
        enabled: vec![],
    });
    let listing = bend_plugins::report::text_with(&res, None, Some(&sd));
    assert!(listing.contains(&format!("  mcp fake · 127.0.0.1:{} · needs a login · /plugins login", f.port)), "{}", listing);
    // the login (as /plugins login runs it): the bridge sees the store and connects
    let t = f.target(None);
    let page = Arc::new(Mutex::new(String::new()));
    login::run(&t, &secrets, Some(&sd), &browser(page), T, None, None).unwrap();
    wait_for("the bridge to connect", || std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap_or_default().lines().count() == 3);
    let listing = bend_plugins::report::text_with(&res, None, Some(&sd));
    assert!(listing.contains(&format!("  mcp fake · 127.0.0.1:{} · connected · 3 tools · just now", f.port)), "{}", listing);
    // a call through the bridge carries the token
    let index = std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap();
    let u = Url::parse(index.split(' ').next().unwrap()).unwrap();
    let h = vec![("Content-Type".to_string(), "application/json".to_string())];
    let body = json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "echo", "arguments": {"text": "x"}}}).to_string();
    let call = || -> Value {
        let r = http::send(&http::Request { method: "POST", url: &u, headers: &h, body: body.as_bytes(), timeout: T }).unwrap();
        serde_json::from_slice(&r.read_all(1 << 20).unwrap()).unwrap()
    };
    assert!(call()["result"].is_object());
    // the login is gone for good: the agent reads the designer's words
    f.control("expire_tokens");
    f.control("revoke_refresh");
    let r = call();
    assert_eq!(r["error"]["message"], "fake needs the user to log in (/plugins login). tell them, or go on without it.");
    parent.kill().unwrap();
    parent.wait().unwrap();
    wait_for("the bridge to stop", || srv.is_finished());
    srv.join().unwrap().unwrap();
}

/// A 403 `insufficient_scope` (MCP authorization 2025-11-25 step-up):
/// the agent reads that a login is needed, /plugins says so, and the
/// next login asks for the scopes granted before plus the one the
/// challenge names; then the call goes through.
#[test]
fn a_403_insufficient_scope_logs_in_again_with_the_scope_asked_for() {
    let f = fake("scope", &["--mode", "streamable", "--oauth"]);
    let secrets = f.dir.join("secrets");
    let sd = f.dir.join("status");
    let t = f.target(None);
    let page = Arc::new(Mutex::new(String::new()));
    login::run(&t, &secrets, Some(&sd), &browser(page.clone()), T, None, None).unwrap();
    let c = Remote::start_with(&t.server, &env, Some(&secrets), none(), T).unwrap();
    let admin = || c.request_raw("tools/call", json!({"name": "admin", "arguments": {}}), T);
    let e = admin().unwrap_err();
    assert!(matches!(e, Fail::Scope { .. }), "{e:?}");
    assert_eq!(e.to_string(), format!("127.0.0.1:{} answered 403: the login needs more access (insufficient_scope)", f.port));
    let resource = oauth::resource_of(&Url::parse(&f.url("/mcp")).unwrap());
    assert_eq!(oauth::load(&secrets, &resource).unwrap().wanted_scope.as_deref(), Some("mcp.admin"));
    login::run(&t, &secrets, Some(&sd), &browser(page), T, None, None).unwrap();
    let asked: Vec<Value> = f.oauth_steps().into_iter().filter(|e| e["oauth"] == "authorize").collect();
    assert_eq!(asked.last().unwrap()["scope"], "mcp.read mcp.write mcp.admin", "granted before, then the step-up's");
    assert_eq!(oauth::load(&secrets, &resource).unwrap().wanted_scope, None, "the new login has it");
    assert_eq!(admin().unwrap()["result"]["content"][0]["text"], "admin ok");
}

/// `/plugins logout` revokes the tokens at the login server (RFC 7009)
/// when its metadata names a revocation endpoint, then forgets them.
#[test]
fn a_logout_revokes_the_tokens_at_the_login_server() {
    let f = fake("revoke", &["--mode", "streamable", "--oauth"]);
    let secrets = f.dir.join("secrets");
    let sd = f.dir.join("status");
    let t = f.target(None);
    login::run(&t, &secrets, Some(&sd), &browser(Arc::new(Mutex::new(String::new()))), T, None, None).unwrap();
    let resource = oauth::resource_of(&Url::parse(&f.url("/mcp")).unwrap());
    let saved = oauth::load(&secrets, &resource).unwrap();
    assert_eq!(saved.revocation_endpoint.as_deref(), Some(f.url("/revoke").as_str()));
    login::logout(&t, &secrets, Some(&sd)).unwrap();
    let revoked: Vec<Value> = f.oauth_steps().into_iter().filter(|e| e["oauth"] == "revoke").collect();
    assert_eq!(revoked.len(), 2, "{revoked:?}");
    assert_eq!((revoked[0]["hint"].as_str(), revoked[0]["known"].as_bool()), (Some("refresh_token"), Some(true)));
    assert_eq!((revoked[1]["hint"].as_str(), revoked[1]["known"].as_bool()), (Some("access_token"), Some(true)));
    assert_eq!(revoked[0]["client"], json!(saved.client_id));
    let gone = oauth::load(&secrets, &resource).unwrap();
    assert!(gone.access_token.is_none() && gone.refresh_token.is_none() && !gone.client_id.is_empty());
    // the server forgot them too: the old token is refused
    let headers = vec![("Authorization".to_string(), format!("Bearer {}", saved.access_token.unwrap()))];
    let c = Remote::connect(Transport::Streamable, Url::parse(&f.url("/mcp")).unwrap(), headers, None, none(), T);
    assert!(matches!(c, Err(Fail::Auth { .. })));
}

/// A login server that takes client ID metadata documents (CIMD, MCP
/// authorization 2025-11-25) gets bise's document URL as the client:
/// no registration; it fetches the document and checks the redirect.
#[test]
fn a_server_that_takes_cimd_gets_the_document_not_a_registration() {
    let f = fake("cimd", &["--mode", "streamable", "--oauth", "--cimd"]);
    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    let url = Url::parse(&f.url("/mcp")).unwrap();
    let challenge = match Remote::start(&t.server, &env, none(), T) {
        Err(Fail::Auth { challenge, .. }) => challenge,
        r => panic!("{:?}", r.err()),
    };
    let doc = f.url("/client.json");
    let open = browser(Arc::new(Mutex::new(String::new())));
    let saved = oauth::login(&oauth::Login {
        server: &url,
        name: "fake",
        config: &oauth::Config::default(),
        dir: &secrets,
        challenge: challenge.as_deref(),
        open: &open,
        wait: T,
        cancel: None,
        cimd: &doc,
        paste: None,
    })
    .unwrap();
    assert_eq!(saved.client_id, doc);
    let steps: Vec<String> = f.oauth_steps().iter().map(|e| e["oauth"].as_str().unwrap().to_string()).collect();
    assert_eq!(steps, ["prm", "as", "authorize", "client_doc", "cimd", "token"], "no register step");
    assert_eq!(f.oauth_steps().iter().find(|e| e["oauth"] == "cimd").unwrap()["ok"], true);
    assert!(Remote::start_with(&t.server, &env, Some(&secrets), none(), T).is_ok());
    // bise's own document says the same (site/oauth/client.json)
    let site: Value = serde_json::from_str(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/oauth/client.json")).unwrap()).unwrap();
    assert_eq!(site["client_id"], oauth::CLIENT_ID);
    assert_eq!(site["token_endpoint_auth_method"], "none");
    assert_eq!(site["redirect_uris"], json!(["http://127.0.0.1/callback", "http://localhost/callback"]));
}

/// The browser on another machine (bise over SSH): its redirect to
/// 127.0.0.1 never loads, the user pastes the address it ended on. A
/// wrong address is said and the login keeps waiting; the right one
/// finishes it. `oauth.callbackUrl` sets the redirect bise sends.
#[test]
fn a_pasted_redirect_address_finishes_the_login() {
    let f = fake("paste", &["--mode", "streamable", "--oauth"]);
    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    let landed = Arc::new(Mutex::new(Vec::<String>::new()));
    let l2 = landed.clone();
    // "logs in" elsewhere: follows the authorize step, never the redirect
    let open = move |u: &str| {
        let a = get(u);
        assert_eq!(a.status, 302);
        let loc = a.header("location").unwrap().to_string();
        let mut l = l2.lock().unwrap();
        l.push(loc.replace("state=", "state=x"));
        l.push(format!("  {}  ", loc.replace("127.0.0.1", "localhost")));
        Ok(())
    };
    let bad = AtomicUsize::new(0);
    let next = || {
        let mut l = landed.lock().unwrap();
        (!l.is_empty()).then(|| l.remove(0))
    };
    let said = || {
        bad.fetch_add(1, Ordering::SeqCst);
    };
    let paste = oauth::Paste { next: &next, bad: &said };
    assert_eq!(login::run(&t, &secrets, None, &open, T, None, Some(&paste)).unwrap(), 3);
    assert_eq!(bad.load(Ordering::SeqCst), 1, "the wrong address was said once");

    // a fixed callbackUrl: sent as the redirect, listened for on its port
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let cfg = oauth::Config::parse(&json!({"callbackUrl": format!("http://localhost:{}/oauth/back", port)})).unwrap();
    let page = Arc::new(Mutex::new(String::new()));
    let page2 = page.clone();
    let open = move |u: &str| {
        assert!(u.contains(&format!("redirect_uri=http%3A%2F%2Flocalhost%3A{}%2Foauth%2Fback", port)), "{u}");
        browser(page2.clone())(u)
    };
    let t2 = f.target(Some(cfg));
    assert_eq!(login::run(&t2, &f.dir.join("secrets2"), None, &open, T, None, None).unwrap(), 3);
    assert!(page.lock().unwrap().contains("is logged in to fake"), "{}", page.lock().unwrap());
    assert!(oauth::Config::parse(&json!({"callbackUrl": "http://127.0.0.1/back"})).unwrap_err().contains("must name its port"));
    assert!(oauth::Config::parse(&json!({"callbackUrl": "nope"})).is_err());
}
