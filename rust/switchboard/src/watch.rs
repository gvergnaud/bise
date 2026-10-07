//! One progress rule for everything the hub waits on (pure): a wait
//! fails only after `limit` WITHOUT progress, never after a fixed time in
//! all. A hub's boot (crate::boot) and each REPL start (crate::repl_start)
//! use it: on a loaded machine a slow step that moves is not a failure.

/// When something last moved, and how long it may stay still (ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    moved_ms: u64,
    limit_ms: u64,
}

impl Watch {
    pub fn new(now_ms: u64, limit_ms: u64) -> Watch {
        Watch { moved_ms: now_ms, limit_ms }
    }

    /// It moved: a new step, a batch done.
    pub fn moved(&mut self, now_ms: u64) {
        self.moved_ms = now_ms;
    }

    /// How long it has been still (ms).
    pub fn still_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.moved_ms)
    }

    /// Still for `limit` or more: stalled.
    pub fn stalled(&self, now_ms: u64) -> bool {
        self.still_ms(now_ms) >= self.limit_ms
    }
}

#[cfg(test)]
mod tests {
    use super::Watch;

    #[test]
    fn a_wait_fails_only_after_its_limit_without_progress() {
        let mut w = Watch::new(0, 45_000);
        assert!(!w.stalled(44_999));
        assert!(w.stalled(45_000));
        // a move restarts the count, however long the wait has been
        w.moved(300_000);
        assert!(!w.stalled(344_999));
        assert_eq!(w.still_ms(310_000), 10_000);
    }
}
