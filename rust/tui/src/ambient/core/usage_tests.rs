use super::*;
use std::sync::{Arc, Mutex};

use bise_usage::install_id::InstallId;
use bise_usage::law::{check_packets_body, wildcard_arms};
use bise_usage::packet::{Arch, Build, Channel, Packet, Parts, Surface};
use bise_usage::state::LocalState;

/// What a sink got: each write's day and tally.
type Written = Arc<Mutex<Vec<(Day, Tally)>>>;

fn recording() -> (UsageSink, Written) {
    let got: Written = Arc::default();
    let g = got.clone();
    let sink: UsageSink = Box::new(move |day, t| {
        g.lock().unwrap().push((day, t.clone()));
        Ok(())
    });
    (sink, got)
}

const NOV_5: u64 = 1_793_836_800; // 2026-11-05 00:00 UTC

/// A line from the app, as the core takes it.
fn feed(u: &mut Usage, line: &str) {
    let (c, counted) = Cmd::parse_counted(line).unwrap();
    u.took(&c, counted);
}

#[test]
fn every_mapping_names_each_case() {
    let src = include_str!("usage.rs");
    for f in ["of_cmd", "of_gesture", "of_check"] {
        assert_eq!(wildcard_arms(src, f), Ok(vec![]), "{f} has a `_ =>`: say counted or not counted for each case");
    }
}

#[test]
fn commands_count_by_name_and_usage_time_by_its_numbers() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut u = Usage::new(sink, NOV_5 + 3600, t0);
    for l in [
        r#"{"cmd":"index"}"#,
        r#"{"cmd":"plugin_set","name":"docs","on":true}"#,
        r#"{"cmd":"plugin_set","name":"docs","on":false}"#,
        r#"{"cmd":"dictate_start","id":"w1"}"#,
        r#"{"cmd":"send","text":"hello"}"#,
        r#"{"cmd":"usage_time","open_s":60,"focused_s":40,"engaged_s":20,"gestures":["inbox_opened","artifact_sheet","crash_core"]}"#,
        r#"{"cmd":"usage_time","open_s":60,"focused_s":0,"engaged_s":0,"gestures":["a_gesture_of_a_newer_app"]}"#,
    ] {
        feed(&mut u, l);
    }
    u.tick(t0 + WRITE_EVERY, NOV_5 + 3600 + 300);
    let got = got.lock().unwrap();
    assert_eq!(got.len(), 1, "one write after 5 minutes");
    let (day, t) = &got[0];
    assert_eq!(*day, Day::from_unix_secs(NOV_5));
    let counts: Vec<(&str, u64)> = t.counts.iter().map(|(k, n)| (k.name(), *n)).collect();
    assert_eq!(counts, [
        ("inbox_opened", 1),
        ("artifact_opened_sheet", 1),
        ("plugin_enabled", 1),
        ("dictation_started", 1),
        ("search", 1),
        ("crash_core", 1)
    ]);
    assert_eq!(t.seconds[&MinutesKey::AppOpen], 120);
    assert_eq!(t.seconds[&MinutesKey::AppFocused], 40);
    assert_eq!(t.seconds[&MinutesKey::AppEngaged], 20);
}

#[test]
fn a_new_utc_day_writes_the_old_one_and_starts_at_zero() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut u = Usage::new(sink, NOV_5 + 86_000, t0);
    feed(&mut u, r#"{"cmd":"index"}"#);
    // nothing new: a tick writes nothing
    u.tick(t0 + Duration::from_secs(1), NOV_5 + 86_001);
    assert!(got.lock().unwrap().is_empty(), "written only every 5 minutes");
    u.tick(t0 + Duration::from_secs(500), NOV_5 + 86_500);
    feed(&mut u, r#"{"cmd":"index"}"#);
    u.write(t0 + Duration::from_secs(600));
    u.write(t0 + Duration::from_secs(601));
    let got = got.lock().unwrap();
    let days: Vec<(Day, u64)> = got.iter().map(|(d, t)| (*d, t.counts[&CountKey::Search])).collect();
    assert_eq!(days, [(Day::from_unix_secs(NOV_5), 1), (Day::from_unix_secs(NOV_5 + 86_400), 1)], "a clean exit writes once, nothing twice");
}

#[test]
fn a_provider_check_counts_by_its_class() {
    assert_eq!(of_check(Ok(())), CountKey::SetupDone);
    assert_eq!(of_check(Err(CheckWhy::WrongKey)), CountKey::SetupFailedAuth);
    assert_eq!(of_check(Err(CheckWhy::Unreachable)), CountKey::SetupFailedNetwork);
    assert_eq!(of_check(Err(CheckWhy::Config)), CountKey::SetupFailedOther);
}

/// What looks like his data, in every command's arguments.
const FIXTURE: &[&str] = &[
    "camille",
    "/Users/camille/projects/taxes-2026",
    "camille.martin@example.com",
    "sk-fake-7f3a9c2e1b",
    "taxes-2026",
    "refactor the invoice parser",
    "https://acme.example.com/brief",
    "invoice-agent",
];

/// The PII law on the core's path (plan, les tests): commands full of his
/// data through the core's counting, then the packet and its body.
#[test]
fn law_pii_the_core_counts_nothing_of_his_data() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut u = Usage::new(sink, NOV_5, t0);
    let lines = [
        r#"{"cmd":"project_add","path":"/Users/camille/projects/taxes-2026"}"#,
        r#"{"cmd":"provider_use","id":"mistral","key":"sk-fake-7f3a9c2e1b"}"#,
        r#"{"cmd":"plugin_set","project":"taxes-2026","name":"camille.martin@example.com","on":true}"#,
        r#"{"cmd":"send","text":"refactor the invoice parser"}"#,
        r#"{"cmd":"agent_send","agent":"invoice-agent","text":"https://acme.example.com/brief"}"#,
        r#"{"cmd":"files","project":"taxes-2026","q":"camille","rid":1}"#,
        r#"{"cmd":"talk_start","page":{"id":"camille","url":"https://acme.example.com/brief"}}"#,
        r#"{"cmd":"index"}"#,
        r#"{"cmd":"usage_time","open_s":4000000000,"focused_s":90,"engaged_s":30,"gestures":["artifact_doc","camille"]}"#,
    ];
    for l in lines {
        feed(&mut u, l);
    }
    u.write(t0);
    let tally = got.lock().unwrap().last().unwrap().1.clone();
    let state = LocalState::new(NOV_5);
    let build = Build { version: "2026.11.5".into(), channel: Channel::Stable, os_major: 26, arch: Arch::of(std::env::consts::ARCH) };
    let id = InstallId::draw([7; 16], Day::from_unix_secs(NOV_5));
    let day = Day::from_unix_secs(NOV_5);
    let p = Packet::build(Parts { day, install_id: &id, view: state.view(day), build: &build, surface: Surface::of(true, false), tally: &tally, milestones: state.unsent() });
    let body = bise_usage::encode::packets(&[p]);
    assert_eq!(check_packets_body(&body, FIXTURE), Ok(()), "{}", String::from_utf8_lossy(&body));
}
