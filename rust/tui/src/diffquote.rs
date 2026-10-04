//! Select + type to quote in the diff panel (diffview.rs): the thread's
//! "ask about this" (quote.rs, BISE-134) on a diff's lines, one
//! behaviour (designer m_7568).
//!
//! A drag in the panel's body selects whole lines (the thread's
//! selection tint, the full row); shift+↑↓ does the same from the
//! cursor while the panel has the keys. The release copies the lines,
//! their diff marks kept (`copied 3 lines`). Then the thread's popup
//! sits over the selection and names who gets it, the agent in view
//! (` type ask t1 about it · cmd+c copy `; [`crate::quote::hint_line`]).
//!
//! Typing quotes it: the chip goes in the composer you're in (never
//! another thread: the agent in view gets it, as in the thread), one
//! quote per file the selection touches. Its tag says where
//! ([`crate::quote::tag_at`]): `<selection from="t1 vs main"
//! file="src/a.rs" new="12-14" old="11-12">`, then the lines with
//! their marks (` `, `-`, `+`). The strip and the history name the
//! place: `src/a.rs:12-14 · 3 lines`, `src/a.rs:11-12 · 2 removed
//! lines`. Full screen (no composer in sight) the panel closes as the
//! chip goes in. esc, a plain ↑↓, a fold or new rows end the selection.

use crate::app::App;
use crate::diffview::{File, Kind, Panel};
use crate::quote::Where;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// A line of a file in the diff: its numbers in the old and the new
/// file, and the line as the hub sent it (its mark first).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CodeLine {
    pub(crate) old: Option<u32>,
    pub(crate) new: Option<u32>,
    pub(crate) raw: String,
}

impl CodeLine {
    /// The line ⏎ opens: the new one, else the old one (removed).
    pub(crate) fn line(&self) -> Option<u32> {
        self.new.or(self.old)
    }
}

/// The lines selected: body rows `anchor..=head` (either way).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sel {
    pub(crate) anchor: usize,
    pub(crate) head: usize,
    /// the button is down
    pub(crate) dragging: bool,
    /// the mouse moved since the press (else a plain click)
    pub(crate) moved: bool,
}

impl Sel {
    pub(crate) fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
    pub(crate) fn has(&self, k: usize) -> bool {
        let (a, b) = self.range();
        (a..=b).contains(&k)
    }
}

// ---- the quotes (pure) ----

/// `12`, `12-14`, empty for none.
fn span(nums: &[u32]) -> String {
    match (nums.iter().min(), nums.iter().max()) {
        (Some(a), Some(b)) if a == b => a.to_string(),
        (Some(a), Some(b)) => format!("{a}-{b}"),
        _ => String::new(),
    }
}

/// The selected rows `a..=b` as quotes, one per file they touch, in
/// order: where, and the lines with their marks. Hunk heads, file heads
/// and blanks are left out.
pub(crate) fn quotes(rows: &[Kind], files: &[File], a: usize, b: usize) -> Vec<(Where, String)> {
    let mut out: Vec<(usize, Vec<&CodeLine>)> = Vec::new();
    for k in rows.iter().take(b.saturating_add(1)).skip(a) {
        if let Kind::Code(i, c) = k {
            match out.last_mut() {
                Some((f, v)) if f == i => v.push(c),
                _ => out.push((*i, vec![c])),
            }
        }
    }
    out.into_iter()
        .map(|(i, v)| {
            let new: Vec<u32> = v.iter().filter_map(|c| c.new).collect();
            let old: Vec<u32> = v.iter().filter_map(|c| c.old).collect();
            let at = Where { file: files.get(i).map(|f| f.path.clone()).unwrap_or_default(), new: span(&new), old: span(&old) };
            let text = v.iter().map(|c| c.raw.trim_end_matches(['\r', '\n'])).collect::<Vec<_>>().join("\n");
            (at, text)
        })
        .collect()
}

/// What the selection copies: its lines, marks kept.
pub(crate) fn copy_text(p: &Panel) -> Option<String> {
    let s = p.sel.filter(|s| !s.dragging || s.moved)?;
    let d = p.diff.as_ref()?;
    let (a, b) = s.range();
    let t = quotes(&p.rows, &d.files, a, b).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("\n");
    (!t.is_empty()).then_some(t)
}

// ---- the app ----

/// A selection is up in the diff (its drag ended): the key bar's quote
/// mode.
pub(crate) fn selected(app: &App) -> bool {
    app.diff.as_ref().and_then(|p| p.sel).is_some_and(|s| !s.dragging)
}

/// The selected lines' text (cmd+c).
pub(crate) fn text(app: &App) -> Option<String> {
    app.diff.as_ref().and_then(copy_text)
}

