//! The find bar (BISE-237, BISE-297; the user's QA of 2026-10-04): the
//! box of ctrl+f in the top-right corner of the history pane, flush
//! against its borders (under the frame's top edge, left of the panel's
//! rule or the frame's right edge), like Ghostty's search bar:
//! ` ⌕ query▏        3/17  ↑  ↓  × `.
//!
//! The field is the composer's editing model (`editor::Editor` and its
//! key map `editor::action`): one text field behaviour, shift
//! selections, word moves and deletes, line start/end, undo. The bar
//! only takes what is find's own: ⏎ / ↑ older, shift+⏎ / ↓ newer, esc
//! close, ctrl+f / cmd+f older. The chevrons and the × click; a press in
//! the field places the cursor, a drag selects, a double click selects
//! the word. The search itself is find.rs.

use crate::app::App;
use crate::editor::{self, Action, Motion};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::Frame;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The box's height: its border, the field, its border.
pub(crate) const BOX_H: u16 = 3;
/// Its width: 48 columns, the pane's width when narrower; none under 20.
const BOX_W: u16 = 48;
const BOX_MIN_W: u16 = 20;
/// Each button is 3 columns (` ↑ `): an easy target for the mouse.
const BUTTON_W: u16 = 3;
/// The field keeps at least this many columns; the counter goes first.
const FIELD_MIN: u16 = 8;

/// Where the last frame drew the box's parts (screen cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Parts {
    pub(crate) bx: Rect,
    pub(crate) field: Rect,
    pub(crate) count: Option<Rect>,
    pub(crate) prev: Rect,
    pub(crate) next: Rect,
    pub(crate) close: Rect,
}

/// The box in the history pane `pane` (from the row under the frame's
/// top edge down to the history's last row, its right edge the column
/// left of the panel's rule or of the frame's edge): its top-right
/// corner, flush. None when the pane has no room.
pub(crate) fn box_rect(pane: Rect) -> Option<Rect> {
    if pane.width < BOX_MIN_W || pane.height < BOX_H + 1 {
        return None;
    }
    let w = BOX_W.min(pane.width);
    Some(Rect { x: pane.right() - w, y: pane.y, width: w, height: BOX_H })
}

/// The parts of a box whose inside is `inner` (1 row), with a counter
/// `count_w` columns wide: ` ⌕ ` and the field, the counter right-aligned
/// against the buttons (dropped when the field would get less than
/// FIELD_MIN), the 3 buttons at the right end.
pub(crate) fn parts(bx: Rect, inner: Rect, count_w: u16) -> Parts {
    let btn_x = inner.right().saturating_sub(3 * BUTTON_W).max(inner.x);
    let b = |i: u16| Rect { x: btn_x + i * BUTTON_W, y: inner.y, width: BUTTON_W, height: 1 }.intersection(inner);
    let field_x = (inner.x + 3).min(btn_x);
    // 2 blank columns between the counter and the first chevron
    let with_count = count_w > 0 && btn_x >= field_x + FIELD_MIN + count_w + 2;
    let count = with_count.then(|| Rect { x: btn_x - 1 - count_w, y: inner.y, width: count_w, height: 1 });
    let field_end = count.map_or(btn_x, |c| c.x - 1);
    let field = Rect { x: field_x, y: inner.y, width: field_end.saturating_sub(field_x), height: 1 };
    Parts { bx, field, count, prev: b(0), next: b(1), close: b(2) }
}

/// The first char shown in a field `w` columns wide, so that the cursor
/// (and its cell) stays in view; from the last frame's `hscroll`.
pub(crate) fn scroll_to_cursor(text: &str, cursor: usize, hscroll: usize, w: usize) -> usize {
    let mut h = hscroll.min(cursor);
    let width = |a: usize, b: usize| -> usize { text.chars().skip(a).take(b - a).collect::<String>().width() };
    while h < cursor && width(h, cursor) + 1 > w.max(1) {
        h = editor::next_grapheme(text, h);
    }
    h
}

/// The field's cells from `hscroll`, `w` columns: the query in the text
/// color, the selection on the selection tint (NO_COLOR: underlined),
/// the cursor's cell reversed; empty, the cursor then the placeholder
/// (dim).
pub(crate) fn field_line(ed: &editor::Editor, hscroll: usize, w: usize, placeholder: &str) -> Line<'static> {
    let plain = crate::find::no_color();
    let txt = Style::default().fg(crate::theme::text());
    let cur = txt.add_modifier(Modifier::REVERSED);
    if ed.text.is_empty() {
        let p: String = format!(" {}", placeholder).chars().take(w.saturating_sub(1)).collect();
        return Line::from(vec![Span::styled(" ", cur), Span::styled(p, Style::default().fg(crate::theme::dim()))]);
    }
    let sel = ed.selection();
    let sel_st = if plain { txt.add_modifier(Modifier::UNDERLINED) } else { txt.bg(crate::theme::selection_bg()) };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let (mut ci, mut used) = (0usize, 0usize);
    for g in ed.text.graphemes(true) {
        let n = g.chars().count();
        if ci >= hscroll {
            let gw = g.width();
            if used + gw > w {
                break;
            }
            let st = if ci == ed.cursor {
                cur
            } else if sel.is_some_and(|(a, b)| ci >= a && ci < b) {
                sel_st
            } else {
                txt
            };
            match spans.last_mut() {
                Some(l) if l.style == st && st != cur => l.content.to_mut().push_str(g),
                _ => spans.push(Span::styled(g.to_string(), st)),
            }
            used += gw;
        }
        ci += n;
    }
    if ed.cursor >= ed.len() && used < w {
        spans.push(Span::styled(" ", cur));
    }
    Line::from(spans)
}

