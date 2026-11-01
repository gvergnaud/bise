//! The limits of each MCP App view in the hub (docs/mcp-apps.md, risk 1;
//! architect m_50047: a pure shell fn per view, like `StateGate`, that
//! decides no wake): at most [`CALLS`] `view/call` in [`CALL_WINDOW_MS`],
//! one `view/message` every [`MESSAGE_EVERY_MS`], and a message only
//! within [`GESTURE_MS`] of his pointer-down in that view's frame, as
//! Electron's main process saw it (`gesture_ms`); a `view/context` of
//! at most [`CONTEXT_BYTES`], one every [`CONTEXT_EVERY_MS`] (review
//! d5: a view can't slip megabytes into his next message). The proxy and the
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
/// The most a view's context may hold (its text, in bytes).
pub const CONTEXT_BYTES: usize = 16 * 1024;
/// One `view/context` per view at most this often.
pub const CONTEXT_EVERY_MS: u64 = 1_000;
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

/// The server (the bridge's base) a view's calls went through last, and
/// whether they went through the hub's views bridge once.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    base: String,
    moved: bool,
    at: u64,
}

/// The server each view's calls last went through, so a restart is said
/// once per server, and the views moved to the views bridge
/// (docs/mcp-apps.md C5); pruned like [`Limits`].
#[derive(Debug, Default)]
pub struct Servers {
    map: BTreeMap<ViewKey, Seen>,
}

impl Servers {
    /// A call of `key` through the bridge `base` at `now` (`hubs`: the
    /// hub's views bridge): true once per server, when it is not the one
    /// before (another bridge, a views bridge started again: its base
    /// carries a new token; or the views bridge for its first call: the
    /// REPL that ran it is gone). A view that went through the views
    /// bridge is [`Servers::moved`] from then on.
    pub fn restarted(&mut self, key: &ViewKey, base: &str, hubs: bool, now: u64) -> bool {
        let fresh = match self.map.get(key) {
            Some(s) => s.base != base,
            None => hubs,
        };
        let seen = self.map.entry(key.clone()).or_insert_with(|| Seen { base: base.to_string(), moved: false, at: now });
        seen.base = base.to_string();
        seen.moved |= hubs;
        seen.at = now;
        fresh
    }

    /// The view's calls went through the hub's views bridge: they stay
    /// there for the view's life (until it is pruned), never back to its
    /// agent's own bridge. A fresh REPL's servers never ran that view,
    /// and a REPL respawns often (a switch's reload, a recycle): the view
    /// keeps the server, and the state, it has (architect m_51383).
    pub fn moved(&self, key: &ViewKey) -> bool {
        self.map.get(key).is_some_and(|s| s.moved)
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

/// Why a context is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtxRefused {
    /// over [`CONTEXT_BYTES`]
    TooLarge,
    /// one already in the last [`CONTEXT_EVERY_MS`]
    TooSoon,
}

impl CtxRefused {
    /// The error the view gets: `too-large` or `rate-limited`.
    pub fn error(self) -> bise_proto::view_rpc::ViewError {
        match self {
            CtxRefused::TooLarge => bise_proto::view_rpc::ViewError::TooLarge,
            CtxRefused::TooSoon => bise_proto::view_rpc::ViewError::RateLimited,
        }
    }
}

#[derive(Debug, Default)]
pub struct Limits {
    calls: BTreeMap<ViewKey, VecDeque<u64>>,
    messages: BTreeMap<ViewKey, u64>,
    contexts: BTreeMap<ViewKey, u64>,
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

    /// A `view/context` of `key` at `now`, `bytes` long: allowed and
    /// noted, or why not (a refused one is not counted).
    pub fn context(&mut self, key: &ViewKey, now: u64, bytes: usize) -> Result<(), CtxRefused> {
        if bytes > CONTEXT_BYTES {
            return Err(CtxRefused::TooLarge);
        }
        if self.contexts.get(key).is_some_and(|t| now.saturating_sub(*t) < CONTEXT_EVERY_MS) {
            return Err(CtxRefused::TooSoon);
        }
        self.contexts.insert(key.clone(), now);
        Ok(())
    }

    /// Forget the views with nothing in their windows at `now` (the
    /// shell's tick), so the maps never grow with old views.
    pub fn prune(&mut self, now: u64) {
        self.calls.retain(|_, q| q.back().is_some_and(|t| now.saturating_sub(*t) < CALL_WINDOW_MS));
        self.messages.retain(|_, t| now.saturating_sub(*t) < MESSAGE_EVERY_MS);
        self.contexts.retain(|_, t| now.saturating_sub(*t) < CONTEXT_EVERY_MS);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.calls.len() + self.messages.len() + self.contexts.len()
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

    /// d5: a context is at most 16 KB, one a second per view; a refused
    /// one is not counted, and old views are forgotten.
    #[test]
    fn a_context_is_capped_in_size_and_rate() {
        use bise_proto::view_rpc::ViewError;
        let mut l = Limits::default();
        assert_eq!(l.context(&k("1-1"), 1000, CONTEXT_BYTES + 1), Err(CtxRefused::TooLarge));
        assert_eq!(l.context(&k("1-1"), 1000, CONTEXT_BYTES), Ok(()), "the refused one left no trace");
        assert_eq!(l.context(&k("1-1"), 1999, 10), Err(CtxRefused::TooSoon));
        assert_eq!(l.context(&k("1-2"), 1999, 10), Ok(()), "another view");
        assert_eq!(l.context(&k("1-1"), 2000, 10), Ok(()));
        assert_eq!(CtxRefused::TooLarge.error(), ViewError::TooLarge);
        assert_eq!(CtxRefused::TooSoon.error(), ViewError::RateLimited);
        l.prune(2000 + CONTEXT_EVERY_MS);
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn a_restart_is_said_once_per_view_and_old_views_are_forgotten() {
        let mut s = Servers::default();
        assert!(!s.restarted(&k("1-1"), "http://own", false, 1000), "its own bridge, first call");
        assert!(!s.restarted(&k("1-1"), "http://own", false, 1001));
        assert!(s.restarted(&k("1-1"), "http://hubs", true, 1002), "another bridge: said");
        assert!(!s.restarted(&k("1-1"), "http://hubs", true, 1003), "once per server");
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

    /// A view moved to the hub's views bridge stays there (its server
    /// keeps its state across the REPL's respawns, architect m_51383);
    /// a views bridge started again is a new server: said again.
    #[test]
    fn a_moved_view_stays_on_the_views_bridge_and_each_new_server_is_said() {
        let mut s = Servers::default();
        assert!(!s.moved(&k("1-1")), "never called");
        assert!(!s.restarted(&k("1-1"), "http://own", false, 1000));
        assert!(!s.moved(&k("1-1")), "on its own bridge");
        assert!(s.restarted(&k("1-1"), "http://hubs/a", true, 1001), "its REPL went");
        assert!(s.moved(&k("1-1")), "moved");
        assert!(!s.restarted(&k("1-1"), "http://hubs/a", true, 1002), "the same server: said once");
        assert!(s.moved(&k("1-1")), "still moved");
        assert!(s.restarted(&k("1-1"), "http://hubs/b", true, 1003), "the views bridge stopped idle, a new one: said again");
        assert!(!s.restarted(&k("1-1"), "http://hubs/b", true, 1004));
        s.prune(1004 + SERVER_MEMORY_MS);
        assert!(!s.moved(&k("1-1")), "forgotten with its server");
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
