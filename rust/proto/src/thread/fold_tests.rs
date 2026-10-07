use super::*;
use crate::rows::ReportKind;
use crate::thread::lines::{publish_in, report_in};
use crate::thread::words::{one_line, summary};
use crate::thread::{page, EntryKind, Line, PageRef, ReportRef};

fn lines() -> Vec<Line> {
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    vec![
        l(1, "sb you : make it fast"),
        l(2, "  obs: turn_started"),
        l(3, "  obs: assistant: <think>hmm</think>on it"),
        l(4, "tool #1 bash : cargo test -q"),
        l(5, "tool_intent #1 : running the tests"),
        l(6, "tool_result #1 ok : 3 failed"),
        l(7, "tool #2 read_file : {\"path\":\"a.rs\"}"),
        l(8, "tool #3 bash : sb land \"fast\""),
        l(9, "tool_intent #3 : landing the fix"),
        l(10, "sb msg-in : ambient-lead m_3 : nice"),
        l(11, "second line"),
        l(12, "tool #4 bash : sb report done \"the e2e takes 40 s\""),
        l(13, "sb card : #9 question @perf : which bench?\\n1. cold\\n2. warm"),
        l(14, "tool #5 bash : sb page publish $TMPDIR/n.html --id perf-notes"),
        l(15, "  obs: turn_done: completed"),
    ]
}

fn ctx_with<'a>(open: &'a [u64], page: &'a dyn Fn(&str) -> Option<PageRef>) -> Ctx<'a> {
    Ctx { open_cards: open, page, provider: &|id: &str, key: &str| if id.is_empty() { key.to_string() } else { id.to_uppercase() }, width: &|s: &str| s.chars().count(), offset: &|_| 0 }
}

#[test]
fn a_thread_folds_into_entries() {
    let page = |id: &str| (id == "perf-notes").then(|| PageRef { id: id.into(), title: "Perf notes".into(), v: Some(2), url: "http://p/perf-notes".into() });
    let e = fold(&lines(), &ctx_with(&[9], &page));
    let kinds: Vec<EntryKind> = e.iter().map(|e| e.kind).collect();
    use EntryKind::*;
    assert_eq!(kinds, [You, Agent, Tools, FromAgent, Report, Card, Page]);
    assert_eq!(e[1].thinking.as_ref().map(|t| t.text.as_str()), Some("hmm"), "its thinking rides on the reply (one line, one entry)");
    assert_eq!((e[0].pos, e[0].at_ms, e[0].text.as_str()), (1, 1001, "make it fast"));
    assert_eq!(e[1].text, "on it", "no thinking");
    let t = e[2].tools.as_ref().unwrap();
    assert_eq!((e[2].pos, t.count, t.items[0].text.as_str(), t.items[2].land), (4, 3, "running the tests", true));
    assert_eq!(t.summary, "read 1 file, ran 2 commands");
    assert_eq!(e[2].text, t.summary);
    assert_eq!((e[3].text.as_str(), e[3].from.as_deref()), ("nice\nsecond line", Some("ambient-lead")));
    assert_eq!(e[4].report, Some(ReportRef { kind: ReportKind::Done }));
    let c = e[5].card.as_ref().unwrap();
    assert_eq!((c.id, c.question.as_str(), c.options.len(), c.answered), (9, "which bench?", 2, false));
    assert_eq!(e[6].page.as_ref().unwrap().v, Some(2));
    assert_eq!(e[6].text, "Perf notes v2");
    // closed card: answered
    assert!(fold(&lines(), &ctx_with(&[], &page))[5].card.as_ref().unwrap().answered);
}

