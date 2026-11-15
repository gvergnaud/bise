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
    let r = task(&mut t, MAIN, Task::Show { id: a.clone(), log: false });
    assert!(r["text"].as_str().unwrap().contains("1. [ ] it builds"), "{r}");
    // main sees it in its state, to put new work on it
    let ctx = crate::board::main_context(&t.hub.st, t.env.now, &[], &[]);
    assert!(ctx.contains("<open_tasks>\nnative-mac · Native mac · working · docs · main, docs\n</open_tasks>"), "{ctx}");
    // done: out of the list, in --all
    assert_eq!(task(&mut t, MAIN, Task::Done { id: a.clone() })["ok"], true);
    let ctx = crate::board::main_context(&t.hub.st, t.env.now, &[], &[]);
    assert!(!ctx.contains("<open_tasks>"), "a done task is not open: {ctx}");
    assert_eq!(t.hub.st.works[&a].status, "done");
    let open = task(&mut t, MAIN, Task::List { all: false })["text"].as_str().unwrap().to_string();
    assert!(!open.contains("native-mac"), "{open}");
    let all = task(&mut t, MAIN, Task::List { all: true })["text"].as_str().unwrap().to_string();
    assert!(all.contains("native-mac · Native mac · done · today"), "{all}");
    let r = task(&mut t, MAIN, Task::Show { id: "nope".into(), log: false });
    assert_eq!(r["ok"], false, "{r}");
}

#[test]
fn his_page_waits_make_needs_you_and_a_ragged_input_is_refused() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    let b = new(&mut t, MAIN, "Digest", &[]);
    t.go(Input::PageWaits { works: vec![a.clone()], ns: vec![2] });
    assert_eq!((t.hub.st.works[&a].status.as_str(), t.hub.st.works[&a].needs), ("needs_you", 2));
    let r = task(&mut t, MAIN, Task::List { all: false });
    assert!(r["text"].as_str().unwrap().starts_with("native-mac · Native mac · needs you · 2 questions"), "{r}");
    // the other work's row is untouched: still its last one
    assert_eq!(t.hub.st.works[&b].status, "waiting");
    // two lists of different lengths: refused, the page waits kept
    t.go(Input::PageWaits { works: vec![a.clone(), b.clone()], ns: vec![1] });
    assert_eq!(t.hub.st.works[&a].needs, 2, "a short list is never zipped");
    // his waits answered: back to its status
    t.go(Input::PageWaits { works: vec![], ns: vec![] });
    assert_eq!(t.hub.st.works[&a].status, "waiting");
}

fn notice_to(fx: &[Effect], client: ClientId) -> Option<String> {
    fx.iter().find_map(|e| match e {
        Effect::ToClient { client: c, body } if *c == client && body["ev"] == "notice" => body["text"].as_str().map(String::from),
        _ => None,
    })
}

#[test]
fn his_gesture_on_a_task_is_made_as_him_and_a_refusal_is_his_notice() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    // his pause from the desktop: by the user, never an agent
    let fx = t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    assert_eq!(notice_to(&fx, 7), None, "{fx:?}");
    let ch = lines(&t, "work_changed");
    assert_eq!((ch.last().unwrap()["op"].as_str(), ch.last().unwrap()["by"].as_str()), (Some("pause"), Some("user")), "{ch:?}");
    assert_eq!(t.hub.st.works[&a].status, "paused");
    // his gesture starts no turn: no agent serves what he made
    let turns = lines(&t, "work_turn").len();
    let fx = t.go(Input::ClientWorkAct { client: 7, req: Task::New { name: "Digest".into(), why: String::new(), goals: vec![] } });
    assert_eq!(notice_to(&fx, 7), None, "{fx:?}");
    assert_eq!(lines(&t, "work_created").last().unwrap()["work"]["by"], "user");
    assert_eq!(lines(&t, "work_turn").len(), turns, "his new work joins no agent's turn");
    // an unknown task: refused, its word to his client only
    let fx = t.go(Input::ClientWorkAct { client: 7, req: Task::Done { id: "nope".into() } });
    assert!(notice_to(&fx, 7).is_some_and(|n| n.contains("no task nope")), "{fx:?}");
    // the reads are the hub's: a gesture never asks for them
    let fx = t.go(Input::ClientWorkAct { client: 7, req: Task::List { all: false } });
    assert!(notice_to(&fx, 7).is_some(), "{fx:?}");
}

