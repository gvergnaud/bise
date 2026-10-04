//! The dev flow in the agents' words (docs/dev-flow.md §2, §6, §7):
//! which flow a repo should use (detection), the Flow sections of main's
//! and the tasks' prompts, `/flow` and `sb flow`. The config itself (`[flow] mode, check, push`) and main's feed
//! lines for lands are `crate::flow`.
//!
//! Pure but for [`GitProbe`] (git and gh as processes): detection reads
//! the repo only through a [`Probe`], so the tests fake it.

use crate::flow::{FlowConfig, FlowMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a repo says about its flow (dev-flow §2's table, in its order).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "signal")]
pub enum Signal {
    /// No remote: a PR is impossible.
    NoRemote,
    /// The default branch is protected, or a ruleset requires a PR.
    Protected { branch: String },
    /// Someone else (not a bot) committed on the default branch in the
    /// last 90 days: their names, the most recent first.
    Others { branch: String, names: Vec<String> },
    /// The repo's AGENTS.md or CONTRIBUTING says to open a PR.
    Asked { file: String },
    /// None of the above: you alone.
    Alone { branch: String },
}

/// The detection's verdict: the signal, and the default branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detected {
    #[serde(flatten)]
    pub signal: Signal,
    /// The default branch (`main` when the remote does not say).
    pub base: String,
    /// The remote's URL the detection ran for (a change runs it again).
    #[serde(default)]
    pub remote: String,
}

impl Detected {
    pub fn mode(&self) -> FlowMode {
        match self.signal {
            Signal::NoRemote | Signal::Alone { .. } => FlowMode::Trunk,
            _ => FlowMode::Pr,
        }
    }

    /// A protected branch forces PRs: no question.
    pub fn forced(&self) -> bool {
        matches!(self.signal, Signal::Protected { .. })
    }

    /// Why, in one sentence (the question's line, `/flow`).
    pub fn why(&self) -> String {
        match &self.signal {
            Signal::NoRemote => "this repo has no remote, so a PR is impossible.".into(),
            Signal::Protected { branch } => {
                format!("{branch} is protected here: a push to it would fail.")
            }
            Signal::Others { branch, names } => {
                let who = match names.as_slice() {
                    [] => "someone else".to_string(),
                    [a] => a.clone(),
                    [a, b] => format!("{a} and {b}"),
                    [a, rest @ ..] => format!("{a} and {} others", rest.len()),
                };
                format!("{who} committed on {branch} in the last 90 days.")
            }
            Signal::Asked { file } => format!("the repo's {file} says to open a PR."),
            Signal::Alone { branch } => {
                format!("only you committed on {branch} in the last 90 days.")
            }
        }
    }
}

/// How the repo is read: a program's stdout (None when it fails), a
/// file of the repo.
pub trait Probe {
    fn run(&self, prog: &str, args: &[&str]) -> Option<String>;
    fn read(&self, rel: &str) -> Option<String>;
}

/// The real probe: git and gh in `repo`. gh reads its own token; bise
/// never sees it.
pub struct GitProbe {
    pub repo: PathBuf,
}

impl Probe for GitProbe {
    fn run(&self, prog: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new(prog)
            .args(args)
            .current_dir(&self.repo)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .env("GH_PROMPT_DISABLED", "1")
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.repo.join(rel)).ok()
    }
}

/// `owner/repo` of a GitHub remote URL (https, ssh, scp-like).
pub fn github_slug(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = u
        .strip_prefix("git@github.com:")
        .or_else(|| u.split_once("github.com/").map(|(_, r)| r))?;
    let slug = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut it = slug.split('/');
    let (o, r) = (it.next()?, it.next()?);
    (!o.is_empty() && !r.is_empty() && it.next().is_none()).then(|| format!("{o}/{r}"))
}

/// A bot's commit: GitHub's `[bot]` accounts, dependabot and friends.
fn bot(name: &str, email: &str) -> bool {
    let (n, e) = (name.to_ascii_lowercase(), email.to_ascii_lowercase());
    n.ends_with("[bot]") || e.contains("[bot]") || n.ends_with("-bot") || n == "github-actions"
}

