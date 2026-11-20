//! The worktrees scan through its ports (no git runs): agents' temp
//! worktrees never reach git, a worktree another poll holds keeps its
//! last facts with no git call there.

use super::*;
use std::cell::RefCell;

const LIST: &str = "worktree /r/dash\nHEAD 1111\nbranch refs/heads/main\n\n\
worktree /w/perf/dash\nHEAD 2222\nbranch refs/heads/sb/perf\n\n\
worktree /u/.bise/hubs/dash-0a1b/agents/vb-workflow/tmp/upgrade-wt\nHEAD 3333\ndetached\n\n\
worktree /u/.bise/hubs/dash-0a1b/agents/vb-agent/tmp/nonjson-wt\nHEAD 4444\ndetached\n";

/// A fake git: the list at the repo, a dirty status and 2 ahead in
/// /w/perf, clean elsewhere; each call's folder and args kept.
struct FakeGit(RefCell<Vec<(String, String)>>);

impl FakeGit {
    fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
        let (d, a) = (dir.to_string_lossy().to_string(), args.join(" "));
        self.0.borrow_mut().push((d.clone(), a.clone()));
        Some(match (d.as_str(), args[0]) {
            (_, "worktree") => LIST.to_string(),
            ("/w/perf/dash", "status") => " M src/a.rs".into(),
            ("/w/perf/dash", "rev-list") => "0\t2".into(),
            (_, "rev-list") => "0\t0".into(),
            _ => String::new(),
        })
    }
    fn dirs(&self, cmd: &str) -> Vec<String> {
        self.0.borrow().iter().filter(|(_, a)| a.starts_with(cmd)).map(|(d, _)| d.clone()).collect()
    }
}

fn home() -> bise_home::Home {
    bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| "/u".to_string()))
}

#[test]
fn agents_temp_worktrees_never_reach_git_nor_the_rows() {
    let (g, h, slots, last) = (FakeGit(RefCell::default()), home(), Slots::default(), HashMap::new());
    let git = |d: &Path, a: &[&str]| g.run(d, a);
    let tmp = |p: &Path| bise_home::hub_state::is_agent_tmp(&h, p);
    let (rows, facts) = scan(Path::new("/r/dash"), "main", &[], &Ports { git: &git, tmp: &tmp, slots: &slots, last: &last });
    assert_eq!(rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/r/dash", "/w/perf/dash"]);
    assert_eq!(g.dirs("status"), ["/r/dash", "/w/perf/dash"], "no status in an agent's tmp");
    assert!(g.0.borrow().iter().all(|(d, _)| !d.contains("/tmp/")), "{:?}", g.0.borrow());
    assert_eq!(facts.get("/w/perf/dash"), Some(&(true, 2, 0)));
    assert_eq!(facts.len(), 2);
}

#[test]
fn a_worktree_another_poll_holds_keeps_its_last_facts_with_no_git_there() {
    let (g, h, slots) = (FakeGit(RefCell::default()), home(), Slots::default());
    let last = HashMap::from([("/w/perf/dash".to_string(), (true, 5, 1))]);
    let git = |d: &Path, a: &[&str]| g.run(d, a);
    let tmp = |p: &Path| bise_home::hub_state::is_agent_tmp(&h, p);
    let changes = slots.claim(Path::new("/w/perf/dash")).expect("an agent's Changes runs there");
    let (rows, facts) = scan(Path::new("/r/dash"), "main", &[], &Ports { git: &git, tmp: &tmp, slots: &slots, last: &last });
    assert_eq!(g.dirs("status"), ["/r/dash"], "no second git status in a held worktree");
    assert!(g.dirs("rev-list").iter().all(|d| d != "/w/perf/dash"));
    let perf = rows.iter().find(|r| r.path == "/w/perf/dash").expect("its row stays");
    assert_eq!((perf.dirty, perf.ahead, perf.behind), (true, 5, 1), "its last facts, never clean");
    assert_eq!(facts.get("/w/perf/dash"), Some(&(true, 5, 1)));
    drop(changes);
    assert!(slots.claim(Path::new("/r/dash")).is_some(), "the scan freed what it held");
}
