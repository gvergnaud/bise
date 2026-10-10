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
    /// the moves made by themselves this core run, (from, to): one each
    /// (architect m_15957: never a loop of moves)
    tried: Vec<(String, String)>,
    /// a move ended well (or the new hub answered): an older answer
    /// after it is a rollback, never moved again by itself
    there: bool,
    /// the new hub answered `initialize` while this move ran (a move that
    /// says Late afterwards came up all the same)
    answered: bool,
    /// the home connections made so far, and the last one made before a
    /// move ended well: an older answer on it is the old hub's, late
    conns: u64,
    stale_upto: Option<u64>,
    /// the last connection made before a move said Late: an older answer
    /// on a later one means it went back (architect m_16077)
    late_upto: u64,
    /// how the last move failed: the refusal is shown, nothing moves
    /// until he says `try again`
    failed: Option<Failed>,
}

/// How a move failed (the refusal's words, and whether the reader holds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ambient) enum Failed {
    /// the hub didn't take the switch: the reader holds (no reconnect)
    Refused,
    /// it took it but wasn't up within the bound: the reader goes on, so
    /// the new hub's `initialize` clears it when it comes up after all
    Late,
    /// it switched, then went back to the older one: the reader holds
    Back,
}

/// The quiet line while the move runs (designer m_15492, (a)).
pub(super) fn moving_words(to: &str) -> String {
    format!("updating the bise running here to {to}… your agents keep running.")
}

/// The last line of every refusal: where his agents are.
fn agents_line(from: &str) -> String {
    if from.is_empty() { "your agents are still running on it.".to_string() } else { format!("your agents are still running on {from}.") }
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
    lines.push(agents_line(from));
    lines.join("\n")
}

/// The refusal after a rollback: it switched, then went back (designer
/// m_15974).
pub(super) fn back_words(from: &str, to: &str) -> String {
    let back = if from.is_empty() { "the older one".to_string() } else { from.to_string() };
    [format!("the bise running in this folder switched to this one ({to}), but it didn't start right, so it went back to {back}."), agents_line(from)].join("\n")
}

impl Core {
    /// `bise ambient-core`'s move port (mod.rs core_main).
    pub fn set_home_move(&mut self, ids: Box<dyn Fn() -> (String, String)>, start: Box<dyn Fn()>) {
        self.home_move = Some(HomeMoveState {
            ids,
            start,
            from: String::new(),
            to: String::new(),
            moving: false,
            tried: Vec::new(),
            there: false,
            answered: false,
            conns: 0,
            stale_upto: None,
            late_upto: 0,
            failed: None,
        });
    }

    fn home_project(&self) -> String {
        bise_home::hub_id(Path::new(&self.workspace))
    }

    /// The home hub answered `initialize` the older way (core/home.rs's
    /// seam, proto-zone-a): move it, once per (from, to) this core run.
    /// While it moves, after it failed, or the old hub's late answer: no
    /// change. After a move that ended well (a rollback) or one already
    /// made: the refusal, the reader held, until `try again`. No port (a
    /// test core): the older hub's refusal, as before.
    pub(super) fn home_older(&mut self) {
        let project = self.home_project();
        let Some(m) = self.home_move.as_mut() else {
            let why = "this folder's bise is older than this app: update bise".to_string();
            self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
            return;
        };
        // while it moves, after it was refused or went back, or the old
        // hub's late answer on a connection made before the move ended
        let late = m.failed == Some(Failed::Late);
        if m.moving || (m.failed.is_some() && !late) || m.stale_upto.is_some_and(|s| m.conns <= s) || (late && m.conns <= m.late_upto) {
            return;
        }
        let (from, to) = (m.ids)();
        // after a Late move, a new connection that answers older: it went
        // back (a slow switch whose probation rolled back)
        if late || m.there || m.tried.contains(&(from.clone(), to.clone())) {
            // it went back (probation): never moved again by itself
            m.failed = Some(Failed::Back);
            let why = back_words(&from, &to);
            m.from = from;
            m.to = to;
            self.hub.backoff(false);
            self.hub.hold(true);
            self.hub_up = Some(false);
            self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
            return;
        }
        self.home_move_start(from, to);
    }

