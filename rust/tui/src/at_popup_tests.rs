//! The `@` popup as a state machine driven by keys (input::on_key):
//! folders are browsed without leaving the popup, a file is inserted,
//! and no key on any row ever panics. The composer text is the whole
//! state: `@rust/tui/` lists that folder's entries.

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// One workspace for the whole file (the file index is per process).
pub(crate) fn ws() -> &'static str {
    static WS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    WS.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("at-popup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let long = "a-very-long-folder-name-that-goes-on-and-on/".repeat(4);
        for d in ["rust/tui/src/sb", "docs/my notes", "docs/émoji 👍", "empty", &long] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let files = [
            "rust/tui/Cargo.toml",
            "rust/tui/src/files.rs",
            "rust/tui/src/input.rs",
            "rust/tui/src/sb/mention.rs",
            "README.md",
            "docs/my notes/a.md",
            "docs/émoji 👍/日本語ファイル.md",
            &format!("{long}x.rs"),
        ];
        for f in files {
            std::fs::write(root.join(f), "").unwrap();
        }
        root.to_string_lossy().into_owned()
    })
}

fn app() -> App {
    let mut app = sb::bench::test_app();
    sb::bench::set_workspace(&mut app, ws());
    sb::bench::add_agent(&mut app, "notes-👍-agent", "an objective with émojis 🎉 and a long tail ".repeat(5).as_str());
    // the index is walked in the background: wait for it (up to 30 s: a
    // loaded machine took more than the 2 s this waited, and the test then
    // failed on an empty index, BISE-292)
    app.ed.set("@", 1);
    let t0 = std::time::Instant::now();
    while !rows(&app).iter().any(|r| r == "rust/") {
        assert!(t0.elapsed().as_secs() < 30, "the file index of {} is not ready after 30 s", ws());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.ed.clear();
    app
}

fn rows(app: &App) -> Vec<String> {
    commands::popup_items(app).into_iter().map(|c| c.label).collect()
}

fn key(app: &mut App, code: KeyCode) {
    input::on_key(app, &KeyEvent::new(code, KeyModifiers::NONE));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        key(app, KeyCode::Char(c));
    }
}

fn text(app: &App) -> (&str, usize) {
    (app.ed.text.as_str(), app.ed.cursor)
}

fn select(app: &mut App, label: &str) {
    let r = rows(app);
    app.popup_sel = r.iter().position(|l| l == label).unwrap_or_else(|| panic!("no row {label:?} in {r:?}"));
}

#[test]
fn left_before_the_at_never_panics() {
    // the reported panic: the cursor on the `@` sliced chars[1..0]
    let mut app = app();
    typed(&mut app, "@");
    key(&mut app, KeyCode::Left);
    assert_eq!(text(&app), ("@", 0));
    assert!(rows(&app).is_empty()); // the cursor left the token: closed
    key(&mut app, KeyCode::Right);
    assert!(rows(&app).contains(&"rust/".to_string()));
    // same for `$` (skills) and Home on an inline token
    app.ed.set("$", 0);
    let _ = commands::popup_items(&app);
    app.ed.set("see @ru", 4);
    let _ = commands::popup_items(&app);
    key(&mut app, KeyCode::Home);
}

#[test]
fn browse_into_folders_then_pick_a_file() {
    let mut app = app();
    typed(&mut app, "@rust/");
    assert_eq!(rows(&app)[0], "rust/tui/");
    // → on a folder: the popup lists its entries, the `@` stays
    key(&mut app, KeyCode::Right);
    assert_eq!(text(&app), ("@rust/tui/", 10));
    // folders first, the folder itself last (a recent pick of another
    // test may come first: compare as a set)
    let mut r = rows(&app);
    assert_eq!(r.pop().as_deref(), Some("rust/tui/"));
    r.sort();
    assert_eq!(r, ["rust/tui/Cargo.toml", "rust/tui/src/"]);
    // Enter on a folder browses too
    select(&mut app, "rust/tui/src/");
    key(&mut app, KeyCode::Enter);
    assert_eq!(text(&app), ("@rust/tui/src/", 14));
    assert_eq!(rows(&app), ["rust/tui/src/sb/", "rust/tui/src/files.rs", "rust/tui/src/input.rs", "rust/tui/src/"]);
    // typing narrows inside the folder
    typed(&mut app, "fi");
    assert_eq!(rows(&app), ["rust/tui/src/files.rs"]);
    // Enter on a file: the final reference, the popup closed
    key(&mut app, KeyCode::Enter);
    assert_eq!(text(&app), ("rust/tui/src/files.rs ", 22));
    assert!(rows(&app).is_empty());
}

