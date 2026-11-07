//! When to ask the forge (pr-design §7) and the worker that asks, on its
//! own thread: the hub's loop never waits on `gh`.
//!
//! Cadence: 15 s while a PR's checks run or for 5 min after a branch's
//! tip moved (the hub sees it locally, for free); 60 s otherwise; 5 min
//! when no client is attached. A merged PR's branch is not asked again.
//! An error backs off (×2, at most 10 min); the boxes keep their last
//! state (faint once stale).
//!
//! pr-integration rule 2: the PRs an agent only reviews (their artifact,
//! no place) are followed by number in the same round ([`Plan::followed`],
//! the own repo's in the branches' query, another repo's in one more
//! call); one that no longer resolves is not asked again.

use super::{Activity, Forge, ForgeError, RepoRef};
use crate::place::{Checks, PrSnapshot, PrState};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::Duration;

pub const FAST_MS: u64 = 15_000;
pub const SLOW_MS: u64 = 60_000;
pub const IDLE_MS: u64 = 5 * 60_000;
/// A branch whose tip moved this recently is asked at the fast rate.
pub const PUSHED_MS: u64 = 5 * 60_000;
pub const BACKOFF_MAX_MS: u64 = 10 * 60_000;
/// How often the worker reads the branches' tips (local git, cheap).
pub const TIPS_MS: u64 = 5_000;

/// The interval between two asks (no error).
pub fn every(clients: bool, fast: bool) -> u64 {
    match (clients, fast) {
        (false, _) => IDLE_MS,
        (true, true) => FAST_MS,
        (true, false) => SLOW_MS,
    }
}

/// When the next ask is due: after the last one, the interval; after an
/// error, the back-off (twice the interval, then twice the last
/// back-off, at most [`BACKOFF_MAX_MS`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cadence {
    last_ms: Option<u64>,
    backoff_ms: Option<u64>,
}

impl Cadence {
    pub fn due(&self, now: u64, every: u64) -> bool {
        match self.last_ms {
            None => true,
            Some(l) => now >= l + self.backoff_ms.unwrap_or(every),
        }
    }

    pub fn ok(&mut self, now: u64) {
        self.last_ms = Some(now);
        self.backoff_ms = None;
    }

    pub fn failed(&mut self, now: u64, every: u64) {
        self.last_ms = Some(now);
        let b = self.backoff_ms.map_or(every * 2, |b| b * 2);
        self.backoff_ms = Some(b.min(BACKOFF_MAX_MS));
    }

    pub fn failing(&self) -> bool {
        self.backoff_ms.is_some()
    }

    /// A new branch to follow: ask soon, unless backing off.
    pub fn poke(&mut self) {
        if !self.failing() {
            self.last_ms = None;
        }
    }
}

/// A place's branch to follow (from the hub's places).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Watch {
    pub place: String,
    pub branch: String,
    /// The worktree (to see whether a merged place has work left).
    pub path: String,
    /// The commit the branch started from (for `no PR yet · N commits`).
    pub base: Option<String>,
    /// BISE-136: a private worktree (`gate.sh new`): follow what `path`
    /// has checked out. `branch` is the one the hub last heard of (""
    /// while detached: nothing to ask the forge); each round reads it
    /// again, and its commits not on the trunk.
    pub head: bool,
}

/// What the hub wants followed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub watches: Vec<Watch>,
    /// A client is attached (else the slow cadence).
    pub clients: bool,
    /// pr-integration rule 2: the PRs followed by number, by their link's
    /// one form (`forge::pr_link`): the open PR artifacts no place has
    /// (an agent reviews it), newest first, [`FOLLOW_MAX`] at most.
    /// Asked in the same round as the branches, at their pace.
    pub followed: Vec<String>,
}

/// The PRs followed by number at most (the newest artifacts first).
pub const FOLLOW_MAX: usize = 20;

/// A followed PR's answer: its PR, or None when it no longer resolves
/// (the hub stops following it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Followed {
    pub url: String,
    pub pr: Option<PrSnapshot>,
}

/// A place's branch as git sees it here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Local {
    pub place: String,
    pub branch: String,
    pub tip: Option<String>,
    /// Commits on the branch past its base.
    pub commits: Option<u32>,
    /// For a place whose PR is merged: uncommitted changes in its
    /// worktree (None: not looked at).
    pub dirty: Option<bool>,
}

