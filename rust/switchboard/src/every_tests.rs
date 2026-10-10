//! `sb every`'s timers on sb-core (bend/hub/timers.bend, core.bend's
//! timers section): the tests every.rs had when the timers were Rust's,
//! now through the hub and a real sb-core, plus the replay of a journal
//! today's hub wrote (tests/fixtures/every-journal-298407ff.jsonl).

use super::*;
use crate::every::{next_daily, Sched, ENDED_SHOWN_MS, MIN_MS};

const N: u64 = 1_790_000_000_000;

/// `sb every` from `from`: the new timer's id.
fn every(t: &mut T, from: &str, to: &str, sched: Sched, until_ms: Option<u64>, times: Option<u64>) -> u64 {
    let req = EveryReq::Add { to: to.into(), text: "check HN".into(), sched, until_ms, times, page: None, name: None };
    let (tok, fx) = t.req(from, AgentReq::Every(req));
    let r = reply(&fx, tok).unwrap();
    r["id"].as_u64().unwrap_or_else(|| panic!("{}", r))
}

fn tick(t: &mut T, at: u64) -> Vec<Effect> {
    t.env.now = at;
    t.go(Input::Tick)
}

fn journal<'a>(fx: &'a [Effect], kind: &str) -> Vec<&'a Value> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Journal(j) if j["type"] == kind => Some(j),
            _ => None,
        })
        .collect()
}

/// A hub with a task `w`, idle, at N.
fn hub() -> T {
    let mut t = T::new();
    t.env.now = N;
    t.spawn_task("w");
    t.hub.force_run("w", Run::Idle);
    t
}

#[test]
fn a_timer_fires_when_due_and_its_agent_is_idle_never_stacked() {
    let mut t = hub();
    let id = every(&mut t, MAIN, "w", Sched::Every(10 * MIN_MS), None, None);
    assert_eq!(id, 1);
    assert!(say_to(&tick(&mut t, N + MIN_MS), "w").is_none(), "not due yet");
    // due while busy: nothing, however long it stays busy
    t.hub.force_run("w", Run::Busy);
    for k in 0..30 {
        let fx = tick(&mut t, N + (10 + k) * MIN_MS);
        assert!(journal(&fx, "every_fired").is_empty() && say_to(&fx, "w").is_none());
    }
    // idle again: one wake, the next one a period later (no catch-up)
    t.hub.force_run("w", Run::Idle);
    let fx = tick(&mut t, N + 40 * MIN_MS);
    let text = say_to(&fx, "w").expect("the wake");
    // due at +10m, busy until +40m: it says how long it waited
    assert!(text.contains("timer #1 (every 10m, waited 30m for w to finish, set by main): check HN"), "{}", text);
    assert_eq!(journal(&fx, "every_fired")[0]["next_ms"], N + 50 * MIN_MS);
    t.hub.force_run("w", Run::Idle);
    assert!(say_to(&tick(&mut t, N + 41 * MIN_MS), "w").is_none());
    assert!(say_to(&tick(&mut t, N + 50 * MIN_MS), "w").is_some());
}

#[test]
fn the_journal_rebuilds_the_timers() {
    let mut t = hub();
    every(&mut t, MAIN, "w", Sched::Every(MIN_MS), None, None);
    every(&mut t, MAIN, MAIN, Sched::Every(5 * MIN_MS), None, Some(2));
    tick(&mut t, N + 5 * MIN_MS);
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::Stop(1)));
    assert_eq!(reply(&fx, tok).unwrap()["text"], "timer #1 stopped");
    let now = N + 6 * MIN_MS;
    let events = t.journal.borrow().clone();
    assert!(events.iter().any(|e| e["type"] == "every_fired"));
    // a new hub (a new sb-core) on the same journal: the same timers
    let mut back = T { hub: Hub::new("/w"), env: FakeEnv::new(), token: 0, journal: Default::default() };
    back.env.now = now;
    assert!(back.hub.replay(&events).is_empty(), "every line read");
    assert_eq!(back.hub.timers().state(now), t.hub.timers().state(now));
    assert_eq!(back.hub.timers().map[&2].fired, 1);
    back.go(Input::Boot);
    back.go(Input::ReplReady { agent: MAIN.into() });
    assert_eq!(every(&mut back, MAIN, MAIN, Sched::Every(MIN_MS), None, None), 3, "ids never reused");
}

