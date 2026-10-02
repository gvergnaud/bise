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
        since_ms: 0,
        flow: crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), ..Default::default() },
        onto: Some(format!("refs/heads/{}", onto)),
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
    let e = rebase(&ws, &scratch, "cu", Some("exit 3")).unwrap_err();
    assert!(e.contains("the check `exit 3` failed"), "{}", e);
    assert_eq!(out(&ws, &["rev-parse", "cu"]), tip);
    // merge: rebased, checked, main fast-forwarded (the shared folder's
    // files too), linear; the branch then goes to the trash
    let flow = crate::flow::FlowConfig { mode: Some(crate::flow::FlowMode::Trunk), check: Some("test -f n".into()), ..Default::default() };
    let m = merge(&ws, &scratch, "cu", &flow).unwrap();
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
    assert!(e.starts_with("a changed on main too"), "{}", e);
    assert_eq!(out(&ws, &["rev-parse", "cu"]), tip);
    let e = merge(&ws, &scratch, "cu", &crate::flow::FlowConfig::default()).unwrap_err();
    assert!(e.contains("conflicts"), "{}", e);
    assert_eq!(out(&ws, &["log", "-1", "--format=%s", "main"]), "main edits a");
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
