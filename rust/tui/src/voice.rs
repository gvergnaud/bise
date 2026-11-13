//! Speech-to-text in the composer, the Vibe CLI way (vibe/cli/voice_manager,
//! vibe/cli/transcribe, vibe/cli/audio_recorder).
//!
//! Ctrl+R records from the default microphone (16 kHz mono PCM); any
//! key stops, then the whole clip goes to the voice model in one
//! request (BISE-130: batch, the full model) and the text lands in the
//! composer. The model, the language and a vocabulary come from
//! `[voice]` in ~/.bise/config.toml (bise_catalog: Mistral's Voxtral
//! Mini Transcribe 3 by default; OpenAI, Groq, ElevenLabs, Deepgram, any
//! OpenAI-compatible server), the key from the chat keys' resolution. A
//! model the provider no longer knows (a 404) is tried once more with
//! the provider's voice pick ([`VoiceJob::fallback`]).
//! While recording, Ctrl+C or Esc cancels. Off by default: `/voice`
//! toggles it, saved in bise's prefs. The state shows as one chip in
//! the composer text at the cursor ([`chip`], BISE-222); the transcript
//! replaces it.
//!
//! Layout: pure parts first (key decisions, resampling, settings), then
//! the controller [`Voice`] driven through two ports ([`Recorder`],
//! [`Transcriber`]) so the tests run without a microphone or a network,
//! then the real adapters (cpal; [`BatchTranscriber`] over `stt` and
//! `http`). The audio stays in memory: it is never written to a file.

// Linux has no microphone yet (MicRecorder): the capture side, the
// meter and the resampler are only used by the tests there
#![cfg_attr(all(target_os = "linux", not(test)), allow(dead_code))]

use bise_proto::words::composer;
use crossterm::event::{KeyCode, KeyModifiers};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---- configuration ----

pub const SAMPLE_RATE: u32 = 16_000;
/// The request: a 5-minute clip takes Voxtral ~10-20 s; past this the
/// transcription failed. The one wait for a stopped take's words, in the
/// TUI and in the ambient core (its dictation and fn's talk, architect
/// m_22168).
pub(crate) const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(120);
/// Shorter than this, a recording has no audio blocks yet: silence then
/// means "stopped too early", not "the microphone is muted".
const MIN_SIGNAL_DURATION: Duration = Duration::from_millis(500);
/// A denied or muted microphone gives pure silence: any peak above this
/// floor means a real signal reached us.
const SILENCE_PEAK: f32 = 0.001;
/// The levels of the voice chip's meter ([`chip`]).
pub const PEAK_BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn peak_glyph(peak: f32) -> char {
    let i = (peak.clamp(0.0, 1.0) * PEAK_BLOCKS.len() as f32) as usize;
    PEAK_BLOCKS[i.min(PEAK_BLOCKS.len() - 1)]
}

/// The quietest peak the meter shows above its floor, in dBFS: a room's
/// hiss (~-55 dBFS on a laptop mic) stays flat, speech (~-35 to -10)
/// fills the bars.
pub const METER_FLOOR_DB: f32 = -50.0;

/// A peak (0..1 of full scale) as a meter level (0..1) on a decibel
/// scale: [`METER_FLOOR_DB`] and below → 0, full scale → 1. Linear,
/// speech peaks (0.02-0.2) all sat on the lowest bar (BISE-246).
pub fn loudness(peak: f32) -> f32 {
    if peak <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * peak.min(1.0).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

fn mic_access_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        composer::voice_mic_access_mac
    } else if cfg!(target_os = "windows") {
        composer::voice_mic_access_windows
    } else {
        ""
    }
}

/// Why the microphone did not open, as the user reads it (dictation and
/// voice mode).
pub(crate) fn start_error_line(e: StartError) -> String {
    match e {
        StartError::NoInputDevice => composer::voice_no_device(mic_access_hint()),
        StartError::Backend(m) => composer::voice_backend(m),
    }
}

/// A muted or refused microphone (BISE-298: macOS gives silence when the
/// terminal may not use it).
fn no_audio_detected_message() -> String {
    if cfg!(target_os = "macos") {
        composer::voice_cant_hear_mac.into()
    } else {
        composer::voice_cant_hear(mic_access_hint())
    }
}

