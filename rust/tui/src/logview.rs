//! `/log`: the raw session of the agent in view, read
//! from its session log (`~/.bise/sessions/<id>/`, rust/session), in a
//! full screen. Two views: the full history (every entry in order, the
//! turns and the compactions as rules) and what the model got for one
//! request (the system prompt, the tools and the context the log held
//! right before it, compaction applied). Read-only: the log is read
//! once at open (`r` reads it again); every text is redacted.
//!
//! Keys: ↑↓ (j/k) move between entries, space or ⏎ opens one, ctrl+o
//! opens them all, / searches (n/N), f filters roles and tools, tab
//! switches views, [ ] change the request, g/G top/bottom, esc closes.
//!
//! Shipped to everyone ([`SHIPPED`]; false would bring back the dev
//! build or `BISE_DEV=1` only). Later: an "export for a bug report" action (the
//! redacted entries to a file) fits here; not built yet.

mod model;
#[cfg(test)]
mod tests;

use crate::{theme, App};
use bise_session::reader::Log;
use bise_session::Redactor;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use model::{Body, Item, Request, Role};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

/// `/log` for everyone (false: the dev build or `BISE_DEV=1` only).
pub(crate) const SHIPPED: bool = true;

/// The hub's state folder (`~/.bise/hubs/<hub>`): the TUI's socket's
/// folder, set once at connect.
static HUB_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn set_hub_dir(socket: &Path) {
    if let Some(d) = socket.parent() {
        let _ = HUB_DIR.set(d.to_path_buf());
    }
}

