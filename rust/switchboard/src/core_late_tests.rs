//! withdrawn-answer (architect m_30039): main withdraws a card while he
//! answers it, his answer reaches the hub after the close. It goes to
//! main as a message, once, never dropped (bend/hub/gone.bend, laws
//! late_answer_*); every other closed card keeps the refusal. Out of
//! core_tests.rs (its line limit).

use super::super::*;

fn answer(t: &mut T, card: u64, text: &str, parts: &str) -> Vec<Effect> {
    t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card, text: text.into(), parts: parts.into() } })
}

fn notices(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::ToClient { body, .. } if body["ev"] == "notice" => body["text"].as_str().map(String::from),
            _ => None,
        })
        .collect()
}

/// The late answers main got: the hub's messages that say so.
fn late_msgs(t: &T) -> Vec<Value> {
    t.journal
        .borrow()
        .iter()
        .filter(|j| j["type"] == "message_sent" && j["msg"]["to"] == MAIN && j["msg"]["text"].as_str().unwrap_or("").starts_with("he answered #"))
        .map(|j| j["msg"].clone())
        .collect()
}

fn journaled(t: &T, kind: &str, card: u64) -> usize {
    t.journal.borrow().iter().filter(|j| j["type"] == kind && j["id"] == card).count()
}

/// The card main asks him about docs' question, withdrawn.
fn withdrawn_card(t: &mut T) -> u64 {
    let (_, card) = docs_question_card(t);
    let (tok, fx) = t.req(MAIN, AgentReq::Withdraw { card, why: "moot".into() });
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
    assert!(!t.hub.st.cards.contains_key(&card));
    assert_eq!(journaled(t, "card_withdrawn", card), 1);
    card
}

#[test]
fn his_answer_to_a_withdrawn_card_reaches_main_once() {
    let mut t = T::new();
    let card = withdrawn_card(&mut t);
    let words = "v2\nand keep the v1 page";
    let fx = answer(&mut t, card, words, "");
    assert!(notices(&fx).is_empty(), "no refusal: {fx:?}");
    let late = late_msgs(&t);
    assert_eq!(late.len(), 1, "{late:?}");
    let text = late[0]["text"].as_str().unwrap();
    assert!(text.starts_with(&format!("he answered #{card} after you withdrew it (\"")), "{text}");
    assert!(text.ends_with(&format!("\"):\n{words}")), "his words whole: {text}");
    assert_eq!(late[0]["from"], HUB);
    assert_eq!(journaled(&t, "card_late", card), 1);
    // once: a second answer is the refusal, nothing more reaches main
    let fx = answer(&mut t, card, "v1 after all", "");
    assert_eq!(notices(&fx), vec![format!("notification #{card} isn't open")]);
    assert_eq!(late_msgs(&t).len(), 1);
    assert_eq!(journaled(&t, "card_late", card), 1);
}

/// His parts (a quote, an image) ride on the message to main.
#[test]
fn a_late_answer_keeps_his_parts() {
    let mut t = T::new();
    let card = withdrawn_card(&mut t);
    let parts = r#"[{"kind":"quote","from":"main","text":"q"},{"kind":"text","text":"v2"}]"#;
    answer(&mut t, card, "<selection from=\"main\">\nq\n</selection>\nv2", parts);
    let late = late_msgs(&t);
    assert_eq!(late.len(), 1);
    let id = t.hub.st.whole_msgs().find(|m| t.hub.st.text(m.id).starts_with("he answered #")).expect("main's message").id;
    assert_eq!(t.hub.st.words(id).unwrap().parts, parts);
}

/// A replay rebuilds the list: withdrawn, a restart, his answer still
/// reaches main; withdrawn and answered, a restart, refused.
#[test]
fn a_restart_keeps_the_withdrawn_card_answerable_once() {
    let mut t = T::new();
    let card = withdrawn_card(&mut t);
    let events = t.journal.borrow().clone();
    let mut h = Hub::new("/w");
    assert!(h.replay(&events).is_empty(), "both kinds known");
    t.hub = h;
    answer(&mut t, card, "v2", "");
    assert_eq!(late_msgs(&t).len(), 1);
    let events = t.journal.borrow().clone();
    let mut h = Hub::new("/w");
    assert!(h.replay(&events).is_empty());
    t.hub = h;
    let fx = answer(&mut t, card, "v2 again", "");
    assert_eq!(notices(&fx), vec![format!("notification #{card} isn't open")]);
    assert_eq!(late_msgs(&t).len(), 1);
}

/// Every other card that isn't open keeps the refusal: one he closed,
/// one he answered, one that never was.
#[test]
fn any_other_closed_card_is_refused() {
    let mut t = T::new();
    let (_, closed) = docs_question_card(&mut t);
    t.user(MAIN, &format!("/close {closed}"));
    let fx = answer(&mut t, closed, "v2", "");
    assert_eq!(notices(&fx), vec![format!("notification #{closed} isn't open")]);
    t.req(MAIN, AgentReq::Card { text: "ship it?".into(), for_msg: None, page: None, artifact: None, open: Vec::new(), about: None });
    let answered = *t.hub.st.cards.keys().max().unwrap();
    answer(&mut t, answered, "yes", "");
    let fx = answer(&mut t, answered, "no", "");
    assert_eq!(notices(&fx), vec![format!("notification #{answered} isn't open")]);
    let fx = answer(&mut t, 999, "v2", "");
    assert_eq!(notices(&fx), vec!["notification #999 isn't open".to_string()]);
    assert!(late_msgs(&t).is_empty());
    assert_eq!(t.journal.borrow().iter().filter(|j| j["type"] == "card_late").count(), 0);
}
