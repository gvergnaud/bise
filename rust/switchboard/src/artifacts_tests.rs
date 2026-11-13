//! Laws of the artifact store (docs/artifacts.md): what gets in, how a
//! version is made, the copies, the pages read as they are, the rows.

use super::*;
use serde_json::json;

/// `sb artifact list <words> [--agent <a>]` as the store reads it, days at UTC.
fn list(s: &Store, words: &str, agent: Option<&str>, archived: bool, now: u64) -> String {
    let f = crate::artifacts_filter::of_list(words, agent.into_iter().map(String::from).collect(), vec![], None).unwrap();
    s.list_text(&f, archived, now, &|_| 0, &|_| Default::default())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-artifacts-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn add(store: &Store, cwd: &Path, target: &str, title: Option<&str>, now: u64) -> Result<Added, String> {
    store.add(
        &Add {
            target: target.into(),
            title: title.map(String::from),
            kind: None,
            agent: "pricing-page".into(),
            by: "pricing-page".into(),
            cwd: cwd.to_string_lossy().to_string(),
            known_only: false,
            ..Default::default()
        },
        now,
    )
}

fn nobody(_: &str) -> Option<(String, bool)> {
    None
}

/// A page as the page store writes it (ambient-app's layout).
fn page(state: &Path, id: &str, title: &str, versions: &[(u64, u64)], notes: Value) {
    let d = state.join("pages").join(id);
    std::fs::create_dir_all(&d).unwrap();
    let vs: Vec<Value> = versions.iter().map(|(n, at)| json!({"n": n, "at_ms": at, "blocks": []})).collect();
    std::fs::write(
        d.join("meta.json"),
        json!({"id": id, "title": title, "agent": "ambient-pm", "created_ms": 1, "versions": vs, "state": "ready"}).to_string(),
    )
    .unwrap();
    std::fs::write(d.join("notes.json"), notes.to_string()).unwrap();
}

#[test]
fn a_file_gets_in_with_a_copy_and_its_id() {
    let root = tmp("file");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("pricing-plans.xlsx"), b"v1").unwrap();
    let s = Store::new(&state);
    let a = add(&s, &work, "pricing-plans.xlsx", None, 10).unwrap();
    assert!(a.new_version && !a.page);
    assert_eq!(a.meta.id, "pricing-plans-xlsx");
    assert_eq!(a.meta.kind, "sheet");
    assert_eq!(a.meta.title, "pricing-plans.xlsx");
    let copy = state.join("artifacts/pricing-plans-xlsx").join(a.meta.versions[0].copy.as_ref().unwrap());
    assert_eq!(std::fs::read(copy).unwrap(), b"v1");
    assert_eq!(
        added_text(&a),
        "added pricing-plans.xlsx (sheet) · v1 · link it as [pricing-plans.xlsx](artifact:pricing-plans-xlsx)"
    );
}

#[test]
fn the_same_path_again_is_a_new_version_never_a_second_row_and_unchanged_stays() {
    let root = tmp("versions");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let f = work.join("plan.md");
    std::fs::write(&f, b"one").unwrap();
    let s = Store::new(&state);
    add(&s, &work, "plan.md", Some("the plan"), 10).unwrap();
    // again, unchanged: same version
    let same = add(&s, &work, f.to_str().unwrap(), None, 20).unwrap();
    assert!(!same.new_version);
    assert_eq!(same.meta.versions.len(), 1);
    assert!(added_text(&same).starts_with("the plan is unchanged: still v1"));
    std::fs::write(&f, b"two, longer").unwrap();
    let two = add(&s, &work, "./plan.md", None, 30).unwrap();
    assert!(two.new_version);
    assert_eq!(two.meta.versions.iter().map(|v| v.v).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(s.all().len(), 1);
    // each version keeps its own copy
    let dir = state.join("artifacts").join(&two.meta.id);
    assert_eq!(std::fs::read(dir.join(two.meta.versions[0].copy.as_ref().unwrap())).unwrap(), b"one");
    assert_eq!(std::fs::read(dir.join(two.meta.versions[1].copy.as_ref().unwrap())).unwrap(), b"two, longer");
}

#[test]
fn a_gone_file_still_opens_the_copy() {
    let root = tmp("gone");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("design.md"), b"doc").unwrap();
    let s = Store::new(&state);
    add(&s, &work, "design.md", Some("subscriptions design"), 10).unwrap();
    std::fs::remove_dir_all(&work).unwrap();
    let rows = s.rows(&|_| Some(("subs-lead".into(), true)), "/nowhere");
    assert_eq!(rows[0]["gone"], true);
    assert_eq!(rows[0]["archived"], true);
    assert_eq!(rows[0]["agent"], "subs-lead");
    let copy = rows[0]["copy"].as_str().unwrap();
    assert_eq!(std::fs::read(copy).unwrap(), b"doc");
    assert!(list(&s, "", None, false, 20).contains("▲ gone from disk · bise kept a copy"));
}

#[test]
fn a_file_over_50_mb_has_no_copy_and_says_why() {
    let root = tmp("big");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let f = std::fs::File::create(work.join("film.mp4")).unwrap();
    f.set_len(MAX_COPY + 1).unwrap();
    let s = Store::new(&state);
    let a = add(&s, &work, "film.mp4", Some("launch film"), 10).unwrap();
    assert_eq!(a.meta.kind, "video");
    assert_eq!(a.meta.versions[0].copy, None);
    assert_eq!(a.meta.versions[0].no_copy.as_deref(), Some("over 50 MB"));
    assert!(added_text(&a).contains("no copy kept (over 50 MB)"));
    let rows = s.rows(&nobody, "/");
    assert_eq!(rows[0]["versions"][0]["note"], "no copy: over 50 MB");
}

