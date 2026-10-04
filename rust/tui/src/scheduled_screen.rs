//! `/scheduled` (site/m/timers): every scheduled task of every agent,
//! full screen like /artifacts. The active ones, soonest first; tab shows
//! the ended ones too (the last 7 days, dim). `/` finds by agent or
//! words. ⏎ opens one: who it wakes, who set it, how often, the next run,
//! so far, when it ends, the words it sends, its runs. `r` runs it now
//! (one run outside its count, the next run unchanged), `x` stops it
//! after a question on the row (`y` stops, `n` or esc keeps). The same
//! at 80 and 150 columns: columns drop as it narrows.

use crate::app::App;
use crate::scheduled::{ahead, clip, countdown, Task};
use crate::theme::{self, accent, dim, faint, text};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// From this many columns of text the rows have every column.
const WIDE_FROM: usize = 110;

/// The open screen.
#[derive(Default)]
pub(crate) struct Screen {
    /// the search's text
    pub(crate) query: String,
    /// the search has the keys (after `/`)
    pub(crate) typing: bool,
    /// tab: the ended ones too
    pub(crate) ended_too: bool,
    /// the selected one, by id
    pub(crate) sel: Option<u64>,
    /// one opened (⏎)
    pub(crate) opened: Option<u64>,
    /// `x` asks on the row: stop this one?
    pub(crate) confirm: Option<u64>,
    /// what the last key did (`ran now: …`), for 6 s
    pub(crate) note: Option<(String, Instant)>,
    /// the first list row shown
    pub(crate) top: usize,
    /// what the last frame drew where: screen row → task id
    pub(crate) hits: Vec<(u16, u64)>,
}

/// `/scheduled` opens the screen on the next one to run.
pub(crate) fn open(app: &mut App) {
    let sel = shown(&Screen::default(), &app.sb.timers).first().map(|t| t.id);
    app.scheduled = Some(Screen { sel, ..Default::default() });
}

// ---- the list (pure) ----

/// The tasks the screen lists: the active ones soonest first, then (tab)
/// the ended ones newest first; the search keeps those whose agent, who
/// set it or words have it.
pub(crate) fn shown(sc: &Screen, all: &[Task]) -> Vec<Task> {
    let q = sc.query.trim().to_lowercase();
    let hit = |t: &Task| {
        q.is_empty()
            || t.agent.to_lowercase().contains(&q)
            || t.by.to_lowercase().contains(&q)
            || t.text.to_lowercase().contains(&q)
            || format!("#{}", t.id) == q
    };
    let mut active: Vec<Task> = all.iter().filter(|t| t.active() && hit(t)).cloned().collect();
    active.sort_by_key(|t| (t.next_ms, t.id));
    if sc.ended_too {
        let mut ended: Vec<Task> = all.iter().filter(|t| !t.active() && hit(t)).cloned().collect();
        ended.sort_by_key(|t| std::cmp::Reverse(t.ended_ms));
        active.extend(ended);
    }
    active
}

fn pad(s: &str, w: usize) -> String {
    let s = cut(s, w);
    let n = w.saturating_sub(s.width());
    format!("{s}{}", " ".repeat(n))
}

