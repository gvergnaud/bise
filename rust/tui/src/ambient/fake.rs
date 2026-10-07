//! Harness A's fake voice (ambient-lead m_7970, m_7976): the real app
//! driven by amb-tools' `qa.py real` holds fn and its words reach main
//! through the core's real talk path (talk_start, level, heard, talk_end,
//! the same `input` to main), with no mic, no speech-to-text and no
//! sound. The words come from a file (`<AMBIENT_HARNESS>/voice.txt`, or
//! `BISE_AMBIENT_FAKE_VOICE=<file>`) read and removed at talk_start, or
//! from the `fake_words` cmd. Never on in his normal run: only with one of
//! those variables AND an isolated BISE_HOME (set, and not his ~/.bise),
//! the app's own rule for harness mode.

use crate::voicemode::{Heard, ListenJob, ListenMsg, Listener, Mic, MicBlock, MicStream, SayJob, Speaker, Synth, Synthesizer, UttId, TTS_RATE};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One heard word every this long while fn is held (the rest come at
/// fn up, like a realtime listener's last words).
pub const WORD_EVERY: Duration = Duration::from_millis(150);
/// A said word's length (the core's own word clock: 290 ms per word).
const SAID_WORD: f32 = 0.29;

/// Where the fake voice's words come from, if it is on: Ok(None) off (no
/// variable), Err the reason it is refused (said in core.log). `file`:
/// the test setting BISE_AMBIENT_FAKE_VOICE (bise_home::env::test_setting);
/// `get` reads AMBIENT_HARNESS, BISE_HOME and HOME.
pub fn words_file(file: Option<String>, get: &dyn Fn(&str) -> Option<String>) -> Result<Option<PathBuf>, String> {
    let val = |k: &str| get(k).filter(|v| !v.trim().is_empty());
    let file = match (file.filter(|f| !f.trim().is_empty()), val("AMBIENT_HARNESS")) {
        (Some(f), _) => PathBuf::from(f),
        (None, Some(d)) => Path::new(&d).join("voice.txt"),
        (None, None) => return Ok(None),
    };
    let Some(bise_home) = val("BISE_HOME") else {
        return Err("fake voice refused: BISE_HOME is not set (harness A only, never his own ~/.bise)".into());
    };
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mine = canon(Path::new(&bise_home));
    // harness A's homes live under ~/.bise/gate (HOME is the isolated one
    // there, so HOME/.bise is BISE_HOME); elsewhere BISE_HOME must not be
    // HOME's own .bise
    let gate = mine.to_string_lossy().contains("/.bise/gate/");
    let his = val("HOME").map(|h| canon(&Path::new(&h).join(".bise")));
    if !gate && his.is_none_or(|h| h == mine) {
        return Err(format!("fake voice refused: BISE_HOME {} is his own ~/.bise", mine.display()));
    }
    Ok(Some(file))
}

/// The words of the next talk: set by the `fake_words` cmd, else read
/// (and removed) from the words file at talk_start.
#[derive(Clone)]
pub struct Words {
    file: PathBuf,
    next: Arc<Mutex<Option<String>>>,
}

impl Words {
    pub fn new(file: PathBuf) -> Words {
        Words { file, next: Arc::new(Mutex::new(None)) }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn set(&self, text: &str) {
        *self.next.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.to_string());
    }

    fn take(&self) -> String {
        if let Some(t) = self.next.lock().unwrap_or_else(|e| e.into_inner()).take() {
            return t;
        }
        let t = std::fs::read_to_string(&self.file).unwrap_or_default();
        let _ = std::fs::remove_file(&self.file);
        t
    }
}

/// A mic that never opens a device: a voice-like level while open.
pub struct FakeMic;

struct FakeStream(Instant);

impl MicStream for FakeStream {
    fn level(&self) -> f32 {
        let t = self.0.elapsed().as_secs_f32();
        0.35 + 0.2 * (t * 7.0).sin().abs()
    }
}

impl Mic for FakeMic {
    fn open(&mut self, _blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        Ok(Box::new(FakeStream(Instant::now())))
    }
}

/// The listener: the talk's words, one every [`WORD_EVERY`] while fn is
/// held, the rest at the flush (fn up).
pub struct FakeListener(pub Words);

impl Listener for FakeListener {
    fn start(&self, _job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>) {
        let text = self.0.take();
        std::thread::spawn(move || {
            let mut words = text.split_whitespace().map(|w| format!(" {w}")).collect::<std::collections::VecDeque<_>>();
            loop {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                match audio.recv_timeout(WORD_EVERY) {
                    Ok(ListenMsg::Flush) => {
                        let rest: String = words.drain(..).collect();
                        if !rest.is_empty() && events.send(Heard::Text(rest)).is_err() {
                            return;
                        }
                        let _ = events.send(Heard::Flushed);
                        return;
                    }
                    Ok(ListenMsg::Clear) => words.clear(),
                    Ok(ListenMsg::Audio(_)) => {}
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(w) = words.pop_front() {
                            if events.send(Heard::Text(w)).is_err() {
                                return;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        });
    }
}

/// The listener job it never uses (no network): the core asks for one.
pub fn listen_job() -> ListenJob {
    ListenJob {
        realtime: None,
        batch: crate::voice::VoiceJob {
            name: "fake/voice".into(),
            provider_name: "fake".into(),
            billing_url: String::new(),
            api: "openai".into(),
            base_url: "http://127.0.0.1:9".into(),
            model: "fake".into(),
            key: String::new(),
            language: None,
            vocabulary: Vec::new(),
        },
    }
}

pub fn say_job() -> SayJob {
    SayJob {
        api: crate::voicemode::Endpoint {
            name: "fake/tts".into(),
            provider_name: "fake".into(),
            base_url: "http://127.0.0.1:9".into(),
            model: "fake".into(),
            key: String::new(),
        },
        voice: "fake".into(),
        speed: 1.0,
    }
}

/// Main's answer "said": silence as long as its words, no network.
pub struct SilentSynth;

impl Synthesizer for SilentSynth {
    fn start(&self, _job: SayJob, text: String, events: Sender<Synth>, cancel: Arc<AtomicBool>) {
        let n = text.split_whitespace().count().max(1) as f32;
        let samples = (n * SAID_WORD * TTS_RATE as f32) as usize;
        if !cancel.load(Ordering::SeqCst) {
            let _ = events.send(Synth::Audio(vec![0.0; samples]));
            let _ = events.send(Synth::Done);
        }
    }
}

/// A speaker that plays nothing on time: its clock runs on the wall
/// clock, utterance after utterance.
#[derive(Default)]
pub struct SilentSpeaker {
    /// (utt, samples, all pushed)
    queue: std::collections::VecDeque<(UttId, usize, bool)>,
    /// when the head utterance started
    head_at: Option<Instant>,
    played: std::collections::HashSet<UttId>,
}

impl SilentSpeaker {
    fn len(samples: usize) -> Duration {
        Duration::from_secs_f64(samples as f64 / TTS_RATE as f64)
    }

    /// Played utterances leave the queue.
    fn advance(&mut self) {
        let now = Instant::now();
        while let Some(&(utt, n, ended)) = self.queue.front() {
            let at = *self.head_at.get_or_insert(now);
            let len = Self::len(n);
            if !ended || now.duration_since(at) < len {
                return;
            }
            self.queue.pop_front();
            self.played.insert(utt);
            self.head_at = self.queue.front().map(|_| at + len);
        }
    }
}

impl Speaker for SilentSpeaker {
    fn push(&mut self, utt: UttId, pcm: &[f32]) {
        match self.queue.iter_mut().find(|q| q.0 == utt) {
            Some(q) => q.1 += pcm.len(),
            None => self.queue.push_back((utt, pcm.len(), false)),
        }
    }
    fn end(&mut self, utt: UttId) {
        match self.queue.iter_mut().find(|q| q.0 == utt) {
            Some(q) => q.2 = true,
            None => {
                self.played.insert(utt);
            }
        }
    }
    fn stop(&mut self) {
        for (u, _, _) in self.queue.drain(..) {
            self.played.insert(u);
        }
        self.head_at = None;
    }
    fn clock(&self) -> Option<(UttId, Duration)> {
        let (utt, n, _) = *self.queue.front()?;
        let at = self.head_at.unwrap_or_else(Instant::now);
        Some((utt, Instant::now().duration_since(at).min(Self::len(n))))
    }
    fn done(&self, utt: UttId) -> bool {
        // the queue moves on in FakeSpeaker's `with`, before each call
        self.played.contains(&utt)
    }
    fn level(&self) -> f32 {
        if self.queue.is_empty() {
            0.0
        } else {
            0.3
        }
    }
}

/// [`SilentSpeaker`] behind a lock, so its queue can move on at each
/// look (the trait's reads take `&self`).
pub struct FakeSpeaker(Mutex<SilentSpeaker>);

impl FakeSpeaker {
    pub fn new() -> FakeSpeaker {
        FakeSpeaker(Mutex::new(SilentSpeaker::default()))
    }

    fn with<T>(&self, f: impl FnOnce(&mut SilentSpeaker) -> T) -> T {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        s.advance();
        f(&mut s)
    }
}

impl Default for FakeSpeaker {
    fn default() -> Self {
        Self::new()
    }
}

impl Speaker for FakeSpeaker {
    fn push(&mut self, utt: UttId, pcm: &[f32]) {
        self.with(|s| s.push(utt, pcm))
    }
    fn end(&mut self, utt: UttId) {
        self.with(|s| s.end(utt))
    }
    fn stop(&mut self) {
        self.with(|s| s.stop())
    }
    fn clock(&self) -> Option<(UttId, Duration)> {
        self.with(|s| s.clock())
    }
    fn done(&self, utt: UttId) -> bool {
        self.with(|s| s.done(utt))
    }
    fn level(&self) -> f32 {
        self.with(|s| s.level())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn the_fake_voice_is_never_on_in_his_normal_run() {
        // no variable: off
        assert_eq!(words_file(None, &env(&[("HOME", "/Users/g")])), Ok(None));
        assert_eq!(words_file(Some(" ".into()), &env(&[("HOME", "/Users/g")])), Ok(None));
        // harness A: its home under ~/.bise/gate, HOME the isolated one
        let h = "/Users/g/.bise/gate/ra-amb-tools/h";
        let gate = [("AMBIENT_HARNESS", "/tmp/hx"), ("BISE_HOME", "/Users/g/.bise/gate/ra-amb-tools/h/.bise"), ("HOME", h)];
        assert_eq!(words_file(None, &env(&gate)), Ok(Some(PathBuf::from("/tmp/hx/voice.txt"))));
        // the test setting's file wins
        let other = [("BISE_HOME", "/tmp/bh"), ("HOME", "/Users/g")];
        assert_eq!(words_file(Some("/tmp/w.txt".into()), &env(&other)), Ok(Some(PathBuf::from("/tmp/w.txt"))));
        // his own ~/.bise, or none: refused, said why
        assert!(words_file(None, &env(&[("AMBIENT_HARNESS", "/tmp/hx"), ("HOME", "/Users/g")])).is_err());
        assert!(words_file(None, &env(&[("AMBIENT_HARNESS", "/tmp/hx"), ("BISE_HOME", "/Users/g/.bise"), ("HOME", "/Users/g")])).is_err());
        assert!(words_file(None, &env(&[("AMBIENT_HARNESS", "/tmp/hx"), ("BISE_HOME", "/tmp/bh")])).is_err());
    }

    #[test]
    fn the_silent_speaker_plays_on_the_wall_clock() {
        let mut s = FakeSpeaker::new();
        s.push(1, &vec![0.0; TTS_RATE as usize / 20]);
        s.end(1);
        s.push(2, &[0.0; 10]);
        assert_eq!(s.clock().map(|c| c.0), Some(1));
        assert!(!s.done(1));
        std::thread::sleep(Duration::from_millis(70));
        assert!(s.done(1), "50 ms of audio played");
        assert_eq!(s.clock().map(|c| c.0), Some(2));
        s.stop();
        assert!(s.done(2) && s.clock().is_none() && s.level() == 0.0);
    }
}
