//! The inbox (cards v2): the strip, the card view, the keys (book
//! screens `inbox · 1-7`; BISE-302: ctrl+1-9 and a click open an item).

use super::*;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::io::Read;

/// An app whose hub end the test reads.
fn app_with_hub() -> (App, UnixStream) {
    let (a, b) = UnixStream::pair().unwrap();
    b.set_nonblocking(true).unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::mem::forget(tx);
    let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
    (sb_app(sb, rx, false, 100, crate::voice::Voice::live(false)), b)
}

/// What the TUI typed to the hub since the last call (`input` ops).
fn sent(b: &mut UnixStream) -> Vec<String> {
    let mut s = String::new();
    let mut buf = [0u8; 4096];
    while let Ok(n) = b.read(&mut buf) {
        if n == 0 {
            break;
        }
        s.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    s.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == "input")
        .map(|v| v["text"].as_str().unwrap_or("").to_string())
        .collect()
}

fn card(id: u64, kind: &str, agent: &str, text: &str) -> Card {
    Card { id, kind: kind.into(), agent: agent.into(), text: text.into(), age_ms: 360_000, ..Card::default() }
}

const PERF: &str = "the hero image is 4.2 MB. compress it, or lazy-load it?\n1. compress it (webp, ~300 kB)\n2. both: compress, and lazy-load below the fold";

/// The mocks' cast: release (an approval), perf (2 options), dark-mode
/// (9 options).
fn cast() -> Vec<Card> {
    let dark = format!(
        "the settings page has 3 grays in tokens.css and 2 hardcoded ones. i found them because the dark toggle left two borders white.\n{}",
        (1..=9).map(|i| format!("{i}. option {i}")).collect::<Vec<_>>().join("\n")
    );
    vec![
        card(12, "question", "perf", PERF),
        card(13, "question", "dark-mode", &dark),
        card(14, "approval", "release", "npm publish --tag next\n\npublishes 2.5.0 to npm under your name"),
    ]
}

fn key(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
    super::super::key(app, &KeyEvent::new(code, m), false)
}

fn ctrl(app: &mut App, c: char) -> bool {
    key(app, KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// ctrl+1: the card view on the inbox's first row.
fn open(app: &mut App) {
    assert!(ctrl(app, '1'), "ctrl+1 opens row 1");
    assert!(app.sb.card.open);
}

/// A key through the whole handler (the composer included).
fn press(app: &mut App, code: KeyCode) {
    crate::input::on_key(app, &KeyEvent::new(code, KeyModifiers::NONE));
}

/// The key bar's text.
fn bar(app: &App) -> String {
    crate::keybar::line(app, 200).spans.iter().map(|s| s.content.as_ref()).collect()
}

fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

fn row_of(rows: &[String], needle: &str) -> usize {
    rows.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("no {needle:?} in\n{}", rows.join("\n")))
}

fn click(app: &mut App, x: u16, y: u16) -> bool {
    card_mouse(
        app,
        &MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE },
    )
}

fn col_of(row: &str, needle: &str) -> u16 {
    let i = row.find(needle).unwrap();
    row[..i].chars().count() as u16
}

/// The box above the divider (designer's round 2 A): a rounded border,
/// its title in it (`inbox · 3 waiting for you … ctrl+1-3 open`), a row
/// per item, most blocking first, numbered (BISE-302: ctrl+N opens row
/// N), the age on the right; no other keys on the rows.
#[test]
fn the_strip_shows_every_card_most_blocking_first() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    let rows = draw(&mut app, 140, 40);
    let lab = row_of(&rows, " ctrl+1-3 open ─╮");
    assert!(rows[lab].contains("╭─ inbox · 3 waiting for you ─"), "{}", rows[lab]);
    let rel = row_of(&rows, "│ 1 ? release · $ npm publish --tag next");
    let perf = row_of(&rows, "│ 2 ? perf · the hero image");
    let dark = row_of(&rows, "│ 3 ? dark-mode · ");
    assert_eq!((rel, perf, dark), (lab + 1, lab + 2, lab + 3), "{}", rows.join("\n"));
    assert!(rows[rel].contains(" 6m │"), "the age: {}", rows[rel]);
    for r in [rel, perf, dark] {
        assert!(!rows[r].contains('×') && !rows[r].contains("⏎") && !rows[r].contains('▸'), "no keys: {}", rows[r]);
    }
    assert!(rows[dark].contains("…"), "cut: {}", rows[dark]);
    // its bottom border, a blank row, the divider
    assert!(rows[dark + 1].contains("╰──"), "{}", rows[dark + 1]);
    assert!(rows[dark + 3].contains("you → main"), "{}", rows[dark + 3]);
    // from the gutter (a column left of the composer's bar) to the panel
    let bar_x = rows[dark + 4].find('│').map(|i| rows[dark + 4][i + 3..].find('│').unwrap() + i + 3);
    assert_eq!(rows[lab].chars().position(|c| c == '╭'), bar_x.map(|b| rows[dark + 4][..b].chars().count() - 1), "{}", rows.join("\n"));
    assert!(bar(&app).starts_with("@ file   $ skills   / commands   ctrl+1 inbox"), "{}", bar(&app));
    // the label follows the rows shown: 1 row, 2 rows
    app.sb.cards.truncate(2);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("ctrl+1-2 open ")), "{}", rows.join("\n"));
    app.sb.cards.truncate(1);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("ctrl+1 open ")), "{}", rows.join("\n"));
    // nothing waits: no inbox pair
    app.sb.cards.clear();
    assert!(!bar(&app).contains("inbox"), "{}", bar(&app));
}

