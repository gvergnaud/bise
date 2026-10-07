//! The embedded terminal: Ctrl+` shows a real shell (a PTY, `$SHELL` in
//! the workspace) at the bottom of the window and gives it the keyboard;
//! Ctrl+` again hides it (the shell keeps running) and the composer gets
//! the keys back. The shell is spawned on the first show, respawned on a
//! show after it exited, and killed when the TUI exits.
//!
//! The mouse (BISE-250): a drag in the panel selects (highlighted, the
//! history's tint), the release copies it, like the history; a double
//! click selects the word, a triple the row; cmd+c or ctrl+shift+c copy
//! the selection again. A program that asked for the mouse (vim, less
//! --mouse, htop) gets it instead, and shift+drag still selects, as in
//! any terminal. No quote into the composer: while the panel is shown
//! the keys go to the shell, so "select, then type" has nothing to type
//! into.
//!
//! Pure parts (tested): `is_toggle`, `key_bytes`, `mouse_bytes`,
//! `split`, `Sel`. The rest is the PTY plumbing: a reader thread feeds a
//! vt100 parser that the `tui-term` widget draws.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use portable_pty::{Child, MasterPty, PtySize};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tui_term::widget::PseudoTerminal;
use vt100::{MouseProtocolEncoding, MouseProtocolMode};

const SCROLLBACK: usize = 5000;
const MIN_ROWS: u16 = 5;

/// Ctrl+`. With the kitty DISAMBIGUATE flag it arrives as '`' + Ctrl; a
/// legacy terminal (tmux without extended keys) sends NUL for it, which
/// crossterm reads as Ctrl+Space (also Ctrl+@ and Ctrl+2).
pub(crate) fn is_toggle(k: &KeyEvent) -> bool {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let others = k.modifiers - KeyModifiers::CONTROL - KeyModifiers::SHIFT;
    ctrl && others.is_empty() && matches!(k.code, KeyCode::Char('`') | KeyCode::Char(' ') | KeyCode::Char('@'))
}

/// The bytes a key sends to the shell, xterm style. `app_cursor`: the
/// program asked for the application cursor keys (ESC O A…).
pub(crate) fn key_bytes(k: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let m = k.modifiers;
    // cmd+key is the app's (copy) or the terminal's, never the shell's:
    // a terminal sends nothing for it
    if m.contains(KeyModifiers::SUPER) {
        return None;
    }
    let alt = m.contains(KeyModifiers::ALT);
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let shift = m.contains(KeyModifiers::SHIFT);
    // the xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl
    let param = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;
    let esc = |alt: bool, mut b: Vec<u8>| {
        if alt {
            b.insert(0, 0x1b);
        }
        b
    };
    // arrows, Home/End: ESC [ X, ESC O X in application mode, ESC [1;m X
    let letter = |c: u8| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[1;{}{}", param, c as char).into_bytes()
        } else if app_cursor {
            vec![0x1b, b'O', c]
        } else {
            vec![0x1b, b'[', c]
        }
    };
    // Insert/Delete/PgUp/PgDn/F5+: ESC [ n ~, ESC [ n;m ~
    let tilde = |n: u8| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[{};{}~", n, param).into_bytes()
        } else {
            format!("\x1b[{}~", n).into_bytes()
        }
    };
    Some(match k.code {
        KeyCode::Char(c) if ctrl => {
            let b = match c.to_ascii_lowercase() {
                c @ 'a'..='z' => c as u8 - b'a' + 1,
                ' ' | '@' | '2' | '`' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '_' | '-' | '/' | '7' => 0x1f,
                '?' | '8' => 0x7f,
                _ => return None,
            };
            esc(alt, vec![b])
        }
        KeyCode::Char(c) => esc(alt, c.to_string().into_bytes()),
        KeyCode::Enter => esc(alt, vec![b'\r']),
        KeyCode::Tab if shift => b"\x1b[Z".to_vec(),
        KeyCode::Tab => esc(alt, vec![b'\t']),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace if ctrl => esc(alt, vec![0x08]),
        KeyCode::Backspace => esc(alt, vec![0x7f]),
        KeyCode::Esc => esc(alt, vec![0x1b]),
        KeyCode::Up => letter(b'A'),
        KeyCode::Down => letter(b'B'),
        KeyCode::Right => letter(b'C'),
        KeyCode::Left => letter(b'D'),
        KeyCode::Home => letter(b'H'),
        KeyCode::End => letter(b'F'),
        KeyCode::Insert => tilde(2),
        KeyCode::Delete => tilde(3),
        KeyCode::PageUp => tilde(5),
        KeyCode::PageDown => tilde(6),
        KeyCode::F(n @ 1..=4) => {
            let c = b'P' + (n - 1);
            if param > 1 {
                format!("\x1b[1;{}{}", param, c as char).into_bytes()
            } else {
                vec![0x1b, b'O', c]
            }
        }
        KeyCode::F(n @ 5..=12) => tilde([15, 17, 18, 19, 20, 21, 23, 24][(n - 5) as usize]),
        _ => return None,
    })
}

