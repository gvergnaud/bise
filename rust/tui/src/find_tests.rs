//! Find in the history (BISE-237): the scan order, the keys, the
//! counter, folds, the highlight, and a 50 000-event history.

use super::*;
use crate::wire::{Mark, ToolState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn app_with(events: Vec<Ev>) -> App {
    let mut app = crate::sb::bench::test_app();
    app.events = events;
    app.cache.clear();
    app
}

fn tool(id: u32, intent: &str, out: &str) -> Ev {
    let mut td = ToolData::bare(id, ToolState::Ok);
    td.name = Some("bash".into());
    td.code = Some("echo hi".into());
    td.intent = Some(intent.into());
    td.elapsed = Some("0.1s".into());
    td.took = Some(Duration::from_millis(100));
    td.result = Some((true, out.into()));
    Ev::Tool(td)
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    crate::input::on_key(app, &KeyEvent::new(code, m));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn draw(app: &mut App) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| crate::run::draw_frame(app, f)).unwrap();
    t
}

fn screen(t: &Terminal<TestBackend>) -> String {
    let b = t.backend().buffer();
    (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>() + "\n").collect()
}

fn counter(app: &App) -> String {
    app.find.as_ref().unwrap().counter(false, Instant::now())
}

#[test]
fn lower_keeps_the_bytes_where_they_are() {
    for s in ["Déjà VU", "İstanbul", "ẞig", "ÉCOLE 日本 Ω"] {
        assert_eq!(lower(s).len(), s.len(), "{s}");
    }
    assert_eq!(lower("Déjà VU"), "déjà vu");
    // columns, not bytes; wide chars count 2
    assert_eq!(matches_in("日本 Signup and signup", "signup", false), vec![(5, 11), (16, 22)]);
    assert_eq!(matches_in("Signup signup", "Signup", true), vec![(0, 6)]);
    assert!(matches_in("abc", "", false).is_empty());
}

#[test]
fn ctrl_f_opens_the_field_and_esc_closes_it() {
    let mut app = app_with(vec![Ev::You("ship the signup page".into(), Mark::Sent, false)]);
    app.ed.insert("my draft");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert!(app.find.is_some());
    let t = draw(&mut app);
    let s = screen(&t);
    assert!(s.contains("⌕   find in main"), "the box names the feed: {s}");
    assert!(s.contains("you → main"), "the divider stays: {s}");
    assert!(s.contains("my draft"), "the composer stays with its draft: {s}");
    assert!(s.contains("⏎ older   shift+⏎ newer   esc close"), "{s}");
    typed(&mut app, "sign");
    assert_eq!(app.find.as_ref().unwrap().ed.text, "sign");
    assert_eq!(app.ed.text, "my draft", "the draft waits");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.find.is_none());
    let s = screen(&draw(&mut app));
    assert!(s.contains("my draft"), "{s}");
}

#[test]
fn messages_come_first_then_up_and_down_with_a_counter() {
    let mut app = app_with(vec![
        Ev::You("deploy the site".into(), Mark::Sent, false),     // 0
        Ev::Assistant("deploy done, then deploy docs".into()), // 1: 2 matches
        tool(7, "run the deploy script", "ok"),               // 2
        Ev::Info("nothing here".into()),                      // 3
    ]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "deploy");
    draw(&mut app);
    let f = app.find.as_ref().unwrap();
    // the newest message wins over the newer tool call
    assert_eq!(f.cur, Some((1, 1)));
    assert_eq!(counter(&app), "3/4");
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    press(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)));
    assert_eq!(counter(&app), "1/4");
    // past the oldest: back to the newest, and it says so
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((2, 0)));
    assert_eq!(counter(&app), "back to the newest");
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)));
    assert_eq!(counter(&app), "back to the oldest");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    // no match, then smart-case
    typed(&mut app, "zz");
    draw(&mut app);
    assert_eq!(counter(&app), "no match");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typed(&mut app, "Deploy");
    draw(&mut app);
    assert_eq!(counter(&app), "no match", "no `Deploy` with a capital");
    assert_eq!(app.find.as_ref().unwrap().cur, None);
}