/// Whether `/log` is offered: shipped, the dev build (the hub runs in
/// bise's source tree), or `BISE_DEV=1`.
pub(crate) fn enabled(app: &App) -> bool {
    SHIPPED || crate::sb::release::dev(app) || std::env::var("BISE_DEV").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The `/log` command, for the popup and `/help` when enabled.
pub(crate) const COMMAND: crate::commands::Cmd = crate::commands::Cmd {
    name: "/log",
    desc: "the raw session of the agent in view: every entry, and what the model got: /log [<request>]",
    args: &[],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    History,
    Model,
}

/// The `/` field: what is typed, and whether it still takes keys.
#[derive(Clone, Debug, Default)]
pub(crate) struct Search {
    pub(crate) text: String,
    pub(crate) typing: bool,
}

/// The filter picker: its rows (role keys, then `tool:<name>`) and the
/// cursor.
#[derive(Clone, Debug, Default)]
pub(crate) struct Picker {
    pub(crate) rows: Vec<String>,
    pub(crate) sel: usize,
}

/// The screen while open.
pub(crate) struct View {
    pub(crate) agent: String,
    log: Log,
    blobs: PathBuf,
    redact: Redactor,
    pub(crate) mode: Mode,
    history: Vec<Item>,
    requests: Vec<Request>,
    /// the request of the model view (an index of `requests`)
    pub(crate) req: usize,
    model: Vec<Item>,
    /// the entries whose fold differs from the default, per view
    flipped: [HashSet<usize>; 2],
    /// ctrl+o: everything open
    all_open: bool,
    /// the shown entries (indexes of the view's items) and the cursor
    /// among them
    vis: Vec<usize>,
    pub(crate) sel: usize,
    /// the first row on screen: (index in `vis`, row in that entry)
    top: (usize, usize),
    /// the filtered-out keys (a role key, or `tool:<name>`)
    pub(crate) off: HashSet<String>,
    pub(crate) picker: Option<Picker>,
    pub(crate) search: Option<Search>,
    /// the shown entries that match the search (indexes in `vis`)
    hits: Vec<usize>,
    /// the rows of each entry, by (view, item, width, open)
    cache: HashMap<(u8, usize), (usize, bool, Vec<Line<'static>>)>,
    /// rows shown by the last frame (paging)
    page: usize,
    /// a word under the top bar (the log could not be read, ...)
    pub(crate) note: String,
    /// the width of the last frame (the default folds depend on it)
    width: usize,
    /// the request bodies logged by `BISE_DEBUG_REQUESTS` (ms, file)
    dumps: Vec<(u64, PathBuf)>,
    /// the model view's request has its exact body (its first entry)
    exact: bool,
    /// where the cursor was when `/` opened (esc in the field goes back)
    origin: (usize, (usize, usize)),
    /// `n` or `N` went round the end: "back to the first" (until a key)
    wrapped: &'static str,
}

/// The session folder of `agent` in this hub: `agents/<agent>/session`
/// holds its id.
fn session_dir(agent: &str) -> Result<PathBuf, String> {
    let hub = HUB_DIR.get().ok_or("no hub folder known")?;
    let idf = hub.join("agents").join(agent).join("session");
    let id = std::fs::read_to_string(&idf).map_err(|_| format!("{agent} has no session log yet ({})", idf.display()))?;
    let id = id.trim();
    if id.is_empty() {
        return Err(format!("{agent} has no session log yet"));
    }
    Ok(bise_home::Home::from_env().sessions_dir().join(id))
}

/// `/log [N]` on the agent in view: the screen, or why not.
pub(crate) fn open(app: &mut App, typed: &str) -> Result<(), String> {
    let agent = app.sb.focus.clone();
    let dir = session_dir(&agent)?;
    let req: Option<u64> = typed.split_whitespace().nth(1).and_then(|w| w.trim_start_matches('#').parse().ok());
    let mut v = View::load(agent, &dir)?;
    if let Some(n) = req {
        // requests count from 1 in the log (a REPL restart starts its own
        // numbers again: the position, never the REPL's number)
        match (n as usize).checked_sub(1).filter(|&i| i < v.requests.len()) {
            Some(i) => {
                v.req = i;
                v.set_mode(Mode::Model);
            }
            None => v.note = format!("no request {n} in this log: the last one is {}", v.requests.len()),
        }
    }
    app.logview = Some(v);
    Ok(())
}

impl View {
    pub(crate) fn load(agent: String, dir: &Path) -> Result<View, String> {
        let log = bise_session::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let home = bise_home::Home::from_env();
        let redact = Redactor::from_home(&home.auth_file(), &home.env_files());
        let dumps = HUB_DIR.get().map(|h| model::request_files(&h.join("agents").join(&agent).join("requests"))).unwrap_or_default();
        let mut v = View::of(agent, log, home.blobs_dir(), redact);
        v.dumps = dumps;
        Ok(v)
    }

    pub(crate) fn of(agent: String, log: Log, blobs: PathBuf, redact: Redactor) -> View {
        let src = model::Source { log: &log, blobs: &blobs, redact: &redact };
        let history = model::history(&src);
        let requests = model::requests(&log);
        let mut v = View {
            agent,
            log,
            blobs,
            redact,
            mode: Mode::History,
            history,
            req: requests.len().saturating_sub(1),
            requests,
            model: Vec::new(),
            flipped: [HashSet::new(), HashSet::new()],
            all_open: false,
            vis: Vec::new(),
            sel: 0,
            top: (0, 0),
            off: HashSet::new(),
            picker: None,
            search: None,
            hits: Vec::new(),
            cache: HashMap::new(),
            page: 20,
            note: String::new(),
            width: 100,
            dumps: Vec::new(),
            exact: false,
            origin: (0, (0, 0)),
            wrapped: "",
        };
        v.refilter();
        v.bottom();
        v
    }

    fn mi(&self) -> usize {
        (self.mode == Mode::Model) as usize
    }

    pub(crate) fn items(&self) -> &[Item] {
        match self.mode {
            Mode::History => &self.history,
            Mode::Model => &self.model,
        }
    }

    pub(crate) fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        if m == Mode::Model {
            self.build_model();
        }
        self.refilter();
        if m == Mode::Model {
            self.sel = 0;
            self.top = (0, 0);
        } else {
            self.bottom();
        }
    }

    fn build_model(&mut self) {
        self.cache.retain(|k, _| k.0 == 0);
        self.flipped[1].clear();
        self.model = match self.requests.get(self.req) {
            Some(r) => {
                let src = model::Source { log: &self.log, blobs: &self.blobs, redact: &self.redact };
                model::model_view(&src, r)
            }
            None => Vec::new(),
        };
        self.exact = false;
        // the exact body, when the REPL logged it (BISE_DEBUG_REQUESTS)
        let at = |i: usize| self.requests.get(i).and_then(|r| model::iso_ms(&self.log.events[r.idx].at));
        if let (Some(reply), false) = (at(self.req), self.model.is_empty()) {
            let since = self.req.checked_sub(1).and_then(at).unwrap_or(0);
            if let Some(p) = model::dump_for(&self.dumps, since, reply) {
                let it = model::exact_item(p, &self.redact, &self.model[0]);
                self.model.insert(0, it);
                self.exact = true;
            }
        }
    }

    /// The request of the model view moves by `d` (clamped).
    pub(crate) fn step_request(&mut self, d: isize) {
        if self.requests.is_empty() {
            return;
        }
        let n = self.requests.len() as isize;
        let r = (self.req as isize + d).clamp(0, n - 1) as usize;
        if r != self.req || self.mode != Mode::Model {
            self.req = r;
            self.set_mode(Mode::Model);
        }
    }

    /// The tools of the view's calls and results, sorted.
    fn tools(&self) -> Vec<String> {
        let mut t: Vec<String> = self.items().iter().filter(|i| !i.tool.is_empty()).map(|i| i.tool.clone()).collect();
        t.sort();
        t.dedup();
        t
    }

    fn shown(&self, it: &Item) -> bool {
        if it.rule.is_some() {
            return self.off.is_empty();
        }
        if self.off.contains(it.role.key()) {
            return false;
        }
        it.tool.is_empty() || !self.off.contains(&format!("tool:{}", it.tool))
    }

    /// The shown entries again (a filter or a view changed); the cursor
    /// stays on its entry when it is still shown.
    pub(crate) fn refilter(&mut self) {
        let cur = self.vis.get(self.sel).copied();
        self.vis = (0..self.items().len()).filter(|&i| self.shown(&self.items()[i])).collect();
        self.sel = cur.and_then(|c| self.vis.iter().position(|&i| i >= c)).unwrap_or(0).min(self.vis.len().saturating_sub(1));
        self.top = (self.sel, 0);
        self.rehit();
    }

    fn rehit(&mut self) {
        self.hits.clear();
        let Some(s) = &self.search else { return };
        let needle = s.text.to_ascii_lowercase();
        if needle.is_empty() {
            return;
        }
        let items = self.items();
        self.hits = (0..self.vis.len()).filter(|&k| items[self.vis[k]].hay().contains(&needle)).collect();
    }

    fn bottom(&mut self) {
        self.sel = self.vis.len().saturating_sub(1);
        self.top = (self.sel, 0);
    }

    /// The fold of item `i` by default: open when its body is short.
    /// The fold of an entry by default: open when its body takes at
    /// most 6 rows at `width` (wrapped), never the system prompt or tools.
    fn default_open(it: &Item, width: usize) -> bool {
        let t = it.body.text();
        if matches!(it.role, Role::System | Role::Tools) || t.len() > 1200 || t.trim_end().lines().count() > 6 {
            return false;
        }
        body_rows(&it.body, body_width(width), it.role == Role::Thinking).len() <= 6
    }

    pub(crate) fn is_open(&self, i: usize) -> bool {
        let it = &self.items()[i];
        if matches!(it.body, Body::None) {
            return false;
        }
        let flipped = self.flipped[self.mi()].contains(&i);
        if self.all_open {
            // ctrl+o: all open but the ones closed since
            !flipped
        } else {
            Self::default_open(it, self.width) != flipped
        }
    }

    pub(crate) fn toggle(&mut self, i: usize) {
        let mi = self.mi();
        if !self.flipped[mi].remove(&i) {
            self.flipped[mi].insert(i);
        }
    }

    pub(crate) fn toggle_all(&mut self) {
        self.all_open = !self.all_open;
        let mi = self.mi();
        self.flipped[mi].clear();
    }

    /// Rows of the shown entry `k` at `width` (cached).
    fn rows(&mut self, k: usize, width: usize) -> &[Line<'static>] {
        let i = self.vis[k];
        let open = self.is_open(i);
        let key = (self.mi() as u8, i);
        let fresh = matches!(self.cache.get(&key), Some((w, o, _)) if *w == width && *o == open);
        if !fresh {
            let mut rows = entry_rows(&self.items()[i], width, open, self.mode, &agent_word(&self.agent));
            // a blank row after each entry: they read apart
            rows.push(Line::from(""));
            self.cache.insert(key, (width, open, rows));
        }
        &self.cache[&key].2
    }

    /// Move the first row so the cursor's header shows.
    fn reveal(&mut self, width: usize, height: usize) {
        if self.vis.is_empty() {
            return;
        }
        if self.sel < self.top.0 || (self.sel == self.top.0 && self.top.1 > 0) {
            self.top = (self.sel, 0);
            return;
        }
        // rows from the top to the cursor's header
        let mut used = self.rows(self.top.0, width).len().saturating_sub(self.top.1);
        for k in self.top.0 + 1..=self.sel {
            if k == self.sel {
                break;
            }
            used += self.rows(k, width).len();
        }
        // the cursor's header and up to 3 rows of its body
        let want = used + self.rows(self.sel, width).len().min(4);
        let mut over = want.saturating_sub(height);
        while over > 0 && self.top.0 < self.sel {
            let left = self.rows(self.top.0, width).len() - self.top.1;
            if left <= over {
                over -= left;
                self.top = (self.top.0 + 1, 0);
            } else {
                self.top.1 += over;
                over = 0;
            }
        }
        self.fill(width, height);
    }

    /// No blank rows under the last entry while there are rows above
    /// the first one shown: the end sits on the bottom row.
    fn fill(&mut self, width: usize, height: usize) {
        let mut below = self.rows(self.top.0, width).len().saturating_sub(self.top.1);
        let mut k = self.top.0 + 1;
        while below < height && k < self.vis.len() {
            below += self.rows(k, width).len();
            k += 1;
        }
        while below < height {
            if self.top.1 > 0 {
                self.top.1 -= 1;
            } else if self.top.0 > 0 {
                let n = self.rows(self.top.0 - 1, width).len();
                self.top = (self.top.0 - 1, n.saturating_sub(1));
            } else {
                break;
            }
            below += 1;
        }
    }

    /// Scroll by `d` rows (the cursor follows to the top entry shown).
    fn scroll(&mut self, d: isize, width: usize) {
        if self.vis.is_empty() {
            return;
        }
        if d > 0 {
            let mut d = d as usize;
            while d > 0 {
                let n = self.rows(self.top.0, width).len();
                if self.top.1 + 1 < n {
                    self.top.1 += 1;
                } else if self.top.0 + 1 < self.vis.len() {
                    self.top = (self.top.0 + 1, 0);
                } else {
                    break;
                }
                d -= 1;
            }
        } else {
            let mut d = (-d) as usize;
            while d > 0 {
                if self.top.1 > 0 {
                    self.top.1 -= 1;
                } else if self.top.0 > 0 {
                    let n = self.rows(self.top.0 - 1, width).len();
                    self.top = (self.top.0 - 1, n.saturating_sub(1));
                } else {
                    break;
                }
                d -= 1;
            }
        }
        self.sel = if self.top.1 == 0 { self.top.0 } else { (self.top.0 + 1).min(self.vis.len() - 1) };
    }

    /// The next (or previous) search hit from the cursor: the cursor
    /// goes there and its entry opens.
    pub(crate) fn jump(&mut self, forward: bool) {
        if self.hits.is_empty() {
            return;
        }
        let next = if forward {
            self.hits.iter().copied().find(|&k| k > self.sel).unwrap_or_else(|| {
                self.wrapped = "back to the first";
                self.hits[0]
            })
        } else {
            self.hits.iter().rev().copied().find(|&k| k < self.sel).unwrap_or_else(|| {
                self.wrapped = "back to the last";
                *self.hits.last().unwrap()
            })
        };
        self.show_hit(next);
    }

    /// The cursor on the shown entry `next`, opened, at the top.
    fn show_hit(&mut self, next: usize) {
        self.sel = next;
        let i = self.vis[next];
        if !self.is_open(i) && !matches!(self.items()[i].body, Body::None) {
            self.toggle(i);
        }
        self.top = (next, 0);
    }

    /// `/` opens the field; the cursor's place is kept for esc.
    pub(crate) fn start_search(&mut self) {
        self.origin = (self.sel, self.top);
        self.search = Some(Search { text: String::new(), typing: true });
        self.hits.clear();
    }

    /// The field changed: the cursor goes to the first match from where
    /// `/` opened (round the end if none after), or back there.
    pub(crate) fn incsearch(&mut self) {
        self.rehit();
        let (o, top) = self.origin;
        match self.hits.iter().copied().find(|&k| k >= o).or(self.hits.first().copied()) {
            Some(k) => (self.sel, self.top) = (k, (k, 0)),
            None => (self.sel, self.top) = (o.min(self.vis.len().saturating_sub(1)), top),
        }
    }

    /// ⏎ in the field: the cursor's match opens (the next one when the
    /// cursor is on none).
    pub(crate) fn search_done(&mut self) {
        if let Some(s) = self.search.as_mut() {
            s.typing = false;
        }
        if self.hits.contains(&self.sel) {
            self.show_hit(self.sel);
        } else {
            self.jump(true);
        }
    }

    /// esc in the field: no search, the cursor back where it was.
    pub(crate) fn search_cancel(&mut self) {
        self.search = None;
        self.hits.clear();
        let (o, top) = self.origin;
        (self.sel, self.top) = (o.min(self.vis.len().saturating_sub(1)), top);
    }

    /// A search is set (typed and ⏎): `n`/`N` go through it.
    fn searching(&self) -> bool {
        self.search.as_ref().is_some_and(|s| !s.typing && !s.text.is_empty())
    }

    /// `3 of 12` for the key bar, after the wrap note.
    fn hit_label(&self) -> String {
        if self.search.as_ref().is_none_or(|s| s.text.is_empty()) {
            return String::new();
        }
        if self.hits.is_empty() {
            return "no match".into();
        }
        let n = match self.hits.iter().position(|&k| k == self.sel) {
            Some(p) => format!("{} of {}", p + 1, self.hits.len()),
            None => format!("{} matches", self.hits.len()),
        };
        if self.wrapped.is_empty() {
            n
        } else {
            format!("{} {} {n}", self.wrapped, dot())
        }
    }
}

// ---- keys ----

/// Keys while the screen is open: it takes them all.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(v) = app.logview.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let width = app.area_w.saturating_sub(2).clamp(20, MEASURE);
    // the search field takes the keys while typed in
    // the wrap note lasts until the next key
    v.wrapped = "";
    if let Some(s) = v.search.as_mut().filter(|s| s.typing) {
        match k.code {
            KeyCode::Esc => v.search_cancel(),
            KeyCode::Enter => v.search_done(),
            KeyCode::Backspace => {
                s.text.pop();
                v.incsearch();
            }
            KeyCode::Char(c) if !ctrl => {
                s.text.push(c);
                v.incsearch();
            }
            _ => {}
        }
        return true;
    }
    if let Some(p) = v.picker.as_mut() {
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('f') | KeyCode::Char('q') => v.picker = None,
            KeyCode::Up | KeyCode::Char('k') => p.sel = p.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => p.sel = (p.sel + 1).min(p.rows.len().saturating_sub(1)),
            KeyCode::Char(' ') => {
                if let Some(r) = p.rows.get(p.sel).cloned() {
                    if !v.off.remove(&r) {
                        v.off.insert(r);
                    }
                    v.refilter();
                }
            }
            KeyCode::Char('a') => {
                v.off.clear();
                v.refilter();
            }
            _ => {}
        }
        return true;
    }
    let n = v.vis.len();
    let page = v.page.max(3);
    match k.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if v.search.is_some() {
                v.search = None;
                v.hits.clear();
            } else {
                app.logview = None;
            }
            return true;
        }
        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => {
            app.logview = None;
            return true;
        }
        KeyCode::Char('o') if ctrl => v.toggle_all(),
        KeyCode::Char('d') if ctrl => v.scroll(page as isize / 2, width),
        KeyCode::Char('u') if ctrl => v.scroll(-(page as isize / 2), width),
        KeyCode::Tab | KeyCode::BackTab => {
            let m = if v.mode == Mode::History { Mode::Model } else { Mode::History };
            // from the history, the model view opens on the cursor's request
            if m == Mode::Model {
                if let Some(&i) = v.vis.get(v.sel) {
                    let idx = v.history[i].idx;
                    if let Some(r) = v.requests.iter().position(|r| r.idx >= idx) {
                        v.req = r;
                    }
                }
            }
            v.set_mode(m);
        }
        KeyCode::Char('[') => v.step_request(-1),
        KeyCode::Char(']') => v.step_request(1),
        KeyCode::Up | KeyCode::Char('k') => v.sel = v.sel.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => v.sel = (v.sel + 1).min(n.saturating_sub(1)),
        KeyCode::PageDown => v.scroll(page as isize, width),
        KeyCode::PageUp => v.scroll(-(page as isize), width),
        KeyCode::Home | KeyCode::Char('g') => (v.sel, v.top) = (0, (0, 0)),
        KeyCode::End | KeyCode::Char('G') => v.bottom(),
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(&i) = v.vis.get(v.sel) {
                v.toggle(i);
            }
        }
        KeyCode::Char('/') => v.start_search(),
        KeyCode::Char('n') => v.jump(true),
        KeyCode::Char('N') => v.jump(false),
        KeyCode::Char('f') => {
            let mut rows: Vec<String> = model::ROLE_KEYS.iter().map(|s| s.to_string()).collect();
            rows.extend(v.tools().into_iter().map(|t| format!("tool:{t}")));
            v.picker = Some(Picker { rows, sel: 0 });
        }
        KeyCode::Char('r') => {
            // read the log again (it grew)
            let agent = v.agent.clone();
            match session_dir(&agent).and_then(|d| View::load(agent, &d)) {
                Ok(mut fresh) => {
                    fresh.off = std::mem::take(&mut v.off);
                    fresh.search = v.search.take();
                    fresh.refilter();
                    fresh.bottom();
                    *v = fresh;
                }
                Err(e) => v.note = e,
            }
        }
        _ => {}
    }
    true
}

