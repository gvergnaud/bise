//! A take of any length, heard in segments (dictate-nocap, architect
//! m_64474). One request per take hit real limits: OpenAI and Groq's
//! free tier take 25 MB at most (~13 min of our 16 kHz WAV), Mistral a
//! 65k-token context, and each request has its timeout. So a clip is cut
//! once it reaches [`SEGMENT_MAX`], at its quietest half second past
//! [`SEGMENT_MIN`] ([`cut_at`]), and each segment is heard on its own,
//! while he still talks.
//!
//! [`Segments`] keeps the order: each waiting segment is tried once per
//! [`Segments::hear`], and the heard ones go out from the front only, so
//! a segment that failed holds back the ones heard after it until it is
//! heard too. Nothing is dropped: a failed segment waits for the next
//! pass; the sound not yet handed out ([`Segments::unheard`]) is what a
//! retry needs. The one cut and the one queue for the TUI's ctrl+r
//! (voice::BatchTranscriber) and the ambient core's listener
//! (voicemode/listen.rs). Pure: no clock, no I/O.

use super::SAMPLE_RATE;
use std::collections::VecDeque;

/// A clip this long is cut (3 min: ~5.8 MB of WAV, a few seconds of
/// Voxtral).
pub const SEGMENT_MAX: usize = 3 * 60 * SAMPLE_RATE as usize;
/// The cut goes past this (2 min): the segments stay long enough for the
/// model's context.
pub const SEGMENT_MIN: usize = 2 * 60 * SAMPLE_RATE as usize;
/// 100 ms: the cut's step.
const BLOCK: usize = SAMPLE_RATE as usize / 10;
/// The pause the cut looks for: half a second.
const WINDOW_BLOCKS: usize = 5;
/// A half second whose mean |sample| is under this (-40 dBFS) is a
/// pause; speech sits around -30 to -20.
const QUIET_MEAN: u64 = 328;

/// Where to cut `clip`: None while it is shorter than [`SEGMENT_MAX`];
/// else the middle of its quietest half second between [`SEGMENT_MIN`]
/// and [`SEGMENT_MAX`] (the earliest of equals), or [`SEGMENT_MAX`] when
/// no half second there is a pause.
pub fn cut_at(clip: &[i16]) -> Option<usize> {
    if clip.len() < SEGMENT_MAX {
        return None;
    }
    let blocks: Vec<u64> = clip[SEGMENT_MIN..SEGMENT_MAX].chunks(BLOCK).map(|b| b.iter().map(|s| u64::from(s.unsigned_abs())).sum()).collect();
    let quietest = (0..=blocks.len().saturating_sub(WINDOW_BLOCKS)).map(|i| (blocks[i..i + WINDOW_BLOCKS].iter().sum::<u64>(), i)).min();
    match quietest {
        Some((sum, i)) if sum <= QUIET_MEAN * (WINDOW_BLOCKS * BLOCK) as u64 => Some(SEGMENT_MIN + i * BLOCK + WINDOW_BLOCKS * BLOCK / 2),
        _ => Some(SEGMENT_MAX),
    }
}

