use super::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// A fake world: files with their contents, live pids with their start
/// times, launchd jobs; it counts the costly calls.
#[derive(Default)]
struct Fake {
    files: BTreeMap<String, String>,
    pids: BTreeMap<u32, String>,
    jobs: BTreeMap<String, JobState>,
    calls: RefCell<Vec<String>>,
}

impl Probe for Fake {
    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
    fn head(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }
    fn tail(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }
    fn alive(&self, pid: u32) -> bool {
        self.pids.contains_key(&pid)
    }
    fn start(&self, pid: u32) -> Option<String> {
        self.calls.borrow_mut().push(format!("ps {}", pid));
        self.pids.get(&pid).cloned()
    }
    fn job(&self, label: &str) -> Option<JobState> {
        self.calls.borrow_mut().push(format!("launchctl {}", label));
        self.jobs.get(label).cloned()
    }
}

const ALL: Costly = Costly { start: true, job: true };
const CHEAP: Costly = Costly { start: false, job: false };

fn ended(rc: Option<&str>) -> Seen {
    Seen::Ended { rc: rc.map(String::from) }
}

#[test]
fn a_bash_slot_ends_when_its_rc_appears() {
    let s = Spec::bg("/t/bg/3", "cargo test -p x\n  --lib");
    let mut f = Fake::default();
    f.files.insert("/t/bg/3.pid".into(), "4242".into());
    f.files.insert("/t/bg/3.out".into(), "compiling\n".into());
    f.files.insert("/t/bg/3.slot".into(), String::new());
    assert_eq!(look(&s, &f, CHEAP, None).0, Seen::Running);
    f.files.insert("/t/bg/3.rc".into(), "0\n".into());
    assert_eq!(look(&s, &f, CHEAP, None).0, ended(Some("0")));
    // its files removed by hand: ended, rc unknown
    let g = Fake::default();
    assert_eq!(look(&s, &g, CHEAP, None).0, ended(None));
    assert_eq!(s.label(), "background 3 (`cargo test -p x --lib`)");
    assert_eq!(s.tail.as_deref(), Some("/t/bg/3.out"));
}

#[test]
fn a_pid_ends_when_it_dies_or_another_process_takes_it() {
    let s = Spec { what: What::Pid { pid: 7, start: "Mon 10:00".into() }, tail: None, note: String::new() };
    let mut f = Fake::default();
    f.pids.insert(7, "Mon 10:00".into());
    assert_eq!(look(&s, &f, ALL, None).0, Seen::Running);
    // the cheap look never runs ps
    f.calls.borrow_mut().clear();
    f.pids.insert(7, "Tue 09:00".into());
    assert_eq!(look(&s, &f, CHEAP, None).0, Seen::Running);
    assert!(f.calls.borrow().is_empty());
    assert_eq!(look(&s, &f, ALL, None).0, ended(None));
    f.pids.clear();
    assert_eq!(look(&s, &f, CHEAP, None).0, ended(None));
}

#[test]
fn a_file_ends_when_it_appears_and_says_its_rc() {
    let s = Spec { what: What::File { path: "/t/rc".into() }, tail: Some("/t/log".into()), note: "the gate".into() };
    let mut f = Fake::default();
    assert_eq!(look(&s, &f, CHEAP, None).0, Seen::Running);
    f.files.insert("/t/rc".into(), "2\n".into());
    assert_eq!(look(&s, &f, CHEAP, None).0, ended(Some("2")));
    f.files.insert("/t/rc".into(), "done\n".into());
    assert_eq!(look(&s, &f, CHEAP, None).0, ended(None));
}

