//! The desktop app's updates, core side (bar S.6; architect m_11169):
//! the app says its own build (`app_version {id, built?}`, its VERSION),
//! the core reads bise's release channel (`latest.json`, through
//! [`crate::ambient::setup::SetupPorts::manifest`] on its own thread) at
//! once and every [`RECHECK`], and when bise_home::release says the
//! channel's desktop build replaces the running one it sends the app
//! `app_update {version, url, sha256}`, once per version between two
//! `app_version`s (each one is a window asking again). The manifest's
//! one reader and the one version rule are bise_home::release's
//! (`parse_desktop`, `is_desktop_update`); the app only downloads, checks
//! and swaps, on his click. A fetch that fails is silent: the next hour
//! tries again.

use super::Core;
use bise_home::release;
use serde_json::json;
use std::time::{Duration, Instant};

/// How often the channel is read again while the app runs.
pub(crate) const RECHECK: Duration = Duration::from_secs(3600);

/// The check's state, in the setup.
#[derive(Default)]
pub(crate) struct UpdateCheck {
    /// the running app's build: its VERSION's id and built
    app: Option<(String, Option<String>)>,
    /// when the last read started
    last: Option<Instant>,
    /// a read runs (one at a time)
    running: bool,
    /// the version last said since the last app_version (the hourly
    /// reads never say it twice)
    said: Option<String>,
}

/// The `app_update` to send for this manifest, if any: `(version, url,
/// sha256)`, the zip's url made absolute against the channel's `base`.
pub(crate) fn offer(text: &str, base: &str, target: &str, id: &str, built: Option<&str>) -> Option<(String, String, String)> {
    let r = release::parse_desktop(text, target).ok()?;
    release::is_desktop_update(&r, id, built).then(|| (r.version, release::resolve_url(base, &r.url), r.sha256))
}

/// A read is due: none runs, and none started in the last [`RECHECK`].
pub(crate) fn due(last: Option<Instant>, running: bool, now: Instant) -> bool {
    !running && last.is_none_or(|l| now.saturating_duration_since(l) >= RECHECK)
}

impl Core {
    /// `app_version`: the running app's build; the channel is read now.
    pub(super) fn app_version(&mut self, id: String, built: Option<String>) {
        let Some(s) = self.setup.as_mut() else { return };
        s.update.app = Some((id, built));
        // a window asking again (a reload, a new window): it hears the
        // offer again; `said` only dedupes the hourly reads
        s.update.said = None;
        if !s.update.running {
            s.update.last = None;
        }
        self.update_tick();
    }

    /// On the tick: read the channel when a read is due (only once the app
    /// said its build).
    pub(super) fn update_tick(&mut self) {
        let Some(s) = self.setup.as_mut() else { return };
        if s.update.app.is_none() || !due(s.update.last, s.update.running, Instant::now()) {
            return;
        }
        s.update.running = true;
        s.update.last = Some(Instant::now());
        (s.ports.manifest)(s.tx.clone());
    }

    /// The channel's answer: `app_update` when it offers a newer app.
    pub(super) fn on_manifest(&mut self, text: Result<String, String>, base: &str, target: &str) {
        let Some(s) = self.setup.as_mut() else { return };
        s.update.running = false;
        let (Ok(text), Some((id, built))) = (text, s.update.app.clone()) else { return };
        let Some((version, url, sha256)) = offer(&text, base, target, &id, built.as_deref()) else { return };
        if s.update.said.as_deref() == Some(version.as_str()) {
            return;
        }
        s.update.said = Some(version.clone());
        self.emit(json!({"ev": "app_update", "version": version, "url": url, "sha256": sha256}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(id: &str, built: &str) -> String {
        // the build times are only compared in their one form
        let built = if built.len() == 10 { format!("{built}T10:00:00Z") } else { built.to_string() };
        format!(r#"{{"desktop":{{"darwin-arm64":{{"version":"v-{id}","id":"{id}","url":"app.zip","sha256":"{SHA}","built":"{built}"}}}}}}"#)
    }

    #[test]
    fn a_newer_app_is_offered_with_its_url_under_the_channel() {
        let o = offer(&manifest("d26", "2026-10-08"), "file:///feed/", "darwin-arm64", "d25", Some("2026-10-07T10:00:00Z"));
        assert_eq!(o, Some(("v-d26".into(), "file:///feed/app.zip".into(), SHA.into())));
    }

    #[test]
    fn the_same_build_an_older_one_a_dev_run_or_another_mac_get_nothing() {
        let m = manifest("d26", "2026-10-08");
        assert_eq!(offer(&m, "b", "darwin-arm64", "d26", Some("2026-10-07T10:00:00Z")), None);
        assert_eq!(offer(&manifest("d24", "2026-10-06"), "b", "darwin-arm64", "d25", Some("2026-10-07T10:00:00Z")), None);
        assert_eq!(offer(&m, "b", "darwin-arm64", "dev", None), None);
        assert_eq!(offer(&m, "b", "darwin-x86_64", "d25", Some("2026-10-07T10:00:00Z")), None);
        assert_eq!(offer("{", "b", "darwin-arm64", "d25", Some("2026-10-07T10:00:00Z")), None);
    }

    #[test]
    fn one_read_at_a_time_then_once_an_hour() {
        let t0 = Instant::now();
        assert!(due(None, false, t0));
        assert!(!due(None, true, t0), "one runs");
        assert!(!due(Some(t0), false, t0 + RECHECK - Duration::from_secs(1)));
        assert!(due(Some(t0), false, t0 + RECHECK));
        assert!(!due(Some(t0 + RECHECK), false, t0), "a clock that went back: not due");
    }
}