#[test]
fn tab_browses_a_folder_and_inserts_a_file() {
    let mut app = app();
    typed(&mut app, "look at @rust/tu");
    select(&mut app, "rust/tui/");
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.ed.text, "look at @rust/tui/");
    select(&mut app, "rust/tui/Cargo.toml");
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.ed.text, "look at rust/tui/Cargo.toml ");
}

#[test]
fn left_and_backspace_go_one_folder_up() {
    let mut app = app();
    typed(&mut app, "@rust/tui/src/");
    key(&mut app, KeyCode::Left);
    assert_eq!(text(&app), ("@rust/tui/", 10));
    key(&mut app, KeyCode::Backspace);
    assert_eq!(text(&app), ("@rust/", 6));
    key(&mut app, KeyCode::Left);
    assert_eq!(text(&app), ("@", 1));
    assert!(rows(&app).contains(&"rust/".to_string()));
    // a name part: Backspace deletes a char, ← moves
    typed(&mut app, "rust/tu");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(text(&app), ("@rust/t", 7));
    key(&mut app, KeyCode::Left);
    assert_eq!(text(&app), ("@rust/t", 6));
}

#[test]
fn right_on_a_file_or_an_agent_moves_the_cursor() {
    let mut app = app();
    typed(&mut app, "@READ now");
    app.ed.set("@READ now", 5);
    select(&mut app, "README.md");
    key(&mut app, KeyCode::Right);
    assert_eq!(text(&app), ("@READ now", 6));
    app.ed.set("@notes", 6);
    assert!(rows(&app)[0].starts_with("@notes"));
    key(&mut app, KeyCode::Right);
    assert_eq!(text(&app), ("@notes", 6));
}

#[test]
fn inline_browse_keeps_the_rest_of_the_line() {
    let mut app = app();
    app.ed.set("see @ru now", 7);
    select(&mut app, "rust/");
    key(&mut app, KeyCode::Right);
    assert_eq!(text(&app), ("see @rust/ now", 10));
    select(&mut app, "rust/tui/");
    key(&mut app, KeyCode::Enter);
    select(&mut app, "rust/tui/Cargo.toml");
    key(&mut app, KeyCode::Enter);
    assert_eq!(text(&app), ("see rust/tui/Cargo.toml now", 24));
}

#[test]
fn the_folder_itself_is_the_last_row() {
    let mut app = app();
    typed(&mut app, "@rust/tui/");
    // ↑ from the first row wraps to it; ⏎ inserts the folder reference
    key(&mut app, KeyCode::Up);
    key(&mut app, KeyCode::Enter);
    assert_eq!(text(&app), ("rust/tui/ ", 10));
    // a folder query that is not a folder path (fuzzy `tui/`) has none
    app.ed.set("@tui/", 5);
    assert!(!rows(&app).contains(&"tui/".to_string()));
}

#[test]
fn a_folder_with_a_space_is_browsed_quoted() {
    let mut app = app();
    typed(&mut app, "@docs/");
    select(&mut app, "docs/my notes/");
    key(&mut app, KeyCode::Right);
    assert_eq!(app.ed.text, "@\"docs/my notes/");
    assert_eq!(rows(&app)[0], "docs/my notes/a.md");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.ed.text, "\"docs/my notes/a.md\" ");
    // up from a quoted folder
    app.ed.set("@\"docs/my notes/", 16);
    key(&mut app, KeyCode::Left);
    assert_eq!(app.ed.text, "@docs/");
}

#[test]
fn an_empty_folder_leaves_the_popup_empty_and_up_works() {
    let mut app = app();
    typed(&mut app, "@empty/");
    assert!(rows(&app).is_empty());
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.ed.text, "@");
}