/// The mouse while open: the wheel scrolls, the rest is swallowed.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let width = app.area_w.saturating_sub(2).clamp(20, MEASURE);
    let Some(v) = app.logview.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => v.scroll(-3, width),
        MouseEventKind::ScrollDown => v.scroll(3, width),
        _ => {}
    }
    true
}

// ---- the words and rows (pure) ----

/// The color of a kind of entry: its label and its rail (designer's
/// pick; /log's alone, the thread never colors by kind). The bodies stay
/// text and dim. Under `NO_COLOR`, none: the labels' glyphs tell.
pub(crate) fn kind_color(role: &Role) -> ratatui::style::Color {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return ratatui::style::Color::Reset;
    }
    match role {
        Role::You => theme::accent(),
        Role::Assistant => theme::text(),
        Role::Thinking => theme::dim(),
        Role::Call => theme::syntax_call(),
        Role::Result { ok: true } => theme::dim(),
        Role::Result { ok: false } | Role::Error => theme::error(),
        Role::Message => theme::syntax_keyword(),
        Role::Injected => theme::syntax_type(),
        Role::System | Role::Tools => theme::syntax_number(),
        Role::Summary => theme::syntax_string(),
        Role::Event => theme::faint(),
    }
}

/// The role column: glyph and word, and its color.
pub(crate) fn role_label(it: &Item, agent_word: &str) -> (String, Style) {
    let st = Style::default().fg(kind_color(&it.role));
    let g = theme::glyph;
    let w = match &it.role {
        Role::You => format!("{} you", g(theme::G_YOU)),
        Role::Assistant => format!("{} {agent_word}", g(theme::G_MAIN)),
        Role::Thinking => format!("{} thinking", g(theme::G_THINK)),
        Role::Call => match it.tool.as_str() {
            "bash" => format!("{} bash", g(theme::G_BASH)),
            "run_typescript" => format!("{} ts", g(theme::G_TS)),
            "edit" => format!("{} edit", g(theme::G_PATCH)),
            "write_file" => format!("{} write", g(theme::G_PATCH)),
            "apply_patch" => format!("{} patch", g(theme::G_PATCH)),
            "read_file" => "⇄ read".to_string(),
            t => t.to_string(),
        },
        Role::Result { ok: true } => format!("  {} result", if theme::ascii_mode() { "`" } else { "└" }),
        Role::Result { ok: false } => format!("  {} result", g(theme::G_FAILED)),
        Role::Message => format!("{} message", g(theme::G_MSG)),
        Role::Injected => "⊕ bise".into(),
        Role::System => "§ system".into(),
        Role::Tools => "§ tools".into(),
        Role::Summary => format!("{} summary", g(theme::G_SUMMARY)),
        Role::Error => format!("{} error", g(theme::G_INTERRUPTED)),
        Role::Event => "· event".into(),
    };
    (w, st)
}

