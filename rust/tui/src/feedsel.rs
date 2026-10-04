//! The in-app selection in the feed: positions over the wrapped rows,
//! the highlight, and the text a selection copies (soft-wrapped rows
//! joined, the code-box borders and line numbers left out, trailing
//! blanks trimmed).
//!
//! A continuation row of a wrapped line carries `Alignment::Left` (an
//! explicit left alignment renders as the default one): that is how
//! the copy knows the row break was not in the text.

use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A feed position: event index, row among the event's wrapped rows,
/// display column.
pub(crate) type FeedPos = (usize, usize, usize);

/// A selection being made or made: where the press was and where the
/// pointer is; both cells are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FeedSel {
    pub(crate) anchor: FeedPos,
    pub(crate) head: FeedPos,
}

impl FeedSel {
    /// (first, last) positions, in feed order, both included.
    pub(crate) fn range(&self) -> (FeedPos, FeedPos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// The columns [from, to) selected on the row (ev, row), if any.
    pub(crate) fn cols(&self, ev: usize, row: usize) -> Option<(usize, usize)> {
        let (a, b) = self.range();
        let at = (ev, row);
        if at < (a.0, a.1) || at > (b.0, b.1) {
            return None;
        }
        let from = if at == (a.0, a.1) { a.2 } else { 0 };
        let to = if at == (b.0, b.1) { b.2 + 1 } else { usize::MAX };
        Some((from, to))
    }
}

/// Marks a row as the continuation of the row before (a soft wrap).
pub(crate) fn mark_soft(line: &mut Line<'static>) {
    line.alignment = Some(Alignment::Left);
}

pub(crate) fn is_soft(line: &Line) -> bool {
    line.alignment == Some(Alignment::Left)
}

pub(crate) fn line_text(line: &Line) -> String {
    // an artifact's chip keeps its words on one row with non-breaking
    // spaces (render::artifact_chip): copied, they are spaces
    line.spans.iter().map(|s| s.content.as_ref()).collect::<String>().replace('\u{a0}', " ")
}

/// The text of the columns [from, to) of `s` (a wide grapheme counts
/// when its first column is inside).
pub(crate) fn slice_cols(s: &str, from: usize, to: usize) -> String {
    let mut col = 0usize;
    let mut out = String::new();
    for g in s.graphemes(true) {
        if col >= to {
            break;
        }
        if col >= from {
            out.push_str(g);
        }
        col += g.width();
    }
    out
}

/// The columns of a row that are text, not decoration: a row behind a
/// rail (" │ code", a code block, an agent message, a thinking section)
/// keeps only its text, and a code continuation row (" │ » rest") drops
/// its wrap mark too; your own line (" › text") drops its mark.
fn content_cols(s: &str, line: &Line) -> (usize, usize) {
    let w = s.width();
    let lead = s.len() - s.trim_start_matches(' ').len();
    let rest = &s[lead..];
    // your own line: `› text`
    if rest.strip_prefix(crate::theme::G_YOU).is_some_and(|t| t.starts_with(' ')) {
        let c0 = lead + crate::theme::G_YOU.width() + 1;
        return (c0.min(w), w);
    }
    let Some(inner) = rest.strip_prefix("│ ") else {
        return (0, w);
    };
    let mut c0 = lead + 2;
    if inner.starts_with(crate::theme::G_WRAP) && inner[crate::theme::G_WRAP.len()..].starts_with(' ') {
        c0 += crate::theme::G_WRAP.width() + 1;
    }
    // a box's row (`│ code   │`, toolbox.rs): its padding and its
    // right border too
    let c1 = if inner.ends_with(" │") { w - 2 - box_pad(line) } else { w };
    (c0.min(c1), c1)
}

/// The padding of a box's row: the blank span before its right border
/// (toolbox.rs inner_row), so a wrapped line's own spaces stay.
fn box_pad(line: &Line) -> usize {
    match line.spans.as_slice() {
        [.., pad, last] if last.content.ends_with('│') && pad.content.chars().all(|c| c == ' ') => pad.content.width(),
        _ => 0,
    }
}

/// The text of the selected rows: `rows` are the rows from the first to
/// the last selected one; the first starts at column `from`, the last
/// ends before column `to`.
pub(crate) fn selection_text(rows: &[Line], from: usize, to: usize) -> String {
    let n = rows.len();
    let mut out = String::new();
    for (i, l) in rows.iter().enumerate() {
        let s = line_text(l);
        let (c0, c1) = content_cols(&s, l);
        let a = if i == 0 { from.max(c0) } else { c0 };
        let b = if i + 1 == n { to.min(c1) } else { c1 };
        let part = if a < b { slice_cols(&s, a, b) } else { String::new() };
        if i > 0 && !out.is_empty() && !is_soft(l) {
            let t = out.trim_end_matches(' ').len();
            out.truncate(t);
            out.push('\n');
        }
        out.push_str(&part);
    }
    let t = out.trim_end_matches(' ').len();
    out.truncate(t);
    out
}

/// The row with the columns [from, to) on the selection background.
pub(crate) fn highlight(line: &Line<'static>, from: usize, to: usize, bg: Color) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut col = 0usize;
    let push = |spans: &mut Vec<Span<'static>>, text: &str, st: Style| match spans.last_mut() {
        Some(l) if l.style == st => l.content.to_mut().push_str(text),
        _ => spans.push(Span::styled(text.to_string(), st)),
    };
    for sp in &line.spans {
        for g in sp.content.graphemes(true) {
            let st = if col >= from && col < to { sp.style.bg(bg) } else { sp.style };
            push(&mut spans, g, st);
            col += g.width();
        }
    }
    let mut out = Line::from(spans);
    out.style = line.style;
    out.alignment = line.alignment;
    out
}

