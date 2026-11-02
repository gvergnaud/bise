//! The turn controller (owner: voice-mode, the lead; plan §4.4): who
//! talks when. Driven by the run loop's ticks, the keys, the mic blocks,
//! the listener's words, the agent's feed events and the speaker's
//! clock; tells the app what to do ([`Act`]) and what the pane shows
//! ([`PaneView`]). Pure over its ports: the tests use the fakes.

use super::config::{ListenMode, ReadAloud, VoiceModeConfig};
use super::debug;
use super::sounds::{self, Sound};
use super::{
    Heard, ListenJob, ListenMsg, Listener, Lit, LitState, Mic, MicBlock, MicStream, PaneView, Phase, Route, SayJob,
    Sentence, Speaker, Spoken, Synth, Synthesizer, UttId, Who, Word, WordState, BACKCHANNELS, BARGE_IN, BARGE_IN_ANYWAY,
    END_OF_TURN, MIC_RATE, PAUSE, TTS_RATE, WAVE_LEN, WAVE_STEP,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the controller asks of the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// your turn, said: send `text` to `agent` (the `sb` input, `voice:
    /// true`); the app first tries it as an answer to an open inbox item
    Send { agent: String, text: String },
    /// you cut in while `agent`'s turn still runs
    Interrupt { agent: String },
    /// a faint line in the thread (`· voice mode ended · 7 min · …`)
    Note(String),
}

/// A space key as the terminal reports it. Without the kitty protocol's
/// event types every report is a `Press` (repeats included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKey {
    Press,
    Repeat,
    Release,
}

/// Shorter than this, a press then release is a tap (send now).
const TAP: Duration = Duration::from_millis(250);
/// Without release events: no other press within this after the first
/// one, it was a tap (longer than the OS's usual repeat delay).
const TAP_NO_RELEASE: Duration = Duration::from_millis(450);
/// Without release events: repeats stopped this long ago, the hold ended.
const REPEAT_GAP: Duration = Duration::from_millis(160);
/// The listener's Flushed waits at most this long; then what was heard goes.
const FLUSH_WAIT: Duration = Duration::from_secs(3);
/// `you cut in` stays this long.
const CUT_SHOWN: Duration = Duration::from_millis(800);
/// `heard "…" → 1 smaller` stays this long (design §5).
const HEARD_SHOWN: Duration = Duration::from_millis(1500);
/// A failure line stays this long, then voice mode listens again.
const FAIL_SHOWN: Duration = Duration::from_secs(4);

/// Is a block speech: the VAD ([`super::vad::Vad`]), a fake in tests.
pub trait Voicing {
    fn feed(&mut self, pcm: &[i16]) -> bool;
}

impl Voicing for super::vad::Vad {
    fn feed(&mut self, pcm: &[i16]) -> bool {
        super::vad::Vad::feed(self, pcm)
    }
}

/// The ports, real or fake.
pub struct Ports {
    pub mic: Box<dyn Mic>,
    pub vad: Box<dyn Voicing>,
    pub speaker: Box<dyn Speaker>,
    pub listener: Box<dyn Listener>,
    pub synth: Box<dyn Synthesizer>,
    pub route: Route,
    /// your voice may cut the agent off on speakers too: the speaker's
    /// echo is cancelled in the mic (VoiceProcessingIO) AND that was
    /// asked for (`BISE_VOICE_BARGE=1`; voice-echo3: the unit's
    /// cancelling was never proven on a real Mac, and a leak made the
    /// agent answer itself, so on speakers the mic waits by default)
    pub cut_in_by_voice: bool,
}

/// Round 2: speech with no words yet (the batch listener has none until
/// the turn ends) cuts the agent off after this long; "mm" is shorter.
const BARGE_IN_NO_WORDS: Duration = Duration::from_millis(800);
/// Without echo cancelling, the mic stays shut this long after the
/// agent's voice ends (the speaker's buffer and the room's tail).
const ECHO_TAIL: Duration = Duration::from_millis(500);
/// A sound's tail in the room (they are soft and short).
const SOUND_TAIL: Duration = Duration::from_millis(150);
/// Speech that starts (captured) this long after the agent spoke is
/// checked for its echo, however late its words come back (voice-echo3:
/// the batch listener answers 2-4 s after the turn ends).
const ECHO_WINDOW: Duration = Duration::from_secs(3);
/// The sounds' utterance ids start here (the speeches count from 1).
pub const SOUND_UTTS: UttId = 1 << 62;
/// The echo check keeps this much of what the agent said since your
/// last turn (bytes, the end kept).
const ECHO_SAID_MAX: usize = 4000;

/// Lowercase letters and digits of each word (the echo check).
fn norm_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// `heard` is the agent's own voice coming back through the mic: most of
/// its words are words the agent just said (round 2: on speakers it
/// answered itself).
pub fn is_echo(heard: &str, said: &str) -> bool {
    let h = norm_words(heard);
    if h.is_empty() {
        return false;
    }
    let s: std::collections::HashSet<String> = norm_words(said).into_iter().collect();
    if s.is_empty() {
        return false;
    }
    let hits = h.iter().filter(|w| s.contains(*w)).count();
    hits * 10 >= h.len() * 6
}

