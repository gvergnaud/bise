//! What a turn's messages carry (architect m_66621, m_66646; tasks-fresh's
//! cause 3 in his dashboard hub): a coordinator steered with several tasks
//! keeps its turn's one, its reply carries its target's, and a journal
//! written before the change replays the same.

use super::super::*;
use bise_proto::sb::Task;

fn new(t: &mut T, name: &str) -> String {
    let (tok, fx) = t.req(MAIN, AgentReq::Task(Task::New { name: name.into(), why: String::new(), goals: Vec::new() }));
    let r = reply(&fx, tok).expect("sb task answers at once");
    r["id"].as_str().unwrap().to_string()
}

fn send(to: &str, text: &str, reply_to: Option<u64>, task: &[&str]) -> AgentReq {
    AgentReq::Send {
        to: to.into(),
        text: text.into(),
        expect_reply: false,
        reply_to,
        queued: false,
        why: String::new(),
        switch: None,
        parts: String::new(),
        attach: Vec::new(),
        fwd: None,
        task: task.iter().map(|s| s.to_string()).collect(),
    }
}

fn last_msg(t: &T, from: &str, to: &str) -> Value {
    t.journal.borrow().iter().rev().find(|j| j["type"] == "message_sent" && j["msg"]["from"] == from && j["msg"]["to"] == to).expect("a message")["msg"].clone()
}

fn turn_works(t: &T, agent: &str) -> Vec<String> {
    t.hub.st.agents[agent].turn_works.clone().unwrap_or_default()
}

/// vb-architect in his dashboard hub: five tasks steered into one turn.
/// Before, the turn collected all five and stamped them on everything
/// after (73 of its messages carried three tasks at once).
#[test]
fn a_coordinator_steered_with_five_tasks_keeps_its_turns_one_and_replies_carry_their_targets() {
    let mut t = T::new();
    let ids: Vec<String> = ["Pool", "Triggers", "Codeowners", "Infra page", "Blokhus page"].iter().map(|n| new(&mut t, n)).collect();
    t.spawn_task("coord");
    t.go(Input::ReplIdle { agent: "coord".into(), leftover: false });
    t.spawn_task("worker");
    t.go(Input::ReplIdle { agent: "worker".into(), leftover: false });
    // the delivery that starts its turn sets its task (law delivered_sets_turn_works)
    t.req(MAIN, send("coord", "start the pool", None, &[ids[0].as_str()]));
    assert_eq!(turn_works(&t, "coord"), vec![ids[0].clone()]);
    // four steers mid-turn, each its own task: the turn keeps its one
    let mut asks = Vec::new();
    for id in &ids[1..] {
        t.req(MAIN, send("coord", &format!("also {id}"), None, &[id.as_str()]));
        asks.push((id.clone(), last_msg(&t, MAIN, "coord")["id"].as_u64().unwrap()));
    }
    assert_eq!(turn_works(&t, "coord"), vec![ids[0].clone()], "law steered_keeps_turn_works");
    // its answer to each steer carries that steer's task (law reply_carries_its_targets_works)
    for (id, m) in &asks {
        t.req("coord", send(MAIN, "on it", Some(*m), &[]));
        assert_eq!(last_msg(&t, "coord", MAIN)["works"], json!([id]), "a reply to {m}");
    }
    // anything else it sends carries its turn's one task, never the five
    t.req("coord", send("worker", "build the pool", None, &[]));
    assert_eq!(last_msg(&t, "coord", "worker")["works"], json!([ids[0]]));
    // and --task still replaces both rules (law agent_task_flag_replaces_all)
    let (_, m) = asks[0].clone();
    t.req("coord", send(MAIN, "moving it", Some(m), &[ids[4].as_str()]));
    assert_eq!(last_msg(&t, "coord", MAIN)["works"], json!([ids[4]]));
}

/// Architect m_66646: a journal written before the change (a steered
/// work_turn line with add, no join key) replays exactly as it did: the
/// turn's works are the union and the agent is on both tasks.
#[test]
fn an_old_journals_steered_line_replays_as_before() {
    let mut t = T::new();
    let a = new(&mut t, "Pool");
    let b = new(&mut t, "Triggers");
    t.spawn_task("coord");
    let at = t.env.now;
    let old = |ws: &[&str], add: bool| json!({"type": "work_turn", "turn": {"agent": "coord", "works": ws, "add": add, "at": at}});
    t.hub.replay(&[old(&[a.as_str()], false), old(&[b.as_str()], true)]);
    assert_eq!(turn_works(&t, "coord"), vec![a.clone(), b.clone()], "law old_steered_line_still_unions");
    for id in [&a, &b] {
        let agents = &t.hub.st.works[id].work.agents;
        assert!(agents.iter().any(|x| x == "coord"), "{id}: {agents:?}");
    }
}
