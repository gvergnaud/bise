use super::*;
use crate::rows::ReportKind;
use crate::thread::words::{one_line, summary};

#[test]
fn runtime_lines_read_as_their_kind() {
    assert_eq!(read(""), Rec::Empty);
    assert_eq!(read("--- idle"), Rec::Idle);
    assert_eq!(read("  ev: turn"), Rec::Fact);
    assert_eq!(read("tool #3 bash : cargo test"), Rec::Tool { id: 3, name: "bash".into(), args: "cargo test".into() });
    assert_eq!(read("tool #x bash : y"), Rec::Dropped);
    assert_eq!(read("tool_intent #3 : running the tests "), Rec::ToolIntent { id: 3, text: "running the tests".into() });
    assert_eq!(read("tool_intent #3 :  "), Rec::Dropped);
    assert_eq!(read("tool_code #3 : a\\Nb"), Rec::ToolCode { id: 3, code: "a\\Nb".into() });
    assert_eq!(read("tool_result #3 fail : exit 1"), Rec::ToolResult { id: 3, ok: false, preview: "exit 1".into() });
    assert_eq!(read("subtool slack.send ok : sent"), Rec::Sub { name: "slack.send".into(), ok: true, preview: "sent".into() });
    assert_eq!(read("core rejected: no pending completion"), Rec::Rejected("no pending completion".into()));
    assert_eq!(read("second line"), Rec::Raw("second line".into()));
    assert_eq!(read_history("you : a\\nb"), Rec::HistYou("a\nb".into()));
    assert_eq!(read_history("injected : x"), Rec::Injected("x".into()));
    assert_eq!(read_history("--- idle"), Rec::Idle);
}

#[test]
fn observations_read_as_their_kind() {
    let o = |s: &str| match read(&format!("  obs: {s}")) {
        Rec::Obs(o) => o,
        r => panic!("{r:?}"),
    };
    assert_eq!(o("turn_started"), Obs::TurnStarted);
    assert_eq!(o("assistant: <think>hm</think>ok"), Obs::Assistant("<think>hm</think>ok".into()));
    assert_eq!(o("assistant:"), Obs::Assistant(String::new()));
    assert_eq!(o("tool_started #4"), Obs::ToolStarted(4));
    assert_eq!(o("tool_finished #4 failed"), Obs::ToolFinished { id: 4, ok: false });
    assert_eq!(o("tool_result_committed #4"), Obs::Plumbing);
    assert_eq!(o("compaction_started #2 auto"), Obs::CompactionStarted);
    assert_eq!(o("compaction_done: the summary"), Obs::CompactionDone("the summary".into()));
    assert_eq!(o("context_compaction_failed: x"), Obs::CompactionFailed("x".into()));
    assert_eq!(o("usage: model=m in=1 out=2"), Obs::Usage("model=m in=1 out=2".into()));
    assert_eq!(o("provider_retry: 2/10 · 529 · retry in 4s"), Obs::ProviderRetry("2/10 · 529 · retry in 4s".into()));
    assert_eq!(o("turn_done: completed"), Obs::TurnDone(TurnEnd::Completed));
    assert_eq!(o("turn_done: interrupted"), Obs::TurnDone(TurnEnd::Interrupted { by: None }));
    assert_eq!(o("turn_done: failed: interrupted by main"), Obs::TurnDone(TurnEnd::Interrupted { by: Some("main".into()) }));
    assert_eq!(o("turn_done: failed: 500"), Obs::TurnDone(TurnEnd::Failed("500".into())));
    assert_eq!(o("turn_done: budget"), Obs::TurnDone(TurnEnd::Other("budget".into())));
    assert_eq!(o("turn_stalled: budget"), Obs::TurnStalled("budget".into()));
    assert_eq!(o("null_iteration"), Obs::NullIteration);
    assert_eq!(o("something new"), Obs::Other("something new".into()));
}

