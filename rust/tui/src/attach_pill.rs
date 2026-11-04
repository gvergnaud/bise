//! How a chip is drawn in the composer, the strip and the attachments
//! box (out of attach.rs, its line limit): its glyph and its number, or
//! its words for an artifact (its title) and a file (its name, designer
//! m_27881); the box's rows keep a file's number (`named` false).

use super::artifact_title;
use crate::theme::{accent, dim, glyph, text, G_FILE, G_IMAGE, G_PASTE, G_QUOTE};
use ratatui::style::Style;
use ratatui::text::Span;
use std::borrow::Cow;

/// A chip's glyph (`▣`, `❝` or `▤`) and number, from its label
/// `[Image #N]`, `[Quote #N]` or `[Paste #N]`.
pub(super) fn chip_parts(label: &str) -> (&'static str, Cow<'_, str>) {
    chip_parts_as(label, true)
}

/// [`chip_parts`]; `named` false: a file chip by its number (the box's
/// row, designer m_27881: '▯ 3  e2e.mjs').
fn chip_parts_as(label: &str, named: bool) -> (&'static str, Cow<'_, str>) {
    // a plugin he picked with `@` says its name (at_plugins.rs)
    if let Some(name) = crate::at_plugins::chip_name(label) {
        return (crate::theme::plugin_glyph(), Cow::Owned(name));
    }
    if let Some(n) = label.strip_prefix(crate::quote::OPEN) {
        return (glyph(G_QUOTE), Cow::Borrowed(n.trim_end_matches(']')));
    }
    if let Some(n) = label.strip_prefix(crate::pasted::OPEN) {
        return (glyph(G_PASTE), Cow::Borrowed(n.trim_end_matches(']')));
    }
    if let Some(n) = label.strip_prefix(crate::attach_file::OPEN) {
        // a file chip says its name (designer m_27881), its number when unknown
        let words = if named { crate::attach_file::chip_words(label) } else { None };
        return (glyph(G_FILE), words.map_or(Cow::Borrowed(n.trim_end_matches(']')), Cow::Owned));
    }
    (G_IMAGE, Cow::Borrowed(label.trim_start_matches("[Image #").trim_end_matches(']')))
}

/// A chip named in a sentence (the flash): `▣ 1`, `❝ 1`.
pub(crate) fn chip_name(label: &str) -> String {
    if let Some(t) = artifact_title(label) {
        return format!("↗ {t}");
    }
    let (g, n) = chip_parts(label);
    format!("{g} {n}")
}

/// How the composer and the strip draw a chip (BISE-205): a pill
/// ` ❝ 1 ` (one padding cell each side, on the `pill` tint), or, with no
/// tint (`NO_COLOR`, `BISE_ASCII=1`), `[❝ 1]`. Same width both ways.
pub(crate) fn chip_text(label: &str) -> String {
    chip_text_as(label, true)
}

pub(super) fn chip_text_as(label: &str, named: bool) -> String {
    if let Some(t) = artifact_title(label) {
        return match crate::render::chip_form() {
            crate::render::ChipForm::Tinted => format!(" ↗ {t} "),
            crate::render::ChipForm::Bracketed => format!("[↗ {t}]"),
        };
    }
    let (g, n) = chip_parts_as(label, named);
    match crate::render::chip_form() {
        crate::render::ChipForm::Tinted => format!(" {g} {n} "),
        crate::render::ChipForm::Bracketed => format!("[{g} {n}]"),
    }
}

/// The columns the chip `label` takes in `inner` columns of text.
pub(crate) fn chip_width(label: &str, inner: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    if is_voice(label) {
        crate::voice::chip::width(inner)
    } else {
        chip_text(label).width()
    }
}

/// `label` is the voice chip.
pub(crate) fn is_voice(label: &str) -> bool {
    label.starts_with(crate::voice::chip::OPEN)
}

/// The spans of [`chip_text`]: the glyph accent, the number in the text
/// color, all on the pill tint (the brackets dim); `over` is patched on
/// every span (the selection's background, the cursor's REVERSED), so
/// the whole pill takes it.
pub(crate) fn chip_pill(label: &str, over: Style) -> Vec<Span<'static>> {
    chip_pill_as(label, over, true)
}

pub(super) fn chip_pill_as(label: &str, over: Style, named: bool) -> Vec<Span<'static>> {
    let artifact = artifact_title(label);
    let (g, n) = match &artifact {
        Some(t) => ("↗", Cow::Borrowed(t.as_str())),
        None => chip_parts_as(label, named),
    };
    let (open, close, bg) = match crate::render::chip_form() {
        crate::render::ChipForm::Tinted => (" ", " ", Some(crate::theme::pill_bg())),
        crate::render::ChipForm::Bracketed => ("[", "]", None),
    };
    let st = |fg| {
        let s = Style::default().fg(fg);
        bg.map_or(s, |b| s.bg(b)).patch(over)
    };
    vec![
        Span::styled(open.to_string(), st(dim())),
        Span::styled(g.to_string(), st(accent())),
        Span::styled(format!(" {n}"), st(text())),
        Span::styled(close.to_string(), st(dim())),
    ]
}
