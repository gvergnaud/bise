//! `/artifacts` (site/m/artifacts, B): what your agents made, full
//! screen, like /log. One list, newest first, grouped by day, the same
//! at 80 and 150 columns (columns drop as it narrows); the line under
//! the list says what the selected one is and where it lives.
//!
//! Keys (designer, m_7193): the list holds them when it opens: ↑↓, ⏎
//! open, space Quick Look, v versions, r show in Finder, c copy, @ put
//! it in a message, tab the scope, esc close. `/` focuses the search:
//! every letter, digit and space types, the list narrows live; ⏎ or ↑↓
//! give the keys back to the list (the search keeps its text). esc
//! clears the search, a second esc closes; `/` again edits it.

use crate::app::App;
use crate::artifacts::{self, Artifact, Group};
use crate::theme::{self, accent, dim, faint, text};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// The url of the header's `↗ 3 new` (textlayer.rs opens the screen).
pub(crate) const OPEN_URL: &str = "bise-artifacts:open";

/// From this many columns of text the screen has its wide columns
/// (agent before age, the last column's words, the long key bar).
const WIDE_FROM: usize = 110;

/// The open screen.
#[derive(Default)]
pub(crate) struct Screen {
    /// the search's text
    pub(crate) query: String,
    /// the search has the keys (after `/`)
    pub(crate) typing: bool,
    /// the scope: the agent in view only (tab)
    pub(crate) this_agent: bool,
    /// the agent in view when it opened
    pub(crate) agent: String,
    /// the selected artifact, by id (stays on it when the list changes)
    pub(crate) sel: Option<String>,
    /// the first body row shown
    pub(crate) top: usize,
    /// the versions under the selected row, open: the version selected
    /// (an index, newest first)
    pub(crate) versions: Option<usize>,
    /// Quick Look is up (macOS `qlmanage -p`)
    pub(crate) look: Option<Look>,
    /// `c` copied, for 2 s
    pub(crate) copied: Option<Instant>,
    /// what the last frame drew where (clicks): body row → target
    pub(crate) hits: Vec<(u16, Hit)>,
    /// the screen row of the search (a click focuses it)
    pub(crate) search_y: Option<u16>,
    /// the screen column where the rows start
    pub(crate) x0: u16,
}

/// What a row of the last frame was.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hit {
    /// an artifact's row; the columns of its `v3` (a click opens the
    /// versions)
    Row(String, Option<(u16, u16)>),
    /// a version's row in the versions box
    Version(usize),
}

/// Quick Look's process and the file it shows.
pub(crate) struct Look {
    child: Option<std::process::Child>,
}

impl Drop for Look {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// `/artifacts` opens the screen (`add <path or link>` adds one).
pub(crate) fn open(app: &mut App) {
    let agent = app.sb.focus_name().to_string();
    let sel = artifacts::all().first().map(|a| a.id.clone());
    app.artifacts = Some(Screen { agent, sel, ..Default::default() });
    // you looked: the header's `↗ N new` goes
    artifacts::mark_seen();
    app.sb.send(serde_json::json!({"op": "artifacts", "do": "seen"}));
}

// ---- the list (pure) ----

/// The artifacts the screen shows: the scope, then the search; with the
/// title's letters that match.
pub(crate) fn shown(sc: &Screen, all: &[Artifact]) -> Vec<(Artifact, Vec<usize>)> {
    all.iter()
        .filter(|a| !sc.this_agent || a.agent == sc.agent || (a.agent.is_empty() && a.by == sc.agent))
        .filter_map(|a| artifacts::find(a, &sc.query).map(|h| (a.clone(), h)))
        .collect()
}

/// The time of the clock: now, and the UTC offset at a time (tests fix it).
pub(crate) struct Clock<'a> {
    pub(crate) now: u64,
    pub(crate) off: &'a dyn Fn(u64) -> i32,
}

