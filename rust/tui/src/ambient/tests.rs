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
mod dictate;
mod fake_hub;
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
        T::make(None)
    }

    /// harness A's ports (mod.rs fake_ports): the fake voice's words.
    fn fake_voice(words: super::fake::Words) -> T {
        T::make(Some(words))
    }

    fn make(fake: Option<super::fake::Words>) -> T {
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
        };
        let ports = match fake {
            Some(w) => {
                let mut p = super::fake_ports(user_kind, w);
                p.store_image = Box::new(fake_store(dir.clone()));
                p
            }
            None => ports,
        };
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

/// Law (pm's C fail 36): bise's own bookkeeping (main's archive
/// suggestion, a `drop` card) is never a card on ambient: not on the
/// glass, not in the count, not in needs-you; it stays in the TUI.
#[test]
fn housekeeping_cards_stay_in_the_tui() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 2, "kind": "drop", "agent": "proxy-fix", "text": "main suggests: archive @proxy-fix? the agent is mid-turn. (answer yes or no)"},
        {"id": 3, "kind": "question", "agent": "main", "text": "ship as a PR?
1. yes
2. no"}]}));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    let ids: Vec<u64> = st["cards"].as_array().unwrap().iter().map(|c| c["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, vec![3]);
    // and it cannot be answered from the capsule
    t.cmd(Cmd::Answer { card: 2, reply: "1".into() });
    assert!(has(&t.take(), "error"));
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
fn send_goes_to_main_with_the_shot_marker_and_the_png_is_deleted() {
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
    let text = req["text"].as_str().unwrap();
    assert!(text.starts_with("why is this red?\n\n<image name=\"[Screen]\" path=\"Chrome · localhost:4801/settings\" mime=\"image/png\" b64=\""), "{text}");
    let ms = bend_images::markers(text);
    assert_eq!(ms.len(), 1);
    let b64 = PathBuf::from(&ms[0].b64);
    assert!(b64.exists());
    let out = t.take();
    assert!(out.contains(&json!({"ev": "sent", "text": "why is this red?", "voice": false, "shot": true})), "{out:?}");
    assert!(phase_is(&out, "sending"));
    // the next message has no shot
    t.cmd(Cmd::Send { text: "and this?".into() });
    assert_eq!(t.hub.next()["text"], "and this?");
    // kept while main's turn reads it, forgotten at its end
    t.hub.line("main", "  obs: turn_started");
    t.until(|o| phase_is(o, "working"));
    assert!(b64.exists());
    t.hub.line("main", "  obs: assistant: it's the CSS.");
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    assert!(!b64.exists(), "the stored shot is not kept after main's turn");
}