/// One answer for the hub (`Input::Prs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub at_ms: u64,
    /// The PRs of the watched branches, or why the forge did not answer;
    /// None: not asked this time (only a tip moved).
    pub prs: Option<Result<Vec<PrSnapshot>, ForgeError>>,
    pub local: Vec<Local>,
    /// pr-news: the activity of the PRs that changed, by branch (only
    /// with an answer; see [`Watcher`]'s `activity`).
    pub activity: Vec<(String, Activity)>,
    /// The followed PRs' answers (only with an answer: every one asked).
    pub followed: Vec<Followed>,
}

/// The local git side the worker needs (a fake in the tests).
pub trait Git: Send {
    fn tip(&self, branch: &str) -> Option<String>;
    fn commits(&self, base: &str, branch: &str) -> Option<u32>;
    fn dirty(&self, worktree: &Path) -> Option<bool>;
    /// BISE-136: what `worktree` has checked out, its branch (None:
    /// detached), and its commits not on the trunk (the workspace's
    /// HEAD). None: not a worktree (gone).
    fn checkout(&self, worktree: &Path) -> Option<(Option<String>, Option<u32>)>;
}

/// The workspace's git.
pub struct RepoGit {
    pub workspace: PathBuf,
}

impl Git for RepoGit {
    fn tip(&self, branch: &str) -> Option<String> {
        crate::worktree::git(&self.workspace, &["rev-parse", "--verify", "-q", &format!("refs/heads/{}^{{commit}}", branch)]).ok()
    }
    fn commits(&self, base: &str, branch: &str) -> Option<u32> {
        crate::worktree::git(&self.workspace, &["rev-list", "--count", &format!("{}..refs/heads/{}", base, branch)])
            .ok()?
            .trim()
            .parse()
            .ok()
    }
    fn dirty(&self, worktree: &Path) -> Option<bool> {
        if !worktree.exists() {
            return Some(false);
        }
        crate::worktree::git(worktree, &["status", "--porcelain"]).ok().map(|s| !s.trim().is_empty())
    }
    fn checkout(&self, worktree: &Path) -> Option<(Option<String>, Option<u32>)> {
        if !worktree.exists() {
            return None;
        }
        let head = crate::worktree::git(worktree, &["rev-parse", "--verify", "-q", "HEAD"]).ok()?;
        let branch = crate::worktree::git(worktree, &["symbolic-ref", "-q", "--short", "HEAD"])
            .ok()
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty());
        let trunk = crate::worktree::git(&self.workspace, &["rev-parse", "--verify", "-q", "HEAD"]).ok();
        let commits = trunk.and_then(|t| {
            let range = format!("{}..{}", t.trim(), head.trim());
            crate::worktree::git(worktree, &["rev-list", "--count", &range]).ok()?.trim().parse().ok()
        });
        Some((branch, commits))
    }
}

/// The worker's state: what to follow, the cadence, the last answer, the
/// tips. [`Watcher::step`] does one round (tests drive it with their own
/// clock); [`start`] runs it on a thread.
pub struct Watcher {
    pub repo: RepoRef,
    forge: Box<dyn Forge>,
    git: Box<dyn Git>,
    plan: Plan,
    cadence: Cadence,
    /// The last PRs by branch (for the cadence: checks running, merged).
    prs: BTreeMap<String, PrSnapshot>,
    /// Branch -> (tip, when it last moved, commits past the base).
    tips: BTreeMap<String, (Option<String>, u64, Option<u32>)>,
    tips_ms: Option<u64>,
    /// pr-news: by branch, the `updatedAt` whose activity was read, and
    /// the head whose failing checks' logs were read.
    looked: BTreeMap<String, String>,
    logs: BTreeMap<String, String>,
    /// BISE-136: place -> what its private worktree has checked out (its
    /// branch, "" detached) and its commits not on the trunk.
    heads: BTreeMap<String, (String, Option<u32>)>,
    /// pr-integration rule 2: the followed PRs that no longer resolve
    /// (the forge's typed not-found): never asked again by this worker.
    /// It lives as long as the worker: after a hub restart a gone PR is
    /// asked once more (one alias in the round's query), then dropped
    /// again (architect m_54590).
    gone: BTreeSet<String>,
}