/// The rail left of an open body, in its kind's color (none for events).
fn rail(role: &Role) -> Span<'static> {
    if *role == Role::Event {
        return Span::raw("    ");
    }
    let r = if theme::ascii_mode() { "  | " } else { "  ▎ " };
    Span::styled(r, Style::default().fg(kind_color(role)))
}

/// The widest an entry draws: a body's 4-column indent and the code
/// measure. A wider screen leaves the rest empty.
pub(crate) const MEASURE: usize = 4 + crate::render::CODE_MAX + 1;

/// The assistant's word in the role column: the agent's name when it
/// fits (`:* main`), else `agent`.
fn agent_word(name: &str) -> String {
    if name.chars().count() <= 7 { name.to_string() } else { "agent".into() }
}

fn pad(s: &str, w: usize) -> String {
    let sw = s.width();
    if sw >= w {
        crate::render::fit_chars(s, w)
    } else {
        format!("{s}{}", " ".repeat(w - sw))
    }
}

/// The right column of a header: the tokens (model view) or the body's
/// size (history).
fn size_label(it: &Item, mode: Mode) -> String {
    match (mode, it.tokens) {
        (Mode::Model, Some((n, est))) => format!("{}{} tok", if est { "~" } else { "" }, model::short_num(n)),
        _ if it.bytes() > 0 => model::short_bytes(it.bytes()),
        _ => String::new(),
    }
}