/// Does this guide ask for PRs ("open a PR", "pull request")?
fn asks_for_prs(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    ["pull request", "open a pr", "a pr for", "send a pr", "merge request"]
        .iter()
        .any(|k| t.contains(k))
}

/// dev-flow §2: the flow a repo should use, from its remote, its rules,
/// who commits there and its guides. Runs git, and gh for a GitHub
/// remote (best effort: gh missing or offline is no protection).
pub fn detect(p: &dyn Probe) -> Detected {
    let remotes = p.run("git", &["remote"]).unwrap_or_default();
    let remote = if remotes.lines().any(|r| r.trim() == "origin") {
        "origin".to_string()
    } else {
        match remotes.lines().map(str::trim).find(|r| !r.is_empty()) {
            Some(r) => r.to_string(),
            None => {
                return Detected { signal: Signal::NoRemote, base: "main".into(), remote: String::new() }
            }
        }
    };
    let url = p
        .run("git", &["remote", "get-url", &remote])
        .unwrap_or_default()
        .trim()
        .to_string();
    let head = format!("refs/remotes/{remote}/HEAD");
    let base = p
        .run("git", &["symbolic-ref", "--short", &head])
        .and_then(|s| s.trim().split_once('/').map(|(_, b)| b.to_string()))
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "main".into());
    let done = |signal| Detected { signal, base: base.clone(), remote: url.clone() };
    if let Some(slug) = github_slug(&url) {
        let protected = p
            .run("gh", &["api", &format!("repos/{slug}/branches/{base}"), "--jq", ".protected"])
            .is_some_and(|s| s.trim() == "true");
        let ruled = p
            .run("gh", &["api", &format!("repos/{slug}/rules/branches/{base}"), "--jq", ".[].type"])
            .is_some_and(|s| s.lines().any(|t| t.trim() == "pull_request"));
        if protected || ruled {
            return done(Signal::Protected { branch: base.clone() });
        }
    }
    let me = p
        .run("git", &["config", "user.email"])
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let log = p
        .run(
            "git",
            &["log", "--since=90.days", "--format=%ae%x09%an", &format!("{remote}/{base}")],
        )
        .unwrap_or_default();
    let mut emails: Vec<String> = vec![];
    let mut names: Vec<String> = vec![];
    for l in log.lines() {
        let Some((e, n)) = l.split_once('\t') else { continue };
        let e = e.trim().to_ascii_lowercase();
        if e.is_empty() || e == me || bot(n, &e) || emails.contains(&e) {
            continue;
        }
        emails.push(e);
        names.push(n.trim().to_string());
    }
    if !names.is_empty() {
        return done(Signal::Others { branch: base.clone(), names });
    }
    for f in ["AGENTS.md", "CONTRIBUTING.md", ".github/CONTRIBUTING.md"] {
        if p.read(f).is_some_and(|t| asks_for_prs(&t)) {
            let file = f.rsplit('/').next().unwrap_or(f).to_string();
            return done(Signal::Asked { file });
        }
    }
    done(Signal::Alone { branch: base.clone() })
}

/// Where the flow comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `[flow] mode` in the repo's config (asked once, or `/flow`).
    Saved,
    /// The branch is protected: PRs whatever the config says.
    Forced,
    /// Detected, not asked yet: the work goes on (nothing pushed) and
    /// main asks once, in a card that blocks nothing.
    Suggested,
    /// No remote, nothing saved: trunk is the only flow, no question.
    Only,
}

/// The flow as the prompts, the approvals and `/flow` read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    pub mode: FlowMode,
    pub source: Source,
    /// The signal's sentence, when detected.
    pub why: Option<String>,
    /// The default branch.
    pub base: String,
    /// `[flow] check`: the command every commit passes.
    pub check: Option<String>,
    /// `[flow] push` (trunk flow: push main after every land).
    pub push: bool,
}

