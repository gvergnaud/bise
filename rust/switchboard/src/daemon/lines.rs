//! A feed's lines as they enter: each one gets the agent's next position,
//! goes into the memory buffer (the last BUFFER_LINES), into the text the
//! transcript appends at once, and into its `line` event. Moved out of
//! Shell::feed unchanged (architect m_12280).

use super::*;

impl Shell {
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
            events.push(line_event(name, pos, ts, &line));
        }
        (out, events)
    }
}
