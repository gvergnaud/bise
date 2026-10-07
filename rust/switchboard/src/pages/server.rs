//! The page server (docs/ambient-pages.md §2.4), inside the hub: HTTP/1.1
//! on 127.0.0.1 only, one thread per connection, a small hand parser
//! (no crate). Every response carries the CSP (no inline, no outside);
//! a Host other than the server's own is refused (DNS rebinding), and
//! `/api/*` needs the session token (`X-Bise-Token`, in the shell's
//! `<meta name="bise-token">`) and the server's own `Origin`.

use super::store::valid_id;
use super::{PageMsg, Pages};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

pub const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'";
const MAX_HEAD: usize = 64 * 1024;
const MAX_BODY: usize = 2 * 1024 * 1024;
/// An SSE stream's keep-alive comment.
const PING: Duration = Duration::from_secs(15);

/// Accept connections on their own thread.
pub fn serve(listener: TcpListener, pages: Arc<Pages>) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            // out of descriptors (EMFILE) or a reset before accept: wait a
            // little instead of spinning; the connection stays queued
            let Ok(stream) = conn else {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            let pages = pages.clone();
            std::thread::spawn(move || handle(stream, &pages));
        }
    });
}

#[derive(Debug, Default, PartialEq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// One request off the stream; None: closed, malformed or too big.
pub fn read_request(r: &mut impl BufRead) -> Option<Request> {
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let path = target.split(['?', '#']).next().unwrap_or("/").to_string();
    let mut headers = Vec::new();
    let mut size = line.len();
    loop {
        let mut h = String::new();
        let n = r.read_line(&mut h).ok()?;
        size += n;
        if n == 0 || size > MAX_HEAD {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (k, v) = h.split_once(':')?;
        headers.push((k.trim().to_string(), v.trim().to_string()));
    }
    let mut req = Request { method, path, headers, body: Vec::new() };
    let len: usize = req.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        return None;
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    req.body = body;
    Some(req)
}

pub struct Response {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
    /// the response's CSP: [`CSP`], or a site artifact's sandbox (site.rs)
    pub csp: &'static str,
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Error",
    }
}

impl Response {
    fn json(status: u16, v: &Value) -> Response {
        Response { status, ctype: "application/json", body: v.to_string().into_bytes(), csp: CSP }
    }
    fn html(body: String) -> Response {
        Response { status: 200, ctype: "text/html; charset=utf-8", body: body.into_bytes(), csp: CSP }
    }
    fn err(status: u16, why: &str) -> Response {
        Response::json(status, &json!({"error": why}))
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            self.status,
            reason(self.status),
            self.ctype,
            self.body.len(),
            self.csp
        )
        .into_bytes();
        out.extend_from_slice(&self.body);
        out
    }
}

fn handle(stream: TcpStream, pages: &Pages) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    // read through a borrow, no second descriptor (pm's B, m_5765: a
    // try_clone that failed with no descriptor left closed the connection
    // with no response while the hub was busy republishing); a request we
    // can't read gets a 400, never a silent close
    let req = {
        let mut r = BufReader::new(&stream);
        read_request(&mut r)
    };
    let mut w = stream;
    let Some(req) = req else {
        let _ = w.write_all(&Response::err(400, "bad request").bytes());
        return;
    };
    if let Some(id) = events_route(&req) {
        if host_ok(pages, &req) {
            return events(&mut w, pages, &id);
        }
    }
    let resp = route(pages, &req);
    let _ = w.write_all(&resp.bytes());
}

/// `GET /p/<id>/events` (or `/events` for the home list): the page id.
fn events_route(req: &Request) -> Option<String> {
    if req.method != "GET" {
        return None;
    }
    if req.path == "/events" {
        return Some(String::new());
    }
    let id = req.path.strip_prefix("/p/")?.strip_suffix("/events")?;
    valid_id(id).then(|| id.to_string())
}

/// The Host is the server's own (127.0.0.1 or localhost, its port).
fn host_ok(pages: &Pages, req: &Request) -> bool {
    let p = pages.port;
    matches!(req.header("host"), Some(h) if h == format!("127.0.0.1:{p}") || h == format!("localhost:{p}"))
}

fn origin_ok(pages: &Pages, req: &Request) -> bool {
    let p = pages.port;
    matches!(req.header("origin"), Some(o) if o == format!("http://127.0.0.1:{p}") || o == format!("http://localhost:{p}"))
}

