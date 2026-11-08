//! send-parts (laws agent_send_keeps_parts, agent_no_parts_no_line,
//! brief_no_parts): an agent's message with parts (`sb send --attach`,
//! the daemon's `parts`) keeps them on the message, writes them after its
//! msg-in (and the sender's `sent`, main's `msg`) by its id, and a spawn's
//! brief carries them with the task's head in front. Out of core_tests.rs
//! (its line limit).

use super::super::*;

const IMG: &str = r#"{"kind":"image","name":"[Image #1]","source":"/u/s.png","mime":"image/png","b64_file":"/h/images/1.b64"}"#;

fn parts_of(text: &str) -> String {
    format!(r#"[{{"kind":"text","text":"{text}\n\n"}},{IMG}]"#)
}

fn rendered(text: &str) -> String {
    let p: Vec<bise_proto::parts::Part> = serde_json::from_str(&parts_of(text)).unwrap();
    bise_proto::parts::render(&p)
}

fn send(to: &str, text: &str, parts: &str) -> AgentReq {
    AgentReq::Send { to: to.into(), text: text.into(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, parts: parts.into(), attach: Vec::new(), fwd: None }
}

#[test]
fn an_agents_parts_ride_on_its_message() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.turn("a", "ok");
    t.turn("b", "ok");
    // main → a: a's msg-in then its parts; main's own feed likewise
    let text = rendered("look");
    let (_, fx) = t.req(MAIN, send("a", &text, &parts_of("look")));
    let m = t.hub.st.whole_msgs().find(|m| t.hub.st.text(m.id) == text).expect("sent").clone();
    assert_eq!(t.hub.st.words(m.id).unwrap().parts, parts_of("look"), "kept in its journal line");
    assert!(m.has_parts);
    assert!(say_to(&fx, "a").is_some_and(|s| s.contains("<image name=\"[Image #1]\"")), "the model's text has the marker: {fx:?}");
    let want = json!({"sb": "msg-parts", "id": m.id, "parts": serde_json::from_str::<Value>(&parts_of("look")).unwrap()});
    let ls = recs(&fx, "a");
    let at = ls.iter().position(|r| r["sb"] == "msg-in").expect("its msg-in");
    assert_eq!(ls[at + 1], want);
    assert!(recs_of(&fx, MAIN, "msg-parts").contains(&want), "main's msg line too: {fx:?}");
    // a task → another: the sender's `sent` then sent-parts
    let text = rendered("for you");
    let (_, fx) = t.req("a", send("b", &text, &parts_of("for you")));
    let id = t.hub.st.msgs().values().find(|m| t.hub.st.text(m.id) == text).unwrap().id;
    let ls = recs(&fx, "a");
    let at = ls.iter().position(|r| r["sb"] == "sent").expect("its sent line");
    assert_eq!(ls[at + 1]["sb"], "sent-parts");
    assert_eq!(ls[at + 1]["id"], id);
    assert_eq!(recs_of(&fx, "b", "msg-parts").len(), 1, "{fx:?}");
    // without parts: no such line anywhere, the journal as before
    let (_, fx) = t.req(MAIN, send("a", "plain", ""));
    for who in [MAIN, "a"] {
        assert!(recs_of(&fx, who, "msg-parts").is_empty(), "{fx:?}");
    }
    assert!(!fx.iter().any(|e| matches!(e, Effect::Journal(j) if j.to_string().contains("\"parts\""))), "{fx:?}");
}

#[test]
fn a_brief_keeps_its_parts_with_the_head_in_front() {
    let mut t = T::new();
    let body = "## Objective\n\nlook";
    let parts = format!(r#"[{{"kind":"text","text":"{}\n\n"}},{IMG}]"#, body.replace('\n', "\\n"));
    let (_, _) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "a".into(),
            brief: Brief { objective: "look".into(), ..Brief::default() },
            worktree: false,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
            parts: parts.clone(),
            attach: Vec::new(),
            fwd: None,
        },
    );
    let id = t.hub.st.whole_msgs().find(|m| m.to == "a").expect("its brief").id;
    let m = t.hub.st.words(id).unwrap();
    let got: Vec<bise_proto::parts::Part> = serde_json::from_str(&m.parts).expect("its parts");
    assert_eq!(got[0], bise_proto::parts::Part::Text { text: "# Task `a`\n\n".into() });
    assert_eq!(bise_proto::parts::render(&got), m.text, "the text is their render");
    assert!(m.text.contains("<image name=\"[Image #1]\""), "{}", m.text);
}
