//! Voice mode in the app (owner: voice-mode, the lead; plan §6): ctrl+r
//! twice enters, the keys while it is on, the run loop's pump (the
//! controller's acts → the hub), the agent's feed events → the
//! controller, the real ports.

use super::config::{self, VoiceModeConfig};
use super::turn::{Act, Jobs, Ports, SpaceKey, VoiceMode};
use crate::app::App;
use crate::feed::push_event;
use crate::wire::Ev;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// The terminal reports key releases (the kitty protocol's event types,
/// set by run.rs when the terminal confirmed them): hold space is exact.
pub static RELEASES: AtomicBool = AtomicBool::new(false);

/// The real ports: the default mic and speaker, the listener of the
/// voice role, Voxtral TTS.
fn live_ports(listen: &super::ListenJob) -> Result<Ports, String> {
    // round 2: the mic and the speaker together, the echo cancelled when
    // the platform can; no output device: the words show, nothing plays
    let (mic, speaker, aec): (Box<dyn super::Mic>, Box<dyn super::Speaker>, bool) = match super::audio::open_voice_io() {
        Ok(io) => (io.mic, io.speaker, io.aec),
        Err(_) => (Box::new(super::audio::CpalMic), Box::new(super::audio::NoSpeaker), false),
    };
    let route = super::route::output_route();
    let cut_in_by_voice = aec && barge_wanted(std::env::var("BISE_VOICE_BARGE").ok().as_deref());
    super::debug::log(|| {
        format!(
            "output {} · route {:?} · echo cancelling {} · cut in by voice on speakers {}",
            super::route::output_line(),
            route,
            if aec { "on" } else { "off" },
            cut_in_by_voice
        )
    });
    Ok(Ports {
        mic,
        vad: Box::new(super::vad::Vad::new()),
        speaker,
        listener: super::listen::listener_for(listen),
        synth: Box::new(super::tts::VoxtralTts),
        route,
        cut_in_by_voice,
    })
}

/// `BISE_VOICE_BARGE=1`: with the echo cancelled, your voice may cut the
/// agent off on speakers too (voice-echo3: off by default until the
/// unit's cancelling is proven on a real Mac; space always cuts in).
pub fn barge_wanted(env: Option<&str>) -> bool {
    matches!(env.map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("1" | "on" | "true" | "yes"))
}

// ---- BISE_VOICE_FAKE: voice mode with no mic, no sound, no network ----
//
// BISE_VOICE_FAKE=<a 16 kHz mono WAV> plays that file as the mic, paced
// (then silence); the listener hears BISE_VOICE_FAKE_HEARD (default
// below) at each flush; the voice is silence as long as the sentence
// would take, on a speaker with a real clock and no device. For the
// tmux e2e and the designer's captures.

const FAKE_HEARD: &str = "what is the state of the build";

struct FakeListener(String);

impl super::Listener for FakeListener {
    fn start(
        &self,
        _job: super::ListenJob,
        audio: std::sync::mpsc::Receiver<super::ListenMsg>,
        events: std::sync::mpsc::Sender<super::Heard>,
        _cancel: std::sync::Arc<AtomicBool>,
    ) {
        let heard = self.0.clone();
        std::thread::spawn(move || {
            while let Ok(m) = audio.recv() {
                if m == super::ListenMsg::Flush {
                    let _ = events.send(super::Heard::Text(format!(" {}", heard)));
                    let _ = events.send(super::Heard::Flushed);
                }
            }
        });
    }
}

struct FakeSynth;

impl super::Synthesizer for FakeSynth {
    fn start(
        &self,
        job: super::SayJob,
        text: String,
        events: std::sync::mpsc::Sender<super::Synth>,
        cancel: std::sync::Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            let n = (super::timing::estimate(&text, job.speed).as_secs_f64() * super::TTS_RATE as f64) as usize;
            // a soft tone, so the mouth moves; nobody hears it
            let pcm: Vec<f32> = (0..n).map(|i| 0.2 * ((i as f32) * 0.05).sin()).collect();
            for chunk in pcm.chunks(super::TTS_RATE as usize / 10) {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let _ = events.send(super::Synth::Audio(chunk.to_vec()));
            }
            let _ = events.send(super::Synth::Done);
        });
    }
}

