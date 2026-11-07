use super::*;
use crate::inbox_list::Waiting;
use bise_proto::inbox::{AnswerAt, InboxKind, InboxState, Where};

// 2025-09-30 12:41 UTC
const NOW: u64 = 1_759_236_060_000;

fn rec(card: u64, done: bool) -> InboxRecord {
    InboxRecord {
        card,
        project: "p".into(),
        asker: "perf".into(),
        kind: InboxKind::Question,
        question: format!("webp or avif for hero {card}?"),
        options: vec![],
        state: if done { InboxState::Answered } else { InboxState::Withdrawn },
        said: done.then(|| "avif".into()),
        pick_n: None,
        at: done.then_some(Where::Inbox),
        reason: (!done).then(|| "moot".into()),
        asked_ms: NOW,
        at_ms: NOW - card * 60_000,
        thread: done.then(|| AnswerAt { agent: "perf".into(), pos: 9 }),
        line2: if done { "you: avif · in the inbox".into() } else { "withdrawn: moot".into() },
        about: None,
    }
}

fn texts(ls: &[Line]) -> Vec<String> {
    ls.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
}

#[test]
fn waiting_on_top_and_the_keys_at_the_bottom() {
    let w = [Waiting { n: 1, id: 30, agent: "docs".into(), text: "ship the readme?".into(), age: "6m".into() }];
    let rows = list::rows(&w, &[rec(3, true), rec(2, false)], false, NOW, 0);
    for width in [80, 150] {
        let mut sc = Screen::default();
        let t = texts(&lines(&mut sc, &rows, width, 14));
        assert_eq!(sc.sel, Some(Pick::Card(30)));
        assert_eq!(t[0], "  waiting for you · 1");
        // its age in the time column, the number last (notif-id): no hole under the answers' times
        assert!(t[1].starts_with("› 1  docs  ship the readme?") && t[1].ends_with("6m · #30"), "{}", t[1]);
        assert!(t[3].starts_with("  ✓ perf  webp or avif for hero 3?") && t[3].ends_with("12:38 · #3"), "{}", t[3]);
        assert_eq!(t[4], "    you: avif · in the inbox");
        assert_eq!(t[6], "    withdrawn: moot");
        // ⏎ says what it does on the selected row
        assert_eq!(t.last().unwrap(), "⏎ answer   ↑↓ move   esc back");
        sc.sel = Some(Pick::Past(3));
        let t = texts(&lines(&mut sc, &rows, width, 14));
        assert_eq!(t.last().unwrap(), "⏎ go to it   ↑↓ move   esc back");
        assert!(t.iter().all(|l| l.width() <= width), "{t:#?}");
    }
}

#[test]
fn the_selection_stays_in_view_with_its_two_lines() {
    let past: Vec<InboxRecord> = (1..=20).rev().map(|c| rec(c, true)).collect();
    let rows = list::rows(&[], &past, true, NOW, 0);
    let mut sc = Screen::default();
    lines(&mut sc, &rows, 80, 10);
    assert_eq!(sc.sel, Some(Pick::Past(20)));
    sc.sel = list::step(&rows, sc.sel, 6);
    let t = texts(&lines(&mut sc, &rows, 80, 10));
    let at = t.iter().position(|l| l.starts_with("› ✓ perf  webp or avif for hero 14?")).expect("selected row shows");
    assert_eq!(t[at + 1], "    you: avif · in the inbox");
}

#[test]
fn down_walks_every_past_one_through_the_terminal_keys() {
    let mut app = crate::sb::bench::test_app();
    app.inbox = Some(Screen { past: vec![rec(2, false), rec(1, true)], ..Default::default() });
    let rs = rows(&app);
    lines(app.inbox.as_mut().unwrap(), &rs, 80, 20);
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.inbox.as_ref().unwrap().sel, Some(Pick::Past(2)));
    crate::input::on_key(&mut app, &down);
    let rs = rows(&app);
    lines(app.inbox.as_mut().unwrap(), &rs, 80, 20);
    assert_eq!(app.inbox.as_ref().unwrap().sel, Some(Pick::Past(1)));
    crate::input::on_key(&mut app, &down);
    assert_eq!(app.inbox.as_ref().map(|s| s.sel), Some(Some(Pick::Past(1))));
}

#[test]
fn a_record_that_changed_replaces_its_row_and_a_new_one_goes_on_top() {
    let mut app = crate::sb::bench::test_app();
    app.inbox = Some(Screen { past: vec![rec(5, false), rec(3, true)], ..Default::default() });
    recorded(&mut app, rec(5, true));
    recorded(&mut app, rec(7, true));
    recorded(&mut app, rec(4, true));
    let cards: Vec<(u64, InboxState)> = app.inbox.as_ref().unwrap().past.iter().map(|r| (r.card, r.state)).collect();
    assert_eq!(cards, [(7, InboxState::Answered), (5, InboxState::Answered), (4, InboxState::Answered), (3, InboxState::Answered)]);
}
