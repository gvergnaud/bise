//! Dictation (bar I, core/dictate.rs): the composer's mic on the fake
//! mic and listener. Laws: the whole text in each event (the last one is
//! the composer's text), exactly one final, never a hub op or a main
//! phase, cancel, one mic user (fn wins), the quiet stop (never a time
//! limit), a long take's segments.

use super::*;
use super::super::core::QUIET_STOP;
use crate::voice::TRANSCRIBE_TIMEOUT;

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
    // a talk still listening (fn up missed): his press in the window ends
    // it as at fn up (its words go to main) and the take gets the mic,
    // never 'the mic is busy.' (amb-tools' repro, m_22196)
    t.cmd(Cmd::TalkStart);
    let talk = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    t.take();
    let heard = start(&mut t, "d0");
    let out = t.take();
    assert!(!dictations(&out).iter().any(|v| v["final"] == true), "{out:#?}");
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 2);
    talk.send(Heard::Text(" is it green".into())).unwrap();
    talk.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    assert_eq!(t.hub.next()["text"], "is it green");
    // never silent: one notice in the window says why (architect m_22291)
    let notices: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "notice").collect();
    assert_eq!(notices.len(), 1, "{notices:#?}");
    assert_eq!(notices[0]["text"], "fn talk sent to main: the mic went to dictation");
    heard.send(Heard::Text(" the take".into())).unwrap();
    t.cmd(Cmd::DictateStop { id: "d0".into() });
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()).last().unwrap(), &json!({"ev": "dictation", "id": "d0", "text": "the take", "final": true}));
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

/// Law (architect m_10537, m_64474): no silent hot mic: a dictation left
/// on (the fake mic's quiet blocks: nothing above the floor) stops by
/// itself after QUIET_STOP, its words kept, its final out and saying why.
#[test]
fn a_forgotten_dictation_stops_after_a_long_silence() {
    let mut t = T::new();
    t.ready();
    let Some(long_ago) = Instant::now().checked_sub(QUIET_STOP + Duration::from_secs(1)) else { return };
    t.core.cmd(Cmd::DictateStart { id: "d1".into() }, long_ago);
    let heard = t.fakes.listen.lock().unwrap()[0].heard.clone();
    heard.send(Heard::Text(" left on".into())).unwrap();
    let listen = t.fakes.listen.clone();
    t.until(move |_| listen.lock().unwrap()[0].audio.try_iter().any(|m| m == ListenMsg::Flush));
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    let d = dictations(&t.take());
    assert_eq!(d.last().unwrap(), &json!({"ev": "dictation", "id": "d1", "text": "left on", "final": true, "stopped": "silence"}));
    assert_eq!(d.iter().filter(|v| v["final"] == true).count(), 1);
}

/// No time limit (dictate-nocap): a take he talks in (loud blocks) runs
/// far past the old 2 minutes, the mic stays his until his stop.
#[test]
fn a_take_he_talks_in_has_no_time_limit() {
    let mut t = T::new();
    t.ready();
    let Some(long_ago) = Instant::now().checked_sub(QUIET_STOP * 4) else { return };
    t.core.cmd(Cmd::DictateStart { id: "d1".into() }, long_ago);
    // his voice, now: the quiet clock is fresh
    t.fakes.mic.lock().unwrap().blocks.as_ref().unwrap().send(crate::voicemode::MicBlock { pcm: vec![3000; 1600], at: Instant::now() }).unwrap();
    for _ in 0..5 {
        t.core.tick(Instant::now());
        t.out.extend(t.core.take_out());
    }
    let listen = t.fakes.listen.clone();
    assert!(!listen.lock().unwrap()[0].audio.try_iter().any(|m| m == ListenMsg::Flush), "his take was cut");
    assert!(dictations(&t.take()).iter().all(|v| v["final"] != true));
}