/// cmd+c, or ctrl+shift+c (the Linux terminals' copy): copies the
/// panel's selection when there is one.
pub(crate) fn is_copy(k: &KeyEvent) -> bool {
    let m = k.modifiers;
    let c = matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'));
    c && (m == KeyModifiers::SUPER
        || m == KeyModifiers::SUPER | KeyModifiers::SHIFT
        || m == KeyModifiers::CONTROL | KeyModifiers::SHIFT)
}

/// The bytes that report a mouse event to a program that asked for the
/// mouse (`mode`, in `enc`), at the 0-based cell (x, y) of the panel;
/// none when the mode does not report this kind of event.
pub(crate) fn mouse_bytes(
    kind: MouseEventKind,
    mods: KeyModifiers,
    x: u16,
    y: u16,
    mode: MouseProtocolMode,
    enc: MouseProtocolEncoding,
) -> Option<Vec<u8>> {
    use MouseProtocolMode as M;
    let button = |b: MouseButton| match b {
        MouseButton::Left => 0u16,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    // (button code, a release)
    let (code, release) = match kind {
        _ if mode == M::None => return None,
        MouseEventKind::Down(b) => (button(b), false),
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        MouseEventKind::Up(b) if mode != M::Press => (button(b), true),
        MouseEventKind::Drag(b) if matches!(mode, M::ButtonMotion | M::AnyMotion) => (button(b) + 32, false),
        // motion with no button: code 3 + motion
        MouseEventKind::Moved if mode == M::AnyMotion => (35, false),
        _ => return None,
    };
    let mut code = code;
    if mods.contains(KeyModifiers::SHIFT) {
        code += 4;
    }
    if mods.contains(KeyModifiers::ALT) {
        code += 8;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        code += 16;
    }
    let (cx, cy) = (x as u32 + 1, y as u32 + 1);
    Some(match enc {
        MouseProtocolEncoding::Sgr => format!("\x1b[<{};{};{}{}", code, cx, cy, if release { 'm' } else { 'M' }).into_bytes(),
        legacy => {
            // X10: a release is button 3 (which one is not said); each
            // number + 32 as one byte (UTF-8 mode: one char), 223 at most
            // in the byte form
            let code = if release { (code & !3) | 3 } else { code } as u32;
            let mut out = b"\x1b[M".to_vec();
            for n in [code + 32, cx + 32, cy + 32] {
                if legacy == MouseProtocolEncoding::Utf8 {
                    let mut b = [0u8; 4];
                    out.extend(char::from_u32(n.min(2047))?.encode_utf8(&mut b).as_bytes());
                } else {
                    out.push(n.min(255) as u8);
                }
            }
            out
        }
    })
}

/// A cell of the shell's whole text: the row counted from the oldest row
/// kept in the history (so a selection stays on its text when the view
/// scrolls), the column.
pub(crate) type Cell = (usize, u16);

/// A selection in the panel: where the press was and where the pointer
/// is, both cells included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sel {
    pub(crate) anchor: Cell,
    pub(crate) head: Cell,
}

impl Sel {
    /// (first, last), in text order.
    pub(crate) fn range(&self) -> (Cell, Cell) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// The columns [from, to) selected on the row, if any.
    pub(crate) fn cols(&self, row: usize, width: u16) -> Option<(u16, u16)> {
        let (a, b) = self.range();
        if row < a.0 || row > b.0 {
            return None;
        }
        let from = if row == a.0 { a.1 } else { 0 };
        let to = if row == b.0 { b.1.saturating_add(1) } else { width };
        Some((from.min(width), to.min(width)))
    }
}

/// The rows of the history (scrollback) the parser keeps now.
fn history_len(p: &mut vt100::Parser) -> usize {
    let at = p.screen().scrollback();
    p.set_scrollback(usize::MAX);
    let n = p.screen().scrollback();
    p.set_scrollback(at);
    n
}

/// The text of the columns [from, to) of the row `row` (see [`Cell`]),
/// and whether the row goes on in the next one (a soft wrap). Moves the
/// parser's view: the caller puts it back.
fn row_text(p: &mut vt100::Parser, row: usize, from: u16, to: u16) -> (String, bool) {
    let hist = history_len(p);
    let r = if row < hist {
        p.set_scrollback(hist - row);
        0
    } else {
        p.set_scrollback(0);
        row - hist
    };
    let Ok(r) = u16::try_from(r) else { return (String::new(), false) };
    let s = p.screen();
    if r >= s.size().0 || from >= to {
        return (String::new(), s.row_wrapped(r));
    }
    (s.contents_between(r, from, r, to), s.row_wrapped(r))
}

/// The selected text: soft-wrapped rows joined, a newline between the
/// others, trailing blanks of each line dropped.
fn sel_text(p: &mut vt100::Parser, sel: &Sel) -> String {
    let at = p.screen().scrollback();
    let cols = p.screen().size().1;
    let (a, b) = sel.range();
    let mut out = String::new();
    for row in a.0..=b.0 {
        let Some((from, to)) = sel.cols(row, cols) else { continue };
        let (text, wrapped) = row_text(p, row, from, to);
        out.push_str(&text);
        if row < b.0 && !(wrapped && to >= cols) {
            let kept = out.trim_end_matches(' ').len();
            out.truncate(kept);
            out.push('\n');
        }
    }
    p.set_scrollback(at);
    out.trim_end_matches(' ').to_string()
}