#[test]
fn popup_selection_steps_are_total() {
    use input::popup_step;
    assert_eq!(popup_step(0, 0, true), 0);
    assert_eq!(popup_step(0, 0, false), 0);
    assert_eq!(popup_step(0, 3, false), 2);
    assert_eq!(popup_step(2, 3, true), 0);
    assert_eq!(popup_step(99, 3, true), 0); // stale
    assert_eq!(popup_step(usize::MAX, 3, false), 0);
}

#[test]
fn long_labels_keep_their_end() {
    assert_eq!(ui::truncate_left("abcdef", 10), "abcdef");
    assert_eq!(ui::truncate_left("abcdef", 4), "…def");
    assert_eq!(ui::truncate_left("ab日本語", 5), "…本語");
    assert_eq!(ui::truncate_left("abc", 0), "");
}

/// Every key on every row of every popup state, drawn at several
/// widths: no panic, the selection stays in range.
#[test]
fn every_key_on_every_row_never_panics() {
    let mut app = app();
    let keys = [
        KeyCode::Right,
        KeyCode::Left,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Esc,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ];
    let queries = [
        ("@", 1),
        ("@", 0),
        ("@r", 2),
        ("@rust/", 6),
        ("@rust/tui/", 10),
        ("@src/", 5),
        ("@zzz", 4),
        ("@empty/", 7),
        ("@日本", 3),
        ("@a-very", 7),
        ("@\"docs/my notes/", 16),
        ("@docs/é", 7),
        ("x @do y", 5),
        ("@no", 3),
        ("\"@", 2),
        ("@\"", 2),
    ];
    for (q, cur) in queries {
        app.ed.set(q, cur);
        app.popup_dismissed = None;
        let n = commands::popup_items(&app).len();
        for row in [0, n / 2, n.saturating_sub(1), n + 3] {
            for k in keys {
                for w in [12u16, 40, 150] {
                    app.ed.set(q, cur);
                    app.popup_dismissed = None;
                    app.popup_sel = row;
                    for _ in 0..3 {
                        // Enter with no popup sends to the hub (the test
                        // socket is never read: it would fill and block)
                        if k == KeyCode::Enter && commands::popup_items(&app).is_empty() {
                            break;
                        }
                        key(&mut app, k);
                        let mut term = Terminal::new(TestBackend::new(w, 30)).unwrap();
                        term.draw(|f| sb::draw_sb(&mut app, f)).unwrap();
                    }
                }
            }
        }
    }
}

/// Rows once the outside folder is read (a background read): up to
/// 30 s, like the index above (a 1 s bound failed on a loaded machine,
/// docs/issues/10-tests-wait.md); returns as soon as a file row shows.
fn rows_read(app: &App) -> Vec<String> {
    let t0 = std::time::Instant::now();
    loop {
        let r = rows(app);
        if r.iter().any(|l| !l.ends_with("/")) {
            return r;
        }
        assert!(t0.elapsed().as_secs() < 30, "the outside folder is not read after 30 s: {r:?}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn dot_dot_browses_outside_the_workspace() {
    // BISE-206: `@../` lists the folder typed so far, not the index
    let mut app = app();
    let me = std::path::Path::new(ws()).file_name().unwrap().to_string_lossy().into_owned();
    typed(&mut app, &format!("@../{me}/"));
    let r = rows_read(&app);
    assert_eq!(r.last().map(String::as_str), Some(format!("../{me}/").as_str()));
    assert!(r.contains(&format!("../{me}/rust/")) && r.contains(&format!("../{me}/README.md")), "{r:?}");
    // tab browses into a folder, ⏎ on a file inserts it as typed
    select(&mut app, &format!("../{me}/rust/"));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.ed.text, format!("@../{me}/rust/"));
    typed(&mut app, "tui/Ca");
    select(&mut app, &format!("../{me}/rust/tui/Cargo.toml"));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.ed.text, format!("../{me}/rust/tui/Cargo.toml "));
    // ← goes one folder up, back to the workspace from `@../`
    app.ed.clear();
    typed(&mut app, "@../");
    key(&mut app, KeyCode::Left);
    assert_eq!(text(&app), ("@", 1));
}
