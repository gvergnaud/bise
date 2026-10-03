//! `sb land` (dev-flow §5): what this repo did by hand (a private
//! `GIT_INDEX_FILE` from the current tip, `commit-tree`, a compare-and-
//! swap `update-ref`, then the place's index synced), as one checked step
//! the hub runs for an agent, off its loop (the daemon's thread).
//!
//! - `--here`: the agent's own files (the hub's list, RFC 0001 §10.3) are
//!   committed on its place's branch: the shared folder's (main), or its
//!   worktree's. Another agent's half-done files are never in it; a file
//!   another agent of the place also changed is refused (main decides).
//! - plain, from a worktree: its own files first (with the message), then
//!   the branch is rebased on main (the worktree must be clean: every
//!   agent landed its files), the repo's check runs if main had moved,
//!   main moves to the branch (fast-forward); in trunk flow with `push`,
//!   main is pushed. From the shared folder: `--here`, then the push.
//! - One land at a time per target ref ([`Queue`]): the others wait in
//!   line, and the views say so (`waits to land · 2nd`).

use crate::flow::{FlowConfig, FlowMode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

/// What the hub knows when an agent asks to land (core.rs builds it from
/// the state; the daemon adds the repo's flow).
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub agent: String,
    pub here: bool,
    pub message: String,
    /// The place: its id, its folder, and whether it is a worktree.
    pub place: String,
    pub dir: PathBuf,
    pub worktree: bool,
    /// The shared folder (main's checkout).
    pub shared: PathBuf,
    /// The agent's files and the other agents' of the place (not
    /// archived), as the hub tracked them (absolute or relative).
    pub files: Vec<String>,
    pub others: Vec<(String, Vec<String>)>,
    /// `sb land --add <path>`: new files the agent claims (made by bash:
    /// a generator, a download), a folder for every new file under it.
    pub add: Vec<String>,
    /// When the agent was created (ms): a new file older than that is not
    /// its own (the left-out list skips it).
    pub since_ms: u64,
    pub flow: FlowConfig,
    /// dev-flow §5.1: the agent's feature branch (`refs/heads/computer-
    /// use`): a plain land from its worktree moves it, never main, and is
    /// never pushed. None: main.
    pub onto: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The ref that moved (`main`, `sb/x`), its new tip (short), how many
    /// commits it gained.
    pub target: String,
    pub sha: String,
    pub commits: usize,
    /// None: no push tried; Some(false): tried, failed (`push_error`).
    pub pushed: Option<bool>,
    pub push_error: Option<String>,
    /// New files of the place that no agent claimed, made since the agent
    /// started: not landed, named in the answer ([`left_out_note`]).
    pub left_out: Vec<String>,
}

// ---- the queue: one land at a time per target ref ----

#[derive(Default)]
struct Line {
    /// (ticket, target ref, place id), the first one landing.
    waiting: Vec<(u64, String, String)>,
    next: u64,
}

/// The land queue, shared by the daemon's land threads.
#[derive(Clone, Default)]
pub struct Queue {
    inner: Arc<(Mutex<Line>, Condvar)>,
}

/// A place in line; dropping it leaves the line.
pub struct Turn {
    q: Queue,
    ticket: u64,
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (m, cv) = &*self.q.inner;
        let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
        l.waiting.retain(|(t, _, _)| *t != self.ticket);
        cv.notify_all();
    }
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{}{}", n, suffix)
}

impl Queue {
    /// Join the line for `target` and wait until first. `joined`: called
    /// once in line (the views refresh: the lid says it waits).
    pub fn wait_turn(&self, target: &str, place: &str, joined: &mut dyn FnMut()) -> Turn {
        let (m, cv) = &*self.inner;
        let ticket = {
            let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
            l.next += 1;
            let t = l.next;
            l.waiting.push((t, target.to_string(), place.to_string()));
            t
        };
        joined();
        let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
        while l.waiting.iter().find(|(_, r, _)| r == target).map(|(t, _, _)| *t) != Some(ticket) {
            l = cv.wait(l).unwrap_or_else(|e| e.into_inner());
        }
        Turn {
            q: self.clone(),
            ticket,
        }
    }

    /// The held line of each place in line (pr-design §4.1, the lid):
    /// `landing` for the first, `waits to land · 2nd` for the next.
    pub fn lids(&self) -> BTreeMap<String, String> {
        let (m, _) = &*self.inner;
        let l = m.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = BTreeMap::new();
        let mut rank: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, r, p) in &l.waiting {
            let n = rank.entry(r.as_str()).or_insert(0);
            let lid = if *n == 0 { "landing".to_string() } else { format!("waits to land · {}", ordinal(*n + 1)) };
            out.entry(p.clone()).or_insert(lid);
            *n += 1;
        }
        out
    }
}

// ---- git ----