/// The calls voice mode makes; a missing voice leaves voice mode
/// working, silent (its reason shown once).
pub struct Jobs {
    pub listen: ListenJob,
    pub say: Result<SayJob, String>,
}

/// The agent's message being said.
struct Speech {
    /// (agent, message text) for the thread's lighting
    src: Option<(String, String)>,
    spoken: Spoken,
    /// sentence i plays as utt `first + i`
    first: UttId,
    /// the next sentence to synthesize
    next: usize,
    synth: Option<(usize, Receiver<Synth>, Arc<AtomicBool>)>,
    /// per sentence: samples received, all in, words said
    samples: Vec<usize>,
    complete: Vec<bool>,
    said: Vec<usize>,
    /// per sentence, for the debug log: when the speaker started it, and
    /// its "played" line written
    playing_since: Vec<Option<Instant>>,
    logged: Vec<bool>,
    cut: bool,
    over: bool,
}

impl Speech {
    fn new(src: Option<(String, String)>, spoken: Spoken, first: UttId) -> Speech {
        let n = spoken.sentences.len();
        Speech {
            src,
            spoken,
            first,
            next: 0,
            synth: None,
            samples: vec![0; n],
            complete: vec![false; n],
            said: vec![0; n],
            playing_since: vec![None; n],
            logged: vec![false; n],
            cut: false,
            over: n == 0,
        }
    }

    fn last_utt(&self) -> UttId {
        self.first + self.spoken.sentences.len().saturating_sub(1) as UttId
    }

    fn all_said(&mut self) {
        for (i, s) in self.spoken.sentences.iter().enumerate() {
            self.said[i] = s.words.len();
        }
    }

    fn stop_synth(&mut self) {
        if let Some((_, _, cancel)) = self.synth.take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Every word with its state, in order.
    fn word_states(&self) -> Vec<(&Word, &Sentence, WordState)> {
        let mut out = Vec::new();
        for (i, s) in self.spoken.sentences.iter().enumerate() {
            for (j, w) in s.words.iter().enumerate() {
                let st = if j < self.said[i] {
                    WordState::Said
                } else if self.cut {
                    WordState::Cut
                } else {
                    WordState::ToSay
                };
                out.push((w, s, st));
            }
        }
        out
    }
}

/// The next sentence of `sp` goes to the synthesizer (one at a time).
fn start_next(synth: &dyn Synthesizer, sp: &mut Speech, job: &SayJob) {
    if sp.synth.is_some() || sp.next >= sp.spoken.sentences.len() {
        return;
    }
    let i = sp.next;
    sp.next += 1;
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    synth.start(job.clone(), sp.spoken.sentences[i].say.clone(), tx, cancel.clone());
    sp.synth = Some((i, rx, cancel));
}

fn is_backchannel(word: &str) -> bool {
    let w: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-').collect::<String>().to_lowercase();
    w.is_empty() || BACKCHANNELS.contains(&w.as_str())
}

/// Only backchannels ("mm", "ok", "right") in `text` (or nothing).
pub fn only_backchannels(text: &str) -> bool {
    text.split_whitespace().all(is_backchannel)
}

/// How loud the mic was while the agent talked (the debug log: what
/// leaks through the echo cancelling, or the room).
#[derive(Default)]
struct MicWhileSpeaking {
    blocks: u32,
    /// louder than -45 dBFS (speech level in a quiet room)
    loud: u32,
    peak: f32,
}

impl MicWhileSpeaking {
    fn add(&mut self, pcm: &[i16]) {
        let p = crate::voice::peak(pcm);
        self.blocks += 1;
        if p > 0.0056 {
            self.loud += 1;
        }
        self.peak = self.peak.max(p);
    }

    fn line(&self) -> String {
        let db = if self.peak > 0.0 { 20.0 * self.peak.log10() } else { -120.0 };
        format!("{} blocks, {} over -45 dBFS, peak {:.0} dBFS", self.blocks, self.loud, db)
    }
}

/// The first words of `text`, for the log.
fn head(text: &str) -> String {
    let w: Vec<&str> = text.split_whitespace().take(12).collect();
    w.join(" ")
}

/// The debug log's line for sentence `i` once it played: the audio it
/// got (`samples` at [`TTS_RATE`]) against the time the speaker took
/// (`took`, None when its clock never showed it); a speaker done more
/// than 150 ms early skipped audio past the queue.
fn played_line(i: usize, samples: usize, took: Option<Duration>, say: &str) -> String {
    let audio = samples as f64 / TTS_RATE as f64;
    let Some(took) = took else {
        return format!("sentence {} played · {:.2} s of audio · never on the clock · \"{}\"", i + 1, audio, head(say));
    };
    let took = took.as_secs_f64();
    let skipped = if audio - took > 0.15 { format!(" · SKIPPED {:.2} s", audio - took) } else { String::new() };
    format!("sentence {} played · {:.2} s of audio in {:.2} s{} · \"{}\"", i + 1, audio, took, skipped, head(say))
}

pub struct VoiceMode {
    agent: String,
    started: Instant,
    cfg: VoiceModeConfig,
    route: Route,
    releases: bool,

