//! Dictation (bar I): the composer's mic. The window sends
//! `dictate_start {id}`; the core opens the mic with the talk's own ports
//! (the voice role's listener job, his languages) and sends the words
//! back as `dictation {id, text, final}` for that window only (the app's
//! main routes them by id). Never a hub op: nothing reaches main or a
//! journal, and no log line carries the words.
//!
//! Each `dictation` carries the WHOLE text so far (replace, never
//! append), and exactly one `final: true` ends a dictation: stop (after
//! the listener's flush), the quiet stop ([`QUIET_STOP`] with nothing
//! above the meter's floor: a forgotten mic; `stopped: silence`), fn's
//! talk (the mic is his talk's now), a listener failure (`error`, the
//! words heard so far), cancel (`cancelled`, an empty text), or a refused
//! start (`error`: the mic is busy, no mic, no voice model). No time
//! limit on a take he talks in (dictate-nocap): the listener hears a long
//! take in segments while he talks (voice::segments), each a partial.
//! One mic user at a time: a dictate_start while a talk or a dictation
//! holds the mic is refused; fn wins over a dictation. A stopped take no
//! longer holds the mic: its words come at the listener's flush, however
//! long that takes (the wait, [`TRANSCRIBE_TIMEOUT`], counts from the
//! listener's last sign: the stop or a segment heard; each segment heard
//! once stopped is a partial too, so the window's wait restarts), while
//! the next take records. It was 1.5 s: a batch listener sends every word
//! at the flush, after one HTTP request (two with the language guard); a
//! slower one ended the take with no words and the listener threw them
//! away, one take in two (m_22120).
//!
//! Each take's sound not yet heard (a heard segment's goes: architect
//! m_64474) is kept on disk once it stops (core/takes.rs: at the stop,
//! and again at its final), until the window says it has the take:
//! `dictate_cancel {id}` (his esc, or its words landed in a draft), or the
//! prune (7 days, 200 MB). So a take survives a chat switch, a window
//! reload and a quit (dictation-keep, architect m_49881). A final that
//! failed, timed out or came back empty says `kept`; `dictate_retry {id}`
//! transcribes the file again with a fresh listener: one final, no
//! partials. A retry for a take still transcribed is a no-op (main moved
//! its route to the window that asked: the final goes there); one with no
//! file says 'nothing to try again.'.
//!
//! core.log gets one line per step of a take (start, refused and why,
//! stop, final with its length, its time after the stop and its error,
//! kept, retry), never its words.

use super::*;
use crate::voice::{SAMPLE_RATE, TRANSCRIBE_TIMEOUT};
use bise_proto::core_ev::TakeStop;
use std::collections::VecDeque;

/// A mic left on (the window lost focus, he forgot) stops by itself
/// after this long with no sound above the meter's floor ([`loud`]): a
/// dictation and a fn talk alike, their words kept (designer m_64473,
/// architect m_64474). Never a cap on a take he talks in.
pub const QUIET_STOP: Duration = Duration::from_secs(300);

/// A retry feeds the listener the sound as the mic would: 100 ms blocks.
const RETRY_BLOCK: usize = SAMPLE_RATE as usize / 10;

/// A block with sound above the meter's floor (voice::METER_FLOOR_DB,
/// where the chip's bars are flat): a room's hiss is not.
pub(super) fn loud(block: &[i16]) -> bool {
    crate::voice::loudness(crate::voice::peak(block)) > 0.0
}

/// When the sound was last above the floor, after `block` came at `now`.
pub(super) fn sound_at(last: Instant, block: &[i16], now: Instant) -> Instant {
    if loud(block) {
        now
    } else {
        last
    }
}

/// The heard segment's sound goes: the first `n` samples not yet heard
/// (the listener's [`Heard::Segment`], in order). What stays is what a
/// retry needs, never more (architect m_64474).
fn heard(pcm: &mut VecDeque<i16>, n: usize) {
    pcm.drain(..n.min(pcm.len()));
}

