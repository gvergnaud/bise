//! Feature branches in trunk flow (dev-flow §5.1): one feature on its
//! own local branch, several agents each in a worktree landing onto it,
//! a build the user tries, a merge on the user's go only.
//!
//! - A **feature** is a local branch named after it (`computer-use`, no
//!   prefix), made from main's tip (`trunk::trunk_ref`: the default
//!   branch, whatever the shared folder has checked out; issue #8) (or
//!   an existing local branch, adopted:
//!   the by-hand recipe of approvals and computer-use), never pushed.
//! - Its agents: `sb spawn --feature <name>` gives each a worktree on
//!   `sb/<agent>` from the feature's tip (`Workspace::feature`); `sb land`
//!   rebases that branch on the feature and moves the feature, never
//!   main (`land::Job::onto`).
//! - The place table (`crate::place`) groups them: a place of kind
//!   `feature`, id `feature:<name>`.
//! - The registry (`<state>/features.json`) keeps what git does not: when
//!   it was made, the tip the check last passed on, the last try.
//! - Sync and merge ([`rebase`], [`merge`]): a rebase on main, but a
//!   feature that holds merges ([`has_merges`]: it merged main in) gets
//!   main merged into it, and lands as one merge commit (`merge --no-ff`,
//!   first parent main's tip): never a rebase of its merges, nor
//!   `--rebase-merges`. A race on a ref: once more, then [`Failed::Moved`].
//! - His answer that failed is never asked again: the hub closes its item
//!   [`HANDED`] and main gets [`handoff`]'s ready brief
//!   (brief-feature-merge, #780 -> #782 -> #789).
//!
//! THE CONTRACT (with the TUI and pr-merge, docs/dev-flow.md §5.1, §7):
//! - the place id [`place_id`] (`feature:<name>`), the card kinds
//!   [`TRY`] and [`MERGE`] with `place` = that id, their options (the
//!   TUI's, by kind: [`TRY_OPTIONS`], [`MERGE_OPTIONS`], the drop's
//!   second ask [`DROP_OPTIONS`] and [`drop_question`]) and the answers
//!   the hub takes (`1`..`3`; for MERGE, `3` only after the second ask);
//! - `PlaceView { feature: true, trying }` and its lid ([`lid`]).
//!
//! Git runs here, in the daemon's threads (never on the hub's loop),
//! through `land`'s helpers; the words are here too, pure and tested.

use crate::land::{git, short};
use crate::scratch::Scratch;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The inbox kinds (pr-merge's choice cards): "<name> is ready to try",
/// then "merge <name> into main?".
pub const TRY: &str = "feature_try";
pub const MERGE: &str = "feature_merge";
pub const TRY_OPTIONS: [&str; 3] = ["try it", "show the diff", "not yet"];
pub const MERGE_OPTIONS: [&str; 3] = ["merge", "keep working", "drop the branch"];
/// MERGE's `3` asks once more on the same item (designer, d4172e6).
pub const DROP_OPTIONS: [&str; 2] = ["drop it", "keep it"];

/// The place id of feature `name`.
pub fn place_id(name: &str) -> String {
    format!("feature:{}", name)
}

/// The feature of a place id.
pub fn of_place(id: &str) -> Option<&str> {
    id.strip_prefix("feature:")
}

/// A feature as the registry keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    /// main's tip when it was made (adopted: where it left main).
    pub base: String,
    pub created_ms: u64,
    /// An existing local branch `sb feature new` took as it was.
    #[serde(default)]
    pub adopted: bool,
    /// The tip the check last passed on (full sha).
    #[serde(default)]
    pub checked: Option<String>,
    /// The last try build.
    #[serde(default)]
    pub tried: Option<Tried>,
    /// On trial: built, and the user has not said merge, keep working or
    /// drop yet (the sidebar's Δ, with a build running).
    #[serde(default)]
    pub trial: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tried {
    /// The tip built (short).
    pub sha: String,
    /// What to run, in the user's words: `~/.bise/dev/versions/fd25c45/bise`.
    pub run: String,
    pub at_ms: u64,
}

/// The features of a repo, `<state>/features.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub features: Vec<Feature>,
}

impl Registry {
    pub fn file(state: &Path) -> PathBuf {
        state.join("features.json")
    }

