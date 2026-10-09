//! Timing of long feeds (infinite-feed): `SB_BENCH_TRANSCRIPT=<transcript.log>
//! SB_BENCH_LINES=50000 cargo test --release -p bend-tui bench_long_feed -- --ignored --nocapture`.

use super::*;
use super::entries_for_tests::Hub;
use super::feed::{MAX_EVENTS, PAGE_LINES};
use bise_proto::thread::Line;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::Instant;

pub(crate) fn test_app() -> App {
    let (a, _b) = UnixStream::pair().unwrap();
    let (_tx, rx) = mpsc::channel::<String>();
    let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
    std::mem::forget(_b);
    sb_app(sb, rx, false, 100, crate::voice::Voice::live(false))
}

/// A test app whose hub end is read and dropped by a thread: any number
/// of sends (Enter, commands) never fills the socket and blocks.
pub(crate) fn test_app_drained() -> App {
    let (a, mut b) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut b, &mut std::io::sink());
    });
    let (_tx, rx) = mpsc::channel::<String>();
    std::mem::forget(_tx);
    let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
    sb_app(sb, rx, false, 100, crate::voice::Voice::live(false))
}

/// Test setup: the workspace of the `@` popup, and a live agent.
pub(crate) fn set_workspace(app: &mut App, ws: &str) {
    let sb = &mut app.sb;
    sb.workspace = ws.to_string();
}

pub(crate) fn add_agent(app: &mut App, name: &str, objective: &str) {
    let sb = &mut app.sb;
    sb.agents.push(Agent {
        name: name.into(),
        status: "working".into(),
        objective: objective.into(),
        ..Agent::default()
    });
}

/// Test setup: the model and effort the hub says an agent runs
/// (BISE-135), and the efforts its model takes.
pub(crate) fn set_model(app: &mut App, name: &str, model: &str, effort: &str) {
    let efforts = crate::models::efforts(model).0;
    for a in app.sb.agents.iter_mut().filter(|a| a.name == name) {
        a.model = model.into();
        a.effort = effort.into();
        a.efforts = efforts.clone();
    }
}

/// Test setup: whether the hub runs in bise's source tree (its
/// `versions` event), with one commit in its list.
pub(crate) fn set_versions_dev(app: &mut App, dev: bool) {
    let mut v = versions::VersionItem::default();
    v.rev = "abc1234".into();
    app.sb.versions = vec![v];
    app.sb.versions_dev = Some(dev);
}

