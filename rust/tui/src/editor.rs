//! The composer's editing model: the text, a cursor and a selection on
//! grapheme boundaries, undo/redo (the stack and its steps: undo.rs),
//! and the history recall that keeps the
//! draft. The key map (`action`) turns terminal key events into editing
//! actions; the popups, sending and voice live in lib.rs and call in.
//!
//! Positions are char indices into `text` (never inside a grapheme: an
//! emoji with a ZWJ, a skin tone or a variation selector is one step).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::undo::{Kind, Snap, Steps};

// ---- grapheme helpers ----

/// Byte offset of the char at index `ci` (the text end past the last).
pub(crate) fn byte_at_char(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// The char index of the first `needle` in `s`.
fn char_find(s: &str, needle: &str) -> Option<usize> {
    s.find(needle).map(|b| s[..b].chars().count())
}

/// The char index of the grapheme before the one at `cursor`. An image
/// chip `[Image #N]` is one step (attach.rs).
pub(crate) fn prev_grapheme(s: &str, cursor: usize) -> usize {
    if let Some((a, _)) = crate::attach::chips(s).into_iter().find(|&(a, b, _)| a < cursor && cursor <= b).map(|(a, b, _)| (a, b)) {
        return a;
    }
    let mut ci = 0usize;
    let mut prev = 0usize;
    for g in s.graphemes(true) {
        if ci >= cursor {
            break;
        }
        prev = ci;
        ci += g.chars().count();
    }
    prev
}

/// The char index of the grapheme after the one at `cursor`. An image
/// chip is one step.
pub(crate) fn next_grapheme(s: &str, cursor: usize) -> usize {
    if let Some((_, b, _)) = crate::attach::chips(s).into_iter().find(|&(a, b, _)| a <= cursor && cursor < b) {
        return b;
    }
    let mut ci = 0usize;
    for g in s.graphemes(true) {
        ci += g.chars().count();
        if ci > cursor {
            return ci;
        }
    }
    ci
}

/// The graphemes of `s` with their first char index.
fn graphemes_ci(s: &str) -> Vec<(usize, &str)> {
    let mut ci = 0usize;
    s.graphemes(true)
        .map(|g| {
            let at = ci;
            ci += g.chars().count();
            (at, g)
        })
        .collect()
}

fn is_word(g: &str) -> bool {
    g.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// macOS Option+←: skip the non-word graphemes before the cursor, then
/// the word before them.
pub(crate) fn word_left(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i > 0 && !is_word(gs[i - 1].1) {
        i -= 1;
    }
    while i > 0 && is_word(gs[i - 1].1) {
        i -= 1;
    }
    gs.get(i).map(|g| g.0).unwrap_or(0)
}

/// macOS Option+→: skip the non-word graphemes after the cursor, then
/// the word after them (the cursor lands at the word's end).
pub(crate) fn word_right(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let end = s.chars().count();
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i < gs.len() && !is_word(gs[i].1) {
        i += 1;
    }
    while i < gs.len() && is_word(gs[i].1) {
        i += 1;
    }
    gs.get(i).map(|g| g.0).unwrap_or(end)
}

/// The case class of an alphanumeric grapheme, for subword moves:
/// None for the rest (`_`, `-`, spaces, punctuation, emojis), which
/// separate subwords and are skipped like the gaps between words.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Upper,
    /// lowercase, or a letter without case
    Lower,
    Digit,
}

fn sub_class(g: &str) -> Option<Sub> {
    let c = g.chars().next()?;
    if c.is_numeric() {
        Some(Sub::Digit)
    } else if c.is_uppercase() {
        Some(Sub::Upper)
    } else if c.is_alphabetic() {
        Some(Sub::Lower)
    } else {
        None
    }
}

/// True when a subword boundary falls between the graphemes `i - 1` and
/// `i` (0 < i < len): the edges of alphanumeric runs, lower→Upper
/// (camel|Case), letter↔digit (utf|8), and before the last capital of
/// an acronym followed by a lowercase letter (HTTP|Server).
fn sub_boundary(cs: &[Option<Sub>], i: usize) -> bool {
    use Sub::*;
    match (cs[i - 1], cs[i]) {
        (None, _) | (_, None) => true,
        (Some(Lower), Some(Upper)) => true,
        (Some(p), Some(c)) if (p == Digit) != (c == Digit) => true,
        (Some(Upper), Some(Upper)) => cs.get(i + 1) == Some(&Some(Lower)),
        _ => false,
    }
}

/// Zed-style subword ←: to the start of the subword before the cursor
/// (in snake_case, camelCase, PascalCase, kebab-case, digits).
pub(crate) fn subword_left(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let cs: Vec<Option<Sub>> = gs.iter().map(|g| sub_class(g.1)).collect();
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i > 0 {
        i -= 1;
        if cs[i].is_some() && (i == 0 || sub_boundary(&cs, i)) {
            return gs[i].0;
        }
    }
    0
}

/// Zed-style subword →: to the end of the subword after the cursor.
pub(crate) fn subword_right(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let cs: Vec<Option<Sub>> = gs.iter().map(|g| sub_class(g.1)).collect();
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i < gs.len() {
        i += 1;
        if cs[i - 1].is_some() && (i == gs.len() || sub_boundary(&cs, i)) {
            break;
        }
    }
    gs.get(i).map(|g| g.0).unwrap_or(s.chars().count())
}

/// Start of the (newline-separated) line holding `cursor`.
pub(crate) fn line_start(s: &str, cursor: usize) -> usize {
    let head: Vec<char> = s.chars().take(cursor).collect();
    head.iter().rposition(|&c| c == '\n').map(|p| p + 1).unwrap_or(0)
}

/// End of the (newline-separated) line holding `cursor` (before its '\n').
pub(crate) fn line_end(s: &str, cursor: usize) -> usize {
    let mut ci = cursor;
    for c in s.chars().skip(cursor) {
        if c == '\n' {
            break;
        }
        ci += 1;
    }
    ci
}

/// The word (or the run of non-word graphemes) around `ci`, for a
/// double click.
pub(crate) fn word_at(s: &str, ci: usize) -> (usize, usize) {
    let gs = graphemes_ci(s);
    let Some(i) = gs.iter().rposition(|&(c, _)| c <= ci) else {
        return (0, 0);
    };
    if gs[i].1 == "\n" {
        return (gs[i].0, gs[i].0);
    }
    let kind = is_word(gs[i].1);
    let same = |g: &str| is_word(g) == kind && g != "\n" && (kind || !g.chars().all(char::is_whitespace) == !gs[i].1.chars().all(char::is_whitespace));
    let mut a = i;
    while a > 0 && same(gs[a - 1].1) {
        a -= 1;
    }
    let mut b = i + 1;
    while b < gs.len() && same(gs[b].1) {
        b += 1;
    }
    (gs[a].0, gs.get(b).map(|g| g.0).unwrap_or(s.chars().count()))
}

// ---- the wrapped layout ----

/// One cell of the composer layout: a grapheme, its first char index,
/// its width in columns. A newline is a 1-column slot shown only when the
/// cursor is on it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InputCell<'a> {
    pub(crate) ci: usize,
    pub(crate) text: &'a str,
    pub(crate) w: usize,
    pub(crate) newline: bool,
    /// an image chip: `text` is its label `[Image #N]`, drawn `▣ N`
    /// (attach::chip_text) in `w` columns
    pub(crate) chip: bool,
}

