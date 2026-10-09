//! ctrl+f: find in the history (BISE-237, book §16 "Find").
//!
//! The field is a box in the top-right corner of the history pane
//! (find_bar.rs: its place, its keys, its chevrons and ×): the composer
//! stays with its draft, the box has the keys until esc; every edit
//! searches again.
//! The whole thread is searched: once the events the feed holds are
//! scanned, the feed asks the hub for the page before them
//! (sb::feed::want_older), one at a time until the first line; a page
//! that comes in front shifts the counts and only its events are
//! scanned (the counter says `3/12+` meanwhile).
//! Long histories stay fast: the search never renders the history. It
//! reads an index of each event's raw text (lowered once, kept while
//! the field is open) and scans it in time slices (a few ms per frame),
//! your messages and the replies first, then the rest (tool calls: their
//! row, script and output; errors, cards). Only the rows on screen are
//! matched against the drawn text, for the highlight. Thinking is not
//! searched. A match hidden in a fold (a call's box, `▸ n commands`, a
//! report) opens it while it is the current match (feed::reveal).

use crate::app::App;
use crate::wire::{Ev, ToolData};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Scan time per frame: typing never waits on a long history.
pub(crate) const BUDGET: Duration = Duration::from_millis(6);
/// Each text of an event is indexed up to this many bytes (a huge
/// output is searched in its first 256 KB).
const FIELD_CAP: usize = 256 * 1024;
/// The newest events are checked again on every frame: a reply that
/// streams, a call whose output arrives.
const LIVE_TAIL: usize = 16;
/// How long `back to the newest` / `back to the oldest` shows.
const NOTE_FOR: Duration = Duration::from_millis(1500);


/// Which scan an event belongs to: your messages and the replies are
/// searched first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Message = 0,
    Other = 1,
}

/// The searchable text of one event, lowered (same bytes as the text:
/// [`lower`]), and the shape it was built from.
struct Entry {
    sig: u64,
    lower: String,
}

/// Where the current match is drawn: its row in its event, its columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Loc {
    pub(crate) ev: usize,
    pub(crate) row: usize,
    pub(crate) from: usize,
    pub(crate) to: usize,
    width: usize,
}

pub(crate) struct Find {
    /// the field: the composer's editing model (one text field
    /// behaviour: selection, word moves and deletes, undo)
    pub(crate) ed: crate::editor::Editor,
    /// the field's first shown char (a long query scrolls to its cursor)
    pub(crate) hscroll: usize,
    /// where the last frame drew the box's parts (clicks, drags)
    pub(crate) bar: Option<crate::find_bar::Parts>,
    /// a press in the field selects while the mouse drags
    pub(crate) dragging: bool,
    /// rows of the history the box covers (the view keeps the match
    /// under it)
    pub(crate) cover: usize,
    needle: String,
    sensitive: bool,
    /// the feed it searches (another agent's view closes it)
    focus: String,
    index: Vec<Option<Entry>>,
    /// matches per event, and their sum
    hits: Vec<u32>,
    total: usize,
    /// the scans, per tier: the events below this one are still to see
    /// (newest first; 0: done)
    next: [usize; 2],
    /// the events seen at the last step, and the first transcript
    /// position (a page of older lines moves every index)
    seen_len: usize,
    seen_first: Option<usize>,
    /// the current match: (event, its n-th match in the event's text)
    pub(crate) cur: Option<(usize, u32)>,
    pub(crate) loc: Option<Loc>,
    /// the view goes to the current match on the next frame
    jump: bool,
    undo: Vec<crate::feed::Undo>,
    note: Option<(&'static str, Instant)>,
}

impl Find {
    fn new(focus: &str, n: usize, first: Option<usize>) -> Find {
        Find {
            ed: crate::editor::Editor::default(),
            hscroll: 0,
            bar: None,
            dragging: false,
            // the box from the row over the history's first: 2 of its rows
            // (find_bar::draw says it from the first frame on)
            cover: crate::find_bar::BOX_H as usize - 1,
            needle: String::new(),
            sensitive: false,
            focus: focus.to_string(),
            index: Vec::new(),
            hits: vec![0; n],
            total: 0,
            next: [0, 0],
            seen_len: n,
            seen_first: first,
            cur: None,
            loc: None,
            jump: false,
            undo: Vec::new(),
            note: None,
        }
    }

