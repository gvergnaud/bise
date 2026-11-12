//! Feature branches (dev-flow §5.1): git in temp repos, and the words.

use super::*;
use crate::land::{self, Job, Queue};
use std::process::Command;

fn sh(dir: &Path, script: &str) {
    let ok = Command::new("/bin/sh").arg("-c").arg(script).current_dir(dir).status().unwrap().success();
    assert!(ok, "{}", script);
}

fn out(dir: &Path, args: &[&str]) -> String {
    git(dir, args).unwrap()
}

/// A repo on main with a, b, c; its root holds `repo/` and room for
/// worktrees and scratch.
fn repo(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sb-feature-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()));
    let ws = root.join("repo");
    std::fs::create_dir_all(&ws).unwrap();
    sh(
        &ws,
        "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo a > a && echo b > b && echo c > c && git add . && git commit -qm init",
    );
    ws.canonicalize().unwrap()
}

fn job(agent: &str, dir: &Path, shared: &Path, files: &[&str], onto: &str) -> Job {
    Job {
        agent: agent.into(),
        here: false,
        message: format!("{}'s work", agent),
        place: format!("wt:{}", agent),
        dir: dir.to_path_buf(),
        worktree: true,
        shared: shared.to_path_buf(),
        files: files.iter().map(|s| s.to_string()).collect(),
        others: Vec::new(),
        add: Vec::new(),
        decides: false,
        since_ms: 0,
        flow: crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), ..Default::default() },
        onto: Some(format!("refs/heads/{}", onto)),
        checks: shared.parent().unwrap().join("worktrees"),
        lands: shared.parent().unwrap().join("lands.jsonl"),
    }
}

/// An agent's worktree from the feature's tip, as `worktree_feature`
/// makes it.
fn agent_wt(ws: &Path, agent: &str, feature: &str) -> PathBuf {
    let wt = ws.parent().unwrap().join(agent);
    sh(ws, &format!("git worktree add -q -b sb/{} {} {}", agent, wt.display(), feature));
    wt.canonicalize().unwrap()
}

