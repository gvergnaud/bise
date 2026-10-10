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
        l(4, "  obs: tool_started #1"),
        l(4, "tool #1 bash : cargo test -q"),
        l(5, "tool_intent #1 : running the tests"),
        l(6, "tool_result #1 ok : 3 failed"),
        l(7, "  obs: tool_started #2"),
        l(7, "tool #2 read_file : {\"path\":\"a.rs\"}"),
        l(8, "  obs: tool_started #3"),
        l(8, "tool #3 bash : sb land \"fast\""),
        l(9, "tool_intent #3 : landing the fix"),
        l(10, "sb msg-in : ambient-lead m_3 : nice"),
        l(11, "second line"),
        l(12, "  obs: tool_started #4"),
        l(12, "tool #4 bash : sb report done \"the e2e takes 40 s\""),
        l(13, "sb card : #9 question @perf : which bench?\\n1. cold\\n2. warm"),
        l(14, "  obs: tool_started #5"),
        l(14, "tool #5 bash : sb page publish $TMPDIR/n.html --id perf-notes"),
        l(15, "  obs: turn_done: completed"),
    ]
}

fn ctx_with<'a>(open: &'a [u64], page: &'a dyn Fn(&str) -> Option<PageRef>) -> Ctx<'a> {
    Ctx { open_cards: open, page, provider: &|id: &str, key: &str| if id.is_empty() { key.to_string() } else { id.to_uppercase() }, width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &crate::thread::Attached::plain }
}

/// R41 (architect m_14049): an answered line whose answer has his pasted
/// files rendered in (the hub's attached::split reads them back, given as
/// Ctx.attached; a stand-in here) is his words, the images and the files
/// apart: no marker or file list in any text the window shows.
#[test]
fn an_answer_with_pasted_files_is_its_words_images_and_files() {
    use crate::thread::{Attached, ImageRef};
    let split = |t: &str| match t.split_once("\n\n") {
        Some((w, _)) => Attached { words: w.into(), images: vec![ImageRef { name: "[Image #1]".into(), path: "/Users/ana/shot.png".into() }], files: vec!["/w/notes.md".into()] },
        None => Attached::plain(t),
    };
    let ls = vec![
        (1, 1000, "sb answered : gift-ui : which balance? : the cart page\\n\\n<image name=\"[Image #1]\" path=\"/Users/ana/shot.png\" mime=\"image/png\" b64=\"/s/a.b64\"> : he picked".to_string()),
        (2, 2000, "sb answered : docs : v1 or v2? : v2 : the brief".to_string()),
        // his answer to an item (designer m_14030's red row: 'you answered
        // gift-ui' then the marker)
        (3, 3000, "sb route : you → @gift-ui (answer to card #7) : the cart page\\n\\n<image name=\"[Image #1]\" path=\"/Users/ana/shot.png\" mime=\"image/png\" b64=\"/s/a.b64\">".to_string()),
    ];
    let ctx = Ctx { attached: &split, ..ctx_with(&[], &|_| None) };
    let e = fold(&ls, &ctx);
    let a = e[0].answered.as_ref().unwrap();
    assert_eq!((e[0].text.as_str(), a.answer.as_str(), a.why.as_str()), ("the cart page", "the cart page", "he picked"));
    assert_eq!(a.images, [ImageRef { name: "[Image #1]".into(), path: "/Users/ana/shot.png".into() }]);
    assert_eq!(a.files, ["/w/notes.md"]);
    let json = serde_json::to_string(&e[0]).unwrap();
    assert!(!json.contains("<image") && !json.contains("b64"), "{json}");
    // a plain answer: no images or files on the wire
    let b = e[1].answered.as_ref().unwrap();
    assert_eq!((b.answer.as_str(), b.images.len(), b.files.len()), ("v2", 0, 0));
    assert!(!serde_json::to_string(b).unwrap().contains("images"));
    let r = e[2].approval.as_ref().unwrap();
    assert_eq!((r.text.as_str(), r.note.as_str()), ("you answered gift-ui: the cart page", ""));
    assert_eq!((r.images.len(), r.files.as_slice()), (1, ["/w/notes.md".to_string()].as_slice()));
    let json = serde_json::to_string(&e[2]).unwrap();
    assert!(!json.contains("<image") && !json.contains("b64"), "{json}");
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
    assert_eq!((c.kind.as_deref(), c.agent.as_deref()), (Some("question"), Some("perf")));
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
        (1, 1001, "  obs: tool_started #1".to_string()),
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
    assert_eq!((c.kind.as_deref(), c.agent.as_deref()), (Some("question"), Some("gift-ui")));
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

/// Law (architect m_16499): a you-id line gives its id to the 'you' line
/// right before it (his context line may sit between them, the hub writes
/// it there); a stray one attaches to nothing and makes no entry; an
/// older transcript (no you-id) gives None, nothing guessed.
#[test]
fn a_you_id_line_belongs_to_the_you_line_right_before_it() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let l = |pos: u64, line: &str| (pos, pos, line.to_string());
    let lines = vec![
        l(1, "sb you : first"),
        l(2, "sb you-id : m_3"),
        l(3, "sb you : with his screen"),
        l(4, r#"sb context : {"app":"Safari"}"#),
        l(5, "sb you-id : m_4"),
        l(6, "  obs: assistant: looking"),
        l(7, "sb you-id : m_9"),
        l(8, "sb you : an older one"),
        l(9, "  obs: turn_started"),
        l(10, "sb you-id : m_5"),
        l(11, "sb you : bad id"),
        l(12, "sb you-id : 7"),
    ];
    let e = fold(&lines, &ctx);
    let got: Vec<(&str, Option<u64>)> = e.iter().map(|e| (e.text.as_str(), e.msg_id)).collect();
    assert_eq!(got.len(), 6, "{got:?}");
    assert_eq!(&got[..5], [("first", Some(3)), ("with his screen", Some(4)), ("looking", None), ("an older one", None), ("bad id", None)]);
    assert_eq!(e[1].context.as_ref().and_then(|c| c.app.as_deref()), Some("Safari"));
    // a malformed id line is an older kind's line: its words, as any other
    assert_eq!(e[5].kind, EntryKind::Notice);
    assert!(e.iter().all(|e| e.payload_matches_kind()), "{e:?}");
}

/// Laws (architect m_16497/m_16499): a steered receipt marks the entries
/// whose message ids it names with its own pos (after theirs), all of a
/// bundled receipt's the same; his by msg_id, an agent's by msg; an entry
/// keeps its first mark; ids it never showed, nothing; no entry of its
/// own. An older transcript (no receipt): steered_at None.
#[test]
fn a_steered_receipt_marks_the_messages_it_names() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let l = |pos: u64, line: &str| (pos, pos, line.to_string());
    let lines = vec![
        l(1, "sb you : go"),
        l(2, "sb you-id : m_1"),
        l(3, "  obs: turn_started"),
        l(4, "  obs: tool_started #1"),
        l(4, "tool #1 bash : cargo test"),
        l(5, "sb you : use nextest"),
        l(6, "sb you-id : m_2"),
        l(7, "sb you : and -q"),
        l(8, "sb you-id : m_3"),
        l(9, "sb msg-in : docs m_4 : ping"),
        l(10, "tool_result #1 ok : done"),
        l(11, "sb steered : m_2 m_3 m_4 m_77"),
        l(12, "sb you : later"),
        l(13, "sb you-id : m_5"),
        l(14, "sb steered : m_5 m_2"),
        l(15, "  obs: turn_done: completed"),
    ];
    let e = fold(&lines, &ctx);
    let by = |t: &str| e.iter().find(|e| e.text == t).unwrap_or_else(|| panic!("{t}: {e:?}"));
    assert_eq!(by("go").steered_at, None);
    assert_eq!(by("use nextest").steered_at, Some(11), "bundled: one pos for all");
    assert_eq!(by("and -q").steered_at, Some(11));
    assert_eq!(by("ping").steered_at, Some(11));
    assert_eq!(by("later").steered_at, Some(14));
    assert!(e.iter().all(|e| e.steered_at.is_none_or(|s| s > e.pos)), "{e:?}");
    assert!(e.iter().all(|e| e.kind != EntryKind::Notice), "the receipts make no entry: {e:?}");
    assert!(e.iter().all(|e| e.payload_matches_kind()), "{e:?}");
}

