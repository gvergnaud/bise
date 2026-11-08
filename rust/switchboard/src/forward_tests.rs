use super::*;

fn lines(xs: &[&str]) -> impl Iterator<Item = String> {
    xs.iter().enumerate().map(|(i, l)| format!("{}\t{}", 1000 + i, l)).collect::<Vec<_>>().into_iter()
}

#[test]
fn msg_pos_finds_an_agent_message_by_its_msg_in_line() {
    let t = lines(&["sb msg-in : docs m_3 : hi", "sb msg-in : main m_7 : the brief", "assistant ok"]);
    assert_eq!(msg_pos(t, 7), Some(2));
}

#[test]
fn msg_pos_finds_his_message_at_its_you_line() {
    let t = lines(&["sb you : first", "sb you-id : m_4", "sb you : tiens au fait", "sb you-id : m_12", "assistant ok"]);
    assert_eq!(msg_pos(t, 12), Some(3));
}

#[test]
fn msg_pos_none_when_not_there() {
    let t = lines(&["sb msg-in : docs m_3 : hi", "sb you-id : m_4"]);
    assert_eq!(msg_pos(t, 30), None);
    // m_3 is not m_30: the id is matched whole
    assert_eq!(msg_pos(lines(&["sb msg-in : docs m_30 : hi"]), 3), None);
}

#[test]
fn keys_carry_the_id_and_the_originals_time() {
    let mut st = State::new("/w");
    st.put_msg_judged(crate::model::Msg { id: 5, from: "user".into(), to: "main".into(), created_ms: 0, ..Default::default() }, None, false);
    st.test_words(5, "hi", "[1]");
    let mut q = json!({"cmd": "send"});
    keys(&st, &mut q, Some(5));
    assert_eq!(q["fwd"], json!(5));
    assert_eq!(q["fwd_at"], json!("1970-01-01 00:00 UTC"));
    // its words from the journal (c6: sb-core keeps none)
    assert_eq!(q["orig"], json!({"text": "hi", "parts": "[1]"}));
    // not in the mirror: no time, the id still goes (sb-core's door decides)
    let mut q = json!({"cmd": "send"});
    keys(&st, &mut q, Some(9));
    assert_eq!((q["fwd"].clone(), q["fwd_at"].clone()), (json!(9), json!("")));
    assert_eq!(q["orig"], json!({"text": bise_proto::core::body::Missing(9).text(), "parts": ""}));
    // no forward: the line is what it was
    let mut q = json!({"cmd": "send"});
    keys(&st, &mut q, None);
    assert_eq!(q, json!({"cmd": "send"}));
}
