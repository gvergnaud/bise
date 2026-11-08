//! The install id (Gabriel's choice q-id, architect m_53687): a UUID v4
//! drawn from random bytes on this Mac, with its creation day, kept in
//! `Home::usage_install_id`. Never derived from the host id, the hardware
//! or an account: its only input is the random bytes the caller passes.
//!
//! Renewed at 13 months at the latest ([`current`], the clock an
//! argument), at once on "Reset ID", deleted when he turns sharing off
//! (after the delete request went through: the sender's job).
//!
//! It shows only in the day's packet, the delete request and `bise
//! telemetry show`: its `Debug` prints `InstallId(…)`, it has no
//! `Display`, and its text comes out through [`InstallId::reveal`] or its
//! serde form only, so a log line made of `{:?}` never carries it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::day::Day;

/// The id and the day it was drawn.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallId {
    #[serde(rename = "id")]
    uuid: String,
    created: Day,
}

/// The id is renewed this many months after its creation, at the latest
/// (the CNIL's audience-measurement exemption).
pub const RENEW_MONTHS: u32 = 13;

impl InstallId {
    /// A fresh id from 16 random bytes, drawn on `today`.
    pub fn draw(random: [u8; 16], today: Day) -> InstallId {
        let mut b = random;
        b[6] = (b[6] & 0x0f) | 0x40; // version 4
        b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let uuid = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
        InstallId { uuid, created: today }
    }

    /// The day it was drawn.
    pub fn created(&self) -> Day {
        self.created
    }

    /// The first day it is no longer used.
    pub fn renew_on(&self) -> Day {
        self.created.plus_months(RENEW_MONTHS)
    }

    /// Its text, for the packet's encoder, the delete request and `bise
    /// telemetry show` only.
    pub fn reveal(&self) -> &str {
        &self.uuid
    }

    /// Its file's text.
    pub fn to_file(&self) -> String {
        serde_json::to_string(self).expect("an id serializes") + "\n"
    }

    /// Read its file back: None for anything but a UUID v4 and a day.
    pub fn from_file(text: &str) -> Option<InstallId> {
        let id: InstallId = serde_json::from_str(text.trim()).ok()?;
        is_uuid_v4(&id.uuid).then_some(id)
    }
}

impl fmt::Debug for InstallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstallId(…)")
    }
}

/// A packet's `install_id`: the UUID alone.
pub fn serialize_uuid<S: serde::Serializer>(id: &InstallId, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(id.reveal())
}

/// Whether `s` is a lowercase UUID v4.
pub fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// The id to use on `today`: the stored one while it is young enough,
/// else a fresh one from `random` (none stored, or 13 months old).
/// Returns it and whether it is new (the caller writes its file).
pub fn current(stored: Option<InstallId>, today: Day, random: impl FnOnce() -> [u8; 16]) -> (InstallId, bool) {
    match stored {
        Some(id) if today < id.renew_on() && today >= id.created => (id, false),
        _ => (InstallId::draw(random(), today), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn an_id_is_a_uuid_v4_of_its_random_bytes() {
        let a = InstallId::draw([0xff; 16], day("2026-11-05"));
        let b = InstallId::draw([0x00; 16], day("2026-11-05"));
        assert!(is_uuid_v4(a.reveal()) && is_uuid_v4(b.reveal()), "{} {}", a.reveal(), b.reveal());
        assert_ne!(a.reveal(), b.reveal(), "two homes, two draws, two ids");
        assert_eq!(InstallId::from_file(&a.to_file()), Some(a.clone()));
        assert_eq!(InstallId::from_file(r#"{"id":"camille","created":"2026-11-05"}"#), None);
    }

    #[test]
    fn debug_never_prints_it() {
        let a = InstallId::draw([0x5a; 16], day("2026-11-05"));
        let shown = format!("{a:?} {:?}", Some(&a));
        assert!(!shown.contains(&a.reveal()[..8]), "{shown}");
    }

    #[test]
    fn it_is_renewed_at_thirteen_months() {
        let made = day("2026-01-15");
        let id = InstallId::draw([1; 16], made);
        let fresh = || [2; 16];
        assert_eq!(current(Some(id.clone()), day("2027-02-14"), fresh), (id.clone(), false), "a day before 13 months: kept");
        let (renewed, new) = current(Some(id.clone()), day("2027-02-16"), fresh);
        assert!(new && renewed != id && renewed.created() == day("2027-02-16"), "13 months and a day: renewed");
        assert!(current(Some(id.clone()), day("2027-02-15"), fresh).1, "13 months to the day: renewed");
        assert!(current(None, made, fresh).1, "none stored: drawn");
        assert!(current(Some(id), day("2025-12-01"), fresh).1, "a creation day in the future (a clock set back): drawn again");
    }
}