impl Clock<'_> {
    fn group(&self, ms: u64) -> Group {
        artifacts::group(ms, (self.off)(ms), self.now, (self.off)(self.now))
    }
    fn age(&self, ms: u64, short: bool) -> String {
        artifacts::age(ms, (self.off)(ms), self.now, (self.off)(self.now), short)
    }
    fn ago(&self, ms: u64) -> String {
        artifacts::ago_at(ms, (self.off)(ms), self.now, (self.off)(self.now))
    }
}

/// A body row.
enum Body {
    Group(Group),
    Item(usize),
    /// the versions box: its top edge, a version (index newest first),
    /// its key row (wide only), its bottom edge
    BoxTop,
    Version(usize),
    BoxKeys,
    BoxBottom,
}

fn pad(s: &str, w: usize) -> String {
    let s = cut(s, w);
    let n = w.saturating_sub(s.width());
    format!("{}{}", s, " ".repeat(n))
}

/// `s` in `w` columns at most, `…` at the cut.
pub(crate) fn cut(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) + 1 > w {
            break;
        }
        out.push(c);
    }
    out.push_str(theme::ellipsis());
    out
}

/// The columns of a row: (title, kind, agent, age) widths; wide puts the
/// agent before the age and has the last column's words.
struct Cols {
    wide: bool,
    title: usize,
    kind: usize,
    agent: usize,
    age: usize,
}

fn cols(width: usize) -> Cols {
    if width >= WIDE_FROM {
        Cols { wide: true, title: 34, kind: 10, agent: 24, age: 11 }
    } else {
        // 80 columns: 74 of text; the title takes what the rest leaves
        let fixed = 4 + 9 + 7 + 16 + 3;
        Cols { wide: false, title: width.saturating_sub(fixed).clamp(12, 34), kind: 9, agent: 16, age: 7 }
    }
}

/// The title with the search's letters in the accent.
fn title_spans(title: &str, hits: &[usize], w: usize, st: Style) -> Vec<Span<'static>> {
    let shown = pad(title, w);
    if hits.is_empty() {
        return vec![Span::styled(shown, st)];
    }
    let mut out: Vec<Span<'static>> = Vec::new();
    let hit_st = st.fg(accent()).add_modifier(Modifier::BOLD);
    let n = cut(title, w).chars().count();
    for (i, c) in shown.chars().enumerate() {
        let s = if i < n && hits.contains(&i) { hit_st } else { st };
        match out.last_mut() {
            Some(last) if last.style == s => last.content.to_mut().push(c),
            _ => out.push(Span::styled(c.to_string(), s)),
        }
    }
    out
}

/// One artifact's row, its `v3`'s columns (from the row's start).
fn item_line(a: &Artifact, hits: &[usize], selected: bool, c: &Cols, clock: &Clock, width: usize) -> (Line<'static>, Option<(u16, u16)>) {
    let st = if selected { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) };
    let d = Style::default().fg(dim());
    let mark = if selected { Span::styled(format!("{} ", theme::glyph(theme::G_YOU)), Style::default().fg(accent())) } else { Span::raw("  ") };
    let mut spans = vec![mark];
    spans.extend(title_spans(&a.title, hits, c.title, st));
    spans.push(Span::styled(pad(&a.kind_word(), c.kind), d));
    let age = Span::styled(pad(&clock.age(a.ts_ms, !c.wide), c.age), d);
    let agent = Span::styled(pad(&a.agent_words(), c.agent), d);
    if c.wide {
        spans.push(agent);
        spans.push(age);
    } else {
        spans.push(age);
        spans.push(agent);
    }
    let x = spans.iter().map(|s| s.content.width()).sum::<usize>();
    let last = if c.wide {
        a.last_words()
    } else if a.gone {
        theme::glyph(theme::G_INTERRUPTED).to_string()
    } else if a.versioned() {
        format!("v{}", a.v)
    } else {
        String::new()
    };
    let room = width.saturating_sub(x);
    let last = cut(&last, room);
    let mut vhit = None;
    if !last.is_empty() {
        let gone = a.gone;
        let lst = if gone { Style::default().fg(theme::error()) } else { d };
        if a.versioned() && !gone {
            let vw = format!("v{}", a.v).width();
            vhit = Some((x as u16, (x + vw) as u16));
        }
        spans.push(Span::styled(last, lst));
    }
    (Line::from(spans), vhit)
}

