//! BISE-272: the pointer's shape, as the bytes the terminal gets.

use super::*;
use crate::links::LinkBackend;
use crate::run::draw_frame;
use crate::wire::Ev;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

const LINK: &str = "read [the guide](https://guide.example/start) first";

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

#[test]
fn ghostty_and_kitty_have_it_tmux_and_the_others_not() {
    assert!(supported_by(env(&[("TERM_PROGRAM", "ghostty"), ("TERM", "xterm-ghostty")])));
    assert!(supported_by(env(&[("TERM", "xterm-ghostty")])), "over ssh: TERM only");
    assert!(supported_by(env(&[("TERM", "xterm-kitty")])));
    assert!(supported_by(env(&[("TERM", "xterm-256color"), ("KITTY_WINDOW_ID", "1")])));
    // tmux does not pass OSC 22 on
    assert!(!supported_by(env(&[("TERM_PROGRAM", "tmux"), ("TERM", "tmux-256color"), ("TMUX", "/tmp/t,1,0")])));
    assert!(!supported_by(env(&[("TERM", "xterm-kitty"), ("TMUX", "/tmp/t,1,0")])));
    // nothing written to the terminals that do not have it
    assert!(!supported_by(env(&[("TERM_PROGRAM", "Apple_Terminal"), ("TERM", "xterm-256color")])));
    assert!(!supported_by(env(&[("TERM_PROGRAM", "iTerm.app"), ("TERM", "xterm-256color")])));
    assert!(!supported_by(env(&[("TERM_PROGRAM", "WezTerm"), ("TERM", "xterm-256color")])));
    assert!(!supported_by(env(&[])));
    // BISE_POINTER decides first
    assert!(!supported_by(env(&[("TERM_PROGRAM", "ghostty"), ("BISE_POINTER", "0")])));
    assert!(supported_by(env(&[("TERM_PROGRAM", "WezTerm"), ("BISE_POINTER", "1")])));
    assert!(supported_by(env(&[("TERM_PROGRAM", "ghostty"), ("BISE_POINTER", "auto")])));
}

#[test]
fn the_last_region_drawn_wins() {
    begin_frame();
    region(Rect::new(0, 0, 10, 2), Shape::Pointer);
    region(Rect::new(2, 1, 3, 1), Shape::Default);
    region(Rect::new(0, 5, 0, 1), Shape::Text);
    assert_eq!(at(0, 0), Shape::Pointer);
    assert_eq!(at(3, 1), Shape::Default, "a popup over it");
    assert_eq!(at(9, 1), Shape::Pointer);
    assert_eq!(at(10, 0), Shape::Default);
    assert_eq!(at(0, 5), Shape::Default, "an empty region is none");
    begin_frame();
    assert_eq!(at(0, 0), Shape::Default);
}

/// The screen, drawn by the real backend on a byte buffer.
struct Screen {
    app: App,
    term: Terminal<LinkBackend<Vec<u8>>>,
}

impl Screen {
    fn new(app: App) -> Screen {
        let area = Rect::new(0, 0, 120, 36);
        let term = Terminal::with_options(
            LinkBackend::new(Vec::<u8>::new()),
            ratatui::TerminalOptions { viewport: ratatui::Viewport::Fixed(area) },
        )
        .unwrap();
        let mut s = Screen { app, term };
        s.frame();
        s
    }

    /// One turn of the loop: the frame, then the pointer; the OSC 22s written.
    fn frame(&mut self) -> String {
        let app = &mut self.app;
        self.term.draw(|f| draw_frame(app, f)).unwrap();
        self.term.backend_mut().set_pointer(wanted(&self.app)).unwrap();
        osc22s(&self.term.backend().take_output())
    }

    fn mouse(&mut self, kind: MouseEventKind, x: u16, y: u16) -> String {
        crate::input::on_mouse(&mut self.app, &MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }, 36);
        self.frame()
    }

    fn hover(&mut self, x: u16, y: u16) -> String {
        self.mouse(MouseEventKind::Moved, x, y)
    }

    /// Where `text` is on the screen (the same frame on a test backend).
    fn find(&mut self, text: &str) -> (u16, u16) {
        let mut t = Terminal::new(TestBackend::new(120, 36)).unwrap();
        t.draw(|f| draw_frame(&mut self.app, f)).unwrap();
        let buf = t.backend().buffer().clone();
        // the pointer's frame is this one again: draw it on the real one
        self.frame();
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
            if let Some(i) = row.find(text) {
                return (row[..i].chars().count() as u16, y);
            }
        }
        panic!("{text} not on screen")
    }
}

/// The OSC 22 sequences of `out`, in order, joined.
fn osc22s(out: &[u8]) -> String {
    let s = String::from_utf8_lossy(out);
    let mut r = String::new();
    let mut rest = &s[..];
    while let Some(i) = rest.find("\x1b]22;") {
        let end = rest[i..].find("\x1b\\").map_or(rest.len(), |j| i + j + 2);
        r.push_str(&rest[i..end]);
        rest = &rest[end..];
    }
    r
}

const DEFAULT: &str = "\x1b]22;default\x1b\\";
const POINTER: &str = "\x1b]22;pointer\x1b\\";
const TEXT: &str = "\x1b]22;text\x1b\\";
const NS_RESIZE: &str = "\x1b]22;ns-resize\x1b\\";

fn feed_app() -> App {
    let mut app = crate::sb::bench::test_app();
    app.events.push(Ev::Assistant(LINK.to_string()));
    app.events.push(Ev::Thinking { ms: 1200, text: "the login breaks on safari".into(), open: false });
    app.events.push(Ev::Assistant("plain words here".into()));
    app
}