/// The flow from the config and the last detection; None: nothing saved
/// and nothing detected yet.
pub fn resolve(cfg: &FlowConfig, det: Option<&Detected>) -> Option<Flow> {
    let base = det.map(|d| d.base.clone()).unwrap_or_else(|| "main".into());
    let why = det.map(Detected::why);
    let (mode, source) = match (det, cfg.mode) {
        (Some(d), _) if d.forced() => (FlowMode::Pr, Source::Forced),
        (_, Some(m)) => (m, Source::Saved),
        (Some(d), None) if d.signal == Signal::NoRemote => (FlowMode::Trunk, Source::Only),
        (Some(d), None) => (d.mode(), Source::Suggested),
        (None, None) => return None,
    };
    Some(Flow { mode, source, why, base, check: cfg.check.clone(), push: cfg.push })
}

/// The flow in the header's words (dev-flow §7: `lands via PRs` /
/// `lands on main`).
pub fn lands(f: &Flow) -> String {
    match f.mode {
        FlowMode::Pr => "lands via PRs".into(),
        FlowMode::Trunk => format!("lands on {}", f.base),
    }
}

/// The 2 options of the flow question, the suggested one marked.
fn options(suggested: FlowMode) -> String {
    let mark = |m| if m == suggested { "  ← suggested" } else { "" };
    format!(
        "1 a PR per task (you merge){}\n2 straight to main, tested commits{}",
        mark(FlowMode::Pr),
        mark(FlowMode::Trunk)
    )
}

/// The one-time question (dev-flow §2), as main puts it in the user's
/// inbox with `sb card`.
pub fn question(f: &Flow) -> String {
    format!(
        "how should agents ship code here? {}\n{}",
        f.why.clone().unwrap_or_default(),
        options(f.mode)
    )
}

/// `/flow` and `sb flow` with no argument: the flow, why, how to switch.
pub fn show(f: Option<&Flow>) -> String {
    let Some(f) = f else {
        return "flow: not known yet (checking the repo). `/flow pr` or `/flow trunk` sets it.".into();
    };
    let what = match f.mode {
        FlowMode::Pr => "a PR per task (you merge)".to_string(),
        FlowMode::Trunk => format!(
            "straight to {}, tested commits{}",
            f.base,
            if f.push { ", pushed after every land" } else { ", kept local (push = false)" }
        ),
    };
    let why = f.why.clone().map(|w| format!(" {w}")).unwrap_or_default();
    let tail = match f.source {
        Source::Saved => format!(
            " saved in .switchboard/config.toml; `/flow {}` switches.",
            other_word(f.mode)
        ),
        Source::Forced => " no other choice while it is protected.".to_string(),
        Source::Suggested => format!(
            " suggested, not saved: nothing is pushed until it is; `/flow {}` or `/flow {}` saves it.",
            f.mode.as_str(),
            other_word(f.mode)
        ),
        Source::Only => " the only flow without a remote: nothing to push, nothing to ask.".to_string(),
    };
    let check = f
        .check
        .as_ref()
        .map(|c| format!(" check: `{c}`."))
        .unwrap_or_default();
    format!("flow: {} · {what}.{why}{tail}{check}", lands(f))
}

fn other_word(m: FlowMode) -> &'static str {
    match m {
        FlowMode::Pr => "trunk",
        FlowMode::Trunk => "pr",
    }
}

/// `/flow pr|trunk` (also `1|2`, the question's options): the mode, or
/// the usage.
pub fn parse_mode(s: &str) -> Result<FlowMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "pr" | "prs" | "1" => Ok(FlowMode::Pr),
        "trunk" | "main" | "2" => Ok(FlowMode::Trunk),
        _ => Err("usage: /flow [pr|trunk]".into()),
    }
}

/// A switch, checked against the repo: trunk is refused while the branch
/// is protected. Ok: the line the user (or main) reads.
pub fn switch(f: Option<&Flow>, to: FlowMode) -> Result<String, String> {
    if let Some(f) = f.filter(|f| f.source == Source::Forced && to == FlowMode::Trunk) {
        return Err(format!(
            "{} stays a PR flow: {}",
            f.base,
            f.why.clone().unwrap_or_default()
        ));
    }
    Ok(match to {
        FlowMode::Pr => "flow saved: lands via PRs. agents open a PR per task; you merge.".into(),
        FlowMode::Trunk => "flow saved: lands on main. agents land tested commits with `sb land`.".into(),
    })
}

