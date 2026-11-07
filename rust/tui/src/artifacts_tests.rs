use super::*;
use serde_json::{json, Value};

// 2025-09-30 12:41:00 UTC, a tuesday
const T: u64 = 1_759_236_060_000;
const PARIS: i32 = 2 * 3600;
const MIN: u64 = 60_000;

fn row(id: &str, title: &str, kind: &str, agent: &str, ago_min: u64) -> Value {
    json!({"id": id, "title": title, "kind": kind, "agent": agent, "at_ms": T - ago_min * MIN, "version": 1, "url": "", "new": false,
           "target": format!("/w/{id}")})
}

/// The hub's typed rows (`hub/artifacts`) from their JSON.
fn rows_of(v: Value) -> Vec<bise_proto::rows::Artifact> {
    serde_json::from_value(v).unwrap()
}

#[test]
fn the_hub_list_is_read_newest_first_with_its_versions() {
    let items = rows_of(json!([
        row("deck", "onboarding deck", "slides", "launch", 180),
        {"id": "pricing-page", "title": "pricing page", "kind": "page", "agent": "pricing-page", "at_ms": T - 12 * MIN, "version": 3,
         "url": "http://127.0.0.1:47438/p/pricing-page", "new": true,
         "target": "http://127.0.0.1:47438/p/pricing-page", "detail": "2 notes open",
         "versions": [{"v": 3, "at_ms": T - 12 * MIN, "target": "http://127.0.0.1:47438/p/pricing-page", "note": "2 notes open"},
                      {"v": 1, "at_ms": T - 60 * MIN, "target": "x"}, {"v": 2, "at_ms": T - 40 * MIN, "target": "x", "note": "3 notes done"}]},
        {"id": "", "kind": "doc", "title": "no id", "agent": "", "version": 1, "at_ms": 0, "url": "", "new": true},
        {"id": "mine", "kind": "doc", "title": "", "agent": "", "by": "you", "version": 1, "at_ms": T - 200 * MIN, "url": "", "new": false},
    ]));
    set_rows(&items, Some(T - 30 * MIN), "h-1");
    let rows = all();
    assert_eq!(rows.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["pricing-page", "deck", "mine"], "an item without an id is left out");
    assert_eq!(new_count(), 2, "the items the hub marks new");
    assert_eq!(seen_ms(), Some(T - 30 * MIN));
    assert_eq!(rows[2].title, "mine", "no title: its id");
    let p = &rows[0];
    assert_eq!(p.versions.iter().map(|x| x.v).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(p.is_link());
    assert_eq!(p.where_words("/w"), "127.0.0.1:47438/p/pricing-page");
    let doc = Artifact { target: "/w/acme/docs/q3-plan.md".into(), ..Default::default() };
    assert_eq!(doc.where_words("/w/acme"), "docs/q3-plan.md");
    assert_eq!(doc.where_words("/w/other"), "/w/acme/docs/q3-plan.md");
    mark_seen();
    assert_eq!(new_count(), 0);
}

#[test]
fn a_gone_file_says_so_and_opens_the_copy() {
    let a = Artifact {
        id: "subs".into(),
        title: "subscriptions design".into(),
        kind: "doc".into(),
        agent: "subs-lead".into(),
        archived: true,
        target: "/nowhere/at/all/subs.md".into(),
        copy: Some("/store/subs.md".into()),
        gone: true,
        ..Default::default()
    };
    assert_eq!(a.open_target(None), "/store/subs.md");
    assert_eq!(how(&a, None, false), How::Editor("/store/subs.md".into()));
}

#[test]
fn each_kind_opens_where_it_belongs() {
    let a = |kind: &str, target: &str| Artifact { id: "x".into(), kind: kind.into(), target: target.into(), ..Default::default() };
    assert_eq!(how(&a("page", "https://bise.dev/m/x"), None, false), How::Browser("https://bise.dev/m/x".into()));
    assert_eq!(how(&a("doc", "/w/plan.md"), None, false), How::Editor("/w/plan.md".into()));
    assert_eq!(how(&a("sheet", "/w/plans.xlsx"), None, false), How::App("/w/plans.xlsx".into()));
    assert_eq!(how(&a("code", "/w/run"), None, false), How::Editor("/w/run".into()));
    let pr = Artifact { pr: Some(Pr { repo: "o/r".into(), number: 6, branch: "b".into() }), ..a("pr", "https://github.com/o/r/pull/6") };
    assert_eq!(how(&pr, None, true), How::Diff("https://github.com/o/r/pull/6".into()));
    assert_eq!(how(&pr, None, false), How::Browser("https://github.com/o/r/pull/6".into()));
    // a .diff or .patch kept (a feature's diff, try-diff-open): the diff
    // panel, its newest version; an older version opens its file
    let diff = Artifact { id: "cu-diff".into(), ..a("code", "/s/features/cu.diff") };
    assert_eq!(how(&diff, None, false), How::Patch("cu-diff".into()));
    assert_eq!(how(&a("doc", "/w/fix.PATCH"), None, false), How::Patch("x".into()));
    assert_eq!(how(&a("link", "https://x.dev/a.diff"), None, false), How::Browser("https://x.dev/a.diff".into()));
}

/// try-diff-open: a .diff artifact opened from /artifacts or an inbox
/// option (its act) opens the diff panel on it, and the hub is asked
/// for that artifact's diff.
#[test]
fn a_diff_artifact_opens_the_diff_panel() {
    let mut app = crate::sb::bench::test_app();
    let a = Artifact { id: "cu-diff".into(), title: "cu at abc1234".into(), kind: "code".into(), target: "/s/features/cu.diff".into(), ..Default::default() };
    let note = open(&mut app, &a, None);
    assert_eq!(note, "cu at abc1234 · its diff");
    let p = app.diff.as_ref().expect("the panel opens");
    assert_eq!(p.ask, crate::diffview::Ask::Artifact("cu-diff".into()));
    assert!(p.focused, "it takes the keys");
    assert_eq!(crate::diffview::ask_of_url(&crate::diffview::url_of(&p.ask)), Some(p.ask.clone()));
}

#[test]
fn the_age_and_the_groups_follow_the_local_day() {
    let at = |ago: u64, short| age(T - ago, PARIS, T, PARIS, short);
    assert_eq!(at(20_000, false), "now");
    assert_eq!(at(12 * MIN, false), "12 min");
    assert_eq!(at(12 * MIN, true), "12m");
    assert_eq!(at(3 * 60 * MIN, false), "3 h");
    assert_eq!(at(3 * 60 * MIN, true), "3h");
    // 14:41 in Paris: yesterday 18:40 is 20h01 earlier
    let y = T - (20 * 60 + 1) * MIN;
    assert_eq!(group(y, PARIS, T, PARIS), Group::Yesterday);
    assert_eq!(age(y, PARIS, T, PARIS, false), "18:40");
    assert_eq!(ago_at(y, PARIS, T, PARIS), "yesterday 18:40");
    // T is a tuesday: the sunday before is this week
    let sun = T - 2 * 24 * 60 * MIN;
    assert_eq!(group(sun, PARIS, T, PARIS), Group::Week);
    assert_eq!(age(sun, PARIS, T, PARIS, false), "sun");
    let old = T - 30 * 24 * 60 * MIN;
    assert_eq!(group(old, PARIS, T, PARIS), Group::Earlier);
    assert_eq!(age(old, PARIS, T, PARIS, false), "aug 31");
    assert_eq!(ago_at(T - 12 * MIN, PARIS, T, PARIS), "12 min ago");
}

#[test]
fn the_search_finds_titles_agents_and_kinds_and_marks_the_letters() {
    let a = Artifact { id: "d".into(), title: "Onboarding Deck".into(), kind: "slides".into(), agent: "launch".into(), ..Default::default() };
    assert_eq!(find(&a, "deck"), Some(vec![11, 12, 13, 14]));
    assert_eq!(find(&a, "launch"), Some(vec![]));
    assert_eq!(find(&a, "slides deck"), Some(vec![11, 12, 13, 14]));
    assert_eq!(find(&a, "sheet"), None);
    assert_eq!(find(&a, ""), Some(vec![]));
    let pr = Artifact { kind: "pr".into(), ..a };
    assert!(find(&pr, "PR").is_some());
}

#[test]
fn links_and_paths_name_their_artifact() {
    set_for_test(
        vec![
            Artifact {
                id: "artifacts".into(),
                title: "artifacts mock".into(),
                kind: "page".into(),
                target: "https://bise.dev/m/artifacts".into(),
                keys: vec!["bise.dev/m/artifacts".into()],
                ..Default::default()
            },
            Artifact { id: "q3".into(), title: "q3 plan".into(), target: "/w/acme/docs/plans/q3.xlsx".into(), v: 2, ..Default::default() },
        ],
        0,
    );
    assert_eq!(resolve("https://bise.dev/m/artifacts/"), Some("artifacts".into()));
    assert_eq!(resolve("bise.dev/m/artifacts"), Some("artifacts".into()));
    assert_eq!(resolve("docs/plans/q3.xlsx"), Some("q3".into()));
    assert_eq!(resolve("q3.xlsx"), None, "a bare name is too loose");
    assert_eq!(resolve("https://bise.dev/m/other"), None);
    assert_eq!(parse_url("artifact:q3@v1"), Some(("q3".into(), Some(1))));
    assert_eq!(parse_url("artifact:q3"), Some(("q3".into(), None)));
    assert_eq!(parse_url("https://x"), None);
    assert_eq!(copy_link("artifact:artifacts").as_deref(), Some("bise.dev/m/artifacts"));
    assert_eq!(copy_link("artifact:nope"), None);
}

fn row_text(lines: &[ratatui::text::Line]) -> String {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>().join("\n").replace('\u{a0}', " ")
}

#[test]
fn the_thread_lines_are_chips_and_a_door() {
    set_for_test(vec![Artifact { id: "pricing-page".into(), title: "pricing page".into(), kind: "page".into(), v: 3, ..Default::default() }], 0);
    let made = crate::wire::Ev::Made { id: "pricing-page".into(), agent: "pricing-page".into(), title: "pricing page".into(), kind: "page".into(), v: 3 };
    let (rows, urls) = crate::links::collect(|| crate::render::ev_lines(&made, 110));
    assert_eq!(row_text(&rows).trim_end(), "  ↗ pricing page    page · v3 · pricing-page");
    assert_eq!(urls, ["artifact:pricing-page"]);
    // at 80 columns: the title and the kind only
    let rows = crate::render::ev_lines(&made, 60);
    assert!(!row_text(&rows).contains("· pricing-page"));
    // the landed door: `± 3 files +42 −18  a1b2c3d`, a link to its diff
    let landed = crate::wire::Ev::Landed { agent: "pricing-page".into(), from: "0f0f0f0".into(), sha: "a1b2c3d9".into(), files: 3, add: 42, del: 18 };
    let (rows, urls) = crate::links::collect(|| crate::render::ev_lines(&landed, 110));
    assert_eq!(row_text(&rows).trim_end(), "   ± 3 files +42 −18  a1b2c3d");
    assert_eq!(urls, ["bise-diff:range/0f0f0f0..a1b2c3d9?agent=pricing-page"]);
}

#[test]
fn a_reply_names_an_artifact_as_a_chip() {
    set_for_test(
        vec![Artifact {
            id: "artifacts".into(),
            title: "artifacts mock".into(),
            kind: "page".into(),
            target: "https://bise.dev/m/artifacts".into(),
            keys: vec!["bise.dev/m/artifacts".into()],
            ..Default::default()
        }],
        0,
    );
    let st = ratatui::style::Style::default();
    let (spans, urls) = crate::links::collect(|| crate::markdown::inline_spans("done: the [artifacts mock](artifact:artifacts) has diffs", st));
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect::<String>().replace('\u{a0}', " ");
    assert!(text.contains("↗ artifacts mock"), "{text}");
    assert_eq!(urls, ["artifact:artifacts"]);
    // a plain link the list knows: its chip, with its title
    let (spans, urls) = crate::links::collect(|| crate::markdown::inline_spans("see https://bise.dev/m/artifacts now", st));
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect::<String>().replace('\u{a0}', " ");
    assert!(text.contains("↗ artifacts mock") && !text.contains("https://"), "{text}");
    assert_eq!(urls, ["artifact:artifacts"]);
    // a link it does not know stays a link
    let (_, urls) = crate::links::collect(|| crate::markdown::inline_spans("see https://example.com", st));
    assert_eq!(urls, ["https://example.com"]);
}

/// about-chip: his words about an open artifact (the desktop's `about
/// [title](url): …`) draw its chip, never the link or its path, for an
/// xlsx copy (spaces, an older version's), a page and an html file; an
/// artifact the list lacks: its title alone.
#[test]
fn his_about_lead_is_the_artifacts_chip() {
    let hub = "/Users/me/.bise/hubs/docs-1/artifacts";
    let items = rows_of(json!([
        {"id": "projection", "title": "projection achat ou location", "kind": "sheet", "agent": "office", "at_ms": T, "version": 3, "new": false,
         "url": format!("{hub}/projection/v3/projection achat ou location.xlsx"), "target": "/w/projection achat ou location.xlsx",
         "copy": format!("{hub}/projection/v3/projection achat ou location.xlsx"),
         "versions": [{"v": 2, "at_ms": T - MIN, "target": "/w/projection achat ou location.xlsx", "copy": format!("{hub}/projection/v2/projection achat ou location.xlsx")}]},
        {"id": "q3-notes", "title": "Q3 notes", "kind": "page", "agent": "writer", "at_ms": T, "version": 1, "new": false,
         "url": "http://127.0.0.1:47438/p/q3-notes", "target": "/Users/me/.bise/pages/q3-notes/v1.html"},
        {"id": "mock", "title": "the mock", "kind": "site", "agent": "web", "at_ms": T, "version": 2, "new": false,
         "url": format!("{hub}/mock/v2/index.html"), "target": "/w/mock/index.html"},
    ]));
    set_rows(&items, None, "h-1");
    let st = ratatui::style::Style::default();
    let draw = |text: &str| crate::links::collect(|| crate::attach::chip_spans(text, st));
    let words = |spans: &[ratatui::text::Span<'static>]| spans.iter().map(|s| s.content.as_ref()).collect::<String>().replace('\u{a0}', " ");
    let (spans, urls) = draw(&format!("about [projection achat ou location]({hub}/projection/v2/projection achat ou location.xlsx): En fait 3200"));
    assert_eq!(words(&spans), "about  ↗ projection achat ou location : En fait 3200");
    assert_eq!(urls, ["artifact:projection"]);
    let (spans, urls) = draw("about [Q3 notes](http://127.0.0.1:47438/p/q3-notes): why?");
    assert_eq!(words(&spans), "about  ↗ Q3 notes : why?");
    assert_eq!(urls, ["artifact:q3-notes"]);
    let (_, urls) = draw(&format!("about [the mock]({hub}/mock/v2/index.html): ok"));
    assert_eq!(urls, ["artifact:mock"]);
    let (spans, urls) = draw("about [old deck](/x/y (1).pdf): hm");
    assert_eq!(words(&spans), "about old deck: hm");
    assert!(urls.is_empty());
}

/// page-unreach (Benjamin's refused page): a page link is asked before the
/// browser opens it; one whose server doesn't answer says so, no dead tab.
#[test]
fn a_page_whose_server_does_not_answer_is_not_opened() {
    assert_eq!(page_port("http://127.0.0.1:47415/p/blokhus-own-deployment"), Some(47415));
    assert_eq!(page_port("http://127.0.0.1:47415/a/x/view"), Some(47415));
    assert_eq!(page_port("http://127.0.0.1:47415x/p"), None);
    assert_eq!(page_port("https://github.com/o/r/pull/1"), None);
    assert_eq!(page_port("http://localhost:47415/p/x"), None);
    // a port nothing listens on: one free a moment ago
    let port = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
    assert!(!answers(port));
    let live = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    assert!(answers(live.local_addr().unwrap().port()));
    assert!(no_answer_note(47415).starts_with("not opened: nothing answers at 127.0.0.1:47415"));
}
