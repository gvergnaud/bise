//! The ambient core against a fake hub (a socket pair per connection),
//! a fake mic, listener, synthesizer and speaker, and a fake image store
//! in a temp folder. Never a real mic, sound, network or app.

use super::core::{Cmd, Core, Ports};
use super::hub::{Hub, HubIn};
use crate::voicemode::fakes::{FakeListener, FakeMic, FakeSpeaker, FakeSynth, Fakes};
use crate::voicemode::{Heard, ListenMsg, SayJob, Synth};
use bise_proto::thread::{self, Ctx, Entry, Line, PageRef};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WS: &str = "/w/repo";

mod agents;
mod capsule;
mod computer;
mod connection;
mod dictate;
mod fake_hub;
mod fake_voice;
mod home_switch;
mod hubs;
mod setup;
mod voice_mode;

use fake_hub::HubEnd;

/// The hub's `user_kind` (switchboard::model), as main.rs hands it in.
fn user_kind(kind: &str) -> bool {
    matches!(kind, "question" | "drop" | "confirm" | "merge" | "feature_try" | "feature_merge" | "update")
}

struct T {
    core: Core,
    rx: Receiver<HubIn>,
    ends: Receiver<HubEnd>,
    hub: HubEnd,
    fakes: Fakes,
    /// voice mode's fakes (voicemode/fakes.rs): its mic, listener, speaker
    vf: crate::voicemode::fakes::Fakes,
    out: Vec<Value>,
    dir: PathBuf,
    say_ok: Arc<AtomicBool>,
}

