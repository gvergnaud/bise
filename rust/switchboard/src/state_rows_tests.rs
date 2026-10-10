//! client-protocol step 4, P4c-4a's law: the typed rows the terminal
//! reads instead of the older `state` event (hub/agents' `places`,
//! hub/cards' `cards` + `others`, hub/scheduled's `items` + `ended`) carry
//! every key of the older rows with the same value, on a real hub's
//! snapshot (a worktree agent with a late PR in review, his question
//! card, bise's drop card, a live timer and a stopped one). Absent, null,
//! "", false and [] are the same (nothing to say); a renamed key is named
//! in the law; a key no reader takes is named as left out.

use super::*;
use crate::forge::poll::{Local, Report};
use crate::place::{Checks, PrSnapshot, PrState, Review};

/// absent, null, "", false and [] are the same: nothing to say
fn some(v: Option<&Value>) -> Option<Value> {
    v.filter(|v| !(v.is_null() || *v == "" || *v == false || v.as_array().is_some_and(Vec::is_empty))).cloned()
}

fn same(older: &Value, typed: &Value, k: &str, tk: &str, what: &str) {
    assert_eq!(some(older.get(k)), some(typed.get(tk)), "{what}: {k}");
}

fn a_real_hub() -> (T, u64) {
    let mut t = T::new();
    // a worktree agent whose PR waits for a review, checks red
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "dark".into(),
            brief: Brief { objective: "objective of dark".into(), ..Brief::default() },
            worktree: true,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
        },
    );
    assert!(fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "dark")), "{fx:?}");
    t.go(Input::ReplReady { agent: "dark".into() });
    let pr = PrSnapshot {
        number: 412,
        url: "https://github.com/o/r/pull/412".into(),
        branch: "sb/dark".into(),
        head_oid: "tip1".into(),
        state: PrState::Open,
        review: Review::Pending,
        checks: Checks::Fail { failing: vec!["ci/test".into()] },
        updated_at: "t".into(),
        facts: Default::default(),
    };
    let local = vec![Local { place: "wt:dark".into(), branch: "sb/dark".into(), tip: Some("tip1".into()), commits: Some(2), dirty: None }];
    t.go(Input::Prs(Report { at_ms: 1_000, prs: Some(Ok(vec![pr])), local, activity: Vec::new() }));
    // his question (with options), bise's drop card (a worktree with work)
    t.req(MAIN, AgentReq::Card { text: "keep the banner?\n1. yes\n2. no".into(), for_msg: None });
    t.user(MAIN, "/new -w fix: x");
    t.go(Input::ReplReady { agent: "fix".into() });
    t.go(Input::ReplIdle { agent: "fix".into(), leftover: false });
    t.env.loss = Loss { dirty: 1, unpushed: 0 };
    t.req(MAIN, AgentReq::Drop { agent: "fix".into() });
    // a live timer and a stopped one
    let add = |text: &str| {
        AgentReq::Every(EveryReq::Add { to: String::new(), text: text.into(), sched: crate::every::Sched::Every(600_000), until_ms: None, times: Some(6), page: None, name: None })
    };
    t.req(MAIN, add("check HN"));
    t.req(MAIN, add("check reddit"));
    t.req(MAIN, AgentReq::Every(EveryReq::Stop(2)));
    // the forge's answer is late by now
    (t, 1_000 + PR_STALE_MS + 5_000)
}

