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

/// A scratch worktree, detached at `rev`, removed when dropped.
pub struct Scratch {
    shared: PathBuf,
    pub dir: PathBuf,
}

impl Scratch {
    pub fn new(shared: &Path, root: &Path, tag: &str, rev: &str) -> Result<Scratch, String> {
        let dir = root.join(format!(".{}-{}", tag, crate::util::now_ms()));
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        git(shared, &["worktree", "add", "-q", "--detach", &dir.to_string_lossy(), rev])?;
        Ok(Scratch { shared: shared.to_path_buf(), dir })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = git(&self.shared, &["worktree", "remove", "--force", &self.dir.to_string_lossy()]);
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = git(&self.shared, &["worktree", "prune"]);
    }
}

/// The feature rebased on main in a scratch worktree, the check run
/// there (`check`), then the branch moved (compare-and-swap). None:
/// already on main's tip. Some(old tip, new tip).
pub fn rebase(shared: &Path, scratch: &Path, name: &str, check: Option<&str>) -> Result<Option<(String, String)>, String> {
    let main = crate::trunk::trunk_ref(shared)?;
    let b = branch_ref(name);
    let old = git(shared, &["rev-parse", "--verify", &b])?;
    let base = git(shared, &["rev-parse", "--verify", &main])?;
    if git(shared, &["merge-base", "--is-ancestor", &base, &old]).is_ok() {
        return Ok(None);
    }
    let s = Scratch::new(shared, scratch, &format!("sync-{}", name), &old)?;
    if let Err(e) = git(&s.dir, &["rebase", "-q", &base]) {
        let conflicts = git(&s.dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        let _ = git(&s.dir, &["rebase", "--abort"]);
        let files: Vec<&str> = conflicts.lines().collect();
        return Err(if files.is_empty() {
            let other = format!("the rebase of {} on {} failed: {}", name, short(&main), e);
            crate::land::rebase_failed(&s.dir, &e, "sync again", other)
        } else {
            format!("{} changed on {} too: the rebase of {} conflicts", files.join(", "), short(&main), name)
        });
    }
    if let Some(c) = check {
        crate::land::run_check(&s.dir, c)?;
    }
    let new = git(&s.dir, &["rev-parse", "HEAD"])?;
    git(shared, &["update-ref", &b, &new, &old]).map_err(|_| format!("{} moved meanwhile: sync it again", name))?;
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

/// What a merge did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub commits: usize,
    pub sha: String,
    pub pushed: Option<bool>,
    pub push_error: Option<String>,
}

/// `1 merge` (the user's go): rebased on main and checked in a scratch
/// worktree (the check always runs: the full one), main fast-forwarded,
/// pushed when the flow says so. The branch itself is deleted after, by
/// [`trash`], once the agents are archived.
pub fn merge(shared: &Path, scratch: &Path, name: &str, flow: &crate::flow::FlowConfig) -> Result<Merged, String> {
    let main = crate::trunk::trunk_ref(shared)?;
    let b = branch_ref(name);
    let base = git(shared, &["rev-parse", "--verify", &main])?;
    let old = git(shared, &["rev-parse", "--verify", &b])?;
    if git(shared, &["merge-base", "--is-ancestor", &old, &base]).is_ok() {
        return Err(format!("nothing to merge: {} has no commit that {} lacks", name, short(&main)));
    }
    let tip = match rebase(shared, scratch, name, flow.check.as_deref())? {
        Some((_, new)) => new,
        None => {
            // on main's tip already: the check still runs on what lands
            check(shared, scratch, name, flow.check.as_deref())?
        }
    };
    let commits = git(shared, &["rev-list", "--count", &format!("{}..{}", base, tip)])?.parse::<usize>().unwrap_or(0);
    crate::land::move_main(shared, &main, &base, &tip)?;
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
