//! The sender against a fake collector (plan, les tests: transport): a
//! clock passed in, a throwaway home, a transport in memory. No test here
//! touches the network.

use std::collections::VecDeque;

use bise_home::Home;

use crate::day::Day;
use crate::install_id::InstallId;
use crate::key::{CountKey, HistKey, Milestone, MinutesKey};
use crate::law::{check_delete_body, check_packets_body};
use crate::packet::{Arch, Build, Channel, Packet, Parts, Surface};
use crate::send::{endpoint, Endpoint, Transport};
use crate::sender::{fitting, forget, request_delete, tick, Ctx, Report, Tick, MAX_BODY, SETTLE_SECS};
use crate::sharing::{test_permit, Off};
use crate::spool::{write_source, Source};
use crate::state::LocalState;
use crate::tally::Tally;

/// What a collector in memory got, and what it answers next (204 when
/// nothing is queued).
#[derive(Default)]
struct Fake {
    got: Vec<(String, Vec<u8>)>,
    answers: VecDeque<Result<u16, String>>,
}

impl Transport for Fake {
    fn post_json(&mut self, url: &str, body: &[u8]) -> Result<u16, String> {
        self.got.push((url.to_string(), body.to_vec()));
        self.answers.pop_front().unwrap_or(Ok(204))
    }
}

