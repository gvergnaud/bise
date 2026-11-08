use super::*;
use std::sync::{Arc, Mutex};

use bise_usage::install_id::InstallId;
use bise_usage::law::check_packets_body;
use bise_usage::packet::{Arch, Build, Channel, Packet, Parts, Surface};
use bise_usage::state::LocalState;

/// What a sink got: each write's day and tally.
type Written = Arc<Mutex<Vec<(Day, Tally)>>>;

fn recording() -> (Sink, Written) {
    let got: Written = Arc::default();
    let g = got.clone();
    let sink: Sink = Box::new(move |day, t| {
        g.lock().unwrap().push((day, t.clone()));
        Ok(())
    });
    (sink, got)
}

const NOV_5: u64 = 1_793_836_800; // 2026-11-05 00:00 UTC
const S: Duration = Duration::from_secs(1);

fn secs(t: &Tally, k: MinutesKey) -> u64 {
    t.seconds.get(&k).copied().unwrap_or(0)
}

#[test]
fn attached_counts_every_second_focused_only_the_focused_ones() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut c = Clock::new(sink, t0, NOV_5 + 3600);
    // 60 s focused, then the focus goes: 120 s away, then back for 30 s
    c.tick(t0 + 60 * S, NOV_5 + 3660, false, 0);
    c.tick(t0 + 180 * S, NOV_5 + 3780, true, 0);
    c.tick(t0 + 210 * S, NOV_5 + 3810, true, 0);
    assert!(got.lock().unwrap().is_empty(), "nothing written before 5 minutes");
    c.tick(t0 + WRITE_EVERY, NOV_5 + 3900, true, 0);
    let got = got.lock().unwrap();
    assert_eq!(got.len(), 1, "one write at 5 minutes");
    let (day, t) = &got[0];
    assert_eq!(*day, Day::from_unix_secs(NOV_5));
    assert_eq!(secs(t, MinutesKey::TuiAttached), 300);
    assert_eq!(secs(t, MinutesKey::TuiFocused), 180);
    assert!(t.counts.is_empty());
}

#[test]
fn a_span_over_midnight_goes_half_to_each_day() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let midnight = NOV_5 + 86_400;
    let mut c = Clock::new(sink, t0, midnight - 60);
    c.tick(t0 + 30 * S, midnight - 30, true, 1);
    // the next tick comes 60 s later, 30 s past midnight
    c.tick(t0 + 90 * S, midnight + 30, true, 0);
    {
        let got = got.lock().unwrap();
        assert_eq!(got.len(), 1, "the new day writes the old one");
        let (day, t) = &got[0];
        assert_eq!(*day, Day::from_unix_secs(NOV_5));
        assert_eq!(secs(t, MinutesKey::TuiAttached), 60);
        assert_eq!(t.counts[&CountKey::CrashTui], 1);
    }
    c.close(t0 + 90 * S, midnight + 30, 0);
    let got = got.lock().unwrap();
    let (day, t) = got.last().unwrap();
    assert_eq!(*day, Day::from_unix_secs(midnight));
    assert_eq!(secs(t, MinutesKey::TuiAttached), 30, "the new day starts at zero");
    assert!(t.counts.is_empty(), "the old day's panic stays in the old day");
}

#[test]
fn a_held_loop_counts_at_most_a_step_and_close_writes_the_panic() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut c = Clock::new(sink, t0, NOV_5);
    // the loop held for an hour (a screen over it): 5 minutes believed
    c.close(t0 + 3600 * S, NOV_5 + 3600, 2);
    let got = got.lock().unwrap();
    let (_, t) = got.last().unwrap();
    assert_eq!(secs(t, MinutesKey::TuiAttached), MAX_STEP.as_secs());
    assert_eq!(t.counts[&CountKey::CrashTui], 2);
}

/// The PII law (plan, les tests): the TUI reads no text at all; what it
/// writes goes into a packet the law accepts, the minutes in their bounds.
#[test]
fn law_pii_the_tui_counts_only_time_and_panics() {
    let (sink, got) = recording();
    let t0 = Instant::now();
    let mut c = Clock::new(sink, t0, NOV_5);
    for i in 1..=600u32 {
        c.tick(t0 + i * 200 * S, NOV_5 + u64::from(i) * 200, i % 3 != 0, u32::from(i % 50 == 0));
    }
    c.close(t0 + 600 * 200 * S, NOV_5 + 86_399, 0);
    let tally = got.lock().unwrap().last().unwrap().1.clone();
    let day = Day::from_unix_secs(NOV_5);
    let state = LocalState::new(NOV_5);
    let build = Build { version: "2026.11.5".into(), channel: Channel::Stable, os_major: 26, arch: Arch::of(std::env::consts::ARCH) };
    let id = InstallId::draw([7; 16], day);
    let p = Packet::build(Parts { day, install_id: &id, view: state.view(day), build: &build, surface: Surface::of(false, true), tally: &tally, milestones: state.unsent() });
    let body = bise_usage::encode::packets(&[p]);
    let fixture = ["camille-taxes-3f9a2b1c", "/Users/camille", "tui-panic"];
    assert_eq!(check_packets_body(&body, &fixture), Ok(()), "{}", String::from_utf8_lossy(&body));
}
