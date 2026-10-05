//! What `sb land` takes and what it names (issue 12), pure: the place's
//! whole `git status` against its tip, the agent's claims and the other
//! agents' files in, the files to commit and the ones left out (named in
//! the answer) out. `land::own_changes` reads git and calls [`pick`].
//!
//! - A worktree the agent has alone: every change is its own (tracked
//!   files modified, deleted or renamed, new files git does not ignore),
//!   however it was made (a file tool, `sed`, a script, cargo's
//!   `Cargo.lock`, a cherry-pick). More than [`SWEEP_MAX`] it did not
//!   claim: refused, with the first ones (build output git does not
//!   ignore).
//! - Elsewhere (the shared folder, a shared worktree): only what it
//!   claimed (its file tools' files, `--add`); every other change made
//!   since it started that is no other agent's is named, never taken.
//! - Nested repos (`x/`) are never taken: named.

/// More unclaimed changes than this in a worktree are not swept in on
/// their own (build output git does not ignore): the land says so.
pub(crate) const SWEEP_MAX: usize = 200;

/// One entry of `git status` against the tip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Change {
    /// Relative to the place's folder; a nested repo ends with `/`.
    pub path: String,
    /// A staged rename's (or copy's) old path: the two go together.
    pub from: Option<String>,
    /// Written since the agent started (a deleted file, an unknown time:
    /// yes).
    pub recent: bool,
}

impl Change {
    fn repo(&self) -> bool {
        self.path.ends_with('/')
    }

    fn paths(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.path).chain(self.from.iter())
    }
}

/// What a land takes, and what it names: changes nobody claimed, nested
/// repos (`x/`).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Picked {
    pub mine: Vec<String>,
    pub left_out: Vec<String>,
}

/// What the agent claims, relative to the place's folder.
pub(crate) struct Claims<'a> {
    /// The files it wrote with a file tool.
    pub files: &'a [String],
    /// `sb land --add`: (the argument, its relative path); a folder takes
    /// every change under it that is no other agent's.
    pub add: &'a [(String, String)],
    /// The other agents of the place and their files.
    pub others: &'a [(String, Vec<String>)],
}

/// The files to commit and the ones to name. `alone`: a worktree no
/// other agent of the place shares.
pub(crate) fn pick(changes: &[Change], claims: &Claims, alone: bool) -> Result<Picked, String> {
    let theirs = |p: &String| claims.others.iter().any(|(_, fs)| fs.contains(p));
    let mut claimed: Vec<bool> = changes.iter().map(|c| c.paths().any(|p| claims.files.contains(p))).collect();
    for (arg, p) in claims.add {
        let p = p.trim_end_matches('/');
        let mut any = false;
        for (i, c) in changes.iter().enumerate().filter(|(_, c)| !c.repo()) {
            let exact = c.paths().any(|x| x == p);
            let under = c.paths().any(|x| p == "." || x.starts_with(&format!("{}/", p))) && !c.paths().any(theirs);
            if exact || under {
                claimed[i] = true;
                any = true;
            }
        }
        if !any {
            return Err(format!("--add {}: no new or changed file there", arg));
        }
    }
    let mut mine: Vec<String> = Vec::new();
    let mut unclaimed: Vec<&Change> = Vec::new();
    let mut repos: Vec<String> = Vec::new();
    for (c, yes) in changes.iter().zip(&claimed) {
        if c.repo() {
            if alone || c.recent {
                repos.push(c.path.clone());
            }
        } else if *yes {
            mine.extend(c.paths().cloned());
        } else if !c.paths().any(theirs) {
            unclaimed.push(c);
        }
    }
    let mut left_out: Vec<String> = Vec::new();
    if alone {
        if unclaimed.len() > SWEEP_MAX {
            let first: Vec<&str> = unclaimed.iter().take(3).map(|c| c.path.as_str()).collect();
            return Err(format!(
                "{} new or changed files not ignored in the worktree (first: {}): add them to .gitignore, or land the ones you want with --add <path>",
                unclaimed.len(),
                first.join(", ")
            ));
        }
        mine.extend(unclaimed.iter().flat_map(|c| c.paths().cloned()));
    } else {
        left_out.extend(unclaimed.iter().filter(|c| c.recent).map(|c| c.path.clone()));
    }
    left_out.extend(repos);
    let mut seen = std::collections::BTreeSet::new();
    mine.retain(|f| seen.insert(f.clone()));
    for f in &mine {
        let who: Vec<&str> = claims.others.iter().filter(|(_, fs)| fs.contains(f)).map(|(n, _)| n.as_str()).collect();
        if !who.is_empty() {
            return Err(format!("{} is also changed by @{}: not landed, main decides who lands it", f, who.join(", @")));
        }
    }
    Ok(Picked { mine, left_out })
}