    // audio in
    _mic_stream: Box<dyn MicStream>,
    mic_rx: Receiver<MicBlock>,
    vad: Box<dyn Voicing>,
    listener: Box<dyn Listener>,
    listen_job: ListenJob,
    listen_tx: Option<Sender<ListenMsg>>,
    heard_rx: Option<Receiver<Heard>>,
    listen_cancel: Arc<AtomicBool>,

    // your turn
    heard: String,
    /// the last voiced audio's end, the start of this run of speech
    last_voice: Option<Instant>,
    speech_start: Option<Instant>,
    /// this turn had speech (else the fill never starts)
    spoke: bool,
    flushing: Option<Instant>,
    holding: bool,
    hold_start: Option<Instant>,
    last_press: Option<Instant>,
    presses: u32,
    muted: bool,
    typing: bool,

    // the agent
    speaker: Box<dyn Speaker>,
    synth: Box<dyn Synthesizer>,
    say_job: Option<SayJob>,
    /// the two sounds, synthesized once (None: the sounds row is off)
    sounds: Option<(Vec<f32>, Vec<f32>)>,
    /// a sound plays until then (the mic waits for it on speakers)
    sound_until: Option<Instant>,
    /// the sounds played (their utt ids: SOUND_UTTS + n)
    sound_utt: UttId,
    /// sent, the agent's messages until the next turn are said
    awaiting: bool,
    turn_running: bool,
    queue: VecDeque<Speech>,
    speech: Option<Speech>,
    /// the last message said, kept lit (all said, or cut) until the next turn
    last: Option<Speech>,
    next_utt: UttId,

    // shown
    cut_at: Option<Instant>,
    failed: Option<(String, Instant)>,
    heard_answer: Option<(String, Instant)>,
    /// whose words the captions show
    captions_you: bool,
    turns: u32,
    answers: u32,
    acts: Vec<Act>,
    /// the lanes' waves (one level per WAVE_STEP) and the last step
    you_wave: VecDeque<f32>,
    agent_wave: VecDeque<f32>,
    wave_at: Instant,
    /// the agent's work this turn, minified for the pane's right side
    work: Vec<super::Work>,
    /// your last turn sent, as transcribed (the pane shows it above the
    /// answer until you talk again)
    question: Option<String>,
    /// your voice may cut the agent off on speakers (Ports::cut_in_by_voice)
    cut_in_by_voice: bool,
    /// what the agent said since your last turn (the echo check) and
    /// when its voice ended (played out, or cut)
    echo_said: String,
    speech_end: Option<Instant>,
    /// this turn's speech started (captured) while the agent talked or
    /// within ECHO_WINDOW of its end: its words get the echo check
    near_voice: bool,
    /// the agent's last speech played out whole (not cut) then: the
    /// kiss's end-of-turn animation (PaneView::kiss_ms), the `back` sound
    said_whole_at: Option<Instant>,
    /// the debug log: the mic was open at the last block, and how loud
    /// it was while the agent talked
    mic_was_live: bool,
    mic_while: MicWhileSpeaking,
}

impl VoiceMode {
    /// Opens the mic and the listener for `agent` (the agent in view).
    /// `releases`: the terminal reports key releases (the kitty protocol's
    /// event types). Err: the one-line reason; voice mode stays closed.
    pub fn start(
        agent: &str,
        mut ports: Ports,
        jobs: Jobs,
        cfg: VoiceModeConfig,
        releases: bool,
        now: Instant,
    ) -> Result<VoiceMode, String> {
        let (tx, mic_rx) = mpsc::channel();
        let mic_stream = ports.mic.open(tx)?;
        let (say_job, failed) = match jobs.say {
            Ok(j) => (Some(j), None),
            Err(e) => (None, Some((e, now))),
        };
        debug::log(|| {
            format!(
                "start · agent {} · route {:?} · cut in by voice on speakers {} · listen {:?} · sounds {} · voice {}",
                agent,
                ports.route,
                ports.cut_in_by_voice,
                cfg.listen,
                cfg.sounds,
                say_job.as_ref().map_or("none".to_string(), |j| j.api.name.clone())
            )
        });
        let sounds = cfg.sounds.then(|| (sounds::pcm(Sound::Gone), sounds::pcm(Sound::Back)));
        let mut vm = VoiceMode {
            agent: agent.to_string(),
            started: now,
            cfg,
            route: ports.route,
            cut_in_by_voice: ports.cut_in_by_voice,
            echo_said: String::new(),
            speech_end: None,
            near_voice: false,
            said_whole_at: None,
            mic_was_live: true,
            mic_while: MicWhileSpeaking::default(),
            sounds,
            sound_until: None,
            sound_utt: 0,
            releases,
            _mic_stream: mic_stream,
            mic_rx,
            vad: ports.vad,
            listener: ports.listener,
            listen_job: jobs.listen,
            listen_tx: None,
            heard_rx: None,
            listen_cancel: Arc::new(AtomicBool::new(false)),
            heard: String::new(),
            last_voice: None,
            speech_start: None,
            spoke: false,
            flushing: None,
            holding: false,
            hold_start: None,
            last_press: None,
            presses: 0,
            muted: false,
            typing: false,
            speaker: ports.speaker,
            synth: ports.synth,
            say_job,
            awaiting: false,
            turn_running: false,
            queue: VecDeque::new(),
            speech: None,
            last: None,
            next_utt: 1,
            cut_at: None,
            failed,
            heard_answer: None,
            captions_you: true,
            turns: 0,
            answers: 0,
            acts: Vec::new(),
            you_wave: VecDeque::new(),
            agent_wave: VecDeque::new(),
            wave_at: now,
            work: Vec::new(),
            question: None,
        };
        vm.start_listener();
        Ok(vm)
    }