    pub fn load(state: &Path) -> Registry {
        std::fs::read_to_string(Registry::file(state))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Written whole, through a temp file and a rename.
    pub fn save(&self, state: &Path) -> Result<(), String> {
        let f = Registry::file(state);
        let tmp = f.with_extension(format!("json.{}", std::process::id()));
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(state).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {}", tmp.display(), e))?;
        std::fs::rename(&tmp, &f).map_err(|e| format!("{}: {}", f.display(), e))
    }

    pub fn get(&self, name: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.name == name)
    }

    pub fn remove(&mut self, name: &str) {
        self.features.retain(|f| f.name != name);
    }
}

/// What git says of a feature against main, for the words.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facts {
    /// Commits on the feature that main lacks, and the other way.
    pub ahead: usize,
    pub behind: usize,
    pub adds: usize,
    pub dels: usize,
    /// The feature's tip (full sha).
    pub tip: String,
}

/// The feature name rules: a task name's (`[a-z0-9-]`, 24 max; the user
/// types it), never the default branch, never `sb/…`.
pub fn valid(name: &str, main: &str) -> Result<(), String> {
    if !crate::router::valid_name(name) || name.ends_with('-') {
        return Err(format!("invalid feature name: {} ([a-z0-9-], 24 characters max)", name));
    }
    if name == short(main) || name == "main" || name == "master" || name == "HEAD" {
        return Err(format!("{} is the default branch: a feature needs its own name", name));
    }
    Ok(())
}

fn branch_ref(name: &str) -> String {
    format!("refs/heads/{}", name)
}

pub fn exists(shared: &Path, name: &str) -> bool {
    git(shared, &["show-ref", "--verify", "--quiet", &branch_ref(name)]).is_ok()
}

/// The facts of `name` against `main` (a ref).
pub fn facts(shared: &Path, main: &str, name: &str) -> Result<Facts, String> {
    let b = branch_ref(name);
    let tip = git(shared, &["rev-parse", "--verify", &b])?;
    let lr = git(shared, &["rev-list", "--left-right", "--count", &format!("{}...{}", main, b)])?;
    let mut n = lr.split_whitespace().map(|x| x.parse::<usize>().unwrap_or(0));
    let (behind, ahead) = (n.next().unwrap_or(0), n.next().unwrap_or(0));
    let stat = git(shared, &["diff", "--shortstat", &format!("{}...{}", main, b)]).unwrap_or_default();
    let num = |word: &str| {
        stat.split(',')
            .find(|p| p.contains(word))
            .and_then(|p| p.split_whitespace().next())
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(0)
    };
    Ok(Facts { ahead, behind, adds: num("insertion"), dels: num("deletion"), tip })
}

/// `sb feature new <name>`: the branch from main's tip, or an existing
/// local branch adopted as it is (computer-use's, made by hand). The
/// feature and whether it was adopted.
pub fn create(shared: &Path, name: &str, now: u64) -> Result<Feature, String> {
    let main = crate::trunk::trunk_ref(shared)?;
    valid(name, &main)?;
    if exists(shared, name) {
        let base = git(shared, &["merge-base", &main, &branch_ref(name)])
            .map_err(|_| format!("{} shares no history with {}: not adopted", name, short(&main)))?;
        return Ok(Feature { name: name.into(), base, created_ms: now, adopted: true, ..Feature::default() });
    }
    let base = git(shared, &["rev-parse", "--verify", &main])?;
    git(shared, &["branch", name, &base])?;
    Ok(Feature { name: name.into(), base, created_ms: now, ..Feature::default() })
}

/// Why a feature step failed: the words the agent's `sb feature` reads
/// (its `Display`), and what the brief handed to main says
/// ([`handoff`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// The rebase or the merge conflicts: these files, and the line.
    Conflicts { files: Vec<String>, line: String },
    /// The check (or the repo's land check) failed: its words, the tail of its output.
    Check(String),
    /// A branch moved meanwhile (the compare-and-swap), twice.
    Moved(String),
    Other(String),
}

impl Failed {
    pub fn line(&self) -> &str {
        match self {
            Failed::Conflicts { line, .. } | Failed::Check(line) | Failed::Moved(line) | Failed::Other(line) => line,
        }
    }
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.line())
    }
}

impl From<String> for Failed {
    fn from(e: String) -> Failed {
        Failed::Other(e)
    }
}