#[test]
fn timers_end_with_their_times_their_end_or_their_agent() {
    let mut t = hub();
    t.spawn_task("u");
    t.spawn_task("g");
    t.hub.force_run("u", Run::Idle);
    every(&mut t, MAIN, "w", Sched::Every(MIN_MS), None, Some(2));
    every(&mut t, MAIN, "u", Sched::Every(MIN_MS), Some(N + 3 * MIN_MS), None);
    every(&mut t, MAIN, "g", Sched::Every(MIN_MS), None, None);
    // a dropped agent's timers stop in the drop's own step
    let fx = t.user(MAIN, "/archive g --force");
    let stop = journal(&fx, "every_stop");
    assert_eq!((stop.len(), stop[0]["id"].as_u64(), stop[0]["why"].as_str()), (1, Some(3), Some("@g is gone")));
    assert!(!t.hub.timers().map.contains_key(&3));
    let fx = tick(&mut t, N + MIN_MS);
    assert_eq!(journal(&fx, "every_fired").len(), 2);
    t.hub.force_run("w", Run::Idle);
    t.hub.force_run("u", Run::Idle);
    // its second fire is its last: it ends in the same step
    let fx = tick(&mut t, N + 2 * MIN_MS);
    let stop = journal(&fx, "every_stop");
    assert_eq!((stop[0]["id"].as_u64(), stop[0]["why"].as_str()), (Some(1), Some("it ran its times")));
    assert!(!t.hub.timers().map.contains_key(&1), "two times: done");
    t.hub.force_run("u", Run::Idle);
    let fx = tick(&mut t, N + 3 * MIN_MS);
    assert_eq!(journal(&fx, "every_stop")[0]["why"], "its end time passed");
    assert!(t.hub.timers().map.is_empty(), "past its end");
}

/// amb-tools m_5822: a one-shot day timer fired, its wake never reached
/// main, and `it ran its times` spent it. Now the fire is counted in the
/// step that queues its message, after it (law fire_is_a_message), and a
/// timer never queues a second wake while one waits (wake_never_stacked).
#[test]
fn a_one_shot_timer_is_spent_only_by_a_delivered_wake() {
    let mut t = hub();
    let id = every(&mut t, MAIN, "w", Sched::Daily(7 * 60 + 30), None, Some(1));
    let due = t.hub.timers().map[&id].next_ms;
    t.hub.force_run("w", Run::Busy);
    assert!(journal(&tick(&mut t, due), "every_fired").is_empty(), "busy: not spent, not counted");
    t.hub.force_run("w", Run::Idle);
    let fx = tick(&mut t, due + 1000);
    let at = |kind: &str| fx.iter().position(|e| matches!(e, Effect::Journal(j) if j["type"] == kind)).unwrap();
    assert!(at("message_sent") < at("every_fired") && at("every_fired") < at("every_stop"), "{:?}", fx);
    assert!(say_to(&fx, "w").unwrap().contains("1/1"));
    assert!(t.hub.timers().map.is_empty());
    // an agent with a wake from bise still queued (a busy one gets it as
    // a steer at once; a starting one keeps it queued): no second one
    let id = every(&mut t, MAIN, "w", Sched::Every(MIN_MS), None, None);
    t.hub.force_run("w", Run::Starting);
    let fx = t.go(Input::EveryRun { id });
    assert_eq!(journal(&fx, "every_run").len(), 1);
    let fx = t.go(Input::EveryRun { id });
    assert!(journal(&fx, "every_run").is_empty() && journal(&fx, "message_sent").is_empty(), "never stacked");
}

/// `/scheduled`: a week of ended timers with why they ended, each timer's
/// last runs, a run now outside the count, and the wake's text says how
/// long a busy agent made it wait.
#[test]
fn ended_timers_runs_and_run_now() {
    let mut t = hub();
    let id = every(&mut t, MAIN, "w", Sched::Every(2 * MIN_MS), None, Some(6));
    // due at +2m, its agent busy until +6m: one wake that waited 4m
    t.hub.force_run("w", Run::Busy);
    assert!(say_to(&tick(&mut t, N + 5 * MIN_MS), "w").is_none());
    t.hub.force_run("w", Run::Idle);
    let text = say_to(&tick(&mut t, N + 6 * MIN_MS), "w").unwrap();
    assert!(text.contains("timer #1 (every 2m, 1/6, waited 4m for w to finish, set by main): check HN"), "{text}");
    // a run now: outside the count, the next wake unchanged
    let next = t.hub.timers().map[&id].next_ms;
    t.hub.force_run("w", Run::Idle);
    t.env.now = N + 7 * MIN_MS;
    let text = say_to(&t.go(Input::EveryRun { id }), "w").unwrap();
    assert!(text.contains("timer #1 (every 2m, run now by the user, set by main): check HN"), "{text}");
    let tm = &t.hub.timers().map[&id];
    assert_eq!((tm.fired, tm.next_ms), (1, next));
    assert_eq!(tm.runs, vec![N + 6 * MIN_MS, N + 7 * MIN_MS]);
    assert!(journal(&t.go(Input::EveryRun { id: 99 }), "every_run").is_empty());
    // stopped by the user: ended, with when and why, in the state a week;
    // its agent hears it from bise
    t.env.now = N + 8 * MIN_MS;
    let fx = t.go(Input::EveryStop { id, why: String::new() });
    assert!(journal(&fx, "message_sent").iter().any(|m| m["msg"]["text"].as_str().unwrap().starts_with("the user stopped timer #1")));
    assert!(t.hub.timers().map.is_empty());
    let st = t.hub.timers().state(N + 9 * MIN_MS);
    assert_eq!(st.len(), 1);
    assert_eq!((st[0]["end"].as_str(), st[0]["stopped_by"].as_str()), (Some("stopped"), Some("user")));
    assert_eq!(st[0]["ended_ms"], N + 8 * MIN_MS);
    assert_eq!(st[0]["runs"].as_array().unwrap().len(), 2);
    assert!(t.hub.timers().state(N + 8 * MIN_MS + ENDED_SHOWN_MS).is_empty(), "a week later: gone from the list");
}

