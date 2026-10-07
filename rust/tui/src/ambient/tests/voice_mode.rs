//! Voice mode with the agent in view (core/voice_mode.rs, architect
//! m_11164) on voicemode's fakes: the mic by blocks, the listener by
//! words, the synthesizer and the speaker recorded. Laws: his turn is one
//! typed send to the agent in view on its project (never main); its
//! answer is said; quiet says nothing; mute, type, cut; switching agent;
//! one mic owner; its thread followed only while the window doesn't.

use super::hubs::{typed, world, World};
use super::*;
use crate::voicemode::MicBlock;

fn modes(out: &[Value]) -> Vec<Value> {
    out.iter().filter(|v| v["ev"] == "voice_mode").cloned().collect()
}

/// bise's home and shop, shop welcomed with perf and main idle.
fn shop(t: &mut T) -> World {
    t.ready();
    let mut w = world(t);
    typed(t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(t, &["home", "shop"]);
    w.welcome(t, "shop");
    agents(t, &mut w, "idle");
    t.take();
    w
}

/// shop's agents event: perf with `status`, main idle.
fn agents(t: &mut T, w: &mut World, status: &str) {
    let a = |name: &str, main: bool, status: &str| json!({"name": name, "main": main, "status": status, "archived": false, "title": "", "purpose": "", "since_ms": 1, "waits": 0});
    w.end("shop").say(json!({"ev": "agents", "project": "shop", "agents": [a("main", true, "idle"), a("perf", false, status)]}));
    let want = status.to_string();
    w.until_ticking(t, move |o| o.iter().any(|v| v["ev"] == "agents" && v["agents"][1]["status"] == want.as_str()));
}

fn on(t: &mut T, w: &mut World, agent: &str) {
    typed(t, json!({"cmd": "voice_mode", "project": "shop", "agent": agent, "on": true}));
    w.until_ticking(t, |o| modes(o).iter().any(|v| v["on"] == true));
}

/// What the core wrote on shop's connection next (its hello skipped).
fn shop_next(w: &mut World) -> Value {
    w.end("shop").next()
}

/// He says `words`: loud blocks, the listener hears them, the tap sends
/// (the TUI's space), the listener flushes: the turn goes.
fn say(t: &mut T, w: &mut World, words: &str) {
    let tx = t.vf.mic.lock().unwrap().blocks.clone().expect("the mic is open");
    for _ in 0..3 {
        tx.send(MicBlock { pcm: vec![5000; 1600], at: Instant::now() }).unwrap();
        w.until_ticking(t, |_| true);
    }
    let heard = t.vf.listen.lock().unwrap().last().unwrap().heard.clone();
    heard.send(Heard::Text(format!(" {words}"))).unwrap();
    let want = words.to_string();
    w.until_ticking(t, move |o| modes(o).iter().any(|v| v["heard"].as_str().is_some_and(|h| h.ends_with(want.as_str()))));
    typed(t, json!({"cmd": "voice_send"}));
    heard.send(Heard::Flushed).unwrap();
    for _ in 0..5 {
        w.until_ticking(t, |_| true);
    }
}

/// shop's hub says `agent` wrote `text` now.
fn answer(t: &mut T, w: &mut World, agent: &str, pos: u64, text: &str) {
    let entry = json!({"pos": pos, "at_ms": now_ms(), "kind": "agent", "text": text});
    w.end("shop").say(json!({"ev": "entry", "project": "shop", "agent": agent, "entry": entry}));
    w.until_ticking(t, move |o| o.iter().any(|v| v["ev"] == "entry" && v["entry"]["pos"] == pos));
    for _ in 0..3 {
        w.until_ticking(t, |_| true);
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

/// Law: his words are ONE typed send to the agent in view on its own
/// project's hub, never main's input; its answer reaches the synthesizer
/// as speakable sentences; the core follows that thread itself (the
/// window doesn't) and lets it go when voice mode ends.
#[test]
fn his_turn_goes_to_the_agent_in_view_and_its_answer_is_said() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    on(&mut t, &mut w, "perf");
    assert_eq!(t.vf.mic.lock().unwrap().opened, 1, "voice mode's own mic");
    assert_eq!(shop_next(&mut w), json!({"cmd": "subscribe", "project": "shop", "agent": "perf"}), "the core follows perf's thread");
    say(&mut t, &mut w, "why is the cold bench slow");
    let sent = shop_next(&mut w);
    assert_eq!((sent["cmd"].as_str(), sent["agent"].as_str(), sent["text"].as_str()), (Some("send"), Some("perf"), Some("why is the cold bench slow")), "{sent}");
    answer(&mut t, &mut w, "perf", 40, "The cold bench reads the cache from disk. **Warm** runs skip it.");
    let said: Vec<String> = t.vf.synth.lock().unwrap().iter().map(|c| c.text.clone()).collect();
    assert!(!said.is_empty() && said.iter().all(|s| !s.contains("**")), "speakable sentences: {said:?}");
    assert!(said[0].starts_with("The cold bench"), "{said:?}");
    // the same entry again (changed or replayed): never said twice
    answer(&mut t, &mut w, "perf", 40, "The cold bench reads the cache from disk.");
    assert_eq!(t.vf.synth.lock().unwrap().len(), said.len());
    // an old message (a page's) is never said
    let old = json!({"pos": 3, "at_ms": 1, "kind": "agent", "text": "an old answer."});
    w.end("shop").say(json!({"ev": "entry", "project": "shop", "agent": "perf", "entry": old}));
    w.until_ticking(&mut t, |o| o.iter().any(|v| v["ev"] == "entry" && v["entry"]["pos"] == 3));
    assert_eq!(t.vf.synth.lock().unwrap().len(), said.len());
    // off: the mic and the speaker close, its thread is let go
    t.take();
    typed(&mut t, json!({"cmd": "voice_mode", "project": "shop", "agent": "perf", "on": false}));
    let last = modes(&t.take()).pop().expect("its last event");
    assert_eq!(last["on"], false);
    assert!(last["note"].as_str().is_some_and(|n| n.starts_with("voice mode ended")), "{last}");
    assert_eq!(shop_next(&mut w), json!({"cmd": "unsubscribe", "project": "shop", "agent": "perf"}));
    assert!(t.hub.r.get_ref().set_read_timeout(Some(Duration::from_millis(50))).is_ok());
    let mut l = String::new();
    while t.hub.r.read_line(&mut l).is_ok_and(|n| n > 0) {
        let v: Value = serde_json::from_str(l.trim()).unwrap();
        assert!(v["op"] != "input", "nothing reached main: {v}");
        l.clear();
    }
}

/// Law: the window follows the thread already: the core doesn't subscribe
/// (nor unsubscribe at the end); the window's unsubscribe while voice
/// mode is on stays in the core (the thread is still followed).
#[test]
fn the_core_follows_the_thread_only_when_the_window_doesnt() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    typed(&mut t, json!({"cmd": "subscribe", "project": "shop", "agent": "perf"}));
    assert_eq!(shop_next(&mut w)["cmd"], "subscribe");
    on(&mut t, &mut w, "perf");
    // the window lets go while voice mode follows it: nothing sent
    typed(&mut t, json!({"cmd": "unsubscribe", "project": "shop", "agent": "perf"}));
    // switching agent: perf's thread goes, main's comes (the core's own)
    typed(&mut t, json!({"cmd": "voice_mode", "project": "shop", "agent": "main", "on": true}));
    w.until_ticking(&mut t, |_| true);
    assert_eq!(shop_next(&mut w), json!({"cmd": "unsubscribe", "project": "shop", "agent": "perf"}));
    assert_eq!(shop_next(&mut w), json!({"cmd": "subscribe", "project": "shop", "agent": "main"}));
    // his next turn goes to main now (main is the one in view)
    say(&mut t, &mut w, "and the warm one");
    let sent = shop_next(&mut w);
    assert_eq!((sent["cmd"].as_str(), sent["agent"].as_str()), (Some("send"), Some("main")), "{sent}");
}

/// Law: quiet (a call, a meeting) or voice answers off: nothing said;
/// mute: his mic is off (muted), the agent's state goes on; type: the mic
/// waits (typing) and his typed send to that agent gets its answer said.
#[test]
fn quiet_says_nothing_mute_and_type_pause_the_mic() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    on(&mut t, &mut w, "perf");
    say(&mut t, &mut w, "run the bench");
    shop_next(&mut w);
    shop_next(&mut w);
    t.cmd(Cmd::Quiet { on: true });
    answer(&mut t, &mut w, "perf", 41, "Done, 4.1 seconds.");
    assert!(t.vf.synth.lock().unwrap().is_empty(), "quiet: nothing aloud");
    t.cmd(Cmd::Quiet { on: false });
    typed(&mut t, json!({"cmd": "voice_mute", "on": true}));
    assert!(modes(&t.take()).last().is_some_and(|v| v["muted"] == true), "muted");
    typed(&mut t, json!({"cmd": "voice_mute", "on": false}));
    typed(&mut t, json!({"cmd": "voice_type", "on": true}));
    assert!(modes(&t.take()).last().is_some_and(|v| v["state"] == "typing"), "typing: the mic waits");
    // his typed send to perf: its answer is said
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "perf", "text": "and warm?", "mode": "now"}));
    assert_eq!(shop_next(&mut w)["text"], "and warm?");
    answer(&mut t, &mut w, "perf", 42, "Warm is 0.9 seconds.");
    assert!(t.vf.synth.lock().unwrap().iter().any(|c| c.text.starts_with("Warm")), "the typed turn's answer is said");
}