fn fake_ports(wav: &str) -> Result<(Ports, Jobs), String> {
    let mic = super::audio::ScriptedMic::from_wav_file(std::path::Path::new(wav))?;
    let heard = bise_home::env::test_setting("BISE_VOICE_FAKE_HEARD").filter(|h| !h.trim().is_empty());
    let ports = Ports {
        mic: Box::new(mic),
        vad: Box::new(super::vad::Vad::new()),
        speaker: super::audio::silent_speaker(),
        listener: Box::new(FakeListener(heard.unwrap_or_else(|| FAKE_HEARD.into()))),
        synth: Box::new(FakeSynth),
        route: super::Route::Headphones,
        cut_in_by_voice: false,
    };
    let fake = |name: &str| super::Endpoint {
        name: name.into(),
        provider_name: "fake".into(),
        base_url: String::new(),
        model: name.into(),
        key: String::new(),
    };
    let listen = config::listen_job().unwrap_or_else(|_| super::ListenJob {
        realtime: None,
        batch: crate::voice::VoiceJob {
            name: "fake".into(),
            provider_name: "fake".into(),
            billing_url: String::new(),
            api: "openai".into(),
            base_url: String::new(),
            model: "fake".into(),
            key: String::new(),
            language: None,
            vocabulary: Vec::new(),
        },
    });
    let say = super::SayJob { api: fake("fake-tts"), voice: "fake".into(), speed: 1.0 };
    Ok((ports, Jobs { listen, say: Ok(say) }))
}

fn live_jobs(cfg: &VoiceModeConfig) -> Result<Jobs, String> {
    Ok(Jobs { listen: config::listen_job()?, say: config::say_job(cfg) })
}

/// The desktop core's voice mode (ambient core/voice_mode.rs): the same
/// ports, jobs and settings as ctrl+r twice here (BISE_VOICE_FAKE's fakes
/// when set). `fake_only`: harness A's core, which never opens a device:
/// without BISE_VOICE_FAKE it refuses.
pub(crate) fn for_core(fake_only: bool) -> Result<(Ports, Jobs, VoiceModeConfig), String> {
    let cfg = config::load();
    let (ports, jobs) = match bise_home::env::test_setting("BISE_VOICE_FAKE") {
        Some(wav) => fake_ports(&wav)?,
        None if fake_only => return Err("voice mode: no fake voice here (BISE_VOICE_FAKE)".into()),
        None => {
            let jobs = live_jobs(&cfg)?;
            (live_ports(&jobs.listen)?, jobs)
        }
    };
    Ok((ports, jobs, cfg))
}

/// A faint line in the thread in view.
fn note(app: &mut App, text: String) {
    push_event(&mut app.events, &mut app.cache, Ev::Info(format!("· {}", text)));
}

/// ctrl+r twice (or the first-time screen's "start"): voice mode with
/// the agent in view. A failure is one line in the thread.
pub(crate) fn enter(app: &mut App) {
    if app.voice_mode.is_some() {
        return;
    }
    // one listener at a time: a composer dictation goes, unwritten
    drop_dictation(app);
    let cfg = config::load();
    let now = Instant::now();
    let fake = bise_home::env::test_setting("BISE_VOICE_FAKE");
    let ready = match fake {
        Some(wav) => fake_ports(&wav),
        None => live_jobs(&cfg).and_then(|jobs| Ok((live_ports(&jobs.listen)?, jobs))),
    };
    let started = ready.and_then(|(ports, jobs)| {
        VoiceMode::start(&app.sb.focus, ports, jobs, cfg, RELEASES.load(Ordering::Relaxed), now)
    });
    match started {
        Ok(vm) => {
            app.voice_mode = Some(vm);
            note(app, format!("voice mode · {}", crate::when::mark_now(crate::when::now_ms())));
        }
        Err(e) => {
            push_event(&mut app.events, &mut app.cache, Ev::Warn(format!("voice mode: {}", e)));
        }
    }
}

/// ctrl+r twice: voice mode, after the first-time "who hears you"
/// screen when it was never shown (voice-settings' settings.rs).
pub(crate) fn request(app: &mut App) {
    if !config::load().seen_privacy && bise_home::env::test_setting("BISE_VOICE_FAKE").is_none() {
        super::settings::request(super::settings::Open::Privacy);
        return;
    }
    enter(app);
}

