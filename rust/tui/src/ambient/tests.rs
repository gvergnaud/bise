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

mod dictate;
mod hubs;
mod setup;
mod voice_mode;

/// The hub's `user_kind` (switchboard::model), as main.rs hands it in.
fn user_kind(kind: &str) -> bool {
    matches!(kind, "question" | "drop" | "confirm" | "merge" | "feature_try" | "feature_merge" | "update")
}

/// The hub's side of one connection.
struct HubEnd {
    w: UnixStream,
    r: BufReader<UnixStream>,
}

impl HubEnd {
    fn say(&mut self, v: Value) {
        writeln!(self.w, "{v}").unwrap();
    }

    fn line(&mut self, agent: &str, line: &str) {
        self.say(json!({"ev": "line", "agent": agent, "line": line, "pos": 1}));
    }

    /// The next request the core wrote (the connector's hello skipped).
    fn next(&mut self) -> Value {
        loop {
            let mut l = String::new();
            self.r.read_line(&mut l).expect("the core wrote nothing");
            let v: Value = serde_json::from_str(l.trim()).unwrap();
            // the connector's hello, then the core's typed hello (S3b)
            if v["op"] != "hello" && v["cmd"] != "hello" {
                return v;
            }
        }
    }
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
            let (mut core_end, hub_end) = UnixStream::pair()?;
            core_end.write_all(b"{\"op\":\"hello\"}\n")?;
            let r = BufReader::new(hub_end.try_clone()?);
            r.get_ref().set_read_timeout(Some(Duration::from_secs(3)))?;
            etx.send(HubEnd { w: hub_end, r }).map_err(std::io::Error::other)?;
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
    fn until(&mut self, done: impl Fn(&[Value]) -> bool) {
        let t0 = Instant::now();
        loop {
            while let Ok(h) = self.rx.try_recv() {
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

    /// The hello replay is over: main's lines are live.
    fn ready(&mut self) {
        self.hub.say(json!({"ev": "ready"}));
        self.until(|o| has(o, "phase"));
        self.take();
    }

    /// Everything the hub said so far is handled (a state round trip).
    fn sync(&mut self) {
        self.hub.say(json!({"ev": "state", "agents": [], "cards": []}));
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
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "/answer 3 yes"}));
    // words go as they are
    t.cmd(Cmd::Answer { card: 3, reply: "only on desktop".into() });
    assert_eq!(t.hub.next()["text"], "/answer 3 only on desktop");
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
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "/answer 3 1"}));
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
        assert_eq!(t.hub.next()["text"], format!("/answer {} straight to main", i + 1), "{}", forms[i]);
    }
    t.cmd(Cmd::Answer { card: 9, reply: "2".into() });
    assert_eq!(t.hub.next()["text"], "/answer 9 2");
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
        "page": {"id": "bugs", "block": "r", "item": "r1", "drafts": true, "url": "http://x/p/bugs#r1"}});
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "cards": [batch(1, 6)]}));
    t.until(|o| has(o, "state"));
    t.take();
    t.hub.say(json!({"ev": "state", "agents": [], "pages": [], "cards": [batch(2, 7)]}));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::Answer { card: 1, reply: "2".into() });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "/answer 1 2"}));
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
    assert_eq!(t.hub.next()["text"], "/answer 1 arrête-le");
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
    assert_eq!(t.hub.next()["text"], "/answer 7 1");
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
    assert_eq!(req["op"], "input");
    assert_eq!(req["focus"], "main");
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
    assert_eq!(t.hub.next(), json!({"op": "every_stop", "id": 3}));
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
    t.take();
    t.hub.line("main", "sb you : an old question");
    t.hub.line("main", "  obs: turn_started");
    t.hub.line("main", "  obs: assistant: an old message");
    t.hub.line("main", "  obs: turn_done: completed");
    t.hub.line("main", "sb msg-you : docs : an old note");
    t.hub.line("main", "sb you : the question main is on");
    t.hub.line("main", "  obs: turn_started");
    t.hub.say(json!({"ev": "ready"}));
    t.until(|o| has(o, "phase"));
    let out = t.take();
    assert_eq!(phases(&out), vec!["working"]);
    assert!(!has(&out, "main"), "nothing of the replay is sent: {out:?}");
    // other agents' lines never move main's phase
    t.hub.line("cookies", "  obs: turn_done: completed");
    t.hub.line("main", "  obs: turn_done: completed");
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
    HubEnd { w: a, r }
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

