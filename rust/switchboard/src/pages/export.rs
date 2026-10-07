//! The public export (pages-ui, main m_7501): the pages an agent published
//! with `--public`, as a static, read-only site in the same look and nav as
//! the hub's `/` (the sidebar, a page opening next to it), for the user's
//! opt-in mirror (`[pages] mirror` in config.toml, `mirror.rs`). Nothing is
//! in it but the opted-in pages at their latest version: no token, no notes,
//! no stream, no send; the sidebar lists only them.
//!
//! The layout (designer m_7509, bise.dev/artifacts on a Vercel project of its
//! own): `vercel.json` (cleanUrls, noindex), `artifacts/index.html` (the
//! index), `artifacts/<id>/index.html` and its `page.css`, the kit under
//! `artifacts/_kit/`. Links are relative and name `index.html`, so the same
//! folder opens offline from disk.
//!
//! Pure: the files as (path, bytes); the shell writes them.

use super::lint::Block;
use super::server::esc;
use super::site::{sidebar, Item};
use super::store::Meta;

/// The folder the pages sit in, inside the export (bise.dev/artifacts/…).
pub const ROOT: &str = "artifacts";

/// The kit files a static page needs (no notes.js, no pearl.js: nothing
/// talks to a hub).
pub const KIT_FILES: &[&str] = &[
    "tokens.css",
    "kit.css",
    "site.css",
    "kit.js",
    "site.js",
    "fonts/newsreader.woff2",
    "fonts/newsreader-italic.woff2",
    "fonts/jetbrains-mono.woff2",
    "fonts/OFL-newsreader.txt",
    "fonts/OFL-jetbrains-mono.txt",
];

/// Block kinds a public page never holds: drafts that leave his accounts
/// and what he handles one by one (his mails, messages, replies, writes).
pub const PRIVATE_KINDS: &[&str] = &["email", "message", "review", "action"];

/// What keeps a page out of the export, one line each (empty: it may be
/// public): his own pages (about-you, morning, promises, meetings) and the
/// private kinds.
pub fn refused(id: &str, blocks: &[Block], html: &str) -> Vec<String> {
    let mut out = Vec::new();
    // his steps' words stay his (ambient-lead m_7535)
    if html.contains("data-who=\"yours\"") && html.contains("data-kit=\"checklist\"") {
        out.push("a checklist row of his (data-who=\"yours\") never goes public: drop it, or publish without --public".into());
    }
    // links that only work on his machine (ambient-lead m_7535): relative or https only
    for bad in ["127.0.0.1", "localhost", "file:", "href=\"/", "src=\"/"] {
        if html.contains(bad) {
            out.push(format!("a link to {bad}… only works on his machine: a public page links with https:// (or to nothing)"));
        }
    }
    if id == "about-you" || id.starts_with("morning") || id.starts_with("promises-") || id.starts_with("meeting-") {
        out.push(format!("{id}: this page is the user's own (about-you, morning, promises, meetings): it never goes public; publish without --public"));
    }
    for b in blocks.iter().filter(|b| PRIVATE_KINDS.contains(&b.kit.as_str())) {
        out.push(format!("{}: a {} block never goes public (his drafts and items stay on his machine): drop it from a --public page, or publish without --public", b.id, b.kit));
    }
    out
}

/// One public page: its meta and the fragment of its latest version.
pub struct Public {
    pub meta: Meta,
    pub html: String,
}

/// The export's files, `kit` reading a kit file by its name.
pub fn files(pages: &[Public], kit: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut pages: Vec<&Public> = pages.iter().filter(|p| p.meta.public && p.meta.version() > 0).collect();
    pages.sort_by(|a, b| b.meta.at_ms().cmp(&a.meta.at_ms()).then(a.meta.id.cmp(&b.meta.id)));
    out.push(("vercel.json".into(), VERCEL.as_bytes().to_vec()));
    out.push((format!("{ROOT}/index.html"), index(&pages).into_bytes()));
    for p in &pages {
        let dir = format!("{ROOT}/{}", p.meta.id);
        out.push((format!("{dir}/index.html"), page(p).into_bytes()));
        out.push((format!("{dir}/page.css"), super::ui::page_css(&p.html).into_bytes()));
    }
    for f in KIT_FILES {
        if let Some(b) = kit(f) {
            out.push((format!("{ROOT}/_kit/{f}"), b));
        }
    }
    out
}

/// Vercel's settings for the export (designer m_7509): clean URLs, and
/// these are working pages, not the site: noindex.
const VERCEL: &str = r#"{
  "cleanUrls": true,
  "headers": [
    { "source": "/artifacts(.*)", "headers": [{ "key": "X-Robots-Tag", "value": "noindex" }] }
  ]
}
"#;

fn head(title: &str, kit: &str, extra: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"robots\" content=\"noindex, nofollow\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"{kit}tokens.css\">\n<link rel=\"stylesheet\" href=\"{kit}kit.css\">\n<script src=\"{kit}kit.js\" defer></script>\n{extra}</head>\n",
        esc(title)
    )
}

