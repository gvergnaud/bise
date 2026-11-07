//! sb-core's answers and effects, typed (issue 25, chunk R): an answer
//! line is read as a `bise_proto::core::link::Answer` ([`answer`], its
//! [`view_of`], [`skipped_of`]); a step's `fx` items come as
//! `bise_proto::core::Fx` through serde ([`read`] logs and skips the ones
//! this hub can't run), and
//! [`plain`] turns the ones that need nothing of the hub's state into the
//! hub's [`Effect`] at once; the others (the journal, a reply, a delivery,
//! the client's effects, ...) go back to `Hub::effect` (core_link.rs).
//! [`delivery`] writes the text a delivery puts in the agent's context.
//! Pure: no I/O, no hub.

use super::{feed, Effect};
use crate::place_job::Job;
use bise_proto::core::link::{ASkipped, Answer, Skipped};
use bise_proto::core::Feed;
use bise_proto::core::Fx as CoreFx;
use serde::Deserialize;
use serde_json::Value;

/// One effect of sb-core, read.
#[derive(Debug, PartialEq)]
pub(super) enum Plain {
    /// the hub's effect, nothing else to do
    Effect(Effect),
    /// nothing to do (`rt`: the runtime state comes with the view)
    Nothing,
    /// an effect on the hub's state: `Hub::effect` runs it
    Hub(CoreFx),
    /// not an effect this hub runs: skipped, with the log line that says
    /// why (a newer sb-core, a version switch in flight: never a crash)
    Skip(String),
}

/// sb-core's answer line, read (any object is an answer: `Unknown` at
/// worst, never an error).
pub(super) fn answer(out: &Value) -> Answer {
    Answer::deserialize(out).unwrap_or_else(|_| Answer::Unknown(out.clone()))
}

/// The view an answer carries (a step's, `view_all`'s).
pub(super) fn view_of(a: &Answer) -> Option<&Value> {
    match a {
        Answer::Step(s) => Some(&s.view),
        Answer::View(v) => Some(&v.view),
        _ => None,
    }
}

/// The indexes of a `replay_many` batch sb-core did not apply.
pub(super) fn skipped_of(a: &Answer) -> Vec<u64> {
    match a {
        Answer::Skipped(ASkipped { skipped: Skipped::At(ix) }) => ix.clone(),
        _ => Vec::new(),
    }
}

/// One item of a step's `fx` list (its `Unknown`: a tag this hub doesn't
/// know, or a known tag whose fields don't decode, or no tag).
pub(super) fn read(f: CoreFx) -> Plain {
    match f {
        CoreFx::Unknown { tag, raw } if CoreFx::TAGS.contains(&tag.as_str()) => {
            Plain::Skip(format!("sb-core: bad effect {} (its fields don't decode, skipped): {}", tag, raw))
        }
        CoreFx::Unknown { tag, raw } => Plain::Skip(format!("sb-core: unknown effect {} (skipped): {}", tag, raw)),
        fx => plain(fx),
    }
}

/// The hub's effect of `f` when it needs nothing of the hub's state.
pub(super) fn plain(f: CoreFx) -> Plain {
    let e = match f {
        CoreFx::Rt(_) => return Plain::Nothing,
        CoreFx::Spawn(s) => Effect::Spawn { agent: s.agent, resume: s.resume, crash_note: s.crash_note, wait_load: s.wait_load },
        CoreFx::Kill(k) => Effect::Kill { agent: k.agent },
        CoreFx::WorktreeCreate(w) => Effect::Place(Job::Create { name: w.name, with_changes: w.with_changes }),
        CoreFx::WorktreeDrop(w) => Effect::Place(Job::Drop { name: w.name, ws: w.ws, dirty: w.dirty, unpushed: w.unpushed }),
        CoreFx::Say(s) => Effect::Say { agent: s.agent, text: s.text },
        CoreFx::Passthrough(p) => Effect::Passthrough { agent: p.agent, line: p.line },
        // a record of an agent's feed, written `sb <kind> :: <json>` (a
        // kind this hub doesn't know too: the reader shows its text)
        CoreFx::Feed(f) => match Feed::decode(f.rec.clone()) {
            Ok(rec) => feed(&f.agent, rec),
            Err(e) => return Plain::Skip(format!("sb-core: bad feed record ({}, skipped): {}", e, f.rec)),
        },
        CoreFx::XDeliver(x) => Effect::XDeliver { xid: x.xid, project: x.project, kind: x.kind, text: x.text, context: x.context, parts: x.parts },
        CoreFx::XReply(x) => Effect::XReply { hub: x.hub, xid: x.xid, text: x.text },
        CoreFx::Route(r) => Effect::Route { rid: r.rid, to: r.to, name: r.name, text: r.text, due_ms: r.due_ms, why: r.why },
        CoreFx::RouteAsked(r) => Effect::RouteAsked { routed: r.routed },
        CoreFx::Spoke(r) => Effect::Spoke { token: r.token, ok: r.ok, why: r.why },
        CoreFx::RouteDone(r) => Effect::RouteDone { rid: r.rid, state: r.state, to: r.to, xid: r.xid, msg: r.msg },
        CoreFx::JobEnd(j) => Effect::JobEnd { agent: j.agent, state: j.state, label: j.label, summary: j.summary, key: j.key },
        CoreFx::FollowedEnd(j) => Effect::FollowedEnd {
            project: j.project,
            agent: j.agent,
            key: j.key,
            state: j.state,
            label: j.label,
            summary: j.summary,
        },
        // a gate card's verdict (approvals)
        CoreFx::Confirm(c) => Effect::Confirm {
            card: c.card.unwrap_or(0),
            agent: c.agent.unwrap_or_default(),
            text: c.text,
        },
        CoreFx::ViewCard(c) => Effect::ViewCard { agent: c.agent, call: c.call, card: c.card },
        CoreFx::ViewContext(c) => Effect::ViewContext { agent: c.agent, call: c.call, kept: c.kept, why: c.why },
        other => return Plain::Hub(other),
    };
    Plain::Effect(e)
}

/// The text a delivery puts in the agent's context: main's `notes`, the
/// task `status` block (None or "": none), then each message as the
/// recipient reads it.
pub(super) fn delivery(notes: &[String], status: Option<String>, msgs: Vec<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !notes.is_empty() {
        let mut t = String::from("<bise_notes>\n");
        for n in notes {
            t.push_str(&format!("- {}\n", n));
        }
        t.push_str("</bise_notes>");
        parts.push(t);
    }
    parts.extend(status.filter(|s| !s.is_empty()));
    parts.extend(msgs);
    parts.join("\n\n")
}

#[cfg(test)]
#[path = "core_fx_tests.rs"]
mod tests;
