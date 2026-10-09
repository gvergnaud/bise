//! The terminal's live reads of a thread's entries (client-protocol
//! step 4, P4d-reads; plan v2 §2, architect m_13977): what an entry of
//! the hub's fold (`bise_proto::thread::Entry`) does besides its feed
//! rows. The feed (P4d-feed) calls, outside `with_feed`:
//!
//! - [`skip`] before it places an entry: true, it places nothing (his
//!   answer to an item, folded here already when he gave it);
//! - [`on_entry`] after it placed an entry, new or changed (`live`: a
//!   `thread/entry` notification, not a page);
//! - [`on_turn`] after it drew a turn's start or end (hub/agents).
//!
//! The reads, each a pure rule over the entry ([`reads_of`]) with its
//! law in `entry_reads_tests.rs`:
//!
//! 1. BISE-61 level 3: the first live message between agents in view;
//! 2. BISE-15 steered: the first steering the model read (his message
//!    in view moves from received to read, `Entry.delivery`);
//! 3. zen (BISE-121): a live card or message to him, in any feed;
//! 4. the activity dot: a live entry he would see, out of view;
//! 5. the answer fold and BISE-307's asked (`ApprovalFold.card`);
//! 6. the queue (BISE-89): at a turn's end the oldest queued message goes;
//! 7. voice mode: the live replies and turns of an agent;
//! 8. line mode (`sb/client.rs`): an entry printed as `[agent] …` lines.
//!
//! An entry comes again when it changes (same pos): each read fires
//! once per pos ([`Seen`]); the answer fold and BISE-307 hold every time
//! the entry is placed (the feed draws it afresh).

// TODO(client-protocol P4d-feed, proto-zone-b): the feed calls skip,
// on_entry and on_turn when it switches to entries (the same sha
// deletes sb.rs's ingest_for and its line reads); until then only line
// mode and the laws read this module.
#![cfg_attr(not(test), allow(dead_code))]

use crate::app::App;
use crate::wire::Ev;
use bise_proto::thread::{Delivery, Entry, EntryKind};
use bise_proto::Pos;

/// His messages' marks kept per thread (a mark moves within the last
/// few messages: older ones are read already).
const MARKS: usize = 64;

/// What the reads remember of one thread: its newest pos seen, and the
/// last mark of his recent messages.
#[derive(Default)]
pub(crate) struct Seen {
    high: Option<Pos>,
    marks: Vec<(Pos, Delivery)>,
    /// its recent items' questions (BISE-307: an answer's fold opens on
    /// what the item asked)
    asks: Vec<(u64, String)>,
}

impl Seen {
    /// Entry `pos` is new (never seen), and it is seen now.
    fn take(&mut self, pos: Pos) -> bool {
        let new = self.high.is_none_or(|h| pos > h);
        if new {
            self.high = Some(pos);
        }
        new
    }

    /// His message `pos`'s mark before this one (None: first seen), and
    /// `now` is kept.
    fn mark(&mut self, pos: Pos, now: Delivery) -> Option<Delivery> {
        let was = self.marks.iter().position(|(p, _)| *p == pos).map(|i| self.marks.remove(i).1);
        self.marks.push((pos, now));
        if self.marks.len() > MARKS {
            self.marks.remove(0);
        }
        was
    }

    /// Item `id` asked `question` in this thread.
    fn ask(&mut self, id: u64, question: &str) {
        self.asks.retain(|(i, _)| *i != id);
        self.asks.push((id, question.trim().to_string()));
        if self.asks.len() > MARKS {
            self.asks.remove(0);
        }
    }

    /// What item `id` asked in this thread, if it did here.
    pub(crate) fn asked(&self, id: u64) -> Option<&str> {
        self.asks.iter().find(|(i, _)| *i == id).map(|(_, q)| q.as_str())
    }
}

/// Where an entry is read: `live` (after the hub's burst, not a page),
/// in the feed `in_focus`, with voice mode on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct At {
    pub(crate) live: bool,
    pub(crate) in_focus: bool,
    pub(crate) voice: bool,
}

