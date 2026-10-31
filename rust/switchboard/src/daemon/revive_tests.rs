//! The proof of site-revive (main m_44354, architect m_44365), on a
//! throwaway state folder and free ports: a local site whose server
//! stopped and whose agent is gone opens again through its door, the hub
//! starting its recipe (a dev server) or serving bise's copy (a static
//! site); another server on its port or its folder gone: a why-page and
//! no start; the hub's quit ends the servers it started.

use super::*;
use crate::artifacts::Add;
use crate::artifacts_probe as probe;
use crate::pages::server::{route, Request, Response};
use crate::pages::{PageMsg, Pages};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A dev server: its html is made by the script, never a file's.
const DEV: &str = "import http.server as h, sys
class H(h.BaseHTTPRequestHandler):
    def do_GET(s):
        b = ('dynamic ' + s.path).encode()
        s.send_response(200); s.send_header('Content-Length', str(len(b))); s.end_headers(); s.wfile.write(b)
    def log_message(s, *a): pass
h.HTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()
";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-revive-{}-{}-{}", name, std::process::id(), crate::util::now_ms()));
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port()
}

/// An agent's server listens: waited on its own life, not on the clock.
/// `until(10, up)` went red at load 14 in side-resize's quick gate (python
/// took over 10 s to bind; green alone, main m_48294). True once it
/// listens; false as soon as it died without listening. The 300 s only
/// stops a server that hangs alive without ever binding.
fn listens(port: u16, server: &mut std::process::Child) -> bool {
    let t0 = Instant::now();
    loop {
        if up(port) {
            return true;
        }
        if !matches!(server.try_wait(), Ok(None)) || t0.elapsed() > Duration::from_secs(300) {
            return up(port);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn until(secs: u64, f: impl Fn() -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(secs) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

fn up(port: u16) -> bool {
    probe::listening(port).is_some()
}

fn body(port: u16, path: &str) -> String {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default()
}

/// An agent's server, started the way an agent's bg job would.
fn agent_server(argv: &[&str], cwd: &Path) -> std::process::Child {
    let mut c = std::process::Command::new(argv[0]);
    c.args(&argv[1..]).current_dir(cwd).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    crate::procs::own_session(&mut c);
    c.spawn().unwrap()
}

fn stop(mut c: std::process::Child) {
    // SAFETY: the group of a child this test started
    unsafe { libc::killpg(c.id() as i32, libc::SIGKILL) };
    let _ = c.wait();
}

/// The page server with the hub's side of `PageMsg::Revive` (what
/// daemon/pages.rs does: the one owner answers).
fn door(state: &Path) -> (Pages, Arc<Mutex<Revive>>) {
    let pages_port = free_port();
    std::fs::write(state.join("pages.port"), pages_port.to_string()).unwrap();
    let pages = Pages::new(state, pages_port, state);
    let revive = Arc::new(Mutex::new(Revive::default()));
    let (rv, st) = (revive.clone(), state.to_path_buf());
    pages.connect_hub(Box::new(move |m| {
        if let PageMsg::Revive { id, reply } = m {
            let (status, body) = rv.lock().unwrap().answer(&Store::new(&st), &st.join("revive"), &id);
            reply.send(status, body);
        }
    }));
    (pages, revive)
}

fn get(p: &Pages, path: &str) -> Response {
    route(p, &Request { method: "GET".into(), path: path.into(), query: String::new(), headers: vec![("Host".into(), format!("127.0.0.1:{}", p.port))], body: vec![] })
}

fn text(r: &Response) -> String {
    String::from_utf8_lossy(&r.body).to_string()
}

fn add_link(store: &Store, link: &str, cwd: &Path) -> String {
    let add = Add { target: link.into(), agent: "maker".into(), by: "maker".into(), cwd: cwd.to_string_lossy().into(), ..Default::default() };
    store.add(&add, 10).unwrap().meta.id
}

#[test]
fn a_dev_server_stopped_with_its_agent_starts_again_on_open() {
    let d = tmp("dev");
    let (state, app) = (d.join("hub"), d.join("work/app"));
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(app.join("serve.py"), DEV).unwrap();
    let port = free_port();
    let p = port.to_string();
    let mut server = agent_server(&["python3", "serve.py", &p], &app);
    assert!(listens(port, &mut server), "the agent's server listens");
    let store = Store::new(&state);
    let link = format!("http://127.0.0.1:{port}/");
    let id = add_link(&store, &link, &app);
    // the add's thread: its recipe, no copy (a dev server's html is its own)
    let (serve, root) = probe::recipe(&link).expect("its listener");
    assert_eq!((serve.cwd.as_str(), serve.port, root), (app.to_str().unwrap(), port, None));
    assert!(serve.argv.iter().any(|a| a == "serve.py"), "{:?}", serve.argv);
    assert!(store.set_serve(&id, 1, serve, None).unwrap());
    // its agent is archived and dropped: its server is gone, nothing of it is left
    stop(server);
    assert!(until(5, || !up(port)));
    let (pages, revive) = door(&state);
    let meta = store.stored().into_iter().find(|m| m.id == id).unwrap();
    assert_eq!(store.live_url(&meta), Some(format!("http://127.0.0.1:{}/a/{id}/live", pages.port)), "the row's url is its door");
    let r = get(&pages, &format!("/a/{id}/live/hello"));
    assert_eq!((r.status, r.location.clone()), (302, Some(format!("http://127.0.0.1:{port}/hello"))), "{}", text(&r));
    assert_eq!(body(port, "/hello"), "dynamic /hello", "the hub started it again");
    let pid = probe::listening(port).unwrap().0;
    // opened again: the same server, never a second start
    assert_eq!(get(&pages, &format!("/a/{id}/live")).status, 302);
    assert_eq!(probe::listening(port).unwrap().0, pid);
    // the hub quits for good: its group goes with it
    revive.lock().unwrap().stop_all(Duration::from_secs(2));
    assert!(until(5, || !up(port)), "stopped with the hub");

    // another server on its port (another agent's, same default port): a why-page, no start
    let other = d.join("work/other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("index.html"), "theirs").unwrap();
    let mut theirs = agent_server(&["python3", "-m", "http.server", &p, "--bind", "127.0.0.1"], &other);
    assert!(listens(port, &mut theirs));
    let r = get(&pages, &format!("/a/{id}/live"));
    assert_eq!((r.status, r.location.is_none()), (502, true));
    assert!(text(&r).contains(&format!("port {port} is used by another server")) && text(&r).contains(other.to_str().unwrap()), "{}", text(&r));
    stop(theirs);
    assert!(until(5, || !up(port)));

    // its folder gone (the worktree dropped): a why-page, nothing started
    std::fs::remove_dir_all(&app).unwrap();
    let r = get(&pages, &format!("/a/{id}/live"));
    assert_eq!(r.status, 502);
    assert!(text(&r).contains("its folder is gone"), "{}", text(&r));
    assert!(!up(port));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_static_site_is_copied_and_opens_from_the_copy_once_its_folder_is_gone() {
    let d = tmp("static");
    let (state, site) = (d.join("hub"), d.join("work/site"));
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.html"), "<h1>static bims</h1><link rel=stylesheet href=style.css>").unwrap();
    std::fs::write(site.join("style.css"), "h1{color:red}").unwrap();
    let port = free_port();
    let mut server = agent_server(&["python3", "-m", "http.server", &port.to_string(), "--bind", "127.0.0.1"], &site);
    assert!(listens(port, &mut server));
    let store = Store::new(&state);
    let link = format!("http://127.0.0.1:{port}/");
    let id = add_link(&store, &link, &site);
    let (serve, root) = probe::recipe(&link).expect("its listener");
    assert_eq!(root.as_deref(), Some(site.as_path()), "GET / is its index.html");
    assert!(store.set_serve(&id, 1, serve, root.as_deref()).unwrap());
    stop(server);
    assert!(until(5, || !up(port)));
    std::fs::remove_dir_all(&site).unwrap();
    let (pages, _revive) = door(&state);
    let r = get(&pages, &format!("/a/{id}/live"));
    assert_eq!((r.status, r.location.clone()), (302, Some(format!("/a/{id}/1/"))), "{}", text(&r));
    let copy = get(&pages, &format!("/a/{id}/1/"));
    assert!(copy.status == 200 && text(&copy).contains("static bims"), "{}", text(&copy));
    assert_eq!(text(&get(&pages, &format!("/a/{id}/1/style.css"))), "h1{color:red}");
    assert!(!up(port), "nothing started: the copy is enough");
    let _ = std::fs::remove_dir_all(&d);
}