    /// Still scanning: the loop draws again at once.
    pub(crate) fn busy(&self) -> bool {
        !self.needle.is_empty() && self.next != [0, 0]
    }

    /// The loaded part is all scanned for a query: the feed asks the hub
    /// for the page before it (sb::feed::want_older), until the whole
    /// thread is in.
    pub(crate) fn wants_older(&self) -> bool {
        !self.needle.is_empty() && self.next == [0, 0]
    }

    /// The feed it searches (the placeholder names it).
    pub(crate) fn focus_name(&self) -> &str {
        &self.focus
    }

    /// Some match found (the chevrons are live).
    pub(crate) fn has_matches(&self) -> bool {
        self.total > 0
    }

    /// A new query: every count again, from the newest.
    fn restart(&mut self) {
        let q = &self.ed.text;
        self.sensitive = q.chars().any(char::is_uppercase);
        self.needle = if self.sensitive { q.clone() } else { lower(q) };
        let n = self.hits.len();
        self.hits.iter_mut().for_each(|h| *h = 0);
        self.total = 0;
        self.next = [n, n];
        self.cur = None;
        self.loc = None;
        self.jump = false;
        self.note = None;
    }

    /// Count event `i` again (its entry rebuilt when it changed). The
    /// first match found becomes the current one: a message's always, the
    /// rest's once the messages are all seen.
    fn rescan(&mut self, events: &[Ev], i: usize) {
        let Some(ev) = events.get(i) else { return };
        let Some(tier) = tier_of(ev) else { return };
        if self.index.len() < events.len() {
            self.index.resize_with(events.len(), || None);
        }
        let s = sig(ev);
        if self.index[i].as_ref().is_none_or(|e| e.sig != s) {
            self.index[i] = Some(Entry { sig: s, lower: lower(&haystack(ev)) });
        }
        let lowered = &self.index[i].as_ref().map_or("", |e| e.lower.as_str());
        let count = if self.sensitive {
            // smart-case: the lowered text finds the candidates, the text
            // itself counts
            if lowered.contains(&lower(&self.needle)) {
                haystack(ev).matches(self.needle.as_str()).count()
            } else {
                0
            }
        } else {
            lowered.matches(self.needle.as_str()).count()
        } as u32;
        let old = std::mem::replace(&mut self.hits[i], count);
        self.total = self.total + count as usize - old as usize;
        match self.cur {
            Some((c, _)) if c == i && count == 0 => self.cur = None,
            Some((c, k)) if c == i && k >= count => self.cur = Some((i, count - 1)),
            None if count > 0 && (tier == Tier::Message || self.next[0] == 0) => {
                self.cur = Some((i, count - 1));
                self.jump = true;
            }
            _ => {}
        }
    }

    /// Scan for at most `budget`: the live tail, then the messages from
    /// the newest, then the rest.
    fn scan(&mut self, events: &[Ev], budget: Duration) {
        if self.needle.is_empty() {
            return;
        }
        let start = Instant::now();
        let n = events.len();
        for i in (n.saturating_sub(LIVE_TAIL)..n).rev() {
            self.rescan(events, i);
        }
        for t in 0..2 {
            let mut seen = 0u32;
            while self.next[t] > 0 {
                let i = self.next[t] - 1;
                self.next[t] = i;
                if tier_of(&events[i]).is_some_and(|x| x as usize == t) {
                    self.rescan(events, i);
                }
                seen += 1;
                if seen.is_multiple_of(64) && start.elapsed() >= budget {
                    return;
                }
            }
        }
    }