// ---- the prompts (dev-flow §6) ----

/// The commit message style, from the last commit subjects (newest
/// first): long and detailed, short, or Conventional Commits.
pub fn commit_style(subjects: &[&str]) -> Option<String> {
    let s: Vec<&str> = subjects.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if s.len() < 5 {
        return None;
    }
    let conventional = s
        .iter()
        .filter(|l| {
            let head = l.split(": ").next().unwrap_or("");
            l.contains(": ")
                && head.len() <= 20
                && head
                    .trim_end_matches('!')
                    .split('(')
                    .next()
                    .is_some_and(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase()))
        })
        .count();
    if conventional * 2 > s.len() {
        return Some("Conventional Commits subjects (`fix(scope): …`, `feat: …`), like the last commits".into());
    }
    let mut lens: Vec<usize> = s.iter().map(|l| l.chars().count()).collect();
    lens.sort_unstable();
    let median = lens[lens.len() / 2];
    Some(if median > 100 {
        format!("long, detailed subject lines that say what changed and why (the last commits: about {median} characters)")
    } else {
        format!("short subject lines (the last commits: about {median} characters), details in the body")
    })
}

/// The check line: run it before every commit and land.
fn check_line(f: &Flow) -> String {
    match &f.check {
        Some(c) => format!("`{c}`"),
        None => "the repo's tests".into(),
    }
}