#[test]
fn his_move_of_an_artifact_or_a_card_is_journaled_and_an_unknown_one_refused() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    let fx = t.go(Input::ClientWorkMove { client: 7, card: None, art: "a_41".into(), page: false, works: vec![a.clone()] });
    assert_eq!(notice_to(&fx, 7), None, "{fx:?}");
    let m = lines(&t, "art_moved");
    assert_eq!(m.last().unwrap()["moved"], json!({"art": "a_41", "works": [a]}), "{m:?}");
    // the store's Meta is told (its one owner): sb-core keeps no map
    assert!(fx.contains(&Effect::ArtWorks { art: "a_41".into(), works: vec![a.clone()], page: false }), "{fx:?}");
    // a page: typed from the row he acted on, all the way to the store
    let fx = t.go(Input::ClientWorkMove { client: 7, card: None, art: "a_41".into(), page: true, works: vec![a.clone()] });
    assert_eq!(lines(&t, "art_moved").last().unwrap()["moved"]["page"], true);
    assert!(fx.contains(&Effect::ArtWorks { art: "a_41".into(), works: vec![a.clone()], page: true }), "{fx:?}");
    // out of every task: an empty list
    t.go(Input::ClientWorkMove { client: 7, card: None, art: "a_41".into(), page: false, works: vec![] });
    assert_eq!(lines(&t, "art_moved").last().unwrap()["moved"]["art"], "a_41");
    // an unknown card or task: nothing moved, his notice
    let fx = t.go(Input::ClientWorkMove { client: 7, card: Some(99), art: String::new(), page: false, works: vec![a.clone()] });
    assert!(notice_to(&fx, 7).is_some_and(|n| n.contains("no card #99")), "{fx:?}");
    let fx = t.go(Input::ClientWorkMove { client: 7, card: None, art: "a_42".into(), page: false, works: vec!["nope".into()] });
    assert!(notice_to(&fx, 7).is_some(), "{fx:?}");
    assert_eq!(lines(&t, "card_moved").len(), 0);
    assert_eq!(lines(&t, "art_moved").len(), 3);
}

fn interrupted(fx: &[Effect], agent: &str) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Interrupt { agent: a, .. } if a == agent))
}

fn sent_to(t: &T, to: &str) -> Vec<Value> {
    lines(t, "message_sent").into_iter().map(|j| j["msg"].clone()).filter(|m| m["to"] == to).collect()
}

#[test]
fn a_paused_task_notes_its_turn_holds_its_messages_and_stops_it_after_the_grace() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    // docs's turn serves it (its brief inherits main's turn)
    t.spawn_task("docs");
    assert_eq!(lines(&t, "work_turn").last().unwrap()["turn"]["works"], json!([a]));
    let before = sent_to(&t, "docs").len();
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    // the note first (no work: a steer), and one resume note carrying [a]
    let new: Vec<Value> = sent_to(&t, "docs").into_iter().skip(before).collect();
    assert_eq!(new.len(), 2, "{new:?}");
    assert!(new[0]["text"].as_str().unwrap().starts_with(&format!("task {a} is paused: save your work")), "{new:?}");
    assert_eq!(new[0]["works"], Value::Null, "the note carries no work");
    assert_eq!(new[1]["works"], json!([a]), "law pause_queues_one_resume_note_per_agent");
    let note = new[1]["id"].as_u64().unwrap();
    assert!(matches!(t.hub.st.msg_state()[&note], MsgState::Queued { .. }), "held while paused");
    // main's next word on that task waits too (law paused_work_holds_its_deliveries)
    t.req(MAIN, send("docs", "and the icon", &[&a]));
    let held = sent_to(&t, "docs").last().unwrap()["id"].as_u64().unwrap();
    assert!(matches!(t.hub.st.msg_state()[&held], MsgState::Queued { .. }));
    // within the grace: never stopped
    t.env.now += 60_000;
    assert!(!interrupted(&t.go(Input::Tick), "docs"), "law pause_interrupts_only_after_grace");
    // past it, still busy: stopped once, its turn serves nothing
    t.env.now += 61_000;
    assert!(interrupted(&t.go(Input::Tick), "docs"));
    let turn = lines(&t, "work_turn").last().unwrap()["turn"].clone();
    assert_eq!((turn["agent"].as_str(), turn["works"].clone()), (Some("docs"), json!([])), "{turn}");
    t.env.now += 1_000;
    assert!(!interrupted(&t.go(Input::Tick), "docs"), "one stop, never again");
    // his resume: the held messages go, the resume note with them
    t.go(Input::ClientWorkAct { client: 7, req: Task::Resume { id: a.clone() } });
    assert_ne!(t.hub.st.works[&a].status, "paused");
    for id in [note, held] {
        assert_eq!(t.hub.st.msg_state()[&id], MsgState::Delivered, "law work_resume_delivers_held");
    }
}

