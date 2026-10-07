//! What the TUI had open, kept across a reload (keep-state): a version
//! switch, `sb restart` or a hub restarted after a crash re-executes the
//! TUI, and the new one lands where the old one was.
//!
//! Two parts, both in the drafts file (sb/drafts.rs writes and reads it,
//! one file per workspace):
//! - the inbox answers: card id → its draft (text, cursor), the one in
//!   the composer while the card is open too. Kept like the composer's
//!   drafts (no age); a card the hub no longer has drops its draft at
//!   `ready`. The TUI's own setup items are never saved (the key card's
//!   text is a key).
//! - the view ([`View`]): the agent in view, the open inbox item (its
//!   option, full screen, scroll), the history's scroll when pinned (as a
//!   transcript position: the replay rebuilds the events), and the open
//!   popup or screen with its query (the agent palette, find, help, the
//!   artifacts and scheduled screens, approvals, the diff panel). It
//!   comes back only when written within [`VIEW_KEPT`] (a reload): a
//!   start the next morning opens on main, plain.
//!
//! Both come back at the hub's `ready` ([`apply`]): the cards, the agents
//! and the replayed feeds are there by then. A hub that comes back (a
//! reload's, before this TUI re-executes; a restarted one) empties the
//! feeds: the scroll of the agent in view is kept for its `ready` too
//! ([`before_reconnect`]). Until then they stay in what
//! is saved ([`answers`], [`view`]), so a write before `ready` loses
//! nothing.
//!
//! Not kept: a selection (in the history or a field), the palette's and
//! the screens' selected row, the help's scroll, a popup's typing mode in
//! the log view (`/log`, dev only), the terminal pane, voice mode.

use super::{focus, App};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Duration;

/// The view comes back only in a TUI started this soon after it was
/// written: a reload, never a real restart later.
pub(crate) const VIEW_KEPT: Duration = Duration::from_secs(60);

/// A text and its cursor (chars).
pub(super) type Text = (String, usize);

/// The open popup or screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Popup {
    Palette(String),
    Find(Text),
    Help { shortcuts: bool, filter: String },
    Artifacts { query: String, typing: bool, this_agent: bool },
    Scheduled { query: String, typing: bool },
    Approvals,
    /// the diff of the agent in view
    Diff,
}

/// The open inbox item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardOpen {
    pub(super) id: u64,
    pub(super) full: bool,
    pub(super) opt: Option<usize>,
    pub(super) scroll: usize,
}

/// The history scrolled up: the transcript line its top row comes from
/// (`pos`), the event of that line (`off`) and the row in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pin {
    pub(super) pos: usize,
    pub(super) off: usize,
    pub(super) row: usize,
}

/// What the TUI shows, beyond its texts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct View {
    pub(super) focus: String,
    pub(super) card: Option<CardOpen>,
    pub(super) pin: Option<Pin>,
    pub(super) popup: Option<Popup>,
}

/// What was read at start: the inbox answers and the view.
type Read = (BTreeMap<u64, Text>, Option<View>);

thread_local! {
    /// Read at start, back at `ready` ([`apply`]).
    static PENDING: RefCell<Option<Read>> = const { RefCell::new(None) };
    /// The pinned history of the agent in view when the hub went away
    /// (a reconnection empties the feeds): back at the next `ready`.
    static REPIN: RefCell<Option<(String, Pin)>> = const { RefCell::new(None) };
}

/// The hub came back (sb.rs `hub_reconnected`), its replay will refill
/// the feeds: the scroll of the agent in view is kept for its `ready`
/// (a reload's hub comes back before the TUI re-executes).
pub(super) fn before_reconnect(app: &App) {
    let pin = if app.follow { None } else { pin_of(app) };
    if let Some(p) = pin {
        REPIN.with(|r| *r.borrow_mut() = Some((app.sb.focus.clone(), p)));
    }
}

// ---- what the app holds ----

/// The inbox answers being written: the open card's (in the composer)
/// and the others'. Never the TUI's own setup items.
pub(super) fn answers(app: &App) -> BTreeMap<u64, Text> {
    let cv = &app.sb.card;
    let own = |ed: &crate::editor::Editor| {
        let (t, c) = ed.own_draft();
        (t.to_string(), c)
    };
    let mut out: BTreeMap<u64, Text> =
        cv.drafts.iter().filter(|(id, _)| !super::setup::is_local(**id)).map(|(id, ed)| (*id, own(ed))).collect();
    if let Some(id) = cv.sel.filter(|id| cv.open && !super::setup::is_local(*id)) {
        out.insert(id, own(&app.ed));
    }
    out.retain(|_, (t, _)| !t.is_empty());
    if let Some((p, _)) = PENDING.with(|p| p.borrow().clone()) {
        for (id, t) in p {
            out.entry(id).or_insert(t);
        }
    }
    out
}

