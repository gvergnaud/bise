//! The release offer's laws (update-flow, architect m_45050 §7).

use super::*;

fn latest(id: &str) -> Latest {
    Latest { id: id.into(), version: format!("2026.10.2-{id}"), notes: vec![], url: String::new(), changelog: String::new() }
}

fn facts<'a>(l: &'a Latest, my: &'a str) -> Facts<'a> {
    Facts { latest: Some(l), newer: l.id != my, my_id: my, ..Facts::default() }
}

fn state(f: &Facts) -> (State, bool) {
    plan(f).map(|o| (o.state, o.switch_me)).expect("an offer")
}

#[test]
fn no_manifest_no_offer() {
    assert_eq!(plan(&Facts { my_id: "4", ..Facts::default() }), None);
}

#[test]
fn a_newer_release_is_ready_and_nothing_switches() {
    let l = latest("5");
    assert_eq!(state(&facts(&l, "4")), (State::Ready, false));
    // the same version, or an older one: up to date
    assert_eq!(state(&facts(&l, "5")), (State::UpToDate, false));
    let older = Facts { newer: false, ..facts(&l, "6") };
    assert_eq!(state(&older), (State::UpToDate, false));
}

#[test]
fn later_hides_it_until_a_newer_release() {
    let l = latest("5");
    assert_eq!(state(&Facts { later: Some("5"), ..facts(&l, "4") }), (State::Later, false));
    let next = latest("6");
    assert_eq!(state(&Facts { later: Some("5"), ..facts(&next, "4") }), (State::Ready, false));
    // later never stops an update asked for anywhere
    assert_eq!(state(&Facts { installing: Some("5"), later: Some("5"), ..facts(&l, "4") }), (State::Installing, false));
    let go = Go { id: "5".into(), at: 100 };
    assert_eq!(state(&Facts { go: Some(&go), later: Some("5"), ..facts(&l, "4") }), (State::Updating, true));
}

#[test]
fn an_install_running_anywhere_is_installing_everywhere() {
    let l = latest("5");
    assert_eq!(state(&Facts { installing: Some("5"), ..facts(&l, "4") }), (State::Installing, false));
    // an install of an older release is not this one's
    assert_eq!(state(&Facts { installing: Some("3"), ..facts(&l, "4") }), (State::Ready, false));
}

#[test]
fn go_switches_every_hub_still_older_and_none_on_it() {
    let l = latest("5");
    let go = Go { id: "5".into(), at: 100 };
    assert_eq!(state(&Facts { go: Some(&go), ..facts(&l, "4") }), (State::Updating, true));
    // on it: nothing to show, no switch
    assert_eq!(state(&Facts { go: Some(&go), ..facts(&l, "5") }), (State::UpToDate, false));
    // a go for an older release is stale: the newer one is ready
    let next = latest("6");
    assert_eq!(state(&Facts { go: Some(&go), ..facts(&next, "4") }), (State::Ready, false));
}

#[test]
fn a_failed_switch_is_not_retried_until_a_new_go_or_a_newer_release() {
    let l = latest("5");
    let go = Go { id: "5".into(), at: 100 };
    let failed = Failed { id: "5".into(), at: 200, why: "the hub died".into(), log: Vec::new() };
    let f = Facts { go: Some(&go), my_failed: Some(&failed), ..facts(&l, "4") };
    assert_eq!(state(&f), (State::Failed { why: "the hub died".into(), log: Vec::new() }, false));
    // a new update now (later than the failure): tried again
    let again = Go { id: "5".into(), at: 300 };
    assert_eq!(state(&Facts { go: Some(&again), ..f.clone() }), (State::Updating, true));
    // a newer release: ready again
    let next = latest("6");
    assert_eq!(state(&Facts { latest: Some(&next), newer: true, ..f }), (State::Ready, false));
}

#[test]
fn only_ready_installing_and_updating_show() {
    let l = latest("5");
    let shown = |s: State| Offer { release: l.clone(), state: s, switch_me: false, install_me: false }.shown();
    assert!(shown(State::Ready) && shown(State::Installing) && shown(State::Updating));
    assert!(!shown(State::Later) && !shown(State::UpToDate) && !shown(State::Failed { why: String::new(), log: Vec::new() }));
}