fn cut(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + 1 >= w {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// One task's row: `› #48  ◷ answer-line   every 2m   next 14:32 · in 1m
/// 2 of 6   by answer-line   check the build…` (wide), `› #48 ◷
/// answer-line  2m  in 1m  2 of 6` (narrow); an ended one is dim and
/// says when and why.
fn row(t: &Task, selected: bool, width: usize, now: u64) -> Line<'static> {
    let wide = width >= WIDE_FROM;
    let base = if t.active() { text() } else { dim() };
    let st = Style::default().fg(base);
    let soft = Style::default().fg(if t.active() { dim() } else { faint() });
    let mut spans = vec![
        Span::styled(if selected { format!("{} ", theme::glyph("›")) } else { "  ".into() }, Style::default().fg(accent())),
        Span::styled(format!("#{:<4}", t.id), soft),
        Span::styled(format!("{} ", theme::glyph(theme::G_SCHEDULED)), Style::default().fg(faint())),
    ];
    let (agent_w, when_w, next_w, far_w) = if wide { (14, 18, 26, 17) } else { (13, 13, 12, 12) };
    spans.push(Span::styled(pad(&t.agent, agent_w), st));
    spans.push(Span::styled(pad(&if wide { t.when() } else { t.when_short() }, when_w), soft));
    let (next, far) = match t.ended_ms {
        Some(e) => (format!("ended {}", ahead(e, now)), t.ended_words()),
        None if wide => {
            let mut n = format!("next {}", ahead(t.next_ms, now));
            if t.next_ms.saturating_sub(now) < 24 * 3_600_000 && !n.contains("tomorrow") {
                n.push_str(&format!(" · {}", countdown(t.next_ms, now)));
            }
            (n, t.so_far(now))
        }
        None => {
            let n = if t.next_ms.saturating_sub(now) < 24 * 3_600_000 && !ahead(t.next_ms, now).contains("tomorrow") {
                countdown(t.next_ms, now)
            } else {
                ahead(t.next_ms, now).replace("tomorrow", "tmrw")
            };
            (n, t.so_far(now))
        }
    };
    spans.push(Span::styled(pad(&next, next_w), soft));
    spans.push(Span::styled(pad(&far, far_w), soft));
    if wide {
        spans.push(Span::styled(pad(&format!("by {}", t.by), 17), soft));
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let room = width.saturating_sub(used + 1);
        match &t.page {
            Some(p) => spans.push(Span::styled(format!("↗ {}", cut(p, room.saturating_sub(2))), Style::default().fg(accent()))),
            None => spans.push(Span::styled(cut(&clip(&t.text, 200), room), st)),
        }
    }
    Line::from(spans)
}

/// The row of `x`: the question in place of the task.
fn confirm_row(t: &Task) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{} ", theme::glyph("›")), Style::default().fg(accent())),
        Span::styled(format!("#{:<4}", t.id), Style::default().fg(dim())),
        Span::styled(format!("{} ", theme::glyph(theme::G_SCHEDULED)), Style::default().fg(faint())),
        Span::styled(format!("stop this scheduled task? it won't wake {} again.", t.agent), Style::default().fg(text())),
        Span::raw("   "),
        Span::styled("y", Style::default().fg(text())),
        Span::styled(" stop   ", Style::default().fg(dim())),
        Span::styled("n", Style::default().fg(text())),
        Span::styled(" or ", Style::default().fg(dim())),
        Span::styled("esc", Style::default().fg(text())),
        Span::styled(" keep", Style::default().fg(dim())),
    ])
}

fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (k, w)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(k.to_string(), Style::default().fg(text())));
        spans.push(Span::styled(format!(" {w}"), Style::default().fg(dim())));
    }
    Line::from(spans)
}

/// The key bar for the state the screen is in.
pub(crate) fn key_bar(sc: &Screen, sel: Option<&Task>, wide: bool) -> Line<'static> {
    if sc.confirm.is_some() {
        return keys(&[("y", "stop"), ("n or esc", "keep")]);
    }
    if sc.typing {
        return keys(&[("⏎", "done"), ("↑↓", "choose"), ("esc", "clear the search")]);
    }
    let active = sel.is_some_and(Task::active);
    if sc.opened.is_some() {
        return if active { keys(&[("r", "run now"), ("x", "stop"), ("esc", "back to the list")]) } else { keys(&[("esc", "back to the list")]) };
    }
    let esc = if sc.query.is_empty() { "close" } else { "clear the search" };
    let tab = if sc.ended_too { "running only" } else { "ended too" };
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if sel.is_some() {
        pairs.push(("⏎", "open"));
    }
    if active {
        pairs.extend([("r", "run now"), ("x", "stop")]);
    }
    if wide {
        pairs.extend([("/", "find"), ("tab", tab)]);
    }
    pairs.push(("esc", esc));
    keys(&pairs)
}

/// The line over the key bar: what the selected one does, or what the
/// last key did; cut to the width.
fn detail_line(sc: &Screen, sel: Option<&Task>, width: usize) -> Line<'static> {
    let dim_st = Style::default().fg(dim());
    if let Some((note, at)) = &sc.note {
        if at.elapsed() < Duration::from_secs(6) {
            return Line::from(Span::styled(cut(note, width), dim_st));
        }
    }
    let Some(t) = sel else { return Line::default() };
    let mut s = format!("#{} · wakes {} {} · set by {}", t.id, t.agent, t.when(), t.by);
    s.push_str(&format!(" · its words: {}", clip(&t.text, 200)));
    Line::from(Span::styled(cut(&s, width), dim_st))
}