/// Law: cut: its voice stops and its running turn is stopped (`stop`
/// on its project).
#[test]
fn a_cut_stops_its_voice_and_its_turn() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    on(&mut t, &mut w, "perf");
    shop_next(&mut w);
    agents(&mut t, &mut w, "working");
    let stops = t.vf.speaker.lock().unwrap().stops;
    typed(&mut t, json!({"cmd": "voice_cut"}));
    assert_eq!(shop_next(&mut w), json!({"cmd": "stop", "project": "shop", "agent": "perf"}));
    assert!(t.vf.speaker.lock().unwrap().stops >= stops);
}

/// Law (ambient-lead m_11168): one mic owner. Voice mode on: fn is refused
/// with where he is and how out, a dictation is refused; a dictation
/// running: voice mode is refused. Never two listeners.
#[test]
fn one_mic_owner() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    on(&mut t, &mut w, "perf");
    t.take();
    let listeners = t.fakes.listen.lock().unwrap().len();
    t.cmd(Cmd::TalkStart);
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("fn refused");
    assert_eq!((e["cmd"].as_str(), e["text"].as_str()), (Some("talk_start"), Some("voice mode is on with perf · ⌃R to leave")));
    t.cmd(Cmd::DictateStart { id: "d1".into() });
    let d = t.take().into_iter().find(|v| v["ev"] == "dictation").expect("dictation refused");
    assert_eq!(d["error"], "the mic is busy.");
    assert_eq!(t.fakes.listen.lock().unwrap().len(), listeners, "no second listener");
    typed(&mut t, json!({"cmd": "voice_mode", "project": "shop", "agent": "perf", "on": false}));
    t.take();
    t.cmd(Cmd::DictateStart { id: "d2".into() });
    typed(&mut t, json!({"cmd": "voice_mode", "project": "shop", "agent": "perf", "on": true}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error" && v["cmd"] == "voice_mode").expect("voice mode refused");
    assert_eq!(e["text"], "the mic is busy.");
}

/// Law: the core stops: voice mode ends with a note (never left listening).
#[test]
fn the_core_stopping_ends_voice_mode() {
    let mut t = T::new();
    let mut w = shop(&mut t);
    on(&mut t, &mut w, "perf");
    t.take();
    t.core.shutdown();
    let last = modes(&t.core.take_out()).pop().expect("its last event");
    assert_eq!(last["on"], false);
    assert!(last["note"].as_str().is_some_and(|n| n.ends_with("bise stopped")), "{last}");
}