/// BISE-302: a terminal without ctrl+1-9 (reach.rs): the strip says
/// `click to open`, without clicks `/inbox opens it`; the key bar
/// `/inbox`; the rows keep their numbers.
#[test]
fn without_ctrl_digits_the_inbox_never_shows_them() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    app.ctrl_digits = false;
    let rows = draw(&mut app, 140, 40);
    let lab = row_of(&rows, " inbox · 3 waiting for you ");
    assert!(rows[lab].contains(" click to open ─╮"), "{}", rows[lab]);
    assert!(rows[lab + 1].contains("│ 1 ? release · $ npm publish"), "{}", rows[lab + 1]);
    assert!(!rows.iter().any(|r| r.contains("ctrl+1")), "{}", rows.join("\n"));
    assert!(bar(&app).starts_with("@ file   $ skills   / commands   /inbox"), "{}", bar(&app));
    app.clicks = false;
    let rows = draw(&mut app, 140, 40);
    assert!(rows[lab].contains("/inbox opens it "), "{}", rows[lab]);
    // the keys still work when they come
    assert!(ctrl(&mut app, '2'));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
}

/// More than 3 cards: `+ n more`; under 24 rows, one row: the top card
/// numbered 1 and `+ n`.
#[test]
fn many_cards_and_small_screens() {
    let (mut app, _hub) = app_with_hub();
    let mut cs = cast();
    cs.push(card(20, "done", "cookies", "moved the banner off buy"));
    cs.push(card(21, "failed", "t9", "the build broke"));
    app.sb.cards = cs;
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("│ + 2 more · ✗ t9 failed · the build broke")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("ctrl+1-3 open ")), "the rows shown: {}", rows.join("\n"));
    let rows = draw(&mut app, 140, 20);
    let top = row_of(&rows, "│ 1 ? release · $ npm publish");
    assert!(rows[top].contains("+ 4  6m │"), "{}", rows[top]);
    assert!(rows[top - 1].contains("╭─ inbox") && rows[top + 1].contains("╰─"), "one row: {}", rows.join("\n"));
}

/// ctrl+N opens row N in the view, the rows under `+ n more` too; in the
/// view it shows that item; past the last row: nothing. A French layout's
/// top row counts as its digits (`&` is 1, `é` 2).
#[test]
fn ctrl_digit_opens_row_n() {
    let (mut app, mut hub) = app_with_hub();
    let mut cs = cast();
    cs.push(card(20, "done", "cookies", "moved the banner off buy"));
    cs.push(card(21, "failed", "t9", "the build broke"));
    app.sb.cards = cs;
    let ids = super::super::card_draw::strip_ids(&app.sb);
    assert_eq!(ids.len(), 5);
    app.ed.insert("my draft");
    assert!(!ctrl(&mut app, '6'), "past the last row");
    assert!(!app.sb.card.open);
    assert!(ctrl(&mut app, '2'));
    assert!(app.sb.card.open);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[1]));
    assert_eq!(app.ed.text, "", "the thread's draft waits");
    // in the view: that item, a hidden row too
    assert!(ctrl(&mut app, '5'));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[4]));
    assert!(key(&mut app, KeyCode::Char('&'), KeyModifiers::CONTROL));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[0]));
    assert!(key(&mut app, KeyCode::Char('é'), KeyModifiers::CONTROL | KeyModifiers::SHIFT));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[1]));
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "my draft");
    assert!(sent(&mut hub).is_empty(), "opening answers nothing");
    // nothing waits: ctrl+1 is not the inbox's
    app.sb.cards.clear();
    assert!(!ctrl(&mut app, '1'));
}

/// From the thread only ctrl+1-9 act on the inbox: the old keys (ctrl+g,
/// ctrl+x/n/p, alt+r, ctrl+a, ctrl+f, digits) and the arrows do nothing
/// to it; a digit is text.
#[test]
fn from_the_thread_only_ctrl_digits() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    for c in ['g', 'x', 'n', 'p', 'a', 'f'] {
        ctrl(&mut app, c);
    }
    key(&mut app, KeyCode::Char('r'), KeyModifiers::ALT);
    assert!(!key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE));
    for k in [KeyCode::Up, KeyCode::Down, KeyCode::Enter, KeyCode::Right] {
        assert!(!key(&mut app, k, KeyModifiers::NONE), "{k:?} is the thread's");
    }
    assert!(sent(&mut hub).is_empty());
    assert!(!app.sb.card.open);
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.ed.text, "2", "a digit is text");
}

