//! A view's own origin on the pages server (docs/mcp-apps.md C6): each
//! view (`plugin`, `server`, `uri`) is served on
//! `http://v-<hash>.localhost:<port>`, `<hash>` = 16 hex of
//! sha256(`plugin\0server\0uri`), the same across calls, never shared by
//! two plugins. On such a host the server answers `GET /mcp-app`, the
//! sandbox proxy page (from the spec's "sandbox proxy"), and nothing
//! else: every other route is 403 there, `/mcp-app` is 404 on the main
//! host. A view with its own folder on disk (a static view, `~views`)
//! also answers `GET /f/<session>/<rel>`, its folder's files
//! (view_folder.rs, docs/plugin-apps.md A5), under the same sessions.
//!
//! The proxy is not a security boundary: the view runs in a `blob:` frame
//! the proxy makes (`allow-same-origin`), so it can run code in the proxy.
//! What bounds it is the origin and the proxy's CSP header ([`view_csp`]),
//! which the blob document inherits. A blob, not the spec's `srcdoc`: its
//! url carries the view's origin, so Electron's permission handler knows
//! which view asks for MIDI or the clipboard (a srcdoc asks as
//! `about:srcdoc`, an origin no rule can check).
//!
//! That CSP comes from the hub, never the request: the hub keeps
//! each origin's [`ViewSandbox`] (registered by `view/resource`, which
//! read the resource's `_meta.ui.csp` and the plugin's grant); an origin
//! the hub never registered gets a 404. So does a load without one of the
//! sessions `view/resource` made for that origin (`/mcp-app?s=<32 hex>`,
//! [`ViewResource::proxy`](bise_proto::view_rpc::ViewResource)): the
//! proxy takes its html from whoever frames it, and `frame-ancestors`
//! lets any `file:` page frame it.
//!
//! [`view_csp`]: super::view_csp

use super::server::{Request, Response};
use super::view_folder::{self, ViewFolder};
use super::Pages;
use bise_proto::view_rules;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// What the hub knows of one view origin: its declared CSP (`_meta.ui.csp`
/// as the resource gave it; [`view_csp::csp`](super::view_csp::csp) keeps
/// only its origins) and its permissions, declared and granted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewSandbox {
    pub csp: Value,
    pub perms: Vec<String>,
    /// its own files on disk (a static view's folder): served at
    /// `/f/<session>/`, `'self'` in its CSP, the proxy's `<base>`
    pub folder: Option<ViewFolder>,
    /// one document's own origin (`isolate: "document"`, A5): its html
    /// at `/h/` runs as a page (view_csp::document_page_csp); any other
    /// origin's `/h/` html is data, sandboxed
    pub isolated: bool,
}

/// At most this many proxy sessions per view origin: an older tab of
/// that view past them loads it again through `view/resource`. Above the
/// desktop's live views (apps/desktop/src/views/place.ts `MAX_LIVE`, 8)
/// twice over: a live tab holds two sessions at most (a reload while the
/// old one drains), so a live tab of a document app never loses its own
/// (architect m_60156).
pub const SESSIONS: usize = 16;
/// At most this many view origins kept (the least recently loaded go
/// first; a tab of one loads it again through `view/resource`).
pub const ORIGINS: usize = 512;

/// One view origin as the pages server keeps it: its sandbox and the
/// sessions `view/resource` made for it, newest last.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ViewOrigin {
    pub(super) sandbox: ViewSandbox,
    pub(super) sessions: Vec<String>,
    /// its last `view/resource` (a count, the newest highest)
    pub(super) seq: u64,
}

/// Drop the least recently loaded origins past [`ORIGINS`]: their
/// sessions.
fn bound(m: &mut std::collections::HashMap<String, ViewOrigin>) -> Vec<String> {
    if m.len() <= ORIGINS {
        return Vec::new();
    }
    let mut seqs: Vec<u64> = m.values().map(|o| o.seq).collect();
    seqs.sort_unstable();
    let cut = seqs[m.len() - ORIGINS];
    let mut dropped = Vec::new();
    m.retain(|_, o| {
        let keep = o.seq >= cut;
        if !keep {
            dropped.append(&mut o.sessions);
        }
        keep
    });
    dropped
}