/// The versions box's rows under the selected row.
fn box_line(a: &Artifact, row: &Body, vsel: usize, clock: &Clock, wide: bool) -> Line<'static> {
    let w: usize = if wide { 58 } else { 44 };
    let edge = Style::default().fg(faint());
    let lead = Span::raw("    ");
    let versions: Vec<&artifacts::Version> = a.versions.iter().rev().collect();
    match row {
        Body::BoxTop => {
            let head = format!("{} · {} versions", a.title, a.versions.len().max(a.v as usize));
            let head = cut(&head, w.saturating_sub(4));
            let fill = w.saturating_sub(head.width() + 3);
            Line::from(vec![
                lead,
                Span::styled("╭─ ", edge),
                Span::styled(head, Style::default().fg(text())),
                Span::styled(format!(" {}╮", "─".repeat(fill.saturating_sub(1))), edge),
            ])
        }
        Body::BoxBottom => Line::from(vec![lead, Span::styled(format!("╰{}╯", "─".repeat(w)), edge)]),
        Body::BoxKeys => {
            let k = Style::default().fg(text());
            let words = vec![
                Span::styled("⏎", k),
                Span::styled(" open this one   ", Style::default().fg(dim())),
                Span::styled("esc", k),
                Span::styled(" back", Style::default().fg(dim())),
            ];
            let used: usize = words.iter().map(|s| s.content.width()).sum();
            let mut spans = vec![lead, Span::styled("│ ", edge)];
            spans.extend(words);
            spans.push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
            spans.push(Span::styled("│", edge));
            Line::from(spans)
        }
        Body::Version(i) => {
            let Some(v) = versions.get(*i) else { return Line::from("") };
            let sel = *i == vsel;
            let mark = if sel { Span::styled(format!("{} ", theme::glyph(theme::G_YOU)), Style::default().fg(accent())) } else { Span::raw("  ") };
            let st = if sel { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) };
            let mut body = vec![
                mark,
                Span::styled(pad(&format!("v{}", v.v), 5), st),
                Span::styled(pad(&clock.ago(v.ts_ms), 13), Style::default().fg(dim())),
                Span::styled(pad(&v.note, 15), Style::default().fg(dim())),
            ];
            if wide && v.v == a.v {
                body.push(Span::styled("the current one", Style::default().fg(faint())));
            }
            let used: usize = body.iter().map(|s| s.content.width()).sum();
            let mut spans = vec![lead, Span::styled("│ ", edge)];
            spans.extend(body);
            spans.push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
            spans.push(Span::styled("│", edge));
            Line::from(spans)
        }
        _ => Line::from(""),
    }
}

/// One key and its words, for the key bar.
fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (k, w)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(k.to_string(), Style::default().fg(text())));
        spans.push(Span::styled(format!(" {}", w), Style::default().fg(dim())));
    }
    Line::from(spans)
}

