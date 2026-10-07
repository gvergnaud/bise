//! ⌥0-9 where Option types characters: iTerm2's default Option key
//! ("Normal"), Terminal.app without "Use Option as Meta key", Ghostty
//! with `macos-option-as-alt = false`. There ⌥1 reaches bise as the
//! character the layout gives it, `¡` on a U.S. layout, with no alt:
//! without the kitty keyboard protocol it is all the terminal sends, and
//! with it (iTerm2 3.5+ reports `CSI 49;3;161u`) our crossterm patch
//! keeps the typed text and drops alt (rust/vendor/crossterm, parse.rs).
//!
//! On the U.S. and ABC layouts the Option layer of the digit row is
//! `¡™£¢∞§¶•ªº` (editor.rs's [`crate::editor::option_layer`]), symbols
//! nobody types into a prompt: those characters come back as ⌥1-9, ⌥0
//! ([`translate`]), whatever the terminal. Other layouts keep their
//! Option characters (German `[`, `{`, `“` on ⌥5, ⌥8, ⌥2, French `«` on
//! ⌥7): there ⌥0-9 needs the terminal's Option key as Esc+ (iTerm2:
//! Profiles › Keys › Left Option key; the setup card says it, tune.rs).
//!
//! The layout is read once at start, off the UI thread ([`detect`]:
//! macOS's `AppleCurrentKeyboardLayoutInputSourceID`); a switch of
//! layout mid-session is not seen. `BISE_OPTION_DIGITS=1|0` forces the
//! mapping on or off (the tmux tests, a layout guessed wrong).

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::OnceLock;

/// The keyboard layouts whose Option digits bise reads as ⌥0-9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    /// U.S. or ABC: `¡™£¢∞§¶•ªº`
    Us,
    /// any other layout, or not known (not macOS, not detected yet)
    Other,
}

/// The layout of a macOS input source id (`com.apple.keylayout.US`).
pub(crate) fn layout_of(id: &str) -> Layout {
    match id.trim().strip_prefix("com.apple.keylayout.") {
        Some("US" | "ABC") => Layout::Us,
        _ => Layout::Other,
    }
}

/// The digit whose Option character `c` is on `layout`.
pub(crate) fn digit_of(c: char, layout: Layout) -> Option<char> {
    if layout != Layout::Us || c.is_ascii() {
        return None;
    }
    ('0'..='9').find(|&d| crate::editor::option_layer(d, false) == Some(crate::editor::OptionKey::Char(c)))
}

/// `ev`, an Option digit typed as its character turned back into ⌥digit.
pub(crate) fn translate(ev: Event, layout: Layout) -> Event {
    match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press && k.modifiers == KeyModifiers::NONE => match k.code {
            KeyCode::Char(c) => match digit_of(c, layout) {
                Some(d) => Event::Key(KeyEvent { code: KeyCode::Char(d), modifiers: KeyModifiers::ALT, ..k }),
                None => ev,
            },
            _ => ev,
        },
        _ => ev,
    }
}

/// An input event as the handlers take it (run.rs): [`translate`] on the
/// layout found at start.
pub(crate) fn read_back(ev: Event) -> Event {
    translate(ev, layout())
}

static LAYOUT: OnceLock<Layout> = OnceLock::new();

/// The layout found at start ([`Layout::Other`] until [`detect`] is done).
pub(crate) fn layout() -> Layout {
    LAYOUT.get().copied().unwrap_or(Layout::Other)
}

/// `BISE_OPTION_DIGITS`: `1` reads the U.S. Option digits, `0` none.
fn forced(v: Option<&str>) -> Option<Layout> {
    match v.map(str::trim) {
        Some("1") => Some(Layout::Us),
        Some("0") => Some(Layout::Other),
        _ => None,
    }
}

/// Finds the layout once, on its own thread (`defaults` takes ~30 ms).
pub(crate) fn detect() {
    if let Some(l) = forced(std::env::var("BISE_OPTION_DIGITS").ok().as_deref()) {
        let _ = LAYOUT.set(l);
        return;
    }
    if !cfg!(target_os = "macos") {
        let _ = LAYOUT.set(Layout::Other);
        return;
    }
    std::thread::spawn(|| {
        let out = std::process::Command::new("defaults")
            .args(["read", "com.apple.HIToolbox", "AppleCurrentKeyboardLayoutInputSourceID"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let l = out.ok().filter(|o| o.status.success()).map_or(Layout::Other, |o| layout_of(&String::from_utf8_lossy(&o.stdout)));
        let _ = LAYOUT.set(l);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char, m: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), m))
    }

    #[test]
    fn the_us_option_digits_are_alt_digits() {
        let (none, alt) = (KeyModifiers::NONE, KeyModifiers::ALT);
        for (c, d) in "¡™£¢∞§¶•ªº".chars().zip("1234567890".chars()) {
            assert_eq!(translate(press(c, none), Layout::Us), press(d, alt), "{c}");
        }
        // what the kitty protocol path gives after our crossterm patch
        // (iTerm2 3.5+, `CSI 49;3;161u`): the same `¡` without alt
        assert_eq!(translate(press('¡', none), Layout::Us), press('1', alt));
    }

    #[test]
    fn everything_else_types() {
        let none = KeyModifiers::NONE;
        // another layout keeps its Option characters (German ⌥5 `[`, French ⌥7 `«`)
        for c in ['¡', '[', '«', '“'] {
            assert_eq!(translate(press(c, none), Layout::Other), press(c, none));
        }
        // other Option characters, letters, accents, shifted option digits
        for c in ['å', 'ç', 'é', '⁄', '€', '1', 'a'] {
            assert_eq!(translate(press(c, none), Layout::Us), press(c, none), "{c}");
        }
        // a real ⌥1 and a ctrl'd one are left alone, so is a paste
        assert_eq!(translate(press('1', KeyModifiers::ALT), Layout::Us), press('1', KeyModifiers::ALT));
        assert_eq!(translate(press('¡', KeyModifiers::CONTROL), Layout::Us), press('¡', KeyModifiers::CONTROL));
        assert_eq!(translate(Event::Paste("¡".into()), Layout::Us), Event::Paste("¡".into()));
    }

    #[test]
    fn layouts_and_the_override() {
        assert_eq!(layout_of("com.apple.keylayout.US\n"), Layout::Us);
        assert_eq!(layout_of("com.apple.keylayout.ABC"), Layout::Us);
        for id in ["com.apple.keylayout.French", "com.apple.keylayout.German", "com.apple.keylayout.British", ""] {
            assert_eq!(layout_of(id), Layout::Other, "{id}");
        }
        assert_eq!(forced(Some("1")), Some(Layout::Us));
        assert_eq!(forced(Some("0")), Some(Layout::Other));
        assert_eq!(forced(None), None);
    }
}