/// Compare without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Everything but the SSE streams.
pub fn route(pages: &Pages, req: &Request) -> Response {
    if !host_ok(pages, req) {
        return Response::err(403, "not this server");
    }
    let segs: Vec<&str> = req.path.trim_start_matches('/').split('/').collect();
    if segs.first() == Some(&"api") {
        if req.method != "POST" {
            return Response::err(405, "POST only");
        }
        let token_ok = req.header("x-bise-token").is_some_and(|t| same(t, &pages.token));
        if !token_ok || !origin_ok(pages, req) {
            return Response::err(403, "no token or another origin");
        }
        return match segs.as_slice() {
            ["api", "p", id, "notes"] if valid_id(id) => post_notes(pages, id, &req.body),
            ["api", "p", id, "send"] if valid_id(id) => post_send(pages, id),
            ["api", "p", id, "answer"] if valid_id(id) => post_answer(pages, id, &req.body),
            _ => Response::err(404, "no such route"),
        };
    }
    if req.method != "GET" {
        return Response::err(405, "GET only");
    }
    match segs.as_slice() {
        [""] => Response::html(home(pages)),
        ["kit", rest @ ..] if !rest.is_empty() => kit_file(pages, &rest.join("/")),
        ["a", id, v, rest @ ..] if valid_id(id) => artifact_file(pages, id, v, rest),
        ["p", id] if valid_id(id) => page(pages, id, None, false),
        ["p", id, "meta"] if valid_id(id) => meta(pages, id),
        ["p", id, "versions"] if valid_id(id) => versions(pages, id),
        ["p", id, "v", n] if valid_id(id) => n.parse().map_or_else(|_| Response::err(404, "no such version"), |n| page(pages, id, Some(n), false)),
        ["p", id, "v", n, "body"] if valid_id(id) => n.parse().map_or_else(|_| Response::err(404, "no such version"), |n| page(pages, id, Some(n), true)),
        ["p", id, "v", n, "page.css"] if valid_id(id) => n.parse().map_or_else(|_| Response::err(404, "no such version"), |n| page_css(pages, id, n)),
        _ => Response::err(404, "no such page"),
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// The shell around a fragment (§2.5): the kit, the token, the page's ids.
fn shell(pages: &Pages, title: &str, head: &str, main_attrs: &str, inner: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"bise-token\" content=\"{}\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"/kit/tokens.css\">\n<link rel=\"stylesheet\" href=\"/kit/kit.css\">\n<link rel=\"stylesheet\" href=\"/kit/notes.css\">\n<script src=\"/kit/kit.js\" defer></script>\n<script src=\"/kit/pearl.js\" defer></script>\n<script src=\"/kit/notelines.js\" defer></script>
<script src=\"/kit/notes.js\" defer></script>\n{head}</head>\n<body>\n<main {main_attrs}>\n{inner}\n</main>\n</body>\n</html>\n",
        pages.token,
        esc(title)
    )
}

fn page(pages: &Pages, id: &str, n: Option<u64>, body_only: bool) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    let n = n.unwrap_or(m.version());
    // `sb page start`'s placeholder: no version yet, an empty body (the
    // kit shows the agent writing, with its progress lines)
    let html = match pages.store.html(id, n) {
        Some(h) => h,
        None if n == 0 && m.version() == 0 => String::new(),
        None => return Response::err(404, "no such version"),
    };
    // a ui block's <style> is served as the version's page.css (the CSP is 'self')
    let html = super::ui::strip_styles(&html);
    if body_only {
        return Response::html(html);
    }
    // the user opened it in a browser (ambient-lead m_5129): the
    // capsule's unopened count follows, wherever he opened it from
    if n > 0 && pages.locked(|s| s.opened(id, n)).ok().flatten().is_some() {
        pages.push("", "pages", &json!({"pages": pages.list_json()}));
        pages.tell_hub(PageMsg::Opened { id: id.to_string() });
    }
    // amb-kit's writing view (m_5105): when it started, what the user asked
    let ask = m.ask.as_deref().map(|a| format!(" data-ask=\"{}\"", esc(a))).unwrap_or_default();
    let attrs = format!(
        "id=\"bise-page\" data-page=\"{}\" data-version=\"{n}\" data-latest=\"{}\" data-agent=\"{}\" data-title=\"{}\" data-state=\"{}\" data-started=\"{}\"{ask}{}",
        esc(&m.id),
        m.version(),
        esc(&m.agent),
        esc(&m.title),
        esc(&m.state),
        m.created_ms,
        // --public: the frame says so before the mirror runs (ambient-lead m_7535)
        if m.public { " data-public" } else { "" }
    );
    let head = format!("<link rel=\"stylesheet\" href=\"/p/{}/v/{n}/page.css\" data-page-css>\n", esc(&m.id));
    Response::html(shell(pages, &m.title, &head, &attrs, &html))
}

