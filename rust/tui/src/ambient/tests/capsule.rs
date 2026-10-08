//! The capsule's talk, voice and state tests (moved out of
//! ambient/tests.rs, architect m_13367: a pure move, no test changed).

use super::*;

#[test]
fn talk_start_heard_talk_end_is_one_input_and_main_answers_aloud() {
    let mut t = T::new();
    t.ready();
    let png = t.dir.join("fn-down.png");
    std::fs::write(&png, b"shot").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Chrome".into(), title: "the build".into() });
    t.cmd(Cmd::TalkStart);
    assert!(phase_is(&t.take(), "listening"));
    assert_eq!(t.fakes.mic.lock().unwrap().opened, 1);
    // the mic's blocks go to the listener; its words come back
    let blocks = t.fakes.mic.lock().unwrap().blocks.clone().unwrap();
    blocks.send(crate::voicemode::MicBlock { pcm: vec![5000; 320], at: Instant::now() }).unwrap();
    let heard = t.fakes.listen.lock().unwrap()[0].heard.clone();
    heard.send(Heard::Text(" is the build".into())).unwrap();
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["text"] == "is the build" && v["final"] == false));
    heard.send(Heard::Text(" green".into())).unwrap();
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["text"] == "is the build green"));
    assert!(t.take().iter().any(|v| v["ev"] == "level" && v["who"] == "you"));
    // fn up: the mic closes, the listener flushes
    t.cmd(Cmd::TalkEnd);
    assert!(t.fakes.mic.lock().unwrap().blocks.as_ref().is_some());
    let audio: Vec<ListenMsg> = t.fakes.listen.lock().unwrap()[0].audio.try_iter().collect();
    assert_eq!(audio, vec![ListenMsg::Audio(vec![5000; 320]), ListenMsg::Flush]);
    heard.send(Heard::Text("?".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    let req = t.hub.next();
    let text = req["text"].as_str().unwrap();
    assert!(text.starts_with("is the build green?\n\n<image name=\"[Screen]\" path=\"Chrome · the build\""), "{text}");
    let out = t.take();
    assert!(out.contains(&json!({"ev": "heard", "text": "is the build green?", "final": true})));
    assert!(out.contains(&json!({"ev": "sent", "text": "is the build green?", "voice": true, "shot": true})));
    // one input only
    t.hub.w.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let mut more = String::new();
    assert!(t.hub.r.read_line(&mut more).is_err() || more.is_empty(), "a second input: {more}");
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    // main's lines: working (with its tool line), main's text, then aloud
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "tool_intent #1 : runs the tests");
    t.hub.line("main", "  obs: assistant: <think>check ci</think>Yes, the build is green.\\nAll 412 tests pass.");
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "speaking"));
    let out = t.take();
    assert_eq!(phases(&out), vec!["working", "working", "speaking"]);
    assert!(out.iter().any(|v| v["ev"] == "phase" && v["line"] == "runs the tests"));
    assert!(out.contains(&json!({"ev": "main", "text": "Yes, the build is green.\nAll 412 tests pass.", "turn": 1})));
    // the voice: sentence by sentence; the speaker's clock lights the words
    let first = {
        let s = t.fakes.synth.lock().unwrap();
        assert_eq!(s.len(), 1);
        assert!(s[0].text.contains("build is green"), "{}", s[0].text);
        s[0].events.clone()
    };
    first.send(Synth::Audio(vec![0.1; 24_000])).unwrap();
    first.send(Synth::Done).unwrap();
    let synth = t.fakes.synth.clone();
    t.until(move |_| synth.lock().unwrap().len() == 2);
    t.fakes.speaker.lock().unwrap().clock = Some((1, Duration::from_millis(500)));
    t.until(|o| o.iter().any(|v| v["ev"] == "word"));
    let w: Vec<u64> = t.take().iter().filter(|v| v["ev"] == "word").map(|v| v["i"].as_u64().unwrap()).collect();
    assert!(w[0] <= 4, "{w:?}");
    let second = t.fakes.synth.lock().unwrap()[1].events.clone();
    second.send(Synth::Audio(vec![0.1; 24_000])).unwrap();
    second.send(Synth::Done).unwrap();
    {
        let mut s = t.fakes.speaker.lock().unwrap();
        s.clock = None;
        s.done.extend([1, 2]);
    }
    t.until(|o| phase_is(o, "done"));
    let out = t.take();
    // the last word of the message (`pass.`, word 8) is lit
    assert_eq!(out.iter().rfind(|v| v["ev"] == "word").unwrap(), &json!({"ev": "word", "turn": 1, "i": 8}));
    assert!(out.contains(&json!({"ev": "level", "who": "main", "v": 0.0})));
}