#[test]
fn a_job_is_asked_rarely_then_watched_by_its_pid() {
    let s = Spec { what: What::Job { label: "dev.x".into() }, tail: None, note: String::new() };
    let mut f = Fake::default();
    f.jobs.insert("dev.x".into(), JobState { pid: Some(9), last_exit: None });
    f.pids.insert(9, String::new());
    // no launchctl allowed yet: running, nothing spawned
    assert_eq!(look(&s, &f, CHEAP, None), (Seen::Running, None));
    assert!(f.calls.borrow().is_empty());
    let (seen, pid) = look(&s, &f, ALL, None);
    assert_eq!((seen, pid), (Seen::Running, Some(9)));
    // its pid alive: no launchctl even when allowed
    assert_eq!(look(&s, &f, ALL, Some(9)), (Seen::Running, Some(9)));
    assert_eq!(f.calls.borrow().len(), 1);
    // it ended: launchctl gives the rc
    f.pids.clear();
    f.jobs.insert("dev.x".into(), JobState { pid: None, last_exit: Some(1) });
    assert_eq!(look(&s, &f, ALL, Some(9)).0, ended(Some("1")));
    // removed (`launchctl remove` at its end): ended, rc unknown
    f.jobs.clear();
    assert_eq!(look(&s, &f, ALL, None).0, ended(None));
}

#[test]
fn launchctl_list_is_read() {
    let out = "{\n\t\"LimitLoadToSessionType\" = \"Aqua\";\n\t\"Label\" = \"dev.x\";\n\t\"PID\" = 812;\n\t\"LastExitStatus\" = 0;\n};\n";
    assert_eq!(parse_job(out), JobState { pid: Some(812), last_exit: Some(0) });
    assert_eq!(parse_job("{\n\t\"LastExitStatus\" = 256;\n};"), JobState { pid: None, last_exit: Some(256) });
}

#[test]
fn the_tail_is_clipped_by_lines_then_bytes_on_a_char_boundary() {
    let many: String = (1..=50).map(|i| format!("line {}\n", i)).collect();
    let t = tail_clip(&many);
    assert_eq!(t.lines().count(), TAIL_LINES);
    assert!(t.starts_with("line 31") && t.ends_with("line 50"));
    let wide = "é".repeat(3000);
    let t = tail_clip(&wide);
    assert!(t.len() <= TAIL_BYTES + "…".len() + 1);
    assert!(t.starts_with('…') && t.ends_with('é'));
}

#[test]
fn the_wake_says_what_ended_its_rc_and_its_last_lines() {
    let s = Spec::bg("/t/bg/3", "cargo test");
    let w = hit_text(&s, Some("0"), Some("a\nb\nok: 12 passed\n"), 124_000);
    assert_eq!(w, "background 3 ended · rc 0 · after 2m04s · cargo test\nits last 3 lines (/t/bg/3.out):\na\nb\nok: 12 passed");
    assert_eq!(hit_text(&s, None, Some(""), 1_000), "background 3 ended · after 1s · cargo test\nit printed nothing");
    let p = Spec { what: What::Pid { pid: 42, start: String::new() }, tail: None, note: "the build".into() };
    assert_eq!(hit_text(&p, None, None, 5_000), "pid 42 ended · after 5s · the build");
    let f = Spec { what: What::File { path: "/t/rc".into() }, tail: None, note: String::new() };
    assert_eq!(hit_text(&f, Some("1"), None, 3_600_000), "/t/rc appeared · after 1h00m · rc 1");
    let j = Spec { what: What::Job { label: "dev.x".into() }, tail: Some("/t/lo".into()), note: String::new() };
    assert_eq!(hit_text(&j, Some("0"), Some("ok\n"), 723_000), "launchd job dev.x ended · rc 0 · after 12m03s\nits last line (/t/lo):\nok");
    assert_eq!(
        max_text(&p, 3_600_000),
        "pid 42 still running · after 1h00m · the build\nthis watch stopped at its --max (1h). to keep waiting: sb wake --on-exit 42 --note 'the build'"
    );
    let g = Spec { what: What::File { path: "/a b/rc".into() }, tail: Some("/t/log".into()), note: String::new() };
    assert_eq!(again(&g, 90 * 60_000), "sb wake --on-file '/a b/rc' --tail /t/log --max 1h30m");
    assert_eq!(set_text(4, &p, 3_600_000), "watch #4 set: you'll be woken when pid 42 ends (at most 1h00m; sb wake --stop 4)");
}