/// The key bar for the state the screen is in.
pub(crate) fn key_bar(sc: &Screen, sel: Option<&Artifact>, empty_list: bool, wide: bool) -> Line<'static> {
    if sc.typing {
        return keys(&[("⏎", "done"), ("↑↓", "choose"), ("esc", "clear the search")]);
    }
    if sc.versions.is_some() {
        return keys(&[("⏎", "open this version"), ("↑↓", "choose"), ("esc", "back to the list")]);
    }
    let esc = if sc.query.is_empty() { "close" } else { "clear the search" };
    if empty_list || sel.is_none() {
        return keys(&[("esc", esc)]);
    }
    if sc.look.is_some() {
        return keys(&[("space", "close the preview"), ("↑↓", "the next one, previewed"), ("⏎", "open"), ("esc", "close")]);
    }
    let a = sel.expect("checked above");
    let copied = sc.copied.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    let c = if copied { "copied" } else { "copy" };
    if !wide {
        return keys(&[("⏎", "open"), ("space", "look"), ("v", "versions"), ("esc", esc)]);
    }
    let mut pairs: Vec<(&str, &str)> = vec![("⏎", "open"), ("space", "quick look"), ("v", "versions")];
    if a.pr.is_some() {
        pairs.push(("o", "GitHub"));
    } else if !a.is_link() {
        pairs.push(("r", "show in Finder"));
    }
    pairs.extend([("c", c), ("@", "put it in a message"), ("esc", esc)]);
    keys(&pairs)
}

/// The line under the list: what the selected one is and where it lives.
fn detail_line(a: &Artifact, clock: &Clock, wide: bool) -> Line<'static> {
    let mut parts = vec![a.title.clone(), a.kind_word()];
    if a.versioned() {
        parts.push(format!("v{}", a.v));
    }
    let who = if a.agent.is_empty() { a.by.clone() } else { a.agent.clone() };
    let when = clock.ago(a.ts_ms);
    if wide {
        let who = if a.archived { format!("{} (archived)", who) } else { who };
        parts.push(format!("by {}, {}", who, when));
        if a.gone && a.copy.is_some() {
            parts.push(if a.archived { "its worktree is gone: ⏎ opens the copy bise kept".to_string() } else { "its file is gone: ⏎ opens the copy bise kept".to_string() });
        } else if a.gone {
            parts.push(artifacts::gone_words(false));
        } else {
            parts.push(a.where_words(&artifacts::workspace()));
        }
    } else {
        parts.push(format!("{}, {}", who, when));
    }
    Line::from(Span::styled(parts.join(" · "), Style::default().fg(dim())))
}

