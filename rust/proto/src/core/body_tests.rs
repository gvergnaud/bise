use super::*;
use crate::core::Feed;
use serde_json::json;

fn words(id: u64) -> Result<Words, Missing> {
    match id {
        1 => Ok(Words { text: "  hello\n\tworld  ".into(), parts: String::new() }),
        2 => Ok(Words { text: "with parts".into(), parts: r#"[{"kind":"text","text":"with parts"}]"#.into() }),
        3 => Ok(Words { text: "x".into(), parts: "not json".into() }),
        _ => Err(Missing(id)),
    }
}

#[test]
fn each_body_reads_its_words() {
    let v = |b: Body| value(&b, FieldKind::Body, &mut words);
    let j = |b: Body| value(&b, FieldKind::BodyJson, &mut words);
    assert_eq!(v(Body::Lit { text: "said".into() }), json!("said"));
    assert_eq!(v(Body::Text { id: 1 }), json!("  hello\n\tworld  "));
    assert_eq!(j(Body::Parts { id: 2 }), json!([{"kind": "text", "text": "with parts"}]));
    // a route's parts sb-core holds: a literal, parsed
    assert_eq!(j(Body::Lit { text: "[1]".into() }), json!([1]));
    // sb-core's parts_json: unparsable parts read null
    assert_eq!(j(Body::Parts { id: 3 }), json!(null));
}

#[test]
fn a_missing_message_reads_as_one_text_never_empty() {
    let m = Missing(9).text();
    assert!(!m.is_empty() && m.contains("m_9"));
    assert_eq!(value(&Body::Text { id: 9 }, FieldKind::Body, &mut words), json!(m));
    assert_eq!(value(&Body::Parts { id: 9 }, FieldKind::BodyJson, &mut words), json!([{"kind": "text", "text": m}]));
}

#[test]
fn a_feed_record_fills_then_decodes() {
    let mut v = json!({"sb": "msg-in", "from": "a", "id": 1, "body": {"body": "text", "id": 1}});
    fill(&Feed::TABLE, &mut v, &mut words).unwrap();
    let Feed::MsgIn(r) = Feed::decode(v).unwrap() else { panic!("not msg-in") };
    assert_eq!(r.body, "  hello\n\tworld  ");
    let mut p = json!({"sb": "msg-parts", "id": 2, "parts": {"body": "parts", "id": 2}});
    fill(&Feed::TABLE, &mut p, &mut words).unwrap();
    assert_eq!(p["parts"], json!([{"kind": "text", "text": "with parts"}]));
    // a record without a body field is left as it is
    let mut i = json!({"sb": "info", "text": "hi"});
    fill(&Feed::TABLE, &mut i, &mut words).unwrap();
    assert_eq!(i, json!({"sb": "info", "text": "hi"}));
    // a body field that isn't a Body: sb-core's bug, an error
    let mut bad = json!({"sb": "msg-in", "from": "a", "body": "raw"});
    assert!(fill(&Feed::TABLE, &mut bad, &mut words).is_err());
}

#[test]
fn the_words_of_a_message_sent_line() {
    let ev = json!({"type": "message_sent", "msg": {"id": 4, "text": "t", "parts": "[]"}});
    assert_eq!(Words::of_journal(&ev), Some((4, Words { text: "t".into(), parts: "[]".into() })));
    assert_eq!(Words::of_journal(&json!({"type": "message_state", "id": 4})), None);
}