/// `cell` onto the composer's last row: a cell that overflows moves,
/// with the word it ends (the cells after the row's last space, `brk`),
/// to a new row; a word alone on its row is cut.
fn place<'a>(
    inner: usize,
    rows: &mut Vec<Vec<InputCell<'a>>>,
    row: &mut Vec<InputCell<'a>>,
    col: &mut usize,
    brk: &mut Option<usize>,
    cell: InputCell<'a>,
    space: bool,
) {
    if !row.is_empty() && *col + cell.w > inner {
        let tail = match *brk {
            Some(b) if b < row.len() => row.split_off(b),
            _ => Vec::new(),
        };
        rows.push(std::mem::replace(row, tail));
        *col = row.iter().map(|c| c.w).sum();
        *brk = None;
        if !row.is_empty() && *col + cell.w > inner {
            rows.push(std::mem::take(row));
            *col = 0;
        }
    }
    *col += cell.w;
    row.push(cell);
    if space {
        *brk = Some(row.len());
    }
}

/// The composer rows at `inner` columns: newlines break rows, long rows
/// wrap at word boundaries like your message in the history (a space
/// stays at the end of its row; only a word longer than the row is cut),
/// by width (the widths ratatui uses, so a 2-column emoji never overflows
/// the row), and the end of the text gets a 1-column cursor slot.
pub(crate) fn layout_input(input: &str, inner: usize) -> Vec<Vec<InputCell<'_>>> {
    let inner = inner.max(2);
    let mut rows: Vec<Vec<InputCell>> = Vec::new();
    let mut row: Vec<InputCell> = Vec::new();
    let mut col = 0usize;
    // where the row may break: after its last space (a cell index)
    let mut brk: Option<usize> = None;
    let mut ci = 0usize;
    // an image chip `[Image #N]` is one cell, drawn `▣ N` (attach.rs)
    let chips = crate::attach::chips(input);
    let mut chip_i = 0usize;
    let mut skip_to = 0usize;
    for (bi, g) in input.grapheme_indices(true) {
        let n = g.chars().count();
        if ci < skip_to {
            ci += n;
            continue;
        }
        while chips.get(chip_i).is_some_and(|c| c.1 <= ci) {
            chip_i += 1;
        }
        if let Some(&(a, b, _)) = chips.get(chip_i).filter(|c| c.0 == ci) {
            let label = &input[bi..bi + (b - a)]; // ASCII: chars = bytes
            let w = crate::attach::chip_width(label, inner);
            place(inner, &mut rows, &mut row, &mut col, &mut brk, InputCell { ci, text: label, w, newline: false, chip: true }, false);
            skip_to = b;
            ci += n;
            continue;
        }
        if g == "\n" || g == "\r\n" {
            // the newline's slot needs a free column, like the end slot
            if col >= inner {
                rows.push(std::mem::take(&mut row));
            }
            row.push(InputCell { ci, text: " ", w: 1, newline: true, chip: false });
            rows.push(std::mem::take(&mut row));
            col = 0;
            brk = None;
            ci += n;
            continue;
        }
        // a control char (a pasted tab) shows as one blank column
        let (text, w) = if g.chars().any(char::is_control) { (" ", 1) } else { (g, g.width().max(1)) };
        let space = g == " " || g == "\t";
        place(inner, &mut rows, &mut row, &mut col, &mut brk, InputCell { ci, text, w, newline: false, chip: false }, space);
        ci += n;
    }
    // the end slot: on a new row when the last one is full
    if col >= inner {
        rows.push(std::mem::take(&mut row));
    }
    row.push(InputCell { ci, text: " ", w: 1, newline: true, chip: false });
    rows.push(row);
    rows
}

/// How many of the `rows` the composer draws: a trailing row holding
/// only the end slot after a full row (the text ends exactly at the
/// width) shows only when the cursor sits on it. The composer height
/// and the draw both use it, so they never disagree by one row (the
/// top row was clipped when the cursor reached that slot).
pub(crate) fn drawn_rows(rows: &[Vec<InputCell>], cursor: usize) -> usize {
    let n = rows.len();
    let only_end_slot = n > 1
        && rows[n - 1].len() == 1
        && !rows[n - 2].last().is_some_and(|c| c.newline);
    if only_end_slot && rows[n - 1][0].ci != cursor {
        n - 1
    } else {
        n
    }
}

/// The first row the composer shows of `total` drawn rows in a box of
/// `h` rows: `top` as it was (the wheel may have moved it), never past
/// the last screenful; with `follow` (the cursor or the text changed
/// since the last frame) moved the least that shows the cursor row,
/// like a web text field.
pub(crate) fn view_top(top: usize, total: usize, h: usize, cur_row: usize, follow: bool) -> usize {
    let h = h.max(1);
    let top = top.min(total.saturating_sub(h));
    if !follow {
        top
    } else if cur_row < top {
        cur_row
    } else if cur_row >= top + h {
        cur_row + 1 - h
    } else {
        top
    }
}

/// (row, column) of the char index `ci` in the layout.
pub(crate) fn row_col(rows: &[Vec<InputCell>], ci: usize) -> (usize, usize) {
    let mut last = (0, 0);
    for (r, row) in rows.iter().enumerate() {
        let mut col = 0;
        for c in row {
            if c.ci == ci {
                return (r, col);
            }
            if c.ci > ci {
                return last;
            }
            last = (r, col);
            col += c.w;
        }
    }
    last
}

/// The char index at (row, column): the cell covering the column, or the
/// row's last cell when the column is past it.
pub(crate) fn ci_at(rows: &[Vec<InputCell>], row: usize, col: usize) -> usize {
    let Some(cells) = rows.get(row.min(rows.len().saturating_sub(1))) else {
        return 0;
    };
    let mut x = 0;
    for c in cells {
        if col < x + c.w {
            // the right half of a wide grapheme: after it, like a text field
            return if col > x && c.w > 1 && !c.newline && col >= x + c.w / 2 + c.w % 2 {
                c.ci + c.text.chars().count()
            } else {
                c.ci
            };
        }
        x += c.w;
    }
    cells.last().map(|c| c.ci).unwrap_or(0)
}

// ---- the editor ----


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    SubwordLeft,
    SubwordRight,
    LineStart,
    LineEnd,
    TextStart,
    TextEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Grapheme,
    Word,
    Subword,
    /// to the line start (backward) or the line end (forward)
    Line,
}

/// An editing action, the output of the key map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Move(Motion, bool),
    /// Up/Down: a row move, the history at the edges (the caller decides:
    /// popups first). `true` = extend the selection (no history).
    Up(bool),
    Down(bool),
    DeleteBack(Unit),
    DeleteForward(Unit),
    Insert(String),
    /// A macOS dead key (Option+` ´ ˆ ¨ ˜): the accent waits for the
    /// next typed letter.
    Dead(char),
    Undo,
    Redo,
    SelectAll,
    Copy,
    Cut,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Editor {
    pub(crate) text: String,
    /// char index, on a grapheme boundary
    pub(crate) cursor: usize,
    /// the other end of the selection (none when equal to the cursor)
    pub(crate) anchor: Option<usize>,
    /// undo/redo (undo.rs)
    steps: Steps,
    /// history browsing: the entry shown (0 = newest), the draft saved on
    /// the first Up, and the edits made to recalled entries
    hist_idx: Option<usize>,
    draft: Option<Snap>,
    scratch: std::collections::HashMap<usize, String>,
    /// a dead key waiting for its letter (its spacing accent)
    dead: Option<char>,
    /// ↑/↓'s column (the goal column of a web text field): kept while
    /// the cursor is where the last row move left it, so going through a
    /// short line comes back to the column you started from
    goal: Option<(usize, usize)>,
}