/// An interrupt's 'stopped' line (sb-core, architect m_9650 D) is an
/// entry of its own at its position, and ends the message before it.
#[test]
fn a_stopped_line_is_a_stopped_entry() {
    let page = |_: &str| None;
    let ls = vec![
        (1, 1001, "sb you : go".to_string()),
        (2, 1002, "  obs: assistant: working on".to_string()),
        (3, 1003, "sb stopped : stopped".to_string()),
        (4, 1004, "a line after".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &page));
    let kinds: Vec<EntryKind> = e.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EntryKind::You, EntryKind::Agent, EntryKind::Stopped]);
    assert_eq!((e[2].pos, e[2].text.as_str()), (3, "stopped"));
    assert_eq!(e[1].text, "working on", "the line after the stop doesn't join the agent's message");
}

/// What a task sent (sb-core's `sent` line, architect m_10203) is a
/// `to_agent` entry: to whom, its id, whether it asks; the card main
/// opens for it, written in the task's feed too, is its card entry.
#[test]
fn a_sent_line_is_the_tasks_own_message() {
    let page = |_: &str| None;
    let ls = vec![
        (1, 1001, "tool #1 bash : sb send main --expect-reply \"cart or checkout?\"".to_string()),
        (2, 1002, "sb sent : main : m_9 : 1 : gift cards: cart \\: or checkout?\\n1. on the cart page\\n2. only at checkout".to_string()),
        (3, 1003, "sb card : #2 question @gift-ui : gift cards: cart or checkout?\\n1. on the cart page\\n2. only at checkout".to_string()),
        (4, 1004, "sb sent : docs : m_11 : 0 : fyi".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[2], &page));
    let kinds: Vec<EntryKind> = e.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EntryKind::Tools, EntryKind::ToAgent, EntryKind::Card, EntryKind::ToAgent]);
    assert_eq!((e[1].to.as_deref(), e[1].msg, e[1].asks), (Some("main"), Some(9), true));
    assert_eq!(e[1].text, "gift cards: cart : or checkout?\n1. on the cart page\n2. only at checkout", "its fields unescaped");
    let c = e[2].card.as_ref().unwrap();
    assert_eq!((c.id, c.options.len(), c.answered), (2, 2, false));
    assert_eq!((e[3].to.as_deref(), e[3].msg, e[3].asks, e[3].text.as_str()), (Some("docs"), Some(11), false, "fyi"));
    let j = serde_json::to_value(&e[3]).unwrap();
    assert_eq!((j["kind"].as_str(), j.get("asks")), (Some("to_agent"), None), "asks: false is left out");
}