/// A note talk on page `id`: its words, then fn up; the hub's
/// page_voice requests, in order.
fn note_talk(t: &mut T, id: &str, words: &[&str]) -> Vec<Value> {
    t.cmd(Cmd::PageTalk { page: id.into() });
    let heard = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
    let mut reqs = vec![t.hub.next()];
    for w in words {
        heard.send(Heard::Text(format!(" {w}"))).unwrap();
        t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == false));
        reqs.push(t.hub.next());
        t.out.retain(|v| v["ev"] != "heard");
    }
    t.cmd(Cmd::TalkEnd);
    heard.send(Heard::Flushed).unwrap();
    t.until(|o| o.iter().any(|v| v["ev"] == "heard" && v["final"] == true));
    reqs.push(t.hub.next());
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
    let pv = |phase: &str, text: &str| json!({"op": "page_voice", "page": "weekly-update", "phase": phase, "text": text});
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
    assert_eq!(t.hub.next(), pv("start", ""));
    t.cmd(Cmd::TalkCancel);
    assert_eq!(t.hub.next(), pv("cancel", ""));
    assert!(phase_is(&t.take(), "idle"));

    // never an input to main
    t.hub.w.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
    t.hub.r.get_ref().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let mut more = String::new();
    assert!(t.hub.r.read_line(&mut more).is_err() || more.is_empty(), "more: {more}");
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
        t.hub.next(),
        json!({"op": "input", "focus": "main", "text": "what's running?", "via": "capsule"})
    );
    // an answer is an /answer command: no via, the hub adds no hint to it
    t.hub.say(json!({"ev": "state", "agents": [], "cards": [
        {"id": 4, "kind": "question", "agent": "a", "text": "now?\n1. yes\n2. no"}]}));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::Answer { card: 4, reply: "1".into() });
    assert!(t.hub.next().get("via").is_none());
}

// ---- round 10: the agents (identity10 #data, ambient-lead m_7026) ----

fn agents_state(perf: &str) -> Value {
    json!({"ev": "state",
        "agents": [
            {"name": "main", "main": true, "status": "idle"},
            {"name": "perf", "status": perf, "objective": "make the e2e fast\nmore", "note": "profiling the hub", "report": "", "created_ms": 5},
            {"name": "old", "status": "archived", "objective": "an old fix", "report": "fixed in 0.4", "report_ms": 7},
        ],
        "cards": [{"id": 9, "kind": "question", "agent": "perf", "text": "which bench?\n1. cold\n2. warm"}],
        "pages": [{"id": "perf-notes", "title": "Perf notes", "agent": "perf", "version": 2, "url": "http://p/perf-notes", "at_ms": 3}]})
}

#[test]
fn agents_rows_carry_status_title_purpose_since_waits_and_archived() {
    let mut t = T::new();
    t.ready();
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    let rows = st["agents"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "main is not a row, an archived agent is: {rows:?}");
    let (perf, old) = (&rows[0], &rows[1]);
    assert_eq!(perf["status"], "working");
    assert_eq!(perf["title"], "profiling the hub");
    assert_eq!(perf["purpose"], "make the e2e fast");
    assert_eq!(perf["waits"], 1);
    assert_eq!(perf["archived"], false);
    assert_eq!(perf["followed"], false);
    assert_eq!(perf["since_ms"], 5, "first sight: its birth");
    assert_eq!((old["status"].as_str(), old["archived"].as_bool(), old["title"].as_str()), (Some("done"), Some(true), Some("fixed in 0.4")));
    assert_eq!(old["since_ms"], 7);
    // the status changes: since moves; follow shows on the row
    t.hub.say(agents_state("idle"));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["agents"][0]["status"], "idle");
    assert!(st["agents"][0]["since_ms"].as_u64().unwrap() > 1_000_000);
    t.cmd(Cmd::Follow { agent: "perf".into(), on: true });
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["agents"][0]["followed"], true);
}