/// The land's word on what it left out: the changes no agent claimed
/// (which, and how to land them), the nested repos. "" when none.
pub fn left_out_note(left_out: &[String]) -> String {
    let (repos, files): (Vec<&String>, Vec<&String>) = left_out.iter().partition(|f| f.ends_with('/'));
    let mut parts: Vec<String> = Vec::new();
    if !files.is_empty() {
        let shown: Vec<&str> = files.iter().take(8).map(|s| s.as_str()).collect();
        let more = if files.len() > shown.len() { format!(" (+{} more)", files.len() - shown.len()) } else { String::new() };
        parts.push(format!(
            "left out {} file{} no agent claimed (changed or new since you started): {}{}. yours (bash, a script, cargo)? land {}: sb land --here --add <file or folder> \"<message>\"",
            files.len(),
            if files.len() == 1 { "" } else { "s" },
            shown.join(", "),
            more,
            if files.len() == 1 { "it" } else { "them" }
        ));
    }
    if !repos.is_empty() {
        let names: Vec<&str> = repos.iter().map(|s| s.as_str()).collect();
        parts.push(if names.len() == 1 {
            format!("{} is its own repo: not landed", names[0])
        } else {
            format!("{} are their own repos: not landed", names.join(", "))
        });
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(path: &str, recent: bool) -> Change {
        Change { path: path.into(), from: None, recent }
    }

    fn mv(path: &str, from: &str) -> Change {
        Change { path: path.into(), from: Some(from.into()), recent: true }
    }

    fn s(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    fn pick_of(changes: &[Change], files: &[&str], add: &[&str], others: &[(&str, &[&str])], alone: bool) -> Result<Picked, String> {
        let files = s(files);
        let add: Vec<(String, String)> = add.iter().map(|a| (a.to_string(), a.to_string())).collect();
        let others: Vec<(String, Vec<String>)> = others.iter().map(|(n, fs)| (n.to_string(), s(fs))).collect();
        pick(changes, &Claims { files: &files, add: &add, others: &others }, alone)
    }

    #[test]
    fn alone_every_change_is_mine_however_it_was_made() {
        // a: a file tool; sed.rs: sed; gone: rm; new.rs (from old.rs): git mv;
        // Cargo.lock: cargo; n.txt: a new file; x/: a nested repo
        let cs = [
            ch("a", true),
            ch("Cargo.lock", false),
            ch("gone", true),
            mv("new.rs", "old.rs"),
            ch("n.txt", true),
            ch("sed.rs", false),
            ch("x/", false),
        ];
        let p = pick_of(&cs, &["a"], &[], &[], true).unwrap();
        assert_eq!(p.mine, s(&["a", "Cargo.lock", "gone", "new.rs", "old.rs", "n.txt", "sed.rs"]));
        assert_eq!(p.left_out, s(&["x/"]), "a nested repo is named, never taken");
    }

    #[test]
    fn alone_past_sweep_max_is_refused_with_the_first_ones() {
        let mut cs: Vec<Change> = (0..=SWEEP_MAX).map(|i| ch(&format!("out/{:03}", i), true)).collect();
        cs.push(ch("a", true));
        let e = pick_of(&cs, &["a"], &[], &[], true).unwrap_err();
        assert!(e.starts_with(&format!("{} new or changed files not ignored", SWEEP_MAX + 1)), "{}", e);
        assert!(e.contains("out/000, out/001, out/002") && e.contains(".gitignore") && e.contains("--add"), "{}", e);
        // the claimed ones do not count: exactly SWEEP_MAX unclaimed land
        cs.remove(0);
        assert_eq!(pick_of(&cs, &["a"], &[], &[], true).unwrap().mine.len(), SWEEP_MAX + 1);
    }

    #[test]
    fn shared_takes_the_claims_and_names_the_other_recent_changes() {
        let cs = [
            ch("a", true),          // mine (file tool)
            ch("b", true),          // y's
            ch("c", true),          // the user's, since I started: named
            ch("old.txt", false),   // the user's, from before: not news
            ch("gone", true),       // deleted, nobody's: named
            ch("gen/v.svg", true),  // bash made it: named
            ch("x/", true),         // a nested repo, recent: named
            ch("y/", false),        // an old one: not news
        ];
        let p = pick_of(&cs, &["a", "unchanged"], &[], &[("y", &["b"])], false).unwrap();
        assert_eq!(p.mine, s(&["a"]));
        assert_eq!(p.left_out, s(&["c", "gone", "gen/v.svg", "x/"]));
    }

    #[test]
    fn add_takes_a_tracked_file_or_every_change_of_a_folder_but_another_agents() {
        let cs = [ch("c", true), ch("gen/v.svg", true), ch("gen/old.rs", false), ch("gen/y.txt", true)];
        let others: &[(&str, &[&str])] = &[("y", &["gen/y.txt"])];
        let p = pick_of(&cs, &[], &["c"], others, false).unwrap();
        assert_eq!((p.mine, p.left_out), (s(&["c"]), s(&["gen/v.svg"])));
        let p = pick_of(&cs, &[], &["gen/"], others, false).unwrap();
        assert_eq!((p.mine, p.left_out), (s(&["gen/v.svg", "gen/old.rs"]), s(&["c"])));
        // another agent's file named on its own: refused, main decides
        let e = pick_of(&cs, &[], &["gen/y.txt"], others, false).unwrap_err();
        assert_eq!(e, "gen/y.txt is also changed by @y: not landed, main decides who lands it");
        let e = pick_of(&cs, &[], &["nope"], others, false).unwrap_err();
        assert_eq!(e, "--add nope: no new or changed file there");
        // `.`: everything but the other agents'
        let p = pick_of(&cs, &[], &["."], others, false).unwrap();
        assert_eq!(p.mine, s(&["c", "gen/v.svg", "gen/old.rs"]));
    }

    #[test]
    fn a_rename_goes_whole_when_staged_and_a_plain_mv_names_the_old_path() {
        // git mv: claiming either side takes both
        let p = pick_of(&[mv("b.rs", "a.rs")], &["a.rs"], &[], &[], false).unwrap();
        assert_eq!(p.mine, s(&["b.rs", "a.rs"]));
        // mv then a file tool on the new path: the deletion is named
        let p = pick_of(&[ch("a.rs", true), ch("b.rs", true)], &["b.rs"], &[], &[], false).unwrap();
        assert_eq!((p.mine, p.left_out), (s(&["b.rs"]), s(&["a.rs"])));
    }

    #[test]
    fn a_change_another_agent_also_made_is_refused_even_alone_claimed() {
        let e = pick_of(&[ch("a", true)], &["a"], &[], &[("y", &["a"]), ("z", &["a"])], false).unwrap_err();
        assert!(e.starts_with("a is also changed by @y, @z"), "{}", e);
    }

    #[test]
    fn the_note_names_the_files_and_the_repos() {
        assert_eq!(left_out_note(&[]), "");
        let n = left_out_note(&s(&["c"]));
        assert!(n.starts_with("left out 1 file no agent claimed (changed or new since you started): c. "), "{}", n);
        assert!(n.contains("land it: sb land --here --add <file or folder>"), "{}", n);
        let many: Vec<String> = (0..10).map(|i| format!("f{}", i)).collect();
        assert!(left_out_note(&many).contains("f7 (+2 more). "), "{}", left_out_note(&many));
        assert_eq!(left_out_note(&s(&["x/"])), "x/ is its own repo: not landed");
        let n = left_out_note(&s(&["c", "x/", "y/"]));
        assert!(n.contains("land it:") && n.ends_with("; x/, y/ are their own repos: not landed"), "{}", n);
    }
}
