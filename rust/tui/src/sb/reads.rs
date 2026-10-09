//! The switchboard side of the live reads of a thread's entries
//! (`crate::entry_reads`, client-protocol step 4, P4d-reads): the few
//! things of `Sb` and of a feed those reads touch, so the reads stay
//! pure there and `Sb` keeps its fields to itself.


use super::*;
use crate::entry_reads::Seen;

impl Sb {
    /// The hub's burst is over: what comes now is live (as `ready`).
    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    /// What the reads remember of `agent`'s thread.
    pub(crate) fn seen_of(&mut self, agent: &str) -> &mut Seen {
        self.seen.entry(agent.to_string()).or_default()
    }

    /// Something asked for him (zen, BISE-121).
    pub(crate) fn called(&mut self) {
        self.calls += 1;
    }

    /// `agent`'s feed moved out of view (its dot in the panel).
    pub(crate) fn light(&mut self, agent: &str) {
        self.activity.insert(agent.to_string());
    }

    /// He answered item `id` here: its fold is in `agent`'s feed already.
    pub(crate) fn folded_here(&self, id: u64, agent: &str) -> bool {
        self.folded_in(id, agent)
    }
}

/// BISE-307: the fold of his answer to item `id` (`text`, in `agent`'s
/// feed) opens on what the item asked: the inbox's card while the hub
/// still holds it, else the question its card entry said in that thread
/// (`here`).
pub(crate) fn ask_answer(app: &mut App, agent: &str, id: u64, text: &str, here: Option<String>) {
    let asked = app.sb.card_by_id(id).map(|c| c.text.trim().to_string()).or(here);
    with_feed(app, agent, |app| {
        let at = app.events.iter().rposition(|e| matches!(e, Ev::Approval { text: t, .. } if t == text));
        if let Some(i) = at {
            ask_of(app, i..i + 1, id, asked);
        }
    });
}

/// BISE-89: `agent`'s turn ended, the oldest queued message goes (and
/// the next one waits for the turn it starts).
pub(crate) fn queue_next(app: &mut App, agent: &str) {
    let mut queued = None;
    with_feed(app, agent, |app| {
        queued = crate::queue::next(app);
        if queued.is_some() {
            app.pending = true;
        }
    });
    if let Some(m) = queued {
        app.sb.send_input_to(agent, m);
    }
}

/// Tests: the hub's burst is over and `focus` is in view.
#[cfg(test)]
pub(crate) fn test_view(app: &mut App, focus: &str) {
    app.sb.ready = true;
    app.sb.focus = focus.to_string();
}

#[cfg(test)]
impl Sb {
    /// Tests: `agent`'s dot is lit.
    pub(crate) fn lit(&self, agent: &str) -> bool {
        self.activity.contains(agent)
    }
}
