use super::*;
use serde_json::json;

fn view(n: u64) -> ArtView {
    ArtView { rows: vec![json!({ "id": n })], ..ArtView::default() }
}

/// architect m_61130 2: three key changes during a build give exactly
/// two builds (the one running, then one for the newest key), never a
/// queue; the landed view is shown even when a newer key waits.
#[test]
fn key_changes_during_a_build_coalesce_into_one_more() {
    let mut r = ArtRows::default();
    let mut builds = 0;
    let (v, start) = r.ask((1, 0, 0, 0));
    assert!(v.is_none(), "no view before the first build: no event yet");
    builds += start as usize;
    for k in 2..=4 {
        let (_, start) = r.ask((k, 0, 0, 0));
        builds += start as usize;
    }
    r.built((1, 0, 0, 0), view(1));
    let (v, start) = r.ask((4, 0, 0, 0));
    assert_eq!(v.as_deref(), Some(&view(1)), "the landed view goes out while the newest builds");
    builds += start as usize;
    r.built((4, 0, 0, 0), view(4));
    let (v, start) = r.ask((4, 0, 0, 0));
    builds += start as usize;
    assert_eq!(v.as_deref(), Some(&view(4)));
    assert_eq!(builds, 2);
}

/// A key that did not move asks no build; a failed start lets the next
/// ask try again.
#[test]
fn a_still_key_builds_nothing_and_a_failed_start_retries() {
    let mut r = ArtRows::default();
    assert!(r.ask((1, 0, 0, 0)).1);
    r.built((1, 0, 0, 0), view(1));
    assert!(!r.ask((1, 0, 0, 0)).1);
    assert!(r.ask((2, 0, 0, 0)).1);
    r.failed();
    assert!(r.ask((2, 0, 0, 0)).1);
}

/// The backstop is said only when it alone moved the key and the rows
/// changed.
#[test]
fn the_backstop_is_said_only_when_it_found_a_change() {
    let mut r = ArtRows::default();
    assert!(!r.built((1, 1, 0, 1), view(1)));
    assert!(!r.built((1, 1, 1, 1), view(1)), "backstop, same rows");
    assert!(r.built((1, 1, 2, 1), view(2)), "backstop, rows changed");
    assert!(!r.built((2, 1, 3, 1), view(3)), "a store write moved it too");
}

/// hub-loop m_61105: with his hub's size (1300 artifacts, 250 pages) a
/// key change on the loop costs an ask, never the build (the build was
/// ~5 s at load 50-85, run on the loop by the state push).
#[test]
fn a_key_change_with_1300_artifacts_answers_at_once() {
    let state = std::env::temp_dir().join(format!("sb-art-rows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&state);
    for i in 0..1300 {
        let d = state.join("artifacts").join(format!("a{i}"));
        std::fs::create_dir_all(&d).unwrap();
        let meta = json!({"id": format!("a{i}"), "title": format!("a{i}"), "kind": "doc", "agent": "t1", "by": "agent", "created_ms": i, "source": format!("/nowhere/ws/f{i}.md"),
            "cwd": "/nowhere/ws", "versions": [{"v": 1, "at_ms": i, "target": format!("/nowhere/ws/f{i}.md")}]});
        std::fs::write(d.join("meta.json"), meta.to_string()).unwrap();
    }
    let job = |now| ArtJob { state: state.clone(), workspace: "/nowhere/ws".into(), who: vec![], now };
    let mut r = ArtRows::default();
    assert!(r.ask((1, 0, 0, 0)).1);
    let t = std::time::Instant::now();
    let v = job(1).build();
    let build = t.elapsed();
    assert_eq!(v.rows.len(), 1300);
    r.built((1, 0, 0, 0), v);
    // the store moved: the loop's part of a key change
    let t = std::time::Instant::now();
    let (v, start) = r.ask((2, 0, 0, 0));
    let ask = t.elapsed();
    assert!(start);
    assert_eq!(v.map(|v| v.rows.len()), Some(1300));
    assert!(ask.as_millis() < 50, "a key change took {ask:?} on the loop (the build alone: {build:?})");
    let _ = std::fs::remove_dir_all(&state);
}
