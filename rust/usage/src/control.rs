//! What he does with his usage counts (page usage-metrics-plan, step 6;
//! architect m_56355): turn sharing on or off, mark the note seen, reset
//! the id, delete his data, and see what is sent. The one module both
//! `bise telemetry` and the app's core call (Settings › Privacy, the
//! note): neither keeps any logic of its own.
//!
//! Edges: `prefs.json` through `prefs::Slot` (his `telemetry.share` and
//! `telemetry.note_seen`, the other prefs kept) and the sender's files
//! (through [`crate::sender`], under its lock). Nothing here sends: the
//! hourly sender does.

use std::io;

use bise_home::prefs::{self, Slot};
use bise_home::{Home, Lookup};
use serde_json::Value;

use crate::day::Day;
use crate::install_id::InstallId;
use crate::packet::{Build, Packet};
use crate::sender;
use crate::sharing::{sharing_allowed, Facts, Off};

/// Write `telemetry.<key>` in his prefs, the other prefs kept.
fn set_pref(home: &Home, key: &str, on: bool) -> io::Result<()> {
    let slot = Slot::file(home.prefs_file());
    let mut p = slot.get().filter(Value::is_object).unwrap_or_else(|| serde_json::json!({}));
    if !p[prefs::TELEMETRY].is_object() {
        p[prefs::TELEMETRY] = serde_json::json!({});
    }
    p[prefs::TELEMETRY][key] = Value::Bool(on);
    slot.set(p)
}

/// He shares the daily counts, or not: `telemetry.share`. Off deletes
/// what was counted on this Mac at once, its id too ([`sender::forget`]).
pub fn set_share(home: &Home, on: bool) -> io::Result<()> {
    set_pref(home, "share", on)?;
    if on {
        Ok(())
    } else {
        sender::forget(home)
    }
}

/// His note about the counts was drawn on a surface (the desktop's hello
/// or home, the TUI): `telemetry.note_seen`, without which nothing is
/// counted. Called when it is drawn, never before.
pub fn note_seen(home: &Home) -> io::Result<()> {
    if prefs::telemetry_note_seen(Slot::file(home.prefs_file()).get().as_ref()) {
        return Ok(());
    }
    set_pref(home, "note_seen", true)
}

/// "Reset ID": [`sender::reset_id`].
pub fn reset_id(home: &Home) -> io::Result<()> {
    sender::reset_id(home)
}

/// What "Delete my data" did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deleted {
    /// The server took packets from this id: the request goes at the next
    /// send, and is kept until it is taken.
    Asked,
    /// The server never took any: deleted here, nothing to ask.
    Here,
}

/// "Delete my data": [`sender::request_delete`], even with sharing off.
pub fn delete(home: &Home) -> io::Result<Deleted> {
    Ok(if sender::request_delete(home)? { Deleted::Asked } else { Deleted::Here })
}

/// What bise counts and sends, as he sees it: `bise telemetry show` and
/// Settings › Privacy's "See what's sent". The only places the id shows.
#[derive(Debug)]
pub struct View {
    /// Whether this process may share now, and if not why.
    pub sharing: Result<(), Off>,
    /// His `telemetry.share`, whatever else turns it off.
    pub share: bool,
    /// The id in use (none: drawn at the first send).
    pub id: Option<InstallId>,
    /// The ids whose delete request waits to be sent.
    pub pending: Vec<InstallId>,
    /// The next packets, as the sender would build them now.
    pub next: Vec<Packet>,
    /// Those sent the last 7 days: (sent on, the day counted, the JSON).
    pub sent: Vec<(Day, Day, String)>,
}

/// The [`View`] of `home` at `now_secs` for `build`, the environment read
/// through `env`.
pub fn view(home: &Home, now_secs: u64, build: &Build, env: Lookup) -> View {
    let facts = Facts::read(home, env, build.channel, false);
    let (id, pending) = sender::ids(home);
    View {
        sharing: sharing_allowed(&facts).map(|_| ()),
        share: facts.share,
        id,
        pending,
        next: sender::preview(home, now_secs, build).0,
        sent: crate::sent::list(&home.usage_sent_dir()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Arch, Channel};

    fn home(name: &str) -> (std::path::PathBuf, Home) {
        let root = std::env::temp_dir().join(format!("bise-usage-control-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let h = Home::at(&root);
        (root, h)
    }

    fn prefs_of(h: &Home) -> Value {
        serde_json::from_str(&std::fs::read_to_string(h.prefs_file()).unwrap()).unwrap()
    }

    fn build() -> Build {
        Build { version: "2026.11.8".into(), channel: Channel::Stable, os_major: 26, arch: Arch::of(std::env::consts::ARCH) }
    }

    #[test]
    fn share_and_note_seen_keep_his_other_prefs() {
        let (root, h) = home("prefs");
        std::fs::write(h.prefs_file(), r#"{"theme":"dark"}"#).unwrap();
        note_seen(&h).unwrap();
        set_share(&h, false).unwrap();
        assert_eq!(prefs_of(&h), serde_json::json!({"theme": "dark", "telemetry": {"note_seen": true, "share": false}}));
        set_share(&h, true).unwrap();
        assert_eq!(prefs_of(&h)["telemetry"]["share"], Value::Bool(true));
        let _ = std::fs::remove_dir_all(root);
    }

    /// architect m_56355 (c): off, then show, gives an empty view and no
    /// id on disk.
    #[test]
    fn off_then_show_is_empty_with_no_id_on_disk() {
        let (root, h) = home("off");
        std::fs::create_dir_all(h.usage_day_dir("2026-11-04")).unwrap();
        std::fs::write(h.usage_day_dir("2026-11-04").join("hub-1.json"), "{}").unwrap();
        std::fs::write(h.usage_install_id(), InstallId::draw([7; 16], Day::parse("2026-11-01").unwrap()).to_file()).unwrap();
        set_share(&h, false).unwrap();
        assert!(!h.usage_install_id().exists());
        let v = view(&h, 1_762_560_000, &build(), &|_| None);
        assert_eq!((v.share, v.id.is_none(), v.pending.len(), v.next.len(), v.sent.len()), (false, true, 0, 0, 0), "{v:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reset_deletes_the_id_only_and_delete_with_nothing_sent_stays_here() {
        let (root, h) = home("reset");
        std::fs::create_dir_all(h.usage_dir()).unwrap();
        std::fs::write(h.usage_install_id(), InstallId::draw([7; 16], Day::parse("2026-11-01").unwrap()).to_file()).unwrap();
        std::fs::create_dir_all(h.usage_day_dir("2026-11-04")).unwrap();
        reset_id(&h).unwrap();
        assert!(!h.usage_install_id().exists() && h.usage_day_dir("2026-11-04").exists(), "the counts stay");
        assert_eq!(delete(&h).unwrap(), Deleted::Here);
        assert!(!h.usage_day_dir("2026-11-04").exists() && !h.usage_delete_request().exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