    fn start_listener(&mut self) {
        self.listen_cancel.store(true, Ordering::SeqCst);
        let (atx, arx) = mpsc::channel();
        let (htx, hrx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.listener.start(self.listen_job.clone(), arx, htx, cancel.clone());
        self.listen_tx = Some(atx);
        self.heard_rx = Some(hrx);
        self.listen_cancel = cancel;
    }

    fn listen(&mut self, msg: ListenMsg) {
        if self.listen_tx.is_none() {
            self.start_listener();
        }
        let gone = self.listen_tx.as_ref().is_some_and(|tx| tx.send(msg).is_err());
        if gone {
            self.listen_tx = None;
        }
    }

    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// The agent in view changed: voice mode talks with it now; what the
    /// other one was saying stops.
    pub fn set_agent(&mut self, agent: &str) {
        if agent != self.agent {
            self.stop_talking(false);
            self.queue.clear();
            self.last = None;
            self.awaiting = false;
            self.turn_running = false;
            self.agent = agent.to_string();
        }
    }

    // ---- inputs ----

    pub fn space(&mut self, k: SpaceKey, now: Instant) {
        if self.typing {
            return;
        }
        if self.releases {
            match k {
                SpaceKey::Press if !self.holding => {
                    self.holding = true;
                    self.hold_start = Some(now);
                    self.take_floor();
                }
                SpaceKey::Press | SpaceKey::Repeat => {}
                SpaceKey::Release => {
                    let tap = self.hold_start.is_some_and(|t| now.duration_since(t) < TAP);
                    self.holding = false;
                    self.hold_start = None;
                    if tap || self.cfg.listen == ListenMode::Hold {
                        self.send_now(now);
                    }
                }
            }
            return;
        }
        // no releases: a run of presses close together is a hold
        if k == SpaceKey::Release {
            return;
        }
        let close = self.last_press.is_some_and(|t| now.duration_since(t) < TAP_NO_RELEASE);
        if self.holding && close {
            self.presses += 1;
        } else {
            self.holding = true;
            self.hold_start = Some(now);
            self.presses = 1;
            self.take_floor();
        }
        self.last_press = Some(now);
    }

    /// Space went down: the floor is yours. Over the agent's voice, that
    /// is a cut (hold-to-talk on speakers is deliberate).
    fn take_floor(&mut self) {
        self.said_whole_at = None;
        if self.speaking() {
            self.cut_in(Instant::now(), true);
        }
    }

    /// m: the mic off or on again.
    pub fn toggle_mute(&mut self) {
        // the kiss plays on: your mic is yours, the face is the agent's
        self.muted = !self.muted;
        if self.muted {
            self.reset_turn();
            self.listen(ListenMsg::Clear);
        }
    }

    /// tab: you type (the mic waits) or back to talking.
    pub fn set_typing(&mut self, on: bool) {
        self.typing = on;
        self.said_whole_at = None;
        if on {
            self.holding = false;
            self.reset_turn();
            self.listen(ListenMsg::Clear);
        }
    }

    pub fn typing(&self) -> bool {
        self.typing
    }

    /// You typed and sent a message in voice mode: its answer is said.
    pub fn typed_sent(&mut self) {
        self.after_send();
        self.typing = false;
    }

    /// An inbox item was answered by voice: `heard "…" → 1 smaller`.
    pub fn show_heard(&mut self, line: String, now: Instant) {
        self.heard_answer = Some((line, now));
    }

    /// The agent in view's turn started or ended.
    pub fn on_turn(&mut self, agent: &str, running: bool) {
        if agent == self.agent {
            self.turn_running = running;
        }
    }

    /// A message of `agent` (whole, as the thread shows it).
    pub fn on_message(&mut self, agent: &str, text: &str, spoken: Spoken) {
        if agent != self.agent {
            return;
        }
        let say = match self.cfg.read_aloud {
            ReadAloud::Nothing => false,
            ReadAloud::All => true,
            ReadAloud::Needs => self.awaiting,
        };
        if !say || spoken.sentences.is_empty() {
            return;
        }
        let first = self.next_utt;
        self.next_utt += spoken.sentences.len() as UttId;
        self.answers += 1;
        self.queue.push_back(Speech::new(Some((agent.to_string(), text.to_string())), spoken, first));
    }

    // ---- the tick ----

    pub fn tick(&mut self, now: Instant) -> Vec<Act> {
        self.pump_mic(now);
        self.pump_heard(now);
        self.pump_speech(now);
        self.turn_taking(now);
        self.wave(now);
        if self.failed.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > FAIL_SHOWN) {
            self.failed = None;
        }
        if self.heard_answer.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > HEARD_SHOWN) {
            self.heard_answer = None;
        }
        std::mem::take(&mut self.acts)
    }