/// How the first-time screen closed: start (hands-free or hold-to-talk,
/// both saved by the screen) or not now.
pub(crate) fn after_settings(app: &mut App, open: super::settings::Open, out: super::settings::Out) {
    use super::settings::{Open, Out};
    if open == Open::Privacy && matches!(out, Out::Start | Out::HoldOnly) {
        enter(app);
    }
}

/// esc: voice mode ends (the mic closes when the controller drops).
pub(crate) fn leave(app: &mut App) {
    if let Some(mut vm) = app.voice_mode.take() {
        let acts = vm.leave(Instant::now());
        apply(app, acts);
    }
    // leaving never writes into the composer
    drop_dictation(app);
}

/// The composer's dictation (one ctrl+r) stops, its recording and its
/// chip dropped, nothing written (voice-echo3: one ctrl+r in voice mode
/// recorded the whole session, both voices, into the composer).
fn drop_dictation(app: &mut App) {
    if app.voice.active() {
        app.voice.cancel();
        crate::input::end_chip(app, None);
    }
}

thread_local! {
    /// The messages you said (not typed) in voice mode, this session: the
    /// thread marks them `said` (in memory; a restart shows them typed).
    static SAID: std::cell::RefCell<std::collections::HashSet<String>> = Default::default();
}

/// Your message `text` (as the hub echoes it) was said in voice mode.
pub(crate) fn was_said(text: &str) -> bool {
    SAID.with(|s| s.borrow().contains(text.trim()))
}

fn apply(app: &mut App, acts: Vec<Act>) {
    for a in acts {
        match a {
            Act::Send { agent, text } => {
                if !answer_by_voice(app, &text) {
                    SAID.with(|s| s.borrow_mut().insert(text.trim().to_string()));
                    app.sb.send(serde_json::json!({"op": "input", "focus": agent, "text": text, "voice": true}));
                    app.pending = true;
                }
            }
            Act::Interrupt { agent } => {
                app.sb.send(serde_json::json!({"op": "interrupt", "agent": agent}));
                app.interrupt_requested = true;
            }
            Act::Note(t) => note(app, t),
        }
    }
}

/// Your words as an answer to the open inbox item, when they are one
/// ("allow", "the first one"); plan §4.4: the heard line shows on the
/// item and in the pane for 1.5 s, the answer counts as the key would.
/// False: send them as words. "yes", "ok", "mm" never allow.
fn answer_by_voice(app: &mut App, text: &str) -> bool {
    use super::answers;
    let Some(q) = crate::sb::voice_question(app) else { return false };
    let now = Instant::now();
    // the first option is an approval's "allow" (once)
    let Some((i, line)) = answers::decide(text, q.approval, &q.options) else { return false };
    crate::sb::show_heard(q.id, line.clone(), now);
    if let Some(vm) = app.voice_mode.as_mut() {
        vm.show_heard(line, now);
    }
    crate::sb::answer_by_voice(app, q.id, i)
}

/// The run loop's tick: the dictation picker a lone ctrl+r asked for,
/// the agent in view, the controller's step.
pub(crate) fn pump(app: &mut App) {
    if app.voice_setup_at.is_some_and(|t| Instant::now() >= t) {
        app.voice_setup_at = None;
        crate::input::open_voice_setup(app, true);
    }
    if app.voice_mode.is_none() {
        return;
    }
    drop_dictation(app);
    let Some(vm) = app.voice_mode.as_mut() else { return };
    if vm.agent() != app.sb.focus {
        vm.set_agent(&app.sb.focus.clone());
    }
    // the agent's work since your last message, minified for the pane
    vm.set_work(work_of(&app.events));
    let acts = vm.tick(Instant::now());
    apply(app, acts);
}

/// How many work lines the pane may show (it keeps the last that fit).
const WORK_MAX: usize = 12;

/// One line, cut to `n` characters with `…`.
fn short(s: &str, n: usize) -> String {
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() <= n {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(n.saturating_sub(1)).collect::<String>())
    }
}

