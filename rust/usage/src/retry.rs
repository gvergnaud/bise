//! The sender's backoff after a failed send (`Home::usage_retry`,
//! `retry.json`): how many sends failed in a row and when the next may
//! go. Shared by the packets and the delete request ("le même backoff"),
//! kept in a file because the hubs and the app's core take turns sending.
//! Pure but for [`Retry::load`] and [`Retry::save`]; the clock and the
//! jitter are arguments.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::send::backoff;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retry {
    /// Sends failed in a row (0: the last one went through).
    pub failed: u32,
    /// No send before this unix time.
    pub next_secs: u64,
}

impl Retry {
    /// Whether a send may go at `now_secs`.
    pub fn due(&self, now_secs: u64) -> bool {
        now_secs >= self.next_secs
    }

    /// One more send failed at `now_secs`: the next waits [`backoff`].
    pub fn failed(self, now_secs: u64, jitter: f64) -> Retry {
        Retry { failed: self.failed.saturating_add(1), next_secs: now_secs + backoff(self.failed, jitter).as_secs() }
    }

    /// The file's retry; none (or unreadable) is no wait.
    pub fn load(path: &Path) -> Retry {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Keep it; a retry with nothing failed is no file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if *self == Retry::default() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        crate::spool::write_atomic(path, serde_json::to_string(self)?.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_waits_longer_and_a_success_clears_it() {
        let r = Retry::default();
        assert!(r.due(0));
        let r1 = r.failed(1_000, 0.0);
        assert_eq!(r1, Retry { failed: 1, next_secs: 1_060 });
        assert!(!r1.due(1_059) && r1.due(1_060));
        assert_eq!(r1.failed(2_000, 0.0).next_secs, 2_120);
        let dir = std::env::temp_dir().join(format!("bise-usage-retry-{}", std::process::id()));
        let path = dir.join("retry.json");
        r1.save(&path).unwrap();
        assert_eq!(Retry::load(&path), r1);
        Retry::default().save(&path).unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
