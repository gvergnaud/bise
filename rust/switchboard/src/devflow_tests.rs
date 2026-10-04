use super::*;
use std::collections::BTreeMap;

/// A repo as git and gh answer it: each command line (`prog args…`) to
/// its stdout; absent = the command fails. Files by path.
#[derive(Default)]
struct Fake {
    out: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
}

impl Fake {
    fn on(mut self, cmd: &str, out: &str) -> Fake {
        self.out.insert(cmd.to_string(), out.to_string());
        self
    }
    fn file(mut self, path: &str, text: &str) -> Fake {
        self.files.insert(path.to_string(), text.to_string());
        self
    }
    /// A GitHub repo `acme/app`, default branch main, me = me@x.
    fn github() -> Fake {
        Fake::default()
            .on("git remote", "origin\n")
            .on("git remote get-url origin", "git@github.com:acme/app.git\n")
            .on("git symbolic-ref --short refs/remotes/origin/HEAD", "origin/main\n")
            .on("git config user.email", "Me@x\n")
    }
}

impl Probe for Fake {
    fn run(&self, prog: &str, args: &[&str]) -> Option<String> {
        self.out.get(&format!("{} {}", prog, args.join(" "))).cloned()
    }
    fn read(&self, rel: &str) -> Option<String> {
        self.files.get(rel).cloned()
    }
}

const LOG: &str = "git log --since=90.days --format=%ae%x09%an origin/main";

#[test]
fn no_remote_is_trunk() {
    let d = detect(&Fake::default());
    assert_eq!(d.signal, Signal::NoRemote);
    assert_eq!((d.mode(), d.forced()), (FlowMode::Trunk, false));
    assert_eq!(d.why(), "this repo has no remote, so a PR is impossible.");
}

#[test]
fn a_protected_branch_forces_prs() {
    let f = Fake::github().on("gh api repos/acme/app/branches/main --jq .protected", "true\n");
    let d = detect(&f);
    assert_eq!(d.signal, Signal::Protected { branch: "main".into() });
    assert!(d.forced());
    // a ruleset that requires a PR, the branch itself unprotected
    let f = Fake::github()
        .on("gh api repos/acme/app/branches/main --jq .protected", "false\n")
        .on("gh api repos/acme/app/rules/branches/main --jq .[].type", "deletion\npull_request\n");
    assert!(detect(&f).forced());
    // gh missing or offline: no protection known, the next signal
    let f = Fake::github().on(LOG, "me@x\tMe\n");
    assert_eq!(detect(&f).signal, Signal::Alone { branch: "main".into() });
}

#[test]
fn other_committers_suggest_prs_bots_and_me_dont_count() {
    let log = "alice@x\tAlice\nme@x\tMe\nbob@x\tBob\nalice@x\tAlice\n49699333+dependabot[bot]@users.noreply.github.com\tdependabot[bot]\ncarol@x\tCarol\ndan@x\tDan\n";
    let d = detect(&Fake::github().on(LOG, log));
    assert_eq!(
        d.signal,
        Signal::Others { branch: "main".into(), names: vec!["Alice".into(), "Bob".into(), "Carol".into(), "Dan".into()] }
    );
    assert_eq!((d.mode(), d.forced()), (FlowMode::Pr, false));
    assert_eq!(d.why(), "Alice and 3 others committed on main in the last 90 days.");
    let d = detect(&Fake::github().on(LOG, "bob@x\tBob\n"));
    assert_eq!(d.why(), "Bob committed on main in the last 90 days.");
    // only bots: alone
    let d = detect(&Fake::github().on(LOG, "x[bot]@y\trenovate[bot]\n"));
    assert_eq!(d.mode(), FlowMode::Trunk);
}

