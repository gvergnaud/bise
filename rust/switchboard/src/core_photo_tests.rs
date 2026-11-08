//! issue 36 chunk 3: sb-core's photo / restore_photo, through the real
//! sb-core. A restore only takes a hub that applied nothing yet: a photo
//! never replaces a live state (architect m_34907, law restore_only_fresh).
//! Out of core_tests.rs (its line limit).

use super::super::*;
use serde_json::json;

/// The photo of a small state: one slice (PH.SLICE records a line).
fn photo(t: &mut T) -> Value {
    let mut lines = t.hub.photo().expect("a photo");
    assert_eq!(lines.len(), 1, "a small state is one slice");
    lines.remove(0)
}

/// A slice's lines as the photo file holds them.
fn texts(slices: &[Value]) -> Vec<String> {
    slices.iter().map(Value::to_string).collect()
}

/// `st` in two slices, as sb-core cuts a bigger one (St.slice_take /
/// slice_drop): its first agent with every scalar, then the rest of its
/// agents and every other list, the scalars at their defaults.
fn two_slices(st: &Value) -> (Value, Value) {
    let agents = st["agents"].as_array().expect("its agents");
    assert!(agents.len() >= 2, "two agents to cut between");
    let mut first = json!({"agents": [agents[0]]});
    let mut rest = json!({"agents": agents[1..]});
    for (k, v) in st.as_object().unwrap().iter().filter(|(k, _)| *k != "agents") {
        if v.is_array() {
            first[k] = json!([]);
            rest[k] = v.clone();
        } else {
            first[k] = v.clone();
        }
    }
    (first, rest)
}

#[test]
fn a_hub_that_stepped_refuses_a_photo_and_stays_as_it_is() {
    let mut t = T::new();
    t.spawn_task("a");
    let before = photo(&mut t);
    let out = t.hub.raw(&json!({"t": "restore_photo", "st": {"agents": []}}));
    assert!(out.get("error").is_some(), "a live hub took a photo: {out}");
    let more = t.hub.raw(&json!({"t": "restore_more", "st": {"agents": []}}));
    assert!(more.get("error").is_some(), "a live hub took a slice: {more}");
    assert_eq!(photo(&mut t), before, "the refused restores changed the state");
}

#[test]
fn a_fresh_hub_restores_the_photo_once() {
    let mut t = T::new();
    t.spawn_task("a");
    let st = photo(&mut t);
    assert_eq!(t.hub.raw(&json!({"t": "init", "workspace": "/w"})), json!({}));
    let more = t.hub.raw(&json!({"t": "restore_more", "st": st}));
    assert!(more.get("error").is_some(), "a fresh hub took a slice before its photo: {more}");
    assert_eq!(t.hub.raw(&json!({"t": "restore_photo", "st": st})), json!({}));
    assert_eq!(photo(&mut t), st, "the restored state is the photo's");
    let again = t.hub.raw(&json!({"t": "restore_photo", "st": st}));
    assert!(again.get("error").is_some(), "a second restore was taken: {again}");
}

/// c6 (c): the slices restored last first (restore_photo, then
/// restore_more) are the state whole: each slice's records before the
/// ones restored, the first slice's scalars (law restore_in_slices).
#[test]
fn the_slices_restored_last_first_are_the_state() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let st = photo(&mut t);
    let (first, rest) = two_slices(&st);
    assert_eq!(t.hub.raw(&json!({"t": "init", "workspace": "/w"})), json!({}));
    assert_eq!(t.hub.raw(&json!({"t": "restore_photo", "st": rest})), json!({}));
    assert_eq!(t.hub.raw(&json!({"t": "restore_more", "st": first})), json!({}));
    assert_eq!(photo(&mut t), st, "the slices joined are the photo");
}

