//! The MCP OAuth store in the macOS keychain (bise_secrets, `[secrets]
//! store = "keychain"`), against tests/fake_mcp_http.py --oauth, on a
//! throwaway keychain (never the user's): the login lands in the keychain
//! with a stub at the file's path, a 401 refreshes it there (the refresh
//! token rotates), a locked keychain (simulated: never a real lock, it
//! prompts on his screen) keeps the login (never forgotten),
//! and the token comes back once it is unlocked. Its own test binary: it
//! sets BISE_HOME and BISE_TEST_KEYCHAIN for the whole process.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bend_plugins::http::{self, Url};
use bend_plugins::login::{self, Target};
use bend_plugins::oauth;
use bend_plugins::remote::{Fail, OnChange, Remote};
use bend_plugins::resolve::{HttpServer, Transport};
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

fn env(_: &str) -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn security(args: &[&str]) -> std::process::Output {
    Command::new("/usr/bin/security").args(args).output().unwrap()
}

#[cfg(target_os = "macos")]
#[test]
fn an_mcp_login_lives_and_refreshes_in_the_keychain_and_a_lock_never_forgets_it() {
    let f = fake("kc", &["--mode", "streamable", "--oauth"]);
    // a bise home whose config says keychain, and a throwaway keychain
    let home = f.dir.join("bise");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("config.toml"), "[secrets]\nstore = \"keychain\"\n").unwrap();
    let kc = f.dir.join("t.keychain-db");
    let k = kc.to_str().unwrap();
    for a in [&["create-keychain", "-p", "pw", k][..], &["set-keychain-settings", k], &["unlock-keychain", "-p", "pw", k]] {
        assert!(security(a).status.success(), "{a:?}");
    }
    std::env::set_var("BISE_HOME", &home);
    std::env::remove_var("BEND_CONFIG");
    std::env::remove_var("BISE_EXPORTS_FOR");
    std::env::set_var("BISE_TEST_KEYCHAIN", &kc);
    assert_eq!(bise_secrets::setting::current(), bise_secrets::Store::Keychain);

    let secrets = f.dir.join("secrets");
    let t = f.target(None);
    let page = Arc::new(Mutex::new(String::new()));
    login::run(&t, &secrets, None, &browser(page), T, None, None).unwrap();
    let resource = oauth::resource_of(&Url::parse(&f.url("/mcp")).unwrap());
    let file = oauth::file_of(&secrets, &resource);
    let stub = std::fs::read_to_string(&file).unwrap();
    assert!(stub.starts_with("bise-secret bise-keychain ") && !stub.contains("token"), "the file is a stub: {stub}");
    let first = oauth::load(&secrets, &resource).unwrap();
    let tok = first.access_token.clone().unwrap();
    // bise's own keychain in its home (issue 19 step B), its password in the throwaway
    let own = home.join("secrets").join(bise_secrets::keychain::FILE);
    let found = security(&["find-generic-password", "-s", "bise", "-a", file.to_str().unwrap(), "-w", own.to_str().unwrap()]);
    assert!(found.status.success(), "the item is in bise's keychain");
    assert!(!security(&["find-generic-password", "-s", "bise", "-a", file.to_str().unwrap(), k]).status.success(), "not in the login one");

    // connected; the server drops every token: a 401, a refresh, stored in the keychain
    let c = Remote::start_with(&t.server, &env, Some(&secrets), none(), T).unwrap();
    let call = || c.request_raw("tools/call", json!({"name": "echo", "arguments": {"text": "t"}}), T);
    assert!(call().unwrap()["result"].is_object());
    f.control("expire_tokens");
    let t0 = Instant::now();
    assert!(call().unwrap()["result"].is_object());
    eprintln!("401 + refresh + keychain write: {:?}", t0.elapsed());
    let after = oauth::load(&secrets, &resource).unwrap();
    assert_ne!(after.access_token.as_deref(), Some(tok.as_str()));
    assert_ne!(after.refresh_token, first.refresh_token, "the refresh token rotates");
    assert!(std::fs::read_to_string(&file).unwrap().starts_with("bise-secret bise-keychain "));

    // locked (simulated: BISE_TEST_KEYCHAIN_LOCKED answers as a locked
    // keychain without running security; a real locked keychain would make
    // macOS prompt on his screen): the login stays, a new reader gets an
    // error, never "no login"
    let stub_before = std::fs::read_to_string(&file).unwrap();
    std::env::set_var("BISE_TEST_KEYCHAIN_LOCKED", "1");
    // another process's view: no cache (a fresh generation in the stub would
    // make this one read too); load_now says it can't read, not "none"
    std::fs::write(&file, stub_before.replacen("gen=", "gen=0", 1)).unwrap();
    let locked = oauth::load_now(&secrets, &resource);
    assert!(locked.is_err(), "a locked keychain is an error: {:?}", locked.map(|s| s.is_some()));
    // the server drops the token while locked: not "needs a login" (designer m_13340)
    f.control("expire_tokens");
    match call() {
        Err(Fail::Locked { .. }) => {}
        other => panic!("a 401 while locked is Fail::Locked: {:?}", other.map(|_| ())),
    }
    std::fs::write(&file, &stub_before).unwrap();
    std::env::remove_var("BISE_TEST_KEYCHAIN_LOCKED");
    let back = oauth::load(&secrets, &resource).unwrap();
    assert_eq!(back.refresh_token, after.refresh_token, "the login is still there, unchanged");
    assert!(call().unwrap()["result"].is_object());
}
