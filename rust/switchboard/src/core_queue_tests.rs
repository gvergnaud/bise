//! queued_steer: his queued message said now (sb-core's steer_queued).
//! Out of core_tests.rs (its line limit).

use super::super::*;

/// His queued message said now (queued_steer, architect m_25139): steered
/// into the busy agent's turn alone, its id kept, never said again at the
/// turn's end; the others stay queued. A take-back after it, a steer after
/// a take-back and the other refusals are notices that change nothing.
#[test]
fn a_queued_message_steered_now_goes_into_the_turn_once_with_its_id() {
    let notice = |fx: &[Effect]| {
        fx.iter().find_map(|e| match e {
            Effect::ToClient { body, .. } => body["text"].as_str().map(str::to_string),
            _ => None,
        })
    };
    let id_of = |t: &T, text: &str| t.hub.st.msgs().values().find(|m| t.hub.st.text(m.id) == text).unwrap().id;
    let mut t = T::new();
    t.spawn_task("a");
    t.user_queued("a", "steer me");
    t.user_queued("a", "wait for the end");
    let (id, other) = (id_of(&t, "steer me"), id_of(&t, "wait for the end"));
    let fx = t.go(Input::ClientSteerQueued { client: 1, id });
    let s = steer_to(&fx, "a").expect("steered");
    assert!(s.contains("steer me") && !s.contains("wait for the end"), "{}", s);
    assert_eq!(t.hub.st.msg_state()[&id], MsgState::Delivered);
    assert!(matches!(t.hub.st.msg_state()[&other], MsgState::Queued { .. }), "the other waits");
    let rows = crate::board::Waiting::of(&t.hub.st).queued_inputs("a");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [other]);
    // the turn's end says the other one, never the steered one again
    let fx = t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    let s = say_to(&fx, "a").expect("the other at the turn's end");
    assert!(s.contains("wait for the end") && !s.contains("steer me"), "{}", s);
    // steered, then taken back or steered again: refused, unchanged
    let fx = t.go(Input::ClientUnqueue { client: 1, id });
    assert_eq!(notice(&fx).as_deref(), Some(format!("m_{id} already reached @a: it can't be taken back").as_str()));
    let fx = t.go(Input::ClientSteerQueued { client: 1, id });
    assert_eq!(notice(&fx).as_deref(), Some(format!("m_{id} isn't waiting any more").as_str()));
    assert_eq!(t.hub.st.msg_state()[&id], MsgState::Delivered);
    // taken back first: a steer is refused, it stays taken back
    t.go(Input::ReplLine { agent: "a".into(), line: "  obs: turn_started".into() });
    t.user_queued("a", "take me");
    let tid = id_of(&t, "take me");
    t.go(Input::ClientUnqueue { client: 1, id: tid });
    let fx = t.go(Input::ClientSteerQueued { client: 1, id: tid });
    assert!(steer_to(&fx, "a").is_none(), "{:?}", fx);
    assert_eq!(t.hub.st.msg_state()[&tid], MsgState::Rejected { error: "taken_back".into() });
    // unknown, not his
    let fx = t.go(Input::ClientSteerQueued { client: 1, id: 9999 });
    assert_eq!(notice(&fx).as_deref(), Some("no message m_9999 to send now"));
    let (_, _) = send(&mut t, MAIN, "a", "from main", false);
    let mid = id_of(&t, "from main");
    let before = t.hub.st.msg_state().get(&mid).cloned();
    let fx = t.go(Input::ClientSteerQueued { client: 1, id: mid });
    assert_eq!(notice(&fx).as_deref(), Some(format!("m_{mid} isn't one of your messages").as_str()));
    assert_eq!(t.hub.st.msg_state().get(&mid).cloned(), before);
}