impl ViewOrigin {
    /// `session` kept, the oldest past [`SESSIONS`] dropped: them.
    fn add(&mut self, session: String) -> Vec<String> {
        self.sessions.push(session);
        let over = self.sessions.len().saturating_sub(SESSIONS);
        self.sessions.drain(..over).collect()
    }
}

/// The proxy's URL of a view origin for one session.
pub fn proxy_url(origin: &str, session: &str) -> String {
    format!("{origin}/mcp-app?s={session}")
}

/// The 16 hex of a view's origin. A cryptographic hash, never a fast one
/// (FNV...): with a 64-bit non-cryptographic hash a hostile plugin could
/// craft a `uri` that lands on another plugin's origin and read its
/// storage (architect m_50106).
pub fn hash(plugin: &str, server: &str, uri: &str) -> String {
    let mut h = Sha256::new();
    h.update(plugin.as_bytes());
    h.update([0]);
    h.update(server.as_bytes());
    h.update([0]);
    h.update(uri.as_bytes());
    h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// A document's key in its view's origin (docs/plugin-apps.md A5,
/// `isolate: "document"`): an artifact by its project and id, a `~/bise`
/// entry by its id; never a version (a site's versions share one storage).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocKey {
    Artifact { project: String, id: String },
    Entry(String),
}

impl DocKey {
    fn text(&self) -> String {
        match self {
            DocKey::Artifact { project, id } => format!("artifact:{project}/{id}"),
            DocKey::Entry(id) => format!("entry:{id}"),
        }
    }
}

/// The one hash input of a view's origin (A5, architect m_60421): the
/// view, and the document for a view that isolates its documents; none:
/// [`hash`] as every view had it.
pub fn origin_key(plugin: &str, server: &str, uri: &str, doc: Option<&DocKey>) -> String {
    match doc {
        None => hash(plugin, server, uri),
        Some(d) => {
            let mut h = Sha256::new();
            for part in [plugin, server, uri, &d.text()] {
                h.update(part.as_bytes());
                h.update([0]);
            }
            h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect()
        }
    }
}

/// A view's origin on the pages server of `port`.
pub fn origin(plugin: &str, server: &str, uri: &str, port: u16) -> String {
    view_rules::origin_of(&hash(plugin, server, uri), port)
}

/// The hash a `Host` header names when it is a view host of this port
/// ([`view_rules::view_host`]: `v-<16 hex>.localhost:<port>` exactly).
pub fn view_host(host: &str, port: u16) -> Option<&str> {
    view_rules::view_host(host).filter(|(_, p)| *p == port).map(|(h, _)| h)
}

/// Where a document's file is served by its url (`resources/url`, A4):
/// `/h/<token>` on its session's view origin, and a folder document's
/// files under it, `/h/<token>/<rel>` (a site, A5).
pub const DOC_FILE_PREFIX: &str = "/h/";

/// A document file's own CSP: it is data the view fetches or plays,
/// never a page (opened alone, nothing in it runs).
pub const DOC_FILE_CSP: &str = "default-src 'none'; sandbox";

/// The token of a `/h/<token>[/<rel>]` path (`pages::new_token`'s 32
/// lowercase hex) and the rest after it (`""` for the document itself,
/// a folder document's file else: view_files_disk::in_folder checks it),
/// else None. A query is not part of the path.
pub fn doc_file_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.split(['?', '#']).next()?.strip_prefix(DOC_FILE_PREFIX)?;
    let (t, rel) = rest.split_once('/').unwrap_or((rest, ""));
    (t.len() == 32 && t.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then_some((t, rel))
}