#[test]
fn the_matches_are_painted_the_current_one_on_the_accent() {
    let mut app = app_with(vec![Ev::You("alpha signup beta signup".into(), Mark::Sent, false)]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "signup");
    let t = draw(&mut app);
    let b = t.backend().buffer();
    let s = screen(&t);
    let y = s.lines().position(|l| l.contains("alpha signup")).expect("the message is shown") as u16;
    let row: String = (0..100).map(|x| b[(x, y)].symbol()).collect();
    let x1 = row.find("signup").unwrap() as u16;
    let x2 = row.rfind("signup").unwrap() as u16;
    // the newest match is current: the second one
    assert_eq!(b[(x2, y)].bg, crate::theme::accent());
    assert_eq!(b[(x1, y)].bg, crate::theme::pill_bg());
    assert!(s.contains("2/2"), "{s}");
    // the match is on the history's first row, the view cannot go up:
    // the box goes to the bottom-right, off the match
    let field = s.lines().position(|l| l.contains('⌕')).expect("the box");
    assert!(field as u16 > y + 3, "the box under the match: {s}");
}

#[test]
fn a_match_in_a_closed_call_opens_it_and_moving_on_closes_it() {
    let out = (1..=40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n") + "\nnpm publish done";
    let mut app = app_with(vec![
        Ev::You("publish it".into(), Mark::Sent, false),
        tool(3, "release the package", &out),
        Ev::Assistant("released".into()),
    ]);
    draw(&mut app);
    let closed = |app: &App| matches!(&app.events[1], Ev::Tool(td) if !td.opened && !td.expanded);
    assert!(closed(&app));
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "npm publish");
    let s = screen(&draw(&mut app));
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    assert!(matches!(&app.events[1], Ev::Tool(td) if td.opened && td.expanded));
    assert!(s.contains("npm publish done"), "the box is open on the match: {s}");
    // a message match next: the call closes again
    typed(&mut app, "\u{8}");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typed(&mut app, "publish");
    draw(&mut app);
    assert!(closed(&app), "{:?}", app.find.as_ref().unwrap().cur);
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(closed(&app));
}

#[test]
fn the_view_moves_to_an_old_match_and_stays_there_on_esc() {
    let mut events = vec![Ev::You("the needle is here".into(), Mark::Sent, false)];
    for i in 0..200 {
        events.push(Ev::Assistant(format!("filler reply {i}")));
    }
    let mut app = app_with(events);
    draw(&mut app);
    assert!(app.follow);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "needle");
    let s = screen(&draw(&mut app));
    assert!(!app.follow);
    assert!(s.contains("the needle is here"), "{s}");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let s = screen(&draw(&mut app));
    assert!(s.contains("the needle is here"), "pinned on the match: {s}");
    assert!(!app.follow);
}

