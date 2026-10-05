//! The composer's undo and redo (undo.rs, editor.rs): editors' steps,
//! the cursor and the selection back with each, ctrl+z / ctrl+shift+z /
//! ctrl+y on the keys, a send starting over.

use crate::editor::{action, Action, Editor, Motion, Unit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn typed(e: &mut Editor, s: &str) {
    for c in s.chars() {
        e.insert(&c.to_string());
    }
}

fn state(e: &Editor) -> (&str, usize, Option<(usize, usize)>) {
    (e.text.as_str(), e.cursor, e.selection())
}

#[test]
fn a_typing_run_is_one_step_per_word_with_its_cursor() {
    let mut e = Editor::default();
    typed(&mut e, "fix the bug");
    e.undo();
    assert_eq!(state(&e), ("fix the ", 8, None));
    e.undo();
    e.undo();
    assert_eq!(state(&e), ("", 0, None));
    assert!(!e.can_undo());
    e.redo();
    assert_eq!(state(&e), ("fix ", 4, None));
}

#[test]
fn a_paste_is_one_step() {
    let mut e = Editor::default();
    typed(&mut e, "see ");
    e.paste("a long pasted log");
    typed(&mut e, "!");
    e.undo();
    assert_eq!(state(&e), ("see a long pasted log", 21, None));
    e.undo();
    assert_eq!(state(&e), ("see ", 4, None));
}

#[test]
fn a_delete_word_is_one_step_and_the_cursor_comes_back() {
    let mut e = Editor::default();
    typed(&mut e, "hello brave world");
    e.move_cursor(Motion::WordLeft, false); // before "world"
    e.delete_back(Unit::Word);
    e.delete_back(Unit::Word);
    assert_eq!(state(&e), ("world", 0, None));
    e.undo();
    assert_eq!(state(&e), ("hello world", 6, None));
    e.undo();
    assert_eq!(state(&e), ("hello brave world", 12, None));
    e.redo();
    assert_eq!(state(&e), ("hello world", 6, None));
}

#[test]
fn the_selection_comes_back_with_its_step() {
    let mut e = Editor::default();
    typed(&mut e, "ship it now");
    e.select_range(5, 7);
    typed(&mut e, "this");
    assert_eq!(state(&e), ("ship this now", 9, None));
    e.undo();
    assert_eq!(state(&e), ("ship it now", 7, Some((5, 7))));
    // a cut too
    e.select_range(0, 5);
    assert_eq!(e.cut().as_deref(), Some("ship "));
    e.undo();
    assert_eq!(state(&e), ("ship it now", 5, Some((0, 5))));
}

#[test]
fn a_history_recall_is_one_step_back_to_the_draft() {
    let hist = vec!["newest".to_string(), "older".to_string()];
    let mut e = Editor::default();
    typed(&mut e, "dr");
    assert!(e.history_up(&hist));
    assert!(e.history_up(&hist));
    assert_eq!(e.text, "older");
    e.undo();
    assert_eq!(state(&e), ("dr", 2, None));
    assert!(!e.browsing(), "the undo leaves the history");
    e.redo();
    assert_eq!(e.text, "older");
    assert!(e.browsing());
    // back to the draft with ↓: still the user's draft at the end
    assert!(e.history_down(&hist));
    assert!(e.history_down(&hist));
    assert_eq!(state(&e), ("dr", 2, None));
    assert!(!e.browsing());
}

#[test]
fn sending_clears_both_stacks() {
    let mut e = Editor::default();
    typed(&mut e, "go now");
    e.undo();
    assert!(e.can_undo() && e.can_redo());
    assert_eq!(e.take(), "go ");
    assert!(!e.can_undo() && !e.can_redo());
}

#[test]
fn ctrl_z_undoes_and_ctrl_shift_z_redoes() {
    let k = |c, m| action(&KeyEvent::new(KeyCode::Char(c), m));
    let (c, s) = (KeyModifiers::CONTROL, KeyModifiers::SHIFT);
    assert_eq!(k('z', c), Some(Action::Undo));
    // the kitty protocol reports ctrl+shift+z as Z or z with SHIFT
    assert_eq!(k('Z', c | s), Some(Action::Redo));
    assert_eq!(k('z', c | s), Some(Action::Redo));
}

/// Through the app's keys: ctrl+z undoes the composer, then says there
/// is no undo of what was sent; ctrl+y redoes right after an undo.
#[test]
fn the_app_keys_undo_redo_then_say_no_undo() {
    let mut app = crate::sb::bench::test_app_drained();
    let press = |app: &mut crate::App, c: char, m: KeyModifiers| {
        crate::input::on_key(app, &KeyEvent::new(KeyCode::Char(c), m));
    };
    for ch in "hi there".chars() {
        press(&mut app, ch, KeyModifiers::NONE);
    }
    press(&mut app, 'w', KeyModifiers::CONTROL);
    assert_eq!(app.ed.text, "hi ");
    press(&mut app, 'z', KeyModifiers::CONTROL);
    assert_eq!((app.ed.text.as_str(), app.ed.cursor), ("hi there", 8));
    press(&mut app, 'y', KeyModifiers::CONTROL);
    assert_eq!((app.ed.text.as_str(), app.ed.cursor), ("hi ", 3));
    press(&mut app, 'z', KeyModifiers::CONTROL);
    press(&mut app, 'z', KeyModifiers::CONTROL);
    press(&mut app, 'z', KeyModifiers::CONTROL);
    assert_eq!(app.ed.text, "");
    let no_undo = |app: &crate::App| {
        app.events.iter().filter(|e| matches!(e, crate::wire::Ev::Info(t) if t.starts_with("no undo"))).count()
    };
    assert_eq!(no_undo(&app), 0);
    press(&mut app, 'z', KeyModifiers::CONTROL);
    assert_eq!(no_undo(&app), 1, "nothing left to undo: no undo of what was sent");
}