/// Test setup: the status of an agent (`archived`, `idle`…).
pub(crate) fn set_status(app: &mut App, name: &str, status: &str) {
    for a in app.sb.agents.iter_mut().filter(|a| a.name == name) {
        a.status = status.into();
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore]
fn bench_long_feed() {
    let path = bise_home::env::test_setting("SB_BENCH_TRANSCRIPT").expect("SB_BENCH_TRANSCRIPT");
    let want: usize = bise_home::env::test_setting("SB_BENCH_LINES").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let raw = std::fs::read_to_string(&path).unwrap();
    let src: Vec<String> = raw
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(_, r)| r.to_string()))
        .collect();
    let mut lines = Vec::with_capacity(want);
    while lines.len() < want {
        for l in &src {
            if lines.len() >= want {
                break;
            }
            lines.push(l.clone());
        }
    }
    let bytes: usize = lines.iter().map(|l| l.len()).sum();
    eprintln!("lines {} bytes {}", lines.len(), bytes);
    let mut app = test_app();
    let (w, h) = (200u16, 50u16);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    let t = Instant::now();
    let mut hub = Hub::new();
    for l in &lines {
        hub.line(&mut app, "big", l);
    }
    eprintln!("replay (dispatch, feed out of focus): {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "big");
    let f = ms(t);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("focus switch: focus() {:.1} ms + first draw {:.1} ms (events {})", f, ms(t), app.events.len());
    let mut worst = 0f64;
    for _ in 0..10 {
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("steady frame at the tail (worst of 10): {:.2} ms", worst);
    for (i, e) in app.events.iter().enumerate() {
        if let Ev::Tool(td) = e {
            if matches!(td.state, ToolState::Run) {
                let rows = app.cache[i].as_ref().map_or(0, |c| c.rows.len());
                let code = td.code.as_ref().map_or(0, |c| c.len());
                eprintln!("  live tool #{} at {} rows {} code {} bytes", td.id, i, rows, code);
            }
        }
    }
    app.follow = false;
    let mut worst = 0f64;
    for _ in 0..50 {
        app.scroll -= 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("PageUp frames (worst of 50): {:.2} ms", worst);
    app.anchor = (0, 0);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("jump to the top: {:.2} ms", ms(t));
    let mut worst = 0f64;
    for _ in 0..50 {
        app.scroll += 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("PageDown frames from the top (worst of 50): {:.2} ms", worst);
    let t = Instant::now();
    for l in lines.iter().take(20) {
        hub.line(&mut app, "big", l);
    }
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("20 live lines + frame: {:.2} ms", ms(t));
    term.backend_mut().resize(w - 30, h);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("resize frame: {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "main");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    focus(&mut app, "big");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("focus away and back + 2 draws: {:.1} ms", ms(t));

    // what the hub does now: the newest entries on subscribe, then pages
    // of older ones while the user scrolls up
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    let n = lines.len();
    let t = Instant::now();
    let mut hub = Hub::new();
    hub.had("big", lines.iter().enumerate().skip(n.saturating_sub(1000)).map(|(i, l)| (i as u64 + 1, 0, l.clone())));
    hub.subscribe(&mut app, "big", n > 1000);
    eprintln!("windowed: replay of the last 1000 lines: {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "big");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("windowed: focus switch + first draw: {:.1} ms", ms(t));
    app.follow = false;
    let (mut worst_frame, mut worst_page, mut pages, mut frames) = (0f64, 0f64, 0, 0);
    let total = Instant::now();
    while app.win.first_pos.is_some_and(|p| p > 1) || app.anchor != (0, 0) {
        app.scroll -= 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst_frame = worst_frame.max(ms(t));
        frames += 1;
        if app.win.loading {
            let before = app.win.first_pos.unwrap();
            let from = before.saturating_sub(1000).max(1);
            let page: Vec<Line> = (from..before).map(|p| (p as u64, 0, lines[p - 1].clone())).collect();
            let t = Instant::now();
            hub.page(&mut app, "big", before, page, from > 1);
            worst_page = worst_page.max(ms(t));
            pages += 1;
        }
        if frames > 1_000_000 {
            break;
        }
    }
    eprintln!(
        "windowed: PageUp to the very top: {} frames, worst {:.2} ms; {} pages, worst page ingest {:.1} ms; total {:.0} ms; events held {}",
        frames, worst_frame, pages, worst_page, ms(total), app.events.len()
    );
}

// ---- the anchored scroll (O(visible) frames) ----

fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    let w = buf.area.width as usize;
    buf.content
        .chunks(w)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
        .collect()
}

fn feed(app: &mut App, from: usize, to: usize) {
    for k in from..to {
        push_event(&mut app.events, &mut app.cache, Ev::Info(format!("event {}", k)));
    }
}

#[test]
fn a_pinned_view_does_not_move_when_lines_arrive() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    feed(&mut app, 0, 300);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.tail_visible);
    assert!(screen(&term).iter().any(|l| l.contains("event 299")));
    app.follow = false;
    app.scroll -= 40;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.tail_visible);
    // the text of the feed (the scrollbar thumb moves: more history)
    let text = |t: &Terminal<TestBackend>, h: usize| -> Vec<String> {
        screen(t).into_iter().take(h).map(|l| l.chars().take(40).collect::<String>().trim_end().to_string()).collect()
    };
    let before = text(&term, app.area_h);
    assert!(!before.iter().any(|l| l.contains("event 299")));
    feed(&mut app, 300, 320);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let after = text(&term, app.area_h);
    assert_eq!(before, after);
    // back down: the view follows again
    app.scroll += 1000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.follow && app.tail_visible);
    assert!(screen(&term).iter().any(|l| l.contains("event 319")));
}

#[test]
fn scrolling_up_then_down_comes_back() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    feed(&mut app, 0, 300);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    app.follow = false;
    app.scroll -= 100;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let a = app.anchor;
    app.scroll -= 37;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    app.scroll += 37;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert_eq!(app.anchor, a);
    // the top of the feed is reachable and stops there
    app.scroll -= 100_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    // (the header, its blank row and the top margin come first)
    assert!(screen(&term)[app.feed_y as usize].contains("event 0"));
    // every row of the feed maps to the event it shows (clicks)
    assert_eq!(app.vis_events[0], 0);
    assert_eq!(app.vis_events.len(), app.area_h);
}

