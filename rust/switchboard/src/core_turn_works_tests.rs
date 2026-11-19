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

/// the works of `ids` whose agents hold `agent`, in `ids`' order
fn on(t: &T, ids: &[String], agent: &str) -> Vec<String> {
    ids.iter().filter(|id| t.hub.st.works[*id].work.agents.iter().any(|x| x == agent)).cloned().collect()
}

/// vb-architect in his dashboard hub: five tasks steered into one turn.
/// Before, the turn collected all five and stamped them on everything
/// after (73 of its messages carried three tasks at once).
#[test]
fn a_coordinator_steered_with_five_tasks_keeps_its_turns_one_and_replies_carry_their_targets() {
    let mut t = T::new();
    // spawned before the tasks: their briefs carry none
    t.spawn_task("coord");
    t.go(Input::ReplIdle { agent: "coord".into(), leftover: false });
    t.spawn_task("worker");
    t.go(Input::ReplIdle { agent: "worker".into(), leftover: false });
    let ids: Vec<String> = ["Pool", "Triggers", "Codeowners", "Infra page", "Blokhus page"].iter().map(|n| new(&mut t, n)).collect();
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
    // a member of its starting batch's task only (laws turn_start_joins_its_batch, a_steer_never_joins)
    assert_eq!(on(&t, &ids, "coord"), vec![ids[0].clone()]);
    // its answer to each steer carries that steer's task (law reply_carries_its_targets_works)
    for (id, m) in &asks {
        t.req("coord", send(MAIN, "on it", Some(*m), &[]));
        assert_eq!(last_msg(&t, "coord", MAIN)["works"], json!([id]), "a reply to {m}");
    }
    // answering each steer is an act with its task: it joins them (law acting_joins_its_works)
    assert_eq!(on(&t, &ids, "coord"), ids);
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

/// His live hub: main's question about a PR carried the 7 tasks of
/// main's turn, and every one read 'needs you · 1 question' (architect
/// m_67167). Main's card carries only its --task.
#[test]
fn mains_card_carries_only_its_task() {
    let mut t = T::new();
    let a = new(&mut t, "Pool");
    let _b = new(&mut t, "Triggers");
    let card = |task: &[&str]| AgentReq::Card { text: "merge?\n1. merge\n2. not yet".into(), for_msg: None, page: None, artifact: None, open: Vec::new(), about: None, task: task.iter().map(|s| s.to_string()).collect() };
    let opened = |t: &T| t.journal.borrow().iter().rev().find(|j| j["type"] == "card_opened").expect("a card")["card"]["works"].clone();
    t.req(MAIN, card(&[]));
    let none = opened(&t);
    assert!(none.is_null() || none == json!([]), "law mains_card_carries_only_its_task: {none}");
    t.req(MAIN, card(&[a.as_str()]));
    assert_eq!(opened(&t), json!([a]), "law mains_card_keeps_its_task");
}

/// His "Not on this task" (architect m_67152): he or main (on his word)
/// takes an agent off a task; another agent is refused; its next act
/// for the task joins it again.
#[test]
fn not_on_this_task_is_his_or_mains_and_an_act_rejoins() {
    let mut t = T::new();
    t.spawn_task("coord");
    t.go(Input::ReplIdle { agent: "coord".into(), leftover: false });
    t.spawn_task("other");
    t.go(Input::ReplIdle { agent: "other".into(), leftover: false });
    let a = new(&mut t, "Pool");
    t.req(MAIN, send("coord", "start", None, &[a.as_str()]));
    let on_a = |t: &T| t.hub.st.works[&a].work.agents.iter().any(|x| x == "coord");
    assert!(on_a(&t), "a turn start joins");
    // another agent is refused
    let (tok, fx) = t.req("other", AgentReq::Task(Task::Unlink { id: a.clone(), agent: "coord".into() }));
    assert_ne!(reply(&fx, tok).unwrap()["ok"], true, "law only_he_or_main_unlinks_an_agent");
    assert!(on_a(&t));
    // his gesture takes it off, his Undo puts it back
    t.go(Input::ClientWorkAct { client: 7, req: Task::Unlink { id: a.clone(), agent: "coord".into() } });
    assert!(!on_a(&t), "law unlink_drops_the_agent");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Link { id: a.clone(), agent: "coord".into() } });
    assert!(on_a(&t), "his Undo");
    // main on his word; then coord acts for it and is back
    let (tok, fx) = t.req(MAIN, AgentReq::Task(Task::Unlink { id: a.clone(), agent: "coord".into() }));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    assert!(!on_a(&t));
    t.req("coord", send(MAIN, "done with the pool", None, &[a.as_str()]));
    assert!(on_a(&t), "law acting_joins_its_works");
}

