//! What to do before touching bise's keychain (pure, architect m_13735
//! rule 1): from the two keychains' states, read without asking
//! ([`crate::status`]), the next step. "Never a dialog" is proved here,
//! on every pair, not by locking a keychain: no step that runs
//! /usr/bin/security is planned while a keychain it touches is locked
//! (law `law_no_security_call_on_a_locked_keychain`).
//!
//! Two keychains: bise's own (`~/.bise/secrets/bise.keychain-db`, the
//! items, closed to the agents' sandbox) and the login keychain (its
//! password item, and the items of a stub from before, v2026.10.2-28).

use crate::status::Status;

/// Why bise touches its keychain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// read items: a missing keychain means no secret
    Read,
    /// write items: a missing keychain is made
    Write,
}

/// The next step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// bise's keychain is open: go (security on it)
    Go,
    /// read its password item in the login keychain, then unlock it (no
    /// dialog: the password is given)
    UnlockWith,
    /// make it: its password item in the login keychain, then the file
    Create,
    /// nothing there to read (a read, no keychain file)
    Absent,
    /// a keychain is locked: say so, run nothing
    Locked,
    /// a keychain can't be read at all (the sandbox, a broken file)
    Unreadable,
}

/// The step for `intent`, bise's keychain being `bise` and the login
/// keychain `login`.
pub fn next_step(intent: Intent, bise: Status, login: Status) -> Step {
    match (bise, login) {
        (Status::Unlocked, _) => Step::Go,
        (Status::Unreadable, _) => Step::Unreadable,
        (Status::Missing, _) if intent == Intent::Read => Step::Absent,
        (_, Status::Locked) => Step::Locked,
        (_, Status::Missing | Status::Unreadable) => Step::Unreadable,
        (Status::Locked, Status::Unlocked) => Step::UnlockWith,
        (Status::Missing, Status::Unlocked) => Step::Create,
    }
}

/// The step for the login keychain alone (a stub from before: its items
/// are there).
pub fn login_step(login: Status) -> Step {
    match login {
        Status::Unlocked => Step::Go,
        Status::Locked => Step::Locked,
        Status::Missing => Step::Absent,
        Status::Unreadable => Step::Unreadable,
    }
}

/// The keychains a step runs /usr/bin/security on: (bise's, the login
/// one).
pub fn touches(step: Step) -> (bool, bool) {
    match step {
        Step::Go => (true, false),
        // the password item, then (unlocked) bise's items
        Step::UnlockWith => (true, true),
        Step::Create => (true, true),
        Step::Absent | Step::Locked | Step::Unreadable => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Status; 4] = [Status::Unlocked, Status::Locked, Status::Missing, Status::Unreadable];

    #[test]
    fn every_pair_has_its_step() {
        use Status::*;
        use Step as S;
        let table = [
            (Unlocked, Locked, S::Go, S::Go),
            (Locked, Unlocked, S::UnlockWith, S::UnlockWith),
            (Locked, Locked, S::Locked, S::Locked),
            (Locked, Missing, S::Unreadable, S::Unreadable),
            (Missing, Unlocked, S::Absent, S::Create),
            (Missing, Locked, S::Absent, S::Locked),
            (Missing, Missing, S::Absent, S::Unreadable),
            (Unreadable, Unlocked, S::Unreadable, S::Unreadable),
        ];
        for (b, l, read, write) in table {
            assert_eq!(next_step(Intent::Read, b, l), read, "read {b:?} {l:?}");
            assert_eq!(next_step(Intent::Write, b, l), write, "write {b:?} {l:?}");
        }
        assert_eq!(login_step(Locked), S::Locked);
        assert_eq!(login_step(Unlocked), S::Go);
    }

    /// Law (architect m_13735, main m_13728): no step runs
    /// /usr/bin/security on a keychain that is locked, so macOS never
    /// shows its password dialog; a locked one is always Step::Locked
    /// unless the step doesn't touch it.
    #[test]
    fn law_no_security_call_on_a_locked_keychain() {
        for intent in [Intent::Read, Intent::Write] {
            for b in ALL {
                for l in ALL {
                    let step = next_step(intent, b, l);
                    let (on_bise, on_login) = touches(step);
                    // UnlockWith opens bise's keychain itself, with its password
                    let bise_ok = !on_bise || b == Status::Unlocked || matches!(step, Step::UnlockWith | Step::Create);
                    assert!(bise_ok, "{intent:?} {b:?} {l:?} -> {step:?} runs security on bise's keychain");
                    assert!(!on_login || l == Status::Unlocked, "{intent:?} {b:?} {l:?} -> {step:?} runs security on a {l:?} login keychain");
                    assert!(!on_bise || b != Status::Unreadable, "{step:?} on an unreadable keychain");
                }
            }
            for l in ALL {
                let step = login_step(l);
                assert!(step != Step::Go || l == Status::Unlocked, "{l:?} -> {step:?}");
            }
        }
    }
}