/// The voice model has no key (BISE-298: replaces "voice transcription
/// needs an API key: …").
pub const NEEDS_KEY: &str = composer::voice_needs_key;

pub const DISABLED_MESSAGE: &str = composer::voice_disabled;
pub const OFF_HINT: &str = composer::voice_off_hint;
/// under `✓ voice is on: <model>.` once the voice picker checked it
pub const ON_HOW: &str = composer::voice_on_how;
/// esc on the voice picker while turning voice on
pub const STAYS_OFF: &str = composer::voice_stays_off;

// ---- keys ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceState {
    Idle,
    Recording,
    Flushing,
}

/// What a key does to the voice input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// not a voice key: the composer handles it
    Pass,
    Start,
    Stop,
    Cancel,
    /// eaten while the clip is transcribed (sending, a new recording)
    Swallow,
    /// Ctrl+R with voice mode off
    OffHint,
}

/// Vibe's text_area._handle_voice_key: while recording every key is
/// taken (Ctrl+C / Esc cancel, any other key stops); while the clip is
/// transcribed you keep typing (BISE-222), Ctrl+C / Esc cancel, and
/// the keys that would send the text or record again are eaten (Enter,
/// Tab, Ctrl+R); at idle, Ctrl+R starts.
pub fn key_action(state: VoiceState, enabled: bool, code: KeyCode, mods: KeyModifiers) -> KeyAction {
    let ctrl_c = code == KeyCode::Char('c') && mods == KeyModifiers::CONTROL;
    let ctrl_r = code == KeyCode::Char('r') && mods == KeyModifiers::CONTROL;
    match state {
        VoiceState::Recording | VoiceState::Flushing if ctrl_c || code == KeyCode::Esc => {
            KeyAction::Cancel
        }
        VoiceState::Recording => KeyAction::Stop,
        VoiceState::Flushing if ctrl_r || code == KeyCode::Tab || (code == KeyCode::Enter && mods == KeyModifiers::NONE) => {
            KeyAction::Swallow
        }
        VoiceState::Flushing => KeyAction::Pass,
        VoiceState::Idle if ctrl_r => {
            if enabled {
                KeyAction::Start
            } else {
                KeyAction::OffHint
            }
        }
        VoiceState::Idle => KeyAction::Pass,
    }
}

// ---- audio processing (pure) ----

/// Interleaved frames → mono (the channel average).
pub fn to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect()
}

/// A streaming linear resampler: feed blocks of any size, the phase and
/// the last sample carry over from one block to the next.
#[derive(Debug)]
pub struct Resampler {
    /// input samples per output sample
    step: f64,
    /// the position of the next output sample, in input samples,
    /// counted from the previous block's last sample (index -1): the
    /// first output is the first input sample (pos 1)
    pos: f64,
    last: Option<f32>,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Resampler {
            step: from_rate.max(1) as f64 / to_rate.max(1) as f64,
            pos: 1.0,
            last: None,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<i16> {
        self.process_f32(input).into_iter().map(to_i16).collect()
    }

    /// [`Resampler::process`] without the i16 step (voice mode's
    /// speaker: the TTS's f32 to the output device's rate).
    pub fn process_f32(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        // x[-1] = the previous block's last sample (or the first one)
        let prev = self.last.unwrap_or(input[0]);
        let at = |i: isize| if i < 0 { prev } else { input[i as usize] };
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 1);
        // pos is relative to index -1: sample k sits at pos - 1
        while self.pos - 1.0 <= (input.len() - 1) as f64 {
            let p = self.pos - 1.0;
            let i0 = p.floor() as isize;
            let frac = (p - i0 as f64) as f32;
            let a = at(i0);
            let b = if i0 < (input.len() - 1) as isize { at(i0 + 1) } else { a };
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= input.len() as f64;
        self.last = input.last().copied();
        out
    }
}

pub fn to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// The block peak, in [0, 1].
pub fn peak(samples: &[i16]) -> f32 {
    let m = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    (m as f32 / i16::MAX as f32).min(1.0)
}

/// The clip as a WAV file (PCM 16-bit mono), in memory.
pub fn wav_bytes(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes()); // the fmt chunk's size
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // bytes per second
    w.extend_from_slice(&2u16.to_le_bytes()); // bytes per frame
    w.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

/// What the transcription thread tells the controller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscribeEvent {
    /// the text (a batch model sends it once)
    Delta(String),
    /// one line to show with the words (the model fell back)
    Note(String),
    Done,
    /// the provider said no or did not answer (BISE-298): why, and the
    /// clip, kept for ctrl+r
    Failed(Failure, Vec<i16>),
}