// ---- the bounded window and the pages of older history ----

/// Line `pos` of main's thread, live.
fn line(hub: &mut Hub, app: &mut App, pos: usize) {
    hub.line_at(app, "main", pos as u64, 0, &format!("  obs: assistant: message {}", pos));
}

/// Line `pos` of main's thread, for a page.
fn older(pos: usize) -> Line {
    (pos as u64, 0, format!("  obs: assistant: message {}", pos))
}

#[test]
fn a_following_feed_keeps_its_last_events_then_pages_back() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    let mut hub = Hub::new();
    for p in 1..=5000 {
        line(&mut hub, &mut app, p);
    }
    assert!(app.events.len() <= MAX_EVENTS, "{}", app.events.len());
    let first = app.win.first_pos.unwrap();
    assert!(first > 1);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.win.loading);
    // up to the top of what the feed holds: a page is asked
    app.follow = false;
    app.scroll -= 1_000_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.win.loading);
    let shown = |t: &Terminal<TestBackend>| -> Vec<String> {
        screen(t).into_iter().take(10).map(|l| l.chars().take(40).collect::<String>().trim_end().to_string()).collect()
    };
    let before = shown(&term);
    let n0 = app.events.len();
    let from = first.saturating_sub(PAGE_LINES).max(1);
    let page: Vec<Line> = (from..first).map(older).collect();
    let got = page.len();
    hub.page(&mut app, "main", first, page, from > 1);
    assert!(!app.win.loading);
    assert_eq!(app.events.len(), n0 + got);
    assert_eq!(app.win.first_pos, Some(first - got));
    // the view did not move: it can keep scrolling up
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert_eq!(shown(&term), before);
    // a stale page (the feed moved since the ask) is ignored
    hub.page(&mut app, "main", 99999, vec![(1, 0, "  obs: assistant: x".into())], false);
    assert_eq!(app.events.len(), n0 + got);
    // back to the tail: the next line trims the feed again
    app.scroll += 10_000_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.follow);
    line(&mut hub, &mut app, 5001);
    assert!(app.events.len() <= MAX_EVENTS);
}

#[test]
fn a_feed_out_of_focus_is_bounded_too() {
    let mut app = test_app();
    let mut hub = Hub::new();
    for p in 1..=5000 {
        hub.line_at(&mut app, "other", p, 0, &format!("  obs: assistant: m {}", p));
    }
    focus(&mut app, "other");
    assert!(app.events.len() <= MAX_EVENTS);
    assert!(app.win.first_pos.unwrap() > 1);
}

#[test]
fn clear_empties_the_feed_and_scrolling_up_brings_it_back_in_order() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    let mut hub = Hub::new();
    for p in 1..=40 {
        line(&mut hub, &mut app, p);
    }
    handle_input(&mut app, "/clear");
    // only the notice is left
    assert_eq!(app.events.len(), 1);
    assert_eq!(app.win.first_pos, Some(41));
    for p in 41..=42 {
        line(&mut hub, &mut app, p);
    }
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.win.loading);
    assert!(!screen(&term).iter().any(|l| l.contains("message 40")));
    // scrolling up asks the lines before the clear
    app.follow = false;
    app.scroll -= 10;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.win.loading);
    hub.page(&mut app, "main", 41, (1..41).map(older).collect(), false);
    assert!(!app.win.loading);
    assert_eq!(app.win.first_pos, Some(1));
    // arrival order: the paged lines, the notice, the lines after it
    let text: Vec<String> = app
        .events
        .iter()
        .map(|e| match e {
            Ev::Assistant(t) | Ev::Info(t) => t.clone(),
            _ => String::new(),
        })
        .filter(|t| !t.is_empty())
        .collect();
    let at = |needle: &str| text.iter().position(|t| t.contains(needle)).unwrap();
    assert!(at("message 1") < at("message 40"));
    assert!(at("message 40") < at("display cleared"));
    assert!(at("display cleared") < at("message 41"));
    assert!(at("message 41") < at("message 42"));
}

