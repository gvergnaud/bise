//! What a connector call does (`connector`) and how `judge` takes it in
//! `auto`: a write is a card even when the cache knows it, a saved rule
//! runs it, a read runs, the rest asks the checker. Pinned on real names.

use super::connector::{act, reason, service, warns, words_of, Act, Write};
use super::*;
use serde_json::json;

const NAMES: &str = include_str!("testdata/connector_names.txt");

fn call(tool: &str) -> Call {
    Call {
        tool: tool.into(),
        args: json!({"channel_id": "C1", "message": "hi"}),
        agent: "a".into(),
        cwd: "/w/repo".into(),
        repo: "/w/repo".into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
        home: "/h".into(),
        bise: "/h/.bise".into(),
        edit_tool: "edit".into(),
        flow: None,
        pending_review: None,
    }
}

fn class(a: Act) -> String {
    match a {
        Act::Read => "read".into(),
        Act::Unknown => "unknown".into(),
        Act::Write(w) => format!("{w:?}").to_lowercase(),
    }
}

#[test]
fn words_drop_the_connector_prefix_and_split_camel_case() {
    assert_eq!(words_of("slack.slack_send_message"), ["send", "message"]);
    assert_eq!(words_of("x.sendDirectMessage"), ["send", "direct", "message"]);
    assert_eq!(words_of("web_search.web_search"), ["web", "search"]);
    assert_eq!(words_of("Granola.get_meetings"), ["get", "meetings"]);
}