#[test]
fn the_manifest_gives_the_release_its_notes_and_page() {
    let m = r#"{"id":"7e187ba","version":"2026.10.2-5","notes":["- the inbox keeps your place","","/update from any thread"]}"#;
    let l = latest_of(m, "https://github.com/o/r/releases/tag/").unwrap();
    assert_eq!(l.id, "7e187ba");
    assert_eq!(l.version, "2026.10.2-5");
    assert_eq!(l.notes, ["the inbox keeps your place", "/update from any thread"]);
    assert_eq!(l.url, "https://github.com/o/r/releases/tag/v2026.10.2-5");
    // the changelog: the notes as written (markdown kept), for the card
    assert_eq!(l.changelog, "- the inbox keeps your place\n\n/update from any thread");
    let one = latest_of(r###"{"id":"a","notes":"## Fixes\n- one\n- two\n"}"###, "").unwrap();
    assert_eq!(one.changelog, "## Fixes\n- one\n- two");
    assert_eq!(latest_of(r#"{"id":"a"}"#, "").unwrap().changelog, "");
    assert_eq!(latest_of(r#"{"version":"1"}"#, ""), None);
}

#[test]
fn a_long_changelog_is_cut_at_a_line_under_16_kb() {
    let long: Vec<String> = (0..2000).map(|i| format!("- change number {i}")).collect();
    let m = serde_json::json!({"id": "a", "notes": long}).to_string();
    let c = latest_of(&m, "").unwrap().changelog;
    assert!(c.len() <= CHANGELOG_MAX, "{}", c.len());
    assert!(c.ends_with('…') && c.starts_with("- change number 0\n"));
    let last = c.trim_end_matches('…').lines().last().unwrap();
    assert!(long.iter().any(|l| l == last), "a whole line last: {last}");
}

fn home() -> (crate::Home, PathBuf) {
    let dir = std::env::temp_dir().join(format!("release-offer-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (crate::Home::at(&dir), dir)
}

#[test]
fn go_and_later_round_trip_and_later_clears() {
    let (h, dir) = home();
    assert_eq!(read_go(&h), None);
    write_go(&h, &Go { id: "5".into(), at: 7 }).unwrap();
    assert_eq!(read_go(&h), Some(Go { id: "5".into(), at: 7 }));
    write_later(&h, Some("5")).unwrap();
    assert_eq!(read_later(&h).as_deref(), Some("5"));
    write_later(&h, None).unwrap();
    write_later(&h, None).unwrap();
    assert_eq!(read_later(&h), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_lock_is_one_holders_and_freed_when_its_holder_dies() {
    let (h, dir) = home();
    let path = h.switch_lock();
    assert!(!held(&path));
    std::fs::create_dir_all(h.cache_dir()).unwrap();
    // a child process holds it, then is killed mid-probation
    let mut child = std::process::Command::new("/usr/bin/python3")
        .args(["-c", "import fcntl,sys,time; f=open(sys.argv[1],'a'); fcntl.flock(f,fcntl.LOCK_EX); print('held',flush=True); time.sleep(60)"])
        .arg(&path)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.as_mut().unwrap()), &mut line).unwrap();
    assert!(held(&path), "the switcher holds it");
    assert!(try_lock(&path).is_none(), "the next hub waits");
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(try_lock(&path).is_some(), "the next hub gets it once the switcher is gone");
    // the install lock says the id it installs
    let mut l = try_lock(&h.update_install()).unwrap();
    l.say("5");
    // held by this process: another process would see it; a second try here fails too
    assert!(try_lock(&h.update_install()).is_none());
    drop(l);
    assert_eq!(installing(&h), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn auto_installs_a_ready_release_and_never_a_failed_one() {
    let l = latest("5");
    let auto = |f: Facts| plan(&Facts { auto: true, ..f }).map(|o| (o.state, o.install_me)).unwrap();
    assert_eq!(auto(facts(&l, "4")), (State::Ready, true));
    // installing, updating, failed, up to date: nothing to install
    assert_eq!(auto(Facts { installing: Some("5"), ..facts(&l, "4") }), (State::Installing, false));
    let go = Go { id: "5".into(), at: 100 };
    assert_eq!(auto(Facts { go: Some(&go), ..facts(&l, "4") }), (State::Updating, false));
    let failed = Failed { id: "5".into(), at: 200, why: "the hub died".into(), log: Vec::new() };
    assert!(!auto(Facts { go: Some(&go), my_failed: Some(&failed), ..facts(&l, "4") }).1);
    assert_eq!(auto(facts(&l, "5")), (State::UpToDate, false));
    // off: never
    assert!(!plan(&facts(&l, "4")).unwrap().install_me);
}