#[test]
fn agents_cmds_parse() {
    let p = |s: &str| Cmd::parse(s);
    assert_eq!(p(r#"{"cmd":"agent_preview","agent":"@perf"}"#), Ok(Cmd::AgentPreview { agent: "perf".into() }));
    assert_eq!(p(r#"{"cmd":"agent_history","agent":"perf"}"#), Ok(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 }));
    assert_eq!(p(r#"{"cmd":"agent_history","agent":"perf","before":40,"limit":20}"#), Ok(Cmd::AgentHistory { agent: "perf".into(), before: Some(40), limit: 20 }));
    assert_eq!(p(r#"{"cmd":"agent_send","agent":"perf","text":"hi","mode":"queued"}"#), Ok(Cmd::AgentSend { agent: "perf".into(), text: "hi".into(), queued: true }));
    assert_eq!(p(r#"{"cmd":"agent_send","agent":"perf","text":"hi","mode":"now"}"#), Ok(Cmd::AgentSend { agent: "perf".into(), text: "hi".into(), queued: false }));
    assert_eq!(p(r#"{"cmd":"archive","agent":"perf","stop_first":true}"#), Ok(Cmd::Archive { agent: "perf".into(), stop_first: true }));
    assert_eq!(p(r#"{"cmd":"unarchive","agent":"perf"}"#), Ok(Cmd::Unarchive { agent: "perf".into() }));
    assert_eq!(p(r#"{"cmd":"follow","agent":"perf","on":false}"#), Ok(Cmd::Follow { agent: "perf".into(), on: false }));
    assert!(p(r#"{"cmd":"stop"}"#).is_err(), "no agent: the app's bug");
}

/// perf's feed: his message, a reply, three tool calls, a report.
fn perf_lines() -> Vec<Line> {
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    vec![
        l(1, "sb you : make it fast"),
        l(2, "  obs: turn_started"),
        l(3, "  obs: assistant: <think>hmm</think>on it"),
        l(4, "tool #1 bash : cargo test -q"),
        l(5, "tool_intent #1 : running the tests"),
        l(6, "tool_result #1 ok : 3 failed"),
        l(7, "tool #2 read_file : {\"path\":\"a.rs\"}"),
        l(8, "tool #3 bash : sb land \"fast\""),
        l(9, "tool_intent #3 : landing the fix"),
        l(10, "sb msg-in : ambient-lead m_3 : nice\\nsecond line"),
        l(11, "tool #4 bash : sb report done \"the e2e takes 40 s\""),
        l(12, "  obs: turn_done: completed"),
    ]
}

const PROJECT: &str = "ws-0000beef";

/// The hub's fold of perf's lines (bise_proto::thread, what the hub
/// sends): card 9 open, perf-notes published.
fn hub_fold(lines: &[Line]) -> Vec<Entry> {
    let pages = |id: &str| {
        (id == "perf-notes").then(|| PageRef { id: id.into(), title: "Perf notes".into(), v: Some(2), url: "http://p/perf-notes".into() })
    };
    thread::fold(lines, &Ctx { open_cards: &[9], page: &pages, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &bise_proto::thread::Attached::plain })
}

/// The hub's typed hello answered (the target's project).
fn welcome(t: &mut T) {
    t.hub.say(json!({"ev": "welcome", "project": PROJECT, "proto": 1, "workspace": "/w", "name": "w"}));
}

fn thread_ev(agent: &str, entries: &[Entry], before: Option<u64>, more: bool) -> Value {
    json!({"ev": "thread", "project": PROJECT, "agent": agent, "entries": entries, "before": before, "more": more})
}

fn entry_ev(agent: &str, e: &Entry) -> Value {
    json!({"ev": "entry", "project": PROJECT, "agent": agent, "entry": e})
}

#[test]
fn a_preview_shows_now_the_last_actions_what_waits_the_report_and_pages() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentPreview { agent: "perf".into() });
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 8, "project": PROJECT}));
    t.hub.say(thread_ev("perf", &hub_fold(&perf_lines()), None, false));
    t.until(|o| has(o, "agent_preview"));
    let p = t.take().into_iter().find(|v| v["ev"] == "agent_preview").unwrap();
    let acts: Vec<(&str, &str)> = p["actions"].as_array().unwrap().iter().map(|a| (a["kind"].as_str().unwrap(), a["text"].as_str().unwrap())).collect();
    assert_eq!(acts, [("message", "on it"), ("tool", "running the tests"), ("tool", "read_file: {\"path\":\"a.rs\"}"), ("land", "landing the fix"), ("report", "the e2e takes 40 s")]);
    assert_eq!(p["waiting"], json!([{"card_id": 9, "question": "which bench?"}]));
    assert_eq!(p["last_report"], json!({"kind": "done", "text": "the e2e takes 40 s", "at_ms": 1011}));
    assert_eq!(p["pages"], json!([{"id": "perf-notes", "title": "Perf notes", "v": 2, "url": "http://p/perf-notes"}]));
    assert_eq!(p["now"], "profiling the hub");
    // its next step re-sends the preview: now is that step
    t.hub.say(json!({"ev": "typing", "project": PROJECT, "agent": "perf", "text": "listing the bench files"}));
    t.until(|o| o.iter().any(|v| v["ev"] == "agent_preview" && v["now"] == "listing the bench files"));
    // another agent selected: perf's thread is left, its entries send nothing
    t.cmd(Cmd::AgentPreview { agent: "old".into() });
    assert_eq!(t.hub.next(), json!({"cmd": "unsubscribe", "agent": "perf", "project": PROJECT}));
    assert_eq!(t.hub.next()["agent"], "old");
    t.take();
    let mut lines = perf_lines();
    lines.push((13, 1_013, "  obs: assistant: done".into()));
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    t.sync();
    assert!(!t.take().iter().any(|v| v["ev"] == "agent_preview" && v["agent"] == "perf"));
}

#[test]
fn a_history_shows_the_hubs_entries_older_and_follows_live() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 });
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 60, "project": PROJECT}));
    let mut lines = perf_lines();
    lines.push((13, 1_013, "sb you : and the cold bench?".into()));
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_history"));
    let h = t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap();
    let kinds: Vec<&str> = h["entries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["you", "agent", "tools", "from-agent", "report", "you", "card"], "from_agent is the panel's from-agent");
    let e = &h["entries"];
    assert_eq!((e[0]["pos"].as_u64(), e[0]["at_ms"].as_u64(), e[0]["text"].as_str()), (Some(1), Some(1001), Some("make it fast")));
    assert_eq!(e[1]["text"], "on it", "no thinking");
    assert_eq!((e[2]["pos"].as_u64(), e[2]["tools"]["count"].as_u64(), e[2]["text"].as_str()), (Some(4), Some(3), Some("read 1 file, ran 2 commands")), "the hub's counted summary");
    assert_eq!((e[3]["text"].as_str(), e[3]["from"].as_str()), (Some("nice\nsecond line"), Some("ambient-lead")));
    assert_eq!(e[4]["report"]["kind"], "done");
    assert_eq!(e[6]["card"], json!({"id": 9, "question": "which bench?", "options": [{"n": 1, "label": "cold"}, {"n": 2, "label": "warm"}], "answered": false}));
    assert_eq!((h["before"].clone(), h["more"].clone()), (Value::Null, json!(false)));
    // live: the hub's changed entries and the step pass to the panel
    lines.push((14, 1_014, "tool #6 bash : sb page publish notes.html --id perf-notes".into()));
    lines.push((15, 1_015, "tool #7 bash : ls".into()));
    lines.push((16, 1_016, "tool_intent #7 : listing".into()));
    for e in hub_fold(&lines).iter().filter(|e| e.pos >= 14) {
        t.hub.say(entry_ev("perf", e));
    }
    t.hub.say(json!({"ev": "typing", "project": PROJECT, "agent": "perf", "text": "listing"}));
    t.until(|o| o.iter().any(|v| v == &json!({"ev": "agent_typing", "agent": "perf", "text": "listing"})));
    let out = t.take();
    let page = out.iter().find(|v| v["ev"] == "agent_entry" && v["entry"]["kind"] == "page").unwrap();
    assert_eq!(page["entry"]["page"], json!({"id": "perf-notes", "title": "Perf notes", "v": 2, "url": "http://p/perf-notes"}));
    assert!(out.iter().any(|v| v["ev"] == "agent_entry" && v["entry"]["tools"]["items"][0]["text"] == "listing"));
    // the same entry again (a replay): nothing
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    // the card answered (the state has no card now): only its entry says so
    t.hub.say(json!({"ev": "state", "agents": agents_state("working")["agents"], "cards": []}));
    t.until(|o| o.iter().any(|v| v["ev"] == "agent_entry" && v["entry"]["card"]["answered"] == true));
    let out = t.take();
    assert_eq!(out.iter().filter(|v| v["ev"] == "agent_entry").count(), 1, "the replayed entry sends nothing: {out:?}");
    // older entries: before the first pos, none at once; else a page of the hub
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: Some(1), limit: 60 });
    assert_eq!(t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap()["entries"], json!([]));
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: Some(11), limit: 2 });
    assert_eq!(t.hub.next(), json!({"cmd": "page", "agent": "perf", "before": 11, "limit": 2, "project": PROJECT}));
    let older: Vec<Line> = perf_lines().into_iter().take(10).skip(2).collect();
    let (entries, before, more) = thread::page(&older, &Ctx { open_cards: &[], page: &|_: &str| None, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &bise_proto::thread::Attached::plain }, 2);
    t.hub.say(thread_ev("perf", &entries, before, more));
    t.until(|o| has(o, "agent_history"));
    let h = t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap();
    let kinds: Vec<&str> = h["entries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["tools", "from-agent"], "the hub's page, as the panel's");
    assert_eq!((h["before"].as_u64(), h["more"].as_bool()), (Some(4), Some(true)));
    // the panel closes: the thread is left, no more entries
    t.cmd(Cmd::AgentUnwatch { agent: "perf".into() });
    assert_eq!(t.hub.next(), json!({"cmd": "unsubscribe", "agent": "perf", "project": PROJECT}));
    lines.push((17, 1_017, "  obs: assistant: bye".into()));
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    t.sync();
    assert!(!has(&t.take(), "agent_entry"));
}

#[test]
fn a_reconnection_subscribes_again_and_sends_only_what_changed() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 });
    assert_eq!(t.hub.next()["cmd"], "subscribe");
    let mut lines = perf_lines();
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_history"));
    t.take();
    // the hub restarts: the core says hello again; its welcome resubscribes,
    // never re-sends a command
    t.hub.w.shutdown(std::net::Shutdown::Both).unwrap();
    t.hub = t.ends.recv_timeout(Duration::from_secs(3)).expect("reconnected");
    t.until(|o| o.iter().any(|v| v["ev"] == "hub" && v["up"] == true));
    t.take();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 60, "project": PROJECT}));
    lines.push((13, 1_013, "  obs: assistant: back".into()));
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_entry"));
    t.sync();
    let out = t.take();
    assert!(!has(&out, "agent_history"), "no new page for an open panel: {out:?}");
    let pos: Vec<u64> = out.iter().filter(|v| v["ev"] == "agent_entry").map(|v| v["entry"]["pos"].as_u64().unwrap()).collect();
    assert_eq!(pos, [13], "only the new entry");
}

