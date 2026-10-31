//! The page server (docs/ambient-pages.md §2.4), inside the hub: HTTP/1.1
//! on 127.0.0.1 only, one thread per connection, a small hand parser
//! (no crate). Every response carries the CSP (no inline, no outside);
//! a Host other than the server's own is refused (DNS rebinding), and
//! `/api/*` needs the session token (`X-Bise-Token`, in the shell's
//! `<meta name="bise-token">`) and the server's own `Origin`. A view's
//! own host (`v-<hash>.localhost:<port>`, views.rs, docs/mcp-apps.md C6)
//! is the one other Host it answers: its sandbox proxy, nothing else.

use super::store::{valid_id, Target};
use super::{PageMsg, Pages};
use serde_json::{json, Value};
use std::borrow::Cow;
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
    /// the target's part after `?` (before any `#`), "" none
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The value of `name` in the query (`a=1&s=..`), as sent (no
    /// percent-decoding: the hub's own values are hex).
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v))
    }

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
    let target = target.split('#').next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (path.to_string(), query.to_string());
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
    let mut req = Request { method, path, query, headers, body: Vec::new() };
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
    /// the response's CSP: [`CSP`], a site artifact's sandbox (site.rs),
    /// or a view's (view_csp.rs, built per view)
    pub csp: Cow<'static, str>,
    /// a redirect's `Location` (a local site's door, artifact/live.rs)
    pub location: Option<String>,
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        502 => "Bad Gateway",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

impl Response {
    pub(crate) fn json(status: u16, v: &Value) -> Response {
        Response { status, ctype: "application/json", body: v.to_string().into_bytes(), csp: CSP.into(), location: None }
    }
    fn html(body: String) -> Response {
        Response { status: 200, ctype: "text/html; charset=utf-8", body: body.into_bytes(), csp: CSP.into(), location: None }
    }
    pub(crate) fn err(status: u16, why: &str) -> Response {
        Response::json(status, &json!({"error": why}))
    }
    /// A missing file a page links (/kit, /a/, page.css): 404 text/plain, never
    /// JSON, which a browser reports as a stylesheet of the wrong MIME type
    /// (Benjamin's -31: "MIME type ('application/json')", the kit missing).
    pub(crate) fn missing(why: &str) -> Response {
        Response { status: 404, ctype: "text/plain; charset=utf-8", body: why.as_bytes().to_vec(), csp: CSP.into(), location: None }
    }

    /// A 302 to `to` (a local site's door: its server, or bise's copy).
    pub(crate) fn redirect(to: &str) -> Response {
        Response { status: 302, ctype: "text/plain; charset=utf-8", body: Vec::new(), csp: CSP.into(), location: Some(to.to_string()) }
    }
    /// A page that says why (a local site's door when it could not open it).
    pub(crate) fn page(status: u16, html: String) -> Response {
        Response { status, ctype: "text/html; charset=utf-8", body: html.into_bytes(), csp: CSP.into(), location: None }
    }

