//! A step's answer, applied (core_link.rs's apply_step): its effects run
//! whatever its view, and his message's id (A1) comes back typed; his card
//! answer's route lines (inbox picks); a watch's pause or alarm on the
//! agents row and his stop of it. Out of core_tests.rs (its line limit).

use super::*;

/// hot-view: a step whose view doesn't decode (a bug of sb-core's
/// encoder) still applies its effects, the journal's included; the
/// mirror stays as the last good view left it.
#[test]
fn a_step_whose_view_does_not_decode_still_applies_its_effects() {
    use bise_proto::core::{fx as cfx, link::AStep, Fx as CoreFx};
    let mut t = T::new();
    let (agents, order) = (t.hub.st.agents.clone(), t.hub.st.order.clone());
    let s = AStep {
        fx: vec![
            CoreFx::Journal(cfx::Journal { ev: json!({"type": "main_note", "text": "fake", "at_ms": 1}) }),
            CoreFx::Kill(cfx::Kill { agent: "fake".into() }),
        ],
        dirty: true,
        view: json!({"agents": 3, "order": "not a list"}),
        ..Default::default()
    };
    let mut fx = Vec::new();
    t.hub.apply_step(&mut fx, &mut t.env, None, 0, s);
    assert!(fx.contains(&Effect::Kill { agent: "fake".into() }), "{:?}", fx);
    assert!(fx.iter().any(|e| matches!(e, Effect::Journal(ev) if ev["text"] == "fake")), "{:?}", fx);
    assert_eq!((&t.hub.st.agents, &t.hub.st.order), (&agents, &order), "the mirror kept");
}

/// A1: his input's step answers the id sb-core gave his message (typed,
/// AStep.msg), the id his you-id line carries when it is delivered; a
/// queued one too, at arrival. A tick makes none.
#[test]
fn his_input_answers_the_id_of_his_message() {
    let mut t = T::new();
    t.spawn_task("a");
    let you_id = |fx: &[Effect], agent: &str| recs(fx, agent).iter().find(|r| r["sb"] == "you-id").and_then(|r| r["id"].as_u64());
    t.hub.said = None;
    let fx = t.user("main", "first words");
    let said = t.hub.said.take().expect("his input answers his message's id");
    assert_eq!(you_id(&fx, "main"), Some(said), "the answered id is the you-id line's: {fx:?}");
    // `a` is busy: queued, its id at arrival, its you-id line at the turn's end
    let fx = t.user_queued("a", "queued words");
    let q = t.hub.said.take().expect("a queued input has its id at arrival");
    assert!(q > said && you_id(&fx, "a").is_none(), "a fresh id, no line yet: {q} after {said}");
    let fx = t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    assert_eq!(you_id(&fx, "a"), Some(q), "its line carries the id answered at arrival: {fx:?}");
    assert_eq!(t.hub.said, None, "a turn's end makes no message of his");
    t.go(Input::Tick);
    assert_eq!(t.hub.said, None, "a tick makes none");
}