#[test]
fn a_folder_with_an_index_is_a_site_and_is_copied_whole() {
    let root = tmp("site");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(work.join("capsule/js")).unwrap();
    std::fs::write(work.join("capsule/index.html"), b"<h1>").unwrap();
    std::fs::write(work.join("capsule/js/a.js"), b"x").unwrap();
    let s = Store::new(&state);
    let a = add(&s, &work, "capsule", Some("capsule, round 9"), 10).unwrap();
    assert_eq!(a.meta.kind, "site");
    let copy = state.join("artifacts").join(&a.meta.id).join(a.meta.versions[0].copy.as_ref().unwrap());
    assert!(copy.join("js/a.js").is_file());
    // its first-level names, what A2 matches a folder rule on (architect m_62393), on the row too
    assert_eq!((a.meta.versions[0].names.names.clone(), a.meta.versions[0].names.more), (vec!["index.html".to_string(), "js".into()], false));
    let rows = s.rows(&nobody, "/");
    assert_eq!(rows[0]["names"], json!(["index.html", "js"]));
    // a version from before names (TODO(art-names-read)): its row reads them from the folder
    let mut m = s.get(&a.meta.id).unwrap();
    m.versions[0].names = Default::default();
    s.save(&m).unwrap();
    assert_eq!(s.rows(&nobody, "/")[0]["names"], json!(["index.html", "js"]));
}

#[test]
fn a_folder_keeps_its_first_names_sorted_and_says_when_cut() {
    use bise_proto::files::Names;
    let names = |v: &[&str]| folder_names(v.iter().map(|s| s.to_string()));
    assert_eq!(names(&["js", ".git", "index.html", "", ".DS_Store", "js", "a.md~"]), Names { names: vec!["index.html".to_string(), "js".into()], more: false });
    let many: Vec<String> = (0..Names::MAX + 3).map(|i| format!("f{i:03}")).collect();
    let Names { names: kept, more } = folder_names(many.iter().rev().cloned());
    assert_eq!((kept.len(), more, kept[0].as_str(), kept[Names::MAX - 1].clone()), (Names::MAX, true, "f000", many[Names::MAX - 1].clone()));
    assert!(!folder_names((0..Names::MAX).map(|i| format!("f{i}"))).more, "exactly the cap: whole");
    assert_eq!(names_of(Path::new("/no/such/folder")), Names::default());
}

#[test]
fn links_get_their_kind_and_a_pr_its_number() {
    let root = tmp("links");
    let s = Store::new(&root.join("state"));
    let pr = add(&s, &root, "https://github.com/acme/web/pull/6", None, 10).unwrap();
    assert_eq!((pr.meta.kind.as_str(), pr.meta.title.as_str(), pr.meta.id.as_str()), ("pr", "PR #6", "pr-6"));
    let mock = add(&s, &root, "bise.dev/m/artifacts", Some("the artifacts mock"), 11).unwrap();
    assert_eq!(mock.meta.kind, "page");
    assert_eq!(mock.meta.versions[0].target, "https://bise.dev/m/artifacts");
    assert_eq!(link_kind("http://127.0.0.1:4747/"), "site");
    assert_eq!(link_kind("https://github.com/acme/web/releases/tag/v2"), "release");
    assert_eq!(link_kind("https://docs.google.com/spreadsheets/d/x"), "sheet");
    assert_eq!(link_kind("https://example.com/a"), "link");
    let rows = s.rows(&nobody, "/");
    let pr_row = rows.iter().find(|r| r["id"] == "pr-6").unwrap();
    assert_eq!(pr_row["pr"], json!({"repo": "acme/web", "number": 6, "tag": "#6"}));
    let mock_row = rows.iter().find(|r| r["id"] == "the-artifacts-mock").unwrap();
    let keys: Vec<&str> = mock_row["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap()).collect();
    assert!(keys.contains(&"bise.dev/m/artifacts") && keys.contains(&"https://bise.dev/m/artifacts"), "{keys:?}");
    // the same link again: a new version of the same row
    let again = add(&s, &root, "https://bise.dev/m/artifacts/", None, 12).unwrap();
    assert_eq!((again.meta.id.as_str(), again.meta.versions.len()), ("the-artifacts-mock", 2));
}

/// pr-integration rules 1 and 3 (architect m_54254): a PR row's status
/// is the hub's row of that same PR (one PR, never two: its repo and
/// number are the row's), joined by the link's one form; none when the
/// hub does not follow it, and a status gone stays gone.
#[test]
fn a_pr_row_carries_the_hub_s_status_of_that_pr() {
    let root = tmp("pr-status");
    let s = Store::new(&root.join("state"));
    add(&s, &root, "https://github.com/Acme/Web/pull/6/files", None, 10).unwrap();
    add(&s, &root, "https://github.com/acme/web/pull/7", None, 11).unwrap();
    let mut rows = s.rows(&nobody, "/");
    let status = bise_proto::rows::Pr {
        number: 6,
        url: "https://github.com/acme/web/pull/6".into(),
        repo: "acme/web".into(),
        words: "approved · checks ✓ 12/12 · your merge".into(),
        ..Default::default()
    };
    let by_url = BTreeMap::from([("https://github.com/acme/web/pull/6".to_string(), status)]);
    join_pr_status(&mut rows, &by_url);
    let row = |rows: &[Value], n: u64| rows.iter().find(|r| r["pr"]["number"] == n).unwrap().clone();
    let six = row(&rows, 6);
    assert_eq!(six["pr"]["repo"], "acme/web");
    assert_eq!((&six["pr"]["status"]["repo"], &six["pr"]["status"]["number"]), (&six["pr"]["repo"], &six["pr"]["number"]));
    assert_eq!(six["pr"]["status"]["words"], "approved · checks ✓ 12/12 · your merge");
    // not followed: no status, never a made-up one
    assert!(row(&rows, 7)["pr"].get("status").is_none());
    // the hub stops following #6: its status goes
    join_pr_status(&mut rows, &BTreeMap::new());
    assert!(row(&rows, 6)["pr"].get("status").is_none());
}

#[test]
fn nothing_gets_in_without_a_file_or_link() {
    let root = tmp("none");
    let s = Store::new(&root.join("state"));
    assert_eq!(add(&s, &root, "notes/plan.md", None, 1).unwrap_err(), "no file or link at notes/plan.md.");
    assert!(s.all().is_empty());
    let bad = s.add(&Add { target: "https://x.io/a".into(), kind: Some("movie".into()), ..Default::default() }, 1);
    assert!(bad.unwrap_err().starts_with("unknown kind movie"));
}

#[test]
fn pages_get_in_by_themselves_with_versions_and_notes() {
    let root = tmp("pages");
    let state = root.join("state");
    page(&state, "pricing-page", "pricing page", &[(1, 100), (2, 200), (3, 300)], json!([
        {"id": "n1", "version": 3, "status": "draft"},
        {"id": "n2", "version": 3, "status": "sent"},
        {"id": "n3", "version": 2, "status": "done"},
        {"id": "n4", "version": 2, "status": "answered"},
        {"id": "n5", "version": 2, "status": "done"},
    ]));
    std::fs::write(state.join("pages.port"), "47438\n").unwrap();
    let s = Store::new(&state);
    let rows = s.rows(&nobody, "/");
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!((r["id"].as_str(), r["kind"].as_str(), r["v"].as_u64(), r["by"].as_str()), (Some("pricing-page"), Some("page"), Some(3), Some("page")));
    assert_eq!(r["target"], "http://127.0.0.1:47438/p/pricing-page");
    assert_eq!(r["detail"], "2 notes open");
    assert_eq!(r["versions"][1]["note"], "3 notes done");
    assert_eq!(r["versions"][1]["target"], "http://127.0.0.1:47438/p/pricing-page/v/2");
    assert_eq!(r["gone"], false);
    // adding the page's link names the page, stores nothing
    let a = add(&s, &root, "http://127.0.0.1:47438/p/pricing-page", None, 400).unwrap();
    assert!(a.page);
    assert_eq!(added_text(&a), "pricing-page is a bise page (v3): it's in already. link it as [pricing page](artifact:pricing-page)");
    assert!(s.stored().is_empty());
    // a stored artifact never takes a page's id
    std::fs::write(root.join("pricing page"), b"x").unwrap();
    let b = add(&s, &root, "pricing page", None, 500).unwrap();
    assert_eq!(b.meta.id, "pricing-page-2");
}

#[test]
fn a_page_still_being_written_or_without_a_port_is_handled() {
    let root = tmp("page-writing");
    let state = root.join("state");
    page(&state, "draft", "draft", &[], json!([]));
    page(&state, "weekly", "weekly update", &[(1, 5)], json!("not a list"));
    let s = Store::new(&state);
    let rows = s.rows(&nobody, "/");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["target"], "page:weekly");
    assert_eq!(rows[0]["detail"], "");
}

