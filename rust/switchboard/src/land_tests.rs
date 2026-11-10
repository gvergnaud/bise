//! `sb land` against real git (issue 12): a worktree the agent has alone
//! lands every change however it was made, the shared folder names the
//! changes it leaves out. The pure part's tests are in `land_pick.rs`.

use super::tests::{job, landed, lines, log, repo, sh};
use super::*;
use crate::land_pick::SWEEP_MAX;

fn worktree(ws: &Path) -> PathBuf {
    let wt = ws.parent().unwrap().join("wt");
    sh(ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
    wt.canonicalize().unwrap()
}

fn status(dir: &Path) -> Vec<String> {
    git(dir, &["status", "--porcelain", "--untracked-files=all"]).unwrap().lines().map(str::to_string).collect()
}

#[test]
fn a_worktree_alone_lands_every_change_however_made_and_is_clean_after() {
    let ws = repo("wt-all");
    sh(&ws, "echo lock > Cargo.lock && echo r > r.rs && echo m > m.rs && echo '*.log' > .gitignore && git add . && git commit -qm deps");
    let wt = worktree(&ws);
    // a: the file tool's; b: sed; c: rm; r.rs -> s.rs: git mv; m.rs -> n.rs:
    // a plain mv; Cargo.lock: cargo; new/x.txt: a script; build.log:
    // ignored output; sub/: a nested repo
    sh(
        &wt,
        "echo a2 > a && sed -i.bak 's/b/b2/' b && rm b.bak && rm c && git mv r.rs s.rs && mv m.rs n.rs \
         && echo lock2 > Cargo.lock && mkdir new && echo x > new/x.txt && echo out > build.log \
         && mkdir sub && cd sub && git init -q && echo z > z && cd ..",
    );
    let mut j = job("x", &wt, &ws, &["a"], &[]);
    j.place = "wt:x".into();
    j.here = false;
    // x started after init (its rm c is no undo of it, the revert guard
    // looks from x's start on)
    j.since_ms = crate::util::now_ms() - 60_000;
    let o = run(&j, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(log(&ws), lines(&["x's work", "deps", "init"]), "one commit");
    // git show names a rename by its new path (r.rs -> s.rs, m.rs -> n.rs)
    assert_eq!(landed(&ws, "main"), ["Cargo.lock", "a", "b", "c", "n.rs", "new/x.txt", "s.rs"]);
    assert_eq!(git(&ws, &["show", "main:b"]).unwrap(), "b2");
    for gone in ["c", "r.rs", "m.rs"] {
        assert!(git(&ws, &["show", &format!("main:{}", gone)]).is_err(), "{} deleted or renamed", gone);
    }
    assert_eq!(o.left_out, ["sub/"], "the nested repo is named, never taken");
    assert!(left_out_note(&o.left_out).contains("sub/ is its own repo"), "{}", left_out_note(&o.left_out));
    assert_eq!(status(&wt), ["?? sub/"], "clean but the nested repo; build.log ignored");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn the_shared_folder_names_a_changed_tracked_file_and_add_takes_it() {
    let ws = repo("shared-tracked");
    // the user's old change to c: from before the agent started, not news
    sh(&ws, "echo c-old > c && touch -t 200001010000 c");
    let since = crate::util::now_ms() - 60_000;
    // mine: a (file tool); y's: b; the user's (or my sed's) since I
    // started: d (its commit older: out of the revert guard's window); a
    // plain mv of mine whose old path no tool saw
    sh(&ws, "echo d > d && git add d && GIT_COMMITTER_DATE=2000-01-01T00:00:00Z git commit -qm d && echo a2 > a && echo b2 > b && echo d2 > d && git mv d e.txt && mv a a.rs && echo a3 > a.rs");
    let mut j = job("x", &ws, &ws, &["a.rs"], &[("y", &["b"])]);
    j.since_ms = since;
    let o = run(&j, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(landed(&ws, "main"), ["a.rs"], "only mine");
    assert_eq!(o.left_out, ["a", "e.txt"], "the deletion and the rename named; b is y's, c is old");
    let note = left_out_note(&o.left_out);
    assert!(note.contains("left out 2 files") && note.contains("changed or new since you started") && note.contains("--add"), "{}", note);
    // --add the rename: both sides land; --add a: the deletion
    j.add = vec!["e.txt".into(), "a".into()];
    j.message = "the rest".into();
    let o = run(&j, &Queue::default(), &mut || {}).unwrap();
    assert!(o.left_out.is_empty(), "{:?}", o.left_out);
    assert_eq!(landed(&ws, "main"), ["a", "d", "e.txt"]);
    assert_eq!(log(&ws), lines(&["the rest", "x's work", "d", "init"]));
    assert_eq!(status(&ws), [" M b", " M c"], "y's and the user's stay");
    // y's tracked file: refused, even named
    j.add = vec!["b".into()];
    let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
    assert!(e.contains("b is also changed by @y"), "{}", e);
    // the user's old change, named: it lands
    j.add = vec!["c".into()];
    run(&j, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(git(&ws, &["show", "main:c"]).unwrap(), "c-old");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn too_many_changes_in_a_worktree_are_refused_with_the_list() {
    let ws = repo("wt-many-tracked");
    sh(&ws, &format!("mkdir t && for i in $(seq 1 {}); do echo $i > t/$i; done && git add t && git commit -qm t", SWEEP_MAX));
    let wt = worktree(&ws);
    // SWEEP_MAX tracked files changed by a script, plus one deleted
    sh(&wt, "for f in t/*; do echo x >> $f; done && rm a");
    let mut j = job("x", &wt, &ws, &[], &[]);
    j.place = "wt:x".into();
    let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
    assert!(e.starts_with(&format!("{} new or changed files not ignored in the worktree (first: a, t/1, t/10)", SWEEP_MAX + 1)), "{}", e);
    assert_eq!(git(&wt, &["log", "--format=%s", "-1"]).unwrap(), "t", "nothing landed");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn a_moved_base_runs_the_land_check_from_mains_tip_and_red_refuses() {
    // bise.toml at main's tip: a check that is red when the merged tree
    // has both a and its twin (the two-lands-each-export-it case)
    let ws = repo("land-check");
    sh(
        &ws,
        "printf '[land]\\ncheck = \"sh check.sh\"\\n' > bise.toml \
         && printf 'echo \"$1 $2\" > ../args; if [ -f twin ] && [ -f mine ]; then echo \"FAIL dup\"; echo \"error: twin and mine\"; echo \"GATE land FAILED: dup\"; exit 1; fi\\n' > check.sh \
         && git add . && git commit -qm check",
    );
    let wt = worktree(&ws);
    let mut j = job("x", &wt, &ws, &[], &[]);
    j.place = "wt:x".into();
    j.here = false;
    // a fast-forward onto the trunk is checked too (issue 28 part 2), on
    // main's tip .. the branch's
    sh(&wt, "echo 1 > one && git add one && git commit -qm one");
    let (base, head) = (git(&ws, &["rev-parse", "main"]).unwrap(), git(&wt, &["rev-parse", "HEAD"]).unwrap());
    run(&j, &Queue::default(), &mut || {}).unwrap();
    let args = std::fs::read_to_string(ws.parent().unwrap().join("args")).unwrap();
    assert_eq!(args.trim(), format!("{} {}", base, head), "a fast-forward runs the check");
    // and a red one is refused: main unmoved
    sh(&ws, "echo t > twin0 && git add twin0 && git commit -qm twin0");
    sh(&wt, "git rebase -q main && cp twin0 twin && echo m > mine && git add . && git commit -qm clash && rm -f ../args");
    let base = git(&ws, &["rev-parse", "main"]).unwrap();
    assert!(run(&j, &Queue::default(), &mut || {}).unwrap_err().contains("red at dup"));
    assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), base, "main did not move");
    sh(&wt, "git reset -q --hard main");
    // main moves (another land), the branch adds the clash: refused with
    // the step's words, main unmoved, the branch rebased on it
    sh(&ws, "echo t > twin && git add twin && git commit -qm twin");
    sh(&wt, "echo m > mine && git add mine && git commit -qm mine");
    let base = git(&ws, &["rev-parse", "main"]).unwrap();
    let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
    assert_eq!(e, "the land check is red at dup on main + your branch:\u{a}error: twin and mine\u{a}your branch is rebased on main: fix it, then sb land again");
    assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), base, "main did not move");
    assert_eq!(log(&wt), lines(&["twin", "twin0", "one", "check", "init"]));
    assert!(git(&wt, &["merge-base", "--is-ancestor", "main", "HEAD"]).is_ok(), "rebased");
    let head = git(&wt, &["rev-parse", "HEAD"]).unwrap();
    let args = std::fs::read_to_string(ws.parent().unwrap().join("args")).unwrap();
    assert_eq!(args.trim(), format!("{} {}", base, head), "<base> <head>");
    // the branch cannot switch its own check off: it is read at main's tip
    sh(&wt, "printf '[land]\\ncheck = \"true\"\\n' > bise.toml && git commit -qam off");
    sh(&ws, "echo t2 > twin && git commit -qam twin2");
    assert!(run(&j, &Queue::default(), &mut || {}).unwrap_err().contains("red at dup"));
    // fixed: lands
    sh(&wt, "git rm -q mine && git commit -qm fix");
    run(&j, &Queue::default(), &mut || {}).unwrap();
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn a_here_land_on_main_is_checked_in_the_check_tree_and_red_moves_nothing() {
    let ws = repo("here-checked");
    sh(&ws, "echo a2 > a");
    run(&job("x", &ws, &ws, &["a"], &[]), &Queue::default(), &mut || {}).unwrap();
    let checks = job("x", &ws, &ws, &[], &[]).checks;
    assert!(!checks.join(".land-check").exists(), "no bise.toml: no check, no tree");
    // red when the tree has `bad`: run where HEAD is the new commit
    sh(
        &ws,
        "printf '[land]\\ncheck = \"sh check.sh\"\\n' > bise.toml && printf 'git rev-parse HEAD > ../head; [ \"$(git rev-parse HEAD)\" = \"$2\" ] || exit 9; if [ -f bad ]; then echo \"FAIL lines\"; echo \"error: bad\"; echo \"GATE land FAILED: lines\"; exit 1; fi\\n' > check.sh && git add . && git commit -qm check",
    );
    let base = git(&ws, &["rev-parse", "main"]).unwrap();
    sh(&ws, "echo x > bad");
    let e = run(&job("x", &ws, &ws, &["bad"], &[]), &Queue::default(), &mut || {}).unwrap_err();
    assert_eq!(
        e,
        "the land check is red at lines on main + your files:\u{a}error: bad\u{a}nothing landed, your files are as they were: fix them, then sb land --here again"
    );
    assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), base, "main did not move");
    assert!(ws.join("bad").exists(), "the file is kept");
    // green: lands, checked on the commit main now points at
    sh(&ws, "rm bad && echo a3 > a");
    let o = run(&job("x", &ws, &ws, &["a"], &[]), &Queue::default(), &mut || {}).unwrap();
    let tree = crate::scratch::check_tree_dir(&checks, &ws);
    let head = std::fs::read_to_string(tree.parent().unwrap().join("head")).unwrap();
    assert_eq!(head.trim(), git(&ws, &["rev-parse", "main"]).unwrap());
    assert_eq!(crate::flow::land_line("x", 1, &o.target, &o.sha, o.pushed), format!("✓ x landed 1 commit on main ({})", o.sha));
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn a_here_land_on_a_worktree_branch_is_not_checked() {
    let ws = repo("here-branch");
    sh(&ws, "printf '[land]\\ncheck = \"false\"\\n' > bise.toml && git add . && git commit -qm check");
    let wt = worktree(&ws);
    sh(&wt, "echo a2 > a");
    let mut j = job("x", &wt, &ws, &["a"], &[]);
    j.place = "wt:x".into();
    let o = run(&j, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(o.target, "sb/x");
}

/// A worktree on feature `feat` for agent `name` (its own branch sb/<name>).
fn on_feat(ws: &Path, name: &str) -> (PathBuf, Job) {
    let wt = ws.parent().unwrap().join(format!("wt-{}", name));
    sh(ws, &format!("git worktree add -q -b sb/{} {} feat", name, wt.display()));
    let wt = wt.canonicalize().unwrap();
    let mut j = job(name, &wt, ws, &[], &[]);
    (j.place, j.here, j.onto) = (format!("wt:{}", name), false, Some("refs/heads/feat".into()));
    (wt, j)
}

/// `run`, then its record as the daemon writes it (`daemon/land.rs`).
fn land(j: &Job) -> Result<Outcome, String> {
    let o = run(j, &Queue::default(), &mut || {})?;
    crate::land_record::append(&j.lands, &crate::land_record::of(j, &o, 1)).unwrap();
    Ok(o)
}

/// The native-mac incident (04aaaa8d3): y lands b and p/new on the
/// feature; x, its tree still the old one, squashes with `git reset
/// --soft feat` and lands: a fast-forward that put back the old b and
/// deleted p/new. Refused now, naming the files and y; nothing moves.
/// An undo on purpose (`git revert`) lands.
#[test]
fn a_land_that_carries_old_copies_of_anothers_fresh_land_is_refused() {
    let ws = repo("revert-guard");
    sh(&ws, "git branch feat main");
    let (wx, jx) = on_feat(&ws, "x");
    let (wy, jy) = on_feat(&ws, "y");
    sh(&wy, "echo b2 > b && mkdir p && echo p > p/new && git add . && git commit -qm 'y: b and p'");
    land(&jy).unwrap();
    let tip = git(&ws, &["rev-parse", "feat"]).unwrap();
    sh(&wx, "echo a2 > a && git add a && git commit -qm 'x: a' && git reset -q --soft feat && git commit -qm 'x: a, squashed'");
    let e = land(&jx).unwrap_err();
    assert!(e.starts_with("your land puts back old copies of b, p/new, undoing @y's "), "{}", e);
    assert!(e.contains("\"y: b and p\" on feat") && e.contains("nothing moved"), "{}", e);
    assert_eq!(git(&ws, &["rev-parse", "feat"]).unwrap(), tip, "feat did not move");
    // x takes y's version back: its own change lands alone
    sh(&wx, "git checkout -q feat -- b p && git commit -q --amend --no-edit");
    land(&jx).unwrap();
    assert_eq!(git(&ws, &["show", "feat:b"]).unwrap(), "b2");
    assert_eq!(git(&ws, &["show", "feat:a"]).unwrap(), "a2");
    // an undo on purpose names the commit: it lands
    sh(&wx, &format!("git revert --no-edit {}", tip));
    land(&jx).unwrap();
    assert_eq!(git(&ws, &["show", "feat:b"]).unwrap(), "b");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// An agent undoes its own land (its record holds the commit): no refusal.
#[test]
fn undoing_your_own_land_lands() {
    let ws = repo("revert-own");
    sh(&ws, "git branch feat main");
    let (wx, jx) = on_feat(&ws, "x");
    sh(&wx, "echo b2 > b && git commit -qam 'x: b2'");
    land(&jx).unwrap();
    sh(&wx, "echo b > b && git commit -qam 'x: b back'");
    land(&jx).unwrap();
    assert_eq!(git(&ws, &["show", "feat:b"]).unwrap(), "b");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// architect m_42458's mixed case: x edits f and carries y's old copy of
/// it too (y's hunk undone, x's own line new): refused. x keeping y's
/// hunk and editing another line of f lands.
#[test]
fn a_file_both_edited_is_refused_only_when_the_others_lines_are_back() {
    let ws = repo("revert-mixed");
    sh(&ws, "seq 1 30 > f && git add f && git commit -qm f && git branch feat main");
    let (wx, jx) = on_feat(&ws, "x");
    let (wy, jy) = on_feat(&ws, "y");
    sh(&wy, "sed -i.bak 's/^3$/three/' f && rm f.bak && git commit -qam 'y: three'");
    land(&jy).unwrap();
    let tip = git(&ws, &["rev-parse", "feat"]).unwrap();
    // x's old tree (3), its own edit at 25, put on feat by a reset
    sh(&wx, "sed -i.bak 's/^25$/twenty-five/' f && rm f.bak && git commit -qam 'x: 25' && git reset -q --soft feat && git commit -qm 'x: 25, squashed'");
    let e = land(&jx).unwrap_err();
    assert!(e.starts_with("your land puts back old copies of f, undoing @y's "), "{}", e);
    assert_eq!(git(&ws, &["rev-parse", "feat"]).unwrap(), tip, "feat did not move");
    // rebuilt on y's f: lands
    sh(&wx, "git reset -q --hard feat && sed -i.bak 's/^25$/twenty-five/' f && rm f.bak && git commit -qam 'x: 25'");
    land(&jx).unwrap();
    assert_eq!(git(&ws, &["show", "feat:f"]).unwrap().lines().filter(|l| l.contains('t')).collect::<Vec<_>>(), ["three", "twenty-five"]);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// native-lead's second case (5e44e9d26): a killed bless kept writing in
/// x's worktree after its rebase: it deleted y's fresh generated file
/// and rewrote the shared one without y's line; x's plain land (every
/// change of a worktree it has alone) took it. Refused, naming both: no
/// old line of y's is back (y only added), but all of y's 2-file commit
/// is out (`land_revert::WHOLE`).
#[test]
fn a_dirty_worktree_that_drops_anothers_fresh_files_is_refused() {
    let ws = repo("revert-dirty");
    sh(&ws, "seq 1 20 > words.gen && git add words.gen && git commit -qm words && git branch feat main");
    let (wx, jx) = on_feat(&ws, "x");
    let (wy, jy) = on_feat(&ws, "y");
    sh(&wy, "echo menus >> words.gen && echo m > menus.gen && git add . && git commit -qm 'y: menus words'");
    land(&jy).unwrap();
    let tip = git(&ws, &["rev-parse", "feat"]).unwrap();
    // x rebased on y's land, then the old bless wrote its own view
    sh(&wx, "git rebase -q feat && seq 1 20 > words.gen && echo side >> words.gen && rm menus.gen && echo h > host.tsx");
    let e = land(&jx).unwrap_err();
    assert!(e.starts_with("your land puts back old copies of menus.gen, words.gen, undoing @y's "), "{}", e);
    assert_eq!(git(&ws, &["rev-parse", "feat"]).unwrap(), tip, "feat did not move");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// native-lead m_42665 (nm-side's L3 e805ff52f): y's change to f was
/// already undone on the feature before x started (a land from before
/// the guard, no record: nm-side's own 1d9aff6bb put back ff62d5826's
/// old side words). x's land only edits another line of f: it undoes
/// nothing, the base already had y's old line. It lands.
#[test]
fn a_change_already_undone_on_the_target_is_not_the_landers() {
    let ws = repo("revert-before");
    sh(&ws, "seq 1 30 > f && git add f && git commit -qm f && git branch feat main");
    let (wx, jx) = on_feat(&ws, "x");
    let (wy, jy) = on_feat(&ws, "y");
    sh(&wy, "sed -i.bak 's/^3$/three/' f && rm f.bak && git commit -qam 'y: three'");
    land(&jy).unwrap();
    // a land from before the guard puts 3 back: no record, no check
    sh(&wy, "sed -i.bak 's/^three$/3/' f && rm f.bak && git commit -qam 'z: 3 back' && git branch -f feat HEAD");
    sh(&wx, "git rebase -q feat && sed -i.bak 's/^25$/twenty-five/' f && rm f.bak && git commit -qam 'x: 25'");
    land(&jx).unwrap();
    assert_eq!(git(&ws, &["show", "feat:f"]).unwrap().lines().filter(|l| l.contains('t')).collect::<Vec<_>>(), ["twenty-five"]);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// y lands `script` from its worktree onto main; x, in the shared folder
/// since before it, gets main's files. The (repo, x's job) of a
/// shared-folder land.
fn shared_after(tag: &str, setup: &str, script: &str) -> (PathBuf, Job) {
    let ws = repo(tag);
    if !setup.is_empty() {
        sh(&ws, &format!("{} && git add . && git commit -qm setup", setup));
    }
    let since = crate::util::now_ms() - 60_000;
    let wy = ws.parent().unwrap().join("wt-y");
    sh(&ws, &format!("git worktree add -q -b sb/y {} main", wt_path(&wy)));
    let mut jy = job("y", &wy.canonicalize().unwrap(), &ws, &[], &[]);
    (jy.place, jy.here) = ("wt:y".into(), false);
    sh(&jy.dir, &format!("{} && git add -A && git commit -qm 'y: theirs'", script));
    land(&jy).unwrap();
    let mut jx = job("x", &ws, &ws, &[], &[]);
    jx.since_ms = since;
    (ws, jx)
}

fn wt_path(p: &Path) -> String {
    p.display().to_string()
}

/// The known_reds 'send' line (70bf3c190, then fast-loop's f669dd292):
/// y adds a line; x, later, removes it from the shared folder. Its file
/// is y's old copy line for line, yet nothing old is back: an edit. It
/// lands.
#[test]
fn removing_a_line_another_agent_added_lands() {
    let (ws, mut jx) = shared_after("undo-line", "printf 'red\\nflake\\n' > known", "echo send >> known");
    sh(&ws, "printf 'red\\nflake\\n' > known");
    jx.files = vec!["known".into()];
    run(&jx, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(git(&ws, &["show", "main:known"]).unwrap(), "red\nflake");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// a1d599824: x rewrites the lines y wrote; one of the lines y had
/// replaced comes back, by chance, elsewhere in x's new code. Not in its
/// place: an edit. It lands.
#[test]
fn an_old_line_written_again_out_of_its_place_lands() {
    let (ws, mut jx) = shared_after("undo-coincidence", "seq 1 10 > f", "sed -i.bak 's/^5$/let y = 5;/' f && rm f.bak");
    // y's line gone, its old `5` written again at the end, far from 4 and 6
    sh(&ws, "seq 1 10 | sed 's/^5$/let x = 5;/' > f && echo 5 >> f");
    jx.files = vec!["f".into()];
    run(&jx, &Queue::default(), &mut || {}).unwrap();
    assert!(git(&ws, &["show", "main:f"]).unwrap().contains("let x = 5;"));
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// An old copy from the shared folder (y's replaced line back in its
/// place) is refused with the shared folder's way through: name the
/// commit in the land's message (a `git revert` there is a commit sb
/// land never reads). Named, it lands.
#[test]
fn a_shared_folder_land_undoes_on_purpose_by_naming_the_commit() {
    let (ws, mut jx) = shared_after("undo-named", "seq 1 10 > f", "sed -i.bak 's/^5$/five/' f && rm f.bak");
    let y = git(&ws, &["rev-parse", "--short=7", "main"]).unwrap();
    sh(&ws, "seq 1 10 > f");
    jx.files = vec!["f".into()];
    let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
    assert!(e.starts_with("your land puts back old copies of f, undoing @y's "), "{}", e);
    assert!(e.ends_with(&format!("to undo theirs on purpose: name it in your sb land message ('reverts {}')", y)), "{}", e);
    jx.message = format!("f: 5 again (reverts {})", y);
    run(&jx, &Queue::default(), &mut || {}).unwrap();
    assert_eq!(git(&ws, &["show", "main:f"]).unwrap(), (1..=10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n"));
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// A 2-file commit of y's: x taking y's line out of one file is an
/// edit; taking all of y's change out of both is a revert by its shape
/// ([`crate::land_revert::WHOLE`]): refused unless named.
#[test]
fn only_a_whole_multi_file_commit_taken_out_is_refused() {
    let (ws, mut jx) = shared_after("undo-whole", "seq 1 5 > f && seq 1 5 > g", "echo y >> f && echo y >> g");
    sh(&ws, "seq 1 5 > f");
    jx.files = vec!["f".into()];
    run(&jx, &Queue::default(), &mut || {}).unwrap();
    // the other file's line too, in a later land: f is already y's old
    // copy at its base, so g alone is not all of y's commit
    sh(&ws, "seq 1 5 > g");
    jx.files = vec!["g".into()];
    run(&jx, &Queue::default(), &mut || {}).unwrap();
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    // both at once: refused
    let (ws, mut jx) = shared_after("undo-whole2", "seq 1 5 > f && seq 1 5 > g", "echo y >> f && echo y >> g");
    sh(&ws, "seq 1 5 > f && seq 1 5 > g");
    jx.files = vec!["f".into(), "g".into()];
    let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
    assert!(e.starts_with("your land puts back old copies of f, g, undoing @y's "), "{}", e);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}