/// The word around column `col` of the row (a double click), as columns.
pub(crate) fn word_cols(s: &str, col: usize) -> (usize, usize) {
    let mut cells: Vec<(usize, usize, bool)> = Vec::new(); // (col, width, word)
    let mut c = 0usize;
    for g in s.graphemes(true) {
        let w = g.width();
        let word = g.chars().next().is_some_and(|ch| ch.is_alphanumeric() || "_-./:@~".contains(ch));
        cells.push((c, w, word));
        c += w;
    }
    let Some(i) = cells.iter().rposition(|&(c, _, _)| c <= col) else {
        return (col, col);
    };
    if !cells[i].2 {
        return (cells[i].0, cells[i].0 + cells[i].1.max(1) - 1);
    }
    let (mut a, mut b) = (i, i);
    while a > 0 && cells[a - 1].2 {
        a -= 1;
    }
    while b + 1 < cells.len() && cells[b + 1].2 {
        b += 1;
    }
    (cells[a].0, cells[b].0 + cells[b].1.max(1) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soft(s: &str) -> Line<'static> {
        let mut l = Line::from(s.to_string());
        mark_soft(&mut l);
        l
    }

    #[test]
    fn soft_wraps_join_and_hard_breaks_stay() {
        let rows = vec![Line::from("  hello wide "), soft("  world, it "), soft("  wraps"), Line::from("  next line   ")];
        // the feed's 2-column indent is text too: the selection starts
        // where the press was
        assert_eq!(selection_text(&rows, 2, usize::MAX), "hello wide   world, it   wraps\n  next line");
        let rows = vec![Line::from("hello wide "), soft("world")];
        assert_eq!(selection_text(&rows, 6, 3), "wide wor");
    }

    #[test]
    fn code_rail_rows_copy_the_code_only() {
        // a wrapped code line: its continuation row carries the wrap
        // mark and joins the row before
        let rows = vec![
            Line::from(" │ ls -la "),
            soft(" │ » --color"),
            Line::from(" │ pwd"),
        ];
        assert_eq!(selection_text(&rows, 0, usize::MAX), "ls -la --color\npwd");
        let rows = vec![Line::from(" │ plain output")];
        assert_eq!(selection_text(&rows, 0, usize::MAX), "plain output");
    }

    #[test]
    fn columns_count_wide_graphemes() {
        assert_eq!(slice_cols("a👍🏽b❤️c", 1, 4), "👍🏽b");
        assert_eq!(slice_cols("a👍🏽b", 2, 4), "b");
        let l = highlight(&Line::from("ab👏cd"), 1, 4, Color::Blue);
        let bg: Vec<(String, bool)> = l.spans.iter().map(|s| (s.content.to_string(), s.style.bg.is_some())).collect();
        assert_eq!(bg, vec![("a".into(), false), ("b👏".into(), true), ("cd".into(), false)]);
    }

    #[test]
    fn selection_ranges_are_ordered_and_inclusive() {
        let s = FeedSel { anchor: (3, 1, 5), head: (2, 0, 4) };
        assert_eq!(s.cols(2, 0), Some((4, usize::MAX)));
        assert_eq!(s.cols(2, 7), Some((0, usize::MAX)));
        assert_eq!(s.cols(3, 1), Some((0, 6)));
        assert_eq!(s.cols(3, 2), None);
        assert_eq!(s.cols(1, 9), None);
    }

    #[test]
    fn double_click_words_keep_paths() {
        let s = "  see rust/tui/src/lib.rs:42 now";
        assert_eq!(word_cols(s, 10), (6, 27));
        assert_eq!(word_cols(s, 2), (2, 4));
        assert_eq!(word_cols(s, 5), (5, 5));
    }
}
