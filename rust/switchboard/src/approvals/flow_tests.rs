//! dev-flow §6, "Approvals (auto mode)": the rows each flow sets, row by
//! row, in a worktree on `sb/x` and in the shared folder.

use super::*;
use crate::flow::FlowMode::{self, Pr, Trunk};
use serde_json::json;

fn call(cmd: &str, flow: Option<FlowRules>) -> Call {
    Call {
        tool: "bash".into(),
        args: json!({ "arg": cmd }),
        agent: "x".into(),
        cwd: "/w/repo".into(),
        repo: "/w/repo".into(),
        tmp: "/h/.bise/hubs/hx/agents/x/tmp".into(),
        home: "/h".into(),
        bise: "/h/.bise".into(),
        edit_tool: "edit".into(),
        flow,
        pending_review: None,
    }
}

fn rules(mode: FlowMode, push: bool, branch: Option<&str>) -> Option<FlowRules> {
    Some(FlowRules { mode, push, base: "main".into(), branch: branch.map(String::from) })
}

#[derive(Debug, PartialEq)]
enum V {
    Runs,
    /// The checker decides (no flow row).
    Check,
    /// A card that offers "always", with its reason.
    Asks(String),
    /// A hard rule, with its reason.
    Always(String),
}

fn judge_in(cmd: &str, flow: Option<FlowRules>, saved: &Rules) -> V {
    match judge_with(&call(cmd, flow), saved, &Cache::default(), false, &LexicalFs) {
        Verdict::Allow { .. } => V::Runs,
        Verdict::Check { .. } => V::Check,
        Verdict::Card { reason, always: Some(_) } => V::Asks(reason),
        Verdict::Card { reason, always: None } => V::Always(reason),
        v => panic!("{cmd}: {v:?}"),
    }
}

fn v(cmd: &str, flow: Option<FlowRules>) -> V {
    judge_in(cmd, flow, &Rules::default())
}

const WT: Option<&str> = Some("sb/x");

#[test]
fn git_commit_runs_in_both_flows() {
    for mode in [Pr, Trunk] {
        assert_eq!(v("git commit -m 'x'", rules(mode, true, WT)), V::Runs, "{mode:?}");
    }
}

#[test]
fn pushing_its_own_branch_runs_in_pr_flow_asks_in_trunk() {
    for cmd in ["git push origin sb/x", "git push", "git push -u origin HEAD", "git push origin sb/x:sb/x"] {
        assert_eq!(v(cmd, rules(Pr, true, WT)), V::Runs, "{cmd}");
        assert_eq!(
            v(cmd, rules(Trunk, true, WT)),
            V::Asks("it pushes sb/x, and this repo lands on main: no PR here.".into()),
            "{cmd}"
        );
    }
    // another agent's branch: no row, the checker decides
    assert_eq!(v("git push origin sb/other", rules(Pr, true, WT)), V::Check);
    // forced stays a hard rule, own branch or not
    assert_eq!(
        v("git push --force-with-lease origin sb/x", rules(Pr, true, WT)),
        V::Always("it rewrites the history of sb/x. this one always asks.".into())
    );
}

#[test]
fn pushing_the_default_branch() {
    let always = V::Always("it pushes to main. this one always asks.".into());
    // PR flow: always asks, named or implied (the shared folder is on main)
    assert_eq!(v("git push origin main", rules(Pr, true, WT)), always);
    assert_eq!(v("git push", rules(Pr, true, None)), always);
    assert_eq!(v("git push origin HEAD:main", rules(Pr, true, WT)), always);
    // trunk flow: runs when the flow pushes, else asks
    assert_eq!(v("git push origin main", rules(Trunk, true, None)), V::Runs);
    assert_eq!(v("git push", rules(Trunk, true, None)), V::Runs);
    assert_eq!(
        v("git push origin main", rules(Trunk, false, None)),
        V::Asks("it pushes main, and this repo keeps its lands local (push = false).".into())
    );
    // no flow known: today's hard rule
    assert_eq!(v("git push origin main", None), always);
}

#[test]
fn gh_pr_reads_and_create_run_in_pr_flow_ask_in_trunk() {
    for sub in ["create --fill", "view 412", "checks", "diff", "list", "status"] {
        let cmd = format!("gh pr {sub}");
        assert_eq!(v(&cmd, rules(Pr, true, WT)), V::Runs, "{cmd}");
        assert_eq!(
            v(&cmd, rules(Trunk, true, WT)),
            V::Asks("it uses a PR, and this repo lands on main: no PR here.".into()),
            "{cmd}"
        );
    }
}

#[test]
fn forge_writes_always_ask_in_both_flows() {
    let cases = [
        ("gh pr merge 412 --squash", "it merges a PR on GitHub. this one always asks."),
        ("gh pr review 412 --approve", "it approves a PR on GitHub. this one always asks."),
        ("gh pr review 412 --comment -b ok", "it reviews a PR on GitHub. this one always asks."),
        ("gh pr comment 412 -b done", "it comments on a PR on GitHub. this one always asks."),
        ("gh pr close 412", "it closes a PR on GitHub. this one always asks."),
        ("gh api -X POST repos/a/b/issues/1/comments -f body=hi", "it writes through the API on GitHub. this one always asks."),
        ("gh api repos/a/b/pulls/1/reviews -f event=APPROVE", "it writes through the API on GitHub. this one always asks."),
        ("gh api --method=DELETE repos/a/b/git/refs/heads/x", "it writes through the API on GitHub. this one always asks."),
    ];
    for mode in [Pr, Trunk] {
        for (cmd, reason) in cases {
            assert_eq!(v(cmd, rules(mode, true, WT)), V::Always(reason.into()), "{mode:?} {cmd}");
        }
    }
    // gh api reads are not writes: no flow row
    assert_eq!(v("gh api repos/a/b/pulls/1", rules(Pr, true, WT)), V::Check);
    assert_eq!(v("gh api -X GET repos/a/b", rules(Pr, true, WT)), V::Check);
}

#[test]
fn sb_land_is_refused_in_pr_flow_runs_in_trunk() {
    assert_eq!(
        v("sb land \"the login fix\"", rules(Pr, true, None)),
        V::Always("this repo ships through pull requests: commit with sb land --here, then open a PR. this one always asks.".into())
    );
    assert_eq!(v("sb land --here \"wip\"", rules(Pr, true, WT)), V::Runs);
    assert_eq!(v("sb land \"the login fix\"", rules(Trunk, true, None)), V::Runs);
    assert_eq!(v("sb land --here \"wip\"", rules(Trunk, true, WT)), V::Runs);
}

#[test]
fn a_saved_rule_runs_what_the_flow_asks() {
    let saved = rules::parse("[[allow]]\ntool = \"bash\"\npattern = \"gh pr view *\"\n").unwrap();
    assert_eq!(judge_in("gh pr view 412", rules(Trunk, true, WT), &saved), V::Runs);
    // a hard row stays hard
    let saved = rules::parse("[[allow]]\ntool = \"bash\"\npattern = \"gh pr merge *\"\n").unwrap();
    assert!(matches!(judge_in("gh pr merge 1", rules(Pr, true, WT), &saved), V::Always(_)));
}

#[test]
fn the_card_offers_the_rows_pattern() {
    let c = call("gh pr checks 412", rules(Trunk, true, WT));
    match judge_with(&c, &Rules::default(), &Cache::default(), false, &LexicalFs) {
        Verdict::Card { always: Some(a), .. } => assert_eq!(a, vec!["gh pr checks *".to_string()]),
        x => panic!("{x:?}"),
    }
}
