//! The hub's durable state (RFC 0001 §12), as the views read it. sb-core
//! (hub/*.bend) is its only source of truth: it applies the journal
//! events and sends this state back after each step (core.rs stores it);
//! the runtime-only fields (`Agent::run`, `Agent::waiting`) are set by
//! the core from REPL activity and are never journaled.

use bise_proto::core::body::{Missing, Words};
use std::collections::{BTreeMap, BTreeSet};

/// sb-core's records, declared once in bise_proto (core::state: the
/// generated ones, and the Rust-only ones sb-core keeps as opaque JSON).
pub use bise_proto::core::state::{Brief, Card, Declared, Lifecycle, Mode, Msg, MsgState, Report, Run, Workspace};
pub use bise_proto::core::{Declaration, ViewAgent};

/// The name of the orchestrator agent. Never a task name.
pub const MAIN: &str = "main";
/// The human, as a message sender. Never an agent name.
pub const USER: &str = "user";
/// The hub itself, as the sender of notifications (a task crashed...).
/// Never an agent name.
pub const HUB: &str = "switchboard";
/// An MCP App view, as the sender of its `ui/message` (sb-core's
/// `M.VIEW()`; its plugin is the message's `view` field). Never an
/// agent name, never the user.
pub const VIEW: &str = "view";

/// The status every view shows (RFC 0001 §9.1, RFC 0003 `ListedAgent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Working,
    Waiting,
    Idle,
    Done,
    Blocked,
    Failed,
    Stopped,
    Archived,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Starting => "starting",
            Status::Working => "working",
            Status::Waiting => "waiting",
            Status::Idle => "idle",
            Status::Done => "done",
            Status::Blocked => "blocked",
            Status::Failed => "failed",
            Status::Stopped => "stopped",
            Status::Archived => "archived",
        }
    }

}

/// An agent as the hub knows it: sb-core's row of it (bise_proto's
/// generated [`ViewAgent`]: its durable record and its runtime, `run`,
/// `waiting`, `waiting_on`, `turn_ms`, `turns`), read through Deref, never
/// copied field by field; and what only the hub's Rust side knows.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub core: ViewAgent,
    /// The last thing it did: (time, "bash `cargo test`", "wrote: ...").
    pub activity: Option<(u64, String)>,
    /// BISE-136: the private git worktree it works in (`gate.sh new`,
    /// `sb worktree`), when not its own workspace.
    pub place: Option<String>,
    /// The branch checked out in `place` as the PR poller last read it
    /// (None: detached, as `gate.sh new` makes it, or not read yet).
    pub place_branch: Option<String>,
}

impl std::ops::Deref for Agent {
    type Target = ViewAgent;
    fn deref(&self) -> &ViewAgent {
        &self.core
    }
}

impl std::ops::DerefMut for Agent {
    fn deref_mut(&mut self) -> &mut ViewAgent {
        &mut self.core
    }
}

impl Agent {
    /// An agent with only sb-core's row (the hub's own facts unknown yet).
    pub fn of(core: ViewAgent) -> Agent {
        Agent { core, activity: None, place: None, place_branch: None }
    }

    /// Its declared status and note (`sb status`), when it said one.
    pub fn declared(&self) -> Option<&Declaration> {
        self.core.declared.as_ref()
    }

    pub fn status(&self) -> Status {
        match self.lifecycle {
            Lifecycle::Archived => return Status::Archived,
            Lifecycle::Stopped => return Status::Stopped,
            Lifecycle::Failed => return Status::Failed,
            Lifecycle::Active => {}
        }
        match self.run {
            Run::Down | Run::Starting => Status::Starting,
            Run::Busy if self.waiting => Status::Waiting,
            Run::Busy => Status::Working,
            Run::Idle => match self.declared().map(|d| d.status) {
                Some(Declared::Done) => Status::Done,
                Some(Declared::Blocked) => Status::Blocked,
                _ => Status::Idle,
            },
        }
    }

    /// One line: what this agent is for.
    pub fn description(&self) -> String {
        if self.is_main {
            "orchestrator: routes the user's requests to the agents".to_string()
        } else {
            crate::util::one_line(&self.brief.objective)
        }
    }
}


