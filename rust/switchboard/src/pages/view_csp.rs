//! A view's Content-Security-Policy (docs/mcp-apps.md C6), pure: bise's
//! floor plus the origins the view's resource declares in `_meta.ui.csp`
//! (MCP Apps, SEP-1865), each list on its directives. The proxy page
//! (`views.rs`) is served with it as a header, so the view, a `blob:`
//! frame the proxy makes, which inherits its creator's policy, can never
//! do more; the proxy also puts it in the view's `<meta>`. `frame-src`
//! always has `blob:` (the view's own frame; a blob is of its origin and
//! inherits the same policy).
//!
//! Only an origin passes (`scheme://host[:port]`, `*.` on the first label
//! of a host with two labels or more); a path, `*` alone, `data:`, a
//! `'keyword'`, a wildcard port, a loopback host without a port or on the
//! pages server's own port are dropped, never loosened into something
//! wider. `domain` (a stable origin the view asks for) is not read: views
//! get `v-<hash>.localhost`.

use serde_json::Value;

/// At most this many origins per list: a header stays small.
const MAX_PER_LIST: usize = 32;

/// The CSP of a view whose resource declared `declared` (`_meta.ui.csp`,
/// any JSON: a missing or bad one is the floor), served by the pages
/// server on `pages_port`. `dev`: the app's dev server origin, a dev
/// build's only (`BISE_DEV`, [`DEV_ORIGIN`]), the one more who may frame it.
/// `own_files`: the view loads its own folder from its origin
/// (view_folder.rs, docs/plugin-apps.md A5): `'self'` joins script, style,
/// img, font and base-uri (the proxy's `<base>` names `/f/<session>/`);
/// without it the floor is as it was, `base-uri` included. `doc`: a
/// document view's session (`view/open`, A4): its file's url
/// (`/h/<token>`, `resources/url`) is on its own origin, so `'self'`
/// joins img, media and connect, for that view only.
pub fn csp(declared: &Value, pages_port: u16, dev: Option<&str>, own_files: bool, doc: bool) -> String {
    build(declared, pages_port, dev, if own_files { Own::Files } else { Own::Nothing }, doc)
}

/// The CSP of a document's own html page on an isolated view origin
/// (`isolate: "document"`, docs/plugin-apps.md A5, architect m_60421): a
/// site's page served at `/h/<token>/<rel>`, framed by its view. A
/// document view's floor and declared origins with `'self'` on script,
/// style, img, media, font, connect, frame and base-uri (that origin holds
/// only this document and its view's own files), `'unsafe-inline'` scripts
/// and styles as a view's; the view's own `frame-ancestors` list (every
/// ancestor is checked: the view, the proxy, the app or the web host); no
/// `sandbox` (the proxy's frame keeps its own).
pub fn document_page_csp(declared: &Value, pages_port: u16, dev: Option<&str>) -> String {
    build(declared, pages_port, dev, Own::DocumentPage, true)
}

/// What a view's origin serves of its own, beyond the floor.
#[derive(Clone, Copy, PartialEq)]
enum Own {
    /// nothing: an inline view
    Nothing,
    /// its folder's files (view_folder.rs)
    Files,
    /// a document's page with its own scripts and requests (a site)
    DocumentPage,
}