#[test]
fn a_spec_survives_its_json() {
    for s in [
        Spec::bg("/t/bg/0", "make"),
        Spec { what: What::Pid { pid: 1, start: "x".into() }, tail: Some("/l".into()), note: "n".into() },
        Spec { what: What::File { path: "/f".into() }, tail: None, note: String::new() },
        Spec { what: What::Job { label: "l".into() }, tail: None, note: String::new() },
    ] {
        assert_eq!(Spec::from_json(&s.json()), Some(s));
    }
    assert_eq!(Spec::from_json(&json!({"kind": "nope"})), None);
}

#[test]
fn the_list_shows_the_agents_own_watches() {
    let mut w = Wakes::default();
    let spec = Spec { what: What::Pid { pid: 42, start: String::new() }, tail: None, note: String::new() };
    let v = |id: u64, agent: &str| json!({"id": id, "agent": agent, "spec": spec.json(), "set_at": 0, "max_at": 3_600_000, "quiet": false});
    w.load_live(&json!([v(1, "a"), v(2, "b")]));
    let mut e = v(3, "a");
    e["ended_at"] = json!(1000);
    e["why"] = json!("hit");
    w.load_ended(&json!([e]));
    let l = w.list("a", 60_000);
    assert_eq!(l, "#1 pid 42 · set 1m00s ago · still running at 59m00s\nended:\n  #3 pid 42 · woke you 59s ago");
    assert!(w.list("c", 0).starts_with("no watch"));
    let ids: BTreeSet<u64> = w.live.keys().copied().collect();
    assert_eq!(ids, BTreeSet::from([1, 2]));
}

#[test]
fn a_watch_is_named_for_the_person() {
    assert_eq!(cmd_words("cargo test -p storefront --release"), "cargo test");
    assert_eq!(cmd_words("cd web && npm run dev"), "npm run dev");
    assert_eq!(cmd_words("RUST_LOG=1 ./scripts/gate.sh quick > /tmp/x 2>&1"), "gate.sh quick");
    assert_eq!(cmd_words("sleep 20"), "sleep 20");
    assert_eq!(Spec::bg("/b/3", "cargo test -p x").name(), "cargo test");
    assert_eq!(Spec::bg("/b/3", "").name(), "background 3");
    let mut p = Spec { what: What::Pid { pid: 4242, start: String::new() }, tail: None, note: String::new() };
    assert_eq!((p.name(), p.kind()), ("pid 4242".to_string(), WatchKind::Pid));
    p.note = "the reindex".into();
    assert_eq!(p.name(), "the reindex", "the agent's note first");
    let f = Spec { what: What::File { path: "/w/out/build.rc".into() }, tail: None, note: String::new() };
    assert_eq!((f.name(), f.kind()), ("build.rc".to_string(), WatchKind::File));
    let j = Spec { what: What::Job { label: "dev.x".into() }, tail: None, note: String::new() };
    assert_eq!(j.name(), "launchd job dev.x");
}

#[test]
fn a_watch_end_is_one_typed_line_but_for_an_archived_agent() {
    let w = Watch { id: 7, agent: "perf".into(), spec: Spec::bg("/b/3", "cargo test"), set_at: 1_000, max_at: 0, quiet: true };
    let hit = Hit::of(Some("101\n"), Some("a\nb\nfailed\n"), 190_000);
    assert_eq!((hit.rc, hit.tail.len()), (Some(101), 3));
    let l = end_line(&w, "hit", 9, Some(&hit), Some(12)).unwrap();
    assert_eq!((l.ev, l.what.as_str(), l.rc, l.after_ms, l.msg), (WakeEv::Ended, "cargo test", Some(101), 190_000, Some(12)));
    assert_eq!(end_line(&w, "max", 86_401_000, None, None).unwrap().ev, WakeEv::Expired, "a bash command's day: no wake");
    assert_eq!(end_line(&Watch { quiet: false, ..w.clone() }, "max", 9, None, Some(3)).unwrap().ev, WakeEv::Still);
    let s = end_line(&w, "stopped by perf", 61_000, None, None).unwrap();
    assert_eq!((s.ev, s.after_ms), (WakeEv::Stopped, 60_000));
    assert_eq!(end_line(&w, "gone", 9, None, None), None);
}
