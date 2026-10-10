//! The hub's thread entries (`bise_proto::thread::Entry`, the one fold
//! of a thread's lines) as the TUI's feed events (client-protocol step 4,
//! architect m_13977 Q1): the one owner of that mapping. Pure.
//!
//! P4a: a first mapping, so the parity law (`entry_ev_tests.rs`) can say
//! what still differs from the TUI's own fold of the same lines
//! (`wire.rs` rec_ev); its named gaps are the work of P4d, and the feed
//! switches to entries only when that list is empty.

use crate::sb::{pr_look, shown_name};
use crate::render::fmt_duration;
use crate::wire::{notice_ev, CardParts, Ev, Gate, Mark, ToolData, ToolState};
use bise_proto::thread::{self as pthread, Entry, EntryKind, NoticeLevel};

/// The feed events of entry `e`, in order (none: an entry the TUI
/// doesn't draw, as an interrupt's `stopped`): a turn's start before
/// its first entry, its end (and the end's time, BISE-271) after its
/// last, as the lines' `turn_started` / `turn_done` drew them.
pub(crate) fn ev_of(e: &Entry) -> Vec<Ev> {
    let mut v = Vec::new();
    if e.turn_start {
        v.push(Ev::Turn);
    }
    v.extend(kind_evs(e));
    if let Some(t) = e.turn_end_ms {
        v.extend([Ev::TurnDone, Ev::Ended(t)]);
    }
    v
}

/// The events of entry `e`'s kind and payload.
fn kind_evs(e: &Entry) -> Vec<Ev> {
    let text = e.text.clone();
    let msg = |from: String, to: String, level: u8| Ev::AgentMsg {
        from,
        to,
        text: text.clone(),
        level,
        id: e.msg.map(|m| format!("m_{m}")).unwrap_or_default(),
        open: false,
        fold: false,
    };
    match e.kind {
        // G1: the mark the fold moved, by the TUI's own rule
        EntryKind::You => vec![Ev::You(text, e.delivery.unwrap_or(Mark::Sent), false)],
        // G5: an agent writing to him, level 2
        EntryKind::Agent | EntryKind::FromAgent if e.to_you => vec![msg(e.from.clone().unwrap_or_default(), "you".into(), 2)],
        EntryKind::Agent => {
            let mut v = Vec::new();
            if let Some(t) = &e.thinking {
                v.push(Ev::Thinking { ms: u128::from(t.ms), text: t.text.clone(), open: false });
            }
            if !text.is_empty() {
                v.push(Ev::Assistant(text));
            }
            v
        }
        EntryKind::Thinking => e.thinking.iter().map(|t| Ev::Thinking { ms: u128::from(t.ms), text: t.text.clone(), open: false }).collect(),
        EntryKind::FromAgent => vec![msg(shown_name(e.from.as_deref().unwrap_or_default()), e.to.clone().unwrap_or_default(), 3)],
        EntryKind::ToAgent => vec![msg(String::new(), e.to.clone().unwrap_or_default(), 3)],
        EntryKind::Tools => e.tools.iter().flat_map(|t| t.items.iter().enumerate().map(|(i, it)| tool(i, it))).collect(),
        EntryKind::Notice => e.notice.iter().map(|n| notice_ev(n.clone())).collect(),
        EntryKind::TurnFailed => vec![notice_ev(pthread::Notice { level: NoticeLevel::Err, text })],
        EntryKind::NotDelivered => e.not_delivered.iter().map(|n| Ev::Undelivered { name: n.to.clone(), text: n.text.clone(), open: true }).collect(),
        EntryKind::Compacting => vec![Ev::Compact],
        EntryKind::Compacted => vec![Ev::Compacted { text, open: false }],
        EntryKind::Landed => e.landed.iter().map(|l| Ev::Landed { agent: l.agent.clone(), from: l.from.clone(), sha: l.sha.clone(), files: l.files, add: l.add, del: l.del }).collect(),
        EntryKind::Pr => e.pr.iter().map(|p| Ev::Pr { tone: pr_look(p.state).into(), number: p.number, url: p.url.clone(), text: p.text.clone(), url_row: false }).collect(),
        EntryKind::Artifact => e.made.iter().map(|m| Ev::Made { id: m.id.clone(), agent: m.agent.clone(), title: m.title.clone(), kind: m.kind.clone(), v: m.v }).collect(),
        EntryKind::Answered => e.answered.iter().map(|a| Ev::Answered { agent: a.agent.clone(), question: a.question.clone(), answer: a.answer.clone(), why: a.why.clone(), open: false }).collect(),
        EntryKind::Approval => e.approval.iter().map(|a| Ev::Approval { ok: a.ok, text: a.text.clone(), note: a.note.clone(), asked: String::new(), open: false }).collect(),
        EntryKind::Scheduled => e.scheduled.iter().map(|s| Ev::Scheduled { head: s.head.clone(), words: s.words.clone(), open: false }).collect(),
        // its typed parts (architect m_15013), faded by its closing word
        EntryKind::Card => e.card.iter().map(card).collect(),
        // P4a: no event yet (the parity law names them)
        EntryKind::Stopped | EntryKind::Page | EntryKind::Report | EntryKind::Unknown => Vec::new(),
    }
}