/// His hot cut (m_22120): a batch listener sends every word at the flush,
/// after an HTTP request. A flush slower than the old 1.5 s (FINAL_WAIT)
/// ended the take with '' and the listener threw the words away. Now the
/// final waits for the words, however long the request takes.
#[test]
fn a_slow_transcription_keeps_its_words() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    // stopped 10 s ago: far past the old 1.5 s, the request still running
    let Some(ago) = Instant::now().checked_sub(Duration::from_secs(10)) else { return };
    t.core.cmd(Cmd::DictateStop { id: "d1".into() }, ago);
    for _ in 0..20 {
        t.until(|_| true);
    }
    assert!(dictations(&t.take()).is_empty(), "a final before the words came");
    heard.send(Heard::Text(" ship the login fix".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "ship the login fix", "final": true})]);
}

/// A take still transcribing no longer holds the mic: the next take
/// starts at once (never 'the mic is busy.'), and both finals come, each
/// with its own words.
#[test]
fn a_take_started_while_the_last_one_transcribes_gets_the_mic_and_both_keep_their_words() {
    let mut t = T::new();
    t.ready();
    let one = start(&mut t, "d1");
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    let two = start(&mut t, "d2");
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 2);
    assert!(!dictations(&t.take()).iter().any(|v| v["error"].is_string()), "the second take was refused");
    two.send(Heard::Text(" and the tests".into())).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["id"] == "d2" && v["text"] == "and the tests"));
    one.send(Heard::Text(" fix the login".into())).unwrap();
    one.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["id"] == "d1" && v["final"] == true));
    // d1's stop again, or its late words, change nothing
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    t.cmd(Cmd::DictateStop { id: "d2".into() });
    two.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["id"] == "d2" && v["final"] == true));
    let d = dictations(&t.take());
    let finals: Vec<&Value> = d.iter().filter(|v| v["final"] == true).collect();
    assert_eq!(
        finals,
        [
            &json!({"ev": "dictation", "id": "d1", "text": "fix the login", "final": true}),
            &json!({"ev": "dictation", "id": "d2", "text": "and the tests", "final": true}),
        ]
    );
    // fn during a flush: the flushing take goes on, fn gets the mic
    let three = start(&mut t, "d3");
    t.cmd(Cmd::DictateStop { id: "d3".into() });
    t.cmd(Cmd::TalkStart);
    assert!(!dictations(&t.take()).iter().any(|v| v["id"] == "d3"), "fn ended a take that no longer had the mic");
    three.send(Heard::Text(" later".into())).unwrap();
    three.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["id"] == "d3" && v["final"] == true && v["text"] == "later"));
}

/// A request that never answers: the take ends after TRANSCRIBE_TIMEOUT
/// with an error (said in his composer), never a silent ''.
#[test]
fn a_transcription_that_never_answers_ends_with_an_error() {
    let mut t = T::new();
    t.ready();
    start(&mut t, "d1");
    let Some(ago) = Instant::now().checked_sub(TRANSCRIBE_TIMEOUT + Duration::from_secs(1)) else { return };
    t.core.cmd(Cmd::DictateStop { id: "d1".into() }, ago);
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "", "final": true, "error": "the transcription timed out."})]);
    // a cancel of a take still transcribing: one cancelled final
    start(&mut t, "d2");
    t.cmd(Cmd::DictateStop { id: "d2".into() });
    t.cmd(Cmd::DictateCancel { id: "d2".into() });
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d2", "text": "", "final": true, "cancelled": true})]);
}

