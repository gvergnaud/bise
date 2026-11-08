//! Whether this process may count and send usage: [`sharing_allowed`],
//! the one gate (plan usage-metrics-plan, "le réglage"; architect m_53623
//! d.2). Counting, writing a source and sending a packet all ask it: its
//! [`Permit`] can only be made here, and [`crate::spool::write_source`]
//! and [`crate::send::send_packets`] take one. The one send without a
//! permit is the delete request ([`crate::send::send_delete`]).
//!
//! Off, whatever else says on: the global switch [`crate::SENDING_ON`];
//! `BISE_TELEMETRY=0`; `DO_NOT_TRACK=1`; CI; a test home (a cargo test's
//! temp HOME, `BISE_TEST_HOME`); an off-screen app run; a dev build; his
//! `telemetry.share` set false; his note about it not shown yet
//! (`telemetry.note_seen`). The facts are gathered by [`Facts::read`]; the
//! rule ([`decide`]) is pure.

use bise_home::{env, prefs, test_home, Home, Lookup};

use crate::packet::Channel;

/// Everything the gate looks at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    /// `BISE_TELEMETRY`.
    pub env_telemetry: Option<String>,
    /// `DO_NOT_TRACK` (the consoledonottrack.com convention).
    pub do_not_track: Option<String>,
    /// `CI`.
    pub ci: Option<String>,
    /// A test home: a cargo test's temp HOME, or a test run's `BISE_TEST_HOME`.
    pub test_home: bool,
    /// The desktop app runs off-screen (`--offscreen`).
    pub offscreen: bool,
    /// The build's channel.
    pub channel: Channel,
    /// His `telemetry.share` (on when unset).
    pub share: bool,
    /// His `telemetry.note_seen`.
    pub note_seen: bool,
}

/// Why nothing is counted or sent: the first reason found, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Off {
    /// The global switch is off (until the plan's step 9).
    NotYet,
    /// `BISE_TELEMETRY=0`.
    Env,
    /// `DO_NOT_TRACK=1`.
    DoNotTrack,
    /// A CI run.
    Ci,
    /// A test home.
    TestHome,
    /// An off-screen app run.
    Offscreen,
    /// A dev build.
    DevBuild,
    /// He turned it off.
    Pref,
    /// His note about it was not shown yet.
    NoteNotSeen,
}

impl Off {
    /// He turned sharing off himself (his pref, `BISE_TELEMETRY=0`,
    /// `DO_NOT_TRACK=1`): what he counted is deleted. Never for the other
    /// reasons: a dev build's or a test's hub on his Mac must not delete
    /// what his release counted.
    pub fn his_choice(self) -> bool {
        match self {
            Off::Env | Off::DoNotTrack | Off::Pref => true,
            Off::NotYet | Off::Ci | Off::TestHome | Off::Offscreen | Off::DevBuild | Off::NoteNotSeen => false,
        }
    }
}

/// The proof that sharing is allowed, made only by [`sharing_allowed`].
#[derive(Debug)]
pub struct Permit(());

/// An environment value that says yes: set, and not `0`, `false`, `no` or `off`.
fn says_yes(v: &Option<String>) -> bool {
    v.as_deref().map(str::trim).is_some_and(|v| !v.is_empty() && !["0", "false", "no", "off"].contains(&v.to_ascii_lowercase().as_str()))
}

/// The rule, with the global switch an argument (so its law can check
/// every other case with the switch on).
pub fn decide(f: &Facts, switch_on: bool) -> Result<(), Off> {
    let telemetry_off = f.env_telemetry.as_deref().is_some_and(|v| !says_yes(&Some(v.to_string())));
    let checks = [
        (!switch_on, Off::NotYet),
        (telemetry_off, Off::Env),
        (says_yes(&f.do_not_track), Off::DoNotTrack),
        (says_yes(&f.ci), Off::Ci),
        (f.test_home, Off::TestHome),
        (f.offscreen, Off::Offscreen),
        (f.channel == Channel::Dev, Off::DevBuild),
        (!f.share, Off::Pref),
        (!f.note_seen, Off::NoteNotSeen),
    ];
    match checks.iter().find(|(off, _)| *off) {
        Some((_, why)) => Err(*why),
        None => Ok(()),
    }
}