/// What one entry does besides its rows.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Reads {
    /// BISE-61: the level-3 hint (a message between agents)
    pub(crate) level3: bool,
    /// BISE-15: the steering hint (the model read his message mid-turn)
    pub(crate) steered: bool,
    /// zen: something asked for him
    pub(crate) call: bool,
    /// the panel's dot of an agent out of view
    pub(crate) activity: bool,
    /// voice mode: the reply to say
    pub(crate) said: Option<String>,
}

/// The reads of entry `e` at `at`, `seen` the thread's memory (updated).
pub(crate) fn reads_of(seen: &mut Seen, at: At, e: &Entry) -> Reads {
    let new = seen.take(e.pos);
    let was = match (e.kind, e.delivery) {
        (EntryKind::You, Some(d)) => seen.mark(e.pos, d),
        _ => None,
    };
    if let Some(c) = &e.card {
        seen.ask(c.id, &c.question);
    }
    let once = at.live && new;
    let reply = e.kind == EntryKind::Agent && !e.to_you && !e.text.is_empty();
    Reads {
        level3: once && at.in_focus && e.kind == EntryKind::FromAgent,
        steered: at.live && at.in_focus && was == Some(Delivery::Received) && e.delivery == Some(Delivery::Read),
        call: once && (e.kind == EntryKind::Card || e.to_you),
        activity: once && !at.in_focus && lights(e.kind),
        said: (once && at.voice && reply).then(|| e.text.clone()),
    }
}

/// An entry that lights an agent out of view: its words or the hub's
/// (what the TUI lit on before, an assistant line or a hub line), not
/// its tools, its reasoning or a compaction (his own message, a hub
/// line, lights as it did).
pub(crate) fn lights(kind: EntryKind) -> bool {
    !matches!(
        kind,
        EntryKind::Tools
            | EntryKind::Thinking
            | EntryKind::Compacting
            | EntryKind::Compacted
            | EntryKind::Page
            | EntryKind::Report
            | EntryKind::TurnFailed
            | EntryKind::Unknown
    )
}

/// The item a fold of his answer is for (the hub's route line).
pub(crate) fn answers(e: &Entry) -> Option<u64> {
    e.approval.as_ref()?.card
}

/// Entry `e` of `agent`'s thread is not placed: his answer to an item he
/// gave here, whose fold the TUI drew when he gave it. The same answer
/// for the same entry every time.
pub(crate) fn skip(app: &App, agent: &str, e: &Entry) -> bool {
    answers(e).is_some_and(|id| app.sb.folded_here(id, agent))
}

/// Entry `e` of `agent`'s thread was placed in its feed (`live`: a
/// notification, not a page): its reads.
pub(crate) fn on_entry(app: &mut App, agent: &str, e: &Entry, live: bool) {
    let at = At { live: live && app.sb.is_ready(), in_focus: app.sb.focus_name() == agent, voice: app.voice_mode.is_some() };
    let r = reads_of(app.sb.seen_of(agent), at, e);
    if r.call {
        app.sb.called();
    }
    if r.activity {
        app.sb.light(agent);
    }
    if let (Some(id), Some(a)) = (answers(e), e.approval.as_ref()) {
        let here = app.sb.seen_of(agent).asked(id).map(str::to_string);
        crate::sb::ask_answer(app, agent, id, &a.text, here);
    }
    if let Some(text) = r.said {
        crate::voicemode::live::on_events(app, agent, &[Ev::Assistant(text)]);
    }
    crate::sb::queue_next(app, agent);
    if r.level3 {
        crate::hints::once(app, crate::hints::Hint::FirstLevel3);
    }
    if r.steered {
        crate::hints::once(app, crate::hints::Hint::FirstSteer);
    }
}

/// The hub's newest entry of a thread moved past what this terminal has
/// seen of it (plan v2 read 4: `hub/agents`' `last_pos`).
pub(crate) fn head_moved(seen: &Seen, last_pos: Option<Pos>) -> bool {
    last_pos.is_some_and(|p| seen.high.is_none_or(|h| p > h))
}

