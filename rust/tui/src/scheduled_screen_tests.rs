use super::*;

const NOW: u64 = 1_790_000_000_000;
const MIN: u64 = 60_000;

fn task(id: u64, agent: &str, next_in: u64) -> Task {
    Task {
        id,
        agent: agent.into(),
        by: agent.into(),
        label: "every 2m".into(),
        text: format!("check the build for {agent}"),
        next_ms: NOW + next_in,
        ..Default::default()
    }
}

fn texts(ls: &[Line]) -> Vec<String> {
    ls.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
}

fn fixture() -> Vec<Task> {
    let mut a = task(48, "answer-line", MIN);
    a.times = Some(6);
    a.fired = 2;
    let mut b = task(53, "designer", 10 * MIN);
    b.name = "designer review".into();
    let mut gone = task(47, "sock-path", 0);
    gone.ended_ms = Some(NOW - 30 * MIN);
    gone.end = "times".into();
    gone.times = Some(3);
    let mut stopped = task(44, "launch", 0);
    stopped.ended_ms = Some(NOW - 60 * MIN);
    stopped.end = "stopped".into();
    stopped.stopped_by = "user".into();
    vec![b, gone, a, stopped]
}

/// site/m/timers: the active ones soonest first; tab adds the ended
/// ones under `ended`, newest first, with why; / finds by agent or words.
#[test]
fn the_list_soonest_first_then_ended_on_tab_and_find() {
    let all = fixture();
    let mut sc = Screen::default();
    assert_eq!(shown(&sc, &all).iter().map(|t| t.id).collect::<Vec<_>>(), vec![48, 53]);
    sc.ended_too = true;
    assert_eq!(shown(&sc, &all).iter().map(|t| t.id).collect::<Vec<_>>(), vec![48, 53, 47, 44]);
    let (ls, hits) = lines(&mut sc, &all, 150, 24, NOW);
    let t = texts(&ls);
    assert!(t[0].starts_with("scheduled · what wakes your agents, and when") && t[0].ends_with("2 active · 2 ended   tab active only"), "{}", t[0]);
    assert_eq!(t[1], "/ find: an agent, a name, the words");
    // designer m_15602: its name right after its agent, never its id
    assert!(t[3].starts_with("› ◷ answer-line   check the build   ") && t[3].contains("every 2m") && !t[3].contains("for answer-line"), "{}", t[3]);
    assert!(t[3].contains("in 1m") && t[3].contains("2 of 6") && t[3].trim_end().ends_with("by answer-line"), "{}", t[3]);
    assert!(t.iter().all(|l| !l.contains('#')), "no id on this screen: {t:?}");
    assert!(t.iter().any(|l| l.trim() == "ended"));
    assert!(t.iter().any(|l| l.contains("◷ launch        check the build ") && l.contains("stopped by you")), "{t:?}");
    assert!(t.iter().any(|l| l.contains("◷ sock-path     check the build ") && l.contains("ran its 3 times")));
    assert_eq!(hits.len(), 4);
    assert!(t[t.len() - 1].starts_with("⏎ open   r run now   x stop   / find   tab active only   esc close"), "{}", t[t.len() - 1]);
    sc.query = "designer".into();
    assert_eq!(shown(&sc, &all).iter().map(|t| t.id).collect::<Vec<_>>(), vec![53]);
    // sched-names: a row shows its name (else the plain fallback), never
    // its words, and never its agent again ('designer review' on
    // designer's row reads 'review', designer m_15602); / finds a name
    assert!(t.iter().any(|l| l.starts_with("  ◷ designer      review   ")), "{t:?}");
    sc.query = "review".into();
    assert_eq!(shown(&sc, &all).iter().map(|t| t.id).collect::<Vec<_>>(), vec![53]);
}

/// At 80 columns (designer m_15602): agent, name and next run stay; the
/// rhythm and who set it go; an ended row says it short.
#[test]
fn eighty_columns() {
    let all = fixture();
    let mut sc = Screen::default();
    let (ls, _) = lines(&mut sc, &all, 76, 20, NOW);
    let t = texts(&ls);
    assert!(t[0].starts_with("scheduled ") && t[0].ends_with("2 active"), "{}", t[0]);
    assert!(t[3].starts_with("› ◷ answer-line  check the build ") && t[3].contains("in 1m") && t[3].contains("2 of 6"), "{}", t[3]);
    assert!(!t[3].contains("every") && !t[3].contains("by "), "{}", t[3]);
    assert!(t.iter().all(|l| l.width() <= 76), "{t:?}");
    assert_eq!(t[t.len() - 1], "⏎ open   r run now   x stop   esc close");
    sc.ended_too = true;
    let t = texts(&lines(&mut sc, &all, 80, 20, NOW).0);
    assert!(t.iter().any(|l| l.contains("◷ launch       check the build ") && l.contains("stopped 15:")), "{t:?}");
    assert!(t.iter().any(|l| l.contains("◷ sock-path    check the build ") && l.contains("ended 15:") && l.contains("· 3 runs")), "{t:?}");
    // a name is a label, never cut with … (the detail line below may be)
    assert!(t.iter().filter(|l| l.contains('◷')).all(|l| !l.contains('…')), "{t:?}");
    assert!(t.iter().all(|l| !l.contains('#') && l.width() <= 80), "{t:?}");
}

/// x asks on the row; one opened says who, when, so far, its words.
#[test]
fn stop_asks_on_the_row_and_one_opens() {
    let all = fixture();
    let mut sc = Screen { confirm: Some(48), sel: Some(48), ..Default::default() };
    let (ls, _) = lines(&mut sc, &all, 150, 20, NOW);
    let t = texts(&ls);
    assert!(t[3].starts_with("› ◷ stop “check the build”? it won't wake answer-line again.   y stop   n or esc keep"), "{}", t[3]);
    assert_eq!(t[t.len() - 1], "y stop   n or esc keep");
    let sc = Screen { opened: Some(48), ..Default::default() };
    let t = texts(&opened_lines(&sc, &all[2], 150, 30, NOW));
    assert!(t[0].starts_with("check the build · answer-line") && t[0].ends_with("◷ active"), "{}", t[0]);
    assert!(t.iter().all(|l| !l.contains('#')), "{t:?}");
    for want in ["wakes       answer-line", "set by      answer-line", "when        every 2m · 6 times", "so far      2 of 6", "ends        after its 6th run"] {
        assert!(t.iter().any(|l| l.starts_with(want)), "{want}: {t:?}");
    }
    assert!(t.iter().any(|l| l.contains("check the build for answer-line")));
    assert_eq!(t[t.len() - 1], "r run now   x stop   esc back to the list");
}