fn build(declared: &Value, pages_port: u16, dev: Option<&str>, own_of: Own, doc: bool) -> String {
    let own_files = own_of != Own::Nothing;
    let page = own_of == Own::DocumentPage;
    let list = |key: &str| origins(declared.get(key), pages_port);
    let connect = list("connectDomains");
    let resource = list("resourceDomains");
    let frame = list("frameDomains");
    let base = list("baseUriDomains");
    let with = |floor: &str, extra: &[String]| {
        let mut d = floor.to_string();
        for o in extra {
            d.push(' ');
            d.push_str(o);
        }
        d
    };
    let or_none = |extra: &[String]| if extra.is_empty() { "'none'".to_string() } else { extra.join(" ") };
    let own = |floor: &str| if own_files { format!("{floor} 'self'") } else { floor.to_string() };
    let base = if own_files { [vec!["'self'".to_string()], base].concat() } else { base };
    let docs = |floor: &str| if doc { format!("{floor} 'self'") } else { floor.to_string() };
    let img = if own_files || doc { format!("{} 'self'", "data: blob:") } else { "data: blob:".to_string() };
    let connect = if doc { [vec!["'self'".to_string()], connect].concat() } else { connect };
    [
        "default-src 'none'".to_string(),
        format!("script-src {}", with(&own("'unsafe-inline'"), &resource)),
        format!("style-src {}", with(&own("'unsafe-inline'"), &resource)),
        format!("img-src {}", with(&img, &resource)),
        format!("media-src {}", with(&docs("data: blob:"), &resource)),
        format!("font-src {}", with(&own("data:"), &resource)),
        format!("connect-src {}", or_none(&connect)),
        // blob: the view's own frame (views.rs): a blob url carries the
        // view's origin, so Electron's permission handler knows who asks
        // (a srcdoc frame asks as `about:srcdoc`); a site's page may frame
        // its own pages
        format!("frame-src {}", with(if page { "blob: 'self'" } else { "blob:" }, &frame)),
        format!("base-uri {}", or_none(&base)),
        "form-action 'none'".to_string(),
        // who may frame the proxy (views.rs): the app (file:), the web host
        // on this pages port exactly (C9), the proxy itself, a dev build's
        // dev server; never another site nor another local server (agents
        // run dev servers all day), which could feed its own html to the
        // view's origin and read its storage (architect m_50106)
        format!(
            "frame-ancestors 'self' file: http://127.0.0.1:{pages_port} http://localhost:{pages_port}{}",
            dev.map(|d| format!(" {d}")).unwrap_or_default()
        ),
    ]
    .join("; ")
}

/// The app's dev server (electron-vite's renderer), an ancestor of views
/// in a dev build only.
pub const DEV_ORIGIN: &str = "http://localhost:5173";

/// The dev ancestor a hub run with `env_dev` (its `BISE_DEV`) allows.
pub fn dev_ancestor(env_dev: Option<&str>) -> Option<&'static str> {
    env_dev.filter(|v| !v.is_empty() && *v != "0").map(|_| DEV_ORIGIN)
}

/// The valid origins of one declared list, normalized, deduplicated, in
/// their order, capped at [`MAX_PER_LIST`].
fn origins(list: Option<&Value>, pages_port: u16) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in list.and_then(Value::as_array).into_iter().flatten() {
        let Some(o) = e.as_str().and_then(|s| origin(s, pages_port)) else { continue };
        if !out.contains(&o) {
            out.push(o);
        }
        if out.len() == MAX_PER_LIST {
            break;
        }
    }
    out
}