#[test]
fn new_lands_sync_merge_end_to_end() {
    let ws = repo("e2e");
    let scratch = ws.parent().unwrap().join("scratch");
    // new: a branch from main's tip, nothing else moves
    let f = create(&ws, "cu", 1).unwrap();
    assert!(!f.adopted);
    assert_eq!(f.base, out(&ws, &["rev-parse", "main"]));
    assert!(exists(&ws, "cu"));
    // two agents, each its own worktree, land on cu, never on main
    let a = agent_wt(&ws, "cu-a", "cu");
    let b = agent_wt(&ws, "cu-b", "cu");
    sh(&a, "echo a2 > a");
    sh(&b, "echo new > n");
    let o = land::run(&job("cu-a", &a, &ws, &["a"], "cu"), &Queue::default(), &mut || {}).unwrap();
    assert_eq!((o.target.as_str(), o.commits, o.pushed), ("cu", 1, None));
    let o = land::run(&job("cu-b", &b, &ws, &["n"], "cu"), &Queue::default(), &mut || {}).unwrap();
    assert_eq!((o.target.as_str(), o.commits), ("cu", 1), "b's land rebased on a's");
    assert_eq!(out(&ws, &["log", "--format=%s", "main"]), "init", "main never moved");
    assert_eq!(out(&ws, &["log", "--format=%s", "cu"]), "cu-b's work\u{a}cu-a's work\u{a}init");
    let fx = facts(&ws, "refs/heads/main", "cu").unwrap();
    assert_eq!((fx.ahead, fx.behind, fx.adds, fx.dels), (2, 0, 2, 1));
    // main moves meanwhile: sync rebases cu, the clean worktrees follow
    sh(&ws, "echo c2 > c && git commit -qam 'main moves'");
    let fx = facts(&ws, "refs/heads/main", "cu").unwrap();
    assert_eq!((fx.ahead, fx.behind), (2, 1));
    let (old, new) = rebase(&ws, &scratch, "cu", Some("test -f n")).unwrap().unwrap();
    assert_ne!(old, new);
    let left = follow(&[("cu-a".into(), a.clone()), ("cu-b".into(), b.clone())], &old, &new);
    assert!(left.is_empty(), "{:?}", left);
    assert_eq!(out(&b, &["rev-parse", "HEAD"]), new, "b's worktree is on the new tip");
    assert_eq!(facts(&ws, "refs/heads/main", "cu").unwrap().behind, 0);
    assert!(rebase(&ws, &scratch, "cu", None).unwrap().is_none(), "on main's tip already");
    // a failing check: nothing moves
    sh(&ws, "echo c3 > c && git commit -qam 'main again'");
    let tip = out(&ws, &["rev-parse", "cu"]);
    let e = rebase(&ws, &scratch, "cu", Some("exit 3")).unwrap_err().to_string();
    assert!(e.contains("the check `exit 3` failed"), "{}", e);
    assert_eq!(out(&ws, &["rev-parse", "cu"]), tip);
    // merge: rebased, checked, main fast-forwarded (the shared folder's
    // files too), linear; the branch then goes to the trash
    let flow = crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), check: Some("test -f n".into()), ..Default::default() };
    let q = Queue::default();
    let m = merge(&ws, &scratch, &scratch, "cu", &flow, &q.wait_turn("refs/heads/main", "feature:cu", &mut || {})).unwrap();
    assert_eq!((m.commits, m.pushed), (2, None), "no remote: no push");
    assert_eq!(out(&ws, &["log", "--format=%s", "main"]), "cu-b's work\u{a}cu-a's work\u{a}main again\u{a}main moves\u{a}init");
    assert_eq!(std::fs::read_to_string(ws.join("n")).unwrap(), "new\u{a}", "the shared folder moved with main");
    assert_eq!(out(&ws, &["status", "--porcelain"]), "");
    let r = trash(&ws, "cu", 7).unwrap();
    assert_eq!(r, "refs/switchboard/trash/feature-cu/7");
    assert!(!exists(&ws, "cu"));
    assert_eq!(out(&ws, &["rev-parse", &r]), out(&ws, &["rev-parse", "main"]));
    // no scratch worktree left behind
    assert_eq!(out(&ws, &["worktree", "list"]).lines().count(), 3, "repo, cu-a, cu-b");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn an_existing_branch_is_adopted_as_it_is() {
    // computer-use, made by hand: commits on a local branch
    let ws = repo("adopt");
    sh(&ws, "git checkout -q -b computer-use && echo x > x && git add x && git commit -qm cu1 && git checkout -q main && echo c2 > c && git commit -qam m1");
    let tip = out(&ws, &["rev-parse", "computer-use"]);
    let f = create(&ws, "computer-use", 1).unwrap();
    assert!(f.adopted);
    assert_eq!(out(&ws, &["rev-parse", "computer-use"]), tip, "untouched");
    assert_eq!(f.base, out(&ws, &["rev-parse", "main~1"]));
    let fx = facts(&ws, "refs/heads/main", "computer-use").unwrap();
    assert_eq!((fx.ahead, fx.behind), (1, 1));
    // names: a task name's, never the default branch
    assert!(create(&ws, "main", 1).unwrap_err().contains("default branch"));
    assert!(create(&ws, "Bad_Name", 1).unwrap_err().contains("invalid feature name"));
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn a_conflicting_sync_says_the_files_and_moves_nothing() {
    let ws = repo("conflict");
    let scratch = ws.parent().unwrap().join("scratch");
    create(&ws, "cu", 1).unwrap();
    let a = agent_wt(&ws, "cu-a", "cu");
    sh(&a, "echo mine > a");
    land::run(&job("cu-a", &a, &ws, &["a"], "cu"), &Queue::default(), &mut || {}).unwrap();
    sh(&ws, "echo theirs > a && git commit -qam 'main edits a'");
    let tip = out(&ws, &["rev-parse", "cu"]);
    let e = rebase(&ws, &scratch, "cu", None).unwrap_err();
    assert_eq!(e, Failed::Conflicts { files: vec!["a".into()], line: "a changed on main too: the rebase of cu on main conflicts".into() });
    assert_eq!(out(&ws, &["rev-parse", "cu"]), tip);
    let q = Queue::default();
    let turn = q.wait_turn("refs/heads/main", "feature:cu", &mut || {});
    let e = merge(&ws, &scratch, &scratch, "cu", &crate::flow::FlowConfig::default(), &turn).unwrap_err();
    assert!(matches!(&e, Failed::Conflicts { files, .. } if files == &["a"]), "{e:?}");
    assert_eq!(out(&ws, &["log", "-1", "--format=%s", "main"]), "main edits a");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

fn now_s() -> u64 {
    crate::util::now_ms() / 1000
}

/// A feature that merged main in (native-mac did): its merge resolved a
/// conflict on `a` and deleted `b` (an evil merge). Then main moves.
/// Its commits are dated a minute after `init`, so the revert guard,
/// started 30 s after `init`, judges main's two commits, not `init`.
fn feature_with_a_main_merge(tag: &str) -> PathBuf {
    let ws = repo(tag);
    create(&ws, "cu", 1).unwrap();
    // its commits after `init` by a minute (the revert guard reads them
    // as recent, `init` not: below)
    let at = format!("@{} +0000", now_s() + 60);
    sh(
        &ws,
        &format!(
            "export GIT_COMMITTER_DATE='{at}' GIT_AUTHOR_DATE='{at}'; \
             git checkout -q cu && echo mine > a && git commit -qam 'cu edits a' \
             && git checkout -q main && echo theirs > a && git commit -qam 'main edits a' \
             && git checkout -q cu && (git merge -q main -m x || true) && echo resolved > a && git rm -q b \
             && git commit -qam 'merge main into cu' && echo d > d && git add d && git commit -qm 'cu adds d' \
             && git checkout -q main && echo c2 > c && git commit -qam 'main moves'"
        ),
    );
    ws
}

/// brief-feature-merge: a feature that holds merges of main lands as a
/// merge commit (first parent: main's tip), its merge's resolution kept
/// and what it deleted still gone. The plain rebase failed there (it
/// replays `cu edits a` on `main edits a`).
#[test]
fn a_feature_with_merges_lands_as_a_merge_commit() {
    let ws = feature_with_a_main_merge("merges");
    let scratch = ws.parent().unwrap().join("scratch");
    assert!(has_merges(&ws, "refs/heads/main", "cu").unwrap());
    let before = out(&ws, &["rev-parse", "main"]);
    let feat = out(&ws, &["rev-parse", "cu"]);
    let flow = crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), check: Some("test -f d && ! test -f b".into()), ..Default::default() };
    let q = Queue::default();
    let m = merge(&ws, &scratch, &scratch, "cu", &flow, &q.wait_turn("refs/heads/main", "feature:cu", &mut || {})).unwrap();
    let tip = out(&ws, &["rev-parse", "main"]);
    assert_eq!(out(&ws, &["rev-parse", "main^1"]), before, "first parent: main's tip");
    assert_eq!(out(&ws, &["rev-parse", "main^2"]), feat, "second parent: the feature, as it was");
    assert_eq!(out(&ws, &["log", "-1", "--format=%s", "main"]), "merge cu");
    assert_eq!(m.sha, crate::land::shorten(&ws, &tip));
    assert_eq!(out(&ws, &["show", "main:a"]), "resolved", "the merge's resolution kept");
    assert!(git(&ws, &["cat-file", "-e", "main:b"]).is_err(), "what the merge deleted stays deleted");
    assert_eq!(out(&ws, &["show", "main:c"]), "c2", "main's own move kept");
    assert_eq!(std::fs::read_to_string(ws.join("d")).unwrap(), "d\u{a}", "the shared folder moved with main");
    // the revert guard judges base -> tip: the merge commit undoes nothing
    // (started after `init`: main's `main edits a` and `main moves` are recent)
    let since = (now_s() + 30) * 1000;
    crate::land_revert::guard(&ws, "refs/heads/main", &before, &tip, since, "feature:cu", &[], crate::land_revert::Route::Revert).unwrap();
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// The same feature on main's tip already (it merged main last): still a
/// merge commit on main, never a fast-forward onto the feature's line.
#[test]
fn a_feature_with_merges_on_main_s_tip_still_lands_as_a_merge_commit() {
    let ws = feature_with_a_main_merge("merges-tip");
    sh(&ws, "git checkout -q cu && git merge -q --no-edit main && git checkout -q main");
    let scratch = ws.parent().unwrap().join("scratch");
    let before = out(&ws, &["rev-parse", "main"]);
    let q = Queue::default();
    let flow = crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), ..Default::default() };
    merge(&ws, &scratch, &scratch, "cu", &flow, &q.wait_turn("refs/heads/main", "feature:cu", &mut || {})).unwrap();
    assert_eq!(out(&ws, &["rev-parse", "main^1"]), before, "first parent: main's tip");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// A sync of a feature with merges merges main into it (never a rebase):