/// `agent`'s row says its newest entry is `last_pos`: out of view, a
/// thread this terminal doesn't follow lights its dot when it moves (a
/// followed one lights by its entries, [`on_entry`]). Called by the
/// agents reader for the threads it hasn't subscribed (P4e).
pub(crate) fn on_head(app: &mut App, agent: &str, last_pos: Option<Pos>) {
    if !app.sb.is_ready() || app.sb.focus_name() == agent {
        return;
    }
    let seen = app.sb.seen_of(agent);
    if head_moved(seen, last_pos) {
        seen.high = last_pos;
        app.sb.light(agent);
    }
}

/// `agent`'s turn `started` or ended (its feed drew it): voice mode
/// hears it live; at its end the oldest queued message goes.
pub(crate) fn on_turn(app: &mut App, agent: &str, started: bool) {
    if app.voice_mode.is_some() && app.sb.is_ready() {
        let ev = if started { Ev::Turn } else { Ev::TurnDone };
        crate::voicemode::live::on_events(app, agent, &[ev]);
    }
    if !started {
        crate::sb::queue_next(app, agent);
    }
}

/// Line mode: entry `e` of `agent`'s thread as `[agent] …` lines, in
/// the words line mode printed for its line (`sb/client.rs`); newlines
/// read ` ⏎ `. A kind whose line said more than its entry keeps (a
/// card's head, a hub line's raw fields) prints `<kind> : <text>`.
pub(crate) fn line_of(agent: &str, e: &Entry) -> Vec<String> {
    let one = |s: &str| s.replace('\n', " ⏎ ");
    let said = |what: String| vec![format!("[{agent}] {what}")];
    let from = e.from.as_deref().unwrap_or_default();
    let id = e.msg.map(|m| format!(" m_{m}")).unwrap_or_default();
    match e.kind {
        EntryKind::Thinking | EntryKind::Unknown => Vec::new(),
        EntryKind::You => said(format!("you : {}", one(&e.text))),
        EntryKind::Agent if e.to_you => said(format!("msg-you : {from} : {}", one(&e.text))),
        EntryKind::Agent if e.text.is_empty() => Vec::new(),
        EntryKind::Agent => said(format!("assistant: {}", one(&e.text))),
        EntryKind::FromAgent => match e.to.as_deref() {
            Some(to) => said(format!("msg : {from} → {to}{id} : {}", one(&e.text))),
            None => said(format!("msg-in : {}{from}{id} : {}", if e.to_you { "@" } else { "" }, one(&e.text))),
        },
        EntryKind::ToAgent => {
            let id = e.msg.map(|m| format!("m_{m}")).unwrap_or_default();
            said(format!("sent : {} : {id} : {} : {}", e.to.as_deref().unwrap_or_default(), u8::from(e.asks), one(&e.text)))
        }
        EntryKind::Tools => e
            .tools
            .iter()
            .flat_map(|t| t.items.iter().enumerate())
            .map(|(i, it)| {
                let call = if it.name.is_empty() { it.text.clone() } else { format!("{} : {}", it.name, it.args) };
                format!("[{agent}] tool {}", crate::render::truncate_chars(&format!("{} {}", i + 1, one(&call)), 200))
            })
            .collect(),
        k => said(format!("{} : {}", kind_word(k), one(&e.text))),
    }
}

/// The lines of `e` not printed yet, `last` the lines printed for the
/// newest entry of that thread (an entry printed again as it changes
/// prints only what it gained).
pub(crate) fn fresh(last: Option<&(Pos, Vec<String>)>, pos: Pos, lines: &[String]) -> Vec<String> {
    match last {
        Some((p, had)) if *p == pos => lines.iter().filter(|l| !had.contains(l)).cloned().collect(),
        Some((p, _)) if *p > pos => Vec::new(),
        _ => lines.to_vec(),
    }
}

/// The wire's word for a kind (`from_agent`, `not_delivered`).
fn kind_word(k: EntryKind) -> String {
    serde_json::to_value(k).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

#[cfg(test)]
#[path = "entry_reads_tests.rs"]
mod tests;