#[test]
fn daily_timers_land_on_the_local_clock() {
    let mut t = hub();
    let id = every(&mut t, MAIN, "w", Sched::Daily(7 * 60 + 30), None, None);
    let due = t.hub.timers().map[&id].next_ms;
    assert_eq!(due, next_daily(N, 450));
    assert!(due > N && due <= N + 25 * 60 * MIN_MS);
    let fx = tick(&mut t, due);
    assert_eq!(journal(&fx, "every_fired")[0]["next_ms"], next_daily(due, 450), "at 07:30 sharp: tomorrow's");
    assert!(t.hub.timers().map[&id].next_ms > due);
}

/// Two timers due in one tick: one need, answered by id, two wakes.
#[test]
fn two_timers_due_in_one_tick() {
    let mut t = hub();
    every(&mut t, MAIN, "w", Sched::Every(3 * MIN_MS), None, None);
    every(&mut t, MAIN, MAIN, Sched::Every(2 * MIN_MS), None, None);
    let fx = tick(&mut t, N + 3 * MIN_MS);
    assert!(say_to(&fx, "w").unwrap().contains("timer #1 (every 3m"));
    assert!(say_to(&fx, MAIN).unwrap().contains("timer #2 (every 2m, waited 1m for main to finish"));
    let ids: Vec<u64> = journal(&fx, "every_fired").iter().filter_map(|j| j["id"].as_u64()).collect();
    assert_eq!(ids, vec![1, 2]);
}

/// idle-cpu (51051c52): a tick with timers that are not due changes
/// nothing (no journal line, no state for the views, no wake).
#[test]
fn a_tick_with_no_timer_due_does_nothing() {
    let mut t = hub();
    for k in 1..=3 {
        every(&mut t, MAIN, "w", Sched::Every(k * 10 * MIN_MS), None, None);
    }
    let fx = tick(&mut t, N + MIN_MS);
    let busy = fx.iter().filter(|e| matches!(e, Effect::Journal(_) | Effect::State | Effect::Say { .. } | Effect::Steer { .. }));
    assert_eq!(busy.count(), 0, "{:?}", fx);
}

/// A journal today's hub wrote (main 298407ff, tests/every_e2e.py:
/// every_set, every_fired, every_stop, two daily timers) replays to the
/// same /scheduled list as that hub's.
#[test]
fn a_journal_of_298407ff_replays_to_the_same_scheduled_list() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/");
    let events: Vec<Value> = std::fs::read_to_string(format!("{dir}every-journal-298407ff.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let want: Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}every-state-298407ff.json")).unwrap()).unwrap();
    let mut hub = Hub::new("/w");
    assert!(hub.replay(&events).is_empty(), "every line read");
    let now = 1_791_131_900_000;
    assert_eq!(Value::Array(hub.timers().state(now)), want);
    let list = hub.timers().list(now);
    // sched-names: an old journal has no name: the list says its plain fallback
    assert!(list.starts_with("#2 @main  make the morning page · every day 07:30 · next ") && list.ends_with(" · by main"), "{list}");
}