/// "for you": the list of pages, newest first.
fn home(pages: &Pages) -> String {
    let items: Vec<String> = pages
        .store
        .list()
        .iter()
        .map(|m| {
            let open = pages.store.notes(&m.id).iter().filter(|n| n.open()).count();
            // amb-kit's home list (m_4884): the meta line, an open question
            let kicker = pages.kicker(m).map(|k| format!(" data-kicker=\"{}\"", esc(&k))).unwrap_or_default();
            let asking = if pages.asking(&m.id) { " data-asking" } else { "" };
            format!(
                "<li data-page=\"{}\" data-agent=\"{}\" data-version=\"{}\" data-state=\"{}\" data-at=\"{}\" data-open-notes=\"{open}\"{kicker}{asking}><a href=\"/p/{}\">{}</a></li>",
                esc(&m.id),
                esc(&m.agent),
                m.version(),
                esc(&m.state),
                m.at_ms(),
                esc(&m.id),
                esc(&m.title)
            )
        })
        .collect();
    let inner = format!("<section data-kit=\"pages\" data-id=\"pages\">\n<ul>\n{}\n</ul>\n</section>", items.join("\n"));
    // one site for everything agents made to look at (site.rs): the sidebar, then "for you"
    let arts = crate::artifacts::Store::new(pages.store.dir.parent().unwrap_or(&pages.store.dir)).all();
    let aside = super::site::sidebar(&super::site::items(&pages.store.list(), &arts), "/");
    let head = "<link rel=\"stylesheet\" href=\"/kit/site.css\">\n<script src=\"/kit/site.js\" defer></script>\n";
    shell(pages, "for you", head, "id=\"bise-home\" data-home", &format!("{aside}{inner}"))
}

/// A site artifact's file, from bise's copy (`/a/<id>/<v>/<path>`), in its sandbox (site.rs).
fn artifact_file(pages: &Pages, id: &str, v: &str, rest: &[&str]) -> Response {
    let state = pages.store.dir.parent().unwrap_or(&pages.store.dir);
    let store = crate::artifacts::Store::new(state);
    let Some(m) = store.get(id).filter(|m| m.kind == "site" && m.by != "page") else { return Response::err(404, "no such artifact") };
    let Some(ver) = v.parse::<u64>().ok().and_then(|n| m.versions.iter().find(|x| x.v == n)) else { return Response::err(404, "no such version") };
    let Some(copy) = &ver.copy else { return Response::err(404, "no copy of it") };
    let base = state.join("artifacts").join(&m.id).join(copy);
    let rest: Vec<&str> = rest.iter().copied().filter(|s| !s.is_empty()).collect();
    let path = if base.is_file() {
        // a copied file: only itself, by its name (or no name)
        let name = base.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !(rest.is_empty() || rest == [name]) {
            return Response::err(404, "no such file");
        }
        base.clone()
    } else {
        let Some(rel) = super::site::safe_rel(&rest) else {
            return if rest.is_empty() { artifact_file(pages, id, v, &["index.html"]) } else { Response::err(404, "no such file") };
        };
        base.join(rel)
    };
    let Some(ctype) = super::site::ctype(&path.to_string_lossy()) else { return Response::err(404, "not served") };
    match std::fs::read(&path) {
        Ok(body) => Response { status: 200, ctype, body, csp: super::site::ARTIFACT_CSP },
        Err(_) => Response::err(404, "no such file"),
    }
}

fn meta(pages: &Pages, id: &str) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    // flat (docs/ambient-pages.md §2.4): meta.json's fields, the url, the notes
    let mut v = serde_json::to_value(&m).unwrap_or_default();
    v["url"] = json!(pages.url(id));
    v["notes"] = json!(pages.store.notes(id));
    Response::json(200, &v)
}

/// The frame's version list (§4.4), newest first: each version, when,
/// its read-only URL and the blocks it changed (their hash differs from
/// the version before, or they are new; the first changes them all).
fn versions(pages: &Pages, id: &str) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    let mut list: Vec<Value> = Vec::new();
    for (i, v) in m.versions.iter().enumerate() {
        let before = i.checked_sub(1).map(|j| &m.versions[j].blocks);
        let changed: Vec<&str> = v
            .blocks
            .iter()
            .filter(|b| before.is_none_or(|bs| !bs.iter().any(|o| o.id == b.id && o.hash == b.hash)))
            .map(|b| b.id.as_str())
            .collect();
        list.push(json!({"n": v.n, "at_ms": v.at_ms, "url": format!("/p/{}/v/{}", m.id, v.n), "changed": changed, "latest": v.n == m.version()}));
    }
    list.reverse();
    Response::json(200, &json!({"id": m.id, "versions": list}))
}

