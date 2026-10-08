//! The TUI's feed events and the hub's fold read the same transcript
//! lines through bise-proto's one parser (`thread::lines`, architect
//! m_10476): for the same lines they say the same words. The proof of
//! that move; each new entry kind adds its pair here.

use super::*;
use bise_proto::thread::{fold, Ctx, EntryKind};

fn lines() -> Vec<&'static str> {
    vec![
        "sb you : make it fast\\nplease",
        "  obs: turn_started",
        "  obs: assistant: <think>hmm</think>\\non it",
        "tool #1 bash : cargo test -q",
        "tool_intent #1 : running the tests",
        "sb msg-in : ambient-lead m_3 : nice : really",
        "sb sent : main : m_9 : 1 : cart \\: or checkout?",
        "sb msg-you : docs : done here",
        "sb stopped : stopped by you",
        "  obs: turn_done: failed: 500",
        "  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s",
        "  obs: turn_done: failed: interrupted by main",
        "  obs: turn_stalled: budget",
        "core rejected: no pending completion",
        "sb warn : the inbox is full",
        "sb spawn : docs started",
        "sb undelivered : perf : then \\: the warm run",
    ]
}

/// (kind, text, who) of each TUI event the fold also makes an entry of.
fn tui_words(ls: &[&str]) -> Vec<(EntryKind, String, String)> {
    let mut out = Vec::new();
    for l in ls {
        match parse_line(l) {
            Some(Ev::You(t, ..)) => out.push((EntryKind::You, t, String::new())),
            Some(Ev::Assistant(t)) => {
                // run.rs makes its thinking a section, its head the shared
                // words (no time: these lines have none); the fold puts it
                // on the reply, or alone when nothing is visible
                let thought = split_thinking(&t).is_some();
                let vis = split_thinking(&t).map(|(_, v)| v).unwrap_or(t);
                let vis = crate::unescape_md(&vis).trim().to_string();
                match (thought, vis.is_empty()) {
                    (_, false) => out.push((EntryKind::Agent, vis, String::new())),
                    (true, true) => out.push((EntryKind::Thinking, words::thought_for(0), String::new())),
                    (false, true) => {}
                }
            }
            Some(Ev::AgentMsg { from, to, text, level: 3, .. }) if from.is_empty() => out.push((EntryKind::ToAgent, text, to)),
            Some(Ev::AgentMsg { from, text, level: 3, .. }) => out.push((EntryKind::FromAgent, text, from)),
            Some(Ev::AgentMsg { text, level: 2, .. }) => out.push((EntryKind::Agent, text, String::new())),
            Some(Ev::Info(t) | Ev::Warn(t) | Ev::Err(t)) => out.push((EntryKind::Notice, t, String::new())),
            Some(Ev::Undelivered { name, text, .. }) => out.push((EntryKind::NotDelivered, text, name)),
            _ => {}
        }
    }
    out
}

#[test]
fn the_tui_and_the_fold_say_the_same_words_for_the_same_lines() {
    let ls = lines();
    let numbered: Vec<bise_proto::thread::Line> = ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 0, l.to_string())).collect();
    let none = |_: &str| None;
    let entries = fold(&numbered, &Ctx { open_cards: &[], page: &none, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0 });
    let folded: Vec<(EntryKind, String, String)> = entries
        .iter()
        // the Stopped entry is the window's only: the TUI says it in its
        // own interrupt lines and draws nothing for sb-core's `stopped`
        .filter(|e| e.kind != EntryKind::Tools && e.kind != EntryKind::Stopped)
        .map(|e| {
            let who = e.from.clone().or_else(|| e.to.clone()).or_else(|| e.not_delivered.as_ref().map(|d| d.to.clone()));
            (e.kind, e.text.clone(), who.unwrap_or_default())
        })
        .collect();
    assert_eq!(folded, tui_words(&ls));
    assert_eq!(folded.len(), 13, "{folded:?}");
    // and the TUI draws nothing for sb-core's `stopped` line
    assert!(parse_line("sb stopped : stopped by you").is_none());
}