/// Whether this process may count, write and send usage.
pub fn sharing_allowed(f: &Facts) -> Result<Permit, Off> {
    decide(f, crate::SENDING_ON).map(|()| Permit(()))
}

/// A permit for this crate's own tests (the spool, the send fn), which
/// run with the global switch off.
#[cfg(test)]
pub(crate) fn test_permit() -> Permit {
    Permit(())
}

impl Facts {
    /// The facts of this process: `prefs.json` from `home`, the user's
    /// variables from `env`, the test home from this process; the build's
    /// `channel` and the app's `offscreen` from the caller.
    pub fn read(home: &Home, env: Lookup, channel: Channel, offscreen: bool) -> Facts {
        let prefs = std::fs::read_to_string(home.prefs_file()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        Facts {
            env_telemetry: env("BISE_TELEMETRY"),
            do_not_track: env("DO_NOT_TRACK"),
            ci: env("CI"),
            test_home: test_home::active() || env::test_setting(test_home::JAIL_VAR).is_some(),
            offscreen,
            channel,
            share: prefs::telemetry_share(prefs.as_ref()),
            note_seen: prefs::telemetry_note_seen(prefs.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A user's Mac on a release, his note seen, nothing set.
    fn his_mac() -> Facts {
        Facts {
            env_telemetry: None,
            do_not_track: None,
            ci: None,
            test_home: false,
            offscreen: false,
            channel: Channel::Stable,
            share: true,
            note_seen: true,
        }
    }

    /// The law "never a send outside a human" (plan, les tests): each case
    /// alone turns sharing off, even with the global switch on.
    #[test]
    fn law_every_off_case_alone_refuses() {
        assert_eq!(decide(&his_mac(), true), Ok(()), "the one case that shares");
        let cases: Vec<(Facts, Off)> = vec![
            (Facts { env_telemetry: Some("0".into()), ..his_mac() }, Off::Env),
            (Facts { env_telemetry: Some("off".into()), ..his_mac() }, Off::Env),
            (Facts { do_not_track: Some("1".into()), ..his_mac() }, Off::DoNotTrack),
            (Facts { ci: Some("true".into()), ..his_mac() }, Off::Ci),
            (Facts { test_home: true, ..his_mac() }, Off::TestHome),
            (Facts { offscreen: true, ..his_mac() }, Off::Offscreen),
            (Facts { channel: Channel::Dev, ..his_mac() }, Off::DevBuild),
            (Facts { share: false, ..his_mac() }, Off::Pref),
            (Facts { note_seen: false, ..his_mac() }, Off::NoteNotSeen),
        ];
        for (f, why) in cases {
            assert_eq!(decide(&f, true), Err(why), "{f:?}");
        }
        assert_eq!(decide(&Facts { env_telemetry: Some("1".into()), do_not_track: Some("0".into()), ci: Some("false".into()), ..his_mac() }, true), Ok(()));
    }

    /// Until step 9, the switch is off: nothing anywhere is allowed.
    #[test]
    fn law_the_global_switch_is_off() {
        // step 9 (Gabriel's go) changes this with privacy.html, in one commit
        assert_eq!(sharing_allowed(&his_mac()).unwrap_err(), Off::NotYet);
    }

    /// A cargo test runs on a temp HOME: its facts say test home, and an
    /// unset pref reads as share on, note not seen.
    #[test]
    fn a_test_process_reads_as_a_test_home() {
        let home = Home::at(std::env::temp_dir().join(format!("bise-usage-facts-{}", std::process::id())));
        let f = Facts::read(&home, &|_| None, Channel::Stable, false);
        assert!(f.test_home);
        assert!(f.share && !f.note_seen);
        assert_eq!(decide(&f, true), Err(Off::TestHome));
    }
}