/// What only a terminal with ctrl+1-9 sends turns them on (reach.rs);
/// ctrl+4-7 come from any terminal (0x1c-0x1f) and prove nothing.
#[test]
fn a_ctrl_digit_proves_the_terminal_sends_them() {
    let k = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    assert!(super::proves_ctrl_digits(&k('1')) && super::proves_ctrl_digits(&k('9')));
    assert!(!super::proves_ctrl_digits(&k('4')) && !super::proves_ctrl_digits(&k('7')));
    assert!(!super::proves_ctrl_digits(&KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)));
    let (mut app, _hub) = app_with_hub();
    app.ctrl_digits = false;
    crate::input::on_key(&mut app, &k('5'));
    assert!(!app.ctrl_digits);
    crate::input::on_key(&mut app, &k('2'));
    assert!(app.ctrl_digits);
}

/// ctrl+1 opens an item in place, where its row stood: the other rows
/// stay above and below it, the thread stays in sight; the item on its
/// tint behind the accent bar: its head and `n of N · age`, what it asks,
/// the options on one line, the hint. ↓ the next item. The divider says
/// who reads the composer; ctrl+o: full screen, the items as tabs.
#[test]
fn the_card_view_takes_the_history_place() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    let rows = draw(&mut app, 140, 40);
    let rel = row_of(&rows, "│ 1 ? release · $ npm publish");
    assert!(rows[rel + 1].contains("│ ┃ "), "a blank bar row on top: {}", rows[rel + 1]);
    let t = row_of(&rows, "┃ ? perf asks");
    assert_eq!(t, rel + 2);
    // its age one tinted column from the tint's edge, then the box's gap
    assert!(rows[t].contains("2 of 3 · 6m  │"), "{}", rows[t]);
    assert!(rows[t + 1].contains("┃ the hero image is 4.2 MB. compress it, or lazy-load it?"), "{}", rows[t + 1]);
    let o = row_of(&rows, "┃ 1 compress it (webp, ~300 kB)   2 both: compress, and lazy-load below the fold");
    assert!(rows[o + 1].contains("┃ or type your answer, ⏎ sends it"), "{}", rows[o + 1]);
    assert!(rows[o + 3].contains("│ 3 ? dark-mode · "), "the next row under it: {}", rows[o + 3]);
    // the item's tint, one step above the composer's
    let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let x = col_of(&rows[t], "? perf");
    assert_eq!(term.backend().buffer()[(x, t as u16)].bg, theme::item_tint());
    let pad = col_of(&rows[t], "6m  │") + 2;
    assert_eq!(term.backend().buffer()[(pad, t as u16)].bg, theme::item_tint(), "the padding after the age is tinted");
    assert!(!rows.iter().any(|r| r.contains('▸')), "nothing highlighted on open");
    assert!(rows.iter().any(|r| r.contains("you → ? perf · your answer")), "{}", rows.join("\n"));
    assert_eq!(bar(&app), "1-2 answer   ←→ choose   ↑↓ other items   ctrl+o full screen   esc back to your message");
    // ctrl+o: full screen, the items as numbered tabs; again: in place
    ctrl(&mut app, 'o');
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "1 ? release");
    assert!(rows[tabs].contains("2 ? perf") && rows[tabs].contains("3 ? dark-mode") && rows[tabs].contains("↑↓"), "{}", rows[tabs]);
    assert!(!rows.iter().any(|r| r.contains("waiting for you")), "no box: {}", rows.join("\n"));
    assert!(rows[row_of(&rows, "? perf asks")].contains("┃ ? perf asks"));
    assert_eq!(bar(&app), "1-2 answer   ↑↓ other items   pgup pgdn scroll   ctrl+o back in place   esc back to your message");
    ctrl(&mut app, 'o');
    let rows = draw(&mut app, 140, 40);
    row_of(&rows, "waiting for you");
}

/// An approval: the command with its `$`, the reason dim, allow once /
/// always here / deny on one line; ⏎ with text says no with a note; the
/// answer folds on top of the box for 2 s, the next item open.
#[test]
fn an_approval_plugs_in() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("┃ ? release wants to run")));
    assert!(rows.iter().any(|r| r.contains("┃ $ npm publish --tag next")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("┃ publishes 2.5.0 to npm")));
    row_of(&rows, "┃ 1 allow once   2 always here   3 deny");
    row_of(&rows, "┃ or type why not, ⏎ says no");
    assert_eq!(bar(&app), "1-3 answer   ←→ choose   ↑↓ other items   ctrl+o full screen   esc back to your message");
    // a reflex ⏎ approves nothing
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(sent(&mut hub).is_empty());
    app.ed.insert("not on friday");
    assert_eq!(bar(&app), "⏎ says no, with your note   ctrl+o full screen   esc back to your message");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 14 deny: not on friday"]);
    // on to the next item; the answer folded on top of the box
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    let rows = draw(&mut app, 140, 40);
    let f = row_of(&rows, "│ ✗ you said no to release: npm publish --tag next · \"not on friday\"");
    assert!(rows[f - 1].contains("╭─ inbox · 2 waiting for you"), "{}", rows.join("\n"));
    assert!(rows[f + 2].contains("┃ ? perf asks"), "{}", rows[f + 2]);
    // the thread says the same sentence; 2 s later the box's is gone
    assert!(rows[..f - 1].iter().any(|r| r.contains("✗ you said no to release: npm publish --tag next")), "{}", rows.join("\n"));
    app.sb.card.fold.as_mut().unwrap().1 -= std::time::Duration::from_secs(3);
    let rows = draw(&mut app, 140, 40);
    assert!(!rows.iter().any(|r| r.contains("│ ✗ you said no to")), "{}", rows.join("\n"));
}