#[test]
fn the_list_is_newest_first_and_new_counts_after_seen() {
    let root = tmp("order");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let s = Store::new(&state);
    assert_eq!(s.seen_ms(50), 50);
    for (i, n) in ["a.md", "b.md", "c.md"].iter().enumerate() {
        std::fs::write(work.join(n), n.as_bytes()).unwrap();
        add(&s, &work, n, None, 40 + i as u64 * 10).unwrap();
    }
    let ids: Vec<String> = s.all().iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids, vec!["c-md", "b-md", "a-md"]);
    // seen at 50: only c (60) came after
    assert_eq!(s.new_count(100), 1);
    s.set_seen(100).unwrap();
    assert_eq!(s.new_count(100), 0);
    // what the user added himself is never "new"
    std::fs::write(work.join("d.md"), b"d").unwrap();
    s.add(&Add { target: "d.md".into(), by: "you".into(), agent: "main".into(), cwd: work.to_string_lossy().to_string(), ..Default::default() }, 200).unwrap();
    assert_eq!(s.new_count(300), 0);
}

#[test]
fn a_late_seen_keeps_what_came_after_the_look() {
    // the TUI looked at 50; its `seen` waited in the socket (a hub
    // still booting) and the hub reads it at 120, after two adds
    let root = tmp("late-seen");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let s = Store::new(&state);
    s.set_seen(40).unwrap();
    for (i, n) in ["a.md", "b.md"].iter().enumerate() {
        std::fs::write(work.join(n), n.as_bytes()).unwrap();
        add(&s, &work, n, None, 100 + i as u64 * 10).unwrap();
    }
    s.saw(50, 120).unwrap();
    assert_eq!(s.seen_ms(0), 50);
    assert_eq!(s.new_count(120), 2, "both came after the look");
    // never back: an older look changes nothing
    s.saw(30, 130).unwrap();
    assert_eq!(s.seen_ms(0), 50);
    // never past now: a client clock ahead is cut to the hub's
    s.saw(500, 140).unwrap();
    assert_eq!(s.seen_ms(0), 140);
    assert_eq!(s.new_count(140), 0);
}

#[test]
fn keys_name_a_path_every_way_a_reply_may() {
    let root = tmp("keys");
    let ws = root.join("ws");
    std::fs::create_dir_all(ws.join("docs/plans")).unwrap();
    std::fs::write(ws.join("docs/plans/q3.xlsx"), b"x").unwrap();
    let s = Store::new(&root.join("state"));
    let a = add(&s, &ws.join("docs"), "plans/q3.xlsx", None, 1).unwrap();
    let k = keys(&a.meta, ws.to_str().unwrap());
    assert!(k.contains(&"plans/q3.xlsx".to_string()), "{k:?}");
    assert!(k.contains(&"docs/plans/q3.xlsx".to_string()), "{k:?}");
    assert!(k.iter().any(|x| x.ends_with("/ws/docs/plans/q3.xlsx") && x.starts_with('/')), "{k:?}");
}

#[test]
fn the_reply_link_form_parses() {
    assert_eq!(parse_ref("artifact:pricing-page"), Some(("pricing-page".into(), None)));
    assert_eq!(parse_ref("artifact:artifacts@v3"), Some(("artifacts".into(), Some(3))));
    assert_eq!(parse_ref("artifact:Bad Id"), None);
    assert_eq!(parse_ref("artifact:x@vx"), None);
    assert_eq!(parse_ref("https://x"), None);
    assert_eq!(slug("Capsule, round 9!"), "capsule-round-9");
    assert_eq!(slug("···"), "artifact");
}

