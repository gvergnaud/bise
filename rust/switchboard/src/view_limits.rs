//! The limits of each MCP App view in the hub (docs/mcp-apps.md, risk 1;
//! architect m_50047: a pure shell fn per view, like `StateGate`, that
//! decides no wake): at most [`CALLS`] `view/call` in [`CALL_WINDOW_MS`],
//! one `view/message` every [`MESSAGE_EVERY_MS`], and a message only
//! within [`GESTURE_MS`] of his pointer-down in that view's frame, as
//! Electron's main process saw it (`gesture_ms`). The proxy and the
//! renderer never decide: a gesture they forge never reaches the hub
//! (Electron main overwrites it), and a view with none is refused.
//!
//! Pure: time is an argument; the shell keeps one [`Limits`].

use std::collections::{BTreeMap, VecDeque};

/// `view/call`s a view may make in [`CALL_WINDOW_MS`].
pub const CALLS: usize = 30;
pub const CALL_WINDOW_MS: u64 = 60_000;
/// One `view/message` per view at most this often.
pub const MESSAGE_EVERY_MS: u64 = 10_000;
/// A `view/message` needs his pointer-down in the view at most this
/// long before.
pub const GESTURE_MS: u64 = 5_000;
/// A gesture stamped this far in the hub's future still counts (two
/// clocks of one machine).
pub const SKEW_MS: u64 = 1_000;

/// One view: its agent and its call (C2's `<turn ms>-<tool id>`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewKey {
    pub agent: String,
    pub call: String,
}

impl ViewKey {
    pub fn new(agent: &str, call: &str) -> ViewKey {
        ViewKey { agent: agent.to_string(), call: call.to_string() }
    }
}

/// Why a message is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgRefused {
    /// no pointer-down of his in the view in the last [`GESTURE_MS`]
    NoGesture,
    /// one already in the last [`MESSAGE_EVERY_MS`]
    TooSoon,
}

#[derive(Debug, Default)]
pub struct Limits {
    calls: BTreeMap<ViewKey, VecDeque<u64>>,
    messages: BTreeMap<ViewKey, u64>,
}

impl Limits {
    /// A `view/call` of `key` at `now`: counted and allowed, or refused
    /// (not counted).
    pub fn call(&mut self, key: &ViewKey, now: u64) -> bool {
        let q = self.calls.entry(key.clone()).or_default();
        while q.front().is_some_and(|t| now.saturating_sub(*t) >= CALL_WINDOW_MS) {
            q.pop_front();
        }
        if q.len() >= CALLS {
            return false;
        }
        q.push_back(now);
        true
    }

    /// A `view/message` of `key` at `now` with Electron main's last
    /// pointer-down in it (`gesture`, ms): allowed and noted, or why not.
    pub fn message(&mut self, key: &ViewKey, now: u64, gesture: Option<u64>) -> Result<(), MsgRefused> {
        let fresh = gesture.is_some_and(|g| g <= now.saturating_add(SKEW_MS) && now.saturating_sub(g) <= GESTURE_MS);
        if !fresh {
            return Err(MsgRefused::NoGesture);
        }
        if self.messages.get(key).is_some_and(|t| now.saturating_sub(*t) < MESSAGE_EVERY_MS) {
            return Err(MsgRefused::TooSoon);
        }
        self.messages.insert(key.clone(), now);
        Ok(())
    }

    /// Forget the views with nothing in their windows at `now` (the
    /// shell's tick), so the maps never grow with old views.
    pub fn prune(&mut self, now: u64) {
        self.calls.retain(|_, q| q.back().is_some_and(|t| now.saturating_sub(*t) < CALL_WINDOW_MS));
        self.messages.retain(|_, t| now.saturating_sub(*t) < MESSAGE_EVERY_MS);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.calls.len() + self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(call: &str) -> ViewKey {
        ViewKey::new("docs", call)
    }

    #[test]
    fn thirty_calls_a_minute_per_view() {
        let mut l = Limits::default();
        for i in 0..CALLS as u64 {
            assert!(l.call(&k("1-1"), 1000 + i));
        }
        assert!(!l.call(&k("1-1"), 2000), "the 31st in the minute");
        assert!(l.call(&k("1-2"), 2000), "another view has its own count");
        assert!(!l.call(&k("1-1"), 1000 + CALL_WINDOW_MS - 1));
        assert!(l.call(&k("1-1"), 1000 + CALL_WINDOW_MS), "the first call left the window");
    }

    #[test]
    fn a_message_needs_his_fresh_click() {
        let mut l = Limits::default();
        let now = 100_000;
        assert_eq!(l.message(&k("1-1"), now, None), Err(MsgRefused::NoGesture));
        assert_eq!(l.message(&k("1-1"), now, Some(now - GESTURE_MS - 1)), Err(MsgRefused::NoGesture));
        assert_eq!(l.message(&k("1-1"), now, Some(now + SKEW_MS + 1)), Err(MsgRefused::NoGesture), "a click from the future");
        assert_eq!(l.message(&k("1-1"), now, Some(now - GESTURE_MS)), Ok(()));
    }

    #[test]
    fn one_message_every_ten_seconds_per_view() {
        let mut l = Limits::default();
        assert_eq!(l.message(&k("1-1"), 50_000, Some(50_000)), Ok(()));
        assert_eq!(l.message(&k("1-1"), 59_999, Some(59_999)), Err(MsgRefused::TooSoon));
        assert_eq!(l.message(&k("1-2"), 59_999, Some(59_999)), Ok(()), "another view");
        assert_eq!(l.message(&k("1-1"), 60_000, Some(60_000)), Ok(()));
    }

    #[test]
    fn a_refused_call_or_message_is_not_counted_and_old_views_are_forgotten() {
        let mut l = Limits::default();
        assert_eq!(l.message(&k("1-1"), 1000, None), Err(MsgRefused::NoGesture));
        assert_eq!(l.message(&k("1-1"), 1001, Some(1001)), Ok(()), "the refused one left no trace");
        assert!(l.call(&k("1-1"), 1000));
        l.prune(1000 + CALL_WINDOW_MS);
        assert_eq!(l.len(), 0);
    }
}