/// One declared entry as a CSP source, when it is an origin; else None.
pub fn origin(entry: &str, pages_port: u16) -> Option<String> {
    let e = entry.trim();
    let (scheme, rest) = e.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "https" | "http" | "wss" | "ws") {
        return None;
    }
    // `https://a.com/` is the origin; any other path, a query or a user is not
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() || rest.bytes().any(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'*'))) {
        return None;
    }
    let (host, port) = match rest.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    let port = match port {
        None => None,
        Some(p) if !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) => Some(p.parse::<u16>().ok().filter(|&n| n > 0)?),
        Some(_) => return None,
    };
    let host = host.to_ascii_lowercase();
    let (wild, name) = match host.strip_prefix("*.") {
        Some(n) => (true, n),
        None => (false, host.as_str()),
    };
    let labels: Vec<&str> = name.split('.').collect();
    let label_ok = |l: &&str| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && !l.contains('*');
    if !labels.iter().all(label_ok) || (wild && labels.len() < 2) {
        return None;
    }
    // loopback: only a named port, never the pages server's (the hub's
    // routes, another view's origin), never a wildcard (v-*.localhost)
    let loopback = name == "localhost" || name.ends_with(".localhost") || name.starts_with("127.") || name == "0.0.0.0";
    if loopback && (wild || port.is_none_or(|p| p == pages_port)) {
        return None;
    }
    Some(match port {
        Some(p) => format!("{scheme}://{host}:{p}"),
        None => format!("{scheme}://{host}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PORT: u16 = 47123;

    fn directive<'a>(csp: &'a str, name: &str) -> &'a str {
        csp.split("; ").find(|d| d.split(' ').next() == Some(name)).unwrap_or_else(|| panic!("no {name} in {csp}"))
    }

    #[test]
    fn nothing_declared_is_the_floor() {
        for declared in [json!(null), json!({}), json!("x"), json!({"connectDomains": "https://a.com"})] {
            let c = csp(&declared, PORT, None, false, false);
            assert_eq!(
                c,
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; media-src data: blob:; font-src data:; connect-src 'none'; frame-src blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'self' file: http://127.0.0.1:47123 http://localhost:47123",
                "{declared}"
            );
        }
    }

    /// The spec's example shapes: each list on its directives, nowhere else.
    #[test]
    fn each_list_goes_to_its_directives() {
        let c = csp(
            &json!({
                "connectDomains": ["https://api.open-meteo.com", "wss://live.example.com"],
                "resourceDomains": ["https://*.tile.openstreetmap.org", "https://cdn.jsdelivr.net"],
                "frameDomains": ["https://www.youtube.com"],
                "baseUriDomains": ["https://cdn.example.com"],
            }),
            PORT,
            None,
            false,
            false,
        );
        assert_eq!(directive(&c, "connect-src"), "connect-src https://api.open-meteo.com wss://live.example.com");
        let res = "https://*.tile.openstreetmap.org https://cdn.jsdelivr.net";
        assert_eq!(directive(&c, "script-src"), format!("script-src 'unsafe-inline' {res}"));
        assert_eq!(directive(&c, "style-src"), format!("style-src 'unsafe-inline' {res}"));
        assert_eq!(directive(&c, "img-src"), format!("img-src data: blob: {res}"));
        assert_eq!(directive(&c, "media-src"), format!("media-src data: blob: {res}"));
        assert_eq!(directive(&c, "font-src"), format!("font-src data: {res}"));
        assert_eq!(directive(&c, "frame-src"), "frame-src blob: https://www.youtube.com");
        assert_eq!(directive(&c, "base-uri"), "base-uri https://cdn.example.com");
        assert_eq!(directive(&c, "default-src"), "default-src 'none'");
        assert_eq!(directive(&c, "form-action"), "form-action 'none'");
    }

    /// No entry ever loosens the floor: whatever is declared, every source
    /// in the policy is the floor's or a plain origin.
    #[test]
    fn bad_entries_are_dropped() {
        let bad = [
            "*",
            "https://*",
            "https://*.com",
            "data:",
            "blob:",
            "'unsafe-eval'",
            "'self'",
            "https:",
            "https://a.com/path",
            "https://a.com/?q",
            "https://user@a.com",
            "https://a.com:*",
            "https://a.com:99999",
            "https://a.com:0",
            "javascript://a.com",
            "file:///etc",
            "https://a.com; script-src *",
            "https://a.com https://b.com",
            "https://a..com",
            "https://-a.com",
            "https://a*b.com",
            "http://localhost",
            "http://127.0.0.1",
            &format!("http://localhost:{PORT}"),
            &format!("http://127.0.0.1:{PORT}"),
            "http://*.localhost:3000",
            &format!("http://v-0123456789abcdef.localhost:{PORT}"),
            "",
        ];
        for b in bad {
            assert_eq!(origin(b, PORT), None, "{b:?} passed");
        }
        let all = json!({"connectDomains": bad, "resourceDomains": bad, "frameDomains": bad, "baseUriDomains": bad, "domain": "https://evil.com"});
        assert_eq!(csp(&all, PORT, None, false, false), csp(&json!({}), PORT, None, false, false), "bad entries leave the floor as it is");
        // non-strings in a list: skipped, the rest kept
        let c = csp(&json!({"connectDomains": [1, null, {"a": 1}, "https://ok.example"]}), PORT, None, false, false);
        assert_eq!(directive(&c, "connect-src"), "connect-src https://ok.example");
    }

    /// Who may frame the proxy: the app, this pages port's web host, the
    /// proxy itself; never another local server; a dev server in a dev build only.
    #[test]
    fn only_the_app_and_this_web_host_frame_a_view() {
        let fa = |dev| directive(&csp(&json!({}), PORT, dev, false, false), "frame-ancestors").to_string();
        assert_eq!(fa(None), "frame-ancestors 'self' file: http://127.0.0.1:47123 http://localhost:47123");
        assert!(!fa(None).contains(":*") && !fa(None).contains("5173"));
        assert_eq!(fa(Some(DEV_ORIGIN)), format!("frame-ancestors 'self' file: http://127.0.0.1:47123 http://localhost:47123 {DEV_ORIGIN}"));
        assert_eq!(dev_ancestor(None), None);
        assert_eq!(dev_ancestor(Some("")), None);
        assert_eq!(dev_ancestor(Some("0")), None);
        assert_eq!(dev_ancestor(Some("1")), Some(DEV_ORIGIN));
    }

    /// A view with its own folder (docs/plugin-apps.md A5, architect
    /// m_59927): `'self'` on script, style, img, font and base-uri, and
    /// nothing else moves; without a folder, the floor exactly.
    #[test]
    fn own_files_add_self_where_the_view_loads_them_only() {
        let declared = json!({"resourceDomains": ["https://cdn.example"], "baseUriDomains": ["https://b.example"]});
        let off = csp(&declared, PORT, None, false, false);
        let on = csp(&declared, PORT, None, true, false);
        assert!(!off.contains("'self'") || directive(&off, "frame-ancestors").contains("'self'"));
        assert_eq!(off.matches("'self'").count(), 1, "only frame-ancestors' without a folder: {off}");
        assert_eq!(directive(&csp(&json!({}), PORT, None, false, false), "base-uri"), "base-uri 'none'");
        assert_eq!(directive(&on, "script-src"), "script-src 'unsafe-inline' 'self' https://cdn.example");
        assert_eq!(directive(&on, "style-src"), "style-src 'unsafe-inline' 'self' https://cdn.example");
        assert_eq!(directive(&on, "img-src"), "img-src data: blob: 'self' https://cdn.example");
        assert_eq!(directive(&on, "font-src"), "font-src data: 'self' https://cdn.example");
        assert_eq!(directive(&on, "base-uri"), "base-uri 'self' https://b.example");
        assert_eq!(directive(&csp(&json!({}), PORT, None, true, false), "base-uri"), "base-uri 'self'");
        for d in ["default-src", "media-src", "connect-src", "frame-src", "form-action", "frame-ancestors"] {
            assert_eq!(directive(&on, d), directive(&off, d), "{d} moves with a folder");
        }
    }

    /// A document view (A4/A5, architect m_60434): its file's url is on its
    /// own origin, so `'self'` joins img, media and connect, and nothing
    /// else moves; a view that is no document's keeps the floor.
    #[test]
    fn a_document_view_reads_its_own_origin_for_its_file_only() {
        let declared = json!({"connectDomains": ["https://api.example"], "resourceDomains": ["https://cdn.example"]});
        let off = csp(&declared, PORT, None, false, false);
        let on = csp(&declared, PORT, None, false, true);
        assert_eq!(directive(&on, "img-src"), "img-src data: blob: 'self' https://cdn.example");
        assert_eq!(directive(&on, "media-src"), "media-src data: blob: 'self' https://cdn.example");
        assert_eq!(directive(&on, "connect-src"), "connect-src 'self' https://api.example");
        assert_eq!(directive(&csp(&json!({}), PORT, None, false, true), "connect-src"), "connect-src 'self'");
        for d in ["default-src", "script-src", "style-src", "font-src", "frame-src", "base-uri", "form-action", "frame-ancestors"] {
            assert_eq!(directive(&on, d), directive(&off, d), "{d} moves for a document");
        }
        assert_eq!(directive(&csp(&declared, PORT, None, true, true), "img-src"), "img-src data: blob: 'self' https://cdn.example", "one 'self' with a folder too");
    }

    /// A site's page on its document's origin (A5, architect m_60421):
    /// 'self' where its own files and requests go, the view's
    /// frame-ancestors exactly, never a sandbox; a view's CSP is unchanged.
    #[test]
    fn a_document_page_gets_self_and_the_views_ancestors() {
        let declared = json!({"resourceDomains": ["https://fonts.googleapis.com"], "connectDomains": ["https://api.example"]});
        let page = document_page_csp(&declared, PORT, None);
        assert_eq!(directive(&page, "script-src"), "script-src 'unsafe-inline' 'self' https://fonts.googleapis.com");
        assert_eq!(directive(&page, "style-src"), "style-src 'unsafe-inline' 'self' https://fonts.googleapis.com");
        assert_eq!(directive(&page, "img-src"), "img-src data: blob: 'self' https://fonts.googleapis.com");
        assert_eq!(directive(&page, "media-src"), "media-src data: blob: 'self' https://fonts.googleapis.com");
        assert_eq!(directive(&page, "font-src"), "font-src data: 'self' https://fonts.googleapis.com");
        assert_eq!(directive(&page, "connect-src"), "connect-src 'self' https://api.example");
        assert_eq!(directive(&page, "frame-src"), "frame-src blob: 'self'");
        assert_eq!(directive(&page, "base-uri"), "base-uri 'self'");
        assert_eq!(directive(&page, "form-action"), "form-action 'none'");
        assert_eq!(directive(&page, "default-src"), "default-src 'none'");
        for dev in [None, Some(DEV_ORIGIN)] {
            let view = csp(&declared, PORT, dev, true, true);
            assert_eq!(directive(&document_page_csp(&declared, PORT, dev), "frame-ancestors"), directive(&view, "frame-ancestors"), "every ancestor: the view's list");
        }
        assert!(!page.contains("sandbox"), "the proxy's frame keeps its own: {page}");
        assert_eq!(directive(&document_page_csp(&json!({}), PORT, None), "connect-src"), "connect-src 'self'");
        let view = csp(&declared, PORT, None, true, false);
        for d in ["media-src", "connect-src", "frame-src"] {
            assert!(!directive(&view, d).contains("'self'"), "{d} stays a view's: {view}");
        }
    }

    #[test]
    fn good_entries_are_normalized_once() {
        assert_eq!(origin(" HTTPS://API.Example.com/ ", PORT).as_deref(), Some("https://api.example.com"));
        assert_eq!(origin("https://a.com:8443", PORT).as_deref(), Some("https://a.com:8443"));
        assert_eq!(origin("http://localhost:3000", PORT).as_deref(), Some("http://localhost:3000"), "a local API on its own port");
        let c = csp(&json!({"connectDomains": ["https://a.com", "https://A.com/", "https://a.com"]}), PORT, None, false, false);
        assert_eq!(directive(&c, "connect-src"), "connect-src https://a.com");
        let many: Vec<String> = (0..100).map(|i| format!("https://h{i}.example")).collect();
        let c = csp(&json!({ "connectDomains": many }), PORT, None, false, false);
        assert_eq!(directive(&c, "connect-src").split(' ').count(), 1 + MAX_PER_LIST);
    }
}