#[test]
fn the_thread_record_keeps_its_fields_whole() {
    let m = Meta {
        id: "x".into(),
        agent: "a".into(),
        title: "plan : v2".into(),
        kind: "doc".into(),
        versions: vec![Version { v: 2, ..Default::default() }],
        ..Default::default()
    };
    let line = thread_rec(&m).line();
    assert_eq!(line, r#"sb artifact :: {"agent":"a","id":"x","kind":"doc","title":"plan : v2","version":2}"#);
    let read = bise_proto::thread::lines::read(&line);
    assert!(matches!(read, bise_proto::thread::lines::Rec::Hub(bise_proto::thread::lines::Hub::Artifact { title, v: 2, .. }) if title == "plan : v2"));
}

#[test]
fn list_text_filters_by_words_and_agent() {
    let root = tmp("listtext");
    let s = Store::new(&root.join("state"));
    assert!(list(&s, "", None, false, 1).starts_with("no artifacts yet."));
    add(&s, &root, "https://github.com/acme/web/pull/6", Some("gateway head checks"), 10).unwrap();
    let t = list(&s, "gateway", None, false, 20);
    assert!(t.starts_with("[gateway head checks](artifact:gateway-head-checks) · pr · v1 · pricing-page"), "{t}");
    assert_eq!(list(&s, "deck", None, false, 20), "nothing matches.");
    assert_eq!(list(&s, "", Some("launch"), false, 20), "nothing matches.");
}

/// The typed `artifacts` (proto_view::artifacts, the window's and, from
/// client-protocol step 4, the terminal's) carries every field of the
/// store's rows (the older `artifacts` event): each key of a row is a
/// typed field with the same value (`ts_ms` is `at_ms`, `v` `version`;
/// an empty one may be left out), each version's too, and the items it
/// marks `new` are the event's `new` count.
#[test]
fn the_typed_artifacts_carry_everything_the_older_rows_did() {
    let root = tmp("typed");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("plan.md"), b"v1").unwrap();
    std::fs::write(work.join("gone.md"), b"x").unwrap();
    let s = Store::new(&state);
    add(&s, &work, "plan.md", Some("q3 plan"), 10).unwrap();
    std::fs::write(work.join("plan.md"), b"v2, longer").unwrap();
    add(&s, &work, "plan.md", None, 20).unwrap();
    add(&s, &work, "gone.md", None, 30).unwrap();
    std::fs::remove_file(work.join("gone.md")).unwrap();
    add(&s, &root, "https://github.com/acme/web/pull/6", None, 40).unwrap();
    add(&s, &root, "bise.dev/m/artifacts", Some("the artifacts mock"), 50).unwrap();
    let rows = s.rows(&|a: &str| Some((a.to_string(), a == "pricing-page")), work.to_str().unwrap());
    let ev = json!({"ev": "artifacts", "rows": rows, "new": s.new_count(25), "seen_ms": 25});
    let items = crate::proto_view::artifacts(&ev, |_| None, None, |_| Default::default());
    assert_eq!(items.len(), rows.len());
    // absent, null, "", false and [] are the same: nothing to say
    let some = |v: Option<&Value>| v.filter(|v| !(v.is_null() || *v == "" || *v == false || v.as_array().is_some_and(Vec::is_empty))).cloned();
    fn typed_key(k: &str) -> &str {
        match k {
            "ts_ms" => "at_ms",
            "v" => "version",
            k => k,
        }
    }
    for (row, item) in rows.iter().zip(&items) {
        let t = serde_json::to_value(item).unwrap();
        for (k, v) in row.as_object().unwrap() {
            if k == "versions" {
                for (rv, tv) in v.as_array().unwrap().iter().zip(t["versions"].as_array().unwrap()) {
                    for (vk, vv) in rv.as_object().unwrap() {
                        let tk = if vk == "ts_ms" { "at_ms" } else { vk.as_str() };
                        assert_eq!(some(Some(vv)), some(tv.get(tk)), "{} version {vk}", row["id"]);
                    }
                }
                continue;
            }
            assert_eq!(some(Some(v)), some(t.get(typed_key(k))), "{}: {k}", row["id"]);
        }
    }
    assert!(items.iter().any(|a| a.gone && a.archived), "a gone file, its agent archived");
    assert!(items.iter().any(|a| a.pr.as_ref().is_some_and(|p| p.number == 6)), "a PR's number");
    assert_eq!(items.iter().filter(|a| a.new).count() as u64, ev["new"].as_u64().unwrap(), "the new count");
}

/// ambient-lead m_19544, architect m_19561: a copied file's row is framed
/// by the page server at its own link (site::copy_href; every type opens in
/// its plugin's view, docs/plugin-apps.md steps 1-5); a link and a bise page
/// get no frame_url, and none without a server. `url` keeps its meaning
/// (the terminal opens it).
#[test]
fn a_copied_file_is_framed_by_the_page_server_and_a_link_is_not() {
    let root = tmp("frame");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("report.html"), b"<h1>v1</h1>").unwrap();
    let s = Store::new(&state);
    add(&s, &work, "report.html", Some("the report"), 10).unwrap();
    std::fs::write(work.join("report.html"), b"<h1>v2, longer</h1>").unwrap();
    add(&s, &work, "report.html", None, 20).unwrap();
    std::fs::write(work.join("notes.txt"), b"plain words").unwrap();
    add(&s, &work, "notes.txt", Some("the notes"), 25).unwrap();
    add(&s, &work, "https://bise.dev/docs", None, 30).unwrap();
    page(&state, "gate-index", "gate index", &[(1, 40)], json!([]));
    let rows = s.rows(&|a: &str| Some((a.to_string(), false)), work.to_str().unwrap());
    let ev = json!({"ev": "artifacts", "rows": rows, "seen_ms": 0});
    let items = crate::proto_view::artifacts(&ev, |_| None, Some("http://127.0.0.1:47123"), |_| Default::default());
    let by_id = |id: &str| items.iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} in {items:?}"));
    let file = by_id("the-report");
    assert_eq!(file.frame_url.as_deref(), Some("http://127.0.0.1:47123/a/the-report/2/report.html"), "a web page: its newest copy's link (Sites opens it)");
    assert_eq!(file.url, file.copy.clone().unwrap(), "url keeps its meaning");
    let plain = by_id("the-notes");
    assert_eq!(plain.frame_url.as_deref(), Some("http://127.0.0.1:47123/a/the-notes/1/notes.txt"), "a plain file: its own link");
    for a in items.iter().filter(|a| a.id != "the-report" && a.id != "the-notes") {
        assert_eq!(a.frame_url, None, "{} ({}) has no frame_url", a.id, a.by);
    }
    let bare = crate::proto_view::artifacts(&ev, |_| None, None, |_| Default::default());
    assert!(bare.iter().all(|a| a.frame_url.is_none()), "no page server, no frame_url");
}