/// Why a transcription failed, in what the user can fix (BISE-298; the
/// key check's kinds, `keycheck::Why`), and the provider's own words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailKind,
    /// one line, the key masked; "" = it said nothing useful
    pub said: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailKind {
    WrongKey,
    NoCredit,
    /// the model is unknown, or the key may not use it
    Model,
    /// no answer, or the provider's own trouble (5xx, rate limit)
    Down,
    /// an answer bise can't read
    Other,
}

impl Failure {
    /// An HTTP answer that is not a success (`keycheck::verdict`'s kinds).
    pub fn of_answer(status: u16, body: &[u8], key: &str) -> Failure {
        use crate::keycheck::Why;
        match crate::keycheck::verdict(status, body, key) {
            // a 400/422 about the request itself: the check calls it Ok
            Ok(()) => Failure { kind: FailKind::Other, said: crate::keycheck::said(body, key) },
            Err(f) => {
                let kind = match f.why {
                    Why::WrongKey => FailKind::WrongKey,
                    Why::NoCredit => FailKind::NoCredit,
                    Why::Model | Why::NoAccess => FailKind::Model,
                    Why::Unreachable(_) => FailKind::Down,
                    // verdict never says it (no call is made without a URL)
                    Why::NoUrl(_) | Why::Configuration(_) => FailKind::Other,
                };
                let said = match (f.said.is_empty(), f.why) {
                    (true, Why::Unreachable(s)) => s,
                    _ => f.said,
                };
                Failure { kind, said }
            }
        }
    }
}

/// A failed transcription as the feed says it (BISE-298, the turn
/// errors' pattern, BISE-293): bise's line, then dim ones (the
/// provider's words, the kept clip). `provider`: its name ("Mistral").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailLines {
    /// ✗, or ? (no credit: nothing is broken, it needs you)
    pub glyph: &'static str,
    pub head: String,
    pub dim: Vec<String>,
}

pub fn fail_lines(f: &Failure, provider: &str, model: &str, billing_url: &str, kept: bool) -> FailLines {
    let (glyph, head) = match f.kind {
        FailKind::WrongKey => ("✗", composer::voice_wrong_key(provider)),
        FailKind::NoCredit if billing_url.is_empty() => ("?", composer::voice_no_credit(provider)),
        FailKind::NoCredit => ("?", composer::voice_no_credit_url(provider, billing_url)),
        FailKind::Model => ("✗", composer::voice_bad_model(provider, model)),
        FailKind::Down => ("✗", composer::voice_down(provider)),
        FailKind::Other => ("✗", composer::voice_unreadable(provider)),
    };
    let mut dim = Vec::new();
    if !f.said.is_empty() {
        dim.push(f.said.clone());
    }
    if kept {
        dim.push(composer::voice_kept.into());
    }
    FailLines { glyph, head, dim }
}

// ---- settings (the `voice` preference: bise_home, prefs.json) ----

fn settings() -> bise_home::Slot {
    bise_home::Home::from_env().pref(bise_home::Pref::Voice)
}

/// Voice mode from the saved choice and the SB_VOICE override
/// (1/on/true forces on, 0/off/false forces off).
pub fn voice_enabled(saved: Option<bool>, env: Option<&str>) -> bool {
    match env.map(|s| s.trim().to_lowercase()) {
        Some(s) if matches!(s.as_str(), "1" | "on" | "true" | "yes") => return true,
        Some(s) if matches!(s.as_str(), "0" | "off" | "false" | "no") => return false,
        _ => {}
    }
    saved.unwrap_or(false)
}

/// [`voice_enabled`] from a settings text (`{"voice_mode_enabled": …}`).
#[cfg(test)]
pub fn voice_enabled_from(settings: Option<&str>, env: Option<&str>) -> bool {
    let saved = settings
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .and_then(|v| v.get("voice_mode_enabled").and_then(|b| b.as_bool()));
    voice_enabled(saved, env)
}