/// The agent's tool calls and thinking since your last message in the
/// feed in view (round 2: the minified history beside the kiss).
pub(crate) fn work_of(events: &[Ev]) -> Vec<super::Work> {
    use super::{Work, WorkKind, WorkState};
    let from = events.iter().rposition(|e| matches!(e, Ev::You(..))).map_or(0, |i| i + 1);
    let mut out: Vec<Work> = Vec::new();
    for e in &events[from..] {
        match e {
            Ev::Tool(t) => {
                let name = t.name.clone().unwrap_or_else(|| "tool".into());
                let what = t.intent.clone().or_else(|| t.args.clone()).unwrap_or_default();
                let text = if what.trim().is_empty() { name } else { format!("{} {}", name, short(&what, 48)) };
                let state = match t.state {
                    crate::wire::ToolState::Run => WorkState::Running,
                    crate::wire::ToolState::Ok => WorkState::Done,
                    crate::wire::ToolState::Fail => WorkState::Failed,
                };
                out.push(Work { kind: WorkKind::Tool, text, state });
            }
            Ev::Thinking { text, .. } => {
                out.push(Work { kind: WorkKind::Thinking, text: short(text, 48), state: WorkState::Done })
            }
            Ev::Assistant(text) => {
                out.push(Work { kind: WorkKind::Message, text: short(text, 48), state: WorkState::Done })
            }
            _ => {}
        }
    }
    let n = out.len();
    out.split_off(n.saturating_sub(WORK_MAX))
}

/// The feed events of `agent`'s line (live, not replayed): its messages
/// are said, its turn's state counts for a cut.
pub(crate) fn on_events(app: &mut App, agent: &str, evs: &[Ev]) {
    let Some(vm) = app.voice_mode.as_mut() else { return };
    let lang = config::load().language;
    for ev in evs {
        match ev {
            Ev::Assistant(text) => vm.on_message(agent, text, super::speak::speakable(text, lang.as_deref())),
            Ev::Turn => vm.on_turn(agent, true),
            Ev::TurnDone | Ev::Idle => vm.on_turn(agent, false),
            _ => {}
        }
    }
}

/// A space before the handlers (they never see releases): the floor.
/// True when voice mode took it.
pub(crate) fn space(app: &mut App, k: &KeyEvent) -> bool {
    let Some(vm) = app.voice_mode.as_mut() else { return false };
    if vm.typing() || k.code != KeyCode::Char(' ') || !(k.modifiers - KeyModifiers::SHIFT).is_empty() {
        return false;
    }
    let kind = match k.kind {
        KeyEventKind::Press => SpaceKey::Press,
        KeyEventKind::Repeat => SpaceKey::Repeat,
        KeyEventKind::Release => SpaceKey::Release,
    };
    vm.space(kind, Instant::now());
    true
}

/// The keys while voice mode is on (after help, the terminal…): esc
/// leaves, m mutes, tab types; typing, esc and tab come back to talking
/// and ⏎ sends as usual (its answer is said). Ctrl and alt keys go on
/// to their handlers (ctrl+c, the agents). True: taken.
pub(crate) fn key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(vm) = app.voice_mode.as_mut() else { return false };
    // round 2 (the user): ctrl+c leaves voice mode, like esc, never bise
    if k.code == KeyCode::Char('c') && k.modifiers == KeyModifiers::CONTROL {
        leave(app);
        return true;
    }
    // voice-echo3: ctrl+r never starts the composer's dictation under
    // the pane (a second listener, on all session long)
    if k.code == KeyCode::Char('r') && k.modifiers == KeyModifiers::CONTROL {
        return true;
    }
    let plain = (k.modifiers - KeyModifiers::SHIFT).is_empty();
    if vm.typing() {
        match k.code {
            KeyCode::Esc | KeyCode::Tab if plain => {
                vm.set_typing(false);
                true
            }
            KeyCode::Enter if plain && !app.ed.text.trim().is_empty() => {
                vm.typed_sent();
                false
            }
            _ => false,
        }
    } else {
        match k.code {
            KeyCode::Esc => {
                leave(app);
                true
            }
            KeyCode::Tab if plain => {
                vm.set_typing(true);
                true
            }
            KeyCode::Char('m') if plain => {
                vm.toggle_mute();
                true
            }
            // the composer is the pane: letters and edits go nowhere
            _ if plain => true,
            _ => false,
        }
    }
}