/// The Flow section of main's prompt (dev-flow §6 "Main's prompt");
/// None: the flow is not known yet.
pub fn main_section(f: Option<&Flow>, style: Option<&str>) -> String {
    let places = "- Places: you decide where each task works: the shared folder, a new worktree, or the worktree of agents already on that change (`sb spawn <name> --place new|<agent>`). Isolate when two agents would edit the same files, when a build must not see another's half-done edits, or when the change will be reviewed or thrown away on its own. Share a place when the work belongs in the same PR or needs the other agent's code now. Read-only work (investigate, review, answer, plan) stays in the shared folder. Say it in your routing line: `dark-mode takes a worktree; i18n joins it`.";
    let words = "- The user's words win for one task: \"open a PR\", \"just commit it\". The user may switch the repo's flow with `/flow`: `sb flow` says the current one.";
    let Some(f) = f else {
        return format!(
            "## Flow\n\n{places}\n- How this repo ships code is not known yet (`sb flow` says it once it is). Never hold work for it: until then `sb land` commits locally, nothing is pushed.\n- Never push or merge unless the user asks.\n{words}"
        );
    };
    let mode = match f.mode {
        FlowMode::Pr => format!(
            "- This repo ships through pull requests (base `{b}`). A task that changes code (docs and config too) ends in a PR: give it a new place (`--place new`: a branch `sb/<name>` from `origin/{b}`) or the place of the PR it belongs to (`--place <agent>`); its done-when says \"its PR is open\" or \"your part is on the branch\". One PR or one per phase: the user chooses; ask when they haven't said. Read-only tasks: no branch.\n- You never merge; the user does (the inbox asks them when a PR is approved with checks passing). GitHub's news about a PR go to the agents of its branch; you get a copy: escalate only product calls and checks still failing after 2 tries.\n- Never push or merge yourself: the PR's own branch is pushed by the hub. A \"just commit it\" from the user: {just}",
            b = f.base,
            just = if f.source == Source::Forced {
                format!("refused, `{}` is protected (say so)", f.base)
            } else {
                format!("ask once (\"{} is shared here, sure?\")", f.base)
            }
        ),
        FlowMode::Trunk => format!(
            "- This repo ships straight to `{b}`, through `sb land`: small tested commits, one land at a time. A small change works in the shared folder when nobody else edits those files; a bigger one in a worktree, landed when its work is done.{push}\n- Most work lands on `{b}`. Put a task on a **feature branch** when it's experimental, risky for a release (core loop, hub, security, sandbox, packaging, a migration), big (several agents or more than a day), or the user asks (\"in a branch\", \"I want to try it first\"); during a launch freeze, everything does. When unsure, ask once (`computer-use straight on {b}, or on a branch you try first?`). The user's words win both ways (\"just land it\").\n- `sb feature new <name>` (a local branch from `{b}`'s tip, never pushed; an existing local branch of that name is adopted as it is), then spawn its agents with `sb spawn <agent> --feature <name>`: each gets its own worktree and lands on that branch. Say it in your routing line: `computer-use goes on its own branch: you'll try it before it reaches main.`\n- When its agents are done, `sb feature ready <name>` opens the try item in the user's inbox (it builds the branch on their go, then asks whether to merge). Never merge a feature without the user's go; `sb feature merge <name>` only after it (\"merge computer-use\"), `sb feature drop <name>` only on their word. When the branch is far behind `{b}`, or before a try: `sb feature sync <name>`. `sb feature` lists them.\n- Never push or merge yourself: `sb land` does what the flow needs. \"open a PR\" from the user: that task gets a branch and a PR.",
            b = f.base,
            push = if f.push { format!(" The hub pushes `{}` after every land.", f.base) } else { " Lands stay local (`push = false`) until the user asks.".into() }
        ),
    };
    let ask = match f.source {
        Source::Suggested => format!(
            "\n- The flow above is only suggested ({why}), not saved, and never holds work: {until} At the {when}, ask the user once in a card that blocks nothing, `sb card \"{q}\"`, save the answer with `sb flow pr|trunk`{then}, and never ask again.",
            why = f.why.clone().unwrap_or_default().trim_end_matches('.'),
            until = match f.mode {
                FlowMode::Trunk => "lands stay local (nothing pushed) until it is saved.",
                FlowMode::Pr => "until it is saved, a task that changes code takes `--place new` and commits on its branch with `sb land --here` (nothing pushed; a task dropped before the answer keeps its commits for `sb restore`).",
            },
            when = match f.mode {
                FlowMode::Trunk => "first land",
                FlowMode::Pr => "first such task",
            },
            then = match f.mode {
                FlowMode::Trunk => "",
                FlowMode::Pr => ", then tell those tasks to `sb land` (pr opens their PR, trunk moves the base)",
            },
            q = question(f).replace('\n', "\\n").replace('"', "\\\"")
        ),
        Source::Forced => format!(
            "\n- The first time a task changes code, tell the user in one line: \"{} is protected here: every agent opens a PR\".",
            f.base
        ),
        Source::Saved | Source::Only => String::new(),
    };
    let check = format!(
        "\n- Every commit passes {} before it lands or is pushed: your briefs need not repeat the git rules (private index, check, push); the tasks' prompts carry them.",
        check_line(f)
    );
    let style = style
        .map(|s| format!("\n- Commit messages: {s}."))
        .unwrap_or_default();
    format!("## Flow\n\n{places}\n{mode}{ask}{check}{style}\n{words}")
}

/// A task's place, as its prompt names it.
pub struct TaskPlace<'a> {
    /// The folder it works in.
    pub path: &'a str,
    /// Its branch, in a worktree.
    pub branch: Option<&'a str>,
    /// The other agents of its place (a worktree's; a feature's agents).
    pub others: &'a [String],
    /// dev-flow §5.1: the feature it lands on, in its own worktree.
    pub feature: Option<&'a str>,
}