/// A whole clip's segments, in order (the TUI's retry of a kept clip).
pub fn split(clip: &[i16]) -> Vec<&[i16]> {
    let mut out = Vec::new();
    let mut rest = clip;
    while let Some(at) = cut_at(rest) {
        let (head, tail) = rest.split_at(at);
        out.push(head);
        rest = tail;
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

enum Seg<T> {
    Waiting(Vec<i16>),
    Heard(T, Vec<i16>),
}

/// The take's sound not yet handed out: the clip being recorded, and the
/// segments cut from it, each waiting or heard (`T`: what hearing gave).
pub struct Segments<T> {
    clip: Vec<i16>,
    queue: VecDeque<Seg<T>>,
}

impl<T> Default for Segments<T> {
    fn default() -> Self {
        Segments { clip: Vec::new(), queue: VecDeque::new() }
    }
}

impl<T> Segments<T> {
    /// The mic's sound in. True: a segment was cut, its turn to be heard.
    pub fn push(&mut self, pcm: &[i16]) -> bool {
        self.clip.extend_from_slice(pcm);
        let mut cut = false;
        while let Some(at) = cut_at(&self.clip) {
            let tail = self.clip.split_off(at);
            self.queue.push_back(Seg::Waiting(std::mem::replace(&mut self.clip, tail)));
            cut = true;
        }
        cut
    }

    /// The take ends: the rest of the clip is the last segment.
    pub fn end(&mut self) {
        if !self.clip.is_empty() {
            self.queue.push_back(Seg::Waiting(std::mem::take(&mut self.clip)));
        }
    }

    /// Everything goes (the turn dropped).
    pub fn clear(&mut self) {
        self.clip.clear();
        self.queue.clear();
    }

    /// Each waiting segment heard once by `hear`, in order; then the heard
    /// ones at the front go out, in order, each with its sound. A segment
    /// heard behind one that failed waits for it. The error: the first
    /// failure of this pass (its segment still waits).
    pub fn hear<E>(&mut self, mut hear: impl FnMut(&[i16]) -> Result<T, E>) -> (Vec<(T, Vec<i16>)>, Option<E>) {
        let mut err = None;
        for seg in self.queue.iter_mut() {
            let Seg::Waiting(pcm) = seg else { continue };
            match hear(pcm) {
                Ok(t) => *seg = Seg::Heard(t, std::mem::take(pcm)),
                Err(e) => {
                    err.get_or_insert(e);
                }
            }
        }
        let mut out = Vec::new();
        while matches!(self.queue.front(), Some(Seg::Heard(..))) {
            if let Some(Seg::Heard(t, pcm)) = self.queue.pop_front() {
                out.push((t, pcm));
            }
        }
        (out, err)
    }

    /// Nothing cut is left (every segment went out).
    pub fn all_out(&self) -> bool {
        self.queue.is_empty()
    }

    /// The sound not yet handed out, in order: a failed take's kept clip.
    pub fn unheard(&self) -> Vec<i16> {
        let mut out = Vec::new();
        for seg in &self.queue {
            let (Seg::Waiting(pcm) | Seg::Heard(_, pcm)) = seg;
            out.extend_from_slice(pcm);
        }
        out.extend_from_slice(&self.clip);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEECH: i16 = 3000;
    const SEC: usize = SAMPLE_RATE as usize;

    fn speech(n: usize) -> Vec<i16> {
        // a voice: loud, never one quiet half second
        (0..n).map(|i| if i % 2 == 0 { SPEECH } else { -SPEECH }).collect()
    }

    /// Speech with a pause of `len` samples at `at`.
    fn with_pause(n: usize, at: usize, len: usize) -> Vec<i16> {
        let mut v = speech(n);
        v[at..at + len].fill(0);
        v
    }

    #[test]
    fn a_clip_under_the_max_is_never_cut() {
        assert_eq!(cut_at(&speech(SEGMENT_MAX - 1)), None);
        assert_eq!(cut_at(&[]), None);
    }

    #[test]
    fn speech_with_no_pause_is_cut_at_the_max() {
        assert_eq!(cut_at(&speech(SEGMENT_MAX)), Some(SEGMENT_MAX));
        assert_eq!(cut_at(&speech(SEGMENT_MAX + 7 * SEC)), Some(SEGMENT_MAX));
    }

    #[test]
    fn silence_is_cut_at_its_first_half_second_past_the_min() {
        assert_eq!(cut_at(&vec![0; SEGMENT_MAX]), Some(SEGMENT_MIN + SEC / 4));
    }

    #[test]
    fn a_pause_at_two_minutes_exactly_takes_the_cut() {
        let clip = with_pause(SEGMENT_MAX, SEGMENT_MIN, SEC);
        assert_eq!(cut_at(&clip), Some(SEGMENT_MIN + SEC / 4));
    }

    #[test]
    fn the_cut_goes_in_the_pause_never_in_a_word() {
        let at = SEGMENT_MIN + 37 * SEC + 3 * BLOCK;
        let clip = with_pause(SEGMENT_MAX, at, SEC / 2);
        assert_eq!(cut_at(&clip), Some(at + SEC / 4));
        // a pause before the min is not looked at
        let early = with_pause(SEGMENT_MAX, SEGMENT_MIN - SEC, SEC / 2);
        assert_eq!(cut_at(&early), Some(SEGMENT_MAX));
    }

    #[test]
    fn a_pause_too_short_is_no_pause() {
        let clip = with_pause(SEGMENT_MAX, SEGMENT_MIN + 30 * SEC, SEC / 10);
        assert_eq!(cut_at(&clip), Some(SEGMENT_MAX));
    }

    #[test]
    fn split_keeps_every_sample_in_order() {
        let clip: Vec<i16> = (0..(SEGMENT_MAX * 3 + 5)).map(|i| (i % 7) as i16 * 1000).collect();
        let parts = split(&clip);
        assert_eq!(parts.len(), 4);
        assert!(parts.iter().all(|p| p.len() <= SEGMENT_MAX));
        assert_eq!(parts.concat(), clip);
        assert!(split(&[]).is_empty());
    }

    /// Ten minutes in 100 ms blocks: every segment out, every sample once.
    #[test]
    fn a_ten_minute_take_goes_out_whole() {
        let mut s: Segments<usize> = Segments::default();
        let mut n = 0;
        let mut out = Vec::new();
        for i in 0..6000 {
            let block = vec![(i % 30000) as i16; BLOCK];
            if s.push(&block) {
                let (heard, err) = s.hear(|pcm| -> Result<usize, ()> {
                    n += 1;
                    Ok(pcm.len())
                });
                assert!(err.is_none());
                out.extend(heard);
            }
        }
        s.end();
        let (heard, err) = s.hear(|pcm| -> Result<usize, ()> { Ok(pcm.len()) });
        assert!(err.is_none());
        out.extend(heard);
        assert!(s.all_out());
        assert!(n >= 3, "cut while he talks, {n} segments");
        assert_eq!(out.iter().map(|(len, _)| len).sum::<usize>(), 6000 * BLOCK);
        let back: Vec<i16> = out.into_iter().flat_map(|(_, pcm)| pcm).collect();
        assert_eq!(back.len(), 6000 * BLOCK);
        assert!(back.chunks(BLOCK).enumerate().all(|(i, b)| b[0] == (i % 30000) as i16));
    }

    /// Segment 1 fails, 2 is heard: 2 waits. Then 1 is heard: 1, then 2.
    #[test]
    fn a_failed_segment_holds_back_the_ones_after_it() {
        let mut s: Segments<&str> = Segments::default();
        assert!(s.push(&speech(SEGMENT_MAX)));
        let (out, err) = s.hear(|_| Err("down"));
        assert!(out.is_empty());
        assert_eq!(err, Some("down"));
        assert!(s.push(&speech(SEGMENT_MAX)));
        let mut calls = 0;
        let (out, err) = s.hear(|_| {
            calls += 1;
            if calls == 1 { Err("down") } else { Ok("two") }
        });
        assert!(out.is_empty(), "two waits for one");
        assert_eq!(err, Some("down"));
        assert_eq!(s.unheard().len(), 2 * SEGMENT_MAX);
        let (out, err) = s.hear(|_| Ok::<_, &str>("one"));
        assert_eq!(err, None);
        assert_eq!(out.iter().map(|(t, _)| *t).collect::<Vec<_>>(), ["one", "two"]);
        assert!(s.all_out());
        assert!(s.unheard().is_empty());
    }

    #[test]
    fn clear_drops_everything_and_unheard_keeps_the_open_clip() {
        let mut s: Segments<()> = Segments::default();
        s.push(&speech(SEGMENT_MAX + 10));
        assert_eq!(s.unheard().len(), SEGMENT_MAX + 10);
        s.clear();
        assert!(s.all_out());
        assert!(s.unheard().is_empty());
    }
}