/// The view now (None: main in view, nothing open, at the bottom); the
/// one read at start while it is not back yet.
pub(super) fn view(app: &App) -> Option<View> {
    if let Some((_, v)) = PENDING.with(|p| p.borrow().clone()) {
        return v;
    }
    let cv = &app.sb.card;
    let card = cv.sel.filter(|id| cv.open && !super::setup::is_local(*id)).map(|id| CardOpen {
        id,
        full: cv.full,
        opt: cv.opt,
        scroll: cv.scroll,
    });
    let held = REPIN.with(|r| r.borrow().clone()).filter(|(a, _)| *a == app.sb.focus).map(|(_, p)| p);
    let pin = if app.follow { held } else { pin_of(app) };
    let popup = popup_of(app);
    let focus = if app.sb.focus == "main" { String::new() } else { app.sb.focus.clone() };
    let v = View { focus, card, pin, popup };
    (v != View::default()).then_some(v)
}

/// The pinned history's top as a transcript position.
fn pin_of(app: &App) -> Option<Pin> {
    let (ev, row) = app.anchor;
    let &(i, pos) = app.win.marks.iter().rev().find(|(i, _)| *i <= ev)?;
    Some(Pin { pos, off: ev - i, row })
}

fn popup_of(app: &App) -> Option<Popup> {
    if let Some(p) = &app.palette {
        return Some(Popup::Palette(p.query.clone()));
    }
    if let Some(f) = &app.find {
        let (t, c) = f.ed.own_draft();
        return Some(Popup::Find((t.to_string(), c)));
    }
    if let Some(h) = &app.help {
        return Some(Popup::Help { shortcuts: h.page() == crate::help::Page::Shortcuts, filter: h.filter.clone() });
    }
    if let Some(s) = &app.artifacts {
        return Some(Popup::Artifacts { query: s.query.clone(), typing: s.typing, this_agent: s.this_agent });
    }
    if let Some(s) = &app.scheduled {
        return Some(Popup::Scheduled { query: s.query.clone(), typing: s.typing });
    }
    if app.approvals.is_some() {
        return Some(Popup::Approvals);
    }
    let own_diff = app.diff.as_ref().is_some_and(|p| p.ask == crate::diffview::Ask::Agent(app.sb.focus.clone()));
    own_diff.then_some(Popup::Diff)
}

// ---- the file ----

fn text_json((t, c): &Text) -> Value {
    json!({"text": t, "cursor": c})
}