fn index(pages: &[&Public]) -> String {
    let items: Vec<Item> = pages
        .iter()
        .map(|p| Item {
            title: p.meta.title.clone(),
            agent: p.meta.agent.clone(),
            kind: "page",
            at_ms: p.meta.at_ms(),
            version: format!("v{}", p.meta.version()),
            href: format!("{}/index.html", p.meta.id),
            outside: false,
        })
        .collect();
    let rows: Vec<String> = pages
        .iter()
        .map(|p| {
            format!(
                "<li data-page=\"{}\" data-agent=\"{}\" data-version=\"{}\" data-at=\"{}\"><a href=\"{}/index.html\">{}</a></li>",
                esc(&p.meta.id),
                esc(&p.meta.agent),
                p.meta.version(),
                p.meta.at_ms(),
                esc(&p.meta.id),
                esc(&p.meta.title)
            )
        })
        .collect();
    let extra = "<link rel=\"stylesheet\" href=\"_kit/site.css\">\n<script src=\"_kit/site.js\" defer></script>\n";
    format!(
        "{}<body>\n<main id=\"bise-home\" data-home data-static>\n{}<section data-kit=\"pages\" data-id=\"pages\">\n<ul>\n{}\n</ul>\n</section>\n</main>\n</body>\n</html>\n",
        head("bise · pages", "_kit/", extra),
        sidebar(&items, "index.html"),
        rows.join("\n")
    )
}

fn page(p: &Public) -> String {
    let m = &p.meta;
    let extra = "<link rel=\"stylesheet\" href=\"page.css\">\n";
    format!(
        "{}<body>\n<main id=\"bise-page\" data-static data-page=\"{}\" data-version=\"{}\" data-agent=\"{}\" data-title=\"{}\" data-at=\"{}\">\n{}\n</main>\n</body>\n</html>\n",
        head(&m.title, "../_kit/", extra),
        esc(&m.id),
        m.version(),
        esc(&m.agent),
        esc(&m.title),
        m.at_ms(),
        super::ui::strip_styles(&p.html)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::store::Version;

    fn meta(id: &str, public: bool, at: u64) -> Meta {
        Meta {
            id: id.into(),
            title: format!("{id} title"),
            agent: "designer".into(),
            created_ms: at,
            versions: vec![Version { n: 3, at_ms: at, ..Default::default() }],
            public,
            ..Default::default()
        }
    }

    fn kit(f: &str) -> Option<Vec<u8>> {
        Some(format!("/* {f} */").into_bytes())
    }

    const UI: &str = "<section data-kit=\"ui\" data-id=\"s\"><style>.r { color: var(--term-dim) }</style><div class=\"r\">x</div></section>";

    #[test]
    fn only_the_public_pages_are_in_it() {
        let pages = vec![
            Public { meta: meta("scheduled", true, 20), html: UI.into() },
            Public { meta: meta("inbox-mails", false, 30), html: "<section data-kit=\"prose\" data-id=\"p\"><p>secret mail</p></section>".into() },
        ];
        let out = files(&pages, &kit);
        let names: Vec<&str> = out.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"artifacts/scheduled/index.html") && names.contains(&"artifacts/scheduled/page.css"));
        assert!(!names.iter().any(|n| n.contains("inbox-mails")), "{names:?}");
        let all: String = out.iter().map(|(_, b)| String::from_utf8_lossy(b).to_string()).collect();
        assert!(!all.contains("secret mail") && !all.contains("inbox-mails"));
        // no token, no notes, no stream
        assert!(!all.contains("bise-token") && !all.contains("notes.js") && !all.contains("/events"));
    }

    #[test]
    fn the_pages_are_static_and_relative() {
        let out = files(&[Public { meta: meta("scheduled", true, 20), html: UI.into() }], &kit);
        let get = |n: &str| String::from_utf8_lossy(&out.iter().find(|(x, _)| x == n).unwrap().1).to_string();
        let page = get("artifacts/scheduled/index.html");
        assert!(page.contains("<main id=\"bise-page\" data-static data-page=\"scheduled\" data-version=\"3\""), "{page}");
        assert!(page.contains("href=\"../_kit/kit.css\"") && page.contains("href=\"page.css\"") && page.contains("noindex"));
        assert!(!page.contains("<style>"), "the style goes to page.css");
        assert!(get("artifacts/scheduled/page.css").contains("[data-id=\"s\"] .r"));
        let index = get("artifacts/index.html");
        assert!(index.contains("<a href=\"scheduled/index.html\">") && index.contains("href=\"_kit/site.css\"") && index.contains("data-static"));
        assert!(index.contains("<a class=\"home\" href=\"index.html\">"));
        assert!(!index.contains("href=\"/"), "no absolute link: it opens offline and under /artifacts");
        assert!(get("vercel.json").contains("noindex"));
        assert_eq!(get("artifacts/_kit/kit.js"), "/* kit.js */");
    }

    #[test]
    fn his_own_pages_and_drafts_never_go_public() {
        let b = |id: &str, kit: &str| Block { id: id.into(), kit: kit.into(), hash: String::new() };
        assert!(refused("scheduled", &[b("s", "ui"), b("q", "question"), b("t", "table")], "<a href=\"https://bise.dev\">x</a>").is_empty());
        let e = refused("weekly", &[b("m1", "email"), b("s", "message"), b("r", "review"), b("a", "action")], "");
        assert_eq!(e.len(), 4, "{e:#?}");
        assert!(e[0].starts_with("m1: a email block never goes public"));
        for id in ["about-you", "morning", "morning-2026-10-04", "promises-week", "meeting-q3"] {
            assert!(!refused(id, &[], "").is_empty(), "{id}");
        }
        // his steps, and links that only work on his machine
        let steps = "<section data-kit=\"checklist\" data-id=\"c\"><ol><li data-id=\"t1\" data-who=\"yours\">pay</li></ol></section>";
        assert!(refused("plan", &[b("c", "checklist")], steps)[0].contains("data-who=\"yours\""));
        for link in ["<a href=\"http://127.0.0.1:47438/p/x\">x</a>", "<a href=\"/p/about-you\">x</a>", "<a href=\"file:///Users/x\">x</a>", "<a href=\"http://localhost:3000\">x</a>"] {
            assert!(!refused("plan", &[], link).is_empty(), "{link}");
        }
    }
}
