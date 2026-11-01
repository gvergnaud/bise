//! The revert guard (land-revert-guard, architect m_42458): a land never
//! silently undoes what another agent landed since it started. The
//! native-mac incident: nm-pages-a built its tree on old copies, then
//! `git reset --soft native-mac` put it on nm-panel's fresh land
//! (cc8245966); its commit 04aaaa8d3 was a fast-forward that put back
//! the old copies of 42 files. No conflict, no rebase, no dirty file:
//! nothing looked.
//!
//! The check, on `base..tip` (the target's tip and what it would move
//! to, after the rebase, before the land check): every recent commit C
//! on the target (first parent, committed since the lander started, at
//! most [`RECENT`]) that is not the lander's own (its [`LandRecord`]s)
//! and that its commits do not name (`git revert`'s "This reverts commit
//! <sha>": an undo on purpose), whose files the land also changes. The
//! land undoes C on F when tip's F is C^1's F (the fast path: absent
//! from `git diff C^1 tip`), or else when one of C's hunks on F has its
//! own new lines gone from tip and its own removed lines back
//! ([`hunk_undone`]; a hunk the lander rewrote has neither; `git apply
//! --check` was too strict: a bless that wrote its own line where C's
//! was matched neither way). Any: refused in one line
//! ([`refusal`]), nothing moved. The guard judges the land's own diff,
//! base -> tip: a C already undone at base (by anyone, a land from
//! before the guard: native-lead m_42665) is not this land's doing.
//!
//! The pure part (the log's parse, the exemptions, the hunks, the words)
//! is unit-tested here; [`guard`] is the git shell beside it.

use crate::land::{git, short};
use crate::land_record::{landed_by, LandRecord};
use std::collections::BTreeSet;
use std::path::Path;

/// The most recent commits of the target the guard looks at.
pub const RECENT: usize = 300;

/// git's empty tree: the "parent" of a root commit.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// A recent commit of the target and the files it changed (vs its first
/// parent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recent {
    pub sha: String,
    pub parent: Option<String>,
    /// its committer time, in ms (a landed commit's: about its land's, a
    /// land rebases)
    pub committed_ms: u64,
    pub subject: String,
    pub files: Vec<String>,
}

/// `git log --first-parent --diff-merges=first-parent --name-only -z`
/// with [`LOG_FORMAT`]: records start with \x01, the header ends with a
/// NUL, then the files, each NUL-ended (the first after a newline).
pub const LOG_FORMAT: &str = "--format=%x01%H%x1f%P%x1f%ct%x1f%s";

pub fn parse_log(out: &str) -> Vec<Recent> {
    out.split('\u{1}')
        .filter(|r| !r.trim().is_empty())
        .filter_map(|r| {
            let mut parts = r.split('\0');
            let mut head = parts.next()?.split('\u{1f}');
            let sha = head.next()?.trim().to_string();
            let parent = head.next()?.split_whitespace().next().map(str::to_string);
            let committed_ms = head.next()?.trim().parse::<u64>().unwrap_or(0) * 1000;
            let subject = head.next().unwrap_or_default().to_string();
            let files = parts.map(|f| f.trim_start_matches('\n')).filter(|f| !f.is_empty()).map(str::to_string).collect();
            (!sha.is_empty()).then_some(Recent { sha, parent, committed_ms, subject, files })
        })
        .collect()
}

/// The paths a land changes, from `git diff -M --name-status -z`: a
/// rename's old path is left out (a file moved is not a file undone; a
/// stale tree drops a file, it never pairs it with a new one).
pub fn changed_paths(out: &str) -> BTreeSet<String> {
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    let mut paths = BTreeSet::new();
    while let Some(status) = it.next() {
        let Some(path) = it.next() else { break };
        if status.starts_with('R') || status.starts_with('C') {
            if let Some(new) = it.next() {
                paths.insert(new.to_string());
            }
            if status.starts_with('C') {
                paths.insert(path.to_string());
            }
        } else {
            paths.insert(path.to_string());
        }
    }
    paths
}