/// A pick or words on a question block (§4.2): `{block, option}` (the
/// 1-based option) or `{block, text}`; the hub answers its card.
fn post_answer(pages: &Pages, id: &str, body: &[u8]) -> Response {
    if pages.store.meta(id).is_none() {
        return Response::err(404, "no such page");
    }
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return Response::err(400, "not JSON") };
    let block = v.get("block").and_then(Value::as_str).unwrap_or("");
    let qs = pages.store.questions(id);
    let Some(q) = qs.iter().find(|q| q.block == block) else { return Response::err(404, "no such question on this page") };
    if q.reply.is_some() {
        return Response::err(409, "answered already");
    }
    let reply = match (v.get("option").and_then(Value::as_u64), v.get("text").and_then(Value::as_str)) {
        (Some(n), _) => match q.option(n as usize) {
            Some(o) => o.to_string(),
            None => return Response::err(400, "no such option"),
        },
        (None, Some(t)) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return Response::err(400, "an option or words"),
    };
    pages.tell_hub(PageMsg::Answer { id: id.to_string(), block: block.to_string(), reply: reply.clone() });
    Response::json(200, &json!({"ok": true, "reply": reply}))
}

/// A version's ui blocks' rules, each scoped to its block (ui.rs); empty for a page without.
fn page_css(pages: &Pages, id: &str, n: u64) -> Response {
    let html = match pages.store.html(id, n) {
        Some(h) => h,
        None if n == 0 && pages.store.meta(id).is_some() => String::new(),
        None => return Response::err(404, "no such version"),
    };
    Response { status: 200, ctype: "text/css; charset=utf-8", body: super::ui::page_css(&html).into_bytes(), csp: CSP }
}

fn kit_file(pages: &Pages, rel: &str) -> Response {
    let ok = !rel.split('/').any(|s| s.is_empty() || s.starts_with('.'))
        && rel.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'/'));
    if !ok {
        return Response::err(404, "no such file");
    }
    let ctype = match rel.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        Some("html") => "text/html; charset=utf-8",
        _ => return Response::err(404, "no such file"),
    };
    match std::fs::read(pages.kit_dir.join(rel)) {
        Ok(body) => Response { status: 200, ctype, body, csp: CSP },
        Err(_) => Response::err(404, "no such file"),
    }
}

fn post_notes(pages: &Pages, id: &str, body: &[u8]) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return Response::err(400, "not JSON") };
    let Some(list) = v.get("notes").and_then(Value::as_array) else { return Response::err(400, "no notes") };
    let now = crate::util::now_ms();
    match pages.locked(|s| s.set_drafts(id, list, m.version(), now)) {
        Ok(notes) => {
            pages.push(id, "notes", &json!({"notes": notes}));
            pages.tell_hub(PageMsg::Notes { id: id.to_string() });
            Response::json(200, &json!({"notes": notes}))
        }
        Err(e) => Response::err(409, &e),
    }
}

fn post_send(pages: &Pages, id: &str) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    let now = crate::util::now_ms();
    let sent = match pages.locked(|s| s.send(id, now)) {
        Ok(s) => s,
        Err(e) => return Response::err(409, &e),
    };
    if sent.is_empty() {
        return Response::json(200, &json!({"sent": 0}));
    }
    let text = super::store::notes_message(&m, &pages.url(id), &sent);
    pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
    pages.tell_hub(PageMsg::Sent { id: id.to_string(), agent: m.agent.clone(), text });
    Response::json(200, &json!({"sent": sent.len(), "agent": m.agent}))
}

/// What a new stream of page `id` gets at once besides its version and
/// state: its items' agents (the last `agent` frame of each) and where
/// its text went (`went`, when it went somewhere).
fn connect_frames(pages: &Pages, id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for f in pages.store.agent_frames(id).values() {
        out.push_str(&format!("event: agent\ndata: {f}\n\n"));
    }
    if let Some(m) = pages.store.meta(id) {
        if !m.went.is_empty() {
            out.push_str(&format!("event: went\ndata: {}\n\n", json!({"went": m.went})));
        }
        if let Some(w) = &m.watch {
            out.push_str(&format!("event: watch\ndata: {w}\n\n"));
        }
        if let Some(t) = &m.taste {
            out.push_str(&format!("event: taste\ndata: {t}\n\n"));
        }
    }
    out
}