/// The screen's lines in `width` × `height` (inside the frame's padding)
/// and the body rows' targets (by line index).
pub(crate) fn lines(sc: &mut Screen, all: &[Artifact], width: usize, height: usize, clock: &Clock) -> (Vec<Line<'static>>, Vec<(usize, Hit)>, usize) {
    let c = cols(width);
    let list = shown(sc, all);
    if sc.sel.as_ref().is_none_or(|id| !list.iter().any(|(a, _)| &a.id == id)) {
        sc.sel = list.first().map(|(a, _)| a.id.clone());
        sc.versions = None;
    }
    let sel_i = sc.sel.as_ref().and_then(|id| list.iter().position(|(a, _)| &a.id == id));
    let sel = sel_i.map(|i| &list[i].0);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut hits: Vec<(usize, Hit)> = Vec::new();
    // the head: blank, title and scope, search, blank
    out.push(Line::from(""));
    let total = all.len();
    let in_scope = shown(&Screen { this_agent: sc.this_agent, agent: sc.agent.clone(), ..Default::default() }, all).len();
    let scope: Vec<Span<'static>> = if sc.this_agent {
        let mut v = vec![Span::styled(format!("{} · {}", sc.agent, in_scope), Style::default().fg(text()))];
        if c.wide {
            v.push(Span::raw("   "));
            v.push(Span::styled("tab", Style::default().fg(text())));
            v.push(Span::styled(" all agents", Style::default().fg(dim())));
        }
        v
    } else {
        let mut v = vec![Span::styled(format!("all agents · {}", total), Style::default().fg(text()))];
        if c.wide {
            v.push(Span::raw("   "));
            v.push(Span::styled("tab", Style::default().fg(text())));
            v.push(Span::styled(" this agent", Style::default().fg(dim())));
        }
        v
    };
    let title = if c.wide { "artifacts · what your agents made" } else { "artifacts" };
    let scope_w: usize = scope.iter().map(|s| s.content.width()).sum();
    let gap = width.saturating_sub(title.width() + scope_w + 4).max(1);
    let mut head = vec![Span::styled(title.to_string(), Style::default().fg(text()).add_modifier(Modifier::BOLD)), Span::raw(" ".repeat(gap))];
    head.extend(scope);
    out.push(Line::from(head));
    // the search row
    let search = if sc.query.is_empty() && !sc.typing {
        Line::from(Span::styled("/ find: a title, an agent, a kind", Style::default().fg(faint())))
    } else {
        let mut v = vec![
            Span::styled("/ ", Style::default().fg(if sc.typing { accent() } else { dim() })),
            Span::styled(sc.query.clone(), Style::default().fg(text())),
        ];
        if sc.typing {
            v.push(Span::styled("▏", Style::default().fg(accent())));
        }
        if !sc.query.is_empty() {
            v.push(Span::styled(format!("   {} of {}", list.len(), in_scope), Style::default().fg(dim())));
        }
        Line::from(v)
    };
    out.push(search);
    out.push(Line::from(""));
    // the body
    let foot = 4; // blank, the detail line, blank, the key bar
    let body_h = height.saturating_sub(out.len() + foot).max(1);
    let mut body: Vec<Body> = Vec::new();
    let mut last: Option<Group> = None;
    let mut sel_row = 0usize;
    let mut sel_end = 0usize;
    for (i, (a, _)) in list.iter().enumerate() {
        let g = clock.group(a.ts_ms);
        if last != Some(g) {
            body.push(Body::Group(g));
            last = Some(g);
        }
        if Some(i) == sel_i {
            sel_row = body.len();
        }
        body.push(Body::Item(i));
        if Some(i) == sel_i && sc.versions.is_some() {
            body.push(Body::BoxTop);
            for k in 0..list[i].0.versions.len() {
                body.push(Body::Version(k));
            }
            if c.wide {
                body.push(Body::BoxKeys);
            }
            body.push(Body::BoxBottom);
        }
        if Some(i) == sel_i {
            sel_end = body.len() - 1;
        }
    }
    // keep the selection (and its open box) in view; the first rows
    // with the group above the first row
    if body.len() > body_h {
        let room = body_h.saturating_sub(1); // the `↓ N more` row
        if sel_row < sc.top {
            sc.top = sel_row.saturating_sub(usize::from(sel_row > 0 && matches!(body[sel_row - 1], Body::Group(_))));
        }
        if sel_end >= sc.top + room {
            sc.top = (sel_end + 1).saturating_sub(room).min(sel_row);
        }
        sc.top = sc.top.min(body.len().saturating_sub(room));
    } else {
        sc.top = 0;
    }
    let mut shown_rows = 0;
    if list.is_empty() {
        out.push(Line::from(""));
        if all.is_empty() {
            out.push(Line::from(Span::styled(
                "  nothing yet. when an agent makes a page, a doc or a file for you, it lands here.",
                Style::default().fg(text()),
            )));
            out.push(Line::from(""));
            out.push(Line::from(Span::styled(
                "  agents add what they make with sb artifact add. bise pages come in by themselves.",
                Style::default().fg(dim()),
            )));
        } else {
            out.push(Line::from(Span::styled("  nothing matches. esc clears the search.", Style::default().fg(text()))));
        }
        shown_rows = out.len() - 4;
    } else {
        let more = body.len() > body_h;
        let room = if more { body_h - 1 } else { body_h };
        for (k, row) in body.iter().enumerate().skip(sc.top).take(room) {
            let _ = k;
            let line = match row {
                Body::Group(g) => Line::from(Span::styled(format!("  {}", g.words()), Style::default().fg(dim()))),
                Body::Item(i) => {
                    let (a, h) = &list[*i];
                    let (line, vhit) = item_line(a, h, Some(*i) == sel_i, &c, clock, width);
                    hits.push((out.len(), Hit::Row(a.id.clone(), vhit)));
                    line
                }
                Body::Version(v) => {
                    hits.push((out.len(), Hit::Version(*v)));
                    box_line(sel.expect("a box is under the selection"), row, sc.versions.unwrap_or(0), clock, c.wide)
                }
                _ => box_line(sel.expect("a box is under the selection"), row, sc.versions.unwrap_or(0), clock, c.wide),
            };
            out.push(line);
            shown_rows += 1;
        }
        if more {
            let below = list.len().saturating_sub(
                body.iter().take(sc.top + room).filter(|b| matches!(b, Body::Item(_))).count(),
            );
            if below > 0 {
                out.push(Line::from(Span::styled(format!("   ↓ {} more", below), Style::default().fg(dim()))));
                shown_rows += 1;
            }
        }
    }
    let _ = shown_rows;
    while out.len() < height.saturating_sub(foot) {
        out.push(Line::from(""));
    }
    out.truncate(height.saturating_sub(foot));
    out.push(Line::from(""));
    out.push(sel.map(|a| detail_line(a, clock, c.wide)).unwrap_or_default());
    out.push(Line::from(""));
    out.push(key_bar(sc, sel, list.is_empty(), c.wide));
    (out, hits, list.len())
}

