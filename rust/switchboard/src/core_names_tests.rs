//! name-free (architect m_31139): an archived agent's name and an old
//! name no longer hold the name forever. A take (a new task, a rename)
//! frees it in sb-core (bend/hub/take.bend): an archived holder moves to
//! `<name>-2` and keeps its folder, an old name's holder keeps what still
//! says it (fx retarget, cards, timers), and nothing owed passes to the
//! new agent. Out of core_tests.rs (its line limit).

use super::super::*;

fn ren(t: &mut T, a: &str, n: &str) -> Vec<Effect> {
    let (tok, fx) = t.req(MAIN, AgentReq::Rename { agent: a.into(), new_name: n.into() });
    assert_eq!(reply(&fx, tok).map(|v| v["name"].clone()), Some(json!(n)), "{fx:?}");
    fx
}

fn archive(t: &mut T, name: &str) {
    t.go(Input::ReplIdle { agent: name.into(), leftover: false });
    t.user(MAIN, &format!("/archive {name} --force"));
    assert_eq!(t.status(name), Status::Archived);
}

fn kinds(t: &T, from: usize) -> Vec<String> {
    t.journal.borrow()[from..].iter().map(|j| j["type"].as_str().unwrap_or("").to_string()).collect()
}

fn infos(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Line { agent, line } if agent == MAIN => match bise_proto::core::Feed::read_rest(line.strip_prefix("sb ")?)?.1.ok()? {
                bise_proto::core::Feed::Info(i) => Some(i.text),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_task_takes_an_archived_name_and_the_old_one_moves_to_dash_2() {
    let mut t = T::new();
    t.spawn_task("x");
    archive(&mut t, "x");
    let n = t.journal.borrow().len();
    t.spawn_task("x");
    // one step: the archived x renamed first, then the new x created
    assert_eq!(kinds(&t, n).iter().filter(|k| *k == "renamed" || *k == "task_created").cloned().collect::<Vec<_>>(), ["renamed", "task_created"]);
    let old = &t.hub.st.agents["x-2"];
    assert_eq!((old.status(), old.dir.as_str()), (Status::Archived, "x"), "its folder stays");
    assert!(old.aliases.is_empty(), "its old name x went to the new x: {:?}", old.aliases);
    let new = &t.hub.st.agents["x"];
    assert_eq!((new.status(), new.dir.as_str()), (Status::Working, "x.2"), "a folder of its own");
    // restore old x: x-2, the new x untouched
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "x-2".into() });
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
    assert_ne!(t.status("x-2"), Status::Archived);
    assert_eq!(t.hub.st.agents["x"].dir, "x.2");
    // a third x: the next folder, the next name
    archive(&mut t, "x");
    t.spawn_task("x");
    assert_eq!((t.hub.st.agents["x-3"].dir.as_str(), t.hub.st.agents["x"].dir.as_str()), ("x.2", "x.3"));
}

#[test]
fn the_feed_says_where_the_archived_agent_went() {
    let mut t = T::new();
    t.spawn_task("x");
    archive(&mut t, "x");
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "x".into(),
            brief: Brief { objective: "again".into(), ..Brief::default() },
            worktree: false,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
            parts: String::new(),
            attach: Vec::new(),
            fwd: None,
        },
    );
    let lines = infos(&fx);
    assert!(lines.iter().any(|l| l == "@x (archived) is now @x-2: its name went to the new @x"), "{lines:?}");
}

#[test]
fn a_live_agent_still_holds_its_name() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let (tok, fx) = t.req(MAIN, AgentReq::Rename { agent: "a".into(), new_name: "b".into() });
    assert!(err_of(&fx, tok).contains("invalid or taken name"), "{fx:?}");
    // a new task asking for b gets b-2, as before
    t.spawn_task("b-2");
    assert!(t.hub.st.agents.contains_key("b") && t.hub.st.agents.contains_key("b-2"));
}

