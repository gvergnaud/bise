//! Gabriel's note about the usage counts in the terminal (page
//! usage-metrics-plan v13, step 6b; designer m_56751): one dim line,
//! wrapped. Under the three lines of the onboarding's "how it works"
//! ([`line`], drawn by onboarding.rs, whose key bar holds `d disable
//! telemetry`), and, for an install that never saw it (esc skipped the
//! onboarding, or it predates the note), once as a block above the feed
//! at the next start ([`start`], [`above`]) until his first message or
//! esc, with the command that turns it off (d types there).
//!
//! The note is marked seen (`bise_usage::control::note_seen`) after it was
//! drawn, never before: without that mark bise counts nothing. The words
//! are bise_proto's (words/settings/setup.rs), the same as the desktop's.

use std::sync::atomic::{AtomicU8, Ordering};

use bise_proto::words::settings;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// The command that turns it off, drawn bold above the feed.
pub(crate) const OFF_CMD: &str = "bise telemetry off";

/// The line's rows at `width`, dim; `off` (d disabled it): `· telemetry
/// off` after it.
pub(crate) fn line(width: usize, off: bool) -> Vec<Line<'static>> {
    let dim = Style::default().fg(theme::dim());
    let mut spans = vec![Span::styled(settings::onb_telemetry_line, dim)];
    if off {
        spans.push(Span::styled(format!(" · {}", settings::onb_telemetry_off_now), dim));
    }
    crate::wrap_line(Line::from(spans), width)
}

/// The block above the feed at `width`: the line, then the way out with
/// its command in bold (never the accent).
pub(crate) fn rows(width: usize) -> Vec<Line<'static>> {
    let dim = Style::default().fg(theme::dim());
    let mut v = line(width, false);
    let (before, after) = settings::onb_telemetry_off_cmd("\u{0}").split_once('\u{0}').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    let spans = [Span::styled(before, dim), Span::styled(OFF_CMD, Style::default().fg(theme::text()).add_modifier(Modifier::BOLD)), Span::styled(after, dim)];
    v.extend(crate::wrap_line(Line::from(spans.into_iter().filter(|s| !s.content.is_empty()).collect::<Vec<_>>()), width));
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
    fn the_note_is_one_dim_line_and_above_the_feed_says_the_command_in_bold() {
        let r = line(76, false);
        let t = text_of(&r);
        assert!(t.lines().all(|l| l.trim_end().width() <= 76), "{t}");
        assert!(t.starts_with("Only aggregated usage counts") && t.trim_end().ends_with("anything personal."), "{t}");
        assert!(r.iter().flat_map(|l| l.spans.iter()).all(|s| s.style.fg == Some(theme::dim())));
        assert!(text_of(&line(76, true)).trim_end().ends_with("anything personal. · telemetry off"));
        let above = rows(76);
        assert_eq!(above.len(), r.len() + 1, "the line, then the way out");
        let bold: Vec<&str> = above.iter().flat_map(|l| l.spans.iter()).filter(|s| s.style.add_modifier.contains(Modifier::BOLD)).map(|s| s.content.as_ref()).collect();
        assert_eq!(bold, [OFF_CMD]);
        assert!(text_of(&above).ends_with(&format!("turn it off: {OFF_CMD}")), "{}", text_of(&above));
    }
}
