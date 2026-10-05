//! The trunk ("main"), one definition (issue #8): the repo's default
//! branch, never whatever the shared folder has checked out. A new
//! worktree starts from its tip ([`worktree_start`]; PR flow: its
//! remote copy), `sb land` from a worktree moves it, a feature is made
//! from it, rebased on it and merged into it.
//!
//! The default branch: the name `refs/remotes/origin/HEAD` points to
//! (`develop`, `trunk`), else `main`, else `master`, as a local branch
//! (`refs/heads/<name>`). None of them: the shared folder's branch, as
//! before. Read from local refs only: no fetch, so PR flow's
//! `origin/<base>` is as fresh as the last fetch.

use crate::worktree::git;
use std::path::Path;

/// Pure: the trunk's local ref. `origin_head`: what
/// `refs/remotes/origin/HEAD` points to (`origin/main`); `has`: whether
/// a ref exists. None: no candidate exists.
pub fn pick(origin_head: Option<&str>, has: &dyn Fn(&str) -> bool) -> Option<String> {
    let named = origin_head.map(|h| h.strip_prefix("origin/").unwrap_or(h));
    named
        .into_iter()
        .chain(["main", "master"])
        .filter(|n| !n.is_empty() && *n != "HEAD")
        .map(|n| format!("refs/heads/{}", n))
        .find(|r| has(r))
}

/// Pure: where a new worktree starts. `configured`: `[worktree] base`
/// (it wins); `trunk`: [`pick`]'s; `pr`: the repo ships through pull
/// requests (its remote copy of the trunk first: the PR's base).
pub fn worktree_start(configured: Option<&str>, trunk: Option<&str>, pr: bool, has: &dyn Fn(&str) -> bool) -> String {
    if let Some(c) = configured.filter(|c| !c.trim().is_empty()) {
        return c.to_string();
    }
    let Some(t) = trunk else {
        return "HEAD".into();
    };
    let remote = format!("refs/remotes/origin/{}", t.strip_prefix("refs/heads/").unwrap_or(t));
    if pr && has(&remote) {
        remote
    } else {
        t.to_string()
    }
}

fn has_ref(shared: &Path) -> impl Fn(&str) -> bool + '_ {
    move |r: &str| git(shared, &["show-ref", "--verify", "--quiet", r]).is_ok()
}

fn origin_head(shared: &Path) -> Option<String> {
    git(shared, &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"]).ok()
}

/// The trunk of the repo at `shared` (`refs/heads/main`): [`pick`]'s,
/// else the shared folder's branch.
pub fn trunk_ref(shared: &Path) -> Result<String, String> {
    match pick(origin_head(shared).as_deref(), &has_ref(shared)) {
        Some(t) => Ok(t),
        None => crate::land::head_ref(shared),
    }
}

/// The commit-ish a new worktree of `shared` starts from.
pub fn start_ref(shared: &Path, configured: Option<&str>, pr: bool) -> String {
    let has = has_ref(shared);
    let trunk = if configured.is_some() { None } else { trunk_ref(shared).ok() };
    worktree_start(configured, trunk.as_deref(), pr, &has)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(rs: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |r: &str| rs.contains(&r)
    }

    #[test]
    fn the_remote_default_first_then_main_then_master() {
        let all = refs(&["refs/heads/develop", "refs/heads/main", "refs/heads/master"]);
        assert_eq!(pick(Some("origin/develop"), &all).as_deref(), Some("refs/heads/develop"));
        assert_eq!(pick(None, &all).as_deref(), Some("refs/heads/main"));
        assert_eq!(pick(Some("origin/gone"), &refs(&["refs/heads/master"])).as_deref(), Some("refs/heads/master"));
        assert_eq!(pick(None, &refs(&["refs/heads/sb/other"])), None);
    }

    #[test]
    fn a_worktree_starts_from_the_trunk_or_its_remote_copy_in_pr_flow() {
        let has = refs(&["refs/heads/main", "refs/remotes/origin/main"]);
        let t = Some("refs/heads/main");
        assert_eq!(worktree_start(None, t, false, &has), "refs/heads/main");
        assert_eq!(worktree_start(None, t, true, &has), "refs/remotes/origin/main");
        assert_eq!(worktree_start(None, t, true, &refs(&["refs/heads/main"])), "refs/heads/main", "no remote copy");
        assert_eq!(worktree_start(Some("origin/dev"), t, true, &has), "origin/dev", "[worktree] base wins");
        assert_eq!(worktree_start(None, None, false, &has), "HEAD");
    }
}
