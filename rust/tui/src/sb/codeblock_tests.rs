//! The markdown code blocks of a reply (codeblock.rs): the box, the
//! colors, the copy icon on hover, the click and ctrl+y copies.

use super::bench::test_app;
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    let w = buf.area.width as usize;
    buf.content.chunks(w).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

fn row_of(term: &Terminal<TestBackend>, text: &str) -> u16 {
    screen(term).iter().position(|l| l.contains(text)).unwrap_or_else(|| panic!("{:?} not on screen", text)) as u16
}

fn press(app: &mut App, x: u16, y: u16) {
    let m = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE };
    crate::input::on_mouse(app, &m, 30);
}

const REPLY: &str = "  obs: assistant: run this:\\n```ts\\nconst answer = 42 // the one\\n```\\nthen this:\\n```bash\\nls -la\\n```";

#[test]
fn a_reply_draws_its_code_blocks_boxed_and_colored() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    super::entries_for_tests::lines(&mut app, "main", &[REPLY]);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let s = screen(&term);
    let top = row_of(&term, "╭─ ts ─") as usize;
    // (the screen's own frame is around: `╮  │`)
    assert!(s[top].contains("─╮  │"), "{:#?}", s);
    assert!(s[top + 1].contains("│ const answer = 42 // the one"), "{:#?}", s);
    assert!(s[top + 1].contains(" │  │"), "{:#?}", s);
    assert!(s[top + 2].contains("╰─"), "{:#?}", s);
    assert!(s.iter().any(|l| l.contains("╭─ bash ─")), "{:#?}", s);
    // the box ends inside the 80 columns
    let corner = s[top].chars().position(|c| c == '╮').unwrap();
    assert!(corner < 80);
    // the composer's highlighter colors the code
    let buf = term.backend().buffer();
    let x = s[top + 1].chars().position(|c| c == 'c').unwrap() as u16;
    assert_eq!(buf[(x, top as u16 + 1)].fg, crate::theme::syntax_keyword());
    let x = s[top + 1].find("42").map(|b| s[top + 1][..b].chars().count()).unwrap() as u16;
    assert_eq!(buf[(x, top as u16 + 1)].fg, crate::theme::syntax_number());
    // no icon until the mouse comes
    assert!(!s.iter().any(|l| l.contains(" copy ")));
}

#[test]
fn the_mouse_over_a_block_shows_the_copy_icon_and_a_click_copies_its_code() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    super::entries_for_tests::lines(&mut app, "main", &[REPLY]);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let before = screen(&term);
    let top = row_of(&term, "╭─ ts ─");
    // over the code row: the icon on the top border, before `─╮`
    app.hover = Some((10, top + 1));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let after = screen(&term);
    assert!(after[top as usize].contains("── copy ─╮  │"), "{:?}", after[top as usize]);
    for (k, (a, b)) in before.iter().zip(&after).enumerate() {
        if k != top as usize {
            assert_eq!(a, b, "row {} changed", k);
        }
    }
    // only the block under the mouse
    let bash_top = row_of(&term, "╭─ bash ─") as usize;
    assert!(!after[bash_top].contains(" copy "));
    // a click on the icon copies the code as written, and says so
    let hit = app.copy_hit.expect("the icon is clickable");
    press(&mut app, hit.rect.x + 1, hit.rect.y);
    assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some("const answer = 42 // the one"));
    assert!(app.flash.as_ref().is_some_and(|(t, _)| t.starts_with("copied")));
    assert!(app.feed_sel.is_none(), "the click starts no selection");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(screen(&term)[top as usize].contains("✓ copied ─╮"), "{:?}", screen(&term)[top as usize]);
    // the mouse away, the icon goes (the `copied` stays a moment)
    app.hover = Some((10, row_of(&term, "then this")));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.copy_hit.is_none());
    assert!(!screen(&term).iter().any(|l| l.contains(" copy ")));
}

#[test]
fn ctrl_y_copies_the_block_under_the_mouse_else_the_newest_on_screen() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    super::entries_for_tests::lines(&mut app, "main", &[REPLY]);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let ctrl_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL);
    crate::input::on_key(&mut app, &ctrl_y);
    assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some("ls -la"));
    let top = row_of(&term, "╭─ ts ─");
    app.hover = Some((10, top + 1));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    crate::input::on_key(&mut app, &ctrl_y);
    assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some("const answer = 42 // the one"));
}

#[test]
fn a_copy_gives_a_wrapped_line_whole_without_marks_fence_or_tag() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    let long = "export async function load(id: string) { const res = await fetch(url + id); return res.json() } // long";
    let code = format!("\tindented();\n{long}");
    let msg = format!("  obs: assistant: see:\\n```ts\\n{}\\n```", code.replace('\n', "\\n"));
    super::entries_for_tests::lines(&mut app, "main", &[&msg]);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(screen(&term).iter().any(|l| l.contains("│ » ")), "{:#?}", screen(&term));
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some(code.as_str()));
}

#[test]
fn no_code_on_screen_ctrl_y_says_so() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    super::entries_for_tests::lines(&mut app, "main", &["  obs: assistant: plain words"]);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(app.flash.as_ref().map(|(t, _)| t.as_str()), Some("no code block on screen"));
}
