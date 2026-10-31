use super::*;
use crate::pages::store::Publish;

fn pages() -> (Pages, std::path::PathBuf) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("sb-srv-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(d.join("kit")).unwrap();
    std::fs::write(d.join("kit/kit.css"), "main{}").unwrap();
    (Pages::new(&d, 47123, &d), d)
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
    let order: Vec<usize> = ["tokens.css", "kit.css", "notes.css", "/kit/ask.js", "/kit/kit.js", "plan.js", "pearl.js", "notelines.js", "notes-anchors.js", "notes-fold.js", "/kit/notes.js"].iter().map(|f| t.find(f).unwrap()).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the shell's order: tokens, kit, notes css, then ask.js, kit.js, plan.js, pearl.js, notelines.js, notes-anchors.js, notes-fold.js, notes.js");
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
    // a missing file a page links: 404 text/plain, never JSON (a browser
    // reports a JSON stylesheet as the wrong MIME type, Benjamin's -31)
    // and it names what is missing by its place, never a path on his machine
    for path in ["/kit/tokens.css", "/kit/nope.js", "/p/w/v/9/page.css", "/p/w/v/x/page.css", "/a/nope/1/s.css"] {
        let r = get(&p, path);
        assert_eq!((r.status, r.ctype), (404, "text/plain; charset=utf-8"), "{path}");
        let why = text(&r);
        assert!(why.contains(&path[1..].replace("/page.css", "")) && !why.contains(&*d.to_string_lossy()), "{path}: {why}");
    }
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
        PageMsg::Sent { target, agent, text, .. } => {
            assert_eq!((target, agent.as_str()), (Target::Page("w".into()), "t1"));
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

/// ambient-lead m_20287: 'sb artifact add proof.html' then GET its
/// frame_url gives the file (the route served sites only, so the
/// window's frame said 'no such artifact'); a type the server never
/// sends gets no frame_url and no file; a bise page and a link none; a
/// pdf is framed in bise's viewer (pdf-notes).
#[test]
fn a_copied_file_is_served_at_its_frame_url() {
    let (p, d) = pages();
    let work = d.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("proof.html"), b"<h1>proof</h1>").unwrap();
    std::fs::write(work.join("notes.pdf"), b"%PDF-1.4").unwrap();
    let arts = crate::artifacts::Store::new(&d);
    let add = |target: &str| {
        let a = crate::artifacts::Add { target: target.into(), title: None, kind: None, agent: "main".into(), by: "main".into(), cwd: work.to_string_lossy().to_string(), known_only: false, ..Default::default() };
        arts.add(&a, 10).unwrap()
    };
    add("proof.html");
    add("notes.pdf");
    add("https://bise.dev/docs");
    let rows = arts.rows(&|a: &str| Some((a.to_string(), false)), work.to_str().unwrap());
    let ev = json!({"ev": "artifacts", "rows": rows, "seen_ms": 0});
    let base = format!("http://127.0.0.1:{}", p.port);
    let items = crate::proto_view::artifacts(&ev, |_| None, Some(&base));
    let html = items.iter().find(|a| a.title.contains("proof")).expect("the html artifact");
    let url = html.frame_url.clone().expect("a copied .html file has a frame_url");
    // framed in bise's viewer (art-notes), which frames the copy, still in its sandbox
    let shell = text(&get(&p, url.strip_prefix(&base).unwrap()));
    assert!(shell.contains("data-kind=\"site\""), "{shell}");
    let r = get(&p, &format!("/a/{}/1/proof.html", html.id));
    assert_eq!((r.status, text(&r)), (200, format!("{}<h1>proof</h1>", super::super::site::STORAGE_TAG)), "{url}");
    assert_eq!(r.csp, super::super::site::ARTIFACT_CSP, "in its sandbox");
    assert_eq!(get(&p, &format!("/a/{}/1/framed/proof.html", html.id)).csp, super::super::site::ARTIFACT_CSP, "framed: the same sandbox");
    for a in items.iter().filter(|a| a.id != html.id && !a.title.contains("notes")) {
        assert_eq!(a.frame_url, None, "{} has no frame_url", a.id);
    }
    // a pdf in the viewer too (pdf-notes: drawn by pdf.js, notes on a passage or a box)
    let pdf = items.iter().find(|a| a.title.contains("notes")).expect("the pdf");
    let shell = text(&get(&p, pdf.frame_url.as_deref().expect("a pdf has a frame_url").strip_prefix(&base).unwrap()));
    assert!(shell.contains("data-kind=\"pdf\""), "{shell}");
    let r = get(&p, &crate::pages::site::copy_href(&pdf.id, 1, pdf.copy.as_deref().unwrap_or("")));
    assert_eq!((r.status, r.ctype), (200, "application/pdf"));
    let _ = std::fs::remove_dir_all(d);
}

/// art-assets: an html artifact's images and css (and the css's
/// url()) are kept beside its copy and served from its version's
/// path, plain and framed, byte for byte; a later version keeps its
/// own; nothing outside the version's folder is served.
#[test]
fn an_html_artifacts_files_are_served_from_its_version() {
    let (p, d) = pages();
    let work = d.join("work");
    std::fs::create_dir_all(work.join("img")).unwrap();
    std::fs::create_dir_all(work.join("css")).unwrap();
    std::fs::write(d.join("secret.txt"), b"no").unwrap();
    std::fs::write(work.join("img/shot one@2x.png"), b"PNG1").unwrap();
    std::fs::write(work.join("img/bg.png"), b"BG").unwrap();
    std::fs::write(work.join("css/site.css"), b".x{background:url('../img/bg.png')}").unwrap();
    std::fs::write(work.join("index.html"), br#"<link rel="stylesheet" href="css/site.css"><img src="img/shot%20one@2x.png"><img src="img/gone.png">"#).unwrap();
    let arts = crate::artifacts::Store::new(&d);
    let add = |at: u64| {
        let a = crate::artifacts::Add { target: "index.html".into(), title: Some("menus".into()), agent: "main".into(), by: "main".into(), cwd: work.to_string_lossy().to_string(), ..Default::default() };
        arts.add(&a, at).unwrap()
    };
    let a = add(10);
    assert_eq!(
        crate::artifacts::added_text(&a),
        "added menus (page) · v1 · with 3 files it links · link it as [menus](artifact:menus)\n▲ 1 file it links is not kept: img/gone.png (missing)"
    );
    for (path, body) in [("img/shot%20one@2x.png", "PNG1"), ("img/bg.png", "BG"), ("css/site.css", ".x{background:url('../img/bg.png')}")] {
        for at in [format!("/a/menus/1/{path}"), format!("/a/menus/1/framed/{path}")] {
            let r = get(&p, &at);
            assert_eq!((r.status, text(&r).as_str(), &*r.csp), (200, body, super::super::site::ARTIFACT_CSP), "{at}");
        }
    }
    // architect m_25527: the plain copy gets the storage tag only, the framed both; the kept copy never changes
    let (plain, framed) = (text(&get(&p, "/a/menus/1/index.html")), text(&get(&p, "/a/menus/1/framed/index.html")));
    assert!(plain.starts_with(super::super::site::STORAGE_TAG) && !plain.contains("site-select"), "{plain}");
    assert!(framed.starts_with(super::super::site::STORAGE_TAG) && framed.contains("site-select"), "{framed}");
    assert!(!std::fs::read_to_string(d.join("artifacts/menus/v1/index.html")).unwrap().contains("/kit/"), "the kept copy as it was");
    // nothing outside the version's folder
    std::os::unix::fs::symlink(d.join("secret.txt"), d.join("artifacts/menus/v1/img/out.png")).unwrap();
    for bad in ["img/gone.png", "../v1/index.html", "..%2f..%2fsecret.txt", "img/.part", "img/out.png", "%2fetc%2fpasswd", "img"] {
        assert_eq!(get(&p, &format!("/a/menus/1/{bad}")).status, 404, "{bad}");
    }
    // unchanged: the same version; one image changed: v2 with its own, v1 keeps its own
    assert!(!add(20).new_version, "nothing changed");
    std::fs::write(work.join("img/bg.png"), b"BG2").unwrap();
    let b = add(30);
    assert_eq!((b.new_version, b.meta.current().unwrap().v), (true, 2));
    assert_eq!(text(&get(&p, "/a/menus/2/img/bg.png")), "BG2");
    assert_eq!(text(&get(&p, "/a/menus/1/img/bg.png")), "BG");
    assert_eq!(text(&get(&p, "/a/menus/2/img/shot%20one@2x.png")), "PNG1");
    let _ = std::fs::remove_dir_all(d);
}

/// art-assets: a folder artifact's root (its frame_url, copy_href with
/// no file) is its index.html, with or without the slash, plain and
/// framed; before site::copy_file it was a 404.
#[test]
fn a_folder_artifacts_root_is_its_index() {
    let (p, d) = pages();
    let site = d.join("work/menus");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.html"), b"<img src=\"a.png\">").unwrap();
    std::fs::write(site.join("a.png"), b"A").unwrap();
    let arts = crate::artifacts::Store::new(&d);
    let a = crate::artifacts::Add { target: site.to_string_lossy().to_string(), title: Some("menus".into()), agent: "main".into(), by: "main".into(), cwd: "/".into(), ..Default::default() };
    let m = arts.add(&a, 10).unwrap().meta;
    let root = crate::pages::site::copy_href(&m.id, 1, m.versions[0].copy.as_deref().unwrap());
    assert_eq!(root, "/a/menus/1/");
    for at in ["/a/menus/1/", "/a/menus/1", "/a/menus/1/framed/"] {
        let r = get(&p, at);
        assert_eq!(r.status, 200, "{at}");
        assert!(text(&r).starts_with(&format!("{}<img src=\"a.png\">", super::super::site::STORAGE_TAG)), "{at}");
    }
    assert_eq!(text(&get(&p, "/a/menus/1/a.png")), "A");
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

/// notif-artifact: a drawn pick is answered on the page like a
/// question (its option's h3 is the answer), and the meta says which
/// ask waits in the inbox and which is answered, where and when.
#[test]
fn a_pick_answers_like_a_question_and_the_meta_says_what_waits() {
    let (p, d) = pages();
    let ui = r#"<section data-kit="ui" data-id="look"><div class="k-pick" data-ui="pick" data-ask="which look?"><div class="k-option" data-pick="a"><h3>A · pink</h3><p>a border.</p></div><div class="k-option" data-pick="b"><h3>B · frame</h3><p>one frame.</p></div></div></section>"#;
    p.publish(&Publish { agent: "t1".into(), id: Some("w".into()), html: ui.into(), ..Default::default() }, 1, &|_| false).unwrap();
    let mut qs = crate::pages::questions::of_fragment(ui);
    qs[0].card = 7;
    p.store.save_questions("w", &qs).unwrap();
    let meta = |p: &Pages| serde_json::from_slice::<Value>(&get(p, "/p/w/meta").body).unwrap()["asks"].clone();
    assert_eq!(meta(&p), json!([{"block": "look", "open": true, "count": 2}]));
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    p.connect_hub(Box::new(move |m| {
        let _ = tx.lock().unwrap().send(m);
    }));
    let (tok, o) = (p.token.clone(), "http://127.0.0.1:47123");
    assert_eq!(post(&p, "/api/p/w/answer", Some(&tok), Some(o), json!({"block": "look", "option": 2})).status, 200);
    assert_eq!(rx.try_recv().unwrap(), PageMsg::Answer { id: "w".into(), block: "look".into(), reply: "B · frame".into() });
    // answered in the inbox (the hub writes it): the page reads where and when
    qs[0].reply = Some("B · frame".into());
    qs[0].via = Some("inbox".into());
    qs[0].at_ms = Some(42);
    p.store.save_questions("w", &qs).unwrap();
    assert_eq!(meta(&p), json!([{"block": "look", "reply": "B · frame", "n": 2, "key": "b", "via": "inbox", "at_ms": 42}]));
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
    let p = std::sync::Arc::new(Pages { holds: holds.clone(), ..Pages::new(&d, port, &d) });
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
    let p = std::sync::Arc::new(Pages::new(&d, port, &d));
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

/// video-view (architect m_32064): a video artifact opens in the viewer
/// (data-stream: the player reads its url, never the bytes whole), and its
/// copy is served by its byte ranges through the real socket: 206 with the
/// part asked, 416 past the end, a HEAD with no body, 200 whole with
/// `Accept-Ranges` when no range is asked; a foreign Host still refused.
#[test]
fn a_video_copy_is_served_by_its_byte_ranges() {
    use std::io::Read;
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let d = std::env::temp_dir().join(format!("sb-srv-video-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("kit")).unwrap();
    std::fs::create_dir_all(d.join("work")).unwrap();
    let bytes: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(d.join("work/clip.mp4"), &bytes).unwrap();
    let arts = crate::artifacts::Store::new(&d);
    let add = crate::artifacts::Add { target: "clip.mp4".into(), agent: "film".into(), by: "film".into(), cwd: d.join("work").to_string_lossy().to_string(), ..Default::default() };
    let id = arts.add(&add, 10).unwrap().meta.id;
    let p = std::sync::Arc::new(Pages::new(&d, port, &d));
    let view = get(&p, &format!("/a/{id}/view"));
    assert_eq!(view.status, 200);
    assert!(text(&view).contains("data-kind=\"video\"") && text(&view).contains(" data-stream "), "{}", text(&view));
    serve(listener, p.clone());
    let ask = |method: &str, host: &str, range: Option<&str>| {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let range = range.map(|r| format!("Range: {r}\r\n")).unwrap_or_default();
        s.write_all(format!("{method} /a/{id}/1/clip.mp4 HTTP/1.1\r\nHost: {host}\r\n{range}\r\n").as_bytes()).unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        (String::from_utf8_lossy(&out[..at]).to_string(), out[at..].to_vec())
    };
    let host = format!("127.0.0.1:{port}");
    let (h, body) = ask("GET", &host, Some("bytes=100-199"));
    assert!(h.starts_with("HTTP/1.1 206 ") && h.contains("Content-Type: video/mp4\r\n") && h.contains("Content-Range: bytes 100-199/1000\r\n"), "{h}");
    assert_eq!(body, bytes[100..200]);
    let (h, body) = ask("GET", &host, Some("bytes=-10"));
    assert!(h.contains("Content-Range: bytes 990-999/1000"), "{h}");
    assert_eq!(body, bytes[990..]);
    let (h, body) = ask("GET", &host, Some("bytes=1000-"));
    assert!(h.starts_with("HTTP/1.1 416 ") && h.contains("Content-Range: bytes */1000"), "{h}");
    assert!(body.is_empty());
    let (h, body) = ask("GET", &host, None);
    assert!(h.starts_with("HTTP/1.1 200 ") && h.contains("Accept-Ranges: bytes") && h.contains(super::super::site::ARTIFACT_CSP), "{h}");
    assert_eq!(body, bytes);
    let (h, body) = ask("HEAD", &host, Some("bytes=0-"));
    assert!(h.starts_with("HTTP/1.1 206 ") && h.contains("Content-Length: 1000"), "{h}");
    assert!(body.is_empty(), "a HEAD: no body");
    let (h, _) = ask("GET", "evil.test", Some("bytes=0-"));
    assert!(h.starts_with("HTTP/1.1 403 "), "another Host: refused as any route: {h}");
    let _ = std::fs::remove_dir_all(d);
}

/// web-storage (architect m_46151): a site's localStorage, kept by the
/// hub. The viewer POSTs it with the token (none: 403); each html page of
/// the copy, plain or framed, any version, comes seeded with it; a '</script'
/// value can't close the seed; non-strings dropped; over 5 MB refused.
#[test]
fn a_sites_storage_is_kept_by_the_hub_and_seeds_every_version() {
    let (p, d) = pages();
    let work = d.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("budget.html"), b"<html><head><script>localStorage</script></head><body>v1</body></html>").unwrap();
    let arts = crate::artifacts::Store::new(&d);
    let add = |at: u64| {
        let a = crate::artifacts::Add { target: "budget.html".into(), title: Some("budget".into()), agent: "main".into(), by: "main".into(), cwd: work.to_string_lossy().to_string(), ..Default::default() };
        arts.add(&a, at).unwrap()
    };
    add(10);
    let (tok, o) = (p.token.clone(), "http://127.0.0.1:47123");
    let seed = |path: &str| {
        let t = text(&get(&p, path));
        let open = format!("<script type=\"application/json\" id=\"{}\">", super::super::site::SEED_ID);
        t.split_once(open.as_str()).map(|(_, rest)| {
            let (json, _) = rest.split_once("</script>").unwrap();
            assert!(rest.contains(super::super::site::STORAGE_TAG), "the shim right after: {t}");
            serde_json::from_str::<Value>(json).unwrap()
        })
    };
    assert_eq!(seed("/a/budget/1/budget.html"), None, "nothing kept: no seed");
    let local = json!({"rent": "</script><script>alert(1)</script>", "n": 3});
    assert_eq!(post(&p, "/api/a/budget/storage", None, Some(o), json!({ "local": local })).status, 403, "no token");
    assert_eq!(post(&p, "/api/a/nope/storage", Some(&tok), Some(o), json!({ "local": {} })).status, 404);
    assert_eq!(post(&p, "/api/a/budget/storage", Some(&tok), Some(o), json!({ "local": "x" })).status, 400);
    let r = post(&p, "/api/a/budget/storage", Some(&tok), Some(o), json!({ "local": local }));
    assert_eq!(r.status, 200, "{}", text(&r));
    let want = json!({"rent": "</script><script>alert(1)</script>"});
    assert_eq!(seed("/a/budget/1/budget.html"), Some(want.clone()), "the plain copy, non-strings dropped");
    assert_eq!(seed("/a/budget/1/framed/budget.html"), Some(want.clone()), "framed");
    // a new version: the same storage, like one origin
    std::fs::write(work.join("budget.html"), b"<html><head></head><body>v2</body></html>").unwrap();
    assert_eq!(add(20).meta.current().unwrap().v, 2);
    assert_eq!(seed("/a/budget/2/framed/budget.html"), Some(want.clone()), "v2 shares it");
    let big = json!({ "local": {"x": "y".repeat(crate::artifacts_storage::MAX_BYTES + 1)} });
    assert_eq!(post(&p, "/api/a/budget/storage", Some(&tok), Some(o), big).status, 413, "over 5 MB");
    assert_eq!(seed("/a/budget/2/budget.html"), Some(want), "kept as it was");
    let _ = std::fs::remove_dir_all(d);
}

/// A view's own origin (docs/mcp-apps.md C6): `v-<hash>.localhost:<port>`
/// answers its proxy page, with the hub's CSP for that view, and refuses
/// every other route; the main host has no `/mcp-app`.
#[test]
fn a_view_host_serves_its_proxy_and_nothing_else() {
    let (p, d) = pages();
    let hash = super::super::views::hash("fake-app", "fake-app", "ui://fake-app/counter");
    let at = |host: &str, method: &str, path: &str| route(&p, &Request { method: method.into(), path: path.into(), headers: vec![("Host".into(), host.into())], body: vec![] });
    let vh = format!("v-{hash}.localhost:{}", p.port);
    assert_eq!(at(&vh, "GET", "/mcp-app").status, 404, "an origin the hub never registered");
    let declared = json!({"connectDomains": ["https://api.open-meteo.com", "*"], "domain": "https://evil.example"});
    let session = p.view_sandbox(&hash, super::super::views::ViewSandbox { csp: declared.clone(), perms: vec!["microphone".into()] });
    assert!(session.len() == 32 && session.bytes().all(|b| b.is_ascii_hexdigit()), "{session}");
    assert_ne!(session, p.view_sandbox(&hash, super::super::views::ViewSandbox { csp: declared.clone(), perms: vec!["microphone".into()] }), "a new session per view/resource");
    let r = at(&vh, "GET", "/mcp-app");
    assert_eq!(r.status, 200);
    assert_eq!(r.csp, super::super::view_csp::csp(&declared, p.port, p.view_dev), "the hub's policy, never the request's");
    assert!(r.csp.contains("connect-src https://api.open-meteo.com;"), "{}", r.csp);
    let t = text(&r);
    assert!(t.contains("sandbox-proxy-ready") && t.contains(r#"const ALLOW = "microphone";"#), "{t}");
    for path in ["/", "/api/p/w/notes", "/p/w", "/kit/kit.css", "/events", "/a/x/1/x.html", "/mcp-app/x", "/v/t1/1-2"] {
        assert_eq!(at(&vh, "GET", path).status, 403, "{path} on a view host");
    }
    assert_eq!(at(&vh, "POST", "/api/p/w/notes").status, 403);
    assert_eq!(at(&vh, "POST", "/mcp-app").status, 405);
    // another port, a sub-subdomain, a bad hash: not this server
    for host in [format!("v-{hash}.localhost:1"), format!("x.v-{hash}.localhost:{}", p.port), format!("v-{}.localhost:{}", &hash[..15], p.port)] {
        assert_eq!(at(&host, "GET", "/mcp-app").status, 403, "{host}");
    }
    // the main host has no proxy
    assert_eq!(get(&p, "/mcp-app").status, 404);
    let _ = std::fs::remove_dir_all(d);
}

/// The web host (docs/mcp-apps.md C9): `GET /v/<agent>/<call>` is the page
/// the views host's script fills, framing only this port's views;
/// `POST /api/view` needs the token and the Origin, and a view's message
/// from a browser is refused every time (no Electron main saw a click).
#[test]
fn the_web_host_serves_its_page_and_refuses_a_browsers_message() {
    let (p, d) = pages();
    let r = get(&p, "/v/t1/1730000000000-3");
    assert_eq!(r.status, 200);
    let t = text(&r);
    assert!(t.contains("<main id=\"bise-view\" data-agent=\"t1\" data-call=\"1730000000000-3\">") && t.contains("/kit/views/web.js"), "{t}");
    assert!(t.contains(&format!("<meta name=\"bise-token\" content=\"{}\">", p.token)));
    assert_eq!(r.csp, super::super::view_web::page_csp(p.port));
    for bad in ["/v/t1/x", "/v/..%2F/1-2", "/v/t1/1-2-3", "/v/t1"] {
        assert_eq!(get(&p, bad).status, 404, "{bad}");
    }
    let (tok, o) = (p.token.clone(), format!("http://127.0.0.1:{}", p.port));
    let msg = json!({"method": "view/message", "params": {"agent": "t1", "call": "1-2", "content": [{"type": "text", "text": "yes do it"}]}});
    assert_eq!(post(&p, "/api/view", None, Some(&o), msg.clone()).status, 403, "no token");
    assert_eq!(post(&p, "/api/view", Some(&tok), Some("http://v-0123456789abcdef.localhost:47123"), msg.clone()).status, 403, "a view's origin");
    let r = post(&p, "/api/view", Some(&tok), Some(&o), msg);
    let v: Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!((r.status, v["error"]["code"].as_i64(), v["error"]["data"]["reason"].as_str()), (200, Some(-32037), Some("no-gesture")), "{v}");
    assert_eq!(post(&p, "/api/view", Some(&tok), Some(&o), json!({"method": "plugin/views", "params": {"agent": "t1", "call": "1-2"}})).status, 400);
    let _ = std::fs::remove_dir_all(d);
}
