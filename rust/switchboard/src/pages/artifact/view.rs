//! bise's viewer of an xlsx, pptx or docx artifact (office-artifacts plan
//! §4): `/a/<id>/view` (the newest version; the side panel keeps this src
//! across versions) and `/a/<id>/<v>/view` (an older one). The server
//! sends a small HTML shell; `kit/office/viewer.js` fetches the copy's bytes
//! from `/a/<id>/<v>/<file>` (pages::site::copy_href) and draws them with a
//! renderer per format (`kit/office/{sheet,slides,docx}.js`) on the office
//! SDK's bundles (`/kit/office/office.js`, `office-view.js`).
//!
//! Which artifacts get the viewer is pages::site's one rule (served +
//! viewer_kind), the same that gives the row its `frame_url`: its copy, or
//! a video too big to copy from its own file while its sig holds
//! (media::own_holds; video-artifact).
//!
//! The shell sets the theme the panel asked for first (`?theme=`,
//! kit/office/theme.js: kit.js's rule; md-artifact). It also carries the page server's token and loads the notes
//! layer (kit/notes-anchors.js, kit/notes.js's model and words,
//! kit/office/select.js: the selection, the marks, the browser's list;
//! pages/artifact/notes.rs is its api). An image gets the same frame
//! (`site::viewer_kind`), drawn by kit/office/image.js; a pdf too, drawn
//! by kit/office/pdf.js (pdf.js, `office-pdf.js` and `pdf.worker.js`); a
//! video too, played by kit/office/video.js from the copy's url (the shell
//! says `data-stream`: no bytes fetched whole; its ranges: pages/media.rs).
//! A markdown file too, drawn as a document by kit/office/markdown.js
//! (micromark + GFM in `office-view.js`), its source a toggle away.
//!
//! [`shell`] is pure; [`serve`] reads the artifact store.

use super::super::server::{esc, Response};
use super::super::site;
use super::super::Pages;

/// The viewer's CSP (architect m_23781): an office file is untrusted
/// content drawn into our DOM, so no script but ours ('self', never
/// 'unsafe-inline' or 'unsafe-eval'); its embedded images and fonts as
/// data: or blob: urls; inline styles allowed (docx-preview draws a
/// document with `<style>` and style attributes; a style runs no code and
/// `connect-src 'self'` takes nothing out). No frame-ancestors: the desktop
/// frames it from file:// (5243d2de). The token in the shell: the viewer
/// has the notes' `/api` (art-notes) under the same script rule.
pub const VIEW_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// What the shell says about the file it draws.
#[derive(Debug, PartialEq)]
pub struct Shown<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub agent: &'a str,
    /// `xlsx`, `pptx`, `docx`, `image`, `site`, `pdf`, `video`, `markdown` (site::viewer_kind)
    pub kind: &'static str,
    /// the version drawn, and the artifact's newest
    pub v: u64,
    pub latest: u64,
    /// true: `/a/<id>/view`, the frame follows the newest version
    pub follows: bool,
    /// where the bytes are (`/a/<id>/<v>/<file>`)
    pub file: String,
    /// the file's name, for the words
    pub name: String,
    /// the page server's token: the notes' `/api/a/<id>/notes|send`
    pub token: &'a str,
}

/// The viewer's HTML: the kit's tokens, the viewer's CSS and script, and
/// the file's facts as `data-` attributes on `main#bise-office`.
pub fn shell(s: &Shown) -> String {
    let follows = if s.follows { " data-follows" } else { "" };
    // a video: the player reads the copy's url by ranges, never the bytes whole (site::STREAMED_KINDS)
    let stream = if site::STREAMED_KINDS.contains(&s.kind) { " data-stream" } else { "" };
    format!(
        "<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<meta name=\"bise-token\" content=\"{token}\">
<title>{title}</title>
<script src=\"/kit/office/theme.js\"></script>
<link rel=\"stylesheet\" href=\"/kit/tokens.css\">
<link rel=\"stylesheet\" href=\"/kit/office/viewer.css\">
<link rel=\"stylesheet\" href=\"/kit/office/select.css\">
<script src=\"/kit/notes-anchors.js\" defer></script>
<script src=\"/kit/notes.js\" defer></script>
<script type=\"module\" src=\"/kit/office/viewer.js\"></script>
<script type=\"module\" src=\"/kit/office/select.js\"></script>
<script type=\"module\" src=\"/kit/office/swap.js\"></script>
</head>
<body>
<main id=\"bise-office\" data-artifact=\"{id}\" data-kind=\"{kind}\" data-version=\"{v}\" data-latest=\"{latest}\"{follows}{stream} data-file=\"{file}\" data-name=\"{name}\" data-agent=\"{agent}\" data-title=\"{title}\">
<p class=\"office-state\" data-state=\"opening\">opening {name}…</p>
</main>
</body>
</html>
",
        title = esc(s.title),
        id = esc(s.id),
        kind = s.kind,
        v = s.v,
        latest = s.latest,
        file = esc(&s.file),
        name = esc(&s.name),
        agent = esc(s.agent),
        token = esc(s.token),
    )
}