/// A rule's row: `── words ───…`.
fn rule_row(words: &str, width: usize, style: Style) -> Line<'static> {
    let h = if theme::ascii_mode() { "-" } else { "─" };
    let head = format!("{h}{h} {} ", words);
    let room = width.saturating_sub(head.width());
    Line::from(Span::styled(format!("{}{}", crate::render::fit_chars(&head, width), h.repeat(room)), style))
}

/// The rows of one entry at `width`: its header, then its body (open),
/// or its fold label.
pub(crate) fn entry_rows(it: &Item, width: usize, open: bool, mode: Mode, who: &str) -> Vec<Line<'static>> {
    let faint = Style::default().fg(theme::faint());
    if let Some(r) = &it.rule {
        let st = if r.contains("compacted") {
            Style::default().fg(kind_color(&Role::Summary))
        } else {
            faint
        };
        return vec![rule_row(r, width, st)];
    }
    let narrow = width < 80;
    let seq = if it.seq == 0 { String::new() } else { format!("#{}", it.seq) };
    let (role, rst) = role_label(it, who);
    let size = size_label(it, mode);
    let mut left = vec![Span::styled(format!("{:>7}  ", seq), faint)];
    if !narrow {
        left.push(Span::styled(format!("{:<8}  ", it.time), faint));
    }
    left.push(Span::styled(format!("{}  ", pad(&role, 10)), rst));
    let used: usize = left.iter().map(|s| s.content.width()).sum();
    let size = if narrow && width < 60 { String::new() } else { size };
    let room = width.saturating_sub(used + size.width() + if size.is_empty() { 0 } else { 2 });
    let head = crate::render::fit_chars(&it.head.replace(['\n', '\t'], " "), room);
    let fill = room.saturating_sub(head.width());
    left.push(Span::styled(head, Style::default().fg(theme::text())));
    if !size.is_empty() {
        left.push(Span::raw(" ".repeat(fill + 2)));
        left.push(Span::styled(size, faint));
    }
    let mut rows = vec![Line::from(left)];
    if matches!(it.body, Body::None) {
        return rows;
    }
    if !open {
        let t = it.body.text();
        rows.push(Line::from(Span::styled(
            format!(
                "    {} {} lines {} {}",
                theme::glyph("▸"),
                t.lines().count().max(1),
                if theme::ascii_mode() { "-" } else { "·" },
                model::short_bytes(t.len())
            ),
            faint,
        )));
        return rows;
    }
    let inner = body_width(width);
    if !it.meta.is_empty() {
        rows.push(Line::from(vec![rail(&it.role), Span::styled(it.meta.clone(), faint)]));
    }
    let body = body_rows(&it.body, inner, it.role == Role::Thinking);
    rows.extend(body.into_iter().map(|l| {
        let mut spans = vec![rail(&it.role)];
        spans.extend(l.spans);
        Line::from(spans)
    }));
    rows
}