fn temp_dir() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("amb-core-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A store like bend_images': the file and its .b64, named by content.
fn fake_store(dir: PathBuf) -> impl Fn(&Path) -> Result<bend_images::Stored, String> {
    move |p: &Path| {
        let b = std::fs::read(p).map_err(|e| e.to_string())?;
        let name: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let file = dir.join(format!("{name}.png"));
        let b64 = dir.join(format!("{name}.b64"));
        std::fs::write(&file, &b).unwrap();
        std::fs::write(&b64, bend_images::base64_encode(&b)).unwrap();
        Ok(bend_images::Stored { kind: bend_images::Kind::Png, width: 1, height: 1, file, b64 })
    }
}

impl T {
    fn new() -> T {
        T::make(None, false)
    }

    /// harness A's ports (mod.rs fake_ports): the fake voice's words.
    fn fake_voice(words: super::fake::Words) -> T {
        T::make(Some(words), false)
    }

    /// an off-screen run with no fake voice (mod.rs no_mic) over the
    /// fakes, whose mics count every open
    fn off_screen() -> T {
        T::make(None, true)
    }

    fn make(fake: Option<super::fake::Words>, off_screen: bool) -> T {
        let (etx, ends) = mpsc::channel::<HubEnd>();
        let connect: super::hub::Connect = Box::new(move || {
            let (core_end, hub_end) = UnixStream::pair()?;
            let r = BufReader::new(hub_end.try_clone()?);
            r.get_ref().set_read_timeout(Some(Duration::from_secs(3)))?;
            etx.send(HubEnd::new(hub_end, r)).map_err(std::io::Error::other)?;
            Ok(core_end)
        });
        let (tx, rx) = mpsc::channel();
        let hub = Hub::start(connect, tx, |h| h, Duration::from_millis(20));
        let fakes = Fakes::new();
        let vf = crate::voicemode::fakes::Fakes::new();
        let dir = temp_dir();
        let say_ok = Arc::new(AtomicBool::new(true));
        let (spk, ok) = (fakes.speaker.clone(), say_ok.clone());
        let ports = Ports {
            mic: Box::new(FakeMic(fakes.mic.clone())),
            listener: {
                let l = fakes.listen.clone();
                Box::new(move |_| Box::new(FakeListener(l.clone())))
            },
            synth: Box::new(FakeSynth(fakes.synth.clone())),
            open_speaker: Box::new(move || Ok(Box::new(FakeSpeaker(spk.clone())) as Box<dyn crate::voicemode::Speaker>)),
            listen_job: Box::new(|| Ok(crate::voicemode::fakes::listen_job())),
            say_job: Box::new(move || {
                if ok.load(Ordering::SeqCst) {
                    Ok(SayJob { api: crate::voicemode::fakes::endpoint("tts"), voice: "v".into(), speed: 1.0 })
                } else {
                    Err("no voice".into())
                }
            }),
            language: None,
            store_image: Box::new(fake_store(dir.clone())),
            user_kind,
            fake_words: None,
            voice_mode: {
                let f = crate::voicemode::fakes::Fakes { mic: vf.mic.clone(), listen: vf.listen.clone(), synth: vf.synth.clone(), speaker: vf.speaker.clone() };
                Box::new(move || Ok((f.ports(crate::voicemode::Route::Headphones), f.jobs(true), crate::voicemode::config::VoiceModeConfig::default())))
            },
            dictation_dir: dir.join("dictation"),
        };
        let ports = match fake {
            Some(w) => {
                let mut p = super::fake_ports(user_kind, w);
                p.store_image = Box::new(fake_store(dir.clone()));
                p
            }
            None => ports,
        };
        let ports = if off_screen { super::no_mic(ports) } else { ports };
        let core = Core::new(WS.into(), hub, ports);
        let hub = ends.recv_timeout(Duration::from_secs(3)).expect("the core never connected");
        let mut t = T { core, rx, ends, hub, fakes, vf, out: Vec::new(), dir, say_ok };
        t.until(|o| o.iter().any(|v| v["ev"] == "hub"));
        t
    }

    /// Hub events in, a tick, events out, until `done` holds on what
    /// came out since the last `take` (3 s at most).
    #[track_caller]
    fn until(&mut self, done: impl Fn(&[Value]) -> bool) {
        let t0 = Instant::now();
        loop {
            // the hub's lines until none comes for a moment: the kinds the
            // fake hub writes at once are read in one batch (one `state`)
            while let Ok(h) = self.rx.recv_timeout(Duration::from_millis(2)) {
                self.core.hub(h);
            }
            self.core.tick(Instant::now());
            self.out.extend(self.core.take_out());
            if done(&self.out) {
                return;
            }
            assert!(t0.elapsed() < Duration::from_secs(3), "timed out; out: {:#?}", self.out);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn take(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.out)
    }

    fn cmd(&mut self, c: Cmd) {
        self.core.cmd(c, Instant::now());
        self.out.extend(self.core.take_out());
    }

    /// main's first page answered (what it said before is a replay):
    /// main's entries are live.
    fn ready(&mut self) {
        self.replay_over();
        self.until(|o| has(o, "phase"));
        self.take();
    }

    /// `initialize` answered, main's thread subscribed, then its first
    /// page (what main said so far): main's entries are live after it.
    fn replay_over(&mut self) {
        self.hub.answer_init();
        let t0 = Instant::now();
        while !self.hub.main_subscribed() {
            assert!(t0.elapsed() < Duration::from_secs(3), "main's thread never subscribed; out: {:#?}", self.out);
            while let Ok(h) = self.rx.try_recv() {
                self.core.hub(h);
            }
            self.out.extend(self.core.take_out());
        }
        self.hub.say(json!({"ev": "ready"}));
    }

    /// Everything the hub said so far is handled (a state round trip).
    fn sync(&mut self) {
        self.hub.say(json!({"ev": "agents", "project": fake_hub::HOME, "agents": []}));
        self.until(|o| has(o, "state"));
        self.out.retain(|v| v["ev"] != "state");
    }

    fn main_turn(&mut self, text: &str) {
        self.hub.line("main", "  obs: turn_started");
        self.hub.line("main", &format!("  obs: assistant: {text}"));
        self.hub.line("main", "  obs: turn_done: completed");
    }
}

impl Drop for T {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn has(out: &[Value], ev: &str) -> bool {
    out.iter().any(|v| v["ev"] == ev)
}

fn phases(out: &[Value]) -> Vec<String> {
    out.iter().filter(|v| v["ev"] == "phase").map(|v| v["phase"].as_str().unwrap().to_string()).collect()
}

fn phase_is(out: &[Value], p: &str) -> bool {
    phases(out).last().is_some_and(|x| x == p)
}

#[test]
fn hub_state_becomes_state_with_agents_and_cards_with_options() {
    let mut t = T::new();
    assert_eq!(t.take()[0], json!({"ev": "hub", "up": true, "workspace": WS}));
    t.hub.say(json!({"ev": "state",
        "agents": [
            {"name": "main", "main": true, "status": "idle"},
            {"name": "cookies", "status": "working", "note": "the banner", "report": "", "turn_ms": 1200},
            {"name": "perf", "status": "done", "report": "signup 4.1 s → 0.9 s. the rest is cleanup."},
            {"name": "old", "status": "archived"},
        ],
        "cards": [
            {"id": 3, "kind": "question", "agent": "cookies", "text": "keep the banner on mobile?\n1. yes\n2. no"},
            {"id": 4, "kind": "done", "agent": "perf", "text": "perf is done"},
        ]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    // the capsule's fields (round 10's are in their own law); an
    // archived agent is a row too (archived: true)
    let keep = |r: &Value| json!({"name": r["name"], "status": r["status"], "note": r["note"], "report": r["report"],
        "working": r["working"], "turn_ms": r["turn_ms"], "archived": r["archived"]});
    assert_eq!(
        st["agents"].as_array().unwrap().iter().map(keep).collect::<Vec<_>>(),
        [
            json!({"name": "cookies", "status": "working", "note": "the banner", "report": "", "working": true, "turn_ms": 1200, "archived": false}),
            json!({"name": "perf", "status": "done", "note": "", "report": "signup 4.1 s → 0.9 s. the rest is cleanup.", "working": false, "turn_ms": null, "archived": false}),
            json!({"name": "old", "status": "done", "note": "", "report": "", "working": false, "turn_ms": null, "archived": true}),
        ]
    );
    // only the user's cards (a `done` is the agents' traffic); the body
    // without its options (ambient m_6476)
    assert_eq!(
        st["cards"],
        json!([{"id": 3, "kind": "question", "agent": "cookies", "text": "keep the banner on mobile?",
            "options": [{"n": 1, "label": "yes"}, {"n": 2, "label": "no"}], "urgent": false}])
    );
    // fn + 1 answers with the option's words, as the TUI's box does
    t.cmd(Cmd::Answer { card: 3, reply: "1".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 3, "reply": "yes"}));
    // words go as they are
    t.cmd(Cmd::Answer { card: 3, reply: "only on desktop".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 3, "reply": "only on desktop"}));
    // a closed card
    t.cmd(Cmd::Answer { card: 9, reply: "1".into() });
    assert!(has(&t.take(), "error"));
}

/// Law (pm's D fail 31): a page's card with one option (a step: "1.
/// done") shows that option, and fn + its digit sends the digit to the
/// hub's page path; it was refused as "opens in the terminal", so the
/// step never closed.
#[test]
fn a_page_cards_digit_goes_to_the_page_path() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 3, "kind": "question", "agent": "main", "text": "sign in to Gandi
1. done",
         "page": {"id": "buy-domain", "block": "steps", "item": "signin", "url": "http://127.0.0.1:1/p/buy-domain#signin"}}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["cards"][0]["options"], json!([{"n": 1, "label": "done"}]));
    t.cmd(Cmd::Answer { card: 3, reply: "1".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 3, "reply": "1"}));
}