/// The list view's lines (header, search, rows, then the detail and the
/// key bar at the bottom) and the rows' task ids by line index.
pub(crate) fn lines(sc: &mut Screen, all: &[Task], width: usize, height: usize, now: u64) -> (Vec<Line<'static>>, Vec<(usize, u64)>) {
    let wide = width >= WIDE_FROM;
    let list = shown(sc, all);
    if sc.sel.is_none_or(|id| !list.iter().any(|t| t.id == id)) {
        sc.sel = list.first().map(|t| t.id);
    }
    let sel = list.iter().find(|t| Some(t.id) == sc.sel).cloned();
    let n_active = all.iter().filter(|t| t.active()).count();
    let n_ended = all.iter().filter(|t| !t.active()).count();
    let mut out: Vec<Line<'static>> = Vec::new();
    // the header: what it is, how many, the tab
    let left = if wide { "scheduled · what wakes your agents, and when" } else { "scheduled" };
    let count = if sc.ended_too { format!("{n_active} active · {n_ended} ended") } else { format!("{n_active} active") };
    let right = if wide { format!("{count}   tab {}", if sc.ended_too { "active only" } else { "ended too" }) } else { count };
    let gap = width.saturating_sub(left.width() + right.width() + 2).max(2);
    out.push(Line::from(vec![
        Span::styled(left.to_string(), Style::default().fg(text())),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(dim())),
    ]));
    // the search
    out.push(if sc.query.is_empty() && !sc.typing {
        Line::from(Span::styled("/ find: an agent, the words", Style::default().fg(faint())))
    } else {
        Line::from(vec![
            Span::styled("/ ", Style::default().fg(dim())),
            Span::styled(sc.query.clone(), Style::default().fg(text())),
            Span::styled(if sc.typing { "▏" } else { "" }, Style::default().fg(accent())),
        ])
    });
    out.push(Line::default());
    let body = height.saturating_sub(out.len() + 3);
    let mut rows: Vec<(Line<'static>, Option<u64>)> = Vec::new();
    if list.is_empty() {
        let words = if !sc.query.is_empty() {
            "nothing scheduled matches."
        } else if n_active == 0 {
            "nothing scheduled. agents set them with sb every; tell one \"check the build every 5m\"."
        } else {
            ""
        };
        rows.push((Line::from(Span::styled(format!("  {words}"), Style::default().fg(dim()))), None));
    }
    let mut ended_head = false;
    for t in &list {
        if !t.active() && !ended_head {
            ended_head = true;
            rows.push((Line::default(), None));
            rows.push((Line::from(Span::styled("  ended", Style::default().fg(dim()))), None));
        }
        let selected = Some(t.id) == sc.sel;
        let line = if selected && sc.confirm == Some(t.id) { confirm_row(t) } else { row(t, selected, width, now) };
        rows.push((line, Some(t.id)));
    }
    // keep the selected row in view
    if let Some(i) = rows.iter().position(|(_, id)| id.is_some() && *id == sc.sel) {
        if i < sc.top {
            sc.top = i;
        } else if body > 0 && i >= sc.top + body {
            sc.top = i + 1 - body;
        }
    }
    sc.top = sc.top.min(rows.len().saturating_sub(1));
    let mut hits = Vec::new();
    for (line, id) in rows.into_iter().skip(sc.top).take(body) {
        if let Some(id) = id {
            hits.push((out.len(), id));
        }
        out.push(line);
    }
    while out.len() + 2 < height {
        out.push(Line::default());
    }
    out.push(detail_line(sc, sel.as_ref(), width));
    out.push(key_bar(sc, sel.as_ref(), wide));
    (out, hits)
}

