use super::*;
use crate::rows::ReportKind;

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
    assert_eq!(h("you : a\\nb"), Hub::You("a\nb".into()));
    assert_eq!(h("undelivered : fix : d'abord \\: les tests"), Hub::Undelivered { to: "fix".into(), text: "d'abord : les tests".into() });
    assert_eq!(h("msg-in : docs m_3 : hi : there"), Hub::MsgIn { from: "docs".into(), id: "m_3".into(), body: "hi : there".into() });
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
    assert_eq!(h("route : you → @docs (answer to card #4) : v2"), Hub::Route { who: "docs".into(), card: 4, said: "v2".into() });
    assert_eq!(h("pr : red : 412 : https://x/412 : checks fail \\: e2e"), Hub::Pr { state: crate::thread::PrNewsState::Failing, number: 412, url: "https://x/412".into(), text: "checks fail : e2e".into() });
    assert_eq!(h("pr : plain : x : u : hi"), Hub::Other { kind: "pr".into(), text: "hi".into() });
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