/// The screen split: the area left for the app above, the panel below
/// (`pct` % of the height, at least MIN_ROWS + borders, the app keeps 8).
pub(crate) fn split(full: Rect, pct: u16) -> (Rect, Rect) {
    let want = (full.height as u32 * pct as u32 / 100) as u16;
    let h = want.max(MIN_ROWS + 2).min(full.height.saturating_sub(8)).max(3.min(full.height));
    let top = Rect { height: full.height - h, ..full };
    let panel = Rect { y: full.y + full.height - h, height: h, ..full };
    (top, panel)
}

struct Pty {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    exited: Arc<AtomicBool>,
    size: (u16, u16),
}

impl Pty {
    /// `argv` (a shell, or an editor on a file: BISE-264) in `cwd`.
    /// (bise's one pty spawn, pty.rs: his shell's environment rule)
    fn spawn(argv: &[&str], cwd: &str, rows: u16, cols: u16) -> Result<Pty, String> {
        let crate::pty::Spawned { master, child } = crate::pty::spawn(argv, std::path::Path::new(cwd), rows, cols)?;
        let mut reader = master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = master.take_writer().map_err(|e| e.to_string())?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let exited = Arc::new(AtomicBool::new(false));
        let (p, x) = (parser.clone(), exited.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut p) = p.lock() {
                            p.process(&buf[..n]);
                        }
                    }
                }
            }
            x.store(true, Ordering::SeqCst);
        });
        Ok(Pty { parser, writer, master, child, exited, size: (rows, cols) })
    }

    fn alive(&mut self) -> bool {
        !self.exited.load(Ordering::SeqCst) && matches!(self.child.try_wait(), Ok(None))
    }

    fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if self.size != (rows, cols) && rows > 0 && cols > 0 {
            self.size = (rows, cols);
            let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
            if let Ok(mut p) = self.parser.lock() {
                p.set_size(rows, cols);
            }
        }
    }

    /// SIGHUP (what closing a terminal does), then SIGKILL if the shell
    /// is still there after a short grace.
    fn kill(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.child.kill();
        for _ in 0..20 {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Some(pid) = self.child.process_id() {
            // no libc dependency: the kill command does it
            let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        let _ = self.child.wait();
    }
}

/// The panel's state, held by the App.
pub(crate) struct Term {
    shown: bool,
    pty: Option<Pty>,
    /// the panel height, % of the window
    pct: u16,
    /// where the panel was drawn (mouse hits)
    area: Option<Rect>,
    /// where the shell's cells were drawn, and the row (see [`Cell`]) of
    /// the first one
    inner: Option<Rect>,
    top: usize,
    /// dragging the top border
    resizing: bool,
    /// rows scrolled back into the history
    scroll: usize,
    error: Option<String>,
    sel: Option<Sel>,
    /// a press made the selection and the button is still down: Some(it
    /// selects: a drag, a double or triple click)
    selecting: Option<bool>,
    /// a press went to the program: its drag and release go too
    forwarding: bool,
    clicks: crate::app::MouseState,
    /// BISE-264: an editor runs in the panel (a click on a file link):
    /// the shell it replaced (if any), whether the panel was shown, and
    /// the editor's name (the title). Back when the editor exits.
    parked: Option<(Option<Pty>, bool, String)>,
}

impl Default for Term {
    fn default() -> Self {
        Term {
            shown: false,
            pty: None,
            pct: 30,
            area: None,
            inner: None,
            top: 0,
            resizing: false,
            scroll: 0,
            error: None,
            sel: None,
            selecting: None,
            forwarding: false,
            clicks: Default::default(),
            parked: None,
        }
    }
}

/// What the panel did with a mouse event.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MouseDone {
    /// not the panel's: the app takes it
    Pass,
    Took,
    /// a selection was made: copy this text
    Copy(String),
}

impl Term {
    pub(crate) fn shown(&self) -> bool {
        self.shown
    }

    /// The panel shown with no shell (tests: its frame and border).
    #[cfg(test)]
    pub(crate) fn show_bare(&mut self) {
        self.shown = true;
    }

    /// Its top border is being dragged (BISE-272: `ns-resize`).
    pub(crate) fn resizing(&self) -> bool {
        self.shown && self.resizing
    }

    /// A press in the panel still holds the mouse: its selection, or a
    /// program's press (BISE-272: the default shape until the release).
    pub(crate) fn mouse_held(&self) -> bool {
        self.shown && (self.selecting.is_some() || self.forwarding)
    }