/// The char index under column `col` of a field showing from `hscroll`.
pub(crate) fn ci_at(text: &str, hscroll: usize, col: usize) -> usize {
    let (mut ci, mut x) = (0usize, 0usize);
    for g in text.graphemes(true) {
        let n = g.chars().count();
        if ci >= hscroll {
            let gw = g.width();
            if col < x + gw.div_ceil(2).max(1) {
                return ci;
            }
            x += gw;
        }
        ci += n;
    }
    ci
}

/// Where the box is drawn this frame: top-right of the pane; at the
/// history's very top (nothing left to scroll) the current match can sit
/// where the box goes: the box then goes to the bottom-right of the
/// history `feed`, the match stays seen.
fn box_at(app: &App, pane: Rect, feed: Rect) -> Option<Rect> {
    let r = box_rect(pane)?;
    let Some(l) = app.find.as_ref().and_then(|f| f.loc) else { return Some(r) };
    let y = (0..app.vis_events.len()).find(|&y| app.vis_events[y] == l.ev && app.vis_rows.get(y) == Some(&l.row));
    // the match's row under the box (its line is read whole, not only
    // the match)
    let under = y.is_some_and(|y| {
        let y = feed.y as usize + y;
        y >= r.y as usize && y < r.bottom() as usize
    });
    if under && feed.height > 2 * BOX_H + 1 {
        return Some(Rect { y: feed.bottom() - BOX_H, ..r });
    }
    Some(r)
}

/// Draw the box over the history: `pane` the history pane (box_rect),
/// `feed` the history's rows. A rounded dim border, the raised grey
/// inside (NO_COLOR: the border only).
pub(crate) fn draw(app: &mut App, frame: &mut Frame, pane: Rect, feed: Rect) {
    let Some(r) = box_at(app, pane, feed) else {
        if let Some(f) = app.find.as_mut() {
            f.bar = None;
        }
        return;
    };
    let r = r.intersection(frame.area());
    if r.height < BOX_H || r.width < BOX_MIN_W {
        return;
    }
    let plain = crate::find::no_color();
    let dim = Style::default().fg(crate::theme::dim());
    let mut block = Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded);
    block = if plain {
        block
    } else {
        block.border_style(dim.bg(crate::theme::raised())).style(Style::default().bg(crate::theme::raised()))
    };
    frame.render_widget(Clear, r);
    let inner = block.inner(r);
    frame.render_widget(block, r);
    let more = crate::find::more_before(app);
    let hover = app.pointer_at;
    let Some(f) = app.find.as_mut() else { return };
    let count = f.counter(more, Instant::now());
    let p = parts(r, inner, count.width() as u16);
    // the rows of the history the box covers in its corner (find keeps
    // its match under it); never the bottom place it moves to
    if let Some(corner) = box_rect(pane) {
        f.cover = corner.bottom().saturating_sub(feed.y) as usize;
    }
    f.bar = Some(p);
    let buf = frame.buffer_mut();
    buf.set_string(inner.x + 1, inner.y, crate::theme::glyph("⌕"), dim);
    f.hscroll = scroll_to_cursor(&f.ed.text, f.ed.cursor, f.hscroll, p.field.width as usize);
    let line = field_line(&f.ed, f.hscroll, p.field.width as usize, &format!("find in {}", f.focus_name()));
    buf.set_line(p.field.x, p.field.y, &line, p.field.width);
    if let Some(c) = p.count {
        let st = if count == "no match" { Style::default().fg(crate::theme::error()) } else { dim };
        buf.set_string(c.x, c.y, &count, st);
    }
    let any = f.has_matches();
    // ↑ older, ↓ newer: what the keys say (designer: not ⌃, the ctrl key)
    for (b, g, live) in [(p.prev, "↑", any), (p.next, "↓", any), (p.close, "×", true)] {
        let over = hover.is_some_and(|(x, y)| b.contains((x, y).into()));
        let st = if over && live { Style::default().fg(crate::theme::text()) } else { dim };
        let st = if live { st } else { st.add_modifier(Modifier::DIM) };
        buf.set_string(b.x + 1, b.y, crate::theme::glyph(g), st);
    }
    crate::pointer::region(p.field, crate::pointer::Shape::Text);
    for b in [p.prev, p.next, p.close] {
        crate::pointer::region(b, crate::pointer::Shape::Pointer);
    }
    // BISE-290: the counter selects and copies
    if let Some(c) = p.count {
        crate::textlayer::text(c);
    }
}