/// Inbox picks (architect m_22243): his answer '2' to docs' card is said
/// as '2' and picked as that option's words, with the message it became,
/// in main's feed and in docs' own (both feeds, one line each); words
/// get no pick; main's own card: one line, in main's feed alone.
#[test]
fn an_answer_by_number_names_its_option_in_both_feeds() {
    let mut t = T::new();
    t.spawn_task("docs");
    let (_, _) = t.req(
        "docs",
        AgentReq::Send { to: MAIN.into(), text: "where?".into(), expect_reply: true, reply_to: None, queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None },
    );
    let id = t.hub.st.msgs().values().find(|m| m.text == "where?").unwrap().id;
    t.req(MAIN, AgentReq::Card { text: "where does the balance show?\n1. at every step\n2. only at checkout".into(), for_msg: Some(id), page: None, artifact: None, open: Vec::new() });
    t.turn(MAIN, "asked");
    let card = *t.hub.st.cards.keys().next().unwrap();
    t.hub.answered = None;
    let fx = t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card, text: "2".into(), parts: String::new() } });
    // (3): the step's answer is the route line's, the card's question with it
    let a = t.hub.answered.take().expect("the step answers his answer");
    assert_eq!((a.agent.as_str(), a.said.as_str(), a.pick.as_deref()), ("docs", "2", Some("only at checkout")));
    assert!(a.question.starts_with("where does the balance show?") && a.notice == "you answered docs: only at checkout", "{a:?}");
    let answer = t.hub.st.msgs().values().find(|m| m.text == "2").expect("the asker's message, as he said it").id;
    for feed in [MAIN, "docs"] {
        let rs: Vec<Value> = recs(&fx, feed).into_iter().filter(|r| r["sb"] == "route").collect();
        assert_eq!(rs, vec![json!({"sb": "route", "who": "docs", "card": card, "said": "2", "msg": answer, "pick_n": 2, "pick": "only at checkout"})], "{feed}: {fx:?}");
    }
    // words: no pick
    t.req(MAIN, AgentReq::Card { text: "and then?\n1. ship\n2. wait".into(), for_msg: Some(id), page: None, artifact: None, open: Vec::new() });
    let card2 = *t.hub.st.cards.keys().next().unwrap();
    let fx = t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card: card2, text: "ship it today".into(), parts: String::new() } });
    let r = recs(&fx, MAIN).into_iter().find(|r| r["sb"] == "route").expect("the route line");
    assert!(r.get("pick").is_none() && r.get("pick_n").is_none() && r["said"] == "ship it today", "{r}");
    let a = t.hub.answered.take().expect("words answer too");
    assert!(a.pick.is_none() && a.said == "ship it today", "{a:?}");
    // a step that answered no card: none
    t.turn(MAIN, "ok");
    assert_eq!(t.hub.answered, None);
    // main's own card: one line, no message, in main's feed alone
    t.req(MAIN, AgentReq::Card { text: "merge?\n1. merge\n2. not yet".into(), for_msg: None, page: None, artifact: None, open: Vec::new() });
    let card3 = *t.hub.st.cards.keys().next().unwrap();
    let fx = t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card: card3, text: "1".into(), parts: String::new() } });
    let rs: Vec<Value> = recs(&fx, MAIN).into_iter().filter(|r| r["sb"] == "route").collect();
    assert_eq!(rs, vec![json!({"sb": "route", "who": MAIN, "card": card3, "said": "1", "pick_n": 1, "pick": "merge"})], "{fx:?}");
}

/// agent-waiting-states (pick A, architect m_22919): sb-core says whether
/// a watch is a pause or an alarm; a backgrounded command and a pid are
/// pauses, a file an alarm, `--then` / `--alarm` win; the agents row
/// carries it with the watch's end; his stop (the window's wake_stop)
/// ends it with one `stopped` line; a watch never moves the agent's
/// status.
#[test]
fn a_watch_is_a_pause_or_an_alarm_on_the_agents_row() {
    use crate::wake::{Spec, What, MAX_DEFAULT_MS};
    let mut t = T::new();
    t.spawn_task("perf");
    t.go(Input::ReplIdle { agent: "perf".into(), leftover: false });
    let row = |t: &mut T| t.hub.snapshot(t.env.now).agent("perf").clone();
    let spec = |what| Spec { what, tail: None, note: String::new() };
    let file = || spec(What::File { path: "/t/build.rc".into() });
    let add = |t: &mut T, s: Spec, then| t.req("perf", AgentReq::Wake(WakeReq::Add { spec: s.json(), max_ms: Some(MAX_DEFAULT_MS), then })).1;
    let status = row(&mut t).status;
    add(&mut t, spec(What::Pid { pid: 4242, start: String::new() }), None);
    add(&mut t, file(), None);
    add(&mut t, file(), Some(true));
    add(&mut t, spec(What::Pid { pid: 4243, start: String::new() }), Some(false));
    let r = row(&mut t);
    let ws = r.watching.clone();
    assert_eq!(ws.iter().map(|w| w.resumes).collect::<Vec<_>>(), [true, false, true, false], "{r:?}");
    assert!(ws.iter().all(|w| w.until_ms == Some(w.since_ms + MAX_DEFAULT_MS)), "{r:?}");
    assert_eq!(r.status, status, "a watch never moves the agent's status");
    // its journal line keeps it: a replay reads the same
    let mut h = Hub::new("/w");
    assert!(h.replay(&t.journal.borrow().clone()).is_empty());
    assert_eq!(h.wakes().live.values().map(|w| w.resumes).collect::<Vec<_>>(), [true, false, true, false]);
    // a line from before the field: its kind's default
    let old = json!({"type": "wake_set", "watch": {"id": 1, "agent": "perf", "spec": {"kind": "bg", "slot": "/b/3", "cmd": "x"}, "max_text": "", "set_at": 1, "max_at": 2, "quiet": true}});
    let mut h = Hub::new("/w");
    let mut evs = t.journal.borrow().iter().take_while(|j| j["type"] != "wake_set").cloned().collect::<Vec<_>>();
    evs.push(old);
    h.replay(&evs);
    assert_eq!(h.wakes().live.get(&1).map(|w| w.resumes), Some(true));
    // his stop: one `stopped` line in perf's thread, and perf hears it
    // once from bise
    let fx = t.go(Input::WakeStop { id: ws[1].id });
    let ls: Vec<_> = lines_of(&fx, "perf").into_iter().filter(|l| l.starts_with("sb wake :: ")).collect();
    assert!(ls.len() == 1 && ls[0].contains("\"ev\":\"stopped\""), "{fx:?}");
    let told: Vec<_> = fx.iter().filter_map(|e| match e {
        Effect::Journal(j) if j["type"] == "message_sent" && j["msg"]["to"] == "perf" => j["msg"]["text"].as_str(),
        _ => None,
    }).collect();
    assert_eq!(told, [format!("the user stopped watch #{} (build.rc): it won't wake you", ws[1].id)], "{fx:?}");
    // the agent's own stop tells it nothing
    let (_, fx) = t.req("perf", AgentReq::Wake(WakeReq::Stop { id: Some(ws[2].id), bg: None }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Journal(j) if j["type"] == "message_sent")), "{fx:?}");
    assert_eq!(row(&mut t).watching.len(), 2);
}

