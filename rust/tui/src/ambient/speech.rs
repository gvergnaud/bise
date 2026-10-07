//! Main's message said aloud (docs/ambient-app.md §3 "Main answers"):
//! [`speakable`] cuts it into sentences, the synthesizer says them one
//! after the other (the next one's voice is made while this one plays),
//! the speaker plays them, and its clock tells which word of the message
//! is said ([`timing::said_upto`], as voice mode's controller does).
//!
//! The words are counted on the message as the core sends it (`main`'s
//! `text`): word `i` is the i-th run of non-whitespace of that text.

use crate::voicemode::speak::speakable;
use crate::voicemode::{timing, SayJob, Speaker, Spoken, Synth, Synthesizer, UttId, TTS_RATE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

/// What a step of the speech says to the core.
#[derive(Debug, PartialEq)]
pub enum Step {
    /// still playing; `word`: the message's word said last, when it moved
    Playing { word: Option<usize> },
    /// every sentence played out; `word` as in Playing (the last one)
    Over { word: Option<usize> },
    /// the voice failed (the one-line reason); the speech stopped
    Failed(String),
}

pub struct Speech {
    pub turn: u64,
    job: SayJob,
    spoken: Spoken,
    /// for each sentence, each word's index in the message (None: a word
    /// the message does not have, "it's on screen.")
    map: Vec<Vec<Option<usize>>>,
    /// the utterance id of sentence 0 (sentence i is `first + i`)
    first: UttId,
    /// the next sentence whose voice is not asked yet
    next: usize,
    synth: Option<(usize, Receiver<Synth>, Arc<AtomicBool>)>,
    samples: Vec<usize>,
    complete: Vec<bool>,
    said: Vec<usize>,
    lit: Option<usize>,
}

/// The byte start of each run of non-whitespace of `text`.
fn word_starts(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut prev_space = true;
    for (i, c) in text.char_indices() {
        let space = c.is_whitespace();
        if !space && prev_space {
            out.push(i);
        }
        prev_space = space;
    }
    out
}

impl Speech {
    /// `text` of turn `turn`, said with `job` from utterance `first` on.
    /// None: nothing in it to say.
    pub fn new(turn: u64, text: &str, language: Option<&str>, job: SayJob, first: UttId) -> Option<Speech> {
        let spoken = speakable(text, language);
        if spoken.sentences.is_empty() {
            return None;
        }
        let starts = word_starts(text);
        let map = spoken
            .sentences
            .iter()
            .map(|s| {
                s.words
                    .iter()
                    .map(|w| w.src.as_ref().map(|r| starts.partition_point(|&b| b <= r.start).saturating_sub(1)))
                    .collect()
            })
            .collect();
        let n = spoken.sentences.len();
        Some(Speech {
            turn,
            job,
            spoken,
            map,
            first,
            next: 0,
            synth: None,
            samples: vec![0; n],
            complete: vec![false; n],
            said: vec![0; n],
            lit: None,
        })
    }

    /// The utterance ids it uses: the next speech starts after them.
    pub fn utts(&self) -> UttId {
        self.spoken.sentences.len() as UttId
    }

    fn start_next(&mut self, synth: &dyn Synthesizer) {
        if self.synth.is_some() || self.next >= self.spoken.sentences.len() {
            return;
        }
        let i = self.next;
        self.next += 1;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        synth.start(self.job.clone(), self.spoken.sentences[i].say.clone(), tx, cancel.clone());
        self.synth = Some((i, rx, cancel));
    }

    /// One step: the voice made so far goes to the speaker, the clock
    /// lights the words.
    pub fn step(&mut self, synth: &dyn Synthesizer, speaker: &mut dyn Speaker) -> Step {
        self.start_next(synth);
        if let Some((i, rx, _)) = &self.synth {
            let (i, utt) = (*i, self.first + *i as UttId);
            let mut ended = false;
            loop {
                match rx.try_recv() {
                    Ok(Synth::Audio(pcm)) => {
                        self.samples[i] += pcm.len();
                        speaker.push(utt, &pcm);
                    }
                    Ok(Synth::Done) | Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                    Ok(Synth::Failed(e)) => {
                        self.stop(speaker);
                        return Step::Failed(e);
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if ended {
                speaker.end(utt);
                self.complete[i] = true;
                self.synth = None;
                self.start_next(synth);
            }
        }
        if let Some((utt, played)) = speaker.clock() {
            let last = self.first + self.utts() - 1;
            if utt >= self.first && utt <= last {
                let i = (utt - self.first) as usize;
                for k in 0..i {
                    self.said[k] = self.spoken.sentences[k].words.len();
                }
                let s = &self.spoken.sentences[i];
                let got = Duration::from_secs_f64(self.samples[i] as f64 / TTS_RATE as f64);
                let total = if self.complete[i] { got } else { got.max(timing::estimate(&s.say, self.job.speed)) };
                self.said[i] = self.said[i].max(timing::said_upto(s, played, total));
            }
        }
        for i in 0..self.said.len() {
            if self.complete[i] && speaker.done(self.first + i as UttId) {
                self.said[i] = self.spoken.sentences[i].words.len();
            }
        }
        let word = self.lit_word();
        let moved = word.is_some() && word != self.lit;
        if moved {
            self.lit = word;
        }
        let over = self.next == self.spoken.sentences.len()
            && self.synth.is_none()
            && self.complete.iter().all(|c| *c)
            && speaker.done(self.first + self.utts() - 1);
        let word = if moved { word } else { None };
        if over {
            return Step::Over { word };
        }
        Step::Playing { word }
    }

    /// The message's last word said: the highest index among the said
    /// words that the message has.
    fn lit_word(&self) -> Option<usize> {
        self.map
            .iter()
            .zip(&self.said)
            .flat_map(|(m, &n)| m.iter().take(n))
            .filter_map(|w| *w)
            .max()
    }

    /// Hush, cut in, a new speech: the voice stops now.
    pub fn stop(&mut self, speaker: &mut dyn Speaker) {
        if let Some((_, _, cancel)) = self.synth.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.next = self.spoken.sentences.len();
        speaker.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_starts_counts_runs_of_non_whitespace() {
        assert_eq!(word_starts("  the build\nis  green."), vec![2, 6, 12, 16]);
        assert!(word_starts("").is_empty());
    }
}
