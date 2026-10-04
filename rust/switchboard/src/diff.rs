//! Diffs for the TUI's right panel (docs/artifacts.md, "diffs"): an
//! agent's changes or any branch against main, a landed range, a PR.
//! `parse` turns `git diff` text into files and hunks (pure, tested);
//! the rest runs git (or gh), off the hub's loop.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// A file's lines past this are cut (`"cut": true`).
pub const MAX_FILE_LINES: usize = 5000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hunk {
    pub old: u64,
    pub new: u64,
    /// The text after the second `@@` (the function git found).
    pub head: String,
    /// `" ctx"`, `"-old"`, `"+new"`.
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    pub path: String,
    pub old_path: Option<String>,
    /// M | A | D | R
    pub status: char,
    pub add: u64,
    pub del: u64,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    pub cut: bool,
}

impl File {
    fn lines(&self) -> usize {
        self.hunks.iter().map(|h| h.lines.len()).sum()
    }
}

/// Lock files and generated files: the TUI folds them by themselves.
pub fn generated(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "Cargo.lock" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "bun.lockb" | "poetry.lock"
            | "uv.lock" | "Gemfile.lock" | "composer.lock" | "go.sum" | "flake.lock" | "Podfile.lock"
    ) || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".map")
        || name.ends_with(".snap")
        || path.split('/').any(|d| matches!(d, "dist" | "generated" | "__generated__" | "vendor"))
}

pub fn image(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "avif" | "heic")
}

/// `a/x b/y` of a `diff --git` line (quoted paths unquoted).
fn git_paths(rest: &str) -> (String, String) {
    let unq = |s: &str| s.trim_matches('"').to_string();
    if let Some(i) = rest.find(" b/") {
        let a = rest[..i].trim_start_matches("\"").trim_start_matches("a/");
        let b = &rest[i + 3..];
        return (unq(a), unq(b));
    }
    let mut it = rest.split_whitespace();
    let a = it.next().unwrap_or("").trim_start_matches("a/").to_string();
    let b = it.next().unwrap_or("").trim_start_matches("b/").to_string();
    (unq(&a), unq(&b))
}

fn hunk_start(s: &str) -> u64 {
    s.trim_start_matches(['-', '+']).split(',').next().unwrap_or("0").parse().unwrap_or(0)
}

/// Parse `git diff` (unified, `--no-color`) text.
pub fn parse(text: &str) -> Vec<File> {
    let mut files: Vec<File> = Vec::new();
    let mut cur: Option<File> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.extend(cur.take());
            let (a, b) = git_paths(rest);
            cur = Some(File {
                path: b.clone(),
                old_path: (a != b).then_some(a),
                status: 'M',
                ..Default::default()
            });
            continue;
        }
        let Some(f) = cur.as_mut() else { continue };
        if f.hunks.is_empty() {
            if line.starts_with("new file mode") {
                f.status = 'A';
                continue;
            }
            if line.starts_with("deleted file mode") {
                f.status = 'D';
                continue;
            }
            if let Some(p) = line.strip_prefix("rename from ") {
                f.old_path = Some(p.to_string());
                f.status = 'R';
                continue;
            }
            if let Some(p) = line.strip_prefix("rename to ") {
                f.path = p.to_string();
                f.status = 'R';
                continue;
            }
            if line.starts_with("Binary files ") || line == "GIT binary patch" {
                f.binary = true;
                continue;
            }
            if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with("index ") || line.starts_with("similarity ") || line.starts_with("old mode") || line.starts_with("new mode") {
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("@@ ") {
            let mut parts = rest.splitn(3, ' ');
            let old = hunk_start(parts.next().unwrap_or(""));
            let new = hunk_start(parts.next().unwrap_or(""));
            let head = parts.next().unwrap_or("").trim_start_matches("@@").trim().to_string();
            f.hunks.push(Hunk { old, new, head, lines: Vec::new() });
            continue;
        }
        if f.hunks.is_empty() {
            continue;
        }
        match line.chars().next() {
            Some('+') => f.add += 1,
            Some('-') => f.del += 1,
            Some(' ') => {}
            // `\ No newline at end of file`, or an empty context line
            Some('\\') => continue,
            None => {}
            _ => continue,
        }
        if f.lines() >= MAX_FILE_LINES {
            f.cut = true;
            continue;
        }
        if let Some(h) = f.hunks.last_mut() {
            h.lines.push(if line.is_empty() { " ".to_string() } else { line.to_string() });
        }
    }
    files.extend(cur);
    files
}

