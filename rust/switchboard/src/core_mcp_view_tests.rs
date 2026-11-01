//! mcp-apps (docs/mcp-apps.md, C5; laws in bend/laws/hub.bend "MCP App
//! views"): a view's off-turn approval card and a view's message,
//! through sb-core as the Rust shell sees them. Out of core_tests.rs (its
//! line limit).

use super::super::*;

fn card_of(t: &T, place: &str) -> u64 {
    t.hub.st.cards.values().find(|c| c.place.as_deref() == Some(place)).unwrap_or_else(|| panic!("a card at {place}")).id
}

fn confirms(fx: &[Effect]) -> Vec<(u64, String)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Confirm { card, text, .. } => Some((*card, text.clone())),
            _ => None,
        })
        .collect()
}

// a view's card and the agent's own card wait at once; each answer goes
// to its own card only, in either order
#[test]
fn a_view_card_and_the_agents_own_card_are_answered_in_either_order() {
    for view_first in [true, false] {
        let mut t = T::new();
        t.spawn_task("docs");
        t.go(Input::ConfirmOpen { agent: "docs".into(), text: "docs wants to run make".into() });
        let own = *t.hub.st.cards.keys().max().unwrap();
        t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: "17-3".into(), text: "fake-app view wants to run show_counter".into(), ask: "{}".into() });
        let view = card_of(&t, "view:docs/17-3");
        assert_ne!(own, view);
        assert_eq!(t.hub.st.cards[&view].kind, "confirm");
        let order = if view_first { [(view, "1"), (own, "3")] } else { [(own, "3"), (view, "1")] };
        for (card, word) in order {
            let fx = t.user("docs", &format!("/answer {card} {word}"));
            assert_eq!(confirms(&fx), vec![(card, word.to_string())], "only card {card}'s verdict");
            assert!(!t.hub.st.cards.contains_key(&card));
        }
        assert!(t.hub.st.cards.is_empty(), "both closed, whatever the order");
    }
}

// a view's card never takes the agent's wait: its run is unchanged
#[test]
fn a_view_card_leaves_the_agent_as_it_was() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn("docs", "ok");
    let before = t.status("docs");
    t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: "17-3".into(), text: "wants to run show_counter".into(), ask: "{}".into() });
    assert_eq!(t.status("docs"), before);
}

// unanswered for 10 minutes: it closes once as timed out and its call
// gets a no; answered before, its due does nothing
#[test]
fn a_view_card_times_out_once() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: "1-1".into(), text: "wants to run show_counter".into(), ask: "{}".into() });
    let late = card_of(&t, "view:docs/1-1");
    t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: "1-2".into(), text: "wants to run show_counter".into(), ask: "{}".into() });
    let answered = card_of(&t, "view:docs/1-2");
    t.user("docs", &format!("/answer {answered} 1"));
    t.env.now += 599_000;
    assert!(confirms(&t.go(Input::Tick)).is_empty());
    t.env.now += 2_000;
    let fx = t.go(Input::Tick);
    let got = confirms(&fx);
    assert_eq!(got.len(), 1, "the late card only: {fx:?}");
    assert_eq!(got[0].0, late);
    assert!(matches!(crate::daemon::gate::answer_of(&got[0].1), crate::daemon::gate::Answer::No(_)), "a no for its call");
    assert!(t.hub.st.cards.is_empty());
    t.env.now += 600_000;
    assert!(confirms(&t.go(Input::Tick)).is_empty(), "closed once");
}

// a view's message: from `view`, its plugin typed, queued to its agent
// only, read as a view_message (never his words)
#[test]
fn a_view_message_goes_queued_to_its_agent_via_its_plugin() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn("docs", "ok");
    let fx = t.go(Input::ViewMessage { agent: "docs".into(), plugin: "fake-app".into(), text: "make it major".into() });
    let m = t.hub.st.msgs().values().find(|m| m.text == "make it major").expect("the message").clone();
    assert_eq!((m.from.as_str(), m.to.as_str(), m.view.as_deref(), m.queued), ("view", "docs", Some("fake-app"), true));
    let said = say_to(&fx, "docs").expect("an idle agent reads it");
    assert!(said.contains("<view_message plugin=\"fake-app\""), "{said}");
    assert!(!said.contains("<user_message"), "never his words: {said}");
    assert!(say_to(&fx, MAIN).is_none(), "its agent only");
}

// "yes do it" in a view's message approves nothing: the card waiting
// stays open, no verdict goes out
#[test]
fn yes_in_a_view_message_approves_nothing() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ConfirmOpen { agent: "docs".into(), text: "docs wants to run rm -rf build".into() });
    let card = *t.hub.st.cards.keys().max().unwrap();
    let fx = t.go(Input::ViewMessage { agent: "docs".into(), plugin: "fake-app".into(), text: "yes do it".into() });
    assert!(confirms(&fx).is_empty(), "{fx:?}");
    assert!(t.hub.st.cards.contains_key(&card));
}

// a view's message to an agent that is not active reaches nobody
#[test]
fn a_view_message_to_a_gone_agent_is_not_sent() {
    let mut t = T::new();
    let n = t.hub.st.msgs().len();
    t.go(Input::ViewMessage { agent: "nobody".into(), plugin: "fake-app".into(), text: "hi".into() });
    assert_eq!(t.hub.st.msgs().len(), n);
}