/// Chunk 4: a hub rebuilt from the photo of the first k journal events
/// plus the events after it holds the state of the full replay.
#[test]
fn a_hub_rebuilt_from_a_photo_and_the_tail_is_the_full_replays() {
    let mut t = T::new();
    t.spawn_task("a");
    let k = t.journal.borrow().len();
    let st = photo(&mut t);
    t.spawn_task("b");
    let events = t.journal.borrow().clone();
    assert!(events.len() > k, "the tail has events");
    let mut full = Hub::new("/w");
    let all = full.rebuild(&events, None, &mut |_, _| {}).unwrap();
    assert_eq!(all.photo, Err("no photo".into()));
    let mut from = Hub::new("/w");
    let got = from.rebuild(&events, Some((k, texts(&[st]))), &mut |_, _| {}).unwrap();
    assert_eq!(got.photo, Ok(k));
    assert_eq!(from.photo(), full.photo(), "the photo + tail is the full replay");
    assert!(from.st.agents.contains_key("b"), "the mirror has the tail's task");
}

/// The same from a photo in two slices: the file's lines, last first.
#[test]
fn a_hub_rebuilt_from_a_photo_in_slices_is_the_full_replays() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let events = t.journal.borrow().clone();
    let (first, rest) = two_slices(&photo(&mut t));
    let mut full = Hub::new("/w");
    full.rebuild(&events, None, &mut |_, _| {}).unwrap();
    let mut from = Hub::new("/w");
    let got = from.rebuild(&events, Some((events.len(), texts(&[first, rest]))), &mut |_, _| {}).unwrap();
    assert_eq!(got.photo, Ok(events.len()));
    assert_eq!(from.photo(), full.photo(), "the slices are the full replay");
}

/// A photo sb-core refuses (not an object; St.photo_of reads any object,
/// so the key keeps other shapes out): the full replay, as without one.
#[test]
fn a_refused_photo_falls_back_to_the_full_replay() {
    let mut t = T::new();
    t.spawn_task("a");
    let events = t.journal.borrow().clone();
    let mut full = Hub::new("/w");
    full.rebuild(&events, None, &mut |_, _| {}).unwrap();
    let mut from = Hub::new("/w");
    let got = from.rebuild(&events, Some((events.len(), texts(&[json!(["not a state"])]))), &mut |_, _| {}).unwrap();
    assert!(matches!(&got.photo, Err(why) if why.contains("refused")), "{:?}", got.photo);
    assert_eq!(from.photo(), full.photo());
}

/// A photo refused halfway (its last slice restored, an earlier one not):
/// a new state, then the full replay; never the slices restored so far
/// under the journal's events.
#[test]
fn a_photo_refused_halfway_starts_over_with_the_full_replay() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let events = t.journal.borrow().clone();
    let (_, rest) = two_slices(&photo(&mut t));
    let mut full = Hub::new("/w");
    full.rebuild(&events, None, &mut |_, _| {}).unwrap();
    for bad in [json!(["not a state"]).to_string(), "{\"agents\": [".to_string()] {
        let mut from = Hub::new("/w");
        let got = from.rebuild(&events, Some((events.len(), vec![bad, rest.to_string()])), &mut |_, _| {}).unwrap();
        assert!(matches!(&got.photo, Err(why) if why.contains("slice 1 of 2")), "{:?}", got.photo);
        assert_eq!(from.photo(), full.photo(), "the full replay, from a new state");
    }
}