/// ask-wait's tests, out of core_tests.rs too.
#[path = "core_ask_tests.rs"]
mod core_ask_tests;

/// withdrawn-answer's tests, out of core_tests.rs too.
#[path = "core_late_tests.rs"]
mod core_late_tests;

/// drop-card's tests, out of core_tests.rs too.
#[path = "core_drop_card_tests.rs"]
mod core_drop_card_tests;

/// name-free's tests, out of core_tests.rs too.
#[path = "core_names_tests.rs"]
mod core_names_tests;

/// send-parts' tests, out of core_tests.rs too.
#[path = "core_attach_tests.rs"]
mod core_attach_tests;

/// msg-forward's tests, out of core_tests.rs too.
#[path = "core_forward_tests.rs"]
mod core_forward_tests;

/// mcp-apps: a view's off-turn card and a view's message.
#[path = "core_mcp_view_tests.rs"]
mod core_mcp_view_tests;

/// issue 36 chunk 3's: photo / restore_photo, only on a fresh hub.
#[path = "core_photo_tests.rs"]
mod core_photo_tests;

/// issue 36's: a long line to sb-core keeps its multi-byte chars.
#[path = "core_utf8_tests.rs"]
mod core_utf8_tests;

/// les bims' tests (doing, activity), out of core_tests.rs too.
#[path = "core_doing_tests.rs"]
mod core_doing_tests;

/// queued_steer's tests, out of core_tests.rs too.
#[path = "core_queue_tests.rs"]
mod core_queue_tests;

/// plan-ok (architect m_26549, m_26571): a page's own card is asked by
/// the first live agent of its chain, else main; it shows in that
/// asker's feed too; a kind that is no page card's opens nothing.
#[test]
fn a_page_card_is_asked_by_the_first_live_agent() {
    let mut t = T::new();
    t.spawn_task("lean-install");
    let card = |t: &mut T, agents: &[&str], kind: &str| {
        let before: Vec<u64> = t.hub.st.open_cards().map(|c| c.id).collect();
        let fx = t.go(Input::PageCard { agents: agents.iter().map(|a| a.to_string()).collect(), kind: kind.into(), text: "lean installs: start the plan lean-install recommends?\n1. start the plan\n2. open the plan".into(), key: "plan:plan".into() });
        let new = t.hub.st.open_cards().find(|c| !before.contains(&c.id)).map(|c| (c.kind.clone(), c.agent.clone()));
        (new, fx)
    };
    let (got, fx) = card(&mut t, &["ghost", "lean-install"], "plan");
    assert_eq!(got, Some(("plan".to_string(), "lean-install".to_string())), "the first live one asks");
    assert!(recs(&fx, "lean-install").iter().any(|r| r["sb"] == "card"), "in its own feed: {fx:?}");
    let (got, _) = card(&mut t, &["ghost"], "plan");
    assert_eq!(got, Some(("plan".to_string(), MAIN.to_string())), "none live: main");
    let (got, _) = card(&mut t, &["lean-install"], "merge");
    assert_eq!(got, None, "no page card's kind");
}

/// hub-tick: a tick on a big history with many live watches (its own
/// file: core.rs and core_tests.rs are at their line limits)
#[path = "core_tick_tests.rs"]
mod tick_tests;