    /// One level per WAVE_STEP for each lane (a late tick fills the gap
    /// with the level now).
    fn wave(&mut self, now: Instant) {
        let you = if self.mic_live_at(now) { self._mic_stream.level() } else { 0.0 };
        let agent = self.speaker.level();
        while now.duration_since(self.wave_at) >= WAVE_STEP {
            self.wave_at += WAVE_STEP;
            for (w, l) in [(&mut self.you_wave, you), (&mut self.agent_wave, agent)] {
                w.push_back(l);
                while w.len() > WAVE_LEN {
                    w.pop_front();
                }
            }
        }
    }

    /// The agent talks (or has words queued to say).
    fn speaking(&self) -> bool {
        self.speech.as_ref().is_some_and(|s| !s.over) || !self.queue.is_empty()
    }

    /// Your voice may cut the agent off: the voice is in headphones, or
    /// the echo is cancelled and cutting in by voice on speakers was
    /// asked for. Else the mic waits while the agent talks, hands-free or
    /// not (round 2 and voice-echo3: it heard itself and answered itself);
    /// space cuts in.
    fn barge_ok(&self) -> bool {
        self.cfg.listen != ListenMode::Hold && (self.cut_in_by_voice || self.route == Route::Headphones)
    }

    /// The mic's words go to the listener now.
    fn mic_live(&self) -> bool {
        self.mic_live_at(Instant::now())
    }

    fn mic_live_at(&self, now: Instant) -> bool {
        if self.muted || self.typing {
            return false;
        }
        if self.cfg.listen == ListenMode::Hold {
            return self.holding;
        }
        if self.speaking() && !self.barge_ok() {
            return self.holding;
        }
        // the voice just ended: its tail is still in the room (`now`: the
        // block's capture time, so a block pumped late is judged by when
        // it was heard); a sound plays, or its tail
        if !self.barge_ok() && !self.holding {
            if self.speech_end.is_some_and(|t| now.saturating_duration_since(t) < ECHO_TAIL) {
                return false;
            }
            if self.sound_until.is_some_and(|t| now < t) {
                return false;
            }
        }
        true
    }

    /// `heard` is the agent's own voice (while it talks or just after).
    fn echo(&self, heard: &str, now: Instant) -> bool {
        let recent = self.near_voice
            || self.speaking()
            || self.speech_end.is_some_and(|t| now.saturating_duration_since(t) < ECHO_WINDOW);
        recent && is_echo(heard, &self.echo_said)
    }

    /// Speech captured at `at` is close to the agent's voice.
    fn near_voice_at(&self, at: Instant) -> bool {
        self.speaking() || self.speech_end.is_some_and(|t| at.saturating_duration_since(t) < ECHO_WINDOW)
    }

    /// The agent's work this turn (live.rs builds it from the feed).
    pub fn set_work(&mut self, work: Vec<super::Work>) {
        self.work = work;
    }

    fn pump_mic(&mut self, now: Instant) {
        while let Ok(b) = self.mic_rx.try_recv() {
            let speaking = self.speaking();
            if speaking {
                self.mic_while.add(&b.pcm);
            }
            // judged at the block's capture time (never later than now)
            let live = self.mic_live_at(b.at.min(now));
            if live != self.mic_was_live {
                self.mic_was_live = live;
                debug::log(|| {
                    format!("mic {} · agent speaking {} · holding {}", if live { "open" } else { "shut" }, speaking, self.holding)
                });
            }
            if !live || self.flushing.is_some() {
                continue;
            }
            let voiced = self.vad.feed(&b.pcm);
            let end = b.at + Duration::from_secs_f64(b.pcm.len() as f64 / MIC_RATE as f64);
            if voiced {
                if self.speech_start.is_none() || self.last_voice.is_some_and(|t| b.at.duration_since(t) > PAUSE) {
                    self.speech_start = Some(b.at);
                    if self.near_voice_at(b.at) {
                        self.near_voice = true;
                    }
                    debug::log(|| {
                        format!("speech starts · agent speaking {} · near its voice {}", speaking, self.near_voice)
                    });
                }
                self.last_voice = Some(end);
                self.said_whole_at = None;
                self.spoke = true;
                self.captions_you = true;
            }
            self.listen(ListenMsg::Audio(b.pcm));
        }
    }

    /// The speech: the next one starts, its synth streams to the speaker
    /// sentence by sentence, the speaker's clock lights its words.
    fn pump_speech(&mut self, now: Instant) {
        let was_talking = self.speaking();
        self.pump_speech_step();
        // its voice ended on its own (a cut sets speech_end in cut_in)
        if was_talking && !self.speaking() {
            self.speech_end = Some(now);
            // said aloud to the end: not cut, and a voice played it (not only shown)
            let whole = self.speech.as_ref().is_some_and(|s| s.over && !s.cut && s.samples.iter().any(|n| *n > 0));
            debug::log(|| format!("agent voice ends · whole {} · mic while it spoke: {}", whole, self.mic_while.line()));
            self.mic_while = MicWhileSpeaking::default();
            if whole {
                self.said_whole_at = Some(now);
                // the mic is yours again; never over your own voice
                let you_talk = self.holding || self.last_voice.is_some_and(|t| now.saturating_duration_since(t) < PAUSE);
                if !self.muted && !self.typing && !you_talk {
                    self.play(Sound::Back, now);
                }
            }
        }
    }