    /// BISE-264: run `argv` (a terminal editor on a file) in the panel,
    /// shown, with the keys; the shell waits behind it and comes back
    /// when it exits. One editor at a time.
    pub(crate) fn run(&mut self, cwd: &str, argv: &[String]) -> Result<(), String> {
        self.restore();
        if let Some((_, _, name)) = &self.parked {
            let msg = format!("{} is already open in the terminal panel: quit it first", name);
            self.shown = true;
            return Err(msg);
        }
        let (rows, cols) = self.pty.as_ref().map_or((10, 80), |p| p.size);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let p = Pty::spawn(&args, cwd, rows, cols).map_err(|e| format!("cannot start {}: {}", argv[0], e))?;
        let name = std::path::Path::new(&argv[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let shell = self.pty.replace(p);
        self.parked = Some((shell, self.shown, name));
        self.shown = true;
        self.scroll = 0;
        self.sel = None;
        self.error = None;
        Ok(())
    }

    /// The editor exited: the shell back, the panel as it was.
    fn restore(&mut self) {
        if self.parked.is_none() || self.pty.as_mut().is_some_and(|p| p.alive()) {
            return;
        }
        if let Some(mut p) = self.pty.take() {
            p.kill();
        }
        if let Some((shell, shown, _)) = self.parked.take() {
            self.pty = shell;
            self.shown = shown;
        }
        self.scroll = 0;
        self.sel = None;
        self.selecting = None;
        self.forwarding = false;
    }

    /// Show (spawning the shell in `cwd` if none runs) or hide.
    pub(crate) fn toggle(&mut self, cwd: &str) {
        self.restore();
        if self.shown {
            self.shown = false;
            self.resizing = false;
            return;
        }
        self.shown = true;
        self.scroll = 0;
        let alive = self.pty.as_mut().is_some_and(|p| p.alive());
        if !alive {
            if let Some(mut p) = self.pty.take() {
                p.kill();
            }
            self.sel = None;
            // the real size comes with the first draw
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            match Pty::spawn(&[&shell], cwd, 10, 80) {
                Ok(p) => {
                    self.pty = Some(p);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("cannot start the shell: {}", e)),
            }
        }
    }

    /// A key while shown: to the shell (the caller handles the toggle).
    /// False when hidden: the key is the app's.
    pub(crate) fn key(&mut self, k: &KeyEvent) -> bool {
        if !self.shown {
            return false;
        }
        let Some(pty) = self.pty.as_mut() else { return true };
        // Shift+PgUp/PgDn scroll the history, like most terminals
        if k.modifiers == KeyModifiers::SHIFT && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown) {
            let page = (pty.size.0 as usize / 2).max(1);
            self.scroll = if k.code == KeyCode::PageUp {
                self.scroll + page
            } else {
                self.scroll.saturating_sub(page)
            };
            return true;
        }
        let app_cursor = pty.parser.lock().map(|p| p.screen().application_cursor()).unwrap_or(false);
        if let Some(b) = key_bytes(k, app_cursor) {
            self.scroll = 0;
            self.sel = None;
            pty.send(&b);
        }
        true
    }

    /// The selected text, if any (cmd+c).
    pub(crate) fn selection_text(&mut self) -> Option<String> {
        let sel = self.sel?;
        let pty = self.pty.as_mut()?;
        let mut p = pty.parser.lock().ok()?;
        Some(sel_text(&mut p, &sel)).filter(|t| !t.is_empty())
    }

    /// A paste while shown goes to the shell (bracketed when it asked).
    pub(crate) fn paste(&mut self, text: &str) -> bool {
        if !self.shown {
            return false;
        }
        if let Some(pty) = self.pty.as_mut() {
            let bracketed = pty.parser.lock().map(|p| p.screen().bracketed_paste()).unwrap_or(false);
            let text = text.replace("\r\n", "\r").replace('\n', "\r");
            if bracketed {
                pty.send(format!("\x1b[200~{}\x1b[201~", text).as_bytes());
            } else {
                pty.send(text.as_bytes());
            }
            self.scroll = 0;
        }
        true
    }

    /// The mouse over the panel: the wheel scrolls the history, the top
    /// border drags to resize, a drag selects (a program that asked for
    /// the mouse gets it instead, unless shift is held).
    pub(crate) fn mouse(&mut self, m: &MouseEvent, screen_h: u16) -> MouseDone {
        if !self.shown {
            return MouseDone::Pass;
        }
        if self.resizing {
            match m.kind {
                MouseEventKind::Drag(MouseButton::Left) => {
                    let h = screen_h.saturating_sub(m.row).max(1);
                    self.pct = ((h as u32 * 100 / screen_h.max(1) as u32) as u16).clamp(10, 90);
                }
                MouseEventKind::Up(_) => self.resizing = false,
                _ => {}
            }
            return MouseDone::Took;
        }
        // a selection or a forwarded press goes on outside the panel too
        if self.selecting.is_some() {
            return self.select_mouse(m);
        }
        if self.forwarding {
            if matches!(m.kind, MouseEventKind::Up(_)) {
                self.forwarding = false;
            }
            self.forward(m);
            return MouseDone::Took;
        }
        let Some(r) = self.area else { return MouseDone::Pass };
        let inside = m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        if !inside {
            return MouseDone::Pass;
        }
        if m.kind == MouseEventKind::Down(MouseButton::Left) && m.row == r.y {
            self.resizing = true;
            return MouseDone::Took;
        }
        let in_cells = self.inner.is_some_and(|i| m.row >= i.y && m.row < i.y + i.height);
        let shift = m.modifiers.contains(KeyModifiers::SHIFT);
        if in_cells && !shift && self.program_mouse() != MouseProtocolMode::None {
            if matches!(m.kind, MouseEventKind::Down(_)) {
                self.forwarding = true;
            }
            self.forward(m);
            return MouseDone::Took;
        }
        match m.kind {
            MouseEventKind::ScrollUp => self.scroll += 3,
            MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_sub(3),
            MouseEventKind::Down(MouseButton::Left) if in_cells => return self.select_mouse(m),
            MouseEventKind::Down(_) => self.sel = None,
            _ => {}
        }
        MouseDone::Took
    }

    /// The mode of the mouse reports the program asked for.
    fn program_mouse(&self) -> MouseProtocolMode {
        let Some(pty) = self.pty.as_ref() else { return MouseProtocolMode::None };
        pty.parser.lock().map(|p| p.screen().mouse_protocol_mode()).unwrap_or(MouseProtocolMode::None)
    }

    /// Reports the event to the program, at its cell in the panel.
    fn forward(&mut self, m: &MouseEvent) {
        let (Some(i), Some(pty)) = (self.inner, self.pty.as_mut()) else { return };
        let x = m.column.saturating_sub(i.x).min(i.width.saturating_sub(1));
        let y = m.row.saturating_sub(i.y).min(i.height.saturating_sub(1));
        let (mode, enc) = pty
            .parser
            .lock()
            .map(|p| (p.screen().mouse_protocol_mode(), p.screen().mouse_protocol_encoding()))
            .unwrap_or((MouseProtocolMode::None, MouseProtocolEncoding::Default));
        if let Some(b) = mouse_bytes(m.kind, m.modifiers, x, y, mode, enc) {
            pty.send(&b);
        }
    }

    /// The cell under the pointer, clamped into the panel's cells.
    fn cell_at(&self, x: u16, y: u16) -> Option<Cell> {
        let i = self.inner?;
        if i.width == 0 || i.height == 0 {
            return None;
        }
        let col = x.clamp(i.x, i.x + i.width - 1) - i.x;
        let row = y.clamp(i.y, i.y + i.height - 1) - i.y;
        Some((self.top + row as usize, col))
    }

    /// A press, drag or release that selects (like the history: a double
    /// click the word, a triple the row; the release copies).
    fn select_mouse(&mut self, m: &MouseEvent) -> MouseDone {
        let Some(i) = self.inner else { return MouseDone::Took };
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(at) = self.cell_at(m.column, m.row) else { return MouseDone::Took };
                let clicks = self.clicks.press(m.column, m.row, std::time::Instant::now());
                let (a, b) = match clicks {
                    2 => self.word_at(at),
                    3 => (0, i.width.saturating_sub(1)),
                    _ => (at.1, at.1),
                };
                self.sel = Some(Sel { anchor: (at.0, a), head: (at.0, b) });
                self.selecting = Some(clicks > 1);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // past the top or the bottom row: the view scrolls
                if m.row < i.y {
                    self.scroll += 1;
                    self.top = self.top.saturating_sub(1);
                } else if m.row >= i.y + i.height && self.scroll > 0 {
                    self.scroll -= 1;
                    self.top += 1;
                }
                if let (Some(at), Some(sel)) = (self.cell_at(m.column, m.row), self.sel.as_mut()) {
                    if sel.head != at {
                        sel.head = at;
                        self.selecting = Some(true);
                    }
                }
            }
            MouseEventKind::Up(_) => {
                let selected = self.selecting.take() == Some(true);
                if !selected {
                    // a plain click drops the selection
                    self.sel = None;
                    return MouseDone::Took;
                }
                if let Some(t) = self.selection_text() {
                    return MouseDone::Copy(t);
                }
            }
            _ => {}
        }
        MouseDone::Took
    }

    /// The columns of the word at the cell (a double click).
    fn word_at(&mut self, at: Cell) -> (u16, u16) {
        let Some(pty) = self.pty.as_mut() else { return (at.1, at.1) };
        let Ok(mut p) = pty.parser.lock() else { return (at.1, at.1) };
        let back = p.screen().scrollback();
        let (text, _) = row_text(&mut p, at.0, 0, u16::MAX);
        p.set_scrollback(back);
        let (a, b) = crate::feedsel::word_cols(&text, at.1 as usize);
        (a.min(u16::MAX as usize) as u16, b.min(u16::MAX as usize) as u16)
    }

    /// Draw the panel when shown; returns the area left for the app.
    pub(crate) fn draw(&mut self, frame: &mut Frame, full: Rect) -> Rect {
        self.restore();
        if !self.shown {
            self.area = None;
            return full;
        }
        let (top, panel) = split(full, self.pct);
        self.area = Some(panel);
        // BISE-272: the top border drags to resize (`mouse`)
        crate::pointer::region(Rect { height: 1, ..panel }, crate::pointer::Shape::NsResize);
        let inner = Block::default().borders(Borders::ALL).inner(panel);
        let mut title = " terminal · ctrl+` hide ".to_string();
        let border = Style::default().fg(Color::DarkGray);
        let Some(pty) = self.pty.as_mut() else {
            let msg = self.error.clone().unwrap_or_default();
            frame.render_widget(
                ratatui::widgets::Paragraph::new(msg)
                    .block(Block::default().borders(Borders::ALL).title(title).border_style(border)),
                panel,
            );
            return top;
        };
        pty.resize(inner.height, inner.width);
        let alive = pty.alive();
        let Ok(mut parser) = pty.parser.lock() else { return top };
        let hist = history_len(&mut parser);
        parser.set_scrollback(self.scroll);
        // the clamped offset: the history may be shorter than asked
        self.scroll = parser.screen().scrollback();
        self.inner = Some(inner);
        self.top = hist - self.scroll;
        if self.scroll > 0 {
            title = format!(" terminal · ↑ {} lines · ctrl+` hide ", self.scroll);
        }
        if let Some((_, _, name)) = &self.parked {
            title = format!(" terminal · {} · ctrl+` hide ", name);
        } else if !alive {
            title = " terminal · the shell exited · ctrl+` twice for a new one ".to_string();
        }
        let block = Block::default().borders(Borders::ALL).title(title).border_style(border);
        let mut w = PseudoTerminal::new(parser.screen()).block(block);
        if self.scroll > 0 || !alive {
            w = w.cursor(tui_term::widget::Cursor::default().visibility(false));
        }
        frame.render_widget(w, panel);
        parser.set_scrollback(0);
        // the selection: the history's tint under its cells
        if let Some(sel) = self.sel {
            let buf = frame.buffer_mut();
            for y in 0..inner.height {
                let Some((a, b)) = sel.cols(self.top + y as usize, inner.width) else { continue };
                for x in a..b {
                    if let Some(c) = buf.cell_mut((inner.x + x, inner.y + y)) {
                        c.set_bg(crate::theme::selection_bg());
                    }
                }
            }
        }
        top
    }

    /// Kill the shell (the TUI exits).
    pub(crate) fn shutdown(&mut self) {
        if let Some(mut p) = self.pty.take() {
            p.kill();
        }
        if let Some((Some(mut p), _, _)) = self.parked.take() {
            p.kill();
        }
        self.shown = false;
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ---- the App glue (called from the run loop in lib.rs) ----

pub(crate) fn cwd(app: &crate::App) -> String {
    crate::sb::workspace(app)
        .or_else(|| std::env::current_dir().ok().map(|d| d.to_string_lossy().to_string()))
        .unwrap_or_else(|| ".".into())
}

/// A key event: Ctrl+` toggles; while shown every other key goes to the
/// shell. True when the terminal took the key.
pub(crate) fn on_key(app: &mut crate::App, k: &KeyEvent) -> bool {
    if k.kind != crossterm::event::KeyEventKind::Press {
        return app.term.shown();
    }
    if is_toggle(k) {
        let dir = cwd(app);
        app.term.toggle(&dir);
        return true;
    }
    // cmd+c / ctrl+shift+c with a selection: copy it (without one,
    // ctrl+shift+c is the shell's ctrl+c)
    if app.term.shown() && is_copy(k) {
        if let Some(t) = app.term.selection_text() {
            crate::input::copy_text(app, &t);
            return true;
        }
    }
    app.term.key(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn toggle_key() {
        assert!(is_toggle(&k(KeyCode::Char('`'), KeyModifiers::CONTROL)));
        // legacy NUL, as crossterm reads it
        assert!(is_toggle(&k(KeyCode::Char(' '), KeyModifiers::CONTROL)));
        assert!(is_toggle(&k(KeyCode::Char('@'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)));
        assert!(!is_toggle(&k(KeyCode::Char('`'), KeyModifiers::NONE)));
        assert!(!is_toggle(&k(KeyCode::Char(' '), KeyModifiers::NONE)));
        assert!(!is_toggle(&k(KeyCode::Char('`'), KeyModifiers::CONTROL | KeyModifiers::ALT)));
        assert!(!is_toggle(&k(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }

    fn b(code: KeyCode, m: KeyModifiers) -> Vec<u8> {
        key_bytes(&k(code, m), false).unwrap()
    }

    #[test]
    fn plain_and_control_keys() {
        assert_eq!(b(KeyCode::Char('a'), KeyModifiers::NONE), b"a");
        assert_eq!(b(KeyCode::Char('A'), KeyModifiers::SHIFT), b"A");
        assert_eq!(b(KeyCode::Char('é'), KeyModifiers::NONE), "é".as_bytes());
        assert_eq!(b(KeyCode::Char('c'), KeyModifiers::CONTROL), vec![3]);
        assert_eq!(b(KeyCode::Char('d'), KeyModifiers::CONTROL), vec![4]);
        assert_eq!(b(KeyCode::Char('L'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), vec![12]);
        assert_eq!(b(KeyCode::Char('['), KeyModifiers::CONTROL), vec![0x1b]);
        assert_eq!(b(KeyCode::Char('b'), KeyModifiers::ALT), b"\x1bb");
        assert_eq!(b(KeyCode::Enter, KeyModifiers::NONE), b"\r");
        assert_eq!(b(KeyCode::Tab, KeyModifiers::NONE), b"\t");
        assert_eq!(b(KeyCode::BackTab, KeyModifiers::SHIFT), b"\x1b[Z");
        assert_eq!(b(KeyCode::Backspace, KeyModifiers::NONE), vec![0x7f]);
        assert_eq!(b(KeyCode::Backspace, KeyModifiers::ALT), vec![0x1b, 0x7f]);
        assert_eq!(b(KeyCode::Esc, KeyModifiers::NONE), vec![0x1b]);
        assert!(key_bytes(&k(KeyCode::Char('é'), KeyModifiers::CONTROL), false).is_none());
    }

    #[test]
    fn cursor_and_function_keys() {
        assert_eq!(b(KeyCode::Up, KeyModifiers::NONE), b"\x1b[A");
        assert_eq!(key_bytes(&k(KeyCode::Up, KeyModifiers::NONE), true).unwrap(), b"\x1bOA");
        assert_eq!(b(KeyCode::Left, KeyModifiers::ALT), b"\x1b[1;3D");
        assert_eq!(b(KeyCode::Right, KeyModifiers::CONTROL), b"\x1b[1;5C");
        assert_eq!(b(KeyCode::Home, KeyModifiers::NONE), b"\x1b[H");
        assert_eq!(b(KeyCode::End, KeyModifiers::SHIFT), b"\x1b[1;2F");
        assert_eq!(b(KeyCode::Delete, KeyModifiers::NONE), b"\x1b[3~");
        assert_eq!(b(KeyCode::PageUp, KeyModifiers::NONE), b"\x1b[5~");
        assert_eq!(b(KeyCode::PageDown, KeyModifiers::CONTROL), b"\x1b[6;5~");
        assert_eq!(b(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP");
        assert_eq!(b(KeyCode::F(5), KeyModifiers::NONE), b"\x1b[15~");
        assert_eq!(b(KeyCode::F(12), KeyModifiers::NONE), b"\x1b[24~");
    }

    #[test]
    fn cmd_keys_never_reach_the_shell_and_copy_is_cmd_c_or_ctrl_shift_c() {
        assert!(key_bytes(&k(KeyCode::Char('c'), KeyModifiers::SUPER), false).is_none());
        assert!(key_bytes(&k(KeyCode::Char('k'), KeyModifiers::SUPER | KeyModifiers::SHIFT), false).is_none());
        assert!(is_copy(&k(KeyCode::Char('c'), KeyModifiers::SUPER)));
        assert!(is_copy(&k(KeyCode::Char('C'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)));
        assert!(!is_copy(&k(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!is_copy(&k(KeyCode::Char('c'), KeyModifiers::NONE)));
    }

    #[test]
    fn mouse_reports_follow_the_mode_and_the_encoding() {
        use MouseProtocolEncoding as E;
        use MouseProtocolMode as M;
        let down = MouseEventKind::Down(MouseButton::Left);
        let up = MouseEventKind::Up(MouseButton::Left);
        let drag = MouseEventKind::Drag(MouseButton::Left);
        let none = KeyModifiers::NONE;
        let mb = |kind, mods, mode, enc| mouse_bytes(kind, mods, 4, 1, mode, enc);
        assert_eq!(mb(down, none, M::None, E::Sgr), None);
        assert_eq!(mb(down, none, M::PressRelease, E::Sgr).unwrap(), b"\x1b[<0;5;2M");
        assert_eq!(mb(up, none, M::PressRelease, E::Sgr).unwrap(), b"\x1b[<0;5;2m");
        assert_eq!(mb(up, none, M::Press, E::Sgr), None);
        assert_eq!(mb(drag, none, M::PressRelease, E::Sgr), None);
        assert_eq!(mb(drag, none, M::ButtonMotion, E::Sgr).unwrap(), b"\x1b[<32;5;2M");
        assert_eq!(mb(MouseEventKind::Moved, none, M::ButtonMotion, E::Sgr), None);
        assert_eq!(mb(MouseEventKind::Moved, none, M::AnyMotion, E::Sgr).unwrap(), b"\x1b[<35;5;2M");
        assert_eq!(mb(MouseEventKind::ScrollUp, none, M::Press, E::Sgr).unwrap(), b"\x1b[<64;5;2M");
        assert_eq!(mb(down, KeyModifiers::CONTROL, M::Press, E::Sgr).unwrap(), b"\x1b[<16;5;2M");
        // X10 bytes: code, x, y + 32; a release is button 3
        assert_eq!(mb(down, none, M::PressRelease, E::Default).unwrap(), vec![0x1b, b'[', b'M', 32, 37, 34]);
        assert_eq!(mb(up, none, M::PressRelease, E::Default).unwrap(), vec![0x1b, b'[', b'M', 35, 37, 34]);
        let far = mouse_bytes(down, none, 300, 0, M::Press, E::Utf8).unwrap();
        assert_eq!(&far[3..], "\u{20}\u{14d}\u{21}".as_bytes());
    }

    #[test]
    fn a_selection_spans_rows_in_text_order() {
        let s = Sel { anchor: (7, 3), head: (5, 2) };
        assert_eq!(s.range(), ((5, 2), (7, 3)));
        assert_eq!(s.cols(4, 80), None);
        assert_eq!(s.cols(5, 80), Some((2, 80)));
        assert_eq!(s.cols(6, 80), Some((0, 80)));
        assert_eq!(s.cols(7, 80), Some((0, 4)));
        assert_eq!(s.cols(8, 80), None);
        assert_eq!(Sel { anchor: (0, 99), head: (0, 99) }.cols(0, 10), Some((10, 10)));
    }

    #[test]
    fn the_copied_text_joins_soft_wraps_and_reaches_the_history() {
        // 4 rows of 10 columns: a wrapped line, then enough to push rows
        // into the history
        let mut p = vt100::Parser::new(4, 10, 100);
        p.process(b"0123456789abcde\r\nsecond  \r\nthird\r\nfourth\r\nfifth\r\nsixth");
        let hist = history_len(&mut p);
        assert_eq!(hist, 3, "{:?}", p.screen().contents());
        // rows: 0 "0123456789" (wrapped) 1 "abcde" 2 "second" 3 "third" ...
        let all = Sel { anchor: (0, 2), head: (3, 2) };
        assert_eq!(sel_text(&mut p, &all), "23456789abcde\nsecond\nthi");
        assert_eq!(p.screen().scrollback(), 0, "the view is put back");
        // trailing blanks go, inside a row too
        assert_eq!(sel_text(&mut p, &Sel { anchor: (2, 0), head: (2, 9) }), "second");
        // the last rows, on the screen
        assert_eq!(sel_text(&mut p, &Sel { anchor: (5, 1), head: (hist + 3, 9) }), "ifth\nsixth");
    }

    #[test]
    fn hidden_panel_leaves_keys_to_the_app() {
        let mut t = Term::default();
        assert!(!t.key(&k(KeyCode::Char('a'), KeyModifiers::NONE)));
        assert!(!t.paste("x"));
    }

    #[test]
    fn split_keeps_the_app_usable() {
        let full = Rect::new(0, 0, 100, 40);
        let (top, panel) = split(full, 30);
        assert_eq!(panel.height, 12);
        assert_eq!(top.height + panel.height, 40);
        assert_eq!(panel.y, 28);
        // a tall panel leaves the app 8 rows; a tiny one keeps 5 rows of shell
        assert_eq!(split(full, 90).1.height, 32);
        assert_eq!(split(full, 1).1.height, 7);
    }

    #[test]
    fn an_editor_takes_the_panel_and_gives_the_shell_back_when_it_exits() {
        // BISE-264: a short program stands for the editor (never a real one)
        let mut t = Term::default();
        t.pty = Some(Pty::spawn(&["/bin/sh"], "/tmp", 10, 80).unwrap());
        assert!(!t.shown());
        let argv = vec!["/bin/sh".to_string(), "-c".to_string(), "sleep 0.3".to_string()];
        t.run("/tmp", &argv).unwrap();
        assert!(t.shown(), "the panel shows the editor");
        assert!(t.parked.as_ref().is_some_and(|(shell, was, name)| shell.is_some() && !was && name == "sh"));
        // one editor at a time
        let e = t.run("/tmp", &argv).unwrap_err();
        assert!(e.contains("already open"), "{e}");
        let t0 = std::time::Instant::now();
        while t.parked.is_some() && t0.elapsed().as_secs() < 30 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            t.restore();
        }
        assert!(t.parked.is_none(), "the editor exited: the shell is back");
        assert!(!t.shown(), "the panel as it was: hidden");
        assert!(t.pty.as_mut().is_some_and(|p| p.alive()), "the same shell, still running");
        t.shutdown();
    }

    #[test]
    fn shell_runs_and_keeps_state_while_hidden() {
        // a clean /bin/sh, not the user's $SHELL: rc files (zsh, prompts)
        // can take seconds under load and zle may drop or bracket input
        let mut t = Term::default();
        t.pty = Some(Pty::spawn(&["/bin/sh"], "/tmp", 10, 80).unwrap());
        t.toggle("/tmp");
        assert!(t.shown());
        let screen = |t: &Term| t.pty.as_ref().unwrap().parser.lock().unwrap().screen().contents();
        // wait with a deadline (generous: the whole suite runs in parallel)
        let wait_for = |t: &Term, what: &str| {
            let t0 = std::time::Instant::now();
            while !screen(t).contains(what) && t0.elapsed().as_secs() < 30 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let s = screen(t);
            assert!(s.contains(what), "no {:?} on the screen:\n{}", what, s);
        };
        t.paste("X=kept; echo hi-$((40+2))\n");
        wait_for(&t, "\nhi-42");
        t.toggle("/tmp");
        assert!(!t.shown());
        t.toggle("/tmp");
        assert!(t.shown());
        t.paste("echo $X\n");
        wait_for(&t, "\nkept");
        let pid = t.pty.as_ref().unwrap().child.process_id().unwrap();
        t.shutdown();
        let gone = !std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(gone, "the shell survived the shutdown");
    }
}