#[test]
fn a_paste_goes_to_the_query_and_new_lines_are_searched() {
    let mut app = app_with(vec![Ev::You("one".into(), Mark::Sent, false)]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    // ctrl+w after a wide blank (fuzz): cut at a char boundary
    crate::input::on_paste(&mut app, "x\u{3000}y");
    press(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    crate::input::on_paste(&mut app, "late\nnews");
    assert_eq!(app.find.as_ref().unwrap().ed.text, "late news");
    draw(&mut app);
    assert_eq!(counter(&app), "no match");
    app.events.push(Ev::Assistant("the late news".into()));
    draw(&mut app);
    assert_eq!(counter(&app), "1/1");
}

/// A long history: 50 000 events (your messages, replies, calls with
/// 2 KB outputs). Opening and typing never block a frame longer than
/// the budget (plus one event); messages are found before the calls.
/// `cargo test -p bend-tui --release find_is_fast -- --nocapture`
/// prints the numbers.
#[test]
fn find_is_fast_on_50k_events() {
    let out: String = (0..40).map(|i| format!("out line {i} of the build, nothing to see\n")).collect();
    let mut events = Vec::with_capacity(50_000);
    for i in 0..50_000u32 {
        events.push(match i % 4 {
            0 => Ev::You(format!("message {i}: please check the login flow and the signup page"), Mark::Sent, false),
            1 => Ev::Assistant(format!("reply {i}: I checked the **login** flow; the signup page works. {}", "More words. ".repeat(20))),
            _ => tool(i, "run the build", &out),
        });
    }
    events[10].clone_from(&Ev::You("the rare zebra word".into(), Mark::Sent, false));
    let mut f = Find::new("bench", events.len(), None);
    let budget = Duration::from_millis(6);
    let mut slow = Duration::ZERO;
    let mut steps = 0;
    let t0 = Instant::now();
    f.ed.set("zebra", 5);
    f.restart();
    while f.busy() {
        let t = Instant::now();
        f.scan(&events, budget);
        slow = slow.max(t.elapsed());
        steps += 1;
    }
    let first = t0.elapsed();
    assert_eq!(f.total, 1);
    assert_eq!(f.cur, Some((10, 0)));
    // a second query: the index is built, only the scan runs
    let t1 = Instant::now();
    f.ed.set("signup", 6);
    f.restart();
    let mut steps2 = 0;
    let mut found_at = None;
    while f.busy() {
        let t = Instant::now();
        f.scan(&events, budget);
        slow = slow.max(t.elapsed());
        steps2 += 1;
        if found_at.is_none() && f.cur.is_some() {
            found_at = Some(steps2);
        }
    }
    let second = t1.elapsed();
    assert_eq!(f.total, 25_000);
    assert_eq!(found_at, Some(1), "the newest message is found in the first slice");
    assert_eq!(f.cur, Some((49_997, 0)));
    eprintln!(
        "find 50k events: first query {:?} in {} slices (index built), next query {:?} in {} slices, slowest slice {:?}",
        first, steps, second, steps2, slow
    );
    // a slice stops at the budget (checked every 64 events); generous
    // for a debug build on a busy machine
    assert!(slow < budget + Duration::from_millis(150), "slowest slice {:?}", slow);
}

#[test]
fn a_match_in_the_folded_part_of_your_message_opens_it() {
    // BISE-239: your long message shows 20 rows (BISE-262); find opens it on a match
    // in the hidden lines, esc folds it back
    let text = (1..=30).map(|n| if n == 27 { "the hidden zebra".to_string() } else { format!("line {n}") }).collect::<Vec<_>>().join("\n");
    let mut app = app_with(vec![Ev::You(text, Mark::Sent, false)]);
    let s = screen(&draw(&mut app));
    assert!(!s.contains("zebra") && s.contains("more lines"), "{s}");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "zebra");
    let s = screen(&draw(&mut app));
    assert!(s.contains("the hidden zebra"), "{s}");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let s = screen(&draw(&mut app));
    assert!(!s.contains("zebra") && s.contains("more lines"), "{s}");
}

/// cmd+f that the terminal passes through (SUPER under the kitty
/// keyboard protocol) opens the field like ctrl+f and goes older in it;
/// ctrl+f still works; cmd+shift+f and cmd alone do not open it.
#[test]
fn cmd_f_opens_the_field_like_ctrl_f() {
    let mut app = app_with(vec![Ev::You("ship the signup page".into(), Mark::Sent, false)]);
    press(&mut app, KeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftSuper), KeyModifiers::SUPER);
    assert!(app.find.is_none());
    assert!(!app.cmd_keys, "cmd alone (cmd+tab) says nothing about cmd+f");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::SUPER | KeyModifiers::SHIFT);
    assert!(app.find.is_none());
    press(&mut app, KeyCode::Char('f'), KeyModifiers::SUPER);
    assert!(app.find.is_some());
    assert!(app.find.as_ref().unwrap().ed.text.is_empty(), "no 'f' typed");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.find.is_none());
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert!(app.find.is_some(), "ctrl+f stays");
}

