//! One feed of the switchboard: its events and view state (`View`),
//! swapped into the `App` fields while in focus, and the bounded window
//! of the transcript it holds (trimmed at the tail, paged back from the
//! hub).

use super::*;

/// Events a feed keeps while it follows its tail; past it, the oldest
/// go (down to `KEEP_EVENTS`) and come back from the hub by pages when
/// the user scrolls up to them.
pub(super) const MAX_EVENTS: usize = 3000;
pub(super) const KEEP_EVENTS: usize = 2000;
/// A page is asked when the view gets this close to its first event.
pub(super) const PAGE_AHEAD: usize = 100;

/// Which part of an agent's transcript a feed holds.
#[derive(Default)]
pub(crate) struct FeedWindow {
    /// (event index, transcript position) of each line that added an
    /// event: where the feed can be cut, and where a page starts.
    pub(super) marks: std::collections::VecDeque<(usize, usize)>,
    /// The position of the oldest line taken in (None: a hub without
    /// positions, nothing to page).
    pub(super) first_pos: Option<usize>,
    /// A page was asked and has not arrived.
    pub(super) loading: bool,
    /// The position of the newest line taken in.
    pub(super) last_pos: Option<usize>,
    /// Each entry's pos and how many events it drew (sb/feed_entries.rs:
    /// a changed entry replaces them).
    pub(super) spans: std::collections::BTreeMap<usize, usize>,
}

impl FeedWindow {
    /// The position of the oldest line taken in (find: a page of older
    /// lines moved every index; more than 1: older lines are not loaded).
    pub(crate) fn first_pos(&self) -> Option<usize> {
        self.first_pos
    }
}

/// Everything that belongs to one feed.
pub(super) struct View {
    pub(super) events: Vec<Ev>,
    pub(super) cache: Vec<Option<EventRows>>,
    pub(super) win: FeedWindow,
    pub(super) follow: bool,
    pub(super) anchor: (usize, usize),
    pub(super) scroll: isize,
    pub(super) unseen: usize,
    pub(super) tail_visible: bool,
    pub(super) pending: bool,
    pub(super) interrupt_requested: bool,
    pub(super) last_line_at: u64,
    pub(super) last_ts: Option<u64>,
    /// the agent's composer draft, kept while another is in focus
    pub(super) ed: crate::editor::Editor,
    /// its queued messages (BISE-89)
    pub(super) queued: Vec<crate::queue::Queued>,
    /// its queued message sent, its turn not started (queue.rs)
    pub(super) queue_out: Option<std::time::Instant>,
}

impl View {
    pub(super) fn new() -> View {
        View {
            events: Vec::new(),
            cache: Vec::new(),
            win: FeedWindow::default(),
            follow: true,
            anchor: (0, 0),
            scroll: 0,
            unseen: 0,
            tail_visible: true,
            pending: false,
            interrupt_requested: false,
            last_line_at: 0,
            last_ts: None,
            ed: crate::editor::Editor::default(),
            queued: Vec::new(),
            queue_out: None,
        }
    }
}

pub(super) fn swap_feed(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.events, &mut v.events);
    std::mem::swap(&mut app.cache, &mut v.cache);
    std::mem::swap(&mut app.win, &mut v.win);
    std::mem::swap(&mut app.follow, &mut v.follow);
    std::mem::swap(&mut app.anchor, &mut v.anchor);
    std::mem::swap(&mut app.scroll, &mut v.scroll);
    std::mem::swap(&mut app.unseen, &mut v.unseen);
    std::mem::swap(&mut app.tail_visible, &mut v.tail_visible);
    std::mem::swap(&mut app.pending, &mut v.pending);
    std::mem::swap(&mut app.interrupt_requested, &mut v.interrupt_requested);
    std::mem::swap(&mut app.last_line_at, &mut v.last_line_at);
    std::mem::swap(&mut app.last_ts, &mut v.last_ts);
    std::mem::swap(&mut app.queued, &mut v.queued);
    std::mem::swap(&mut app.queue_out, &mut v.queue_out);
}

pub(super) fn swap_draft(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.ed, &mut v.ed);
}