fn seconds(pcm: &VecDeque<i16>) -> f32 {
    pcm.len() as f32 / SAMPLE_RATE as f32
}

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
    /// the listener's last sign of life once stopped (the stop, a
    /// segment's words): the wait for the rest counts from it, so it
    /// grows with the take's length
    progress: Instant,
    /// the mic's last block above the floor ([`loud`]): [`QUIET_STOP`]
    sound: Instant,
    /// the mic stopped by itself (the final's `stopped`)
    stopped_by: Option<TakeStop>,
    /// the sound sent to the listener and not yet heard (a heard
    /// segment's goes): kept if the final has no words
    pcm: VecDeque<i16>,
    /// the listener's line about the words (the model fell back): the
    /// final's `note`
    note: Option<String>,
}

impl Drop for Dictation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// What one tick of a take says.
#[derive(Debug, PartialEq)]
enum Step {
    Quiet,
    /// new words while the mic is on, or a segment heard once stopped
    /// (the window's wait starts again): a partial
    Words,
    /// its final, with what went wrong
    Over(Option<String>),
}

impl Dictation {
    fn new(id: String, stream: Option<Box<dyn MicStream>>, blocks: Receiver<MicBlock>, audio: Sender<ListenMsg>, heard: Receiver<Heard>, cancel: Arc<AtomicBool>, now: Instant) -> Dictation {
        Dictation {
            id,
            stream,
            blocks,
            audio,
            heard,
            cancel,
            text: String::new(),
            started: now,
            ending: None,
            progress: now,
            sound: now,
            stopped_by: None,
            pcm: VecDeque::new(),
            note: None,
        }
    }

    /// The mic's blocks to the listener, and to the take's sound.
    fn pump(&mut self, now: Instant) {
        while let Ok(b) = self.blocks.try_recv() {
            self.sound = sound_at(self.sound, &b.pcm, now);
            self.pcm.extend(b.pcm.iter().copied());
            let _ = self.audio.send(ListenMsg::Audio(b.pcm));
        }
    }

    /// The mic's blocks to the listener, its words in. A stopped take is
    /// over at the listener's flush (or failure), or after
    /// [`TRANSCRIBE_TIMEOUT`] with no sign of the listener: never before
    /// its words came, however long the take.
    fn step(&mut self, now: Instant) -> Step {
        self.pump(now);
        let mut words = false;
        loop {
            match self.heard.try_recv() {
                Ok(Heard::Text(w)) => {
                    self.text.push_str(&w);
                    self.progress = now;
                    words = true;
                }
                Ok(Heard::Segment(n)) => {
                    heard(&mut self.pcm, n);
                    self.progress = now;
                    words |= self.ending.is_some();
                }
                Ok(Heard::Flushed) if self.ending.is_some() => return Step::Over(None),
                Ok(Heard::Flushed) => {}
                Ok(Heard::Failed(e)) => return Step::Over(Some(e)),
                Ok(Heard::Note(line)) => self.note = Some(line),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) if self.ending.is_some() => return Step::Over(None),
                Err(TryRecvError::Disconnected) => break,
            }
        }
        if self.ending.is_some() && now.duration_since(self.progress) >= TRANSCRIBE_TIMEOUT {
            return Step::Over(Some("the transcription timed out.".into()));
        }
        if words {
            Step::Words
        } else {
            Step::Quiet
        }
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

    /// Refused: one line in core.log (why, never words), one final.
    fn dictate_refused(&mut self, id: &str, why: &str, error: &str) {
        eprintln!("ambient-core: dictate {id}: refused ({why})");
        self.dictate_error(id, "", error);
    }