impl Pages {
    /// `view/resource` registers (or updates) the sandbox of view origin
    /// `hash`, with a new session for its proxy (32 hex from the system's
    /// random source, never written to a log): the session.
    pub fn view_sandbox(&self, hash: &str, s: ViewSandbox) -> String {
        let session = super::new_token();
        self.view_sandbox_as(hash, &session, s);
        session
    }

    /// [`Pages::view_sandbox`] with a session minted before (`view/open`
    /// binds its handle to it before its opener runs). The sessions the
    /// caps push out end here ([`Pages::sessions_end`]).
    pub fn view_sandbox_as(&self, hash: &str, session: &str, s: ViewSandbox) {
        let mut dropped = Vec::new();
        if let Ok(mut m) = self.views.lock() {
            let seq = m.values().map(|o| o.seq).max().unwrap_or(0) + 1;
            let o = m.entry(hash.to_string()).or_default();
            o.sandbox = s;
            o.seq = seq;
            dropped = o.add(session.to_string());
            dropped.extend(bound(&mut m));
        }
        self.sessions_end(&dropped);
    }

    /// A session ends before the caps push it out (a document's tab
    /// closed, `view/close`): its proxy, its `/f/` and its handles answer
    /// 404 or `bad-handle` from now.
    pub fn view_session_end(&self, session: &str) {
        if let Ok(mut m) = self.views.lock() {
            for o in m.values_mut() {
                o.sessions.retain(|s| s != session);
            }
        }
        self.sessions_end(&[session.to_string()]);
    }

    /// The one place sessions end (the per-origin cap, the [`ORIGINS`]
    /// bound, `view/close`): their handles and their URLs with them
    /// (architect m_60434). Called with no other lock held.
    fn sessions_end(&self, sessions: &[String]) {
        if sessions.is_empty() {
            return;
        }
        let mut f = self.files.lock().unwrap_or_else(|e| e.into_inner());
        sessions.iter().for_each(|s| f.end(s));
    }

    /// A handle for `b` (a new 32 hex), on the table its session's end
    /// clears: its `view-file://` uri.
    pub fn mint_handle(&self, b: crate::view_files::Binding) -> String {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).mint(&super::new_token(), b)
    }

    /// The view a document session (`doc`) was opened with: its opener,
    /// copied out (the lock is never held across I/O).
    pub fn doc_opener(&self, doc: &str) -> Option<crate::view_files::Opener> {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).of_session(doc).map(|b| b.opener.clone())
    }

    /// The file a document session (`doc`) opened, copied out (the lock is
    /// never held across I/O): what its notes are read on (A5).
    pub fn doc_target(&self, doc: &str) -> Option<crate::view_files::FileTarget> {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).of_session(doc).map(|b| b.target.clone())
    }

    /// bise's view kit (`kit/view/`) as a view's folder: what
    /// `/f/<session>/~kit/view/<file>` serves (view_folder.rs).
    fn view_kit(&self) -> ViewFolder {
        let dir = self.kit_dir.join("view");
        let dir = dir.canonicalize().unwrap_or(dir);
        ViewFolder { tag: view_folder::tag(&dir, Some(env!("CARGO_PKG_VERSION"))), dir }
    }

    /// bise's DOM-free office reader (`office/dist/`) as a view's folder:
    /// `/f/<session>/~kit/office/v1/<file>`, its allow-listed files only
    /// (architect m_60402).
    fn office_kit(&self) -> ViewFolder {
        let dir = self.office_dir.canonicalize().unwrap_or(self.office_dir.clone());
        ViewFolder { tag: view_folder::tag(&dir, Some(env!("CARGO_PKG_VERSION"))), dir }
    }

    /// The sandbox of view origin `hash` for a proxy load with `session`:
    /// one `view/resource` made it (among the last [`SESSIONS`]), else none.
    pub(super) fn sandbox_of(&self, hash: &str, session: Option<&str>) -> Option<ViewSandbox> {
        let m = self.views.lock().ok()?;
        let o = m.get(hash)?;
        let s = session.filter(|s| !s.is_empty())?;
        o.sessions.iter().any(|k| k == s).then(|| o.sandbox.clone())
    }
}

