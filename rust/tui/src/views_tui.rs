//! MCP Apps views in the terminal (docs/mcp-apps.md C9; architect m_50261,
//! m_50268; designer m_50266). The terminal can't show a view's HTML: under
//! a tool row each view is one dim line, `⊞ counter · fake-app   app ↗
//! browser ↗` (expired: faint, `· expired`, no link), and `/views` lists
//! the thread's views, newest first, for when clicks are off.
//!
//! - `app ↗` / `⏎`: `app/open` (what `view`, the agent, the call) to the
//!   hub, which tells the desktop connections; none got it: the flash says
//!   the app isn't running and how to open it in the browser instead;
//! - `browser ↗` / `b`: `<pages url>/v/<agent>/<call>`, the hub's web host,
//!   in his browser.
//!
//! An artifact's `o` and `O` (the artifacts screen, docs/plugin-apps.md
//! A7) go the same ways: `app/open` (what `artifact`), or the web host's
//! document page `<pages url>/d/<id>`.
//!
//! The words are the catalog's (`words::views::tui_*`); the pure parts
//! (lines, urls, the list) have their tests in views_tui_tests.rs.

use crate::app::App;
use crate::theme::{dim, faint, glyph, G_VIEW};
use crate::wire::{Ev, ViewChip};
use bise_proto::words::views;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use serde_json::{json, Value};

/// A chip's link that opens its view in the app: `bise-view:app:<call>`.
const URL_APP: &str = "bise-view:app:";
/// A chip's link that opens its view in his browser: `bise-view:web:<call>`.
const URL_WEB: &str = "bise-view:web:";

/// Where a view opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum How {
    App,
    Web,
}

/// The `⊞` lines under a tool row, one per view, in line order.
pub(crate) fn chip_lines(chips: &[ViewChip], _width: usize) -> Vec<Line<'static>> {
    chips.iter().map(chip_line).collect()
}

fn chip_line(v: &ViewChip) -> Line<'static> {
    let head = format!("  {} {}", glyph(G_VIEW), views::tui_chip(v.name(), &v.plugin));
    if v.expired {
        let st = Style::default().fg(faint());
        return Line::from(vec![Span::styled(head, st), Span::styled(format!(" · {}", views::tui_expired), st)]);
    }
    let st = Style::default().fg(dim());
    Line::from(vec![
        Span::styled(head, st),
        Span::styled("   ", st),
        crate::textlayer::link(views::tui_app, &format!("{URL_APP}{}", v.call), st),
        Span::styled("   ", st),
        crate::textlayer::link(views::tui_browser, &format!("{URL_WEB}{}", v.call), st),
    ])
}

/// A chip's link: how and which call; None: not a chip's.
pub(crate) fn of_url(url: &str) -> Option<(How, String)> {
    let (how, call) = match (url.strip_prefix(URL_APP), url.strip_prefix(URL_WEB)) {
        (Some(c), _) => (How::App, c),
        (_, Some(c)) => (How::Web, c),
        _ => return None,
    };
    bise_proto::view_rules::call_ok(call).then(|| (how, call.to_string()))
}

/// The web host's url of a view (C9): `<pages url>/v/<agent>/<call>`.
pub(crate) fn web_url(pages_url: &str, agent: &str, call: &str) -> String {
    format!("{}/v/{}/{}", pages_url.trim_end_matches('/'), agent, call)
}

/// Open view `call` of the agent in view, in the app or the browser; the
/// words to flash now (empty: none yet, the app's answer may flash).
pub(crate) fn open(app: &mut App, how: How, call: &str) -> String {
    let agent = app.sb.focus_name().to_string();
    match how {
        How::App => {
            app.sb.call("app/open", json!({"what": "view", "agent": agent, "id": call}), crate::sb::rpc::Then::AppOpen);
            String::new()
        }
        How::Web => match app.sb.pages_url.clone() {
            Some(base) => {
                let url = web_url(&base, &agent, call);
                if crate::links::open(&url) {
                    String::new()
                } else {
                    format!("could not open {url}")
                }
            }
            None => views::tui_no_web.to_string(),
        },
    }
}

/// A document's page on the web host (docs/plugin-apps.md A7, the hub's
/// `GET /d/<artifact id>`): `<pages url>/d/<id>`.
pub(crate) fn doc_web_url(pages_url: &str, id: &str) -> String {
    format!("{}/d/{}", pages_url.trim_end_matches('/'), id)
}

/// `app/open`'s answer: no desktop connection got it, the flash says so.
pub(crate) fn app_opened(app: &mut App, v: &Value) {
    if delivered(v) == 0 {
        crate::input::flash(app, views::tui_no_app.to_string());
    }
}

/// How many desktop connections got `app/open` (its result).
pub(crate) fn delivered(v: &Value) -> u64 {
    v.get("delivered").and_then(Value::as_u64).unwrap_or(0)
}

// ---- /views ----

/// The `/views` screen: its selected row.
#[derive(Debug, Default)]
pub(crate) struct Screen {
    sel: usize,
}