fn text_of(v: &Value) -> Text {
    let t = v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let c = v.get("cursor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    (t, c)
}

pub(super) fn answers_json(a: &BTreeMap<u64, Text>) -> Value {
    Value::Object(a.iter().map(|(id, t)| (id.to_string(), text_json(t))).collect())
}

pub(super) fn answers_from(v: Option<&Value>) -> BTreeMap<u64, Text> {
    v.and_then(|x| x.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(id, t)| Some((id.parse::<u64>().ok()?, text_of(t))))
                .filter(|(id, (t, _))| !t.is_empty() && !super::setup::is_local(*id))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn view_json(v: &View, now_ms: u64) -> Value {
    let mut o = json!({"ms": now_ms, "focus": v.focus});
    if let Some(c) = &v.card {
        o["card"] = json!({"id": c.id, "full": c.full, "opt": c.opt, "scroll": c.scroll});
    }
    if let Some(p) = &v.pin {
        o["pin"] = json!({"pos": p.pos, "off": p.off, "row": p.row});
    }
    if let Some(p) = &v.popup {
        o["popup"] = match p {
            Popup::Palette(q) => json!({"kind": "palette", "query": q}),
            Popup::Find(t) => json!({"kind": "find", "query": t.0, "cursor": t.1}),
            Popup::Help { shortcuts, filter } => json!({"kind": "help", "shortcuts": shortcuts, "query": filter}),
            Popup::Artifacts { query, typing, this_agent } => {
                json!({"kind": "artifacts", "query": query, "typing": typing, "this_agent": this_agent})
            }
            Popup::Scheduled { query, typing } => json!({"kind": "scheduled", "query": query, "typing": typing}),
            Popup::Approvals => json!({"kind": "approvals"}),
            Popup::Diff => json!({"kind": "diff"}),
        };
    }
    o
}

/// The view written within [`VIEW_KEPT`] of `now_ms`; anything this TUI
/// does not know (a newer one's field, a popup kind) is left out.
pub(super) fn view_from(v: Option<&Value>, now_ms: u64) -> Option<View> {
    let v = v?;
    let ms = v.get("ms")?.as_u64()?;
    if now_ms.saturating_sub(ms) > VIEW_KEPT.as_millis() as u64 {
        return None;
    }
    let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let n = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_u64());
    let b = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
    let card = v.get("card").and_then(|c| {
        Some(CardOpen {
            id: n(c, "id")?,
            full: b(c, "full"),
            opt: n(c, "opt").map(|x| x as usize),
            scroll: n(c, "scroll").unwrap_or(0) as usize,
        })
    });
    let pin = v.get("pin").and_then(|p| {
        Some(Pin { pos: n(p, "pos")? as usize, off: n(p, "off").unwrap_or(0) as usize, row: n(p, "row").unwrap_or(0) as usize })
    });
    let popup = v.get("popup").and_then(|p| match s(p, "kind").as_str() {
        "palette" => Some(Popup::Palette(s(p, "query"))),
        "find" => Some(Popup::Find((s(p, "query"), n(p, "cursor").unwrap_or(0) as usize))),
        "help" => Some(Popup::Help { shortcuts: b(p, "shortcuts"), filter: s(p, "query") }),
        "artifacts" => {
            Some(Popup::Artifacts { query: s(p, "query"), typing: b(p, "typing"), this_agent: b(p, "this_agent") })
        }
        "scheduled" => Some(Popup::Scheduled { query: s(p, "query"), typing: b(p, "typing") }),
        "approvals" => Some(Popup::Approvals),
        "diff" => Some(Popup::Diff),
        _ => None,
    });
    let view = View { focus: s(v, "focus"), card, pin, popup };
    (view != View::default()).then_some(view)
}

// ---- back in the app ----

/// At start: what was read waits for the hub's `ready`.
pub(super) fn hold(answers: BTreeMap<u64, Text>, view: Option<View>) {
    let some = !answers.is_empty() || view.is_some();
    PENDING.with(|p| *p.borrow_mut() = some.then_some((answers, view)));
}

/// Whether what was read at start is still to come back.
#[cfg(test)]
pub(super) fn pending() -> bool {
    PENDING.with(|p| p.borrow().is_some())
}

fn editor((t, c): Text) -> crate::editor::Editor {
    let mut ed = crate::editor::Editor::default();
    ed.cursor = c.min(t.chars().count());
    ed.text = t;
    ed
}

/// The hub's `ready`: the agent in view, its answers, the open item, the
/// popup, the scroll come back (each only when it still makes sense: the
/// agent is still there, the card still open).
pub(crate) fn apply(app: &mut App) {
    if let Some((agent, p)) = REPIN.with(|r| r.borrow_mut().take()) {
        if agent == app.sb.focus && app.follow {
            repin(app, p);
        }
    }
    let Some((answers, view)) = PENDING.with(|p| p.borrow_mut().take()) else { return };
    let live = app.sb.card_ids();
    let view = view.unwrap_or_default();
    if !view.focus.is_empty() && view.focus != app.sb.focus && app.sb.agents.iter().any(|a| a.name == view.focus) {
        focus(app, &view.focus);
    }
    for (id, t) in answers {
        if live.contains(&id) {
            app.sb.card.drafts.entry(id).or_insert_with(|| editor(t));
        }
    }
    if let Some(c) = view.card.filter(|c| live.contains(&c.id)) {
        super::cards::open_view(app, Some(c.id));
        let cv = &mut app.sb.card;
        cv.full = c.full;
        cv.opt = c.opt;
        cv.reveal = c.opt.is_some();
        cv.scroll = c.scroll;
        cv.max_scroll = c.scroll;
    }
    if let Some(p) = view.pin {
        repin(app, p);
    }
    if let Some(p) = view.popup {
        reopen(app, p);
    }
}

/// The history back at the same line: its event in the replayed feed (the
/// oldest loaded when that line is older than the replay).
fn repin(app: &mut App, p: Pin) {
    if app.events.is_empty() {
        return;
    }
    let ev = match app.win.marks.iter().find(|(_, pos)| *pos >= p.pos) {
        Some(&(i, pos)) if pos == p.pos => i + p.off,
        Some(&(i, _)) => i,
        None => return,
    };
    app.follow = false;
    app.anchor = (ev.min(app.events.len() - 1), p.row);
    app.scroll = 0;
}

fn reopen(app: &mut App, p: Popup) {
    match p {
        Popup::Palette(q) => crate::sb::palette::open(app, &q),
        Popup::Find(t) => {
            crate::find::open(app);
            if let Some(f) = app.find.as_mut() {
                f.ed = editor(t);
            }
            crate::find::edited(app);
        }
        Popup::Help { shortcuts, filter } => {
            let page = if shortcuts { crate::help::Page::Shortcuts } else { crate::help::Page::Help };
            let mut o = crate::help::Overlay::new(page);
            o.filter = filter;
            app.help = Some(o);
        }
        Popup::Artifacts { query, typing, this_agent } => {
            crate::artifacts_screen::open(app);
            if let Some(s) = app.artifacts.as_mut() {
                s.query = query;
                s.typing = typing;
                s.this_agent = this_agent;
            }
        }
        Popup::Scheduled { query, typing } => {
            crate::scheduled_screen::open(app);
            if let Some(s) = app.scheduled.as_mut() {
                s.query = query;
                s.typing = typing;
            }
        }
        Popup::Approvals => app.approvals = Some(crate::approvals_screen::Screen::default()),
        Popup::Diff => crate::diffview::toggle(app),
    }
}