/// An SSE stream: the page's version and state now, then every event of
/// it, a ping comment every 15 s; ends when the browser goes.
fn events(w: &mut TcpStream, pages: &Pages, id: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nContent-Security-Policy: {CSP}\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n"
    );
    if w.write_all(head.as_bytes()).is_err() {
        return;
    }
    // idle-exit: an open stream (a page open in a browser, the capsule's
    // frame) keeps the hub up; dropped when the stream ends
    let _hold = pages.holds.hold();
    let (tx, rx) = mpsc::channel::<String>();
    let first = if id.is_empty() {
        format!("event: pages\ndata: {}\n\n", json!({"pages": pages.list_json()}))
    } else {
        match pages.store.meta(id) {
            Some(m) => format!("event: version\ndata: {}\n\nevent: state\ndata: {}\n\n", json!({"n": m.version()}), json!({"state": m.state})),
            None => String::new(),
        }
    };
    // the writer's last progress lines: a reload keeps the list
    let first = first + &pages.progress_frames(id) + &connect_frames(pages, id);
    pages.subscribe(id, tx);
    if w.write_all(first.as_bytes()).and_then(|_| w.flush()).is_err() {
        return;
    }
    loop {
        let frame = match rx.recv_timeout(PING) {
            Ok(f) => f,
            Err(mpsc::RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        if w.write_all(frame.as_bytes()).and_then(|_| w.flush()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::store::Publish;

    fn pages() -> (Pages, std::path::PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("sb-srv-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(d.join("kit")).unwrap();
        std::fs::write(d.join("kit/kit.css"), "main{}").unwrap();
        (Pages::new(&d, 47123, d.join("kit")), d)
    }

    fn get(p: &Pages, path: &str) -> Response {
        let req = Request { method: "GET".into(), path: path.into(), headers: vec![("Host".into(), format!("127.0.0.1:{}", p.port))], body: vec![] };
        route(p, &req)
    }

    fn post(p: &Pages, path: &str, token: Option<&str>, origin: Option<&str>, body: Value) -> Response {
        let mut headers = vec![("Host".into(), format!("127.0.0.1:{}", p.port))];
        if let Some(t) = token {
            headers.push(("X-Bise-Token".into(), t.into()));
        }
        if let Some(o) = origin {
            headers.push(("Origin".into(), o.into()));
        }
        route(p, &Request { method: "POST".into(), path: path.into(), headers, body: body.to_string().into_bytes() })
    }

    fn text(r: &Response) -> String {
        String::from_utf8_lossy(&r.body).to_string()
    }

    const FRAG: &str = "<section data-kit=\"prose\" data-id=\"p1\"><p>hello</p></section>";

    #[test]
    fn parses_a_request() {
        let raw = b"POST /api/p/w/notes?x=1 HTTP/1.1\r\nHost: 127.0.0.1:47123\r\nContent-Length: 2\r\nX-Bise-Token: t\r\n\r\n{}";
        let r = read_request(&mut BufReader::new(&raw[..])).unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str(), r.body.as_slice()), ("POST", "/api/p/w/notes", &b"{}"[..]));
        assert_eq!(r.header("x-bise-token"), Some("t"));
        assert!(read_request(&mut BufReader::new(&b""[..])).is_none());
    }

    #[test]
    fn a_page_opened_in_the_browser_counts_its_newest_version() {
        let (p, _d) = pages();
        let pb = Publish { agent: "t1".into(), id: Some("w".into()), html: FRAG.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        p.publish(&pb, 2, &|_| false).unwrap();
        let opened = |p: &Pages| p.list_json()[0]["opened_version"].as_u64();
        assert_eq!(opened(&p), Some(0));
        // the body alone (the frame's diff) and the API never count
        get(&p, "/p/w/v/2/body");
        get(&p, "/p/w/meta");
        assert_eq!(opened(&p), Some(0));
        get(&p, "/p/w/v/1");
        assert_eq!(opened(&p), Some(1));
        get(&p, "/p/w");
        assert_eq!(opened(&p), Some(2));
        // an older one opened after leaves it
        get(&p, "/p/w/v/1");
        assert_eq!(opened(&p), Some(2));
    }

    /// --public sticks to the page; a public page never holds his drafts; only public pages
    /// are in the export (pages-ui, main m_7501).
    #[test]
    fn only_opted_in_pages_reach_the_export() {
        let (p, _d) = pages();
        let pb = |id: &str, html: &str, public: Option<bool>| Publish { agent: "t1".into(), id: Some(id.into()), html: html.into(), public, ..Default::default() };
        let mail = "<section data-kit=\"email\" data-id=\"m\"><p data-field=\"to\">a@b.c</p><p data-field=\"subject\">s</p><p>secret</p></section>";
        p.publish(&pb("mails", mail, None), 1, &|_| false).unwrap();
        p.publish(&pb("mock", FRAG, Some(true)), 2, &|_| false).unwrap();
        // a draft that leaves his accounts can't go public, now or later
        let e = p.publish(&pb("mails", mail, Some(true)), 3, &|_| false).unwrap_err();
        assert!(e[0].contains("a email block never goes public"), "{e:?}");
        let e = p.publish(&pb("mock", mail, None), 4, &|_| false).unwrap_err();
        assert!(e[0].contains("never goes public"), "public sticks: {e:?}");
        let names: Vec<String> = super::super::mirror::export_now(&p).into_iter().map(|(n, _)| n).collect();
        assert!(names.contains(&"artifacts/mock/index.html".to_string()) && !names.iter().any(|n| n.contains("mails")), "{names:?}");
        // --private takes it back
        p.publish(&pb("mock", FRAG, Some(false)), 5, &|_| false).unwrap();
        assert!(!super::super::mirror::export_now(&p).iter().any(|(n, _)| n.contains("mock")));
    }

    /// A ui block's <style> never reaches the page inline (the CSP is 'self'): the shell links
    /// the version's page.css, where the rules are scoped to their block.
    #[test]
    fn a_ui_blocks_style_is_served_as_the_versions_page_css() {
        let (p, _d) = pages();
        let ui = "<section data-kit=\"ui\" data-id=\"mock\"><style>.row { color: var(--term-dim) }</style><div class=\"row\">x</div></section>";
        p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), html: ui.into(), ..Default::default() }, 1, &|_| false).unwrap();
        let t = text(&get(&p, "/p/w"));
        assert!(t.contains("<link rel=\"stylesheet\" href=\"/p/w/v/1/page.css\" data-page-css>"), "{t}");
        assert!(!t.contains("<style>") && t.contains("<div class=\"row\">x</div>"));
        assert!(!text(&get(&p, "/p/w/v/1/body")).contains("<style>"));
        let css = get(&p, "/p/w/v/1/page.css");
        assert_eq!((css.status, css.ctype), (200, "text/css; charset=utf-8"));
        assert!(text(&css).contains("[data-id=\"mock\"] .row { color: var(--term-dim) }"), "{}", text(&css));
        assert_eq!(get(&p, "/p/w/v/9/page.css").status, 404);
    }

    #[test]
    fn serves_the_page_in_its_shell_with_the_csp() {
        let (p, d) = pages();
        p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), title: Some("weekly <update>".into()), html: FRAG.into(), ..Default::default() }, 1, &|_| false).unwrap();
        let r = get(&p, "/p/w");
        assert_eq!(r.status, 200);
        let t = text(&r);
        assert!(t.contains(&format!("<meta name=\"bise-token\" content=\"{}\">", p.token)));
        assert!(t.contains("<title>weekly &lt;update&gt;</title>"));
        assert!(t.contains("<main id=\"bise-page\" data-page=\"w\" data-version=\"1\" data-latest=\"1\" data-agent=\"t1\""));
        assert!(t.contains(FRAG) && t.contains("/kit/kit.js") && t.contains("/kit/notes.js") && t.contains("/kit/tokens.css"));
        let order: Vec<usize> = ["tokens.css", "kit.css", "notes.css", "kit.js", "pearl.js", "notelines.js", "notes.js"].iter().map(|f| t.find(f).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "the shell's order: tokens, kit, notes css, then kit.js, pearl.js, notelines.js, notes.js");
        assert!(String::from_utf8_lossy(&r.bytes()).contains(&format!("Content-Security-Policy: {CSP}\r\n")));
        assert_eq!(text(&get(&p, "/p/w/v/1/body")), FRAG);
        assert_eq!(get(&p, "/p/w/v/2").status, 404);
        assert_eq!(get(&p, "/p/nope").status, 404);
        let meta: Value = serde_json::from_slice(&get(&p, "/p/w/meta").body).unwrap();
        assert_eq!((meta["id"].as_str(), meta["versions"][0]["n"].as_u64()), (Some("w"), Some(1)));
        assert!(meta["notes"].is_array());
        // for you
        let home = text(&get(&p, "/"));
        assert!(home.contains("<main id=\"bise-home\" data-home>") && home.contains("<a href=\"/p/w\">weekly &lt;update&gt;</a>"), "{home}");
        // the kit, nothing outside it
        assert_eq!(text(&get(&p, "/kit/kit.css")), "main{}");
        assert_eq!(get(&p, "/kit/../meta.json").status, 404);
        assert_eq!(get(&p, "/kit/.hidden.css").status, 404);
        // another Host: refused
        let r = route(&p, &Request { method: "GET".into(), path: "/".into(), headers: vec![("Host".into(), "evil.example:47123".into())], body: vec![] });
        assert_eq!(r.status, 403);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_api_needs_the_token_and_the_origin_then_send_tells_the_hub() {
        let (p, d) = pages();
        p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), html: FRAG.into(), ..Default::default() }, 1, &|_| false).unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        p.connect_hub(Box::new(move |m| {
            let _ = tx.lock().unwrap().send(m);
        }));
        let origin = "http://127.0.0.1:47123";
        let note = json!({"notes": [{"block": "p1", "kind": "note", "text": "shorter"}]});
        let tok = p.token.clone();
        assert_eq!(post(&p, "/api/p/w/notes", None, Some(origin), note.clone()).status, 403);
        assert_eq!(post(&p, "/api/p/w/notes", Some("wrong"), Some(origin), note.clone()).status, 403);
        assert_eq!(post(&p, "/api/p/w/notes", Some(&tok), Some("http://evil.example"), note.clone()).status, 403);
        assert_eq!(post(&p, "/api/p/w/notes", Some(&tok), None, note.clone()).status, 403);
        let r = post(&p, "/api/p/w/notes", Some(&tok), Some(origin), note);
        assert_eq!(r.status, 200, "{}", text(&r));
        assert_eq!(rx.try_recv().unwrap(), PageMsg::Notes { id: "w".into() });
        let r = post(&p, "/api/p/w/send", Some(&tok), Some("http://localhost:47123"), json!({}));
        assert_eq!(r.status, 200);
        match rx.try_recv().unwrap() {
            PageMsg::Sent { id, agent, text } => {
                assert_eq!((id.as_str(), agent.as_str()), ("w", "t1"));
                assert!(text.starts_with("you sent 1 note on your page \"w\" (w v1, http://127.0.0.1:47123/p/w):\n1. on p1: shorter\n"), "{text}");
            }
            m => panic!("{m:?}"),
        }
        // nothing left: nothing told
        assert_eq!(text(&post(&p, "/api/p/w/send", Some(&tok), Some(origin), json!({}))), "{\"sent\":0}");
        assert!(rx.try_recv().is_err());
        assert_eq!(get(&p, "/api/p/w/send").status, 405);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn versions_list_what_changed() {
        let (p, d) = pages();
        let pb = |html: &str| Publish { agent: "t1".into(), id: Some("w".into()), html: html.into(), ..Default::default() };
        let two = "<section data-kit=\"prose\" data-id=\"p1\"><p>a</p></section><section data-kit=\"prose\" data-id=\"p2\"><p>b</p></section>";
        let two_b = "<section data-kit=\"prose\" data-id=\"p1\"><p>a</p></section><section data-kit=\"prose\" data-id=\"p2\"><p>b, changed</p></section>";
        p.publish(&pb(two), 1, &|_| false).unwrap();
        p.publish(&pb(two_b), 2, &|_| false).unwrap();
        let v: Value = serde_json::from_slice(&get(&p, "/p/w/versions").body).unwrap();
        assert_eq!(v["versions"][0], json!({"n": 2, "at_ms": 2, "url": "/p/w/v/2", "changed": ["p2"], "latest": true}));
        assert_eq!(v["versions"][1]["changed"], json!(["p1", "p2"]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn an_answer_on_the_page_goes_to_the_hub() {
        let (p, d) = pages();
        let q = "<section data-kit=\"question\" data-id=\"q1\"><p>which one?</p><ol><li>Le Moulin</li><li>Les Pins</li></ol></section>";
        p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), html: q.into(), ..Default::default() }, 1, &|_| false).unwrap();
        p.store.save_questions("w", &crate::pages::questions::of_fragment(q)).unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        p.connect_hub(Box::new(move |m| {
            let _ = tx.lock().unwrap().send(m);
        }));
        let (tok, o) = (p.token.clone(), "http://127.0.0.1:47123");
        assert_eq!(post(&p, "/api/p/w/answer", None, Some(o), json!({"block": "q1", "option": 2})).status, 403);
        assert_eq!(post(&p, "/api/p/w/answer", Some(&tok), Some(o), json!({"block": "q9", "option": 2})).status, 404);
        assert_eq!(post(&p, "/api/p/w/answer", Some(&tok), Some(o), json!({"block": "q1", "option": 5})).status, 400);
        assert_eq!(post(&p, "/api/p/w/answer", Some(&tok), Some(o), json!({"block": "q1", "option": 2})).status, 200);
        assert_eq!(rx.try_recv().unwrap(), PageMsg::Answer { id: "w".into(), block: "q1".into(), reply: "Les Pins".into() });
        assert_eq!(post(&p, "/api/p/w/answer", Some(&tok), Some(o), json!({"block": "q1", "text": "  neither  "})).status, 200);
        assert_eq!(rx.try_recv().unwrap(), PageMsg::Answer { id: "w".into(), block: "q1".into(), reply: "neither".into() });
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn sse_subscribers_get_the_events_of_their_page() {
        let (p, d) = pages();
        let (tx, rx) = mpsc::channel();
        p.subscribe("w", tx);
        p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), html: FRAG.into(), ..Default::default() }, 1, &|_| false).unwrap();
        let got: Vec<String> = rx.try_iter().collect();
        assert_eq!(got[0], "event: version\ndata: {\"n\":1}\n\n");
        assert!(got.iter().any(|f| f.starts_with("event: state\ndata: {\"state\":\"ready\"}")));
        p.set_state("w", "updating");
        assert_eq!(rx.try_iter().next().unwrap(), "event: state\ndata: {\"state\":\"updating\"}\n\n");
        let _ = std::fs::remove_dir_all(d);
    }

    /// idle-exit (main m_7249): an open event stream holds the hub up,
    /// its end lets go; the other routes never hold.
    #[test]
    fn an_open_event_stream_holds_the_hub() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let d = std::env::temp_dir().join(format!("sb-srv-hold-{}", std::process::id()));
        std::fs::create_dir_all(d.join("kit")).unwrap();
        let holds = crate::idle::Holds::default();
        let p = std::sync::Arc::new(Pages { holds: holds.clone(), ..Pages::new(&d, port, d.join("kit")) });
        serve(listener, p.clone());
        let get = |path: &str| {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes()).unwrap();
            s
        };
        let wait = |n: usize| {
            let t0 = std::time::Instant::now();
            while holds.count() != n {
                assert!(t0.elapsed() < std::time::Duration::from_secs(5), "holds {} != {n}", holds.count());
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        let mut home = get("/events");
        let mut first = [0u8; 12];
        home.read_exact(&mut first).unwrap();
        wait(1);
        let page = get("/p/none/events");
        wait(2);
        // a plain GET never holds
        let mut out = String::new();
        let _ = get("/p/none/meta").read_to_string(&mut out);
        assert_eq!(holds.count(), 2);
        // the browser goes: a stream finds out at its next write (a ping
        // every 15 s, or an event: sent here), and its hold goes
        drop(home);
        drop(page);
        let t0 = std::time::Instant::now();
        while holds.count() != 0 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(5), "holds {} after close", holds.count());
            p.push("", "pages", &json!({"pages": []}));
            p.push("none", "state", &json!({"state": "ready"}));
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = std::fs::remove_dir_all(d);
    }

    /// pm's B (m_5765): a reader polling a page during republishes always
    /// gets a whole answer: 200 and the meta, never a 404, a dropped
    /// connection or half a file, through the real socket and threads.
    #[test]
    fn a_page_is_served_whole_while_it_is_republished() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let d = std::env::temp_dir().join(format!("sb-srv-pub-{}", std::process::id()));
        std::fs::create_dir_all(d.join("kit")).unwrap();
        let p = std::sync::Arc::new(Pages::new(&d, port, d.join("kit")));
        let pb = |n: u64| Publish {
            agent: "watch".into(),
            id: Some("launch-watch".into()),
            html: format!("<section data-kit=\"prose\" data-id=\"p1\"><p>{}</p></section>", "new comment ".repeat(2000 + n as usize)),
            ..Default::default()
        };
        p.publish(&pb(0), 1, &|_| false).unwrap();
        serve(listener, p.clone());
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let done = done.clone();
                std::thread::spawn(move || {
                    let mut bad = Vec::new();
                    let mut n = 0;
                    while !done.load(std::sync::atomic::Ordering::SeqCst) || n < 20 {
                        n += 1;
                        for path in ["/p/launch-watch/meta", "/p/launch-watch/v/1/body", "/p/launch-watch"] {
                            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                            let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
                            s.write_all(req.as_bytes()).unwrap();
                            let mut out = String::new();
                            let _ = s.read_to_string(&mut out);
                            let ok = out.starts_with("HTTP/1.1 200")
                                && out.split("\r\n\r\n").nth(1).is_some_and(|b| {
                                    !path.ends_with("/meta") || serde_json::from_str::<Value>(b).is_ok_and(|v| v["id"] == "launch-watch")
                                });
                            if !ok {
                                bad.push(format!("{path}: {:?}", out.chars().take(80).collect::<String>()));
                            }
                        }
                    }
                    bad
                })
            })
            .collect();
        for n in 2..=21 {
            p.publish(&pb(n), n, &|_| false).unwrap();
        }
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        let bad: Vec<String> = readers.into_iter().flat_map(|r| r.join().unwrap()).collect();
        assert!(bad.is_empty(), "{} bad answers, first: {:?}", bad.len(), bad.first());
        assert_eq!(p.store.meta("launch-watch").unwrap().version(), 21);
        let _ = std::fs::remove_dir_all(d);
    }
}
