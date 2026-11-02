//! les bims: `Hub.doing` is the one owner of an agent's `doing`, and its
//! agents row says exactly what the map says while its turn runs
//! (architect m_27675). Out of core_tests.rs (its line limit).

use super::super::*;
use bise_proto::rows::{Doing, DoingKind};

fn row_doing(t: &mut T, name: &str) -> Option<Doing> {
    t.hub.snapshot(t.env.now).agent(name).doing.clone()
}

fn line(t: &mut T, agent: &str, l: &str) -> Vec<Effect> {
    t.go(Input::ReplLine { agent: agent.into(), line: l.into() })
}

#[test]
fn the_row_says_what_the_map_says() {
    let mut t = T::new();
    t.spawn_task("a");
    let steps = [
        ("  obs: assistant: looking", Some(DoingKind::Think)),
        ("tool #1 bash : ls", Some(DoingKind::Tool)),
        ("tool_result #1 ok : x", Some(DoingKind::Think)),
        ("tool #2 edit : {\"file_path\":\"a.rs\"}", Some(DoingKind::Edit)),
        ("tool_result #2 ok : done", Some(DoingKind::Think)),
        ("tool #3 bash : cargo test -p x", Some(DoingKind::Build)),
        ("  obs: compaction_started #4 (auto)", Some(DoingKind::Compact)),
        ("  obs: compaction_done: ## Who I am", Some(DoingKind::Think)),
    ];
    for (l, want) in steps {
        t.env.now += 10;
        line(&mut t, "a", l);
        let map = t.hub.doing.get("a").cloned();
        assert_eq!(map.as_ref().map(|d| d.kind), want, "{l}");
        assert_eq!(row_doing(&mut t, "a"), map, "the row is the map's after {l}");
    }
    // the turn ends: neither says anything any more
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    assert_eq!(t.hub.doing.get("a"), None);
    assert_eq!(row_doing(&mut t, "a"), None);
}

/// The mirror is rebuilt from sb-core's view at every step: the map
/// keeps the fact across, the row never loses it mid-turn.
#[test]
fn a_view_rebuild_keeps_it() {
    let mut t = T::new();
    t.spawn_task("a");
    line(&mut t, "a", "tool #1 bash : sleep 9");
    let before = row_doing(&mut t, "a");
    assert_eq!(before.as_ref().map(|d| d.kind), Some(DoingKind::Tool));
    // a step that goes through sb-core (a message to it) rebuilds the view
    t.req(MAIN, AgentReq::Send { to: "a".into(), text: "hi".into(), expect_reply: false, reply_to: None, queued: true, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None });
    assert_eq!(row_doing(&mut t, "a"), before);
    assert_eq!(t.hub.doing.get("a").cloned(), before);
}

/// sb-core's journal effects are activity events, in the same step.
#[test]
fn a_message_and_a_report_are_activity() {
    let mut t = T::new();
    t.spawn_task("a");
    let (_, fx) = t.req(MAIN, AgentReq::Send { to: "a".into(), text: "secret body".into(), expect_reply: true, reply_to: None, queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None });
    let acts: Vec<_> = fx.iter().filter_map(|e| if let Effect::Activity(a) = e { Some(a.clone()) } else { None }).collect();
    assert_eq!(acts.len(), 1, "{fx:?}");
    assert!(matches!(&acts[0], bise_proto::activity::ActivityEv::Message { expects_reply: true, .. }), "{acts:?}");
    assert!(!serde_json::to_string(&acts[0]).unwrap().contains("secret"));
}