#[test]
fn a_link_has_the_hand_and_leaving_it_the_default() {
    let mut s = Screen::new(feed_app());
    let (x, y) = s.find("the guide");
    assert_eq!(s.hover(x + 2, y), POINTER);
    assert_eq!(s.hover(x + 3, y), "", "the same shape: nothing written");
    assert_eq!(s.frame(), "", "a redraw: nothing written");
    let (px, py) = s.find("plain words");
    assert_eq!(s.hover(px + 1, py), DEFAULT);
    // the link's last cell and the one after
    assert_eq!(s.hover(x + 8, y), POINTER);
    assert_eq!(s.hover(x + 9, y), DEFAULT, "the space after it");
}

#[test]
fn a_row_a_click_opens_has_the_hand() {
    let mut s = Screen::new(feed_app());
    let (x, y) = s.find("thought");
    assert_eq!(s.hover(x, y), POINTER, "the thinking row opens");
    let (px, py) = s.find("plain words");
    assert_eq!(s.hover(px, py), DEFAULT);
    // your long message: only its `▸ n more lines` row folds it
    let mut app = crate::sb::bench::test_app();
    let long: Vec<String> = (0..40).map(|k| format!("line {k} of the spec")).collect();
    app.events.push(Ev::You(long.join("\n"), crate::wire::Mark::Read, false, None));
    let mut s = Screen::new(app);
    let (fx, fy) = s.find("more lines");
    assert_eq!(s.hover(fx, fy), POINTER);
    let (lx, ly) = s.find("line 0 of the spec");
    assert_eq!(s.hover(lx, ly), DEFAULT, "its other rows are for reading");
}

#[test]
fn the_composer_has_the_text_cursor() {
    let mut s = Screen::new(feed_app());
    let c = s.app.composer;
    assert_eq!(s.hover(c.x + 2, c.y), TEXT);
    // a press and a drag there keep it, out of the composer too
    assert_eq!(s.mouse(MouseEventKind::Down(MouseButton::Left), c.x + 2, c.y), "");
    assert_eq!(s.mouse(MouseEventKind::Drag(MouseButton::Left), c.x + 2, c.y.saturating_sub(4)), "");
    assert_eq!(s.mouse(MouseEventKind::Up(MouseButton::Left), c.x + 2, c.y.saturating_sub(4)), DEFAULT);
    assert_eq!(s.hover(c.x + 2, c.y), TEXT);
    assert_eq!(s.hover(c.x + 2, c.y - 2), DEFAULT, "the divider");
}

#[test]
fn the_panel_rows_have_the_hand() {
    let mut app = crate::sb::bench::test_app();
    crate::sb::hub_reads::rows_for_tests::apply(&mut app, vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")], vec![]);
    let mut s = Screen::new(app);
    let (x, y) = s.find("docs");
    assert_eq!(s.hover(x, y), POINTER);
    assert_eq!(s.hover(x, y + 6), DEFAULT, "under the rows");
}

#[test]
fn the_terminal_panel_border_resizes_and_its_drag_keeps_the_shape() {
    let mut s = Screen::new(feed_app());
    s.app.term.show_bare();
    s.frame();
    let (x, y) = s.find(" terminal · ");
    assert_eq!(s.hover(x, y), NS_RESIZE);
    assert_eq!(s.hover(x, y + 2), DEFAULT, "the panel's inside");
    assert_eq!(s.hover(x, y), NS_RESIZE);
    assert_eq!(s.mouse(MouseEventKind::Down(MouseButton::Left), x, y), "");
    assert_eq!(s.mouse(MouseEventKind::Drag(MouseButton::Left), x, y - 5), "", "the drag keeps it");
    // the release: the shape under the mouse again, the border where it went
    let up = s.mouse(MouseEventKind::Up(MouseButton::Left), x, y - 5);
    let (x2, y2) = s.find(" terminal · ");
    assert!(y2 < y, "the panel grew");
    let back = s.hover(x2, y2);
    assert_eq!(format!("{up}{back}"), if up.is_empty() { String::new() } else { format!("{DEFAULT}{NS_RESIZE}") });
    assert_eq!(s.hover(x, 1), DEFAULT);
}

#[test]
fn focus_loss_and_the_exit_give_the_default_back() {
    let mut s = Screen::new(feed_app());
    let (x, y) = s.find("the guide");
    assert_eq!(s.hover(x, y), POINTER);
    s.app.focus_lost = true;
    assert_eq!(s.frame(), DEFAULT);
    s.app.focus_lost = false;
    assert_eq!(s.hover(x, y), POINTER);
    // the exit (run_tui): the default, once
    s.term.backend_mut().set_pointer(Shape::Default).unwrap();
    assert_eq!(osc22s(&s.term.backend().take_output()), DEFAULT);
    // the crash path writes it only when a shape is on
    let mut out = Vec::new();
    wrote(Shape::Pointer);
    restore(&mut out);
    assert_eq!(osc22s(&out), DEFAULT);
    out.clear();
    restore(&mut out);
    assert!(out.is_empty());
}

#[test]
fn the_help_over_a_link_covers_it() {
    let mut s = Screen::new(feed_app());
    let (x, y) = s.find("the guide");
    assert_eq!(s.hover(x, y), POINTER);
    s.app.help = Some(crate::help::Overlay::new(crate::help::Page::Shortcuts));
    assert_eq!(s.frame(), DEFAULT, "the help takes the mouse");
}

#[test]
fn off_nothing_is_written() {
    OFF.with(|c| c.set(true));
    let mut s = Screen::new(feed_app());
    let (x, y) = s.find("the guide");
    assert_eq!(s.hover(x, y), "");
    s.term.backend_mut().set_pointer(Shape::Default).unwrap();
    assert_eq!(osc22s(&s.term.backend().take_output()), "");
    OFF.with(|c| c.set(false));
}