/// The columns of a body under a `width` screen: indented 4.
fn body_width(width: usize) -> usize {
    width.saturating_sub(5).max(16)
}

/// A body's rows at `width`: markdown rendered, code colored in a box
/// (at most the thread's code measure), no blank rows at the end.
pub(crate) fn body_rows(b: &Body, width: usize, thinking: bool) -> Vec<Line<'static>> {
    let mut rows = body_rows_all(b, width, thinking);
    while rows.last().is_some_and(|l| l.spans.iter().all(|s| s.content.trim().is_empty())) {
        rows.pop();
    }
    rows
}

fn body_rows_all(b: &Body, width: usize, thinking: bool) -> Vec<Line<'static>> {
    let boxed = width.min(crate::render::CODE_MAX);
    match b {
        Body::None => Vec::new(),
        Body::Md(t) => {
            // prose at the prose measure, its tables and code to the code one
            let rows = crate::markdown::md_lines(t, width.min(crate::render::PROSE_MAX), width);
            if !thinking {
                return rows;
            }
            rows.into_iter()
                .map(|l| {
                    Line::from(
                        l.spans
                            .into_iter()
                            .map(|s| Span::styled(s.content, s.style.fg(theme::dim()).add_modifier(Modifier::ITALIC)))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        }
        Body::Code { lang, text } => code_box(lang, text, boxed),
        Body::Plain(t) => code_box("", t, boxed),
    }
}

/// `text` in a code box, its lines colored as `lang` (syntax.rs); a
/// plain box when `lang` is "" or unknown.
fn code_box(lang: &str, text: &str, width: usize) -> Vec<Line<'static>> {
    let text = crate::sanitize::clean(text, crate::sanitize::TAB_CODE);
    let l = crate::syntax::lang_of(lang);
    let mut state = crate::syntax::State::Normal;
    let mut hl: Vec<Vec<Span<'static>>> = Vec::new();
    for line in text.trim_end_matches('\n').split('\n') {
        let line = line.trim_end_matches('\r');
        let Some(l) = l else {
            hl.push(vec![Span::styled(line.to_string(), Style::default().fg(theme::text()))]);
            continue;
        };
        let (runs, next) = crate::syntax::line(l, state, line);
        state = next;
        let mut spans = Vec::new();
        let mut rest = line;
        for (n, t) in runs {
            let b = rest.char_indices().nth(n).map(|(b, _)| b).unwrap_or(rest.len());
            spans.push(Span::styled(rest[..b].to_string(), crate::syntax::style(t)));
            rest = &rest[b..];
        }
        if !rest.is_empty() {
            spans.push(Span::styled(rest.to_string(), Style::default().fg(theme::text())));
        }
        hl.push(spans);
    }
    crate::codeblock::lines(lang, &hl, String::new(), width)
}

/// `line` with each match of `needle` (lowercase ASCII) in the find
/// highlight.
fn mark(line: Line<'static>, needle: &str) -> Line<'static> {
    if needle.is_empty() {
        return line;
    }
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let low = text.to_ascii_lowercase();
    let ranges: Vec<(usize, usize)> = low.match_indices(needle).map(|(b, m)| (b, b + m.len())).collect();
    if ranges.is_empty() {
        return line;
    }
    let hit = Style::default().fg(theme::bg()).bg(theme::accent());
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut at = 0usize;
    for s in line.spans {
        let c = s.content.as_ref();
        let end = at + c.len();
        let mut cut = vec![at, end];
        for &(a, b) in &ranges {
            for x in [a, b] {
                if x > at && x < end && c.is_char_boundary(x - at) {
                    cut.push(x);
                }
            }
        }
        cut.sort();
        cut.dedup();
        for w in cut.windows(2) {
            let piece = &c[w[0] - at..w[1] - at];
            if piece.is_empty() {
                continue;
            }
            let inside = ranges.iter().any(|&(a, b)| w[0] >= a && w[1] <= b);
            out.push(Span::styled(piece.to_string(), if inside { s.style.patch(hit) } else { s.style }));
        }
        at = end;
    }
    Line::from(out)
}

/// The top bar: the two views, the active one in text; the filter.
fn top_rows(v: &View, width: usize) -> Vec<Line<'static>> {
    let on = Style::default().fg(theme::text()).add_modifier(Modifier::BOLD);
    let off = Style::default().fg(theme::dim());
    let faint = Style::default().fg(theme::faint());
    let (h, m) = if v.mode == Mode::History { (on, off) } else { (off, on) };
    // the requests by position, as the subtitle counts them
    let (req, m) = match v.requests.get(v.req) {
        Some(_) => (format!("what the model got {} request {} of {}", dot(), v.req + 1, v.requests.len()), m),
        None => (format!("what the model got {} no request yet", dot()), faint),
    };
    let mut first = vec![
        Span::styled(format!("log of {}   ", v.agent), faint),
        Span::styled("full history", h),
        Span::raw("   "),
        Span::styled(req, m),
    ];
    if !v.off.is_empty() {
        let mut keys: Vec<&String> = v.off.iter().collect();
        keys.sort();
        let keys: Vec<String> = keys.iter().map(|k| k.trim_start_matches("tool:").to_string()).collect();
        first.push(Span::styled(format!("   hidden: {}", keys.join(" ")), faint));
    }
    let mut rows = vec![Line::from(first)];
    let second = if v.mode == Mode::Model {
        match v.requests.get(v.req) {
            Some(r) => {
                let usage = r
                    .usage
                    .map(|(i, o, c)| {
                        format!(" {} {} in ({} cached) {} {} out", dot(), model::short_num(i + c), model::short_num(c), dot(), model::short_num(o))
                    })
                    .unwrap_or_default();
                let n = v.model.iter().filter(|i| i.rule.is_none() && !matches!(i.role, Role::System | Role::Tools)).count();
                format!(
                    "turn {} {} {} {} {}{usage} {} {}",
                    r.turn.unwrap_or(0),
                    dot(),
                    r.time,
                    dot(),
                    r.model,
                    dot(),
                    count(n, "entry", "entries")
                )
            }
            None => "no request in this log yet".into(),
        }
    } else {
        format!(
            "every entry of the session log, in order {} {} {} {}",
            dot(),
            count(v.history.iter().filter(|i| i.rule.is_none()).count(), "entry", "entries"),
            dot(),
            count(v.requests.len(), "request", "requests")
        )
    };
    rows.push(Line::from(Span::styled(crate::render::fit_chars(&second, width), faint)));
    if v.mode == Mode::Model {
        let how = if v.exact {
            "first: the exact body sent (BISE_DEBUG_REQUESTS); then the context rebuilt from the session log"
        } else {
            "rebuilt from the log. missing: each call's <bise_state> (BISE_DEBUG_REQUESTS=1 logs it)"
        };
        rows.push(Line::from(Span::styled(crate::render::fit_chars(how, width), faint)));
    }
    if !v.note.is_empty() {
        rows.push(Line::from(Span::styled(crate::render::fit_chars(&v.note, width), Style::default().fg(theme::error()))));
    }
    rows
}

/// `1 entry`, `2 entries`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

/// The key bar, or the search field while typed in.
fn key_row(v: &View, width: usize) -> Line<'static> {
    let dim = Style::default().fg(theme::dim());
    let text = Style::default().fg(theme::text());
    if let Some(s) = v.search.as_ref().filter(|s| s.typing) {
        let right = v.hit_label();
        let left = format!("/{}", s.text);
        let room = width.saturating_sub(left.width() + right.width() + 1);
        return Line::from(vec![
            Span::styled(left, text),
            Span::styled("█", Style::default().fg(theme::accent())),
            Span::raw(" ".repeat(room)),
            Span::styled(right, dim),
        ]);
    }
    let keys: &[(&str, &str)] = if v.picker.is_some() {
        &[("space", "show/hide"), ("a", "show all"), ("esc", "done")]
    } else if v.searching() {
        &[("/", "search"), ("n", "next"), ("N", "previous"), ("esc", "clear search")]
    } else if v.mode == Mode::Model {
        &[
            ("/", "search"),
            ("f", "filter"),
            ("[ ]", "request"),
            ("g", "top"),
            ("G", "bottom"),
            ("tab", "full history"),
            ("space", "open"),
            ("ctrl+o", "open all"),
            ("esc", "close"),
        ]
    } else {
        &[
            ("/", "search"),
            ("f", "filter"),
            ("g", "top"),
            ("G", "bottom"),
            ("tab", "what the model got"),
            ("space", "open"),
            ("ctrl+o", "open all"),
            ("esc", "close"),
        ]
    };
    let right = v.hit_label();
    let room = if right.is_empty() { width } else { width.saturating_sub(right.width() + 3) };
    let key_w = |(k, w): &(&str, &str)| k.width() + 1 + w.width();
    // narrow: drop keys from the right, never the first nor the last
    let mut keys: Vec<(&str, &str)> = keys.to_vec();
    let total = |ks: &[(&str, &str)]| ks.iter().map(key_w).sum::<usize>() + 3 * ks.len().saturating_sub(1);
    while keys.len() > 2 && total(&keys) > room {
        keys.remove(keys.len() - 2);
    }
    let mut spans = Vec::new();
    for (n, (k, w)) in keys.iter().enumerate() {
        spans.push(Span::styled(format!("{k} "), text));
        let gap = if n + 1 < keys.len() { "   " } else { "" };
        spans.push(Span::styled(format!("{w}{gap}"), dim));
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    if !right.is_empty() && used + right.width() < width {
        spans.push(Span::raw(" ".repeat(width - used - right.width())));
        spans.push(Span::styled(right, dim));
    }
    Line::from(spans)
}

/// The filter picker's rows (checkboxes like /approvals).
fn picker_rows(v: &View, p: &Picker) -> Vec<Line<'static>> {
    let mut rows = vec![Line::from(Span::styled("show", Style::default().fg(theme::text()).add_modifier(Modifier::BOLD)))];
    for (k, r) in p.rows.iter().enumerate() {
        let on = !v.off.contains(r);
        let mark = if on { "[x]" } else { "[ ]" };
        let cur = if k == p.sel { theme::glyph(theme::G_YOU) } else { " " };
        let word = r.strip_prefix("tool:").map(|t| format!("tool {t}")).unwrap_or_else(|| r.clone());
        let st = if k == p.sel { Style::default().fg(theme::text()) } else { Style::default().fg(theme::dim()) };
        rows.push(Line::from(Span::styled(format!("{cur} {mark} {word}"), st)));
    }
    rows
}

