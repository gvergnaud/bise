//! The core moves an older home hub to its own version (architect
//! m_15476, core/home_switch.rs): a fake older home hub refuses
//! `initialize` the way accept.rs did before client-protocol, the move
//! port is a fake whose end the test says, then a new hub answers.

use super::*;
use crate::ambient::MoveEnd;

/// The older door's answer to a line it doesn't serve (accept.rs before
/// client-protocol), then the connection closes.
fn refuse_older(hub: &mut HubEnd) {
    let first = hub.raw(Duration::from_secs(3)).expect("the core's first line");
    assert_eq!(first["method"], "initialize", "{first}");
    hub.write(json!({"ok": false, "error": "hub.sock does not serve the op \"\""}));
    let _ = hub.w.shutdown(std::net::Shutdown::Both);
}

/// The core's next connection, its Up handled (its `initialize` sent).
fn next_conn(t: &mut T) -> HubEnd {
    let end = t.ends.recv_timeout(Duration::from_secs(3)).expect("the core never connected again");
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(200) {
        while let Ok(h) = t.rx.try_recv() {
            t.core.hub(h);
        }
        t.out.extend(t.core.take_out());
        std::thread::sleep(Duration::from_millis(5));
    }
    end
}

/// A core whose move port counts its starts; its first hub older.
fn older_home() -> (T, Arc<AtomicUsize>) {
    let mut t = T::new();
    let starts = Arc::new(AtomicUsize::new(0));
    let s = starts.clone();
    t.core.set_home_move(
        Box::new(|| ("v2026.10.2-28".to_string(), "v2026.10.2-30".to_string())),
        Box::new(move || {
            s.fetch_add(1, Ordering::SeqCst);
        }),
    );
    t.take();
    refuse_older(&mut t.hub);
    t.until(|o| o.iter().any(|v| v["ev"] == "hub" && v.get("note").is_some()));
    (t, starts)
}

fn home() -> String {
    bise_home::hub_id(Path::new(WS))
}

#[test]
fn an_older_home_hub_is_moved_then_the_window_gets_home_with_no_refusal() {
    let (mut t, starts) = older_home();
    assert_eq!(starts.load(Ordering::SeqCst), 1, "the move started once");
    let notes: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "hub").collect();
    assert_eq!(
        notes,
        [json!({"ev": "hub", "up": false, "workspace": WS, "project": home(), "note": "updating the bise running here to v2026.10.2-30… your agents keep running."})]
    );
    // while it moves, the older hub is reached again: no second move, no
    // `hub` up that would clear the note, no refusal
    let mut again = next_conn(&mut t);
    refuse_older(&mut again);
    let mut late = next_conn(&mut t);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(t.out.iter().all(|v| v["ev"] != "hub" && v["ev"] != "hub_refused"), "{:#?}", t.out);
    // the move ends: the next connection says initialize to the new hub
    t.core.home_moved(MoveEnd::There);
    refuse_older(&mut late);
    t.hub = next_conn(&mut t);
    assert!(t.out.iter().any(|v| *v == json!({"ev": "hub", "up": true, "workspace": WS})), "the note's end: {:#?}", t.out);
    t.ready();
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "idle"}], "cards": []}));
    t.until(|o| has(o, "state"));
    assert!(t.out.iter().all(|v| v["ev"] != "hub_refused"), "{:#?}", t.out);
}

#[test]
fn a_failed_move_shows_the_refusal_once_and_try_again_moves_it_again() {
    let (mut t, starts) = older_home();
    t.take();
    t.core.home_moved(MoveEnd::Refused("a switch is already running".into()));
    t.out.extend(t.core.take_out());
    let refused: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "hub_refused").collect();
    assert_eq!(
        refused,
        [json!({"ev": "hub_refused", "project": home(), "error": "the bise running in this folder is older (v2026.10.2-28) and couldn't switch to this one (v2026.10.2-30).\na switch is already running\nyour agents are still running on v2026.10.2-28."})]
    );
    // the older hub is reached again: the refusal isn't said twice
    let mut again = next_conn(&mut t);
    refuse_older(&mut again);
    let mut more = next_conn(&mut t);
    assert!(t.out.iter().all(|v| v["ev"] != "hub_refused" && v["ev"] != "hub"), "{:#?}", t.out);
    assert_eq!(starts.load(Ordering::SeqCst), 1, "no move without him");
    // his 'try again': the move again, its note again
    t.cmd(Cmd::HubRetry { project: home() });
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert!(t.out.iter().any(|v| v["ev"] == "hub" && v["note"].as_str().is_some_and(|n| n.starts_with("updating the bise running here"))), "{:#?}", t.out);
    // it times out this time: the timed-out words
    t.take();
    t.core.home_moved(MoveEnd::Late);
    t.out.extend(t.core.take_out());
    let late: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "hub_refused").collect();
    assert_eq!(late.len(), 1);
    assert!(late[0]["error"].as_str().unwrap().starts_with("the bise running in this folder is older (v2026.10.2-28) and didn't switch to this one (v2026.10.2-30) within 30 s."), "{late:?}");
    refuse_older(&mut more);
}
