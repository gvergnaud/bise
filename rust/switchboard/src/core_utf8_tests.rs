//! issue 36: a line to sb-core longer than its 64 KB recv keeps every
//! multi-byte char (sb-core decodes each recv on its own: the hub writes
//! ASCII-only lines, core_proc.rs `line_of`). Red before the fix: a char
//! a recv boundary cut was U+FFFD in sb-core's message. Out of
//! core_tests.rs (its line limit).

use super::super::*;

#[test]
fn a_200_kb_message_keeps_every_char_across_sb_cores_recvs() {
    let mut t = T::new();
    t.spawn_task("a");
    // 9 bytes a round (2 + 3 + 4): the 64 KB boundaries fall inside chars
    let text: String = "é─😀".repeat(200_000 / 9);
    let (_, fx) = t.req(MAIN, AgentReq::Send { to: "a".into(), text: text.clone(), expect_reply: false, reply_to: None, queued: false, why: String::new(), switch: None, attach: Vec::new(), parts: String::new(), fwd: None });
    assert!(!fx.is_empty());
    let m = t.hub.st.msgs().values().find(|m| m.to == "a" && t.hub.st.text(m.id).len() > 100_000).map(|m| t.hub.st.text(m.id));
    let m = m.expect("the message to a");
    assert!(!m.contains('\u{fffd}'), "a char became U+FFFD");
    assert_eq!(m, text);
}