impl Editor {
    pub(crate) fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The selected range, if not empty.
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        Some(self.text.chars().skip(a).take(b - a).collect())
    }

    /// The user's own draft (BISE-120a, saved on disk): the text, or
    /// while browsing the history the draft kept on the first Up.
    pub(crate) fn own_draft(&self) -> (&str, usize) {
        match (self.hist_idx, &self.draft) {
            (Some(_), Some(d)) => (&d.text, d.cursor),
            _ => (&self.text, self.cursor),
        }
    }

    pub(crate) fn browsing(&self) -> bool {
        self.hist_idx.is_some()
    }

    fn snap(&self) -> Snap {
        Snap { text: self.text.clone(), cursor: self.cursor, anchor: self.anchor, hist: self.hist_idx }
    }

    /// Records the state before an edit of `kind` (merged with the edit
    /// before when of the same kind; typing breaks at word starts).
    fn checkpoint(&mut self, kind: Kind, word_start: bool) {
        let before = self.snap();
        self.steps.record(kind, word_start, || before);
    }

    /// Ends the current undo group (a move, a pause in the voice).
    pub(crate) fn break_undo(&mut self) {
        self.steps.end_group();
    }

    fn replace(&mut self, a: usize, b: usize, s: &str) {
        let (ba, bb) = (byte_at_char(&self.text, a), byte_at_char(&self.text, b));
        self.text.replace_range(ba..bb, s);
        self.cursor = a + s.chars().count();
        self.anchor = None;
    }

    fn insert_kind(&mut self, s: &str, kind: Kind) {
        if s.is_empty() {
            return;
        }
        let (a, b) = self.selection().unwrap_or((self.cursor, self.cursor));
        // a new word starts an undo step ("hello world" undoes by word)
        let word_start = kind == Kind::Typing
            && a > 0
            && !s.starts_with(char::is_whitespace)
            && self.text.chars().nth(a - 1).is_some_and(char::is_whitespace);
        self.checkpoint(if a != b { Kind::Other } else { kind }, word_start);
        self.replace(a, b, s);
        if a != b {
            self.steps.continue_as(kind);
        }
    }

    /// Typed text at the cursor (replaces the selection); a pending dead
    /// key composes with it.
    pub(crate) fn insert(&mut self, s: &str) {
        match self.dead.take() {
            Some(a) => self.insert_kind(&compose(a, s), Kind::Typing),
            None => self.insert_kind(s, Kind::Typing),
        }
    }

    /// A dead key: the accent waits for the next typed text (a second
    /// dead key types the first accent alone).
    pub(crate) fn dead_key(&mut self, accent: char) {
        if let Some(a) = self.dead.replace(accent) {
            self.insert_kind(&a.to_string(), Kind::Typing);
        }
    }

    /// The accent of a pending dead key (drawn at the cursor).
    pub(crate) fn pending_dead(&self) -> Option<char> {
        self.dead
    }

    /// A paste: its own undo step.
    pub(crate) fn paste(&mut self, s: &str) {
        self.break_undo();
        self.insert_kind(s, Kind::Other);
        self.break_undo();
    }

    /// Voice deltas: consecutive ones undo together.
    pub(crate) fn insert_voice(&mut self, s: &str) {
        self.insert_kind(s, Kind::Voice);
    }

    /// The char index of the live mark `label` (the voice chip) in the text.
    pub(crate) fn mark_at(&self, label: &str) -> Option<usize> {
        char_find(&self.text, label)
    }

    /// Puts the live mark `label` (the voice chip, BISE-222) at the
    /// cursor, the cursor after it; a selection just ends. Not an undo
    /// step: the mark is transient, what replaces it is the step
    /// ([`Editor::swap_mark`]).
    pub(crate) fn put_mark(&mut self, label: &str) {
        self.anchor = None;
        let at = self.cursor;
        let b = byte_at_char(&self.text, at);
        self.text.insert_str(b, label);
        self.cursor = at + label.chars().count();
        self.break_undo();
    }

    /// Replaces the live mark `label` with `with`, the cursor `cursor`
    /// chars into it: one undo step back to the text without the mark.
    /// An empty `with` takes the mark away, the cursor stays by the
    /// text around it. The mark leaves every saved state too (undo,
    /// redo, the history draft): no undo brings a dead chip back. False
    /// when the text has no mark.
    pub(crate) fn swap_mark(&mut self, label: &str, with: &str, cursor: usize) -> bool {
        let n = label.chars().count();
        let strip = |s: &mut Snap| {
            if let Some(p) = char_find(&s.text, label) {
                let b = byte_at_char(&s.text, p);
                s.text.replace_range(b..b + label.len(), "");
                if s.cursor > p {
                    s.cursor = s.cursor.saturating_sub(n).max(p);
                }
            }
        };
        self.steps.states_mut().chain(self.draft.iter_mut()).for_each(strip);
        for t in self.scratch.values_mut() {
            *t = t.replacen(label, "", 1);
        }
        let Some(p) = self.mark_at(label) else { return false };
        let mut before = self.snap();
        strip(&mut before);
        let b = byte_at_char(&self.text, p);
        self.text.replace_range(b..b + label.len(), with);
        self.anchor = None;
        self.break_undo();
        if with.is_empty() {
            self.cursor = before.cursor;
        } else {
            self.steps.push(Snap { anchor: None, ..before });
            self.cursor = p + cursor.min(with.chars().count());
        }
        true
    }

    /// Replaces the whole text (a popup completion), one undo step.
    pub(crate) fn set(&mut self, s: &str, cursor: usize) {
        if s == self.text {
            self.cursor = cursor.min(self.len());
            self.anchor = None;
            return;
        }
        self.checkpoint(Kind::Other, false);
        self.text = s.to_string();
        self.cursor = cursor.min(self.len());
        self.anchor = None;
    }

    /// Empties the composer (Esc on a popup): undoable.
    pub(crate) fn clear(&mut self) {
        self.set("", 0);
    }

    /// Takes the text to send: the composer, the undo and the history
    /// browsing all start over.
    pub(crate) fn take(&mut self) -> String {
        let t = std::mem::take(&mut self.text);
        *self = Editor::default();
        t
    }

    fn target(&self, m: Motion) -> usize {
        let (s, c) = (&self.text, self.cursor);
        match m {
            Motion::Left => prev_grapheme(s, c),
            Motion::Right => next_grapheme(s, c),
            Motion::WordLeft => word_left(s, c),
            Motion::WordRight => word_right(s, c),
            Motion::SubwordLeft => subword_left(s, c),
            Motion::SubwordRight => subword_right(s, c),
            Motion::LineStart => line_start(s, c),
            Motion::LineEnd => line_end(s, c),
            Motion::TextStart => 0,
            Motion::TextEnd => self.len(),
        }
    }

    fn move_to(&mut self, to: usize, select: bool) {
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        // never inside an image chip: its edge on the side of the move
        let to = match crate::attach::chip_around(&self.text, to) {
            Some((a, b)) => if to < self.cursor { a } else { b },
            None => to,
        };
        self.cursor = to;
        self.goal = None;
        self.break_undo();
    }

    pub(crate) fn move_cursor(&mut self, m: Motion, select: bool) {
        // ←/→ on a selection collapse it to its edge, like a text field
        if let (false, Some((a, b))) = (select, self.selection()) {
            match m {
                Motion::Left => return self.move_to(a, false),
                Motion::Right => return self.move_to(b, false),
                _ => {}
            }
        }
        let to = self.target(m);
        self.move_to(to, select);
    }

    /// Places the cursor (a click), or extends the selection to `ci`
    /// (a drag / shift-click).
    pub(crate) fn click(&mut self, ci: usize, select: bool) {
        let ci = ci.min(self.len());
        self.move_to(ci, select);
    }

    pub(crate) fn select_range(&mut self, a: usize, b: usize) {
        let n = self.len();
        self.anchor = Some(a.min(n));
        self.cursor = b.min(n);
        self.break_undo();
    }

    pub(crate) fn select_all(&mut self) {
        self.select_range(0, self.len());
    }

    /// Up one visual row at `width` columns. False on the first row (the
    /// caller recalls the history); the cursor then goes to the text
    /// start when there is no history to show.
    pub(crate) fn row_up(&mut self, width: usize, select: bool) -> bool {
        self.row_move(width, false, select)
    }

    /// Down one visual row. False on the last row.
    pub(crate) fn row_down(&mut self, width: usize, select: bool) -> bool {
        self.row_move(width, true, select)
    }

    /// One row up or down at the goal column (the cursor's column when
    /// the last move was not a row move); the selection grows or shrinks
    /// from its anchor. False on the first / last row.
    fn row_move(&mut self, width: usize, down: bool, select: bool) -> bool {
        let rows = layout_input(&self.text, width);
        let (r, col) = row_col(&rows, self.cursor);
        let row = match (down, r) {
            (false, 0) => return false,
            (false, r) => r - 1,
            (true, r) if r + 1 >= rows.len() => return false,
            (true, r) => r + 1,
        };
        let col = match self.goal {
            Some((c, at)) if at == self.cursor => c,
            _ => col,
        };
        let to = ci_at(&rows, row, col);
        self.move_to(to, select);
        self.goal = Some((col, self.cursor));
        true
    }

    pub(crate) fn delete_back(&mut self, u: Unit) {
        if let Some((a, b)) = self.selection() {
            self.checkpoint(Kind::Other, false);
            return self.replace(a, b, "");
        }
        let c = self.cursor;
        let a = match u {
            Unit::Grapheme => prev_grapheme(&self.text, c),
            Unit::Word => word_left(&self.text, c),
            Unit::Subword => subword_left(&self.text, c),
            // at a line start, joins the line above
            Unit::Line => match line_start(&self.text, c) {
                s if s == c => prev_grapheme(&self.text, c),
                s => s,
            },
        };
        // an image chip goes whole (attach.rs)
        let (a, c) = crate::attach::chip_widen(&self.text, a, c);
        if a < c {
            self.checkpoint(if u == Unit::Grapheme { Kind::Deleting } else { Kind::Other }, false);
            self.replace(a, c, "");
        }
    }

    pub(crate) fn delete_forward(&mut self, u: Unit) {
        if let Some((a, b)) = self.selection() {
            self.checkpoint(Kind::Other, false);
            return self.replace(a, b, "");
        }
        let c = self.cursor;
        let b = match u {
            Unit::Grapheme => next_grapheme(&self.text, c),
            Unit::Word => word_right(&self.text, c),
            Unit::Subword => subword_right(&self.text, c),
            Unit::Line => match line_end(&self.text, c) {
                e if e == c => next_grapheme(&self.text, c),
                e => e,
            },
        };
        let (c, b) = crate::attach::chip_widen(&self.text, c, b);
        if b > c {
            self.checkpoint(if u == Unit::Grapheme { Kind::Deleting } else { Kind::Other }, false);
            self.replace(c, b, "");
            self.cursor = c;
        }
    }

    pub(crate) fn undo(&mut self) -> bool {
        let now = self.snap();
        let Some(s) = self.steps.undo(now) else { return false };
        self.restore(s);
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let now = self.snap();
        let Some(s) = self.steps.redo(now) else { return false };
        self.restore(s);
        true
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.steps.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.steps.can_redo()
    }

    /// Back to a kept state: the text, the cursor, the selection and the
    /// history entry shown (out of the history: the draft is the text
    /// again; back into it: the text now is the draft).
    fn restore(&mut self, s: Snap) {
        match (self.hist_idx, s.hist) {
            (Some(_), None) => {
                self.draft = None;
                self.scratch.clear();
            }
            (None, Some(_)) => self.draft = Some(self.snap()),
            _ => {}
        }
        let n = s.text.chars().count();
        self.text = s.text;
        self.cursor = s.cursor.min(n);
        self.anchor = s.anchor.map(|a| a.min(n));
        self.hist_idx = s.hist;
        self.goal = None;
    }

    /// Up on the first row: the next older history entry (`history[0]` is
    /// the newest). The draft is saved on the first step, the edits made
    /// to a recalled entry are kept while browsing. False when there is
    /// nothing older.
    pub(crate) fn history_up(&mut self, history: &[String]) -> bool {
        let next = self.hist_idx.map_or(0, |i| i + 1);
        if next >= history.len() {
            return false;
        }
        self.checkpoint(Kind::Recall, false);
        match self.hist_idx {
            None => self.draft = Some(self.snap()),
            Some(i) => self.keep_scratch(history, i),
        }
        self.show_entry(history, next);
        true
    }

    /// Down on the last row while browsing: the next newer entry, then
    /// the saved draft as it was. False when not browsing.
    pub(crate) fn history_down(&mut self, history: &[String]) -> bool {
        let Some(i) = self.hist_idx else { return false };
        self.checkpoint(Kind::Recall, false);
        self.keep_scratch(history, i);
        if i == 0 {
            let d = self.draft.take().unwrap_or_default();
            self.hist_idx = None;
            self.scratch.clear();
            self.text = d.text;
            self.cursor = d.cursor.min(self.len());
            self.anchor = None;
        } else {
            self.show_entry(history, i - 1);
        }
        true
    }

    fn keep_scratch(&mut self, history: &[String], i: usize) {
        if history.get(i) != Some(&self.text) {
            self.scratch.insert(i, self.text.clone());
        } else {
            self.scratch.remove(&i);
        }
    }

    fn show_entry(&mut self, history: &[String], i: usize) {
        self.hist_idx = Some(i);
        // total: a history that changed under the browse shows an empty entry
        self.text = self.scratch.get(&i).or(history.get(i)).cloned().unwrap_or_default();
        self.cursor = self.len();
        self.anchor = None;
    }

    /// Applies an editing action. `Up`/`Down`/`Copy`/`Cut` need the
    /// caller (layout width, history, clipboard): they return false here.
    pub(crate) fn apply(&mut self, a: &Action) -> bool {
        // a pending dead key: Backspace drops it, any other action too
        if self.dead.is_some() && !matches!(a, Action::Insert(_) | Action::Dead(_)) {
            self.dead = None;
            if matches!(a, Action::DeleteBack(_)) {
                return true;
            }
        }
        match a {
            Action::Dead(c) => self.dead_key(*c),
            Action::Move(m, sel) => self.move_cursor(*m, *sel),
            Action::DeleteBack(u) => self.delete_back(*u),
            Action::DeleteForward(u) => self.delete_forward(*u),
            Action::Insert(s) => self.insert(s),
            Action::Undo => {
                self.undo();
            }
            Action::Redo => {
                self.redo();
            }
            Action::SelectAll => self.select_all(),
            Action::Up(_) | Action::Down(_) | Action::Copy | Action::Cut => return false,
        }
        true
    }

    /// Cut: the selected text, removed (one undo step).
    pub(crate) fn cut(&mut self) -> Option<String> {
        let t = self.selected_text()?;
        self.delete_back(Unit::Grapheme);
        Some(t)
    }
}

