//! expired-ux: the ChatGPT plan's sign-in expired (OpenAI refuses the
//! refresh, devkit issue #5: about every hour). Kind `signin`, for the
//! user only, ONE item whichever agents hit it.
//!
//! - An agent's turn ends on the runtime's expired line
//!   (`turn_done: failed: your ChatGPT sign-in expired. …`): the agent
//!   waits for the sign-in ([`SignIn::stopped`]), the item opens if none
//!   is open (`place` = [`PLACE`]); no BR-007 report to its parent (the
//!   item says it, and a parent on the same plan would fail too).
//! - The view's note names the agents that wait (`stopped: t1, main`).
//! - `1 sign in again`: the TUI opens its own ChatGPT sign-in (the
//!   browser), the hub does nothing; so does ⏎ in the thread of an
//!   agent that waits (the snapshot's `waiting`).
//! - The sign-in is back (the daemon reads auth.json: the TUI's sign-in,
//!   `bise login chatgpt`): `Input::SignedIn` closes the item and sends
//!   each agent that waited a message from bise: it goes on.
//! - An agent that ends another turn (a new message from the user) does
//!   not wait any more; no agent waits: the item closes.

use super::*;

pub const KIND: &str = "signin";
pub const PLACE: &str = "signin:chatgpt";

/// The runtime's line (bend/runtime/provider-pure.bend PLAN_EXPIRED).
const EXPIRED: &str = "your ChatGPT sign-in expired.";

/// A turn that ended on the expired sign-in.
pub fn expired(turn_done: &str) -> bool {
    turn_done.strip_prefix("failed: ").is_some_and(|w| w.starts_with(EXPIRED))
}

/// The item's text (designer).
pub fn text() -> String {
    "your ChatGPT sign-in expired
your agents on ChatGPT stopped. sign in again and they go on by themselves.

1. sign in again"
        .to_string()
}

/// What an agent that waited reads once the sign-in is back (the user
/// reads it too, in main's thread: designer m_7456).
pub const RESUME: &str = "your ChatGPT sign-in is back. go on where your turn stopped.";

/// The view's note: who waits.
pub fn note(stopped: &[String]) -> Option<String> {
    (!stopped.is_empty()).then(|| format!("stopped: {}", stopped.join(", ")))
}

/// The hub's side of the item (runtime: a restart forgets who waits;
/// the item, kept by sb-core, still closes at the sign-in).
#[derive(Debug, Default)]
pub struct SignIn {
    /// The agents whose turn stopped on it, in that order.
    pub stopped: Vec<String>,
}

impl Hub {
    fn signin_cards(&self) -> Vec<u64> {
        self.st.open_cards().filter(|c| c.kind == KIND).map(|c| c.id).collect()
    }

    /// An agent's turn ended: on the expired sign-in, it waits (and the
    /// item opens); on anything else, it does not wait any more. True:
    /// the turn ended on the expired sign-in (no report to its parent).
    pub(super) fn signin_turn(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, turn_done: &str) -> bool {
        if expired(turn_done) {
            if !self.signin.stopped.iter().any(|a| a == agent) {
                self.signin.stopped.push(agent.to_string());
            }
            if self.signin_cards().is_empty() {
                let text = text();
                let item = merge::HubItem { kind: KIND, agent: MAIN, text: &text, place: PLACE, pr: None };
                self.open_card(fx, env, item);
            }
            self.dirty = true;
            return true;
        }
        let before = self.signin.stopped.len();
        self.signin.stopped.retain(|a| a != agent);
        if before > 0 && self.signin.stopped.is_empty() {
            for id in self.signin_cards() {
                self.close_card(fx, env, id, "no agent waits for it");
            }
            self.dirty = true;
        }
        false
    }

    /// The sign-in is back (auth.json): the item closes, the agents that
    /// waited go on.
    pub(super) fn signed_in(&mut self, fx: &mut Fx, env: &mut dyn Env) {
        let cards = self.signin_cards();
        if cards.is_empty() && self.signin.stopped.is_empty() {
            return;
        }
        for id in cards {
            self.close_card(fx, env, id, "signed in again");
        }
        for agent in std::mem::take(&mut self.signin.stopped) {
            if agent == MAIN || self.st.agents.contains_key(&agent) {
                self.core(fx, env, None, json!({"t": "hub_msg", "to": agent, "text": RESUME}));
            }
        }
        self.dirty = true;
    }

    /// The view's note on the item: who waits.
    pub(super) fn signin_note(&self) -> Option<String> {
        note(&self.signin.stopped)
    }

    /// The agents that wait, for the view (⏎ in their thread signs in).
    pub(super) fn signin_waiting(&self) -> Vec<String> {
        self.signin.stopped.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_expired_line_is_found() {
        assert!(expired(
            "failed: your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."
        ));
        assert!(!expired("completed"));
        assert!(!expired("failed: your ChatGPT plan's limit for bise is reached."));
        assert!(!expired("your ChatGPT sign-in expired."));
    }

    #[test]
    fn the_note_names_who_waits() {
        assert_eq!(note(&[]), None);
        assert_eq!(note(&["t1".into(), "main".into()]).as_deref(), Some("stopped: t1, main"));
    }
}