/// Two versions of a file, a link, and a bise page: the archive's world.
fn archive_world(name: &str) -> (PathBuf, Store) {
    let root = tmp(name);
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let f = work.join("shot.png");
    std::fs::write(&f, b"old build").unwrap();
    let s = Store::new(&state);
    add(&s, &work, "shot.png", Some("old shot"), 10).unwrap();
    std::fs::write(&f, b"newest build, longer").unwrap();
    add(&s, &work, "shot.png", None, 20).unwrap();
    add(&s, &work, "https://bise.dev/docs", None, 30).unwrap();
    page(&state, "gate-index", "gate index", &[(1, 40)], json!([]));
    (state, s)
}

#[test]
fn archive_takes_it_out_of_the_list_and_keeps_only_the_newest_copy() {
    let (state, s) = archive_world("archive");
    let done = s.archive(&Pick { ids: vec!["old-shot".into(), "nope".into()], ..Default::default() }, 50).unwrap();
    assert_eq!(done.ids, vec!["old-shot".to_string()]);
    assert_eq!(done.unknown, vec!["nope".to_string()]);
    // the old version's copy is gone and counted; the newest still opens
    assert_eq!(done.freed, b"old build".len() as u64);
    let dir = state.join("artifacts/old-shot");
    assert!(!dir.join("v1").exists());
    let m = s.get("old-shot").unwrap();
    assert_eq!(m.versions[0].copy, None);
    assert_eq!(m.versions[0].no_copy.as_deref(), Some("archived"));
    assert_eq!(std::fs::read(dir.join(m.versions[1].copy.as_ref().unwrap())).unwrap(), b"newest build, longer");
    // the row says when; the list and the new count leave it out
    let rows = s.rows(&nobody, "/w");
    let row = rows.iter().find(|r| r["id"] == "old-shot").unwrap();
    assert_eq!(row["archived_at"], 50);
    assert_eq!(rows.iter().find(|r| r["id"] == "bise-dev-docs").unwrap()["archived_at"], Value::Null);
    let t = list(&s, "", None, false, 60);
    assert!(!t.contains("artifact:old-shot"), "{}", t);
    assert!(t.ends_with("1 archived · sb artifact list --archived"), "{}", t);
    assert!(list(&s, "", None, true, 60).contains("artifact:old-shot"));
    s.set_seen(0).unwrap();
    assert_eq!(s.new_count(60), 2);
    assert!(archived_text(&done).starts_with("archived old-shot · 1 KB freed"));
}

#[test]
fn unarchive_brings_the_row_back_and_archiving_again_keeps_its_time() {
    let (_state, s) = archive_world("unarchive");
    let pick = Pick { ids: vec!["old-shot".into()], ..Default::default() };
    s.archive(&pick, 50).unwrap();
    let again = s.archive(&pick, 70).unwrap();
    assert_eq!(again.freed, 0);
    assert_eq!(s.archived().get("old-shot"), Some(&50));
    let (back, not) = s.unarchive(&Pick { ids: vec!["old-shot".into(), "bise-dev-docs".into()], ..Default::default() }).unwrap();
    assert_eq!(back, vec!["old-shot".to_string()]);
    assert_eq!(not, vec!["bise-dev-docs".to_string()]);
    assert!(s.archived().is_empty());
    assert!(list(&s, "", None, false, 60).contains("artifact:old-shot"));
    // the old copy stays gone: only the row comes back
    assert_eq!(s.get("old-shot").unwrap().versions[0].no_copy.as_deref(), Some("archived"));
    assert_eq!(unarchived_text(&back, &not), "old-shot is back in the list\nnot archived: bise-dev-docs.");
}

#[test]
fn archive_picks_by_agent_before_and_kind_and_a_page_archives_too() {
    let (_state, s) = archive_world("pick");
    // an empty pick never archives everything
    assert!(!Pick::default().fits(&s.get("old-shot").unwrap()));
    assert!(s.archive(&Pick::default(), 50).unwrap().ids.is_empty());
    let before = |ms| Pick { before_ms: Some(ms), ..Default::default() };
    let ids = |p: &Pick| s.picked(p).into_iter().map(|m| m.id).collect::<Vec<_>>();
    assert_eq!(ids(&before(25)), vec!["old-shot".to_string()]);
    assert_eq!(ids(&Pick { kind: Some("link".into()), ..before(35) }), vec!["bise-dev-docs".to_string()]);
    assert_eq!(ids(&Pick { agent: Some("ambient-pm".into()), ..Default::default() }), vec!["gate-index".to_string()]);
    // a bise page: out of the list, its store untouched
    let done = s.archive(&Pick { ids: vec!["gate-index".into()], ..Default::default() }, 50).unwrap();
    assert_eq!(done.ids, vec!["gate-index".to_string()]);
    assert!(!list(&s, "", None, false, 60).contains("gate-index"));
    assert!(s.get("gate-index").is_some());
}

#[test]
fn before_reads_a_day_or_a_day_and_time_in_utc() {
    assert_eq!(parse_before("1970-01-02"), Some(86_400_000));
    assert_eq!(parse_before("1970-01-01T01:30"), Some(5_400_000));
    assert_eq!(parse_before("2026-10-10T14:00"), Some(1_791_640_800_000));
    assert_eq!(parse_before("yesterday"), None);
    assert_eq!(parse_before("2026-10-10T25:00"), None);
}

#[test]
fn an_archived_older_version_says_not_kept() {
    let (_state, s) = archive_world("notkept");
    s.archive(&Pick { ids: vec!["old-shot".into()], ..Default::default() }, 50).unwrap();
    let rows = s.rows(&nobody, "/w");
    let row = rows.iter().find(|r| r["id"] == "old-shot").unwrap();
    assert_eq!(row["versions"][0]["note"], "not kept");
    assert_eq!(row["versions"][0]["copy"], Value::Null);
    assert_eq!(row["versions"][1]["note"], "");
}