    /// A sound on the voice's speaker (none when the sounds row is off).
    fn play(&mut self, sound: Sound, now: Instant) {
        let Some((gone, back)) = &self.sounds else { return };
        let pcm = match sound {
            Sound::Gone => gone,
            Sound::Back => back,
        };
        // its own ids, apart from the speeches' (their clocks never see it)
        self.sound_utt += 1;
        let utt = SOUND_UTTS + self.sound_utt;
        self.speaker.push(utt, pcm);
        self.speaker.end(utt);
        self.sound_until = Some(now + sound.length() + SOUND_TAIL);
        debug::log(|| format!("sound {}", sound.name()));
    }

    fn pump_speech_step(&mut self) {
        if self.speech.as_ref().is_none_or(|s| s.over) {
            if let Some(next) = self.queue.pop_front() {
                if let Some(old) = self.speech.take() {
                    if old.src.is_some() {
                        self.last = Some(old);
                    }
                }
                // the echo check: everything said since your last turn
                for s in &next.spoken.sentences {
                    self.echo_said.push(' ');
                    self.echo_said.push_str(&s.say);
                }
                if self.echo_said.len() > ECHO_SAID_MAX {
                    let mut cut = self.echo_said.len() - ECHO_SAID_MAX;
                    while !self.echo_said.is_char_boundary(cut) {
                        cut += 1;
                    }
                    self.echo_said.drain(..cut);
                }
                debug::log(|| format!("agent voice starts · {} sentences · \"{}\"", next.spoken.sentences.len(), head(next.spoken.sentences.first().map_or("", |s| s.say.as_str()))));
                self.said_whole_at = None;
                self.speech = Some(next);
                self.captions_you = false;
            }
        }
        let speed = self.say_job.as_ref().map_or(1.0, |j| j.speed);
        let Some(sp) = self.speech.as_mut() else { return };
        if sp.over {
            return;
        }
        // no voice: the words show, nothing is said
        let Some(job) = self.say_job.clone() else {
            sp.all_said();
            sp.over = true;
            return;
        };
        start_next(self.synth.as_ref(), sp, &job);
        let mut failed = None;
        if let Some((i, rx, _)) = &sp.synth {
            let (i, utt) = (*i, sp.first + *i as UttId);
            let mut ended = false;
            loop {
                match rx.try_recv() {
                    Ok(Synth::Audio(pcm)) => {
                        sp.samples[i] += pcm.len();
                        self.speaker.push(utt, &pcm);
                    }
                    Ok(Synth::Done) | Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                    Ok(Synth::Failed(e)) => {
                        failed = Some(e);
                        ended = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if ended {
                self.speaker.end(utt);
                sp.complete[i] = true;
                sp.synth = None;
                // the next sentence's voice starts while this one plays
                if failed.is_none() {
                    start_next(self.synth.as_ref(), sp, &job);
                }
            }
        }
        if let Some(e) = failed {
            self.failed = Some((e, Instant::now()));
            sp.stop_synth();
            sp.all_said();
            sp.over = true;
            return;
        }
        // the clock: which sentence plays, how far
        if let Some((utt, played)) = self.speaker.clock() {
            if utt >= sp.first && utt <= sp.last_utt() {
                let i = (utt - sp.first) as usize;
                if sp.playing_since[i].is_none() && played > Duration::ZERO {
                    sp.playing_since[i] = Some(Instant::now().checked_sub(played).unwrap_or_else(Instant::now));
                }
                for k in 0..i {
                    sp.said[k] = sp.spoken.sentences[k].words.len();
                }
                let s = &sp.spoken.sentences[i];
                let got = Duration::from_secs_f64(sp.samples[i] as f64 / TTS_RATE as f64);
                let total = if sp.complete[i] { got } else { got.max(super::timing::estimate(&s.say, speed)) };
                let n = super::timing::said_upto(s, played, total);
                sp.said[i] = sp.said[i].max(n);
            }
        }
        // a sentence is over once all its audio played
        for i in 0..sp.spoken.sentences.len() {
            if sp.complete[i] && self.speaker.done(sp.first + i as UttId) {
                sp.said[i] = sp.spoken.sentences[i].words.len();
                if !sp.logged[i] {
                    sp.logged[i] = true;
                    // tts-lastword: a sentence played in less time than its
                    // audio lasts was skipped somewhere past the queue
                    debug::log(|| played_line(i, sp.samples[i], sp.playing_since[i].map(|t| t.elapsed()), &sp.spoken.sentences[i].say));
                }
            }
        }
        if sp.next == sp.spoken.sentences.len() && sp.synth.is_none() && self.speaker.done(sp.last_utt()) {
            sp.all_said();
            sp.over = true;
        }
    }

    fn turn_taking(&mut self, now: Instant) {
        // without releases a hold ends when the repeats stop; a lone
        // press is a tap
        if !self.releases && self.holding {
            if let Some(last) = self.last_press {
                let gap = now.duration_since(last);
                if self.presses <= 1 && gap >= TAP_NO_RELEASE {
                    self.holding = false;
                    self.send_now(now);
                } else if self.presses > 1 && gap >= REPEAT_GAP {
                    self.holding = false;
                    if self.cfg.listen == ListenMode::Hold {
                        self.send_now(now);
                    }
                }
            }
        }
        if let Some(t) = self.flushing {
            if now.duration_since(t) >= FLUSH_WAIT {
                self.finish_turn(now);
            }
            return;
        }
        // over the agent's voice: a cut, or a backchannel let go
        if self.speaking() {
            if let (Some(start), Some(last)) = (self.speech_start, self.last_voice) {
                let talking = now.saturating_duration_since(last) < PAUSE;
                let long = last.saturating_duration_since(start);
                // the batch listener has no words before the turn ends:
                // the length of the speech decides ("mm" is short)
                let none_yet = self.heard.trim().is_empty();
                let words = !only_backchannels(&self.heard) && !self.echo(&self.heard, now);
                let cut = (long >= BARGE_IN && words) || (none_yet && long >= BARGE_IN_NO_WORDS) || long >= BARGE_IN_ANYWAY;
                if talking && cut && (none_yet || !self.echo(&self.heard, now)) {
                    self.cut_in(now, false);
                } else if !talking && !self.holding {
                    // "mm", "ok": the agent goes on
                    self.reset_turn();
                    self.listen(ListenMsg::Clear);
                }
            }
            return;
        }
        if self.spoke && !self.holding && self.cfg.listen != ListenMode::Hold {
            if let Some(last) = self.last_voice {
                if now.saturating_duration_since(last) >= PAUSE + END_OF_TURN {
                    self.flush(now);
                }
            }
        }
    }

    /// space, or a release in hold-to-talk: the turn goes now.
    fn send_now(&mut self, now: Instant) {
        if self.spoke || !self.heard.trim().is_empty() {
            self.flush(now);
        }
    }

    fn flush(&mut self, now: Instant) {
        self.flushing = Some(now);
        self.said_whole_at = None;
        debug::log(|| "turn taken · waiting for its words".to_string());
        // your turn is taken: `gone` (never over the agent's voice)
        if !self.speaking() {
            self.play(Sound::Gone, now);
        }
        self.listen(ListenMsg::Flush);
        // no listener to answer (it failed): what was heard goes now
        if self.listen_tx.is_none() {
            self.finish_turn(now);
        }
    }

    /// The listener has every word: send them (nothing heard: noise).
    fn finish_turn(&mut self, now: Instant) {
        self.flushing = None;
        let text = self.heard.trim().to_string();
        // the agent's own voice through the mic: never a turn (checked
        // when the speech started near its voice, however late the
        // words came back)
        let echo = !text.is_empty() && self.echo(&text, now);
        debug::log(|| {
            format!(
                "turn ends · near its voice {} · {} · \"{}\"",
                self.near_voice,
                if text.is_empty() { "nothing heard" } else if echo { "DROPPED as its echo" } else { "sent" },
                text
            )
        });
        self.reset_turn();
        if text.is_empty() || echo {
            return;
        }
        self.turns += 1;
        self.question = Some(text.clone());
        self.acts.push(Act::Send { agent: self.agent.clone(), text: text.clone() });
        self.after_send();
    }

    fn after_send(&mut self) {
        self.awaiting = true;
        self.last = None;
        self.captions_you = true;
        self.said_whole_at = None;
        // a new exchange: the echo check starts over
        self.echo_said.clear();
    }

    fn reset_turn(&mut self) {
        self.heard.clear();
        self.spoke = false;
        self.speech_start = None;
        self.last_voice = None;
        self.near_voice = false;
    }

    /// You cut in: the voice stops, the unsaid words are cut, the agent's
    /// running turn is interrupted (not for a space tap: a skip).
    fn cut_in(&mut self, now: Instant, by_key: bool) {
        debug::log(|| format!("cut in · by {}", if by_key { "space" } else { "voice" }));
        if self.speaking() {
            // its voice ends here: the echo window and the tail count from now
            self.speech_end = Some(now);
            self.mic_while = MicWhileSpeaking::default();
        }
        self.stop_talking(true);
        self.said_whole_at = None;
        self.cut_at = Some(now);
        self.captions_you = true;
        if self.turn_running && !by_key {
            self.acts.push(Act::Interrupt { agent: self.agent.clone() });
        }
    }

    fn stop_talking(&mut self, cut: bool) {
        self.speaker.stop();
        self.sound_until = None;
        self.queue.clear();
        if let Some(mut sp) = self.speech.take() {
            sp.stop_synth();
            if !sp.over {
                sp.cut = cut;
                sp.over = true;
            }
            if sp.src.is_some() {
                self.last = Some(sp);
            }
        }
    }

    /// esc: voice mode ends. The speaker stops, the half turn is dropped,
    /// the mic closes (when `self` drops).
    pub fn leave(&mut self, now: Instant) -> Vec<Act> {
        self.stop_talking(true);
        self.listen_cancel.store(true, Ordering::SeqCst);
        self.listen_tx = None;
        let mins = (now.duration_since(self.started).as_secs() + 30) / 60;
        let n = |k: u32, one: &str, many: &str| format!("{} {}", k, if k == 1 { one } else { many });
        vec![Act::Note(format!(
            "voice mode ended · {} min · {}, {}",
            mins,
            n(self.turns, "thing said", "things said"),
            n(self.answers, "answer", "answers")
        ))]
    }

    // ---- what shows ----

    fn phase(&self, now: Instant) -> Phase {
        if let Some((line, _)) = &self.failed {
            return Phase::Failed(line.clone());
        }
        if self.typing {
            return Phase::Typing;
        }
        // voice-mute (the user): your mic is off, not the agent's face:
        // it still speaks and works (your side greys, PaneView::muted)
        if self.muted {
            if self.speaking() {
                return Phase::Speaking;
            }
            if self.awaiting && self.turn_running {
                return Phase::Working;
            }
            return Phase::Muted;
        }
        if self.cut_at.is_some_and(|t| now.duration_since(t) < CUT_SHOWN) {
            return Phase::CutIn;
        }
        if self.flushing.is_some() {
            return Phase::AboutToAnswer { fill: 1.0 };
        }
        if self.holding {
            return Phase::Holding;
        }
        if self.speaking() {
            return Phase::Speaking;
        }
        if let Some(last) = self.last_voice {
            let quiet = now.saturating_duration_since(last);
            if quiet < PAUSE {
                return Phase::Hearing;
            }
            if self.spoke {
                let fill = (quiet - PAUSE).as_secs_f32() / END_OF_TURN.as_secs_f32();
                return Phase::AboutToAnswer { fill: fill.clamp(0.0, 1.0) };
            }
        }
        if self.awaiting && self.turn_running {
            return Phase::Working;
        }
        if self.cfg.listen == ListenMode::Hold {
            return Phase::HoldToTalk;
        }
        Phase::Listening
    }

    /// The speech whose words show: the one playing, else the last one.
    fn shown_speech(&self) -> Option<&Speech> {
        self.speech.as_ref().filter(|s| !s.over || s.src.is_some()).or(self.last.as_ref())
    }

    pub fn view(&self, now: Instant) -> PaneView {
        let phase = self.phase(now);
        let question = self.question.clone().filter(|_| !self.you_talk(&phase));
        let (who, words) = if self.captions_you || self.shown_speech().is_none() {
            let mut w: Vec<(String, WordState)> =
                self.heard.split_whitespace().map(|x| (x.to_string(), WordState::Heard)).collect();
            if phase == Phase::Hearing {
                if let Some(l) = w.last_mut() {
                    l.1 = WordState::Partial;
                }
            }
            (Who::You, w)
        } else {
            let sp = self.shown_speech().expect("checked");
            let w = sp.word_states().into_iter().map(|(w, s, st)| (s.say[w.say.clone()].to_string(), st)).collect();
            (Who::Agent(self.agent.clone()), w)
        };
        PaneView {
            phase,
            agent: self.agent.clone(),
            who,
            words,
            you_level: if self.mic_live_at(now) { self._mic_stream.level() } else { 0.0 },
            agent_level: self.speaker.level(),
            you_wave: self.you_wave.iter().copied().collect(),
            agent_wave: self.agent_wave.iter().copied().collect(),
            elapsed: now.duration_since(self.started),
            route: self.route,
            heard_answer: self.heard_answer.as_ref().map(|(l, _)| l.clone()),
            work: self.work.clone(),
            kiss_ms: self.said_whole_at.map(|t| now.saturating_duration_since(t).as_millis() as u64),
            question,
            muted: self.muted,
        }
    }

    /// You talk again (your voice, a cut-in, the floor held, words not
    /// sent yet): the pane is yours, the last question steps aside.
    fn you_talk(&self, phase: &Phase) -> bool {
        self.spoke
            || self.holding
            || !self.heard.trim().is_empty()
            || matches!(phase, Phase::Hearing | Phase::AboutToAnswer { .. } | Phase::Holding | Phase::CutIn)
    }

    /// The agent's message being said (or last said), lit in the thread.
    pub fn lit(&self) -> Option<Lit> {
        let sp = [self.speech.as_ref(), self.last.as_ref()].into_iter().flatten().find(|s| s.src.is_some())?;
        let (agent, text) = sp.src.clone()?;
        let spans = sp
            .word_states()
            .into_iter()
            .filter_map(|(w, _, st)| {
                let st = match st {
                    WordState::Said => LitState::Said,
                    WordState::Cut => LitState::Cut,
                    _ => LitState::ToSay,
                };
                w.src.clone().map(|r| (r, st))
            })
            .collect();
        Some(Lit { agent, text, spans })
    }

    /// The pane moves (the loop wakes every 50 ms while voice mode is on).
    pub fn active(&self) -> bool {
        true
    }
}

impl Drop for VoiceMode {
    fn drop(&mut self) {
        self.listen_cancel.store(true, Ordering::SeqCst);
        if let Some(sp) = self.speech.as_mut() {
            sp.stop_synth();
        }
        self.speaker.stop();
    }
}

#[path = "turn_heard.rs"]
mod heard;

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