fn home(name: &str) -> Home {
    let root = std::env::temp_dir().join(format!("bise-usage-sender-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Home::at(root)
}

fn day(s: &str) -> Day {
    Day::parse(s).unwrap()
}

fn build() -> Build {
    Build { version: "2026.11.8-1".into(), channel: Channel::Stable, os_major: 26, arch: Arch::Arm64 }
}

fn to() -> Endpoint {
    endpoint(Some("http://collector.test".into()))
}

/// One tick at `now` with `gate`, through `fake`.
fn run(h: &Home, now: u64, gate: Result<(), Off>, fake: &mut Fake) -> Tick {
    let b = build();
    let e = to();
    // a draw of its own per tick time
    let mut random = || [(now % 251) as u8; 16];
    let mut ctx = Ctx { build: &b, endpoint: &e, transport: fake, random: &mut random, jitter: 0.0 };
    tick(h, now, gate.map(|()| test_permit()), &mut ctx)
}

fn report(t: Tick) -> Report {
    match t {
        Tick::Ran(r) => r,
        Tick::Busy => panic!("busy"),
    }
}

/// A hub counted on `d`: his message, so the day is active.
fn spool(h: &Home, d: Day, tally: &Tally) {
    write_source(&test_permit(), &h.usage_day_dir(&d.to_string()), &Source::hub("taxes-2026-3f9a2b1c", 7).unwrap(), tally).unwrap();
}

fn busy_hub() -> Tally {
    let mut t = Tally::default();
    t.count(CountKey::UserMessage);
    t.add(CountKey::AgentCreatedByUser, 2);
    t.time(MinutesKey::AgentWork, 3600);
    t
}

/// Every key at its cap: the biggest packet a day can make.
fn busiest() -> Tally {
    let mut t = Tally::default();
    for k in CountKey::ALL {
        t.add(*k, 1_000_000);
    }
    for k in MinutesKey::ALL {
        t.time(*k, 10_000_000);
    }
    for k in HistKey::ALL {
        for v in k.bounds() {
            for _ in 0..1000 {
                t.observe(*k, *v);
            }
        }
        t.observe(*k, u32::MAX);
    }
    t
}

/// 00:20 UTC on `d`: the day before is settled.
fn after_midnight(d: Day) -> u64 {
    d.start_secs() + SETTLE_SECS + 300
}

fn packets_in(body: &[u8]) -> Vec<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap();
    v["packets"].as_array().unwrap().clone()
}

#[test]
fn a_closed_day_is_sent_once_settled_and_today_never() {
    let h = home("closed");
    let (d4, d5) = (day("2026-11-04"), day("2026-11-05"));
    spool(&h, d4, &busy_hub());
    spool(&h, d5, &busy_hub());
    let mut fake = Fake::default();
    // 00:05 on the 5th: the 4th is not settled yet
    let r = report(run(&h, d5.start_secs() + 300, Ok(()), &mut fake));
    assert!(fake.got.is_empty() && r.sent.is_empty(), "{r:?}");
    let r = report(run(&h, after_midnight(d5), Ok(()), &mut fake));
    assert_eq!(r.sent, [d4]);
    assert_eq!(fake.got.len(), 1);
    let (url, body) = &fake.got[0];
    assert_eq!(url, "http://collector.test/v1/packets");
    assert_eq!(check_packets_body(body, &["taxes-2026", "3f9a2b1c"]), Ok(()), "{}", String::from_utf8_lossy(body));
    let p = &packets_in(body)[0];
    assert_eq!((p["day"].as_str(), p["active_days_7"].as_u64(), p["counts"]["agent_created_by_user"].as_u64()), (Some("2026-11-04"), Some(1), Some(2)));
    // the packet moved to sent/, the spool day is gone, today's stays
    assert!(!h.usage_day_dir("2026-11-04").exists() && h.usage_day_dir("2026-11-05").exists());
    assert_eq!(crate::sent::list(&h.usage_sent_dir()).iter().map(|(on, d, _)| (*on, *d)).collect::<Vec<_>>(), [(d5, d4)]);
    let state: LocalState = serde_json::from_str(&std::fs::read_to_string(h.usage_state()).unwrap()).unwrap();
    assert!(state.ever_sent && state.is_active(d4));
    // the next tick has nothing to send
    run(&h, after_midnight(d5) + 3600, Ok(()), &mut fake);
    assert_eq!(fake.got.len(), 1);
}

#[test]
fn a_late_write_of_a_sent_day_is_not_sent_again() {
    let h = home("late");
    let (d4, d5) = (day("2026-11-04"), day("2026-11-05"));
    spool(&h, d4, &busy_hub());
    let mut fake = Fake::default();
    run(&h, after_midnight(d5), Ok(()), &mut fake);
    spool(&h, d4, &busy_hub());
    let r = report(run(&h, after_midnight(d5) + 3600, Ok(()), &mut fake));
    assert!(r.sent.is_empty() && fake.got.len() == 1, "{r:?}");
    assert!(!h.usage_day_dir("2026-11-04").exists());
}

#[test]
fn offline_keeps_the_days_and_waits_the_backoff() {
    let h = home("offline");
    let d5 = day("2026-11-05");
    spool(&h, day("2026-11-04"), &busy_hub());
    let mut fake = Fake { answers: VecDeque::from([Err("Could not resolve host".into()), Ok(503)]), ..Fake::default() };
    let t0 = after_midnight(d5);
    let r = report(run(&h, t0, Ok(()), &mut fake));
    assert_eq!(r.retry, Some(("Could not resolve host".into(), t0 + 60)));
    assert!(h.usage_day_dir("2026-11-04").exists() && h.usage_retry().exists());
    // before the wait is over: nothing posted
    let r = report(run(&h, t0 + 59, Ok(()), &mut fake));
    assert_eq!((fake.got.len(), r.waiting_until), (1, Some(t0 + 60)));
    // a 503 doubles the wait
    let r = report(run(&h, t0 + 60, Ok(()), &mut fake));
    assert_eq!(r.retry, Some(("status 503".into(), t0 + 60 + 120)));
    let r = report(run(&h, t0 + 180, Ok(()), &mut fake));
    assert_eq!(r.sent, [day("2026-11-04")]);
    assert!(!h.usage_retry().exists(), "a success clears the backoff");
    // the same packet each time: the server dedups on (install_id, day)
    assert_eq!(packets_in(&fake.got[0].1), packets_in(&fake.got[2].1));
}

#[test]
fn a_4xx_drops_the_days_and_says_so() {
    let h = home("refused");
    spool(&h, day("2026-11-04"), &busy_hub());
    let mut fake = Fake { answers: VecDeque::from([Ok(422)]), ..Fake::default() };
    let r = report(run(&h, after_midnight(day("2026-11-05")), Ok(()), &mut fake));
    assert_eq!(r.refused, [(day("2026-11-04"), 422)]);
    assert!(!h.usage_day_dir("2026-11-04").exists() && !h.usage_retry().exists());
    assert_eq!(r.log_line().as_deref(), Some("usage: refused (dropped) 2026-11-04:422"));
}

#[test]
fn the_spool_keeps_thirty_days_and_sends_the_oldest_first() {
    let h = home("thirty");
    let today = day("2026-11-05");
    for i in 0..40 {
        spool(&h, Day(today.0 - i), &busy_hub());
    }
    let mut fake = Fake::default();
    let r = report(run(&h, after_midnight(today), Ok(()), &mut fake));
    // 30 days kept, today included: the 29 closed ones are sent
    assert_eq!(r.sent.len(), 29);
    assert_eq!(r.sent.first(), Some(&Day(today.0 - 29)));
    assert_eq!(crate::spool::days_in(&h.usage_dir()), [today]);
    let sent: Vec<Day> = fake.got.iter().flat_map(|(_, b)| packets_in(b)).map(|p| day(p["day"].as_str().unwrap())).collect();
    assert_eq!(sent, r.sent, "oldest first");
}

#[test]
fn a_full_batch_of_the_busiest_days_is_one_body_under_the_cap() {
    let h = home("full");
    let today = day("2026-11-05");
    for i in 1..=29 {
        spool(&h, Day(today.0 - i), &busiest());
    }
    let mut fake = Fake::default();
    let r = report(run(&h, after_midnight(today), Ok(()), &mut fake));
    assert_eq!((r.sent.len(), fake.got.len()), (29, 1));
    let body = &fake.got[0].1;
    assert!(body.len() <= MAX_BODY, "{} bytes", body.len());
    assert_eq!(check_packets_body(body, &[]), Ok(()));
}

/// The split, on sizes: as many packets as fit, the body's own bytes
/// counted exactly as the encoder writes them.
#[test]
fn a_batch_splits_where_the_body_would_pass_the_cap() {
    let p = |d: &str| {
        let id = InstallId::draw([1; 16], day(d));
        let state = LocalState::new(0);
        Packet::build(Parts { day: day(d), install_id: &id, view: state.view(day(d)), build: &build(), surface: Surface::Both, tally: &busy_hub(), milestones: Default::default() })
    };
    let batch = [p("2026-11-03"), p("2026-11-04"), p("2026-11-05")];
    let sizes: Vec<usize> = batch.iter().map(|p| serde_json::to_vec(p).unwrap().len()).collect();
    let body = crate::encode::packets(&batch).len();
    assert_eq!(fitting(&sizes, body), 3, "exactly the encoder's length");
    assert_eq!(fitting(&sizes, body - 1), 2);
    assert_eq!(fitting(&sizes, sizes[0] + 13), 0, "one alone too big");
}

/// The measure behind "no gzip" (architect m_55984): a day with every key
/// at its cap is a few KB, so a body of 64 KB raw holds many days.
#[test]
fn the_busiest_day_is_a_few_kilobytes() {
    let id = InstallId::draw([1; 16], day("2026-11-05"));
    let mut state = LocalState::new(0);
    for m in Milestone::ALL {
        state.reach(*m, 10);
    }
    let p = Packet::build(Parts {
        day: day("2026-11-05"),
        install_id: &id,
        view: state.view(day("2026-11-05")),
        build: &build(),
        surface: Surface::Both,
        tally: &busiest(),
        milestones: state.unsent(),
    });
    let len = serde_json::to_vec(&p).unwrap().len();
    eprintln!("the busiest packet: {len} bytes");
    assert!(len < MAX_BODY / 8, "a day of {len} bytes: time for gzip (flate2)");
}

#[test]
fn milestones_go_once_on_the_oldest_packet() {
    let h = home("milestones");
    let today = day("2026-11-05");
    let mut state = LocalState::new(day("2026-11-01").start_secs());
    state.reach(Milestone::FirstAgent, state.first_launch_secs + 60);
    std::fs::create_dir_all(h.usage_dir()).unwrap();
    std::fs::write(h.usage_state(), serde_json::to_vec(&state).unwrap()).unwrap();
    spool(&h, day("2026-11-03"), &busy_hub());
    spool(&h, day("2026-11-04"), &busy_hub());
    let mut fake = Fake::default();
    run(&h, after_midnight(today), Ok(()), &mut fake);
    let ps = packets_in(&fake.got[0].1);
    assert_eq!(ps[0]["milestones"]["first_agent"].as_str(), Some("under_1h"));
    assert_eq!(ps[1]["milestones"], serde_json::json!({}));
    spool(&h, today, &busy_hub());
    run(&h, after_midnight(today.plus(1)), Ok(()), &mut fake);
    assert_eq!(packets_in(&fake.got[1].1)[0]["milestones"], serde_json::json!({}), "sent once");
}

#[test]
fn one_sender_with_two_flocks() {
    let h = home("flock");
    spool(&h, day("2026-11-04"), &busy_hub());
    let other = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(h.usage_send_lock()).unwrap();
    other.try_lock().unwrap();
    let mut fake = Fake::default();
    assert_eq!(run(&h, after_midnight(day("2026-11-05")), Ok(()), &mut fake), Tick::Busy);
    assert!(fake.got.is_empty());
    drop(other);
    assert_eq!(report(run(&h, after_midnight(day("2026-11-05")), Ok(()), &mut fake)).sent.len(), 1);
}

#[test]
fn off_by_his_choice_wipes_and_off_otherwise_touches_nothing() {
    for why in [Off::NotYet, Off::Ci, Off::TestHome, Off::Offscreen, Off::DevBuild, Off::NoteNotSeen] {
        let h = home("off-other");
        spool(&h, day("2026-11-04"), &busy_hub());
        let mut fake = Fake::default();
        let r = report(run(&h, after_midnight(day("2026-11-05")), Err(why), &mut fake));
        assert!(!r.wiped && fake.got.is_empty() && h.usage_day_dir("2026-11-04").exists(), "{why:?}");
    }
    for why in [Off::Pref, Off::Env, Off::DoNotTrack] {
        let h = home("off-his");
        spool(&h, day("2026-11-03"), &busy_hub());
        let mut fake = Fake::default();
        run(&h, after_midnight(day("2026-11-04")), Ok(()), &mut fake);
        spool(&h, day("2026-11-04"), &busy_hub());
        let r = report(run(&h, after_midnight(day("2026-11-05")), Err(why), &mut fake));
        assert!(r.wiped && fake.got.len() == 1, "{why:?}");
        let left: Vec<String> = std::fs::read_dir(h.usage_dir()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["send.lock"], "{why:?}");
    }
}

#[test]
fn off_with_nothing_written_writes_nothing() {
    let h = home("virgin");
    let mut fake = Fake::default();
    let r = report(run(&h, after_midnight(day("2026-11-05")), Err(Off::NotYet), &mut fake));
    assert_eq!(r.off, Some(Off::NotYet));
    assert!(!h.usage_dir().exists());
    // the real thread's tick, in this test home: the gate stops it first
    assert_eq!(crate::hourly::tick_now(&h, &build(), false), None);
    assert!(!h.usage_dir().exists());
}

#[test]
fn forget_now_wipes_without_a_tick() {
    let h = home("forget");
    spool(&h, day("2026-11-04"), &busy_hub());
    forget(&h).unwrap();
    assert!(crate::spool::days_in(&h.usage_dir()).is_empty());
}

fn id_on_disk(h: &Home) -> Option<InstallId> {
    std::fs::read_to_string(h.usage_install_id()).ok().and_then(|t| InstallId::from_file(&t))
}

#[test]
fn a_delete_before_anything_was_sent_posts_nothing() {
    let h = home("delete-first");
    let mut fake = Fake { answers: VecDeque::from([Err("offline".into())]), ..Fake::default() };
    spool(&h, day("2026-11-04"), &busy_hub());
    // the id is drawn for the first send, which fails: never sent
    run(&h, after_midnight(day("2026-11-05")), Ok(()), &mut fake);
    assert!(id_on_disk(&h).is_some());
    assert!(!request_delete(&h).unwrap());
    assert!(id_on_disk(&h).is_none() && !h.usage_delete_request().exists());
    run(&h, after_midnight(day("2026-11-06")), Err(Off::Pref), &mut fake);
    assert_eq!(fake.got.len(), 1, "only the failed packets");
}

#[test]
fn a_delete_is_kept_until_the_server_takes_it_even_with_sharing_off() {
    let h = home("delete");
    spool(&h, day("2026-11-04"), &busy_hub());
    let mut fake = Fake::default();
    let t0 = after_midnight(day("2026-11-05"));
    run(&h, t0, Ok(()), &mut fake);
    let id = id_on_disk(&h).unwrap();
    assert!(request_delete(&h).unwrap());
    assert!(id_on_disk(&h).is_none(), "no longer used");
    // he turned sharing off too: the wipe keeps the request
    fake.answers = VecDeque::from([Ok(500)]);
    let r = report(run(&h, t0 + 3600, Err(Off::Pref), &mut fake));
    assert!(r.wiped && r.deleted == 0 && h.usage_delete_request().exists() && h.usage_retry().exists(), "{r:?}");
    let (url, body) = fake.got.last().unwrap();
    assert_eq!(url, "http://collector.test/v1/delete");
    assert_eq!(check_delete_body(body, &[]), Ok(()));
    assert!(String::from_utf8_lossy(body).contains(id.reveal()));
    assert!(!r.log_line().unwrap_or_default().contains(id.reveal()));
    // the next due tick posts it again; a 2xx deletes it
    let r = report(run(&h, t0 + 3600 + 60, Err(Off::Pref), &mut fake));
    assert_eq!(r.deleted, 1);
    assert!(!h.usage_delete_request().exists() && !h.usage_retry().exists());
    assert_eq!(fake.got.len(), 3);
}

#[test]
fn a_pending_delete_goes_before_the_packets() {
    let h = home("delete-order");
    spool(&h, day("2026-11-03"), &busy_hub());
    let mut fake = Fake::default();
    run(&h, after_midnight(day("2026-11-04")), Ok(()), &mut fake);
    request_delete(&h).unwrap();
    spool(&h, day("2026-11-04"), &busy_hub());
    let r = report(run(&h, after_midnight(day("2026-11-05")), Ok(()), &mut fake));
    let urls: Vec<&str> = fake.got.iter().map(|(u, _)| u.as_str()).collect();
    assert_eq!(urls, ["http://collector.test/v1/packets", "http://collector.test/v1/delete", "http://collector.test/v1/packets"]);
    assert_eq!((r.deleted, r.sent.len()), (1, 1));
    // a new id for the packets after a delete
    let ids: Vec<String> = [&fake.got[0].1, &fake.got[2].1].iter().map(|b| packets_in(b)[0]["install_id"].as_str().unwrap().to_string()).collect();
    assert_ne!(ids[0], ids[1], "a new id after a delete");
}