/// A file of the `diff` event; `root` makes `abs` (⏎ opens it there).
pub fn file_json(f: &File, root: Option<&Path>) -> Value {
    json!({
        "path": f.path, "old_path": f.old_path, "status": f.status.to_string(),
        "add": f.add, "del": f.del, "binary": f.binary, "image": image(&f.path),
        "generated": generated(&f.path), "cut": f.cut,
        "abs": root.map(|r| r.join(&f.path).to_string_lossy().to_string()),
        "hunks": f.hunks.iter().map(|h| json!({"old": h.old, "new": h.new, "head": h.head, "lines": h.lines})).collect::<Vec<_>>(),
    })
}

/// files, + and − of a diff.
pub fn stat(files: &[File]) -> (usize, u64, u64) {
    (files.len(), files.iter().map(|f| f.add).sum(), files.iter().map(|f| f.del).sum())
}

/// git's error in one readable line: its first line without `fatal:` or
/// `error:`, after `git couldn't read this diff:` (designer m_7393; the
/// TUI adds the ▲).
pub fn readable(stderr: &str) -> String {
    let line = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("it failed");
    let line = ["fatal:", "error:"].iter().find_map(|p| line.strip_prefix(p)).unwrap_or(line).trim();
    format!("git couldn't read this diff: {}", line)
}

/// Whether `br` names a commit in the repo at `dir` (a branch, a tag).
pub fn has_ref(dir: &Path, br: &str) -> bool {
    !br.is_empty() && !br.starts_with('-') && git(dir, &["rev-parse", "--verify", "-q", &format!("{}^{{commit}}", br)]).is_ok()
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    // never in a folder that is gone (a removed worktree): git would
    // say `fatal: cannot change to …`
    if !dir.is_dir() {
        return Err(format!("git couldn't read this diff: {} is gone", dir.display()));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(readable(&String::from_utf8_lossy(&out.stderr)))
    }
}

const DIFF_ARGS: &[&str] = &["-c", "core.quotepath=off", "diff", "--no-color", "--no-ext-diff", "-M", "--src-prefix=a/", "--dst-prefix=b/"];

fn diff_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut v: Vec<&str> = DIFF_ARGS.to_vec();
    v.extend_from_slice(extra);
    v
}

/// The untracked files of a checkout as added files (`git diff
/// --no-index /dev/null <f>`, which exits 1 when they differ).
fn untracked(dir: &Path, only: Option<&[String]>) -> Vec<File> {
    let list = git(dir, &["ls-files", "--others", "--exclude-standard", "-z"]).unwrap_or_default();
    let mut out = Vec::new();
    for f in list.split('\0').filter(|f| !f.is_empty()) {
        if only.is_some_and(|o| !o.iter().any(|x| x == f)) {
            continue;
        }
        let text = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(diff_args(&["--no-index", "--", "/dev/null", f]))
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        for mut file in parse(&text) {
            file.path = f.to_string();
            file.old_path = None;
            file.status = 'A';
            out.push(file);
        }
        if out.len() > 500 {
            break;
        }
    }
    out
}

/// The base a branch is compared to: `main` (or `master`) of the repo.
pub fn trunk(dir: &Path) -> String {
    for b in ["main", "master"] {
        if git(dir, &["rev-parse", "--verify", "-q", &format!("refs/heads/{}", b)]).is_ok() {
            return b.to_string();
        }
    }
    "HEAD".into()
}