#[test]
fn a_turn_with_another_open_task_is_never_stopped() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    let b = new(&mut t, MAIN, "Digest", &[]);
    t.spawn_task("docs");
    assert_eq!(lines(&t, "work_turn").last().unwrap()["turn"]["works"], json!([a, b]));
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    let note = sent_to(&t, "docs").into_iter().find(|m| m["text"].as_str().unwrap_or("").contains("is paused")).unwrap();
    assert!(note["text"].as_str().unwrap().ends_with(&format!("then go on with {b}.")), "{note}");
    t.env.now += 200_000;
    assert!(!interrupted(&t.go(Input::Tick), "docs"), "law pause_never_stops_an_agent_with_an_open_work");
}

#[test]
fn a_pause_until_a_time_resumes_by_itself_and_a_stale_end_does_nothing() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    let until = t.env.now + 10_000;
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: Some(until) } });
    let act = lines(&t, "act_set").last().unwrap()["act"].clone();
    assert_eq!((act["due"].as_u64(), act["what"].clone()), (Some(until), json!({"kind": "work_resume", "work": a})), "law pause_until_is_a_timer");
    assert_eq!(t.hub.st.works[&a].work.until_timer, act["id"].as_u64());
    // paused again with no end: the old act does nothing
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    t.env.now = until + 1;
    t.go(Input::Tick);
    assert_eq!(t.hub.st.works[&a].status, "paused", "law pause_without_end_never_resumes");
    // a new end: it resumes when it comes, by bise
    let until = t.env.now + 5_000;
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: Some(until) } });
    t.env.now = until;
    t.go(Input::Tick);
    let ch = lines(&t, "work_changed").last().unwrap().clone();
    assert_eq!((ch["op"].as_str(), ch["by"].as_str()), (Some("resume"), Some("switchboard")), "by bise: {ch}");
    assert_ne!(t.hub.st.works[&a].status, "paused");
}

fn his_words(t: &mut T, to: &str, text: &str) {
    t.go(Input::ClientInput { client: 1, focus: to.into(), text: text.into(), queued: false, parts: String::new(), from: String::new(), works: Vec::new() });
}

fn his_task_words(t: &mut T, to: &str, text: &str, works: &[&str]) -> Vec<Effect> {
    t.go(Input::ClientInput { client: 1, focus: to.into(), text: text.into(), queued: false, parts: String::new(), from: String::new(), works: works.iter().map(|w| w.to_string()).collect() })
}

#[test]
fn his_task_tab_words_carry_its_task_a_merged_ones_target_and_an_unknown_one_refuses() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Login timeout", &[]);
    let b = new(&mut t, MAIN, "Pricing", &[]);
    // explicit works: exactly them, whatever main's turn serves (law his_task_tab_words_carry_its_task)
    his_task_words(&mut t, MAIN, "new agent on Login timeout: repro on 3G", &[&a]);
    let his = sent_to(&t, MAIN).last().unwrap().clone();
    assert_eq!((his["text"].as_str(), his["works"].clone()), (Some("new agent on Login timeout: repro on 3G"), json!([a])), "{his}");
    // a merged id reads as the task it went into (the --task path)
    let c = new(&mut t, MAIN, "Old login", &[]);
    assert_eq!(task(&mut t, MAIN, Task::Merge { id: c.clone(), into: b.clone() })["ok"], true);
    his_task_words(&mut t, MAIN, "new agent on Old login: the copy", &[&c]);
    assert_eq!(sent_to(&t, MAIN).last().unwrap()["works"], json!([b]));
    // an unknown id: refused with a word, nothing sent (his draft stays in his window)
    let n = sent_to(&t, MAIN).len();
    let fx = his_task_words(&mut t, MAIN, "new agent on Gone: hi", &["gone"]);
    assert_eq!(sent_to(&t, MAIN).len(), n, "nothing sent");
    assert!(format!("{fx:?}").contains("no task gone"), "{fx:?}");
}

