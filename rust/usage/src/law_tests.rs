//! The PII law on this crate's path: PII-like data in every input the
//! crate takes from outside (a spool file's keys, a hub id, a version, the
//! build), through the spool, the sum, the packet and the encoder; the
//! bytes that would leave are checked. The collectors (steps 2 to 4) feed
//! their own fixture through their counting and call the same check.

use super::*;
use crate::day::Day;
use crate::encode;
use crate::install_id::InstallId;
use crate::key::{CountKey, HistKey, Milestone, MinutesKey};
use crate::packet::{Arch, Build, Channel, Packet, Parts, Surface};
use crate::spool::{read_day, write_source, Source, SourceKind};
use crate::state::LocalState;
use crate::tally::Tally;

/// What looks like his data: never in a body.
const FIXTURE: &[&str] = &[
    "camille",
    "/Users/camille/projects/taxes-2026",
    "camille.martin@example.com",
    "sk-fake-7f3a9c2e1b",
    "taxes-2026",
    "refactor the invoice parser",
    "https://acme.example.com/brief",
    "Error: ENOENT open '/Users/camille/.ssh/id_ed25519'",
    "invoice-agent",
    "acme",
];

fn day() -> Day {
    Day::parse("2026-11-05").unwrap()
}

/// A spool day written by processes whose inputs carry the fixture.
fn spooled_day(dir: &std::path::Path) -> Vec<(SourceKind, Tally)> {
    let permit = crate::sharing::test_permit();
    let mut hub = Tally::default();
    hub.add(CountKey::AgentCreatedByUser, 3);
    hub.add(CountKey::TurnDone, 5000);
    hub.time(MinutesKey::AgentWork, 7 * 3600 + 7);
    hub.observe(HistKey::PeakAgents, 6);
    hub.count(CountKey::UserMessage);
    write_source(&permit, dir, &Source::hub("taxes-2026-3f9a2b1c", 4242).unwrap(), &hub).unwrap();
    // a newer bise's core wrote keys this build doesn't know: his words as keys
    let newer = serde_json::json!({
        "counts": { "artifact_opened": 2, "/Users/camille/projects/taxes-2026": 9, "camille.martin@example.com": 1 },
        "seconds": { "app_open": 5000, "sk-fake-7f3a9c2e1b": 60 },
        "hist": { "refactor the invoice parser": [1] },
        "brief": "refactor the invoice parser",
    });
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(Source::core(77).file_name()), newer.to_string()).unwrap();
    // files that are not sources, named and filled with his data
    std::fs::write(dir.join("camille.json"), r#"{"counts":{"land":999999}}"#).unwrap();
    std::fs::write(dir.join("tui-acme-1.json"), r#"{"counts":{"land":1}}"#).unwrap();
    read_day(dir)
}

fn packet_of(sources: &[(SourceKind, Tally)], version: &str) -> Packet {
    let mut state = LocalState::new(Day::parse("2026-10-01").unwrap().start_secs());
    state.mark_active(day());
    state.reach(Milestone::FirstAgent, state.first_launch_secs + 300);
    let tally = Tally::sum(sources.iter().map(|(_, t)| t));
    let build = Build { version: version.to_string(), channel: Channel::Stable, os_major: 26, arch: Arch::of(std::env::consts::ARCH) };
    let id = InstallId::draw([0x3c; 16], day());
    let desktop = sources.iter().any(|(k, _)| *k == SourceKind::Core);
    let terminal = sources.iter().any(|(k, _)| *k == SourceKind::Tui);
    Packet::build(Parts {
        day: day(),
        install_id: &id,
        view: state.view(day()),
        build: &build,
        surface: Surface::of(desktop, terminal),
        tally: &tally,
        milestones: state.unsent(),
    })
}

#[test]
fn law_pii_a_body_holds_nothing_of_the_fixture() {
    let dir = std::env::temp_dir().join(format!("bise-usage-pii-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sources = spooled_day(&dir);
    assert_eq!(sources.len(), 2, "the hub and the core; the other files are not sources");
    let versions = ["2026.11.5-3", "/Users/camille/bise", "camille.martin@example.com"];
    let batch: Vec<Packet> = versions.iter().map(|v| packet_of(&sources, v)).collect();
    let body = encode::packets(&batch);
    assert_eq!(check_packets_body(&body, FIXTURE), Ok(()), "{}", String::from_utf8_lossy(&body));
    // what the packet kept: its known keys, capped and rounded
    let p = &batch[0];
    assert_eq!(p.counts[&CountKey::TurnDone], 999);
    assert_eq!(p.counts[&CountKey::ArtifactOpened], 2);
    assert_eq!(p.minutes[&MinutesKey::AgentWork], 420);
    assert_eq!(p.minutes[&MinutesKey::AppOpen], 85);
    assert_eq!(p.surface, Surface::Desktop);
    assert_eq!(batch[1].version, "unknown");
    let id = InstallId::draw([0x3c; 16], day());
    assert_eq!(check_delete_body(&encode::delete_request(&id), FIXTURE), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The law fails on each kind of leak (so a green run means something).
#[test]
fn law_pii_catches_each_kind_of_leak() {
    let p = packet_of(&[], "2026.11.5-3");
    let good: serde_json::Value = serde_json::from_slice(&encode::packets(&[p])).unwrap();
    let leak = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        edit(&mut v);
        check_packets_body(&serde_json::to_vec(&v).unwrap(), FIXTURE)
    };
    assert_eq!(leak(&|_| {}), Ok(()));
    type Edit = Box<dyn Fn(&mut serde_json::Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("a free field", Box::new(|v| v["packets"][0]["brief"] = "ok".into())),
        ("a path", Box::new(|v| v["packets"][0]["version"] = "/tmp/x".into())),
        ("a fixture word", Box::new(|v| v["packets"][0]["version"] = "acme".into())),
        ("an unknown key", Box::new(|v| v["packets"][0]["counts"]["acme"] = 1.into())),
        ("a count past 999", Box::new(|v| v["packets"][0]["counts"]["land"] = 1000.into())),
        ("minutes off the step", Box::new(|v| v["packets"][0]["minutes"]["app_open"] = 7.into())),
        ("a value off its list", Box::new(|v| v["packets"][0]["channel"] = "nightly".into())),
        ("a field missing", Box::new(|v| drop(v["packets"][0].as_object_mut().unwrap().remove("day")))),
        ("an email", Box::new(|v| v["packets"][0]["version"] = "a@b".into())),
        ("a top field", Box::new(|v| v["host"] = "mac".into())),
    ];
    for (what, edit) in cases {
        assert!(leak(&*edit).is_err(), "{what} passed the law");
    }
}

#[test]
fn a_wildcard_arm_is_found() {
    let src = "fn key_of(c: &Cmd) -> Option<CountKey> {\n    match c {\n        Cmd::Open { .. } => Some(CountKey::ArtifactOpened),\n        Cmd::Quit => None, // not counted\n    }\n}\nfn other() {\n    match x {\n        _ => {}\n    }\n}\n";
    assert_eq!(wildcard_arms(src, "key_of"), Ok(vec![]));
    let bad = src.replace("Cmd::Quit => None", "_ => None");
    assert_eq!(wildcard_arms(&bad, "key_of"), Ok(vec![4]));
    assert!(wildcard_arms(src, "missing").is_err());
}

/// The `/` and `@` rule stands on its own, under the typed checks (a type
/// loosened later still can't let a path or a mail through).
#[test]
fn law_pii_no_slash_or_at_anywhere() {
    assert_eq!(no_slash_or_at(&serde_json::json!({"a": ["2026.11.5", {"b": 1}]})), Ok(()));
    for bad in [serde_json::json!({"a": ["x/y"]}), serde_json::json!({"a@b": 1}), serde_json::json!(["camille@x"])] {
        assert!(no_slash_or_at(&bad).is_err(), "{bad}");
    }
}