/// Item 5 batch 3a (architect m_11122): for the hub's news lines, the
/// TUI's events and the fold's entries say the same: a PR's look comes
/// from its state (`sb::pr_look`), the approval sentences and the answer
/// split are bise_proto's, landed and made carry the same fields.
#[test]
fn the_tui_and_the_fold_agree_on_the_hubs_news() {
    let ls = [
        "sb landed : api : main : sb/api : a1b2c3d : 3 : 42 : 18",
        "sb pr : red : 412 : https://x/412 : checks fail",
        "sb pr : dim : 413 : https://x/413 : merged",
        "sb pr : plain : 414 : https://x/414 : opened",
        "sb artifact : logo : designer : the logo : image : 1",
        "sb answered : docs : v1 or v2? : v2 : the brief",
        "sb approval : outside : api : make bench",
        "sb approval : no : api : rm -rf target : not that",
        "sb route : you → @perf (answer to card #4) : a long answer that never fits on the sentence's own line",
    ];
    let numbered: Vec<_> = ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 0, l.to_string())).collect();
    let none = |_: &str| None;
    let entries = fold(&numbered, &Ctx { open_cards: &[], page: &none, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0 });
    let evs: Vec<Ev> = ls.iter().filter_map(|l| parse_line(l)).collect();
    assert_eq!(entries.len(), evs.len(), "{entries:?}");
    for (e, ev) in entries.iter().zip(&evs) {
        match ev {
            Ev::Landed { agent, from, sha, files, add, del } => {
                let d = e.landed.as_ref().unwrap();
                assert_eq!((&d.agent, &d.from, &d.sha, d.files, d.add, d.del), (agent, from, sha, *files, *add, *del));
            }
            Ev::Pr { tone, number, url, text, .. } => {
                let p = e.pr.as_ref().unwrap();
                assert_eq!((crate::sb::pr_look(p.state), p.number, &p.url, &p.text), (tone.as_str(), *number, url, text));
            }
            Ev::Made { id, agent, title, kind, v } => {
                let m = e.made.as_ref().unwrap();
                assert_eq!((&m.id, &m.agent, &m.title, &m.kind, m.v), (id, agent, title, kind, *v));
                assert_eq!(m.kind_word, crate::artifacts::kind_word(kind));
            }
            Ev::Answered { agent, question, answer, why, .. } => {
                let a = e.answered.as_ref().unwrap();
                assert_eq!((&a.agent, &a.question, &a.answer, &a.why), (agent, question, answer, why));
            }
            Ev::Approval { ok, text, note, .. } => {
                let a = e.approval.as_ref().unwrap();
                assert_eq!((a.ok, &a.text, &a.note), (*ok, text, note));
            }
            _ => panic!("an event the fold doesn't make for {e:?}"),
        }
    }
}

/// architect m_10999: the TUI's gauge (usage::current over its events)
/// and the hub's seed (lines::current_usage over the transcript's lines)
/// read the same usage for the same lines, by the one rule.
#[test]
fn the_tui_and_the_hub_read_the_same_current_usage() {
    let u1 = "  obs: usage: model=mistral/codestral-latest in=1200 out=30";
    let u2 = "  obs: usage: model=mistral/codestral-latest in=42000 out=310";
    let done = "  obs: compaction_done: the summary";
    for ls in [vec![u1], vec![u1, "sb you : hi", u2], vec![u1, done], vec![u1, done, u2], vec!["sb you : hi"]] {
        let evs: Vec<Ev> = ls.iter().filter_map(|l| parse_line(l)).collect();
        let tui = crate::usage::current(&evs).map(|u| (u.model.clone(), u.input, u.output, u.short()));
        let hub = bise_proto::thread::lines::current_usage(ls.iter().map(|l| bise_proto::thread::lines::usage_mark(l)))
            .and_then(|t| bise_session::usage_line::parse(&t))
            .map(|u| (u.model.clone(), u.input, u.output, words::short_words(u.input + u.output, crate::models::context_window(&u.model))));
        assert_eq!(tui, hub, "{ls:?}");
    }
}

/// A failed turn's words come from one place (`thread::words`), which the
/// fold's notice entries will call too.
#[test]
fn a_failed_turn_reads_through_the_shared_words() {
    assert!(matches!(parse_line("  obs: turn_done: failed: 500"), Some(Ev::Err(t)) if t == "turn failed: 500"));
    assert!(matches!(parse_line("  obs: provider_retry: 2/10 · 529 · retry in 4s"), Some(Ev::Warn(t)) if t == words::provider_retry("2/10 · 529 · retry in 4s")));
    assert!(matches!(parse_line("  obs: turn_done: failed: your ChatGPT sign-in expired. x"), Some(Ev::Warn(t)) if t == EXPIRED_TUI));
}

