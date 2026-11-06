//! msg-forward (architect m_41072; laws in bend/laws/forward.bend): `sb
//! send --forward m_<n>` and `sb spawn --forward m_<n>` through sb-core,
//! as the receiver reads them, and the door. Out of core_tests.rs (its
//! line limit).

use super::super::*;
use bise_proto::parts::Part;

const IMG: &str = r#"{"kind":"image","name":"[Image #1]","source":"/u/s.png","mime":"image/png","b64_file":"/h/images/1.b64"}"#;

fn fwd(to: &str, note: &str, n: u64) -> AgentReq {
    AgentReq::Send { to: to.into(), text: note.into(), expect_reply: true, reply_to: None, queued: false, why: String::new(), switch: None, parts: String::new(), attach: Vec::new(), fwd: Some(n) }
}

fn his_id(t: &T, text: &str) -> u64 {
    t.hub.st.msgs().values().find(|m| m.from == USER && m.text == text).expect("his message").id
}

#[test]
fn main_forwards_his_message_whole_with_a_note() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn("docs", "ok");
    t.user(MAIN, "tiens au fait, forward my words");
    t.turn(MAIN, "on it");
    let n = his_id(&t, "tiens au fait, forward my words");
    let at = crate::search::fmt_utc(t.hub.st.msgs()[&n].created_ms);
    let (tok, fx) = t.req(MAIN, fwd("docs", "yours: design first.", n));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
    let s = say_to(&fx, "docs").expect("docs reads it");
    let want = format!(
        "yours: design first.\n\n[forwarded m_{n} · from the user to main · {at} · forwarded by main · its place: sb show m_{n}]\ntiens au fait, forward my words"
    );
    assert!(s.contains(&want), "the receiver's view:\n{s}");
    assert!(s.starts_with("<agent_message from=\"main\""), "{s}");
    // the reference is a typed field: no reader parses the header
    let m = t.hub.st.whole_msgs().find(|m| m.to == "docs" && m.from == MAIN && m.fwd.is_some()).expect("the forward").clone();
    assert_eq!(m.fwd, Some(n));
    // a plain original goes as one text part after the forwarded part
    // (architect m_41547): every client draws a forward from its parts
    let got: Vec<Part> = serde_json::from_str(&m.parts).expect("its parts");
    assert!(matches!(&got[1], Part::Forwarded { id, from, to, by, .. } if *id == n && from == USER && to == MAIN && by == MAIN), "{got:?}");
    assert_eq!(got.last(), Some(&Part::Text { text: "tiens au fait, forward my words".into() }));
}

/// file-mention-wire: a file chip in the middle of his words reaches the
/// model as a link with its name at its place, and a forward of it reads
/// the same link (the receiver's view and the forward's parts).
#[test]
fn a_file_chip_mid_sentence_is_a_link_and_a_forward_keeps_it() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn("docs", "ok");
    let parts = r#"[{"kind":"text","text":"demande d'update "},{"kind":"file","path":"/u/vibebot/The Bot.md","name":"[File #1]"},{"kind":"text","text":" pour interpoler"}]"#;
    let p: Vec<Part> = serde_json::from_str(parts).unwrap();
    let text = bise_proto::parts::render(&p);
    let link = "demande d'update [The Bot.md](/u/vibebot/The Bot.md) pour interpoler";
    assert_eq!(text, link);
    t.req(MAIN, AgentReq::Send { to: "docs".into(), text: text.clone(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, parts: parts.into(), attach: Vec::new(), fwd: None });
    let n = t.hub.st.msgs().values().find(|m| m.text == text).expect("the message").id;
    t.spawn_task("b");
    t.turn("b", "ok");
    let (tok, fx) = t.req("docs", fwd("b", "", n));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
    let s = say_to(&fx, "b").expect("b reads it");
    assert!(s.contains(link), "the receiver's view:\n{s}");
    let m = t.hub.st.whole_msgs().filter(|m| m.fwd == Some(n)).max_by_key(|m| m.id).expect("the forward").clone();
    let got: Vec<Part> = serde_json::from_str(&m.parts).expect("its parts");
    assert!(got.contains(&Part::File { path: "/u/vibebot/The Bot.md".into(), name: "[File #1]".into(), missing: false }), "{got:?}");
    assert_eq!(bise_proto::parts::render(&got), m.text);
}

