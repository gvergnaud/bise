//! The feeds from the hub's entries (client-protocol step 4, P4d; plan
//! v2 signed by architect m_13977): the terminal subscribes the threads
//! it shows (the focus, every agent whose feed it keeps, voice mode's)
//! with `thread/subscribe`, pages older ones with `thread/page`, and
//! reads `thread/entry` notifications; each entry's feed events are
//! `entry_ev::ev_of`'s, placed by the entry's pos: a new pos appends, a
//! known one (the hub sends a changed entry again, at its pos) replaces
//! its events in place. The window keeps `(event index, entry pos)` marks
//! as it kept line positions, so trim, pages, `/clear` and find work as
//! before. Turn edges are no entries: the agents rows say them
//! ([`turn_edge`]).
//!
//! Staged: this sha brings the module and its typed readers (the
//! `thread/entry` notification, `Then::Thread`); the switch (the hello's
//! `reads` lists `thread/entry`, the `line`/`history` arms and
//! `ingest_for` go, focus and reconnection subscribe) comes with
//! proto-reads' `entry_reads` (the text reads `ingest_for` still does).
// TODO(client-protocol P4d switch): drop this allow when the switch calls
// subscribe/on_connect/want_older and the state reader calls turn_edge
#![allow(dead_code)]

use super::*;
use super::feed::{empty_feed, seen_at};
use crate::entry_ev::ev_of;
use bise_proto::thread::Entry;

/// Entries asked per page (the first one and the older ones).
pub(super) const PAGE_ENTRIES: u32 = 200;

/// Subscribe `agent`'s thread, once per connection: its first page
/// replaces its feed, its changes come as `thread/entry`.
pub(super) fn subscribe(app: &mut App, agent: &str) {
    let sb = &mut app.sb;
    if agent.is_empty() || !sb.subscribed.insert(agent.to_string()) {
        return;
    }
    sb.call("thread/subscribe", json!({"agent": agent, "limit": PAGE_ENTRIES}), rpc::Then::Thread(agent.to_string(), None));
}

/// The threads to subscribe after a (re)connection: the focus, and voice
/// mode's agent.
pub(super) fn on_connect(app: &mut App) {
    app.sb.subscribed.clear();
    let focus = app.sb.focus.clone();
    subscribe(app, &focus);
}

/// `thread/subscribe`'s answer (`older` None: the first page, it replaces
/// the feed) or `thread/page`'s (`older`: the `before` it asked).
pub(super) fn page(app: &mut App, agent: &str, older: Option<usize>, entries: Vec<Entry>, more: bool) {
    with_feed(app, agent, |app| match older {
        None => first_page(app, agent, &entries, more),
        Some(before) => older_page(app, agent, before, &entries, more),
    });
    let live = false;
    for e in &entries {
        reads(app, agent, e, live);
    }
    if older.is_none() && app.sb.focus == agent && std::mem::take(&mut app.sb.keep_after_page) {
        keep::apply(app);
    }
}

fn first_page(app: &mut App, agent: &str, entries: &[Entry], more: bool) {
    let keep = (app.ed.clone(), app.queued.clone());
    empty_feed(app);
    app.win = FeedWindow::default();
    app.last_ts = None;
    for e in entries {
        place(app, agent, e);
    }
    app.win.first_pos = Some(first_pos(entries, more));
    (app.ed, app.queued) = keep;
}

/// The oldest pos the feed holds; 1 when nothing is before it (no page
/// to ask).
fn first_pos(entries: &[Entry], more: bool) -> usize {
    match (entries.first(), more) {
        (Some(e), true) => e.pos as usize,
        _ => 1,
    }
}

