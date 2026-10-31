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
mod tests {
    use super::*;
    use serde_json::json;

    /// One effect as a step's answer carries it.
    fn of(v: &Value) -> CoreFx {
        match answer(&json!({"fx": [v], "dirty": false, "view": {}})) {
            Answer::Step(mut s) => s.fx.remove(0),
            a => panic!("not a step: {:?}", a),
        }
    }

    fn rd(v: &Value) -> Plain {
        read(of(v))
    }

    fn eff(v: Value) -> Effect {
        match rd(&v) {
            Plain::Effect(e) => e,
            p => panic!("not a plain effect: {:?}", p),
        }
    }

    #[test]
    fn a_feed_record_is_its_line_and_a_bad_one_is_skipped() {
        let sent = eff(json!({"fx": "feed", "agent": "a", "rec": {"sb": "sent", "to": "b", "id": 3, "ask": false, "body": "x : y"}}));
        assert_eq!(sent, Effect::Line { agent: "a".into(), line: r#"sb sent :: {"ask":false,"body":"x : y","id":3,"to":"b"}"#.into() });
        // a newer sb-core's kind: written as it came, the reader shows its text
        let new = eff(json!({"fx": "feed", "agent": "a", "rec": {"sb": "dance", "text": "hi"}}));
        assert_eq!(new, Effect::Line { agent: "a".into(), line: r#"sb dance :: {"text":"hi"}"#.into() });
        // a known kind whose fields don't fit: skipped and logged, never a guess
        assert!(matches!(rd(&json!({"fx": "feed", "agent": "a", "rec": {"sb": "card", "id": "x"}})), Plain::Skip(_)));
        assert!(matches!(rd(&json!({"fx": "line", "agent": "a", "kind": "you", "text": "go"})), Plain::Skip(_)), "the old line effect is gone");
    }

    #[test]
    fn plain_effects_keep_every_field() {
        assert_eq!(
            eff(json!({"fx": "spawn", "agent": "a", "resume": true, "crash_note": "boom"})),
            Effect::Spawn { agent: "a".into(), resume: true, crash_note: Some("boom".into()), wait_load: false }
        );
        assert_eq!(
            eff(json!({"fx": "spawn", "agent": "w", "resume": false, "crash_note": null, "wait_load": true})),
            Effect::Spawn { agent: "w".into(), resume: false, crash_note: None, wait_load: true }
        );
        assert_eq!(
            eff(json!({"fx": "route_done", "rid": 1, "state": "cancelled", "to": "shop-1", "xid": null})),
            Effect::RouteDone { rid: 1, state: "cancelled".into(), to: "shop-1".into(), xid: None, msg: None }
        );
        assert_eq!(
            eff(json!({"fx": "xdeliver", "xid": 1, "project": "p", "kind": "ask", "msg": 0, "text": "t", "context": ""})),
            Effect::XDeliver { xid: 1, project: "p".into(), kind: "ask".into(), text: "t".into(), context: "".into(), parts: "".into() }
        );
        assert_eq!(
            eff(json!({"fx": "job_end", "agent": "perf", "state": "done", "label": "l", "summary": "s", "key": 7, "hub": "h"})),
            Effect::JobEnd { agent: "perf".into(), state: "done".into(), label: "l".into(), summary: "s".into(), key: 7 }
        );
    }

    #[test]
    fn a_confirm_is_the_gates_verdict() {
        assert_eq!(
            eff(json!({"fx": "confirm", "card": 3, "agent": "t8", "text": "2"})),
            Effect::Confirm { card: 3, agent: "t8".into(), text: "2".into() }
        );
    }

    #[test]
    fn hub_effects_go_back_rt_is_nothing() {
        for v in [
            json!({"fx": "journal", "ev": {"type": "act_done", "id": 1}}),
            json!({"fx": "reply", "token": 1, "body": {"ok": true}}),
            json!({"fx": "deliver", "mode": "say", "agent": "w", "notes": [], "status": false, "msgs": [{"id": 2, "rel": "hub"}]}),
            json!({"fx": "interrupt", "agent": "t7"}),
            json!({"fx": "notice", "text": "x"}),
            json!({"fx": "focus_main", "name": "g"}),
            json!({"fx": "renamed", "old": "a", "new": "b"}),
        ] {
            assert!(matches!(rd(&v), Plain::Hub(_)), "{v}");
        }
        assert_eq!(rd(&json!({"fx": "rt", "agent": "w", "run": "idle", "waiting": false, "turn_ms": null, "turns": 0})), Plain::Nothing);
    }

    /// Every effect sb-core was seen to send (chunk 0's capture) is run:
    /// none is skipped (a known tag whose fields don't decode would be).
    #[test]
    fn every_captured_effect_is_read() {
        let fixtures = include_str!("../../proto/fixtures/core_fx.jsonl");
        let mut n = 0;
        for l in fixtures.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(l).unwrap();
            assert!(!matches!(rd(&v), Plain::Skip(_)), "skipped: {l}");
            n += 1;
        }
        assert!(n >= CoreFx::TAGS.len(), "{n} lines");
    }

    #[test]
    fn an_unknown_or_bad_effect_is_skipped_never_a_crash() {
        let unknown = json!({"fx": "teleport", "agent": "a"});
        assert_eq!(rd(&unknown), Plain::Skip(format!("sb-core: unknown effect teleport (skipped): {}", unknown)));
        assert!(matches!(rd(&json!({"agent": "a"})), Plain::Skip(_)));
        let bad = json!({"fx": "kill", "agent": 3});
        assert_eq!(rd(&bad), Plain::Skip(format!("sb-core: bad effect kill (its fields don't decode, skipped): {}", bad)));
    }

    /// Every answer shape sb-core writes (rust/proto's fixture) reads as
    /// what the hub takes from it: a step's or view_all's view, a batch's
    /// skipped indexes.
    #[test]
    fn answers_give_their_view_and_skips() {
        let fixtures = include_str!("../../proto/fixtures/core_answer.jsonl");
        for l in fixtures.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(l).unwrap();
            let a = answer(&v);
            assert!(!matches!(a, Answer::Unknown(_)), "unknown: {l}");
            assert_eq!(view_of(&a), v.get("view").filter(|_| v.get("need").is_none()), "{l}");
        }
        assert_eq!(skipped_of(&answer(&json!({"skipped": [0, 3]}))), vec![0, 3]);
        assert_eq!(skipped_of(&answer(&json!({"skipped": true}))), Vec::<u64>::new());
        assert_eq!(skipped_of(&answer(&json!({}))), Vec::<u64>::new());
        assert!(matches!(answer(&json!({"what": 1})), Answer::Unknown(_)));
    }

    #[test]
    fn a_delivery_is_notes_then_status_then_messages() {
        assert_eq!(delivery(&[], None, vec!["m".into()]), "m");
        assert_eq!(delivery(&[], Some(String::new()), vec![]), "");
        assert_eq!(
            delivery(&["n1".into(), "n2".into()], Some("S".into()), vec!["a".into(), "b".into()]),
            "<bise_notes>\n- n1\n- n2\n</bise_notes>\n\nS\n\na\n\nb"
        );
    }
}
