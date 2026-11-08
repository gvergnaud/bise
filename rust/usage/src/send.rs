//! The send fn, the same for the hub and the app's core (architect
//! m_53623 c): a body from [`crate::encode`], posted through a
//! [`Transport`], its answer sorted into an [`Outcome`]. The transport is
//! a port: the real one is the system's curl, like harness/update.rs (plan
//! step 5); the tests' is a fake collector in memory. No test touches the
//! network: the endpoint is [`endpoint`]'s, and a test run names its fake
//! with `BISE_TELEMETRY_URL`.
//!
//! Not here: the one sender's flock, the batches, `sent/`, when to retry
//! (step 5). [`backoff`] is the retry's delay, pure.

use std::time::Duration;

use crate::encode;
use crate::install_id::InstallId;
use crate::packet::Packet;
use crate::sharing::Permit;

/// Where the packets go when no test names another place.
pub const DEFAULT_BASE: &str = "https://telemetry.bise.dev";

/// The test setting that names a fake collector.
pub const URL_VAR: &str = "BISE_TELEMETRY_URL";

/// The server's base URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    base: String,
}

/// The endpoint: the test setting's value (read through
/// `bise_home::env::test_setting(URL_VAR)` by the caller) or the default.
pub fn endpoint(test_url: Option<String>) -> Endpoint {
    let base = test_url.filter(|u| !u.trim().is_empty()).unwrap_or_else(|| DEFAULT_BASE.to_string());
    Endpoint { base: base.trim_end_matches('/').to_string() }
}

impl Endpoint {
    /// `POST` a batch of packets here.
    pub fn packets_url(&self) -> String {
        format!("{}/v1/packets", self.base)
    }

    /// `POST` a delete request here.
    pub fn delete_url(&self) -> String {
        format!("{}/v1/delete", self.base)
    }
}

/// How a body reaches the server: one HTTPS POST of JSON, no cookie, no
/// header that names him; its answer's status, or why it never came.
pub trait Transport {
    fn post_json(&mut self, url: &str, body: &[u8]) -> Result<u16, String>;
}

/// What became of a body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A 2xx: the server has it.
    Taken,
    /// A 4xx: the server refuses this body; sending it again won't help
    /// (the packets are dropped, the reason logged locally).
    Refused(u16),
    /// No answer, or another status: keep it and try again later.
    Retry(String),
}

fn outcome(r: Result<u16, String>) -> Outcome {
    match r {
        Ok(s) if (200..300).contains(&s) => Outcome::Taken,
        Ok(s) if (400..500).contains(&s) && s != 408 && s != 429 => Outcome::Refused(s),
        Ok(s) => Outcome::Retry(format!("status {s}")),
        Err(e) => Outcome::Retry(e),
    }
}

/// Send a batch of closed days' packets (at most [`encode::MAX_BATCH`]);
/// only with a [`Permit`].
pub fn send_packets(_: &Permit, to: &Endpoint, batch: &[Packet], t: &mut dyn Transport) -> Outcome {
    if batch.is_empty() {
        return Outcome::Taken;
    }
    outcome(t.post_json(&to.packets_url(), &encode::packets(batch)))
}

/// Ask the server to delete every packet of this id ("Delete my data"):
/// the one send allowed with sharing off. The caller deletes the local id
/// only after [`Outcome::Taken`]; on anything else it keeps the id and
/// tries again.
pub fn send_delete(to: &Endpoint, id: &InstallId, t: &mut dyn Transport) -> Outcome {
    outcome(t.post_json(&to.delete_url(), &encode::delete_request(id)))
}

/// The shortest and the longest wait before a retry.
pub const BACKOFF_MIN: Duration = Duration::from_secs(60);
pub const BACKOFF_MAX: Duration = Duration::from_secs(6 * 3600);

/// The wait before retry number `attempt` (0 = the first): doubling from
/// a minute up to 6 hours, then a random part taken off (`jitter` in
/// [0, 1), up to half the wait), never under a minute.
pub fn backoff(attempt: u32, jitter: f64) -> Duration {
    let full = BACKOFF_MIN.as_secs().saturating_mul(1u64 << attempt.min(20)).min(BACKOFF_MAX.as_secs());
    let off = (full as f64 / 2.0 * jitter.clamp(0.0, 1.0)) as u64;
    Duration::from_secs((full - off).max(BACKOFF_MIN.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_sort_into_taken_refused_retry() {
        assert_eq!(outcome(Ok(204)), Outcome::Taken);
        assert_eq!(outcome(Ok(400)), Outcome::Refused(400));
        assert_eq!(outcome(Ok(429)), Outcome::Retry("status 429".into()));
        assert_eq!(outcome(Ok(503)), Outcome::Retry("status 503".into()));
        assert_eq!(outcome(Err("timeout".into())), Outcome::Retry("timeout".into()));
    }

    #[test]
    fn the_endpoint_is_the_default_unless_a_test_names_one() {
        assert_eq!(endpoint(None).packets_url(), "https://telemetry.bise.dev/v1/packets");
        assert_eq!(endpoint(Some("http://127.0.0.1:9/".into())).delete_url(), "http://127.0.0.1:9/v1/delete");
    }

    #[test]
    fn backoff_doubles_from_a_minute_to_six_hours() {
        let secs = |a, j| backoff(a, j).as_secs();
        assert_eq!([0, 1, 2, 8, 9, 40].map(|a| secs(a, 0.0)), [60, 120, 240, 15_360, 21_600, 21_600]);
        assert_eq!(secs(3, 0.999), 241, "jitter takes up to half off");
        assert_eq!(secs(0, 0.999), 60, "never under a minute");
    }
}
