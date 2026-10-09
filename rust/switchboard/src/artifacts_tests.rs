//! Laws of the artifact store (docs/artifacts.md): what gets in, how a
//! version is made, the copies, the pages read as they are, the rows.

use super::*;
use serde_json::json;

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
    assert!(s.list_text("", None, 20).contains("▲ gone from disk · bise kept a copy"));
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
    assert_eq!(pr_row["pr"], json!({"repo": "acme/web", "number": 6}));
    let mock_row = rows.iter().find(|r| r["id"] == "the-artifacts-mock").unwrap();
    let keys: Vec<&str> = mock_row["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap()).collect();
    assert!(keys.contains(&"bise.dev/m/artifacts") && keys.contains(&"https://bise.dev/m/artifacts"), "{keys:?}");
    // the same link again: a new version of the same row
    let again = add(&s, &root, "https://bise.dev/m/artifacts/", None, 12).unwrap();
    assert_eq!((again.meta.id.as_str(), again.meta.versions.len()), ("the-artifacts-mock", 2));
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
fn the_thread_line_escapes_its_fields() {
    let m = Meta {
        id: "x".into(),
        agent: "a".into(),
        title: "plan : v2".into(),
        kind: "doc".into(),
        versions: vec![Version { v: 2, ..Default::default() }],
        ..Default::default()
    };
    assert_eq!(thread_line(&m), "x : a : plan \\: v2 : doc : 2");
}

#[test]
fn list_text_filters_by_words_and_agent() {
    let root = tmp("listtext");
    let s = Store::new(&root.join("state"));
    assert!(s.list_text("", None, 1).starts_with("no artifacts yet."));
    add(&s, &root, "https://github.com/acme/web/pull/6", Some("gateway head checks"), 10).unwrap();
    let t = s.list_text("gateway", None, 20);
    assert!(t.starts_with("[gateway head checks](artifact:gateway-head-checks) · pr · v1 · pricing-page"), "{t}");
    assert_eq!(s.list_text("deck", None, 20), "nothing matches.");
    assert_eq!(s.list_text("", Some("launch"), 20), "nothing matches.");
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
    let items = crate::proto_view::artifacts(&ev, |_| None);
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
