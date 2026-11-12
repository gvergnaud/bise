//! crate::photo: when a photo fits the journal, and its files.

use super::*;
use serde_json::json;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-photo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn events() -> Vec<Value> {
    vec![
        json!({"type": "init", "ws": {"path": "/w/a"}}),
        json!({"type": "said", "text": "café ─ 😀"}),
        json!({"type": "spawned", "ws": {"path": "/w/b"}}),
        json!({"type": "said", "text": "bye"}),
    ]
}

fn head(events: &[Value]) -> Head {
    Head { key: "k1".into(), mark: Mark::of(events) }
}

/// hub-loop: the mark made with no copy, in one pass, is the mark the
/// photos on disk hold (the old way, written out here): another hash
/// would turn every boot into a full replay of his 245k events.
#[test]
fn the_one_pass_mark_is_the_photos_mark() {
    let evs = events();
    let mut old = Mark::default();
    for ev in &evs {
        old.events += 1;
        let mut b = old.prefix.to_le_bytes().to_vec();
        b.extend_from_slice(ev.to_string().as_bytes());
        old.prefix = fnv(&b);
        if let Some(p) = ev["ws"]["path"].as_str() {
            let mut b = old.paths.to_le_bytes().to_vec();
            b.extend_from_slice(p.as_bytes());
            old.paths = fnv(&b);
        }
    }
    assert_eq!(Mark::of(&evs), old);
    for k in 0..=evs.len() + 1 {
        let (all, at) = Mark::of_and_at(&evs, k, &mut |_, _| {});
        assert_eq!(all, old);
        assert_eq!(at, Mark::of(&evs[..k.min(evs.len())]), "k {k}");
        assert_eq!(usable_at(Some(&head(&evs[..k.min(evs.len())])), "k1", evs.len(), &at, false), usable(Some(&head(&evs[..k.min(evs.len())])), "k1", &evs, false));
    }
}

#[test]
fn a_photo_of_the_first_k_events_fits_the_journal_that_grew_since() {
    let evs = events();
    assert_eq!(usable(Some(&head(&evs[..3])), "k1", &evs, false), Use::Photo { k: 3 });
    assert_eq!(usable(Some(&head(&evs)), "k1", &evs, false), Use::Photo { k: 4 });
}

#[test]
fn no_photo_fits_another_core_a_shorter_or_other_journal_or_a_move() {
    let evs = events();
    let h = head(&evs[..3]);
    let full = |u: Use| matches!(u, Use::Full { .. });
    assert!(full(usable(None, "k1", &evs, false)));
    assert!(full(usable(Some(&h), "k2", &evs, false)), "another sb-core");
    assert!(full(usable(Some(&h), "k1", &evs[..2], false)), "the journal lost events");
    assert!(full(usable(Some(&h), "k1", &evs, true)), "worktrees moved this boot");
    let mut other = evs.clone();
    other[2]["type"] = json!("dropped");
    assert!(full(usable(Some(&h), "k1", &other, false)), "event k differs");
    let mut moved = evs.clone();
    moved[0]["ws"]["path"] = json!("/w/elsewhere");
    assert!(full(usable(Some(&h), "k1", &moved, false)), "a path reads otherwise");
    assert!(full(usable(Some(&head(&[])), "k1", &evs, false)), "a photo of nothing");
}

/// The mark the hub keeps as it writes lines is the one the next boot
/// computes from the lines it reads back.
#[test]
fn the_mark_of_written_lines_is_the_mark_of_the_lines_read_back() {
    let evs = events();
    let text: String = evs.iter().map(|e| e.to_string() + "\n").collect();
    let read: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(Mark::of(&evs), Mark::of(&read));
}

#[test]
fn a_photo_reads_back_as_written_and_two_keys_stay() {
    let d = tmp("files");
    let evs = events();
    let st = [json!({"agents": [{"name": "main", "text": "é─😀\nline"}]}), json!({"old": []})];
    write(&d, "k1", &Mark::of(&evs), &st).unwrap();
    let (h, back) = read(&d, "k1").expect("the photo");
    assert_eq!(h, head(&evs));
    let back: Vec<Value> = back.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(back, st, "one slice a line, read back as written");
    write(&d, "k2", &Mark::of(&evs[..1]), &st).unwrap();
    write(&d, "k3", &Mark::of(&evs[..2]), &st).unwrap();
    assert!(read(&d, "k3").is_some() && read(&d, "k2").is_some(), "this build's and the one before");
    assert!(read(&d, "k1").is_none(), "an older build's photo goes");
    assert!(read(&d, "nope").is_none());
}