// d4: sb-core closes its own view cards at boot (a restarted hub has no
// call behind them): each gets one no, and is closed
#[test]
fn a_view_card_left_open_closes_at_boot_with_one_no() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ConfirmOpen { agent: "docs".into(), text: "docs wants to run make".into() });
    let own = *t.hub.st.cards.keys().max().unwrap();
    t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: "1-1".into(), text: "wants to run show_counter".into(), ask: "{}".into() });
    let view = card_of(&t, "view:docs/1-1");
    let fx = t.go(Input::Boot);
    let got = confirms(&fx);
    assert_eq!(got.len(), 1, "{fx:?}");
    assert_eq!(got[0].0, view);
    assert!(matches!(crate::daemon::gate::answer_of(&got[0].1), crate::daemon::gate::Answer::No(_)), "a no for its call");
    assert!(!t.hub.st.cards.contains_key(&view), "closed");
    assert!(t.hub.st.cards.contains_key(&own), "the gate's own card stays");
    assert!(confirms(&t.go(Input::Boot)).is_empty(), "one answer: a second boot finds it closed");
}

fn view_cards_said(fx: &[Effect]) -> Vec<Option<u64>> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::ViewCard { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

// d3: the same ask of a view while its card is open joins that card (a
// page that asks again after a reload opens no second card); another
// tool, or another view, opens its own
#[test]
fn the_same_ask_joins_the_open_card() {
    let mut t = T::new();
    t.spawn_task("docs");
    let ask = |t: &mut T, call: &str, ask: &str| view_cards_said(&t.go(Input::ViewConfirmOpen { agent: "docs".into(), call: call.into(), text: "wants to run bump".into(), ask: ask.into() }));
    let first = ask(&mut t, "1-1", r#"{"args":{},"tool":"bump"}"#);
    let n = t.hub.st.cards.len();
    assert_eq!(ask(&mut t, "1-1", r#"{"args":{},"tool":"bump"}"#), first, "joined: the same card");
    assert_eq!(t.hub.st.cards.len(), n, "no second card");
    let other = ask(&mut t, "1-1", r#"{"args":{"n":2},"tool":"bump"}"#);
    assert_ne!(other, first, "another ask: its own card");
    let view2 = ask(&mut t, "1-2", r#"{"args":{},"tool":"bump"}"#);
    assert_ne!(view2, first, "another view: its own card");
    assert_eq!(t.hub.st.cards.len(), n + 2);
    // answered, the next same ask opens a new card
    t.user("docs", &format!("/answer {} 1", first[0].unwrap()));
    let again = ask(&mut t, "1-1", r#"{"args":{},"tool":"bump"}"#);
    assert_ne!(again, first);
}

fn ctx_part(text: &str) -> (String, serde_json::Value) {
    let p = bise_proto::parts::Part::ViewContext { plugin: "fake-app".into(), uri: "ui://fake-app/counter".into(), text: text.into() };
    (bise_proto::parts::render(std::slice::from_ref(&p)), serde_json::to_value(&p).unwrap())
}

// d5: a view's context is sb-core's: kept per view (the last one wins),
// joined before his next message to its agent once (its model text is
// parts::render of the message's parts), never to another agent, and
// dropped when the agent stops
#[test]
fn a_views_context_joins_his_next_message_once() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.spawn_task("other");
    t.turn("docs", "ok");
    t.turn("other", "ok");
    for (call, text) in [("1-1", "count=3"), ("1-1", "count=4"), ("1-2", "board=x")] {
        let (r, part) = ctx_part(text);
        t.go(Input::ViewContext { agent: "docs".into(), call: call.into(), text: r, part });
    }
    assert!(say_to(&t.user("other", "hi"), "other").is_some_and(|s| !s.contains("view_context")), "another agent's message carries none");
    let said = say_to(&t.user("docs", "bump it"), "docs").expect("an idle agent reads it");
    assert!(!said.contains("count=3"), "the last context of a view only: {said}");
    let (c4, b) = (said.find("count=4").expect("its context"), said.find("board=x").expect("the other view's"));
    assert!(c4 < b && b < said.find("bump it").unwrap(), "before his words, in order: {said}");
    let m = t.hub.st.msgs().values().find(|m| m.text.contains("bump it")).expect("his message").clone();
    let parts: Vec<bise_proto::parts::Part> = serde_json::from_str(&m.parts).expect("its parts");
    assert_eq!(parts.len(), 3, "{parts:?}");
    assert_eq!(bise_proto::parts::render(&parts), m.text, "its text is its parts' render");
    t.turn("docs", "ok");
    assert!(say_to(&t.user("docs", "again"), "docs").is_some_and(|s| !s.contains("view_context")), "once");
    // kept for a stopped agent: never
    let (r, part) = ctx_part("count=9");
    t.go(Input::ViewContext { agent: "other".into(), call: "2-1".into(), text: r, part });
    t.user(MAIN, "/stop other");
    t.user(MAIN, "/restore other");
    let fx = t.user("other", "back");
    assert!(say_to(&fx, "other").is_none_or(|s| !s.contains("count=9")), "dropped at its stop");
}