/// Law (pm's C fail 41): fn + digit is never refused on a card that
/// shows numbered options: '1 label', '1) label', '1. label', '1 -
/// label' all map to the option's words, and options the split can't
/// read send the digit as is (the hub's question path resolves it).
#[test]
fn a_digit_on_a_numbered_card_is_never_refused() {
    let mut t = T::new();
    let forms = ["1 a PR per task
2 straight to main", "1) a PR per task
2) straight to main", "1. a PR per task
2. straight to main", "1 - a PR per task
2 - straight to main"];
    let cards: Vec<Value> = forms
        .iter()
        .enumerate()
        .map(|(i, f)| json!({"id": i + 1, "kind": "question", "agent": "main", "text": format!("how should agents ship code here?
{f}")}))
        .chain([json!({"id": 9, "kind": "question", "agent": "main", "text": "ship how?
a: a PR
b: straight to main"})])
        .collect();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": cards}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    for i in 0..forms.len() {
        assert_eq!(st["cards"][i]["options"][1]["label"], "straight to main", "{}", forms[i]);
        t.cmd(Cmd::Answer { card: i as u64 + 1, reply: "2".into() });
        assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": i + 1, "reply": "straight to main"}), "{}", forms[i]);
    }
    t.cmd(Cmd::Answer { card: 9, reply: "2".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 9, "reply": "2"}));
    assert!(!has(&t.take(), "error"));
}

/// Law (pm's C fail 43): an answer on a batch card that closed (replaced
/// by a newer one) still goes to the hub, which acts on the page's
/// current drafts or says what changed; a closed card never seen as a
/// batch is refused as before.
#[test]
fn an_answer_on_a_replaced_batch_card_reaches_the_hub() {
    let mut t = T::new();
    let batch = |id: u64, n: usize| json!({"id": id, "kind": "question", "agent": "main", "text": format!("{n} drafts wait for you · bugs
{n} replies to Benjamin
1. review
2. send all {n}"),
        "page": {"id": "bugs", "block": "r", "item": "r1", "drafts": true, "url": "http://x/p/bugs#r1"},
        "batch": {"count": n, "title": "bugs", "what": "replies", "names": ["Benjamin"]}});
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "cards": [batch(1, 6)]}));
    t.until(|o| has(o, "state"));
    t.take();
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "cards": [batch(2, 7)]}));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::Answer { card: 1, reply: "2".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 1, "reply": "2"}));
    assert!(!has(&t.take(), "error"));
    t.cmd(Cmd::Answer { card: 5, reply: "2".into() });
    assert!(has(&t.take(), "error"));
}

