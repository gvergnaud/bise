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
//! - The claims that are over ([`clean_claims`], land-claims): a claim on
//!   a file with no change left ends (sb-core drops it), so a file two
//!   agents once changed can be landed again.

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
/// repos (`x/`), and the other agents' files main took on its word.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Picked {
    pub mine: Vec<String>,
    pub left_out: Vec<String>,
    /// land-decide: the files of `mine` other agents also changed, that
    /// main named exactly with `--add`, and those agents.
    pub decided: Vec<(String, Vec<String>)>,
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
    /// The lander is main (the land job's fact, never the message's):
    /// a file it names exactly with `--add` lands even when other agents
    /// changed it too. Its file tools' claims and a folder `--add` decide
    /// nothing: a contested file there is still refused.
    pub decides: bool,
}

/// The files to commit and the ones to name. `alone`: a worktree no
/// other agent of the place shares.
pub(crate) fn pick(changes: &[Change], claims: &Claims, alone: bool) -> Result<Picked, String> {
    let theirs = |p: &String| claims.others.iter().any(|(_, fs)| fs.contains(p));
    let who = |f: &String| -> Vec<String> { claims.others.iter().filter(|(_, fs)| fs.contains(f)).map(|(n, _)| n.clone()).collect() };
    let refusal = |f: &String| {
        let hint = if claims.decides { "; to land it on your word, name that file with --add <file>" } else { "" };
        format!("{} is also changed by @{}: not landed, main decides who lands it{}", f, who(f).join(", @"), hint)
    };
    let mut claimed: Vec<bool> = changes.iter().map(|c| c.paths().any(|p| claims.files.contains(p))).collect();
    // main's word: the files it named exactly (both sides of a rename)
    let mut named: Vec<&String> = Vec::new();
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
            if exact && claims.decides {
                named.extend(c.paths());
            }
        }
        if !any {
            // only other agents' changes there: refused, one of them named
            let contested = changes.iter().find(|c| !c.repo() && c.paths().any(|x| x.starts_with(&format!("{}/", p))));
            return Err(match contested {
                Some(c) => refusal(&c.path),
                None => format!("--add {}: no new or changed file there", arg),
            });
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
    let mut decided: Vec<(String, Vec<String>)> = Vec::new();
    for f in &mine {
        let who = who(f);
        if who.is_empty() {
            continue;
        }
        if !named.contains(&f) {
            return Err(refusal(f));
        }
        decided.push((f.clone(), who));
    }
    Ok(Picked { mine, left_out, decided })
}

/// land-decide: main's answer for the other agents' files it took, each
/// with its agents, so it sees it took what it meant to. "" when none.
pub fn decided_note(decided: &[(String, Vec<String>)]) -> String {
    if decided.is_empty() {
        return String::new();
    }
    let each: Vec<String> = decided.iter().map(|(f, who)| format!("{} (@{})", f, who.join(", @"))).collect();
    format!("on your word, took {} file{} other agents also changed: {}", each.len(), if each.len() == 1 { "" } else { "s" }, each.join(", "))
}

/// land-decide: what each agent whose files main took hears, one entry
/// in its thread (not a message to answer): (the agent, the words).
pub fn decided_tells(decided: &[(String, Vec<String>)], target: &str, sha: &str) -> Vec<(String, String)> {
    let mut agents: Vec<&String> = decided.iter().flat_map(|(_, who)| who).collect();
    agents.sort();
    agents.dedup();
    agents
        .into_iter()
        .map(|a| {
            let files: Vec<&str> = decided.iter().filter(|(_, who)| who.contains(a)).map(|(f, _)| f.as_str()).collect();
            let text = format!(
                "main landed {} on {} in {} (it held your changes too): they are on {} now, don't redo or revert them",
                files.join(", "),
                target,
                sha,
                target
            );
            (a.clone(), text)
        })
        .collect()
}