/// Laws (architect m_16963): his message's delivery comes from sb-core's
/// receipts by id: `sb steer-rx` received (✓), `sb steered` read (✓✓);
/// a message with the same words but another id stays sent; steer-rx
/// makes no entry; a page cut before the receipts reads them ahead.
#[test]
fn the_receipts_set_his_messages_delivery_by_id() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let l = |pos: u64, line: &str| (pos, pos, line.to_string());
    let all = vec![
        l(1, "sb you : go"),
        l(2, "sb you-id : m_1"),
        l(3, "  obs: turn_started"),
        l(4, "  obs: tool_started #1"),
        l(4, "tool #1 bash : sleep 8"),
        l(5, "sb you : again"),
        l(6, "sb you-id : m_2"),
        l(7, "sb you : again"),
        l(8, "sb you-id : m_3"),
        l(9, "sb steer-rx : m_2"),
        l(10, "tool_result #1 ok : done"),
    ];
    let by_id = |e: &[Entry], id: u64| e.iter().find(|x| x.msg_id == Some(id)).map(|x| x.delivery).unwrap_or_else(|| panic!("m_{id}: {e:?}"));
    let e = fold(&all, &ctx);
    assert_eq!(by_id(&e, 2), Some(Delivery::Received), "steer-rx: ✓");
    assert_eq!(by_id(&e, 3), Some(Delivery::Sent), "the same words, another id: still sent");
    assert!(e.iter().all(|x| x.kind != EntryKind::Notice), "steer-rx makes no entry: {e:?}");
    let mut more = all.clone();
    more.push(l(11, "sb steered : m_2"));
    let e = fold(&more, &ctx);
    assert_eq!(by_id(&e, 2), Some(Delivery::Read), "steered: ✓✓");
    assert_eq!(by_id(&e, 3), Some(Delivery::Sent));
    // a page cut before both receipts: the same marks from the lines ahead
    more.push(l(12, "  obs: turn_done: completed"));
    let cut = more.iter().position(|x| x.0 == 9).unwrap();
    let (p, _, _) = page(&more[..cut], &more[cut..], &ctx, 60);
    assert_eq!(by_id(&p, 2), Some(Delivery::Read));
    assert_eq!(by_id(&p, 3), Some(Delivery::Sent));
}