pub fn load_voice_enabled() -> bool {
    let env = bise_home::env::test_setting("SB_VOICE");
    voice_enabled(settings().get().and_then(|v| v.as_bool()), env.as_deref())
}

pub fn save_voice_enabled(enabled: bool) -> Result<(), String> {
    settings().set(Value::Bool(enabled)).map_err(|e| e.to_string())
}

/// The voice model's call from `[voice]` in config.toml and the key
/// (env > auth.json > the old .env files), read when a recording starts
/// so a config edit applies at once. Err: the one-line warning.
pub fn resolve_job() -> Result<VoiceJob, String> {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let home = bise_home::Home::from_env();
    let setup = bise_catalog::Setup::load(&home.config_file());
    let store = Store::read(&home.auth_file()).unwrap_or_default();
    let files = EnvFile::read_all(&home.env_files());
    let env = |k: &str| std::env::var(k).ok();
    let keys = Keys { env: &env, store: &store, files: &files };
    setup.voice_job(&keys).map_err(|e| if needs_key(&setup, &keys) { NEEDS_KEY.to_string() } else { e })
}

/// The voice model's provider takes a key and none is found.
pub fn needs_key(setup: &bise_catalog::Setup, keys: &bise_catalog::auth::Keys) -> bool {
    let r = setup.catalog.resolve_stt(&setup.voice.model);
    r.known != bise_catalog::Known::NoProvider && !r.key_env.is_empty() && keys.find(&r.provider, &r.key_env).is_none()
}

// ---- ports ----

/// What the recorder sends the transcriber.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioMsg {
    /// mono PCM at [`SAMPLE_RATE`]
    Chunk(Vec<i16>),
    /// the recording stopped: flush and end the stream
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    NoInputDevice,
    Backend(String),
}

/// A running capture; dropping it stops the microphone.
pub trait Capture {
    fn has_signal(&self) -> bool;
    /// The last live levels, oldest first ([`chip::Meter`]).
    fn levels(&self) -> [f32; chip::BARS];
}

pub trait Recorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError>;
}

/// Starts a transcription on its own thread: reads `audio` until
/// [`AudioMsg::End`], sends the events to `events`, stops when `cancel`
/// is set.
pub trait Transcriber {
    fn start(
        &self,
        job: VoiceJob,
        audio: Receiver<AudioMsg>,
        events: Sender<TranscribeEvent>,
        cancel: Arc<AtomicBool>,
    );
}

// ---- the controller (vibe/cli/voice_manager/voice_manager.py) ----

/// What the UI does after a controller step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceOutput {
    /// insert at the composer cursor
    Insert(String),
    /// the end of an utterance (one undo step)
    Utterance,
    /// a failure (Vibe's error toast)
    Error(String),
    /// the provider said no (BISE-298): bise's line and the dim ones
    Failed(FailLines),
    /// a short notice (Vibe's inline notice)
    Notice(String),
}

struct Run {
    capture: Option<Box<dyn Capture>>,
    audio: Sender<AudioMsg>,
    events: Receiver<TranscribeEvent>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    stopped: Option<Instant>,
    /// the transcriber's last sign (a segment's words): once stopped,
    /// the wait counts from the later of it and the stop
    progress: Option<Instant>,
    has_signal: bool,
    text_len: usize,
    /// the job's names, for the lines of a failure
    names: (String, String, String),
}

pub struct Voice {
    pub enabled: bool,
    recorder: Box<dyn Recorder>,
    transcriber: Box<dyn Transcriber>,
    state: VoiceState,
    run: Option<Run>,
    /// the clip of a failed transcription (BISE-298): the next ctrl+r
    /// sends it again instead of recording
    kept: Option<Vec<i16>>,
}

impl Voice {
    pub fn new(enabled: bool, recorder: Box<dyn Recorder>, transcriber: Box<dyn Transcriber>) -> Self {
        Voice { enabled, recorder, transcriber, state: VoiceState::Idle, run: None, kept: None }
    }

    /// Forget the kept clip (voice turned off).
    pub fn drop_kept(&mut self) {
        self.kept = None;
    }