fn git_in(dir: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut cmd = crate::tools_env::git_command()?;
    cmd.arg("-C").arg(dir).args(args);
    if let Some(i) = index {
        cmd.env("GIT_INDEX_FILE", i);
    }
    let out = cmd.output().map_err(|e| format!("git could not start: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_in(dir, args, None)
}

// ---- signing ----

/// `git commit` signs in `dir` (`commit.gpgsign`, repo or global): so do
/// bise's commits. `git rebase` reads it on its own; `commit-tree` does
/// not (it gets `-S`).
pub(crate) fn signs(dir: &Path) -> bool {
    git(dir, &["config", "--type=bool", "--get", "commit.gpgsign"]).is_ok_and(|v| v == "true")
}

/// A git error that is the signing's: gpg or ssh-keygen could not sign,
/// so git could not write the commit (no conflict).
pub(crate) fn signing_error(e: &str) -> bool {
    let e = e.to_lowercase();
    e.contains("failed to write commit object") || e.contains("sign")
}

/// The one line when signing fails: git's own words (its hints left
/// out), the work kept, what to do. `again`: `land again`, `sync again`.
pub(crate) fn signing_failed(e: &str, again: &str) -> String {
    let words: Vec<&str> = e
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("hint:"))
        .map(|l| {
            // `git commit-tree: error: …` (git_in's prefix, then git's)
            let l = match l.strip_prefix("git ").and_then(|r| r.split_once(": ")) {
                Some((cmd, rest)) if !cmd.contains(' ') => rest,
                _ => l,
            };
            l.strip_prefix("error: ").or_else(|| l.strip_prefix("fatal: ")).unwrap_or(l)
        })
        .collect();
    format!("commit signing failed: {}; your work is kept, {} once signing works", words.join("; "), again)
}

/// Why a rebase without conflicts failed: the signing line when it is
/// the signing's, else `other`.
pub(crate) fn rebase_failed(dir: &Path, e: &str, again: &str, other: String) -> String {
    if signing_error(e) && signs(dir) {
        signing_failed(e, again)
    } else {
        other
    }
}

/// A tracked file path, relative to the place's folder: absolute paths
/// outside it are not the place's (a private worktree of `gate.sh new`).
pub fn relative(dir: &Path, f: &str) -> Option<String> {
    let p = Path::new(f);
    let rel = if p.is_absolute() {
        let d = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let pc = p.parent().and_then(|x| x.canonicalize().ok()).map(|x| x.join(p.file_name().unwrap_or_default()));
        let p = pc.as_deref().unwrap_or(p);
        p.strip_prefix(&d).ok().or_else(|| p.strip_prefix(dir).ok())?.to_string_lossy().to_string()
    } else {
        f.trim_start_matches("./").to_string()
    };
    (!rel.is_empty() && !rel.starts_with("..")).then_some(rel)
}

/// Of `files`, those that differ from the checkout's HEAD (changed,
/// added, deleted), in order.
fn changed(dir: &Path, files: &[String]) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all", "--"];
    args.extend(files.iter().map(String::as_str));
    let out = git(dir, &args)?;
    let mut seen: Vec<String> = Vec::new();
    let mut parts = out.split('\0').filter(|s| !s.is_empty());
    while let Some(e) = parts.next() {
        let (xy, path) = e.split_at(e.len().min(3));
        seen.push(path.to_string());
        if xy.starts_with('R') || xy.starts_with('C') {
            if let Some(old) = parts.next() {
                seen.push(old.to_string());
            }
        }
    }
    Ok(files.iter().filter(|f| seen.contains(f)).cloned().collect())
}

/// The ref the checkout at `dir` is on (`refs/heads/main`).
pub(crate) fn head_ref(dir: &Path) -> Result<String, String> {
    git(dir, &["symbolic-ref", "-q", "HEAD"]).map_err(|_| format!("{} is not on a branch (detached HEAD)", dir.display()))
}

pub(crate) fn short(r: &str) -> &str {
    r.strip_prefix("refs/heads/").unwrap_or(r)
}

fn tmp_index(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sb-land-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()))
}

/// Commit `files` (relative to `dir`, as they are in it) on `target`
/// through a private index built from its tip, then move it with a
/// compare-and-swap: a tip moved meanwhile (someone else committed)
/// rebuilds from the new one, so nothing of theirs is lost. `after_read`:
/// tests only (a commit lands between the read and the swap).
pub fn commit_files(
    dir: &Path,
    target: &str,
    files: &[String],
    message: &str,
    after_read: &mut dyn FnMut(),
) -> Result<String, String> {
    let sign = signs(dir);
    for _ in 0..5 {
        let old = git(dir, &["rev-parse", "--verify", target])?;
        let idx = tmp_index("idx");
        let res = (|| {
            git_in(dir, &["read-tree", &old], Some(&idx))?;
            let mut args = vec!["update-index", "--add", "--remove", "--"];
            args.extend(files.iter().map(String::as_str));
            git_in(dir, &args, Some(&idx))?;
            let tree = git_in(dir, &["write-tree"], Some(&idx))?;
            if tree == git(dir, &["rev-parse", &format!("{}^{{tree}}", old)])? {
                return Err("nothing to land: your files are already committed".to_string());
            }
            // commit-tree never reads commit.gpgsign: -S signs it the way
            // `git commit` would (gpg.format, user.signingkey), and a
            // signing that fails fails the land, never an unsigned commit
            let mut args = vec!["commit-tree", "-p", &old, "-m", message, &tree];
            if sign {
                args.insert(1, "-S");
            }
            git(dir, &args).map_err(|e| if sign { signing_failed(&e, "land again") } else { e })
        })();
        let _ = std::fs::remove_file(&idx);
        let new = res?;
        after_read();
        if git(dir, &["update-ref", target, &new, &old]).is_ok() {
            sync_index(dir, target, files);
            return Ok(new);
        }
    }
    Err(format!("{} kept moving: try again", short(target)))
}