/// The followed PRs to ask, by repo (the watcher's own first, in the
/// branches' query): each repo's numbers and their links.
fn follow_by_repo(own: &RepoRef, followed: &[String], gone: &BTreeSet<String>) -> Vec<(RepoRef, Vec<(u64, String)>)> {
    let mut out: Vec<(RepoRef, Vec<(u64, String)>)> = vec![(own.clone(), Vec::new())];
    for l in followed.iter().filter(|u| !gone.contains(*u)).filter_map(|u| super::pr_link(u)) {
        if l.provider != bise_proto::rows::PrProvider::Github {
            continue;
        }
        let Some((owner, name)) = l.repo.split_once('/') else { continue };
        let repo = RepoRef { host: l.host.clone(), owner: owner.into(), name: name.into() };
        let same = |r: &RepoRef| r.host == repo.host && r.owner.eq_ignore_ascii_case(owner) && r.name.eq_ignore_ascii_case(name);
        match out.iter_mut().find(|(r, _)| same(r)) {
            Some((_, ns)) => ns.push((l.number, l.url)),
            None => out.push((repo, vec![(l.number, l.url)])),
        }
    }
    out
}

impl Watcher {
    pub fn new(repo: RepoRef, forge: Box<dyn Forge>, git: Box<dyn Git>) -> Watcher {
        Watcher {
            repo,
            forge,
            git,
            plan: Plan::default(),
            cadence: Cadence::default(),
            prs: BTreeMap::new(),
            tips: BTreeMap::new(),
            tips_ms: None,
            looked: BTreeMap::new(),
            logs: BTreeMap::new(),
            heads: BTreeMap::new(),
            gone: BTreeSet::new(),
        }
    }

    /// pr-news (pr-design §7: details only on change): the activity of
    /// each open PR whose `updatedAt` moved since it was last read, or
    /// whose checks fail on a head whose logs were not read yet (a check
    /// failing does not move `updatedAt`). A failed ask is tried again
    /// at the next answer; the hub keeps what it already passed on.
    fn activity(&mut self, ps: &[PrSnapshot]) -> Vec<(String, Activity)> {
        let mut out = Vec::new();
        for p in ps.iter().filter(|p| matches!(p.state, PrState::Open | PrState::Draft)) {
            let moved = self.looked.get(&p.branch) != Some(&p.updated_at);
            let logs = matches!(p.checks, Checks::Fail { .. }) && self.logs.get(&p.branch) != Some(&p.head_oid);
            if !moved && !logs {
                continue;
            }
            if let Ok(a) = self.forge.activity(&self.repo, p, logs) {
                self.looked.insert(p.branch.clone(), p.updated_at.clone());
                if logs {
                    self.logs.insert(p.branch.clone(), p.head_oid.clone());
                }
                out.push((p.branch.clone(), a));
            }
        }
        let live: BTreeSet<&String> = self.plan.watches.iter().map(|w| &w.branch).collect();
        self.looked.retain(|b, _| live.contains(b));
        self.logs.retain(|b, _| live.contains(b));
        out
    }

    /// A new plan from the hub: a branch not followed yet is asked soon.
    pub fn plan(&mut self, p: Plan) {
        let before: BTreeSet<&String> = self.plan.watches.iter().map(|w| &w.branch).collect();
        if p.watches.iter().any(|w| !before.contains(&w.branch)) {
            self.cadence.poke();
            self.tips_ms = None;
        }
        self.plan = p;
    }

    /// The branches asked: the watched ones whose PR is not merged.
    fn asked(&self) -> Vec<String> {
        let mut bs: Vec<String> = self
            .plan
            .watches
            .iter()
            .filter(|w| !w.branch.is_empty())
            .filter(|w| self.prs.get(&w.branch).is_none_or(|p| p.state != PrState::Merged))
            .map(|w| w.branch.clone())
            .collect();
        bs.sort();
        bs.dedup();
        bs
    }

    fn fast(&self, now: u64) -> bool {
        let running = self.prs.values().any(|p| p.checks == Checks::Running && !matches!(p.state, PrState::Merged | PrState::Closed));
        let pushed = self.tips.values().any(|(_, moved, _)| *moved > 0 && now.saturating_sub(*moved) < PUSHED_MS);
        running || pushed
    }