    /// The real microphone and the configured voice model.
    pub fn live(enabled: bool) -> Self {
        // the tmux tests' microphone (BISE-298): a tone, no device; voice
        // mode's fake (BISE_VOICE_FAKE) never opens the real mic either
        let fake = |k: &str| bise_home::env::test_setting(k).is_some();
        if fake("SB_VOICE_FAKE_MIC") || fake("BISE_VOICE_FAKE") {
            return Voice::new(enabled, Box::new(ToneRecorder), Box::new(BatchTranscriber));
        }
        Voice::new(enabled, Box::new(MicRecorder), Box::new(BatchTranscriber))
    }

    pub fn state(&self) -> VoiceState {
        self.state
    }

    pub fn active(&self) -> bool {
        self.state != VoiceState::Idle
    }

    /// The meter's levels while recording, oldest first (else silence).
    pub fn levels(&self) -> [f32; chip::BARS] {
        self.run
            .as_ref()
            .and_then(|r| r.capture.as_ref())
            .map(|c| c.levels())
            .unwrap_or([0.0; chip::BARS])
    }

    /// The recording's length at `now`; once stopped, the clip's.
    pub fn clip_len(&self, now: Instant) -> Duration {
        self.run
            .as_ref()
            .map(|r| r.stopped.unwrap_or(now).saturating_duration_since(r.started))
            .unwrap_or_default()
    }