/// 1-9 picks on an empty composer only; ⏎ sends the text; the history
/// says `✓ you answered perf: both`; each card keeps its draft; esc brings
/// the thread's draft back; the last answer goes back to the thread.
#[test]
fn answering_picking_drafts_and_back() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("to main");
    open(&mut app);
    assert_eq!(app.ed.text, "", "the card's own composer");
    ctrl(&mut app, 'n');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    // a digit on an empty composer picks
    assert!(key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(sent(&mut hub), vec!["/answer 12 both: compress, and lazy-load below the fold"]);
    assert!(matches!(app.events.last(), Some(Ev::Approval { ok: true, text, .. }) if text == "you answered perf: both"));
    // the next card: dark-mode; typed digits are text
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    app.ed.insert("x");
    assert!(!key(&mut app, KeyCode::Char('3'), KeyModifiers::NONE));
    app.ed.insert("3 then");
    // a draft per card
    ctrl(&mut app, 'p');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(14));
    assert_eq!(app.ed.text, "");
    ctrl(&mut app, 'n');
    assert_eq!(app.ed.text, "x3 then");
    // esc: back to the thread, its draft back
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "to main");
    // ctrl+1: the top card; ctrl+x closes it
    open(&mut app);
    ctrl(&mut app, 'x');
    assert_eq!(sent(&mut hub), vec!["/close 14"]);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert_eq!(app.ed.text, "x3 then", "dark-mode's draft kept");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 13 x3 then"]);
    // none left: back to the thread with its draft
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "to main");
}

/// A long item in place keeps its first lines, then `… n more lines ·
/// ctrl+o full screen`, at most half the feed area, its options in
/// sight. Full screen it scrolls with pgup/pgdn and the wheel; ←→
/// highlight an option and the view scrolls to it.
#[test]
fn a_long_card_scrolls() {
    let (mut app, _hub) = app_with_hub();
    let long = (1..=60).map(|i| format!("line {i:02} of the card")).collect::<Vec<_>>().join("\n") + "\n1. yes\n2. no";
    app.sb.cards = vec![card(3, "question", "t1", &long)];
    open(&mut app);
    let rows = draw(&mut app, 120, 30);
    let more = row_of(&rows, "┃ … ");
    assert!(rows[more].contains("more lines · ctrl+o full screen"), "{}", rows.join("\n"));
    assert!(rows[more + 2].contains("┃ 1 yes   2 no"), "{}", rows.join("\n"));
    let top = row_of(&rows, "╭─ inbox");
    let div = row_of(&rows, "you → ? t1");
    assert!(more + 6 - top <= 2 + (div - 1) / 2 + 2, "about half the feed: {}", rows.join("\n"));
    // pgdn is the thread's in place
    assert!(!key(&mut app, KeyCode::PageDown, KeyModifiers::NONE));
    ctrl(&mut app, 'o');
    let rows = draw(&mut app, 120, 30);
    assert!(rows.iter().any(|r| r.contains("more lines · pgdn")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("1 yes")));
    key(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
    assert!(app.sb.card.scroll > 0);
    let a = app.sb.card.area;
    let wheel = MouseEvent { kind: MouseEventKind::ScrollUp, column: a.x + 3, row: a.y + 3, modifiers: KeyModifiers::NONE };
    let before = app.sb.card.scroll;
    assert!(card_mouse(&mut app, &wheel));
    assert_eq!(app.sb.card.scroll, before - 3);
    // ←: the last option, brought into view
    key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1));
    let rows = draw(&mut app, 120, 30);
    let opts = row_of(&rows, "1 yes   2 no");
    assert!(rows[opts - 2].contains("line 60"), "{}", rows.join("\n"));
}