/// Law (ambient m_6476 via ambient-lead m_6486): a card's body does not
/// repeat its options: a numbered list ending the text, inline (main's
/// real shape) or on lines, that is the card's options leaves the text;
/// main's own questions carry the label '? main needs you'.
#[test]
fn a_cards_body_does_not_repeat_its_options() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "cards": [
        {"id": 1, "kind": "question", "agent": "main", "text": "Que fais-tu ? 1. regarde le diff 2. arrête-le 3. laisse-le finir"},
        {"id": 2, "kind": "question", "agent": "t1", "text": "ship how?
1) a PR per task
2) straight to main"},
        {"id": 3, "kind": "question", "agent": "main", "text": "keep the banner on mobile?"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    let c = &st["cards"];
    assert_eq!(c[0]["text"], "Que fais-tu ?");
    assert_eq!(c[0]["options"], json!([{"n": 1, "label": "regarde le diff"}, {"n": 2, "label": "arrête-le"}, {"n": 3, "label": "laisse-le finir"}]));
    assert_eq!(c[0]["label"], "? main needs you");
    assert_eq!((c[1]["text"].as_str(), c[1]["options"][1]["label"].as_str()), (Some("ship how?"), Some("straight to main")));
    assert!(c[1].get("label").is_none(), "an agent's card: no main label");
    assert_eq!(c[2]["text"], "keep the banner on mobile?");
    // fn + 2 still answers with the option's words
    t.cmd(Cmd::Answer { card: 1, reply: "2".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 1, "reply": "arrête-le"}));
}

/// drop-card (architect m_35666): main's archive suggestion (a `drop`
/// card) is his card like any other: on the glass, answered with its
/// digit.
#[test]
fn an_archive_card_is_on_the_glass_and_answered_by_its_digit() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 2, "kind": "drop", "agent": "proxy-fix", "text": "main suggests it. nothing of its work is lost.\n1. archive it\n2. keep it"},
        {"id": 3, "kind": "question", "agent": "main", "text": "ship as a PR?
1. yes
2. no"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    let ids: Vec<u64> = st["cards"].as_array().unwrap().iter().map(|c| c["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, vec![2, 3]);
    t.cmd(Cmd::Answer { card: 2, reply: "1".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 2, "reply": "1"}));
}

#[test]
fn a_hub_item_digit_goes_as_the_digit() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 7, "kind": "feature_try", "agent": "main", "text": "ambient-app is ready to try\n1. try it\n2. later"}]}));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::Answer { card: 7, reply: "1".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "answer", "card": 7, "reply": "1"}));
}

#[test]
fn send_goes_to_main_with_the_shot_as_a_captioned_image_part_kept_after_the_turn() {
    let mut t = T::new();
    t.ready();
    let png = t.dir.join("front.png");
    std::fs::write(&png, b"\x89PNG fake").unwrap();
    t.cmd(Cmd::Shot { path: png.clone(), app: "Chrome".into(), title: "localhost:4801/settings".into() });
    assert!(!png.exists(), "the app's PNG goes at once");
    t.cmd(Cmd::Send { text: "  why is this red?  ".into() });
    let req = t.hub.next();
    assert_eq!(req["cmd"], "send");
    assert_eq!(req["agent"], "main");
    // architect m_22000: an image part, its caption the app and window
    // (never his words), then his words; the text: the label and his words
    assert_eq!(req["text"], "[Image #1] why is this red?");
    let parts: Vec<bise_proto::parts::Part> = serde_json::from_value(req["parts"].clone()).expect("parts");
    let bise_proto::parts::Part::Image { name, source, caption, b64_file, .. } = &parts[0] else { panic!("{parts:?}") };
    assert_eq!((name.as_str(), caption.as_str(), b64_file.as_str()), ("[Image #1]", "Chrome · localhost:4801/settings", ""));
    assert_eq!(parts[1], bise_proto::parts::Part::Text { text: "why is this red?".into() });
    assert!(req.get("context").is_none() && !req["text"].as_str().unwrap().contains("<image"));
    // its source: the stored PNG (the hub stores it and fills its .b64)
    let b64 = PathBuf::from(source);
    assert!(b64.exists() && !source.ends_with("front.png"), "{source}");
    let out = t.take();
    assert!(out.contains(&json!({"ev": "sent", "text": "why is this red?", "voice": false, "shot": true})), "{out:?}");
    assert!(phase_is(&out, "sending"));
    // the next message has no shot
    t.cmd(Cmd::Send { text: "and this?".into() });
    assert_eq!(t.hub.next()["text"], "and this?");
    // his message's image: kept after main's turn (its chip reads it)
    t.hub.line("main", "  obs: turn_started");
    t.until(|o| phase_is(o, "working"));
    t.hub.line("main", "  obs: assistant: it's the CSS.");
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    assert!(b64.exists(), "a sent still stays with his message");
    // and a dropped still of the same window (the same store file) leaves it
    let png = t.dir.join("front.png");
    std::fs::write(&png, b"\x89PNG fake").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Chrome".into(), title: "localhost:4801/settings".into() });
    t.cmd(Cmd::ShotDrop { id: None });
    assert!(b64.exists(), "a dropped still never deletes a sent one's file");
}