/// A refused wake (fire_is_a_message; unreachable from the tick today:
/// timer_may wants an active, idle agent) writes today's retry line,
/// every_fired undelivered with its count unchanged and the next wake a
/// minute later: two of them replay to no fire counted, no run, the next
/// wake moved twice; an old hub's undelivered line (count - 1, after the
/// fired line it took back) still takes back its run.
#[test]
fn refused_wakes_are_retried_a_minute_apart_and_never_counted() {
    let mut t = hub();
    let id = every(&mut t, MAIN, "w", Sched::Every(10 * MIN_MS), None, Some(1));
    let mut events = t.journal.borrow().clone();
    let due = N + 10 * MIN_MS;
    for k in 0..2 {
        events.push(json!({"type": "every_fired", "id": id, "fired": 0, "next_ms": due + (k + 1) * MIN_MS, "at": 0, "undelivered": true}));
    }
    let mut back = Hub::new("/w");
    assert!(back.replay(&events).is_empty(), "every line read");
    let tm = &back.timers().map[&id];
    assert_eq!((tm.fired, tm.next_ms, tm.last_ms, tm.runs.len()), (0, due + 2 * MIN_MS, 0, 0));
    // an old hub's: fired 1 at `due`, then undelivered (fired 0): the run goes
    events.push(json!({"type": "every_fired", "id": id, "fired": 1, "next_ms": due + 20 * MIN_MS, "at": due + 2 * MIN_MS}));
    events.push(json!({"type": "every_fired", "id": id, "fired": 0, "next_ms": due + 3 * MIN_MS, "at": due + 2 * MIN_MS, "undelivered": true}));
    let mut back = Hub::new("/w");
    assert!(back.replay(&events).is_empty());
    let tm = &back.timers().map[&id];
    assert_eq!((tm.fired, tm.next_ms, tm.runs.len()), (0, due + 3 * MIN_MS, 0));
}

fn asked_name(fx: &[Effect]) -> Option<(u64, String)> {
    fx.iter().find_map(|e| match e {
        Effect::AskTimerName { id, request } => Some((*id, request.clone())),
        _ => None,
    })
}

fn scheduled_lines(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Line { line, .. } if line.starts_with("sb scheduled : ") => Some(line.clone()),
            _ => None,
        })
        .collect()
}

/// sched-names (main m_14458, architect m_14532): a timer set without
/// --name asks the small model once, off the loop; its answer (or, with
/// no model, the plain fallback) is sb-core's every_name, in the view,
/// the list, the wake and the ◷ set line, which waits for it. --name asks
/// nothing; one call at a time; the journal replays the name.
#[test]
fn a_timer_gets_a_name_from_the_model_or_the_fallback() {
    let mut t = hub();
    let req = |text: &str, name: Option<&str>| {
        AgentReq::Every(EveryReq::Add { to: "w".into(), text: text.into(), sched: Sched::Every(10 * MIN_MS), until_ms: None, times: None, page: None, name: name.map(String::from) })
    };
    // no --name: one call, and the set line waits for its name
    let (_, fx) = t.req(MAIN, req("amb-core: read $TMPDIR/s4.log (S29 bench) and report", None));
    let (id, request) = asked_name(&fx).expect("a name call");
    assert!(request.contains(crate::every_name::MARK), "{request}");
    assert!(scheduled_lines(&fx).is_empty(), "the set line waits: {:?}", fx);
    // a second unnamed timer: no call while the first is in flight
    let (_, fx) = t.req(MAIN, req("check the nightly build", None));
    assert!(asked_name(&fx).is_none(), "one at a time");
    // the model answers: every_name, the set line with the name, the next call
    let fx = t.go(Input::TimerName { id, reply: Some("\"S29 Bench.\"".into()) });
    let named = journal(&fx, "every_name");
    assert_eq!((named[0]["id"].as_u64(), named[0]["name"].as_str()), (Some(id), Some("s29 bench")));
    // sb-core says the set line goes now (architect m_15603)
    assert_eq!(named[0]["announce"], true, "{}", named[0]);
    let set = scheduled_lines(&fx);
    // its agent's line, and main's copy (main set it for w)
    assert!(set.len() == 2 && set.iter().all(|l| l.contains("\"name\":\"s29 bench\"")), "{set:?}");
    assert_eq!(t.hub.timers().map[&id].name, "s29 bench");
    let (id2, _) = asked_name(&fx).expect("the next unnamed timer");
    // no model (or it failed): the plain fallback names it
    t.go(Input::TimerName { id: id2, reply: None });
    assert_eq!(t.hub.timers().map[&id2].name, "check the nightly build");
    // --name: no call, the set line at once
    let (tok, fx) = t.req(MAIN, req("x y z", Some("my own name")));
    assert!(asked_name(&fx).is_none());
    assert!(scheduled_lines(&fx).iter().any(|l| l.contains("my own name")));
    let id3 = reply(&fx, tok).unwrap()["id"].as_u64().unwrap();
    // the list shows names, never the words; --show prints them
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::List));
    let list = reply(&fx, tok).unwrap()["text"].as_str().unwrap().to_string();
    assert!(list.contains(&format!("#{id} @w  s29 bench · every 10m · next ")) && !list.contains("$TMPDIR"), "{list}");
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::Show(id3)));
    assert!(reply(&fx, tok).unwrap()["text"].as_str().unwrap().ends_with("\nx y z"));
    // the wake carries the name
    let fx = tick(&mut t, N + 10 * MIN_MS);
    let wake = say_to(&fx, "w").expect("a wake");
    assert!(wake.contains(&format!("\ntimer #{id} \"s29 bench\" (every 10m")), "{wake}");
    // the journal replays the names (every_set's and every_name's)
    let events = t.journal.borrow().clone();
    let mut back = Hub::new("/w");
    assert!(back.replay(&events).is_empty(), "every line read");
    let names: Vec<String> = back.timers().map.values().map(|x| x.name.clone()).collect();
    assert_eq!(names, ["s29 bench", "check the nightly build", "my own name"]);
}

