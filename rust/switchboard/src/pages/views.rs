//! A view's own origin on the pages server (docs/mcp-apps.md C6): each
//! view (`plugin`, `server`, `uri`) is served on
//! `http://v-<hash>.localhost:<port>`, `<hash>` = 16 hex of
//! sha256(`plugin\0server\0uri`), the same across calls, never shared by
//! two plugins. On such a host the server answers `GET /mcp-app`, the
//! sandbox proxy page (from the spec's "sandbox proxy"), and nothing
//! else: every other route is 403 there, `/mcp-app` is 404 on the main
//! host.
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
}

/// At most this many proxy sessions per view origin: an older tab of
/// that view past them loads it again through `view/resource`.
pub const SESSIONS: usize = 8;
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

/// Drop the least recently loaded origins past [`ORIGINS`].
fn bound(m: &mut std::collections::HashMap<String, ViewOrigin>) {
    if m.len() <= ORIGINS {
        return;
    }
    let mut seqs: Vec<u64> = m.values().map(|o| o.seq).collect();
    seqs.sort_unstable();
    let cut = seqs[m.len() - ORIGINS];
    m.retain(|_, o| o.seq >= cut);
}

impl ViewOrigin {
    /// `session` kept, the oldest past [`SESSIONS`] dropped.
    fn add(&mut self, session: String) {
        self.sessions.push(session);
        let over = self.sessions.len().saturating_sub(SESSIONS);
        self.sessions.drain(..over);
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

/// A view's origin on the pages server of `port`.
pub fn origin(plugin: &str, server: &str, uri: &str, port: u16) -> String {
    view_rules::origin_of(&hash(plugin, server, uri), port)
}

/// The hash a `Host` header names when it is a view host of this port
/// ([`view_rules::view_host`]: `v-<16 hex>.localhost:<port>` exactly).
pub fn view_host(host: &str, port: u16) -> Option<&str> {
    view_rules::view_host(host).filter(|(_, p)| *p == port).map(|(h, _)| h)
}


impl Pages {
    /// `view/resource` registers (or updates) the sandbox of view origin
    /// `hash`, with a new session for its proxy (32 hex from the system's
    /// random source, never written to a log): the session.
    pub fn view_sandbox(&self, hash: &str, s: ViewSandbox) -> String {
        let session = super::new_token();
        if let Ok(mut m) = self.views.lock() {
            let seq = m.values().map(|o| o.seq).max().unwrap_or(0) + 1;
            let o = m.entry(hash.to_string()).or_default();
            o.sandbox = s;
            o.seq = seq;
            o.add(session.clone());
            bound(&mut m);
        }
        session
    }

    /// The sandbox of view origin `hash` for a proxy load with `session`:
    /// one `view/resource` made it (among the last [`SESSIONS`]), else none.
    fn sandbox_of(&self, hash: &str, session: Option<&str>) -> Option<ViewSandbox> {
        let m = self.views.lock().ok()?;
        let o = m.get(hash)?;
        let s = session.filter(|s| !s.is_empty())?;
        o.sessions.iter().any(|k| k == s).then(|| o.sandbox.clone())
    }
}

/// Every request on view host `hash`: the proxy page, or 403.
pub fn route(pages: &Pages, req: &Request, hash: &str) -> Response {
    if req.path != "/mcp-app" {
        return Response::err(403, "a view's origin serves its proxy only");
    }
    if req.method != "GET" {
        return Response::err(405, "GET only");
    }
    // a session only view/resource gives (C6): a page that never asked
    // the hub (a .html file opened in his browser, which file: lets frame
    // the proxy) can't load the view's origin with its own html
    let Some(s) = pages.sandbox_of(hash, req.param("s")) else { return Response::missing("no such view") };
    let csp = super::view_csp::csp(&s.csp, pages.port, pages.view_dev);
    let html = proxy_page(&csp, &view_rules::allow(&s.perms));
    let mut r = Response::page(200, html);
    r.csp = csp.into();
    r
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
fn proxy_page(csp: &str, allow: &str) -> String {
    PROXY.replace("__CSP__", &script_json(csp)).replace("__ALLOW__", &script_json(allow))
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
    const meta = '<meta http-equiv="Content-Security-Policy" content="' + esc(CSP) + '">';
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
        for i in 0..SESSIONS + 3 {
            o.add(format!("s{i}"));
        }
        assert_eq!(o.sessions.len(), SESSIONS);
        assert_eq!(o.sessions.first().map(String::as_str), Some("s3"), "the oldest went first");
        assert_eq!(o.sessions.last().map(String::as_str), Some("s10"));
        assert_eq!(proxy_url("http://v-0123456789abcdef.localhost:47123", "ab"), "http://v-0123456789abcdef.localhost:47123/mcp-app?s=ab");
    }

    #[test]
    fn the_view_origins_kept_are_bounded_newest_first() {
        let mut m = std::collections::HashMap::new();
        for i in 0..ORIGINS as u64 + 5 {
            m.insert(format!("{i:016x}"), ViewOrigin { seq: i + 1, ..Default::default() });
            bound(&mut m);
        }
        assert_eq!(m.len(), ORIGINS);
        assert!(!m.contains_key(&format!("{:016x}", 4)), "the least recently loaded went");
        assert!(m.contains_key(&format!("{:016x}", 5)) && m.contains_key(&format!("{:016x}", ORIGINS + 4)));
    }

    #[test]
    fn the_proxy_page_carries_the_hubs_policy_safely() {
        let html = proxy_page("default-src 'none'</script><script>alert(1)", "camera");
        assert!(!html.contains("</script><script>alert"), "a policy never closes the script");
        assert!(html.contains(r#"const ALLOW = "camera";"#));
        assert!(html.contains("sandbox-proxy-ready") && html.contains("allow-scripts allow-same-origin allow-forms"));
        assert!(!html.contains("allow-popups") && !html.contains("allow-top-navigation"));
    }
}