    /// The move starts: once more for this (from, to), its quiet note.
    fn home_move_start(&mut self, from: String, to: String) {
        let project = self.home_project();
        let Some(m) = self.home_move.as_mut() else { return };
        m.tried.push((from.clone(), to.clone()));
        m.from = from;
        m.to = to.clone();
        m.moving = true;
        m.answered = false;
        m.there = false;
        (m.start)();
        self.hub_up = Some(false);
        let ws = self.workspace.clone();
        self.emit(json!({"ev": "hub", "up": false, "workspace": ws, "project": project, "note": moving_words(&to)}));
    }

    /// The move ended. There (or the new hub answered meanwhile): the
    /// home connection says `initialize` to it; else the refusal, once:
    /// Refused holds the reader, Late lets it go on (the new hub may still
    /// come up, `home_answered` clears it then).
    pub fn home_moved(&mut self, end: MoveEnd) {
        let project = self.home_project();
        let Some(m) = self.home_move.as_mut() else { return };
        m.moving = false;
        if end == MoveEnd::There || m.answered {
            m.there = true;
            m.stale_upto = Some(m.conns);
            return;
        }
        let why = failed_words(&m.from, &m.to, &end);
        m.failed = Some(if end == MoveEnd::Late { Failed::Late } else { Failed::Refused });
        if end == MoveEnd::Late {
            // it may still come up: the reader goes on, backing off
            m.late_upto = m.conns;
            self.hub.backoff(true);
        } else {
            self.hub.hold(true);
        }
        self.hub_up = Some(false);
        self.emit(json!({"ev": "hub_refused", "project": project, "error": why}));
    }

    /// A home connection is up (home.rs home_up): counted; true while a
    /// move is under way or failed, so a reconnection to the older hub
    /// isn't news for the window (`home_answered` says it is up).
    pub(super) fn home_conn_up(&mut self) -> bool {
        let Some(m) = self.home_move.as_mut() else { return false };
        m.conns += 1;
        m.moving || m.failed.is_some()
    }

    /// The home hub answered `initialize` (home.rs): a hub of this
    /// version. A move under way or failed (Late: it came up after all)
    /// is over: the failure is cleared and the window hears it is up,
    /// which clears its note or its refusal.
    pub(super) fn home_answered(&mut self) {
        let Some(m) = self.home_move.as_mut() else { return };
        let held = m.moving || m.failed.is_some();
        if m.moving {
            m.answered = true;
        }
        if m.failed.take().is_some() || m.moving {
            m.there = true;
        }
        self.hub.backoff(false);
        if held && self.hub_up != Some(true) {
            self.hub_up = Some(true);
            let ws = self.workspace.clone();
            self.emit(json!({"ev": "hub", "up": true, "workspace": ws}));
        }
    }

    /// `hub_retry` on the home project after a failed move: move it
    /// again (the reader connects again). False: not that case (the
    /// projects' own retry).
    pub(super) fn home_move_retry(&mut self, project: &str) -> bool {
        if project != self.home_project() {
            return false;
        }
        let Some(m) = self.home_move.as_mut().filter(|m| m.failed.is_some()) else { return false };
        m.failed = None;
        let (from, to) = (m.ids)();
        self.home_move_start(from, to);
        self.hub.backoff(false);
        self.hub.hold(false);
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
        assert_eq!(
            back_words(from, to),
            "the bise running in this folder switched to this one (v2026.10.2-30), but it didn't start right, so it went back to v2026.10.2-28.
your agents are still running on v2026.10.2-28."
        );
        // no word of 'hub' anywhere he reads
        for w in [moving_words(to), failed_words(from, to, &MoveEnd::Late), failed_words("", to, &MoveEnd::Refused("x".into())), back_words(from, to)] {
            assert!(!w.contains("hub"), "{w}");
        }
    }
}