#[test]
fn dictate_cmds_need_an_id() {
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_start","id":"w1-3"}"#), Ok(Cmd::DictateStart { id: "w1-3".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_stop","id":"w1-3"}"#), Ok(Cmd::DictateStop { id: "w1-3".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_cancel","id":"w1-3"}"#), Ok(Cmd::DictateCancel { id: "w1-3".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"dictate_retry","id":"w1-3"}"#), Ok(Cmd::DictateRetry { id: "w1-3".into() }));
    assert!(Cmd::parse(r#"{"cmd":"dictate_retry"}"#).is_err());
    assert!(Cmd::parse(r#"{"cmd":"dictate_start","id":" "}"#).is_err());
    assert!(Cmd::parse(r#"{"cmd":"dictate_start"}"#).is_err());
}

/// His mic gives `n` samples to the take holding it.
fn sound(t: &mut T, n: usize) {
    let s = t.fakes.mic.lock().unwrap();
    s.blocks.as_ref().unwrap().send(crate::voicemode::MicBlock { pcm: vec![7; n], at: Instant::now() }).unwrap();
}

/// What a listener got: its samples and whether it was flushed.
fn fed(t: &T, i: usize) -> (usize, bool) {
    let msgs: Vec<ListenMsg> = t.fakes.listen.lock().unwrap()[i].audio.try_iter().collect();
    let n = msgs.iter().map(|m| if let ListenMsg::Audio(p) = m { p.len() } else { 0 }).sum();
    (n, msgs.last() == Some(&ListenMsg::Flush))
}

/// Fix 2 (architect m_22168): a take whose final has no words keeps its
/// sound; dictate_retry sends it to a fresh listener and gives one final,
/// no partials; the window's ack after its words frees it ('nothing to try again.').
#[test]
fn a_failed_take_keeps_its_sound_and_a_retry_gives_one_final() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    sound(&mut t, 4000);
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    heard.send(Heard::Failed("the listener: 503".into())).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "", "final": true, "error": "the listener: 503", "kept": true})]);
    let n = t.fakes.listen.lock().unwrap().len();
    t.cmd(Cmd::DictateRetry { id: "d1".into() });
    assert_eq!(t.fakes.listen.lock().unwrap().len(), n + 1, "a retry has a fresh listener");
    assert_eq!(fed(&t, n), (4000, true), "the whole sound, then the flush");
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 1, "a retry opens no mic");
    let again = t.fakes.listen.lock().unwrap()[n].heard.clone();
    again.send(Heard::Text(" found".into())).unwrap();
    again.send(Heard::Text(" it".into())).unwrap();
    again.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "found it", "final": true})]);
    // the window has its words: its ack (dictate_cancel) frees the sound, silently
    t.cmd(Cmd::DictateCancel { id: "d1".into() });
    assert_eq!(dictations(&t.take()), Vec::<Value>::new());
    t.cmd(Cmd::DictateRetry { id: "d1".into() });
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "", "final": true, "error": "nothing to try again."})]);
    hub_quiet(&mut t);
}

/// His voice model gone, the listener's fallback heard the take: the
/// final has the words and the listener's one line as `note`, nothing
/// kept (the desktop says the line, the words land).
#[test]
fn a_take_heard_by_the_fallback_model_says_so_in_its_final() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    sound(&mut t, 4000);
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    heard.send(Heard::Text(" bonjour".into())).unwrap();
    heard.send(Heard::Note("Mistral no longer has voxtral-transcribe-3: voxtral-mini-transcribe-3 heard this one. /voice picks your model.".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(
        dictations(&t.take()),
        vec![json!({"ev": "dictation", "id": "d1", "text": "bonjour", "final": true, "note": "Mistral no longer has voxtral-transcribe-3: voxtral-mini-transcribe-3 heard this one. /voice picks your model."})]
    );
    hub_quiet(&mut t);
}

/// An empty final keeps the sound too, a retry that fails keeps it again,
/// and his cancel drops it. An id never kept: 'nothing to try again.'.
#[test]
fn an_empty_take_is_kept_until_he_cancels_it() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::DictateRetry { id: "nope".into() });
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "nope", "text": "", "final": true, "error": "nothing to try again."})]);
    let heard = start(&mut t, "d2");
    sound(&mut t, 1600);
    t.cmd(Cmd::DictateStop { id: "d2".into() });
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d2", "text": "", "final": true, "kept": true})]);
    let n = t.fakes.listen.lock().unwrap().len();
    t.cmd(Cmd::DictateRetry { id: "d2".into() });
    t.fakes.listen.lock().unwrap()[n].heard.send(Heard::Failed("offline".into())).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d2", "text": "", "final": true, "error": "offline", "kept": true})]);
    t.cmd(Cmd::DictateCancel { id: "d2".into() });
    t.take();
    t.cmd(Cmd::DictateRetry { id: "d2".into() });
    assert_eq!(dictations(&t.take()).last().unwrap()["error"], "nothing to try again.");
}