/// S9 (amb-mac m_9035): the latest fn_context rides the next message to
/// main as the input op's `context` field (never rendered by the core),
/// its shot is the stored file in the image store, and it's gone after
/// that message; an excluded app's `{}` clears it; a cancelled talk drops
/// it.
#[test]
fn the_fn_context_rides_the_next_message_once() {
    use bise_proto::context::FnContext;
    let mut t = T::new();
    t.ready();
    let safari = FnContext { app: Some("Safari".into()), url: Some("https://grafana.acme.test/d/p99".into()), ..FnContext::default() };
    t.cmd(Cmd::FnContext(FnContext { app: Some("Mail".into()), ..FnContext::default() }));
    t.cmd(Cmd::FnContext(safari.clone()));
    let png = t.dir.join("front.png");
    std::fs::write(&png, b"\x89PNG fake").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Safari".into(), title: "p99".into() });
    t.cmd(Cmd::Send { text: "why is this slow?".into() });
    let req = t.hub.next();
    let text = req["text"].as_str().unwrap();
    assert!(text.starts_with("why is this slow?\n\n<image name=\"[Screen]\""), "the core never renders the context: {text}");
    assert!(!text.contains("screen_context") && !text.contains("grafana"), "{text}");
    let ctx: FnContext = serde_json::from_value(req["context"].clone()).unwrap();
    let shot = ctx.shot.clone().expect("the shot's stored file");
    // the stored file (the app's PNG is gone), beside the marker's b64
    assert!(std::path::Path::new(&shot).exists() && !shot.ends_with("front.png"), "{shot}");
    let b64 = PathBuf::from(&bend_images::markers(text)[0].b64);
    assert_eq!(std::path::Path::new(&shot).parent(), b64.parent(), "{shot}");
    assert_eq!(FnContext { shot: None, ..ctx }, safari);
    // once
    t.cmd(Cmd::Send { text: "and this?".into() });
    assert!(t.hub.next().get("context").is_none());
    // an excluded app clears it
    t.cmd(Cmd::FnContext(safari.clone()));
    t.cmd(Cmd::FnContext(FnContext::default()));
    t.cmd(Cmd::Send { text: "third".into() });
    assert!(t.hub.next().get("context").is_none());
    // a cancelled talk drops it
    t.cmd(Cmd::TalkStart);
    t.cmd(Cmd::FnContext(safari));
    t.cmd(Cmd::TalkCancel);
    t.cmd(Cmd::Send { text: "fourth".into() });
    assert!(t.hub.next().get("context").is_none());
    assert_eq!(Cmd::parse(r#"{"cmd":"fn_context","context":{}}"#), Ok(Cmd::FnContext(FnContext::default())));
    assert_eq!(
        Cmd::parse(r#"{"cmd":"fn_context","context":{"app":"Code","file":"/w/a.rs"}}"#),
        Ok(Cmd::FnContext(FnContext { app: Some("Code".into()), file: Some("/w/a.rs".into()), ..FnContext::default() }))
    );
    assert!(Cmd::parse(r#"{"cmd":"fn_context"}"#).is_err());
}

/// T1 run 5 step 18 (amb-web m_9242): a drop sent from the spotlight,
/// {cmd:'send', text, files}, reaches bise's main with the files' paths
/// after his words (alone when he typed nothing); a relative path is
/// refused, never sent.
#[test]
fn a_dropped_file_reaches_main_with_his_words() {
    let mut t = T::new();
    t.ready();
    let cmd = Cmd::parse(r#"{"cmd":"send","text":" summarize this ","files":["/w/q3-notes.md","/w/b.png"]}"#).unwrap();
    t.cmd(cmd);
    // item H: his words and the files as a field; the hub renders them once
    let input = t.hub.next();
    assert_eq!((input["text"].as_str(), input["files"].clone()), (Some("summarize this"), serde_json::json!(["/w/q3-notes.md", "/w/b.png"])));
    t.cmd(Cmd::parse(r#"{"cmd":"send","text":"","files":["/w/q3-notes.md"]}"#).unwrap());
    let input = t.hub.next();
    assert_eq!((input["text"].as_str(), input["files"].clone()), (Some(""), serde_json::json!(["/w/q3-notes.md"])));
    // a send without files has no files field
    t.cmd(Cmd::parse(r#"{"cmd":"send","text":"hi"}"#).unwrap());
    assert!(t.hub.next().get("files").is_none());
    assert_eq!(Cmd::parse(r#"{"cmd":"send","text":"hi","files":[]}"#), Ok(Cmd::Send { text: "hi".into() }));
    assert!(Cmd::parse(r#"{"cmd":"send","text":"hi","files":["notes.md"]}"#).is_err());
}

#[test]
fn a_shot_steered_into_a_running_turn_is_forgotten_at_its_end() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "working"}], "cards": []}));
    t.hub.line("main", "  obs: turn_started");
    t.ready();
    let png = t.dir.join("a.png");
    std::fs::write(&png, b"shot-a").unwrap();
    t.cmd(Cmd::Shot { path: png, app: "Notes".into(), title: String::new() });
    t.cmd(Cmd::Send { text: String::new() });
    let text = t.hub.next()["text"].as_str().unwrap().to_string();
    assert!(text.starts_with("<image name=\"[Screen]\" path=\"Notes\""), "{text}");
    let b64 = PathBuf::from(&bend_images::markers(&text)[0].b64);
    // steered into the running turn (a report's): from then on it is the
    // user's, main works for them, then done; the shot goes at its end
    assert!(phase_is(&t.take(), "sending"));
    t.sync();
    assert!(!has(&t.take(), "phase"), "nothing yet: the turn is an agent's");
    t.hub.line("main", &format!("  obs: steering_received: {text}"));
    t.until(|o| phase_is(o, "working"));
    assert!(b64.exists());
    t.hub.line("main", "  obs: turn_done: completed");
    t.until(|o| phase_is(o, "done"));
    assert!(!b64.exists());
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
fn the_hello_replay_says_where_main_is_but_not_its_old_words() {
    let mut t = T::new();
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "working"}], "cards": []}));
    t.until(|o| has(o, "state"));
    t.take();
    t.hub.line("main", "sb you : an old question");
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: assistant: an old message");
    t.hub.line("main", "  obs: turn_done: completed");
    t.hub.line("main", "sb msg-you : docs : an old note");
    t.hub.line("main", "sb you : the question main is on");
    t.hub.line("main", "  obs: turn_started");
    t.replay_over();
    t.until(|o| has(o, "phase"));
    let out = t.take();
    assert_eq!(phases(&out), vec!["working"]);
    assert!(!has(&out, "main"), "nothing of the replay is sent: {out:?}");
    // other agents' lines never move main's phase
    t.hub.line("cookies", "  obs: turn_done: completed");
    // a turn with no entry of its own ends when main's row stops running
    t.hub.line("main", "  obs: turn_done: completed");
    t.hub.say(json!({"ev": "state", "agents": [{"name": "main", "main": true, "status": "idle"}], "cards": []}));
    t.until(|o| has(o, "phase"));
    assert_eq!(phases(&t.take()), vec!["done"]);
}

#[test]
fn hub_loss_says_up_false_then_reconnects() {
    let mut t = T::new();
    t.ready();
    // the hub goes away
    let old = std::mem::replace(&mut t.hub, t.ends.recv_timeout(Duration::from_millis(1)).unwrap_or_else(|_| dead_end()));
    drop(old);
    t.until(|o| o.contains(&json!({"ev": "hub", "up": false, "workspace": WS})));
    // a send while it is away: said, never lost silently
    let out = t.take();
    assert!(!has(&out, "phase"));
    // the core reconnects on its own
    t.until(|o| o.contains(&json!({"ev": "hub", "up": true, "workspace": WS})));
    t.hub = t.ends.recv_timeout(Duration::from_secs(3)).unwrap();
    t.ready();
    t.cmd(Cmd::Send { text: "back?".into() });
    assert_eq!(t.hub.next()["text"], "back?");
}

/// A hub end with nobody behind it (placeholder while swapping).
fn dead_end() -> HubEnd {
    let (a, _b) = UnixStream::pair().unwrap();
    let r = BufReader::new(a.try_clone().unwrap());
    HubEnd::new(a, r)
}

#[test]
fn a_send_while_the_hub_is_away_is_an_error() {
    let mut t = T::new();
    let hub = Hub::default();
    let _ = &mut t;
    // a core whose hub never connected
    let fakes = Fakes::new();
    let ports = Ports {
        mic: Box::new(FakeMic(fakes.mic.clone())),
        listener: Box::new(|_| Box::new(crate::voicemode::fakes::FakeListener(Default::default()))),
        synth: Box::new(FakeSynth(fakes.synth.clone())),
        open_speaker: Box::new(|| Err("no speaker".into())),
        listen_job: Box::new(|| Err("no voice model".into())),
        say_job: Box::new(|| Err("no voice".into())),
        language: None,
        store_image: Box::new(|_| Err("no store".into())),
        user_kind,
        fake_words: None,
        voice_mode: Box::new(|| Err("no voice mode".into())),
    };
    let mut core = Core::new(WS.into(), hub, ports);
    core.cmd(Cmd::Send { text: "hello".into() }, Instant::now());
    let out = core.take_out();
    assert!(has(&out, "error") && phase_is(&out, "failed"), "{out:?}");
    core.cmd(Cmd::TalkStart, Instant::now());
    assert!(has(&core.take_out(), "error"));
}

#[test]
fn cmd_lines_parse_as_the_contract_says() {
    assert_eq!(Cmd::parse(r#"{"cmd":"talk_start"}"#), Ok(Cmd::TalkStart));
    assert_eq!(
        Cmd::parse(r#"{"cmd":"shot","path":"/t/a.png","app":"Chrome","title":"x"}"#),
        Ok(Cmd::Shot { path: "/t/a.png".into(), app: "Chrome".into(), title: "x".into() })
    );
    assert_eq!(Cmd::parse(r##"{"cmd":"answer","card":"#3","reply":"1"}"##), Ok(Cmd::Answer { card: 3, reply: "1".into() }));
    assert_eq!(Cmd::parse(r#"{"cmd":"answer","card":3,"reply":"yes"}"#), Ok(Cmd::Answer { card: 3, reply: "yes".into() }));
    assert!(Cmd::parse(r#"{"cmd":"answer","reply":"yes"}"#).is_err());
    assert!(Cmd::parse(r#"{"cmd":"fly"}"#).is_err());
    assert!(Cmd::parse("nope").is_err());
}

#[test]
fn the_run_loop_writes_lines_and_ends_with_stdin() {
    let (tx, rx) = mpsc::channel();
    let hub = Hub::default();
    let fakes = Fakes::new();
    let ports = Ports {
        mic: Box::new(FakeMic(fakes.mic.clone())),
        listener: Box::new(|_| Box::new(crate::voicemode::fakes::FakeListener(Default::default()))),
        synth: Box::new(FakeSynth(fakes.synth.clone())),
        open_speaker: Box::new(|| Err("no speaker".into())),
        listen_job: Box::new(|| Err("no voice model".into())),
        say_job: Box::new(|| Err("no voice".into())),
        language: None,
        store_image: Box::new(|_| Err("no store".into())),
        user_kind,
        fake_words: None,
        voice_mode: Box::new(|| Err("no voice mode".into())),
    };
    let mut core = Core::new(WS.into(), hub, ports);
    tx.send(super::In::Hub(HubIn::Down)).unwrap();
    tx.send(super::In::Line("garbage".into())).unwrap();
    tx.send(super::In::Line(r#"{"cmd":"hush"}"#.into())).unwrap();
    tx.send(super::In::Eof).unwrap();
    let mut out = Vec::new();
    super::run(&mut core, rx, &mut out);
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text, format!("{}\n", json!({"ev": "hub", "up": false, "workspace": WS})));
}

#[test]
fn a_report_triggered_turn_sends_no_words_and_no_phase() {
    let mut t = T::new();
    // the replay at launch: main answering an agent's report
    t.hub.line("main", "sb msg-in : perf m_12 : [report: done] signup 4.1 s -> 0.9 s");
    t.hub.line("main", "  obs: turn_started");
    t.ready();
    t.hub.line("main", "tool_intent #1 : reads the report");
    t.hub.line("main", "  obs: assistant: **perf** is done; I merged it.");
    t.hub.line("main", "  obs: turn_done: completed");
    // live: another report, another turn
    t.hub.line("main", "sb msg-in : docs m_13 : [report: progress] half way");
    t.main_turn("Noted, docs is half way.");
    t.sync();
    let out = t.take();
    assert!(!has(&out, "main") && !has(&out, "phase"), "{out:?}");
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

/// After `initialize` the core writes JSON-RPC only (architect m_15183):
/// no `"op"` line in the core's code but the older door's hello to a hub
/// before client-protocol (hubs.rs, HubIn::Up), the one release it stays.
#[test]
fn the_core_writes_no_op_line_but_the_older_doors_hello() {
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
    // TODO(client-protocol, the plan's 'after the release' step, with
    // Hubs.older_door): none at all
    assert_eq!(at, vec![r#"core/hubs.rs: c.hub.send(&json!({"op": "hello"}));"#.to_string()]);
}

/// Law (client-protocol step 5, proto-lead m_15443): an initialized
/// connection is read as typed events only. The core's code never parses
/// a feed line (`parse_line`) and never reads an event's `"ev"` tag but in
/// the older door's typed line of a project hub before client-protocol
/// (hubs.rs project_line, the one release it stays); its home connection
/// has no older `state`, `ready`, `page` or `line` arm.
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
    // TODO(client-protocol, the plan's 'after the release' step, with
    // Hubs.older_door): none at all
    assert_eq!(at, vec![r#"core/hubs.rs: let ev = v.get("ev").and_then(Value::as_str).unwrap_or("");"#.to_string()]);
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
fn the_fake_voice_talks_to_main_through_the_real_talk_path() {
    // harness A (lead m_7976): no mic, no STT, no sound; the same events
    // and the same input to main as his voice
    let dir = temp_dir();
    let words = super::fake::Words::new(dir.join("voice.txt"));
    std::fs::write(words.file(), "what is running now").unwrap();
    let mut t = T::fake_voice(words.clone());
    t.ready();
    t.cmd(Cmd::TalkStart);
    assert!(!words.file().exists(), "read and removed at talk_start");
    t.until(|o| o.iter().any(|v| v["ev"] == "level" && v["who"] == "you" && v["v"].as_f64().unwrap_or(0.0) > 0.2));
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == false && v["text"] == "what"));
    t.cmd(Cmd::TalkEnd);
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == true));
    let out = t.take();
    assert!(out.iter().any(|v| v["ev"] == "heard" && v["final"] == true && v["text"] == "what is running now"), "{out:?}");
    assert!(out.iter().any(|v| v["ev"] == "sent" && v["voice"] == true));
    assert_eq!(t.hub.sent(), json!({"cmd": "send", "agent": "main", "text": "what is running now", "via": "capsule"}));
    // main answers: said through the silent speaker, word by word, done
    t.main_turn("two agents work.");
    t.until(|o| phase_is(o, "speaking"));
    t.until(|o| has(o, "word") && phase_is(o, "done"));
    // the fake_words cmd: the next talk's words, the file not needed
    t.cmd(Cmd::FakeWords { text: "stop perf".into() });
    t.cmd(Cmd::TalkStart);
    t.cmd(Cmd::TalkEnd);
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == true && v["text"] == "stop perf"));
    assert_eq!(t.hub.next()["text"], "stop perf");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fake_words_is_refused_in_his_normal_run() {
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::parse(r#"{"cmd":"fake_words","text":"hi"}"#).unwrap());
    assert!(has(&t.take(), "error"));
}

#[test]
fn bise_ambient_opens_the_desktop_app_through_desktop_sh() {
    use std::path::Path;
    let (script, args) = super::launch_command(Path::new("/w/shop"), Path::new("/b/bise"), Path::new("/repo"));
    assert_eq!(script, Path::new("/repo/scripts/desktop.sh"));
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args, ["open", "--workspace", "/w/shop", "--bise", "/b/bise"]);
}