#[test]
fn the_typed_state_rows_carry_everything_the_older_state_did() {
    let (t, now) = a_real_hub();
    let snap = t.hub.snapshot(now);

    // places: each key, its PR's too (review's `pending` is `none` +
    // in_review, `changes_requested` is `changes`; checks' state + failing)
    let places = crate::proto_view::places(&snap);
    let older = snap["places"].as_array().unwrap();
    assert_eq!(places.len(), older.len());
    assert!(places.iter().any(|p| p.pr.as_ref().is_some_and(|pr| pr.in_review && pr.stale_ms.is_some() && pr.failing == ["ci/test"])), "{places:?}");
    for (o, p) in older.iter().zip(&places) {
        let ty = serde_json::to_value(p).unwrap();
        for k in o.as_object().unwrap().keys().filter(|k| *k != "pr") {
            same(o, &ty, k, k, "place");
        }
        let (Some(op), Some(tp)) = (o.get("pr").filter(|x| x.is_object()), ty.get("pr")) else {
            assert_eq!(some(o.get("pr")), some(ty.get("pr")), "place pr");
            continue;
        };
        for k in op.as_object().unwrap().keys() {
            match k.as_str() {
                "review" => {
                    let typed = match (tp["review"].as_str().unwrap(), tp.get("in_review") == Some(&json!(true))) {
                        ("none", true) => "pending",
                        ("changes", _) => "changes_requested",
                        (w, _) => w,
                    };
                    assert_eq!(op["review"], typed, "place pr review");
                }
                "checks" => {
                    assert_eq!(op["checks"]["state"], tp["checks"], "place pr checks");
                    same(&op["checks"], tp, "failing", "failing", "place pr checks");
                }
                k => same(op, tp, k, k, "place pr"),
            }
        }
    }

    // cards: his then the others, every open card once, each list in the
    // snapshot's order; each key (`age_ms` is `since_ms` back from now)
    let (his, others) = crate::proto_view::cards(&snap, "p", now, crate::model::user_kind);
    let older = snap["cards"].as_array().unwrap();
    let at = |id: u64| older.iter().position(|c| c["id"] == id).expect("a card of the snapshot");
    for list in [&his, &others] {
        let pos: Vec<usize> = list.iter().map(|c| at(c.id)).collect();
        assert!(pos.windows(2).all(|w| w[0] < w[1]), "the snapshot's order: {pos:?}");
    }
    assert_eq!(his.len() + others.len(), older.len(), "every open card once");
    assert!(his.iter().any(|c| c.kind == "question" && c.options.len() == 2) && others.iter().any(|c| c.kind == "drop"), "{his:?} {others:?}");
    for c in his.iter().chain(&others) {
        let (o, ty) = (&older[at(c.id)], serde_json::to_value(c).unwrap());
        for k in o.as_object().unwrap().keys() {
            match k.as_str() {
                "age_ms" => assert_eq!(now - c.since_ms, o["age_ms"].as_u64().unwrap(), "card age_ms"),
                k => same(o, &ty, k, k, "card"),
            }
        }
    }

    // timers: the live ones then the ended ones of the week, as the
    // daemon's state gives them (`Timers::state`); `text` is `words`,
    // `fired` is `done`, a 0 time is none, `stopped_by` typed;
    // `every_ms` (the label says it) and `why` (end + stopped_by say it)
    // are left out
    let timers = t.hub.timers();
    let older = timers.state(now);
    let typed: Vec<_> = crate::proto_view::scheduled(timers).into_iter().chain(crate::proto_view::scheduled_ended(timers, now)).collect();
    assert_eq!(typed.len(), older.len());
    assert!(typed.iter().any(|x| x.ended_ms.is_some() && x.stopped_by.is_some()), "a stopped one: {typed:?}");
    for (o, x) in older.iter().zip(&typed) {
        let ty = serde_json::to_value(x).unwrap();
        for k in o.as_object().unwrap().keys() {
            match k.as_str() {
                "every_ms" | "daily_min" | "why" => {}
                "text" => same(o, &ty, k, "words", "timer"),
                "fired" => same(o, &ty, k, "done", "timer"),
                "next_ms" | "last_ms" => assert_eq!(o[k].as_u64().filter(|m| *m > 0), ty.get(k).and_then(Value::as_u64), "timer {k}"),
                "stopped_by" => {
                    let typed = match &x.stopped_by {
                        Some(bise_proto::rows::WaitingOn::You) => "user".to_string(),
                        Some(bise_proto::rows::WaitingOn::Agent { name }) => name.clone(),
                        _ => String::new(),
                    };
                    assert_eq!(o[k], typed, "timer stopped_by");
                }
                k => same(o, &ty, k, k, "timer"),
            }
        }
    }
}