/// Nothing highlighted on open (⏎ does nothing); ←→ walk the options
/// (the first → is option 1, the first ← the last, no wrap), ⏎ picks it
/// and the key bar says so; ↑↓ the other items, no wrap; typing: the
/// arrows are your text's, the highlight hidden (remembered), ⏎ sends
/// the text. ctrl+arrows are silent aliases.
#[test]
fn the_arrows_choose_an_option() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    ctrl(&mut app, '2');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    assert_eq!(app.sb.card.opt, None);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(sent(&mut hub).is_empty(), "a reflex ⏎ answers nothing");
    key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1), "the first ←: the last option");
    key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1), "no wrap");
    key(&mut app, KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(app.sb.card.opt, Some(0), "ctrl+← too");
    key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    let rows = draw(&mut app, 140, 40);
    let o = row_of(&rows, "2 both: compress");
    let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let x = col_of(&rows[o], "2 both");
    assert_eq!(term.backend().buffer()[(x, o as u16)].bg, theme::accent(), "accent on the ground");
    assert!(bar(&app).starts_with("⏎ both: compress, and lazy-load"), "{}", bar(&app));
    // typing: the text's arrows, the highlight hidden but kept
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Left);
    assert_eq!((app.ed.cursor, app.sb.current_card().map(|c| c.id)), (0, Some(12)), "← moved the caret");
    assert_eq!(bar(&app), "⏎ sends your answer   ctrl+o full screen   esc back to your message");
    // emptied: the arrows and the highlight are back
    press(&mut app, KeyCode::Delete);
    assert_eq!(app.ed.text, "");
    assert!(bar(&app).starts_with("⏎ both"), "{}", bar(&app));
    // ↓ ↑ the other items, the highlight goes with the item
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!((app.sb.current_card().map(|c| c.id), app.sb.card.opt), (Some(13), None));
    key(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(14), "no wrap");
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    // → ⏎ picks option 1; the next item opens
    key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 12 compress it (webp, ~300 kB)"]);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    // typed text + ⏎ answers with the text
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(sent(&mut hub), vec!["/answer 13 k"]);
}

/// Narrow: ctrl+o goes first, then `←→ choose`; under 100 columns esc
/// says `back` (`1-3 answer   ↑↓ other items   esc back`); then the
/// pairs drop from the right, what answers last.
#[test]
fn the_card_key_bar_keeps_choose_and_enter() {
    let v = vec![
        ("1-3", "answer".to_string()),
        ("←→", "choose".into()),
        ("↑↓", "other items".into()),
        ("ctrl+o", "full screen".into()),
        ("esc", "back to your message".into()),
    ];
    let keys = |w| fit_card_pairs(v.clone(), w).iter().map(|p| format!("{} {}", p.0, p.1)).collect::<Vec<_>>();
    assert_eq!(keys(200).len(), 5);
    assert_eq!(keys(80), ["1-3 answer", "↑↓ other items", "esc back"]);
    assert_eq!(keys(30), ["1-3 answer", "esc back"]);
    assert_eq!(keys(12), ["1-3 answer"]);
}

/// The mouse on the box (BISE-302): a click on a row opens it in place,
/// the thread's draft waits; with an item open, a click on a row jumps
/// to it, on an option picks it; full screen a tab switches items.
#[test]
fn the_mouse_on_the_strip_and_the_tabs() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("draft to main");
    let rows = draw(&mut app, 140, 40);
    let dark = row_of(&rows, "? dark-mode · ");
    assert!(click(&mut app, col_of(&rows[dark], "dark-mode"), dark as u16));
    assert!(app.sb.card.open, "a click on a row opens it");
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert_eq!(app.ed.text, "");
    assert!(sent(&mut hub).is_empty());
    // a new item only adds a row
    app.sb.cards.push(card(30, "question", "api", "v1 or v2?\n1. v1\n2. v2"));
    let rows = draw(&mut app, 140, 40);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    let more = row_of(&rows, "+ 1 more · ? api");
    assert!(click(&mut app, col_of(&rows[more], "api"), more as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(30));
    // full screen: the tabs
    ctrl(&mut app, 'o');
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "? perf");
    assert!(click(&mut app, col_of(&rows[tabs], "perf"), tabs as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.ed.text, "draft to main");
    // the number or the summary: the whole row opens
    let rows = draw(&mut app, 140, 40);
    let perf = row_of(&rows, "? perf · ");
    assert!(click(&mut app, col_of(&rows[perf], "2 ?"), perf as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    // a click on an option picks it
    let rows = draw(&mut app, 140, 40);
    let o = row_of(&rows, "┃ 1 compress it");
    assert!(click(&mut app, col_of(&rows[o], "2 both"), o as u16));
    assert_eq!(sent(&mut hub), vec!["/answer 12 both: compress, and lazy-load below the fold"]);
}

/// The card view's divider keeps a fresh note on its right (a copy from
/// the card's text: `✓ copied 9 chars`), as the thread's divider does.
#[test]
fn the_card_view_divider_shows_a_fresh_note() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    app.flash = Some(("copied 9 chars".into(), std::time::Instant::now()));
    let rows = draw(&mut app, 140, 40);
    let d = row_of(&rows, "you → ? release · your answer");
    assert!(rows[d].contains("your answer  ✓ copied 9 chars "), "{}", rows[d]);
}

/// The card in view answered elsewhere: the view moves on, its draft
/// goes to the history; no card left: back to the thread.
#[test]
fn a_card_closed_elsewhere_moves_the_view_on() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = vec![card(1, "question", "a", "one?"), card(2, "question", "b", "two?")];
    app.ed.insert("mine");
    open(&mut app);
    app.ed.insert("half an answer");
    app.sb.cards.remove(0);
    sync(&mut app);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(2));
    assert_eq!(app.history.first().map(String::as_str), Some("half an answer"));
    app.sb.cards.clear();
    sync(&mut app);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "mine");
}