fn t_synths(t: &T) -> usize {
    t.fakes.synth.lock().unwrap().len()
}

/// Law (ambient-lead m_6513): fn space's typed text has two sends: ⌘⏎
/// asks main ({cmd: send}, via capsule) and tab starts an agent ({cmd:
/// start}): main gets 'start an agent for: <text>' with via
/// capsule-start (the hub's hint: start an agent; one short line), and
/// only main starts it. Empty words send nothing.
#[test]
fn tab_asks_main_to_start_an_agent() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::Start { text: "  fix the login loop on Safari ".into() });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "start an agent for: fix the login loop on Safari", "via": "capsule-start"}));
    assert!(has(&t.take(), "sent"));
    t.cmd(Cmd::Start { text: "   ".into() });
    t.cmd(Cmd::Send { text: "what's running?".into() });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "what's running?", "via": "capsule"}));
    // its parse from the app's line
    assert!(matches!(Cmd::parse(r#"{"cmd":"start","text":"x"}"#), Ok(Cmd::Start { .. })));
}

#[test]
fn voice_out_only_after_a_voice_input() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::Send { text: "typed".into() });
    t.hub.next();
    t.main_turn("Done.");
    t.until(|o| phase_is(o, "done"));
    assert!(has(&t.take(), "main"));
    assert_eq!(t_synths(&t), 0, "a typed message is answered in text");
    // a voice message: aloud
    talk(&mut t, "and now?");
    t.main_turn("Now too.");
    t.until(|o| phase_is(o, "speaking"));
    assert_eq!(t_synths(&t), 1);
    // a typed message after it: text again
    t.cmd(Cmd::Hush);
    t.cmd(Cmd::Send { text: "and typed?".into() });
    t.hub.next();
    t.main_turn("Typed, so text.");
    t.until(|o| o.iter().filter(|v| v["ev"] == "main").count() == 2);
    t.until(|o| phase_is(o, "done"));
    assert_eq!(t_synths(&t), 1);
}

#[test]
fn quiet_is_text_only_and_stops_the_voice() {
    let mut t = T::new();
    t.ready();
    // on a call: a voice message is answered in text
    t.cmd(Cmd::Quiet { on: true });
    talk(&mut t, "is it green?");
    t.main_turn("Green.");
    t.until(|o| phase_is(o, "done"));
    assert!(has(&t.take(), "main"));
    assert_eq!(t_synths(&t), 0, "quiet: no voice");
    // the call ends: aloud again; a call starting mid-voice stops it
    t.cmd(Cmd::Quiet { on: false });
    talk(&mut t, "and now?");
    t.main_turn("Now aloud.");
    t.until(|o| phase_is(o, "speaking"));
    assert_eq!(t_synths(&t), 1);
    t.cmd(Cmd::Quiet { on: true });
    assert!(phase_is(&t.take(), "done"));
    assert_eq!(Cmd::parse(r#"{"cmd":"quiet","on":true,"why":"call"}"#), Ok(Cmd::Quiet { on: true }));
}

#[test]
fn only_mains_words_after_its_last_tool_call_are_its_answer() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::Send { text: "draft the plan".into() });
    t.hub.next();
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: assistant: Spawn first, then start the page.");
    t.hub.line("main", "tool_intent #1 : starts the page");
    t.hub.line("main", "  obs: assistant: The plan is on your screen.");
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    let mains: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "main").collect();
    assert_eq!(mains, vec![json!({"ev": "main", "text": "The plan is on your screen.", "turn": 1})], "planning words never reach the capsule");
    // words then a tool call and nothing after: no main event
    t.cmd(Cmd::Send { text: "and the rest".into() });
    t.hub.next();
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: assistant: Let me check.");
    t.hub.line("main", "tool #2 bash : ls");
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    assert!(!has(&t.take(), "main"));
}