#[test]
fn his_task_tab_words_to_a_paused_task_are_never_held() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "WhatsApp", &[]);
    t.spawn_task("wa");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    // his word with works=[the paused one] goes through (law his_words_are_never_held)
    his_task_words(&mut t, "wa", "one more thing", &[&a]);
    let his = sent_to(&t, "wa").last().unwrap().clone();
    assert_eq!(his["works"], json!([a]));
    assert_eq!(t.hub.st.msg_state()[&his["id"].as_u64().unwrap()], MsgState::Delivered);
}

#[test]
fn his_words_reach_a_paused_tasks_agent_and_a_closed_task_releases_what_it_held() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    t.spawn_task("docs");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    // his word on it goes, paused or not (law his_words_are_never_held)
    his_words(&mut t, "docs", "one more thing");
    let his = sent_to(&t, "docs").last().unwrap().clone();
    assert_eq!((his["text"].as_str(), his["works"].clone()), (Some("one more thing"), json!([a])), "{his}");
    assert_eq!(t.hub.st.msg_state()[&his["id"].as_u64().unwrap()], MsgState::Delivered);
    // main's word on it waits; done on the paused task: what it held goes
    t.req(MAIN, send("docs", "and the icon", &[&a]));
    let held = sent_to(&t, "docs").last().unwrap()["id"].as_u64().unwrap();
    assert!(matches!(t.hub.st.msg_state()[&held], MsgState::Queued { .. }));
    assert_eq!(task(&mut t, MAIN, Task::Done { id: a.clone() })["ok"], true);
    assert_eq!(t.hub.st.works[&a].work.paused, None, "law a_closed_work_holds_nothing");
    assert_eq!(t.hub.st.msg_state()[&held], MsgState::Delivered);
}

#[test]
fn a_merged_paused_task_sends_what_it_held_to_its_target() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Native mac", &[]);
    t.spawn_task("docs");
    let b = new(&mut t, MAIN, "Mac app", &[]);
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    t.req(MAIN, send("docs", "and the icon", &[&a]));
    let held = sent_to(&t, "docs").last().unwrap()["id"].as_u64().unwrap();
    assert_eq!(task(&mut t, MAIN, Task::Merge { id: a.clone(), into: b.clone() })["ok"], true);
    assert_eq!(t.hub.st.msg_state()[&held], MsgState::Delivered, "law a_merged_work_holds_nothing");
    // delivered, it serves the target (merge_redirects at the delivery)
    let turn = lines(&t, "work_turn").last().unwrap()["turn"].clone();
    assert_eq!(turn["agent"], "docs");
    assert!(turn["works"].as_array().unwrap().contains(&json!(b)), "{turn}");
}

#[test]
fn a_timer_of_a_paused_task_waits_and_fires_once_at_its_resume() {
    use crate::every::MIN_MS;
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Digest", &[]);
    t.spawn_task("docs");
    // main's timer carries its turn's work (law timer_carries_turn_works)
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::Add { to: "docs".into(), text: "the digest".into(), every_ms: Some(MIN_MS), daily_min: None, until_ms: None, times: Some(2), page: None, name: None }));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    let fired = |t: &T| lines(t, "every_fired");
    for k in 1..=3 {
        t.env.now += MIN_MS + 1;
        t.go(Input::Tick);
        assert_eq!(fired(&t).len(), 0, "tick {k}: held while its task is paused");
    }
    t.go(Input::ClientWorkAct { client: 7, req: Task::Resume { id: a.clone() } });
    t.env.now += 1_000;
    t.go(Input::Tick);
    assert_eq!(fired(&t).len(), 1, "the missed ones fire once");
    assert_eq!(fired(&t)[0]["fired"], 1, "--times counts the catch-up as one");
    t.env.now += 1_000;
    t.go(Input::Tick);
    assert_eq!(fired(&t).len(), 1);
}

#[test]
fn a_timer_whose_end_passed_while_paused_ends_and_never_fires() {
    use crate::every::MIN_MS;
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Digest", &[]);
    t.spawn_task("docs");
    let end = t.env.now + 2 * MIN_MS + 500;
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::Add { to: "docs".into(), text: "the digest".into(), every_ms: Some(MIN_MS), daily_min: None, until_ms: Some(end), times: None, page: None, name: None }));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    t.go(Input::ClientWorkAct { client: 7, req: Task::Pause { id: a.clone(), until_ms: None } });
    for _ in 0..4 {
        t.env.now += MIN_MS + 1;
        t.go(Input::Tick);
    }
    let stop = lines(&t, "every_stop");
    assert_eq!(stop.last().map(|s| s["why"].clone()), Some(json!("its end time passed")), "{stop:?}");
    t.go(Input::ClientWorkAct { client: 7, req: Task::Resume { id: a.clone() } });
    t.env.now += 1_000;
    t.go(Input::Tick);
    assert!(lines(&t, "every_fired").is_empty(), "law until_passed_while_paused_never_fires");
}