    pub fn bytes(&self) -> Vec<u8> {
        // a Location never carries a line break (the header's end)
        let location = self.location.as_deref().filter(|l| !l.contains(['\r', '\n'])).map(|l| format!("Location: {l}\r\n")).unwrap_or_default();
        let mut out = format!(
            "HTTP/1.1 {} {}\r\n{location}Content-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
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
    // a video's copy: its byte ranges, read from disk as they are asked (media.rs)
    if let Some((path, ctype)) = media(pages, &req) {
        // a player that stops reading frees its thread
        let _ = w.set_write_timeout(Some(Duration::from_secs(30)));
        if super::media::send(&mut w, &path, ctype, super::site::ARTIFACT_CSP, req.header("range"), req.method == "HEAD") {
            return;
        }
    }
    let resp = route(pages, &req);
    let _ = w.write_all(&resp.bytes());
}

/// `GET /p/<id>/events` (or `/events` for the home list): the page id;
/// `/a/<id>/events`: `a/<id>` (pages/artifact/events.rs).
fn events_route(req: &Request) -> Option<String> {
    if req.method != "GET" {
        return None;
    }
    if req.path == "/events" {
        return Some(String::new());
    }
    if let Some(k) = super::artifact::events::route(&req.path) {
        return Some(k);
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
    // a view's own origin (views.rs, docs/mcp-apps.md C6): its proxy page only
    if let Some(hash) = req.header("host").and_then(|h| super::views::view_host(h, pages.port)) {
        return super::views::route(pages, req, hash);
    }
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
        // the notes on an artifact (pages/artifact/notes.rs)
        if let Some(r) = super::artifact::post(pages, &segs, &req.body) {
            return r;
        }
        return match segs.as_slice() {
            // a view's request from the web host (view_web.rs, C9)
            ["api", "view"] => super::view_web::post(pages, &req.body),
            ["api", "p", id, "notes"] if valid_id(id) => post_notes(pages, id, &req.body),
            ["api", "p", id, "send"] if valid_id(id) => post_send(pages, id),
            ["api", "p", id, "answer"] if valid_id(id) => post_answer(pages, id, &req.body),
            _ => Response::err(404, "no such route"),
        };
    }
    if req.method != "GET" {
        return Response::err(405, "GET only");
    }
    // the viewer of an office file and its bundles (pages/artifact)
    if let Some(r) = super::artifact::route(pages, &segs) {
        return r;
    }
    match segs.as_slice() {
        [""] => Response::html(home(pages)),
        // a view in his browser (view_web.rs, C9)
        ["v", agent, call] => super::view_web::page(pages, agent, call),
        ["kit", rest @ ..] if !rest.is_empty() => kit_file(pages, &rest.join("/")),
        ["a", id, v, rest @ ..] if valid_id(id) => artifact_file(pages, id, v, rest),
        ["p", id] if valid_id(id) => page(pages, id, None, false),
        ["p", id, "meta"] if valid_id(id) => meta(pages, id),
        ["p", id, "versions"] if valid_id(id) => versions(pages, id),
        ["p", id, "v", n] if valid_id(id) => n.parse().map_or_else(|_| Response::err(404, "no such version"), |n| page(pages, id, Some(n), false)),
        ["p", id, "v", n, "body"] if valid_id(id) => n.parse().map_or_else(|_| Response::err(404, "no such version"), |n| page(pages, id, Some(n), true)),
        ["p", id, "v", n, "page.css"] if valid_id(id) => n.parse().map_or_else(|_| Response::missing(&format!("no such version: p/{id}/v/{n}")), |n| page_css(pages, id, n)),
        _ => Response::err(404, "no such page"),
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// The shell around a fragment (§2.5): the kit, the token, the page's ids.
fn shell(pages: &Pages, title: &str, head: &str, main_attrs: &str, inner: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"bise-token\" content=\"{}\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"/kit/tokens.css\">\n<link rel=\"stylesheet\" href=\"/kit/kit.css\">\n<link rel=\"stylesheet\" href=\"/kit/notes.css\">\n<script src=\"/kit/ask.js\" defer></script>
<script src=\"/kit/kit.js\" defer></script>\n<script src=\"/kit/plan.js\" defer></script>\n<script src=\"/kit/pearl.js\" defer></script>\n<script src=\"/kit/notelines.js\" defer></script>
<script src=\"/kit/notes-anchors.js\" defer></script>
<script src=\"/kit/notes-fold.js\" defer></script>
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

/// The file `/a/<id>/<v>/<rest>` names in bise's copy, its content type and
/// whether it is the viewer's frame of a site (site::framed); Err: the 404
/// in words. One rule for [`artifact_file`] and a video's ranges (media.rs).
fn artifact_target(pages: &Pages, id: &str, v: &str, rest: &[&str]) -> Result<(std::path::PathBuf, &'static str, bool), Response> {
    let state = pages.store.dir.parent().unwrap_or(&pages.store.dir);
    let store = crate::artifacts::Store::new(state);
    // what is missing, by its place under /a/ (never a path on his machine)
    let at = format!("a/{id}/{v}/{}", rest.join("/"));
    let Some(m) = store.get(id).filter(|m| m.by != "page") else { return Err(Response::missing(&format!("no such artifact: {at}"))) };
    let Some(ver) = v.parse::<u64>().ok().and_then(|n| m.versions.iter().find(|x| x.v == n)) else { return Err(Response::missing(&format!("no such version: {at}"))) };
    let Some(copy) = ver.copy.as_ref().filter(|c| super::site::serves_copy(&m.by, c)) else { return Err(Response::missing(&format!("no copy of it: {at}"))) };
    let base = state.join("artifacts").join(&m.id).join(copy);
    // the viewer's frame of a site (and its own links): the selection
    // script in its html (site::framed, architect m_24172)
    let (select, rest) = super::site::framed(rest);
    let rest: Vec<&str> = rest.iter().copied().filter(|s| !s.is_empty()).collect();
    // a folder's file, or a copied file and the files it links beside it
    let Some(path) = super::site::copy_file(&base, &rest) else { return Err(Response::missing(&format!("no such file: {at}"))) };
    let Some(ctype) = super::site::ctype(&path.to_string_lossy()) else { return Err(Response::missing(&format!("not served: {at}"))) };
    Ok((path, ctype, select))
}

/// A video copy (`GET` or `HEAD /a/<id>/<v>/<file>`, a `video/` type),
/// streamed by its byte ranges (media.rs); None: not one, `route` answers it.
fn media(pages: &Pages, req: &Request) -> Option<(std::path::PathBuf, &'static str)> {
    if !matches!(req.method.as_str(), "GET" | "HEAD") || !host_ok(pages, req) {
        return None;
    }
    let segs: Vec<&str> = req.path.trim_start_matches('/').split('/').collect();
    let ["a", id, v, rest @ ..] = segs.as_slice() else { return None };
    if !valid_id(id) {
        return None;
    }
    let (path, ctype, _) = artifact_target(pages, id, v, rest).ok()?;
    ctype.starts_with("video/").then_some((path, ctype))
}

/// An artifact's file, from bise's copy (`/a/<id>/<v>/<path>`), in its
/// sandbox (site.rs): a site folder, or a copied file the server sends
/// (site::serves_copy, the same rule as the typed row's frame_url) and
/// the files an html file links kept beside it (site::copy_file).
fn artifact_file(pages: &Pages, id: &str, v: &str, rest: &[&str]) -> Response {
    let (path, ctype, select) = match artifact_target(pages, id, v, rest) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let at = format!("a/{id}/{v}/{}", rest.join("/"));
    match std::fs::read(&path) {
        Ok(body) if ctype.starts_with("text/html") => {
            // its kept localStorage, every version's (artifacts_storage, web-storage)
            let kept = crate::artifacts_storage::read(&super::artifact::notes::arts(pages), id);
            Response { status: 200, ctype, body: super::site::tagged(&body, select, &kept), csp: super::site::ARTIFACT_CSP.into(), location: None }
        }
        Ok(body) => Response { status: 200, ctype, body, csp: super::site::ARTIFACT_CSP.into(), location: None },
        Err(_) => Response::missing(&format!("no such file: {at}")),
    }
}

fn meta(pages: &Pages, id: &str) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    // flat (docs/ambient-pages.md §2.4): meta.json's fields, the url, the notes
    let mut v = serde_json::to_value(&m).unwrap_or_default();
    v["url"] = json!(pages.url(id));
    v["notes"] = json!(pages.store.notes(id));
    // its questions and picks (notif-artifact): open while their
    // notification waits in the inbox (a closed one has card 0), or
    // answered (where, when): the kit says '? waits in your inbox'
    v["asks"] = json!(pages.store.questions(id).iter().map(|q| q.ask_json(true)).collect::<Vec<_>>());
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
        None => return Response::missing(&format!("no such version: p/{id}/v/{n}/page.css")),
    };
    Response { status: 200, ctype: "text/css; charset=utf-8", body: super::ui::page_css(&html).into_bytes(), csp: CSP.into(), location: None }
}

fn kit_file(pages: &Pages, rel: &str) -> Response {
    let ok = !rel.split('/').any(|s| s.is_empty() || s.starts_with('.'))
        && rel.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'/'));
    if !ok {
        return Response::missing("no such file");
    }
    let ctype = match rel.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        Some("html") => "text/html; charset=utf-8",
        _ => return Response::missing(&format!("not served: kit/{rel}")),
    };
    match std::fs::read(pages.kit_dir.join(rel)) {
        Ok(body) => Response { status: 200, ctype, body, csp: CSP.into(), location: None },
        Err(_) => Response::missing(&format!("no such file: kit/{rel}")),
    }
}

