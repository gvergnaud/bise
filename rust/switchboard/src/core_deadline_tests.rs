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
/// A step's limit, at load 1 whatever the box's load: the deadline
/// tests' limits are short on purpose, scaled by a loaded tip they
/// passed lines that came late.
fn limits(step: Duration) -> Limits {
    Limits { step, load: Some(1.0), ..Limits::default() }
}

fn fake_hub() -> (T, Rc<RefCell<Vec<String>>>) {
    fake_hub_with(0.0)
}

/// `fake_hub`, the lines of an answer in lines `line_delay` s apart.
fn fake_hub_with(line_delay: f64) -> (T, Rc<RefCell<Vec<String>>>) {
    let real = bise_home::env::test_setting("SB_CORE_BIN").map(PathBuf::from).unwrap_or_else(crate::core::default_core_bin);
    // the interpreter itself for the #! line, never `command -v python3`: a pyenv shim is a bash
    // script, and macOS won't take a script as a #! interpreter (the fake ran under sh, no
    // banner: 'sb-core did not start: ""', pr-guided-build m_58429)
    let python = std::process::Command::new("python3").args(["-c", "import sys; print(sys.executable)"]).output().expect("python3");
    let python = String::from_utf8_lossy(&python.stdout).trim().to_string();
    assert!(!python.is_empty(), "python3 not found");
    let script = include_str!("testdata/fake_sb_core.py").replace("@REAL@", &real.display().to_string()).replace("@LINE_DELAY@", &line_delay.to_string());
    let dir = std::env::temp_dir().join(format!("fake-sb-core-{}-{}", std::process::id(), FAKES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("sb-core");
    std::fs::write(&bin, format!("#!{}\n{}", python, script)).unwrap();
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut hub = Hub::with_core("/w", bin).unwrap_or_else(|e| panic!("the fake sb-core: {}", e));
    hub.set_core_limits(limits(Duration::from_millis(300)));
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

/// A hub whose view_all answers 3 lines (c6 (c)): its head, a line of
/// archived agents (b), a line of cold rows (600 old messages, the
/// newest 256 sent whole).
fn three_line_view(t: &mut T) {
    let ws = json!({"mode": "shared", "path": "/w", "branch": null, "base_commit": null, "dropped": false});
    let mut ev = vec![
        json!({"type": "task_created", "name": "a", "parent": "main", "ws": ws, "at_ms": 0, "brief": {"objective": "fake"}}),
        json!({"type": "task_created", "name": "b", "parent": "main", "ws": ws, "at_ms": 0, "brief": {"objective": "fake"}}),
        json!({"type": "lifecycle", "name": "b", "lifecycle": "archived", "reason": "drop"}),
    ];
    for id in 1..=600u64 {
        ev.push(json!({"type": "message_sent", "msg": {"id": id, "thread": id, "from": "main", "to": "a", "reply_to": null,
            "expect_reply": false, "auto": false, "text": format!("fake {}", id), "created_ms": 1000 + id, "plain": false, "parts": ""}}));
        ev.push(json!({"type": "message_state", "id": id, "state": "delivered"}));
    }
    assert!(t.hub.replay(&ev).is_empty());
}

/// c6 (c): view_all's lines share the one limit of the input, sized as
/// view_all's own (scaled by the load): a slow sb-core whose lines come
/// 0.1 s apart is read whole, every line loaded into the mirror.
#[test]
fn a_slow_answer_in_lines_is_read_whole_within_its_limit() {
    let (mut t, _) = fake_hub_with(0.1);
    t.hub.set_core_limits(limits(Duration::from_secs(5)));
    three_line_view(&mut t);
    let t0 = Instant::now();
    t.hub.try_view_all().unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(200), "3 lines, 2 delays: {:?}", t0.elapsed());
    assert_eq!(t.hub.st.peeks().count(), 600, "every row, the cold ones of the last line too");
    assert!(t.hub.st.agents.contains_key("b"), "the archived one of the second line");
    assert_eq!(t.hub.st.whole_msgs().count(), 256);
    still_works(&mut t);
}

/// ... and the limit is the whole answer's, not each line's: lines that
/// each come within it but together past it make sb-core late. The
/// mirror loads only the whole answer (architect m_56956 (a)): late on
/// its last line, it holds none of the lines before.
#[test]
fn an_answer_in_lines_past_its_limit_is_late_though_each_line_is_not() {
    let (mut t, _) = fake_hub_with(0.25);
    t.hub.set_core_limits(limits(Duration::from_secs(5)));
    three_line_view(&mut t);
    t.hub.st.clear_msgs();
    t.hub.set_core_limits(limits(Duration::from_millis(400)));
    match t.hub.try_view_all() {
        Err(CoreError::Late { what, .. }) => assert_eq!(what, "view_all"),
        other => panic!("late, not {:?}", other.map(|_| ())),
    }
    assert_eq!(t.hub.st.peeks().count(), 0, "the head line's 256 whole messages were never loaded alone");
}