/// The talk's still, told to the app (ambient-lead m_21912): `shot` with
/// its stored file once stored, `shot {file: null}` when he drops it
/// (`shot_drop`: forgotten, the talk goes on with his words only) and when
/// it is sent.
#[test]
fn the_talks_still_is_told_to_the_app_and_his_drop_forgets_it() {
    let mut t = T::new();
    t.ready();
    let shot = |t: &mut T, title: &str| {
        let png = t.dir.join("front.png");
        std::fs::write(&png, [b"\x89PNG ".as_slice(), title.as_bytes()].concat()).unwrap();
        t.cmd(Cmd::Shot { path: png, app: "Safari".into(), title: title.into() });
        let out = t.take();
        let ev = out.iter().find(|v| v["ev"] == "shot").cloned().unwrap_or_else(|| panic!("no shot event: {out:?}"));
        assert_eq!((ev["app"].as_str(), ev["title"].as_str()), (Some("Safari"), Some(title)));
        PathBuf::from(ev["file"].as_str().expect("its stored file"))
    };
    let first = shot(&mut t, "cold bench");
    assert!(first.exists(), "the still is in the image store");
    // his ✕: forgotten, the app told, nothing of it in the send
    t.cmd(Cmd::ShotDrop { id: None });
    assert!(t.take().contains(&json!({"ev": "shot", "file": null})));
    assert!(!first.exists(), "a dropped still is forgotten");
    t.cmd(Cmd::Send { text: "why is it slow?".into() });
    assert_eq!(t.hub.next()["text"], "why is it slow?");
    // a drop with no still says nothing
    t.cmd(Cmd::ShotDrop { id: None });
    assert!(!t.take().iter().any(|v| v["ev"] == "shot"));
    // sent: the app is told it went
    shot(&mut t, "warm bench");
    t.cmd(Cmd::Send { text: "and this one?".into() });
    let _ = t.hub.next();
    assert!(t.take().contains(&json!({"ev": "shot", "file": null})));
}

/// No fn context rides a capsule send any more (architect m_53938): the
/// old `fn_context` command is gone (its selection hid in the model's
/// text); his selection goes as a visible quote part (the spotlight's ❝).
/// The shot still goes as his message's image part.
#[test]
fn no_fn_context_rides_a_capsule_send() {
    let mut t = T::new();
    t.ready();
    assert!(Cmd::parse(r#"{"cmd":"fn_context","context":{"app":"Code","selection":"x"}}"#).is_err());
    let png = t.dir.join("front.png");
    std::fs::write(&png, b"\x89PNG fake").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Safari".into(), title: "p99".into() });
    t.cmd(Cmd::Send { text: "why is this slow?".into() });
    let req = t.hub.next();
    assert_eq!(req["text"], "[Image #1] why is this slow?");
    assert_eq!(req["parts"][0]["caption"], "Safari · p99");
    assert!(req.get("context").is_none(), "{req}");
}

/// The `files` list of a send went in issue 31 (a drop rides as parts): an
/// older window's list is ignored, never forwarded to the hub.
#[test]
fn a_sends_files_list_is_ignored() {
    let mut t = T::new();
    t.ready();
    let cmd = Cmd::parse(r#"{"cmd":"send","text":"hi","files":["/w/q3-notes.md"]}"#).unwrap();
    assert_eq!(cmd, Cmd::Send { text: "hi".into() });
    t.cmd(cmd);
    assert!(t.hub.next().get("files").is_none());
}

#[test]
fn a_shot_steered_into_a_running_turn_stays_after_its_end() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "working"}], "cards": []}));
    t.hub.line("main", "  obs: turn_started");
    t.ready();
    let png = t.dir.join("a.png");
    std::fs::write(&png, b"shot-a").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Notes".into(), title: String::new() });
    t.cmd(Cmd::Send { text: String::new() });
    let req = t.hub.next();
    let text = req["text"].as_str().unwrap().to_string();
    // no words: the image part alone, the text its label
    assert_eq!((text.as_str(), req["parts"][0]["caption"].as_str(), req["parts"].as_array().map(Vec::len)), ("[Image #1]", Some("Notes"), Some(1)));
    let b64 = PathBuf::from(req["parts"][0]["source"].as_str().unwrap());
    // steered into the running turn (a report's): from then on it is the
    // user's, main works for them, then done; the shot stays, his message's
    assert!(phase_is(&t.take(), "sending"));
    t.sync();
    assert!(!has(&t.take(), "phase"), "nothing yet: the turn is an agent's");
    t.hub.line("main", &format!("  obs: steering_received: {text}"));
    t.until(|o| phase_is(o, "working"));
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    assert!(b64.exists(), "a sent still stays with his message");
}

#[test]
fn a_message_sent_during_an_agents_turn_waits_for_the_next_one() {
    let mut t = T::new();
    // main's turn runs with no entry of its own yet: its row says so
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "working"}], "cards": []}));
    t.hub.line("main", "  obs: turn_started");
    t.ready();
    t.cmd(Cmd::Send { text: "after this".into() });
    t.hub.next();
    t.take();
    // the agent's turn ends without it: nothing said
    t.hub.line("main", "  obs: assistant: for the agent");
    t.hub.line("main", "  obs: turn_done: completed");
    t.sync();
    let out = t.take();
    assert!(!has(&out, "main") && !phases(&out).contains(&"done".to_string()), "{out:?}");
    // the next turn is the user's
    t.main_turn("Here, after it.");
    t.until(|o| phase_is(o, "done"));
    assert!(t.take().contains(&json!({"ev": "main", "text": "Here, after it.", "turn": 2})));
}