/// architect m_15603: a hub restarted between a timer's set and its name
/// still writes its ◷ set line, once: the every_set is `held` in the
/// journal (sb-core's state, not the Asker's), the new hub asks at its
/// first step and its every_name `announce`s; a second name does not.
#[test]
fn a_restart_before_the_name_still_announces_the_timer_once() {
    let mut t = hub();
    let req = EveryReq::Add { to: "w".into(), text: "check the nightly build".into(), sched: Sched::Every(10 * MIN_MS), until_ms: None, times: None, page: None, name: None };
    let (_, fx) = t.req(MAIN, AgentReq::Every(req));
    assert!(asked_name(&fx).is_some() && scheduled_lines(&fx).is_empty());
    let set = journal(&fx, "every_set");
    assert_eq!(set[0]["held"], true, "{}", set[0]);
    // the hub restarts before the answer: a new hub from the journal
    let events = t.journal.borrow().clone();
    let mut back = Hub::new("/w");
    assert!(back.replay(&events).is_empty(), "every line read");
    std::mem::swap(&mut t.hub, &mut back);
    let fx = t.go(Input::Tick);
    let (id, _) = asked_name(&fx).expect("the new hub asks at its first step");
    let fx = t.go(Input::TimerName { id, reply: Some("nightly build check".into()) });
    let named = journal(&fx, "every_name");
    assert_eq!((named[0]["name"].as_str(), &named[0]["announce"]), (Some("nightly build check"), &json!(true)));
    let set = scheduled_lines(&fx);
    // its agent's line and main's copy: one each
    assert!(set.len() == 2 && set.iter().all(|l| l.contains("nightly build check")), "{set:?}");
    // a later name (an old hub asking again) announces nothing
    let fx = t.go(Input::TimerName { id, reply: Some("other name".into()) });
    assert!(scheduled_lines(&fx).is_empty(), "once");
    assert!(journal(&fx, "every_name").iter().all(|j| j["announce"] != true));
}

/// A hub that starts with unnamed timers (an older hub's) names them, one
/// call at a time, each once; an every_name for an ended or unknown timer
/// changes nothing.
#[test]
fn old_unnamed_timers_are_named_one_at_a_time() {
    let mut t = hub();
    let mut events = Vec::new();
    for id in 1..=2u64 {
        events.push(json!({"type": "every_set", "id": id, "agent": "w", "by": "main", "text": format!("old timer {id}"),
                           "next_ms": N + 10 * MIN_MS, "at": N, "every_ms": 10 * MIN_MS}));
    }
    let mut back = Hub::new("/w");
    assert!(back.replay(&events).is_empty());
    assert!(back.timers().map.values().all(|x| x.name.is_empty()), "an old line has no name");
    // the next step of this hub asks for the first, then the second
    std::mem::swap(&mut t.hub, &mut back);
    let fx = t.go(Input::Tick);
    let (a, _) = asked_name(&fx).expect("asks at its first step");
    let fx = t.go(Input::TimerName { id: a, reply: Some("first".into()) });
    assert!(scheduled_lines(&fx).is_empty(), "an old timer gets no set line");
    let (b, _) = asked_name(&fx).expect("then the next");
    assert_ne!(a, b);
    let fx = t.go(Input::TimerName { id: b, reply: Some("second".into()) });
    assert!(asked_name(&fx).is_none(), "each once");
    // unknown id: nothing
    let fx = t.go(Input::TimerName { id: 99, reply: Some("ghost".into()) });
    assert!(journal(&fx, "every_name").is_empty());
}
