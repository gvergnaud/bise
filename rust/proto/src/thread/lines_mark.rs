//! His message's mark (G1, contract C3, BISE-86): the one rule, moved
//! out of lines.rs unchanged.

use super::{unescape, Hub, Obs, Rec};
use crate::thread::Delivery;

/// What a line does to the marks of his messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    /// steering (`steering_received`, `steered`) or a message the core
    /// committed (`history injected`): his newest message with these
    /// words goes up to `to`. None with these words: `or_turn` (steering,
    /// BISE-90: the hub steered his words in a block with other words)
    /// raises his messages of this turn instead; else nothing moves (an
    /// injected notification: the TUI shows its info line). Steering
    /// (`or_turn`) moves only his messages WITHOUT an id: transcripts
    /// before 9633cd2a and older hubs; a message with an id gets its marks
    /// from sb-core's receipts ([`Mark::Ids`]), never from words
    Text { text: String, to: Delivery, or_turn: bool },
    /// a turn started: the model reads what he sent since the last one
    /// (a message at idle goes straight to read)
    Turn,
    /// BISE-86: his message `text` didn't reach `to` (his line is the
    /// text, or `@to text` from another view)
    Failed { to: String, text: String },
    /// sb-core's receipts (`steer-rx`, `steered`): his messages with
    /// these ids go up to `to` (no words matched)
    Ids { ids: Vec<u64>, to: Delivery },
}

/// The mark a line moves, if any.
pub fn mark_of(rec: &Rec) -> Option<Mark> {
    let text = |text: &str, to, or_turn| Some(Mark::Text { text: text.to_string(), to, or_turn });
    match rec {
        Rec::Obs(Obs::SteeringReceived(t)) => text(t, Delivery::Received, true),
        Rec::Obs(Obs::Steered(t)) => text(t, Delivery::Read, true),
        Rec::Injected(t) => text(t, Delivery::Read, false),
        Rec::Obs(Obs::TurnStarted) => Some(Mark::Turn),
        Rec::Hub(Hub::Undelivered { to, text, .. }) => Some(Mark::Failed { to: to.clone(), text: text.clone() }),
        Rec::Hub(Hub::SteerRx(ids)) => Some(Mark::Ids { ids: ids.clone(), to: Delivery::Received }),
        Rec::Hub(Hub::Steered(ids)) => Some(Mark::Ids { ids: ids.clone(), to: Delivery::Read }),
        _ => None,
    }
}

/// A thread's item as [`deliver`] reads it: the TUI's feed events, the
/// fold's entries.
pub trait Delivered {
    /// his message: its text and its mark
    fn yours(&self) -> Option<(&str, Delivery)>;
    fn set_mark(&mut self, to: Delivery);
    /// his message's id (its `sb you-id` line), None when it has none
    /// (an item that is not his; a transcript before 9633cd2a)
    fn msg_id(&self) -> Option<u64> {
        None
    }
    /// a turn started here (the TUI keeps one in its feed; the fold's
    /// turns are no entries: it passes the items since its last one)
    fn turn_start(&self) -> bool {
        false
    }
}

/// How far back a steering line looks for his message.
pub const MARK_LOOKBACK: usize = 500;

/// The words of a text, for matching a steering line to his message
/// (the wire flattens its line breaks).
fn same_words(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

/// Mark `m` on `items` (oldest first; a turn mark: the items since the
/// turn's start, or a feed where [`Delivered::turn_start`] says it): the
/// indices whose mark moved, or None when no message of his matched
/// (an injected notification; an undelivered line from another view).
/// Marks only move up ([`Delivery`]'s order); `failed` stays.
pub fn deliver<T: Delivered>(items: &mut [T], m: &Mark) -> Option<Vec<usize>> {
    let mut moved = Vec::new();
    let mut raise = |items: &mut [T], i: usize, to: Delivery| {
        if items[i].yours().is_some_and(|(_, d)| to > d) {
            items[i].set_mark(to);
            moved.push(i);
        }
    };
    match m {
        Mark::Text { text, to, or_turn } => {
            let plain = unescape(text);
            let from = items.len().saturating_sub(MARK_LOOKBACK);
            // steering: id-less messages only (transcripts before 9633cd2a)
            let open = |items: &[T], i: usize| !*or_turn || items[i].msg_id().is_none();
            if let Some(i) = (from..items.len()).rev().find(|&i| open(items, i) && items[i].yours().is_some_and(|(t, _)| same_words(t, &plain))) {
                raise(items, i, *to);
            } else if *or_turn {
                this_turn(items, |items, i| {
                    if open(items, i) {
                        raise(items, i, *to)
                    }
                });
            } else {
                return None;
            }
        }
        Mark::Turn => this_turn(items, |items, i| raise(items, i, Delivery::Read)),
        Mark::Failed { to, text } => {
            let mine = |t: &str| t == text || t.strip_prefix('@').and_then(|r| r.split_once(' ')).is_some_and(|(n, r)| n == to && r.trim() == text);
            let i = (0..items.len()).rev().find(|&i| items[i].yours().is_some_and(|(t, d)| d != Delivery::Failed && mine(t)))?;
            items[i].set_mark(Delivery::Failed);
            moved.push(i);
        }
        Mark::Ids { ids, to } => {
            for i in items.len().saturating_sub(MARK_LOOKBACK)..items.len() {
                if items[i].msg_id().is_some_and(|id| ids.contains(&id)) {
                    raise(items, i, *to);
                }
            }
        }
    }
    Some(moved)
}

/// His messages since the turn started, newest first, still sent or
/// received.
fn this_turn<T: Delivered>(items: &mut [T], mut f: impl FnMut(&mut [T], usize)) {
    for i in (0..items.len()).rev() {
        if items[i].turn_start() {
            break;
        }
        if items[i].yours().is_some_and(|(_, d)| matches!(d, Delivery::Sent | Delivery::Received)) {
            f(items, i);
        }
    }
}