/// architect m_64926: a watch carries its setter's turn's works; its hit
/// goes out with them (law watch_carries_turn_works), and a done task
/// never holds it (law a_done_works_watch_is_never_held).
#[test]
fn a_watch_carries_its_turns_works_and_a_done_task_never_holds_its_wake() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Digest", &[]);
    let spec = crate::wake::Spec { what: crate::wake::What::Pid { pid: 4242, start: String::new() }, tail: None, note: "the build".into() };
    t.req(MAIN, AgentReq::Wake(WakeReq::Add { spec: spec.json(), max_ms: Some(crate::wake::MAX_DEFAULT_MS), then: None }));
    let set = lines(&t, "wake_set");
    let w = &set.last().expect("a wake_set line")["watch"];
    assert_eq!(w["works"], json!([a]), "{w}");
    let id = w["id"].as_u64().unwrap();
    assert_eq!(task(&mut t, MAIN, Task::Done { id: a.clone() })["ok"], true);
    t.go(Input::WakeHit { id, text: "pid 4242 ended · after 5s · the build".into(), rc: None, tail: vec![], after_ms: 5_000 });
    let m = last_msg_to(&t, MAIN);
    assert_eq!((m["from"].as_str(), m["works"].clone()), (Some("switchboard"), json!([a])), "{m}");
    assert!(!matches!(t.hub.st.msg_state()[&m["id"].as_u64().unwrap()], MsgState::Queued { .. }), "a done task holds nothing");
}

fn report(t: &mut T, from: &str, kind: &str, summary: &str) {
    t.req(from, AgentReq::Report { kind: kind.into(), summary: summary.into(), decisions: vec![], step: None, result: None, done_when: None, task: Vec::new() });
}

fn report_cards(t: &T, agent: &str) -> Vec<u64> {
    t.hub.st.cards.values().filter(|c| c.from_report && c.agent == agent).map(|c| c.id).collect()
}

/// spec tasks-center row 63, architect m_64926/m_64931: an agent's `sb
/// report blocked` opens ONE card for him (law
/// blocked_report_opens_one_card), main's notice names it; his answer
/// reaches that agent with the card's works and closes it (law
/// his_answer_on_a_blocked_card_reaches_its_agent); a progress report
/// closes the next one (law blocked_card_ends_when_it_moves_on).
#[test]
fn a_blocked_report_is_one_card_his_answer_reaches_its_agent_and_moving_on_closes_it() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Shared KB", &[]);
    t.spawn_task("kb");
    t.go(Input::ReplIdle { agent: "kb".into(), leftover: false });
    t.req(MAIN, send("kb", "build the KB", &[a.as_str()]));
    report(&mut t, "kb", "blocked", "create the KB bise-poc first");
    let ids = report_cards(&t, "kb");
    assert_eq!(ids.len(), 1, "{:?}", t.hub.st.cards);
    let c = &t.hub.st.cards[&ids[0]];
    assert_eq!((c.kind.as_str(), c.works.clone()), ("blocked", vec![a.clone()]), "the wire kind stays, the card carries its task");
    let named = format!("card #{}", ids[0]);
    assert!(t.hub.st.msgs().values().any(|m| m.to == MAIN && t.hub.st.text(m.id).contains(&named)), "main's notice names the card");
    report(&mut t, "kb", "blocked", "still blocked");
    assert_eq!(t.hub.st.cards.values().filter(|c| c.agent == "kb").count(), 1, "one card, never two");
    t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card: ids[0], text: "done, it's created".into(), parts: String::new() } });
    assert!(t.hub.st.cards.is_empty(), "answering closes it");
    let m = last_msg_to(&t, "kb");
    assert_eq!((m["from"].as_str(), m["works"].clone()), (Some("user"), json!([a])), "{m}");
    report(&mut t, "kb", "blocked", "a key now");
    assert_eq!(report_cards(&t, "kb").len(), 1);
    report(&mut t, "kb", "progress", "unblocked");
    assert!(report_cards(&t, "kb").is_empty(), "it moved on: its card closes");
}

