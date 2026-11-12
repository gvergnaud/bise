//! daemon.rs' unit tests: hub-fifo's urgent lane, a stop ahead of queued
//! REPL lines, the live lines' time (the journal's: daemon/journal_file.rs).
use super::*;

/// hub-fifo's urgent lane holds a client's own messages, client-protocol's
/// too: its `initialize` connection (RpcNew) and a thread's typed answer
/// to its request (Typed); the rest waits its turn.
#[test]
fn a_clients_own_messages_take_the_urgent_lane() {
    let (a, _b) = UnixStream::pair().unwrap();
    assert!(Msg::is_urgent(&Msg::RpcNew { id: 1, stream: a, v: Value::Null, peer: String::new() }));
    assert!(Msg::is_urgent(&Msg::Typed { to: rpc::Typed::Answer(1), v: Value::Null }));
    assert!(Msg::is_urgent(&Msg::ClientGone { id: 1 }));
    assert!(!Msg::is_urgent(&Msg::input(Input::Tick)));
}

/// 2026-10-11, his hub: `bise switchboard --stop` at 22:18:16, its
/// Shutdown queued behind ~85 s of REPL lines, the client said
/// "stopped", bise-dev attached again, and the stop fired at 22:19:41
/// under him (main and 7 busy agents SIGTERMed). A stop, keep or not,
/// is the loop's next message whatever waits in the rest lane.
#[test]
fn a_stop_goes_ahead_of_queued_repl_lines() {
    use super::inbox::{Inbox, Next};
    let (tx, rx) = std::sync::mpsc::channel();
    let line = |n: u64| Msg::ReplLine { dir: "amb-win".into(), gen: 1, line: format!("--- line {n}"), offset: n };
    for n in 0..5000 {
        tx.send(line(n)).unwrap();
    }
    tx.send(Msg::Shutdown { keep: false }).unwrap();
    tx.send(line(5000)).unwrap();
    let mut inbox = Inbox::new(rx, Msg::is_urgent);
    match inbox.next(Some(Duration::ZERO)) {
        Next::Msg(Msg::Shutdown { keep }) => assert!(!keep),
        Next::Msg(_) => panic!("a REPL line went before the stop"),
        _ => panic!("nothing in the inbox"),
    }
    assert!(Msg::is_urgent(&Msg::Shutdown { keep: true }));
}

/// A live `line` carries its transcript time as `ts` too (BISE-271:
/// the turns' end times, the pause marks of a replayed feed); an
/// unknown time (0) is left out.
#[test]
fn live_lines_carry_their_time() {
    assert_eq!(
        lines::line_event("main", 3, 1700000000000, "  obs: turn_done: completed"),
        json!({"ev": "line", "agent": "main", "line": "  obs: turn_done: completed", "pos": 3, "ts": 1700000000000u64})
    );
    assert_eq!(lines::line_event("t1", 1, 0, "x"), json!({"ev": "line", "agent": "t1", "line": "x", "pos": 1}));
}

/// architect m_38142: two hubs of one workspace that start at once. The
/// first one claims it (hub.root written before its socket is up, the
/// tip red of older_tui_bundle_hub_tmux on 8c3644957: hub.root read once
/// the socket appeared, 16 starts of 20 too early); the second one finds
/// that socket and leaves the first one's hub.root as it was.
#[test]
fn a_second_hub_leaves_the_first_ones_hub_root() {
    let t = std::env::temp_dir().join(format!("sb-claim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    let paths = Paths { workspace: t.join("ws"), state: t.join("st"), worktrees: t.join("wt"), run: t.join("run") };
    std::fs::create_dir_all(&paths.state).unwrap();
    paths.prepare_socket().unwrap();
    let first = claim(&paths, &t.join("v1")).unwrap().expect("the first hub claims the workspace");
    assert!(UnixStream::connect(paths.socket()).is_ok());
    let root = || std::fs::read_to_string(paths.state.join("hub.root")).unwrap();
    assert_eq!(root(), t.join("v1").to_string_lossy());
    assert!(claim(&paths, &t.join("v2")).unwrap().is_none(), "a second hub claimed it too");
    assert_eq!(root(), t.join("v1").to_string_lossy(), "the second hub wrote its hub.root");
    drop(first);
    let _ = std::fs::remove_dir_all(&t);
}

/// marketing m_59649: a start that queued on hub.bind.lock during a
/// take-back found the new hub answering, and still waited 15 s for it to
/// stop (wait_previous_hub on its live pid): two sbd for the folder. A hub
/// that answers is the live one: the start gives way at once.
#[test]
fn a_start_gives_way_at_once_to_a_hub_that_answers() {
    let t = std::env::temp_dir().join(format!("sb-claim-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    let paths = Paths { workspace: t.join("ws"), state: t.join("st"), worktrees: t.join("wt"), run: t.join("run") };
    std::fs::create_dir_all(&paths.state).unwrap();
    paths.prepare_socket().unwrap();
    let first = claim(&paths, &t.join("v1")).unwrap().expect("the first hub claims the workspace");
    // hub.pid names a live process that isn't this one (the new hub)
    let mut live = std::process::Command::new("sleep").arg("30").spawn().unwrap();
    std::fs::write(paths.pid_file(), live.id().to_string()).unwrap();
    let t0 = std::time::Instant::now();
    assert!(claim(&paths, &t.join("v2")).unwrap().is_none(), "a second hub claimed it");
    assert!(t0.elapsed() < Duration::from_secs(2), "it waited {:?} for the live hub to stop", t0.elapsed());
    let _ = live.kill();
    let _ = live.wait();
    drop(first);
    let _ = std::fs::remove_dir_all(&t);
}

/// A switch's reload keeps its port only while it is free: the 11:01
/// switch respawned changelog on the port its old REPL still held, the
/// new one died at startup (exit 48, EADDRINUSE) and rolled it back.
#[test]
fn a_held_port_is_not_free() {
    let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = l.local_addr().unwrap().port();
    assert!(!port_free(port));
    drop(l);
    assert!(port_free(port));
}

/// A hub never adopts a REPL whose say port opens in a newer way than
/// its own: 11:02, a pre-hello hub adopted four hello-only REPLs and
/// their messages sat unread for 2.5 h. Same or older (no `say`: a
/// REPL from before the field): adopted as before.
#[test]
fn a_repl_of_a_newer_say_port_is_too_new() {
    let say = repl::SAY_PROTO;
    assert_eq!(repl::too_new(&json!({"pid": 1, "say": say + 1})), Some(say + 1));
    assert_eq!(repl::too_new(&json!({"pid": 1, "say": say})), None);
    assert_eq!(repl::too_new(&json!({"pid": 1, "token": "t"})), None);
    assert_eq!(repl::too_new(&json!({"pid": 1})), None);
}