    /// Start recording; Err is the warning to show (no model or key,
    /// Vibe's RecordingStartError messages).
    pub fn start(&mut self, job: Result<VoiceJob, String>, now: Instant) -> Result<(), String> {
        if self.state != VoiceState::Idle {
            return Ok(());
        }
        let job = job?;
        let names = (job.provider_name.clone(), job.name.clone(), job.billing_url.clone());
        // a kept clip: sent again, no recording (BISE-298)
        if let Some(clip) = self.kept.take() {
            let (audio_tx, audio_rx) = mpsc::channel();
            let _ = audio_tx.send(AudioMsg::Chunk(clip));
            let _ = audio_tx.send(AudioMsg::End);
            let (ev_tx, ev_rx) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            self.transcriber.start(job, audio_rx, ev_tx, cancel.clone());
            self.run = Some(Run {
                capture: None,
                audio: audio_tx,
                events: ev_rx,
                cancel,
                started: now,
                stopped: Some(now),
                progress: None,
                has_signal: true,
                text_len: 0,
                names,
            });
            self.state = VoiceState::Flushing;
            return Ok(());
        }
        let (audio_tx, audio_rx) = mpsc::channel();
        let capture = self.recorder.start(SAMPLE_RATE, audio_tx.clone()).map_err(start_error_line)?;
        let (ev_tx, ev_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.transcriber.start(job, audio_rx, ev_tx, cancel.clone());
        self.run = Some(Run {
            capture: Some(capture),
            audio: audio_tx,
            events: ev_rx,
            cancel,
            started: now,
            stopped: None,
            progress: None,
            has_signal: false,
            text_len: 0,
            names,
        });
        self.state = VoiceState::Recording;
        Ok(())
    }

    /// Stop the microphone; the clip is transcribed.
    pub fn stop(&mut self, now: Instant) {
        if self.state != VoiceState::Recording {
            return;
        }
        if let Some(run) = self.run.as_mut() {
            stop_capture(run, now);
            let _ = run.audio.send(AudioMsg::End);
        }
        self.state = VoiceState::Flushing;
    }

    /// Drop the recording (the text already inserted stays).
    pub fn cancel(&mut self) {
        if let Some(run) = self.run.take() {
            run.cancel.store(true, Ordering::SeqCst);
            // dropping the capture stops the microphone
        }
        self.state = VoiceState::Idle;
    }

    /// Drain the transcription events; checks the flush timeout (from
    /// the transcriber's last sign: a long clip's segments each get
    /// their time). No time limit while he records (dictate-nocap). Call
    /// it on every UI tick.
    pub fn poll(&mut self, now: Instant) -> Vec<VoiceOutput> {
        let mut out = Vec::new();
        while let Some(run) = self.run.as_mut() {
            match run.events.try_recv() {
                Ok(TranscribeEvent::Delta(t)) => {
                    run.progress = Some(now);
                    run.text_len += t.chars().count();
                    if !t.is_empty() {
                        out.push(VoiceOutput::Insert(t));
                    }
                }
                Ok(TranscribeEvent::Note(line)) => out.push(VoiceOutput::Notice(line)),
                Ok(TranscribeEvent::Failed(f, clip)) => {
                    let (provider, model, billing) = run.names.clone();
                    self.cancel();
                    let kept = !clip.is_empty();
                    self.kept = kept.then_some(clip);
                    out.push(VoiceOutput::Failed(fail_lines(&f, &provider, &model, &billing, kept)));
                }
                Ok(TranscribeEvent::Done) | Err(TryRecvError::Disconnected) => {
                    out.extend(self.finish(now));
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match (self.state, self.run.as_ref()) {
            (VoiceState::Flushing, Some(r))
                if r.stopped.max(r.progress).is_some_and(|s| now.duration_since(s) >= TRANSCRIBE_TIMEOUT) =>
            {
                self.cancel();
                out.push(VoiceOutput::Error(
                    composer::voice_failed(composer::voice_timed_out),
                ));
            }
            _ => {}
        }
        out
    }

    fn finish(&mut self, now: Instant) -> Vec<VoiceOutput> {
        let Some(mut run) = self.run.take() else { return Vec::new() };
        stop_capture(&mut run, now);
        run.cancel.store(true, Ordering::SeqCst);
        self.state = VoiceState::Idle;
        let duration = run.stopped.unwrap_or(now).duration_since(run.started);
        if run.text_len > 0 {
            vec![VoiceOutput::Utterance]
        } else if !run.has_signal && duration >= MIN_SIGNAL_DURATION {
            vec![VoiceOutput::Error(composer::voice_failed(no_audio_detected_message()))]
        } else {
            vec![VoiceOutput::Notice(composer::voice_no_speech.into())]
        }
    }
}

fn stop_capture(run: &mut Run, now: Instant) {
    if let Some(c) = run.capture.take() {
        run.has_signal = c.has_signal();
        run.stopped = Some(now);
    }
}

// ---- the microphone (cpal: CoreAudio on macOS; none on Linux yet) ----

/// The capture level, shared with the audio callback.
#[derive(Default)]
pub(crate) struct Level {
    signal: AtomicBool,
    meter: std::sync::Mutex<chip::Meter>,
}

impl Level {
    /// One block of audio from the microphone (the audio thread): the
    /// level and the meter, then the block to the transcriber.
    pub(crate) fn block(&self, samples: Vec<i16>, audio: &Sender<AudioMsg>) {
        self.record(&samples);
        let _ = audio.send(AudioMsg::Chunk(samples));
    }

    fn record(&self, samples: &[i16]) {
        if peak(samples) > SILENCE_PEAK {
            self.signal.store(true, Ordering::Relaxed);
        }
        if let Ok(mut m) = self.meter.lock() {
            m.push(samples);
        }
    }

    pub(crate) fn levels(&self) -> [f32; chip::BARS] {
        self.meter.lock().map(|m| m.levels()).unwrap_or_default()
    }
}

#[cfg(not(target_os = "linux"))]
struct CpalCapture {
    _stream: cpal::Stream,
    level: Arc<Level>,
}

#[cfg(not(target_os = "linux"))]
impl Capture for CpalCapture {
    fn has_signal(&self) -> bool {
        self.level.signal.load(Ordering::Relaxed)
    }
    fn levels(&self) -> [f32; chip::BARS] {
        self.level.levels()
    }
}

/// The tmux tests' microphone (`SB_VOICE_FAKE_MIC=1`, BISE-298): a
/// 440 Hz tone in 100 ms blocks until the capture is dropped, so a
/// recording reaches the transcription without a device.
pub struct ToneRecorder;

struct ToneCapture {
    stop: Arc<AtomicBool>,
    level: Arc<Level>,
}

impl Drop for ToneCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Capture for ToneCapture {
    fn has_signal(&self) -> bool {
        true
    }
    fn levels(&self) -> [f32; chip::BARS] {
        self.level.levels()
    }
}

impl Recorder for ToneRecorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        let stop = Arc::new(AtomicBool::new(false));
        let level = Arc::new(Level::default());
        let (s, l) = (stop.clone(), level.clone());
        std::thread::spawn(move || {
            let n = (sample_rate / 10) as usize;
            let mut t = 0usize;
            while !s.load(Ordering::SeqCst) {
                let block: Vec<i16> = (0..n)
                    .map(|i| {
                        let x = ((t + i) as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32).sin();
                        (x * 8000.0) as i16
                    })
                    .collect();
                t += n;
                l.block(block, &audio);
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        Ok(Box::new(ToneCapture { stop, level }))
    }
}

/// The system microphone: cpal where bise links it (not Linux: no ALSA
/// dependency, tui/Cargo.toml), else a clear "not available".
pub struct MicRecorder;

#[cfg(target_os = "linux")]
impl Recorder for MicRecorder {
    fn start(&mut self, _sample_rate: u32, _audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        Err(StartError::Backend(composer::voice_no_linux.into()))
    }
}

#[cfg(not(target_os = "linux"))]
impl Recorder for MicRecorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        let level = Arc::new(Level::default());
        let l = level.clone();
        let stream = open_input(sample_rate, move |block| l.block(block, &audio))?;
        Ok(Box::new(CpalCapture { _stream: stream, level }))
    }
}

/// The default input device, playing: mono PCM at `to_rate` to
/// `on_block`, one call per device buffer (on the audio thread). Dropping
/// the stream closes the device. Shared by dictation ([`MicRecorder`])
/// and voice mode's mic (`voicemode::audio::CpalMic`).
#[cfg(not(target_os = "linux"))]
pub(crate) fn open_input(to_rate: u32, on_block: impl FnMut(Vec<i16>) + Send + 'static) -> Result<cpal::Stream, StartError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(StartError::NoInputDevice)?;
    let config = device
        .default_input_config()
        .map_err(|_| StartError::NoInputDevice)?;
    let stream_config: cpal::StreamConfig = config.config();
    let channels = stream_config.channels as usize;
    let rate = stream_config.sample_rate.0;
    let stream = match config.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, channels, rate, to_rate, on_block),
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, channels, rate, to_rate, on_block),
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, channels, rate, to_rate, on_block),
        SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, channels, rate, to_rate, on_block),
        other => Err(StartError::Backend(format!("unsupported sample format {}", other))),
    }?;
    stream.play().map_err(|e| StartError::Backend(e.to_string()))?;
    Ok(stream)
}

