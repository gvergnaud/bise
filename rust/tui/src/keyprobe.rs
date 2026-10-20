//! `bise keyprobe`: prints the key and mouse events this
//! terminal delivers with the flags the TUI uses, and the composer
//! action each key maps to. Checks which macOS shortcuts reach the app
//! (Ghostty keeps some for itself unless unbound).

use crossterm::event::{
    read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};

pub fn keyprobe() -> io::Result<()> {
    let mut out = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(
        out,
        EnableMouseCapture,
        EnableBracketedPaste,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let say = |out: &mut io::Stdout, s: &str| {
        let _ = write!(out, "{}\r\n", s);
        let _ = out.flush();
    };
    say(&mut out, "keyprobe: press the shortcuts to check (Option/Cmd + arrows, Backspace, Ctrl+/ …). Ctrl+C twice quits.");
    let mut ctrl_c = 0;
    loop {
        match read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                let action = crate::editor::action(&k);
                say(&mut out, &format!("key   {:?} + {:?}  →  {}", k.code, k.modifiers,
                    action.map_or("(none: the app's own key or unmapped)".to_string(), |a| format!("{:?}", a))));
                if k.code == KeyCode::Char('c') && k.modifiers == KeyModifiers::CONTROL {
                    ctrl_c += 1;
                    if ctrl_c == 2 {
                        break;
                    }
                } else {
                    ctrl_c = 0;
                }
            }
            Event::Mouse(m) if !matches!(m.kind, crossterm::event::MouseEventKind::Moved) => {
                say(&mut out, &format!("mouse {:?} at {},{} {:?}", m.kind, m.column, m.row, m.modifiers));
            }
            Event::Paste(p) => say(&mut out, &format!("paste {:?}", p)),
            _ => {}
        }
    }
    crossterm::execute!(out, PopKeyboardEnhancementFlags, DisableBracketedPaste, DisableMouseCapture)?;
    disable_raw_mode()
}