#[test]
fn a_rename_takes_a_freed_old_name_and_its_holder_keeps_its_things() {
    let mut t = T::new();
    t.spawn_task("marketing");
    ren(&mut t, "marketing", "design");
    assert_eq!(t.hub.st.agents["design"].aliases, ["marketing"]);
    t.spawn_task("launch");
    let fx = ren(&mut t, "launch", "marketing");
    assert!(fx.contains(&Effect::Retarget { from: "marketing".into(), to: "design".into() }), "{fx:?}");
    assert!(t.hub.st.agents["design"].aliases.is_empty());
    assert_eq!(t.hub.st.agents["marketing"].aliases, ["launch"]);
    assert_eq!((t.hub.st.agents["design"].dir.as_str(), t.hub.st.agents["marketing"].dir.as_str()), ("marketing", "launch"));
    // a message to marketing is for the new one now
    let (tok, fx) = t.req(
        "design",
        AgentReq::Send { to: "marketing".into(), text: "hi".into(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None },
    );
    assert!(reply(&fx, tok).is_some(), "{fx:?}");
    assert!(t.hub.st.msgs().values().any(|m| t.hub.st.text(m.id) == "hi" && m.to == "marketing"));
}

#[test]
fn a_live_holders_queued_and_owed_messages_move_to_it() {
    let mut t = T::new();
    t.spawn_task("marketing");
    // idle: main's question is delivered, marketing owes the reply...
    t.go(Input::ReplIdle { agent: "marketing".into(), leftover: false });
    let send = |t: &mut T, text: &str, expect: bool| {
        let (tok, fx) = t.req(
            MAIN,
            // the second one waits for the turn's end (queued mode)
            AgentReq::Send { to: "marketing".into(), text: text.into(), expect_reply: expect, reply_to: None, queued: !expect, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None },
        );
        assert!(reply(&fx, tok).is_some(), "{fx:?}");
        t.hub.st.msgs().values().find(|m| t.hub.st.text(m.id) == text).unwrap().id
    };
    let owed = send(&mut t, "status?", true);
    // ...and busy with it: the next one waits in its queue
    let queued = send(&mut t, "later", false);
    assert!(t.hub.st.hot().any(|(m, h)| m.id == queued && h.queued), "queued before the take");
    ren(&mut t, "marketing", "design");
    t.spawn_task("launch");
    let n = t.journal.borrow().len();
    ren(&mut t, "launch", "marketing");
    let lines: Vec<_> = t.journal.borrow()[n..].to_vec();
    assert!(lines.iter().any(|j| j["type"] == "msgs_moved" && j["from"] == "marketing" && j["to"] == "design"), "{lines:?}");
    let ended = |id: u64| lines.iter().any(|j| j["id"] == id && (j["type"] == "message_settled" || j["type"] == "message_state"));
    assert!(!ended(owed) && !ended(queued), "neither refused nor settled: {lines:?}");
    // same ids, now to design, still hot (owed, queued)
    let hot: Vec<_> = t.hub.st.hot().map(|(m, h)| (m.id, m.to.clone(), h.queued, h.owes)).collect();
    assert!(hot.contains(&(owed, "design".into(), false, true)), "{hot:?}");
    assert!(hot.contains(&(queued, "design".into(), true, false)), "{hot:?}");
}

#[test]
fn a_card_under_the_old_name_moves_to_its_holder() {
    let mut t = T::new();
    let (_, card) = docs_question_card(&mut t);
    ren(&mut t, "docs", "site");
    assert_eq!(t.hub.st.cards[&card].agent, "docs", "a rename leaves it (its alias resolves)");
    t.spawn_task("docs");
    assert!(!t.hub.st.cards.contains_key(&card), "closed");
    let moved: Vec<_> = t.hub.st.cards.values().filter(|c| c.text.contains("v1 ou v2")).map(|c| c.agent.clone()).collect();
    assert_eq!(moved, ["site"], "opened again for its holder, never for the new docs");
}

#[test]
fn a_reply_the_archived_agent_owed_is_never_the_new_ones() {
    let mut t = T::new();
    t.spawn_task("x");
    // idle: main's question is delivered, x owes the reply
    t.go(Input::ReplIdle { agent: "x".into(), leftover: false });
    let (_, _) = t.req(
        MAIN,
        AgentReq::Send { to: "x".into(), text: "status?".into(), expect_reply: true, reply_to: None, queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: None },
    );
    let id = t.hub.st.msgs().values().find(|m| t.hub.st.text(m.id) == "status?").unwrap().id;
    // archived mid-turn: the reply is still owed (an idle turn's end
    // would have answered it for x)
    t.user(MAIN, "/archive x --force");
    assert_eq!(t.status("x"), Status::Archived);
    let settled = |t: &T| t.journal.borrow().iter().any(|j| j["type"] == "message_settled" && j["id"] == id);
    assert!(!settled(&t), "still owed after the archive");
    t.spawn_task("x");
    assert!(settled(&t), "settled at the take: never the new x's to answer");
}
