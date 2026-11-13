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
///
/// No `frame-ancestors`, on purpose (ambient-lead m_20610, architect
/// m_20628): the desktop app frames this file from its renderer, which is
/// `file://` (or the dev server), never the page server's origin, so
/// `frame-ancestors 'self'` left the frame blank; listing `file:` there is
/// unreliable. What it guarded (clickjacking) has nothing to take here: the
/// content is static, sandboxed, with `connect-src 'none'` and the Host
/// check, and a framing page can't read it. `/p/` has none either.
/// `tests::the_artifact_csp_keeps_its_sandbox` pins the other directives.
pub const ARTIFACT_CSP: &str = "sandbox allow-scripts allow-popups; default-src 'self' data: blob:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; script-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'none'";

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
        // a copy first: framed from it (the typed row's frame_url rule, serves_copy)
        let (href, outside) = if let Some(copy) = cur.copy.as_ref().filter(|c| serves_copy(&a.by, c)) {
            // framed as itself (a web page opens in Sites, plugins/sites)
            (copy_href(&a.id, cur.v, copy), false)
        } else if crate::artifacts_serve::has_door(cur) {
            // a local site bise starts again: its door (artifacts_serve)
            (crate::artifacts_serve::live_href(&a.id), true)
        } else if is_link(&cur.target) {
            (cur.target.clone(), true)
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

/// Where the page server serves version `v` of artifact `id` from bise's
/// copy (server.rs `/a/<id>/<v>/<file>`): the one rule for the pages'
/// artifact links and the typed row's `frame_url` (proto_view::artifacts).
/// The file's name is %-encoded byte for byte ([`href_name`]).
pub fn copy_href(id: &str, v: u64, copy: &str) -> String {
    format!("/a/{}/{}/{}", id, v, href_name(&file_of(copy)))
}

/// A copy's name in a link: each byte but A-Z a-z 0-9 - . _ ~ %-encoded,
/// so the server decodes it back to the very bytes on disk (an accent
/// written NFD stays NFD; architect m_52591: no NFC/NFD guess).
pub fn href_name(name: &str) -> String {
    name.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The tag every html page served under [`ARTIFACT_CSP`] gets, first
/// (kit/storage.js): an in-memory localStorage/sessionStorage where the
/// opaque origin forbids them, so a page that reads one at its top level
/// still draws (art-viewer, architect m_25527).
pub const STORAGE_TAG: &str = "<script src=\"/kit/storage.js\"></script>";

/// The id of the inert JSON block that carries a site's kept localStorage
/// (crate::artifacts_storage) to kit/storage.js, just before
/// [`STORAGE_TAG`] (web-storage, architect m_46151).
pub const SEED_ID: &str = "bise-storage";

/// The seed block for `kept`: `<script type="application/json">`, never run,
/// every `<` in it written `\u003c` so no value can close it. Empty when
/// nothing is kept.
pub fn seed_block(kept: &crate::artifacts_storage::Kept) -> String {
    if kept.is_empty() {
        return String::new();
    }
    let json = serde_json::to_string(kept).unwrap_or_else(|_| "{}".into()).replace('<', "\\u003c");
    format!("<script type=\"application/json\" id=\"{SEED_ID}\">{json}</script>")
}

/// The one rule for what an html page of a copy gets (architect m_24172,
/// m_25527, m_46151): [`seed_block`] of `kept` then [`STORAGE_TAG`], before
/// the page's own scripts (just inside its `<head>`, else its `<html>`,
/// else after its doctype, else at its start). The kept copy never
/// changes; the rest of the bytes as they are. A site opened in Sites
/// (plugins/sites) is served on its own origin at `/h/`, untouched.
pub fn tagged(html: &[u8], kept: &crate::artifacts_storage::Kept) -> Vec<u8> {
    let lower: Vec<u8> = html.iter().map(u8::to_ascii_lowercase).collect();
    let first = after_open(&lower, b"<head").or_else(|| after_open(&lower, b"<html")).or_else(|| after_open(&lower, b"<!doctype")).unwrap_or(0);
    let seed = seed_block(kept);
    let mut out = Vec::with_capacity(html.len() + seed.len() + STORAGE_TAG.len());
    out.extend_from_slice(&html[..first]);
    out.extend_from_slice(seed.as_bytes());
    out.extend_from_slice(STORAGE_TAG.as_bytes());
    out.extend_from_slice(&html[first..]);
    out
}

/// Just past the first `<name …>` open tag of `lower` (a lowercased page):
/// `<head>` or `<head lang=…>`, never `<header>`. None: it has none.
fn after_open(lower: &[u8], name: &[u8]) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = lower[from..].windows(name.len()).position(|w| w == name).map(|i| i + from) {
        let next = lower.get(i + name.len()).copied();
        if matches!(next, Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/')) {
            return lower[i..].iter().position(|&b| b == b'>').map(|j| i + j + 1);
        }
        from = i + 1;
    }
    None
}

/// Whether the page server serves an artifact from its kept copy at
/// [`copy_href`] (server.rs `artifact_file`): any artifact but a bise page
/// (`by` page: served at /p/), with a copy, that is a folder (its
/// index.html) or a file of a type the server sends ([`ctype`]). The one
/// rule for the server's route and the typed row's `frame_url`
/// (proto_view::artifacts; ambient-lead m_20287: the route served sites
/// only, so a copied .html file's frame_url said 'no such artifact').
pub fn serves_copy(by: &str, copy: &str) -> bool {
    let file = file_of(copy);
    by != "page" && !copy.is_empty() && (file.is_empty() || ctype(&file).is_some())
}

/// What the page server serves for a version of an artifact added `by`
/// with that `copy`: its copy when it serves one ([`serves_copy`]); else
/// nothing (a link, a bise page, a file too big to copy). The one rule for
/// the server's route (`artifact_target`) and the typed row's `frame_url`
/// (proto_view::artifacts). A file bise kept no copy of is read by its app
/// through a handle, from its own file while its sig holds
/// (view_files_disk, docs/plugin-apps.md A4), never here.
pub fn served<'a>(by: &str, copy: Option<&'a str>) -> Option<&'a str> {
    copy.filter(|c| serves_copy(by, c))
}