/// The claims that are over (land-claims): of `claims` (the strings as
/// the hub keeps them, absolute or relative, with `rel` their path
/// relative to the place's folder), the ones whose file has no change in
/// `changes` (the place's `git status` against its tip). A clean file has
/// nothing left to protect, however it got clean (landed, a hand commit,
/// a checkout); a claim outside the folder is never over here.
pub(crate) fn clean_claims(changes: &[Change], claims: &[String], rel: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let dirty: std::collections::BTreeSet<&String> = changes.iter().flat_map(|c| c.paths()).collect();
    let mut out: Vec<String> = Vec::new();
    for c in claims {
        let over = rel(c).is_some_and(|p| !dirty.contains(&p));
        if over && !out.contains(c) {
            out.push(c.clone());
        }
    }
    out
}

/// The 1-based line numbers of git conflict blocks in `text`: a line
/// starting with `<<<<<<< `, then a line exactly `=======`, then a line
/// starting with `>>>>>>> `, in that order, outside a ``` fence. A lone
/// marker (a setext heading, a doc that quotes one line) or a block shown
/// in a markdown code fence is not a conflict. Each hit: the `<<<<<<<`
/// line and its `>>>>>>>` line.
pub(crate) fn conflict_blocks(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut fence, mut open, mut mid) = (false, None::<usize>, false);
    for (i, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim_start().starts_with("```") {
            fence = !fence;
            open = None;
            continue;
        }
        if fence {
            continue;
        }
        if line.starts_with("<<<<<<< ") {
            (open, mid) = (Some(i + 1), false);
        } else if line == "=======" && open.is_some() {
            mid = true;
        } else if line.starts_with(">>>>>>> ") {
            if let (Some(o), true) = (open, mid) {
                out.push((o, i + 1));
            }
            (open, mid) = (None, false);
        }
    }
    out
}