/// What a checkout changed against `base`: its commits since the
/// merge-base and what is not committed yet, untracked files included.
/// (files, commits, uncommitted)
pub fn checkout(dir: &Path, base: &str) -> Result<(Vec<File>, usize, bool), String> {
    let mb = git(dir, &["merge-base", base, "HEAD"])?.trim().to_string();
    let mut files = parse(&git(dir, &diff_args(&[&mb]))?);
    let ut = untracked(dir, None);
    let uncommitted = !ut.is_empty() || !git(dir, &["status", "--porcelain", "--untracked-files=no"])?.trim().is_empty();
    files.extend(ut);
    let commits = git(dir, &["rev-list", "--count", &format!("{}..HEAD", mb)])?.trim().parse().unwrap_or(0);
    Ok((files, commits, uncommitted))
}

/// A shared-folder agent's own files against HEAD.
pub fn own_files(dir: &Path, files: &[String]) -> Result<Vec<File>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = diff_args(&["HEAD", "--"]);
    args.extend(files.iter().map(String::as_str));
    let mut out = parse(&git(dir, &args)?);
    out.extend(untracked(dir, Some(files)));
    Ok(out)
}

/// A branch with no checkout of its own: `base...branch`.
pub fn branch(dir: &Path, base: &str, br: &str) -> Result<(Vec<File>, usize), String> {
    if !has_ref(dir, br) {
        return Err(format!("there's no branch named {}.", br));
    }
    let files = parse(&git(dir, &diff_args(&[&format!("{}...{}", base, br)]))?);
    let commits = git(dir, &["rev-list", "--count", &format!("{}..{}", base, br)])?.trim().parse().unwrap_or(0);
    Ok((files, commits))
}

/// A range `from..to` (a land).
pub fn range(dir: &Path, r: &str) -> Result<(Vec<File>, usize), String> {
    let ok = r.split_once("..").is_some_and(|(a, b)| {
        let w = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/-_.".contains(c));
        w(a) && w(b.trim_start_matches('.'))
    });
    if !ok {
        return Err(format!("not a range: {}", r));
    }
    let files = parse(&git(dir, &diff_args(&[r]))?);
    let commits = git(dir, &["rev-list", "--count", r])?.trim().parse().unwrap_or(0);
    Ok((files, commits))
}

/// A PR, through `gh pr diff <n>`.
pub fn pr(dir: &Path, n: u64) -> Result<Vec<File>, String> {
    let out = Command::new("gh")
        .current_dir(dir)
        .args(["pr", "diff", &n.to_string(), "--color", "never"])
        .output()
        .map_err(|e| format!("gh: {}", e))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        let why = why.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("it failed");
        return Err(format!("gh couldn't read PR #{}: {}", n, why));
    }
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// The local branches ahead of `base`, with their + and − and commits:
/// (branch, commits, add, del).
pub fn branches(dir: &Path, base: &str) -> Vec<(String, usize, u64, u64)> {
    let list = git(dir, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]).unwrap_or_default();
    let mut out = Vec::new();
    for b in list.lines().map(str::trim).filter(|b| !b.is_empty() && *b != base) {
        let commits: usize = git(dir, &["rev-list", "--count", &format!("{}..{}", base, b)])
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(0);
        if commits == 0 {
            continue;
        }
        let (add, del) = numstat(&git(dir, &["diff", "--numstat", &format!("{}...{}", base, b)]).unwrap_or_default());
        out.push((b.to_string(), commits, add, del));
    }
    out
}

/// The + and − of `git diff --numstat` text (binary rows count 0).
pub fn numstat(text: &str) -> (u64, u64) {
    text.lines().fold((0, 0), |(a, d), l| {
        let mut it = l.split('\t');
        let x = it.next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
        let y = it.next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
        (a + x, d + y)
    })
}

/// A land's facts for the `landed` line: (files, add, del) of `from..to`.
pub fn range_stat(dir: &Path, from: &str, to: &str) -> (usize, u64, u64) {
    let text = git(dir, &["diff", "--numstat", &format!("{}..{}", from, to)]).unwrap_or_default();
    let (a, d) = numstat(&text);
    (text.lines().filter(|l| !l.trim().is_empty()).count(), a, d)
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