/// One tool call of a `tools` entry as the TUI's tool row (G3: its name,
/// args and intent, as the row's annotations give them): its call id
/// (ToolItem.id; an older hub's 0: its place), its duration (none: a
/// replay, `0.0s` as the line path drew it), its result (the exit code and
/// output line under a failed bash) and the gate holding it (`? waiting
/// for you`), since their times.
fn tool(i: usize, it: &pthread::ToolItem) -> Ev {
    let state = match it.state {
        pthread::ToolState::Run | pthread::ToolState::Unknown => ToolState::Run,
        pthread::ToolState::Ok => ToolState::Ok,
        pthread::ToolState::Err => ToolState::Fail,
    };
    let id = u32::try_from(it.id).ok().filter(|&n| n > 0).unwrap_or_else(|| u32::try_from(i + 1).unwrap_or(u32::MAX));
    let mut t = ToolData::bare(id, state.clone());
    t.name = Some(it.name.clone()).filter(|n| !n.is_empty());
    t.args = Some(it.args.clone()).filter(|a| !a.is_empty());
    t.intent = it.intent.clone();
    t.code = it.code.clone();
    t.result = it.out.clone().map(|o| (it.state != pthread::ToolState::Err, crate::sanitize::clean(&o, crate::sanitize::TAB_OUTPUT).into_owned()));
    match state {
        // a running call's clock and its gate's run from their lines' times
        ToolState::Run => {
            t.started = since(it.at_ms);
            t.gate = it.gate.and_then(|g| match g.wait {
                pthread::GateWait::Check => Some((Gate::Check, since(g.at_ms))),
                pthread::GateWait::Card => Some((Gate::Card, since(g.at_ms))),
                pthread::GateWait::Unknown => None,
            });
        }
        _ => {
            let took = std::time::Duration::from_millis(it.ms.unwrap_or(0));
            (t.elapsed, t.took) = (Some(fmt_duration(took)), Some(took));
        }
    }
    Ev::Tool(t)
}

/// The instant of `at_ms` on the hub's clock (now when unknown).
fn since(at_ms: u64) -> std::time::Instant {
    let now = std::time::Instant::now();
    let ago = crate::when::now_ms().saturating_sub(at_ms);
    match at_ms {
        0 => now,
        _ => now.checked_sub(std::time::Duration::from_millis(ago)).unwrap_or(now),
    }
}

/// A card entry's feed event: its kind and asker as the hub's row says
/// them (an older hub's entry has neither: a question).
fn card(c: &pthread::EntryCard) -> Ev {
    let card = CardParts {
        id: Some(c.id),
        kind: c.kind.clone().unwrap_or_else(|| "question".into()),
        agent: c.agent.clone().unwrap_or_default(),
        question: c.question.clone(),
        options: c.options.iter().map(|o| o.label.clone()).collect(),
    };
    // faded with the hub's closing word, as the line path's card-closed
    Ev::Card { card, closed: c.closed.clone().unwrap_or_default() }
}

#[cfg(test)]
#[path = "entry_ev_tests.rs"]
mod tests;