/// The keys of the find bar; `true` when handled. ctrl+f opens it; so
/// does cmd+f when the terminal passes it through (SUPER under the
/// kitty keyboard protocol, e.g. Ghostty `keybind = super+f=unbind`).
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let ctrl_f = k.code == KeyCode::Char('f') && (k.modifiers == KeyModifiers::CONTROL || k.modifiers == KeyModifiers::SUPER);
    let Some(f) = app.find.as_mut() else {
        if ctrl_f {
            crate::find::open(app);
            return true;
        }
        return false;
    };
    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) => {
            crate::find::close(app);
            return true;
        }
        (KeyCode::Enter, m) if m.contains(KeyModifiers::SHIFT) => f.newer(),
        (KeyCode::Enter, _) => f.older(),
        _ if ctrl_f => f.older(),
        // the feed still scrolls, ctrl+c still interrupts or quits
        (KeyCode::PageUp | KeyCode::PageDown, _) => return false,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return false,
        _ => edit_key(app, k),
    }
    true
}

/// A key for the field: the composer's key map, ↑/↓ go through the
/// matches (a one-row field has no row to move to).
fn edit_key(app: &mut App, k: &KeyEvent) {
    let Some(a) = editor::action(k) else { return };
    let Some(f) = app.find.as_mut() else { return };
    let before = f.ed.text.clone();
    match a {
        Action::Up(false) => f.older(),
        Action::Down(false) => f.newer(),
        Action::Up(true) => f.ed.move_cursor(Motion::TextStart, true),
        Action::Down(true) => f.ed.move_cursor(Motion::TextEnd, true),
        Action::Copy => {
            if let Some(t) = f.ed.selected_text() {
                crate::input::copy_text(app, &t);
            }
            return;
        }
        Action::Cut => {
            if let Some(t) = f.ed.cut() {
                crate::input::copy_text(app, &t);
            }
        }
        // one line: a typed new line is nothing here
        Action::Insert(t) if t.contains('\n') => {}
        other => {
            f.ed.apply(&other);
        }
    }
    if app.find.as_ref().is_some_and(|f| f.ed.text != before) {
        crate::find::edited(app);
    }
}

/// A paste while the bar is open goes to the field (one line).
pub(crate) fn on_paste(app: &mut App, text: &str) -> bool {
    let Some(f) = app.find.as_mut() else { return false };
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    f.ed.paste(&one);
    crate::find::edited(app);
    true
}

/// A mouse event on the bar; `true` when it was the bar's: the chevrons
/// go older / newer, the × closes, a press in the field places the
/// cursor (shift extends, a double click selects the word, a triple
/// click all), a drag from it selects. A press anywhere on the box is
/// the box's.
pub(crate) fn on_mouse(app: &mut App, m: &MouseEvent) -> bool {
    let Some(f) = app.find.as_mut() else { return false };
    let Some(p) = f.bar else { return false };
    let at = (m.column, m.row).into();
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) if p.bx.contains(at) => {
            if p.prev.contains(at) {
                f.older();
            } else if p.next.contains(at) {
                f.newer();
            } else if p.close.contains(at) {
                crate::find::close(app);
            } else if p.field.contains(at) {
                let ci = ci_at(&f.ed.text, f.hscroll, (m.column - p.field.x) as usize);
                let clicks = app.mouse.press(m.column, m.row, Instant::now());
                let Some(f) = app.find.as_mut() else { return true };
                match clicks {
                    2 => {
                        let (a, b) = editor::word_at(&f.ed.text, ci);
                        f.ed.select_range(a, b);
                    }
                    3 => f.ed.select_all(),
                    _ => f.ed.click(ci, m.modifiers.contains(KeyModifiers::SHIFT)),
                }
                f.dragging = true;
            }
            true
        }
        MouseEventKind::Drag(MouseButton::Left) if f.dragging => {
            let col = m.column.clamp(p.field.x, p.field.right().saturating_sub(1)) - p.field.x;
            // past the field's edges: the text scrolls that way
            let ci = if m.column < p.field.x {
                f.hscroll.saturating_sub(1)
            } else {
                ci_at(&f.ed.text, f.hscroll, col as usize + usize::from(m.column >= p.field.right()))
            };
            f.ed.click(ci, true);
            true
        }
        MouseEventKind::Up(MouseButton::Left) if f.dragging => {
            f.dragging = false;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "find_bar_tests.rs"]
mod tests;