/// Every request on view host `hash`: the proxy page, its own files, or 403.
pub fn route(pages: &Pages, req: &Request, hash: &str) -> Response {
    let files = view_folder::split(&req.path);
    if req.path != "/mcp-app" && files.is_none() {
        return Response::err(403, "a view's origin serves its proxy and its own files only");
    }
    if req.method != "GET" {
        return Response::err(405, "GET only");
    }
    if let Some((session, rel)) = files {
        return own_file(pages, req, hash, session, &rel);
    }
    // a session only view/resource gives (C6): a page that never asked
    // the hub (a .html file opened in his browser, which file: lets frame
    // the proxy) can't load the view's origin with its own html
    let Some(session) = req.param("s") else { return Response::missing("no such view") };
    let Some(s) = pages.sandbox_of(hash, Some(session)) else { return Response::missing("no such view") };
    let csp = super::view_csp::csp(&s.csp, pages.port, pages.view_dev, s.folder.is_some(), pages.doc_of(session).is_some());
    // the base is the hub's, written now: never from the view's html
    let base = s.folder.as_ref().map(|_| format!("{}{}{session}/", view_rules::origin_of(hash, pages.port), view_folder::PREFIX)).unwrap_or_default();
    let html = proxy_page(&csp, &view_rules::allow(&s.perms), &base);
    let mut r = Response::page(200, html);
    r.csp = csp.into();
    r
}

/// `GET /f/<session>/<rel>` on view host `hash`: a file of its folder, or
/// of bise's view kit under `~kit/view/` (docs/plugin-apps.md A5);
/// another origin's session, a view with no folder: 404, as `/mcp-app`.
fn own_file(pages: &Pages, req: &Request, hash: &str, session: &str, rel: &[&str]) -> Response {
    let Some(s) = pages.sandbox_of(hash, Some(session)) else { return Response::missing("no such file") };
    let Some(folder) = s.folder.as_ref() else { return Response::missing("no such file") };
    let csp = super::view_csp::csp(&s.csp, pages.port, pages.view_dev, true, pages.doc_of(session).is_some());
    let inm = req.header("if-none-match");
    match rel {
        [view_folder::KIT, "view", file] => view_folder::serve(&pages.view_kit(), &[file], inm, csp),
        [view_folder::KIT, "office", view_folder::OFFICE_V, file] if view_folder::OFFICE_FILES.contains(file) => view_folder::serve(&pages.office_kit(), &[file], inm, csp),
        [view_folder::KIT, ..] => Response::missing("no such file"),
        _ => view_folder::serve(folder, rel, inm, csp),
    }
}

/// JSON for an inline script: `<` never closes it.
fn script_json(s: &str) -> String {
    Value::from(s).to_string().replace('<', "\\u003c")
}

/// The sandbox proxy (the spec's "sandbox proxy", written from it): it
/// runs only framed cross-origin, says `ui/notifications/sandbox-proxy-ready`,
/// takes `ui/notifications/sandbox-resource-ready {html}` from its parent
/// only, once, builds the view's blob frame (the hub's CSP in a meta,
/// `allow=` the granted permissions) and relays JSON-RPC both ways, each
/// way checking `event.source`.
fn proxy_page(csp: &str, allow: &str, base: &str) -> String {
    PROXY.replace("__CSP__", &script_json(csp)).replace("__ALLOW__", &script_json(allow)).replace("__BASE__", &script_json(base))
}