#[test]
fn stop_watching_goes_to_the_hub_and_timers_ride_the_state() {
    let mut t = T::new();
    t.cmd(Cmd::parse(r#"{"cmd":"every_stop","id":3}"#).unwrap());
    // scheduled/stop (the fake end gives a request back as its HubCmd)
    let stop = t.hub.next();
    assert_eq!((stop["cmd"].as_str(), stop["id"].as_u64()), (Some("scheduled_stop"), Some(3)), "{stop}");
    assert!(Cmd::parse(r#"{"cmd":"every_stop"}"#).is_err());
    let timers = json!([{"id": 3, "what": "the launch", "every": "15m"}]);
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [], "pages": [{"id": "w", "opened_version": 2}], "timers": timers}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["timers"], timers);
    assert_eq!(st["pages"][0]["opened_version"], 2);
}

#[test]
fn a_turn_on_input_from_a_page_never_shows_in_the_capsule() {
    let mut t = T::new();
    t.ready();
    // the hub's notes message to main, with its page marker
    t.hub.line("main", "sb you : you sent 1 note on your page \"plan\" (plan v1): 1. start an agent on b2\\n\\n[from the page: answer on the page; one short line at most]");
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: assistant: I started fixer on b2. It hasn't confirmed yet.");
    t.hub.line("main", "  obs: turn_done: completed");
    t.sync();
    let out = t.take();
    assert!(!has(&out, "main") && phases(&out).is_empty(), "{out:?}");
    // his own message after it: shown as usual
    t.hub.line("main", "sb you : is it done?");
    t.main_turn("Yes.");
    t.until(|o| has(o, "main"));
}

#[test]
fn voice_answers_off_is_text_only_until_on_again() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::Voice { on: false });
    talk(&mut t, "is it green?");
    t.main_turn("Green.");
    t.until(|o| phase_is(o, "done"));
    assert_eq!(t_synths(&t), 0, "voice answers off: text only");
    t.cmd(Cmd::Voice { on: true });
    talk(&mut t, "and now?");
    t.main_turn("Now aloud.");
    t.until(|o| phase_is(o, "speaking"));
    assert_eq!(t_synths(&t), 1);
    // turned off mid-voice: it stops, the text stays
    t.cmd(Cmd::Voice { on: false });
    assert!(phase_is(&t.take(), "done"));
    assert_eq!(Cmd::parse(r#"{"cmd":"voice","on":false}"#), Ok(Cmd::Voice { on: false }));
}

// the window's shown is bise-proto's AppCmd (qa-flows m_9714): the
// fixture line parses to the projects it names; a bad one is an error
#[test]
fn shown_is_the_proto_app_cmd() {
    assert_eq!(
        Cmd::parse(r#"{"cmd":"shown","projects":["bise-home-00000000","acme-1a2b3c4d"]}"#),
        Ok(Cmd::Shown { projects: vec!["bise-home-00000000".into(), "acme-1a2b3c4d".into()] })
    );
    assert_eq!(Cmd::parse(r#"{"cmd":"shown","projects":[]}"#), Ok(Cmd::Shown { projects: vec![] }));
    assert!(Cmd::parse(r#"{"cmd":"shown"}"#).is_err());
}

#[test]
fn the_state_carries_pages_url_and_urgent_cards() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "pages_url": "http://127.0.0.1:47123",
        "cards": [{"id": 1, "kind": "confirm", "agent": "a", "text": "run rm?"}, {"id": 2, "kind": "question", "agent": "a", "text": "which?"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["pages_url"], "http://127.0.0.1:47123");
    assert_eq!((st["cards"][0]["urgent"].as_bool(), st["cards"][1]["urgent"].as_bool()), (Some(true), Some(false)));
}

/// A batch of drafts' card keeps the hub's fields for the capsule's
/// words (amb-web m_6061); other cards have none.
#[test]
fn a_batch_card_carries_its_fields() {
    let mut t = T::new();
    let batch = json!({"count": 3, "title": "inbox", "what": "replies", "names": ["legal", "Lucas", "Marc"]});
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [],
        "cards": [{"id": 4, "kind": "question", "agent": "main", "text": "3 drafts wait for you · inbox
replies to legal, Lucas and Marc
1. review
2. send all 3",
            "page": {"id": "inbox", "block": "replies", "item": "r1", "drafts": true, "url": "http://x/p/inbox#r1"}, "batch": batch},
            {"id": 5, "kind": "question", "agent": "a", "text": "which?"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["cards"][0]["batch"], batch);
    assert_eq!(st["cards"][0]["options"][1]["label"], "send all 3");
    assert!(st["cards"][1].get("batch").is_none());
}

#[test]
fn hush_and_cut_in_stop_the_speaker() {
    let mut t = T::new();
    t.ready();
    talk(&mut t, "say something");
    t.main_turn("Something long enough to say.");
    t.until(|o| phase_is(o, "speaking"));
    let cancel = t.fakes.synth.lock().unwrap()[0].cancel.clone();
    t.cmd(Cmd::Hush);
    assert_eq!(t.fakes.speaker.lock().unwrap().stops, 1);
    assert!(cancel.load(Ordering::SeqCst), "the voice being made is dropped too");
    assert!(phase_is(&t.take(), "done"));
    // hush when nothing speaks: nothing
    t.cmd(Cmd::Hush);
    assert!(t.take().is_empty());

    talk(&mut t, "again");
    t.main_turn("Again, something to say.");
    t.until(|o| phase_is(o, "speaking"));
    t.cmd(Cmd::CutIn);
    assert_eq!(t.fakes.speaker.lock().unwrap().stops, 2);
    assert!(phase_is(&t.take(), "idle"));
    t.cmd(Cmd::TalkStart);
    assert!(phase_is(&t.take(), "listening"));

    // fn down while main speaks, without cut_in first: the voice stops too
    t.cmd(Cmd::TalkCancel);
    talk(&mut t, "once more");
    t.main_turn("Once more, said.");
    t.until(|o| phase_is(o, "speaking"));
    t.cmd(Cmd::TalkStart);
    assert_eq!(t.fakes.speaker.lock().unwrap().stops, 3);
}

#[test]
fn talk_cancel_sends_nothing_and_drops_the_shot() {
    let mut t = T::new();
    t.ready();
    let png = t.dir.join("c.png");
    std::fs::write(&png, b"cancelled").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Mail".into(), title: "x".into() });
    let stored: Vec<_> = std::fs::read_dir(&t.dir).unwrap().collect();
    assert_eq!(stored.len(), 2);
    t.cmd(Cmd::TalkStart);
    let heard = t.fakes.listen.lock().unwrap()[0].heard.clone();
    heard.send(Heard::Text(" never mind".into())).unwrap();
    t.cmd(Cmd::TalkCancel);
    assert!(phase_is(&t.take(), "idle"));
    assert_eq!(std::fs::read_dir(&t.dir).unwrap().count(), 0, "the shot is gone");
    t.cmd(Cmd::Send { text: "typed after".into() });
    assert_eq!(t.hub.next()["text"], "typed after");
}

#[test]
fn no_voice_model_keeps_the_text() {
    let mut t = T::new();
    t.ready();
    t.say_ok.store(false, Ordering::SeqCst);
    talk(&mut t, "hello");
    t.main_turn("Hi.");
    t.until(|o| phase_is(o, "done"));
    let out = t.take();
    assert!(has(&out, "error") && has(&out, "main"));
    assert_eq!(t_synths(&t), 0);
}
