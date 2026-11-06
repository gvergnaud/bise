//! A feed's lines as they enter: each one gets the agent's next position,
//! goes into the memory buffer (the last BUFFER_LINES), into the text the
//! transcript appends at once, and into its `line` event. Moved out of
//! Shell::feed unchanged (architect m_12280).

use super::*;

/// A live line of a feed (C2 `line`): `ts`, the time the transcript
/// wrote it (ms since the epoch), is left out when unknown (0, a stamp
/// that did not parse); a client reads a line without it as before.
pub(super) fn line_event(agent: &str, pos: usize, ts: u64, line: &str) -> Value {
    let mut v = json!({"ev": "line", "agent": agent, "line": line, "pos": pos});
    if ts > 0 {
        v["ts"] = json!(ts);
    }
    v
}

/// How many lines a transcript holds (the position of its last line).
pub(super) fn transcript_len(path: &Path) -> usize {
    std::fs::read(path)
        .map(|b| b.iter().filter(|c| **c == b'\n').count())
        .unwrap_or(0)
}

impl Shell {
    /// An agent's feed from its transcript: its last position and its last
    /// BUFFER_LINES lines. At boot for every agent not archived; an
    /// archived one's when its head is seeded (it came back). Never twice:
    /// a feed already held stays as it is. An agent never loaded gets its
    /// next position from the file (`transcript_len`).
    pub(super) fn load_feed(&mut self, name: &str, dir: &str) {
        if self.positions.contains_key(name) {
            return;
        }
        let (pos, tail) = transcript::tail(&self.transcript(dir), BUFFER_LINES);
        self.positions.insert(name.to_string(), pos);
        if !tail.is_empty() {
            self.buffers.insert(name.to_string(), tail.into_iter().collect());
        }
    }

    /// The lines of one feed entry (its context line too, fn_ctx_lines),
    /// stamped `ts`: (the transcript text to append, the events to send).
    pub(super) fn buffer_lines(&mut self, name: &str, path: &Path, lines: Vec<String>, ts: u64) -> (String, Vec<Value>) {
        let mut out = String::new();
        let mut events = Vec::new();
        for line in lines {
            let pos = match self.positions.get(name) {
                Some(p) => p + 1,
                None => transcript_len(path) + 1,
            };
            self.positions.insert(name.to_string(), pos);
            let b = self.buffers.entry(name.to_string()).or_default();
            b.push_back((pos, ts, line.clone()));
            while b.len() > BUFFER_LINES {
                b.pop_front();
            }
            out.push_str(&format!("{ts}\t{line}\n"));
            self.answers_line(name, pos, &line);
            // the pulse's live side (daemon/pulse.rs): turns, main's entries
            self.pulse_line(name, ts, &line);
            events.push(line_event(name, pos, ts, &line));
        }
        (out, events)
    }

    /// A `partial` line of `name`: the delta onto its live answer, and
    /// the same delta, unescaped, to every client (`partial` event: text
    /// at off; typed connections subscribed to it get HubEv::Partial). A
    /// delta whose offset doesn't match drops the block (one hub.log
    /// line).
    pub(super) fn on_partial(&mut self, name: &str, line: &str) {
        use bise_proto::thread::lines::{read, PartialStep, Rec};
        let Rec::Partial { call, off, text } = read(line) else { return };
        let p = self.partials.entry(name.to_string()).or_default();
        match p.step(&call, off, &text) {
            PartialStep::Delta { off, text } => self.broadcast(&partial_event(name, &call, off, &text)),
            PartialStep::Mismatch { call, off, have } => {
                log_line(&self.opts.paths, &format!("partial of {name} #{call} dropped: offset {off}, the hub has {have}"));
                // a client keeps no stale text meanwhile
                self.broadcast(&partial_event(name, &call, 0, ""));
            }
            PartialStep::Drop => {}
        }
    }

    /// The `sb kept` line to write before `line` when `line` ends a
    /// streamed attempt without its answer (lines::keeps_partial: a
    /// retry, a discarded candidate, a failure, a stall, a harness
    /// restart) while `name` has text streamed in: what he read stays in
    /// the thread, dim, at its place, the notices under it (designer
    /// m_13334, m_13562).
    pub(super) fn partial_kept(&self, name: &str, line: &str) -> Option<String> {
        use bise_proto::thread::lines::{keeps_partial, read};
        let p = self.partials.get(name)?;
        if !keeps_partial(&read(line)) {
            return None;
        }
        p.kept_line()
    }

    /// A feed line that ends `name`'s live answer (its entry, the turn's
    /// end, a retry, a discarded candidate, a stop, a restart:
    /// lines::clears_partial, one rule): cleared
    /// once, "" to every client.
    pub(super) fn partial_clear(&mut self, name: &str, line: &str) {
        use bise_proto::thread::lines::{clears_partial, read};
        if !self.partials.contains_key(name) || !clears_partial(&read(line)) {
            return;
        }
        if let Some(p) = self.partials.remove(name) {
            self.broadcast(&partial_event(name, &p.call, 0, ""));
        }
    }
}

/// The `partial` event (C2): `text` goes at `off` (UTF-16 code units) of
/// the text so far (architect m_17813, m_17866); "" at 0 when it ended.
pub(super) fn partial_event(agent: &str, call: &str, off: usize, text: &str) -> Value {
    json!({"ev": "partial", "agent": agent, "call": call, "off": off, "text": text})
}