/// A page of older entries: its events go in front of the feed, the view
/// stays on the rows it shows (as `feed::prepend_page` did for lines).
fn older_page(app: &mut App, agent: &str, before: usize, entries: &[Entry], more: bool) {
    if app.win.first_pos != Some(before) {
        // the feed changed since the ask (cut, reconnection): stale
        app.win.loading = false;
        return;
    }
    let events = std::mem::take(&mut app.events);
    let cache = std::mem::take(&mut app.cache);
    let marks = std::mem::take(&mut app.win.marks);
    let kept = (app.follow, app.unseen, app.last_ts);
    app.follow = true;
    app.last_ts = None;
    for e in entries {
        place(app, agent, e);
    }
    let k = app.events.len();
    app.cache.resize_with(k, || None);
    app.events.extend(events);
    app.cache.extend(cache);
    let mut shift = 0;
    if let Some(Some(old)) = app.cache.get(k) {
        let rows = event_rows(&app.events, k, app.debug, old.width as usize, app.tick);
        shift = rows.rows.len().saturating_sub(old.rows.len());
        app.cache[k] = Some(rows);
    }
    app.win.marks.extend(marks.into_iter().map(|(i, p)| (i + k, p)));
    (app.follow, app.unseen, app.last_ts) = kept;
    if app.anchor.0 == 0 {
        app.anchor.1 += shift;
    }
    app.anchor.0 += k;
    app.win.first_pos = Some(first_pos(entries, more));
    app.win.loading = false;
}

/// One `thread/entry`: new or changed, live.
pub(super) fn entry(app: &mut App, agent: &str, e: &Entry) {
    if !app.sb.subscribed.contains(agent) {
        return;
    }
    with_feed(app, agent, |app| {
        let n0 = app.events.len();
        place(app, agent, e);
        if app.events.len() > n0 && !app.follow {
            app.unseen += 1;
        }
        trim_window(app);
    });
    reads(app, agent, e, true);
}

/// Entry `e` in the feed at its pos: a known pos's events are replaced in
/// place, a new one's appended (after a time mark when its time follows a
/// pause, as for lines). His answer to an item he gave here draws
/// nothing (`entry_reads::skip`: its fold is in this feed already), only
/// its pos's mark.
fn place(app: &mut App, agent: &str, e: &Entry) {
    let pos = e.pos as usize;
    if crate::entry_reads::skip(app, agent, e) {
        if !app.win.spans.contains_key(&pos) {
            let n0 = app.events.len();
            app.win.spans.insert(pos, 0);
            seen_at(app, Some(pos), n0);
        }
        return;
    }
    let mut evs = ev_of(e);
    for ev in evs.iter_mut() {
        if let Ev::Thinking { open, .. } = ev {
            *open = app.show_thinking;
        }
    }
    if let Some(&n) = app.win.spans.get(&pos) {
        return replace(app, pos, n, evs);
    }
    if e.at_ms > 0 {
        if let Some(prev) = app.last_ts {
            let gap = u128::from(e.at_ms.saturating_sub(prev));
            crate::feed::pause_mark(&mut app.events, &mut app.cache, gap, || crate::when::mark_now(e.at_ms));
        }
        app.last_ts = Some(e.at_ms);
    }
    let n0 = app.events.len();
    for ev in evs {
        push_event(&mut app.events, &mut app.cache, ev);
    }
    let n = app.events.len() - n0;
    app.win.spans.insert(pos, n);
    seen_at(app, Some(pos), n0);
}