#[test]
fn ctrl_l_clears_like_clear() {
    let mut app = test_app();
    let mut hub = Hub::new();
    for p in 1..=10 {
        line(&mut hub, &mut app, p);
    }
    clear_display(&mut app);
    assert!(app.events.is_empty());
    assert_eq!(app.win.first_pos, Some(11));
    assert!(app.follow);
}

/// A page of replayed history carries each line's time (`ts`, C2
/// amendment): a pause of 5 minutes between two replayed lines gets its
/// `· hh:mm ·` mark, as live; a line without `ts` (an older hub) still
/// reads, and gets no mark.
#[test]
fn replayed_history_gets_its_time_marks() {
    use crate::wire::{parse_history, HistLine};
    let t0: u64 = 1_700_000_000_000;
    let v = json!({"ev": "history", "agent": "main", "before": 10, "lines": [
        {"pos": 1, "line": "  obs: assistant: one", "ts": t0},
        {"pos": 2, "line": "  obs: assistant: two", "ts": t0 + 60_000},
        {"pos": 3, "line": "  obs: assistant: three", "ts": t0 + 60_000 + 5 * 60_000},
        {"pos": 4, "line": "  obs: assistant: old hub"},
    ]});
    let lines = parse_history(&v);
    assert_eq!(lines[3], HistLine { pos: 4, line: "  obs: assistant: old hub".into(), ts: None });
    assert_eq!(lines[2].ts, Some(t0 + 360_000));
    let mut app = test_app();
    let mut hub = Hub::new();
    for p in 10..=12 {
        hub.line_at(&mut app, "main", p, 0, &format!("  obs: assistant: live {}", p));
    }
    app.win.first_pos = Some(10);
    // the same page as entries (`thread/page`): their times, the old
    // hub's line untimed
    let page = lines.iter().map(|l| (l.pos as u64, l.ts.unwrap_or(0), l.line.clone())).collect();
    hub.page(&mut app, "main", 10, page, false);
    let marks: Vec<(usize, String)> = app
        .events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Ev::TimeMark(t) => Some((i, t.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].1, crate::when::mark_now(t0 + 360_000));
    // the mark sits right before the line after the pause
    assert!(matches!(&app.events[marks[0].0 + 1], Ev::Assistant(t) if t.contains("three")));
    assert_eq!(app.win.first_pos, Some(1));
    // an old page (no `ts` at all) reads as before, without marks
    let old = json!({"lines": [{"pos": 1, "line": "x"}, {"pos": 2, "line": "y"}]});
    assert_eq!(parse_history(&old).iter().map(|l| l.ts).collect::<Vec<_>>(), vec![None, None]);
}

/// Find searches the whole thread (user QA 2026-10-04): once what the
/// feed holds is scanned, it asks the hub for the page before it, even
/// while the view follows the tail; a match in that page is found and
/// becomes the current one, and only the new page is scanned.
#[test]
fn find_pages_in_the_older_lines() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    // the thread's newest entries on subscribe, older ones left for pages
    let mut hub = Hub::new();
    hub.had("main", (1001..1011u64).map(|pos| (pos, 0, format!("  obs: assistant: new {}", pos))));
    hub.subscribe(&mut app, "main", true);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.win.loading, "a following view asks nothing by itself");
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    for c in "ancient".chars() {
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.win.loading, "the loaded part is scanned: a page is asked");
    let page: Vec<Line> = (1..1001u64)
        .map(|p| {
            let t = if p == 3 { "the ancient bug" } else { "filler" };
            (p, 0, format!("  obs: assistant: {} {}", t, p))
        })
        .collect();
    hub.page(&mut app, "main", 1001, page, false);
    for _ in 0..100 {
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        if !app.find.as_ref().unwrap().busy() {
            break;
        }
    }
    let f = app.find.as_ref().unwrap();
    assert_eq!(f.counter(crate::find::more_before(&app), Instant::now()), "1/1");
    assert!(f.cur.is_some());
    assert!(!app.win.loading && !crate::find::more_before(&app), "the whole thread is in");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let s = screen(&term).join("\n");
    assert!(
        s.contains("the ancient bug"),
        "the view goes to it: cur {:?} anchor {:?} follow {} n {}\n{s}",
        app.find.as_ref().unwrap().cur,
        app.anchor,
        app.follow,
        app.events.len()
    );
}