    fn read_tips(&mut self, now: u64) -> bool {
        let mut moved = false;
        for w in &self.plan.watches {
            if w.head {
                // what is checked out there now: a change is news for the
                // hub (its branch, its held line), a first sight too
                let (b, n) = self.git.checkout(Path::new(&w.path)).unwrap_or((None, None));
                let now_head = (b.unwrap_or_default(), n);
                moved |= self.heads.get(&w.place) != Some(&now_head);
                self.heads.insert(w.place.clone(), now_head);
            }
            if w.branch.is_empty() {
                continue;
            }
            let tip = self.git.tip(&w.branch);
            let old = self.tips.get(&w.branch).cloned();
            match old {
                Some((t, at, n)) if t == tip => {
                    self.tips.insert(w.branch.clone(), (t, at, n));
                }
                _ => {
                    // a first sight is not a push; a move is
                    let at = if old.is_some() { now } else { 0 };
                    moved |= old.is_some();
                    let n = w.base.as_deref().and_then(|b| self.git.commits(b, &w.branch));
                    self.tips.insert(w.branch.clone(), (tip, at, n));
                }
            }
        }
        let live: BTreeSet<&String> = self.plan.watches.iter().map(|w| &w.branch).collect();
        self.tips.retain(|b, _| live.contains(b));
        let heads: BTreeSet<&String> = self.plan.watches.iter().filter(|w| w.head).map(|w| &w.place).collect();
        self.heads.retain(|p, _| heads.contains(p));
        self.tips_ms = Some(now);
        moved
    }

    fn local(&self, merged: &BTreeSet<String>) -> Vec<Local> {
        self.plan
            .watches
            .iter()
            .map(|w| {
                let (tip, _, commits) = self.tips.get(&w.branch).cloned().unwrap_or((None, 0, None));
                // a private worktree: what it has checked out, as read
                let (branch, commits) = match self.heads.get(&w.place).filter(|_| w.head) {
                    Some((b, n)) => (b.clone(), *n),
                    None => (w.branch.clone(), commits),
                };
                // the tip is the watched branch's: none for another one
                let tip = tip.filter(|_| branch == w.branch);
                Local {
                    place: w.place.clone(),
                    branch,
                    tip,
                    commits,
                    dirty: merged.contains(&w.branch).then(|| self.git.dirty(Path::new(&w.path))).flatten(),
                }
            })
            .collect()
    }

    /// One round at `now`: read the tips when due, ask the forge when
    /// due. Some(report) when something was asked or a tip moved (the
    /// hub's held lines count commits).
    pub fn step(&mut self, now: u64) -> Option<Report> {
        let moved = if self.tips_ms.is_none_or(|t| now >= t + TIPS_MS) { self.read_tips(now) } else { false };
        let asked = self.asked();
        let follow = follow_by_repo(&self.repo, &self.plan.followed, &self.gone);
        let every = every(self.plan.clients, self.fast(now));
        if (asked.is_empty() && follow.iter().all(|(_, ns)| ns.is_empty())) || !self.cadence.due(now, every) {
            // only the local side is new (a commit: the held line counts it)
            return moved.then(|| Report { at_ms: now, prs: None, local: self.local(&BTreeSet::new()), activity: Vec::new(), followed: Vec::new() });
        }
        let (prs, followed) = self.fetch_all(&asked, &follow);
        let mut merged = BTreeSet::new();
        let mut activity = Vec::new();
        match &prs {
            Ok(ps) => {
                activity = self.activity(ps);
                self.cadence.ok(now);
                for b in &asked {
                    self.prs.remove(b);
                }
                for p in ps {
                    if p.state == PrState::Merged {
                        merged.insert(p.branch.clone());
                    }
                    self.prs.insert(p.branch.clone(), p.clone());
                }
            }
            Err(_) => self.cadence.failed(now, every),
        }
        // the merged ones not asked this time: still the place's PR
        let prs = prs.map(|mut ps| {
            for (b, p) in &self.prs {
                if !asked.contains(b) && self.plan.watches.iter().any(|w| &w.branch == b) {
                    ps.push(p.clone());
                }
            }
            ps
        });
        Some(Report { at_ms: now, prs: Some(prs), local: self.local(&merged), activity, followed })
    }