/// The `n` events of entry `pos` become `evs`: spliced in place, the rows
/// around drawn again, the marks after it moved.
fn replace(app: &mut App, pos: usize, n: usize, evs: Vec<Ev>) {
    let at = match app.win.marks.iter().find(|(_, p)| *p == pos) {
        Some(&(i, _)) => i,
        // it drew nothing before: before the first entry after it
        None => app.win.marks.iter().find(|(_, p)| *p > pos).map_or(app.events.len(), |&(i, _)| i),
    };
    let end = (at + n).min(app.events.len());
    let k = evs.len();
    app.events.splice(at..end, evs);
    app.cache.splice(at..end.min(app.cache.len()).max(at.min(app.cache.len())), std::iter::repeat_with(|| None).take(k));
    for i in [at.checked_sub(1), Some(at + k)].into_iter().flatten() {
        if let Some(c) = app.cache.get_mut(i) {
            *c = None;
        }
    }
    let delta = k as isize - (end - at) as isize;
    for m in app.win.marks.iter_mut().filter(|(i, p)| *i > at || (*i == at && *p != pos)) {
        m.0 = (m.0 as isize + delta).max(0) as usize;
    }
    let has = app.win.marks.iter().position(|(_, p)| *p == pos);
    match (has, k) {
        (Some(j), 0) => {
            app.win.marks.remove(j);
        }
        (None, k) if k > 0 => {
            let j = app.win.marks.iter().position(|(_, p)| *p > pos).unwrap_or(app.win.marks.len());
            app.win.marks.insert(j, (at, pos));
        }
        _ => {}
    }
    app.win.spans.insert(pos, k);
}

/// The view came close to its first event: the entries before it, by
/// `thread/page` (one page at a time).
pub(super) fn want_older(app: &mut App, before: usize) {
    let agent = app.sb.focus.clone();
    app.sb.call("thread/page", json!({"agent": agent, "before": before, "limit": PAGE_ENTRIES}), rpc::Then::Thread(agent.clone(), Some(before)));
    app.win.loading = true;
}

/// The turn edges between what an agent's row said (`was`: working, its
/// ended turns) and what it says now: one end per ended turn (a turn that
/// started and ended between two rows is a start then an end), then a
/// start when it works now; a working flip alone is that edge. True: a
/// start, false: an end, in order (proto-lead m_14731).
pub(crate) fn turn_edges(was: (bool, u64), now: (bool, u64)) -> Vec<bool> {
    let ended = now.1.saturating_sub(was.1);
    let mut out = Vec::new();
    if ended > 0 {
        if was.0 {
            out.push(false);
        } else {
            out.extend([true, false]);
        }
        for _ in 1..ended {
            out.extend([true, false]);
        }
        if now.0 {
            out.push(true);
        }
    } else if was.0 != now.0 {
        out.push(now.0);
    }
    out
}

/// An agent's row as the state reader applies it (zone-a's 4b): its turn
/// edges since the row before ([`turn_edges`]) go to its feed; the first
/// row of an agent fires none.
pub(crate) fn agent_row(app: &mut App, agent: &str, working: bool, turns: u64) {
    let was = app.sb.turns_seen.insert(agent.to_string(), (working, turns));
    let Some(was) = was else { return };
    for started in turn_edges(was, (working, turns)) {
        turn_edge(app, agent, started);
    }
}

/// An agent's turn started or ended, as its agents row says (its status
/// turned working, or stopped being): the feed's turn rows (the tools a
/// turn left running freeze, its end's time for the hover), then the
/// reads of a turn's edge.
pub(crate) fn turn_edge(app: &mut App, agent: &str, working: bool) {
    with_feed(app, agent, |app| {
        if working {
            push_event(&mut app.events, &mut app.cache, Ev::Turn);
        } else {
            push_event(&mut app.events, &mut app.cache, Ev::TurnDone);
            push_event(&mut app.events, &mut app.cache, Ev::Ended(crate::when::now_ms()));
            app.pending = false;
            app.interrupt_requested = false;
        }
    });
    // voice mode's turn, the queue's next message (outside with_feed)
    crate::entry_reads::on_turn(app, agent, working);
}

/// The text reads of an entry placed (level 3, steered, zen, activity,
/// answer fold, queue, voice: `entry_reads::on_entry`, contract m_14707:
/// outside `with_feed`, once per entry new or changed; `live`: a
/// `thread/entry`, not a page).
fn reads(app: &mut App, agent: &str, e: &Entry, live: bool) {
    crate::entry_reads::on_entry(app, agent, e, live);
}

#[cfg(test)]
#[path = "feed_entries_tests.rs"]
mod tests;