/// One task opened: its fields, the words it sends, its runs.
pub(crate) fn opened_lines(sc: &Screen, t: &Task, width: usize, height: usize, now: u64) -> Vec<Line<'static>> {
    let label = |k: &str, v: String| {
        Line::from(vec![Span::styled(format!("{k:<12}"), Style::default().fg(dim())), Span::styled(v, Style::default().fg(text()))])
    };
    let mut out = Vec::new();
    let left = format!("scheduled task #{} · {}", t.id, t.agent);
    let right = format!("{} {}", theme::glyph(theme::G_SCHEDULED), if t.active() { "active" } else { "ended" });
    let gap = width.saturating_sub(left.width() + right.width() + 2).max(2);
    out.push(Line::from(vec![
        Span::styled(left, Style::default().fg(text())),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(if t.active() { accent() } else { dim() })),
    ]));
    out.push(Line::default());
    out.push(label("wakes", t.agent.clone()));
    out.push(label("set by", t.by.clone()));
    let mut when = t.when();
    if let Some(n) = t.times.filter(|n| *n > 1) {
        when.push_str(&format!(" · {n} times"));
    }
    out.push(label("when", when));
    match t.ended_ms {
        Some(e) => out.push(label("ended", format!("{} · {}", ahead(e, now), t.ended_words()))),
        None => out.push(label("next run", format!("{} · {}", ahead(t.next_ms, now), countdown(t.next_ms, now)))),
    }
    let mut far = t.so_far(now);
    if far.starts_with("until") || far.is_empty() {
        far = format!("{} run{}", t.fired, if t.fired == 1 { "" } else { "s" });
    }
    if t.last_ms > 0 {
        far.push_str(&format!(" · last {}", ahead(t.last_ms, now)));
    }
    out.push(label("so far", far));
    let ends = match (t.times, t.until_ms) {
        (Some(1), _) => "after its run".to_string(),
        (Some(n), _) => format!("after its {}{} run", n, ordinal(n)),
        (None, Some(u)) => format!("at {}", ahead(u, now)),
        _ => "when it's stopped".to_string(),
    };
    if t.active() {
        out.push(label("ends", ends));
    }
    if let Some(p) = &t.page {
        out.push(label("keeps fresh", format!("↗ {p}")));
    }
    out.push(Line::default());
    out.push(Line::from(Span::styled("the words it sends", Style::default().fg(dim()))));
    let bar = Span::styled("│ ", Style::default().fg(theme::rule()));
    for l in wrap(&t.text, width.saturating_sub(4).max(10)) {
        out.push(Line::from(vec![bar.clone(), Span::styled(l, Style::default().fg(text()))]));
    }
    out.push(Line::default());
    out.push(Line::from(Span::styled("its runs", Style::default().fg(dim()))));
    if t.runs.is_empty() {
        out.push(Line::from(Span::styled("  none yet", Style::default().fg(faint()))));
    }
    for r in t.runs.iter().rev() {
        out.push(Line::from(Span::styled(format!("  {}", crate::when::ended_now(*r)), Style::default().fg(dim()))));
    }
    while out.len() + 2 < height {
        out.push(Line::default());
    }
    out.truncate(height.saturating_sub(2));
    out.push(detail_line(&Screen { note: sc.note.clone(), ..Default::default() }, None, width));
    out.push(key_bar(sc, Some(t), width >= WIDE_FROM));
    out
}

/// `s` in rows of at most `w` columns, cut at spaces (a longer word
/// gets its own row), its own line breaks kept.
fn wrap(s: &str, w: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.lines() {
        let mut row = String::new();
        for word in para.split_whitespace() {
            if !row.is_empty() && row.width() + 1 + word.width() > w {
                out.push(std::mem::take(&mut row));
            }
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(word);
        }
        out.push(row);
    }
    out
}

fn ordinal(n: u64) -> &'static str {
    match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    }
}

// ---- drawing ----

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    if app.scheduled.is_none() {
        return;
    }
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 10 {
        return;
    }
    let area = crate::artifacts_screen::draw_frame(app, frame, full, "scheduled");
    let all = app.sb.timers.clone();
    let now = crate::when::now_ms();
    let Some(sc) = app.scheduled.as_mut() else { return };
    let (w, h) = (area.width as usize, area.height as usize);
    let opened = sc.opened.and_then(|id| all.iter().find(|t| t.id == id).cloned());
    let rows = match opened {
        Some(t) => {
            sc.hits.clear();
            opened_lines(sc, &t, w, h, now)
        }
        None => {
            sc.opened = None;
            let (rows, hits) = lines(sc, &all, w, h, now);
            sc.hits = hits.into_iter().map(|(i, id)| (area.y + i as u16, id)).collect();
            rows
        }
    };
    frame.render_widget(Paragraph::new(rows), area);
    crate::textlayer::text(area);
    for (y, _) in &sc.hits {
        crate::pointer::region(ratatui::layout::Rect { x: area.x, y: *y, width: area.width, height: 1 }, crate::pointer::Shape::Pointer);
    }
}

// ---- keys and the mouse ----

fn step(sc: &mut Screen, all: &[Task], by: isize) {
    let list = shown(sc, all);
    if list.is_empty() {
        return;
    }
    let i = sc.sel.and_then(|id| list.iter().position(|t| t.id == id));
    let j = match i {
        Some(i) => (i as isize + by).clamp(0, list.len() as isize - 1) as usize,
        None => 0,
    };
    sc.sel = Some(list[j].id);
}

