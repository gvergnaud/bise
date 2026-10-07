//! keep-state: the inbox answers and the view come back after a reload
//! (sb/keep.rs, through the drafts file of sb/drafts.rs).

use super::bench::{add_agent, test_app};
use super::drafts::{file_name, flush, restore, use_dir};
use super::*;
use serde_json::json;
use std::path::PathBuf;

/// A throwaway folder, removed at the end of the test.
struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        use_dir(None);
    }
}

fn setup() -> (Tmp, App) {
    let d = std::env::temp_dir().join(format!("bise-keep-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("ws")).unwrap();
    use_dir(Some(d.join("drafts")));
    let mut app = test_app();
    app.sb.workspace = d.join("ws").to_string_lossy().to_string();
    restore(&mut app);
    keep::apply(&mut app);
    (Tmp(d), app)
}

fn file(app: &App) -> PathBuf {
    std::env::temp_dir()
        .join(format!("bise-keep-{}-{:?}", std::process::id(), std::thread::current().id()))
        .join("drafts")
        .join(file_name(&app.sb.workspace))
}

fn on_disk(app: &App) -> Value {
    serde_json::from_str(&std::fs::read_to_string(file(app)).unwrap_or_else(|_| "{}".into())).unwrap()
}

fn card(id: u64, text: &str) -> Card {
    Card { id, kind: "question".into(), agent: "docs".into(), text: text.into(), ..Card::default() }
}

/// The hub's side at start: its agents and cards, as the snapshot gives.
fn hub(app: &mut App) {
    add_agent(app, "main", "");
    add_agent(app, "docs", "the docs");
    app.sb.cards = vec![card(12, "which title?"), card(13, "ship it?")];
}

/// A new TUI on the same workspace: restored, the hub's snapshot, then
/// `ready`.
fn reloaded(app: &App) -> App {
    let mut b = test_app();
    b.sb.workspace = app.sb.workspace.clone();
    restore(&mut b);
    hub(&mut b);
    keep::apply(&mut b);
    b
}

#[test]
fn an_inbox_answer_and_the_threads_draft_survive_a_reload() {
    let (_d, mut app) = setup();
    hub(&mut app);
    app.ed.insert("for main");
    cards::open_view(&mut app, Some(12));
    app.ed.insert("half an answer");
    // another card's answer, parked
    cards::open_view(&mut app, Some(13));
    app.ed.insert("yes");
    cards::open_view(&mut app, Some(12));
    flush(&app);
    let disk = on_disk(&app);
    assert_eq!(disk["answers"]["12"]["text"], "half an answer");
    assert_eq!(disk["answers"]["13"]["text"], "yes");
    assert_eq!(disk["drafts"]["main"]["text"], "for main");
    let mut b = test_app();
    b.sb.workspace = app.sb.workspace.clone();
    restore(&mut b);
    hub(&mut b);
    assert!(keep::pending() && !b.sb.card.open, "back at ready, not before");
    keep::apply(&mut b);
    assert!(b.sb.card.open);
    assert_eq!(b.sb.card.sel, Some(12));
    assert_eq!(b.ed.text, "half an answer");
    assert_eq!(b.ed.cursor, 14);
    cards::open_view(&mut b, Some(13));
    assert_eq!(b.ed.text, "yes");
    cards::close_view(&mut b);
    assert_eq!(b.ed.text, "for main");
}

#[test]
fn a_write_before_ready_keeps_what_is_not_back_yet() {
    let (_d, mut app) = setup();
    hub(&mut app);
    cards::open_view(&mut app, Some(12));
    app.ed.insert("an answer");
    flush(&app);
    let mut b = test_app();
    b.sb.workspace = app.sb.workspace.clone();
    restore(&mut b);
    // the new TUI ends before the hub's ready: nothing lost
    flush(&b);
    assert_eq!(on_disk(&b)["answers"]["12"]["text"], "an answer");
    assert_eq!(on_disk(&b)["view"]["card"]["id"], 12);
    keep::apply(&mut b);
}

#[test]
fn the_agent_in_view_the_popup_and_the_scroll_come_back() {
    let (_d, mut app) = setup();
    hub(&mut app);
    focus(&mut app, "docs");
    for i in 0..30 {
        super::feed::ingest_at(&mut app, format!("sb main: line {i}"), Some(100 + i), None);
    }
    app.follow = false;
    app.anchor = (12, 1);
    palette::open(&mut app, "do");
    flush(&app);
    assert_eq!(on_disk(&app)["view"]["focus"], "docs");
    let mut b = test_app();
    b.sb.workspace = app.sb.workspace.clone();
    restore(&mut b);
    hub(&mut b);
    // the replay: docs's lines, the same positions
    with_feed(&mut b, "docs", |b| {
        for i in 0..30 {
            super::feed::ingest_at(b, format!("sb main: line {i}"), Some(100 + i), None);
        }
    });
    keep::apply(&mut b);
    assert_eq!(b.sb.focus, "docs");
    assert_eq!(b.palette.as_ref().map(|p| p.query.as_str()), Some("do"));
    assert!(!b.follow);
    assert_eq!(b.anchor, (12, 1));
}

#[test]
fn find_and_help_queries_come_back() {
    let (_d, mut app) = setup();
    crate::find::open(&mut app);
    app.find.as_mut().unwrap().ed.insert("needle");
    flush(&app);
    let b = reloaded(&app);
    assert_eq!(b.find.as_ref().map(|f| f.ed.text.as_str()), Some("needle"));
    let (_d, mut app) = setup();
    let mut o = crate::help::Overlay::new(crate::help::Page::Shortcuts);
    o.filter = "inbox".into();
    app.help = Some(o);
    flush(&app);
    let b = reloaded(&app);
    let h = b.help.as_ref().unwrap();
    assert_eq!((h.page(), h.filter.as_str()), (crate::help::Page::Shortcuts, "inbox"));
}

#[test]
fn an_old_view_is_dropped_the_answers_stay() {
    let (_d, mut app) = setup();
    hub(&mut app);
    cards::open_view(&mut app, Some(12));
    app.ed.insert("still mine");
    flush(&app);
    // written long ago: a start the next morning
    let mut v = on_disk(&app);
    v["view"]["ms"] = json!(1);
    std::fs::write(file(&app), v.to_string()).unwrap();
    let mut b = reloaded(&app);
    assert!(!b.sb.card.open, "the view is a reload's only");
    cards::open_view(&mut b, Some(12));
    assert_eq!(b.ed.text, "still mine");
}

#[test]
fn a_gone_card_or_agent_is_skipped() {
    let (_d, mut app) = setup();
    hub(&mut app);
    focus(&mut app, "docs");
    cards::open_view(&mut app, Some(12));
    app.ed.insert("too late");
    flush(&app);
    let mut b = test_app();
    b.sb.workspace = app.sb.workspace.clone();
    restore(&mut b);
    add_agent(&mut b, "main", "");
    b.sb.cards = vec![card(13, "ship it?")];
    keep::apply(&mut b);
    assert_eq!(b.sb.focus, "main");
    assert!(!b.sb.card.open);
    assert!(keep::answers(&b).is_empty());
}

#[test]
fn a_setup_items_text_is_never_saved() {
    let (_d, mut app) = setup();
    let id = setup::LOCAL + 1;
    app.sb.cards = vec![card(id, "your key")];
    cards::open_view(&mut app, Some(id));
    app.ed.insert("sk-secret");
    flush(&app);
    assert!(!std::fs::read_to_string(file(&app)).unwrap_or_default().contains("sk-secret"));
}

#[test]
fn an_older_file_a_newer_file_and_a_corrupt_file_all_start() {
    let (_d, app) = setup();
    // an older TUI's file: no answers, no view
    std::fs::create_dir_all(file(&app).parent().unwrap()).unwrap();
    std::fs::write(file(&app), json!({"workspace": app.sb.workspace, "drafts": {"main": {"text": "old", "cursor": 3}}}).to_string())
        .unwrap();
    let b = reloaded(&app);
    assert_eq!(b.ed.text, "old");
    // a newer TUI's: fields and a popup kind this one does not know
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    std::fs::write(
        file(&app),
        json!({"workspace": app.sb.workspace, "drafts": {"main": {"text": "new", "cursor": 1, "sel": [0, 1]}},
               "answers": {"12": {"text": "a", "cursor": 1}, "x": {"text": "b"}},
               "view": {"ms": now, "focus": "docs", "popup": {"kind": "hologram"}, "zoom": 3}, "later": true})
        .to_string(),
    )
    .unwrap();
    let mut b = reloaded(&app);
    assert_eq!(b.sb.focus, "docs");
    assert!(b.palette.is_none() && b.find.is_none());
    focus(&mut b, "main");
    assert_eq!(b.ed.text, "new");
    cards::open_view(&mut b, Some(12));
    assert_eq!(b.ed.text, "a");
    // garbage: a plain start
    std::fs::write(file(&app), "{not json").unwrap();
    let b = reloaded(&app);
    assert_eq!(b.ed.text, "");
}

#[test]
fn the_scroll_survives_the_hub_coming_back() {
    let (_d, mut app) = setup();
    let lines = |app: &mut App| {
        for i in 0..30 {
            super::feed::ingest_at(app, format!("sb main: line {i}"), Some(100 + i), None);
        }
    };
    lines(&mut app);
    app.follow = false;
    app.anchor = (7, 2);
    // a reload's hub comes back before the TUI re-executes: the feed is
    // empty until the replay, the scroll is still what is saved
    hub_reconnected(&mut app);
    assert!(app.follow && app.events.is_empty());
    assert_eq!(keep::view(&app).and_then(|v| v.pin).map(|p| p.pos), Some(107));
    lines(&mut app);
    keep::apply(&mut app);
    assert!(!app.follow);
    assert_eq!(app.anchor, (7, 2));
}
