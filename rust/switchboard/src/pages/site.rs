//! The page server's `/`: one site for everything agents made to look at
//! (pages-ui, main m_7432). A sidebar lists every page and every site
//! artifact (`sb artifact add` of kind `site`), newest first, by day or by
//! agent, with a search; a page opens inside the site, next to the list
//! (kit/site.js). Below it, the "for you" list stays as it was.
//!
//! A local site artifact (an HTML file or folder bise keeps a copy of) is
//! served from that copy under `/a/<id>/<v>/…` in a sandboxed frame: its
//! own CSP ([`ARTIFACT_CSP`]) lets its inline styles and scripts run in an
//! opaque origin, with no token and no way to the page API. A site on
//! another origin (a dev server, a deploy) opens in a tab.
//!
//! Pure: no I/O (the server reads the stores and the files).

use super::server::esc;
use super::store::Meta as PageMeta;
use crate::artifacts::{is_link, Meta as ArtMeta};

/// The CSP of a site artifact's files: sandboxed (an opaque origin, no
/// token, no `/api`), its inline styles and scripts allowed, fonts from
/// Google Fonts (designer's mocks), no remote scripts, no requests out.
pub const ARTIFACT_CSP: &str = "sandbox allow-scripts allow-popups; default-src 'self' data: blob:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; script-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'none'; frame-ancestors 'self'";

/// One row of the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub title: String,
    pub agent: String,
    /// `page` or `site`
    pub kind: &'static str,
    pub at_ms: u64,
    /// `v3`, or empty
    pub version: String,
    /// where it opens: `/p/<id>`, `/a/<id>/<v>/<file>`, or an outside link
    pub href: String,
    /// true: it opens in a tab (another origin), not inside the site
    pub outside: bool,
}

/// The sidebar's rows: the pages, then the site artifacts that are not
/// pages, newest first.
pub fn items(pages: &[PageMeta], arts: &[ArtMeta]) -> Vec<Item> {
    let mut out: Vec<Item> = pages
        .iter()
        .filter(|m| m.version() > 0)
        .map(|m| Item {
            title: m.title.clone(),
            agent: m.agent.clone(),
            kind: "page",
            at_ms: m.at_ms(),
            version: format!("v{}", m.version()),
            href: format!("/p/{}", m.id),
            outside: false,
        })
        .collect();
    for a in arts.iter().filter(|a| a.kind == "site" && a.by != "page") {
        let Some(cur) = a.current() else { continue };
        let (href, outside) = if is_link(&cur.target) {
            (cur.target.clone(), true)
        } else if let Some(copy) = &cur.copy {
            (format!("/a/{}/{}/{}", a.id, cur.v, file_of(copy)), false)
        } else {
            continue;
        };
        out.push(Item {
            title: a.title.clone(),
            agent: a.agent.clone(),
            kind: "site",
            at_ms: cur.at_ms,
            version: format!("v{}", cur.v),
            href,
            outside,
        });
    }
    out.sort_by(|a, b| b.at_ms.cmp(&a.at_ms).then(a.title.cmp(&b.title)));
    out
}

/// The file a copy opens on: a copied file's name (`v2/plan.html` →
/// `plan.html`), nothing for a folder (its index.html).
fn file_of(copy: &str) -> String {
    let name = copy.rsplit('/').next().unwrap_or("");
    if name.contains('.') {
        name.to_string()
    } else {
        String::new()
    }
}

/// The sidebar: a search, the by day / by agent switch, the rows (site.js
/// groups and filters them from their data- attributes; without it, the
/// plain list reads and its links open).
/// `home`: where "for you" goes (`/` on the hub, `index.html` in the export).
pub fn sidebar(items: &[Item], home: &str) -> String {
    let rows: Vec<String> = items
        .iter()
        .map(|i| {
            let target = if i.outside { " target=\"_blank\" rel=\"noopener\"" } else { "" };
            format!(
                "<li data-kind=\"{}\" data-agent=\"{}\" data-at=\"{}\"{}><a href=\"{}\"{target}><span class=\"t\">{}</span><span class=\"v\">{}</span></a></li>",
                i.kind,
                esc(&i.agent),
                i.at_ms,
                if i.outside { " data-outside" } else { "" },
                esc(&i.href),
                esc(&i.title),
                esc(&i.version)
            )
        })
        .collect();
    format!(
        "<aside id=\"bise-site\" aria-label=\"everything agents made\">\n<a class=\"home\" href=\"{}\">for you</a>\n<input type=\"search\" placeholder=\"search  /\" aria-label=\"search\">\n<div class=\"by\" role=\"group\"><button type=\"button\" data-by=\"day\" aria-pressed=\"true\">by day</button><button type=\"button\" data-by=\"agent\" aria-pressed=\"false\">by agent</button></div>\n<ul>\n{}\n</ul>\n</aside>\n",
        esc(home),
        rows.join("\n")
    )
}