/// fn held, `words` heard, fn up: sent.
fn talk(t: &mut T, words: &str) {
    t.cmd(Cmd::TalkStart);
    let heard = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    heard.send(Heard::Text(format!(" {words}"))).unwrap();
    t.cmd(Cmd::TalkEnd);
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    assert_eq!(t.hub.next()["text"], words);
    t.take();
}

/// m_22120 (architect m_22168): fn up, a batch listener's words come at
/// its flush, after an HTTP request. 10 s later (the old wait was 1.5 s)
/// they still go to main; a request that never answers is said, never a
/// silent idle.
#[test]
fn a_slow_transcription_after_fn_up_still_sends_his_words() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::TalkStart);
    let heard = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    let Some(ago) = Instant::now().checked_sub(Duration::from_secs(10)) else { return };
    t.core.cmd(Cmd::TalkEnd, ago);
    for _ in 0..20 {
        t.until(|_| true);
    }
    assert!(!has(&t.out, "sent") && !phase_is(&t.out, "idle"), "the talk ended before its words: {:#?}", t.out);
    heard.send(Heard::Text(" is the build green".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    assert_eq!(t.hub.next()["text"], "is the build green");
    t.take();
    // never answers: past TRANSCRIBE_TIMEOUT, said as a failure
    t.cmd(Cmd::TalkStart);
    let Some(long_ago) = Instant::now().checked_sub(crate::voice::TRANSCRIBE_TIMEOUT + Duration::from_secs(1)) else { return };
    t.core.cmd(Cmd::TalkEnd, long_ago);
    t.until(|o| phase_is(o, "failed"));
    let out = t.take();
    assert!(out.iter().any(|v| v["ev"] == "error" && v["text"] == "the transcription timed out."), "{out:#?}");
}

/// A fn talk with no talk_end (fn up missed, the capsule hidden) stopped
/// nothing: it kept the mic and refused every dictation after it
/// (ambient-lead m_22196). After QUIET_STOP with no sound (the fake mic's
/// quiet blocks) it ends as at fn up; never a time limit while he talks.
#[test]
fn a_talk_never_ended_stops_after_a_long_silence_and_sends_his_words() {
    let mut t = T::new();
    t.ready();
    let Some(long_ago) = Instant::now().checked_sub(super::core::QUIET_STOP + Duration::from_secs(1)) else { return };
    t.core.cmd(Cmd::TalkStart, long_ago);
    let (heard, audio) = {
        let l = t.fakes.listen.lock().unwrap();
        (l.last().unwrap().heard.clone(), l.len() - 1)
    };
    let listen = t.fakes.listen.clone();
    t.until(move |_| listen.lock().unwrap()[audio].audio.try_iter().any(|m| m == ListenMsg::Flush));
    heard.send(Heard::Text(" left held".into())).unwrap();
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| has(o, "sent"));
    assert_eq!(t.hub.next()["text"], "left held");
    // never silent: one notice in the window says why (architect m_22291)
    let notices: Vec<Value> = t.take().into_iter().filter(|v| v["ev"] == "notice").collect();
    assert_eq!(notices.len(), 1, "{notices:#?}");
    assert_eq!(notices[0]["text"], "sent to main · the mic stopped after 5 min of silence");
}

#[test]
fn a_failed_turn_is_phase_failed_with_its_line() {
    let mut t = T::new();
    t.ready();
    t.hub.line("main", "sb you : what's up");
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: turn_done: failed: the provider is down");
    t.until(|o| phase_is(o, "failed"));
    let out = t.take();
    let f = out.iter().rfind(|v| v["ev"] == "phase").unwrap();
    assert_eq!(f["line"], "turn failed: the provider is down");
}

#[test]
fn a_tui_user_turn_sends_main_words_as_plain_text() {
    let mut t = T::new();
    t.ready();
    // typed in the terminal: the hub echoes it in main's feed
    t.hub.line("main", "sb you : is the build green?");
    t.main_turn("**Yes**: the `gate` is green, see [the PR](https://x/412).\\n\\n- 412 tests\\n- 0 flaky");
    t.until(|o| phase_is(o, "done"));
    let out = t.take();
    assert_eq!(phases(&out), vec!["working", "done"]);
    let main: Vec<&Value> = out.iter().filter(|v| v["ev"] == "main").collect();
    assert_eq!(main, vec![&json!({"ev": "main", "text": "Yes: the gate is green, see the PR.\n\n- 412 tests\n- 0 flaky", "turn": 1})]);
    // a slash command is the hub's: the turn after it is not the user's
    t.hub.line("main", "sb you : /answer 3 yes");
    t.main_turn("t1 has your answer.");
    t.sync();
    assert!(!has(&t.take(), "main"));
}