/// The file a copy opens on: a copied file's name (`v2/plan.html` →
/// `plan.html`), nothing for a folder (its index.html).
pub fn file_of(copy: &str) -> String {
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

/// The file a request under version `v`'s copy names (`rest`, after
/// `/a/<id>/<v>/`), `base` the copy (`<id>/v<n>/<name>`):
/// a folder's file (none: its index.html); a copied file by its name (or
/// none), or a file it links kept beside it (artifacts_assets). The one
/// rule for both: inside the version's folder, no `..`, hidden name or
/// separator ([`safe_rel`], after %-decoding), and its real path stays there (a
/// symlink out is refused). None: not served.
pub fn copy_file(base: &std::path::Path, rest: &[&str]) -> Option<std::path::PathBuf> {
    let file = base.is_file();
    let root = if file { base.parent()? } else { base };
    let rest: &[&str] = match (file, rest) {
        (true, []) => return Some(base.to_path_buf()),
        (false, []) => &["index.html"],
        _ => rest,
    };
    // a request's names come %-encoded (`a%20b.png`); a decoded `/` is refused
    let names: Vec<String> = rest.iter().map(|s| crate::artifacts_assets::decode(s)).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let real = root.join(safe_rel(&names)?).canonicalize().ok()?;
    (real.is_file() && real.starts_with(root.canonicalize().ok()?)).then_some(real)
}

/// A path under an artifact's copy (its names, %-decoded), safe to join:
/// refused only what can leave the folder or reach a hidden file: no
/// names, an empty one, one that starts with `.` (`..`, `.git`), a `/`
/// or `\` inside one (a decoded `%2f`, `%5c`), a control character (NUL).
/// Any other name passes, accents and parentheses too (artifact-name-404);
/// copy_file's canonicalize + starts_with stays the real guard.
pub fn safe_rel(rest: &[&str]) -> Option<String> {
    let ok = !rest.is_empty()
        && rest.iter().all(|s| !s.is_empty() && !s.starts_with('.') && !s.chars().any(|c| c == '/' || c == '\\' || c.is_control()));
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
        // the bytes a plugin's view reads (Writer, Slides, Sheets); a browser downloads them
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        // drawn by pdf.js in bise's viewer (pdf-notes)
        "pdf" => "application/pdf",
        // played in bise's viewer, streamed by their byte ranges (media.rs)
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::Version as ArtVersion;

    /// The viewer shell is gone (docs/plugin-apps.md steps 1-5): the one
    /// built-in type left is a bise page, framed at its own /p/<id> (step 6).
    #[test]
    fn a_bise_page_is_the_last_builtin_type() {
        use bend_plugins::opens::BUILTIN;
        assert_eq!(BUILTIN.iter().map(|b| (b.key, b.kind, b.folder)).collect::<Vec<_>>(), [("*.page", "page", false)]);
    }
    use crate::artifacts_storage::Kept;
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
        // a local site's copy at its own link (Sites opens it, plugins/sites)
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
        // safe_rel gets names decoded once (copy_file): a decoded %2f is a `/`
        assert_eq!(safe_rel(&["a/..", "b"]), None, "a decoded %2f");
        assert_eq!(safe_rel(&["..\\x"]), None);
        assert_eq!(safe_rel(&["a\\b.pdf"]), None, "a decoded %5c");
        assert_eq!(safe_rel(&["a\0.pdf"]), None);
        assert_eq!(safe_rel(&["a\n.pdf"]), None);
        assert_eq!(safe_rel(&["a", ""]), None);
        assert_eq!(safe_rel(&[]), None);
        let name = "Résumé (à signer) 日本.pdf";
        assert_eq!(safe_rel(&[name]).as_deref(), Some(name));
        assert_eq!(href_name(name), "R%C3%A9sum%C3%A9%20%28%C3%A0%20signer%29%20%E6%97%A5%E6%9C%AC.pdf");
        assert_eq!(crate::artifacts_assets::decode(&href_name(name)), name, "the bytes come back");
        assert_eq!(ctype("x/a.HTML"), Some("text/html; charset=utf-8"));
        assert_eq!(ctype("a.sh"), None);
    }

    /// The desktop app frames an artifact from file:// (architect m_20628):
    /// no frame-ancestors, and dropping it took nothing else with it.
    #[test]
    fn the_artifact_csp_keeps_its_sandbox() {
        let d: Vec<&str> = ARTIFACT_CSP.split(';').map(str::trim).collect();
        assert!(d.contains(&"sandbox allow-scripts allow-popups"), "an opaque origin: {ARTIFACT_CSP}");
        assert!(!d[0].contains("allow-same-origin"), "never same-origin with the page API");
        assert!(d.contains(&"connect-src 'none'"), "no requests out");
        assert!(d.contains(&"script-src 'self' 'unsafe-inline'"), "no remote scripts");
        assert!(d.contains(&"default-src 'self' data: blob:"));
        assert!(!ARTIFACT_CSP.contains("frame-ancestors"), "the desktop's file:// renderer frames it");
    }

    /// Every type opens in its plugin's view (docs/plugin-apps.md steps
    /// 1-5: Preview, Writer, Slides, Sheets, Sites), so a served copy is
    /// framed as itself at its own link, by one rule (serves_copy).
    #[test]
    fn a_served_copy_is_framed_as_itself() {
        for copy in ["v2/q3.xlsx", "v1/memo.docx", "v2/Deck.PPTX"] {
            assert!(serves_copy("office-lead", copy), "{copy}: its bytes are served");
        }
        // a web page or a site folder opens in Sites (plugins/sites, step 5); its copy keeps its own link
        assert_eq!((copy_href("r", 3, "v3/r.html"), copy_href("f", 1, "v1/site")), ("/a/r/3/r.html".to_string(), "/a/f/1/".to_string()));
        // images, pdfs and videos open in their app (plugins/media's Preview), their copy at its
        // own href, a video's by its ranges (media.rs)
        for f in ["v1/shot.PNG", "v2/report.pdf", "v1/film.mp4", "v1/a.MOV", "v1/loop.gif"] {
            assert!(serves_copy("main", f), "{f}");
        }
        assert_eq!(copy_href("s", 1, "v1/shot.png"), "/a/s/1/shot.png");
        // a markdown file opens in Writer (plugins/docs, step 1), its copy text/plain; a .txt too
        assert!(serves_copy("learnings", "v3/learnings.md") && ctype("v3/learnings.md") == Some("text/plain; charset=utf-8"));
        for f in ["v1/film.mp4", "v1/a.MOV", "v1/b.webm", "v1/c.m4v"] {
            assert!(ctype(f).is_some_and(|c| c.starts_with("video/")), "{f}: a video copy is sent by its ranges");
        }
        assert!(!serves_copy("page", "v1/q3.xlsx"), "a bise page is served at /p/");
        assert!(!serves_copy("x", "v1/a.xls"), "an old binary format is not sent");
    }

    /// The server frames a copy only: a file too big to copy (a long video)
    /// opens in its app, read through a handle from its own file
    /// (docs/plugin-apps.md A4), never at a page server url.
    #[test]
    fn only_a_copy_is_served() {
        assert_eq!(served("main", Some("v1/film.mp4")), Some("v1/film.mp4"));
        assert_eq!(served("main", None), None, "too big to copy: no frame");
        assert_eq!(served("main", Some("")), None);
        assert_eq!(served("page", Some("v1/q3.xlsx")), None, "a bise page is served at /p/");
    }

    /// architect m_25527: the one table of what an html copy gets. The
    /// storage tag first, before the page's own scripts (never before the
    /// doctype: quirks mode).
    #[test]
    fn an_html_copy_gets_storage_first() {
        let s = STORAGE_TAG;
        let cases: [(&str, String); 6] = [
            ("<!doctype html><html><HEAD><script>localStorage</script></head><body><p>hi</p></BODY></html>", format!("<!doctype html><html><HEAD>{s}<script>localStorage</script></head><body><p>hi</p></BODY></html>")),
            ("<html lang=\"en\"><head data-x><title>t</title>", format!("<html lang=\"en\"><head data-x>{s}<title>t</title>")),
            ("<html><body><header>h</header>", format!("<html>{s}<body><header>h</header>")),
            ("<!DOCTYPE html>\n<script>a()</script>", format!("<!DOCTYPE html>{s}\n<script>a()</script>")),
            ("<p>no body", format!("{s}<p>no body")),
            ("", s.to_string()),
        ];
        for (page, want) in cases {
            assert_eq!(String::from_utf8(tagged(page.as_bytes(), &Kept::new())).unwrap(), want, "{page}");
        }
    }

    /// architect m_46151: a kept localStorage goes in as an inert JSON block
    /// just before the storage tag; no value can close the block.
    #[test]
    fn a_kept_storage_goes_in_inert_before_the_storage_tag() {
        let kept = Kept::from([("rent".to_string(), "</script><script>alert(1)</script>".to_string())]);
        let out = String::from_utf8(tagged(b"<html><head><script>localStorage</script></head>", &kept)).unwrap();
        let block = seed_block(&kept);
        assert_eq!(out, format!("<html><head>{block}{STORAGE_TAG}<script>localStorage</script></head>"));
        assert!(block.starts_with("<script type=\"application/json\" id=\"bise-storage\">"));
        let inner = block.trim_start_matches(|c| c != '>').trim_start_matches('>').trim_end_matches("</script>");
        assert!(!inner.contains('<'), "no '<' inside: {inner}");
        let back: Kept = serde_json::from_str(inner).unwrap();
        assert_eq!(back, kept, "the value round-trips");
        assert_eq!(seed_block(&Kept::new()), "", "nothing kept, no block");
    }
}
