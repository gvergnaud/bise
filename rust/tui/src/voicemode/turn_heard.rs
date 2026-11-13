//! What the listener heard, into the turn (split out of turn.rs, its
//! line limit): the words, the flush that ends a turn, a failure that
//! stops the listening, and a note (the voice model fell back: its one
//! line where a failure's goes, the listening goes on).

use super::*;

impl VoiceMode {
    pub(super) fn pump_heard(&mut self, now: Instant) {
        let Some(rx) = &self.heard_rx else { return };
        let mut got = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(h) => got.push(h),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.heard_rx = None;
                    self.listen_tx = None;
                    break;
                }
            }
        }
        for h in got {
            match h {
                Heard::Text(t) => {
                    self.heard.push_str(&t);
                    self.captions_you = true;
                }
                Heard::Flushed => {
                    if self.flushing.is_some() {
                        self.finish_turn(now);
                    }
                }
                // a long turn's segment: its words came as Text
                Heard::Segment(_) => {}
                // shown FAIL_SHOWN, like a failure's line
                Heard::Note(line) => self.failed = Some((line, now)),
                Heard::Failed(line) => {
                    self.failed = Some((line, now));
                    self.listen_tx = None;
                    self.heard_rx = None;
                    if self.flushing.is_some() {
                        self.finish_turn(now);
                    }
                }
            }
        }
    }
}