/// Law (architect m_9048): a context line attaches only to the 'you'
/// line right before it; a stray one (after anything else, or a second
/// one) is dropped, never attached elsewhere.
#[test]
fn a_context_line_belongs_to_the_you_line_right_before_it() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let l = |pos: u64, line: &str| (pos, pos, line.to_string());
    let lines = vec![
        l(1, r#"sb you : why is this slow?"#),
        l(2, r#"sb context : {"app":"Safari","url":"https://grafana.acme.test/d/p99"}"#),
        l(3, r#"sb context : {"app":"Mail"}"#),
        l(4, "  obs: assistant: looking"),
        l(5, r#"sb context : {"app":"Notes"}"#),
        l(6, "sb you : and this?"),
        l(7, "  obs: turn_started"),
        l(8, r#"sb context : {"app":"Code"}"#),
        l(9, "sb you : third"),
        l(10, "sb context : not json"),
    ];
    let e = fold(&lines, &ctx);
    assert_eq!(e.len(), 4);
    assert_eq!(e[0].context.as_ref().and_then(|c| c.url.as_deref()), Some("https://grafana.acme.test/d/p99"));
    assert_eq!(e[0].text, "why is this slow?");
    assert!(e[1..].iter().all(|e| e.context.is_none()), "{e:?}");
    assert_eq!(e[2].text, "and this?");
}

#[test]
fn a_page_keeps_the_newest_and_says_what_is_before() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let (e, before, more) = page(&lines(), &ctx, 60);
    assert_eq!((e.len(), before, more), (7, None, false));
    let (e, before, more) = page(&lines(), &ctx, 2);
    assert_eq!((e.len(), before, more), (2, Some(13), true));
    // a page that doesn't start the thread: its first entry may be cut
    let (e, before, more) = page(&lines()[3..10], &ctx, 60);
    assert_eq!(e[0].kind, EntryKind::FromAgent);
    assert_eq!((before, more), (Some(10), true));
}

#[test]
fn bash_calls_read_as_reports_and_publishes() {
    assert_eq!(report_in(r#"cd x && sb report done "landed \"a\"" --decision y"#), Some((ReportKind::Done, "landed \"a\"".into())));
    assert_eq!(report_in("sb report progress 'half'"), Some((ReportKind::Progress, "half".into())));
    assert_eq!(report_in("sb report nope x"), None);
    assert_eq!(publish_in("sb page publish ./weekly.html"), Some("weekly".into()));
    assert_eq!(summary(&[]), "");
    assert_eq!(one_line("  \nabcdef", 4), "abc…");
}

/// Batch 2 (bar T): thinking (its time from the line before), the
/// compaction's two rows, the notices (a failed turn and why, a retry, a
/// stop someone asked for, a hub warning) and a message not delivered;
/// each entry carries exactly its kind's payload.
#[test]
fn the_turns_facts_are_entries_of_their_kind() {
    let none = |_: &str| None;
    let ls: Vec<Line> = vec![
        (1, 1_000, "sb you : go".to_string()),
        (2, 1_100, "  obs: turn_started".to_string()),
        // a tool-call-only reply: its thinking is an entry alone
        (3, 4_300, "  obs: assistant: <think>cold\\nor warm</think>".to_string()),
        (4, 4_350, "  obs: assistant: on it".to_string()),
        (5, 4_400, "  obs: compaction_started #1 auto".to_string()),
        (6, 4_500, "  obs: compaction_done: we profiled".to_string()),
        (7, 4_600, "  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s".to_string()),
        (8, 4_700, "  obs: turn_done: failed: provider 500".to_string()),
        (9, 4_800, "  obs: turn_done: failed: interrupted by main".to_string()),
        (10, 4_900, "  obs: turn_done: failed: no openrouter key yet (OPENROUTER_API_KEY is not set): /provider sets it up".to_string()),
        (11, 5_000, "sb warn : the inbox is full".to_string()),
        (12, 5_100, "sb undelivered : perf : then \\: the warm run".to_string()),
        // a reply's own thinking rides on it (one line, one entry)
        (13, 5_200, "history   obs: assistant: <think>old</think>replayed".to_string()),
        (14, 5_300, "  obs: usage: model=m in=1 out=2 cache_read=0 cache_write=0".to_string()),
        (15, 5_400, "  obs: turn_done: completed".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let kinds: Vec<EntryKind> = e.iter().map(|e| e.kind).collect();
    use EntryKind::*;
    assert_eq!(kinds, [You, Thinking, Agent, Compacting, Compacted, Notice, Notice, Notice, Notice, Notice, NotDelivered, Agent]);
    let t = e[1].thinking.as_ref().unwrap();
    assert_eq!((t.ms, t.text.as_str(), e[1].text.as_str()), (3_200, "cold\nor warm", "thought for 3.2s"), "the time since the line before");
    assert_eq!(e[2].text, "on it");
    assert_eq!(e[4].text, "we profiled");
    let n = |i: usize| e[i].notice.clone().map(|n| (n.level, n.text)).unwrap();
    use crate::thread::NoticeLevel::*;
    assert_eq!(n(5), (Warn, "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s".into()));
    assert_eq!(n(6), (Err, "turn failed: provider 500".into()));
    assert_eq!(n(7), (Warn, "turn interrupted by main".into()), "a stop someone asked for, not a failure");
    assert_eq!(n(8), (Err, "turn stopped: no OPENROUTER key yet. /provider sets it up.".into()), "the provider's name from the ctx");
    assert_eq!(n(9), (Warn, "the inbox is full".into()));
    let d = e[10].not_delivered.as_ref().unwrap();
    assert_eq!((d.to.as_str(), d.text.as_str(), e[10].text.as_str()), ("perf", "then : the warm run", "then : the warm run"));
    assert_eq!(e[11].thinking.as_ref().map(|t| (t.ms, t.text.as_str(), e[11].text.as_str())), Some((0, "old", "replayed")), "a replay has no duration");
    assert!(e.iter().all(Entry::payload_matches_kind), "{e:?}");
    assert_pos_unique(&e);
}

/// Law (architect m_10725): every pos appears once in a fold, pos being
/// the entry's key (Live's diff, the window's store).
fn assert_pos_unique(e: &[Entry]) {
    let mut seen = std::collections::BTreeSet::new();
    for x in e {
        assert!(seen.insert(x.pos), "pos {} twice: {e:?}", x.pos);
    }
}

/// Item 5 batch 3a (architect m_11122): the hub's news lines are entries
/// of their kind with their payload and the TUI's words; a page's
/// artifact line comes once (its publish's page entry says it); the
/// hub's own timer wakes in main's thread are not shown.
#[test]
fn the_hubs_news_lines_are_entries_of_their_kind() {
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let ls = vec![
        l(1, "sb landed : api : main : sb/api : a1b2c3d : 3 : 42 : 18"),
        l(2, "sb pr : red : 412 : https://x/412 : checks fail \\: e2e"),
        l(3, "sb pr : dim : 413 : https://x/413 : merged"),
        l(4, "tool #1 bash : sb page publish n.html --id q3"),
        l(5, "sb artifact : q3 : designer : Q3 plan : page : 2"),
        l(6, "sb artifact : logo : designer : the logo : image : 1"),
        l(7, "sb answered : docs : v1 or v2? : v2 : the brief says v2"),
        l(8, "sb approval : no : api : rm -rf target : not that"),
        l(9, "sb approval : allowed : api : cargo test"),
        l(10, "sb route : you → @perf (answer to card #4) : both"),
        l(11, "sb msg : perf → docs m_7 : the numbers are in"),
        l(12, "sb msg : switchboard → perf m_8 : timer #3 (every 2m, 1/4, set by perf): check it"),
    ];
    let page = |id: &str| Some(PageRef { id: id.into(), title: "Q3 plan".into(), v: Some(2), url: format!("http://h/p/{id}") });
    let e = fold(&ls, &ctx_with(&[], &page));
    let kinds: Vec<EntryKind> = e.iter().map(|x| x.kind).collect();
    use EntryKind::*;
    assert_eq!(kinds, [Landed, Pr, Pr, Page, Artifact, Answered, Approval, Approval, Approval, FromAgent], "{e:?}");
    assert!(e.iter().all(|x| x.payload_matches_kind()), "{e:?}");
    assert_eq!(e[0].text, "3 files +42 −18");
    assert_eq!(e[0].landed.as_ref().map(|d| (d.sha.as_str(), d.target.as_str())), Some(("a1b2c3d", "main")));
    assert_eq!(e[1].pr.as_ref().map(|p| (p.number, p.state, p.text.as_str())), Some((412, crate::thread::PrNewsState::Failing, "checks fail : e2e")));
    assert_eq!(e[2].pr.as_ref().map(|p| p.state), Some(crate::thread::PrNewsState::Done));
    let made = e[4].made.as_ref().unwrap();
    assert_eq!((made.id.as_str(), made.kind_word.as_str(), made.v, made.url.as_deref()), ("logo", "image", 1, None));
    assert_eq!(e[5].answered.as_ref().map(|a| a.why.as_str()), Some("the brief says v2"));
    let ap = |i: usize| e[i].approval.clone().unwrap();
    assert_eq!((ap(6).ok, ap(6).text.as_str(), ap(6).note.as_str()), (false, "you said no to api: rm -rf target", "not that"));
    assert_eq!((ap(7).ok, ap(7).text.as_str()), (true, "you allowed api: cargo test"));
    assert_eq!((ap(8).text.as_str(), ap(8).note.as_str()), ("you answered perf: both", ""));
    assert_eq!((e[9].from.as_deref(), e[9].to.as_deref(), e[9].msg), (Some("perf"), Some("docs"), Some(7)));
}

/// batch 3b (site/m/timers): a task set and ended (the hub's
/// `scheduled` lines) and a run (bise's wake in the agent's own feed) are
/// `scheduled` entries with the TUI's words, read at the line's own time;
/// the stop note stays the agent's; main's copy of another agent's run
/// is not shown (its ◷ lines say enough).
#[test]
fn scheduled_lines_are_scheduled_entries() {
    let set = r#"{"ev":"set","id":48,"agent":"perf","by":"main","label":"every 2m","text":"check the build","next_ms":1790000120000,"times":6}"#;
    let end = r#"{"ev":"end","id":48,"agent":"perf","by":"main","label":"every 2m","text":"check the build","ended_ms":1790000500000,"end":"stopped","stopped_by":"user","times":6}"#;
    let ls = vec![
        (1, 1_790_000_000_000, format!("sb scheduled : {set}")),
        (2, 1_790_000_120_000, "sb msg-in : switchboard m_9 : timer #48 (every 2m, 1/6, set by main): check the build\\n(stop it: sb every --stop 48)".to_string()),
        (3, 1_790_000_200_000, "sb msg-in : switchboard m_10 : the user stopped timer #48 (check the build): don't set it again unless they ask".to_string()),
        (4, 1_790_000_500_000, format!("sb scheduled : {end}")),
        (5, 1_790_000_600_000, "sb msg : switchboard → docs m_11 : timer #7 (every 5m, set by docs): poll".to_string()),
    ];
    let none = |_: &str| None;
    let e = fold(&ls, &ctx_with(&[], &none));
    assert!(e.iter().all(|x| x.kind == EntryKind::Scheduled && x.payload_matches_kind()), "{e:?}");
    let s: Vec<_> = e.iter().map(|x| x.scheduled.clone().unwrap()).collect();
    assert_eq!(s.len(), 3, "{s:?}");
    assert_eq!((s[0].id, s[0].words.as_str()), (48, "check the build"));
    assert!(s[0].head.starts_with("main scheduled #48 for perf · every 2m · 6 times · next "), "{}", s[0].head);
    assert_eq!(s[1].head, "scheduled #48 · 1 of 6 · check the build");
    assert_eq!((s[2].head.as_str(), s[2].words.as_str()), ("scheduled #48 ended · stopped by you", ""));
    assert_eq!(e[0].text, s[0].head, "the text is the line's head");
    // a replay reads the same words: `now` is the line's own time
    assert_eq!(fold(&ls, &ctx_with(&[], &none)), e);
}

#[test]
fn every_pos_appears_once_in_a_fold() {
    let none = |_: &str| None;
    assert_pos_unique(&fold(&lines(), &ctx_with(&[9], &none)));
}

/// Law: in every fixture, an entry of a kind with a payload carries
/// exactly that payload (architect m_10476).
#[test]
fn every_fixture_entry_carries_its_kinds_payload() {
    let text = include_str!("../../fixtures/hub_ev.jsonl");
    let mut n = 0;
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        let entries: Vec<serde_json::Value> = match v["ev"].as_str() {
            Some("entry") => vec![v["entry"].clone()],
            Some("thread") => v["entries"].as_array().cloned().unwrap_or_default(),
            _ => continue,
        };
        for e in entries {
            let e: Entry = serde_json::from_value(e).unwrap();
            assert!(e.payload_matches_kind(), "{e:?}");
            n += 1;
        }
    }
    assert!(n >= 14, "{n} entries");
}