/// A path under an artifact's copy, safe to join: plain names, no `..`,
/// no hidden files.
pub fn safe_rel(rest: &[&str]) -> Option<String> {
    let ok = rest.iter().all(|s| {
        !s.is_empty()
            && !s.starts_with('.')
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b' ' | b'@' | b'+'))
    });
    ok.then(|| rest.join("/"))
}

/// The content type of an artifact's file, by its extension; None: not served.
pub fn ctype(path: &str) -> Option<&'static str> {
    Some(match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::Version as ArtVersion;
    use crate::pages::store::Version;

    fn page(id: &str, at: u64) -> PageMeta {
        PageMeta {
            id: id.into(),
            title: format!("{id} <title>"),
            agent: "designer".into(),
            created_ms: at,
            versions: vec![Version { n: 2, at_ms: at, ..Default::default() }],
            ..Default::default()
        }
    }

    fn art(id: &str, kind: &str, target: &str, copy: Option<&str>, at: u64) -> ArtMeta {
        ArtMeta {
            id: id.into(),
            title: id.into(),
            kind: kind.into(),
            agent: "pages-ui".into(),
            by: "pages-ui".into(),
            versions: vec![ArtVersion { v: 1, at_ms: at, target: target.into(), copy: copy.map(Into::into), ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn lists_pages_and_site_artifacts_newest_first() {
        let pages = vec![page("arch", 10), page("plan", 30)];
        let arts = vec![
            art("mock", "site", "/tmp/x/mock.html", Some("v1/mock.html"), 20),
            art("deploy", "site", "https://bise.dev/m/timers", None, 40),
            art("sheet", "sheet", "/tmp/a.csv", Some("v1/a.csv"), 50),
            art("folder-site", "site", "/tmp/site", Some("v1/site"), 5),
            art("gone", "site", "/tmp/nocopy.html", None, 60),
        ];
        let it = items(&pages, &arts);
        let hrefs: Vec<&str> = it.iter().map(|i| i.href.as_str()).collect();
        assert_eq!(hrefs, vec!["https://bise.dev/m/timers", "/p/plan", "/a/mock/1/mock.html", "/p/arch", "/a/folder-site/1/"]);
        assert!(it[0].outside && !it[1].outside);
        assert_eq!((it[1].kind, it[1].version.as_str()), ("page", "v2"));
    }

    #[test]
    fn the_sidebar_escapes_and_marks_outside_links() {
        let html = sidebar(&items(&[page("w", 1)], &[art("d", "site", "https://x.dev", None, 2)]), "/");
        assert!(html.contains("<a class=\"home\" href=\"/\">for you</a>"));
        assert!(html.contains("<span class=\"t\">w &lt;title&gt;</span>"), "{html}");
        assert!(html.contains("data-outside><a href=\"https://x.dev\" target=\"_blank\" rel=\"noopener\">"), "{html}");
        assert!(html.contains("data-kind=\"page\" data-agent=\"designer\" data-at=\"1\"><a href=\"/p/w\">"), "{html}");
    }

    #[test]
    fn artifact_paths_stay_in_the_copy() {
        assert_eq!(safe_rel(&["index.html"]).as_deref(), Some("index.html"));
        assert_eq!(safe_rel(&["css", "a.css"]).as_deref(), Some("css/a.css"));
        assert_eq!(safe_rel(&[".."]), None);
        assert_eq!(safe_rel(&["a", "..", "b"]), None);
        assert_eq!(safe_rel(&[".git", "config"]), None);
        assert_eq!(safe_rel(&["a%2f..", "b"]), None);
        assert_eq!(ctype("x/a.HTML"), Some("text/html; charset=utf-8"));
        assert_eq!(ctype("a.sh"), None);
    }
}