#[test]
fn a_page_keeps_the_newest_and_says_what_is_before() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let (e, before, more) = page(&lines(), &[], &ctx, 60);
    assert_eq!((e.len(), before, more), (7, None, false));
    let (e, before, more) = page(&lines(), &[], &ctx, 2);
    assert_eq!((e.len(), before, more), (2, Some(13), true));
    // a page that doesn't start the thread: its first entry may be cut
    let (e, before, more) = page(&lines()[3..13], &[], &ctx, 60);
    assert_eq!(e[0].kind, EntryKind::FromAgent);
    assert_eq!((before, more), (Some(10), true));
}

/// Law (architect m_16543): a page cut between a steered message and its
/// receipt still shows steered_at: the page reads the lines after it up to
/// its last turn's end; a receipt after that end marks nothing (it can't
/// name a message of this page), and the page equals the whole fold.
#[test]
fn a_page_cut_before_the_receipt_still_shows_the_steer() {
    let none = |_: &str| None;
    let ctx = ctx_with(&[], &none);
    let l = |pos: u64, line: &str| (pos, pos, line.to_string());
    let all = vec![
        l(1, "sb you : go"),
        l(2, "sb you-id : m_1"),
        l(3, "  obs: turn_started"),
        l(4, "  obs: tool_started #1"),
        l(4, "tool #1 bash : sleep 8"),
        l(5, "sb you : and nextest"),
        l(6, "sb you-id : m_2"),
        // the page ends here: the receipt is in the next page
        l(7, "tool_result #1 ok : done"),
        l(8, "sb steered : m_2"),
        l(9, "  obs: assistant: switching"),
        l(10, "  obs: turn_done: completed"),
        l(11, "sb steered : m_1"),
    ];
    let cut = all.iter().position(|x| x.0 == 7).unwrap();
    let (e, _, _) = page(&all[..cut], &all[cut..], &ctx, 60);
    let by = |t: &str| e.iter().find(|x| x.text == t).unwrap_or_else(|| panic!("{t}: {e:?}"));
    assert_eq!(by("and nextest").steered_at, Some(8));
    assert_eq!(by("go").steered_at, None, "a receipt after the turn's end marks nothing");
    let whole = fold(&all[..10], &ctx);
    assert_eq!(whole.iter().find(|x| x.text == "and nextest").and_then(|x| x.steered_at), Some(8), "the same as the whole fold");
    // without the lines ahead (the old page): nothing guessed
    let (e, _, _) = page(&all[..cut], &[], &ctx, 60);
    assert!(e.iter().all(|x| x.steered_at.is_none()), "{e:?}");
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
    // R12: the two failed turns are turn_failed entries, not notices
    assert_eq!(kinds, [You, Thinking, Agent, Compacting, Compacted, Notice, TurnFailed, Notice, TurnFailed, Notice, NotDelivered, Agent]);
    let t = e[1].thinking.as_ref().unwrap();
    assert_eq!((t.ms, t.text.as_str(), e[1].text.as_str()), (3_200, "cold\nor warm", "thought for 3.2s"), "the time since the line before");
    assert_eq!(e[2].text, "on it");
    assert_eq!(e[4].text, "we profiled");
    let n = |i: usize| e[i].notice.clone().map(|n| (n.level, n.text)).unwrap();
    use crate::thread::NoticeLevel::*;
    assert_eq!(n(5), (Warn, "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s".into()));
    let f = |i: usize| (e[i].turn_failed.clone().map(|t| t.why).unwrap(), e[i].text.clone());
    assert_eq!(f(6), ("provider 500".into(), "turn failed: provider 500".into()));
    assert_eq!(n(7), (Warn, "turn interrupted by main".into()), "a stop someone asked for, not a failure");
    assert_eq!(f(8).1, "turn stopped: no OPENROUTER key yet. /provider sets it up.", "the provider's name from the ctx");
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
        l(4, "  obs: tool_started #1"),
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
    // a message he routed by hand (no item) is the TUI's info line
    let hand = fold(&[l(1, "sb route : you → @docs : thanks")], &ctx_with(&[], &page));
    assert_eq!(hand.iter().map(|e| (e.kind, e.text.as_str())).collect::<Vec<_>>(), [(EntryKind::Notice, "→ you → @docs : thanks")]);
    // an answer to an item says which one (a client that folded it itself skips it); a gate's fold says none
    assert_eq!((ap(6).card, ap(7).card, ap(8).card), (None, None, Some(4)));
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
    // sched-names (designer m_14531): its name, never its id; these lines
    // have none (an older hub's): the plain fallback of its words
    assert!(s[0].head.starts_with("main scheduled check the build for perf · every 2m · 6 times · next "), "{}", s[0].head);
    assert_eq!(s[1].head, "check the build · 1 of 6");
    assert_eq!((s[2].head.as_str(), s[2].words.as_str()), ("check the build ended · stopped by you", ""));
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

/// R11 (amb-win S9, architect m_13028): a tool item carries its state
/// (run until its result, then ok or err), its duration from the two
/// lines' times, a failed bash's exit code and first error line, its
/// full code and output capped, and an edit's files with their counts.
#[test]
fn a_tool_item_carries_its_state_time_exit_error_code_output_and_files() {
    let none = |_: &str| None;
    let ls = vec![
        (1, 1_000, "  obs: tool_started #1".to_string()),
        (2, 1_000, "tool #1 bash : cargo test -q".to_string()),
        (3, 1_001, "tool_code #1 : cargo test -q\\N  --lib".to_string()),
        (4, 1_400, "  obs: tool_finished #1 fail".to_string()),
        (5, 1_400, "tool_result #1 fail : exit 101: thread 'x' panicked at src/a.rs:3".to_string()),
        (6, 2_000, "  obs: tool_started #2".to_string()),
        (7, 2_000, "tool #2 apply_patch : {}".to_string()),
        (8, 2_001, "tool_code #2 : *** Begin Patch\\N*** Update File: src/a.rs\\N+one\\N+two\\N-old\\N*** End Patch".to_string()),
        (9, 2_050, "  obs: tool_finished #2 ok".to_string()),
        (10, 2_050, "tool_result #2 ok : done".to_string()),
        (11, 3_000, "history   obs: tool_started #3".to_string()),
        (12, 3_000, "history tool #3 read_file : {\"path\":\"b.rs\"}".to_string()),
        (13, 3_500, "history   obs: tool_finished #3 ok".to_string()),
        (14, 3_500, "history tool_result #3 ok : fn b() {}".to_string()),
        (15, 4_000, "  obs: tool_started #4".to_string()),
        (16, 4_000, "tool #4 bash : ls".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let items = &e[0].tools.as_ref().expect("one tools entry").items;
    let bash = &items[0];
    assert_eq!((bash.state, bash.ms, bash.exit), (ToolState::Err, Some(400), Some(101)));
    assert_eq!(bash.err.as_deref(), Some("panicked at src/a.rs:3"));
    assert_eq!(bash.code.as_deref(), Some("cargo test -q\n  --lib"));
    assert_eq!(bash.out.as_deref(), Some("exit 101: thread 'x' panicked at src/a.rs:3"));
    let patch = &items[1];
    assert_eq!((patch.state, patch.ms, patch.exit, patch.err.as_deref()), (ToolState::Ok, Some(50), None, None));
    assert_eq!(patch.files, vec![FileCount { path: "src/a.rs".into(), add: 2, del: 1 }]);
    // a replayed result has no duration
    assert_eq!((items[2].state, items[2].ms), (ToolState::Ok, None));
    assert!(items[2].files.is_empty(), "only an edit counts files");
    // no result yet: running
    assert_eq!((items[3].state, items[3].ms, items[3].out.as_deref()), (ToolState::Run, None, None));
}

/// Law (architect m_13242): code and out are capped at TOOL_TEXT_CAP
/// bytes, cut on a char boundary, with '…' after a cut.
#[test]
fn a_tool_text_is_capped_on_a_char_boundary() {
    use crate::thread::{cap, TOOL_TEXT_CAP};
    assert_eq!(cap("short"), "short");
    let exact = "a".repeat(TOOL_TEXT_CAP);
    assert_eq!(cap(&exact), exact);
    let long = format!("{}é tail", "a".repeat(TOOL_TEXT_CAP - 1));
    let c = cap(&long);
    assert!(c.ends_with('…') && c.len() <= TOOL_TEXT_CAP + '…'.len_utf8(), "{}", c.len());
    assert_eq!(c.trim_end_matches('…'), "a".repeat(TOOL_TEXT_CAP - 1), "é straddles the cap: cut before it");
    for n in [1usize, 2, 3, 4, 5] {
        let s = "€".repeat(TOOL_TEXT_CAP / 3 + n);
        let c = cap(&s);
        assert!(c.trim_end_matches('…').chars().all(|ch| ch == '€'));
    }
}

/// R12 (architect m_13028 option A): a failed turn is a turn_failed
/// entry (why, as the runtime said it; the TUI's words as its text); an
/// interrupt is not one; an item without a state reads as Unknown, never
/// as running (architect m_13326).
#[test]
fn a_failed_turn_is_its_own_entry() {
    let none = |_: &str| None;
    let ls = vec![
        (1, 1_000, "  obs: turn_done: failed: provider 500".to_string()),
        (2, 2_000, "  obs: turn_done: failed: interrupted by main".to_string()),
        (3, 3_000, "  obs: turn_done: completed".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    assert_eq!(e[0].kind, EntryKind::TurnFailed);
    assert_eq!(e[0].text, "turn failed: provider 500");
    assert_eq!(e[0].turn_failed, Some(TurnFailed { why: "provider 500".into() }));
    assert!(e[0].payload_matches_kind());
    assert!(e.iter().skip(1).all(|x| x.kind != EntryKind::TurnFailed), "an interrupt is not a failed turn: {e:?}");
    let old: ToolItem = serde_json::from_value(serde_json::json!({"pos": 1, "at_ms": 0, "text": "ls", "kind": "run"})).unwrap();
    assert_eq!(old.state, ToolState::Unknown);
}

/// G1 (architect m_14145): his message's mark by the TUI's one rule
/// (lines::deliver): sent, received on steering, read when steered or
/// when a turn starts after it, failed when the hub couldn't deliver it;
/// only ever up.
#[test]
fn his_messages_carry_their_delivery_mark() {
    use crate::thread::Delivery::*;
    let none = |_: &str| None;
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let marks = |ls: &[(u64, u64, String)]| -> Vec<_> { fold(ls, &ctx_with(&[], &none)).iter().filter(|e| e.kind == EntryKind::You).map(|e| e.delivery).collect() };
    let ls = vec![
        l(1, "sb you : first"),
        l(2, "  obs: turn_started"),
        l(3, "  obs: assistant: on it"),
        l(4, "sb you : and the   logs"),
        l(5, "  obs: steering_received: and the logs"),
        l(6, "sb you : third"),
        l(7, "sb you : lost one"),
        l(8, "sb undelivered : perf : lost one"),
    ];
    assert_eq!(marks(&ls[..1]), [Some(Sent)]);
    assert_eq!(marks(&ls), [Some(Read), Some(Received), Some(Sent), Some(Failed)]);
    // steered: read; a turn that starts reads what was sent since the last
    let mut more = ls.clone();
    more.push(l(9, "  obs: steered: and the logs"));
    assert_eq!(marks(&more), [Some(Read), Some(Read), Some(Sent), Some(Failed)]);
    more.push(l(10, "  obs: turn_started"));
    assert_eq!(marks(&more), [Some(Read), Some(Read), Some(Read), Some(Failed)], "failed stays");
    // BISE-90: a steered block with other words raises this turn's
    let ls = vec![l(1, "sb you : a"), l(2, "  obs: turn_started"), l(3, "sb you : b"), l(4, "  obs: steering_received: <agent_message from=x>")];
    assert_eq!(marks(&ls), [Some(Read), Some(Received)]);
    let e = fold(&ls, &ctx_with(&[], &none));
    assert!(e.iter().all(|x| x.payload_matches_kind()), "{e:?}");
}

/// G3 (architect m_14145): a tool row exists from its tool_started (the
/// TUI's rule); a call line without one adds nothing; the item keeps its
/// name, args and intent apart from its text; a report's call
/// leaves no item; a finish with no running row is an ended row.
#[test]
fn a_tool_row_starts_with_tool_started() {
    let none = |_: &str| None;
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let ls = vec![
        l(1, "tool #9 bash : ls"),
        l(2, "  obs: tool_started #1"),
        l(3, "tool #1 bash : cargo test -q"),
        l(4, "tool_intent #1 : running the tests"),
        l(5, "  obs: tool_finished #1 ok"),
        l(6, "  obs: tool_started #2"),
        l(7, "tool #2 bash : sb report done \"ok\""),
        l(8, "  obs: tool_started #3"),
        l(9, "  obs: tool_finished #4 fail"),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let kinds: Vec<EntryKind> = e.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, [EntryKind::Tools, EntryKind::Report, EntryKind::Tools], "{e:?}");
    let items = &e[0].tools.as_ref().unwrap().items;
    assert_eq!(items.len(), 1, "no row for #9, none left for the report's #2");
    let it = &items[0];
    assert_eq!((e[0].pos, it.name.as_str(), it.args.as_str(), it.intent.as_deref(), it.text.as_str()), (2, "bash", "cargo test -q", Some("running the tests"), "running the tests"));
    assert_eq!((it.state, it.ms), (ToolState::Ok, Some(3)));
    let later = &e[2].tools.as_ref().unwrap().items;
    assert_eq!(later.iter().map(|i| (i.state, i.name.as_str())).collect::<Vec<_>>(), [(ToolState::Run, ""), (ToolState::Err, "")]);
    let j = serde_json::to_value(&later[0]).unwrap();
    assert!(j.get("name").is_none() && j.get("args").is_none() && j.get("intent").is_none(), "empty left out: {j}");
    assert_eq!(e[2].tools.as_ref().unwrap().count, 2);
}

/// G4/G5 (architect m_14145, m_14424): a message in carries its id; one
/// written to him keeps the kind it always folded to (msg-you: agent, an
/// old direct reply from `@name`: from_agent), says to_you and who.
#[test]
fn a_message_to_him_says_to_you() {
    let none = |_: &str| None;
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let ls = vec![
        l(1, "sb msg-in : ambient-lead m_3 : nice"),
        l(2, "sb msg-in : @docs m_4 : done here"),
        l(3, "sb msg-you : switchboard : your keys"),
        l(4, "next line"),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let got: Vec<_> = e.iter().map(|x| (x.kind, x.to_you, x.from.as_deref(), x.msg, x.text.as_str())).collect();
    use EntryKind::*;
    assert_eq!(got, [(FromAgent, false, Some("ambient-lead"), Some(3), "nice"), (FromAgent, true, Some("docs"), Some(4), "done here"), (Agent, true, Some("bise"), None, "your keys\nnext line")]);
    let j = serde_json::to_value(&e[0]).unwrap();
    assert!(j.get("to_you").is_none(), "false is left out: {j}");
    assert!(e.iter().all(|x| x.payload_matches_kind() && x.to.is_none()), "{e:?}");
}

/// Law (architect m_15013): a card entry says the kind and the asker the
/// hub's card row says, for every card of the hub's fixtures: the line
/// is core.bend's `#{id} {kind} @{agent} : {text}` written from that
/// card, and `rows::card_rank` reads the entry's kind as the row's.
#[test]
fn a_card_entry_says_its_rows_kind_and_asker() {
    let cards: Vec<crate::rows::Card> = include_str!("../../fixtures/hub_ev.jsonl")
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["ev"] == "cards")
        .flat_map(|v| serde_json::from_value::<Vec<crate::rows::Card>>(v["cards"].clone()).unwrap_or_default())
        .collect();
    assert!(cards.len() >= 3, "{cards:?}");
    let none = |_: &str| None;
    for c in cards.iter().filter(|c| c.kind != "confirm") {
        let opts: String = c.options.iter().map(|o| format!("\\n{}. {}", o.n, o.label)).collect();
        let line = format!("sb card : #{} {} @{} : {}{opts}", c.id, c.kind, c.agent, c.question);
        let e = fold(&[(1, 1001, line)], &ctx_with(&[c.id], &none));
        let got = e[0].card.as_ref().expect("a card entry");
        assert_eq!((got.kind.as_deref(), got.agent.as_deref()), (Some(c.kind.as_str()), Some(c.agent.as_str())), "{c:?}");
        assert_eq!(crate::rows::card_rank(got.kind.as_deref().unwrap_or_default()), crate::rows::card_rank(&c.kind));
        // a list of one is no choice (rows::split_choices): its line stays in the words
        if c.options.len() > 1 {
            assert_eq!((got.question.as_str(), got.options.len()), (c.question.as_str(), c.options.len()), "{c:?}");
        }
    }
}

/// The hub's plain lines ('sb info : @t1 archived · its worktree
/// removed', any kind this reader doesn't know) are info notices with
/// their words, as the TUI's line path drew them (Ev::Info); a landed
/// line that doesn't parse says nothing.
#[test]
fn a_hub_info_line_is_an_info_notice() {
    let none = |_: &str| None;
    let ls = vec![
        (1, 1_000, "sb info : @t1 archived · its worktree removed".to_string()),
        (2, 2_000, "sb something-new : a newer hub's words".to_string()),
        (3, 3_000, "sb landed : garbled".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let got: Vec<_> = e.iter().map(|x| (x.pos, x.kind, x.notice.clone().map(|n| (n.level, n.text)))).collect();
    use crate::thread::NoticeLevel::Info;
    assert_eq!(
        got,
        [
            (1, EntryKind::Notice, Some((Info, "@t1 archived · its worktree removed".to_string()))),
            (2, EntryKind::Notice, Some((Info, "a newer hub's words".to_string()))),
        ]
    );
}

/// BISE-271 from entries (proto-lead m_14961, architect m_14963): a
/// turn's first entry says it starts one, its newest entry at its end
/// keeps the end's time, whatever the end (completed, interrupted, failed:
/// the turn_failed entry itself); a replayed end has no time and sets
/// nothing; a turn with no entry of its own never puts its end on an
/// earlier turn's entry, so a reply's turn end is always its own.
#[test]
fn a_turns_start_and_end_time_ride_on_its_entries() {
    let none = |_: &str| None;
    let l = |pos: u64, ms: u64, line: &str| (pos, ms, line.to_string());
    let ls = vec![
        l(1, 1_000, "sb you : go"),
        l(2, 1_100, "  obs: turn_started"),
        l(3, 1_200, "  obs: assistant: done"),
        l(4, 1_300, "  obs: turn_done: completed"),
        // interrupted: the notice is the newest entry
        l(5, 2_100, "  obs: turn_started"),
        l(6, 2_200, "  obs: assistant: half"),
        l(7, 2_300, "  obs: turn_done: failed: interrupted by main"),
        // failed: its own entry carries the end
        l(8, 3_100, "  obs: turn_started"),
        l(9, 3_200, "  obs: turn_done: failed: provider 500"),
        // replayed: no time, no end
        l(10, 4_100, "  obs: turn_started"),
        l(11, 4_200, "  obs: assistant: old"),
        l(12, 4_300, "history   obs: turn_done: completed"),
        // a turn with no entry of its own: 'old' keeps no end of it
        l(13, 5_100, "  obs: turn_started"),
        l(14, 5_300, "  obs: turn_done: completed"),
        // and the next turn's end stays on the next turn's entry
        l(15, 6_100, "  obs: turn_started"),
        l(16, 6_200, "  obs: assistant: new"),
        l(17, 6_300, "  obs: turn_done: completed"),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let got: Vec<_> = e.iter().map(|x| (x.pos, x.kind, x.turn_start, x.turn_end_ms)).collect();
    use EntryKind::*;
    assert_eq!(
        got,
        [
            (1, You, false, None),
            (3, Agent, true, Some(1_300)),
            (6, Agent, true, None),
            (7, Notice, false, Some(2_300)),
            (9, TurnFailed, true, Some(3_200)),
            (11, Agent, true, None),
            (16, Agent, true, Some(6_300)),
        ]
    );
    let j = serde_json::to_value(&e[0]).unwrap();
    assert!(j.get("turn_start").is_none() && j.get("turn_end_ms").is_none(), "false and none are left out: {j}");
    // a page that starts mid-turn: its end still lands, no start drawn
    let mid = fold(&ls[2..4], &ctx_with(&[], &none));
    assert_eq!(mid.iter().map(|x| (x.turn_start, x.turn_end_ms)).collect::<Vec<_>>(), [(false, Some(1_300))]);
}

/// Law (proto-lead m_15137, BISE-31): a card's closing line gives its
/// entry the hub's word (and answered), the entry it changes is the
/// newest card with that id, and a closing line for a card this thread
/// never showed changes nothing and makes no entry, as the TUI's line
/// path (the card fades with the word, `closed_word` reads it).
#[test]
fn a_closed_card_says_the_hubs_word() {
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let none = |_: &str| None;
    let ls = vec![
        l(1, "sb card : #9 question @perf : which bench?\\n1. cold\\n2. warm"),
        l(2, "sb card : #4 drop @mig-db : drop the v1 tables now?"),
        l(3, "sb card-closed : #4 accepted"),
        l(4, "sb card-closed : #77 closed"),
    ];
    let e = fold(&ls, &ctx_with(&[9, 4], &none));
    assert_eq!(e.len(), 2, "a closing line is no entry: {e:?}");
    let c = |i: usize| e[i].card.clone().unwrap();
    assert_eq!((c(0).closed, c(0).answered), (None, false), "still open");
    assert_eq!((c(1).closed.as_deref(), c(1).answered), (Some("accepted"), true));
    let j = serde_json::to_value(&e[0]).unwrap();
    assert!(j["card"].get("closed").is_none(), "none is left out: {j}");
}

/// F2 (tui-parity m_15345, architect m_15394): a transcript writes a
/// call's `tool_result` before its `tool_finished`: the finish completes
/// the item the result ended (its state and duration), never a row of its
/// own (the stray `#1 ✓ 0.0s`). A finish with no item of its id still is
/// one.
#[test]
fn a_finish_after_its_result_completes_the_same_item() {
    let none = |_: &str| None;
    let ls = vec![
        (1, 1_000, "  obs: tool_started #3".to_string()),
        (2, 1_000, "tool #3 bash : ls".to_string()),
        (3, 1_001, "tool_code #3 : ls".to_string()),
        (4, 1_200, "tool_result #3 ok : a.rs".to_string()),
        (5, 1_250, "  obs: tool_finished #3 ok".to_string()),
        (6, 2_000, "  obs: tool_started #1".to_string()),
        (7, 2_000, "tool #1 bash : git push origin main --force".to_string()),
        (8, 4_100, "tool_result #1 fail : exit 128: fatal: 'origin' does not appear to be a git repository".to_string()),
        (9, 4_150, "  obs: tool_finished #1 fail".to_string()),
        (10, 5_000, "  obs: tool_finished #7 ok".to_string()),
    ];
    let e = fold(&ls, &ctx_with(&[], &none));
    let items = &e[0].tools.as_ref().expect("one tools entry").items;
    let rows: Vec<(u64, &str, ToolState, Option<u64>)> = items.iter().map(|i| (i.id, i.name.as_str(), i.state, i.ms)).collect();
    assert_eq!(rows, [(3, "bash", ToolState::Ok, Some(250)), (1, "bash", ToolState::Err, Some(2_150)), (7, "", ToolState::Ok, None)], "{items:?}");
    assert_eq!((items[1].exit, items[1].err.as_deref()), (Some(128), Some("fatal: 'origin' does not appear to be a git repository")));
    assert_eq!(items[0].out.as_deref(), Some("a.rs"));
}

/// F2: the approvals gate's lines hold the running call (the newest, the
/// TUI's rule) since the line's time; `done` and the call's end let it go.
#[test]
fn the_gate_holds_the_running_call_until_done_or_its_end() {
    let none = |_: &str| None;
    let at = |ls: &[Line]| {
        let e = fold(ls, &ctx_with(&[], &none));
        e.iter().flat_map(|x| x.tools.iter().flat_map(|t| t.items.iter().map(|i| (i.id, i.gate)))).collect::<Vec<_>>()
    };
    let mut ls: Vec<Line> = vec![
        (1, 1_000, "  obs: tool_started #1".to_string()),
        (2, 1_000, "tool #1 bash : ls".to_string()),
        (3, 1_100, "  obs: tool_finished #1 ok".to_string()),
        (4, 2_000, "  obs: tool_started #2".to_string()),
        (5, 2_000, "tool #2 bash : git push origin main --force".to_string()),
        (6, 2_010, "sb gate : check 2".to_string()),
    ];
    assert_eq!(at(&ls), [(1, None), (2, Some(ToolGate { wait: GateWait::Check, at_ms: 2_010 }))]);
    ls.push((7, 2_300, "sb gate : card 5".to_string()));
    assert_eq!(at(&ls)[1], (2, Some(ToolGate { wait: GateWait::Card, at_ms: 2_300 })));
    let mut done = ls.clone();
    done.push((8, 9_000, "sb gate : done 2".to_string()));
    assert_eq!(at(&done)[1], (2, None));
    ls.push((8, 9_000, "tool_result #2 fail : exit 128: fatal: no".to_string()));
    ls.push((9, 9_001, "  obs: tool_finished #2 fail".to_string()));
    assert_eq!(at(&ls)[1], (2, None), "an ended call waits on nothing");
    let j = serde_json::to_value(ToolGate { wait: GateWait::Card, at_ms: 7 }).unwrap();
    assert_eq!(j, serde_json::json!({"wait": "card", "at_ms": 7}));
    let later: ToolGate = serde_json::from_value(serde_json::json!({"wait": "vote", "at_ms": 7})).unwrap();
    assert_eq!(later.wait, GateWait::Unknown, "a newer hub's wait");
}

/// tui-parity m_15380, F3 (architect m_15394, proto-lead m_15384): a
/// thinking counts from the previous line the feed shows (here the turn's
/// start, a tool's finish), never from a hidden one: the usage line the
/// runtime writes just before the reply, receipts, plumbing, facts.
#[test]
fn a_thinking_time_counts_from_the_last_shown_line() {
    let none = |_: &str| None;
    let think = |ls: &[(u64, u64, &str)]| {
        let ls: Vec<Line> = ls.iter().map(|&(p, t, l)| (p, t, l.to_string())).collect();
        fold(&ls, &ctx_with(&[], &none)).iter().find_map(|e| e.thinking.as_ref().map(|t| t.ms))
    };
    let reply = "  obs: assistant: <think>weighing the options</think>done";
    // tui-parity's transcript: usage 152 ms before the reply
    assert_eq!(think(&[(1, 104_443, "  obs: turn_started"), (2, 106_622, "  obs: usage: model=m in=1 out=2 cache_read=0 cache_write=0"), (3, 106_774, reply)]), Some(2_331));
    for hidden in [
        "  obs: usage: model=m in=1 out=2 cache_read=0 cache_write=0",
        "  obs: tool_result_committed #1",
        "  obs: steering_received: m_3",
        "  obs: steered: m_3",
        "  obs: notification_received: m_4",
        "  obs: notification_delivered: m_4",
        "  ev: anything",
        "--- idle",
    ] {
        let ls = [(1, 1_000, "  obs: tool_started #1"), (2, 1_000, "tool #1 bash : ls"), (3, 1_500, "  obs: tool_finished #1 ok"), (4, 2_900, hidden), (5, 3_000, reply)];
        assert_eq!(think(&ls), Some(1_500), "{hidden} moved the start");
    }
    // a shown line does move it: a provider retry's notice
    assert_eq!(think(&[(1, 1_000, "  obs: turn_started"), (2, 2_500, "  obs: provider_retry: 1/10 · provider 529 (transient) · retry in 2s"), (3, 3_000, reply)]), Some(500));
}

/// tui-parity R1 (proto-lead m_15643): a live tail starts where it folds
/// its entries as the whole did: never between a call's start and a line
/// of it after the cut (a card line written while `sb card` runs), nor
/// while the call is still open; a finish or the turn's end lets it go.
#[test]
fn a_tail_never_starts_between_a_call_and_its_lines() {
    let none = |_: &str| None;
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    let mut ls = vec![
        l(1, "sb you : ask"),
        l(2, "  obs: turn_started"),
        l(3, "  obs: tool_started #2"),
        l(4, "tool #2 bash : sb card \"second?\""),
        l(5, "sb card : #2 question @main : second?"),
        l(6, "sb info : one"),
    ];
    let at = |ls: &[Line]| {
        let e = fold(ls, &ctx_with(&[], &none));
        let card = e.iter().position(|x| x.kind == EntryKind::Card).unwrap();
        tail_start(ls, &e, card)
    };
    assert_eq!(at(&ls), 3, "the call is open: its row's entry stays");
    ls.push(l(7, "tool_result #2 ok : #2"));
    assert_eq!(at(&ls), 3, "its result is after the cut");
    ls.push(l(8, "  obs: tool_finished #2 ok"));
    assert_eq!(at(&ls), 3, "its finish too");
    let tail: Vec<Line> = ls.iter().filter(|x| x.0 >= 3).cloned().collect();
    let kinds = |ls: &[Line]| fold(ls, &ctx_with(&[], &none)).iter().map(|e| (e.pos, e.kind)).collect::<Vec<_>>();
    assert_eq!(kinds(&tail), kinds(&ls)[1..], "the tail folds as the whole did");
    // the gate's card: its fold (`sb approval`) is an entry written while
    // the push runs; the push's result and finish come after it
    let push = vec![
        l(1, "  obs: tool_started #5"),
        l(2, "tool #5 bash : git push origin main --force"),
        l(3, "sb gate : check 5"),
        l(4, "sb gate : card 12"),
        l(5, "sb card : #12 confirm @main : git push origin main --force"),
        l(6, "sb approval : allowed : main : git push origin main --force : "),
        l(7, "sb gate : done 5"),
        l(8, "tool_result #5 fail : exit 128: fatal: no"),
        l(9, "  obs: tool_finished #5 fail"),
        l(10, "sb info : one"),
    ];
    let e = fold(&push, &ctx_with(&[], &none));
    let approval = e.iter().position(|x| x.kind == EntryKind::Approval).unwrap();
    assert_eq!(tail_start(&push, &e, approval), 1, "the approval entry doesn't cut the push from its finish");
    assert_eq!(tail_start(&push, &e, approval + 1), 10, "past its finish it may go");
    // a call that ended before the cut doesn't hold it
    let mut done = vec![l(1, "  obs: tool_started #1"), l(2, "tool #1 bash : ls"), l(3, "tool_result #1 ok : a"), l(4, "  obs: tool_finished #1 ok"), l(5, "sb info : one")];
    let e = fold(&done, &ctx_with(&[], &none));
    assert_eq!(tail_start(&done, &e, 1), 5);
    // a turn's end closes a call that never finished (a crash)
    done = vec![l(1, "  obs: tool_started #1"), l(2, "tool #1 bash : ls"), l(3, "  obs: turn_done: failed: boom"), l(4, "sb info : one")];
    let e = fold(&done, &ctx_with(&[], &none));
    let info = e.iter().position(|x| x.pos == 4).unwrap();
    assert_eq!(tail_start(&done, &e, info), 4);
}