/// The land's messages name `sha`: a hex word of 7+ characters that
/// starts it (`git revert`'s full sha, a short one written by hand).
pub fn named(messages: &str, sha: &str) -> bool {
    messages
        .split(|c: char| !c.is_ascii_hexdigit())
        .any(|w| w.len() >= 7 && sha.starts_with(&w.to_ascii_lowercase()))
}

/// A file's `git diff -U0` patch as its hunks: (C's removed lines, C's
/// added lines).
pub fn hunks(patch: &str) -> Vec<(Vec<String>, Vec<String>)> {
    let mut out: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    for line in patch.lines() {
        if line.starts_with("@@") {
            out.push((Vec::new(), Vec::new()));
        } else if let Some((minus, plus)) = out.last_mut() {
            if let Some(l) = line.strip_prefix('-') {
                minus.push(l.to_string());
            } else if let Some(l) = line.strip_prefix('+') {
                plus.push(l.to_string());
            }
        }
    }
    out
}

/// A hunk of C is undone at tip when C's own new lines are gone from it
/// and C's own removed lines are back. "Own": a line C added that was
/// nowhere in the file before it (`before`), a line it removed that is
/// nowhere in it after (`after`): a `}` or a blank line proves nothing.
/// A hunk with no own line is never undone; a hunk the lander rewrote
/// (C's lines gone, the old ones too) is not.
pub fn hunk_undone(minus: &[String], plus: &[String], before: &BTreeSet<&str>, after: &BTreeSet<&str>, tip: &BTreeSet<&str>) -> bool {
    let added: Vec<&str> = plus.iter().map(String::as_str).filter(|l| !l.trim().is_empty() && !before.contains(l)).collect();
    let removed: Vec<&str> = minus.iter().map(String::as_str).filter(|l| !l.trim().is_empty() && !after.contains(l)).collect();
    if added.is_empty() && removed.is_empty() {
        return false;
    }
    added.iter().all(|l| !tip.contains(l)) && removed.iter().all(|l| tip.contains(l))
}

/// What a land undoes of one commit: who landed it (None: no agent's land
/// record: the user, a feature merge), whether it is older than every
/// record ([`before_records`]), and the files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Undone {
    pub sha: String,
    pub subject: String,
    pub by: Option<String>,
    pub before_records: bool,
    pub files: Vec<String>,
}

/// A commit with no land record (`by` None), committed before the first
/// record (or with no record at all): it may be anyone's land, the
/// lander's own too, from before bise kept them (architect m_42765).
pub fn before_records(by: Option<&str>, committed_ms: u64, records: &[LandRecord]) -> bool {
    by.is_none() && records.iter().map(|r| r.at_ms).min().is_none_or(|first| committed_ms < first)
}

/// The files named in a refusal before "and N more".
const NAMED: usize = 5;

/// The one-line refusal for a land onto `target` that undoes `undone`
/// (None: nothing undone).
pub fn refusal(target: &str, undone: &[Undone]) -> Option<String> {
    let first = undone.first()?;
    let sha7: String = first.sha.chars().take(7).collect();
    let mut files: Vec<&str> = Vec::new();
    for u in undone {
        for f in &u.files {
            if !files.contains(&f.as_str()) {
                files.push(f);
            }
        }
    }
    let more = files.len().saturating_sub(NAMED);
    let mut list = files.iter().take(NAMED).copied().collect::<Vec<_>>().join(", ");
    if more > 0 {
        list = format!("{} and {} more", list, more);
    }
    let who = |u: &Undone| match &u.by {
        Some(a) => format!("@{}'s", a),
        None => "a".to_string(),
    };
    let mut whose = format!("{} {} \"{}\"", who(first), sha7, first.subject);
    if undone.len() > 1 {
        whose = format!("{} and {} other commits", whose, undone.len() - 1);
    }
    let mut line = format!(
        "your land puts back old copies of {}, undoing {} on {}: your commits carry an old tree (a reset or a squash on a stale copy?). nothing moved: take their version (git checkout {} -- <file>), redo your change on top, commit, sb land again; to undo theirs on purpose: git revert {}",
        list, whose, target, target, sha7
    );
    let old: Vec<String> = undone.iter().filter(|u| u.before_records).map(|u| u.sha.chars().take(7).collect()).collect();
    if !old.is_empty() {
        line.push_str(&format!(
            ". {} landed before bise kept land records: if you mean to undo it (your own earlier land, or a planned removal), name it in your commit message ('undoes {}') and land again",
            old.join(", "),
            old[0]
        ));
    }
    Some(line)
}

