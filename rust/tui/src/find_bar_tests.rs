//! The find bar: its place, its parts, the composer's editing keys in
//! its field, its chevrons and × by mouse. (The pages of older lines
//! find asks for: sb/bench.rs, `find_pages_in_the_older_lines`.)

use super::*;
use crate::app::App;
use crate::wire::{Ev, Mark};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn app_with(events: Vec<Ev>) -> App {
    let mut app = crate::sb::bench::test_app();
    app.events = events;
    app.cache.clear();
    app
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    crate::input::on_key(app, &KeyEvent::new(code, m));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn draw(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| crate::run::draw_frame(app, f)).unwrap();
    t
}

fn click(app: &mut App, r: Rect) {
    let m = |kind| MouseEvent { kind, column: r.x + r.width / 2, row: r.y, modifiers: KeyModifiers::NONE };
    crate::input::on_mouse(app, &m(MouseEventKind::Down(MouseButton::Left)), 40);
    crate::input::on_mouse(app, &m(MouseEventKind::Up(MouseButton::Left)), 40);
}

fn query(app: &App) -> (String, usize, Option<(usize, usize)>) {
    let f = app.find.as_ref().unwrap();
    (f.ed.text.clone(), f.ed.cursor, f.ed.selection())
}

#[test]
fn the_box_sits_flush_in_the_pane_corner() {
    let r = box_rect(Rect::new(1, 1, 100, 30)).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (1 + 100 - 48, 1, 48, 3));
    assert_eq!(box_rect(Rect::new(0, 1, 30, 20)).unwrap().width, 30);
    assert!(box_rect(Rect::new(0, 1, 19, 20)).is_none());
    assert!(box_rect(Rect::new(0, 1, 90, 3)).is_none());
}

#[test]
fn the_parts_keep_the_field_before_the_counter() {
    let bx = Rect::new(0, 0, 48, 3);
    let inner = Rect::new(1, 1, 46, 1);
    let p = parts(bx, inner, 5);
    assert_eq!((p.prev.x, p.next.x, p.close.x), (38, 41, 44));
    assert_eq!(p.close.right(), inner.right());
    assert_eq!(p.count, Some(Rect::new(32, 1, 5, 1)));
    assert_eq!((p.field.x, p.field.right()), (4, 31));
    // narrow: the counter goes, the field and the buttons stay
    let p = parts(Rect::new(0, 0, 22, 3), Rect::new(1, 1, 20, 1), 18);
    assert!(p.count.is_none());
    assert!(p.field.width >= FIELD_MIN);
}

#[test]
fn a_long_query_scrolls_to_its_cursor() {
    let t = "abcdefghijklmnop";
    assert_eq!(scroll_to_cursor(t, 16, 0, 10), 7, "the cursor's cell is the last column");
    assert_eq!(scroll_to_cursor(t, 3, 7, 10), 3, "back left with the cursor");
    assert_eq!(scroll_to_cursor(t, 9, 7, 10), 7, "stays while the cursor is in view");
    assert_eq!(ci_at(t, 7, 0), 7);
    assert_eq!(ci_at(t, 7, 3), 10);
    assert_eq!(ci_at(t, 0, 99), 16);
}

/// The composer's keys in the field: shift+arrows select, alt/ctrl move
/// and delete by word, ctrl+a / ctrl+e (cmd+←/→ in Ghostty) and
/// home/end go to the line's ends, typing replaces the selection.
#[test]
fn the_field_has_the_composers_editing_keys() {
    let mut app = app_with(vec![Ev::You("the signup page works".into(), Mark::Sent, false)]);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "signup page");
    assert_eq!(query(&app), ("signup page".into(), 11, None));
    press(&mut app, KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(query(&app).1, 7, "alt+← a word left");
    press(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(query(&app).1, 0, "ctrl+a the start");
    press(&mut app, KeyCode::Right, KeyModifiers::SHIFT | KeyModifiers::ALT);
    assert_eq!(query(&app).2, Some((0, 6)), "shift+alt+→ selects a word");
    typed(&mut app, "login");
    assert_eq!(query(&app).0, "login page", "typing replaces the selection");
    press(&mut app, KeyCode::End, KeyModifiers::NONE);
    assert_eq!(query(&app).1, 10);
    press(&mut app, KeyCode::Home, KeyModifiers::SHIFT);
    assert_eq!(query(&app).2, Some((0, 10)), "shift+home selects to the start");
    press(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(query(&app).0, "login ", "alt+backspace deletes a word");
    press(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(query(&app).0, "");
    typed(&mut app, "x");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(query(&app).0, "", "ctrl+u (cmd+backspace) to the line start");
    // undo is the composer's too; the composer's draft never moves
    press(&mut app, KeyCode::Char('z'), KeyModifiers::SUPER);
    assert_eq!(query(&app).0, "x");
    assert!(app.ed.text.is_empty());
}

/// ↑ / ↓ still go through the matches; shift+↑ selects in the field.
#[test]
fn up_and_down_go_through_the_matches() {
    let mut app = app_with(vec![Ev::You("deploy one".into(), Mark::Sent, false), Ev::Assistant("deploy two".into())]);
    draw(&mut app, 120, 30);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "deploy");
    draw(&mut app, 120, 30);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    press(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)));
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    press(&mut app, KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(query(&app).2, Some((0, 6)));
}

/// The chevrons and the × by mouse; a press in the field places the
/// cursor, a drag selects.
#[test]
fn the_chevrons_and_the_cross_click() {
    let mut app = app_with(vec![Ev::You("deploy one".into(), Mark::Sent, false), Ev::Assistant("deploy two".into())]);
    draw(&mut app, 150, 40);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "deploy");
    let t = draw(&mut app, 150, 40);
    let p = app.find.as_ref().unwrap().bar.expect("drawn");
    let b = t.backend().buffer();
    assert_eq!(b[(p.prev.x + 1, p.prev.y)].symbol(), "↑");
    assert_eq!(b[(p.next.x + 1, p.next.y)].symbol(), "↓");
    assert_eq!(b[(p.close.x + 1, p.close.y)].symbol(), "×");
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    click(&mut app, p.prev);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)), "↑ older");
    click(&mut app, p.next);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)), "↓ newer");
    let at = |x: u16, kind| MouseEvent { kind, column: p.field.x + x, row: p.field.y, modifiers: KeyModifiers::NONE };
    crate::input::on_mouse(&mut app, &at(2, MouseEventKind::Down(MouseButton::Left)), 40);
    assert_eq!(query(&app).1, 2);
    crate::input::on_mouse(&mut app, &at(5, MouseEventKind::Drag(MouseButton::Left)), 40);
    crate::input::on_mouse(&mut app, &at(5, MouseEventKind::Up(MouseButton::Left)), 40);
    assert_eq!(query(&app).2, Some((2, 5)));
    click(&mut app, p.close);
    assert!(app.find.is_none(), "× closes");
}
