//! Dictation (bar I): the composer's mic. The window sends
//! `dictate_start {id}`; the core opens the mic with the talk's own ports
//! (the voice role's listener job, his languages) and sends the words
//! back as `dictation {id, text, final}` for that window only (the app's
//! main routes them by id). Never a hub op: nothing reaches main or a
//! journal, and no log line carries the words.
//!
//! Each `dictation` carries the WHOLE text so far (replace, never
//! append), and exactly one `final: true` ends a dictation: stop (after
//! the listener's flush, at most [`FINAL_WAIT`]), the time limit
//! ([`DICTATE_MAX`]: no silent hot mic), fn's talk (the mic is his talk's
//! now), a listener failure (`error`, the words heard so far), cancel
//! (`cancelled`, an empty text), or a refused start (`error`: the mic is
//! busy, no mic, no voice model). One mic user at a time: a dictate_start
//! while a talk or a dictation runs is refused; fn wins over a dictation.

use super::*;

/// A dictation left running (the window lost focus, he forgot) stops by
/// itself after this long, its final out.
pub const DICTATE_MAX: Duration = Duration::from_secs(120);

/// The composer's mic: the mic, the listener, the words so far.
pub(super) struct Dictation {
    id: String,
    /// None once stopped (the mic is off)
    stream: Option<Box<dyn MicStream>>,
    blocks: Receiver<MicBlock>,
    audio: Sender<ListenMsg>,
    heard: Receiver<Heard>,
    cancel: Arc<AtomicBool>,
    text: String,
    started: Instant,
    /// stopped at
    ending: Option<Instant>,
}

impl Drop for Dictation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl Core {
    fn dictation_ev(&mut self, id: &str, text: &str, done: bool, cancelled: bool) {
        let mut v = json!({"ev": "dictation", "id": id, "text": text.trim(), "final": done});
        if cancelled {
            v["cancelled"] = json!(true);
        }
        self.emit(v);
    }

    /// The dictation's one final, with what went wrong.
    fn dictate_error(&mut self, id: &str, text: &str, error: &str) {
        self.emit(json!({"ev": "dictation", "id": id, "text": text.trim(), "final": true, "error": error}));
    }

    pub(super) fn dictate_start(&mut self, id: String, now: Instant) {
        if self.talk.is_some() || self.dictation.is_some() || self.voice_on.is_some() {
            return self.dictate_error(&id, "", "the mic is busy.");
        }
        // he dictates while main speaks: the voice stops, its text stays
        if self.stop_voice() {
            self.set_phase(Phase::Done, None);
        }
        let job = match (self.ports.listen_job)() {
            Ok(j) => j,
            Err(e) => return self.dictate_error(&id, "", &e),
        };
        let (btx, blocks) = mpsc::channel();
        let stream = match self.ports.mic.open(btx) {
            Ok(s) => s,
            Err(e) => return self.dictate_error(&id, "", &e),
        };
        let (audio, arx) = mpsc::channel();
        let (htx, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        (self.ports.listener)(&job).start(job, arx, htx, cancel.clone());
        self.dictation = Some(Dictation { id, stream: Some(stream), blocks, audio, heard, cancel, text: String::new(), started: now, ending: None });
    }

    /// The mic off now; the final comes after the listener's flush.
    pub(super) fn dictate_stop(&mut self, id: &str, now: Instant) {
        let Some(d) = self.dictation.as_mut().filter(|d| d.id == id) else { return };
        if d.ending.is_some() {
            return;
        }
        while let Ok(b) = d.blocks.try_recv() {
            let _ = d.audio.send(ListenMsg::Audio(b.pcm));
        }
        d.stream = None;
        let _ = d.audio.send(ListenMsg::Flush);
        d.ending = Some(now);
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
    }

    pub(super) fn dictate_cancel(&mut self, id: &str) {
        if self.dictation.as_ref().is_none_or(|d| d.id != id) {
            return;
        }
        self.dictation = None;
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
        self.dictation_ev(id, "", true, true);
    }

    /// fn down during a dictation: the mic is his talk's now. The
    /// dictation ends with the words it has (no flush wait).
    pub(super) fn dictate_yield(&mut self) {
        let Some(d) = self.dictation.take() else { return };
        let (id, text) = (d.id.clone(), d.text.clone());
        drop(d);
        self.dictation_ev(&id, &text, true, false);
    }

    /// The clock: the mic to the listener, its words to the window, the
    /// time limit, the final.
    pub(super) fn tick_dictation(&mut self, now: Instant) {
        if self.dictation.as_ref().is_some_and(|d| d.ending.is_none() && now.duration_since(d.started) >= DICTATE_MAX) {
            let id = self.dictation.as_ref().map(|d| d.id.clone()).unwrap_or_default();
            self.dictate_stop(&id, now);
        }
        let Some(d) = self.dictation.as_mut() else { return };
        while let Ok(b) = d.blocks.try_recv() {
            let _ = d.audio.send(ListenMsg::Audio(b.pcm));
        }
        let (mut flushed, mut failed, mut words) = (false, None, false);
        loop {
            match d.heard.try_recv() {
                Ok(Heard::Text(w)) => {
                    d.text.push_str(&w);
                    words = true;
                }
                Ok(Heard::Flushed) => {
                    flushed = true;
                    break;
                }
                Ok(Heard::Failed(e)) => {
                    failed = Some(e);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    flushed = d.ending.is_some();
                    break;
                }
            }
        }
        let level = d.stream.as_ref().map(|s| s.level());
        let late = d.ending.is_some_and(|e| now.duration_since(e) >= FINAL_WAIT);
        let over = d.ending.is_some() && (flushed || late);
        let (id, text, ending) = (d.id.clone(), d.text.clone(), d.ending.is_some());
        if let Some(v) = level {
            self.emit(json!({"ev": "level", "who": "you", "v": v}));
        }
        if let Some(e) = failed {
            // what was heard stays in his composer
            self.dictation = None;
            self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
            return self.dictate_error(&id, &text, &e);
        }
        if over {
            self.dictation = None;
            return self.dictation_ev(&id, &text, true, false);
        }
        if words && !ending {
            self.dictation_ev(&id, &text, false, false);
        }
    }
}