const PROXY: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>view</title>
<style>html,body{margin:0;height:100%;overflow:hidden;background:transparent}iframe{border:0;width:100%;height:100%;display:block}</style>
</head>
<body>
<script>
(() => {
  "use strict";
  const CSP = __CSP__;
  const ALLOW = __ALLOW__;
  const BASE = __BASE__;
  const host = window.parent;
  if (host === window) return;
  let cross = false;
  try { void host.location.href; } catch { cross = true; }
  if (!cross) return;
  let inner = null;
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
  function build(params) {
    const html = params && typeof params.html === "string" ? params.html : "";
    const f = document.createElement("iframe");
    f.setAttribute("sandbox", "allow-scripts allow-same-origin allow-forms");
    if (ALLOW) f.setAttribute("allow", ALLOW);
    f.setAttribute("title", "view");
    // the meta goes after the view's own leading doctype: before it, the
    // document is in quirks mode (tables at 16 px, box sizing); nothing of
    // the view but that doctype comes before the meta
    // a view with its own folder: its relative urls resolve at its
    // /f/<session>/ (a blob has no base of its own), set before any of
    // its markup
    const meta = '<meta http-equiv="Content-Security-Policy" content="' + esc(CSP) + '">' + (BASE ? '<base href="' + esc(BASE) + '">' : "");
    const dt = /^\uFEFF?\s*<!doctype[^>]*>/i.exec(html);
    const doc = dt ? dt[0] + meta + html.slice(dt[0].length) : meta + html;
    f.src = URL.createObjectURL(new Blob([doc], { type: "text/html" }));
    document.body.appendChild(f);
    inner = f;
  }
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (e.source === host) {
      if (m && m.method === "ui/notifications/sandbox-resource-ready") {
        if (inner === null) build(m.params);
        return;
      }
      if (inner !== null && inner.contentWindow) inner.contentWindow.postMessage(m, "*");
      return;
    }
    if (inner !== null && e.source === inner.contentWindow) host.postMessage(m, "*");
  });
  host.postMessage({ jsonrpc: "2.0", method: "ui/notifications/sandbox-proxy-ready", params: {} }, "*");
})();
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_stable_origin_per_view_never_shared() {
        let a = hash("fake-app", "fake-app", "ui://fake-app/counter");
        assert_eq!(a.len(), 16);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert_eq!(a, hash("fake-app", "fake-app", "ui://fake-app/counter"), "the same across calls");
        assert_ne!(a, hash("other", "fake-app", "ui://fake-app/counter"), "another plugin, another origin");
        assert_ne!(a, hash("fake-app", "fake-app", "ui://fake-app/hostile"));
        // the separator: no two splits of the same bytes meet
        assert_ne!(hash("ab", "c", "u"), hash("a", "bc", "u"));
        assert_eq!(origin("fake-app", "fake-app", "ui://fake-app/counter", 47123), format!("http://v-{a}.localhost:47123"));
    }

    /// The shared origins (tests/fixtures/mcp-apps/origins.json, the
    /// desktop's viewrules.test.ts reads them too): a view origin is a view
    /// host at its own port, and nothing else is (mcp-apps review d2).
    #[test]
    fn a_view_host_is_what_the_shared_origins_say() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/mcp-apps/origins.json");
        let fx: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for c in fx["origins"].as_array().unwrap() {
            let o = c["origin"].as_str().unwrap();
            let host = o.strip_prefix("http://");
            let port = host.and_then(|h| h.rsplit_once(':')).and_then(|(_, p)| p.parse::<u16>().ok());
            let got = host.zip(port).and_then(|(h, p)| view_host(h, p)).is_some();
            assert_eq!(got, c["view"].as_bool().unwrap(), "{o}");
        }
    }

    /// isolate: "document": one origin per document, never per version,
    /// and the same artifact id in two projects is two origins (m_60439).
    #[test]
    fn a_document_gets_its_own_origin_when_its_view_isolates() {
        let v = ("sites", "~views", "ui://sites/views/site");
        let a = |p: &str, id: &str| DocKey::Artifact { project: p.into(), id: id.into() };
        assert_eq!(origin_key(v.0, v.1, v.2, None), hash(v.0, v.1, v.2), "a view's own origin as before");
        let one = origin_key(v.0, v.1, v.2, Some(&a("acme", "site")));
        assert_ne!(one, hash(v.0, v.1, v.2));
        assert_eq!(one, origin_key(v.0, v.1, v.2, Some(&a("acme", "site"))), "stable across opens and versions");
        assert_ne!(one, origin_key(v.0, v.1, v.2, Some(&a("other", "site"))), "another project");
        assert_ne!(one, origin_key(v.0, v.1, v.2, Some(&a("acme", "site2"))));
        assert_ne!(one, origin_key(v.0, v.1, v.2, Some(&DocKey::Entry("site".into()))));
        assert_eq!(one.len(), 16);
    }

    #[test]
    fn a_view_host_is_exactly_its_shape() {
        let h = "0123456789abcdef";
        assert_eq!(view_host(&format!("v-{h}.localhost:47123"), 47123), Some(h));
        for bad in [
            format!("v-{h}.localhost:47124"),
            format!("v-{h}.localhost"),
            format!("v-{h}.localhost:47123.evil.com"),
            format!("v-{h}0.localhost:47123"),
            "v-0123456789abcde.localhost:47123".to_string(),
            "v-0123456789ABCDEF.localhost:47123".to_string(),
            "v-0123456789abcdeg.localhost:47123".to_string(),
            format!("x.v-{h}.localhost:47123"),
            format!("w-{h}.localhost:47123"),
            format!("v-{h}.127.0.0.1:47123"),
            "localhost:47123".to_string(),
            "v-ééééééééééééééé.localhost:47123".to_string(),
        ] {
            assert_eq!(view_host(&bad, 47123), None, "{bad}");
        }
    }

    #[test]
    fn an_origin_keeps_its_last_sessions_only() {
        let mut o = ViewOrigin::default();
        let mut dropped = Vec::new();
        for i in 0..SESSIONS + 3 {
            dropped.extend(o.add(format!("s{i}")));
        }
        assert_eq!(dropped, ["s0", "s1", "s2"], "the ones pushed out are said");
        assert_eq!(o.sessions.len(), SESSIONS);
        assert_eq!(o.sessions.first().map(String::as_str), Some("s3"), "the oldest went first");
        assert_eq!(o.sessions.last().map(String::as_str), Some(format!("s{}", SESSIONS + 2).as_str()));
        assert_eq!(proxy_url("http://v-0123456789abcdef.localhost:47123", "ab"), "http://v-0123456789abcdef.localhost:47123/mcp-app?s=ab");
    }

    #[test]
    fn the_view_origins_kept_are_bounded_newest_first() {
        let mut m = std::collections::HashMap::new();
        for i in 0..ORIGINS as u64 + 5 {
            m.insert(format!("{i:016x}"), ViewOrigin { seq: i + 1, sessions: vec![format!("s{i}")], ..Default::default() });
            let dropped = bound(&mut m);
            assert_eq!(dropped, if i < ORIGINS as u64 { vec![] } else { vec![format!("s{}", i - ORIGINS as u64)] }, "an origin's sessions go with it");
        }
        assert_eq!(m.len(), ORIGINS);
        assert!(!m.contains_key(&format!("{:016x}", 4)), "the least recently loaded went");
        assert!(m.contains_key(&format!("{:016x}", 5)) && m.contains_key(&format!("{:016x}", ORIGINS + 4)));
    }

    #[test]
    fn the_proxy_page_carries_the_hubs_policy_safely() {
        let html = proxy_page("default-src 'none'</script><script>alert(1)", "camera", "http://x/</script><script>alert(2)");
        assert!(!html.contains("</script><script>alert(2)"), "nor does a base");
        assert!(!html.contains("</script><script>alert"), "a policy never closes the script");
        assert!(html.contains(r#"const ALLOW = "camera";"#));
        assert!(html.contains("sandbox-proxy-ready") && html.contains("allow-scripts allow-same-origin allow-forms"));
        assert!(!html.contains("allow-popups") && !html.contains("allow-top-navigation"));
    }
}