    /// Follow the feed: new events at the end are scanned; a page of
    /// older lines (or a cleared feed) starts the scan again.
    fn sync(&mut self, events: &[Ev], first: Option<usize>) {
        let n = events.len();
        if n == self.seen_len && first == self.seen_first {
            return;
        }
        if first != self.seen_first && n > self.seen_len {
            // older lines came in front: every index moves by `d`
            let d = n - self.seen_len;
            let mut idx: Vec<Option<Entry>> = (0..d).map(|_| None).collect();
            idx.append(&mut self.index);
            self.index = idx;
            // the counts so far hold, moved by `d`; only the new events
            // in front are left to scan (every page of a long thread is
            // scanned once, never the whole feed again)
            let mut hits = vec![0; d];
            hits.append(&mut self.hits);
            self.hits = hits;
            self.cur = self.cur.map(|(i, k)| (i + d, k));
            self.loc = None;
            crate::feed::shift_undo(&mut self.undo, d);
            if !self.needle.is_empty() {
                self.next = [self.next[0] + d, self.next[1] + d];
            }
        } else if n > self.seen_len {
            // new lines at the end: seen now, the scans go on below
            self.hits.resize(n, 0);
            for i in (self.seen_len..n).rev() {
                self.rescan(events, i);
            }
        } else if n == self.seen_len && first < self.seen_first {
            // a page of older lines that made no event (the thread's
            // first lines): every index holds
        } else {
            // cleared or cut: nothing of the old indexes holds
            self.index.clear();
            self.hits = vec![0; n];
            self.undo.clear();
            self.restart();
        }
        self.seen_len = n;
        self.seen_first = first;
    }

    /// The current match's rank, oldest first (1-based).
    fn rank(&self) -> Option<usize> {
        let (i, k) = self.cur?;
        Some(self.hits[..i.min(self.hits.len())].iter().map(|&h| h as usize).sum::<usize>() + k as usize + 1)
    }

    /// Up the history (older); past the oldest, back to the newest.
    pub(crate) fn older(&mut self) {
        let newest = || self.hits.iter().rposition(|&h| h > 0);
        let to = match self.cur {
            Some((i, k)) if k > 0 => Some((i, k - 1)),
            Some((i, _)) => match (0..i).rev().find(|&j| self.hits[j] > 0) {
                Some(j) => Some((j, self.hits[j] - 1)),
                None => {
                    self.note = Some(("back to the newest", Instant::now()));
                    newest().map(|j| (j, self.hits[j] - 1))
                }
            },
            None => newest().map(|j| (j, self.hits[j] - 1)),
        };
        if to.is_some() {
            self.cur = to;
            self.jump = true;
        }
    }

    /// Down the history (newer); past the newest, back to the oldest.
    pub(crate) fn newer(&mut self) {
        let n = self.hits.len();
        let to = match self.cur {
            Some((i, k)) if k + 1 < self.hits[i] => Some((i, k + 1)),
            Some((i, _)) => match (i + 1..n).find(|&j| self.hits[j] > 0) {
                Some(j) => Some((j, 0)),
                None => {
                    self.note = Some(("back to the oldest", Instant::now()));
                    self.hits.iter().position(|&h| h > 0).map(|j| (j, 0))
                }
            },
            None => self.hits.iter().rposition(|&h| h > 0).map(|j| (j, self.hits[j] - 1)),
        };
        if to.is_some() {
            self.cur = to;
            self.jump = true;
        }
    }

    /// The counter, right of the field: `3/12` (`3/12+` while older
    /// lines are not loaded), `no match`, or a fresh wrap note.
    pub(crate) fn counter(&self, more: bool, now: Instant) -> String {
        if let Some((t, at)) = self.note {
            if now.saturating_duration_since(at) < NOTE_FOR {
                return t.to_string();
            }
        }
        if self.needle.is_empty() {
            return String::new();
        }
        if self.total == 0 {
            return if self.busy() { String::new() } else { "no match".into() };
        }
        let plus = if more { "+" } else { "" };
        match self.rank() {
            Some(r) => format!("{}/{}{}", r, self.total, plus),
            None => format!("{}{}", self.total, plus),
        }
    }

