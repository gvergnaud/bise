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
    assert_eq!(w, "background 3 (`cargo test`) ended: rc 0 after 2m04s\nlast lines of /t/bg/3.out:\na\nb\nok: 12 passed");
    let p = Spec { what: What::Pid { pid: 42, start: String::new() }, tail: None, note: "the build".into() };
    assert_eq!(hit_text(&p, None, None, 5_000), "pid 42 ended after 5s · the build");
    let f = Spec { what: What::File { path: "/t/rc".into() }, tail: None, note: String::new() };
    assert_eq!(hit_text(&f, Some("1"), None, 3_600_000), "/t/rc appeared after 1h00m: rc 1");
    assert_eq!(max_text(&p, 3_600_000), "still running after 1h00m: pid 42 · the build. This watch ended: sb wake again to keep waiting");
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