/// Run `f` on the feed of `agent`, swapped into the `App` fields when it
/// is not the one in focus.
pub(super) fn with_feed(app: &mut App, agent: &str, f: impl FnOnce(&mut App)) {
    let sb = &mut app.sb;
    if sb.focus == agent {
        f(app);
        return;
    }
    let mut view = sb.views.remove(agent).unwrap_or_else(View::new);
    swap_feed(app, &mut view);
    f(app);
    swap_feed(app, &mut view);
    let sb = &mut app.sb;
    sb.views.insert(agent.to_string(), view);
}

/// One line of the feed, at transcript position `pos`, written at `ts`
/// (ms since the epoch; None: a hub that does not say). Tests only since
/// P4d-feed: the feeds come from the hub's entries (sb/feed_entries.rs).
#[cfg(test)]
pub(super) fn ingest_at(app: &mut App, line: String, pos: Option<usize>, ts: Option<u64>) {
    let n0 = app.events.len();
    ingest_line(app, line, ts);
    seen_at(app, pos, n0);
}

/// Line `pos` of the transcript was read, its events from `n0`: the
/// window's ends and marks (a line that drew nothing moves the ends
/// only).
pub(super) fn seen_at(app: &mut App, pos: Option<usize>, n0: usize) {
    if let Some(p) = pos {
        app.win.first_pos.get_or_insert(p);
        app.win.last_pos = Some(app.win.last_pos.map_or(p, |l| l.max(p)));
        if app.events.len() > n0 {
            app.win.marks.push_back((n0, p));
        }
    }
}

/// A feed that follows its tail keeps its last events only: the oldest
/// go, cut at a line boundary (they come back by pages).
pub(super) fn trim_window(app: &mut App) {
    // an open find keeps the pages it brought in (its counts and its
    // matches index them)
    if !app.follow || app.find.is_some() || app.events.len() <= MAX_EVENTS {
        return;
    }
    let want = app.events.len() - KEEP_EVENTS;
    let Some(&(k, pos)) = app.win.marks.iter().find(|(i, _)| *i >= want) else {
        return;
    };
    app.events.drain(..k);
    let c = k.min(app.cache.len());
    app.cache.drain(..c);
    if let Some(first) = app.cache.first_mut() {
        // its breathing gap depended on the event before it
        *first = None;
    }
    while app.win.marks.front().is_some_and(|(i, _)| *i < k) {
        app.win.marks.pop_front();
    }
    for m in app.win.marks.iter_mut() {
        m.0 -= k;
    }
    app.win.first_pos = Some(pos);
    app.win.spans = app.win.spans.split_off(&pos);
    app.anchor.0 = app.anchor.0.saturating_sub(k);
}

/// `/clear` and Ctrl+L: the feed in focus shows nothing, like a
/// terminal clear. Nothing is lost: the transcript keeps every line (and
/// the agent its context); the feed now starts after the last line it
/// took in, so scrolling up pages the cleared lines back from the hub,
/// in their order.
pub(super) fn clear_feed(app: &mut App) {
    empty_feed(app);
    app.win.marks.clear();
    app.win.spans.clear();
    app.win.loading = false;
    if let Some(p) = app.win.last_pos {
        app.win.first_pos = Some(p + 1);
    }
}

/// The feed in focus holds nothing and follows the tail: no event, no
/// cached row, no scroll, no unseen count, no selection. The one place
/// that empties it (`clear_feed`, a hub reconnection): a new per-feed
/// field is reset here.
pub(super) fn empty_feed(app: &mut App) {
    app.events.clear();
    app.cache.clear();
    app.anchor = (0, 0);
    app.scroll = 0;
    app.follow = true;
    app.unseen = 0;
    app.feed_sel = None;
}

/// The view came close to the first event it holds, or an open find has
/// scanned all it holds: ask the hub for the lines before it (one page
/// at a time).
pub(super) fn want_older(app: &mut App) {
    // find searches the whole thread: once what is loaded is scanned,
    // the page before it, until the first line (find.rs)
    let finding = app.find.as_ref().is_some_and(|f| f.wants_older());
    if app.win.loading || (!finding && (app.follow || app.anchor.0 >= PAGE_AHEAD)) {
        return;
    }
    let Some(before) = app.win.first_pos.filter(|p| *p > 1) else { return };
    super::feed_entries::want_older(app, before);
}