    /// What the feed needs to paint the rows it shows.
    pub(crate) fn marker(&self) -> Option<Marker<'_>> {
        (!self.needle.is_empty()).then(|| Marker { needle: &self.needle, sensitive: self.sensitive, cur: self.loc })
    }
}

/// Paints the matches of the rows on screen.
pub(crate) struct Marker<'a> {
    needle: &'a str,
    sensitive: bool,
    cur: Option<Loc>,
}

impl Marker<'_> {
    /// Row `row` of event `ev` with its matches painted: the pill tint
    /// (NO_COLOR: underlined); the current one on the accent, bold
    /// (NO_COLOR: reversed).
    pub(crate) fn paint(&self, line: Line<'static>, ev: usize, row: usize) -> Line<'static> {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let ranges = matches_in(&text, self.needle, self.sensitive);
        if ranges.is_empty() {
            return line;
        }
        let cur = self.cur.filter(|l| l.ev == ev && l.row == row).map(|l| l.from);
        let plain = no_color();
        let other = |st: Style| {
            if plain {
                st.add_modifier(Modifier::UNDERLINED)
            } else {
                st.bg(crate::theme::pill_bg())
            }
        };
        let current = |st: Style| {
            if plain {
                st.add_modifier(Modifier::REVERSED)
            } else {
                st.fg(crate::theme::bg()).bg(crate::theme::accent()).add_modifier(Modifier::BOLD)
            }
        };
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        for sp in &line.spans {
            for g in sp.content.graphemes(true) {
                let st = match ranges.iter().find(|&&(a, b)| col >= a && col < b) {
                    Some(&(a, _)) if Some(a) == cur => current(sp.style),
                    Some(_) => other(sp.style),
                    None => sp.style,
                };
                match spans.last_mut() {
                    Some(l) if l.style == st => l.content.to_mut().push_str(g),
                    _ => spans.push(Span::styled(g.to_string(), st)),
                }
                col += g.width();
            }
        }
        let mut out = Line::from(spans);
        out.style = line.style;
        out.alignment = line.alignment;
        out
    }
}

pub(crate) fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// `s` lowered char by char, keeping its bytes where they are: a char
/// whose lowercase is longer (or not one char) stays as it is, so an
/// offset in the lowered text is the same offset in `s`.
pub(crate) fn lower(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) if x.len_utf8() == c.len_utf8() => x,
                _ => c,
            }
        })
        .collect()
}

/// The columns of each match of `needle` in the drawn `text`.
pub(crate) fn matches_in(text: &str, needle: &str, sensitive: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let hay = if sensitive { std::borrow::Cow::Borrowed(text) } else { std::borrow::Cow::Owned(lower(text)) };
    let found: Vec<(usize, usize)> = hay.match_indices(needle).map(|(b, m)| (b, b + m.len())).collect();
    if found.is_empty() {
        return Vec::new();
    }
    // the column where each grapheme starts, by byte
    let mut starts: Vec<(usize, usize)> = Vec::new();
    let mut col = 0usize;
    for (b, g) in text.grapheme_indices(true) {
        starts.push((b, col));
        col += g.width();
    }
    let col_at = |b: usize| match starts.binary_search_by_key(&b, |&(x, _)| x) {
        Ok(i) => starts[i].1,
        Err(0) => 0,
        Err(i) if i >= starts.len() && b >= text.len() => col,
        Err(i) => starts[i - 1].1,
    };
    found.into_iter().map(|(a, b)| (col_at(a), col_at(b).max(col_at(a) + 1))).collect()
}