#[cfg(not(target_os = "linux"))]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    from_rate: u32,
    to_rate: u32,
    mut on_block: impl FnMut(Vec<i16>) + Send + 'static,
) -> Result<cpal::Stream, StartError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;
    let mut resampler = Resampler::new(from_rate, to_rate);
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let floats: Vec<f32> = data.iter().map(|s| cpal::Sample::to_sample::<f32>(*s)).collect();
                on_block(resampler.process(&to_mono(&floats, channels)));
            },
            |_err| {},
            None,
        )
        .map_err(|e| StartError::Backend(e.to_string()))
}

// ---- the batch transcription (one HTTP request per clip) ----

/// The clip in segments ([`segments`]): a long recording is heard while
/// he talks, one request per segment, in order; the rest at the end. A
/// failed segment waits for the next cut; still failing at the end, the
/// sound not yet heard is kept for ctrl+r.
pub struct BatchTranscriber;

impl Transcriber for BatchTranscriber {
    fn start(&self, job: VoiceJob, audio: Receiver<AudioMsg>, events: Sender<TranscribeEvent>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || run_segments(&job, &audio, &events, &cancel, &|req| http::send(req, TRANSCRIBE_TIMEOUT)));
    }
}

/// [`BatchTranscriber`]'s thread. `send`: the HTTP call.
pub fn run_segments(
    job: &VoiceJob,
    audio: &Receiver<AudioMsg>,
    events: &Sender<TranscribeEvent>,
    cancel: &AtomicBool,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) {
    let mut segs: segments::Segments<Words> = segments::Segments::default();
    let mut said = false;
    // the heard segments out, in order: a space between two segments' words
    let mut hand_out = |heard: Vec<(Words, Vec<i16>)>| {
        for (w, _) in heard {
            let text = if said && !w.text.is_empty() { format!(" {}", w.text) } else { w.text };
            said |= !text.is_empty();
            // an empty one too: the controller's wait starts again
            let _ = events.send(TranscribeEvent::Delta(text));
            if let Some(line) = w.fell_back.and_then(|used| fell_back_line(job, &used)) {
                let _ = events.send(TranscribeEvent::Note(line));
            }
        }
    };
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        match audio.recv_timeout(Duration::from_millis(50)) {
            Ok(AudioMsg::Chunk(c)) => {
                if segs.push(&c) {
                    let (heard, _) = segs.hear(|clip| transcribe(job, clip, cancel, send));
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }
                    hand_out(heard);
                }
            }
            Ok(AudioMsg::End) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    segs.end();
    let (heard, failed) = segs.hear(|clip| transcribe(job, clip, cancel, send));
    if cancel.load(Ordering::SeqCst) {
        return;
    }
    hand_out(heard);
    let _ = match failed {
        None => events.send(TranscribeEvent::Done),
        Some(f) => events.send(TranscribeEvent::Failed(f, segs.unheard())),
    };
}