/// A task's working-directory line by flow and place (dev-flow §6 "A
/// task's prompt"); `f` None: the flow is not known, today's lines.
pub fn task_place(f: Option<&Flow>, p: &TaskPlace, style: Option<&str>) -> String {
    let shared_by = match p.others {
        [] => "yours alone for now; others may join".to_string(),
        o => format!(
            "shared with {}",
            o.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ")
        ),
    };
    let style = style
        .map(|s| format!(" Commit messages: {s}."))
        .unwrap_or_default();
    // dev-flow §5.1: a feature's agent, whatever the flow says of main
    if let (Some(feat), Some(b)) = (p.feature, p.branch) {
        let base = f.map(|f| f.base.as_str()).unwrap_or("main");
        let with = match p.others {
            [] => String::new(),
            o => format!(
                " Its other agents: {}.",
                o.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ")
            ),
        };
        let check = f.map(check_line).unwrap_or_else(|| "the repo's tests".into());
        return format!(
            "`{path}` — a git worktree on branch `{b}`, yours. You work for the feature `{feat}`, a local branch the user will try before it reaches `{base}`.{with} Commit your files with `sb land --here \"<message>\"`; run {check} first. `sb land` puts your commits on `{feat}` (rebased on its tip), never on `{base}`. Never merge it, never push it.{style}",
            path = p.path
        );
    }
    match (f, p.branch) {
        (None, Some(b)) => format!(
            "`{}` — an isolated git worktree on branch `{b}`. Work only there. You may commit on your branch; never push unless the user asks.",
            p.path
        ),
        (None, None) => format!(
            "`{}` — the shared workspace (the user and other tasks work there too). Do not revert changes you did not make.",
            p.path
        ),
        (Some(f), Some(b)) => match f.mode {
            // the PR flow is only suggested: commits on the branch, no PR
            // and no push until main says the user's answer
            FlowMode::Pr if f.source == Source::Suggested => format!(
                "`{path}` — a git worktree on branch `{b}`, {shared_by}. Work only there. Commit your files on the branch with `sb land --here \"<message>\"`; run {check} first. How this repo ships is not chosen yet: no PR, no push; main tells you when to `sb land`.{style}",
                path = p.path,
                check = check_line(f)
            ),
            FlowMode::Pr => format!(
                "`{path}` — a git worktree on branch `{b}` from `origin/{base}`, {shared_by}. Work only there. Commit only your files, with `sb land --here \"<message>\"`; run {check} first. Open the PR with `gh pr create` (the repo's template if it has one) if nobody has; the hub pushes the branch. You stay until it is merged: fix the reviews and red checks sent to you, rebase when it conflicts (`git push --force-with-lease`, this branch only, after telling the branch's other agents). Never merge, approve, close, or write on GitHub.{style}",
                path = p.path,
                base = f.base,
                check = check_line(f)
            ),
            FlowMode::Trunk => format!(
                "`{path}` — a git worktree on branch `{b}`, {shared_by}. Work only there. Commit your files on the branch with `sb land --here \"<message>\"`; run {check} first. When the place's work is done, `sb land` rebases it on `{base}` and moves `{base}`. Never push.{style}",
                path = p.path,
                base = f.base,
                check = check_line(f)
            ),
        },
        (Some(f), None) => match f.mode {
            FlowMode::Pr => format!(
                "`{path}` — the shared workspace (the user and other tasks work there too). Do not revert changes you did not make. This repo ships through PRs, so commit nothing here: if your task turns out to need a code change, ask main for a place of its own (`sb send main`).",
                path = p.path
            ),
            FlowMode::Trunk => format!(
                "`{path}` — the shared workspace (the user and other tasks work there too). Do not revert changes you did not make. Commit nothing by hand: run {check}, then `sb land \"<message>\"` (it commits only your files, onto `{base}`). Never `git add -A`, stash, reset, rebase or amend here.{style}",
                path = p.path,
                base = f.base,
                check = check_line(f)
            ),
        },
    }
}

/// The brief's done-when tail for a task that changes code (dev-flow §6
/// "The brief").
pub fn done_when_tail(f: &Flow) -> String {
    match f.mode {
        FlowMode::Pr => "its PR is open".into(),
        FlowMode::Trunk => format!("landed on {}", f.base),
    }
}

/// The detection's cache file, next to the hub's state.
pub fn cache_file(state: &Path) -> PathBuf {
    state.join("flow-detect.json")
}

pub fn read_cache(state: &Path) -> Option<Detected> {
    serde_json::from_str(&std::fs::read_to_string(cache_file(state)).ok()?).ok()
}

pub fn write_cache(state: &Path, d: &Detected) {
    if let Ok(s) = serde_json::to_string(d) {
        let tmp = cache_file(state).with_extension("json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, cache_file(state));
        }
    }
}

#[cfg(test)]
#[path = "devflow_tests.rs"]
mod tests;