/// ctrl+r then ctrl+r within DOUBLE_CTRL_R: voice mode. `last` keeps
/// the first press. True: the second press, voice mode is asked for.
pub(crate) fn double_ctrl_r(last: &mut Option<Instant>, k: &KeyEvent, now: Instant) -> bool {
    if k.code != KeyCode::Char('r') || k.modifiers != KeyModifiers::CONTROL {
        return false;
    }
    if last.take().is_some_and(|t| now.duration_since(t) <= super::DOUBLE_CTRL_R) {
        return true;
    }
    *last = Some(now);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The app's own path: two ctrl+r through on_key open voice mode (the
    /// fake ports: a recorded sentence, no sound, no network), esc leaves.
    #[test]
    fn ctrl_r_twice_through_the_keys_opens_voice_mode_and_esc_leaves() {
        let wav = concat!(env!("CARGO_MANIFEST_DIR"), "/src/voicemode/testdata/sentence.wav");
        std::env::set_var("BISE_VOICE_FAKE", wav);
        let mut app = crate::sb::bench::test_app_drained();
        let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        crate::input::on_key(&mut app, &r);
        crate::input::on_key(&mut app, &r);
        let shown: Vec<String> = app
            .events
            .iter()
            .filter_map(|e| match e {
                Ev::Info(t) | Ev::Warn(t) | Ev::Err(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(app.voice_mode.is_some(), "{shown:?}");
        assert!(shown.iter().any(|t| t.starts_with("· voice mode · ")), "{shown:?}");
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.voice_mode.is_none());
    }

    /// voice-echo3: ctrl+r in voice mode never starts the composer's
    /// dictation; a dictation running when voice mode opens goes, and
    /// leaving writes nothing into the composer.
    #[test]
    fn voice_mode_has_one_listener_and_leaving_writes_nothing() {
        let wav = concat!(env!("CARGO_MANIFEST_DIR"), "/src/voicemode/testdata/sentence.wav");
        std::env::set_var("BISE_VOICE_FAKE", wav);
        let mut app = crate::sb::bench::test_app_drained();
        app.voice.enabled = true;
        let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        crate::input::on_key(&mut app, &r);
        crate::input::on_key(&mut app, &r);
        assert!(app.voice_mode.is_some());
        assert!(!app.voice.active(), "the first ctrl+r's dictation went");
        // one ctrl+r later, alone, in voice mode: no dictation
        app.ctrl_r_at = None;
        crate::input::on_key(&mut app, &r);
        assert!(!app.voice.active(), "ctrl+r in voice mode starts no dictation");
        assert!(app.ed.mark_at(crate::voice::chip::LABEL).is_none());
        // a dictation running anyway (an older path): pump drops it
        let job = crate::voice::fakes::job();
        app.voice.start(Ok(job.clone()), Instant::now()).unwrap();
        crate::attach::insert_live_chip(&mut app.ed);
        pump(&mut app);
        assert!(!app.voice.active(), "one listener at a time");
        // leaving writes nothing into the composer
        app.voice.start(Ok(job), Instant::now()).unwrap();
        crate::attach::insert_live_chip(&mut app.ed);
        crate::input::on_key(&mut app, &esc);
        assert!(app.voice_mode.is_none());
        assert!(!app.voice.active());
        assert_eq!(app.ed.text.trim(), "", "nothing written: {:?}", app.ed.text);
    }

    /// One ctrl+r (dictation off), then frames and a key: nothing hangs.
    #[test]
    fn one_ctrl_r_then_frames_and_keys_go_on() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut app = crate::sb::bench::test_app_drained();
            let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
            crate::input::on_key(&mut app, &r);
            let _ = tx.send("key");
            let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
            for _ in 0..3 {
                pump(&mut app);
                t.draw(|f| crate::run::draw_frame(&mut app, f)).unwrap();
                let _ = tx.send("frame");
            }
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
            let _ = tx.send("done");
        });
        let mut got = Vec::new();
        while let Ok(m) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            got.push(m);
        }
        assert_eq!(got.last(), Some(&"done"), "{got:?}");
    }

    #[test]
    fn ctrl_r_twice_within_400_ms_is_voice_mode_and_a_third_starts_over() {
        let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        let t = Instant::now();
        let mut last = None;
        assert!(!double_ctrl_r(&mut last, &r, t));
        assert!(double_ctrl_r(&mut last, &r, t + Duration::from_millis(300)));
        assert!(!double_ctrl_r(&mut last, &r, t + Duration::from_millis(400)));
        assert!(!double_ctrl_r(&mut last, &r, t + Duration::from_millis(900)), "too slow: a new first press");
        let x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(!double_ctrl_r(&mut last, &x, t + Duration::from_millis(1000)));
    }
}