/// architect m_64931: the fact decides, never the kind: main's own card
/// of kind "blocked" stays when its agent moves on (law
/// a_notify_blocked_card_never_ends_on_moved_on).
#[test]
fn a_blocked_card_main_opened_stays_when_the_agent_moves_on() {
    let mut t = T::new();
    t.spawn_task("kb");
    t.req("kb", AgentReq::Status { status: Declared::Blocked, note: "need a key".into() });
    assert!(t.hub.st.cards.is_empty(), "sb status blocked opens no card");
    report(&mut t, "kb", "blocked", "need a key");
    let id = report_cards(&t, "kb")[0];
    t.req("kb", AgentReq::Status { status: Declared::Working, note: String::new() });
    assert!(!t.hub.st.cards.contains_key(&id), "status working closes it");
}

/// tasks-qa (architect m_65127): an agent's blocked report is HIS card:
/// it reaches the client's cards (model::user_card) and an sb-core
/// restarted on the journal keeps it open (cardkind.bend user_card: laws
/// a_report_blocked_card_reaches_his_inbox, legacy_blocked_still_closes_at_boot).
/// On a live hub it was dropped from hub/cards and closed at boot while
/// its task said Needs you.
#[test]
fn a_blocked_report_card_reaches_his_inbox_and_survives_a_restart() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Shared KB", &[]);
    t.spawn_task("kb");
    t.go(Input::ReplIdle { agent: "kb".into(), leftover: false });
    t.req(MAIN, send("kb", "build the KB", &[a.as_str()]));
    report(&mut t, "kb", "blocked", "create the KB bise-poc first");
    let ids = report_cards(&t, "kb");
    assert_eq!(ids.len(), 1);
    let (his, _) = crate::proto_view::cards(&t.hub.snapshot(1_000), "p", 1_000, crate::model::user_kind);
    assert!(his.iter().any(|c| c.id == ids[0]), "his inbox has the blocked card: {his:?}");
    let journal = t.journal.clone();
    t.hub.set_revive(Revive::new(Box::new(move || journal.borrow().clone()), Box::new(|_| {})));
    t.hub.kill_core();
    t.user(MAIN, "hello");
    assert!(t.hub.st.cards.contains_key(&ids[0]), "the card survives the restart: {:?}", t.hub.st.cards);
    assert_eq!(t.hub.st.works[&a].status, "needs_you");
}

/// tasks-qa (architect m_65127): a mid-turn birth (an agent's `sb task
/// new`) updates that agent's row: the shell's mirror of its turn_works,
/// which stamps the artifacts, pages and lands of the rest of the turn
/// (daemon/art.rs art_turn_works). On a live hub the artifact added right
/// after carried none.
#[test]
fn a_mid_turn_birth_reaches_the_shells_turn_works() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplIdle { agent: "docs".into(), leftover: false });
    his_words(&mut t, "docs", "the docs look stale");
    let b = new(&mut t, "docs", "Docs cleanup", &[]);
    assert_eq!(t.hub.st.agents["docs"].turn_works.clone().unwrap_or_default(), vec![b], "the mirror's turn_works has the new task");
}

/// tasks-qa (architect m_65127): his plain words to main carry no task,
/// even when main's turn serves exactly one (law
/// his_plain_words_to_main_carry_no_works); main's new task's agent,
/// sent with --task <new id>, serves that one alone. On a live hub the
/// new agent's brief carried the old task and the new one.
#[test]
fn his_new_ask_to_main_after_a_report_starts_a_task_alone() {
    let mut t = T::new();
    let a = new(&mut t, MAIN, "Login timeout", &[]);
    t.spawn_task("fixer");
    t.go(Input::ReplIdle { agent: "fixer".into(), leftover: false });
    assert_eq!(t.hub.st.agents[MAIN].turn_works.clone().unwrap_or_default(), vec![a.clone()], "main's turn serves one task");
    his_words(&mut t, MAIN, "also: write the release notes");
    assert_eq!(sent_to(&t, MAIN).last().unwrap()["works"], Value::Null, "his plain words to main carry none");
    let b = new(&mut t, MAIN, "Release notes", &[]);
    t.spawn_task("writer");
    t.req(MAIN, send("writer", "write them", &[b.as_str()]));
    assert_eq!(last_msg_to(&t, "writer")["works"], json!([b]), "--task <new id> replaces the turn's works");
    // to another agent, the single-task rule still holds
    t.req(MAIN, send("fixer", "go", &[a.as_str()]));
    his_words(&mut t, "fixer", "one more thing");
    assert_eq!(sent_to(&t, "fixer").last().unwrap()["works"], json!([a]));
}
