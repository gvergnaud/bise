//! An agent's attachments on its rows (send-parts, designer m_24147): the
//! image and file markers `sb send --attach` puts after its words, drawn
//! as his message's chips (`▣ shot.png`, `▯ deck.pptx`) in their quiet
//! form on a level-3 row and in an opened brief. Pure.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// How an image marker starts (bise_proto::parts' image marker, as
/// `parts::render` writes it); a file's mention is its link
/// (`parts::find_file_marker`).
const IMAGE_OPEN: &str = "<image name=\"";

/// `s` holds an image marker or a file's mention.
pub(crate) fn has(s: &str) -> bool {
    s.contains(IMAGE_OPEN) || bise_proto::parts::find_file_marker(s).is_some()
}

/// The next attachment in `s` from byte `from`: its start and end (an
/// image marker to its `">`, a file's mention whole).
fn next_attachment(s: &str, from: usize) -> Option<(usize, usize)> {
    let rest = s.get(from..)?;
    let image = rest.find(IMAGE_OPEN).map(|i| (from + i, rest[i..].find("\">").map_or(s.len(), |e| from + i + e + 2)));
    let file = bise_proto::parts::find_file_marker(rest).map(|(i, _, used)| (from + i, from + i + used));
    [image, file].into_iter().flatten().min()
}

/// A brief's words and its attachments after them (send-parts: the
/// markers come last, with only spaces between them); none: (`body`,
/// ""). A file's link inside the words stays with the words.
pub(crate) fn split(body: &str) -> (&str, &str) {
    let mut tail: Option<usize> = None;
    let mut last = 0usize;
    while let Some((a, b)) = next_attachment(body, last) {
        if !body[last..a].trim().is_empty() {
            tail = None;
        }
        tail = tail.or(Some(a));
        last = b;
    }
    match tail.filter(|_| body[last..].trim().is_empty()) {
        Some(i) => body.split_at(i),
        None => (body, ""),
    }
}

/// A level-3 line in `st`; its attachments (send-parts: the image and
/// file markers `sb send --attach` put after the words) as his message's
/// chips (`▣ shot.png`, `▯ deck.pptx`) in their quiet form: the same
/// glyphs and names, all in `st`, never the accent (designer m_24147: a
/// message between agents is not for him). A line without one: one span,
/// as before.
pub(crate) fn spans(line: &str, st: Style) -> Vec<Span<'static>> {
    match has(line) {
        true => spaced(crate::attach::chip_spans(line, st).into_iter().map(|s| Span::styled(s.content, st)).collect(), st),
        false => vec![Span::styled(line.to_string(), st)],
    }
}

/// A chip's first span: its image, file or paste glyph (a forward's
/// original draws its pastes' chips, designer m_41661).
fn is_chip(s: &Span) -> bool {
    [crate::theme::G_IMAGE, crate::theme::glyph(crate::theme::G_FILE), crate::theme::G_FILE, crate::theme::G_PASTE].iter().any(|g| s.content.starts_with(g))
}

/// One space before a chip glued to what comes before it: `sb send
/// --attach` puts its markers back to back (his composer spaces his).
fn spaced(spans: Vec<Span<'static>>, st: Style) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::with_capacity(spans.len() * 2);
    for s in spans {
        let glued = out.last().is_some_and(|p| !p.content.ends_with(char::is_whitespace));
        if glued && is_chip(&s) {
            out.push(Span::styled(" ", st));
        }
        out.push(s);
    }
    out
}

/// A brief's rows: its words as `rows_of` draws them, then its
/// attachments as one row of quiet chips in `st` (none: its rows alone).
pub(crate) fn with_chips(body: &str, st: Style, rows_of: impl FnOnce(&str) -> Vec<Line<'static>>) -> Vec<Line<'static>> {
    let (words, chips) = split(body);
    let mut rows = rows_of(words.trim_end());
    if !chips.is_empty() {
        rows.push(Line::from(spans(chips, st)));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_sent_back_to_back_read_one_space_apart() {
        let line = "<image name=\"[Image #1]\" path=\"/x/flame.png\" mime=\"image/png\" b64=\"\">[cold.csv](/x/cold.csv)";
        let flat: String = spans(line, Style::default()).iter().map(|s| s.content.as_ref()).collect();
        assert!(flat.contains("flame.png ") && !flat.contains("flame.png▯"), "{flat}");
        assert!(!flat.starts_with(' '), "{flat}");
    }

    /// file-mention-wire: the attachments are the links and markers at
    /// the end; a file's link inside the words stays with them.
    #[test]
    fn a_briefs_chips_are_its_last_markers_only() {
        let body = "update [The Bot.md](/u/The Bot.md) pour moi\n\n[cold.csv](/x/cold.csv) <file name=\"[File #2]\" path=\"/x/old.csv\">";
        let (words, chips) = split(body);
        assert_eq!(words, "update [The Bot.md](/u/The Bot.md) pour moi\n\n");
        assert_eq!(chips, "[cold.csv](/x/cold.csv) <file name=\"[File #2]\" path=\"/x/old.csv\">");
        assert_eq!(split("see [The Bot.md](/u/The Bot.md) now"), ("see [The Bot.md](/u/The Bot.md) now", ""));
        assert!(has("see [The Bot.md](/u/The Bot.md) now") && !has("see [the doc](/u/The Bot.md)"));
    }
}
