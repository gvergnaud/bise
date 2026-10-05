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
    // started: d; a plain mv of mine whose old path no tool saw
    sh(&ws, "echo d > d && git add d && git commit -qm d && echo a2 > a && echo b2 > b && echo d2 > d && git mv d e.txt && mv a a.rs && echo a3 > a.rs");
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
