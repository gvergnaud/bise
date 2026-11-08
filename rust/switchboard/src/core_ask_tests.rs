//! ask-wait: a reply still pending after an `sb ask` / `sb wait` hold is
//! a pause (sb-core's `reply` watch, bend/hub/wakes.bend make_reply).
//! Out of core_tests.rs (its line limit).

use super::super::*;

/// ask-wait (laws ask_*): a hold that ends with no reply leaves a reply
/// watch, a pause counted from the ask; the reply ends it with no wake
/// line, a second hold sets no second one, done ends it, and the asked
/// agent's drop tells the asker once at the next tick.
#[test]
fn a_reply_still_pending_after_the_hold_is_a_pause() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let watching = |t: &mut T, n: &str| serde_json::to_value(&t.hub.snapshot(t.env.now).agent(n).watching).unwrap();
    let asked_at = t.env.now;
    let ask = |t: &mut T, text: &str| t.req("a", AgentReq::Ask { to: "b".into(), text: text.into(), timeout_s: 600 }).0;
    let tok = ask(&mut t, "which port?");
    let id = t.hub.st.msgs().values().find(|m| t.hub.st.text(m.id) == "which port?").unwrap().id;
    assert_eq!(watching(&mut t, "a"), json!([]), "held: no watch yet");
    t.env.now += 26_000;
    let r = reply(&t.go(Input::Tick), tok).expect("the hold ends at 25 s");
    assert_eq!(r["error"], "timeout");
    let w = watching(&mut t, "a");
    assert_eq!(w.as_array().unwrap().len(), 1, "{w}");
    assert_eq!((w[0]["kind"].as_str(), w[0]["what"].as_str(), w[0]["resumes"].as_bool()), (Some("reply"), Some("b"), Some(true)), "{w}");
    assert_eq!(w[0]["since_ms"].as_u64(), Some(asked_at), "counted from the ask: {w}");
    // a second hold on the same message: still one watch
    let (tok2, _) = t.req("a", AgentReq::Wait { msg: id, timeout_s: 600 });
    t.env.now += 26_000;
    assert!(reply(&t.go(Input::Tick), tok2).is_some());
    assert_eq!(watching(&mut t, "a").as_array().unwrap().len(), 1);
    // its reply ends it in the same step: no wake line, no bise message
    let (_, fx) = t.req("b", AgentReq::Send { to: "a".into(), text: "8080".into(), expect_reply: false, reply_to: Some(id), queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None });
    assert!(fx.iter().any(|e| matches!(e, Effect::Journal(j) if j["type"] == "wake_end" && j["why"] == "answered")), "{fx:?}");
    assert!(lines_of(&fx, "a").iter().all(|l| !l.starts_with("sb wake :: ")), "{fx:?}");
    assert!(!t.journal.borrow().iter().any(|j| j["type"] == "message_sent" && j["msg"]["from"] == "switchboard" && j["msg"]["to"] == "a"), "no bise message");
    assert_eq!(watching(&mut t, "a"), json!([]));
    // done: it moved on, the watch ends quietly
    ask(&mut t, "and the host?");
    t.env.now += 26_000;
    t.go(Input::Tick);
    assert_eq!(watching(&mut t, "a").as_array().unwrap().len(), 1);
    let (_, fx) = t.req("a", AgentReq::Status { status: Declared::Done, note: String::new() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Journal(j) if j["type"] == "wake_end" && j["why"] == "moved on")), "{fx:?}");
    assert_eq!(watching(&mut t, "a"), json!([]));
    // b's turn ends with no written reply: its auto reply to each ask
    // wakes a, and ends the watch of each (a reply all the same)
    t.go(Input::ReplIdle { agent: "b".into(), leftover: false });
    // the asked agent dropped: the next tick ends it and tells the asker once
    t.req("a", AgentReq::Status { status: Declared::Working, note: String::new() });
    ask(&mut t, "still there?");
    t.env.now += 26_000;
    t.go(Input::Tick);
    assert_eq!(watching(&mut t, "a").as_array().unwrap().len(), 1);
    // b is mid-turn on the ask: the drop asks him, he says yes
    t.req(MAIN, AgentReq::Drop { agent: "b".into() });
    let card = *t.hub.st.cards.keys().next().unwrap();
    let fx = t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card, text: "yes".into(), parts: String::new() } });
    assert_eq!(t.hub.st.agents["b"].lifecycle, Lifecycle::Archived, "{fx:?}");
    assert_eq!(watching(&mut t, "a").as_array().unwrap().len(), 1, "told at the next tick");
    t.go(Input::Tick);
    assert_eq!(watching(&mut t, "a"), json!([]));
    let told = t.journal.borrow().iter().filter(|j| j["type"] == "message_sent" && j["msg"]["from"] == "switchboard" && j["msg"]["to"] == "a" && j["msg"]["text"].as_str().unwrap_or("").contains("was dropped")).count();
    assert_eq!(told, 1, "{:?}", t.journal.borrow().iter().rev().take(8).collect::<Vec<_>>());
    t.go(Input::Tick);
    let told2 = t.journal.borrow().iter().filter(|j| j["type"] == "message_sent" && j["msg"]["text"].as_str().unwrap_or("").contains("was dropped")).count();
    assert_eq!(told2, 1, "once");
    // a replay reads the ended watches back, with no effect
    let mut h = Hub::new("/w");
    let events = t.journal.borrow().clone();
    assert!(h.replay(&events).is_empty());
    assert!(h.wakes().live.is_empty() && h.wakes().ended.len() == 3, "{:?}", h.wakes());
}
