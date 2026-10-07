//! Dictation (bar I, core/dictate.rs): the composer's mic on the fake
//! mic and listener. Laws: the whole text in each event (the last one is
//! the composer's text), exactly one final, never a hub op or a main
//! phase, cancel, one mic user (fn wins), the time limit.

use super::*;
use super::super::core::DICTATE_MAX;

fn dictations(out: &[Value]) -> Vec<Value> {
    out.iter().filter(|v| v["ev"] == "dictation").cloned().collect()
}

/// The core wrote nothing to the hub (no input, no op).
fn hub_quiet(t: &mut T) {
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let mut more = String::new();
    loop {
        more.clear();
        match t.hub.r.read_line(&mut more) {
            Ok(n) if n > 0 => {
                let v: Value = serde_json::from_str(more.trim()).unwrap();
                assert!(v["op"] == "hello" || v["cmd"] == "hello", "the core wrote to the hub: {more}");
            }
            _ => break,
        }
    }
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_secs(3))).unwrap();
}

fn start(t: &mut T, id: &str) -> std::sync::mpsc::Sender<Heard> {
    let n = t.fakes.listen.lock().unwrap().len();
    t.cmd(Cmd::DictateStart { id: id.into() });
    let s = t.fakes.listen.lock().unwrap();
    assert_eq!(s.len(), n + 1, "no listener for {id}: {:#?}", t.out);
    s[n].heard.clone()
}

/// Law (architect m_10537): each partial carries the whole text so far,
/// the final replaces it, the composer's text is the last event's;
/// exactly one final; nothing to the hub, no capsule phase.
#[test]
fn dictation_partials_then_one_final_whole_text_never_to_main() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 1);
    heard.send(Heard::Text(" fix the".into())).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["text"] == "fix the"));
    heard.send(Heard::Text(" login".into())).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["text"] == "fix the login"));
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    let audio: Vec<ListenMsg> = t.fakes.listen.lock().unwrap()[0].audio.try_iter().collect();
    assert_eq!(audio, vec![ListenMsg::Flush]);
    heard.send(Heard::Text(" test".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    // late events after the final change nothing
    heard.send(Heard::Text(" again".into())).ok();
    for _ in 0..5 {
        t.until(|_| true);
    }
    let out = t.take();
    let d = dictations(&out);
    assert_eq!(
        d,
        vec![
            json!({"ev": "dictation", "id": "d1", "text": "fix the", "final": false}),
            json!({"ev": "dictation", "id": "d1", "text": "fix the login", "final": false}),
            json!({"ev": "dictation", "id": "d1", "text": "fix the login test", "final": true}),
        ]
    );
    assert!(phases(&out).is_empty(), "a dictation shows no capsule phase: {out:#?}");
    assert!(!has(&out, "heard") && !has(&out, "sent"), "{out:#?}");
    hub_quiet(&mut t);
}

#[test]
fn dictation_cancel_is_one_cancelled_final_and_the_mic_off() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    heard.send(Heard::Text(" never mind".into())).unwrap();
    t.until(|o| !dictations(o).is_empty());
    t.take();
    // another window's id stops nothing
    t.cmd(Cmd::DictateCancel { id: "other".into() });
    t.cmd(Cmd::DictateStop { id: "other".into() });
    assert!(dictations(&t.take()).is_empty());
    t.cmd(Cmd::DictateCancel { id: "d1".into() });
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "", "final": true, "cancelled": true})]);
    // the listener's session is over (its audio sender dropped)
    let gone = matches!(t.fakes.listen.lock().unwrap()[0].audio.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected));
    assert!(gone, "the dictation still holds the listener");
    // the mic is free again
    start(&mut t, "d2");
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 2);
    hub_quiet(&mut t);
}

#[test]
fn one_mic_user_and_fn_wins_over_a_dictation() {
    let mut t = T::new();
    t.ready();
    // a talk holds the mic: a dictation is refused
    t.cmd(Cmd::TalkStart);
    t.take();
    t.cmd(Cmd::DictateStart { id: "d1".into() });
    let out = t.take();
    assert!(out.contains(&json!({"ev": "dictation", "id": "d1", "text": "", "final": true, "error": "the mic is busy."})), "{out:#?}");
    t.cmd(Cmd::TalkCancel);
    t.take();
    // a dictation holds it: a second one is refused
    let heard = start(&mut t, "d1");
    t.cmd(Cmd::DictateStart { id: "d2".into() });
    assert!(t.take().contains(&json!({"ev": "dictation", "id": "d2", "text": "", "final": true, "error": "the mic is busy."})));
    heard.send(Heard::Text(" half a".into())).unwrap();
    t.until(|o| !dictations(o).is_empty());
    t.take();
    // fn: the dictation ends with its words, then the talk starts
    t.cmd(Cmd::TalkStart);
    let out = t.take();
    let i = out.iter().position(|v| v == &json!({"ev": "dictation", "id": "d1", "text": "half a", "final": true})).expect("the dictation's final");
    let j = out.iter().position(|v| v["ev"] == "phase" && v["phase"] == "listening").expect("the talk");
    assert!(i < j, "{out:#?}");
    // its stop afterwards says nothing more
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    assert!(dictations(&t.take()).is_empty());
}

/// Law (architect m_10537): no silent hot mic: a dictation left running
/// stops by itself after DICTATE_MAX, its final out.
#[test]
fn a_forgotten_dictation_stops_by_itself() {
    let mut t = T::new();
    t.ready();
    let Some(long_ago) = Instant::now().checked_sub(DICTATE_MAX + Duration::from_secs(1)) else { return };
    t.core.cmd(Cmd::DictateStart { id: "d1".into() }, long_ago);
    let heard = t.fakes.listen.lock().unwrap()[0].heard.clone();
    heard.send(Heard::Text(" left on".into())).unwrap();
    let listen = t.fakes.listen.clone();
    t.until(move |_| listen.lock().unwrap()[0].audio.try_iter().any(|m| m == ListenMsg::Flush));
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    let d = dictations(&t.take());
    assert_eq!(d.last().unwrap(), &json!({"ev": "dictation", "id": "d1", "text": "left on", "final": true}));
    assert_eq!(d.iter().filter(|v| v["final"] == true).count(), 1);
}

#[test]
fn dictate_cmds_need_an_id() {
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_start","id":"w1-3"}"#), Ok(Cmd::DictateStart { id: "w1-3".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_stop","id":"w1-3"}"#), Ok(Cmd::DictateStop { id: "w1-3".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_cancel","id":"w1-3"}"#), Ok(Cmd::DictateCancel { id: "w1-3".into() }));
    assert!(Cmd::parse(r#"{"cmd":"dictate_start","id":" "}"#).is_err());
    assert!(Cmd::parse(r#"{"cmd":"dictate_start"}"#).is_err());
}