/// architect m_21128 (4): the class of every real name, frozen; a write
/// classed read fails first, with its name.
#[test]
fn real_connector_names_keep_their_class() {
    let mut wrong = vec![];
    for line in NAMES.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (name, want) = line.split_once(' ').expect("name class");
        let got = class(act(name));
        assert!(!(got == "read" && want != "read"), "{name}: a write classed read");
        if got != want {
            wrong.push(format!("{name}: {got}, want {want}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// architect m_21128 (2), (3): read is narrow.
#[test]
fn a_read_is_only_a_sure_read() {
    for t in [
        "x.get_and_mark_read",
        "github.open_pull_request",
        "web_search.open_url",
        "github_app.get_label",
        "outlook_calendar.get_schedule",
        "gmail.get_email",
        "hn.story",
        "bigquery.sql",
    ] {
        assert_ne!(act(t), Act::Read, "{t}");
    }
    assert_eq!(act("x.get_and_mark_read"), Act::Unknown, "the checker's");
    for t in ["slack.slack_read_channel", "linear.list_issues", "wandb.query_wandb_tool", "Granola.get_meeting_transcript"] {
        assert_eq!(act(t), Act::Read, "{t}");
    }
}

#[test]
fn the_kind_of_write() {
    for (t, w) in [
        ("slack.slack_send_message", Write::Send),
        ("x.sendDirectMessage", Write::Send),
        ("bluesky.post", Write::Send),
        ("gmail.reply_to_thread", Write::Send),
        ("slack.message_user", Write::Send),
        ("slack.slack_schedule_message", Write::Schedule),
        ("outlook_calendar.schedule_event", Write::Schedule),
        ("gmail.delete_label", Write::Delete),
        ("outlook_calendar.cancel_event", Write::Delete),
        ("linear.save_issue", Write::Change),
        ("gmail.label_message", Write::Change),
        ("github_app.merge_pull_request", Write::Change),
    ] {
        assert_eq!(act(t), Act::Write(w), "{t}");
    }
}

/// designer m_21125's words; a delete is the card's warning.
#[test]
fn the_service_and_the_sentences() {
    assert_eq!(service("gmail.send_email"), "Gmail");
    assert_eq!(service("outlook_calendar.schedule_event"), "Outlook Calendar");
    assert_eq!(service("sentry_agent.x"), "Sentry Agent");
    assert_eq!(reason("slack.slack_send_message", Write::Send), "it sends from your Slack account, as you.");
    assert_eq!(reason("slack.slack_schedule_message", Write::Schedule), "it schedules a send from your Slack account, as you.");
    assert_eq!(reason("gmail.delete_label", Write::Delete), "it deletes from your Gmail account. it may not be undone.");
    assert_eq!(reason("linear.save_issue", Write::Change), "it changes your Linear account.");
    assert!(warns("gmail.delete_label"));
    assert!(!warns("gmail.send_email") && !warns("bash") && !warns("edit"));
}

/// In `auto` a write never runs without his word: a card, with "always"
/// for the whole tool, even when the checker's cache has its exact call.
#[test]
fn a_write_is_a_card_even_when_the_cache_knows_it() {
    let c = call("slack.slack_send_message");
    let mut cache = Cache::default();
    cache.allow(CacheKey::Exact(format!("{} {}", c.tool, c.args)));
    match judge_with(&c, &Rules::default(), &cache, false, &LexicalFs) {
        Verdict::Card { reason, always } => {
            assert_eq!(reason, "it sends from your Slack account, as you.");
            assert_eq!(always, Some(vec!["slack.slack_send_message".to_string()]));
        }
        v => panic!("{v:?}"),
    }
}

/// His "always" (a whole-tool rule of this repo) is used again; another
/// repo's is not.
#[test]
fn a_saved_rule_runs_the_write_again() {
    let rules = rules::parse(
        "[[allow]]
project = \"/w/repo\"
tool = \"slack.slack_send_message\"
",
    )
    .unwrap();
    let c = call("slack.slack_send_message");
    assert_eq!(judge_with(&c, &rules, &Cache::default(), false, &LexicalFs), Verdict::Allow { tier: 2 });
    let mut other = call("slack.slack_send_message");
    other.repo = "/w/other".into();
    assert!(matches!(judge_with(&other, &rules, &Cache::default(), false, &LexicalFs), Verdict::Card { .. }));
    // a rule for one tool is not a rule for its neighbor
    assert!(matches!(
        judge_with(&call("slack.slack_schedule_message"), &rules, &Cache::default(), false, &LexicalFs),
        Verdict::Card { .. }
    ));
}

#[test]
fn a_read_runs_and_the_rest_asks_the_checker() {
    let j = |t: &str| judge_with(&call(t), &Rules::default(), &Cache::default(), false, &LexicalFs);
    assert_eq!(j("slack.slack_read_channel"), Verdict::Allow { tier: 1 });
    assert!(matches!(j("web_search.open_url"), Verdict::Check { .. }));
    assert!(matches!(j("hn.story"), Verdict::Check { .. }));
    // drafts stay free (docs/ambient-roadmap.md A)
    assert_eq!(j("gmail.create_draft"), Verdict::Allow { tier: 1 });
}

/// computer use's upload (task cu-upload, architect m_52654): the checker
/// judges it with his words, by the exact call; a saved `computer.act` rule
/// doesn't cover it, its card never saves "always", and a click is as before.
#[test]
fn an_upload_goes_to_the_checker_by_its_exact_call() {
    let up = |files: serde_json::Value| {
        let mut c = call("computer.act");
        c.args = json!({"target": "tab:7", "action": "set_input_files", "locator": {"role": "button", "name": "Upload"}, "files": files});
        c
    };
    let a = up(json!(["/h/Documents/CAFPI/01 payslip.pdf"]));
    let key = CacheKey::Exact(format!("{} {}", a.tool, a.args));
    assert_eq!(
        judge_with(&a, &Rules::default(), &Cache::default(), false, &LexicalFs),
        Verdict::Check {
            parts: vec![],
            keys: vec![key.clone()]
        }
    );
    // the checker sees the action and the file names
    let st = checker::checker_state(&a, &[], "attach my payslips on cafpi", &[], None);
    assert!(
        st.commands[0].contains("set_input_files") && st.commands[0].contains("01 payslip.pdf"),
        "{:?}",
        st.commands
    );
    // the cache allows that call again, never another file
    let mut cache = Cache::default();
    cache.allow(key);
    assert_eq!(
        judge_with(&a, &Rules::default(), &cache, false, &LexicalFs),
        Verdict::Allow { tier: 4 }
    );
    assert!(matches!(
        judge_with(
            &up(json!(["/h/.aws/x"])),
            &Rules::default(),
            &cache,
            false,
            &LexicalFs
        ),
        Verdict::Check { .. }
    ));
    // his "always" for computer.act runs clicks, not uploads
    let rules = rules::parse(
        "[[allow]]
project = \"/w/repo\"
tool = \"computer.act\"
",
    )
    .unwrap();
    assert!(matches!(
        judge_with(&a, &rules, &Cache::default(), false, &LexicalFs),
        Verdict::Check { .. }
    ));
    let mut click = call("computer.act");
    click.args = json!({"target": "tab:7", "action": "click", "locator": {"role": "button", "name": "Upload"}});
    assert_eq!(
        judge_with(&click, &rules, &Cache::default(), false, &LexicalFs),
        Verdict::Allow { tier: 2 }
    );
    assert!(matches!(
        judge_with(
            &click,
            &Rules::default(),
            &Cache::default(),
            false,
            &LexicalFs
        ),
        Verdict::Check { .. }
    ));
    // its card (checker off, or the checker asks) offers no "always"
    assert_eq!(always_tool(&a), None);
    assert_eq!(always_tool(&click), Some("computer.act".to_string()));
    assert_eq!(
        card_when_off(&[], always_tool(&a)),
        Verdict::Card {
            reason: CHECKER_OFF.into(),
            always: None
        }
    );
}