/// The help says ctrl+f until a cmd key arrives (the terminal passes
/// them), then cmd+f; the ctrl hints always ctrl+f, the cmd hints
/// (BISE-277) cmd+f once they show.
#[test]
fn the_hints_say_cmd_f_once_a_cmd_key_arrived() {
    use crate::ctrlhint::{Held, Hold};
    let mut app = app_with(vec![Ev::You("ship the signup page".into(), Mark::Sent, false)]);
    let held_long = |h| Hold::of(h, std::time::Instant::now() - std::time::Duration::from_secs(1));
    let find_key = |app: &mut App, h| {
        app.hold = held_long(h);
        let k = crate::ctrlhint::pairs(app).into_iter().find(|p| p.1 == "find").map(|p| p.0);
        app.hold = Hold::default();
        k
    };
    let help_keys = |app: &App| {
        let r = crate::help::rows(crate::help::Page::Help, "find in the history", app.cmd_keys, true);
        r.iter().map(|r| r.keys).collect::<Vec<_>>()
    };
    assert_eq!(find_key(&mut app, Held::Ctrl), Some("ctrl+f"));
    // cmd held before any cmd key: no cmd hints at all
    app.hold = held_long(Held::Cmd);
    assert!(!crate::ctrlhint::on(&app));
    app.hold = Hold::default();
    assert_eq!(help_keys(&app), ["ctrl+f"]);
    // any cmd key, e.g. cmd+c that Ghostty passes on without a selection
    press(&mut app, KeyCode::Char('c'), KeyModifiers::SUPER);
    assert!(app.cmd_keys);
    assert_eq!(find_key(&mut app, Held::Ctrl), Some("ctrl+f"));
    assert_eq!(find_key(&mut app, Held::Cmd), Some("cmd+f"));
    assert_eq!(help_keys(&app), ["cmd+f|ctrl+f"]);
    let lines = crate::help::page_lines(crate::help::Page::Shortcuts, "find in", &[], 80, true, true);
    let all: String = lines.iter().flat_map(|l| l.spans.iter().map(|s| s.content.to_string())).collect();
    assert!(all.contains(" cmd+f ") && all.contains(" ctrl+f "), "{all}");
}

/// The box (user QA 2026-10-04): flush in the top-right corner of the
/// history pane (the row under the frame's top edge, its right border
/// against the panel's rule), 48 columns, rounded dim border on the
/// raised grey, ` ⌕ signup  1/1  ↑  ↓  × `; the composer under it keeps
/// its draft, no caret (the box has the keys).
#[test]
fn the_box_sits_in_the_corner_and_the_composer_stays() {
    // the match low on the screen: the box stays in its corner
    let mut events: Vec<Ev> = (0..40).map(|i| Ev::Assistant(format!("filler {i}"))).collect();
    events.push(Ev::You("ship the signup page".into(), Mark::Sent, false));
    let mut app = app_with(events);
    app.ed.insert("my draft");
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "signup");
    let t = draw(&mut app);
    let b = t.backend().buffer();
    let s = screen(&t);
    let lines: Vec<&str> = s.lines().collect();
    let top = lines.iter().position(|l| l.contains('⌕')).expect("the box's field") - 1;
    let field = lines[top + 1];
    assert!(lines[top + 2].contains('╰'), "{s}");
    assert!(field.contains("⌕ signup") && field.contains("1/1  ↑  ↓  × │"), "{field}");
    // 48 columns wide, in the corner: right under the frame's top edge,
    // its right border against the panel's rule
    assert_eq!(top, 1, "under the top edge: {s}");
    let row: Vec<&str> = (0..b.area.width).map(|x| b[(x, top as u16)].symbol()).collect();
    let x0 = row.iter().skip(1).position(|&c| c == "╭").unwrap() + 1;
    let x1 = x0 + row[x0..].iter().position(|&c| c == "╮").unwrap();
    assert_eq!(x1 - x0 + 1, 48, "{s}");
    let rule = (0..b.area.width).find(|&x| b[(x, 5)].symbol() == "│" && x > 2).expect("the panel's rule") as usize;
    assert_eq!(x1 + 1, rule, "against the rule: {s}");
    // the border dim, the inside on the raised grey
    assert_eq!(b[(x0 as u16, top as u16)].fg, crate::theme::dim());
    assert_eq!(b[(x0 as u16 + 3, top as u16 + 1)].bg, crate::theme::raised());
    // the composer keeps its draft, without a caret
    let dy = lines.iter().position(|l| l.contains("my draft")).expect("the draft") as u16;
    assert!(dy as usize > top + 2, "the draft is the composer's: {s}");
    let caret = (0..b.area.width).any(|x| b[(x, dy)].modifier.contains(Modifier::REVERSED));
    assert!(!caret, "the box has the caret, not the composer");
    assert_eq!(app.ed.text, "my draft");
    // esc: the box closes, the composer has the keys again
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    typed(&mut app, "!");
    assert_eq!(app.ed.text, "my draft!");
    let s = screen(&draw(&mut app));
    assert!(!s.contains('⌕'), "{s}");
}