    /// One round's asks: the own repo's branches and followed numbers in
    /// one query, each other repo's followed numbers in one more (rule
    /// 2: the same round, never a timer of their own). The branches'
    /// answer, and the followed ones' (none when the own query failed;
    /// another repo's failure only skips its PRs this round). A followed
    /// PR that no longer resolves is not asked again.
    fn fetch_all(&mut self, asked: &[String], follow: &[(RepoRef, Vec<(u64, String)>)]) -> (Result<Vec<PrSnapshot>, super::ForgeError>, Vec<Followed>) {
        let mut followed = Vec::new();
        let mut own = Err(super::ForgeError::Other("not asked".into()));
        for (i, (repo, ns)) in follow.iter().enumerate() {
            let numbers: Vec<u64> = ns.iter().map(|(n, _)| *n).collect();
            let branches = if i == 0 { asked } else { &[] };
            let answer = self.forge.fetch(repo, branches, &numbers);
            if let Ok(a) = &answer {
                for ((n, url), (m, pr)) in ns.iter().zip(&a.numbered) {
                    debug_assert_eq!(n, m);
                    if pr.is_none() {
                        self.gone.insert(url.clone());
                    }
                    followed.push(Followed { url: url.clone(), pr: pr.clone() });
                }
            }
            if i == 0 {
                own = answer.map(|a| a.prs);
                if own.is_err() {
                    return (own, Vec::new());
                }
            }
        }
        (own, followed)
    }
}

/// The worker on its thread: `plan` it, it sends each report to `sink`.
/// It ends when the [`Poller`] is dropped.
pub struct Poller {
    tx: Sender<Plan>,
    last: Option<Plan>,
}

/// Makes the watcher on the worker's thread (finding gh, the repo's
/// forge: slow); None: nothing to follow (not a GitHub repo).
pub type Setup = Box<dyn FnOnce() -> Option<Watcher> + Send>;

