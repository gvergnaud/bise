use super::*;
use crate::diffview::{lines, Ask, Diff};
use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use serde_json::json;

/// pricing.tsx: its head, its hunk head, then 7 lines: 38 39 40 (both),
/// old 41 42 removed, new 41 added, 43/42 (both).
fn one_file() -> serde_json::Value {
    json!({"ev": "diff", "req": 1, "title": "pricing-page vs main", "branch": "pricing-page",
        "files": [{"path": "src/pages/pricing.tsx", "status": "M", "add": 1, "del": 2, "abs": "/w/src/pages/pricing.tsx", "hunks": [
            {"old": 38, "new": 38, "head": "export function Pricing()", "lines": [
                " export function Pricing() {", "   return (", "     <section className=\"plans\">",
                "-      <Banner text=\"save 20% this week\" />", "-      <Plan name=\"free\" />",
                "+      <Plan name=\"free\" note=\"for side projects\" />", "       <Plan name=\"team\" highlight />"]}]}]})
}

/// The app on `focus`'s view with the panel open on the right (or full
/// screen), the keys its, its rows built, its body at rows 3.. of 30.
fn app(side: bool) -> App {
    let mut app = crate::sb::bench::test_app_drained();
    app.sb.focus = "pricing-page".into();
    crate::diffview::request(&mut app, Ask::Agent("pricing-page".into()), crate::diffview::By::Key);
    let p = app.diff.as_mut().unwrap();
    p.diff = Some(Diff::of(&one_file()));
    p.side = side;
    let _ = lines(p, 78, 30, 0);
    p.area = Rect { x: 70, y: 0, width: 80, height: 30 };
    p.body = Rect { x: 71, y: 3, width: 78, height: 27 };
    app
}

fn key(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}

/// Keys as the app gives them: the panel's first, else the composer's.
fn type_keys(app: &mut App, s: &str) {
    for c in s.chars() {
        let k = key(KeyCode::Char(c), KeyModifiers::NONE);
        if !crate::diffview::on_key(app, &k) {
            crate::input::composer_key(app, &k);
        }
    }
}

#[test]
fn the_rows_quote_with_where_they_are() {
    let a = app(true);
    let p = a.diff.as_ref().unwrap();
    let d = p.diff.as_ref().unwrap();
    // rows 4..=6: line 40, old 41-42 removed... up to the added 41
    let q = quotes(&p.rows, &d.files, 4, 7);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].0, Where { file: "src/pages/pricing.tsx".into(), new: "40-41".into(), old: "40-42".into() });
    assert_eq!(q[0].1, "     <section className=\"plans\">\n-      <Banner text=\"save 20% this week\" />\n-      <Plan name=\"free\" />\n+      <Plan name=\"free\" note=\"for side projects\" />");
    // only removed lines: no new lines
    let q = quotes(&p.rows, &d.files, 5, 6);
    assert_eq!(q[0].0, Where { file: "src/pages/pricing.tsx".into(), new: String::new(), old: "41-42".into() });
    let qq = crate::quote::Quote { from: "x".into(), text: q[0].1.clone(), at: q[0].0.clone() };
    assert_eq!(crate::quote::about(&qq), "src/pages/pricing.tsx:41-42 · 2 removed lines");
    // the file head and the hunk head are left out; nothing else: none
    assert_eq!(quotes(&p.rows, &d.files, 0, 2)[0].0.new, "38");
    assert!(quotes(&p.rows, &d.files, 0, 1).is_empty());
}

/// shift+↓ selects from the cursor; the key bar is in quote mode; a
/// letter puts the chip in the composer you're in, then types; ⏎ sends
/// the tag with the file and the lines, then the words.
#[test]
fn shift_arrows_select_and_typing_quotes() {
    let mut app = app(true);
    app.diff.as_mut().unwrap().cursor = 2;
    for _ in 0..2 {
        assert!(crate::diffview::on_key(&mut app, &key(KeyCode::Down, KeyModifiers::SHIFT)));
    }
    assert_eq!(app.diff.as_ref().unwrap().sel.map(|s| s.range()), Some((2, 4)));
    assert_eq!(crate::keybar::mode(&app), crate::keybar::Mode::Quote);
    assert_eq!(text(&app).as_deref(), Some(" export function Pricing() {\n   return (\n     <section className=\"plans\">"));
    type_keys(&mut app, "why?");
    assert_eq!(app.ed.text, "[Quote #1] why?");
    assert!(app.diff.as_ref().unwrap().sel.is_none(), "the selection is taken");
    assert!(app.diff.is_some(), "the panel stays");
    let rows: Vec<String> = crate::attach::strip_lines(&app, 90).iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
    assert!(rows[1].contains("src/pages/pricing.tsx:38-40 · 3 lines"), "{rows:?}");
    crate::input::on_key(&mut app, &key(KeyCode::Enter, KeyModifiers::NONE));
    let sent = app.history[0].clone();
    assert!(sent.starts_with("<selection from=\"pricing-page vs main\" file=\"src/pages/pricing.tsx\" new=\"38-40\" old=\"38-40\">\n export function Pricing() {\n"), "{sent}");
    assert!(sent.ends_with("</selection>\nwhy?"), "{sent}");
}