/// Whether feature `name` holds merge commits the trunk (`main`, a ref)
/// lacks (it merged main in, native-mac did): it then syncs and lands by
/// merges, never by a rebase (which fails on the merge's conflicts again,
/// or brings back what the merge deleted; never `--rebase-merges`
/// either, which re-resolves every conflict).
pub fn has_merges(shared: &Path, main: &str, name: &str) -> Result<bool, String> {
    let range = format!("{}..{}", main, branch_ref(name));
    Ok(!git(shared, &["rev-list", "--merges", "-n1", &range])?.is_empty())
}

/// Which way main and the feature meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum How {
    /// main into the feature, the feature's ref moves (`sb feature sync`, a try).
    Sync,
    /// the feature onto main, then main moves ([`merge`]).
    Merge,
}

/// The feature synced on main in a scratch worktree (rebased; merged
/// when it holds merges: [`has_merges`]), the check run there (`check`),
/// then the branch moved (compare-and-swap; a race: once more on the new
/// tips, then [`Failed::Moved`]). None: already on main's tip. Some(old
/// tip, new tip).
pub fn rebase(shared: &Path, scratch: &Path, name: &str, check: Option<&str>) -> Result<Option<(String, String)>, Failed> {
    again_once(|| integrate(shared, scratch, name, check, How::Sync, &mut |_, _| Ok(())))
}

/// `f`, once more when a branch moved under it.
fn again_once<T>(mut f: impl FnMut() -> Result<T, Failed>) -> Result<T, Failed> {
    match f() {
        Err(Failed::Moved(_)) => f(),
        r => r,
    }
}