/// BISE-299: the kinds of the user's inbox, set at the card's creation
/// (hub/model.bend `user_kind`): main's escalations, the tool-call
/// confirmations and the hub's own items ([`choice_kind`]). The agents'
/// traffic is main's, never a card.
pub fn user_kind(kind: &str) -> bool {
    matches!(kind, "question" | "plan" | "drop" | "confirm") || choice_kind(kind)
}

/// The hub's items with numbered options, about a place
/// ([`bise_proto::rows::CHOICE_KINDS`], sb-core's hub/cardkind.bend
/// `choice_kind` pinned to it): opened and closed by the hub's Rust side,
/// the user's digit comes back as `Effect::CardChoice`.
pub use bise_proto::rows::choice_kind;

/// The durable state, as sb-core last sent it.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub agents: BTreeMap<String, Agent>,
    pub order: Vec<String>,
    /// The messages, their states and the settled ones: written only by
    /// [`State::put_msg`] and [`State::clear_msgs`], which keep the
    /// indexes built from them (`threads`, `hot`). Read with
    /// [`State::msgs`] and siblings.
    ///
    /// Every message stays here, delivered and owing nothing included,
    /// as a [`Row`]: whole, or cold (view-all-perf: `view_all` sends a
    /// cold one as a compact row). Neither has its words (c6): sb-core
    /// keeps none, [`State::words`] reads them in the journal.
    msgs: BTreeMap<u64, Row>,
    /// where each message's words are in the journal (c6, bodies.rs):
    /// filled by the daemon at the boot's read and after each append
    pub bodies: crate::bodies::Bodies,
    msg_state: BTreeMap<u64, MsgState>,
    settled: BTreeSet<u64>,
    /// main's `<agent_threads>`, kept by the same writer (board::Threads)
    threads: crate::board::Threads,
    /// the hot messages: what sb-core's view says of each (queued, or
    /// owing a reply), only those with one of them: Waiting and
    /// `unanswered_for` walk these, never every message
    hot: BTreeMap<u64, Hot>,
    pub cards: BTreeMap<u64, Card>,
    pub main_notes: Vec<String>,
    pub next_msg: u64,
    pub next_card: u64,
    /// cost-pause: since when the project is paused (sb-core's view
    /// `paused`, hub/pause.bend); None: it runs
    pub paused: Option<u64>,
    /// `sb every`'s timers (sb-core's view `timers`, `timers_ended`)
    pub timers: crate::every::Timers,
    /// `sb wake`'s watches (sb-core's view `wakes`, `wakes_ended`)
    pub wakes: crate::wake::Wakes,
}

impl State {
    /// A state with only main, working in `workspace`.
    pub fn new(workspace: &str) -> State {
        let mut st = State {
            next_msg: 1,
            next_card: 1,
            ..State::default()
        };
        st.agents.insert(
            MAIN.to_string(),
            Agent::of(ViewAgent {
                name: MAIN.to_string(),
                dir: MAIN.to_string(),
                is_main: true,
                ws: Workspace::shared(workspace),
                ..ViewAgent::empty()
            }),
        );
        st.order.push(MAIN.to_string());
        st
    }

    /// Message `id`, whole (its row as a step's view sends it: `via`,
    /// `fwd`...); None: unknown, or cold.
    pub fn msg(&self, id: u64) -> Option<&Msg> {
        self.msgs.get(&id)?.whole()
    }

    /// Message `id`'s words (its text and parts), read in the journal
    /// (bodies.rs); a [`Missing`] renders the same everywhere.
    pub fn words(&self, id: u64) -> Result<Words, Missing> {
        self.bodies.words(id)
    }

    /// Message `id`'s text, or the words that say it is missing.
    pub fn text(&self, id: u64) -> String {
        self.words(id).map(|w| w.text).unwrap_or_else(|m| m.text())
    }