#[test]
fn a_guide_that_asks_for_prs_suggests_them() {
    let f = Fake::github().file("AGENTS.md", "# Rules\nAlways open a PR for your change.\n");
    let d = detect(&f);
    assert_eq!(d.signal, Signal::Asked { file: "AGENTS.md".into() });
    assert_eq!(d.mode(), FlowMode::Pr);
    let f = Fake::github().file(".github/CONTRIBUTING.md", "Send a pull request against main.");
    assert_eq!(detect(&f).why(), "the repo's CONTRIBUTING.md says to open a PR.");
    let f = Fake::github().file("AGENTS.md", "Commit straight to main.");
    assert_eq!(detect(&f).signal, Signal::Alone { branch: "main".into() });
}

#[test]
fn the_default_branch_and_a_non_github_remote() {
    let f = Fake::default()
        .on("git remote", "upstream\n")
        .on("git remote get-url upstream", "https://gitlab.com/a/b.git")
        .on("git symbolic-ref --short refs/remotes/upstream/HEAD", "upstream/trunk\n")
        .on("git log --since=90.days --format=%ae%x09%an upstream/trunk", "zoe@x\tZoe\n");
    let d = detect(&f);
    assert_eq!(d.base, "trunk");
    assert_eq!(d.signal, Signal::Others { branch: "trunk".into(), names: vec!["Zoe".into()] });
    assert_eq!(github_slug("https://github.com/acme/app.git"), Some("acme/app".into()));
    assert_eq!(github_slug("ssh://git@github.com/acme/app"), Some("acme/app".into()));
    assert_eq!(github_slug("https://gitlab.com/a/b.git"), None);
}