/// `run.write` without `{ artifact: true }` (architect m_23785): a
/// `known_only` add of a file that is no artifact stores nothing and says
/// so; of a file that is one, it is its next version like any add.
#[test]
fn a_known_only_add_is_only_a_next_version() {
    let state = tmp("known-only");
    let work = state.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let store = Store::new(&state);
    std::fs::write(work.join("q3.pptx"), b"v1").unwrap();
    let known = |now| {
        store.add(&Add { target: "q3.pptx".into(), agent: "deck".into(), by: "deck".into(), cwd: work.to_string_lossy().to_string(), known_only: true, ..Default::default() }, now)
    };
    let a = known(10).unwrap();
    assert!(a.not_artifact && !a.new_version);
    assert!(store.stored().is_empty(), "nothing stored");
    assert_eq!(added_text(&a), format!("{} is not an artifact: written only", work.join("q3.pptx").display()));
    // added once for real: the next known_only add is its v2
    add(&store, &work, "q3.pptx", None, 20).unwrap();
    std::fs::write(work.join("q3.pptx"), b"v2, longer").unwrap();
    let a = known(30).unwrap();
    assert!(!a.not_artifact && a.new_version);
    assert_eq!(a.meta.current().unwrap().v, 2);
    assert_eq!(store.stored().len(), 1);
}

/// art-assets (architect m_25093): an html file's caps count the files it
/// links too; a set over them copies nothing (never a half set) and says why.
#[test]
fn an_html_file_and_its_files_over_50_mb_have_no_copy() {
    let root = tmp("html-big");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("deck.html"), r#"<img src="a.png"><img src="b.png">"#).unwrap();
    for f in ["a.png", "b.png"] {
        std::fs::File::create(work.join(f)).unwrap().set_len(MAX_COPY / 2 + 1).unwrap();
    }
    let s = Store::new(&state);
    let a = add(&s, &work, "deck.html", Some("deck"), 10).unwrap();
    assert_eq!((a.meta.versions[0].copy.as_deref(), a.meta.versions[0].no_copy.as_deref()), (None, Some("over 50 MB")));
    assert_eq!(added_text(&a), "added deck (page) · v1 · no copy kept (over 50 MB) · link it as [deck](artifact:deck)");
    assert!(!state.join("artifacts/deck/v1").exists(), "nothing copied");
}

/// His edits in the viewer (xlsx-univer, architect m_37328): one version
/// per sitting. The same sitting again replaces its version (same v, new
/// bytes, rev grows); a new sitting, or a sitting after an agent's
/// version or after another sitting, is v+1.
#[test]
fn his_saves_in_one_sitting_make_one_version() {
    let root = tmp("sitting");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let f = work.join("q3.xlsx");
    let s = Store::new(&state);
    std::fs::write(&f, b"PK agent v1").unwrap();
    add(&s, &work, "q3.xlsx", None, 10).unwrap();
    let his = |bytes: &[u8], session: &str, seq: u64, at: u64| {
        std::fs::write(&f, bytes).unwrap();
        let sitting = Some(crate::artifacts_turns::Sitting { by: "you".into(), session: session.into(), seq });
        s.add(&Add { target: f.to_string_lossy().to_string(), agent: "pricing-page".into(), by: "you".into(), cwd: work.to_string_lossy().to_string(), known_only: true, sitting, ..Default::default() }, at).unwrap()
    };
    let copy = |a: &Added| s.copy_bytes(&a.meta, a.meta.current().unwrap()).unwrap();
    let a = his(b"PK his edit 1", "s1", 1, 20);
    assert_eq!((a.meta.current().unwrap().v, a.replaced), (2, false), "his first save: a new version over the agent's");
    let b = his(b"PK his edit 1 and 2", "s1", 2, 30);
    let v = b.meta.current().unwrap();
    assert_eq!((v.v, v.rev(), b.replaced, v.sitting.as_ref().unwrap().seq), (2, 2, true, 2), "the same sitting: the same version, its rev grew");
    assert_eq!(copy(&b), b"PK his edit 1 and 2", "with the new bytes");
    let c = his(b"PK another sitting", "s2", 1, 40);
    assert_eq!((c.meta.current().unwrap().v, c.replaced), (3, false), "a new sitting");
    let d = his(b"PK back to the old sitting", "s1", 3, 50);
    assert_eq!(d.meta.current().unwrap().v, 4, "an older sitting after another one saved");
    // the agent saves (no sitting), then his sitting s1 again: a new version
    std::fs::write(&f, b"PK agent again").unwrap();
    assert_eq!(add(&s, &work, "q3.xlsx", None, 60).unwrap().meta.current().unwrap().v, 5);
    assert_eq!(his(b"PK s1 after the agent", "s1", 4, 70).meta.current().unwrap().v, 6, "the newest by the agent: v+1 even with his sitting");
}

fn filed(store: &Store, cwd: &Path, target: &str, folder: Option<Filed>, now: u64) -> Added {
    let a = Add { target: target.into(), agent: "s1".into(), by: "s1".into(), cwd: cwd.to_string_lossy().to_string(), folder, ..Default::default() };
    store.add(&a, now).unwrap()
}

#[test]
fn law_a_default_folder_files_only_a_new_artifact() {
    let root = tmp("filed");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let store = Store::new(&state);
    std::fs::write(work.join("plan.md"), "a").unwrap();
    let a = filed(&store, &work, "plan.md", Some(Filed::Default("f-task".into())), 10);
    assert_eq!(a.meta.folder.as_deref(), Some("f-task"));
    // he moved it; the agent's next version keeps his place
    store.move_to(std::slice::from_ref(&a.meta.id), Some("f-his")).unwrap();
    std::fs::write(work.join("plan.md"), "ab").unwrap();
    let b = filed(&store, &work, "plan.md", Some(Filed::Default("f-task".into())), 20);
    assert_eq!((b.meta.versions.len(), b.meta.folder.as_deref()), (2, Some("f-his")));
    // an asked folder moves it, a next version too
    std::fs::write(work.join("plan.md"), "abc").unwrap();
    let c = filed(&store, &work, "plan.md", Some(Filed::Asked("f-asked".into())), 30);
    assert_eq!(c.meta.folder.as_deref(), Some("f-asked"));
    // none asked: it stays
    std::fs::write(work.join("plan.md"), "abcd").unwrap();
    assert_eq!(filed(&store, &work, "plan.md", None, 40).meta.folder.as_deref(), Some("f-asked"));
}

