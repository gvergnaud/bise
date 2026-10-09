//! When the hub's state goes to the clients (hub-fifo, architect
//! m_14659). sb-core's `state` effect came with most inputs, and each one
//! rebuilt and sent the whole snapshot: with 30 busy agents, a snapshot
//! per REPL line. Now a change marks the state pending and the loop sends
//! it when [`StateGate::due`]: at once after [`EVERY_MS`] of quiet (no
//! added delay on a calm hub), then at most once per [`EVERY_MS`]; a
//! pending change always goes ([`StateGate::wait`] bounds the loop's
//! wait), and the clients always get the latest state.
//!
//! Pure: the caller gives the time.

use std::time::Duration;

/// The shortest gap between two state broadcasts.
pub(super) const EVERY_MS: u64 = 100;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StateGate {
    /// The last snapshot sent.
    last_ms: Option<u64>,
    /// A change not sent yet.
    pending: bool,
}

impl StateGate {
    /// The hub's state changed (sb-core's `state` effect).
    pub(super) fn changed(&mut self) {
        self.pending = true;
    }

    /// A pending change may go now.
    pub(super) fn due(&self, now: u64) -> bool {
        self.pending && self.last_ms.is_none_or(|t| now.saturating_sub(t) >= EVERY_MS)
    }

    /// How long the loop may wait for a message before a pending change
    /// is due; None: nothing pending, wait for a message.
    pub(super) fn wait(&self, now: u64) -> Option<Duration> {
        self.pending.then(|| {
            let at = self.last_ms.map_or(0, |t| t + EVERY_MS);
            Duration::from_millis(at.saturating_sub(now))
        })
    }

    /// The whole state push ran (the snapshot and the page hooks): nothing
    /// pending.
    pub(super) fn sent(&mut self, now: u64) {
        self.last_ms = Some(now);
        self.pending = false;
    }

    /// A user action broadcast a snapshot directly: the next pending push
    /// waits [`EVERY_MS`] from now rather than sending the same snapshot
    /// again at once. It stays pending: its page hooks still run then.
    pub(super) fn shown(&mut self, now: u64) {
        self.last_ms = Some(now);
    }

    pub(super) fn pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_on_a_quiet_hub_goes_at_once() {
        let mut g = StateGate::default();
        assert!(!g.due(0) && g.wait(0).is_none(), "nothing pending");
        g.changed();
        assert!(g.due(5));
        assert_eq!(g.wait(5), Some(Duration::ZERO));
        g.sent(5);
        assert!(!g.due(1_000));
        g.changed();
        assert!(g.due(1_000), "after a quiet gap, at once again");
    }

    /// Law: a flood gives at most one push per EVERY_MS, and the last
    /// change always goes.
    #[test]
    fn a_flood_is_sent_at_most_once_per_period_and_the_last_change_goes() {
        let mut g = StateGate::default();
        let mut pushes = Vec::new();
        // a change every ms for one second
        for now in 1_000..2_000u64 {
            g.changed();
            if g.due(now) {
                g.sent(now);
                pushes.push(now);
            }
        }
        assert_eq!(pushes.len(), 10, "{:?}", pushes);
        assert!(pushes.windows(2).all(|w| w[1] - w[0] >= EVERY_MS));
        // the flood stops: the last change is pending and due within EVERY_MS
        assert!(g.pending());
        let at = 1_999 + g.wait(1_999).unwrap().as_millis() as u64;
        assert!(at - pushes.last().unwrap() == EVERY_MS && g.due(at));
    }

    #[test]
    fn a_direct_broadcast_defers_the_pending_push_without_dropping_it() {
        let mut g = StateGate::default();
        g.changed();
        g.shown(500);
        assert!(!g.due(550));
        assert_eq!(g.wait(550), Some(Duration::from_millis(50)));
        assert!(g.due(600));
        let mut q = StateGate::default();
        q.shown(500);
        assert!(!q.due(700), "nothing pending: a direct broadcast is not a change");
    }
}