/// A typed key with lines selected in the diff: they become quotes in
/// the composer (and the selection ends). None without one.
pub(crate) fn take(app: &mut App) -> Option<Result<String, String>> {
    let p = app.diff.as_mut()?;
    let s = p.sel.filter(|s| !s.dragging)?;
    p.sel = None;
    let d = p.diff.as_ref()?;
    let (a, b) = s.range();
    let from = d.title.clone();
    let qs = quotes(&p.rows, &d.files, a, b);
    if qs.is_empty() {
        return Some(Err("no code line selected".into()));
    }
    let mut last = Err(String::new());
    for (at, t) in qs {
        last = crate::quote::add_at(app, &from, &at, &t);
        if last.is_err() {
            break;
        }
    }
    Some(last)
}

/// A key while the panel has the keys and lines are selected (or
/// shift+↑↓ starts a selection). True when taken; false lets it go on
/// (a letter then quotes in the composer).
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(p) = app.diff.as_mut() else { return false };
    if p.list.is_some() || p.rows.is_empty() {
        return false;
    }
    let shift = k.modifiers.contains(KeyModifiers::SHIFT);
    let other = k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
    match k.code {
        KeyCode::Up | KeyCode::Down if shift && !other => {
            let n = p.rows.len();
            let anchor = p.sel.map_or(p.cursor, |s| s.anchor);
            p.cursor = if k.code == KeyCode::Up { p.cursor.saturating_sub(1) } else { (p.cursor + 1).min(n.saturating_sub(1)) };
            p.sel = Some(Sel { anchor, head: p.cursor, dragging: false, moved: true });
            true
        }
        _ if p.sel.is_none() => false,
        KeyCode::Esc => {
            p.sel = None;
            true
        }
        KeyCode::Up | KeyCode::Down => {
            p.sel = None;
            false
        }
        // full screen: no composer in sight; a letter quotes, the panel
        // closes so you see the chip and your words
        KeyCode::Char(_) if !p.side && !other => {
            match take(app) {
                Some(Err(e)) if !e.is_empty() => app.flash = Some((e, std::time::Instant::now())),
                _ => {}
            }
            app.diff = None;
            false
        }
        _ => false,
    }
}

/// The mouse on the panel's body: a press on a line starts a selection,
/// a drag extends it (and scrolls at the edges), the release copies it;
/// a release that didn't move was a click (the cursor's). Some: the
/// text to copy.
pub(crate) fn mouse(p: &mut Panel, m: &MouseEvent) -> Option<String> {
    let body = p.body;
    let row_at = |p: &Panel, y: u16| -> usize {
        let y = y.clamp(body.y, body.bottom().saturating_sub(1));
        (p.top + (y - body.y) as usize).min(p.rows.len().saturating_sub(1))
    };
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let k = row_at(p, m.row);
            p.sel = Some(Sel { anchor: k, head: k, dragging: true, moved: false });
            None
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if m.row < body.y {
                p.top = p.top.saturating_sub(1);
            } else if m.row >= body.bottom() {
                p.top = (p.top + 1).min(p.rows.len().saturating_sub(p.page));
            }
            let k = row_at(p, m.row);
            if let Some(s) = p.sel.as_mut() {
                s.head = k;
                s.moved = true;
            }
            p.cursor = k;
            None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let s = p.sel?;
            if !s.moved {
                p.sel = None;
                return None;
            }
            p.sel = Some(Sel { dragging: false, ..s });
            copy_text(p)
        }
        _ => None,
    }
}

/// The drag the panel holds (its Drag and Up events are its own, out
/// of it too).
pub(crate) fn dragging(p: &Panel) -> bool {
    p.sel.is_some_and(|s| s.dragging)
}

/// Where the popup goes over the panel's body: above the selection's
/// first row, else under its last ([`crate::quote::hint_rect`], the
/// thread's rules), at the code's first column.
pub(crate) fn hint_rect(p: &Panel, w: u16) -> Option<Rect> {
    let s = p.sel.filter(|s| !s.dragging)?;
    let (a, b) = s.range();
    let vis: Vec<(usize, usize)> = (p.top..p.top + p.body.height as usize).map(|k| (k, 0)).collect();
    let sel = crate::feedsel::FeedSel { anchor: (a, 0, CODE_X), head: (b, 0, CODE_X) };
    crate::quote::hint_rect(sel, &vis, p.body, w)
}

/// Where the code starts on a line (`  38   38 + `).
const CODE_X: usize = 12;

/// Draws the popup over the selection (last over the panel).
pub(crate) fn draw_hint(app: &App, frame: &mut ratatui::Frame) {
    let Some(p) = app.diff.as_ref() else { return };
    if p.list.is_some() || app.term.shown() {
        return;
    }
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let Some(line) = crate::quote::hint_line(p.body.width as usize, no_color, app.sb.focus_name()) else { return };
    let Some(r) = hint_rect(p, line.width() as u16).map(|r| r.intersection(frame.area())) else { return };
    crate::pointer::region(r, crate::pointer::Shape::Default);
    frame.render_widget(ratatui::widgets::Paragraph::new(line), r);
}

#[cfg(test)]
#[path = "diffquote_tests.rs"]
mod tests;