/// ⌥N in the view: to that agent; the view goes, the drafts stay.
#[test]
fn going_to_an_agent_closes_the_view() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("to main");
    open(&mut app);
    app.ed.insert("note");
    crate::sb::focus(&mut app, "perf");
    assert!(!app.sb.card.open);
    crate::sb::focus(&mut app, "main");
    assert_eq!(app.ed.text, "to main");
    open(&mut app);
    assert_eq!(app.ed.text, "note");
}

/// A done card needs no words: ⏎ on an empty composer acknowledges it.
#[test]
fn a_done_card_is_acknowledged_with_enter() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = vec![card(5, "done", "cookies", "moved the banner off buy")];
    open(&mut app);
    assert_eq!(bar(&app), "⏎ got it   ctrl+o full screen   esc back to your message");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 5 seen"]);
    assert!(!app.sb.card.open);
}

/// The shapes: an approval's command and reason, a patch's summary, the
/// short labels of the strip.
#[test]
fn shapes() {
    let c = card(1, "approval", "r", "set -e\nnpm test\nnpm publish\n\nships it");
    let s = shape(&c);
    assert_eq!((s.who.as_str(), s.summary.as_str(), s.enter), ("r wants to run", "set -e · 3 lines", Enter::Deny));
    assert!(matches!(&s.parts[1], Part::Reason(r) if r == "ships it"));
    let patch = "diff --git a/x b/x\n--- a/src/a.rs\n+++ b/src/a.rs\n+x\n+y\n-z\n--- a/b.rs\n+++ b/b.rs\n+q\n--- a/c.rs\n+++ b/c.rs\n-w\n\nrewrites it";
    let s = shape(&card(2, "approval", "r", patch));
    assert_eq!(s.title, "r wants to edit src/a.rs");
    assert_eq!(s.summary, "src/a.rs +2 files · +3 −2");
    let s = shape(&card(3, "question", "perf", PERF));
    assert_eq!(s.short, vec!["compress", "both"]);
    assert_eq!(s.summary, "the hero image is 4.2 MB. compress it, or lazy-load it?");
    assert_eq!(short_labels(&["map a".into(), "map b".into()]), vec!["map a", "map b"]);
}

/// A sandbox card (brief 1e): 1 runs it again, 2 says it saves a "run
/// outside the sandbox" rule (designer): `it` when the rule is the command.
#[test]
fn a_sandbox_card_says_outside_the_sandbox() {
    let text = "wants to run it outside the sandbox\n| echo hi > ~/Desktop/x.txt\nreason: the sandbox stopped a write outside the repo: ~/Desktop/x.txt.\nalways: echo hi > ~/Desktop/x.txt";
    let s = shape(&card(4, "confirm", "t3", text));
    assert_eq!(s.options, vec!["run it again without the sandbox", "always run it outside the sandbox here", "no"]);
    let text = "wants to run it outside the sandbox\n| cp a.txt ~/Desktop/\nreason: the sandbox stopped a write outside the repo: ~/Desktop.\nalways: cp *";
    let s = shape(&card(5, "confirm", "t3", text));
    assert_eq!(s.options[1], "always run cp * outside the sandbox here");
    let text = "wants to run\n| npm run build\nreason: the checker is off, so commands ask first.\nalways: npm run build *";
    let s = shape(&card(6, "confirm", "t1", text));
    assert_eq!(s.options[1], "always allow npm run build * here");
}


/// 1, 1, 3: each answer opens the next item; the last one answered, the
/// box goes, your draft comes back, the divider says `✓ inbox clear` for
/// 2 s; each answer has its line in the thread; the agent's `?` in the
/// panel turns back at once.
#[test]
fn the_last_answer_clears_the_inbox() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = vec![
        card(1, "question", "a", "banner: smaller, or gone?\n1. smaller\n2. gone"),
        card(2, "question", "b", "now or later?\n1. now\n2. later\n3. never"),
    ];
    app.ed.insert("and once it ships");
    open(&mut app);
    key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(2), "the next one opens by itself");
    assert!(app.sb.answered_here(1));
    let rows = draw(&mut app, 140, 40);
    row_of(&rows, "│ ✓ you answered a: smaller");
    key(&mut app, KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 1 smaller", "/answer 2 never"]);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "and once it ships", "your draft back");
    let rows = draw(&mut app, 140, 40);
    assert!(!rows.iter().any(|r| r.contains("╭─ inbox")), "{}", rows.join("\n"));
    let d = row_of(&rows, "you → main");
    assert!(rows[d].contains("✓ inbox clear"), "{}", rows[d]);
    row_of(&rows, "✓ you answered a: smaller");
    row_of(&rows, "✓ you answered b: never");
}

