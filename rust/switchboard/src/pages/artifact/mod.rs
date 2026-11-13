//! The page server's artifact routes beyond a copy's own file
//! (office-artifacts plan §8: one file per chunk, this dispatch made by
//! office-viewer). server.rs calls [`route`] once, before its `/a/<id>/<v>/<file>`
//! arm; what is not ours falls through to it.
//!
//! - `GET /a/<id>/view`, `GET /a/<id>/<v>/view`: the old viewer's urls
//!   (gone with step 5), a 301 to the document's page `/d/<id>`
//!   (view_web.rs), for the links he already has;
//! - `GET /a/<id>/events`: the stream of its saves ([`events`]; server.rs
//!   runs it, as `/p/<id>/events`);
//! - `GET /a/<id>/live[/<path>]`: the door of a local site bise serves
//!   again ([`live`], site-revive): its server, bise's copy, or the hub
//!   starts it again;
//! - `GET /a/<id>/notes`, `POST /api/a/<id>/notes|send`: the notes on an
//!   artifact ([`notes`]; the POSTs through [`post`], server.rs's `/api`
//!   arm, after its token and origin checks);

pub mod events;
pub mod live;
pub mod notes;

use super::server::Response;
use super::store::valid_id;
use super::Pages;

/// The response for `segs` (the path's segments), or None: not an
/// artifact route of this module.
pub fn route(pages: &Pages, segs: &[&str]) -> Option<Response> {
    Some(match segs {
        // the old viewer's urls, already in his threads, notes and pages: a
        // permanent redirect to the document's place (architect m_63317)
        ["a", id, "view"] | ["a", id, _, "view"] if valid_id(id) => Response::moved(&format!("/d/{id}")),
        ["a", id, "notes"] if valid_id(id) => notes::get(pages, id),
        ["a", id, "live", rest @ ..] if valid_id(id) => live::serve(pages, id, rest),
        _ => return None,
    })
}

/// An `/api/a/…` POST (server.rs checked the token and the origin), or
/// None: not one of ours.
pub fn post(pages: &Pages, segs: &[&str], body: &[u8]) -> Option<Response> {
    Some(match segs {
        ["api", "a", id, "notes"] if valid_id(id) => notes::post_notes(pages, id, body),
        ["api", "a", id, "send"] if valid_id(id) => notes::post_send(pages, id),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::server::{route, Request};
    use super::*;
    use serde_json::json;

    fn get(p: &Pages, path: &str) -> Response {
        route(p, &Request { method: "GET".into(), path: path.into(), query: String::new(), headers: vec![("Host".into(), format!("127.0.0.1:{}", p.port))], body: vec![] })
    }

    fn text(r: &Response) -> String {
        String::from_utf8_lossy(&r.body).to_string()
    }

    /// The repo's office fixtures (office/fixtures, made by office/fixtures/make.ts).
    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../office/fixtures").join(name)).unwrap()
    }

    /// Every type opens in its plugin's view (docs/plugin-apps.md steps
    /// 1-5): an xlsx in Sheets, a web page in Sites; each row frames its
    /// copy, served as itself, and bise's old viewer routes are gone.
    #[test]
    fn an_artifact_row_frames_its_copy() {
        let d = std::env::temp_dir().join(format!("sb-office-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let work = d.join("work");
        std::fs::create_dir_all(work.join("dist")).unwrap();
        let p = Pages::new(&d, 47123, &d);
        let arts = crate::artifacts::Store::new(&d);
        let add = |file: &str, at: u64| {
            let a = crate::artifacts::Add { target: file.into(), title: None, kind: None, agent: "office-lead".into(), by: "office-lead".into(), cwd: work.to_string_lossy().to_string(), known_only: false, ..Default::default() };
            arts.add(&a, at).unwrap().meta
        };
        std::fs::write(work.join("q3.xlsx"), fixture("tiny.xlsx")).unwrap();
        let xlsx = add("q3.xlsx", 10);
        std::fs::write(work.join("q3.xlsx"), [fixture("tiny.xlsx"), b"more".to_vec()].concat()).unwrap();
        add("q3.xlsx", 20);
        std::fs::write(work.join("r.html"), "<h1>r</h1>").unwrap();
        add("r.html", 30);

        let rows = arts.rows(&|a: &str| Some((a.to_string(), false)), work.to_str().unwrap());
        let base = format!("http://127.0.0.1:{}", p.port);
        let items = crate::proto_view::artifacts(&json!({"ev": "artifacts", "rows": rows, "seen_ms": 0}), |_| None, Some(&base), |_| Default::default());
        let frame = |id: &str| items.iter().find(|a| a.id == id).and_then(|a| a.frame_url.clone()).unwrap_or_default();
        assert_eq!(frame(&xlsx.id), format!("{base}/a/{}/2/q3.xlsx", xlsx.id), "its copy: Sheets opens it");
        let html = items.iter().find(|a| a.title.starts_with("r.html")).unwrap();
        // a web page opens in Sites (plugins/sites, docs/plugin-apps.md step 5): its copy framed
        // as itself, the storage shim first (architect m_25527), never a selection script
        assert_eq!(frame(&html.id), format!("{base}/a/{}/1/r.html", html.id), "not in the viewer");
        assert_eq!(text(&get(&p, &format!("/a/{}/1/r.html", html.id))), format!("{}<h1>r</h1>", crate::pages::site::STORAGE_TAG));
        assert_eq!(get(&p, &format!("/a/{}/1/framed/r.html", html.id)).status, 404, "no framed path any more");

        let bytes = get(&p, &format!("/a/{}/2/q3.xlsx", xlsx.id));
        assert_eq!((bytes.status, bytes.ctype), (200, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
        assert!(bytes.body.starts_with(b"PK"), "the copy's bytes");

        // the viewer shell went with step 5: its links he already has move for good to the
        // document's page (architect m_63317); no /kit/office/ bundle
        for path in [format!("/a/{}/view", html.id), format!("/a/{}/1/view", html.id)] {
            let r = get(&p, &path);
            assert_eq!((r.status, r.location.as_deref()), (301, Some(format!("/d/{}", html.id).as_str())), "{path}");
        }
        assert_eq!(get(&p, "/a/No_Id/view").status, 404, "a bad id is no redirect");
        assert_eq!(get(&p, "/kit/office/office.js").status, 404);
        let _ = std::fs::remove_dir_all(d);
    }
}