// ---- the macOS Option layer ----
//
// Ghostty treats Option as Alt on the U.S. layouts by default
// (`macos-option-as-alt` unset = true there): Option+` then e reaches
// the app as Alt+` then e, never as "è". The composer rebuilds what
// macOS would type: the dead keys compose with the next letter, the
// other Option keys give their character. Word moves (Alt+b/f, what
// Ghostty sends for Option+←/→), Alt+d, Alt+/ and Alt+digits keep
// their app meaning.

/// A key of the macOS U.S. Option layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptionKey {
    /// the spacing accent: ` ´ ˆ ¨ ˜
    Dead(char),
    Char(char),
}

/// The U.S. shifted symbol of an unshifted key (the kitty protocol
/// reports Option+Shift+1 as '1' with Shift; the legacy one as '!').
fn us_shift(c: char) -> char {
    const BASE: &str = "`1234567890-=[]\\;',./";
    const SHIFTED: &str = "~!@#$%^&*()_+{}||:\"<>?";
    if c.is_ascii_lowercase() {
        return c.to_ascii_uppercase();
    }
    match BASE.chars().position(|b| b == c) {
        Some(i) => SHIFTED.chars().nth(i).unwrap_or(c),
        None => c,
    }
}

/// What Option + `c` types on the macOS U.S. layout.
pub(crate) fn option_layer(c: char, shift: bool) -> Option<OptionKey> {
    let c = if shift { us_shift(c) } else { c };
    let dead = match c {
        '`' => Some('`'),
        'e' => Some('´'),
        'i' => Some('ˆ'),
        'u' => Some('¨'),
        'n' => Some('˜'),
        _ => None,
    };
    if let Some(a) = dead {
        return Some(OptionKey::Dead(a));
    }
    const KEYS: &str = "1234567890-=qwrtyop[]\\asdfghjkl;'zxcvbm,./!@#$%^&*()_+QWERTYUIOP{}|ASDFGHJKL:\"ZXCVBNM<>?~";
    const CHARS: &str = "¡™£¢∞§¶•ªº–≠œ∑®†¥øπ“‘«åß∂ƒ©˙∆˚¬…æΩ≈ç√∫µ≤≥÷⁄€‹›ﬁﬂ‡°·‚—±Œ„´‰ˇÁ¨ˆØ∏”’»ÅÍÎÏ˝ÓÔ\u{F8FF}ÒÚÆ¸˛Ç◊ı˜Â¯˘¿`";
    KEYS.chars().position(|k| k == c).and_then(|i| CHARS.chars().nth(i)).map(OptionKey::Char)
}