#[test]
fn a_torn_or_old_photo_is_none_and_a_crash_tmp_goes_at_boot() {
    let d = tmp("bad");
    std::fs::write(d.join("k1.json"), "{\"v\":1,\"key\":\"k1\",\"ev").unwrap();
    assert!(read(&d, "k1").is_none(), "torn");
    // a released hub's photo (v 1, before c6: its messages carried their
    // words): refused, the boot replays the journal
    std::fs::write(d.join("k2.json"), "{\"v\":1,\"key\":\"k2\",\"events\":1,\"prefix\":\"0\",\"paths\":\"0\",\"st\":{}}").unwrap();
    assert!(read(&d, "k2").is_none(), "another file version");
    // a V2 photo (c6 5b, the state in one object): refused too
    std::fs::write(d.join("k4.json"), "{\"v\":2,\"key\":\"k4\",\"events\":1,\"prefix\":\"0\",\"paths\":\"0\",\"st\":{}}").unwrap();
    assert!(read(&d, "k4").is_none(), "a V2 photo");
    // fewer slices than its head counts: a file cut short
    std::fs::write(d.join("k5.json"), "{\"v\":3,\"key\":\"k5\",\"events\":1,\"prefix\":\"0\",\"paths\":\"0\",\"slices\":2}\n{}\n").unwrap();
    assert!(read(&d, "k5").is_none(), "a slice missing");
    std::fs::write(d.join("k3.json.tmp"), "half").unwrap();
    clean(&d);
    assert!(!d.join("k3.json.tmp").exists());
    assert!(d.join("k2.json").exists(), "clean leaves photos alone");
}

#[test]
fn an_idle_photo_waits_ten_minutes_of_quiet_and_comes_once_a_day() {
    let now = 10 * IDLE_EVERY_MS;
    assert!(!idle_due(now, now - IDLE_AFTER_MS + 1, 0), "not quiet long enough");
    assert!(idle_due(now, now - IDLE_AFTER_MS, 0), "quiet, never taken");
    assert!(!idle_due(now, now - IDLE_AFTER_MS, now - IDLE_EVERY_MS + 1), "taken today");
    assert!(idle_due(now, now - IDLE_AFTER_MS, now - IDLE_EVERY_MS));
}

#[test]
fn the_key_is_the_binarys_bytes() {
    let d = tmp("key");
    std::fs::write(d.join("a"), b"sb-core one").unwrap();
    std::fs::write(d.join("b"), b"sb-core two").unwrap();
    let (a, b) = (key_of(&d.join("a")).unwrap(), key_of(&d.join("b")).unwrap());
    assert_ne!(a, b);
    assert_eq!(key_of(&d.join("a")).unwrap(), a);
    assert_eq!(key_of(&d.join("none")), None);
    assert_eq!(format!("{:016x}", fnv(b"")), "cbf29ce484222325", "FNV-1a's offset basis");
}

/// architect m_37035: a due photo waits for a good moment (sb-core's
/// encode holds the loop): under load or behind waiting messages it stays
/// due, one hub.log line; at a quiet, unloaded tick it is taken, once.
#[test]
fn a_due_photo_waits_for_a_quiet_unloaded_tick() {
    let mut w = Wait { due: true, ..Default::default() };
    assert_eq!(w.tick(false, true, false, 4.0), Some(Act::Say("photo deferred: load x4.0".into())), "loaded: deferred, said once");
    assert_eq!(w.tick(false, true, false, 4.0), None, "said once only");
    assert!(w.due, "still due");
    let mut w = Wait { due: true, ..Default::default() };
    assert_eq!(w.tick(false, false, false, 1.0), Some(Act::Say("photo deferred: messages waiting".into())), "a client line waiting");
    assert_eq!(w.tick(false, true, true, 1.0), None, "a land in line: still deferred, no second line");
    assert!(w.due);
    assert_eq!(w.tick(false, true, false, 1.0), Some(Act::Take), "quiet and unloaded: taken");
    assert_eq!(w.tick(false, true, false, 1.0), None, "taken once");
    assert_eq!(w.tick(true, true, false, 1.0), Some(Act::Take), "an idle photo, the same rule");
    assert_eq!(moment(true, false, 1.9), Ok(()), "x1.9 is not loaded");
}