// ---- drawing ----

/// The workspace as the frame says it: `~/acme`.
fn workspace_words(app: &App) -> String {
    let ws = crate::sb::workspace(app).unwrap_or_default();
    artifacts::short_target(&ws)
}

/// The frame of a full screen of bise: `╭─ bise :* ── {name} ──── ~/acme ─╮`.
pub(crate) fn draw_frame(app: &App, frame: &mut Frame, full: Rect, name: &str) -> Rect {
    let cols = crate::layout::cols(full.width, full.height);
    let cols = crate::layout::Cols { panel: None, ..cols };
    let mut title = app.sb.title();
    title.push(Span::styled(" ── ", crate::chrome::line_style()));
    title.push(Span::styled(name.to_string(), Style::default().fg(text())));
    let ws = workspace_words(app);
    let summary = move |room: usize| {
        if ws.width() <= room {
            vec![Span::styled(ws.clone(), Style::default().fg(dim()))]
        } else {
            Vec::new()
        }
    };
    if cols.framed {
        crate::chrome::draw_frame(frame.buffer_mut(), full, cols, title, Vec::new(), summary, full.bottom());
    }
    let m = cols.margin;
    let top = u16::from(cols.framed);
    Rect {
        x: full.x + m,
        y: full.y + top,
        width: full.width.saturating_sub(2 * m),
        height: full.height.saturating_sub(2 * top),
    }
}

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    if app.artifacts.is_none() {
        return;
    }
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 10 {
        return;
    }
    let area = draw_frame(app, frame, full, "artifacts");
    let all = artifacts::all();
    let now = crate::when::now_ms();
    let off = |ms: u64| crate::when::offset_at(ms);
    let clock = Clock { now, off: &off };
    let Some(sc) = app.artifacts.as_mut() else { return };
    let (rows, hits, _) = lines(sc, &all, area.width as usize, area.height as usize, &clock);
    sc.hits = hits.into_iter().map(|(i, h)| (area.y + i as u16, h)).collect();
    sc.search_y = Some(area.y + 2);
    sc.x0 = area.x;
    frame.render_widget(Paragraph::new(rows), area);
    crate::textlayer::text(area);
    // a click on a row's `v3` and the rows: the hand
    for (y, _) in &sc.hits {
        crate::pointer::region(Rect { x: area.x, y: *y, width: area.width, height: 1 }, crate::pointer::Shape::Pointer);
    }
}

// ---- keys and the mouse ----

fn selected(sc: &mut Screen) -> (Vec<(Artifact, Vec<usize>)>, Option<usize>) {
    let list = shown(sc, &artifacts::all());
    let i = sc.sel.as_ref().and_then(|id| list.iter().position(|(a, _)| &a.id == id));
    (list, i)
}

