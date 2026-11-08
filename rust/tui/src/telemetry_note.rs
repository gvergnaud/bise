//! Gabriel's note about the usage counts in the terminal (page
//! usage-metrics-plan, step 6; designer m_56350): under the three lines of
//! the onboarding's "how it works" ([`rows`], drawn by onboarding.rs),
//! and, for an install that never saw it (esc skipped the onboarding, or
//! it predates the note), once as a block above the feed at the next
//! start ([`start`], [`draw`]) until his first message or esc.
//!
//! The note is marked seen (`bise_usage::control::note_seen`) after it was
//! drawn, never before: without that mark bise counts nothing. The words
//! are bise_proto's (words/settings/setup.rs), the same as the desktop's.

use std::sync::atomic::{AtomicU8, Ordering};

use bise_proto::words::settings;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// The command and the variable that turn it off, drawn as code.
pub(crate) const OFF_CMD: &str = "bise telemetry off";
pub(crate) const OFF_ENV: &str = "BISE_TELEMETRY=0";

/// The note's rows at `width`: the dim title, the text, the way out with
/// its command and variable in bold (never the accent).
pub(crate) fn rows(width: usize) -> Vec<Line<'static>> {
    let text = Style::default().fg(theme::text());
    let code = text.add_modifier(Modifier::BOLD);
    let mut v = vec![Line::from(Span::styled(settings::onb_telemetry_title, Style::default().fg(theme::dim())))];
    let para = format!("{} {} {}", settings::onb_telemetry_why_line, settings::onb_telemetry_what, settings::onb_telemetry_never);
    v.extend(crate::wrap_line(Line::from(Span::styled(para, text)), width));
    // the way out: the words around the two holes, the holes in bold
    let words = settings::onb_telemetry_off_tui("\u{0}", "\u{1}");
    let spans: Vec<Span<'static>> = words
        .split_inclusive(['\u{0}', '\u{1}'])
        .flat_map(|part| {
            let (t, hole) = match part.chars().last() {
                Some('\u{0}') => (&part[..part.len() - 1], Some(OFF_CMD)),
                Some('\u{1}') => (&part[..part.len() - 1], Some(OFF_ENV)),
                _ => (part, None),
            };
            let mut out = vec![Span::styled(t.to_string(), text)];
            out.extend(hole.map(|h| Span::styled(h.to_string(), code)));
            out
        })
        .filter(|s| !s.content.is_empty())
        .collect();
    v.extend(crate::wrap_line(Line::from(spans), width));
    v
}

const HIDDEN: u8 = 0;
const SHOWN: u8 = 1;
/// Drawn once: marked seen.
const MARKED: u8 = 2;

/// The block above the feed: hidden, shown, or shown and marked seen.
static ABOVE: AtomicU8 = AtomicU8::new(HIDDEN);

/// At the TUI's start: the block shows when he went through the
/// onboarding (or skipped it) but never saw the note.
pub(crate) fn start(home: &bise_home::Home) {
    let prefs = bise_home::Slot::file(home.prefs_file()).get();
    let onboarded = prefs.as_ref().and_then(|p| p.get("onboarded")).and_then(serde_json::Value::as_bool).unwrap_or(false);
    if onboarded && !bise_home::prefs::telemetry_note_seen(prefs.as_ref()) {
        ABOVE.store(SHOWN, Ordering::SeqCst);
    }
}

/// His first message, or esc: the block goes.
pub(crate) fn dismiss() {
    ABOVE.store(HIDDEN, Ordering::SeqCst);
}

/// The block's rows at `width` while it shows (with a blank row after).
pub(crate) fn above(width: usize) -> Option<Vec<Line<'static>>> {
    (ABOVE.load(Ordering::SeqCst) != HIDDEN).then(|| rows(width))
}

/// After a frame that drew the block: marked seen, once.
pub(crate) fn drawn(home: &bise_home::Home) {
    if ABOVE.compare_exchange(SHOWN, MARKED, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        let _ = bise_usage::control::note_seen(home);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn text_of(rows: &[Line]) -> String {
        rows.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn the_note_fits_80_columns_and_ends_with_the_way_out_in_bold() {
        let r = rows(76);
        // the wrap keeps the space it broke at; the words fit
        assert!(text_of(&r).lines().all(|l| l.trim_end().width() <= 76), "{}", text_of(&r));
        let t = text_of(&r);
        assert!(t.starts_with("a note from me, Gabriel"), "a name keeps its capital: {t}");
        assert!(t.contains("Never what you write or own"));
        let last: Vec<&Span> = r.iter().flat_map(|l| l.spans.iter()).collect();
        let bold: Vec<&str> = last.iter().filter(|s| s.style.add_modifier.contains(Modifier::BOLD)).map(|s| s.content.as_ref()).collect();
        assert_eq!(bold, [OFF_CMD, OFF_ENV]);
        assert!(t.ends_with(&format!("Turn it off with {OFF_CMD}, or {OFF_ENV}.")), "{t}");
    }
}