/// One line per answer in the thread (the user: « ça se retrouve
/// affiché 2 fois »): the box's fold line, never the hub's route line
/// under it; the hub's line, in a feed this TUI wrote no fold in (an
/// answer from another view, a reload), reads as the same fold.
#[test]
fn an_answer_says_one_line_in_the_thread() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    ctrl(&mut app, 'n');
    assert!(key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(sent(&mut hub), vec!["/answer 12 both: compress, and lazy-load below the fold"]);
    let line = |agent: &str, l: &str| {
        serde_json::json!({"ev": "line", "agent": agent, "line": format!("sb route : {l}")}).to_string()
    };
    let folds = |evs: &[Ev]| -> Vec<String> {
        evs.iter()
            .filter_map(|e| match e {
                Ev::Approval { text, .. } => Some(text.clone()),
                Ev::Info(t) if t.contains("answer to card") => Some(t.clone()),
                _ => None,
            })
            .collect()
    };
    // the hub's line for it comes back to main's feed: nothing more
    let focus = app.sb.focus.clone();
    super::super::dispatch(&mut app, &line(&focus, "you → @perf (answer to card #12) : both: compress, and lazy-load below the fold"));
    assert_eq!(folds(&app.events), vec!["you answered perf: both"]);
    // an answer given elsewhere: the hub's line, as a fold
    super::super::dispatch(&mut app, &line(&focus, "you → @docs (answer to card #40) : v2"));
    assert_eq!(folds(&app.events), vec!["you answered perf: both", "you answered docs: v2"]);
    // a message routed by hand stays as it was
    super::super::dispatch(&mut app, &line(&focus, "you → @docs : thanks"));
    assert!(matches!(app.events.last(), Some(Ev::Info(t)) if t == "→ you → @docs : thanks"));
}

/// update-card: the new-release item (designer): the head alone as the
/// title, `bise · v… is out` in the strip, the notes text, the running
/// version dim; `3` opens the release page here and the item stays.
#[test]
fn the_new_release_item() {
    let (mut app, _hub) = app_with_hub();
    let text = "bise v2026.10.2-5 is out\nthe inbox keeps your place\n/update from any thread\nyou're on v2026.10.2-4\n\n1. update now · your agents keep running\n2. later\n3. release notes ↗";
    let url = "https://github.com/gvergnaud/bise/releases/tag/v2026.10.2-5";
    app.sb.cards = vec![Card { place: Some("release:bbb".into()), link: Some(url.into()), ..card(21, "update", "main", text) }];
    let s = shape(&app.sb.cards[0]);
    assert_eq!((s.title.as_str(), s.who.as_str(), s.summary.as_str()), ("bise v2026.10.2-5 is out", "bise", "v2026.10.2-5 is out"));
    assert_eq!(s.options, ["update now · your agents keep running", "later", "release notes ↗"]);
    assert!(matches!(s.parts.first(), Some(Part::Text(t)) if t == "the inbox keeps your place\n/update from any thread"));
    assert!(matches!(s.parts.get(1), Some(Part::Evidence(t)) if t == "you're on v2026.10.2-4"));
    // no notes: no body
    let bare = card(22, "update", "main", "bise v2026.10.2-5 is out\nyou're on v2026.10.2-4\n\n1. update now · your agents keep running\n2. later\n3. release notes ↗");
    let s = shape(&bare);
    assert_eq!(s.parts.len(), 1);
    assert!(matches!(s.parts.first(), Some(Part::Evidence(t)) if t == "you're on v2026.10.2-4"));
    // 3: the page opens here, nothing sent, the item stays
    open(&mut app);
    crate::links::OPENED.with(|o| o.borrow_mut().clear());
    assert!(pick_digit(&mut app, 21, 3));
    crate::links::OPENED.with(|o| assert_eq!(*o.borrow(), [url.to_string()]));
    assert!(app.sb.card_by_id(21).is_some());
}