/// the feature's tip is a merge whose first parent is its old tip.
#[test]
fn a_sync_of_a_feature_with_merges_merges_main_in() {
    let ws = feature_with_a_main_merge("merges-sync");
    let scratch = ws.parent().unwrap().join("scratch");
    // an agent's commit on the feature, not landed yet
    let a = agent_wt(&ws, "cu-a", "cu");
    sh(&a, "echo e > e && git add e && git commit -qm 'cu-a, not landed'");
    let (old, new) = rebase(&ws, &scratch, "cu", Some("test -f d")).unwrap().unwrap();
    // its worktree follows (rebase --onto new old): its commit survives
    assert!(follow(&[("cu-a".into(), a.clone())], &old, &new).is_empty());
    assert_eq!(out(&a, &["rev-parse", "HEAD^"]), new);
    assert_eq!(out(&a, &["log", "-1", "--format=%s"]), "cu-a, not landed");
    assert_eq!(out(&a, &["show", "HEAD:a"]), "resolved");
    assert_eq!(out(&ws, &["rev-parse", "cu^1"]), old);
    assert_eq!(out(&ws, &["rev-parse", "cu^2"]), out(&ws, &["rev-parse", "main"]));
    assert_eq!(out(&ws, &["rev-parse", "cu"]), new);
    assert_eq!(out(&ws, &["show", "cu:a"]), "resolved");
    assert_eq!(out(&ws, &["show", "cu:c"]), "c2");
    assert!(rebase(&ws, &scratch, "cu", None).unwrap().is_none(), "on main's tip now");
    // a conflict of the merge: the files said, nothing moved
    sh(&ws, "echo main2 > d && git add d && git commit -qm 'main adds d'");
    let tip = out(&ws, &["rev-parse", "cu"]);
    let e = rebase(&ws, &scratch, "cu", None).unwrap_err();
    assert!(e.to_string().starts_with("d changed on main too"), "{e}");
    assert_eq!(out(&ws, &["rev-parse", "cu"]), tip);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn the_try_build_and_its_run_line() {
    let ws = repo("try");
    let scratch = ws.parent().unwrap().join("scratch");
    create(&ws, "cu", 1).unwrap();
    // `{branch}`: from the shared folder, the name in place (this repo's
    // `scripts/versions.sh build {branch}`), the last line is the output
    let t = try_build(&ws, &scratch, "cu", "echo building {branch}; echo /v/abc1234", Some("{out}/bise"), 5).unwrap();
    assert_eq!((t.run.as_str(), t.at_ms), ("/v/abc1234/bise", 5));
    assert_eq!(t.sha, out(&ws, &["rev-parse", "--short", "cu"]));
    // no `{branch}`: run in a worktree of the tip, kept for the try
    let t = try_build(&ws, &scratch, "cu", "test -f a && pwd", None, 6).unwrap();
    assert!(t.run.ends_with("scratch/try-cu"), "{}", t.run);
    let t = try_build(&ws, &scratch, "cu", "true", Some("cd {dir} && make run"), 7).unwrap();
    assert!(t.run.starts_with("cd ") && t.run.ends_with("try-cu && make run"), "{}", t.run);
    let e = try_build(&ws, &scratch, "cu", "echo nope >&2; exit 1", None, 8).unwrap_err();
    assert!(e.contains("the try build `echo nope >&2; exit 1` failed: nope"), "{}", e);
    // the diff for `2 show the diff`
    sh(&ws, "git checkout -q cu && echo x > x && git add x && git commit -qm x && git checkout -q main");
    let file = ws.parent().unwrap().join("d/cu.diff");
    write_diff(&ws, "cu", &file).unwrap();
    let d = std::fs::read_to_string(&file).unwrap();
    assert!(d.contains("1 file changed") && d.contains("+x"), "{}", d);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[test]
fn the_registry_round_trips() {
    let dir = std::env::temp_dir().join(format!("sb-feature-reg-{}-{}", std::process::id(), crate::util::now_ms()));
    assert_eq!(Registry::load(&dir), Registry::default(), "none yet");
    let mut r = Registry::default();
    r.features.push(Feature { name: "cu".into(), base: "abc".into(), created_ms: 1, ..Feature::default() });
    r.get_mut("cu").unwrap().tried = Some(Tried { sha: "fd25c45".into(), run: "~/v/fd25c45/bise".into(), at_ms: 2 });
    r.save(&dir).unwrap();
    assert_eq!(Registry::load(&dir), r);
    // an older file (no tried, no trial): the defaults
    std::fs::write(Registry::file(&dir), r#"{"features":[{"name":"x","base":"b","created_ms":3}]}"#).unwrap();
    let old = Registry::load(&dir);
    assert_eq!((old.features[0].tried.is_none(), old.features[0].trial), (true, false));
    r.remove("cu");
    assert!(r.get("cu").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The words, as designer wrote them (d4172e6, dev-flow §5.1, §7).
#[test]
fn the_items_lids_and_lines() {
    let f = Facts { ahead: 14, behind: 3, adds: 3120, dels: 410, tip: "x".into() };
    assert_eq!(
        try_text("computer-use", &f, "main", true),
        "computer-use is ready to try\u{a}14 commits on computer-use, 3 behind main · +3,120 −410\u{a}the check passes. none of it is on main yet.\u{a}\u{a}1. try it\u{a}2. show the diff\u{a}3. not yet"
    );
    let synced = Facts { behind: 0, ..f.clone() };
    assert!(try_text("cu", &synced, "main", false).starts_with("cu is ready to try\u{a}14 commits on cu · +3,120 −410\u{a}none of it"));
    let t = Tried { sha: "fd25c45".into(), run: "~/.bise/dev/versions/fd25c45/bise".into(), at_ms: 0 };
    assert_eq!(
        merge_text("computer-use", &f, "main", &t, true),
        "merge computer-use into main?\u{a}you tried fd25c45: ~/.bise/dev/versions/fd25c45/bise\u{a}14 commits · the check passes.\u{a}\u{a}1. merge\u{a}2. keep working\u{a}3. drop the branch"
    );
    assert_eq!(drop_question("computer-use", 14), "drop computer-use? 14 commits go.");
    assert_eq!(drop_question("x", 1), "drop x? 1 commit go.");
    let feat = Feature { name: "computer-use".into(), ..Feature::default() };
    assert_eq!(lid(&feat, Some(&f), "main", false, 0), "feature · 14 commits · 3 behind main · not tried");
    assert_eq!(lid(&feat, Some(&synced), "main", true, 0), "feature · 14 commits · building to try");
    let tried = Feature { tried: Some(Tried { at_ms: 0, ..t.clone() }), ..feat.clone() };
    assert_eq!(lid(&tried, Some(&synced), "main", false, 18 * 60_000), "feature · 14 commits · tried fd25c45 18m ago");
    assert_eq!(lid(&feat, None, "main", false, 0), "feature · not tried");
    assert_eq!(flow_line(&[]), "");
    assert_eq!(flow_line(&[(&feat, 3, Some(&f))]), "1 feature branch: computer-use (3 agents, 14 commits, not tried)");
    assert_eq!(
        flow_line(&[(&feat, 1, Some(&f)), (&tried, 0, None)]),
        "2 feature branches: computer-use (1 agent, 14 commits, not tried), computer-use (0 agents, tried fd25c45)"
    );
    let m = Merged { commits: 14, sha: "a1b2c3d".into(), pushed: Some(true), push_error: None };
    assert_eq!(merged_line("computer-use", "main", &m, 3), "✓ computer-use merged into main (14 commits, a1b2c3d) · pushed · its 3 agents archived");
    let local = Merged { pushed: None, ..m.clone() };
    assert_eq!(merged_line("cu", "main", &local, 1), "✓ cu merged into main (14 commits, a1b2c3d) · its agent archived");
    assert_eq!((thousands(0), thousands(999), thousands(1000), thousands(1234567)), ("0".into(), "999".into(), "1,000".into(), "1,234,567".into()));
    assert_eq!((place_id("cu"), of_place("feature:cu"), of_place("wt:cu")), ("feature:cu".into(), Some("cu"), None));
}

// `sb feature retire`: the feature's work in main (merged by hand, a
// merge commit, like ambient-app's), the check, the rename, the words
#[test]
fn a_retire_checks_the_tip_is_in_main_and_renames_the_branch() {
    let ws = repo("retire");
    create(&ws, "cu", 1).unwrap();
    let a = agent_wt(&ws, "cu-a", "cu");
    sh(&a, "echo a2 > a");
    land::run(&job("cu-a", &a, &ws, &["a"], "cu"), &Queue::default(), &mut || {}).unwrap();
    // one commit main lacks: refused, the number said
    assert_eq!(missing(&ws, "refs/heads/main", "cu").unwrap(), 1);
    assert_eq!(
        retire_refusal("cu", "main", 1),
        "cu has 1 commit not in main: a retire ends only a feature whose work is on main already. `sb feature merge cu` puts it there; `sb feature drop cu` throws it away"
    );
    // merged by hand (a merge commit): inside main now
    sh(&ws, "echo c2 > c && git commit -qam 'main moves' && git merge -q --no-ff cu -m 'Merge cu'");
    assert_eq!(missing(&ws, "refs/heads/main", "cu").unwrap(), 0);
    let tip = out(&ws, &["rev-parse", "cu"]);
    // the name: never one that exists
    assert_eq!(retired_name("cu", &|b| exists(&ws, b)), "cu-retired");
    sh(&ws, "git branch cu-retired main");
    assert_eq!(retired_name("cu", &|b| exists(&ws, b)), "cu-retired-2");
    assert_eq!(last_retired(&ws, "cu").as_deref(), Some("cu-retired"));
    rename(&ws, "cu", "cu-retired-2").unwrap();
    assert!(!exists(&ws, "cu"));
    assert_eq!(out(&ws, &["rev-parse", "cu-retired-2"]), tip, "the branch keeps its commits");
    assert_eq!(last_retired(&ws, "cu").as_deref(), Some("cu-retired-2"));
    assert_eq!(last_retired(&ws, "other"), None);
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

// a land in line on a feature that a retire ended meanwhile goes to the
// trunk: `onto_now` is read again once its turn comes
#[test]
fn a_land_queued_behind_a_retire_goes_to_main() {
    let ws = repo("retire-land");
    create(&ws, "cu", 1).unwrap();
    let a = agent_wt(&ws, "cu-a", "cu");
    sh(&a, "echo a2 > a");
    let q = Queue::default();
    // the retire holds the feature's turn; the land waits behind it
    let turn = q.wait_turn("refs/heads/cu", "feature:cu", &mut || {});
    let (j, q2) = (job("cu-a", &a, &ws, &["a"], "cu"), q.clone());
    let land = std::thread::spawn(move || land::run_onto(&j, &q2, &mut || {}, &mut || None));
    std::thread::sleep(std::time::Duration::from_millis(100));
    sh(&ws, "git branch -m cu cu-retired");
    drop(turn);
    let o = land.join().unwrap().unwrap();
    assert_eq!((o.target.as_str(), o.commits), ("main", 1));
    assert_eq!(out(&ws, &["log", "--format=%s", "main"]), "cu-a's work\u{a}init");
    // the feature still there and still the agent's: it lands there
    create(&ws, "cv", 2).unwrap();
    let b = agent_wt(&ws, "cv-b", "cv");
    sh(&b, "echo b2 > b");
    let o = land::run_onto(&job("cv-b", &b, &ws, &["b"], "cv"), &q, &mut || {}, &mut || Some("refs/heads/cv".into())).unwrap();
    assert_eq!(o.target, "cv");
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

/// brief-feature-merge: the brief main gets when his pick failed, over
/// the 3 ops x the 4 failures: what he picked and when, what failed, the
/// steps (merge the trunk in, never rebase; the gate; the op again), the
/// agents that must not land meanwhile.
#[test]
fn the_handoff_brief_says_what_failed_and_the_steps() {
    let failures = [
        Failed::Conflicts { files: vec!["a.rs".into(), "b.rs".into()], line: "a.rs, b.rs changed on main too".into() },
        Failed::Check("the check `tests/gate.sh quick` failed: ...\nFAILED tests::x\nerror: 1 test failed".into()),
        Failed::Moved("main moved meanwhile: sb land again".into()),
        Failed::Other("no feature cu: `sb feature` lists them".into()),
    ];
    let agents = vec!["cu-a".to_string(), "cu-b".to_string()];
    for op in ["merge", "try", "drop"] {
        for failed in &failures {
            let h = Handoff { op, name: "cu", trunk: "main", base: "abc1234", tip: "def5678", picked_ms: 1_793_021_520_000, failed, agents: &agents };
            let t = handoff(&h);
            let ctx = format!("{op} {failed:?}:\n{t}");
            assert!(t.starts_with(&format!("the user picked '{}' on feature cu at 2026-10-26 ", op)), "{ctx}");
            assert!(t.contains("his card is closed") && t.contains("(main at abc1234, cu at def5678)"), "{ctx}");
            assert!(t.contains("@cu-a, @cu-b") && t.contains("must not land on cu meanwhile"), "{ctx}");
            let again = match op {
                "merge" => "`sb feature merge cu` again",
                "try" => "`sb feature ready cu`",
                _ => "`sb feature drop cu`",
            };
            assert!(t.contains(again), "{ctx}");
            let fix = matches!(failed, Failed::Conflicts { .. } | Failed::Check(_));
            assert_eq!(t.contains("never rebase a feature that holds merges") && t.contains("`tests/gate.sh quick`"), fix, "{ctx}");
            match failed {
                Failed::Conflicts { .. } => assert!(t.contains("conflicts on 2 files:\n- a.rs\n- b.rs"), "{ctx}"),
                Failed::Check(_) => assert!(t.contains("its last lines:\n") && t.ends_with("must not land on cu meanwhile: tell them.") && t.contains("error: 1 test failed"), "{ctx}"),
                Failed::Moved(_) => assert!(t.contains("main moved under it twice"), "{ctx}"),
                Failed::Other(e) => assert!(t.contains(e.as_str()), "{ctx}"),
            }
        }
    }
    // no live agent, no tips known
    let failed = Failed::Other("x".into());
    let t = handoff(&Handoff { op: "merge", name: "cu", trunk: "main", base: "", tip: "", picked_ms: 0, failed: &failed, agents: &[] });
    assert!(t.contains("cu has no live agent.") && t.contains("what failed: it failed: x"), "{t}");
}

/// try-diff-open (architect m_52963), code-viewer-width (m_57792): the
/// try item's `2 show the diff` opens the guided review of the feature's
/// branch in the client, and carries the feature's diff artifact for a
/// client that can't (an older one); nothing sent (the row's act, read by
/// both clients; the wire: core_place_tests); no diff kept, or another
/// kind: no act.
#[test]
fn the_try_items_show_the_diff_opens_its_branchs_review() {
    let act = bise_proto::rows::OptAct::Open {
        url: None,
        artifact: Some("cu-diff".into()),
        at: None,
        review: Some(bise_proto::diff::GuideTarget::Branch { name: "cu".into() }),
    };
    assert_eq!(try_acts(TRY, "cu", Some("cu-diff")), Some((2, act)));
    assert_eq!(try_acts(TRY, "cu", None), None, "no diff kept yet");
    assert_eq!(try_acts(MERGE, "cu", Some("cu-diff")), None, "the merge item's 2 is keep working");
    // its artifact: one per feature, the tip in its title
    let add = diff_artifact("cu", "abc1234", Path::new("/s/features/cu.diff"));
    assert_eq!((add.id.as_deref(), add.title.as_deref(), add.kind.as_deref()), (Some("cu-diff"), Some("cu at abc1234"), Some("code")));
}