/// [`rebase`]'s one try, `how`, with `verify(base, new tip)` before a
/// ref moves (a merge's land check): an Err there moves nothing. A
/// rebase moves the feature (a Sync, or a Merge before main
/// fast-forwards to it); a Merge of a feature with merges moves nothing
/// here: [`merge`] moves main to the merge commit. A Merge of a feature with merges is always a
/// merge commit, its first parent main's tip (even when the feature holds
/// that tip already: main never fast-forwards onto the feature's line).
fn integrate(
    shared: &Path,
    scratch: &Path,
    name: &str,
    check: Option<&str>,
    how: How,
    verify: &mut dyn FnMut(&str, &str) -> Result<(), String>,
) -> Result<Option<(String, String)>, Failed> {
    let main = crate::trunk::trunk_ref(shared)?;
    let b = branch_ref(name);
    let old = git(shared, &["rev-parse", "--verify", &b])?;
    let base = git(shared, &["rev-parse", "--verify", &main])?;
    let merges = has_merges(shared, &main, name)?;
    let on_tip = git(shared, &["merge-base", "--is-ancestor", &base, &old]).is_ok();
    if on_tip && !(how == How::Merge && merges) {
        return Ok(None);
    }
    let (from, what) = match (how, merges) {
        (_, false) => (&old, format!("the rebase of {} on {}", name, short(&main))),
        (How::Sync, true) => (&old, format!("the merge of {} into {}", short(&main), name)),
        (How::Merge, true) => (&base, format!("the merge of {} into {}", name, short(&main))),
    };
    let s = Scratch::new(shared, scratch, &format!("sync-{}", name), from)?;
    let run = match (how, merges) {
        (_, false) => git(&s.dir, &["rebase", "-q", &base]),
        (How::Sync, true) => git(&s.dir, &["merge", "--no-ff", "-q", "-m", &format!("merge {} into {}", short(&main), name), &base]),
        (How::Merge, true) => git(&s.dir, &["merge", "--no-ff", "-q", "-m", &format!("merge {}", name), &old]),
    };
    if let Err(e) = run {
        let conflicts = git(&s.dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        let _ = git(&s.dir, &[if merges { "merge" } else { "rebase" }, "--abort"]);
        let files: Vec<String> = conflicts.lines().map(str::to_string).collect();
        if files.is_empty() {
            let other = format!("{} failed: {}", what, e);
            return Err(Failed::Other(crate::land::rebase_failed(&s.dir, &e, "sync again", other)));
        }
        let line = format!("{} changed on {} too: {} conflicts", files.join(", "), short(&main), what);
        return Err(Failed::Conflicts { files, line });
    }
    if let Some(c) = check {
        crate::land::run_check(&s.dir, c).map_err(Failed::Check)?;
    }
    let new = git(&s.dir, &["rev-parse", "HEAD"])?;
    verify(&base, &new).map_err(Failed::Check)?;
    if how == How::Sync || !merges {
        git(shared, &["update-ref", &b, &new, &old]).map_err(|_| Failed::Moved(format!("{} moved meanwhile: sync it again", name)))?;
    }
    Ok(Some((old, new)))
}

/// After a sync, each agent's worktree moves to the new tip (`git rebase
/// --onto new old` there): only a clean one on a branch made from the old
/// tip. The agents whose worktree did not move, and why.
pub fn follow(worktrees: &[(String, PathBuf)], old: &str, new: &str) -> Vec<(String, String)> {
    let mut left = Vec::new();
    for (agent, dir) in worktrees {
        let clean = git(dir, &["status", "--porcelain", "--untracked-files=no"]).map(|s| s.is_empty()).unwrap_or(false);
        if !clean {
            left.push((agent.clone(), "its worktree has changes not committed".into()));
            continue;
        }
        if git(dir, &["merge-base", "--is-ancestor", old, "HEAD"]).is_err() {
            continue;
        }
        if let Err(e) = git(dir, &["rebase", "-q", "--onto", new, old]) {
            let conflicts = git(dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            let _ = git(dir, &["rebase", "--abort"]);
            let why = format!("its rebase conflicts ({})", crate::util::clip(&e, 120));
            let why = if conflicts.is_empty() { crate::land::rebase_failed(dir, &e, "sync again", why) } else { why };
            left.push((agent.clone(), why));
        }
    }
    left
}

/// The check on the feature's tip, in a scratch worktree (`sb feature
/// ready`): Ok(the tip checked).
pub fn check(shared: &Path, scratch: &Path, name: &str, check: Option<&str>) -> Result<String, String> {
    let tip = git(shared, &["rev-parse", "--verify", &branch_ref(name)])?;
    if let Some(c) = check {
        let s = Scratch::new(shared, scratch, &format!("check-{}", name), &tip)?;
        crate::land::run_check(&s.dir, c)?;
    }
    Ok(tip)
}

/// The tip kept in `refs/switchboard/trash/` (a drop, or after a merge:
/// `/restore` territory), then the local branch deleted. The ref.
pub fn trash(shared: &Path, name: &str, now: u64) -> Result<String, String> {
    let b = branch_ref(name);
    let tip = git(shared, &["rev-parse", "--verify", &b])?;
    let r = format!("refs/switchboard/trash/feature-{}/{}", name, now);
    git(shared, &["update-ref", &r, &tip])?;
    git(shared, &["update-ref", "-d", &b, &tip])?;
    Ok(r)
}

/// `sb feature retire`: how many commits of feature `name` the trunk
/// (`main`, a ref) lacks; 0: its tip is in the trunk.
pub fn missing(shared: &Path, main: &str, name: &str) -> Result<usize, String> {
    let n = git(shared, &["rev-list", "--count", &format!("{}..{}", main, branch_ref(name))])?;
    n.trim().parse::<usize>().map_err(|e| format!("git rev-list --count: {}: {}", n, e))
}

/// The refusal of a retire whose feature has work the trunk lacks.
pub fn retire_refusal(name: &str, main: &str, missing: usize) -> String {
    format!(
        "{} has {} not in {}: a retire ends only a feature whose work is on {} already. `sb feature merge {}` puts it there; `sb feature drop {}` throws it away",
        name,
        plural(missing, "commit", "commits"),
        main,
        main,
        name,
        name
    )
}

/// The retired branch's name: `<name>-retired`, else the first free
/// `<name>-retired-2`, `-3`... (`taken`: a branch by that name exists;
/// a retire never overwrites a branch).
pub fn retired_name(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let first = format!("{}-retired", name);
    if !taken(&first) {
        return first;
    }
    (2..).map(|i| format!("{}-{}", first, i)).find(|b| !taken(b)).unwrap_or(first)
}

/// The newest retired branch of `name` (a retire run again: the branch
/// is renamed already), if any.
pub fn last_retired(shared: &Path, name: &str) -> Option<String> {
    let first = format!("{}-retired", name);
    let mut last = None;
    for b in std::iter::once(first.clone()).chain((2..100).map(|i| format!("{}-{}", first, i))) {
        if !exists(shared, &b) {
            break;
        }
        last = Some(b);
    }
    last
}

/// The retire's rename: the branch keeps its commits under `to`.
pub fn rename(shared: &Path, name: &str, to: &str) -> Result<(), String> {
    git(shared, &["branch", "-m", name, to]).map(|_| ())
}

/// What a merge did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub commits: usize,
    pub sha: String,
    pub pushed: Option<bool>,
    pub push_error: Option<String>,
}

/// `1 merge` (the user's go): integrated with main and checked in a
/// scratch worktree (the check always runs: the full one): rebased, or,
/// for a feature that holds merges ([`has_merges`]), a merge commit
/// (`merge --no-ff`, first parent main's tip; the feature ref stays as
/// it was, its tip inside main, until [`trash`]: a failure in between
/// leaves a harmless ref that a later retire or drop clears). Then the
/// repo's land check on the new tip in the warm check tree under
/// `checks` (`land_check`, issue 28 part 2; red: neither the feature nor
/// main moves), main moved (compare-and-swap; a race: once more on the
/// new tip, then [`Failed::Moved`]), pushed when the flow says so. The
/// branch itself is deleted after, by [`trash`], once the agents are
/// archived. `turn`: main's land-queue turn, held by the caller.
pub fn merge(
    shared: &Path,
    scratch: &Path,
    checks: &Path,
    name: &str,
    flow: &crate::flow::FlowConfig,
    turn: &crate::land::Turn,
) -> Result<Merged, Failed> {
    let main = crate::trunk::trunk_ref(shared)?;
    let (base, tip) = again_once(|| merge_once(shared, scratch, checks, name, flow, turn, &main))?;
    let commits = git(shared, &["rev-list", "--count", &format!("{}..{}", base, tip)])?.parse::<usize>().unwrap_or(0);
    let (pushed, push_error) = if flow.mode == Some(crate::flow::FlowMode::Trunk) && flow.push {
        match crate::land::push(shared, &main) {
            Ok(()) => (Some(true), None),
            Err(e) if e == "no remote" => (None, None),
            Err(e) => (Some(false), Some(e)),
        }
    } else {
        (None, None)
    };
    Ok(Merged { commits, sha: crate::land::shorten(shared, &tip), pushed, push_error })
}

/// [`merge`]'s one try: main moved to the integrated tip. (base, tip).
fn merge_once(
    shared: &Path,
    scratch: &Path,
    checks: &Path,
    name: &str,
    flow: &crate::flow::FlowConfig,
    turn: &crate::land::Turn,
    main: &str,
) -> Result<(String, String), Failed> {
    use crate::land_check::{checked, in_check_tree, Via};
    let b = branch_ref(name);
    let base = git(shared, &["rev-parse", "--verify", main])?;
    let old = git(shared, &["rev-parse", "--verify", &b])?;
    if git(shared, &["merge-base", "--is-ancestor", &old, &base]).is_ok() {
        return Err(Failed::Other(format!("nothing to merge: {} has no commit that {} lacks", name, short(main))));
    }
    let via = Via::Merge(name);
    let land_check = |base: &str, tip: &str| in_check_tree(shared, checks, base, tip, via, turn);
    let tip = match integrate(shared, scratch, name, flow.check.as_deref(), How::Merge, &mut |b, t| {
        if checked(via, true, true) { land_check(b, t) } else { Ok(()) }
    })? {
        Some((_, new)) => new,
        None => {
            // on main's tip already: the checks still run on what lands
            let tip = check(shared, scratch, name, flow.check.as_deref()).map_err(Failed::Check)?;
            if checked(via, true, false) {
                land_check(&base, &tip).map_err(Failed::Check)?;
            }
            tip
        }
    };
    crate::land::move_main(shared, main, &base, &tip).map_err(|e| {
        let moved = git(shared, &["rev-parse", "--verify", main]).map(|now| now != base).unwrap_or(false);
        if moved { Failed::Moved(e) } else { Failed::Other(e) }
    })?;
    Ok((base, tip))
}

/// The `[flow] try` build of the feature's tip. `{branch}` in the command:
/// run from the shared folder with the name in place (this repo:
/// `scripts/versions.sh build {branch}`, which builds in its own temp
/// worktree); else run in a worktree at the tip, kept for the try
/// (`<scratch>/try-<name>`, replaced at the next try). `run`: `[flow]
/// try_run`, `{out}` the build's last output line, `{dir}` its folder;
/// none: that last line as it is.
pub fn try_build(shared: &Path, scratch: &Path, name: &str, cmd: &str, run: Option<&str>, now: u64) -> Result<Tried, String> {
    let b = branch_ref(name);
    let tip = git(shared, &["rev-parse", "--verify", &b])?;
    let dir = if cmd.contains("{branch}") {
        shared.to_path_buf()
    } else {
        let d = scratch.join(format!("try-{}", name));
        if d.exists() {
            let _ = git(shared, &["worktree", "remove", "--force", &d.to_string_lossy()]);
            let _ = std::fs::remove_dir_all(&d);
            let _ = git(shared, &["worktree", "prune"]);
        }
        std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
        git(shared, &["worktree", "add", "-q", "--detach", &d.to_string_lossy(), &tip])?;
        d
    };
    let line = cmd.replace("{branch}", name);
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&line)
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("the try build `{}` could not start: {}", line, e))?;
    if !out.status.success() {
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        return Err(format!("the try build `{}` failed: {}", line, crate::util::clip_tail(text.trim(), 600)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string();
    let run = match run {
        Some(r) => r.replace("{out}", &last).replace("{dir}", &dir.to_string_lossy()),
        None if !last.is_empty() => last,
        None => dir.to_string_lossy().to_string(),
    };
    Ok(Tried { sha: crate::land::shorten(shared, &tip), run: home_short(&run), at_ms: now })
}

/// `~/…` for a path under the user's home, as the user types it.
pub fn home_short(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&format!("{}/", h)) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// The try item's option that shows the diff (`2 show the diff`).
pub const DIFF_OPTION: u32 = 2;

/// The try item's options' acts (a client's, never sent, the item stays):
/// its `2 show the diff` opens the guided review of feature `name`'s
/// branch (code-viewer-width, architect m_57792), and `diff`, the
/// feature's diff artifact, for a client that can't (an older one); when
/// there is a diff (none: the item as it is). Pure.
pub fn try_acts(kind: &str, name: &str, diff: Option<&str>) -> Option<(u32, bise_proto::rows::OptAct)> {
    (kind == TRY).then_some(())?;
    let review = bise_proto::diff::GuideTarget::Branch { name: name.to_string() };
    Some((DIFF_OPTION, bise_proto::rows::OptAct::Open { url: None, artifact: Some(diff?.to_string()), at: None, review: Some(review) }))
}

/// Each feature's diff artifact, by feature: of `names`, the stored
/// artifact not archived whose source is its diff file (`file`,
/// `Paths::feature_diff`). Pure but for `file`'s canonical form.
pub fn diff_ids(
    names: &[String],
    stored: &[crate::artifacts::Meta],
    archived: &std::collections::BTreeMap<String, u64>,
    file: &dyn Fn(&str) -> std::path::PathBuf,
) -> std::collections::BTreeMap<String, String> {
    names
        .iter()
        .filter_map(|n| {
            let f = file(n);
            let source = f.canonicalize().unwrap_or(f).to_string_lossy().to_string();
            let m = stored.iter().filter(|m| m.source == source && !archived.contains_key(&m.id)).max_by_key(|m| m.current().map_or(m.created_ms, |v| v.at_ms))?;
            Some((n.clone(), m.id.clone()))
        })
        .collect()
}

/// The add that keeps feature `name`'s diff (`file`, written at `at`, the
/// tip's short sha) as its one artifact, `<name>-diff`: each `sb feature
/// ready` a next version, its title the tip it shows. Pure.
pub fn diff_artifact(name: &str, at: &str, file: &Path) -> crate::artifacts::Add {
    crate::artifacts::Add {
        target: file.to_string_lossy().to_string(),
        title: Some(format!("{name} at {at}")),
        kind: Some("code".into()),
        agent: crate::model::MAIN.into(),
        by: crate::model::MAIN.into(),
        cwd: file.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default(),
        id: Some(format!("{name}-diff")),
        ..Default::default()
    }
}

/// The diff for `2 show the diff`, written to `file`: the stat, then the
/// patch, against where it leaves main.
pub fn write_diff(shared: &Path, name: &str, file: &Path) -> Result<(), String> {
    let main = crate::trunk::trunk_ref(shared)?;
    let range = format!("{}...{}", main, branch_ref(name));
    let stat = git(shared, &["diff", "--stat", &range])?;
    let patch = git(shared, &["diff", &range])?;
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(file, format!("{}\n\n{}\n", stat, patch)).map_err(|e| e.to_string())
}

// ---- the words (designer, d4172e6; dev-flow §5.1, §7) ----

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

/// `+3,120`: thousands with a comma, as the item writes them.
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `14 commits on computer-use, 3 behind main · +3,120 −410`.
fn summary(name: &str, f: &Facts, main: &str) -> String {
    let behind = if f.behind > 0 { format!(", {} behind {}", f.behind, main) } else { String::new() };
    format!(
        "{} on {}{} · +{} −{}",
        plural(f.ahead, "commit", "commits"),
        name,
        behind,
        thousands(f.adds),
        thousands(f.dels)
    )
}

/// The "ready to try" item's text: its title line, then its body (the
/// options are the TUI's, by kind).
pub fn try_text(name: &str, f: &Facts, main: &str, checked: bool) -> String {
    let check = if checked { "the check passes. " } else { "" };
    format!(
        "{} is ready to try\u{a}{}\u{a}{}none of it is on {} yet.{}",
        name,
        summary(name, f, main),
        check,
        main,
        options(&TRY_OPTIONS)
    )
}

/// The close word of a feature item whose answer failed: the hub hands
/// it to main ([`handoff`]), it never opens again (the clients' word:
/// `render::closed_word`).
pub const HANDED: &str = "handed to main";

/// A step of his that failed, for [`handoff`].
pub struct Handoff<'a> {
    /// What he picked: `merge`, `try`, `drop`.
    pub op: &'a str,
    pub name: &'a str,
    /// The trunk's short name.
    pub trunk: &'a str,
    /// The trunk's and the feature's tips when it failed (short; "" unknown).
    pub base: &'a str,
    pub tip: &'a str,
    pub picked_ms: u64,
    pub failed: &'a Failed,
    /// The feature's live agents.
    pub agents: &'a [String],
}

/// The ready brief main gets when his pick on a feature item failed
/// (brief-feature-merge): what he picked and when, what failed, the
/// steps to finish it, the agents that must not land meanwhile.
/// Model-facing text (main's), not UI words: no words catalog.
pub fn handoff(h: &Handoff) -> String {
    let (name, trunk) = (h.name, h.trunk);
    let tips = match (h.base.is_empty(), h.tip.is_empty()) {
        (false, false) => format!(" ({} at {}, {} at {})", trunk, h.base, name, h.tip),
        _ => String::new(),
    };
    let what = match h.failed {
        Failed::Conflicts { files, .. } => format!("it conflicts on {}:\n{}", plural(files.len(), "file", "files"), files.iter().map(|f| format!("- {}", f)).collect::<Vec<_>>().join("\n")),
        Failed::Check(out) => format!("the check failed. its last lines:\n{}", crate::util::clip_tail(out.trim(), 1200)),
        Failed::Moved(e) => format!("{} moved under it twice (a race with another land): {}", trunk, e),
        Failed::Other(e) => format!("it failed: {}", e),
    };
    let finish = match h.op {
        "try" => format!("`sb feature sync {}` (it takes the clean path), then `sb feature ready {}`: his inbox asks him to try it again.", name, name),
        "drop" => format!("`sb feature drop {}` (only his word allowed it: he gave it).", name),
        _ => format!("`sb feature merge {}` again: it takes the clean path now (he said merge: no need to ask him again).", name),
    };
    let fix = match h.failed {
        Failed::Conflicts { .. } | Failed::Check(_) => format!(
            "1. a worktree on {} (an agent spawned with `--feature {}`, or yours).\n2. merge {} into it (`git merge {}`); never rebase a feature that holds merges.\n3. resolve the conflicts, fix what the check says.\n4. `tests/gate.sh quick` green, then `sb land` (onto {}).\n5. {}",
            name, name, trunk, trunk, name, finish
        ),
        _ => format!("1. see why above; fix it if it needs fixing.\n2. {}", finish),
    };
    let agents = if h.agents.is_empty() {
        format!("{} has no live agent.", name)
    } else {
        let who: Vec<String> = h.agents.iter().map(|a| format!("@{}", a)).collect();
        format!("its live agents ({}) must not land on {} meanwhile: tell them.", who.join(", "), name)
    };
    format!(
        "the user picked '{}' on feature {} at {}. it did not go through, so bise hands it to you: his card is closed, nothing asks him again. do it, or give it to an agent; ask him only a NEW question if a choice really is his.\n\nwhat failed{}: {}\n\nthe steps:\n{}\n\n{}",
        h.op,
        name,
        crate::search::fmt_utc(h.picked_ms),
        tips,
        what,
        fix,
        agents
    )
}

/// The options as pr-merge's choice cards write them (tui cards.rs
/// `split_choices`): a blank line, then `1. try it`, `2. …`.
fn options(xs: &[&str]) -> String {
    let lines: Vec<String> = xs.iter().enumerate().map(|(i, o)| format!("{}. {}", i + 1, o)).collect();
    format!("\u{a}\u{a}{}", lines.join("\u{a}"))
}

/// The "merge it into main?" item, after a try.
pub fn merge_text(name: &str, f: &Facts, main: &str, tried: &Tried, checked: bool) -> String {
    let check = if checked { " · the check passes" } else { "" };
    format!(
        "merge {} into {}?\u{a}you tried {}: {}\u{a}{}{}.{}",
        name,
        main,
        tried.sha,
        tried.run,
        plural(f.ahead, "commit", "commits"),
        check,
        options(&MERGE_OPTIONS)
    )
}

/// The second ask of `3 drop the branch` (the TUI shows it on the same
/// item): `drop computer-use? 14 commits go.`
pub fn drop_question(name: &str, commits: usize) -> String {
    format!("drop {}? {} go.", name, plural(commits, "commit", "commits"))
}

/// The held lid of a feature (the box, or under a solo row):
/// `feature · 14 commits · 3 behind main · not tried`, `… · tried
/// fd25c45 18m ago`, `… · building fd25c45`.
pub fn lid(feat: &Feature, f: Option<&Facts>, main: &str, building: bool, now: u64) -> String {
    let mut parts = vec!["feature".to_string()];
    if let Some(f) = f {
        parts.push(plural(f.ahead, "commit", "commits"));
        if f.behind > 0 {
            parts.push(format!("{} behind {}", f.behind, main));
        }
    }
    parts.push(match (&feat.tried, building) {
        (_, true) => "building to try".to_string(),
        (Some(t), _) => format!("tried {} {} ago", t.sha, crate::util::age(t.at_ms, now)),
        (None, _) => "not tried".to_string(),
    });
    parts.join(" · ")
}

/// `/flow`'s line for the open features: `1 feature branch: computer-use
/// (3 agents, 14 commits, not tried)`. "" when none.
pub fn flow_line(list: &[(&Feature, usize, Option<&Facts>)]) -> String {
    if list.is_empty() {
        return String::new();
    }
    let each: Vec<String> = list
        .iter()
        .map(|(feat, agents, f)| {
            let mut p = vec![plural(*agents, "agent", "agents")];
            if let Some(f) = f {
                p.push(plural(f.ahead, "commit", "commits"));
            }
            p.push(match &feat.tried {
                Some(t) => format!("tried {}", t.sha),
                None => "not tried".into(),
            });
            format!("{} ({})", feat.name, p.join(", "))
        })
        .collect();
    format!(
        "{}: {}",
        if list.len() == 1 { "1 feature branch".to_string() } else { format!("{} feature branches", list.len()) },
        each.join(", ")
    )
}

/// Main's feed after a merge: `✓ computer-use merged into main (14
/// commits, a1b2c3d) · pushed · its 3 agents archived`.
pub fn merged_line(name: &str, main: &str, m: &Merged, agents: usize) -> String {
    let mut s = format!("✓ {} merged into {} ({}, {})", name, main, plural(m.commits, "commit", "commits"), m.sha);
    match (m.pushed, &m.push_error) {
        (Some(true), _) => s.push_str(" · pushed"),
        (Some(false), Some(e)) => s.push_str(&format!(" · not pushed ({})", e)),
        _ => {}
    }
    if agents > 0 {
        s.push_str(&format!(" · {} archived", if agents == 1 { "its agent".to_string() } else { format!("its {} agents", agents) }));
    }
    s
}

#[cfg(test)]
#[path = "feature_tests.rs"]
mod tests;