/// The measure on a real journal (issue 36 chunk 4's proof), by hand:
/// `PHOTO_JOURNAL=<journal.jsonl> cargo test -p switchboard --lib
/// photo_on_a_real_journal -- --ignored --nocapture`. For k at 10, 50, 90
/// and 100 % of the events (`PHOTO_PCTS=90,100`: those only): the photo
/// of the first k written to a file and read back (crate::photo), a fresh
/// hub rebuilt from it and the rest, its photo and view the full
/// replay's. Printed: retired instructions (the box's load does not move
/// them; wall times on a loaded box do), sb-core's and the test's own.
#[test]
#[ignore]
fn photo_on_a_real_journal() {
    let Ok(path) = std::env::var("PHOTO_JOURNAL") else { return };
    let pcts: Vec<usize> = std::env::var("PHOTO_PCTS").unwrap_or("10,50,90,100".into()).split(',').filter_map(|p| p.parse().ok()).collect();
    let text = std::fs::read_to_string(path).unwrap();
    let events: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let dir = std::env::temp_dir().join(format!("sb-photo-real-{}", std::process::id()));
    let me = std::process::id();
    let g = |n: u64| format!("{:.1} G", n as f64 / 1e9);
    let mut full = Hub::new("/w");
    let (c0, h0) = (instructions(full.core_pid()), instructions(me));
    full.rebuild(&events, None, &mut |_, _| {}).unwrap();
    let (c1, h1) = (instructions(full.core_pid()), instructions(me));
    println!("full replay: {} events, sb-core {}, hub {}", events.len(), g(c1 - c0), g(h1 - h0));
    let (want, want_view) = (full.photo().expect("the full replay's photo"), full.st.agents.len());
    for pct in pcts {
        let k = events.len() * pct / 100;
        let mut at = Hub::new("/w");
        at.rebuild(&events[..k], None, &mut |_, _| {}).unwrap();
        let (c0, h0) = (instructions(at.core_pid()), instructions(me));
        let st = at.photo().expect("a photo");
        crate::photo::write(&dir, "real", &crate::photo::Mark::of(&events[..k]), &st).unwrap();
        let took = format!("sb-core {}, hub {}", g(instructions(at.core_pid()) - c0), g(instructions(me) - h0));
        drop(at);
        let mut boot = Hub::new("/w");
        let (c0, h0) = (instructions(boot.core_pid()), instructions(me));
        let (head, st) = crate::photo::read(&dir, "real").unwrap();
        let from = match crate::photo::usable(Some(&head), "real", &events, false) {
            crate::photo::Use::Photo { k } => Some((k, st)),
            u => panic!("{pct} %: {u:?}"),
        };
        assert_eq!(boot.rebuild(&events, from, &mut |_, _| {}).unwrap().photo, Ok(k));
        let (c1, h1) = (instructions(boot.core_pid()), instructions(me));
        let bytes = std::fs::metadata(dir.join("real.json")).map_or(0, |m| m.len());
        println!("{pct} %: k {k}, photo {bytes} bytes; taken+written: {took}; boot from it: sb-core {}, hub {}", g(c1 - c0), g(h1 - h0));
        assert!(boot.photo().as_ref() == Some(&want), "{pct} %: the photo + tail differs from the full replay");
        assert_eq!(boot.st.agents.len(), want_view, "{pct} %: the mirror's agents");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// A process's retired instructions so far (macOS's proc_pid_rusage).
fn instructions(pid: u32) -> u64 {
    let mut ri: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    let p = &mut ri as *mut libc::rusage_info_v4 as *mut libc::rusage_info_t;
    let rc = unsafe { libc::proc_pid_rusage(pid as i32, libc::RUSAGE_INFO_V4, p) };
    if rc == 0 { ri.ri_instructions } else { 0 }
}

/// The boot's cost by phase on a real journal (architect m_35726), by
/// hand, on a release build: `PHOTO_JOURNAL=<journal.jsonl> cargo test
/// --release -p switchboard --lib boot_phases_on_a_real_journal --
/// --ignored --nocapture`. Retired instructions of sb-core and of the hub
/// (this test) for each phase of a full-replay boot and of a boot from
/// the photo of every event: the photo file read and checked, restore,
/// the replay batches, view_all's call (sb-core's encode, the line, its
/// parse) and the view's decode + load into the mirror.
#[test]
#[ignore]
fn boot_phases_on_a_real_journal() {
    let Ok(path) = std::env::var("PHOTO_JOURNAL") else { return };
    let text = std::fs::read_to_string(path).unwrap();
    let all: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    // PHOTO_TAIL=n (boot-speed): the photo holds all but the last n
    // events, which the boot replays after the restore, as a real boot does
    let tail: usize = std::env::var("PHOTO_TAIL").ok().and_then(|n| n.parse().ok()).unwrap_or(0).min(all.len());
    let events = &all[..all.len() - tail];
    let dir = std::env::temp_dir().join(format!("sb-photo-phases-{}", std::process::id()));
    let me = std::process::id();
    let mut lines = Vec::new();
    let t0 = std::cell::Cell::new(std::time::Instant::now());
    let phase = |lines: &mut Vec<String>, hub: &Hub, name: &str, from: &mut (u64, u64)| {
        let now = (instructions(hub.core_pid()), instructions(me));
        let ms = t0.replace(std::time::Instant::now()).elapsed().as_millis();
        lines.push(format!("{name:<28} sb-core {:>6.1} G  hub {:>6.1} G  {ms:>6} ms", (now.0 - from.0) as f64 / 1e9, (now.1 - from.1) as f64 / 1e9));
        *from = now;
    };
    // the full replay, its batches as replay_with sends them
    let mut full = Hub::new("/w");
    let mut at = (instructions(full.core_pid()), instructions(me));
    let core: Vec<&Value> = events.iter().filter(|e| !crate::forge::is_pr_line(e)).collect();
    for batch in core.chunks(REPLAY_BATCH) {
        let evs = batch.iter().map(|ev| (*ev).clone()).collect();
        full.raw(&Link::ReplayMany(cl::ReplayMany { evs }).to_value());
    }
    phase(&mut lines, &full, "full: replay batches", &mut at);
    let out = full.link.call_lines(&Link::ViewAll(cl::ViewAll {}).to_value()).unwrap();
    phase(&mut lines, &full, "full: view_all call", &mut at);
    full.load_answer_views(&out);
    phase(&mut lines, &full, "full: view decode + load", &mut at);
    let st = full.photo().expect("a photo");
    phase(&mut lines, &full, "photo: take", &mut at);
    crate::photo::write(&dir, "real", &crate::photo::Mark::of(events), &st).unwrap();
    phase(&mut lines, &full, "photo: mark + write", &mut at);
    drop(st);
    // the boot from it
    let mut boot = Hub::new("/w");
    let mut at = (instructions(boot.core_pid()), instructions(me));
    let (head, st) = crate::photo::read(&dir, "real").unwrap();
    phase(&mut lines, &boot, "boot: photo file read", &mut at);
    let k = match crate::photo::usable(Some(&head), "real", events, false) {
        crate::photo::Use::Photo { k } => k,
        u => panic!("{u:?}"),
    };
    phase(&mut lines, &boot, "boot: usable (mark of all)", &mut at);
    assert_eq!(boot.restore(&st).unwrap(), Ok(()));
    drop(st);
    phase(&mut lines, &boot, "boot: restore", &mut at);
    for ev in events[..k].iter().filter(|ev| crate::forge::is_pr_line(ev)) {
        crate::forge::read_line(&mut boot.pr_known, ev);
        boot.pr_news.read_journal(ev);
    }
    phase(&mut lines, &boot, "boot: PR lines", &mut at);
    let after: Vec<&Value> = all[k..].iter().filter(|e| !crate::forge::is_pr_line(e)).collect();
    for batch in after.chunks(REPLAY_BATCH) {
        let evs = batch.iter().map(|ev| (*ev).clone()).collect();
        boot.raw(&Link::ReplayMany(cl::ReplayMany { evs }).to_value());
    }
    phase(&mut lines, &boot, &format!("boot: replay {} after it", after.len()), &mut at);
    let out = boot.link.call_lines(&Link::ViewAll(cl::ViewAll {}).to_value()).unwrap();
    phase(&mut lines, &boot, "boot: view_all call", &mut at);
    let bytes: usize = out.iter().map(|l| l.to_string().len()).sum();
    boot.load_answer_views(&out);
    phase(&mut lines, &boot, "boot: view decode + load", &mut at);
    println!("{}\nview_all answer: {bytes} bytes", lines.join("\n"));
    let _ = std::fs::remove_dir_all(dir);
}