/// One row of `/views`: the view and how long ago its call started.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) view: ViewChip,
    pub(crate) ago: std::time::Duration,
}

/// The thread's views, newest first; `now_ms` (ms since the epoch) dates
/// each from its call's start (ViewChip.at_ms), so a view loaded from the
/// history keeps its age.
pub(crate) fn rows(events: &[Ev], now_ms: u64) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for e in events.iter().rev() {
        let Ev::Tool(td) = e else { continue };
        for v in td.views.iter().rev() {
            out.push(Row { view: v.clone(), ago: std::time::Duration::from_millis(now_ms.saturating_sub(v.at_ms)) });
        }
    }
    out
}

/// The next openable row from `from` in direction `down` (expired rows are skipped).
fn step(rows: &[Row], from: usize, down: bool) -> usize {
    let mut i = from;
    loop {
        let next = if down { i.checked_add(1).filter(|&n| n < rows.len()) } else { i.checked_sub(1) };
        let Some(n) = next else { return from };
        if !rows[n].view.expired {
            return n;
        }
        i = n;
    }
}

/// The first openable row.
fn first(rows: &[Row]) -> usize {
    rows.iter().position(|r| !r.view.expired).unwrap_or(0)
}

/// `/views`: the screen opens on the newest live view.
pub(crate) fn open_screen(app: &mut App) {
    let sel = first(&rows(&app.events, crate::when::now_ms()));
    app.views_list = Some(Screen { sel });
}

/// A key while `/views` is up (true: it took it).
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(sc) = app.views_list.as_ref() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let rows = rows(&app.events, crate::when::now_ms());
    let sel = sc.sel.min(rows.len().saturating_sub(1));
    let live = rows.get(sel).filter(|r| !r.view.expired).map(|r| r.view.call.clone());
    match k.code {
        KeyCode::Esc => app.views_list = None,
        KeyCode::Char('c' | 'g') if ctrl => app.views_list = None,
        KeyCode::Up | KeyCode::Char('k') => set_sel(app, step(&rows, sel, false)),
        KeyCode::Down | KeyCode::Char('j') => set_sel(app, step(&rows, sel, true)),
        KeyCode::Enter => {
            if let Some(call) = live {
                app.views_list = None;
                let note = open(app, How::App, &call);
                flash_if(app, note);
            }
        }
        KeyCode::Char('b') => {
            if let Some(call) = live {
                app.views_list = None;
                let note = open(app, How::Web, &call);
                flash_if(app, note);
            }
        }
        _ => {}
    }
    true
}

fn set_sel(app: &mut App, sel: usize) {
    if let Some(sc) = app.views_list.as_mut() {
        sc.sel = sel;
    }
}

fn flash_if(app: &mut App, note: String) {
    if !note.is_empty() {
        crate::input::flash(app, note);
    }
}

/// The screen's lines (pure): its rows, the selected one marked, the key bar.
pub(crate) fn lines(rows: &[Row], sel: usize) -> Vec<Line<'static>> {
    let mut ls = vec![Line::from(Span::styled(views::tui_list_title.to_string(), Style::default().add_modifier(Modifier::BOLD))), Line::default()];
    if rows.is_empty() {
        ls.push(Line::from(Span::styled(views::tui_list_empty.to_string(), Style::default().fg(dim()))));
        return ls;
    }
    for (i, r) in rows.iter().enumerate() {
        let age = views::tui_list_age(age_words(r.ago));
        let words = format!("{} {} · {}", glyph(G_VIEW), views::tui_chip(r.view.name(), &r.view.plugin), age);
        let (mark, st) = match (i == sel && !r.view.expired, r.view.expired) {
            (true, _) => ("› ", Style::default().add_modifier(Modifier::BOLD)),
            (false, true) => ("  ", Style::default().fg(faint())),
            (false, false) => ("  ", Style::default()),
        };
        ls.push(Line::from(vec![Span::styled(mark, st), Span::styled(words, st)]));
    }
    ls.push(Line::default());
    ls.push(Line::from(Span::styled(views::tui_list_keys.to_string(), Style::default().fg(dim()))));
    ls
}

/// A row's age: `40s`, `2 min`, `3 h`, `2 d`.
pub(crate) fn age_words(d: std::time::Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{} min", s / 60),
        3600..=86_399 => format!("{} h", s / 3600),
        _ => format!("{} d", s / 86_400),
    }
}

/// Draws `/views` over the screen when it is up.
pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let Some(sc) = app.views_list.as_ref() else { return };
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 20 || full.height < 6 {
        return;
    }
    let rows = rows(&app.events, crate::when::now_ms());
    let sel = sc.sel.min(rows.len().saturating_sub(1));
    let area = Rect { x: full.x + 2, y: full.y + 1, width: full.width.saturating_sub(4), height: full.height.saturating_sub(2) };
    frame.render_widget(Paragraph::new(lines(&rows, sel)), area);
}

#[cfg(test)]
#[path = "views_tui_tests.rs"]
mod tests;