/// The checkout's index follows the new tip for those paths, so `git
/// status` stays clean there (the "D / ??" lag of 18b7443). Only when
/// the checkout is on `target`; a locked index is retried.
fn sync_index(dir: &Path, target: &str, files: &[String]) {
    if head_ref(dir).as_deref() != Ok(target) || files.is_empty() {
        return;
    }
    let mut args = vec!["reset", "-q", "--"];
    args.extend(files.iter().map(String::as_str));
    for _ in 0..10 {
        if git(dir, &args).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The untracked files of the checkout that git does not ignore, relative
/// to `dir`, new folders walked (`kit/fonts/a.woff2`, never `kit/`); a
/// nested repo (`x/`) is not a file to land.
fn untracked(dir: &Path) -> Result<Vec<String>, String> {
    let out = git(dir, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    Ok(out.split('\0').filter(|s| !s.is_empty() && !s.ends_with('/')).map(str::to_string).collect())
}

/// More new files than this in a worktree are not swept in on their own
/// (build output git does not ignore): the land says so.
const SWEEP_MAX: usize = 200;

/// What a land takes: the agent's files that differ from the tip, and
/// the new files it leaves out (no agent claimed them).
struct Picked {
    mine: Vec<String>,
    left_out: Vec<String>,
}

/// The agent's files of the place that differ from its tip, refused when
/// another agent of the place changed one too. Its files: the ones it
/// wrote with a file tool, the ones it names (`--add`), and in a worktree
/// it has alone, every new file not ignored (bash made them: a generator,
/// a download, a new folder). Elsewhere (the shared folder, a shared
/// worktree) a new file nobody claimed may be anyone's: it is left out,
/// and named ([`Picked::left_out`]), never swept in.
fn own_changes(job: &Job) -> Result<Picked, String> {
    let news = untracked(&job.dir)?;
    let theirs: Vec<String> = job.others.iter().flat_map(|(_, fs)| fs.iter().filter_map(|x| relative(&job.dir, x))).collect();
    let mut claim: Vec<String> = job.files.iter().filter_map(|f| relative(&job.dir, f)).collect();
    let mut named: Vec<(String, Vec<String>)> = Vec::new();
    for a in &job.add {
        let p = relative(&job.dir, a).ok_or_else(|| format!("{}: not in your place's folder ({})", a, job.dir.display()))?;
        let p = p.trim_end_matches('/').to_string();
        // a folder: its new files, not another agent's (named one by one,
        // another agent's file is refused below)
        let under: Vec<String> = news
            .iter()
            .filter(|u| **u == p || ((u.starts_with(&format!("{}/", p)) || p == ".") && !theirs.contains(u)))
            .cloned()
            .collect();
        let paths = if under.is_empty() { vec![p.clone()] } else { under };
        claim.extend(paths.iter().cloned());
        named.push((a.clone(), paths));
    }
    let unclaimed: Vec<String> = news.iter().filter(|u| !claim.contains(u) && !theirs.contains(u)).cloned().collect();
    let left_out = if job.worktree && job.others.is_empty() {
        if unclaimed.len() > SWEEP_MAX {
            return Err(format!(
                "{} new files not ignored in the worktree (first: {}): add them to .gitignore, or land the ones you want with --add <path>",
                unclaimed.len(),
                unclaimed.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        claim.extend(unclaimed);
        Vec::new()
    } else {
        unclaimed.into_iter().filter(|f| made_since(&job.dir.join(f), job.since_ms)).collect()
    };
    let mut seen = std::collections::BTreeSet::new();
    claim.retain(|f| seen.insert(f.clone()));
    let mine = changed(&job.dir, &claim)?;
    if let Some((a, _)) = named.iter().find(|(_, ps)| !ps.iter().any(|p| mine.contains(p))) {
        return Err(format!("--add {}: no new or changed file there", a));
    }
    for f in &mine {
        let who: Vec<&str> = job
            .others
            .iter()
            .filter(|(_, fs)| fs.iter().filter_map(|x| relative(&job.dir, x)).any(|x| &x == f))
            .map(|(n, _)| n.as_str())
            .collect();
        if !who.is_empty() {
            return Err(format!(
                "{} is also changed by @{}: not landed, main decides who lands it",
                f,
                who.join(", @")
            ));
        }
    }
    Ok(Picked { mine, left_out })
}

/// `path` was last written at or after `since_ms` (unknown: yes).
fn made_since(path: &Path, since_ms: u64) -> bool {
    let ms = std::fs::symlink_metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    ms.is_none_or(|ms| ms >= since_ms)
}

/// The land's word on new files it left out: which, and how to land
/// them. "" when none.
pub fn left_out_note(files: &[String]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = files.iter().take(8).map(String::as_str).collect();
    let more = if files.len() > shown.len() { format!(" (+{} more)", files.len() - shown.len()) } else { String::new() };
    format!(
        "left out {} new file{} no agent claimed: {}{}. yours (made by bash)? land {}: sb land --here --add <file or folder> \"<message>\"",
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        shown.join(", "),
        more,
        if files.len() == 1 { "it" } else { "them" }
    )
}

pub(crate) fn shorten(dir: &Path, sha: &str) -> String {
    git(dir, &["rev-parse", "--short", sha]).unwrap_or_else(|_| sha.chars().take(7).collect())
}

/// Run `check` in `dir` (`sh -c`); its output's tail when it fails.
pub(crate) fn run_check(dir: &Path, check: &str) -> Result<(), String> {
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(check)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("the check `{}` could not start: {}", check, e))?;
    if out.status.success() {
        return Ok(());
    }
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Err(format!("the check `{}` failed: {}", check, crate::util::clip_tail(text.trim(), 600)))
}

/// Push `target` to its remote (trunk flow, `push = true`): a refused
/// push fetches; main behind the remote is rebased (only in a clean
/// shared folder), then one more try. Err: why it is not pushed.
pub(crate) fn push(shared: &Path, target: &str) -> Result<(), String> {
    let remote = git(shared, &["remote"])?.lines().next().map(str::to_string);
    let Some(remote) = remote else {
        return Err("no remote".into());
    };
    let b = short(target);
    let spec = format!("{}:{}", target, target);
    if git(shared, &["push", "-q", &remote, &spec]).is_ok() {
        return Ok(());
    }
    git(shared, &["fetch", "-q", &remote, b])?;
    let theirs = format!("{}/{}", remote, b);
    let behind = git(shared, &["merge-base", "--is-ancestor", &theirs, target]).is_err();
    if behind {
        let clean = git(shared, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
        if !clean || head_ref(shared).as_deref() != Ok(target) {
            return Err(format!("{} moved on {}; not rebased: the shared folder has changes", b, remote));
        }
        git(shared, &["rebase", "-q", &theirs]).map_err(|e| {
            let conflicts = git(shared, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            let _ = git(shared, &["rebase", "--abort"]);
            let other = format!("{} moved on {} and the rebase conflicts: {}", b, remote, e);
            if conflicts.is_empty() {
                rebase_failed(shared, &e, "push again", other)
            } else {
                other
            }
        })?;
    }
    git(shared, &["push", "-q", &remote, &spec]).map(|_| ())
}

fn pushed(job: &Job, target: &str, main: &str) -> (Option<bool>, Option<String>) {
    if job.flow.mode != Some(FlowMode::Trunk) || !job.flow.push || target != main {
        return (None, None);
    }
    match push(&job.shared, target) {
        Ok(()) => (Some(true), None),
        Err(e) if e == "no remote" => (None, None),
        Err(e) => (Some(false), Some(e)),
    }
}

/// `sb land` for `job`, in line on `queue`. `joined`: the views refresh.
pub fn run(job: &Job, queue: &Queue, joined: &mut dyn FnMut()) -> Result<Outcome, String> {
    let main = head_ref(&job.shared)?;
    if !job.here && job.worktree && job.flow.mode == Some(FlowMode::Pr) {
        return Err("this repo ships through pull requests: commit with `sb land --here`, then open a PR".into());
    }
    let Picked { mine, left_out } = own_changes(job)?;
    if !job.here && job.worktree {
        // a feature's agent lands on the feature (dev-flow §5.1)
        let onto = job.onto.clone().unwrap_or_else(|| main.clone());
        return land_branch(job, queue, joined, &onto, &main, &mine, left_out);
    }
    if mine.is_empty() {
        let note = left_out_note(&left_out);
        let note = if note.is_empty() { note } else { format!("; {}", note) };
        return Err(format!("nothing of yours to land: the files you changed match the branch{}", note));
    }
    if job.message.trim().is_empty() {
        return Err("sb land needs a commit message: sb land [--here] \"<message>\"".into());
    }
    let target = head_ref(&job.dir)?;
    let _turn = queue.wait_turn(&target, &job.place, joined);
    let new = commit_files(&job.dir, &target, &mine, &job.message, &mut || {})?;
    let (pushed, push_error) = pushed(job, &target, &main);
    Ok(Outcome {
        target: short(&target).to_string(),
        sha: shorten(&job.dir, &new),
        commits: 1,
        pushed,
        push_error,
        left_out,
    })
}

/// Move `main` (a ref) from `base` to `tip`, a fast-forward: in the
/// shared folder when it has `main` checked out (its files move too,
/// refused when someone's changes are in the way), else a compare-and-
/// swap of the ref.
pub(crate) fn move_main(shared: &Path, main: &str, base: &str, tip: &str) -> Result<(), String> {
    if head_ref(shared).as_deref() == Ok(main) {
        git(shared, &["merge", "--ff-only", "-q", tip]).map(|_| ()).map_err(|e| {
            format!("{} could not move: {} (the shared folder has changes on the same files, or {} moved)", short(main), e, short(main))
        })
    } else {
        git(shared, &["update-ref", main, tip, base]).map(|_| ()).map_err(|_| format!("{} moved meanwhile: sb land again", short(main)))
    }
}

/// A worktree's branch onto `main` (the default branch, or the agent's
/// feature: `real_main` says which is pushed): its agent's files first,
/// then rebase, check, fast-forward.
fn land_branch(
    job: &Job,
    queue: &Queue,
    joined: &mut dyn FnMut(),
    main: &str,
    real_main: &str,
    mine: &[String],
    left_out: Vec<String>,
) -> Result<Outcome, String> {
    let branch = head_ref(&job.dir)?;
    if !mine.is_empty() {
        if job.message.trim().is_empty() {
            return Err("you have changes not committed: give a message (sb land \"<message>\"), or commit them with sb land --here first".into());
        }
        commit_files(&job.dir, &branch, mine, &job.message, &mut || {})?;
    }
    let dirty = git(&job.dir, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        let files: Vec<&str> = dirty.lines().map(|l| l.get(3..).unwrap_or(l)).take(5).collect();
        return Err(format!(
            "the worktree has changes not committed ({}): each agent lands its own with sb land --here first",
            files.join(", ")
        ));
    }
    let _turn = queue.wait_turn(main, &job.place, joined);
    let base = git(&job.shared, &["rev-parse", "--verify", main])?;
    let tip = git(&job.dir, &["rev-parse", "HEAD"])?;
    if git(&job.dir, &["merge-base", "--is-ancestor", &tip, &base]).is_ok() {
        return Err(format!("nothing to land: {} has no commit that {} lacks", short(&branch), short(main)));
    }
    let moved = git(&job.dir, &["merge-base", "--is-ancestor", &base, &tip]).is_err();
    if moved {
        if let Err(e) = git(&job.dir, &["rebase", "-q", &base]) {
            let conflicts = git(&job.dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            let _ = git(&job.dir, &["rebase", "--abort"]);
            let files: Vec<&str> = conflicts.lines().collect();
            return Err(if files.is_empty() {
                rebase_failed(&job.dir, &e, "land again", format!("the rebase on {} failed: {}", short(main), e))
            } else {
                format!(
                    "{} changed on {} too: the rebase conflicts. rebase {} on {} yourself, then sb land again",
                    files.join(", "),
                    short(main),
                    short(&branch),
                    short(main)
                )
            });
        }
        if let Some(check) = &job.flow.check {
            run_check(&job.dir, check)?;
        }
    }
    let tip = git(&job.dir, &["rev-parse", "HEAD"])?;
    let commits = git(&job.dir, &["rev-list", "--count", &format!("{}..{}", base, tip)])?
        .parse::<usize>()
        .unwrap_or(0);
    move_main(&job.shared, main, &base, &tip)?;
    // a feature is never pushed (pushed: only when the target is main)
    let (pushed, push_error) = pushed(job, main, real_main);
    Ok(Outcome {
        target: short(main).to_string(),
        sha: shorten(&job.shared, &tip),
        commits,
        pushed,
        push_error,
        left_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, script: &str) {
        let ok = Command::new("/bin/sh").arg("-c").arg(script).current_dir(dir).status().unwrap().success();
        assert!(ok, "{}", script);
    }

    fn repo(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sb-land-test-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()));
        let ws = root.join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        sh(
            &ws,
            "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo a > a && echo b > b && echo c > c && git add . && git commit -qm init",
        );
        ws.canonicalize().unwrap()
    }

    fn job(agent: &str, dir: &Path, shared: &Path, files: &[&str], others: &[(&str, &[&str])]) -> Job {
        Job {
            agent: agent.into(),
            here: true,
            message: format!("{}'s work", agent),
            place: "shared".into(),
            dir: dir.to_path_buf(),
            worktree: dir != shared,
            shared: shared.to_path_buf(),
            files: files.iter().map(|s| s.to_string()).collect(),
            others: others
                .iter()
                .map(|(n, fs)| (n.to_string(), fs.iter().map(|s| s.to_string()).collect()))
                .collect(),
            add: Vec::new(),
            since_ms: 0,
            flow: FlowConfig::default(),
            onto: None,
        }
    }

    fn lines(xs: &[&str]) -> String {
        xs.join("\u{a}")
    }

    fn log(dir: &Path) -> String {
        git(dir, &["log", "--format=%s", "main"]).unwrap()
    }

    #[test]
    fn land_here_commits_only_my_files_and_keeps_the_status_clean() {
        let ws = repo("here");
        // mine: a (changed), n (new); another agent's: b; the user's: c
        sh(&ws, "echo a2 > a && echo new > n && echo b2 > b && echo c2 > c");
        let abs_a = ws.join("a").to_string_lossy().to_string();
        let j = job("x", &ws, &ws, &[&abs_a, "n", "/elsewhere/z"], &[("y", &["b"])]);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!((o.target.as_str(), o.commits, o.pushed), ("main", 1, None));
        assert_eq!(log(&ws), lines(&["x's work", "init"]));
        let files = git(&ws, &["show", "--name-only", "--format=", "main"]).unwrap();
        assert_eq!(files.lines().collect::<Vec<_>>(), ["a", "n"]);
        // the shared index follows: a and n clean, b and c still the others'
        let st = git(&ws, &["status", "--porcelain"]).unwrap();
        assert_eq!(st.lines().collect::<Vec<_>>(), [" M b", " M c"]);
        // again: nothing of mine left
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("nothing of yours"), "{}", e);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn an_overlap_is_refused() {
        let ws = repo("overlap");
        sh(&ws, "echo a2 > a");
        let j = job("x", &ws, &ws, &["a"], &[("y", &["a", "b"])]);
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("a is also changed by @y") && e.contains("main decides"), "{}", e);
        assert_eq!(log(&ws), "init", "nothing landed");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_cas_race_keeps_the_other_commit() {
        let ws = repo("cas");
        sh(&ws, "echo a2 > a");
        let mut once = true;
        let mut race = || {
            if std::mem::take(&mut once) {
                // someone commits b on main between our read and our swap
                let w = ws.clone();
                sh(&w, "git worktree add -q ../other main 2>/dev/null || git worktree add -q --detach ../other main; cd ../other && echo b2 > b && git -c commit.gpgsign=false commit -qam other && git update-ref refs/heads/main HEAD");
            }
        };
        let new = commit_files(&ws, "refs/heads/main", &["a".into()], "mine", &mut race).unwrap();
        assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), new);
        assert_eq!(log(&ws), lines(&["mine", "other", "init"]), "rebuilt on the moved tip");
        assert_eq!(git(&ws, &["show", "main:b"]).unwrap(), "b2", "their commit is kept");
        assert_eq!(git(&ws, &["show", "main:a"]).unwrap(), "a2");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn two_agents_in_one_worktree_then_the_branch_lands_on_main() {
        let ws = repo("wt");
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        // x and y share the worktree; each lands only its own file
        sh(&wt, "echo a2 > a && echo b2 > b");
        let mut jx = job("x", &wt, &ws, &["a"], &[("y", &["b"])]);
        jx.place = "wt:x".into();
        run(&jx, &Queue::default(), &mut || {}).unwrap();
        let st = git(&wt, &["status", "--porcelain"]).unwrap();
        assert_eq!(st.trim(), "M b", "y's file is untouched");
        // the plain land refuses while y's file is not committed
        jx.here = false;
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("not committed (b)"), "{}", e);
        let mut jy = job("y", &wt, &ws, &["b"], &[("x", &["a"])]);
        jy.place = "wt:x".into();
        run(&jy, &Queue::default(), &mut || {}).unwrap();
        // main moved meanwhile (c): the branch is rebased, checked, then main
        // fast-forwards, the shared folder's files with it
        sh(&ws, "echo c2 > c && git commit -qam c-on-main");
        jx.flow.check = Some("test -f a".into());
        let o = run(&jx, &Queue::default(), &mut || {}).unwrap();
        assert_eq!((o.target.as_str(), o.commits), ("main", 2));
        assert_eq!(log(&ws), lines(&["y's work", "x's work", "c-on-main", "init"]));
        assert_eq!(std::fs::read_to_string(ws.join("a")).unwrap().trim_end(), "a2");
        assert!(git(&ws, &["status", "--porcelain"]).unwrap().is_empty());
        // a failing check after a rebase: main does not move
        sh(&ws, "echo c3 > c && git commit -qam c3");
        sh(&wt, "echo a3 > a");
        jx.flow.check = Some("echo broken; exit 1".into());
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("the check `echo broken; exit 1` failed: broken"), "{}", e);
        assert_eq!(log(&ws).lines().next(), Some("c3"));
        // PR flow: no landing on main
        jx.flow.mode = Some(FlowMode::Pr);
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("pull requests"), "{}", e);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_queue_lands_one_at_a_time_per_ref_and_says_who_waits() {
        let q = Queue::default();
        let first = q.wait_turn("refs/heads/main", "wt:a", &mut || {});
        let q2 = q.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let h = std::thread::spawn(move || {
            let _t = q2.wait_turn("refs/heads/main", "wt:b", &mut || {});
            tx.send(()).unwrap();
        });
        // another ref is not in that line
        let other = q.wait_turn("refs/heads/sb/c", "wt:c", &mut || {});
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(rx.try_recv().is_err(), "b waits while a lands");
        let lids = q.lids();
        assert_eq!(lids["wt:a"], "landing");
        assert_eq!(lids["wt:b"], "waits to land · 2nd");
        assert_eq!(lids["wt:c"], "landing");
        drop(first);
        rx.recv_timeout(std::time::Duration::from_secs(5)).expect("b's turn");
        h.join().unwrap();
        drop(other);
        assert!(q.lids().is_empty());
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(11), "11th");
    }

    #[test]
    fn trunk_pushes_main_after_a_land() {
        let ws = repo("push");
        let remote = ws.parent().unwrap().join("remote.git");
        sh(&ws, &format!("git init -q --bare {} && git remote add origin {} && git push -q origin main", remote.display(), remote.display()));
        sh(&ws, "echo a2 > a");
        let mut j = job("x", &ws, &ws, &["a"], &[]);
        j.flow.mode = Some(FlowMode::Trunk);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(o.pushed, Some(true));
        assert_eq!(git(&remote, &["log", "--format=%s", "-1", "main"]).unwrap(), "x's work");
        // the remote moved: fetch, rebase (the shared folder is clean), push
        let other = ws.parent().unwrap().join("other");
        sh(&ws, &format!("git clone -q {} {} && cd {} && git config user.email o@o && git config user.name o && echo c2 > c && git -c commit.gpgsign=false commit -qam theirs && git push -q origin main", remote.display(), other.display(), other.display()));
        sh(&ws, "echo b2 > b");
        let mut j = job("x", &ws, &ws, &["b"], &[]);
        j.flow.mode = Some(FlowMode::Trunk);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(o.pushed, Some(true), "{:?}", o.push_error);
        assert_eq!(git(&remote, &["log", "--format=%s", "main"]).unwrap(), lines(&["x's work", "theirs", "x's work", "init"]));
        // push = false: lands stay local
        sh(&ws, "echo a3 > a");
        let mut j = job("x", &ws, &ws, &["a"], &[]);
        j.flow = FlowConfig { mode: Some(FlowMode::Trunk), check: None, push: false, ..FlowConfig::default() };
        assert_eq!(run(&j, &Queue::default(), &mut || {}).unwrap().pushed, None);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    fn landed(dir: &Path, rev: &str) -> Vec<String> {
        git(dir, &["show", "--name-only", "--format=", rev]).unwrap().lines().map(str::to_string).collect()
    }

    #[test]
    fn a_worktree_alone_lands_the_new_files_bash_made_and_its_new_folders() {
        // amb-kit's fonts (a new folder, fetched by curl) and launch's svgs
        // (written by a generator run through bash): no file tool saw them
        let ws = repo("wt-new");
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        sh(
            &wt,
            "echo gen > a && mkdir -p kit/fonts feat && echo w > kit/fonts/n.woff2 && echo l > kit/fonts/OFL.txt && echo s > feat/v-light.svg && echo ignored > junk.log",
        );
        // an ignored file stays out (git's own rule; the worktrees share
        // the repo's info/exclude)
        sh(&ws, "echo '*.log' >> .git/info/exclude");
        let mut j = job("x", &wt, &ws, &["a"], &[]);
        j.place = "wt:x".into();
        j.here = false;
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert!(o.left_out.is_empty(), "{:?}", o.left_out);
        assert_eq!(landed(&ws, "main"), ["a", "feat/v-light.svg", "kit/fonts/OFL.txt", "kit/fonts/n.woff2"]);
        assert_eq!(git(&ws, &["show", "main:kit/fonts/n.woff2"]).unwrap(), "w");
        let st = git(&wt, &["status", "--porcelain"]).unwrap();
        assert!(!st.contains("kit") && !st.contains("feat") && !st.contains("junk"), "{}", st);
        // --here too, with only new files (no file tool at all)
        sh(&wt, "mkdir -p more/deep && echo d > more/deep/x.txt");
        let mut j = job("x", &wt, &ws, &[], &[]);
        j.place = "wt:x".into();
        run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(landed(&wt, "HEAD"), ["more/deep/x.txt"]);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_shared_folder_names_the_new_files_it_left_out_and_lands_them_with_add() {
        let ws = repo("shared-new");
        // the user's old scratch file: not news, never listed
        sh(&ws, "echo old > scratch.txt && touch -t 200001010000 scratch.txt");
        // launch: make.py edited with a tool, run through bash: 2 new svgs in
        // a new folder; another agent's new file (write_file) next to them
        sh(&ws, "echo gen2 > a && mkdir -p feat && echo l > feat/v-light.svg && echo d > feat/v-dark.svg && echo y > feat/theirs.txt");
        let mut j = job("x", &ws, &ws, &["a"], &[("y", &["feat/theirs.txt"])]);
        j.since_ms = crate::util::now_ms() - 60_000;
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(landed(&ws, "main"), ["a"], "never swept in: they may be anyone's");
        assert_eq!(o.left_out, ["feat/v-dark.svg", "feat/v-light.svg"], "named, not dropped silently");
        let note = left_out_note(&o.left_out);
        assert!(note.contains("left out 2 new files") && note.contains("feat/v-dark.svg") && note.contains("--add"), "{}", note);
        // nothing else of mine: the error names them too
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("nothing of yours") && e.contains("feat/v-light.svg") && e.contains("--add"), "{}", e);
        // --add the folder: the svgs land, y's file and the user's stay out
        j.add = vec![ws.join("feat").to_string_lossy().to_string()];
        j.message = "the svgs".into();
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert!(o.left_out.is_empty(), "{:?}", o.left_out);
        assert_eq!(landed(&ws, "main"), ["feat/v-dark.svg", "feat/v-light.svg"]);
        let st = git(&ws, &["status", "--porcelain"]).unwrap();
        assert_eq!(st.lines().collect::<Vec<_>>(), ["?? feat/theirs.txt", "?? scratch.txt"]);
        // --add another agent's file: refused, main decides
        j.add = vec!["feat/theirs.txt".into()];
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("also changed by @y"), "{}", e);
        // --add a path with nothing new: said, not ignored
        j.add = vec!["nope".into()];
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("--add nope: no new or changed file"), "{}", e);
        assert_eq!(log(&ws), lines(&["the svgs", "x's work", "init"]));
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn a_shared_worktree_never_sweeps_in_new_files_nobody_claimed() {
        let ws = repo("wt-shared-new");
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        // x and y share the worktree: y made gen/y.txt by bash, its own
        // y-new.txt with a tool
        sh(&wt, "echo a2 > a && mkdir gen && echo y > gen/y.txt && echo n > y-new.txt");
        let mut jx = job("x", &wt, &ws, &["a"], &[("y", &["y-new.txt"])]);
        jx.place = "wt:x".into();
        let o = run(&jx, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(landed(&wt, "HEAD"), ["a"]);
        assert_eq!(o.left_out, ["gen/y.txt"]);
        // the plain land: the untracked files do not block, and are named
        jx.here = false;
        let o = run(&jx, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(log(&ws), lines(&["x's work", "init"]));
        assert_eq!(o.left_out, ["gen/y.txt"]);
        assert!(!ws.join("gen").exists() && wt.join("y-new.txt").exists());
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn too_many_new_files_in_a_worktree_are_not_swept_in() {
        let ws = repo("wt-many");
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        sh(&wt, &format!("mkdir out && for i in $(seq 1 {}); do echo $i > out/$i; done && echo a2 > a", SWEEP_MAX + 1));
        let mut j = job("x", &wt, &ws, &["a"], &[]);
        j.place = "wt:x".into();
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains(&format!("{} new files not ignored", SWEEP_MAX + 1)) && e.contains(".gitignore"), "{}", e);
        assert_eq!(git(&wt, &["log", "--format=%s"]).unwrap(), "init", "nothing landed");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    // ---- signing: a throwaway ssh key in the test's temp folder, the
    // repo's own config (never the user's key or config) ----

    /// A throwaway ed25519 key next to the repo; the repo signs with it
    /// (`gpg.format ssh`, the private key file: no ssh agent asked).
    fn sign_with_throwaway_key(ws: &Path) -> PathBuf {
        let key = ws.parent().unwrap().join("throwaway-key");
        sh(ws, &format!("ssh-keygen -q -t ed25519 -N '' -C throwaway -f {}", key.display()));
        sh(
            ws,
            &format!(
                "git config gpg.format ssh && git config user.signingkey {} && git config commit.gpgsign true",
                key.display()
            ),
        );
        key
    }

    fn signed(dir: &Path, rev: &str) -> bool {
        git(dir, &["cat-file", "-p", rev]).unwrap().contains("gpgsig -----BEGIN SSH SIGNATURE-----")
    }

    fn assert_signing_line(e: &str, again: &str) {
        assert!(e.starts_with("commit signing failed: "), "{}", e);
        assert!(e.ends_with(&format!("; your work is kept, {} once signing works", again)), "{}", e);
        assert!(e.contains("missing-key"), "git's words name the key: {}", e);
        assert!(!e.contains('\n') && !e.contains("hint:"), "one line: {}", e);
    }

    #[test]
    fn a_land_signs_when_commit_gpgsign_is_true() {
        let ws = repo("sign-on");
        sign_with_throwaway_key(&ws);
        sh(&ws, "echo a2 > a");
        run(&job("x", &ws, &ws, &["a"], &[]), &Queue::default(), &mut || {}).unwrap();
        assert_eq!(log(&ws), lines(&["x's work", "init"]));
        assert!(signed(&ws, "main"), "{}", git(&ws, &["cat-file", "-p", "main"]).unwrap());
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn a_land_is_unsigned_when_commit_gpgsign_is_false() {
        let ws = repo("sign-off");
        let key = sign_with_throwaway_key(&ws);
        sh(&ws, "git config commit.gpgsign false");
        sh(&ws, "echo a2 > a");
        run(&job("x", &ws, &ws, &["a"], &[]), &Queue::default(), &mut || {}).unwrap();
        assert!(!signed(&ws, "main"));
        assert!(key.exists());
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn a_missing_key_fails_the_land_in_one_line_and_keeps_the_work() {
        let ws = repo("sign-missing");
        sign_with_throwaway_key(&ws);
        let missing = ws.parent().unwrap().join("missing-key");
        sh(&ws, &format!("git config user.signingkey {}", missing.display()));
        sh(&ws, "echo a2 > a");
        let before = git(&ws, &["rev-parse", "main"]).unwrap();
        let e = run(&job("x", &ws, &ws, &["a"], &[]), &Queue::default(), &mut || {}).unwrap_err();
        assert_signing_line(&e, "land again");
        // never an unsigned commit: main did not move, the change is there
        assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), before);
        assert_eq!(std::fs::read_to_string(ws.join("a")).unwrap().trim_end(), "a2");
        assert_eq!(git(&ws, &["status", "--porcelain"]).unwrap().trim(), "M a");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn a_land_that_rebases_signs_and_a_missing_key_keeps_the_branch() {
        let ws = repo("sign-rebase");
        let key = sign_with_throwaway_key(&ws);
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        sh(&wt, "echo a2 > a");
        let mut j = job("x", &wt, &ws, &["a"], &[]);
        j.place = "wt:x".into();
        run(&j, &Queue::default(), &mut || {}).unwrap();
        assert!(signed(&wt, "HEAD"), "the --here commit is signed");
        // main moves: the plain land rebases, and the rebased commit is signed
        sh(&ws, "echo c2 > c && git commit -qam c-on-main");
        j.here = false;
        run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(log(&ws), lines(&["x's work", "c-on-main", "init"]));
        assert!(signed(&ws, "main"), "the rebased commit is signed");
        // the key goes missing: the rebase fails on signing, nothing moves
        sh(&wt, "echo a3 > a");
        j.here = true;
        run(&j, &Queue::default(), &mut || {}).unwrap();
        sh(&ws, &format!("git config user.signingkey {}", key.display()));
        sh(&ws, "echo c3 > c && git commit -qam c3");
        let missing = ws.parent().unwrap().join("missing-key");
        sh(&ws, &format!("git config user.signingkey {}", missing.display()));
        let (main_before, branch_before) = (git(&ws, &["rev-parse", "main"]).unwrap(), git(&wt, &["rev-parse", "HEAD"]).unwrap());
        j.here = false;
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert_signing_line(&e, "land again");
        assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), main_before);
        assert_eq!(git(&wt, &["rev-parse", "HEAD"]).unwrap(), branch_before, "the branch keeps its commit");
        assert!(git(&wt, &["status", "--porcelain"]).unwrap().is_empty(), "the rebase was aborted");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_signing_line_keeps_gits_words_and_drops_its_hints() {
        let e = "git rebase: error: Couldn't load public key /k/missing-key: No such file or directory?\n\nerror: failed to write commit object\nhint: Could not execute the todo command\nhint:     git rebase --continue";
        assert_eq!(
            signing_failed(e, "land again"),
            "commit signing failed: Couldn't load public key /k/missing-key: No such file or directory?; failed to write commit object; your work is kept, land again once signing works"
        );
        assert!(signing_error("error: gpg failed to sign the data\nfatal: failed to write commit object"));
        assert!(!signing_error("git rebase: error: could not apply 1234... x"));
    }
}
