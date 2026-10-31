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

/// How long the hub remembers the server a view's calls went through
/// with no call of it: past that, its next call through the views bridge
/// says `server-restarted` again (the views bridge stops when idle too:
/// what it kept in memory is gone by then).
pub const SERVER_MEMORY_MS: u64 = 60 * 60_000;
/// At most this many views remembered (the least recently called go
/// first).
pub const SERVERS: usize = 4096;

/// The server (the bridge's base) a view's calls went through last.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    base: String,
    told: bool,
    at: u64,
}

/// The server each view's calls last went through, so a restart is said
/// once per view (docs/mcp-apps.md C5); pruned like [`Limits`].
#[derive(Debug, Default)]
pub struct Servers {
    map: BTreeMap<ViewKey, Seen>,
}

impl Servers {
    /// A call of `key` through the bridge `base` at `now` (`hubs`: the
    /// hub's views bridge): true once per view when its server is not the
    /// one before (another bridge, or the views bridge for its first
    /// call: the REPL that ran it is gone).
    pub fn restarted(&mut self, key: &ViewKey, base: &str, hubs: bool, now: u64) -> bool {
        let fresh = match self.map.get(key) {
            Some(s) => s.base != base,
            None => hubs,
        };
        let seen = self.map.entry(key.clone()).or_insert_with(|| Seen { base: base.to_string(), told: false, at: now });
        seen.base = base.to_string();
        seen.at = now;
        let say = fresh && !seen.told;
        seen.told |= fresh;
        say
    }

    /// Forget the views not called for [`SERVER_MEMORY_MS`], and the
    /// least recently called past [`SERVERS`].
    pub fn prune(&mut self, now: u64) {
        self.map.retain(|_, s| now.saturating_sub(s.at) < SERVER_MEMORY_MS);
        if self.map.len() > SERVERS {
            let mut ats: Vec<u64> = self.map.values().map(|s| s.at).collect();
            ats.sort_unstable();
            let cut = ats[self.map.len() - SERVERS];
            self.map.retain(|_, s| s.at >= cut);
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
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

impl MsgRefused {
    /// The error the view gets: `no-gesture` or `rate-limited`.
    pub fn error(self) -> bise_proto::view_rpc::ViewError {
        match self {
            MsgRefused::NoGesture => bise_proto::view_rpc::ViewError::NoGesture,
            MsgRefused::TooSoon => bise_proto::view_rpc::ViewError::RateLimited,
        }
    }
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
    fn each_refusal_reaches_the_view_with_its_own_reason() {
        use bise_proto::view_rpc::ViewError;
        assert_eq!(MsgRefused::NoGesture.error(), ViewError::NoGesture);
        assert_eq!(MsgRefused::TooSoon.error(), ViewError::RateLimited);
        assert_eq!(MsgRefused::NoGesture.error().rpc("ui/message").data.and_then(|d| d.reason).as_deref(), Some("no-gesture"));
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
    fn a_restart_is_said_once_per_view_and_old_views_are_forgotten() {
        let mut s = Servers::default();
        assert!(!s.restarted(&k("1-1"), "http://own", false, 1000), "its own bridge, first call");
        assert!(!s.restarted(&k("1-1"), "http://own", false, 1001));
        assert!(s.restarted(&k("1-1"), "http://hubs", true, 1002), "another bridge: said");
        assert!(!s.restarted(&k("1-1"), "http://other", true, 1003), "once per view");
        assert!(s.restarted(&k("1-2"), "http://hubs", true, 1004), "the views bridge for its first call");
        assert_eq!(s.len(), 2);
        s.prune(1003 + SERVER_MEMORY_MS - 1);
        assert_eq!(s.len(), 2);
        s.prune(1004 + SERVER_MEMORY_MS);
        assert!(s.is_empty(), "not called for an hour: forgotten");
        for i in 0..SERVERS as u64 + 10 {
            s.restarted(&k(&format!("{i}-1")), "http://own", false, 5000 + i);
        }
        s.prune(5000 + SERVERS as u64 + 10);
        assert_eq!(s.len(), SERVERS, "the least recently called went first");
        assert!(!s.restarted(&k(&format!("{}-1", SERVERS + 9)), "http://own", false, 9_999), "the newest stayed");
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
