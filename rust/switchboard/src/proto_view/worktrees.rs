//! The typed `worktrees` rows (bise desktop S7 emitter 3): pure, from
//! git's own outputs (`git worktree list --porcelain`, `rev-list
//! --left-right --count`) and the agents' worktree paths. The shell that
//! runs git, in a thread, is `daemon/worktrees.rs`.

use bise_proto::rows::Worktree;
use std::path::Path;

/// A checkout `git worktree list --porcelain` names.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub path: String,
    /// "" when detached
    pub branch: String,
    /// the sha it sits at
    pub head: Option<String>,
}

/// The worktrees `git worktree list --porcelain` names; a bare repo is no
/// checkout.
pub fn worktree_list(porcelain: &str) -> Vec<Listed> {
    let mut out = Vec::new();
    for block in porcelain.split("\n\n") {
        let mut path = None;
        let mut branch = String::new();
        let mut head = None;
        let mut bare = false;
        for l in block.lines() {
            if let Some(p) = l.strip_prefix("worktree ") {
                path = Some(p.to_string());
            } else if let Some(h) = l.strip_prefix("HEAD ") {
                head = Some(h.trim().to_string()).filter(|h| !h.is_empty());
            } else if let Some(b) = l.strip_prefix("branch ") {
                branch = b.strip_prefix("refs/heads/").unwrap_or(b).to_string();
            } else if l == "bare" {
                bare = true;
            }
        }
        if let Some(p) = path.filter(|_| !bare) {
            out.push(Listed { path: p, branch, head });
        }
    }
    out
}

/// `rev-list --left-right --count <trunk>...HEAD`: (behind, ahead).
pub fn left_right(out: &str) -> Option<(u32, u32)> {
    let mut it = out.split_whitespace().map(str::parse::<u32>);
    match (it.next(), it.next(), it.next()) {
        (Some(Ok(l)), Some(Ok(r)), None) => Some((l, r)),
        _ => None,
    }
}

/// The rows: each listed worktree with the agent working there (the
/// first of `agents`, (name, path), whose path is it) and its facts
/// (`facts(path)`: dirty, ahead, behind).
pub fn worktrees(list: &[Listed], agents: &[(String, String)], facts: impl Fn(&str) -> (bool, u32, u32)) -> Vec<Worktree> {
    let same = |a: &str, b: &str| {
        let canon = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| Path::new(p).to_path_buf());
        a.trim_end_matches('/') == b.trim_end_matches('/') || canon(a) == canon(b)
    };
    list.iter()
        .map(|l| {
            let (dirty, ahead, behind) = facts(&l.path);
            let agent = agents.iter().find(|(_, p)| same(p, &l.path)).map(|(n, _)| n.clone());
            Worktree { path: l.path.clone(), branch: l.branch.clone(), head: l.head.clone(), agent, dirty, ahead, behind }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gits_worktree_list_becomes_the_typed_rows() {
        let porcelain = "worktree /r/acme\nHEAD 1111\nbranch refs/heads/main\n\nworktree /w/perf/acme\nHEAD 2222\nbranch refs/heads/sb/perf\n\nworktree /w/try\nHEAD 3333\ndetached\n\nworktree /r/bare.git\nbare\n";
        let list = worktree_list(porcelain);
        let l = |p: &str, b: &str, h: &str| Listed { path: p.into(), branch: b.into(), head: Some(h.into()) };
        assert_eq!(list, [l("/r/acme", "main", "1111"), l("/w/perf/acme", "sb/perf", "2222"), l("/w/try", "", "3333")], "a bare repo is no checkout; detached: '' and its head");
        assert_eq!(left_right("3\t1\n"), Some((3, 1)));
        assert_eq!((left_right(""), left_right("x 1"), left_right("1 2 3")), (None, None, None));
        let agents = [("perf".to_string(), "/w/perf/acme/".to_string())];
        let rows = worktrees(&list, &agents, |p| if p == "/w/perf/acme" { (true, 2, 0) } else { (false, 0, 1) });
        assert_eq!(rows[1], Worktree { path: "/w/perf/acme".into(), branch: "sb/perf".into(), head: Some("2222".into()), agent: Some("perf".into()), dirty: true, ahead: 2, behind: 0 });
        assert_eq!((rows[0].agent.as_deref(), rows[0].behind), (None, 1), "his own checkout: no agent");
        assert_eq!(rows.len(), 3);
    }
}