#[test]
fn law_move_is_one_folder_per_artifact_all_or_none() {
    let root = tmp("move");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let store = Store::new(&state);
    std::fs::write(work.join("a.md"), "a").unwrap();
    let a = filed(&store, &work, "a.md", Some(Filed::Asked("f-1".into())), 10).meta.id;
    page(&state, "pricing", "pricing", &[(1, 5)], json!([]));
    // an unknown id: nothing moves
    assert_eq!(store.move_to(&[a.clone(), "nope".into()], Some("f-2")), Err("no artifact nope".into()));
    assert_eq!(store.get(&a).unwrap().folder.as_deref(), Some("f-1"));
    // a stored one and a page together; each one's folder before comes back
    let moved = store.move_to(&[a.clone(), "pricing".into()], Some("f-2")).unwrap();
    assert_eq!(moved, vec![(a.clone(), Some("f-1".into())), ("pricing".into(), None)]);
    assert_eq!(store.get(&a).unwrap().folder.as_deref(), Some("f-2"), "the move replaced its one folder");
    assert_eq!(store.get("pricing").unwrap().folder.as_deref(), Some("f-2"), "a page's folder is in filed.json");
    let page_meta = std::fs::read_to_string(state.join("pages/pricing/meta.json")).unwrap();
    assert!(!page_meta.contains("f-2"), "the page store's file is not touched");
    store.move_to(&["pricing".into()], None).unwrap();
    assert_eq!(store.get("pricing").unwrap().folder, None);
}

#[test]
fn law_rows_read_the_folder_through_the_library() {
    use bise_home::library::{self, Library, YOU};
    let root = tmp("join");
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let mut lib = Library::default();
    let docs = library::make(&mut lib, "docs", None, YOU).unwrap().folder().id.clone();
    let task = library::make(&mut lib, "pricing", Some(&docs), "main").unwrap().folder().id.clone();
    let shots = library::make(&mut lib, "shots", None, YOU).unwrap().folder().id.clone();
    let store = Store::new(&state);
    for (f, folder) in [("a.md", &task), ("b.md", &shots)] {
        std::fs::write(work.join(f), f).unwrap();
        filed(&store, &work, f, Some(Filed::Asked(folder.clone())), 10);
    }
    let folder_of = |lib: &Library, title: &str| {
        let rows = rows_of(&state, "", lib);
        rows.iter().find(|r| r["title"] == title).unwrap()["folder"].as_str().map(String::from)
    };
    assert_eq!(folder_of(&lib, "a.md"), Some(task.clone()));
    // merge: read as the folder it went into, no meta rewritten
    let before = std::fs::read_to_string(state.join("artifacts/a-md/meta.json")).unwrap();
    library::merge(&mut lib, &task, &shots).unwrap();
    assert_eq!(folder_of(&lib, "a.md"), Some(shots.clone()));
    // delete: unfiled, the artifact still listed, no meta rewritten
    library::delete(&mut lib, &shots).unwrap();
    assert_eq!((folder_of(&lib, "a.md"), folder_of(&lib, "b.md")), (None, None));
    assert_eq!(rows_of(&state, "", &lib).len(), 2, "delete never deletes an artifact");
    assert_eq!(std::fs::read_to_string(state.join("artifacts/a-md/meta.json")).unwrap(), before);
}

/// Who saved a version (docs/plugin-apps.md A6b's save line): his sitting
/// says `you`, else its turn's agent; a version from before both: none.
#[test]
fn a_version_says_who_saved_it() {
    let turn = Some(crate::artifacts_turns::Turn { agent: "perf".into(), started_ms: 1 });
    let sitting = Some(crate::artifacts_turns::Sitting { by: "you".into(), session: "s".into(), seq: 1 });
    assert_eq!(Version { turn: turn.clone(), ..Default::default() }.saved_by(), Some(SavedBy::Agent { name: "perf".into() }));
    assert_eq!(Version { sitting, turn, ..Default::default() }.saved_by(), Some(SavedBy::You));
    assert_eq!(Version::default().saved_by(), None);
}

// his pick on page artifacts-signal (artifacts_marks): audience, kept, opens, sections

fn add_as(store: &Store, work: &Path, file: &str, title: &str, by: &str, for_you: bool, now: u64) -> Added {
    std::fs::write(work.join(file), format!("{file}@{now}")).unwrap();
    let a = Add { target: file.into(), title: Some(title.into()), agent: by.into(), by: by.into(), cwd: work.to_string_lossy().to_string(), for_you, ..Default::default() };
    store.add(&a, now).unwrap()
}

fn marks_world(name: &str) -> (PathBuf, PathBuf, Store) {
    let root = tmp(name);
    let (state, work) = (root.join("state"), root.join("work"));
    std::fs::create_dir_all(&work).unwrap();
    let s = Store::new(&state);
    (state, work, s)
}

fn sections(s: &Store, who: Who, waiting: &[&str]) -> BTreeMap<String, (String, bool)> {
    let mut rows = s.rows(who, "/");
    join_sections(&mut rows, &waiting.iter().map(|w| w.to_string()).collect());
    rows.iter().map(|r| (r["id"].as_str().unwrap().to_string(), (r["section"].as_str().unwrap().to_string(), r["done"].as_bool().unwrap()))).collect()
}

#[test]
fn law_an_add_sets_its_audience_and_for_you_only_raises_it() {
    let (state, work, s) = marks_world("audience");
    let shot = add_as(&s, &work, "a.png", "gate shots", "ui-review", false, 1);
    assert_eq!(shot.meta.audience, Some(Audience::Agents));
    let mail = add_as(&s, &work, "m.md", "mail to Benjamin", "writer", true, 2);
    assert_eq!(mail.meta.audience, Some(Audience::You));
    // its next version without --for-you stays for him; --for-you raises the shot
    assert_eq!(add_as(&s, &work, "m.md", "mail to Benjamin", "writer", false, 3).meta.audience, Some(Audience::You));
    assert_eq!(add_as(&s, &work, "a.png", "gate shots", "ui-review", true, 4).meta.audience, Some(Audience::You));
    // his own add is for him; a page always
    let his = add_as(&s, &work, "h.md", "his", "you", false, 5);
    assert_eq!(his.meta.audience, Some(Audience::You));
    page(&state, "plan", "the plan", &[(1, 6)], json!([]));
    let rows = s.rows(&nobody, "/");
    assert!(rows.iter().all(|r| r["audience"] == "you"), "{rows:?}");
}