/// esc ends the selection (the panel stays); a plain ↓ too, and moves.
#[test]
fn esc_and_a_plain_arrow_end_the_selection() {
    let mut app = app(true);
    app.diff.as_mut().unwrap().cursor = 2;
    crate::diffview::on_key(&mut app, &key(KeyCode::Down, KeyModifiers::SHIFT));
    assert!(crate::diffview::on_key(&mut app, &key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.diff.as_ref().is_some_and(|p| p.sel.is_none()));
    crate::diffview::on_key(&mut app, &key(KeyCode::Down, KeyModifiers::SHIFT));
    assert!(crate::diffview::on_key(&mut app, &key(KeyCode::Down, KeyModifiers::NONE)));
    let p = app.diff.as_ref().unwrap();
    assert!(p.sel.is_none());
    assert_eq!(p.cursor, 5, "2, two shift+↓, then ↓");
}

fn mouse(app: &mut App, kind: MouseEventKind, row: u16) {
    crate::diffview::mouse(app, &MouseEvent { kind, column: 90, row, modifiers: KeyModifiers::NONE });
}

/// A drag over 3 lines selects them whole and copies them; the popup
/// sits above the first, names the agent in view; a click without a
/// move selects nothing.
#[test]
fn a_drag_selects_lines_and_the_popup_names_who_gets_it() {
    let mut app = app(true);
    // body row k is screen row 3 + k
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 3 + 5);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 3 + 6);
    assert!(!selected(&app), "not while dragging");
    // out of the panel's body the drag keeps going (and stops at its edge)
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 3 + 7);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 3 + 7);
    assert!(selected(&app));
    assert_eq!(app.diff.as_ref().unwrap().sel.map(|s| s.range()), Some((5, 7)));
    assert!(app.flash.as_ref().is_some_and(|(f, _)| f.contains("copied")), "{:?}", app.flash);
    let p = app.diff.as_ref().unwrap();
    let r = hint_rect(p, 30).unwrap();
    assert_eq!((r.x, r.y), (71 + 12, 3 + 4), "above the first line, at the code");
    // the whole frame: the pill over the panel, the tint on the rows
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 30)).unwrap();
    t.draw(|f| crate::run::draw_frame(&mut app, f)).unwrap();
    let b = t.backend().buffer().clone();
    let screen: Vec<String> = (0..30).map(|y| (0..150).map(|x| b[(x, y)].symbol()).collect()).collect();
    assert!(screen.iter().any(|r| r.contains(" type to ask pricing-page about it · cmd+c copy ")), "{screen:#?}");
    // a plain click: no selection
    let p = app.diff.as_ref().unwrap();
    let y = p.body.y + 3;
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), y);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), y);
    assert!(app.diff.as_ref().unwrap().sel.is_none());
}

/// Full screen (no composer in sight): a letter quotes, the panel
/// closes, the letter types after the chip.
#[test]
fn full_screen_a_letter_quotes_and_closes_the_panel() {
    let mut app = app(false);
    app.diff.as_mut().unwrap().cursor = 5;
    crate::diffview::on_key(&mut app, &key(KeyCode::Down, KeyModifiers::SHIFT));
    // `f` would be the file list: with lines selected it quotes
    type_keys(&mut app, "fix");
    assert!(app.diff.is_none());
    assert_eq!(app.ed.text, "[Quote #1] fix");
    let q = crate::quote::of(&app.attachments[0]).unwrap();
    assert_eq!(q.at.old, "41-42");
    assert_eq!(q.at.new, "");
}