#[test]
fn acts_send_now_queued_stop_archive_unarchive() {
    let mut t = T::new();
    t.ready();
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentSend { agent: "perf".into(), text: " look at the cold bench ".into(), queued: false });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "perf", "text": "look at the cold bench", "via": "ambient"}));
    assert!(t.take().contains(&json!({"ev": "sent", "agent": "perf", "mode": "now"})));
    // queued while it works: the hub holds it (sb-core), never the core
    t.cmd(Cmd::AgentSend { agent: "perf".into(), text: "then the warm one".into(), queued: true });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "perf", "text": "then the warm one", "via": "ambient", "queued": true}));
    let out = t.take();
    assert!(out.contains(&json!({"ev": "agent_queued", "agent": "perf", "text": "then the warm one"})));
    assert!(!out.iter().any(|v| v["ev"] == "phase"), "never main's orb");
    t.hub.say(agents_state("idle"));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::Stop { agent: "perf".into() });
    assert_eq!(t.hub.next(), json!({"op": "interrupt", "agent": "perf"}), "nothing resent at idle");
    t.cmd(Cmd::Archive { agent: "perf".into(), stop_first: true });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "/archive perf --force"}));
    t.cmd(Cmd::Archive { agent: "perf".into(), stop_first: false });
    assert_eq!(t.hub.next()["text"], "/archive perf");
    t.cmd(Cmd::Unarchive { agent: "old".into() });
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "/restore old"}));
    t.cmd(Cmd::AgentSend { agent: "nobody".into(), text: "hi".into(), queued: false });
    assert!(has(&t.take(), "error"));
}

#[test]
fn a_preview_of_an_agent_the_hub_does_not_know_comes_at_once() {
    // the QA scenes inject agents into the page only (m_7106): the core
    // still answers, empty, so the bar never waits
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::AgentPreview { agent: "cookies".into() });
    let p = t.take().into_iter().find(|v| v["ev"] == "agent_preview").expect("a preview at once");
    assert_eq!(p, json!({"ev": "agent_preview", "agent": "cookies", "now": "", "actions": [], "waiting": [], "last_report": null, "pages": []}));
    // asked again (the page asks once, the app may relaunch the page): again
    t.cmd(Cmd::AgentPreview { agent: "cookies".into() });
    assert!(has(&t.take(), "agent_preview"));
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
    assert_eq!(t.hub.next(), json!({"op": "input", "focus": "main", "text": "what is running now", "via": "capsule"}));
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