fn tier_of(ev: &Ev) -> Option<Tier> {
    match ev {
        Ev::You(..) | Ev::Assistant(_) | Ev::AgentMsg { .. } | Ev::Answered { .. } => Some(Tier::Message),
        Ev::Tool(_)
        | Ev::Err(_)
        | Ev::Warn(_)
        | Ev::Info(_)
        | Ev::Pr { .. }
        | Ev::Card { .. }
        | Ev::Compacted { .. }
        | Ev::Fold { .. }
        | Ev::Scheduled { .. }
        | Ev::Undelivered { .. } => Some(Tier::Other),
        _ => None,
    }
}

fn cap(s: &str) -> &str {
    if s.len() <= FIELD_CAP {
        return s;
    }
    let mut e = FIELD_CAP;
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    &s[..e]
}

/// The shape of an event's text: its variant and the lengths of its
/// texts (a reply that streams, an output that arrives change it).
fn sig(ev: &Ev) -> u64 {
    let mix = |h: u64, x: usize| (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
    let lens: [usize; 5] = match ev {
        Ev::You(t, ..) | Ev::Assistant(t) | Ev::Err(t) | Ev::Warn(t) | Ev::Info(t) => [1, t.len(), 0, 0, 0],
        Ev::AgentMsg { text, .. } => [2, text.len(), 0, 0, 0],
        Ev::Answered { question, answer, why, .. } => [3, question.len(), answer.len(), why.len(), 0],
        Ev::Tool(td) => [
            4,
            td.name.as_ref().map_or(0, |s| s.len() + 1),
            td.args.as_ref().map_or(0, |s| s.len() + 1) + td.code.as_ref().map_or(0, |s| s.len() + 1),
            td.intent.as_ref().map_or(0, |s| s.len() + 1),
            td.result.as_ref().map_or(0, |(_, r)| r.len() + 1),
        ],
        Ev::Card { card, .. } => [5, card.question.len(), card.options.len(), 0, 0],
        Ev::Compacted { text, .. } | Ev::Undelivered { text, .. } => [5, text.len(), 0, 0, 0],
        Ev::Fold { head, text, open } => [6, head.len(), text.len(), *open as usize, 0],
        Ev::Scheduled { head, words, open } => [8, head.len(), words.len(), *open as usize, 0],
        Ev::Pr { text, .. } => [7, text.len(), 0, 0, 0],
        _ => [0; 5],
    };
    lens.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| mix(h, x))
}

/// The text find searches in an event: what its row shows and what its
/// folds hide; one text per line (a match never spans two).
fn haystack(ev: &Ev) -> String {
    let parts: Vec<&str> = match ev {
        Ev::You(t, ..) | Ev::Assistant(t) | Ev::Err(t) | Ev::Warn(t) | Ev::Info(t) => vec![cap(t)],
        Ev::AgentMsg { text, .. } => vec![cap(text)],
        Ev::Answered { question, answer, why, .. } => vec![cap(question), cap(answer), cap(why)],
        Ev::Card { card, .. } => return cap(&card.text()).to_string(),
        Ev::Compacted { text, .. } | Ev::Undelivered { text, .. } => vec![cap(text)],
        Ev::Fold { head, text, .. } | Ev::Scheduled { head, words: text, .. } => vec![cap(head), cap(text)],
        Ev::Pr { number, text, .. } => return format!("#{} {}", number, cap(text)),
        Ev::Tool(td) => return tool_text(td),
        _ => Vec::new(),
    };
    parts.join("\n")
}

fn tool_text(td: &ToolData) -> String {
    let (name, args, code) = crate::render::tool_meta(td);
    let mut out = String::new();
    for p in [td.intent.as_deref().unwrap_or(""), &name, &args, code.as_ref().map_or("", |(_, c)| c.as_str())] {
        out.push_str(cap(p));
        out.push('\n');
    }
    if let Some((_, r)) = &td.result {
        out.push_str(cap(r));
    }
    out
}

// ---- the app side ----

