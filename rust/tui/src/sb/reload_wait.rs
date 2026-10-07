//! A reload never lands in the middle of typing (keep-state): the hub
//! asked this TUI to follow it (a version switch, `sb restart`, a hub
//! restarted after a crash: `hello` with another binary or another reload
//! id), and the TUI re-executes once no key has arrived for [`QUIET`],
//! never later than [`CAP`] after the ask. Meanwhile the divider says
//! [`NOTE`].
//!
//! Pure: the loop feeds it the keys and the clock (run.rs), the hub line
//! the ask (sb.rs `hello`), and quits when [`Wait::due`].

use std::time::{Duration, Instant};

/// No key for this long: the reload goes.
pub(crate) const QUIET: Duration = Duration::from_secs(3);
/// The reload never waits longer than this after it was asked.
pub(crate) const CAP: Duration = Duration::from_secs(30);
/// What the divider says while it waits.
pub(crate) const NOTE: &str = "bise restarts when you stop typing";

#[derive(Debug, Default, Clone)]
pub(crate) struct Wait {
    /// the hub asked for a reload, at this time
    since: Option<Instant>,
    /// the last key or paste
    last_key: Option<Instant>,
}

impl Wait {
    /// The hub asked for a reload (the first ask's time stays).
    pub(crate) fn ask(&mut self, now: Instant) {
        self.since.get_or_insert(now);
    }

    /// A key or a paste arrived.
    pub(crate) fn key(&mut self, now: Instant) {
        self.last_key = Some(now);
    }

    /// The reload is waiting for the keys to stop.
    pub(crate) fn waiting(&self, now: Instant) -> bool {
        self.since.is_some() && !self.due(now)
    }

    /// The reload goes now: asked, and no key for [`QUIET`] (or asked
    /// [`CAP`] ago).
    pub(crate) fn due(&self, now: Instant) -> bool {
        let Some(since) = self.since else { return false };
        now.duration_since(since) >= CAP || self.last_key.is_none_or(|k| now.duration_since(k) >= QUIET)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn no_ask_never_due() {
        let t0 = Instant::now();
        let w = Wait::default();
        assert!(!w.due(t0 + CAP * 2));
        assert!(!w.waiting(t0));
    }

    #[test]
    fn no_keys_goes_at_once() {
        let t0 = Instant::now();
        let mut w = Wait::default();
        w.ask(t0);
        assert!(w.due(t0));
    }

    #[test]
    fn keys_hold_it_until_they_stop() {
        let t0 = Instant::now();
        let mut w = Wait::default();
        w.key(t0);
        w.ask(t0 + ms(100));
        assert!(w.waiting(t0 + ms(100)), "a key just before the ask counts");
        w.key(t0 + ms(2000));
        assert!(!w.due(t0 + ms(4900)));
        assert!(w.due(t0 + ms(5000)));
    }

    #[test]
    fn never_longer_than_the_cap() {
        let t0 = Instant::now();
        let mut w = Wait::default();
        w.ask(t0);
        let mut t = t0;
        while t < t0 + CAP {
            w.key(t);
            assert!(!w.due(t) || t.duration_since(t0) >= CAP);
            t += ms(500);
        }
        w.key(t0 + CAP);
        assert!(w.due(t0 + CAP));
    }

    #[test]
    fn a_second_ask_keeps_the_first_time() {
        let t0 = Instant::now();
        let mut w = Wait::default();
        w.ask(t0);
        w.ask(t0 + ms(20_000));
        w.key(t0 + CAP);
        assert!(w.due(t0 + CAP), "the cap counts from the first ask");
    }
}