fn post_notes(pages: &Pages, id: &str, body: &[u8]) -> Response {
    let Some(m) = pages.store.meta(id) else { return Response::err(404, "no such page") };
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return Response::err(400, "not JSON") };
    let Some(list) = v.get("notes").and_then(Value::as_array) else { return Response::err(400, "no notes") };
    let now = crate::util::now_ms();
    match pages.locked(|s| s.set_drafts(&Target::Page(id.to_string()), list, m.version(), now)) {
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
    let sent = match pages.locked(|s| s.send(&Target::Page(id.to_string()), now)) {
        Ok(s) => s,
        Err(e) => return Response::err(409, &e),
    };
    if sent.is_empty() {
        return Response::json(200, &json!({"sent": 0}));
    }
    let text = super::store::notes_message(&m, &pages.url(id), &sent);
    pages.push(id, "notes", &json!({"notes": pages.store.notes(id)}));
    let n = sent.len();
    pages.tell_hub(PageMsg::Sent { target: Target::Page(id.to_string()), agent: m.agent.clone(), text, notes: sent });
    Response::json(200, &json!({"sent": n, "agent": m.agent}))
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
    let first = if let Some(art) = id.strip_prefix("a/") {
        super::artifact::events::first(pages, art)
    } else if id.is_empty() {
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
#[path = "server_tests.rs"]
mod tests;