#[test]
fn hub_lines_read_their_fields() {
    let h = |s: &str| match read(&format!("sb {s}")) {
        Rec::Hub(h) => h,
        r => panic!("{r:?}"),
    };
    assert_eq!(h("you : a\\nb"), Hub::You("a\nb".into(), None));
    assert_eq!(h("undelivered : fix : d'abord \\: les tests"), Hub::Undelivered { to: "fix".into(), text: "d'abord : les tests".into() });
    assert_eq!(h("msg-in : docs m_3 : hi : there"), Hub::MsgIn { from: "docs".into(), id: "m_3".into(), body: "hi : there".into(), view: None });
    assert_eq!(h("msg : a → b m_9 : hello"), Hub::Msg { from: "a".into(), to: "b".into(), id: "m_9".into(), body: "hello".into() });
    assert_eq!(h("msg : a → b c : hello"), Hub::Msg { from: "a".into(), to: "b c".into(), id: String::new(), body: "hello".into() });
    assert_eq!(h("sent : main : m_9 : 1 : cart \\: or not"), Hub::Sent { to: "main".into(), id: "m_9".into(), ask: true, body: "cart : or not".into() });
    assert_eq!(h("sent : main"), Hub::Other { kind: "sent".into(), text: "main".into() });
    assert_eq!(h("answered : docs : v1 or v2? : v2 : the brief"), Hub::Answered { agent: "docs".into(), question: "v1 or v2?".into(), answer: "v2".into(), why: "the brief".into() });
    assert_eq!(h("card : #3 confirm @api : run it?"), Hub::Card { text: "#3 confirm @api : run it?".into(), id: Some(3), kind: "confirm".into(), body: "run it?".into() });
    assert_eq!(h("gate : check 4"), Hub::Gate(GateStep::Check));
    assert_eq!(h("approval : no : api : rm -rf : not that"), Hub::Approval { how: "no".into(), who: "api".into(), what: "rm -rf".into(), note: "not that".into() });
    assert_eq!(h("card-closed : #3 answered"), Hub::CardClosed { id: 3, res: "answered".into() });
    assert_eq!(h("card-closed : odd"), Hub::Other { kind: "card-closed".into(), text: "odd".into() });
    assert_eq!(h("route : you → @docs (answer to card #4) : v2"), Hub::Route { who: "docs".into(), card: 4, said: "v2".into(), msg: None, pick: None });
    // at-start: typed here, went to docs as message 7 (sb-core's trace); no id: the old text line
    assert_eq!(h(r#"route :: {"sb":"route","who":"docs","said":"fais","msg":7}"#), Hub::Went { to: "docs".into(), said: "fais".into(), msg: 7 });
    assert_eq!(h(r#"route :: {"sb":"route","who":"docs","said":"fais"}"#), Hub::Other { kind: "route".into(), text: "you → @docs : fais".into() });
    assert_eq!(h("pr : red : 412 : https://x/412 : checks fail \\: e2e"), Hub::Pr { state: crate::thread::PrNewsState::Failing, number: 412, url: "https://x/412".into(), text: "checks fail : e2e".into() });
    // no number: its text alone, an info line (thread::legacy)
    assert_eq!(h("pr : plain : x : u : hi"), Hub::Other { kind: "info".into(), text: "hi".into() });
    assert_eq!(h("artifact : q3 : designer : Q3 plan : page : v2"), Hub::Artifact { id: "q3".into(), agent: "designer".into(), title: "Q3 plan".into(), kind: "page".into(), v: 2 });
    assert_eq!(h("landed : api : main : sb/api : a1b2c3d : 3 : 42 : 18"), Hub::Landed { agent: "api".into(), target: "main".into(), from: "sb/api".into(), sha: "a1b2c3d".into(), files: 3, add: 42, del: 18 });
    assert_eq!(h("landed : api : main : sb/api :  : 3"), Hub::Other { kind: "landed".into(), text: "api : main : sb/api :  : 3".into() });
    assert_eq!(h("stopped : stopped"), Hub::Stopped("stopped".into()));
    assert_eq!(h("whatever : x"), Hub::Other { kind: "whatever".into(), text: "x".into() });
    assert_eq!(h("you-id : m_12"), Hub::YouId(12));
    assert_eq!(h("you-id : 12"), Hub::Other { kind: "you-id".into(), text: "12".into() });
    assert_eq!(h("steered : m_3 m_4"), Hub::Steered(vec![3, 4]));
    assert_eq!(h("steered : m_3"), Hub::Steered(vec![3]));
    assert_eq!(h("steered : m_3 x"), Hub::Other { kind: "steered".into(), text: "m_3 x".into() });
    assert_eq!(h("steered : "), Hub::Other { kind: "steered".into(), text: String::new() });
    assert_eq!(h("steer-rx : m_3 m_4"), Hub::SteerRx(vec![3, 4]));
    assert_eq!(h("steer-rx : m_3"), Hub::SteerRx(vec![3]));
    assert_eq!(h("steer-rx : 3"), Hub::Other { kind: "steer-rx".into(), text: "3".into() });
    assert_eq!(h("steer-rx : "), Hub::Other { kind: "steer-rx".into(), text: String::new() });
}

/// Laws (architect m_16963): sb-core's receipts mark by id, never by
/// words: steer-rx raises the messages it names to received, steered to
/// read; a message with the same words but another id (or none) stays.
#[test]
fn receipts_mark_the_messages_by_id() {
    use crate::thread::lines::{deliver, mark_of, Delivered, Mark};
    use crate::thread::Delivery;
    #[derive(Debug)]
    struct M(&'static str, Option<u64>, Delivery);
    impl Delivered for M {
        fn yours(&self) -> Option<(&str, Delivery)> {
            Some((self.0, self.2))
        }
        fn set_mark(&mut self, to: Delivery) {
            self.2 = to;
        }
        fn msg_id(&self) -> Option<u64> {
            self.1
        }
    }
    let rx = mark_of(&Rec::Hub(Hub::SteerRx(vec![2, 9]))).unwrap();
    let read = mark_of(&Rec::Hub(Hub::Steered(vec![2]))).unwrap();
    assert_eq!(rx, Mark::Ids { ids: vec![2, 9], to: Delivery::Received });
    assert_eq!(read, Mark::Ids { ids: vec![2], to: Delivery::Read });
    let mut items = vec![M("again", Some(1), Delivery::Sent), M("again", Some(2), Delivery::Sent), M("again", None, Delivery::Sent)];
    assert_eq!(deliver(&mut items, &rx), Some(vec![1]));
    assert_eq!(items.iter().map(|m| m.2).collect::<Vec<_>>(), [Delivery::Sent, Delivery::Received, Delivery::Sent]);
    assert_eq!(deliver(&mut items, &read), Some(vec![1]));
    // marks only go up: a late steer-rx after steered leaves it read
    assert_eq!(deliver(&mut items, &rx), Some(vec![]));
    assert_eq!(items.iter().map(|m| m.2).collect::<Vec<_>>(), [Delivery::Sent, Delivery::Read, Delivery::Sent]);
}

#[test]
fn bash_calls_read_as_reports_and_publishes() {
    assert_eq!(report_in(r#"cd x && sb report done "landed \"a\"" --decision y"#), Some((ReportKind::Done, "landed \"a\"".into())));
    assert_eq!(report_in("sb report progress 'half'"), Some((ReportKind::Progress, "half".into())));
    assert_eq!(report_in("sb report nope x"), None);
    assert_eq!(publish_in("sb page publish ./weekly.html"), Some("weekly".into()));
    assert_eq!(publish_in("sb page publish a.html --id perf-notes"), Some("perf-notes".into()));
    assert_eq!(summary(&[]), "");
    assert_eq!(one_line("  \nabcdef", 4), "abc…");
}

/// architect m_10999: the current usage is the last usage line unless a
/// compaction ended after it (the TUI over its events, the hub over a
/// transcript tail).
#[test]
fn the_current_usage_is_the_last_one_unless_compacted_after() {
    let cur = |ls: &[&str]| current_usage(ls.iter().map(|l| usage_mark(l)));
    let u1 = "  obs: usage: model=m in=10 out=2";
    let u2 = "  obs: usage: model=m in=30 out=1";
    let done = "  obs: compaction_done: the summary";
    assert_eq!(cur(&[]), None);
    assert_eq!(cur(&[u1, "--- idle", u2, "sb you : hi"]), Some("model=m in=30 out=1".into()));
    assert_eq!(cur(&[u1, done]), None);
    assert_eq!(cur(&[u1, done, u2]), Some("model=m in=30 out=1".into()));
    assert_eq!(usage_mark("tool #3 bash : ls"), UsageMark::Other);
}

/// The tool readings the TUI's rows and the hub's tool items share
/// (R11): wire decoding, a failed bash's exit code, its first error
/// line, a patch's files.
#[test]
fn tool_results_and_patches_read_once() {
    assert_eq!(wire_decode("a\\Nb\\Rc\\\\Nd"), "a\nb\rc\\Nd");
    assert_eq!(exit_code("exit 1: boom"), Some(1));
    assert_eq!(exit_code("exit x: boom"), None);
    assert_eq!(exit_code("exit : boom"), None);
    assert_eq!(exit_code("all good"), None);
    assert_eq!(error_line("exit 2: building\nerror[E0425]: cannot find x"), Some("error[E0425]: cannot find x".into()));
    assert_eq!(error_line("no marks here"), Some("no marks here".into()));
    assert_eq!(error_line("exit 1:   "), None);
    let p = "*** Begin Patch\n*** Update File: a.rs\n+x\n-y\n-z\n*** Add File: b.rs\n+1\n*** Update File: c.rs\n*** Move to: d.rs\n+k\n*** End Patch";
    assert_eq!(patch_files(p), vec![("a.rs".into(), 1, 2), ("b.rs".into(), 1, 0), ("c.rs → d.rs".into(), 1, 0)]);
}

/// Law (architect m_14532, sched-names): a wake from an older hub (no
/// name) still reads, with an empty name; a named one gives its name; a
/// name never eats the parenthesis.
#[test]
fn an_old_wake_still_reads() {
    let old = "timer #48 (every 2m, 2/6, set by x): check the build\n(stop it: sb every --stop 48)";
    assert_eq!(timer_wake(old), Some((48, "", "every 2m, 2/6, set by x", "check the build")));
    let new = "timer #48 \"build check\" (every 2m, 2/6, set by x): check the build\n(stop it: sb every --stop 48)";
    assert_eq!(timer_wake(new), Some((48, "build check", "every 2m, 2/6, set by x", "check the build")));
    for not in ["timer #48 \"open (every 2m): x", "timer #x (every 2m): y", "timer set: #1 @main every 10m", "hello"] {
        assert_eq!(timer_wake(not), None, "{not}");
    }
}

#[test]
fn partial_lines_read_as_deltas() {
    assert_eq!(read("partial #17.0 +0 : Hel"), Rec::Partial { call: "17.0".into(), off: 0, text: "Hel".into() });
    assert_eq!(read("partial #17.1 +3 : a\\nb : c"), Rec::Partial { call: "17.1".into(), off: 3, text: "a\\nb : c".into() });
    assert_eq!(read("partial #17.0 +0 :"), Rec::Partial { call: "17.0".into(), off: 0, text: String::new() });
    assert_eq!(read("partial #17.0 +x : y"), Rec::Dropped);
    assert_eq!(read("partial #17.0 : y"), Rec::Dropped);
}

// the hub's live answer: deltas append (offsets in escaped chars), a new
// block or call starts again, a mismatch drops the block until the next
#[test]
fn partial_appends_checks_and_restarts() {
    let d = |off: usize, text: &str| PartialStep::Delta { off, text: text.into() };
    let mut p = Partial::default();
    assert_eq!(p.step("1.0", 0, "a\\n"), d(0, "a\n"));
    // offsets on the wire count escaped chars, the deltas unescaped ones
    assert_eq!(p.step("1.0", 3, "é"), d(2, "é"));
    // a `\` at the end waits for the next delta: here it starts a `\n`
    assert_eq!(p.step("1.0", 4, "x\\"), d(3, "x"));
    assert_eq!(p.text(), "a\néx\\");
    assert_eq!(p.step("1.0", 6, "ny"), d(4, "\ny"));
    // a lone held `\` is nothing to send yet; the next char decides
    assert_eq!(p.step("1.0", 8, "\\"), PartialStep::Drop);
    assert_eq!(p.step("1.0", 9, "z"), d(6, "\\z"));
    // a gap: the first mismatch is logged, the rest of the block dropped
    assert_eq!(p.step("1.0", 99, "x"), PartialStep::Mismatch { call: "1.0".into(), off: 99, have: 10 });
    assert_eq!(p.step("1.0", 100, "y"), PartialStep::Drop);
    assert_eq!(p.text(), "");
    // the next block (offset 0) streams again
    assert_eq!(p.step("1.0", 0, "b"), d(0, "b"));
    // a retry is another call: it replaces the text
    assert_eq!(p.step("1.1", 0, "c"), d(0, "c"));
    // another call that doesn't start at 0: dropped, never glued on
    assert!(matches!(p.step("2.0", 1, "d"), PartialStep::Mismatch { have: 0, .. }));
}

// law (architect m_17813): whatever the cuts of the escaped stream, a
// client folding the hub's deltas (LiveText) holds the hub's text after
// every event, a repeated event changes nothing, and a client that
// subscribes midway (the whole text at 0, then the deltas) ends with the
// same text; the deltas sum to the text once, never quadratic
#[test]
fn law_live_text_fold_is_the_hubs_text() {
    let pieces = ["a", "\\", "n", "\\n", "é", "日本", "\\\\", "x\\", "\\nb", " ", "**", "😀"];
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for _ in 0..500 {
        let esc: String = (0..next(40)).map(|_| pieces[next(pieces.len())]).collect();
        let chars: Vec<char> = esc.chars().collect();
        let (mut hub, mut client, mut late) = (Partial::default(), LiveText::default(), None::<LiveText>);
        let (mut at, mut sent) = (0, 0);
        while at < chars.len() || at == 0 {
            let n = (1 + next(5)).min(chars.len() - at).max(usize::from(chars.is_empty()));
            let chunk: String = chars[at..at + n.min(chars.len() - at)].iter().collect();
            if let PartialStep::Delta { off, text } = hub.step("1.0", at, &chunk) {
                sent += utf16_len(&text);
                assert!(client.apply("1.0", off, &text));
                let once = client.clone();
                assert!(client.apply("1.0", off, &text), "a repeated event");
                assert_eq!(client, once, "a repeated event changes nothing");
                if let Some(l) = late.as_mut() {
                    l.apply("1.0", off, &text);
                }
            }
            // the client's text is the hub's, but for a held `\`
            assert!(hub.text() == client.text || hub.text() == format!("{}\\", client.text), "{esc:?}: {:?} vs {:?}", hub.text(), client.text);
            if late.is_none() && next(3) == 0 {
                let mut l = LiveText::default();
                l.apply("1.0", 0, &hub.text());
                late = Some(l);
            }
            at += chunk.chars().count();
            if chars.is_empty() {
                break;
            }
        }
        assert_eq!(hub.text(), unescape(&esc), "{esc:?}");
        assert!(sent <= utf16_len(&unescape(&esc)), "each char sent once: {esc:?}");
        if let Some(l) = late {
            // a late joiner ends with the same text (its whole text may
            // have carried the held `\` already)
            assert!(l.text == client.text || l.text == format!("{}\\", client.text), "{esc:?}: {:?} vs {:?}", l.text, client.text);
        }
    }
    // an event past the text or of another call is a gap: empty, never glued
    let mut c = LiveText::default();
    c.apply("1.0", 0, "abc");
    assert!(!c.apply("1.0", 9, "x") && c.is_empty());
    c.apply("1.0", 0, "abc");
    assert!(!c.apply("2.0", 3, "x") && c.is_empty());
    // an older offset rewrites from there (a resend after a reconnect)
    c.apply("1.0", 0, "abcdé");
    c.apply("1.0", 2, "Z");
    assert_eq!(c.text, "abZ");
    // off counts UTF-16 code units, as the desktop's JS does (architect
    // m_17866): an astral char before the cut counts 2, a cut inside its
    // surrogate pair is a gap
    c.apply("1.0", 0, "a😀b");
    assert!(c.apply("1.0", 4, "c"));
    assert_eq!(c.text, "a😀bc");
    assert!(c.apply("1.0", 3, "Z"));
    assert_eq!(c.text, "a😀Z");
    assert!(!c.apply("1.0", 2, "Z") && c.is_empty());
    let mut h = Partial::default();
    assert_eq!(h.step("1.0", 0, "日😀"), PartialStep::Delta { off: 0, text: "日😀".into() });
    assert_eq!(h.step("1.0", 2, "x"), PartialStep::Delta { off: 3, text: "x".into() });
    // "" at 0 ends it
    c.apply("1.0", 0, "");
    assert!(c.is_empty());
}

// every clear reason ends the live answer; a tool line or usage doesn't
#[test]
fn partial_clears_on_every_end() {
    for l in [
        "  obs: assistant: hi",
        "  obs: turn_done: completed",
        "  obs: turn_done: failed: boom",
        "  obs: turn_done: interrupted",
        "sb stopped : by the user",
        "--- idle",
        // a new attempt starts empty: its text or its tool calls never
        // run under the old attempt's live text (architect m_13563)
        "  obs: candidate_discarded: empty",
        "  obs: provider_retry: 2/10 · 529 · retry in 4s",
    ] {
        assert!(clears_partial(&read(l)), "{l}");
    }
    for l in ["  obs: usage: model=m in=1 out=2", "tool #3 bash : ls", "partial #1.0 +0 : x", "  obs: turn_started"] {
        assert!(!clears_partial(&read(l)), "{l}");
    }
}

// an attempt that ends without its answer keeps the text read so far,
// at its place, the notices under it (designer m_13334, m_13562); a
// stop, a completed turn or an answer don't
#[test]
fn partial_kept_when_an_attempt_ends_without_its_answer() {
    for l in [
        "  obs: turn_done: failed: boom",
        "  obs: turn_stalled: no progress",
        "  obs: harness_restarted: crash",
        "  obs: provider_retry: 2/10 · 529 · retry in 4s",
        "  obs: candidate_discarded: empty",
    ] {
        assert!(keeps_partial(&read(l)), "{l}");
        assert!(clears_partial(&read(l)), "{l}: kept, then cleared");
    }
    for l in [
        "  obs: turn_done: completed",
        "  obs: turn_done: interrupted",
        "  obs: turn_done: failed: interrupted by the user",
        "  obs: assistant: hi",
        "sb stopped : by the user",
        "tool #3 bash : ls",
    ] {
        assert!(!keeps_partial(&read(l)), "{l}");
    }
}

// the kept record carries the text whole (a new line), read back whole;
// nothing to keep: no line
#[test]
fn partial_kept_line_round_trips() {
    let mut p = Partial::default();
    assert_eq!(p.kept_line(), None);
    p.step("1.0", 0, "first\\nsecond");
    p.step("1.0", 13, " and more");
    let l = p.kept_line().expect("a kept line");
    assert!(l.starts_with("sb kept :: {"), "{l}");
    assert_eq!(read(&l), Rec::Hub(Hub::Kept("first
second and more".into())));
    // a new attempt's first delta replaces the text
    p.step("1.1", 0, "again");
    assert_eq!(read(&p.kept_line().unwrap()), Rec::Hub(Hub::Kept("again".into())));
    // a broken block keeps nothing
    p.step("1.1", 9, "x");
    assert_eq!(p.kept_line(), None);
    let mut blank = Partial::default();
    blank.step("2.0", 0, "  ");
    assert_eq!(blank.kept_line(), None);
}

// a real line break in a record's text reads as the old line's escaped
// one did (the TUI splits it into rows the same way); his typed
// backslash-n is his words: the record keeps the two characters, where
// the old line turned them into a line break the agent never got
#[test]
fn a_records_line_break_reads_as_the_old_lines() {
    for (old, rec) in [
        ("sb you : a\\nb", r#"sb you :: {"text":"a\nb"}"#),
        ("sb msg-in : docs m_3 : a\\nb", r#"sb msg-in :: {"body":"a\nb","from":"docs","id":3}"#),
        ("sb msg : perf → docs m_7 : a\\nb", r#"sb msg :: {"body":"a\nb","from":"perf","id":7,"to":"docs"}"#),
        ("sb msg-you : docs : a\\nb", r#"sb msg-you :: {"body":"a\nb","from":"docs"}"#),
        ("sb sent : main : m_9 : 1 : a\\nb", r#"sb sent :: {"ask":true,"body":"a\nb","id":9,"to":"main"}"#),
        ("sb answered : docs : q\\nr : a\\nb : why", r#"sb answered :: {"agent":"docs","answer":"a\nb","question":"q\nr","why":"why"}"#),
        ("sb card : #2 question @ui : pick\\n1. a", r#"sb card :: {"agent":"ui","body":"pick\n1. a","id":2,"kind":"question","v":1}"#),
        ("sb route : you → @docs (answer to card #40) : a\\nb", r#"sb route :: {"card":40,"said":"a\nb","who":"docs"}"#),
        ("sb undelivered : fix : a\\nb", r#"sb undelivered :: {"text":"a\nb","to":"fix"}"#),
    ] {
        assert_eq!(read(rec), read(old), "{rec}");
        assert!(format!("{:?}", read(rec)).contains("a\\nb") || format!("{:?}", read(rec)).contains("pick\\n1. a"), "{rec}: a real line break");
    }
    assert_eq!(read(r#"sb you :: {"text":"a\\nb"}"#), Rec::Hub(Hub::You("a\\nb".into(), None)), "a typed backslash-n, as typed");
}

/// notif-rename (architect m_24654): main's note of his answer to its own
/// notification is dropped by its head, the new one and, in transcripts
/// written before, the old one; nothing else is.
#[test]
fn main_answer_notes_read_with_either_head() {
    assert!(is_main_answer_note("the user answered notification #3 (\"ship?\"): \"yes\""));
    assert!(is_main_answer_note("the user answered card #3 (\"ship?\"): \"yes\""));
    assert!(!is_main_answer_note("the user answered a question"));
}

/// docs/mcp-apps.md C3: `tool_view #<id> : <ref>`; a ref that is not
/// the four strings, a bad call id or a non-ui:// uri is dropped.
#[test]
fn a_tool_view_line_reads_its_ref() {
    let r = r#"{"plugin":"fake-app","server":"fake-app","uri":"ui://fake-app/counter","call":"1730000000000-0"}"#;
    assert_eq!(
        read(&format!("tool_view #4 : {r}")),
        Rec::ToolView { id: 4, view: ToolViewRef { plugin: "fake-app".into(), server: "fake-app".into(), uri: "ui://fake-app/counter".into(), call: "1730000000000-0".into() } }
    );
    for bad in [
        "tool_view #4 : {}",
        "tool_view #4 : not json",
        r#"tool_view #x : {"plugin":"p","server":"s","uri":"ui://p/v","call":"1-0"}"#,
        r#"tool_view #4 : {"plugin":"p","server":"s","uri":"ui://p/v","call":"../1-0"}"#,
        r#"tool_view #4 : {"plugin":"p","server":"s","uri":"https://x","call":"1-0"}"#,
    ] {
        assert_eq!(read(bad), Rec::Dropped, "{bad}");
    }
}
