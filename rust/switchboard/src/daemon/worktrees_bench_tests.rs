//! The before/after measure of git-calm (not a check: `#[ignore]`, run
//! by hand through `heavy-run --low`): a fixture repo with 27 worktrees
//! and 8 agents' temp worktrees, real git, each `git status` made 300 ms
//! slower (the monorepo under load); 30 agent status flips, 100 ms apart.
//! Before: a scan thread per flip, every listed worktree, no slot (the
//! old `worktrees_typed`). After: the hub's `Scans` and `Slots`, the
//! temp worktrees left out. Counted: `git status` runs, the most at once
//! in one worktree and in all.
//!
//! cd rust && cargo test -q -p switchboard --lib worktrees_bench -- --ignored --nocapture

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WORKTREES: usize = 27;
const TMP: usize = 8;
const FLIPS: usize = 30;
const SLOW: Duration = Duration::from_millis(300);

#[derive(Default)]
struct Count {
    total: AtomicUsize,
    now: Mutex<(HashMap<String, usize>, usize)>,
    peak_one: AtomicUsize,
    peak_all: AtomicUsize,
}

impl Count {
    fn git(&self, dir: &Path, args: &[&str]) -> Option<String> {
        if args[0] != "status" {
            return crate::worktree::git(dir, args).ok();
        }
        let d = dir.to_string_lossy().to_string();
        {
            let mut n = self.now.lock().unwrap();
            let one = {
                let e = n.0.entry(d.clone()).or_default();
                *e += 1;
                *e
            };
            n.1 += 1;
            self.peak_one.fetch_max(one, Ordering::SeqCst);
            self.peak_all.fetch_max(n.1, Ordering::SeqCst);
        }
        self.total.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(SLOW);
        let out = crate::worktree::git(dir, args).ok();
        let mut n = self.now.lock().unwrap();
        *n.0.get_mut(&d).unwrap() -= 1;
        n.1 -= 1;
        out
    }
    fn report(&self, what: &str, took: Duration) -> String {
        format!(
            "{what}: {} git status, at most {} at once in one worktree, {} at once in all, {:.1} s until quiet",
            self.total.load(Ordering::SeqCst),
            self.peak_one.load(Ordering::SeqCst),
            self.peak_all.load(Ordering::SeqCst),
            took.as_secs_f64()
        )
    }
}

/// The fixture: a repo, its worktrees, the agents' temp ones under a
/// fake home's hubs; (repo, home).
fn fixture(root: &Path) -> (PathBuf, bise_home::Home) {
    let g = |dir: &Path, args: &[&str]| crate::worktree::git(dir, args).unwrap();
    let repo = root.join("dash");
    std::fs::create_dir_all(&repo).unwrap();
    g(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    g(&repo, &["add", "."]);
    g(&repo, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a"]);
    for i in 0..WORKTREES {
        let p = root.join(format!("wt/w{i}"));
        g(&repo, &["worktree", "add", "-q", "-b", &format!("sb/w{i}"), &p.to_string_lossy()]);
    }
    for i in 0..TMP {
        let p = root.join(format!("u/.bise/hubs/dash-0a1b/agents/a{i}/tmp/wt"));
        g(&repo, &["worktree", "add", "-q", "--detach", &p.to_string_lossy()]);
    }
    let user = root.join("u").to_string_lossy().to_string();
    (repo, bise_home::Home::from_lookup(&move |k: &str| (k == "HOME").then(|| user.clone())))
}

fn before(repo: &Path, c: &Arc<Count>) -> Duration {
    let start = Instant::now();
    let mut threads = Vec::new();
    for _ in 0..FLIPS {
        let (repo, c) = (repo.to_path_buf(), c.clone());
        threads.push(std::thread::spawn(move || {
            let git = |d: &Path, a: &[&str]| c.git(d, a);
            let (no_tmp, own, last) = (|_: &Path| false, Slots::default(), HashMap::new());
            scan(&repo, "main", &[], &Ports { git: &git, tmp: &no_tmp, slots: &own, last: &last });
        }));
        std::thread::sleep(Duration::from_millis(100));
    }
    threads.into_iter().for_each(|t| t.join().unwrap());
    start.elapsed()
}

fn after(repo: &Path, home: &bise_home::Home, c: &Arc<Count>) -> Duration {
    let start = Instant::now();
    let (slots, gap) = (Slots::default(), crate::git_poll::SCAN_GAP);
    let mut scans: Scans<()> = Scans::default();
    let (tx, rx) = std::sync::mpsc::channel::<HashMap<String, Facts>>();
    let ms = |t: Instant| t.duration_since(start).as_millis() as u64;
    let run = |scans: &Scans<()>| {
        let (repo, home, c, slots, last, tx) = (repo.to_path_buf(), home.clone(), c.clone(), slots.clone(), scans.last().clone(), tx.clone());
        std::thread::spawn(move || {
            let git = |d: &Path, a: &[&str]| c.git(d, a);
            let tmp = |p: &Path| bise_home::hub_state::is_agent_tmp(&home, p);
            let (_, facts) = scan(&repo, "main", &[], &Ports { git: &git, tmp: &tmp, slots: &slots, last: &last });
            let _ = tx.send(facts);
        });
    };
    let (mut flips, mut next_flip, mut running) = (0, start, false);
    loop {
        let now = Instant::now();
        if flips < FLIPS && now >= next_flip {
            flips += 1;
            next_flip += Duration::from_millis(100);
            if scans.ask((), ms(now), gap).is_some() {
                running = true;
                run(&scans);
            }
        }
        if let Ok(facts) = rx.recv_timeout(Duration::from_millis(20)) {
            scans.done(ms(Instant::now()), facts);
            running = false;
        }
        if scans.due(ms(Instant::now()), gap).is_some() {
            running = true;
            run(&scans);
        }
        if flips == FLIPS && !running && !scans.waiting() {
            return start.elapsed();
        }
    }
}

#[test]
#[ignore]
fn worktrees_bench() {
    let root = std::env::temp_dir().join(format!("git-calm-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (repo, home) = fixture(&root);
    let (b, a) = (Arc::new(Count::default()), Arc::new(Count::default()));
    let tb = before(&repo, &b);
    let ta = after(&repo, &home, &a);
    println!("{} worktrees + {} agents' tmp, {} flips 100 ms apart, status +{} ms", WORKTREES + 1, TMP, FLIPS, SLOW.as_millis());
    println!("{}", b.report("before", tb));
    println!("{}", a.report("after", ta));
    let _ = std::fs::remove_dir_all(&root);
}