/// Item 5 batch 3b (architect m_11122): a scheduled task's lines (set,
/// ended, a run) read the same in the TUI and in the fold, at the same
/// moment and on the same clock; the stop note is the agent's in both.
#[test]
fn the_tui_and_the_fold_agree_on_scheduled_lines() {
    let set = r#"{"ev":"set","id":48,"agent":"perf","by":"main","label":"every day 07:30","text":"check the build","next_ms":1790050000000,"until_ms":1790300000000}"#;
    let end = r#"{"ev":"end","id":48,"agent":"perf","by":"perf","label":"every 2m","text":"check the build","ended_ms":1790000500000,"end":"times","times":6,"in":"main"}"#;
    let ls: Vec<(u64, u64, String)> = vec![
        (1, 1_790_000_000_000, format!("sb scheduled : {set}")),
        (2, 1_790_000_100_000, "sb msg-in : switchboard m_9 : timer #48 (every 2m, 2/6, waited 4m for perf to finish, set by main): check\\n(stop it: sb every --stop 48)".into()),
        (3, 1_790_000_200_000, "sb msg-in : switchboard m_10 : the user stopped timer #48 (check): don't set it again unless they ask".into()),
        (4, 1_790_000_500_000, format!("sb scheduled : {end}")),
    ];
    let none = |_: &str| None;
    let entries = fold(&ls, &Ctx { open_cards: &[], page: &none, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &crate::when::offset_at });
    let tui: Vec<(String, String)> = ls
        .iter()
        .filter_map(|(_, at, l)| match bise_proto::thread::lines::read(l) {
            bise_proto::thread::lines::Rec::Hub(bise_proto::thread::lines::Hub::Scheduled(json)) => crate::scheduled::hub_line_at(&json, *at),
            _ => parse_line(l),
        })
        .map(|ev| match ev {
            Ev::Scheduled { head, words, .. } => (head, words),
            _ => panic!("not a ◷ line"),
        })
        .collect();
    let folded: Vec<(String, String)> = entries
        .iter()
        .map(|e| {
            assert_eq!(e.kind, EntryKind::Scheduled, "{e:?}");
            let s = e.scheduled.clone().unwrap();
            (s.head, s.words)
        })
        .collect();
    assert_eq!(folded, tui);
    assert_eq!(folded.len(), 3, "the stop note is the agent's: {folded:?}");
}

/// R11 (architect m_13028): the TUI's tool rows and the fold's tool items
/// read a call's result the same way through bise-proto: ok or failed,
/// a failed bash's exit code and first error line, an edit's files with
/// their counts.
#[test]
fn the_tui_and_the_fold_agree_on_tool_results() {
    use bise_proto::thread::ToolState as S;
    let ls = [
        "  obs: tool_started #1",
        "tool #1 bash : cargo test -q",
        "tool_code #1 : cargo test -q",
        "  obs: tool_finished #1 fail",
        "tool_result #1 fail : exit 101: thread 'x' panicked at src/a.rs:3",
        "  obs: tool_started #2",
        "tool #2 apply_patch : {}",
        "tool_code #2 : *** Begin Patch\\N*** Update File: src/a.rs\\N+one\\N+two\\N-old\\N*** End Patch",
        "  obs: tool_finished #2 ok",
        "tool_result #2 ok : done",
        "  obs: tool_started #3",
        "tool #3 read_file : {\"path\":\"b.rs\"}",
        "  obs: tool_finished #3 fail",
        "tool_result #3 fail : no such file b.rs",
    ];
    let mut app = crate::sb::bench::test_app();
    for l in ls {
        crate::run::ingest_line(&mut app, l.to_string(), None);
    }
    let tui: Vec<&ToolData> = app.events.iter().filter_map(|e| if let Ev::Tool(td) = e { Some(td) } else { None }).collect();
    let page = |_: &str| None;
    let ctx = Ctx { open_cards: &[], page: &page, provider: &|_: &str, k: &str| k.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0 };
    let lines: Vec<(u64, u64, String)> = ls.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_000 + i as u64, l.to_string())).collect();
    let entries = fold(&lines, &ctx);
    let items = &entries.iter().find_map(|e| e.tools.as_ref()).expect("a tools entry").items;
    assert_eq!(tui.len(), items.len());
    for (td, it) in tui.iter().zip(items) {
        let state = match td.state {
            ToolState::Run => S::Run,
            ToolState::Ok => S::Ok,
            ToolState::Fail => S::Err,
        };
        assert_eq!(state, it.state, "{:?}", it.text);
        let exit = crate::toolrow::exit_code(td);
        assert_eq!(exit, it.exit.map(|c| format!("exit {c}")), "{:?}", it.text);
        assert_eq!(crate::toolrow::error_line(td), it.err, "{:?}", it.text);
        let patches: Vec<String> = td.code.iter().filter(|_| it.kind == bise_proto::thread::ToolKind::Edit).map(|c| wire_decode(c)).collect();
        let files: Vec<(String, u32, u32)> = crate::toolrow::edit_files(&patches).into_iter().map(|(p, a, d)| (p, a as u32, d as u32)).collect();
        assert_eq!(files, it.files.iter().map(|f| (f.path.clone(), f.add, f.del)).collect::<Vec<_>>(), "{:?}", it.text);
    }
}
