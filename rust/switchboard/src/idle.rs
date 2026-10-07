//! idle-exit (docs/idle-exit.md): a hub nobody looks at stops by itself.
//!
//! The hub counts its UIs: the clients that said `hello` (the TUI, the
//! ambient app's core) and the holds others take on it (`Holds`: a
//! page's open event stream). When the count stays at 0 for the grace
//! period (`idle_exit` in config.toml or `$BISE_IDLE_EXIT`, default 2
//! min), the hub stops for good once nothing runs: no agent mid-turn,
//! no background job of an agent, no build of its own. A UI that comes
//! back during the grace or the wait cancels it. The stop is the one of
//! `bise --stop`: every REPL checkpoints and exits, what the agents
//! started goes with them; the next `bise` brings it all back from the
//! journal and the saved sessions.
//!
//! Pure but for `bg_jobs` (a folder read): the clock, the count and the
//! reasons to wait come from the hub (`daemon.rs`).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub const ENV: &str = "BISE_IDLE_EXIT";
pub const DEFAULT_GRACE: Duration = Duration::from_secs(120);
/// A background job older than this no longer holds the hub up: a dev
/// server or a `tail -f` an agent forgot would keep it forever.
pub const JOB_MAX: Duration = Duration::from_secs(6 * 3600);

/// The grace period: `$BISE_IDLE_EXIT`, else `idle_exit` in config.toml,
/// else 2 min. A number of seconds or `90s`, `5m`, `1h`; `off` (or
/// `never`, `false`): the hub never stops by itself. Unreadable: the
/// default.
pub fn grace(env: Option<&str>, config: &str) -> Option<Duration> {
    let from_config = || {
        let t: toml::Table = config.parse().ok()?;
        match t.get("idle_exit")? {
            toml::Value::String(s) => Some(s.clone()),
            toml::Value::Integer(n) => Some(n.to_string()),
            toml::Value::Boolean(false) => Some("off".into()),
            _ => None,
        }
    };
    let text = env.map(str::to_string).filter(|s| !s.trim().is_empty()).or_else(from_config);
    match text {
        Some(t) => parse(&t).unwrap_or(Some(DEFAULT_GRACE)),
        None => Some(DEFAULT_GRACE),
    }
}

/// `off` -> Some(None); `90`, `90s`, `5m`, `1h` -> Some(Some(d)); else None.
fn parse(s: &str) -> Option<Option<Duration>> {
    let s = s.trim().to_ascii_lowercase();
    if matches!(s.as_str(), "off" | "never" | "false" | "no") {
        return Some(None);
    }
    let (num, unit) = match s.char_indices().find(|(_, c)| !c.is_ascii_digit()) {
        Some((i, _)) => (&s[..i], s[i..].trim()),
        None => (s.as_str(), "s"),
    };
    let n: u64 = num.parse().ok()?;
    let secs = match unit {
        "s" | "sec" | "secs" => n,
        "m" | "min" | "mins" => n * 60,
        "h" => n * 3600,
        _ => return None,
    };
    Some(Some(Duration::from_secs(secs)))
}

/// Holds on the hub besides its `hello` clients (a page's open event
/// stream): the hub does not stop while one is held.
#[derive(Clone, Default)]
pub struct Holds(Arc<AtomicUsize>);

/// One hold; dropping it lets go.
pub struct Hold(Arc<AtomicUsize>);

impl Holds {
    pub fn hold(&self) -> Hold {
        self.0.fetch_add(1, Ordering::AcqRel);
        Hold(self.0.clone())
    }

    pub fn count(&self) -> usize {
        self.0.load(Ordering::Acquire)
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// What the hub does after a look.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Stay,
    /// stay, and say this in hub.log
    Say(String),
    /// stop for good (the line for hub.log)
    Stop(String),
}

/// The watch: since when no UI is there, and what it last said.
pub struct Watch {
    grace: Option<Duration>,
    alone_since: Option<Instant>,
    said: Option<String>,
}

impl Watch {
    /// A hub starts alone: one nobody opens (a test's, a relaunch nobody
    /// came back to) stops after the grace like any other.
    pub fn new(grace: Option<Duration>, now: Instant) -> Watch {
        Watch {
            grace,
            alone_since: Some(now),
            said: None,
        }
    }