    /// Message `id`'s row without its body, whole or cold.
    pub fn peek(&self, id: u64) -> Option<Peek<'_>> {
        self.msgs.get(&id).map(Row::peek)
    }

    /// Every message's row without its body, by id.
    pub fn peeks(&self) -> impl DoubleEndedIterator<Item = Peek<'_>> + '_ {
        self.msgs.values().map(Row::peek)
    }

    /// Tests only: the whole messages by id (a test hub's messages are
    /// all among view_all's newest whole ones, view.bend RECENT).
    #[cfg(test)]
    pub fn msgs(&self) -> BTreeMap<u64, &Msg> {
        self.whole_msgs().map(|m| (m.id, m)).collect()
    }

    /// The whole messages, by id (a cold one is left out).
    pub fn whole_msgs(&self) -> impl DoubleEndedIterator<Item = &Msg> + '_ {
        self.msgs.values().filter_map(Row::whole)
    }

    /// Each message's delivery state, by id.
    pub fn msg_state(&self) -> &BTreeMap<u64, MsgState> {
        &self.msg_state
    }

    /// The messages a reply settled.
    pub fn settled(&self) -> &BTreeSet<u64> {
        &self.settled
    }

    /// The one writer of a message: its row, its state (None: unchanged),
    /// whether a reply settled it, and what sb-core says of it (`hot`).
    /// `row`: whole, or cold (view_all's row of a message without its
    /// body). sb-core sends every hot message whole (view.bend
    /// `ships_whole`): [`State::hot`] reads whole rows only.
    pub fn put_msg(&mut self, row: Row, state: Option<MsgState>, settled: bool, hot: Hot) {
        let (id, thread) = (row.peek().id, row.peek().thread);
        if hot.queued || hot.owes {
            self.hot.insert(id, hot);
        } else {
            self.hot.remove(&id);
        }
        if let Some(state) = state {
            self.msg_state.insert(id, state);
        }
        if settled {
            self.settled.insert(id);
        } else {
            self.settled.remove(&id);
        }
        let was = self.msgs.insert(id, row).map(|o| o.peek().thread);
        if let Some(old) = was.filter(|t| *t != thread) {
            self.threads.take(&self.msgs, old, id);
        }
        self.threads.put(&self.msgs, thread, id);
    }

    /// Forget every message (a view that carries them all).
    pub fn clear_msgs(&mut self) {
        self.msgs.clear();
        self.msg_state.clear();
        self.settled.clear();
        self.threads = Default::default();
        self.hot.clear();
    }

    /// Tests only: message `id`'s words, as its journal line has them
    /// (the daemon's append, bodies.rs).
    #[cfg(test)]
    pub fn test_words(&mut self, id: u64, text: &str, parts: &str) {
        self.bodies.keep(&serde_json::json!({"type": "message_sent", "msg": {"id": id, "text": text, "parts": parts}}));
    }

    /// Tests only: put_msg with sb-core's facts as model.bend's
    /// predicates give them (`is_queued`; delivered, expecting a reply,
    /// not settled), the state kept when `state` is None.
    #[cfg(test)]
    pub fn put_msg_judged(&mut self, m: Msg, state: Option<MsgState>, settled: bool) {
        let st = state.clone().or_else(|| self.msg_state.get(&m.id).cloned());
        let hot = Hot {
            queued: matches!(st, Some(MsgState::Queued { .. })),
            owes: m.expect_reply && !settled && matches!(st, Some(MsgState::Delivered)),
        };
        self.put_msg(Row::Whole(m), state, settled, hot);
    }

    /// The hot messages, oldest first, with what sb-core says of each
    /// (each whole: sb-core never sends a hot one cold).
    pub fn hot(&self) -> impl Iterator<Item = (&Msg, Hot)> + '_ {
        self.hot.iter().filter_map(|(id, h)| Some((self.msg(*id)?, *h)))
    }

    /// The threads of main's `<agent_threads>` (board::agent_threads).
    pub fn threads(&self) -> &crate::board::Threads {
        &self.threads
    }

    /// The canonical name of an agent: itself, or the task an alias
    /// (an old name) points to.
    pub fn resolve(&self, name: &str) -> Option<String> {
        if self.agents.contains_key(name) {
            return Some(name.to_string());
        }
        self.agents
            .values()
            .find(|a| a.aliases.iter().any(|x| x == name))
            .map(|a| a.name.clone())
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Agent> {
        self.order
            .iter()
            .filter_map(|n| self.agents.get(n))
            .filter(|a| !a.is_main)
    }

    /// The user's inbox: an older journal's cards of other kinds are
    /// hidden until the hub's boot closes them.
    pub fn open_cards(&self) -> impl Iterator<Item = &Card> {
        self.cards.values().filter(|c| user_kind(&c.kind))
    }

    /// Tests only: a task as its creation leaves it (the real state
    /// comes from sb-core, see core.rs).
    #[cfg(test)]
    pub fn test_task(&mut self, name: &str, objective: &str) {
        let mut a = self.agents[MAIN].clone();
        a.name = name.to_string();
        a.dir = name.to_string();
        a.is_main = false;
        a.parent = Some(MAIN.to_string());
        a.brief = Brief {
            objective: objective.to_string(),
            ..Brief::default()
        };
        self.agents.insert(name.to_string(), a);
        self.order.push(name.to_string());
    }

    /// Delivered messages to `name` that expect a reply it has not given.
    pub fn unanswered_for(&self, name: &str) -> Vec<&Msg> {
        self.hot().filter(|(m, h)| h.owes && m.to == name).map(|(m, _)| m).collect()
    }

    /// `m` waits for delivery (sb-core's word, its view's `queued`).
    pub fn is_queued(&self, m: &Msg) -> bool {
        self.hot.get(&m.id).is_some_and(|h| h.queued)
    }
}

