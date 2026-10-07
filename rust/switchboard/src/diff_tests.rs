//! Laws of the diff side (docs/artifacts.md, "diffs"): the parse of
//! `git diff`, and a real throwaway repo for an agent's changes, a
//! branch, a land's range and the branch list.

use super::*;
use std::path::PathBuf;

const SAMPLE: &str = "diff --git a/src/pricing.tsx b/src/pricing.tsx
index 1111111..2222222 100644
--- a/src/pricing.tsx
+++ b/src/pricing.tsx
@@ -38,4 +38,3 @@ export function Pricing() {
   return (
-    <Banner />
-    <section className=\"plans\">
+    <section className=\"plans\">
 
diff --git a/src/Banner.tsx b/src/Banner.tsx
deleted file mode 100644
index 3333333..0000000
--- a/src/Banner.tsx
+++ /dev/null
@@ -1,2 +0,0 @@
-export const Banner = () => null;
-// end
\\ No newline at end of file
diff --git a/old name.css b/new name.css
similarity 90%
rename from old name.css
rename to new name.css
index 4444444..5555555 100644
--- a/old name.css
+++ b/new name.css
@@ -1 +1 @@
-a{}
+b{}
diff --git a/logo.png b/logo.png
new file mode 100644
index 0000000..6666666
Binary files /dev/null and b/logo.png differ
diff --git a/Cargo.lock b/Cargo.lock
index 7777777..8888888 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1 +1,2 @@
 x
+y
";

#[test]
fn parse_reads_files_hunks_and_statuses() {
    let f = parse(SAMPLE);
    assert_eq!(f.len(), 5);
    let p = &f[0];
    assert_eq!((p.path.as_str(), p.status, p.add, p.del), ("src/pricing.tsx", 'M', 1, 2));
    assert_eq!(p.hunks[0].old, 38);
    assert_eq!(p.hunks[0].new, 38);
    assert_eq!(p.hunks[0].head, "export function Pricing() {");
    assert_eq!(p.hunks[0].lines, vec!["   return (", "-    <Banner />", "-    <section className=\"plans\">", "+    <section className=\"plans\">", " "]);
    assert_eq!((f[1].status, f[1].del, f[1].hunks[0].lines.len()), ('D', 2, 2));
    assert_eq!((f[2].status, f[2].path.as_str(), f[2].old_path.as_deref()), ('R', "new name.css", Some("old name.css")));
    assert!(f[3].binary && f[3].status == 'A' && image(&f[3].path));
    assert!(generated(&f[4].path) && !generated(&f[0].path));
    assert_eq!(stat(&f), (5, 3, 5));
    let j = file_json(&f[0], Some(Path::new("/w")));
    assert_eq!(j["abs"], "/w/src/pricing.tsx");
    assert_eq!(j["status"], "M");
    assert_eq!(j["hunks"][0]["lines"][1], "-    <Banner />");
}

#[test]
fn a_huge_file_is_cut() {
    let mut t = String::from("diff --git a/big.txt b/big.txt\n--- a/big.txt\n+++ b/big.txt\n@@ -0,0 +1,6000 @@\n");
    for i in 0..6000 {
        t.push_str(&format!("+line {}\n", i));
    }
    let f = parse(&t);
    assert!(f[0].cut);
    assert_eq!(f[0].hunks[0].lines.len(), MAX_FILE_LINES);
    assert_eq!(f[0].add, 6000, "the counts stay whole");
}

#[test]
fn numstat_sums_and_skips_binary() {
    assert_eq!(numstat("4\t6\ta.rs\n-\t-\tlogo.png\n10\t0\tb.rs\n"), (14, 6));
}

fn sh(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(ok.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&ok.stderr));
}

fn repo(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-diff-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    sh(&d, &["init", "-q", "-b", "main"]);
    std::fs::write(d.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    sh(&d, &["add", "."]);
    sh(&d, &["commit", "-q", "-m", "base"]);
    d
}

#[test]
fn a_checkout_shows_commits_uncommitted_and_untracked_together() {
    let d = repo("checkout");
    sh(&d, &["checkout", "-q", "-b", "sb/x"]);
    std::fs::write(d.join("a.txt"), "one\n2\nthree\n").unwrap();
    sh(&d, &["commit", "-q", "-am", "c1"]);
    std::fs::write(d.join("a.txt"), "one\n2\nthree\nfour\n").unwrap();
    std::fs::write(d.join("new.md"), "hello\n").unwrap();
    assert_eq!(trunk(&d), "main");
    let (files, commits, uncommitted) = checkout(&d, "main").unwrap();
    assert_eq!(commits, 1);
    assert!(uncommitted);
    let a = files.iter().find(|f| f.path == "a.txt").unwrap();
    assert_eq!((a.add, a.del), (2, 1));
    let n = files.iter().find(|f| f.path == "new.md").unwrap();
    assert_eq!((n.status, n.add), ('A', 1));
    // the same branch, from elsewhere: committed only
    let (b, c) = branch(&d, "main", "sb/x").unwrap();
    assert_eq!((b.len(), c), (1, 1));
    let rows = branches(&d, "main");
    assert_eq!(rows, vec![("sb/x".to_string(), 1, 1, 1)]);
    // a shared-folder agent's own files only
    let own = own_files(&d, &["new.md".to_string()]).unwrap();
    assert_eq!(own.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["new.md"]);
}

#[test]
fn a_land_range_has_its_stat_and_a_bad_range_is_refused() {
    let d = repo("range");
    let from = String::from_utf8(std::process::Command::new("git").arg("-C").arg(&d).args(["rev-parse", "--short", "HEAD"]).output().unwrap().stdout).unwrap();
    let from = from.trim();
    std::fs::write(d.join("b.txt"), "x\ny\n").unwrap();
    sh(&d, &["add", "."]);
    sh(&d, &["commit", "-q", "-m", "c2"]);
    assert_eq!(range_stat(&d, from, "HEAD"), (1, 2, 0));
    let (files, commits) = range(&d, &format!("{}..HEAD", from)).unwrap();
    assert_eq!((files.len(), commits), (1, 1));
    assert!(range(&d, "a; rm -rf /..b").is_err());
    assert!(range(&d, "--output=x..y").is_err());
    assert!(range(&d, "x^^..y").is_err() && range(&d, "^..y").is_err());
}

/// T1 run 6 step 9 (amb-tools m_9325): one commit is `<sha>^..<sha>`
/// (the window's "merged · 7f5be63"); a feature's merge commit shows the
/// feature's whole change against main's first parent.
#[test]
fn one_commit_and_a_merge_commit_are_ranges() {
    let d = repo("merge");
    let head = |d: &Path| String::from_utf8(std::process::Command::new("git").arg("-C").arg(d).args(["rev-parse", "HEAD"]).output().unwrap().stdout).unwrap().trim().to_string();
    // the root commit: against the empty tree
    let root = head(&d);
    let (files, commits) = range(&d, &format!("{root}^..{root}")).unwrap();
    assert_eq!((files.iter().map(|f| (f.path.as_str(), f.status)).collect::<Vec<_>>(), commits), (vec![("a.txt", 'A')], 1));
    sh(&d, &["checkout", "-q", "-b", "feat"]);
    std::fs::write(d.join("f.txt"), "f\n").unwrap();
    sh(&d, &["add", "."]);
    sh(&d, &["commit", "-q", "-m", "f1"]);
    std::fs::write(d.join("g.txt"), "g\n").unwrap();
    sh(&d, &["add", "."]);
    sh(&d, &["commit", "-q", "-m", "f2"]);
    sh(&d, &["checkout", "-q", "main"]);
    std::fs::write(d.join("m.txt"), "m\n").unwrap();
    sh(&d, &["add", "."]);
    sh(&d, &["commit", "-q", "-m", "on main"]);
    let plain = head(&d);
    let (files, commits) = range(&d, &format!("{plain}^..{plain}")).unwrap();
    assert_eq!((files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), commits), (vec!["m.txt"], 1));
    sh(&d, &["merge", "-q", "--no-ff", "-m", "merge feat", "feat"]);
    let merge = head(&d);
    let (files, commits) = range(&d, &format!("{merge}^..{merge}")).unwrap();
    let mut paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
    paths.sort();
    assert_eq!((paths, commits), (vec!["f.txt", "g.txt"], 3), "the feature against main's first parent");
}