/// pr-design §6.3: the ready-to-merge item, as the hub writes it. Its
/// head line is the title, the facts dim; a digit answers with the digit
/// (the hub merges, or files it); `2` opens the PR here and leaves it.
#[test]
fn the_ready_to_merge_item() {
    let (mut app, mut hub) = app_with_hub();
    let text = "#409 is ready to merge\nthe cookie banner stops covering buy\napproved by alice · 6 of 6 checks pass · 3 commits · +84 −12\ngithub.com/acme/web/pull/409\n\n1. squash and merge\n2. open it on GitHub\n3. not yet";
    app.sb.cards = vec![Card { place: Some("wt:cookies".into()), pr: Some(409), ..card(20, "merge", "cookies", text) }];
    let s = shape(&app.sb.cards[0]);
    assert_eq!(s.title, "cookies: #409 is ready to merge");
    assert_eq!(s.summary, "#409 is ready to merge");
    assert_eq!(s.options, ["squash and merge", "open it on GitHub", "not yet"]);
    // designer: the title text, the facts and the link dim evidence, the link clickable
    assert!(matches!(s.parts.first(), Some(Part::Text(t)) if t == "the cookie banner stops covering buy"));
    assert!(matches!(s.parts.get(1), Some(Part::Evidence(t)) if t.starts_with("approved by alice")));
    assert!(matches!(s.parts.get(2), Some(Part::Evidence(t)) if t == "[github.com/acme/web/pull/409](https://github.com/acme/web/pull/409)"));
    assert_eq!(kind_look("merge").2, theme::accent());
    open(&mut app);
    // 2: the link opens here (the text's, no box in this test); nothing sent, the item stays
    crate::links::OPENED.with(|o| o.borrow_mut().clear());
    assert!(key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE));
    assert!(sent(&mut hub).is_empty());
    assert_eq!(crate::links::OPENED.with(|o| o.borrow().clone()), ["https://github.com/acme/web/pull/409"]);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(20));
    // 1: the digit goes to the hub, the history says the option's words
    assert!(key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE));
    assert_eq!(sent(&mut hub), vec!["/answer 20 1"]);
    assert!(matches!(app.events.last(), Some(Ev::Approval { ok: true, text, .. }) if text == "you answered cookies: squash and merge"));
}

/// Voice mode (design §5): the item to answer by voice and its options;
/// `heard "…" → 1 …` on it for 1.5 s, gone on esc (undo), and the
/// answer counts as the key would.
#[test]
fn an_answer_by_voice_shows_then_counts() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    // the top item: the approval, its options
    let q = voice_question(&app).unwrap();
    assert_eq!((q.id, q.approval), (14, true));
    assert_eq!(q.options, vec!["allow once", "always here", "deny"]);
    open(&mut app);
    let now = std::time::Instant::now();
    show_heard(14, "heard \"allow\" → allow once".into(), now);
    let rows = draw(&mut app, 140, 40);
    row_of(&rows, "heard \"allow\" → allow once");
    // only on its item, only for 1.5 s
    assert_eq!(heard_on(12, now), None);
    assert_eq!(heard_on(14, now + std::time::Duration::from_millis(1600)), None);
    // esc undoes: the line goes, nothing sent
    undo_heard();
    assert!(!draw(&mut app, 140, 40).iter().any(|r| r.contains("heard \"allow\"")));
    assert!(sent(&mut hub).is_empty());
    // it counts: the same answer as the key 1
    show_heard(14, "heard \"allow\" → allow once".into(), now);
    assert!(answer_by_voice(&mut app, 14, 0));
    assert_eq!(heard_on(14, now), None);
    assert_eq!(sent(&mut hub), vec!["/answer 14 allow once"]);
    // then the question: its two options
    let q = voice_question(&app).unwrap();
    assert_eq!((q.id, q.approval, q.options.len()), (12, false, 2));
}

/// card-paste (cards #390, #395): an answer typed in a card carries
/// what the user pasted, like a message to the thread: a long paste's
/// text inline in its tag, a pasted image as its marker with the saved
/// path, never the bare chips `[Paste #1]` / `[Image #2]`.
#[test]
fn an_answer_carries_its_paste_and_its_image() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = vec![card(395, "question", "ci", "what does the build print?")];
    open(&mut app);
    app.ed.insert("here: ");
    let out: String = (1..=20).map(|i| format!("error[E0{i:03}]: line {i}\n")).collect();
    crate::pasted::add(&mut app, &out);
    // a pasted screenshot, already in the image store (fake: no file written)
    let stored = bend_images::Stored {
        kind: bend_images::Kind::Png,
        width: 800,
        height: 600,
        file: "/fake/.bise/images/ab12.png".into(),
        b64: "/fake/.bise/images/ab12.b64".into(),
    };
    let n = crate::attach::next_number(&mut app);
    let l = crate::attach::label(n);
    let marker = bend_images::marker(&l, &stored.file.to_string_lossy(), &stored);
    app.attachments.push(crate::attach::Attachment { label: l.clone(), marker, info: Default::default() });
    crate::attach::insert_chip(&mut app.ed, &l);
    assert_eq!(app.ed.text, "here: [Paste #1] [Image #2] ");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    let s = sent(&mut hub);
    assert_eq!(s.len(), 1, "{s:?}");
    let a = &s[0];
    assert!(a.starts_with("/answer 395 here: <pasted n=\"1\" lines=\"20\">\nerror[E0001]: line 1\n"), "{a}");
    assert!(a.contains("error[E0020]: line 20\n</pasted>"), "{a}");
    assert!(a.contains("path=\"/fake/.bise/images/ab12.png\""), "{a}");
    // no bare chip left (the marker keeps the label as its name)
    assert!(!a.contains("[Paste #"), "{a}");
    assert!(!a.replace("<image name=\"[Image #2]\"", "").contains("[Image #"), "{a}");
    assert!(app.attachments.is_empty(), "sent with the answer");
}