    pub(super) fn dictate_start(&mut self, id: String, now: Instant) {
        // a fn talk still listening: his press in the window is the newer
        // act (a stuck talk, fn up missed, refused every take after it:
        // amb-tools' repro, m_22196). The talk ends as at fn up, its words
        // go; then this take has the mic. A talk past fn up holds no mic.
        if let voice::TalkRule::End(by) = voice::talk_rule(self.talk.as_ref().map(Talk::state), voice::TalkEv::DictateStart, now) {
            eprintln!("ambient-core: dictate {id}: a fn talk was listening: it ends as at fn up");
            self.talk_end_by(now, by);
        }
        let holder = if self.dictation.is_some() {
            Some("a take has the mic")
        } else if self.voice_on.is_some() {
            Some("voice mode has the mic")
        } else {
            None
        };
        if let Some(why) = holder {
            return self.dictate_refused(&id, why, "the mic is busy.");
        }
        // he dictates while main speaks: the voice stops, its text stays
        if self.stop_voice() {
            self.set_phase(Phase::Done, None);
        }
        let job = match (self.ports.listen_job)() {
            Ok(j) => j,
            Err(e) => return self.dictate_refused(&id, "no listener", &e),
        };
        let (btx, blocks) = mpsc::channel();
        let stream = match self.ports.mic.open(btx) {
            Ok(s) => s,
            Err(e) => return self.dictate_refused(&id, &format!("the mic: {e}"), &e),
        };
        let flushing = self.dictations_ending.len();
        eprintln!("ambient-core: dictate {id}: start{}", if flushing > 0 { format!(" ({flushing} take(s) still transcribing)") } else { String::new() });
        let (audio, arx) = mpsc::channel();
        let (htx, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        (self.ports.listener)(&job).start(job, arx, htx, cancel.clone());
        self.dictation = Some(Dictation::new(id, Some(stream), blocks, audio, heard, cancel, now));
    }

    /// The mic off now, free for the next take; this take's final comes
    /// at the listener's flush.
    pub(super) fn dictate_stop(&mut self, id: &str, now: Instant) {
        let Some(mut d) = self.dictation.take_if(|d| d.id == id) else {
            if !self.dictations_ending.iter().any(|d| d.id == id) {
                eprintln!("ambient-core: dictate {id}: stop with no such take: ignored");
            }
            return;
        };
        d.pump(now);
        d.stream = None;
        let _ = d.audio.send(ListenMsg::Flush);
        d.ending = Some(now);
        d.progress = now;
        eprintln!("ambient-core: dictate {id}: stop after {} ms, transcribing", now.duration_since(d.started).as_millis());
        // on disk now: a quit while it is transcribed loses nothing
        let pcm = std::mem::take(&mut d.pcm);
        self.dictate_keep(id, &pcm);
        d.pcm = pcm;
        self.dictations_ending.push(d);
        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
    }

    pub(super) fn dictate_cancel(&mut self, id: &str) {
        let held = self.dictation.take_if(|d| d.id == id).is_some();
        let before = self.dictations_ending.len();
        self.dictations_ending.retain(|d| d.id != id);
        // his cancel, or the window has the take: its sound goes
        if takes::remove(&self.ports.dictation_dir, id) {
            eprintln!("ambient-core: dictate {id}: its kept sound dropped");
        }
        if !held && self.dictations_ending.len() == before {
            return;
        }
        eprintln!("ambient-core: dictate {id}: cancelled{}", if held { "" } else { " while transcribing" });
        if held {
            self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
        }
        self.dictation_ev(id, "", true, true);
    }

    /// fn down during a dictation: the mic is his talk's now. The take
    /// holding the mic ends with the words it has (no flush wait); a
    /// stopped take still transcribing goes on.
    pub(super) fn dictate_yield(&mut self, now: Instant) {
        let Some(mut d) = self.dictation.take() else { return };
        d.pump(now);
        let (id, text, pcm) = (d.id.clone(), d.text.clone(), std::mem::take(&mut d.pcm));
        drop(d);
        eprintln!("ambient-core: dictate {id}: fn took the mic, final with {} chars", text.trim().chars().count());
        self.dictation_final_ev(&id, &text, None, pcm);
    }

    /// A take's one final, and its line in core.log (length, time after
    /// the stop, error: never its words).
    fn dictate_final(&mut self, mut d: Dictation, error: Option<String>, now: Instant) {
        let (id, text, pcm, note, by) = (d.id.clone(), d.text.clone(), std::mem::take(&mut d.pcm), d.note.take(), d.stopped_by);
        let after = d.ending.map(|e| format!(", {} ms after the stop", now.duration_since(e).as_millis())).unwrap_or_default();
        drop(d);
        let n = text.trim().chars().count();
        let err = error.as_deref().map(|e| format!(", error: {e}")).unwrap_or_default();
        let said = note.as_deref().map(|l| format!(", note: {l}")).unwrap_or_default();
        eprintln!("ambient-core: dictate {id}: final with {n} chars{after}{err}{said}");
        self.dictation_final_note(&id, &text, error.as_deref(), pcm, note.as_deref(), by);
    }

    /// The final `dictation`: with no words (an error, or nothing heard)
    /// the take's sound is kept for a retry and the final says `kept`.
    fn dictation_final_ev(&mut self, id: &str, text: &str, error: Option<&str>, pcm: VecDeque<i16>) {
        self.dictation_final_note(id, text, error, pcm, None, None)
    }

    /// [`Self::dictation_final_ev`] with the listener's `note`: the one
    /// line the window shows with the words (their model fell back).
    fn dictation_final_note(&mut self, id: &str, text: &str, error: Option<&str>, pcm: VecDeque<i16>, note: Option<&str>, by: Option<TakeStop>) {
        // on disk until the window has the take (its words may reach no
        // window: a reload): the sound not yet heard, the file of its stop
        // replaced (segments heard since went)
        let on_disk = self.dictate_keep(id, &pcm);
        let kept = (error.is_some() || text.trim().is_empty()) && on_disk;
        let mut v = json!({"ev": "dictation", "id": id, "text": text.trim(), "final": true});
        if let Some(e) = error {
            v["error"] = json!(e);
        }
        if let Some(n) = note {
            v["note"] = json!(n);
        }
        if kept {
            v["kept"] = json!(true);
        }
        if let Some(by) = by {
            v["stopped"] = json!(by);
        }
        self.emit(v);
    }

    /// Keeps a take's sound not yet heard on disk (core/takes.rs) until
    /// the window has it, replacing what its stop kept; all of it heard,
    /// its file goes. False: no sound to keep, or the disk refused
    /// (core.log says).
    fn dictate_keep(&mut self, id: &str, pcm: &VecDeque<i16>) -> bool {
        let dir = &self.ports.dictation_dir;
        if pcm.is_empty() {
            takes::remove(dir, id);
            return false;
        }
        match takes::save(dir, id, pcm) {
            Ok(true) => {
                eprintln!("ambient-core: dictate {id}: its sound kept ({:.1} s) until the window has it", seconds(pcm));
                true
            }
            Ok(false) => false,
            Err(e) => {
                eprintln!("ambient-core: dictate {id}: its sound could not be kept: {e}");
                false
            }
        }
    }

    /// `dictate_retry`: the kept sound to a fresh listener, as a stopped
    /// take (no mic, no partials): its one final comes at the flush, kept
    /// again if it has no words. A take still transcribed: nothing (its
    /// final goes to the window that asked, main moved its route).
    pub(super) fn dictate_retry(&mut self, id: String, now: Instant) {
        if self.dictation.as_ref().is_some_and(|d| d.id == id) || self.dictations_ending.iter().any(|d| d.id == id) {
            eprintln!("ambient-core: dictate {id}: retry while it is transcribed: its final comes");
            return;
        }
        let Some(pcm) = takes::load(&self.ports.dictation_dir, &id) else {
            eprintln!("ambient-core: dictate {id}: retry with nothing kept");
            return self.dictate_error(&id, "", "nothing to try again.");
        };
        let job = match (self.ports.listen_job)() {
            Ok(j) => j,
            Err(e) => {
                eprintln!("ambient-core: dictate {id}: retry refused (no listener)");
                return self.dictation_final_ev(&id, "", Some(&e), pcm);
            }
        };
        eprintln!("ambient-core: dictate {id}: retry, {:.1} s of sound", seconds(&pcm));
        let (audio, arx) = mpsc::channel();
        let (htx, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        (self.ports.listener)(&job).start(job, arx, htx, cancel.clone());
        let flat: Vec<i16> = pcm.iter().copied().collect();
        for block in flat.chunks(RETRY_BLOCK) {
            let _ = audio.send(ListenMsg::Audio(block.to_vec()));
        }
        let _ = audio.send(ListenMsg::Flush);
        // no mic: an empty, closed block channel
        let (_, blocks) = mpsc::channel();
        let mut d = Dictation::new(id, None, blocks, audio, heard, cancel, now);
        d.ending = Some(now);
        d.pcm = pcm;
        self.dictations_ending.push(d);
    }

    /// The clock: the mic to the listener, its words to the window, the
    /// quiet stop, the final.
    pub(super) fn tick_dictation(&mut self, now: Instant) {
        // the takes no window came back for: at the start, then once a day
        if self.dictations_pruned.is_none_or(|t| now.duration_since(t) >= takes::PRUNE_EVERY) {
            self.dictations_pruned = Some(now);
            let gone = takes::prune(&self.ports.dictation_dir, std::time::SystemTime::now(), takes::MAX_AGE, takes::MAX_TOTAL);
            if gone > 0 {
                eprintln!("ambient-core: dictate: {gone} old take(s) pruned");
            }
        }
        // a mic left on: nothing above the floor for QUIET_STOP
        if let Some(d) = self.dictation.as_mut() {
            d.pump(now);
        }
        if let Some(d) = self.dictation.as_mut().filter(|d| now.duration_since(d.sound) >= QUIET_STOP) {
            d.stopped_by = Some(TakeStop::Silence);
            let id = d.id.clone();
            eprintln!("ambient-core: dictate {id}: no sound in {} s, the mic stops", QUIET_STOP.as_secs());
            self.dictate_stop(&id, now);
        }
        // the take holding the mic: its level, its words, a failure
        if let Some(d) = self.dictation.as_mut() {
            let step = d.step(now);
            let level = d.stream.as_ref().map(|s| s.level());
            let (id, text) = (d.id.clone(), d.text.clone());
            if let Some(v) = level {
                self.emit(json!({"ev": "level", "who": "you", "v": v}));
            }
            match step {
                Step::Words => self.dictation_ev(&id, &text, false, false),
                Step::Over(error) => {
                    // what was heard stays in his composer
                    if let Some(d) = self.dictation.take() {
                        self.emit(json!({"ev": "level", "who": "you", "v": 0.0}));
                        self.dictate_final(d, error, now);
                    }
                }
                Step::Quiet => {}
            }
        }
        // the stopped takes: a partial per segment heard (the window's
        // wait starts again), each final at its own flush
        let mut i = 0;
        while i < self.dictations_ending.len() {
            match self.dictations_ending[i].step(now) {
                Step::Over(error) => {
                    let d = self.dictations_ending.remove(i);
                    self.dictate_final(d, error, now);
                }
                Step::Words => {
                    let (id, text) = (self.dictations_ending[i].id.clone(), self.dictations_ending[i].text.clone());
                    self.dictation_ev(&id, &text, false, false);
                    i += 1;
                }
                Step::Quiet => i += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A heard segment's sound goes from the front; never more than there is.
    #[test]
    fn a_heard_segment_drops_its_sound_from_the_front() {
        let mut pcm: VecDeque<i16> = (0..10).collect();
        heard(&mut pcm, 4);
        assert_eq!(pcm.iter().copied().collect::<Vec<_>>(), (4..10).collect::<Vec<_>>());
        heard(&mut pcm, 99);
        assert!(pcm.is_empty());
    }

    /// The quiet clock moves only on a block above the meter's floor.
    #[test]
    fn only_sound_above_the_floor_resets_the_quiet_clock() {
        let t0 = Instant::now();
        let later = t0 + Duration::from_secs(9);
        // a room's hiss (~-55 dBFS) and silence: not sound
        assert_eq!(sound_at(t0, &[58, -58, 0], later), t0);
        assert_eq!(sound_at(t0, &[0; 160], later), t0);
        // a voice (~-20 dBFS)
        assert_eq!(sound_at(t0, &[3000, -3000], later), later);
    }
}