fn selected(app: &App) -> Option<Task> {
    let sc = app.scheduled.as_ref()?;
    let id = sc.opened.or(sc.sel)?;
    app.sb.timers.iter().find(|t| t.id == id).cloned()
}

fn note(app: &mut App, words: String) {
    if let Some(sc) = app.scheduled.as_mut() {
        sc.note = Some((words, Instant::now()));
    }
}

/// `r`: one run at once, outside its count.
fn run_now(app: &mut App) {
    let Some(t) = selected(app).filter(Task::active) else { return };
    app.sb.send(serde_json::json!({"op": "every_run", "id": t.id}));
    let now = crate::when::now_ms();
    note(app, format!("#{} · ran now: {} is on it. the next run stays at {}.", t.id, t.agent, ahead(t.next_ms, now)));
}

/// `y` after `x`: the hub stops it (its agent hears it from bise).
fn stop(app: &mut App, id: u64) {
    let agent = app.sb.timers.iter().find(|t| t.id == id).map(|t| t.agent.clone()).unwrap_or_default();
    app.sb.send(serde_json::json!({"op": "every_stop", "id": id}));
    if let Some(sc) = app.scheduled.as_mut() {
        sc.confirm = None;
        sc.opened = None;
    }
    note(app, format!("#{id} · stopped: it won't wake {agent} again."));
}

pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    if app.scheduled.is_none() {
        return false;
    }
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let all = app.sb.timers.clone();
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let Some(sc) = app.scheduled.as_mut() else { return false };
    if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('g')) {
        app.scheduled = None;
        return true;
    }
    // `x` asked: y stops, n or esc keeps
    if let Some(id) = sc.confirm {
        match k.code {
            KeyCode::Char('y') => stop(app, id),
            KeyCode::Char('n') | KeyCode::Esc => sc.confirm = None,
            _ => {}
        }
        return true;
    }
    if sc.typing {
        match k.code {
            KeyCode::Esc => {
                sc.query.clear();
                sc.typing = false;
            }
            KeyCode::Enter => sc.typing = false,
            KeyCode::Up => {
                sc.typing = false;
                step(sc, &all, -1);
            }
            KeyCode::Down => {
                sc.typing = false;
                step(sc, &all, 1);
            }
            KeyCode::Backspace => {
                sc.query.pop();
            }
            KeyCode::Char(c) if !ctrl => sc.query.push(c),
            _ => {}
        }
        return true;
    }
    let active = selected(app).is_some_and(|t| t.active());
    let Some(sc) = app.scheduled.as_mut() else { return false };
    match k.code {
        KeyCode::Esc if sc.opened.is_some() => sc.opened = None,
        KeyCode::Esc if !sc.query.is_empty() => sc.query.clear(),
        KeyCode::Esc => app.scheduled = None,
        KeyCode::Char('r') if active => run_now(app),
        KeyCode::Char('x') if active => sc.confirm = sc.opened.or(sc.sel),
        _ if sc.opened.is_some() => {}
        KeyCode::Up | KeyCode::Char('k') if !ctrl => step(sc, &all, -1),
        KeyCode::Down | KeyCode::Char('j') if !ctrl => step(sc, &all, 1),
        KeyCode::PageUp => step(sc, &all, -10),
        KeyCode::PageDown => step(sc, &all, 10),
        KeyCode::Home => step(sc, &all, -100_000),
        KeyCode::End => step(sc, &all, 100_000),
        KeyCode::Enter => sc.opened = sc.sel,
        KeyCode::Char('/') => sc.typing = true,
        KeyCode::Tab | KeyCode::BackTab => {
            sc.ended_too = !sc.ended_too;
            sc.top = 0;
        }
        _ => {}
    }
    true
}

/// The mouse while open: the wheel moves the selection, a click on a row
/// opens it.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let all = app.sb.timers.clone();
    let Some(sc) = app.scheduled.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => step(sc, &all, -3),
        MouseEventKind::ScrollDown => step(sc, &all, 3),
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(id) = sc.hits.iter().find(|(y, _)| *y == m.row).map(|(_, id)| *id) {
                sc.sel = Some(id);
                sc.confirm = None;
                sc.opened = Some(id);
            }
        }
        _ => {}
    }
    true
}

#[cfg(test)]
#[path = "scheduled_screen_tests.rs"]
mod tests;
