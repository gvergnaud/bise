//! tasks (docs/tasks.md): `sb task` verbs and `--task` through the hub,
//! sb-core's journal lines checked. Out of core_tests.rs (its line limit).

use super::super::*;
use bise_proto::sb::Task;

fn task(t: &mut T, from: &str, req: Task) -> Value {
    let (tok, fx) = t.req(from, AgentReq::Task(req));
    reply(&fx, tok).expect("sb task answers at once")
}

fn new(t: &mut T, from: &str, name: &str, goals: &[&str]) -> String {
    let r = task(t, from, Task::New { name: name.into(), why: "he asked".into(), goals: goals.iter().map(|g| g.to_string()).collect() });
    assert_eq!(r["ok"], true, "{r}");
    r["id"].as_str().unwrap().to_string()
}

fn lines(t: &T, kind: &str) -> Vec<Value> {
    t.journal.borrow().iter().filter(|j| j["type"] == kind).cloned().collect()
}

fn last_msg_to(t: &T, to: &str) -> Value {
    lines(t, "message_sent").into_iter().rev().find(|j| j["msg"]["to"] == to).expect("a message").clone()["msg"].clone()
}

fn send(to: &str, text: &str, task: &[&str]) -> AgentReq {
    AgentReq::Send {
        to: to.into(),
        text: text.into(),
        expect_reply: false,
        reply_to: None,
        queued: false,
        why: String::new(),
        switch: None,
        parts: String::new(),
        attach: Vec::new(),
        fwd: None,
        task: task.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn a_new_task_gets_a_free_id_and_its_maker_serves_it() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &["it builds", "he tried it"]);
    let b = new(&mut t, MAIN, "Native mac", &[]);
    assert_eq!((a.as_str(), b.as_str()), ("native-mac", "native-mac-2"), "law work_id_unique");
    let made = lines(&t, "work_created");
    assert_eq!(made.len(), 2);
    assert_eq!(made[0]["work"]["by"], MAIN);
    assert_eq!(made[0]["work"]["goals"].as_array().map(Vec::len), Some(2));
    // law work_birth_takes_turn: main's turn serves both from then
    let turns = lines(&t, "work_turn");
    assert_eq!(turns.last().unwrap()["turn"]["agent"], MAIN, "{turns:?}");
    // a message main sends in that turn carries them
    t.spawn_task("docs");
    t.req(MAIN, send("docs", "go", &[]));
    let m = last_msg_to(&t, "docs");
    assert_eq!(m["works"], json!([a, b]), "law sent_in_turn_carries_turn_works: {m}");
}

#[test]
fn the_task_flag_replaces_the_turns_works_and_an_unknown_id_is_refused() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    let b = new(&mut t, MAIN, "Digest", &[]);
    t.spawn_task("docs");
    t.req(MAIN, send("docs", "only this one", &[&b]));
    assert_eq!(last_msg_to(&t, "docs")["works"], json!([b]), "law task_flag_replaces_turn_works");
    let (tok, fx) = t.req(MAIN, send("docs", "x", &["nope"]));
    let r = reply(&fx, tok).unwrap();
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("no task nope"), "{r}");
    // a merged id reads its target (law merge_redirects)
    assert_eq!(task(&mut t, MAIN, Task::Merge { id: b.clone(), into: a.clone() })["ok"], true);
    t.req(MAIN, send("docs", "again", &[&b]));
    assert_eq!(last_msg_to(&t, "docs")["works"], json!([a]));
}

#[test]
fn an_agent_ticks_its_done_when_and_only_main_changes_the_rest() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &["it builds", "he tried it"]);
    t.spawn_task("docs");
    // docs' brief came in main's turn: its turn serves the task
    let r = task(&mut t, "docs", Task::Done { id: a.clone() });
    assert_eq!(r["ok"], false, "{r}");
    let r = task(&mut t, "docs", Task::Tick { id: a.clone(), n: 3 });
    assert!(r["error"].as_str().unwrap().contains("no done-when #3"), "{r}");
    assert_eq!(task(&mut t, "docs", Task::Tick { id: a.clone(), n: 1 })["ok"], true);
    // law an_agent_never_acts_as_him: the change is the agent's
    let ch = lines(&t, "work_changed");
    assert_eq!(ch.last().unwrap()["by"], "docs", "{ch:?}");
    assert_eq!(task(&mut t, MAIN, Task::Done { id: a.clone() })["ok"], true);
    let r = task(&mut t, MAIN, Task::Reopen { id: "gone".into() });
    assert!(r["error"].as_str().unwrap().contains("no task gone"), "{r}");
}

#[test]
fn a_report_ticks_a_done_when_of_its_turns_task() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &["it builds"]);
    t.spawn_task("docs");
    t.go(Input::ReplReady { agent: "docs".into() });
    let report = AgentReq::Report { kind: "progress".into(), summary: "built".into(), decisions: vec![], step: None, result: None, done_when: Some(1), task: vec![a.clone()] };
    t.req("docs", report);
    let ch = lines(&t, "work_changed");
    let tick = ch.iter().find(|j| j["id"] == a.as_str()).expect("a tick");
    assert_eq!(tick["by"], "docs", "{tick}");
}

#[test]
fn list_and_show_read_the_status_sb_core_computed() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &["it builds"]);
    let w = &t.hub.st.works[&a];
    assert_eq!(w.work.name, "Native mac", "the step's view carries the work it made");
    // nobody runs a turn of it yet: Waiting, quiet since its birth
    assert_eq!((w.status.as_str(), w.why.as_str(), w.at), ("waiting", "quiet", Some(t.env.now)), "{w:?}");
    // docs' brief came in main's turn: the turn it starts serves it (law
    // delivered_sets_turn_works): Working, and who
    t.spawn_task("docs");
    let w = &t.hub.st.works[&a];
    assert_eq!(w.work.agents, ["main", "docs"], "{w:?}");
    assert_eq!((w.status.as_str(), w.who.as_str()), ("working", "docs"), "{w:?}");
    let r = task(&mut t, MAIN, Task::List { all: false });
    let text = r["text"].as_str().unwrap();
    assert!(text.starts_with("native-mac · Native mac · working · docs"), "{text}");
    let r = task(&mut t, MAIN, Task::Show { id: a.clone() });
    assert!(r["text"].as_str().unwrap().contains("1. [ ] it builds"), "{r}");
    // done: out of the list, in --all
    assert_eq!(task(&mut t, MAIN, Task::Done { id: a.clone() })["ok"], true);
    assert_eq!(t.hub.st.works[&a].status, "done");
    let open = task(&mut t, MAIN, Task::List { all: false })["text"].as_str().unwrap().to_string();
    assert!(!open.contains("native-mac"), "{open}");
    let all = task(&mut t, MAIN, Task::List { all: true })["text"].as_str().unwrap().to_string();
    assert!(all.contains("native-mac · Native mac · done"), "{all}");
    let r = task(&mut t, MAIN, Task::Show { id: "nope".into() });
    assert_eq!(r["ok"], false, "{r}");
}
