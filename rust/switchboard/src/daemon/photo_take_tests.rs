//! The hub's pick of a photo file (daemon/photo_take.rs).

use super::*;
use serde_json::json;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-photo-take-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_boot_picks_its_builds_photo_when_it_fits_and_says_why_not() {
    let d = tmp("pick");
    let evs: Vec<Value> = (0..5).map(|i| json!({"type": "said", "n": i})).collect();
    let st = json!({"agents": []});
    photo::write(&d, "k1", &Mark::of(&evs[..3]), std::slice::from_ref(&st)).unwrap();
    let (from, why) = pick(&d, Some("k1"), &evs, false);
    assert_eq!((from, why), (Some((3, vec![st.to_string()])), None));
    let (from, why) = pick(&d, Some("k2"), &evs, false);
    assert_eq!(from, None);
    assert_eq!(why.as_deref(), Some("no photo"), "another build has no photo of its own");
    let (from, why) = pick(&d, Some("k1"), &evs, true);
    assert_eq!((from, why.as_deref()), (None, Some("worktrees moved this boot")));
    let (from, why) = pick(&d, None, &evs, false);
    assert_eq!((from, why.as_deref()), (None, Some("sb-core's binary unreadable")));
}