/// Architect m_41547: the header has two writers (forward.bend composes
/// it, bise_proto::parts renders it). Real forwards through sb-core (a
/// note and none, a plain original, one with an image, one with a quote
/// in front, a chain): the parts render to the text the model reads.
#[test]
fn a_forwards_parts_render_to_its_text() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.turn("a", "ok");
    t.turn("b", "ok");
    t.user(MAIN, "plain words");
    t.turn(MAIN, "ok");
    let plain = his_id(&t, "plain words");
    let rich = format!(r#"[{{"kind":"quote","from":"t1","text":"the quoted line"}},{{"kind":"text","text":"look "}},{IMG},{{"kind":"paste","n":1,"text":"a\nb"}}]"#);
    let p: Vec<Part> = serde_json::from_str(&rich).unwrap();
    let text = bise_proto::parts::render(&p);
    t.req(MAIN, AgentReq::Send { to: "a".into(), text: text.clone(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, parts: rich, attach: Vec::new(), fwd: None });
    let rich = t.hub.st.msgs().values().find(|m| m.text == text).unwrap().id;
    let check = |t: &T, n: u64| {
        let m = t.hub.st.whole_msgs().filter(|m| m.fwd == Some(n)).max_by_key(|m| m.id).expect("the forward").clone();
        let got: Vec<Part> = serde_json::from_str(&m.parts).expect("its parts");
        assert_eq!(bise_proto::parts::render(&got), m.text, "the parts render to the text the model reads");
        m.id
    };
    for (who, to, note, n) in [(MAIN, "a", "see", plain), (MAIN, "b", "", plain), ("a", "b", "", rich), ("a", "b", "look at it", rich)] {
        let (tok, fx) = t.req(who, fwd(to, note, n));
        assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
        check(&t, n);
    }
    // a chain: b forwards a's forward back to a
    let last = t.hub.st.msgs().values().filter(|m| m.fwd == Some(rich)).max_by_key(|m| m.id).unwrap().id;
    let (tok, fx) = t.req("b", fwd("a", "again", last));
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{fx:?}");
    check(&t, last);
}

#[test]
fn no_note_starts_with_the_header() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn("docs", "ok");
    t.user(MAIN, "hello");
    t.turn(MAIN, "ok");
    let n = his_id(&t, "hello");
    let (_, fx) = t.req(MAIN, fwd("docs", "", n));
    let s = say_to(&fx, "docs").expect("docs reads it");
    assert!(s.contains(&format!("\">\n[forwarded m_{n} · from the user to main")), "{s}");
}

#[test]
fn only_a_message_you_got_or_sent_is_forwarded() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.turn("a", "ok");
    t.turn("b", "ok");
    t.user(MAIN, "secret plan");
    t.turn(MAIN, "ok");
    let n = his_id(&t, "secret plan");
    let before = t.hub.st.msgs().len();
    let (tok, fx) = t.req("a", fwd("b", "", n));
    let r = reply(&fx, tok).unwrap();
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("not a message you got or sent"), "{r}");
    let (tok, fx) = t.req(MAIN, fwd("b", "", 999));
    assert!(reply(&fx, tok).unwrap()["error"].as_str().unwrap().contains("m_999 unknown"));
    assert_eq!(t.hub.st.msgs().len(), before, "a refused forward sends nothing");
}

#[test]
fn a_forward_carries_the_originals_images() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.turn("a", "ok");
    t.turn("b", "ok");
    // main → a with an image (send-parts), then a forwards it to b
    let parts = format!(r#"[{{"kind":"text","text":"look\n\n"}},{IMG}]"#);
    let p: Vec<bise_proto::parts::Part> = serde_json::from_str(&parts).unwrap();
    let text = bise_proto::parts::render(&p);
    let send = AgentReq::Send { to: "a".into(), text: text.clone(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, parts, attach: Vec::new(), fwd: None };
    t.req(MAIN, send);
    let n = t.hub.st.msgs().values().find(|m| m.text == text).unwrap().id;
    let (_, fx) = t.req("a", fwd("b", "see", n));
    assert!(say_to(&fx, "b").is_some_and(|s| s.contains("<image name=\"[Image #1]\"")), "the model sees the image: {fx:?}");
    let m = t.hub.st.whole_msgs().find(|m| m.fwd == Some(n)).unwrap().clone();
    let got: Vec<bise_proto::parts::Part> = serde_json::from_str(&m.parts).expect("its parts");
    assert_eq!(bise_proto::parts::render(&got), m.text, "the parts render to the text");
}

#[test]
fn a_spawn_brief_carries_the_forward_last() {
    let mut t = T::new();
    t.user(MAIN, "build the forward");
    t.turn(MAIN, "ok");
    let n = his_id(&t, "build the forward");
    let (_, _) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "fw".into(),
            brief: Brief { objective: crate::cli::FORWARD_OBJECTIVE.into(), ..Brief::default() },
            worktree: false,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
            parts: String::new(),
            attach: Vec::new(),
            fwd: Some(n),
        },
    );
    let m = t.hub.st.whole_msgs().find(|m| m.to == "fw").expect("its brief").clone();
    assert_eq!(m.fwd, Some(n));
    assert!(m.text.starts_with("# Task `fw`"), "{}", m.text);
    assert!(m.text.contains(&format!("## Forwarded message\n\n[forwarded m_{n} · from the user to main")), "{}", m.text);
    assert!(m.text.ends_with("build the forward"), "{}", m.text);
}