fn step(sc: &mut Screen, by: isize) {
    if let Some(v) = sc.versions.as_mut() {
        let n = sc.sel.as_ref().and_then(|id| artifacts::get(id)).map_or(0, |a| a.versions.len());
        *v = (*v as isize + by).clamp(0, n.saturating_sub(1) as isize) as usize;
        return;
    }
    let (list, i) = selected(sc);
    if list.is_empty() {
        return;
    }
    let j = match i {
        Some(i) => (i as isize + by).clamp(0, list.len() as isize - 1) as usize,
        None => 0,
    };
    sc.sel = Some(list[j].0.id.clone());
}

fn flash(app: &mut App, note: String) {
    app.flash = Some((note, Instant::now()));
}

/// Opens the selected artifact (its version when the box is open).
fn open_selected(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sc.sel.as_ref().and_then(|id| artifacts::get(id)) else { return };
    let v = sc.versions.and_then(|k| a.versions.iter().rev().nth(k)).map(|x| x.v);
    if v.is_none() {
        if let artifacts::How::Diff(n) = artifacts::how(&a, None, true) {
            app.artifacts = None;
            crate::diffview::request(app, crate::diffview::Ask::Pr(n), crate::diffview::By::Key);
            return;
        }
    }
    let note = artifacts::open(app, &a, v);
    flash(app, note);
}

/// Quick Look (macOS): `qlmanage -p` on the file; a link, or Linux,
/// opens like ⏎.
fn quick_look(app: &mut App) {
    let Some(sc) = app.artifacts.as_mut() else { return };
    let Some(a) = sc.sel.as_ref().and_then(|id| artifacts::get(id)) else { return };
    let target = a.open_target(None);
    if !cfg!(target_os = "macos") || artifacts::is_url(&target) || target.is_empty() {
        sc.look = None;
        return open_selected(app);
    }
    sc.look = Some(Look { child: spawn_look(&target) });
}

fn spawn_look(path: &str) -> Option<std::process::Child> {
    #[cfg(test)]
    {
        let _ = path;
        None
    }
    #[cfg(not(test))]
    {
        std::process::Command::new("qlmanage")
            .args(["-p", path])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
    }
}

/// `r`: the file in Finder (its folder elsewhere).
fn reveal(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sc.sel.as_ref().and_then(|id| artifacts::get(id)) else { return };
    if let Some(pr) = &a.pr {
        let url = if a.is_link() { a.target.clone() } else { format!("https://github.com/{}/pull/{}", pr.repo, pr.number) };
        let ok = crate::links::open(&url);
        return flash(app, if ok { format!("opening {}", artifacts::short_target(&url)) } else { format!("could not open {}", url) });
    }
    if a.is_link() {
        return;
    }
    let path = a.open_target(None);
    #[cfg(not(test))]
    {
        let ok = if cfg!(target_os = "macos") {
            std::process::Command::new("open").args(["-R", &path]).spawn().is_ok()
        } else {
            let dir = std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            std::process::Command::new("xdg-open").arg(dir).spawn().is_ok()
        };
        if !ok {
            flash(app, format!("could not show {}", path));
        }
    }
    #[cfg(test)]
    crate::links::OPENED.with(|o| o.borrow_mut().push(format!("reveal:{}", path)));
}

/// `c`: the path or the link.
fn copy(app: &mut App) {
    let Some(sc) = app.artifacts.as_mut() else { return };
    let Some(a) = sc.sel.as_ref().and_then(|id| artifacts::get(id)) else { return };
    let what = a.open_target(None);
    if crate::clipboard::copy(&what) {
        sc.copied = Some(Instant::now());
    }
}

/// `@`: the screen closes, the artifact's chip goes in your message.
fn put_in_message(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sc.sel.as_ref().and_then(|id| artifacts::get(id)) else { return };
    app.artifacts = None;
    crate::attach::insert_artifact(app, &a.id, &a.title);
}