/// A dead key's accent on the next typed text: the accented letter,
/// the accent alone before a space, else the accent then the text
/// (what macOS does).
pub(crate) fn compose(accent: char, text: &str) -> String {
    let mut it = text.chars();
    let (Some(c), rest) = (it.next(), it.as_str()) else { return accent.to_string() };
    if c == ' ' {
        return format!("{}{}", accent, rest);
    }
    let (from, to) = match accent {
        '`' => ("aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        '´' => ("aeiouyAEIOUY", "áéíóúýÁÉÍÓÚÝ"),
        'ˆ' => ("aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        '¨' => ("aeiouyAEIOUY", "äëïöüÿÄËÏÖÜŸ"),
        '˜' => ("anoANO", "ãñõÃÑÕ"),
        _ => ("", ""),
    };
    match from.chars().position(|f| f == c).and_then(|i| to.chars().nth(i)) {
        Some(x) => format!("{}{}", x, rest),
        None => format!("{}{}", accent, text),
    }
}

// ---- the key map ----

/// The editing action of a key, macOS text-field style. What Ghostty
/// sends by default: Option+←/→ = ESC b / ESC f (Alt+b/f), Cmd+←/→ =
/// Ctrl+A / Ctrl+E, Cmd+Backspace = Ctrl+U; Cmd+↑/↓, Cmd+A/C/Z are its
/// own unless unbound (then they arrive with SUPER under the kitty
/// keyboard protocol). Ctrl+Option+←/→ has no Ghostty binding: it
/// arrives as ←/→ with CONTROL|ALT (CSI 1;7D/C, also without the kitty
/// flags) and moves by subword; macOS keeps only Ctrl+←/→ (Spaces).
pub(crate) fn action(k: &KeyEvent) -> Option<Action> {
    use Action::*;
    use Motion::*;
    let m = k.modifiers;
    let shift = m.contains(KeyModifiers::SHIFT);
    let alt = m.contains(KeyModifiers::ALT);
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let sup = m.contains(KeyModifiers::SUPER);
    let plain = !alt && !ctrl && !sup;
    Some(match k.code {
        KeyCode::Left | KeyCode::Right => {
            let right = k.code == KeyCode::Right;
            let mo = if sup {
                if right { LineEnd } else { LineStart }
            } else if alt && ctrl {
                if right { SubwordRight } else { SubwordLeft }
            } else if alt || ctrl {
                if right { WordRight } else { WordLeft }
            } else if right {
                Right
            } else {
                Left
            };
            Move(mo, shift)
        }
        KeyCode::Up if sup || ctrl => Move(TextStart, shift),
        KeyCode::Down if sup || ctrl => Move(TextEnd, shift),
        KeyCode::Up if plain => Up(shift),
        KeyCode::Down if plain => Down(shift),
        KeyCode::Home if ctrl || sup => Move(TextStart, shift),
        KeyCode::End if ctrl || sup => Move(TextEnd, shift),
        KeyCode::Home => Move(LineStart, shift),
        KeyCode::End => Move(LineEnd, shift),
        KeyCode::Backspace if sup => DeleteBack(Unit::Line),
        KeyCode::Backspace if alt && ctrl => DeleteBack(Unit::Subword),
        KeyCode::Backspace if alt || ctrl => DeleteBack(Unit::Word),
        KeyCode::Backspace => DeleteBack(Unit::Grapheme),
        KeyCode::Delete if sup => DeleteForward(Unit::Line),
        KeyCode::Delete if alt && ctrl => DeleteForward(Unit::Subword),
        KeyCode::Delete if alt || ctrl => DeleteForward(Unit::Word),
        KeyCode::Delete => DeleteForward(Unit::Grapheme),
        KeyCode::Char(c) if ctrl && !alt && !sup => match c.to_ascii_lowercase() {
            'a' => Move(LineStart, shift),
            'e' => Move(LineEnd, shift),
            'b' if !shift => Move(Left, false),
            'f' if !shift => Move(Right, false),
            'u' => DeleteBack(Unit::Line),
            'k' => DeleteForward(Unit::Line),
            'w' => DeleteBack(Unit::Word),
            'h' => DeleteBack(Unit::Grapheme),
            'd' => DeleteForward(Unit::Grapheme),
            'c' if shift => Copy,
            'x' if shift => Cut,
            // ctrl+shift+z only where the terminal tells it from ctrl+z
            // (the kitty keyboard protocol); ctrl+y redoes everywhere
            // (input.rs, it copies a code block when there is no redo)
            'z' if shift => Redo,
            'z' => Undo,
            // Ctrl+/ (kitty) = Ctrl+_ = 0x1F, which the legacy parser
            // reads as Ctrl+7; with Shift (Ctrl+?) it redoes
            '/' | '_' | '7' if !shift => Undo,
            '/' | '?' | '_' => Redo,
            _ => return None,
        },
        // legacy Ctrl+Option+Backspace: ESC ^H
        KeyCode::Char('h') if alt && ctrl && !sup => DeleteBack(Unit::Subword),
        KeyCode::Char(c) if alt && !ctrl && !sup => match c {
            'b' => Move(WordLeft, false),
            'f' => Move(WordRight, false),
            'B' => Move(WordLeft, true),
            'F' => Move(WordRight, true),
            'd' => DeleteForward(Unit::Word),
            '/' => Redo,
            // the rest of the Option layer (Ghostty sends Option as Alt
            // on U.S. layouts): dead keys and the Option characters
            _ => match option_layer(c, shift) {
                Some(OptionKey::Dead(a)) => Dead(a),
                Some(OptionKey::Char(ch)) => Insert(ch.to_string()),
                None => return None,
            },
        },
        KeyCode::Char(c) if sup && !ctrl && !alt => match c.to_ascii_lowercase() {
            'z' if shift => Redo,
            'z' => Undo,
            'a' => SelectAll,
            'c' => Copy,
            'x' => Cut,
            _ => return None,
        },
        KeyCode::Char(c) if plain => Insert(c.to_string()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str, cursor: usize) -> Editor {
        Editor { text: text.into(), cursor, ..Default::default() }
    }

    fn key(code: KeyCode, m: KeyModifiers) -> Option<Action> {
        action(&KeyEvent::new(code, m))
    }

    #[test]
    fn image_chips_are_atomic() {
        // `[Image #1]` is chars 4..14: one cell ` ▣ 1 `, one step, one delete
        let t = "see [Image #1] ok";
        let rows = layout_input(t, 40);
        let chip: Vec<_> = rows[0].iter().filter(|c| c.chip).collect();
        assert_eq!(chip.len(), 1);
        assert_eq!((chip[0].ci, chip[0].text, chip[0].w), (4, "[Image #1]", 5));
        assert_eq!(rows[0].iter().filter(|c| !c.newline).map(|c| c.w).sum::<usize>(), 4 + 5 + 3);
        assert_eq!(next_grapheme(t, 4), 14);
        assert_eq!(prev_grapheme(t, 14), 4);
        assert_eq!(row_col(&rows, 14), (0, 9));
        let mut e = ed(t, 14);
        e.delete_back(Unit::Grapheme);
        assert_eq!((e.text.as_str(), e.cursor), ("see  ok", 4));
        let mut e = ed(t, 4);
        e.delete_forward(Unit::Grapheme);
        assert_eq!((e.text.as_str(), e.cursor), ("see  ok", 4));
        // a word delete that bites into the chip takes it whole
        let mut e = ed(t, 14);
        e.delete_back(Unit::Word);
        assert_eq!(e.text, "see  ok");
        // arrows never land inside
        let mut e = ed(t, 4);
        e.move_cursor(Motion::Right, false);
        assert_eq!(e.cursor, 14);
        e.move_cursor(Motion::Left, false);
        assert_eq!(e.cursor, 4);
        e.click(9, false);
        assert!(e.cursor == 4 || e.cursor == 14);
        // a chip wraps whole
        let rows = layout_input("abcdef [Image #2]", 8);
        assert!(rows.iter().any(|r| r.first().is_some_and(|c| c.chip)));
    }

    #[test]
    fn word_moves_skip_punctuation_and_emojis() {
        let s = "hello, wörld 👍🏽 foo_bar";
        let mut stops = vec![s.chars().count()];
        loop {
            let c = word_left(s, *stops.last().unwrap());
            if c == *stops.last().unwrap() {
                break;
            }
            stops.push(c);
        }
        assert_eq!(stops, vec![23, 16, 7, 0]);
        let mut stops = vec![0];
        loop {
            let c = word_right(s, *stops.last().unwrap());
            if c == *stops.last().unwrap() {
                break;
            }
            stops.push(c);
        }
        assert_eq!(stops, vec![0, 5, 12, 23]);
    }

    /// Every stop of repeated moves from one end of `s`.
    fn stops(s: &str, mv: fn(&str, usize) -> usize, from: usize) -> Vec<usize> {
        let mut v = vec![from];
        loop {
            let c = mv(s, *v.last().unwrap());
            if c == *v.last().unwrap() {
                return v;
            }
            v.push(c);
        }
    }

    /// The pieces between the stops of subword → from the start.
    fn right_pieces(s: &str) -> Vec<String> {
        let st = stops(s, subword_right, 0);
        let cs: Vec<char> = s.chars().collect();
        st.windows(2).map(|w| cs[w[0]..w[1]].iter().collect()).collect()
    }

    /// The pieces between the stops of subword ← from the end.
    fn left_pieces(s: &str) -> Vec<String> {
        let st = stops(s, subword_left, s.chars().count());
        let cs: Vec<char> = s.chars().collect();
        st.windows(2).map(|w| cs[w[1]..w[0]].iter().collect()).collect()
    }

    #[test]
    fn subwords_snake_kebab_camel() {
        assert_eq!(right_pieces("snake_case_name"), ["snake", "_case", "_name"]);
        assert_eq!(left_pieces("snake_case_name"), ["name", "case_", "snake_"]);
        assert_eq!(right_pieces("__init__"), ["__init", "__"]);
        assert_eq!(right_pieces("kebab-case-name"), ["kebab", "-case", "-name"]);
        assert_eq!(left_pieces("kebab-case"), ["case", "kebab-"]);
        assert_eq!(right_pieces("camelCaseName"), ["camel", "Case", "Name"]);
        assert_eq!(left_pieces("PascalCase"), ["Case", "Pascal"]);
        assert_eq!(right_pieces("ÉtéÀParis"), ["Été", "À", "Paris"]);
        // words still split at spaces and punctuation
        assert_eq!(right_pieces("fooBar baz.quxQuux"), ["foo", "Bar", " baz", ".qux", "Quux"]);
    }

    #[test]
    fn subwords_acronyms() {
        assert_eq!(right_pieces("HTTPServer"), ["HTTP", "Server"]);
        assert_eq!(left_pieces("HTTPServer"), ["Server", "HTTP"]);
        assert_eq!(right_pieces("parseHTTPResponse"), ["parse", "HTTP", "Response"]);
        assert_eq!(right_pieces("getURL"), ["get", "URL"]);
        assert_eq!(right_pieces("ALL_CAPS"), ["ALL", "_CAPS"]);
        assert_eq!(right_pieces("IOError"), ["IO", "Error"]);
        assert_eq!(right_pieces("A"), ["A"]);
    }

    #[test]
    fn subwords_digits() {
        assert_eq!(right_pieces("utf8Decode"), ["utf", "8", "Decode"]);
        assert_eq!(right_pieces("x86_64"), ["x", "86", "_64"]);
        assert_eq!(left_pieces("v2Beta10"), ["10", "Beta", "2", "v"]);
        assert_eq!(right_pieces("HTTP2Server"), ["HTTP", "2", "Server"]);
        assert_eq!(right_pieces("1234"), ["1234"]);
    }

    #[test]
    fn subwords_emojis_are_gaps_and_stay_whole() {
        // 👨‍👩‍👧 is one grapheme (ZWJ), 👍🏽 too (skin tone)
        let s = "fooBar👨‍👩‍👧bazQux 👍🏽x";
        assert_eq!(right_pieces(s), ["foo", "Bar", "👨‍👩‍👧baz", "Qux", " 👍🏽x"]);
        assert_eq!(left_pieces(s), ["x", "Qux 👍🏽", "baz", "Bar👨‍👩‍👧", "foo"]);
        // from inside a run of emojis, never inside one
        let n = "a👍🏽👍🏽b".chars().count();
        assert_eq!(subword_left("a👍🏽👍🏽b", n - 1), 0);
        assert_eq!(subword_right("a👍🏽👍🏽b", 1), n);
        assert_eq!(right_pieces("👍🏽"), ["👍🏽"]);
        assert_eq!(right_pieces(""), Vec::<String>::new());
    }

    #[test]
    fn subword_edits_and_selection() {
        let mut e = ed("fooBarBaz", 9);
        e.delete_back(Unit::Subword);
        assert_eq!((e.text.as_str(), e.cursor), ("fooBar", 6));
        let mut e = ed("fooBarBaz", 0);
        e.delete_forward(Unit::Subword);
        assert_eq!((e.text.as_str(), e.cursor), ("BarBaz", 0));
        let mut e = ed("snake_case", 5);
        e.move_cursor(Motion::SubwordRight, true);
        assert_eq!(e.selected_text().as_deref(), Some("_case"));
        e.move_cursor(Motion::SubwordLeft, false);
        assert_eq!(e.cursor, 6);
    }

    #[test]
    fn line_bounds_are_logical_lines() {
        let s = "ab\ncde\n";
        assert_eq!((line_start(s, 4), line_end(s, 4)), (3, 6));
        assert_eq!((line_start(s, 7), line_end(s, 7)), (7, 7));
        assert_eq!((line_start(s, 0), line_end(s, 0)), (0, 2));
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut e = ed("hello world", 11);
        e.move_cursor(Motion::WordLeft, true);
        assert_eq!(e.selected_text().as_deref(), Some("world"));
        e.insert("there");
        assert_eq!(e.text, "hello there");
        assert_eq!(e.selection(), None);
        e.move_cursor(Motion::LineStart, true);
        e.delete_back(Unit::Grapheme);
        assert_eq!((e.text.as_str(), e.cursor), ("", 0));
    }

    #[test]
    fn arrows_collapse_a_selection_to_its_edge() {
        let mut e = ed("abcdef", 2);
        e.move_cursor(Motion::Right, true);
        e.move_cursor(Motion::Right, true);
        assert_eq!(e.selection(), Some((2, 4)));
        e.move_cursor(Motion::Left, false);
        assert_eq!((e.cursor, e.selection()), (2, None));
    }

    #[test]
    fn selection_is_grapheme_aware() {
        let mut e = ed("a👨‍👩‍👧b", 1);
        e.move_cursor(Motion::Right, true);
        assert_eq!(e.selected_text().as_deref(), Some("👨‍👩‍👧"));
        assert_eq!(e.cut().as_deref(), Some("👨‍👩‍👧"));
        assert_eq!(e.text, "ab");
    }

    #[test]
    fn delete_units() {
        let mut e = ed("one two three", 13);
        e.delete_back(Unit::Word);
        assert_eq!(e.text, "one two ");
        e.delete_back(Unit::Line);
        assert_eq!(e.text, "");
        let mut e = ed("ab\ncd", 3);
        e.delete_back(Unit::Line); // at a line start: joins
        assert_eq!(e.text, "abcd");
        let mut e = ed("ab cd\nef", 1);
        e.delete_forward(Unit::Line);
        assert_eq!((e.text.as_str(), e.cursor), ("a\nef", 1));
        e.delete_forward(Unit::Line); // at a line end: joins
        assert_eq!(e.text, "aef");
    }

    #[test]
    fn undo_groups_words_and_redo_replays() {
        let mut e = Editor::default();
        for c in "hello world".chars() {
            e.insert(&c.to_string());
        }
        e.undo();
        assert_eq!(e.text, "hello ");
        e.undo();
        assert_eq!(e.text, "");
        assert!(!e.undo());
        e.redo();
        e.redo();
        assert_eq!(e.text, "hello world");
        // backspaces group; a move ends the group
        e.delete_back(Unit::Grapheme);
        e.delete_back(Unit::Grapheme);
        e.move_cursor(Motion::Left, false);
        e.delete_back(Unit::Grapheme);
        e.undo();
        assert_eq!(e.text, "hello wor");
        e.undo();
        assert_eq!(e.text, "hello world");
        // a new edit drops the redo
        e.insert("!");
        assert!(!e.redo());
    }

    #[test]
    fn voice_deltas_undo_together() {
        let mut e = ed("say: ", 5);
        for d in ["hello ", "there ", "friend"] {
            e.insert_voice(d);
        }
        e.undo();
        assert_eq!(e.text, "say: ");
    }

    #[test]
    fn up_past_history_and_down_restores_the_draft() {
        let hist = vec!["newest".to_string(), "older".to_string()];
        let mut e = ed("my draft", 3);
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "newest");
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "older");
        assert!(!e.history_up(&hist));
        // an edit to a recalled entry survives browsing
        e.insert("!");
        assert!(e.history_down(&hist));
        assert_eq!(e.text, "newest");
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "older!");
        assert!(e.history_down(&hist));
        assert!(e.history_down(&hist));
        assert_eq!((e.text.as_str(), e.cursor), ("my draft", 3));
        assert!(!e.browsing());
        assert!(!e.history_down(&hist));
        // an empty draft comes back empty
        let mut e = Editor::default();
        e.history_up(&hist);
        e.history_down(&hist);
        assert_eq!(e.text, "");
    }

    #[test]
    fn rows_move_across_wraps_before_the_history() {
        // 10 columns: "aaaa bbbb cccc" wraps after 10 chars
        let mut e = ed("aaaa bbbb cccc", 14);
        assert!(e.row_up(10, false));
        assert_eq!(e.cursor, 4);
        assert!(!e.row_up(10, false));
        assert!(e.row_down(10, false));
        assert_eq!(e.cursor, 14);
        assert!(!e.row_down(10, false));
        // emojis keep the display column
        let mut e = ed("👏👏x\nabcdef", 2);
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 8);
        assert!(e.row_up(40, false));
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn shift_rows_grow_and_shrink_the_selection_from_its_anchor() {
        // 5 lines of 3 chars + newline: line i starts at 4 * i
        let text = "aaa\nbbb\nccc\nddd\neee";
        let mut e = ed(text, 18); // "ee|e", the last line, column 2
        for _ in 0..3 {
            assert!(e.row_up(40, true));
        }
        // from the anchor (18) up to "bb|b" (6), the anchor kept
        assert_eq!((e.anchor, e.cursor), (Some(18), 6));
        assert_eq!(e.selected_text().as_deref(), Some("b\nccc\nddd\nee"));
        // shift+↓ shrinks it back by one row
        assert!(e.row_down(40, true));
        assert_eq!(e.selection(), Some((10, 18)));
        // back to the anchor: nothing selected, then past it the other way
        assert!(e.row_down(40, true));
        assert!(e.row_down(40, true));
        assert_eq!((e.cursor, e.selection()), (18, None));
        e.move_cursor(Motion::LineStart, true);
        assert_eq!(e.selection(), Some((16, 18)));
        // shift+← / shift+→ / shift+home extend the same selection
        e.move_cursor(Motion::Right, true);
        assert_eq!(e.selection(), Some((17, 18)));
        assert!(e.row_up(40, true));
        assert_eq!(e.selection(), Some((13, 18)));
        // cmd+shift+↑: to the text's start, still from the anchor
        e.move_cursor(Motion::TextStart, true);
        assert_eq!(e.selection(), Some((0, 18)));
        // a plain arrow collapses it
        e.move_cursor(Motion::Left, false);
        assert_eq!((e.cursor, e.selection(), e.anchor), (0, None, None));
        // a plain ↑/↓ too
        e.select_range(4, 10);
        assert!(e.row_down(40, false));
        assert_eq!((e.cursor, e.selection()), (14, None));
    }

    #[test]
    fn rows_keep_their_goal_column_through_short_lines() {
        let mut e = ed("abcdef\nx\nabcdef", 5);
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 8); // the end of "x"
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 14); // column 5 again
        assert!(e.row_up(40, true));
        assert!(e.row_up(40, true));
        assert_eq!(e.selection(), Some((5, 14)));
        // any other move sets a new column: "abc|" → "x|" → "abc|"
        e.move_cursor(Motion::Left, false);
        e.move_cursor(Motion::Left, false);
        e.move_cursor(Motion::Left, false);
        assert_eq!(e.cursor, 3);
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 8);
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 12);
        // typing forgets it too
        let mut e = ed("abcdef\nx\nabcdef", 5);
        assert!(e.row_down(40, false));
        e.insert("y");
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 12); // column 2, under "xy|"
    }

    #[test]
    fn the_view_follows_the_cursor_and_keeps_a_wheel_scroll() {
        // 20 rows in a box of 5
        assert_eq!(view_top(0, 20, 5, 0, true), 0);
        assert_eq!(view_top(0, 20, 5, 19, true), 15);
        // the cursor row already in view: no move
        assert_eq!(view_top(15, 20, 5, 17, true), 15);
        // above / under the view: the least move that shows it
        assert_eq!(view_top(15, 20, 5, 3, true), 3);
        assert_eq!(view_top(3, 20, 5, 9, true), 5);
        // the wheel moved it (no follow): kept, even without the cursor
        assert_eq!(view_top(8, 20, 5, 19, false), 8);
        // never past the last screenful (the text shrank)
        assert_eq!(view_top(15, 6, 5, 0, false), 1);
        assert_eq!(view_top(15, 3, 5, 2, true), 0);
    }

    #[test]
    fn double_click_word() {
        let s = "say hello, world";
        assert_eq!(word_at(s, 5), (4, 9));
        assert_eq!(word_at(s, 9), (9, 10));
        assert_eq!(word_at(s, 3), (3, 4));
    }

    #[test]
    fn option_dead_keys_compose_like_macos() {
        use KeyCode::Char;
        let (a, s) = (KeyModifiers::ALT, KeyModifiers::SHIFT);
        let mut e = Editor::default();
        // what Ghostty sends on a U.S. layout (Option as Alt), in the
        // legacy encoding (ESC `) and the kitty one (CSI 96;3u): both
        // parse to Alt+`
        let seq = [
            (Char('`'), a), (Char('e'), KeyModifiers::NONE),   // è
            (Char('e'), a), (Char('e'), KeyModifiers::NONE),   // é
            (Char('e'), a), (Char('E'), s),                    // É
            (Char('i'), a), (Char('o'), KeyModifiers::NONE),   // ô
            (Char('u'), a), (Char('u'), KeyModifiers::NONE),   // ü
            (Char('n'), a), (Char('n'), KeyModifiers::NONE),   // ñ
            (Char('c'), a),                                    // ç
            (Char('q'), a),                                    // œ
            (Char('\\'), a), (Char('|'), a | s),             // « »
            (Char('`'), a), (Char('x'), KeyModifiers::NONE),   // `x (no composition)
            (Char('e'), a), (Char(' '), KeyModifiers::NONE),   // ´ alone
        ];
        for (code, m) in seq {
            let act = action(&KeyEvent::new(code, m)).expect("mapped");
            e.apply(&act);
        }
        assert_eq!(e.text, "èéÉôüñçœ«»`x´");
        // kitty reports Option+Shift+\ as '\\' + Shift: the same »
        assert_eq!(key(Char('\\'), a | s), Some(Action::Insert("»".into())));
        assert_eq!(key(Char('e'), a | s), Some(Action::Insert("´".into())));
        // Backspace drops a pending accent, not the text
        let mut e = ed("ab", 2);
        e.apply(&Action::Dead('´'));
        assert_eq!(e.pending_dead(), Some('´'));
        e.apply(&Action::DeleteBack(Unit::Grapheme));
        assert_eq!((e.text.as_str(), e.pending_dead()), ("ab", None));
        // two dead keys: the first accent alone, the second waits
        e.apply(&Action::Dead('`'));
        e.apply(&Action::Dead('¨'));
        e.apply(&Action::Insert("o".into()));
        assert_eq!(e.text, "ab`ö");
        // the app's Alt keys stay theirs
        assert_eq!(key(Char('b'), a), Some(Action::Move(Motion::WordLeft, false)));
        assert_eq!(key(Char('/'), a), Some(Action::Redo));
    }

    #[test]
    fn ghostty_default_encodings() {
        use KeyCode::*;
        let (n, s, a, c, sup) =
            (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::CONTROL, KeyModifiers::SUPER);
        // Option+←/→ = ESC b / ESC f
        assert_eq!(key(Char('b'), a), Some(Action::Move(Motion::WordLeft, false)));
        assert_eq!(key(Char('f'), a), Some(Action::Move(Motion::WordRight, false)));
        assert_eq!(key(Left, a), Some(Action::Move(Motion::WordLeft, false)));
        // Cmd+←/→ = Ctrl+A / Ctrl+E, Cmd+Backspace = Ctrl+U
        assert_eq!(key(Char('a'), c), Some(Action::Move(Motion::LineStart, false)));
        assert_eq!(key(Char('e'), c), Some(Action::Move(Motion::LineEnd, false)));
        assert_eq!(key(Char('u'), c), Some(Action::DeleteBack(Unit::Line)));
        // kitty protocol: Cmd arrives as SUPER
        assert_eq!(key(Left, sup | s), Some(Action::Move(Motion::LineStart, true)));
        assert_eq!(key(Up, sup), Some(Action::Move(Motion::TextStart, false)));
        assert_eq!(key(Char('z'), sup | s), Some(Action::Redo));
        assert_eq!(key(Char('c'), sup), Some(Action::Copy));
        assert_eq!(key(Char('a'), sup), Some(Action::SelectAll));
        // selection
        assert_eq!(key(Right, s), Some(Action::Move(Motion::Right, true)));
        assert_eq!(key(Right, s | a), Some(Action::Move(Motion::WordRight, true)));
        assert_eq!(key(Up, s), Some(Action::Up(true)));
        assert_eq!(key(Up, n), Some(Action::Up(false)));
        // deletes
        assert_eq!(key(Backspace, a), Some(Action::DeleteBack(Unit::Word)));
        assert_eq!(key(Char('w'), c), Some(Action::DeleteBack(Unit::Word)));
        assert_eq!(key(Delete, a), Some(Action::DeleteForward(Unit::Word)));
        // undo: Ctrl+/ (kitty) and its legacy byte 0x1F (Ctrl+7); redo
        assert_eq!(key(Char('/'), c), Some(Action::Undo));
        assert_eq!(key(Char('7'), c), Some(Action::Undo));
        assert_eq!(key(Char('/'), a), Some(Action::Redo));
        assert_eq!(key(Char('?'), c | s), Some(Action::Redo));
        // copy/cut without Cmd
        assert_eq!(key(Char('C'), c | s), Some(Action::Copy));
        // text
        assert_eq!(key(Char('É'), s), Some(Action::Insert("É".into())));
        // Ctrl+Option+←/→: subwords (CSI 1;7D/C), Shift selects
        let ca = c | a;
        assert_eq!(key(Left, ca), Some(Action::Move(Motion::SubwordLeft, false)));
        assert_eq!(key(Right, ca), Some(Action::Move(Motion::SubwordRight, false)));
        assert_eq!(key(Left, ca | s), Some(Action::Move(Motion::SubwordLeft, true)));
        assert_eq!(key(Right, ca | s), Some(Action::Move(Motion::SubwordRight, true)));
        // Ctrl+←/→ alone stay words
        assert_eq!(key(Left, c), Some(Action::Move(Motion::WordLeft, false)));
        assert_eq!(key(Backspace, ca), Some(Action::DeleteBack(Unit::Subword)));
        assert_eq!(key(Delete, ca), Some(Action::DeleteForward(Unit::Subword)));
        assert_eq!(key(Char('h'), ca), Some(Action::DeleteBack(Unit::Subword)));
        assert_eq!(key(Backspace, c), Some(Action::DeleteBack(Unit::Word)));
        // Alt+↑/↓ stay the task navigation
        assert_eq!(key(Up, a), None);
    }
}
