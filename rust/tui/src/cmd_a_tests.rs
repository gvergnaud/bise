//! cmd+a (SUPER+a under the kitty keyboard protocol, e.g. Ghostty with
//! `keybind = super+a=unbind`, BISE-267): the whole composer text
//! selected, the next key typed replaces it. Only the composer: the
//! help, find and the agent palette type nothing and keep the draft.

use crate::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    crate::input::on_key(app, &KeyEvent::new(code, m));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn cmd_a(app: &mut App) {
    press(app, KeyCode::Char('a'), KeyModifiers::SUPER);
}

#[test]
fn cmd_a_selects_the_whole_composer_and_typing_replaces_it() {
    let mut app = crate::sb::bench::test_app();
    typed(&mut app, "first line");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    typed(&mut app, "second é line");
    press(&mut app, KeyCode::Left, KeyModifiers::NONE);
    cmd_a(&mut app);
    let n = app.ed.text.chars().count();
    assert_eq!(app.ed.selection(), Some((0, n)));
    assert_eq!(app.ed.selected_text().as_deref(), Some("first line\nsecond é line"));
    assert!(app.key_in_composer, "cmd+a is a composer key (zen)");
    typed(&mut app, "x");
    assert_eq!(app.ed.text, "x");
    assert_eq!(app.ed.selection(), None);
    // an empty composer: nothing to select, nothing breaks
    press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
    cmd_a(&mut app);
    assert_eq!((app.ed.text.as_str(), app.ed.selection()), ("", None));
    // backspace on everything selected clears it
    typed(&mut app, "draft");
    cmd_a(&mut app);
    press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(app.ed.text, "");
}

#[test]
fn cmd_a_elsewhere_types_nothing_and_keeps_the_draft() {
    let mut app = crate::sb::bench::test_app();
    typed(&mut app, "my draft");
    // the help overlay: no `a` in its filter
    app.help = Some(crate::help::Overlay::new(crate::help::Page::Shortcuts));
    cmd_a(&mut app);
    assert_eq!(app.help.as_ref().unwrap().filter, "");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.help.is_none());
    // find and the agent palette: their query untouched
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    cmd_a(&mut app);
    assert_eq!(app.find.as_ref().unwrap().ed.text, "");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    cmd_a(&mut app);
    assert_eq!(app.palette.as_ref().unwrap().query, "");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    // back in the composer: the draft whole, nothing selected by them
    assert_eq!(app.ed.text, "my draft");
    assert_eq!(app.ed.selection(), None);
    cmd_a(&mut app);
    assert_eq!(app.ed.selected_text().as_deref(), Some("my draft"));
}

fn ed_state(app: &App) -> (usize, Option<(usize, usize)>) {
    (app.ed.cursor, app.ed.selection())
}

/// cmd+↑ / cmd+↓ (SUPER+Up/Down under the kitty keyboard protocol, what
/// Ghostty sends with `keybind = super+arrow_up=unbind` and the three
/// other arrow lines /setup offers): the very start / end of a
/// multi-line text, wherever the cursor is; with shift they select from
/// the cursor to there.
#[test]
fn cmd_up_and_down_go_to_the_text_start_and_end_shift_selects() {
    let mut app = crate::sb::bench::test_app();
    typed(&mut app, "one");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    typed(&mut app, "two");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    typed(&mut app, "three");
    let n = app.ed.text.chars().count();
    // the cursor in the middle row, mid-word: t|wo
    let mid = 5;
    app.ed.cursor = mid;
    press(&mut app, KeyCode::Up, KeyModifiers::SUPER);
    assert_eq!(ed_state(&app), (0, None));
    assert!(app.key_in_composer, "cmd+↑ is a composer key (zen)");
    press(&mut app, KeyCode::Down, KeyModifiers::SUPER);
    assert_eq!(ed_state(&app), (n, None));
    // cmd+↑ at the start (and cmd+↓ at the end) stay: no history recall
    app.history = vec!["an old message".into()];
    press(&mut app, KeyCode::Up, KeyModifiers::SUPER);
    press(&mut app, KeyCode::Up, KeyModifiers::SUPER);
    assert_eq!((app.ed.text.chars().count(), ed_state(&app)), (n, (0, None)));
    // shift: from the cursor to the start, then to the end
    app.ed.cursor = mid;
    press(&mut app, KeyCode::Up, KeyModifiers::SUPER | KeyModifiers::SHIFT);
    assert_eq!(ed_state(&app), (0, Some((0, mid))));
    assert_eq!(app.ed.selected_text().as_deref(), Some(&app.ed.text[..mid]));
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    app.ed.cursor = mid;
    press(&mut app, KeyCode::Down, KeyModifiers::SUPER | KeyModifiers::SHIFT);
    assert_eq!(ed_state(&app), (n, Some((mid, n))));
    assert_eq!(app.ed.selected_text().as_deref(), Some("wo\nthree"));
    // typing replaces the selection
    typed(&mut app, "!");
    assert_eq!(app.ed.text, "one\nt!");
    // a plain cmd+↓ drops a selection
    press(&mut app, KeyCode::Up, KeyModifiers::SUPER | KeyModifiers::SHIFT);
    press(&mut app, KeyCode::Down, KeyModifiers::SUPER);
    assert_eq!(ed_state(&app), (6, None));
}