/// dictation-keep (architect m_49881): a stopped take's sound is on disk
/// from its stop, so a core started after a quit transcribes it again
/// (the sound a core before it kept, read from the folder); cancel deletes it.
#[test]
fn a_stopped_take_is_on_disk_and_a_new_core_transcribes_it_again() {
    let mut t = T::new();
    t.ready();
    let dir = t.dir.join("dictation");
    let _heard = start(&mut t, "d1");
    sound(&mut t, 3200);
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    assert_eq!(std::fs::metadata(dir.join("d1.pcm")).map(|m| m.len()).ok(), Some(6400), "on disk at the stop (16-bit), still transcribed");
    // the app quit: this core is gone; the next one finds the file
    let mut t2 = T::new();
    t2.ready();
    let dir2 = t2.dir.join("dictation");
    std::fs::create_dir_all(&dir2).unwrap();
    std::fs::copy(dir.join("d1.pcm"), dir2.join("d1.pcm")).unwrap();
    let n = t2.fakes.listen.lock().unwrap().len();
    t2.cmd(Cmd::DictateRetry { id: "d1".into() });
    assert_eq!(fed(&t2, n), (3200, true), "the whole sound from the file, then the flush");
    let again = t2.fakes.listen.lock().unwrap()[n].heard.clone();
    again.send(Heard::Text(" back".into())).unwrap();
    again.send(Heard::Flushed).unwrap();
    t2.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t2.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "back", "final": true})]);
    assert!(dir2.join("d1.pcm").exists(), "kept until the window has the words");
    t2.cmd(Cmd::DictateCancel { id: "d1".into() });
    assert!(!dir2.join("d1.pcm").exists(), "the window's ack deletes it");
}

/// A reloaded window asks again for a take the core still transcribes:
/// nothing new starts, no final yet; its one final still comes.
#[test]
fn a_retry_while_the_take_is_transcribed_is_a_no_op() {
    let mut t = T::new();
    t.ready();
    let heard = start(&mut t, "d1");
    sound(&mut t, 800);
    t.cmd(Cmd::DictateStop { id: "d1".into() });
    t.take();
    let n = t.fakes.listen.lock().unwrap().len();
    t.cmd(Cmd::DictateRetry { id: "d1".into() });
    assert_eq!(t.fakes.listen.lock().unwrap().len(), n, "no second listener");
    assert_eq!(dictations(&t.take()), Vec::<Value>::new());
    heard.send(Heard::Text(" still here".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| dictations(o).iter().any(|v| v["final"] == true));
    assert_eq!(dictations(&t.take()), vec![json!({"ev": "dictation", "id": "d1", "text": "still here", "final": true})]);
}

/// amb-tools m_22412 (#40): a take ends a listening talk, its flush is
/// slow, and a talk_cancel comes meanwhile (the fn helper's exit, a fn
/// cancel). The talk was promised to main: its words still go, with its
/// one notice; a cancel only takes a talk still listening.
#[test]
fn a_cancel_during_the_flush_of_a_talk_a_take_ended_keeps_its_words() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::TalkStart);
    let talk = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    t.take();
    let _take = start(&mut t, "d0");
    t.take();
    t.cmd(Cmd::TalkCancel);
    let out = t.take();
    assert!(!out.iter().any(|v| v["ev"] == "phase" && v["phase"] == "idle"), "the cancel took the ending talk: {out:#?}");
    talk.send(Heard::Text(" ship the fix".into())).unwrap();
    talk.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    assert_eq!(t.hub.next()["text"], "ship the fix");
    let notices: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "notice").collect();
    assert_eq!(notices.len(), 1, "{notices:#?}");
    assert_eq!(notices[0]["text"], "fn talk sent to main: the mic went to dictation");
    // a talk still listening: the cancel takes it, nothing to main
    t.cmd(Cmd::DictateCancel { id: "d0".into() });
    t.cmd(Cmd::TalkStart);
    t.take();
    t.cmd(Cmd::TalkCancel);
    assert!(t.take().iter().any(|v| v["ev"] == "phase" && v["phase"] == "idle"));
    hub_quiet(&mut t);
}