/// The land's refusal for files with conflict blocks ([`conflict_blocks`]):
/// `files` are (path, its text); None when no file has one.
pub(crate) fn conflict_refusal(files: &[(String, String)]) -> Option<String> {
    let hits: Vec<String> = files
        .iter()
        .flat_map(|(p, t)| conflict_blocks(t).into_iter().map(move |(a, b)| format!("{}:{}-{}", p, a, b)))
        .collect();
    if hits.is_empty() {
        return None;
    }
    Some(format!(
        "not landed: git conflict markers (<<<<<<< … ======= … >>>>>>>) in {}. resolve them, then land again",
        hits.join(", ")
    ))
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

    #[test]
    fn conflict_blocks_only_full_blocks_outside_fences() {
        let text = |ls: &[&str], end: &str| -> String { ls.iter().map(|l| format!("{l}{end}")).collect() };
        let lf = char::from(10).to_string();
        let crlf = format!("{}{}", char::from(13), lf);
        let block = ["a", "<<<<<<< Updated upstream", "x", "=======", "y", ">>>>>>> Stashed changes", "b"];
        let real = text(&block, &lf);
        assert_eq!(conflict_blocks(&real), vec![(2, 6)]);
        assert_eq!(conflict_blocks(&text(&block, &crlf)), vec![(2, 6)]);
        // lone markers: a setext heading, one quoted line, an unordered block
        assert!(conflict_blocks(&text(&["Title", "======="], &lf)).is_empty());
        assert!(conflict_blocks(&text(&["<<<<<<< HEAD", "only"], &lf)).is_empty());
        assert!(conflict_blocks(&text(&[">>>>>>> x", "=======", "<<<<<<< y"], &lf)).is_empty());
        assert!(conflict_blocks(&text(&["<<<<<<< a", ">>>>>>> b"], &lf)).is_empty());
        // a block shown in a markdown fence
        assert!(conflict_blocks(&text(&["```", "<<<<<<< a", "=======", ">>>>>>> b", "```"], &lf)).is_empty());
        let r = conflict_refusal(&[("ok.md".into(), "fine".into()), ("src/a.rs".into(), real.clone())]).unwrap();
        assert!(r.contains("src/a.rs:2-6") && !r.contains("ok.md"), "{r}");
        assert_eq!(conflict_refusal(&[("ok.md".into(), "fine".into())]), None);
    }

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
        pick_by(changes, files, add, others, alone, false)
    }

    /// [`pick_of`], `decides`: the lander is main.
    fn pick_by(changes: &[Change], files: &[&str], add: &[&str], others: &[(&str, &[&str])], alone: bool, decides: bool) -> Result<Picked, String> {
        let files = s(files);
        let add: Vec<(String, String)> = add.iter().map(|a| (a.to_string(), a.to_string())).collect();
        let others: Vec<(String, Vec<String>)> = others.iter().map(|(n, fs)| (n.to_string(), s(fs))).collect();
        pick(changes, &Claims { files: &files, add: &add, others: &others, decides }, alone)
    }

    /// land-decide: main names a file two agents changed with --add: it
    /// lands, named with its agents; its file tools' claim on it or a
    /// folder --add over it still refuse it, and another agent's exact
    /// --add too (the case that deadlocked msg-dots + artifacts-keep).
    #[test]
    fn main_takes_a_contested_file_only_when_it_names_it() {
        let cs = [ch("p/a.rs", true), ch("p/b.rs", true), ch("c", true)];
        let both: &[(&str, &[&str])] = &[("msg-dots", &["p/a.rs", "p/b.rs"]), ("artifacts-keep", &["p/a.rs"])];
        let p = pick_by(&cs, &[], &["p/a.rs", "p/b.rs"], both, false, true).unwrap();
        assert_eq!(p.mine, s(&["p/a.rs", "p/b.rs"]));
        assert_eq!(p.left_out, s(&["c"]));
        assert_eq!(p.decided, vec![("p/a.rs".into(), s(&["msg-dots", "artifacts-keep"])), ("p/b.rs".into(), s(&["msg-dots"]))]);
        // a folder decides nothing: refused, and main is told how
        let e = pick_by(&cs, &[], &["p"], both, false, true).unwrap_err();
        assert!(e.starts_with("p/a.rs is also changed by @msg-dots, @artifacts-keep: not landed, main decides") && e.ends_with("name that file with --add <file>"), "{e}");
        // nor its file tools' claim
        assert!(pick_by(&cs, &["p/a.rs"], &[], both, false, true).is_err());
        // another agent naming it: refused as before
        let e = pick_by(&cs, &[], &["p/a.rs"], both, false, false).unwrap_err();
        assert_eq!(e, "p/a.rs is also changed by @msg-dots, @artifacts-keep: not landed, main decides who lands it");
        // a staged rename named by one side goes whole, decided
        let p = pick_by(&[mv("n.rs", "o.rs")], &[], &["o.rs"], &[("y", &["n.rs"])], false, true).unwrap();
        assert_eq!((p.mine, p.decided), (s(&["n.rs", "o.rs"]), vec![("n.rs".into(), s(&["y"]))]));
    }

    #[test]
    fn decided_words_name_each_file_and_tell_each_agent_once() {
        assert_eq!(decided_note(&[]), "");
        assert!(decided_tells(&[], "main", "abc").is_empty());
        let d = vec![("p/a.rs".to_string(), s(&["y", "z"])), ("p/b.rs".to_string(), s(&["y"]))];
        assert_eq!(decided_note(&d), "on your word, took 2 files other agents also changed: p/a.rs (@y, @z), p/b.rs (@y)");
        let t = decided_tells(&d, "main", "abc1234");
        assert_eq!(t.iter().map(|(a, _)| a.as_str()).collect::<Vec<_>>(), ["y", "z"]);
        assert!(t[0].1.starts_with("main landed p/a.rs, p/b.rs on main in abc1234 (it held your changes too)"), "{}", t[0].1);
        assert!(t[1].1.starts_with("main landed p/a.rs on main in abc1234"), "{}", t[1].1);
    }

    /// land-claims: a claim on a file with no change left is over; the
    /// stale claim that blocked a land no longer does once dropped.
    #[test]
    fn clean_claims_are_the_claims_on_files_with_no_change() {
        let rel = |f: &str| crate::land::relative(std::path::Path::new("/w"), f);
        // @marketing changed tests/k.txt once and landed it: still claimed.
        // @native-lead changes it again: its land is refused
        let cs = [ch("tests/k.txt", true), ch("b", true), mv("new.rs", "old.rs")];
        let blocked = pick_of(&cs, &["tests/k.txt"], &[], &[("marketing", &["tests/k.txt"])], false);
        assert!(blocked.unwrap_err().contains("also changed by @marketing"));
        // the claims while tests/k.txt was clean (marketing's land): over
        let landed = [ch("b", true)];
        let claims = s(&["/w/tests/k.txt", "b", "tests/k.txt", "/elsewhere/x", "old.rs", "/w/tests/k.txt"]);
        assert_eq!(clean_claims(&landed, &claims, rel), s(&["/w/tests/k.txt", "tests/k.txt", "old.rs"]));
        // a claimed file still changed (another agent still editing it,
        // either side of a rename) stays claimed
        assert_eq!(clean_claims(&cs, &claims, rel), Vec::<String>::new());
        // dropped, the new change lands
        let marketing: Vec<String> = s(&["tests/k.txt"]).into_iter().filter(|c| !clean_claims(&landed, &claims, rel).contains(c)).collect();
        let others = [("marketing".to_string(), marketing)];
        let files = s(&["tests/k.txt"]);
        let p = pick(&cs, &Claims { files: &files, add: &[], others: &others, decides: false }, false).unwrap();
        assert_eq!(p.mine, s(&["tests/k.txt"]));
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

    /// issue 37 #3: rust/proto/fixtures/land_pick.jsonl, today's pick on
    /// every case of these tests and more, one per line: its input
    /// (changes, files, add, others, alone) and its answer (mine and
    /// left_out, or why). sb-core's land.bend reads the same file.
    /// `BISE_PROTO_BLESS=1` writes it.
    #[test]
    fn the_pick_fixture_is_todays_pick() {
        use serde_json::json;
        let ch = |p: &str, r: bool| ch(p, r);
        let many: Vec<Change> = (0..=SWEEP_MAX).map(|i| ch(&format!("out/{:03}", i), true)).chain([ch("a", true)]).collect();
        let max: Vec<Change> = many[1..].to_vec();
        let shared = vec![
            ch("a", true),
            ch("b", true),
            ch("c", true),
            ch("old.txt", false),
            ch("gone", true),
            ch("gen/v.svg", true),
            ch("x/", true),
            ch("y/", false),
        ];
        let alone = vec![ch("a", true), ch("Cargo.lock", false), ch("gone", true), mv("new.rs", "old.rs"), ch("n.txt", true), ch("sed.rs", false), ch("x/", false)];
        let gen = vec![ch("c", true), ch("gen/v.svg", true), ch("gen/old.rs", false), ch("gen/y.txt", true)];
        let y: &[(&str, &[&str])] = &[("y", &["gen/y.txt"])];
        type Case<'a> = (Vec<Change>, &'a [&'a str], &'a [&'a str], &'a [(&'a str, &'a [&'a str])], bool);
        let cases: Vec<Case> = vec![
            (alone.clone(), &["a"], &[], &[], true),
            (alone.clone(), &[], &[], &[], true),
            (alone.clone(), &["a"], &[], &[], false),
            (alone, &["a"], &["n.txt"], &[("z", &["sed.rs"])], true),
            (many, &["a"], &[], &[], true),
            (max, &["a"], &[], &[], true),
            (shared.clone(), &["a", "unchanged"], &[], &[("y", &["b"])], false),
            (shared.clone(), &["a"], &[], &[("y", &["b"])], true),
            (shared.clone(), &[], &[], &[], false),
            (shared, &["a"], &["."], &[("y", &["b"])], false),
            (gen.clone(), &[], &["c"], y, false),
            (gen.clone(), &[], &["gen/"], y, false),
            (gen.clone(), &[], &["gen"], y, false),
            (gen.clone(), &[], &["gen/y.txt"], y, false),
            (gen.clone(), &[], &["nope"], y, false),
            (gen.clone(), &[], &["."], y, false),
            (gen.clone(), &[], &["c", "nope"], y, false),
            (gen, &["gen/y.txt"], &[], y, true),
            (vec![mv("b.rs", "a.rs")], &["a.rs"], &[], &[], false),
            (vec![mv("b.rs", "a.rs")], &["b.rs"], &[], &[], false),
            (vec![mv("b.rs", "a.rs")], &[], &[], &[], true),
            (vec![mv("b.rs", "a.rs")], &[], &["a.rs"], &[], false),
            (vec![ch("a.rs", true), ch("b.rs", true)], &["b.rs"], &[], &[], false),
            (vec![ch("a", true)], &["a"], &[], &[("y", &["a"]), ("z", &["a"])], false),
            (vec![ch("a", true)], &[], &[], &[("y", &["a"])], true),
            (vec![ch("a", true), ch("a", true)], &["a"], &[], &[], false),
            (vec![ch("x/", true)], &["x/"], &["x"], &[], false),
            (vec![ch("x/", false), ch("y/", true)], &[], &[], &[], false),
            (vec![], &["a"], &[], &[], true),
            (vec![], &[], &["a"], &[], false),
            (vec![ch("é/ü.txt", true), ch("sp ace", true)], &["sp ace"], &[], &[], false),
            (vec![ch("d/a", true), ch("d/b", false), ch("dd/c", true)], &[], &["d"], &[], false),
            (vec![ch("d/a", true), mv("e/b", "d/b")], &[], &["d/"], &[("y", &["e/b"])], false),
        ];
        // land-decide, after the rows above (theirs keep their bytes: no
        // `decides` in, no `decided` out): main exact, main folder, main's
        // file tools, another agent's exact --add
        let ab: &[(&str, &[&str])] = &[("y", &["p/a.rs"]), ("z", &["p/a.rs", "p/b.rs"])];
        let pab = || vec![ch("p/a.rs", true), ch("p/b.rs", true), ch("c", true)];
        let decided: Vec<Case> = vec![
            (pab(), &[], &["p/a.rs", "p/b.rs"], ab, false),
            (pab(), &[], &["p"], ab, false),
            (pab(), &["p/a.rs"], &[], ab, false),
            (pab(), &[], &["p/b.rs", "c"], ab, false),
            (vec![mv("n.rs", "o.rs")], &[], &["o.rs"], &[("y", &["n.rs"])], false),
        ];
        let rows = cases.iter().map(|c| (c, false)).chain(decided.iter().map(|c| (c, true))).chain([(&decided[0], false)]);
        let strs = |xs: &[String]| json!(xs);
        let lines: Vec<String> = rows
            .map(|((cs, files, add, others, alone), decides)| {
                let mut input = json!({
                    "changes": cs.iter().map(|c| json!({"path": c.path, "from": c.from.clone().unwrap_or_default(), "recent": c.recent})).collect::<Vec<_>>(),
                    "files": files,
                    "add": add.iter().map(|a| json!([a, a])).collect::<Vec<_>>(),
                    "others": others.iter().map(|(n, fs)| json!([n, fs])).collect::<Vec<_>>(),
                    "alone": alone,
                });
                if decides {
                    input["decides"] = json!(true);
                }
                let out = match pick_by(cs, files, add, others, *alone, decides) {
                    Ok(p) => {
                        let mut o = json!({"ok": true, "mine": strs(&p.mine), "left_out": strs(&p.left_out), "why": ""});
                        if !p.decided.is_empty() {
                            o["decided"] = json!(p.decided);
                        }
                        o
                    }
                    Err(e) => json!({"ok": false, "mine": [], "left_out": [], "why": e}),
                };
                json!({"in": input, "out": out}).to_string()
            })
            .collect();
        let text = lines.join("\n") + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../proto/fixtures/land_pick.jsonl");
        if std::env::var("BISE_PROTO_BLESS").is_ok() {
            std::fs::write(&path, &text).expect("write the fixture");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), text, "land_pick.jsonl: BISE_PROTO_BLESS=1 writes it");
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
