//! No number a client sends panics the hub (crate::core_num): the user's
//! `/close`, `/answer` and an agent's `sb close`, `sb card --withdraw`,
//! `sb wait`, `sb ask` with huge, negative or garbage values get one
//! line naming the value, and the hub still answers the next command.

use super::*;

fn notices(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::ToClient { body, .. } if body["ev"] == "notice" => body["text"].as_str().map(String::from),
            _ => None,
        })
        .collect()
}

/// The hub still works: a card closes after the bad input.
fn still_up(t: &mut T) {
    let (_, card) = docs_question_card(t);
    let fx = t.user(MAIN, &format!("/close {}", card));
    assert!(has_line(&fx, MAIN, &format!("#{} closed", card)), "{:?}", fx);
}

#[test]
fn the_user_s_close_and_answer_refuse_a_bad_number_and_the_hub_stays_up() {
    let mut t = T::new();
    let big = "281474976710656"; // core_num::MAX + 1
    let cases: &[(&str, &str)] = &[
        ("/close 999", "no open card #999"),
        (&format!("/close {}", big), &format!("no open card #{}", big)),
        ("/close 18446744073709551615", "no open card #18446744073709551615"),
        ("/close 99999999999999999999999", "no open card #99999999999999999999999"),
        ("/close -1", "not a card number: -1"),
        ("/close abc", "not a card number: abc"),
        (&format!("/answer {} yes", big), &format!("no open card #{}", big)),
        ("/answer -3 yes", "not a card number: -3"),
        ("/answer x yes", "not a card number: x"),
    ];
    for (input, want) in cases {
        let fx = t.user(MAIN, input);
        let said: Vec<String> = notices(&fx)
            .into_iter()
            .chain(fx.iter().filter_map(|e| match e {
                Effect::Line { line, .. } => Some(line.clone()),
                _ => None,
            }))
            .collect();
        assert!(said.iter().any(|l| l.contains(want)), "{}: want {:?} in {:?}", input, want, fx);
    }
    still_up(&mut t);
}

#[test]
fn an_agent_s_numbers_over_what_sb_core_reads_are_refused_naming_them() {
    let mut t = T::new();
    let big: u64 = crate::core_num::MAX + 1;
    let reqs = vec![
        AgentReq::Close { card: big, note: String::new() },
        AgentReq::Close { card: u64::MAX, note: String::new() },
        AgentReq::Withdraw { card: big, why: "x".into() },
        AgentReq::Wait { msg: big, timeout_s: 1 },
        AgentReq::Wait { msg: 1, timeout_s: u64::MAX },
        AgentReq::Ask { to: "docs".into(), text: "q".into(), timeout_s: u64::MAX },
    ];
    for req in reqs {
        let what = format!("{:?}", req);
        let (tok, fx) = t.req(MAIN, req);
        let r = reply(&fx, tok).unwrap_or_else(|| panic!("{}: no reply in {:?}", what, fx));
        assert_eq!(r["ok"], false, "{}: {}", what, r);
        assert!(r["error"].as_str().unwrap_or("").contains("too large a number"), "{}: {}", what, r);
    }
    still_up(&mut t);
}

#[test]
fn an_agent_s_garbage_numbers_are_refused_at_parse() {
    use serde_json::json;
    for v in [
        json!({"cmd": "close", "card": -1}),
        json!({"cmd": "close", "card": "abc"}),
        json!({"cmd": "close", "card": 1.5}),
        json!({"cmd": "withdraw", "card": -4, "why": "x"}),
        json!({"cmd": "wait", "msg": "m_x"}),
        json!({"cmd": "every", "step": "stop", "id": -1}),
    ] {
        assert!(AgentReq::from_json(&v).is_err(), "{}", v);
    }
}