    pub fn grace(&self) -> Option<Duration> {
        self.grace
    }

    /// One look: `uis` the UIs there now, `busy` what still runs (asked
    /// only once the grace is over: it reads folders).
    pub fn step(&mut self, now: Instant, uis: usize, busy: impl FnOnce() -> Vec<String>) -> Step {
        let Some(grace) = self.grace else { return Step::Stay };
        if uis > 0 {
            self.alone_since = None;
            return match self.said.take() {
                Some(_) => Step::Say("idle exit: a UI is back, the hub stays".into()),
                None => Step::Stay,
            };
        }
        let since = *self.alone_since.get_or_insert(now);
        if now.duration_since(since) < grace {
            if self.said.is_none() && since == now {
                let s = format!("idle exit: no UI left, the hub stops in {} s once nothing runs", grace.as_secs());
                self.said = Some(s.clone());
                return Step::Say(s);
            }
            return Step::Stay;
        }
        let busy = busy();
        if busy.is_empty() {
            return Step::Stop(format!(
                "idle exit: no UI for {} s and nothing runs: the hub stops (the next bise brings it all back)",
                now.duration_since(since).as_secs()
            ));
        }
        let s = format!("idle exit: no UI, the hub waits for: {}", busy.join(", "));
        if self.said.as_deref() == Some(s.as_str()) {
            return Step::Stay;
        }
        self.said = Some(s.clone());
        Step::Say(s)
    }
}

/// Live `sb every` timers keep the hub up (a standing order fires only
/// while the hub runs): the reason, or None.
pub fn timers_busy(live: usize) -> Option<String> {
    match live {
        0 => None,
        1 => Some("a standing order (sb every)".into()),
        n => Some(format!("{} standing orders (sb every)", n)),
    }
}

/// The background jobs of an agent's bash tool still running in `bg`
/// (its `BEND_BG_DIR`): a `<n>.slot` folder whose `<n>.pid` is alive and
/// younger than `JOB_MAX`. Their numbers.
pub fn bg_jobs(bg: &Path, alive: impl Fn(u32) -> bool) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(bg) else { return Vec::new() };
    let mut out: Vec<String> = rd
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".slot")).map(str::to_string))
        .filter(|n| {
            let f = bg.join(format!("{}.pid", n));
            let young = std::fs::metadata(&f)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age < JOB_MAX);
            young
                && std::fs::read_to_string(&f)
                    .ok()
                    .and_then(|p| p.trim().parse().ok())
                    .is_some_and(&alive)
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn the_grace_reads_the_env_then_the_config() {
        assert_eq!(grace(None, ""), Some(DEFAULT_GRACE));
        assert_eq!(grace(Some("30"), "idle_exit = \"5m\""), Some(S(30)));
        assert_eq!(grace(None, "idle_exit = \"5m\""), Some(S(300)));
        assert_eq!(grace(None, "idle_exit = 45"), Some(S(45)));
        assert_eq!(grace(None, "idle_exit = \"off\""), None);
        assert_eq!(grace(None, "idle_exit = false"), None);
        assert_eq!(grace(Some("never"), ""), None);
        assert_eq!(grace(Some("1h"), ""), Some(S(3600)));
        assert_eq!(grace(Some("2 m"), ""), Some(S(120)));
        // unreadable: the default, never "off" by mistake
        assert_eq!(grace(Some("soon"), ""), Some(DEFAULT_GRACE));
        assert_eq!(grace(None, "idle_exit = [1]"), Some(DEFAULT_GRACE));
        assert_eq!(grace(Some(""), "idle_exit = \"10s\""), Some(S(10)));
    }

    #[test]
    fn the_hub_stops_after_the_grace_once_nothing_runs() {
        let t0 = Instant::now();
        let mut w = Watch::new(Some(S(120)), t0);
        // a UI the first second: nothing said, no stop
        assert_eq!(w.step(t0, 1, Vec::new), Step::Stay);
        // it leaves: the grace starts
        assert!(matches!(w.step(t0 + S(10), 0, Vec::new), Step::Say(s) if s.contains("120 s")));
        assert_eq!(w.step(t0 + S(100), 0, || panic!("not asked during the grace")), Step::Stay);
        // over: an agent mid-turn holds it, said once
        let busy = || vec!["t1 mid-turn".to_string()];
        assert!(matches!(w.step(t0 + S(131), 0, busy), Step::Say(s) if s.contains("t1 mid-turn")));
        assert_eq!(w.step(t0 + S(140), 0, busy), Step::Stay);
        // the turn ends: stop
        assert!(matches!(w.step(t0 + S(150), 0, Vec::new), Step::Stop(_)));
    }

    #[test]
    fn a_ui_back_during_the_grace_or_the_wait_cancels_it() {
        let t0 = Instant::now();
        let mut w = Watch::new(Some(S(60)), t0);
        assert!(matches!(w.step(t0, 0, Vec::new), Step::Say(_)));
        // a quick relaunch: back before the grace ends
        assert!(matches!(w.step(t0 + S(50), 1, Vec::new), Step::Say(s) if s.contains("back")));
        // gone again: a fresh grace, from now
        assert!(matches!(w.step(t0 + S(70), 0, Vec::new), Step::Say(_)));
        assert_eq!(w.step(t0 + S(120), 0, Vec::new), Step::Stay);
        assert!(matches!(w.step(t0 + S(131), 0, Vec::new), Step::Stop(_)));
    }

    #[test]
    fn a_hub_nobody_opens_stops_and_off_never_does() {
        let t0 = Instant::now();
        let mut w = Watch::new(Some(S(60)), t0);
        assert!(matches!(w.step(t0 + S(61), 0, Vec::new), Step::Stop(_)));
        let mut off = Watch::new(None, t0);
        assert_eq!(off.step(t0 + S(100_000), 0, Vec::new), Step::Stay);
    }

    #[test]
    fn holds_count_until_dropped() {
        let h = Holds::default();
        let a = h.hold();
        let b = h.clone().hold();
        assert_eq!(h.count(), 2);
        drop(a);
        assert_eq!(h.count(), 1);
        drop(b);
        assert_eq!(h.count(), 0);
    }

    #[test]
    fn a_standing_order_keeps_the_hub_up() {
        assert_eq!(timers_busy(0), None);
        assert_eq!(timers_busy(1).as_deref(), Some("a standing order (sb every)"));
        let t0 = Instant::now();
        let mut w = Watch::new(Some(S(60)), t0);
        let busy = || timers_busy(2).into_iter().collect::<Vec<_>>();
        assert!(matches!(w.step(t0 + S(61), 0, busy), Step::Say(s) if s.contains("2 standing orders")));
        assert_eq!(w.step(t0 + S(600), 0, busy), Step::Stay);
        // the last one stopped: the hub goes
        assert!(matches!(w.step(t0 + S(601), 0, || timers_busy(0).into_iter().collect()), Step::Stop(_)));
    }

    #[test]
    fn a_running_young_job_holds_the_hub() {
        let d = std::env::temp_dir().join(format!("idle-bg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("3.slot")).unwrap();
        std::fs::write(d.join("3.pid"), "4242\n").unwrap();
        // done: no slot, a .rc
        std::fs::write(d.join("1.rc"), "0\n").unwrap();
        // a slot whose process is gone
        std::fs::create_dir_all(d.join("5.slot")).unwrap();
        std::fs::write(d.join("5.pid"), "999999\n").unwrap();
        assert_eq!(bg_jobs(&d, |p| p == 4242), vec!["3".to_string()]);
        assert!(bg_jobs(&d.join("none"), |_| true).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
