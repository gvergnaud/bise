//! The demo's guided tips (tour.rs) on a real app: the hub's states,
//! your keys, the drawn frame.

use super::*;
use super::hub_reads::rows_for_tests;
use crate::tour::{self, Tip};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

const OBJ: &str = "bise demo (role-play, not real work): you are";

fn agent(name: &str, status: &str) -> bise_proto::rows::Agent {
    let objective = if name == "main" { String::new() } else { format!("{OBJ} {name}") };
    rows_for_tests::agent(name, status, &objective)
}

/// hub/agents and hub/cards as the hub sends them; `cards`: the older
/// rows' id, kind, agent and text.
fn state(app: &mut App, agents: &[(&str, &str)], cards: Value) {
    let agents = agents.iter().map(|(n, s)| agent(n, s)).collect();
    let cards = cards
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| rows_for_tests::card(c["id"].as_u64().unwrap(), c["kind"].as_str().unwrap(), c["agent"].as_str().unwrap(), c["text"].as_str().unwrap_or("")))
        .collect();
    rows_for_tests::apply(app, agents, cards);
}

fn screen(app: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(150, 40)).unwrap();
    t.draw(|f| crate::run::draw_frame(app, f)).unwrap();
    let b = t.backend().buffer().clone();
    (0..40).map(|y| (0..150).map(|x| b[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// The tip text on screen, its box's rows joined (the box wraps it).
fn shows(sc: &str, words: &str) -> bool {
    let flat: String = sc.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.contains(words)
}

#[test]
fn the_demo_walks_its_tips() {
    tour::reset();
    let mut app = bench::test_app_drained();
    let team = |pm: &'static str| [("main", "working"), ("pm", pm), ("designer", "working"), ("dev-api", "working")];
    state(&mut app, &[("main", "idle")], json!([]));
    assert_eq!(tour::current(&app), None);
    // the team starts: ⌥1 looks inside pm, left of the panel
    state(&mut app, &team("working"), json!([]));
    assert_eq!(tour::current(&app), Some(Tip::Start));
    let sc = screen(&mut app);
    assert!(shows(&sc, "your team just started."), "{sc}");
    assert!(shows(&sc, "looks inside pm. →"), "{sc}");
    // inside pm
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT));
    assert_eq!(app.sb.focus, "pm");
    assert_eq!(tour::current(&app), Some(Tip::Inside));
    assert!(shows(&screen(&mut app), "this is pm's own thread, live."));
    // back in main; designer's card
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.sb.focus, "main");
    let card = json!([{"id": 7, "kind": "question", "agent": "designer", "text": "where does the button go?\n1. next to the filters\n2. in the ⋯ menu"}]);
    state(&mut app, &team("done"), card);
    assert_eq!(tour::current(&app), Some(Tip::Card));
    // answered: a word for dev-api
    state(&mut app, &team("done"), json!([]));
    assert_eq!(tour::current(&app), Some(Tip::Steer));
    let sc = screen(&mut app);
    assert!(shows(&sc, "dev-api waits for a word from"), "{sc}");
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT));
    assert_eq!(tour::current(&app), Some(Tip::Steer));
    handle_input(&mut app, "ship it friday");
    assert_eq!(tour::current(&app), Some(Tip::Steer), "it stays while the marks move");
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(tour::current(&app), None);
    // main drops pm: the end
    state(&mut app, &[("main", "working"), ("pm", "archived"), ("designer", "done"), ("dev-api", "done")], json!([]));
    assert_eq!(tour::current(&app), Some(Tip::End));
    let sc = screen(&mut app);
    assert!(shows(&sc, "that's it. pm is archived:"), "{sc}");
    // ctrl+s: over
    crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(app.palette.is_some());
    assert!(!shows(&screen(&mut app), "that's it."));
    assert_eq!(tour::current(&app), None);
}

#[test]
fn no_tips_for_other_agents() {
    tour::reset();
    let mut app = bench::test_app_drained();
    state(&mut app, &[("main", "idle")], json!([]));
    rows_for_tests::apply(&mut app, vec![agent("main", "idle"), rows_for_tests::agent("fix-login", "working", "fix the login")], vec![]);
    assert_eq!(tour::current(&app), None);
}