/// `no match` in the error red; the empty field names the feed.
#[test]
fn no_match_is_red() {
    let mut app = app_with(vec![Ev::You("hello".into(), Mark::Sent, false)]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "zz");
    let t = draw(&mut app);
    let b = t.backend().buffer();
    let s = screen(&t);
    let y = s.lines().position(|l| l.contains("no match")).expect("the counter") as u16;
    let x = s.lines().nth(y as usize).unwrap().split("no match").next().unwrap().chars().count() as u16;
    assert_eq!(b[(x, y)].fg, crate::theme::error());
}

/// A match under the box is not "on screen": the view moves so the
/// current match lands under the box, 4 rows down (designer).
#[test]
fn the_current_match_is_never_under_the_box() {
    let mut events: Vec<Ev> = (0..60).map(|i| Ev::Assistant(format!("filler reply {i}"))).collect();
    events.push(Ev::Assistant("the target line".into()));
    for i in 0..60 {
        events.push(Ev::Assistant(format!("more filler {i}")));
    }
    let mut app = app_with(events);
    draw(&mut app);
    // the target on the history's first row, where the box goes
    let at = app.vis_events.iter().position(|&e| e == 60);
    assert!(at.is_none());
    app.follow = false;
    app.anchor = (60, 0);
    app.scroll = 0;
    draw(&mut app);
    assert_eq!(app.vis_events.first(), Some(&60), "the target on the first row");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "target");
    draw(&mut app);
    let loc = app.find.as_ref().unwrap().loc.expect("the match is drawn");
    let row = (0..app.vis_events.len()).position(|y| app.vis_events[y] == 60 && app.vis_rows[y] == loc.row).expect("on screen");
    // the box covers the history's first rows from under the frame's
    // top edge: the match lands under it, 1 blank row between
    let cover = app.find.as_ref().unwrap().cover;
    assert!(cover > 0 && cover <= crate::find_bar::BOX_H as usize, "{cover}");
    assert_eq!(row, cover + 1, "under the box, 1 blank row between");
}

/// A click in your message takes the keys back: the box closes.
#[test]
fn a_click_in_the_composer_closes_the_box() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut app = app_with(vec![Ev::You("hello".into(), Mark::Sent, false)]);
    app.ed.insert("my draft");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    draw(&mut app);
    let (x, y) = (app.composer.x + 2, app.composer.y);
    let m = |kind| MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE };
    crate::input::on_mouse(&mut app, &m(MouseEventKind::Down(MouseButton::Left)), 30);
    crate::input::on_mouse(&mut app, &m(MouseEventKind::Up(MouseButton::Left)), 30);
    assert!(app.find.is_none());
    assert_eq!(app.ed.text, "my draft");
}

/// The hold hints while the box is open (BISE-277): its own ctrl keys.
#[test]
fn the_ctrl_hints_are_the_boxs_keys() {
    let mut app = app_with(vec![Ev::You("hello".into(), Mark::Sent, false)]);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    app.hold = crate::ctrlhint::Hold::of(crate::ctrlhint::Held::Ctrl, Instant::now() - Duration::from_secs(2));
    let p = crate::ctrlhint::pairs(&app);
    assert!(p.contains(&("ctrl+f", "older")) && p.contains(&("ctrl+u", "clear")), "{p:?}");
    assert!(!p.iter().any(|(k, _)| *k == "ctrl+o" || *k == "ctrl+l"), "{p:?}");
}