/// architect m_66621 point 5: `sb task list --check` names an open task
/// with 8 agents or more (for agents and the gates, never his UI), and
/// only those.
#[test]
fn task_list_check_names_an_over_linked_task() {
    let mut t = T::new();
    let a = new(&mut t, "Pool");
    let _b = new(&mut t, "Triggers");
    for i in 0..8 {
        let (tok, fx) = t.req(MAIN, AgentReq::Task(Task::Link { id: a.clone(), agent: format!("vb-{i}") }));
        assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    }
    let (tok, fx) = t.req(MAIN, AgentReq::Task(Task::List { all: false, check: true }));
    let text = reply(&fx, tok).unwrap()["text"].as_str().unwrap().to_string();
    let lines: Vec<&str> = text.lines().filter(|l| l.contains(" · ")).collect();
    assert_eq!(lines.len(), 1, "{text}");
    assert!(lines[0].starts_with(&format!("{a} · Pool · ")) && lines[0].contains("agents"), "{text}");
}

/// main m_67068: a resume note held by its task's pause reads 'held until
/// <task> resumes' in main's <agent_threads>, never its words as if
/// delivered; once the task resumes and it goes, its words come back.
#[test]
fn a_held_resume_note_says_it_is_held_in_mains_agent_threads() {
    let mut t = T::new();
    let a = new(&mut t, "Native mac");
    t.spawn_task("docs");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    let threads = crate::board::agent_threads(&t.hub.st, 20).join("\n");
    assert!(threads.contains(&format!("held until {a} resumes")), "{threads}");
    assert!(!threads.contains("resumed: pick it up"), "{threads}");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Resume { id: a.clone() } });
    let threads = crate::board::agent_threads(&t.hub.st, 20).join("\n");
    assert!(!threads.contains("held until"), "{threads}");
}

/// architect m_67498 (1): spawn's answer carries its brief's works and
/// the typed several, from the same several_kvs as a send; one task, none.
#[test]
fn a_spawn_whose_brief_carries_several_tasks_says_so() {
    let mut t = T::new();
    let a = new(&mut t, "Pool");
    let b = new(&mut t, "Triggers");
    let spawn = |name: &str, task: Vec<String>| AgentReq::Spawn {
        name: name.into(),
        brief: Brief { objective: format!("objective of {name}"), ..Brief::default() },
        worktree: false,
        with_changes: false,
        place: String::new(),
        feature: String::new(),
        ask: Default::default(),
        parts: String::new(),
        attach: Vec::new(),
        fwd: None,
        task,
    };
    let (tok, fx) = t.req(MAIN, spawn("both", vec![a.clone(), b.clone()]));
    let r = reply(&fx, tok).expect("spawn answers at once");
    assert_eq!(r["several"], true, "{r}");
    let mut ws: Vec<String> = r["works"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
    ws.sort();
    let mut want = vec![a.clone(), b.clone()];
    want.sort();
    assert_eq!(ws, want, "{r}");
    let (tok, fx) = t.req(MAIN, spawn("one", vec![a.clone()]));
    let r = reply(&fx, tok).expect("spawn answers at once");
    assert!(r.get("several").is_none() && r.get("works").is_none(), "{r}");
}