#[test]
fn the_cache_round_trips() {
    let d = Detected { signal: Signal::Others { branch: "main".into(), names: vec!["A".into()] }, base: "main".into(), remote: "u".into() };
    let dir = std::env::temp_dir().join(format!("devflow-cache-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_cache(&dir, &d);
    assert_eq!(read_cache(&dir), Some(d));
    let _ = std::fs::remove_dir_all(&dir);
}

fn det(signal: Signal) -> Detected {
    Detected { signal, base: "main".into(), remote: String::new() }
}

fn cfg(mode: Option<FlowMode>) -> FlowConfig {
    FlowConfig { mode, check: Some("tests/gate.sh".into()), push: true, ..FlowConfig::default() }
}

#[test]
fn forced_then_saved_then_suggested() {
    let others = det(Signal::Others { branch: "main".into(), names: vec!["Alice".into()] });
    let prot = det(Signal::Protected { branch: "main".into() });
    assert_eq!(resolve(&cfg(None), None), None);
    let f = resolve(&cfg(None), Some(&others)).unwrap();
    assert_eq!((f.mode, f.source), (FlowMode::Pr, Source::Suggested));
    let f = resolve(&cfg(Some(FlowMode::Trunk)), Some(&others)).unwrap();
    assert_eq!((f.mode, f.source), (FlowMode::Trunk, Source::Saved));
    let f = resolve(&cfg(Some(FlowMode::Trunk)), Some(&prot)).unwrap();
    assert_eq!((f.mode, f.source), (FlowMode::Pr, Source::Forced));
    let f = resolve(&cfg(Some(FlowMode::Trunk)), None).unwrap();
    assert_eq!((f.mode, f.source, f.base.as_str()), (FlowMode::Trunk, Source::Saved, "main"));
    // issue #9: no remote, nothing saved: trunk, the only flow, no question
    let f = resolve(&cfg(None), Some(&det(Signal::NoRemote))).unwrap();
    assert_eq!((f.mode, f.source), (FlowMode::Trunk, Source::Only));
    let f = resolve(&cfg(Some(FlowMode::Pr)), Some(&det(Signal::NoRemote))).unwrap();
    assert_eq!((f.mode, f.source), (FlowMode::Pr, Source::Saved));
}

#[test]
fn the_question_has_2_options_the_suggested_first_marked() {
    let f = resolve(&cfg(None), Some(&det(Signal::Others { branch: "main".into(), names: vec!["alice".into(), "b".into(), "c".into(), "d".into()] }))).unwrap();
    assert_eq!(
        question(&f),
        "how should agents ship code here? alice and 3 others committed on main in the last 90 days.\n1 a PR per task (you merge)  ← suggested\n2 straight to main, tested commits"
    );
    let f = resolve(&cfg(None), Some(&det(Signal::Alone { branch: "main".into() }))).unwrap();
    assert!(question(&f).ends_with("2 straight to main, tested commits  ← suggested"));
}

#[test]
fn slash_flow_shows_why_and_switches() {
    assert!(show(None).starts_with("flow: not known yet"));
    let alone = det(Signal::Alone { branch: "main".into() });
    let f = resolve(&cfg(Some(FlowMode::Trunk)), Some(&alone)).unwrap();
    assert_eq!(
        show(Some(&f)),
        "flow: lands on main · straight to main, tested commits, pushed after every land. only you committed on main in the last 90 days. saved in .switchboard/config.toml; `/flow pr` switches. check: `tests/gate.sh`."
    );
    let f = resolve(&cfg(None), Some(&alone)).unwrap();
    assert!(show(Some(&f)).contains("suggested, not saved: nothing is pushed until it is; `/flow trunk` or `/flow pr` saves it."));
    let only = resolve(&cfg(None), Some(&det(Signal::NoRemote))).unwrap();
    assert!(show(Some(&only)).contains("the only flow without a remote: nothing to push, nothing to ask."));
    let prot = resolve(&cfg(None), Some(&det(Signal::Protected { branch: "main".into() }))).unwrap();
    assert!(show(Some(&prot)).starts_with("flow: lands via PRs · a PR per task (you merge). main is protected here"));
    assert!(switch(Some(&prot), FlowMode::Trunk).unwrap_err().contains("main is protected here"));
    assert!(switch(Some(&prot), FlowMode::Pr).is_ok());
    assert_eq!(parse_mode(" PR "), Ok(FlowMode::Pr));
    assert_eq!(parse_mode("2"), Ok(FlowMode::Trunk));
    assert!(parse_mode("maybe").is_err());
}

#[test]
fn the_commit_style_of_the_last_commits() {
    assert_eq!(commit_style(&["a"; 3]), None);
    let long = "x".repeat(300);
    let s = commit_style(&[long.as_str(); 10]).unwrap();
    assert!(s.starts_with("long, detailed subject lines"), "{s}");
    let s = commit_style(&["fix(tui): the divider", "feat: places", "chore: deps", "fix: x", "docs: y", "Merge branch z"]).unwrap();
    assert!(s.starts_with("Conventional Commits"), "{s}");
    let s = commit_style(&["Fix the login", "Add places", "Bump deps", "Tidy", "Docs"]).unwrap();
    assert!(s.starts_with("short subject lines"), "{s}");
}

fn flow(mode: FlowMode, source: Source) -> Flow {
    Flow { mode, source, why: Some("alice committed on main in the last 90 days.".into()), base: "main".into(), check: Some("./gate.sh --quick".into()), push: true }
}

#[test]
fn mains_flow_section_by_flow() {
    // both: the placement hints replace "--worktree ONLY when the user asks"
    for f in [None, Some(flow(FlowMode::Pr, Source::Saved)), Some(flow(FlowMode::Trunk, Source::Saved))] {
        let s = main_section(f.as_ref(), None);
        assert!(s.starts_with("## Flow\n\n- Places: you decide where each task works"));
        assert!(s.contains("`sb spawn <name> --place new|<agent>`"));
        assert!(s.contains("`dark-mode takes a worktree; i18n joins it`"));
        assert!(s.contains("\"open a PR\", \"just commit it\""));
        assert!(!s.contains("ONLY when the user"));
    }
    assert!(main_section(None, None).contains("Never hold work for it: until then `sb land` commits locally, nothing is pushed."));
    let pr = main_section(Some(&flow(FlowMode::Pr, Source::Saved)), Some("short subject lines"));
    assert!(pr.contains("ships through pull requests (base `main`)"));
    assert!(pr.contains("You never merge; the user does"));
    assert!(pr.contains("checks still failing after 2 tries"));
    assert!(pr.contains("Every commit passes `./gate.sh --quick`"));
    assert!(pr.contains("- Commit messages: short subject lines."));
    assert!(pr.contains("ask once (\"main is shared here, sure?\")"));
    assert!(!pr.contains("sb card"), "saved: no question");
    let trunk = main_section(Some(&flow(FlowMode::Trunk, Source::Saved)), None);
    assert!(trunk.contains("ships straight to `main`, through `sb land`"));
    assert!(trunk.contains("The hub pushes `main` after every land."));
    let local = Flow { push: false, ..flow(FlowMode::Trunk, Source::Saved) };
    assert!(main_section(Some(&local), None).contains("Lands stay local (`push = false`)"));
    // not saved yet (issue #9): the question, once, in a card that holds
    // no work; PR suggested: commits on the task's branch meanwhile
    let s = main_section(Some(&flow(FlowMode::Pr, Source::Suggested)), None);
    assert!(s.contains("ask the user once in a card that blocks nothing, `sb card \"how should agents ship code here? alice committed on main in the last 90 days.\\n1 a PR per task (you merge)  ← suggested\\n2 straight to main, tested commits\"`"));
    assert!(s.contains("never holds work: until it is saved, a task that changes code takes `--place new` and commits on its branch with `sb land --here`"));
    assert!(s.contains("keeps its commits for `sb restore`"));
    assert!(s.contains("At the first such task,"));
    assert!(s.contains("save the answer with `sb flow pr|trunk`, then tell those tasks to `sb land` (pr opens their PR, trunk moves the base), and never ask again."));
    let s = main_section(Some(&flow(FlowMode::Trunk, Source::Suggested)), None);
    assert!(s.contains("never holds work: lands stay local (nothing pushed) until it is saved. At the first land, ask the user once"));
    assert!(s.contains("save the answer with `sb flow pr|trunk`, and never ask again."));
    // no remote: no question at all
    let s = main_section(Some(&flow(FlowMode::Trunk, Source::Only)), None);
    assert!(s.contains("ships straight to `main`, through `sb land`"));
    assert!(!s.contains("sb card"));
    // forced: no question, one line the first time
    let s = main_section(Some(&flow(FlowMode::Pr, Source::Forced)), None);
    assert!(!s.contains("sb card"));
    assert!(s.contains("\"main is protected here: every agent opens a PR\""));
    assert!(s.contains("refused, `main` is protected"));
}

#[test]
fn a_tasks_place_line_by_flow_and_place() {
    let others = vec!["i18n".to_string()];
    let wt = TaskPlace { path: "/w/dark", branch: Some("sb/dark-mode"), others: &others, feature: None };
    let alone = TaskPlace { path: "/w/dark", branch: Some("sb/dark-mode"), others: &[], feature: None };
    let shared = TaskPlace { path: "/w", branch: None, others: &[], feature: None };
    let pr = flow(FlowMode::Pr, Source::Saved);
    let trunk = flow(FlowMode::Trunk, Source::Saved);

    let s = task_place(Some(&pr), &wt, Some("short subject lines"));
    assert!(s.starts_with("`/w/dark` — a git worktree on branch `sb/dark-mode` from `origin/main`, shared with `i18n`."));
    for w in ["sb land --here", "run `./gate.sh --quick` first", "gh pr create", "the hub pushes the branch", "You stay until it is merged", "--force-with-lease", "Never merge, approve, close, or write on GitHub.", "Commit messages: short subject lines."] {
        assert!(s.contains(w), "pr worktree: {w}");
    }
    assert!(task_place(Some(&pr), &alone, None).contains("yours alone for now; others may join"));
    let s = task_place(Some(&pr), &shared, None);
    assert!(s.contains("commit nothing here") && s.contains("Do not revert changes you did not make."));
    // issue #9: PR flow only suggested: commits on the branch, no PR, no push
    let s = task_place(Some(&flow(FlowMode::Pr, Source::Suggested)), &wt, None);
    assert!(s.contains("Commit your files on the branch with `sb land --here \"<message>\"`; run `./gate.sh --quick` first."));
    assert!(s.contains("no PR, no push; main tells you when to `sb land`.") && !s.contains("gh pr create"));

    let s = task_place(Some(&trunk), &shared, None);
    for w in ["Commit nothing by hand: run `./gate.sh --quick`, then `sb land \"<message>\"`", "Never `git add -A`, stash, reset, rebase or amend here.", "Do not revert changes you did not make."] {
        assert!(s.contains(w), "trunk shared: {w}");
    }
    let s = task_place(Some(&trunk), &wt, None);
    assert!(s.contains("`sb land --here \"<message>\"`") && s.contains("`sb land` rebases it on `main` and moves `main`"));
    // no check configured: the repo's tests
    let nocheck = Flow { check: None, ..trunk.clone() };
    assert!(task_place(Some(&nocheck), &shared, None).contains("run the repo's tests"));
    // flow unknown: today's lines
    assert!(task_place(None, &wt, None).contains("never push unless the user asks"));
    assert!(task_place(None, &shared, None).ends_with("Do not revert changes you did not make."));
    assert_eq!(done_when_tail(&pr), "its PR is open");
    assert_eq!(done_when_tail(&trunk), "landed on main");
    // dev-flow §5.1: a feature's agent lands on the feature, never main
    let mates = vec!["cu-apps".to_string()];
    let cu = TaskPlace { path: "/w/cu-broker", branch: Some("sb/cu-broker"), others: &mates, feature: Some("computer-use") };
    let s = task_place(Some(&trunk), &cu, None);
    for w in [
        "You work for the feature `computer-use`, a local branch the user will try before it reaches `main`.",
        "Its other agents: `cu-apps`.",
        "`sb land` puts your commits on `computer-use` (rebased on its tip), never on `main`.",
        "Never merge it, never push it.",
        "run `./gate.sh --quick` first",
    ] {
        assert!(s.contains(w), "feature: {w}\u{a}{s}");
    }
}

/// dev-flow §5.1: main's prompt says when a feature branch, how, and that
/// the merge is the user's go (trunk flow only).
#[test]
fn mains_feature_lines_in_trunk_flow() {
    let trunk = main_section(Some(&flow(FlowMode::Trunk, Source::Saved)), None);
    for w in [
        "Put a task on a **feature branch** when",
        "`sb feature new <name>`",
        "`sb spawn <agent> --feature <name>`",
        "computer-use goes on its own branch: you'll try it before it reaches main.",
        "`sb feature ready <name>` opens the try item",
        "Never merge a feature without the user's go",
        "`sb feature sync <name>`",
    ] {
        assert!(trunk.contains(w), "trunk: {w}");
    }
    let pr = main_section(Some(&flow(FlowMode::Pr, Source::Saved)), None);
    assert!(!pr.contains("sb feature"), "PR flow: every change is a branch already");
}

/// dev-flow §2: the question is asked once: once the answer is saved
/// (`sb flow pr`, `crate::flow::save_mode`), main's prompt no longer
/// carries it and `/flow` says it is saved.
#[test]
fn the_question_is_asked_once_then_saved() {
    let others = det(Signal::Others { branch: "main".into(), names: vec!["alice".into()] });
    let text = "[worktree]\nbase = \"HEAD\"\n";
    let before = resolve(&FlowConfig::parse(text), Some(&others)).unwrap();
    assert!(main_section(Some(&before), None).contains("ask the user once in a card that blocks nothing, `sb card"));
    let saved = crate::flow::with_mode(text, switch(Some(&before), FlowMode::Trunk).map(|_| FlowMode::Trunk).unwrap());
    let after = resolve(&FlowConfig::parse(&saved), Some(&others)).unwrap();
    assert_eq!((after.mode, after.source), (FlowMode::Trunk, Source::Saved));
    assert!(!main_section(Some(&after), None).contains("sb card"));
    assert!(show(Some(&after)).contains("saved in .switchboard/config.toml"));
    assert!(saved.contains("base = \"HEAD\""), "the rest of the config stays");
}
