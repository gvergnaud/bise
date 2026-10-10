//! The core moves an older home hub to its own version (architect
//! m_15476, client-protocol's one release): the home hub answers
//! `initialize` as Older (a hub from before client-protocol, still
//! running from an older install), so instead of refusing, the core asks
//! it to switch to this core's version (the binary's one move,
//! `switchboard::switch::move_hub`, handed in as [`super::super::HomeMove`])
//! and its connection says `initialize` again once the new hub listens.
//! Only a move that failed or timed out shows `hub_refused`, once, with
//! designer's words (m_15492) and the window's `try again`
//! (`hub_retry {project: home}`), which moves it again.

use super::Core;
use crate::ambient::MoveEnd;
use serde_json::json;
use std::path::Path;

/// The move's ports and where it stands.
pub(in crate::ambient) struct HomeMoveState {
    /// the versions as `/version` shows them: the running hub's, this core's
    ids: Box<dyn Fn() -> (String, String)>,
    /// starts the move on its thread; its end comes as `In::HomeMoved`
    start: Box<dyn Fn()>,
    /// the versions of the move under way or last ended
    from: String,
    to: String,
    /// a move is under way
    moving: bool,
    /// the home connections made so far, and the last one made before a
    /// move ended well: an older answer on it is the old hub's, late
    conns: u64,
    stale_upto: Option<u64>,
    /// the last move failed: the refusal is shown, nothing moves until
    /// he says `try again`
    failed: bool,
}

/// The quiet line while the move runs (designer m_15492, (a)).
pub(super) fn moving_words(to: &str) -> String {
    format!("updating the bise running here to {to}… your agents keep running.")
}

/// The refusal of a move that failed or timed out (designer m_15492,
/// (b)): its lines joined by `\n` (the window keeps them).
pub(super) fn failed_words(from: &str, to: &str, end: &MoveEnd) -> String {
    let older = if from.is_empty() { "is older".to_string() } else { format!("is older ({from})") };
    let mut lines = vec![match end {
        MoveEnd::Late => format!("the bise running in this folder {older} and didn't switch to this one ({to}) within 30 s."),
        _ => format!("the bise running in this folder {older} and couldn't switch to this one ({to})."),
    }];
    if let MoveEnd::Refused(why) = end {
        let why = why.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        if !why.is_empty() {
            lines.push(why.to_string());
        }
    }
    lines.push(if from.is_empty() { "your agents are still running on it.".to_string() } else { format!("your agents are still running on {from}.") });
    lines.join("\n")
}

impl Core {
    /// `bise ambient-core`'s move port (mod.rs core_main).
    pub fn set_home_move(&mut self, ids: Box<dyn Fn() -> (String, String)>, start: Box<dyn Fn()>) {
        self.home_move = Some(HomeMoveState { ids, start, from: String::new(), to: String::new(), moving: false, failed: false, conns: 0, stale_upto: None });
    }

    fn home_project(&self) -> String {
        bise_home::hub_id(Path::new(&self.workspace))
    }

    /// The home hub answered `initialize` the older way (core/home.rs's
    /// seam, proto-zone-a): move it, once
    /// (a reconnection to the same older hub while it moves, or after a
    /// failed move, changes nothing). No port (a test core): the older
    /// hub's refusal, as before.
    pub(super) fn home_older(&mut self) {
        let project = self.home_project();
        let Some(m) = self.home_move.as_mut() else {
            let why = "this folder's bise is older than this app: update bise".to_string();
            self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
            return;
        };
        // while it moves, after it failed, or the old hub's late answer
        // on a connection made before the move ended
        if m.moving || m.failed || m.stale_upto.is_some_and(|s| m.conns <= s) {
            return;
        }
        let (from, to) = (m.ids)();
        m.from = from;
        m.to = to.clone();
        m.moving = true;
        (m.start)();
        self.hub_up = Some(false);
        let ws = self.workspace.clone();
        self.emit(json!({"ev": "hub", "up": false, "workspace": ws, "project": project, "note": moving_words(&to)}));
    }

    /// The move ended. There: the home connection says `initialize`
    /// again when the new hub listens (its `hub` up clears the note);
    /// else the refusal, once, until he says `try again`.
    pub fn home_moved(&mut self, end: MoveEnd) {
        let project = self.home_project();
        let Some(m) = self.home_move.as_mut() else { return };
        m.moving = false;
        if end == MoveEnd::There {
            m.stale_upto = Some(m.conns);
            return;
        }
        m.failed = true;
        let why = failed_words(&m.from, &m.to, &end);
        self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
    }

    /// A home connection is up (home.rs home_up): counted; true while a
    /// move is under way or failed, so a reconnection to the older hub
    /// isn't news for the window.
    pub(super) fn home_conn_up(&mut self) -> bool {
        let Some(m) = self.home_move.as_mut() else { return false };
        m.conns += 1;
        m.moving || m.failed
    }

    /// `hub_retry` on the home project after a failed move: move it
    /// again. False: not that case (the projects' own retry).
    pub(super) fn home_move_retry(&mut self, project: &str) -> bool {
        if project != self.home_project() {
            return false;
        }
        let Some(m) = self.home_move.as_mut().filter(|m| m.failed) else { return false };
        m.failed = false;
        self.home_older();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_are_designers() {
        let (from, to) = ("v2026.10.2-28", "v2026.10.2-30");
        assert_eq!(moving_words(to), "updating the bise running here to v2026.10.2-30… your agents keep running.");
        assert_eq!(
            failed_words(from, to, &MoveEnd::Refused("a switch is already running\n".into())),
            "the bise running in this folder is older (v2026.10.2-28) and couldn't switch to this one (v2026.10.2-30).\na switch is already running\nyour agents are still running on v2026.10.2-28."
        );
        assert_eq!(
            failed_words(from, to, &MoveEnd::Refused(String::new())),
            "the bise running in this folder is older (v2026.10.2-28) and couldn't switch to this one (v2026.10.2-30).\nyour agents are still running on v2026.10.2-28."
        );
        assert_eq!(
            failed_words(from, to, &MoveEnd::Late),
            "the bise running in this folder is older (v2026.10.2-28) and didn't switch to this one (v2026.10.2-30) within 30 s.\nyour agents are still running on v2026.10.2-28."
        );
        // no word of 'hub' anywhere he reads
        for w in [moving_words(to), failed_words(from, to, &MoveEnd::Late), failed_words("", to, &MoveEnd::Refused("x".into()))] {
            assert!(!w.contains("hub"), "{w}");
        }
    }
}