/// Whether older lines of this feed are not loaded yet (the counter
/// says `3/12+` while find pages them in).
pub(crate) fn more_before(app: &App) -> bool {
    app.win.first_pos().is_some_and(|p| p > 1)
}

pub(crate) fn open(app: &mut App) {
    let f = Find::new(app.sb.focus_name(), app.events.len(), app.win.first_pos());
    app.find = Some(f);
}

/// Close the field: what it opened closes, the view stays on the match.
pub(crate) fn close(app: &mut App) {
    if let Some(f) = app.find.take() {
        crate::feed::unreveal(&mut app.events, &mut app.cache, f.undo);
    }
}

/// The query changed: what the old match opened closes, the scan starts.
pub(crate) fn edited(app: &mut App) {
    let Some(f) = app.find.as_mut() else { return };
    let undo = std::mem::take(&mut f.undo);
    f.restart();
    crate::feed::unreveal(&mut app.events, &mut app.cache, undo);
}

/// The frame's share of the work, before the feed is drawn at width
/// `width`: follow the feed, scan for `budget`, move the view to a new
/// current match (opening what hides it).
pub(crate) fn step(app: &mut App, width: usize, budget: Duration) {
    let Some(f) = app.find.as_mut() else { return };
    if f.focus != app.sb.focus_name() {
        // another feed: what was opened belongs to the one we left
        app.find = None;
        return;
    }
    // a page came in front: the last frame's rows (vis_events) name
    // other events now, they say nothing of what is on screen
    let moved = f.seen_first != app.win.first_pos();
    f.sync(&app.events, app.win.first_pos());
    f.scan(&app.events, budget);
    let Some((i, k)) = f.cur else {
        f.loc = None;
        return;
    };
    let jump = std::mem::take(&mut f.jump);
    if !jump && f.loc.is_some_and(|l| l.ev == i && l.width == width) {
        return;
    }
    let (needle, sensitive) = (f.needle.clone(), f.sensitive);
    let mut revealed = false;
    if jump {
        let undo = std::mem::take(&mut f.undo);
        crate::feed::unreveal(&mut app.events, &mut app.cache, undo);
    }
    let mut spots = spots_of(app, i, width, &needle, sensitive);
    if jump && spots.len() <= k as usize {
        // the match is in a fold: open it while it is the current one
        let undo = crate::feed::reveal(&mut app.events, &mut app.cache, i);
        revealed = !undo.is_empty();
        if let Some(f) = app.find.as_mut() {
            f.undo = undo;
        }
        spots = spots_of(app, i, width, &needle, sensitive);
    }
    let (row, from, to) = spots.get(k as usize).or(spots.last()).copied().unwrap_or((0, 0, 0));
    if let Some(f) = app.find.as_mut() {
        f.loc = Some(Loc { ev: i, row, from, to, width });
    }
    if !jump {
        return;
    }
    // already on screen, below the box (nothing opened above it): the
    // view stays
    let cover = app.find.as_ref().map_or(0, |f| f.cover);
    let shown = !moved && app.vis_events.iter().zip(&app.vis_rows).skip(cover).any(|(&e, &r)| e == i && r == row);
    if shown && !revealed {
        return;
    }
    app.follow = false;
    app.anchor = (i, row);
    // the match lands under the box, 1 blank row between (designer)
    app.scroll = -(cover as isize + 1);
}

/// Every match in the drawn rows of event `i`: (row, from, to).
fn spots_of(app: &mut App, i: usize, width: usize, needle: &str, sensitive: bool) -> Vec<(usize, usize, usize)> {
    crate::feed::ensure_rows(&app.events, &mut app.cache, i, app.debug, width, app.tick);
    let Some(er) = app.cache.get(i).and_then(|c| c.as_ref()) else { return Vec::new() };
    let mut out = Vec::new();
    for (r, line) in er.rows.iter().enumerate() {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        out.extend(matches_in(&text, needle, sensitive).into_iter().map(|(a, b)| (r, a, b)));
    }
    out
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