/// `GET /a/<id>/view` (`v` None: the newest) or `/a/<id>/<v>/view`.
pub fn serve(pages: &Pages, id: &str, v: Option<u64>) -> Response {
    let state = pages.store.dir.parent().unwrap_or(&pages.store.dir);
    let store = crate::artifacts::Store::new(state);
    let at = match v {
        Some(n) => format!("a/{id}/{n}/view"),
        None => format!("a/{id}/view"),
    };
    let Some(m) = store.get(id) else { return Response::missing(&format!("no such artifact: {at}")) };
    let Some(latest) = m.current() else { return Response::missing(&format!("no version yet: {at}")) };
    let Some(ver) = (match v {
        Some(n) => m.versions.iter().find(|x| x.v == n),
        None => Some(latest),
    }) else {
        return Response::missing(&format!("no such version: {at}"));
    };
    // its copy, or its own file for a video too big to copy (site::served, architect m_54229)
    let Some(served) = site::served(&m.by, ver.copy.as_deref(), ver.no_copy.as_deref(), &ver.target) else { return Response::missing(&format!("no copy of it: {at}")) };
    let copy = served.file();
    let Some(kind) = site::viewer_kind(copy) else { return Response::missing(&format!("not a file bise's viewer draws: {at}")) };
    // its copy gone from bise's store, or its own file changed or gone: the 404 the window reads
    // to draw its 'file gone' state (desktop probe.ts; artifact-find), never a viewer that fails
    // inside the frame
    let there = match served {
        site::Served::Copy(c) => site::copy_file(&state.join("artifacts").join(&m.id).join(c), &[]).is_some(),
        site::Served::Own(file) => super::super::media::own_holds(std::path::Path::new(file), &ver.sig),
    };
    if !there {
        return Response::missing(&format!("no such file: {at}"));
    }
    let shown = Shown {
        id: &m.id,
        title: &m.title,
        agent: &m.agent,
        kind,
        v: ver.v,
        latest: latest.v,
        follows: v.is_none(),
        // a site is framed with the selection script (site::framed_href)
        file: if kind == "site" { site::framed_href(&m.id, ver.v, copy) } else { site::copy_href(&m.id, ver.v, copy) },
        name: site::file_of(copy),
        token: &pages.token,
    };
    Response { status: 200, ctype: "text/html; charset=utf-8", body: shell(&shown).into_bytes(), csp: VIEW_CSP.into(), location: None, etag: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// architect m_23781: the viewer draws untrusted files into our DOM;
    /// these directives are the line it never crosses.
    #[test]
    fn the_viewer_csp_runs_only_our_scripts() {
        let d: Vec<&str> = VIEW_CSP.split(';').map(str::trim).collect();
        assert!(d.contains(&"script-src 'self'"), "our scripts only: {VIEW_CSP}");
        assert!(!VIEW_CSP.contains("unsafe-eval"));
        assert!(!d.iter().any(|x| x.starts_with("script-src") && x.contains("unsafe-inline")), "never an inline script");
        assert!(d.contains(&"img-src 'self' data: blob:"), "the file's embedded images");
        assert!(d.contains(&"object-src 'none'"));
        // no source names another origin (architect m_23818): an injected
        // style has no way out, even once art-notes puts a token in the frame
        let sources = ["'self'", "'none'", "data:", "blob:", "'unsafe-inline'"];
        for x in &d {
            let srcs: Vec<&str> = x.split_whitespace().skip(1).collect();
            assert!(srcs.iter().all(|s| sources.contains(s)), "{x}: only self, data:, blob:");
        }
        assert!(!VIEW_CSP.contains("frame-ancestors"), "the desktop frames it from file://");
        assert!(!VIEW_CSP.contains("sandbox"), "same origin: it fetches the copy's bytes");
    }

    #[test]
    fn the_shell_carries_the_file_and_escapes_it() {
        let s = Shown {
            id: "q3",
            title: "Q3 <numbers>",
            agent: "office-lead",
            kind: "xlsx",
            v: 2,
            latest: 3,
            follows: false,
            file: "/a/q3/2/q3.xlsx".into(),
            name: "q3.xlsx".into(),
            token: "t0k",
        };
        let html = shell(&s);
        assert!(html.contains("data-artifact=\"q3\" data-kind=\"xlsx\" data-version=\"2\" data-latest=\"3\" data-file=\"/a/q3/2/q3.xlsx\""), "{html}");
        assert!(html.contains("<title>Q3 &lt;numbers&gt;</title>"));
        assert!(!html.contains("data-follows"));
        assert!(html.contains("<script type=\"module\" src=\"/kit/office/viewer.js\"></script>"));
        // md-artifact: the panel's ?theme on <html data-theme> before any style or paint
        let theme = html.find("<script src=\"/kit/office/theme.js\"></script>").expect("theme.js");
        assert!(theme < html.find("/kit/tokens.css").unwrap(), "theme first, then the tokens");
        // the notes' api (art-notes): the token, then notes.js's model and the selection layer
        assert!(html.contains("<meta name=\"bise-token\" content=\"t0k\">"));
        let order: Vec<usize> = ["notes-anchors.js", "/kit/notes.js", "office/viewer.js", "office/select.js"].iter().map(|f| html.find(f).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "anchors, notes, viewer, select");
        // art-live: the swap in place, after the viewer (window.biseOffice)
        assert!(html.contains("<script type=\"module\" src=\"/kit/office/swap.js\"></script>"));
        assert!(!html.contains("<script>"), "no inline script: the CSP would refuse it");
        assert!(!html.contains("data-stream"), "an office file's bytes are fetched whole");
        assert!(shell(&Shown { follows: true, ..s }).contains("data-follows"));
        // video-view: a video is streamed, its player reads the copy's url by ranges (site::STREAMED_KINDS)
        assert!(shell(&Shown { kind: "video", file: "/a/q3/2/film.mp4".into(), name: "film.mp4".into(), ..s }).contains(" data-stream "));
    }
}