#[test]
fn msg_you_lines_are_main_words_with_who_wrote_them() {
    let mut t = T::new();
    // every live agent's thread is the capsule's (its words to him)
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "idle"}, {"name": "docs", "status": "working"}], "cards": []}));
    t.ready();
    t.hub.line("docs", "sb msg-you : docs : the **v2** page is up");
    t.until(|o| has(o, "main"));
    let out = t.take();
    assert!(out.contains(&json!({"ev": "main", "text": "the v2 page is up", "turn": 0, "from": "docs"})), "{out:?}");
    assert!(!has(&out, "phase"));
}

#[test]
fn pages_ride_the_state_and_a_waited_page_comes_in_front() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "idle"}, {"name": "old", "status": "idle"}], "cards": [],
        "pages": [{"id": "w", "title": "weekly update", "agent": "old", "version": 1, "url": "http://127.0.0.1:47100/p/w", "at_ms": 5, "state": "ready", "open_notes": 0}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["pages"][0]["id"], "w");
    t.ready();
    let page = |agent: &str| json!({"ev": "page", "id": "p", "title": "p", "agent": agent, "version": 1, "url": "u", "at_ms": 1, "state": "ready"});
    let front = |t: &mut T| -> bool {
        t.until(|o| has(o, "page"));
        let out = t.take();
        out.iter().find(|v| v["ev"] == "page").unwrap()["front"].as_bool().unwrap()
    };
    // nobody asked: never in front
    t.hub.say(page("main"));
    assert!(!front(&mut t));
    // the user asks; main publishes in that turn: in front
    t.hub.line("main", "sb you : draft my weekly update");
    t.hub.line("main", "  obs: turn_started");
    t.hub.say(page("main"));
    assert!(front(&mut t));
    // an agent that was there before the user's turn: not in front
    t.hub.say(page("old"));
    assert!(!front(&mut t));
    // an agent born since (main started it for the user): in front, even after main's turn
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "idle"}, {"name": "old", "status": "idle"}, {"name": "weekly-update", "status": "working"}], "cards": [], "pages": []}));
    t.hub.line("main", "  obs: turn_done: completed");
    t.hub.say(page("weekly-update"));
    assert!(front(&mut t));
    // updating: not in front
    t.hub.say(json!({"ev": "page", "id": "p", "agent": "weekly-update", "state": "updating"}));
    assert!(!front(&mut t));
}

/// A page/voice request on the home connection, as the fake hub gives it
/// (its HubCmd), without its project (the test workspace's hub id).
fn voice_req(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("project");
    }
    v
}

/// A note talk on page `id`: its words, then fn up; the hub's page/voice
/// requests, in order.
fn note_talk(t: &mut T, id: &str, words: &[&str]) -> Vec<Value> {
    t.cmd(Cmd::PageTalk { page: id.into() });
    let heard = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    let mut reqs = vec![voice_req(t.hub.next())];
    for w in words {
        heard.send(Heard::Text(format!(" {w}"))).unwrap();
        t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == false));
        reqs.push(voice_req(t.hub.next()));
        t.out.retain(|v| v["ev"] != "heard");
    }
    t.cmd(Cmd::TalkEnd);
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == true));
    reqs.push(voice_req(t.hub.next()));
    reqs
}

#[test]
fn a_note_talk_goes_to_the_page_never_to_main() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [],
        "pages": [{"id": "weekly-update", "title": "Weekly update", "agent": "a", "version": 1, "url": "u", "state": "ready"}]}));
    t.until(|o| has(o, "state"));
    t.ready();
    let reqs = note_talk(&mut t, "weekly-update", &["make the intro", "shorter"]);
    let pv = |phase: &str, text: &str| {
        let mut v = json!({"cmd": "page_voice", "page": "weekly-update", "phase": phase});
        if !text.is_empty() {
            v["text"] = json!(text);
        }
        v
    };
    assert_eq!(reqs, vec![pv("start", ""), pv("heard", "make the intro"), pv("heard", "make the intro shorter"), pv("end", "make the intro shorter")]);
    let out = t.take();
    // the capsule says where the words go, then idle: no sent, no main phases
    let first = out.iter().find(|v| v["ev"] == "phase").unwrap();
    assert_eq!(first, &json!({"ev": "phase", "phase": "listening", "page": {"id": "weekly-update", "title": "Weekly update"}}));
    assert_eq!(phases(&out), vec!["listening", "idle"]);
    assert!(!has(&out, "sent"), "{out:?}");

    // "send" alone sends the page's notes; a page not in the state: its id
    let reqs = note_talk(&mut t, "q3-plan", &["Envoie."]);
    assert_eq!(reqs[0]["page"], "q3-plan");
    assert_eq!(reqs.last().unwrap()["phase"], "send");
    let out = t.take();
    assert_eq!(out.iter().find(|v| v["ev"] == "phase").unwrap()["page"]["title"], "q3 plan");

    // nothing heard: cancel; talk_cancel: cancel
    let reqs = note_talk(&mut t, "weekly-update", &[]);
    assert_eq!(reqs.last().unwrap(), &pv("cancel", ""));
    t.cmd(Cmd::PageTalk { page: "weekly-update".into() });
    assert_eq!(voice_req(t.hub.next()), pv("start", ""));
    t.cmd(Cmd::TalkCancel);
    assert_eq!(voice_req(t.hub.next()), pv("cancel", ""));
    assert!(phase_is(&t.take(), "idle"));

    // never an input to main
    t.hub.w.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let mut more = String::new();
    assert!(t.hub.r.read_line(&mut more).is_err() || more.is_empty(), "more: {more}");
}

