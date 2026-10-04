//! `sb every`'s timers on sb-core (bend/hub/timers.bend, core.bend's
//! timers section): the tests every.rs had when the timers were Rust's,
//! now through the hub and a real sb-core, plus the replay of a journal
//! today's hub wrote (tests/fixtures/every-journal-298407ff.jsonl).

use super::*;
use crate::every::{next_daily, Sched, ENDED_SHOWN_MS, MIN_MS};

const N: u64 = 1_790_000_000_000;

/// `sb every` from `from`: the new timer's id.
fn every(t: &mut T, from: &str, to: &str, sched: Sched, until_ms: Option<u64>, times: Option<u64>) -> u64 {
    let req = EveryReq::Add { to: to.into(), text: "check HN".into(), sched, until_ms, times, page: None };
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
    assert!(list.starts_with("#2 @main every day 07:30 · next ") && list.ends_with("\"make the morning page\" (by main)"), "{list}");
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