/// The guard on `base..tip` in the checkout `dir`, a land of `lander`
/// onto `target` (a ref): Err its [`refusal`] when the land undoes a
/// commit landed by someone else since `since_ms`.
pub fn guard(dir: &Path, target: &str, base: &str, tip: &str, since_ms: u64, lander: &str, records: &[LandRecord]) -> Result<(), String> {
    let changed = changed_paths(&git(dir, &["diff", "-M", "--name-status", "-z", base, tip])?);
    if changed.is_empty() {
        return Ok(());
    }
    let messages = git(dir, &["log", "--format=%B", &format!("{}..{}", base, tip)])?;
    // "@<secs> +0000": git's strict epoch form. A bare "@<secs>" is an
    // epoch only past 8 digits; "@1" read as day 1 of this month at this
    // hour, so on the 1st the guard saw no commit at all (2026-11-01)
    let since = format!("--since=@{} +0000", since_ms / 1000);
    let n = format!("-n{}", RECENT);
    let log = git(
        dir,
        &["log", "--first-parent", "--diff-merges=first-parent", "--no-renames", "--name-only", "-z", &since, &n, LOG_FORMAT, base],
    )?;
    let mut undone = Vec::new();
    for c in parse_log(&log) {
        let by = landed_by(records, &c.sha).map(str::to_string);
        if by.as_deref() == Some(lander) || named(&messages, &c.sha) {
            continue;
        }
        let both: Vec<&str> = c.files.iter().filter(|f| changed.contains(*f)).map(String::as_str).collect();
        if both.is_empty() {
            continue;
        }
        let files = undone_files(dir, &c, base, tip, &both)?;
        if !files.is_empty() {
            let before_records = before_records(by.as_deref(), c.committed_ms, records);
            undone.push(Undone { sha: c.sha, subject: c.subject, by, before_records, files });
        }
    }
    match refusal(short(target), &undone) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn set_of(v: &[String]) -> BTreeSet<&str> {
    v.iter().map(String::as_str).collect()
}

/// Of `both` (C's files the land changes too), the ones whose C change
/// the land undoes, base -> tip: tip's equal to C^1's and base's not
/// (fast path), else a hunk of C's back at tip and not at base. A file
/// already C^1's at base is not this land's doing.
fn undone_files(dir: &Path, c: &Recent, base: &str, tip: &str, both: &[&str]) -> Result<Vec<String>, String> {
    let parent = c.parent.as_deref().unwrap_or(EMPTY_TREE);
    let differ = |rev: &str| -> Result<BTreeSet<String>, String> {
        let mut args = vec!["--literal-pathspecs", "diff", "--no-renames", "--name-only", "-z", parent, rev, "--"];
        args.extend(both);
        Ok(git(dir, &args)?.split('\0').filter(|f| !f.is_empty()).map(str::to_string).collect())
    };
    let (at_tip, at_base) = (differ(tip)?, differ(base)?);
    // already C^1's at base: undone before this land, whoever did it
    let live: Vec<&str> = both.iter().copied().filter(|f| at_base.contains(*f)).collect();
    let mut out: Vec<String> = live.iter().filter(|f| !at_tip.contains(**f)).map(|f| f.to_string()).collect();
    let rest: Vec<&str> = live.into_iter().filter(|f| at_tip.contains(*f)).collect();
    if rest.is_empty() {
        return Ok(out);
    }
    // the mixed case: tip's file is neither C^1's nor C's: C's hunks
    // against tip's lines, and base's
    let show = |rev: &str, f: &str| git(dir, &["show", &format!("{}:{}", rev, f)]).unwrap_or_default();
    for f in rest {
        // a file C made that tip still has: changed since, not undone
        // (deleting it is, the fast path's)
        if c.parent.is_none() || git(dir, &["cat-file", "-e", &format!("{}:{}", parent, f)]).is_err() {
            continue;
        }
        let patch = git(dir, &["--literal-pathspecs", "diff", "--no-renames", "--no-color", "--no-ext-diff", "-U0", parent, &c.sha, "--", f])?;
        let (b, a, t, s) = (show(parent, f), show(&c.sha, f), show(tip, f), show(base, f));
        let lines = |s: &str| s.lines().map(|l| l.to_string()).collect::<Vec<_>>();
        let (b, a, t, s) = (lines(&b), lines(&a), lines(&t), lines(&s));
        let (before, after, at, was) = (set_of(&b), set_of(&a), set_of(&t), set_of(&s));
        if hunks(&patch).iter().any(|(minus, plus)| hunk_undone(minus, plus, &before, &after, &at) && !hunk_undone(minus, plus, &before, &after, &was)) {
            out.push(f.to_string());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_reads_each_commit_its_first_parent_and_files() {
        let out = "\u{1}c2\u{1f}p2 m2\u{1f}1700\u{1f}y: b and p\0\nb\0p/new\0\u{1}c1\u{1f}\u{1f}1600\u{1f}init\0\na\0\u{1}c0\u{1f}p0\u{1f}1500\u{1f}empty\0";
        let r = |sha: &str, parent: Option<&str>, s: u64, subject: &str, files: &[&str]| Recent {
            sha: sha.into(),
            parent: parent.map(str::to_string),
            committed_ms: s * 1000,
            subject: subject.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
        };
        assert_eq!(
            parse_log(out),
            vec![
                r("c2", Some("p2"), 1700, "y: b and p", &["b", "p/new"]),
                r("c1", None, 1600, "init", &["a"]),
                r("c0", Some("p0"), 1500, "empty", &[]),
            ]
        );
    }

    #[test]
    fn a_renames_old_path_is_not_a_change_the_guard_looks_at() {
        let out = "M\0a\0R100\0r.rs\0s.rs\0D\0p/new\0A\0n\0";
        let got: Vec<String> = changed_paths(out).into_iter().collect();
        assert_eq!(got, ["a", "n", "p/new", "s.rs"]);
    }

    #[test]
    fn a_message_that_names_the_commit_exempts_it() {
        let sha = "cc82459661f0aa";
        assert!(named("Revert \"x\"\n\nThis reverts commit cc82459661f0aa.", sha));
        assert!(named("undo CC82459 on purpose", sha), "a short sha, any case");
        assert!(!named("undo cc8245 (too short)", sha));
        assert!(!named("feat: add cafe", sha));
    }

    #[test]
    fn a_patch_reads_as_its_hunks_removed_and_added_lines() {
        let p = "diff --git a/f b/f\nindex 1..2 100644\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+A\n@@ -9,0 +10 @@\n+z\n";
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(hunks(p), vec![(v(&["a"]), v(&["A"])), (v(&[]), v(&["z"]))]);
        assert!(hunks("Binary files a/x and b/x differ\n").is_empty(), "no hunk: never undone by lines");
    }

    /// architect m_42458's mixed case and native-lead's (m_42531): C's
    /// old lines back, its new ones gone, whatever the lander wrote
    /// around them, is undone; C's lines kept, or rewritten by the
    /// lander, is not.
    #[test]
    fn a_hunk_is_undone_only_when_its_own_lines_are_gone_and_the_old_ones_back() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let set = |xs: &[&'static str]| xs.iter().copied().collect::<BTreeSet<&str>>();
        let (before, after) = (set(&["1", "3", "}", "25"]), set(&["1", "three", "}", "25"]));
        // y: 3 -> three
        let (m, p) = (v(&["3"]), v(&["three"]));
        assert!(hunk_undone(&m, &p, &before, &after, &set(&["1", "3", "}", "twenty-five"])), "old line back, x's own edit elsewhere");
        assert!(!hunk_undone(&m, &p, &before, &after, &set(&["1", "three", "}", "twenty-five"])), "y's line kept");
        assert!(!hunk_undone(&m, &p, &before, &after, &set(&["1", "THREE", "}"])), "x rewrote it");
        // y appends `menus`; x's bless wrote `side` there instead
        let (m, p) = (v(&[]), v(&["menus", "}"]));
        assert!(hunk_undone(&m, &p, &before, &after, &set(&["1", "3", "}", "side"])), "y's line gone");
        assert!(!hunk_undone(&m, &p, &before, &after, &set(&["1", "menus", "}", "side"])), "both kept");
        // a hunk of common lines only proves nothing
        assert!(!hunk_undone(&v(&[]), &v(&["}", ""]), &before, &after, &set(&["1"])));
    }

    #[test]
    fn the_refusal_names_the_files_the_agent_and_what_to_do() {
        let u = |sha: &str, by: Option<&str>, files: &[&str]| Undone {
            sha: sha.into(),
            subject: "y: b and p".into(),
            by: by.map(str::to_string),
            before_records: false,
            files: files.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(refusal("feat", &[]), None);
        assert_eq!(
            refusal("feat", &[u("1a2b3c4d5e", Some("y"), &["b", "p/new"])]).unwrap(),
            "your land puts back old copies of b, p/new, undoing @y's 1a2b3c4 \"y: b and p\" on feat: your commits carry an old tree (a reset or a squash on a stale copy?). nothing moved: take their version (git checkout feat -- <file>), redo your change on top, commit, sb land again; to undo theirs on purpose: git revert 1a2b3c4"
        );
        let e = refusal("main", &[u("1a2b3c4d5e", None, &["a", "b", "c", "d"]), u("9f9f9f9f", Some("z"), &["d", "e", "f"])]).unwrap();
        assert!(e.starts_with("your land puts back old copies of a, b, c, d, e and 1 more, undoing a 1a2b3c4 \"y: b and p\" and 1 other commits on main"), "{}", e);
        assert!(!e.contains("land records"), "{}", e);
    }

    /// architect m_42765: a commit with no record from before the first
    /// one may be the lander's own (nm-panel's 723e639d5): the refusal
    /// says how to name it. Nothing else changes.
    #[test]
    fn a_commit_from_before_the_records_is_named_with_the_way_through() {
        let rec = |at_ms: u64| LandRecord {
            agent: "y".into(),
            place: "wt:y".into(),
            target: "feat".into(),
            base: "b".into(),
            tip: "t".into(),
            commits: vec!["t".into()],
            at_ms,
        };
        let records = [rec(5_000), rec(9_000)];
        assert!(before_records(None, 4_000, &records), "no record, older than the first");
        assert!(!before_records(None, 6_000, &records), "no record, after the first: the user, a merge");
        assert!(!before_records(Some("y"), 4_000, &records), "a record says whose");
        assert!(before_records(None, 6_000, &[]), "no record at all yet");
        let u = |sha: &str, old: bool| Undone { sha: sha.into(), subject: "s".into(), by: None, before_records: old, files: vec!["f".into()] };
        let e = refusal("feat", &[u("723e639d5a", true), u("9f9f9f9f", false)]).unwrap();
        assert!(
            e.ends_with("git revert 723e639. 723e639 landed before bise kept land records: if you mean to undo it (your own earlier land, or a planned removal), name it in your commit message ('undoes 723e639') and land again"),
            "{}",
            e
        );
    }
}