/// A clip shorter than this is a slip of the key: no request.
const MIN_CLIP: usize = SAMPLE_RATE as usize / 5;

/// One clip → its text (trimmed; "" when there was nothing to send:
/// too short, or pure silence). `send` is the HTTP call. Err: why, in
/// the user's kinds (BISE-298).
pub fn transcribe_clip(
    job: &VoiceJob,
    samples: &[i16],
    cancel: &AtomicBool,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) -> Result<String, Failure> {
    transcribe(job, samples, cancel, send).map(|w| w.text)
}

/// A clip's words, and the model that heard them when it was not the
/// job's (the job's model is gone: [`VoiceJob::fallback`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    pub text: String,
    /// the fallback's id: the take says so in one line
    pub fell_back: Option<String>,
}

/// [`transcribe_clip`], with the fallback it took.
pub fn transcribe(
    job: &VoiceJob,
    samples: &[i16],
    cancel: &AtomicBool,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) -> Result<Words, Failure> {
    let words = |text: String| Words { text, fell_back: None };
    if samples.len() < MIN_CLIP || peak(samples) <= SILENCE_PEAK || cancel.load(Ordering::SeqCst) {
        return Ok(words(String::new()));
    }
    let wav = wav_bytes(samples, SAMPLE_RATE);
    match transcribe_wav(job, &wav, send) {
        // the provider doesn't know the model (Mistral took
        // voxtral-transcribe-3 away): one it lists, once; still no
        // model, the user's model's failure says what to do
        Err(f) if f.kind == FailKind::Model && !cancel.load(Ordering::SeqCst) => match &job.fallback {
            Some(id) => {
                let again = VoiceJob { model: id.clone(), name: format!("{}/{}", provider_of(&job.name), id), fallback: None, ..job.clone() };
                match transcribe_wav(&again, &wav, send) {
                    Err(g) if g.kind == FailKind::Model => Err(f),
                    r => r.map(|text| Words { text, fell_back: Some(id.clone()) }),
                }
            }
            None => Err(f),
        },
        r => r.map(words),
    }
}

/// The one line of a take that fell back (the job's model is gone, this
/// one heard it): once a session per model (designer m_33749), then the
/// fallback is silent until he picks another model. None: said already.
pub fn fell_back_line(job: &VoiceJob, used: &str) -> Option<String> {
    static SAID: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    if said.contains(&job.name) {
        return None;
    }
    said.push(job.name.clone());
    Some(composer::voice_fell_back(&job.provider_name, &job.model, used))
}

/// "mistral/voxtral-mini-transcribe-3" -> "mistral".
fn provider_of(name: &str) -> &str {
    name.split_once('/').map(|(p, _)| p).unwrap_or(name)
}

/// One request with the job's model: the text, or why not.
fn transcribe_wav(
    job: &VoiceJob,
    wav: &[u8],
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) -> Result<String, Failure> {
    let req = stt::request(job, wav);
    let resp = send(&req).map_err(|e| Failure { kind: FailKind::Down, said: stt::one_line(&e) })?;
    if !(200..300).contains(&resp.status) {
        return Err(Failure::of_answer(resp.status, &resp.body, &job.key));
    }
    stt::parse(&job.api, &resp)
        .map(|t| t.trim().to_string())
        .map_err(|e| Failure { kind: FailKind::Other, said: e })
}

pub use bise_catalog::voice::VoiceJob;

pub mod http;
pub(crate) mod chip;
pub mod segments;
pub mod stt;

#[cfg(test)]
pub(crate) mod fakes;
#[cfg(test)]
mod tests;