/// A message of the mirror (view-all-perf): whole, or cold: `view_all`
/// sent its compact row (view.bend `cold_json`: no `via`, `fwd`...).
/// Neither has its words: [`State::words`].
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Whole(Msg),
    Cold(Head),
}

/// A cold message's row: what the hub reads of every message (main's
/// threads, a forward's time, `sb show`'s recipient, a card's note).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Head {
    pub id: u64,
    pub thread: u64,
    pub from: String,
    pub to: String,
    pub reply_to: Option<u64>,
    pub expect_reply: bool,
    pub created_ms: u64,
    pub plain: bool,
}

impl Head {
    /// The row of `m` (its body left out).
    pub fn of(m: Msg) -> Head {
        Head { id: m.id, thread: m.thread, from: m.from, to: m.to, reply_to: m.reply_to, expect_reply: m.expect_reply, created_ms: m.created_ms, plain: m.plain }
    }
}

/// A message's row without its body, whole or cold ([`State::peek`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peek<'a> {
    pub id: u64,
    pub thread: u64,
    pub from: &'a str,
    pub to: &'a str,
    pub reply_to: Option<u64>,
    pub expect_reply: bool,
    pub created_ms: u64,
    pub plain: bool,
}

impl Row {
    pub fn peek(&self) -> Peek<'_> {
        match self {
            Row::Whole(m) => Peek { id: m.id, thread: m.thread, from: &m.from, to: &m.to, reply_to: m.reply_to, expect_reply: m.expect_reply, created_ms: m.created_ms, plain: m.plain },
            Row::Cold(h) => Peek { id: h.id, thread: h.thread, from: &h.from, to: &h.to, reply_to: h.reply_to, expect_reply: h.expect_reply, created_ms: h.created_ms, plain: h.plain },
        }
    }

    /// The message, when it is whole.
    pub fn whole(&self) -> Option<&Msg> {
        match self {
            Row::Whole(m) => Some(m),
            Row::Cold(_) => None,
        }
    }
}

/// What sb-core says of a message (its view's `ViewMsg.queued`, `owes`,
/// hub/model.bend's predicates): the hub never decides it again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hot {
    /// it waits for delivery
    pub queued: bool,
    /// it was delivered and expects a reply its recipient has not given
    pub owes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_lifecycle_then_run_then_declared() {
        let mut st = State::new("/w");
        st.test_task("a", "do it");
        let a = st.agents.get_mut("a").unwrap();
        assert_eq!(a.status(), Status::Starting);
        a.run = Run::Busy;
        assert_eq!(a.status(), Status::Working);
        a.waiting = true;
        assert_eq!(a.status(), Status::Waiting);
        a.waiting = false;
        a.run = Run::Idle;
        a.declared = Some(Declaration { status: Declared::Done, note: String::new() });
        assert_eq!(a.status(), Status::Done);
        a.lifecycle = Lifecycle::Archived;
        assert_eq!(a.status(), Status::Archived);
    }
}