#[test]
fn law_a_keep_is_all_or_none_and_rows_say_who_kept_it() {
    let (_state, work, s) = marks_world("keeprows");
    add_as(&s, &work, "n.md", "learnings notebook", "learnings", true, 1);
    let err = s.keep_t(&["learnings-notebook".into(), "nope".into()], artifacts_marks::Who::Agent("learnings"), true, 5).unwrap_err();
    assert!(err.contains("no artifact nope"), "{err}");
    assert!(s.marks().marks.is_empty(), "an unknown id keeps nothing");
    s.keep_t(&["learnings-notebook".into()], artifacts_marks::Who::Agent("learnings"), true, 5).unwrap();
    assert!(list(&s, "", None, false, 9).ends_with(" · ◆ kept by learnings"), "sb artifact list says who kept it");
    // the row names the keeper by its name now (a renamed agent)
    let renamed = |a: &str| (a == "learnings").then(|| ("notebook".to_string(), false));
    let row = &s.rows(&renamed, "/")[0];
    assert_eq!(row["kept"], json!({"by": "notebook", "at_ms": 5}));
    assert_eq!(sections(&s, &renamed, &[])["learnings-notebook"].0, "kept");
}

#[test]
fn law_his_opens_move_touched_and_seen_v_forward() {
    let (_state, work, s) = marks_world("opens");
    add_as(&s, &work, "d.md", "deck", "writer", true, 10);
    assert!(s.opened(&[("deck".into(), 50, Some(1)), ("deck".into(), 40, None)]).unwrap());
    let row = &s.rows(&nobody, "/")[0];
    assert_eq!((row["opened_ms"].as_u64(), row["seen_v"].as_u64(), row["touched_ms"].as_u64()), (Some(50), Some(1), Some(50)));
    assert!(!s.opened(&[("deck".into(), 20, Some(1))]).unwrap(), "an older open moves nothing");
}

#[test]
fn law_each_end_moves_a_row_to_done_and_a_keep_holds_it() {
    let (state, work, s) = marks_world("ends");
    add_as(&s, &work, "a.md", "plan a", "task-a", true, 1);
    add_as(&s, &work, "b.md", "plan b", "task-b", true, 2);
    add_as(&s, &work, "c.png", "shots", "task-a", false, 3);
    page(&state, "ask", "a page that asks", &[(1, 4)], json!([]));
    let live = |a: &str| Some((a.to_string(), false));
    let now = sections(&s, &live, &[]);
    assert_eq!(now["plan-a"], ("open".into(), false));
    assert_eq!(now["shots"].0, "agents");
    // its task archived: done in the next rows; a page of an archived task that waits on him stays open
    let a_gone = |a: &str| Some((a.to_string(), a == "task-a" || a == "ambient-pm"));
    let now = sections(&s, &a_gone, &["ask"]);
    assert_eq!(now["plan-a"], ("done".into(), true));
    assert_eq!(now["plan-b"].0, "open");
    assert_eq!(now["shots"].0, "agents", "between agents stays there, done or not");
    assert_eq!(now["ask"], ("open".into(), false));
    assert_eq!(sections(&s, &a_gone, &[])["ask"].0, "done");
    // the artifact itself archived
    s.archive(&archive::Pick { ids: vec!["plan-b".into()], ..Default::default() }, 9).unwrap();
    assert_eq!(sections(&s, &live, &[])["plan-b"].0, "done");
    // kept: never done
    s.keep_t(&["plan-a".into()], artifacts_marks::Who::You, true, 9).unwrap();
    assert_eq!(sections(&s, &a_gone, &[])["plan-a"].0, "kept");
}

#[test]
fn law_a_merged_or_closed_pr_is_done() {
    let (_state, work, s) = marks_world("prdone");
    let a = Add { target: "https://github.com/o/r/pull/6".into(), agent: "perf".into(), by: "perf".into(), cwd: work.to_string_lossy().to_string(), ..Default::default() };
    s.add(&a, 1).unwrap();
    for (state, want) in [("open", "open"), ("merged", "done"), ("closed", "done")] {
        let mut rows = s.rows(&nobody, "/");
        rows[0]["pr"]["status"] = json!({"state": state});
        join_sections(&mut rows, &Default::default());
        assert_eq!(rows[0]["section"], want, "{state}");
    }
}

#[test]
fn law_the_backfill_runs_once_and_never_touches_what_has_its_fields() {
    let (state, work, s) = marks_world("backfill");
    // three artifacts from before the fields
    for (file, title, by) in [("p.pptx", "bise-pitch-investisseurs.pptx", "pitch"), ("s.png", "agent view shot 3", "ui-review"), ("b.md", "brief: land-all", "fast-loop")] {
        let m = add_as(&s, &work, file, title, by, false, 1).meta;
        s.save(&Meta { audience: None, ..m }).unwrap();
    }
    // one already for him, and a page
    add_as(&s, &work, "c.md", "changelog notes", "changelog", true, 2);
    page(&state, "learnings", "learnings", &[(1, 3)], json!([]));
    let n = s.backfill().unwrap().unwrap();
    assert_eq!((n.you, n.agents), (3, 2));
    let all: BTreeMap<String, Meta> = s.all().into_iter().map(|m| (m.id.clone(), m)).collect();
    assert_eq!(all["bise-pitch-investisseurs-pptx"].audience, Some(Audience::You));
    assert_eq!(all["agent-view-shot-3"].audience, Some(Audience::Agents));
    // kept by their own agent: the pitch (for him, evergreen) and the page; never one that had its audience
    let m = s.marks();
    assert_eq!(m.kept("bise-pitch-investisseurs-pptx").map(|k| k.by.as_str()), Some("pitch"));
    assert_eq!(m.kept("learnings").map(|k| k.by.as_str()), Some("ambient-pm"));
    assert!(m.kept("changelog-notes").is_none(), "it had its audience: untouched");
    assert_eq!(n.kept, 2);
    // once: he lets everything go, the backfill never keeps it again
    s.keep_t(&["learnings".into(), "bise-pitch-investisseurs-pptx".into()], artifacts_marks::Who::You, false, 9).unwrap();
    assert_eq!(s.backfill().unwrap(), None);
    assert!(s.marks().kept("learnings").is_none());
}

impl Store {
    /// keep with the titles from the store's own list (the daemon reads them from its rows)
    fn keep_t(&self, ids: &[String], who: artifacts_marks::Who, keep: bool, now: u64) -> Result<Vec<(String, String, artifacts_marks::Outcome)>, String> {
        let all = self.all();
        self.keep(ids, &|id: &str| all.iter().find(|m| m.id == id).map(|m| m.title.clone()), who, keep, now)
    }
}