impl Poller {
    pub fn start(setup: Setup, sink: Box<dyn Fn(Report) + Send>) -> Poller {
        let (tx, rx) = channel::<Plan>();
        std::thread::spawn(move || {
            let Some(mut w) = setup() else {
                // plans are taken and dropped until the hub goes
                while rx.recv().is_ok() {}
                return;
            };
            loop {
                match rx.recv_timeout(Duration::from_millis(1000)) {
                    Ok(p) => w.plan(p),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                // the plans sent meanwhile: the last one counts
                while let Ok(p) = rx.try_recv() {
                    w.plan(p);
                }
                if let Some(r) = w.step(crate::util::now_ms()) {
                    sink(r);
                }
            }
        });
        Poller { tx, last: None }
    }

    /// Tell the worker what to follow (only when it changed).
    pub fn plan(&mut self, p: Plan) {
        if self.last.as_ref() != Some(&p) {
            let _ = self.tx.send(p.clone());
            self.last = Some(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn the_cadence() {
        assert_eq!(every(true, true), FAST_MS);
        assert_eq!(every(true, false), SLOW_MS);
        assert_eq!(every(false, true), IDLE_MS);
        let mut c = Cadence::default();
        assert!(c.due(0, SLOW_MS));
        c.ok(1_000);
        assert!(!c.due(60_999, SLOW_MS));
        assert!(c.due(61_000, SLOW_MS));
        // faster when the interval shrinks (a push): no wait for the old one
        assert!(c.due(16_000, FAST_MS));
        // errors: ×2 each time, at most 10 min
        c.failed(100_000, SLOW_MS);
        assert!(c.failing());
        assert!(!c.due(219_999, SLOW_MS));
        assert!(c.due(220_000, SLOW_MS));
        let mut at = 220_000;
        for _ in 0..10 {
            c.failed(at, SLOW_MS);
            at += 1;
        }
        let last = at - 1;
        assert!(!c.due(last + BACKOFF_MAX_MS - 1, SLOW_MS));
        assert!(c.due(last + BACKOFF_MAX_MS, SLOW_MS));
        // a poke does not jump the back-off
        c.poke();
        assert!(c.failing());
        c.ok(at);
        assert!(!c.failing());
        c.poke();
        assert!(c.due(at, SLOW_MS));
    }

    /// A forge answering from a shared script; counts its calls.
    struct Fake {
        answer: Arc<Mutex<Result<Vec<PrSnapshot>, ForgeError>>>,
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl Forge for Fake {
        /// A followed number answers the scripted PR of that number
        /// (`u<n>`), none when the script has no such PR.
        fn fetch(&self, repo: &RepoRef, branches: &[String], numbers: &[u64]) -> Result<super::super::Answer, ForgeError> {
            let mut call = branches.to_vec();
            call.extend(numbers.iter().map(|n| format!("{}/{}#{}", repo.owner, repo.name, n)));
            self.calls.lock().unwrap().push(call);
            let all = self.answer.lock().unwrap().clone()?;
            let prs = all.iter().filter(|p| branches.contains(&p.branch)).cloned().collect();
            let numbered = numbers.iter().map(|n| (*n, all.iter().find(|p| p.number == *n).cloned())).collect();
            Ok(super::super::Answer { prs, numbered })
        }
    }

    struct FakeGit {
        tips: Arc<Mutex<BTreeMap<String, String>>>,
    }

    impl Git for FakeGit {
        fn tip(&self, branch: &str) -> Option<String> {
            self.tips.lock().unwrap().get(branch).cloned()
        }
        fn commits(&self, _base: &str, branch: &str) -> Option<u32> {
            self.tips.lock().unwrap().get(branch).map(|t| t.len() as u32)
        }
        fn dirty(&self, _w: &Path) -> Option<bool> {
            Some(false)
        }
        /// `@<path>` in the tips: its branch ("" detached), `#<path>`:
        /// its commit count.
        fn checkout(&self, w: &Path) -> Option<(Option<String>, Option<u32>)> {
            let t = self.tips.lock().unwrap();
            let p = w.to_string_lossy();
            let b = t.get(&format!("@{}", p))?.clone();
            let n = t.get(&format!("#{}", p)).and_then(|n| n.parse().ok());
            Some(((!b.is_empty()).then_some(b), n))
        }
    }

    fn pr(branch: &str, n: u64, state: PrState, checks: Checks) -> PrSnapshot {
        PrSnapshot {
            number: n,
            url: format!("u{}", n),
            branch: branch.into(),
            head_oid: "h".into(),
            state,
            review: crate::place::Review::None,
            checks,
            updated_at: "t".into(),
            facts: Default::default(),
        }
    }

    fn watch(b: &str) -> Watch {
        Watch { place: format!("wt:{}", b), branch: b.into(), path: "/nowhere".into(), base: Some("base".into()), head: false }
    }

    /// The thread and the real git: a repo with a branch 2 commits past
    /// its base, a merged PR at its tip, a dirty worktree.
    #[test]
    fn the_poller_thread_on_a_real_repo() {
        let root = std::env::temp_dir().join(format!("sb-poll-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let g = |args: &[&str]| {
            let id = [
                ("GIT_AUTHOR_NAME", "t"),
                ("GIT_AUTHOR_EMAIL", "t@t"),
                ("GIT_COMMITTER_NAME", "t"),
                ("GIT_COMMITTER_EMAIL", "t@t"),
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ];
            let o = std::process::Command::new("git").args(args).current_dir(&root).envs(id).output().unwrap();
            assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        g(&["init", "-q", "-b", "main"]);
        g(&["commit", "-q", "--allow-empty", "-m", "base"]);
        let base = g(&["rev-parse", "HEAD"]);
        g(&["checkout", "-q", "-b", "sb/a"]);
        g(&["commit", "-q", "--allow-empty", "-m", "one"]);
        g(&["commit", "-q", "--allow-empty", "-m", "two"]);
        let tip = g(&["rev-parse", "HEAD"]);
        std::fs::write(root.join("dirty.txt"), "x").unwrap();
        let mut merged = pr("sb/a", 5, PrState::Merged, Checks::Pass);
        merged.head_oid = tip.clone();
        let forge = Fake { answer: Arc::new(Mutex::new(Ok(vec![merged]))), calls: Arc::new(Mutex::new(Vec::new())) };
        let repo = RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() };
        let ws = root.clone();
        let (tx, rx) = channel();
        let mut p = Poller::start(
            Box::new(move || Some(Watcher::new(repo, Box::new(forge), Box::new(RepoGit { workspace: ws })))),
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        let w = Watch { place: "wt:a".into(), branch: "sb/a".into(), path: root.to_string_lossy().into(), base: Some(base), head: false };
        p.plan(Plan { watches: vec![w], clients: true, followed: Vec::new() });
        let r = rx.recv_timeout(Duration::from_secs(10)).expect("a report");
        assert_eq!(r.local[0].tip.as_deref(), Some(tip.as_str()));
        assert_eq!(r.local[0].commits, Some(2));
        assert_eq!(r.local[0].dirty, Some(true));
        assert_eq!(r.prs.unwrap().unwrap()[0].number, 5);
        // the same plan again: not sent (the worker keeps its cadence)
        p.plan(Plan { watches: p.last.clone().unwrap().watches, clients: true, followed: Vec::new() });
        drop(p);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// BISE-136: a private worktree's watch reads what it has checked
    /// out each round: detached, nothing is asked of the forge, its count
    /// goes to the hub; a branch checked out there is reported, and asked
    /// once the hub's plan names it.
    #[test]
    fn a_private_worktree_is_followed_by_its_head() {
        let answer = Arc::new(Mutex::new(Ok(vec![pr("feat/x", 7, PrState::Open, Checks::Pass)])));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let tips = Arc::new(Mutex::new(BTreeMap::from([("@/p/fix".to_string(), String::new()), ("#/p/fix".into(), "0".into())])));
        let repo = RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() };
        let mut w = Watcher::new(
            repo,
            Box::new(Fake { answer: answer.clone(), calls: calls.clone() }),
            Box::new(FakeGit { tips: tips.clone() }),
        );
        let head = |b: &str| Watch { place: "pt:/p/fix".into(), branch: b.into(), path: "/p/fix".into(), base: None, head: true };
        w.plan(Plan { watches: vec![head("")], clients: true, followed: Vec::new() });
        // a first sight: the hub hears it, the forge is not asked
        let r = w.step(1_000).unwrap();
        assert_eq!(r.prs, None);
        assert_eq!((r.local[0].branch.as_str(), r.local[0].commits), ("", Some(0)));
        assert!(calls.lock().unwrap().is_empty());
        assert!(w.step(7_000).is_none(), "nothing moved");
        // a commit: news
        tips.lock().unwrap().insert("#/p/fix".into(), "2".into());
        assert_eq!(w.step(13_000).unwrap().local[0].commits, Some(2));
        // a branch checked out there: reported; the hub's next plan names it
        tips.lock().unwrap().insert("@/p/fix".into(), "feat/x".into());
        tips.lock().unwrap().insert("feat/x".into(), "t9".into());
        let r = w.step(19_000).unwrap();
        assert_eq!(r.local[0].branch, "feat/x");
        assert_eq!(r.local[0].tip, None, "the tip is the watched branch's");
        w.plan(Plan { watches: vec![head("feat/x")], clients: true, followed: Vec::new() });
        let r = w.step(25_000).unwrap();
        assert_eq!(calls.lock().unwrap().last().unwrap(), &["feat/x"]);
        assert_eq!(r.prs.unwrap().unwrap()[0].number, 7);
        assert_eq!(r.local[0].tip.as_deref(), Some("t9"));
        // the worktree is gone: no branch, no count
        tips.lock().unwrap().remove("@/p/fix");
        let r = w.step(31_000).unwrap();
        assert_eq!((r.local[0].branch.as_str(), r.local[0].commits), ("", None));
    }

    #[test]
    fn the_worker_asks_on_its_cadence() {
        let answer = Arc::new(Mutex::new(Ok(vec![pr("a", 1, PrState::Open, Checks::Running)])));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let tips = Arc::new(Mutex::new(BTreeMap::from([("a".to_string(), "t1".to_string()), ("b".into(), "u1".into())])));
        let repo = RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() };
        let mut w = Watcher::new(
            repo,
            Box::new(Fake { answer: answer.clone(), calls: calls.clone() }),
            Box::new(FakeGit { tips: tips.clone() }),
        );
        // nothing to follow: nothing asked
        assert!(w.step(0).is_none());
        w.plan(Plan { watches: vec![watch("a"), watch("b")], clients: true, followed: Vec::new() });
        let r = w.step(1_000).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(calls.lock().unwrap()[0], ["a", "b"]);
        assert_eq!(r.prs.as_ref().unwrap().as_ref().unwrap().len(), 1);
        assert_eq!(r.local[0].commits, Some(2));
        // checks running: 15 s
        assert!(w.step(15_999).is_none());
        assert!(w.step(16_000).is_some());
        // checks done, no push: 60 s
        *answer.lock().unwrap() = Ok(vec![pr("a", 1, PrState::Open, Checks::Pass)]);
        w.step(31_000);
        assert!(w.step(46_000).is_none());
        assert!(w.step(90_000).is_none());
        assert!(w.step(91_000).is_some());
        // a commit on b: a report for the held line at once, then fast
        tips.lock().unwrap().insert("b".into(), "u22".into());
        let r = w.step(100_000).unwrap();
        assert_eq!(r.prs, None);
        assert_eq!(r.local[1].commits, Some(3));
        assert_eq!(calls.lock().unwrap().len(), 4, "a tip move alone asks nothing");
        assert!(w.step(106_000).is_some(), "fast again (pushed), 15 s after the last ask");
        // no client: 5 min
        w.plan(Plan { watches: vec![watch("a"), watch("b")], clients: false, followed: Vec::new() });
        assert!(w.step(200_000).is_none());
        assert!(w.step(406_000).is_some());
        // offline: back-off, the hub gets the error
        *answer.lock().unwrap() = Err(ForgeError::Offline("x".into()));
        w.plan(Plan { watches: vec![watch("a"), watch("b")], clients: true, followed: Vec::new() });
        let r = w.step(466_000).unwrap();
        assert!(matches!(r.prs, Some(Err(ForgeError::Offline(_)))));
        assert!(w.step(466_000 + 2 * SLOW_MS - 1).is_none());
        // merged: asked no more, still reported as the place's PR
        *answer.lock().unwrap() = Ok(vec![pr("a", 1, PrState::Merged, Checks::Pass)]);
        let r = w.step(466_000 + 2 * SLOW_MS).unwrap();
        assert_eq!(r.local[0].dirty, Some(false), "a merged place: its worktree looked at");
        assert_eq!(r.local[1].dirty, None);
        let n = calls.lock().unwrap().len();
        let r = w.step(466_000 + 3 * SLOW_MS).unwrap();
        assert_eq!(calls.lock().unwrap()[n], ["b"]);
        assert_eq!(r.prs.unwrap().unwrap()[0].state, PrState::Merged);
    }

    /// pr-integration rule 2 (architect m_54087): the followed PRs go in
    /// the same round as the branches, the own repo's in the branches'
    /// query, another repo's in one more call; one that no longer
    /// resolves is not asked again; with nothing but followed PRs the
    /// worker still asks.
    #[test]
    fn followed_prs_ride_the_same_round() {
        let answer = Arc::new(Mutex::new(Ok(vec![pr("a", 1, PrState::Open, Checks::Pass), pr("feat/x", 75286, PrState::Open, Checks::Running)])));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let repo = RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() };
        let mut w = Watcher::new(repo, Box::new(Fake { answer: answer.clone(), calls: calls.clone() }), Box::new(FakeGit { tips: Default::default() }));
        let followed = vec![
            "https://github.com/O/R/pull/75286".to_string(),
            "https://github.com/o/r/pull/12".to_string(),
            "https://github.com/acme/web/pull/1".to_string(),
        ];
        w.plan(Plan { watches: vec![watch("a")], clients: true, followed: followed.clone() });
        let r = w.step(1_000).unwrap();
        assert_eq!(*calls.lock().unwrap(), [vec!["a", "o/r#75286", "o/r#12"], vec!["acme/web#1"]]);
        assert_eq!(r.prs.unwrap().unwrap().len(), 1);
        let got: Vec<(&str, Option<u64>)> = r.followed.iter().map(|f| (f.url.as_str(), f.pr.as_ref().map(|p| p.number))).collect();
        // each by its link's one form
        assert_eq!(got, [("https://github.com/o/r/pull/75286", Some(75286)), ("https://github.com/o/r/pull/12", None), ("https://github.com/acme/web/pull/1", Some(1))]);
        // #12 does not resolve: not asked again; no branch left: still asked
        w.plan(Plan { watches: Vec::new(), clients: true, followed });
        calls.lock().unwrap().clear();
        let r = w.step(100_000).unwrap();
        assert_eq!(*calls.lock().unwrap(), [vec!["o/r#75286"], vec!["acme/web#1"]]);
        assert_eq!(r.followed.len(), 2);
        // the own query fails: the round's error, no followed answers
        *answer.lock().unwrap() = Err(ForgeError::Offline("x".into()));
        let r = w.step(200_000).unwrap();
        assert!(matches!(r.prs, Some(Err(_))) && r.followed.is_empty());
    }
}