/// The core writes JSON-RPC only (architect m_15183): no `"op"` line in
/// the core's code (the older door's hello went with v2026.10.2-30's
/// window, debt-t3).
#[test]
fn the_core_writes_no_op_line() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ambient");
    let mut files = vec![root.join("core.rs")];
    let mut dirs = vec![root.join("core")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let needle = concat!("\"", "op", "\"");
    let mut at = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap_or("");
        for l in code.lines().filter(|l| l.contains(needle) && !l.trim_start().starts_with("//")) {
            at.push(format!("{}: {}", f.strip_prefix(&root).unwrap().display(), l.trim()));
        }
    }
    assert_eq!(at, Vec::<String>::new());
}

/// Law (client-protocol step 5, proto-lead m_15443): an initialized
/// connection is read as typed events only. The core's code never parses
/// a feed line (`parse_line`) and never reads an event's `"ev"` tag; its
/// home connection has no older `state`, `ready`, `page` or `line` arm.
#[test]
fn an_initialized_connection_reads_no_older_event() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ambient");
    let mut files = vec![root.join("core.rs")];
    for e in std::fs::read_dir(root.join("core")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "rs") {
            files.push(e.path());
        }
    }
    let tag = concat!("\"", "ev", "\")");
    let parse = concat!("parse", "_line(");
    // a match arm on an older event's tag (`"state" => ...`)
    let arms = ["state", "ready", "page", "line"].map(|k| format!("\"{k}\""));
    let mut at = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap_or("");
        let arm = |l: &str| arms.iter().any(|a| l.trim_start().starts_with(a.as_str()) && l.contains("=>"));
        let older = |l: &str| l.contains(tag) || l.contains(parse) || arm(l);
        for l in code.lines().filter(|l| older(l) && !l.trim_start().starts_with("//")) {
            at.push(format!("{}: {}", f.strip_prefix(&root).unwrap().display(), l.trim()));
        }
    }
    assert_eq!(at, Vec::<String>::new());
}

#[test]
fn a_page_question_card_carries_its_page_link() {
    let mut t = T::new();
    let link = json!({"id": "w", "block": "q1", "url": "http://127.0.0.1:47100/p/w#q1"});
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [],
        "cards": [{"id": 7, "kind": "question", "agent": "a", "text": "post now?\n1. post now\n2. wait", "page": link},
                  {"id": 8, "kind": "question", "agent": "a", "text": "plain?"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["cards"][0]["page"], link);
    assert_eq!(st["cards"][0]["options"][1]["label"], "wait");
    assert!(st["cards"][1].get("page").is_none());
}

#[test]
fn talk_start_with_a_page_is_a_note_talk() {
    assert_eq!(
        Cmd::parse(r#"{"cmd":"talk_start","page":{"id":"w","url":"http://127.0.0.1:47100/p/w"}}"#),
        Ok(Cmd::PageTalk { page: "w".into() })
    );
    assert_eq!(Cmd::parse(r#"{"cmd":"talk_start","page":{"id":" "}}"#), Ok(Cmd::TalkStart));
    assert_eq!(Cmd::parse(r#"{"cmd":"talk_start","page":null}"#), Ok(Cmd::TalkStart));
}

#[test]
fn what_the_capsule_sends_main_says_it_came_from_the_capsule() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::Send { text: "what's running?".into() });
    assert_eq!(
        t.hub.sent(),
        json!({"cmd": "send", "agent": "main", "text": "what's running?", "via": "capsule"})
    );
    // an answer is an /answer command: no via, the hub adds no hint to it
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 4, "kind": "question", "agent": "a", "text": "now?\n1. yes\n2. no"}]}));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::Answer { card: 4, reply: "1".into() });
    assert!(t.hub.next().get("via").is_none());
}

#[test]
fn bise_ambient_opens_the_desktop_app_through_desktop_sh() {
    use std::path::Path;
    let (script, args) = super::launch_command(Path::new("/w/shop"), Path::new("/b/bise"), Path::new("/repo"));
    assert_eq!(script, Path::new("/repo/scripts/desktop.sh"));
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args, ["open", "--workspace", "/w/shop", "--bise", "/b/bise"]);
}
