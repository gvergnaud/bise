//! The worker on a throwaway hub folder: a bg slot's files, the ended
//! log, the order of a reap (architect m_38423).

use super::*;
use bise_proto::rows::ProcState;

/// A pid no process has (above macOS's and Linux's default pid_max).
const GONE: u32 = 4_000_000;

fn state(dir: &Path) -> (State, Receiver<Msg>) {
    let paths = Paths { workspace: dir.join("ws"), state: dir.join("hub"), worktrees: dir.join("wt"), run: dir.join("run") };
    let (out, rx) = channel();
    let (me, _) = channel();
    let mut s = State::new(paths, "aa".into(), "acme".into(), out, me);
    s.agents = vec![Live { name: "perf".into(), dir: "perf".into() }];
    (s, rx)
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-procs-{}-{}-{}", name, std::process::id(), now_ms()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// perf's bg job 3 as the last scan saw it running; its slot's .out
/// (and .rc when given) on disk.
fn running_bg(s: &mut State, rc: Option<&str>) -> Key {
    let bg = s.paths.agent_bg("perf");
    std::fs::create_dir_all(&bg).unwrap();
    std::fs::write(bg.join("3.out"), "VITE ready\nlistening on 5173\n").unwrap();
    if let Some(rc) = rc {
        std::fs::write(bg.join("3.rc"), rc).unwrap();
    }
    let row = Proc {
        id: "bg3.1791100100".into(),
        agent: "perf".into(),
        kind: ProcKind::Bg { n: 3 },
        cmd: "pnpm dev".into(),
        cwd: None,
        pid: GONE,
        started_ms: 1_791_100_100_000,
        ports: vec![5173],
        url: Some("http://127.0.0.1:5173".into()),
        name: Some("dev server".into()),
        state: bise_proto::rows::ProcState::Running,
        ended_ms: None,
        cpu: 3.5,
        rss_mb: 300,
        procs: 2,
        out: OutKind::Bg,
        fd1: None,
    };
    let k = (row.agent.clone(), row.id.clone());
    s.running.insert(k.clone(), Seen { row, dir: "perf".into(), src: Src::File(bg.join("3.out")), bg: Some(bg), pids: vec![GONE] });
    k
}

#[test]
fn a_bg_job_that_ended_keeps_its_rc_and_its_output_a_day() {
    let d = tmp("exit");
    let (mut s, rx) = state(&d);
    running_bg(&mut s, Some("2\n"));
    s.scan(false);
    assert!(s.running.is_empty());
    assert_eq!(s.ended.len(), 1);
    let e = &s.ended[0];
    assert_eq!((e.state, e.out, e.procs, e.ended_ms.is_some()), (ProcState::Exited { rc: Some(2) }, OutKind::Bg, 0, true));
    // the ended log and the output copy survive a new worker (a hub reload)
    let (s2, _) = state(&d);
    assert_eq!(s2.ended, s.ended);
    let copy = std::fs::read_to_string(s2.paths.procs_dir().join("perf-bg3.1791100100.out")).unwrap();
    assert_eq!(copy, "VITE ready\nlistening on 5173\n");
    // the count went to 0: the agents rows hear it
    assert!(rx.try_iter().any(|m| matches!(m, Msg::ProcCounts(c) if c.is_empty())) || s.counts.is_empty());
    // a day later the row and its copy are gone
    let mut s2 = s2;
    s2.prune(now_ms() + pv::ENDED_KEEP_MS + 1);
    assert!(s2.ended.is_empty());
    assert!(!s2.paths.procs_dir().join("perf-bg3.1791100100.out").exists());
    assert_eq!(read_ended(&ended_file(&s2.paths)), Vec::<Proc>::new());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_reap_then_its_kill_then_a_scan_is_stopped_with_its_agent_never_exited() {
    let d = tmp("reap");
    let (mut s, _rx) = state(&d);
    running_bg(&mut s, Some("143\n"));
    // the loop sends Reaping before proc-cleanup kills; the kill (the
    // process is gone: GONE), then the next scan
    s.reaping(Some(&["perf".to_string()].into()));
    s.scan(false);
    let states: Vec<ProcState> = s.ended.iter().map(|p| p.state).collect();
    assert_eq!(states, [ProcState::StoppedWith], "once, never exited");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn his_stop_marks_the_row_before_it_goes() {
    let d = tmp("stop");
    let (mut s, _rx) = state(&d);
    let k = running_bg(&mut s, Some("143\n"));
    s.stopping.insert(k);
    s.scan(false);
    assert_eq!(s.ended[0].state, ProcState::StoppedByYou);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_row_whose_agent_is_no_longer_live_went_with_it() {
    let d = tmp("gone");
    let (mut s, _rx) = state(&d);
    running_bg(&mut s, None);
    s.agents.clear();
    s.scan(false);
    assert_eq!(s.ended[0].state, ProcState::StoppedWith);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn an_output_is_read_by_range_and_a_follow_starts_where_the_last_read_ended() {
    let d = tmp("out");
    let (mut s, rx) = state(&d);
    let k = running_bg(&mut s, None);
    s.out_range(7, k.0.clone(), k.1.clone(), Some(5));
    let got: Vec<HubEv> = rx.try_iter().filter_map(|m| match m {
        Msg::Typed { v, .. } => HubEv::from_value(v).ok(),
        _ => None,
    }).collect();
    let [HubEv::ProcOut { at, data, end, .. }] = got.as_slice() else { panic!("{got:?}") };
    assert_eq!((*at, data.as_str(), *end), (5, b64(b"ready\nlistening on 5173\n").as_str(), 29));
    s.on(Ask::Follow { client: 7, agent: k.0.clone(), id: k.1.clone(), on: true });
    assert_eq!(s.followers.values().copied().collect::<Vec<_>>(), [29], "no gap after the read");
    note_line(&s.paths.agent_bg("perf").join("3.out"), pv::STOPPED_NOTE);
    s.follow_tick();
    let more: Vec<HubEv> = rx.try_iter().filter_map(|m| match m {
        Msg::Typed { v, .. } => HubEv::from_value(v).ok(),
        _ => None,
    }).collect();
    let [HubEv::ProcOut { at, data, .. }] = more.as_slice() else { panic!("{more:?}") };
    assert_eq!((*at, data.as_str()), (29, b64(format!("{}\n", pv::STOPPED_NOTE).as_bytes()).as_str()));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn base64_is_the_standard_one() {
    assert_eq!(b64(b""), "");
    assert_eq!(b64(b"f"), "Zg==");
    assert_eq!(b64(b"fo"), "Zm8=");
    assert_eq!(b64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn an_unknown_process_is_an_error_to_that_client() {
    let d = tmp("unknown");
    let (mut s, rx) = state(&d);
    s.out_range(3, "perf".into(), "bg9.1".into(), None);
    s.stop(3, "perf", Some("bg9.1"));
    let errs = rx.try_iter().filter(|m| matches!(m, Msg::Typed { v, .. } if v["ev"] == "error")).count();
    assert_eq!(errs, 2);
    let _ = std::fs::remove_dir_all(d);
}
