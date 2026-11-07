//! audit-hub finding 1: sb-core never holds the hub nor kills it. A fake
//! sb-core (testdata/fake_sb_core.py: the real one behind a proxy that
//! misbehaves on a marker) sleeps, answers `{error}` or a broken line;
//! the hub drops that input, says it, and keeps working. Short limits
//! (300 ms): the three run in about a second.

use super::super::*;
use crate::core_proc::Limits;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// A hub on the fake sb-core, its revival on the test's journal, its
/// hub.log lines in the returned list.
fn fake_hub() -> (T, Rc<RefCell<Vec<String>>>) {
    let real = bise_home::env::test_setting("SB_CORE_BIN").map(PathBuf::from).unwrap_or_else(crate::core::default_core_bin);
    let python = std::process::Command::new("sh").args(["-c", "command -v python3"]).output().expect("sh");
    let python = String::from_utf8_lossy(&python.stdout).trim().to_string();
    assert!(!python.is_empty(), "python3 not found");
    let script = include_str!("testdata/fake_sb_core.py").replace("@REAL@", &real.display().to_string());
    let dir = std::env::temp_dir().join(format!("fake-sb-core-{}-{}", std::process::id(), FAKES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("sb-core");
    std::fs::write(&bin, format!("#!{}\n{}", python, script)).unwrap();
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut hub = Hub::with_core("/w", bin).unwrap_or_else(|e| panic!("the fake sb-core: {}", e));
    hub.set_core_limits(Limits { step: Duration::from_millis(300), ..Limits::default() });
    let mut t = T::with_hub(hub);
    let journal = t.journal.clone();
    let logged = Rc::new(RefCell::new(Vec::<String>::new()));
    let log = logged.clone();
    t.hub.set_revive(Revive::new(Box::new(move || journal.borrow().clone()), Box::new(move |s| log.borrow_mut().push(s.to_string()))));
    (t, logged)
}

static FAKES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn warns(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Line { agent, line } if agent == MAIN && line.starts_with("sb warn :: ") => Some(line.clone()),
            _ => None,
        })
        .collect()
}

/// After a misbehaving input: the next one runs as usual.
fn still_works(t: &mut T) {
    let fx = t.user(MAIN, "hello again");
    assert!(say_to(&fx, MAIN).is_some_and(|s| s.contains("hello again")), "{:?}", fx);
    assert_eq!(t.hub.gave_up(), None);
}

/// A step sb-core holds past its limit (a Tick held his hub 38 min):
/// killed and restarted on the journal at the limit, the input dropped,
/// main's feed says it once, the state is back.
#[test]
fn a_late_sb_core_is_killed_and_restarted_at_its_limit() {
    let (mut t, logged) = fake_hub();
    t.spawn_task("t1");
    let t0 = Instant::now();
    let fx = t.user(MAIN, "fake:sleep");
    assert!(t0.elapsed() < Duration::from_secs(5), "held the hub {:?}", t0.elapsed());
    assert!(say_to(&fx, MAIN).is_none(), "the late input was dropped: {:?}", fx);
    let w = warns(&fx);
    assert_eq!(w.len(), 1, "one warning per restart: {:?}", w);
    assert!(w[0].contains("restarted") && w[0].contains("no answer to"), "{}", w[0]);
    assert!(logged.borrow().iter().any(|l| l.contains("sb-core restarted")), "{:?}", logged.borrow());
    assert!(t.hub.st.agents.contains_key("t1"), "the state rebuilt from the journal");
    still_works(&mut t);
}

/// sb-core answers `{error}`: the input is dropped, never a panic;
/// hub.log has each, main's feed the first of a run.
#[test]
fn an_error_answer_drops_the_input_without_a_panic() {
    let (mut t, logged) = fake_hub();
    let fx = t.user(MAIN, "fake:error");
    let w = warns(&fx);
    assert!(w.len() == 1 && w[0].contains("could not run an input"), "{:?}", fx);
    let fx = t.user(MAIN, "fake:error again");
    assert!(warns(&fx).is_empty(), "no flood: {:?}", fx);
    let errors = logged.borrow().iter().filter(|l| l.contains("fake refused it")).count();
    assert_eq!(errors, 2, "{:?}", logged.borrow());
    // a request refused gets its reply, never silence
    let (tok, fx) = t.req(MAIN, AgentReq::Status { status: Declared::Working, note: "fake:error".into() });
    assert_eq!(reply(&fx, tok).map(|b| b["ok"].clone()), Some(json!(false)), "{:?}", fx);
    still_works(&mut t);
}

/// An answer that is not JSON: a death (restarted), the input dropped
/// after its one retry, never a panic.
#[test]
fn a_broken_answer_line_is_a_death_not_a_panic() {
    let (mut t, logged) = fake_hub();
    let fx = t.user(MAIN, "fake:garbage");
    assert!(say_to(&fx, MAIN).is_none(), "{:?}", fx);
    assert!(warns(&fx).iter().any(|l| l.contains("a bad answer")), "{:?}", fx);
    assert!(logged.borrow().iter().any(|l| l.contains("sb-core restarted")), "{:?}", logged.borrow());
    still_works(&mut t);
}