/// Keys while the screen is open: it takes them all.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(sc) = app.artifacts.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('g')) {
        app.artifacts = None;
        return true;
    }
    // the search has the keys
    if sc.typing {
        match k.code {
            KeyCode::Esc => {
                sc.query.clear();
                sc.typing = false;
            }
            KeyCode::Enter => sc.typing = false,
            KeyCode::Up => {
                sc.typing = false;
                step(sc, -1);
            }
            KeyCode::Down => {
                sc.typing = false;
                step(sc, 1);
            }
            KeyCode::Backspace => {
                sc.query.pop();
            }
            KeyCode::Char(c) if !ctrl => sc.query.push(c),
            _ => {}
        }
        return true;
    }
    // Quick Look up: space closes it, ↑↓ preview the next one
    if sc.look.is_some() {
        match k.code {
            KeyCode::Char(' ') => sc.look = None,
            KeyCode::Up | KeyCode::Down => {
                step(sc, if k.code == KeyCode::Up { -1 } else { 1 });
                quick_look(app);
            }
            KeyCode::Enter => {
                sc.look = None;
                open_selected(app);
            }
            KeyCode::Esc => app.artifacts = None,
            _ => {}
        }
        return true;
    }
    match k.code {
        KeyCode::Esc if sc.versions.is_some() => sc.versions = None,
        KeyCode::Esc if !sc.query.is_empty() => sc.query.clear(),
        KeyCode::Esc => app.artifacts = None,
        KeyCode::Up | KeyCode::Char('k') if !ctrl => step(sc, -1),
        KeyCode::Down | KeyCode::Char('j') if !ctrl => step(sc, 1),
        KeyCode::PageUp => step(sc, -10),
        KeyCode::PageDown => step(sc, 10),
        KeyCode::Home => step(sc, -100_000),
        KeyCode::End => step(sc, 100_000),
        KeyCode::Enter => open_selected(app),
        KeyCode::Char('/') => {
            sc.typing = true;
            sc.versions = None;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            sc.this_agent = !sc.this_agent;
            sc.versions = None;
            sc.top = 0;
        }
        KeyCode::Char('v') if sc.versions.is_none() => {
            let n = sc.sel.as_ref().and_then(|id| artifacts::get(id)).map_or(0, |a| a.versions.len());
            if n > 0 {
                sc.versions = Some(0);
            } else {
                flash(app, "it has one version".to_string());
            }
        }
        KeyCode::Char('v') => sc.versions = None,
        KeyCode::Char(' ') if sc.versions.is_none() => quick_look(app),
        KeyCode::Char('r') | KeyCode::Char('o') => reveal(app),
        KeyCode::Char('c') if !ctrl => copy(app),
        KeyCode::Char('@') => put_in_message(app),
        _ => {}
    }
    true
}

/// The mouse while open: the wheel scrolls, a click opens, a click on
/// `v3` opens the versions, a click on the search focuses it, on the
/// scope switches it.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(sc) = app.artifacts.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => step(sc, -3),
        MouseEventKind::ScrollDown => step(sc, 3),
        MouseEventKind::Down(MouseButton::Left) => {
            if sc.search_y == Some(m.row) {
                sc.typing = true;
                return true;
            }
            let hit = sc.hits.iter().find(|(y, _)| *y == m.row).map(|(_, h)| h.clone());
            match hit {
                Some(Hit::Row(id, vcols)) => {
                    sc.sel = Some(id);
                    sc.typing = false;
                    let x0 = sc.x0;
                    let on_v = vcols.is_some_and(|(a, b)| m.column >= x0 + a && m.column < x0 + b);
                    if on_v {
                        sc.versions = Some(0);
                    } else {
                        sc.versions = None;
                        open_selected(app);
                    }
                }
                Some(Hit::Version(k)) => {
                    sc.versions = Some(k);
                    open_selected(app);
                }
                None => {}
            }
        }
        _ => {}
    }
    true
}

#[cfg(test)]
#[path = "artifacts_screen_tests.rs"]
mod tests;
