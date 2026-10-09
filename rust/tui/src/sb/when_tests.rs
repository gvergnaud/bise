//! BISE-271: the time of a turn's end, from the hub's `ts` to the hover.

use super::bench::test_app;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn line(l: &str, pos: usize, ts: Option<u64>) -> String {
    let mut v = json!({"ev": "line", "agent": "main", "line": l, "pos": pos});
    if let Some(t) = ts {
        v["ts"] = json!(t);
    }
    v.to_string()
}

fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    let w = buf.area.width as usize;
    buf.content.chunks(w).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

/// The screen row of the first row that shows `text`.
fn row_of(term: &Terminal<TestBackend>, text: &str) -> u16 {
    screen(term).iter().position(|l| l.contains(text)).unwrap_or_else(|| panic!("{:?} not on screen", text)) as u16
}

fn ended(app: &App) -> Vec<u64> {
    app.events.iter().filter_map(|e| if let Ev::Ended(t) = e { Some(*t) } else { None }).collect()
}

const HOUR: u64 = 3_600_000;

// TODO(client-protocol P4d switch, proto-zone-b): the three turn-end
// tests below still feed `line` events: from entries a turn's end is an
// agents row's edge (feed_entries::turn_edge, the TUI's clock) and a page
// has none; the switch decides their typed fact (proto-reads m_14950)
#[test]
fn a_turn_end_keeps_the_hubs_time() {
    let mut app = test_app();
    let t = crate::when::now_ms() - HOUR;
    dispatch(&mut app, &line("  obs: turn_started", 1, Some(t - 5_000)));
    dispatch(&mut app, &line("  obs: assistant: done here", 2, Some(t - 1_000)));
    dispatch(&mut app, &line("  obs: turn_done: completed", 3, Some(t)));
    assert_eq!(ended(&app), vec![t]);
    assert_eq!(crate::feed::turn_end_of(&app.events, 1), Some(t));
    // an interrupted turn ends too
    dispatch(&mut app, &line("  obs: turn_started", 4, Some(t + 1_000)));
    dispatch(&mut app, &line("  obs: turn_done: interrupted", 5, Some(t + 2_000)));
    assert_eq!(ended(&app), vec![t, t + 2_000]);
    // a line the REPL replays has the replay's time: no end
    dispatch(&mut app, &line("history   obs: turn_done: completed", 6, Some(t + 3_000)));
    assert_eq!(ended(&app).len(), 2);
    // a hub without `ts`: the time it arrived
    let before = crate::when::now_ms();
    dispatch(&mut app, &line("  obs: turn_done: completed", 7, None));
    assert!(ended(&app)[2] >= before);
}

#[test]
fn a_running_turn_has_no_end_yet() {
    let mut app = test_app();
    let t = crate::when::now_ms();
    dispatch(&mut app, &line("  obs: turn_started", 1, Some(t - 3 * HOUR)));
    dispatch(&mut app, &line("  obs: assistant: first", 2, Some(t - 3 * HOUR)));
    dispatch(&mut app, &line("  obs: turn_done: completed", 3, Some(t - 2 * HOUR)));
    dispatch(&mut app, &line("  obs: turn_started", 4, Some(t)));
    dispatch(&mut app, &line("  obs: assistant: still going", 5, Some(t)));
    let i = |text: &str| app.events.iter().position(|e| matches!(e, Ev::Assistant(a) if a.contains(text))).unwrap();
    assert_eq!(crate::feed::turn_end_of(&app.events, i("first")), Some(t - 2 * HOUR));
    assert_eq!(crate::feed::turn_end_of(&app.events, i("still going")), None);
}

#[test]
fn the_mouse_over_a_reply_shows_when_its_turn_ended() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let t = crate::when::now_ms() - HOUR - 60_000;
    dispatch(&mut app, &line("sb you : hello there", 1, Some(t - 10_000)));
    dispatch(&mut app, &line("  obs: turn_started", 2, Some(t - 9_000)));
    dispatch(&mut app, &line("  obs: assistant: the answer is here", 3, Some(t - 1_000)));
    dispatch(&mut app, &line("  obs: turn_done: completed", 4, Some(t)));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let label = crate::when::ended_now(t);
    assert!(label.ends_with("· 1h ago"), "{}", label);
    let y = row_of(&term, "the answer is here");
    let before = screen(&term);
    assert!(!before.iter().any(|l| l.contains("1h ago")));
    // over the reply: the time, right-aligned in the history's column,
    // on that row; the text did not move
    app.hover = Some((10, y));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let after = screen(&term);
    let row: Vec<char> = after[y as usize].chars().collect();
    let col_end = (app.feed_x as usize) + app.area_w;
    let shown: String = row[col_end - label.chars().count()..col_end].iter().collect();
    assert_eq!(shown, label, "right-aligned in the column: {:?}", after[y as usize]);
    assert_eq!(row[col_end - label.chars().count() - 1], ' ');
    let x = after[y as usize].find("the answer").unwrap();
    assert_eq!(before[y as usize].find("the answer"), Some(x));
    for (k, (a, b)) in before.iter().zip(&after).enumerate() {
        if k != y as usize {
            assert_eq!(a, b, "row {} changed", k);
        }
    }
    // over your message: nothing
    app.hover = Some((10, row_of(&term, "hello there")));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!screen(&term).iter().any(|l| l.contains("1h ago")));
    // back on the reply, then away (a click): gone
    app.hover = Some((10, y));
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(screen(&term)[y as usize].contains("1h ago"));
    let click = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: 10,
        row: y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    crate::input::on_mouse(&mut app, &click, 30);
    assert_eq!(app.hover, None);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!screen(&term).iter().any(|l| l.contains("1h ago")));
}

#[test]
fn a_replayed_feed_marks_its_pauses_with_the_hubs_time() {
    let mut app = test_app();
    let t = crate::when::now_ms() - 3 * HOUR;
    // the thread's first page: its entries arrive at once, the pause is
    // in their times
    let mut hub = super::entries_for_tests::Hub::new();
    hub.had("main", [
        (1, t, "  obs: assistant: before the pause".to_string()),
        (2, t + 2 * HOUR, "  obs: assistant: after the pause".to_string()),
        (3, t + 2 * HOUR + 1_000, "  obs: assistant: right after".to_string()),
    ]);
    hub.subscribe(&mut app, "main", false);
    let marks: Vec<String> = app.events.iter().filter_map(|e| if let Ev::TimeMark(m) = e { Some(m.clone()) } else { None }).collect();
    assert_eq!(marks, vec![crate::when::mark_now(t + 2 * HOUR)]);
}