/// The screen: the top bar, the entries from `top`, the key bar.
pub(crate) fn lines(v: &mut View, width: usize, height: usize) -> Vec<Line<'static>> {
    // the entries at the measure; the bars across the screen
    let ew = width.min(MEASURE);
    if v.width != ew {
        v.width = ew;
        v.cache.clear();
    }
    let top = top_rows(v, width);
    let body_h = height.saturating_sub(top.len() + 2).max(1);
    v.page = body_h;
    v.reveal(ew, body_h);
    let needle = v.search.as_ref().map(|s| s.text.to_ascii_lowercase()).unwrap_or_default();
    let mut out = top;
    out.push(Line::from(""));
    let mut body: Vec<Line<'static>> = Vec::new();
    let mut k = v.top.0;
    let mut skip = v.top.1;
    while body.len() < body_h && k < v.vis.len() {
        let sel = k == v.sel;
        let rows = v.rows(k, ew).to_vec();
        for (r, mut l) in rows.into_iter().enumerate() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            if body.len() >= body_h {
                break;
            }
            if r == 0 && sel {
                // the cursor: its header bold, a mark in the first column
                if let Some(first) = l.spans.first_mut() {
                    let c = first.content.to_string();
                    let c = format!("{}{}", theme::glyph(theme::G_YOU), c.chars().skip(1).collect::<String>());
                    *first = Span::styled(c, Style::default().fg(theme::accent()));
                }
                l = Line::from(l.spans.into_iter().map(|s| Span::styled(s.content, s.style.add_modifier(Modifier::BOLD))).collect::<Vec<_>>());
            }
            body.push(mark(l, &needle));
        }
        k += 1;
    }
    if v.vis.is_empty() {
        body.push(Line::from(Span::styled("nothing to show: f changes the filter", Style::default().fg(theme::dim()))));
    }
    while body.len() < body_h {
        body.push(Line::from(""));
    }
    out.extend(body);
    out.push(key_row(v, width));
    out
}

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let Some(v) = app.logview.as_mut() else { return };
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 8 {
        return;
    }
    let area = Rect { x: full.x + 1, y: full.y, width: full.width - 2, height: full.height };
    let rows = lines(v, area.width as usize, area.height as usize);
    frame.render_widget(Paragraph::new(rows), area);
    if let Some(p) = v.picker.clone() {
        let rows = picker_rows(v, &p);
        let w = rows.iter().map(|l| l.width()).max().unwrap_or(10) as u16 + 4;
        let h = (rows.len() as u16 + 2).min(full.height.saturating_sub(2));
        let r = Rect { x: full.x + full.width.saturating_sub(w + 2), y: full.y + 3, width: w.min(full.width), height: h };
        frame.render_widget(Clear, r);
        let inner = Rect { x: r.x + 2, y: r.y + 1, width: r.width.saturating_sub(4), height: r.height.saturating_sub(2) };
        frame.render_widget(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(theme::faint())),
            r,
        );
        frame.render_widget(Paragraph::new(rows), inner);
    }
    crate::textlayer::text(area);
}
