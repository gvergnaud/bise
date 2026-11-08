//! c6 (architect m_56402): the words of the messages, read from the
//! journal when a reader needs them. sb-core's state keeps a message's
//! header only; its text and parts stay in its `message_sent` line.
//! [`Bodies`] keeps where each line starts (filled at the boot's journal
//! read and after each append, daemon.rs) and reads one line back on
//! demand: a pread, no cache.
//!
//! A message whose line can't be read (no offset, an I/O error, a parse
//! error, another message's line) is a [`Missing`], rendered by
//! [`Missing::text`] wherever it goes, never an empty string.

use bise_proto::core::body::{Missing, Words};
use serde_json::Value;
use std::path::PathBuf;

/// Where each message's `message_sent` line starts in the journal.
#[derive(Clone, Debug, Default)]
pub struct Bodies {
    /// by message id (ids are dense): its line's byte offset + 1; 0 none
    at: Vec<u64>,
    src: Src,
}

#[derive(Clone, Debug)]
enum Src {
    /// the hub's journal file
    File(PathBuf),
    /// a test hub's journal, in memory ([`Bodies::keep`])
    Mem(Vec<u8>),
}

impl Default for Src {
    fn default() -> Self {
        Src::Mem(Vec::new())
    }
}

impl Bodies {
    /// The bodies of the journal at `path` (none indexed yet).
    pub fn file(path: PathBuf) -> Bodies {
        Bodies { at: Vec::new(), src: Src::File(path) }
    }

    /// The journal event `ev` starts at byte `off`: a `message_sent`
    /// one is indexed, any other is not a message's.
    pub fn index(&mut self, ev: &Value, off: u64) {
        if ev["type"] != "message_sent" {
            return;
        }
        let Some(id) = ev["msg"]["id"].as_u64().and_then(|i| usize::try_from(i).ok()) else { return };
        if self.at.len() <= id {
            self.at.resize(id + 1, 0);
        }
        self.at[id] = off + 1;
    }

    /// Tests: a test hub journals `ev` (no file): kept in memory,
    /// indexed as the daemon's append does.
    pub fn keep(&mut self, ev: &Value) {
        let Src::Mem(bytes) = &mut self.src else { return };
        let off = bytes.len() as u64;
        bytes.extend_from_slice(ev.to_string().as_bytes());
        bytes.push(b'\n');
        self.index(ev, off);
    }

    /// Message `id`'s words, read from its journal line.
    pub fn words(&self, id: u64) -> Result<Words, Missing> {
        let off = usize::try_from(id).ok().and_then(|i| self.at.get(i)).copied().filter(|o| *o > 0).ok_or(Missing(id))? - 1;
        let line = self.line(off).ok_or(Missing(id))?;
        let ev: Value = serde_json::from_slice(&line).map_err(|_| Missing(id))?;
        match Words::of_journal(&ev) {
            Some((got, w)) if got == id => Ok(w),
            _ => Err(Missing(id)),
        }
    }

    /// The line that starts at byte `off`, without its end.
    fn line(&self, off: u64) -> Option<Vec<u8>> {
        match &self.src {
            Src::Mem(bytes) => {
                let rest = bytes.get(usize::try_from(off).ok()?..)?;
                Some(rest.split(|b| *b == b'\n').next().unwrap_or_default().to_vec())
            }
            Src::File(path) => read_line(path, off),
        }
    }
}

/// The line of the file at `path` that starts at byte `off` (pread, in
/// chunks until its end of line or the file's).
fn read_line(path: &std::path::Path, off: u64) -> Option<Vec<u8>> {
    use std::os::unix::fs::FileExt;
    let f = std::fs::File::open(path).ok()?;
    let mut line = Vec::new();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = f.read_at(&mut buf, off + line.len() as u64).ok()?;
        if n == 0 {
            return Some(line);
        }
        match buf[..n].iter().position(|b| *b == b'\n') {
            Some(i) => {
                line.extend_from_slice(&buf[..i]);
                return Some(line);
            }
            None => line.extend_from_slice(&buf[..n]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sent(id: u64, text: &str) -> Value {
        json!({"type": "message_sent", "msg": {"id": id, "from": "a", "to": "b", "text": text, "parts": ""}})
    }

    /// A file journal: each message's words come back from its line,
    /// one longer than a read chunk too; an unknown id, an offset on
    /// another message's line or on a torn line is a Missing.
    #[test]
    fn the_words_come_back_from_the_journal_file() {
        let d = std::env::temp_dir().join(format!("sb-bodies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("journal.jsonl");
        let long = "é".repeat(20_000);
        let evs = [json!({"type": "agent_created"}), sent(1, "hi"), sent(2, &long)];
        let mut text = String::new();
        let mut b = Bodies::file(p.clone());
        for ev in &evs {
            b.index(ev, text.len() as u64);
            text.push_str(&format!("{ev}\n"));
        }
        let torn = text.len() as u64;
        text.push_str("{\"type\":\"message_sent\",\"msg\":{\"id\":3,\"te");
        std::fs::write(&p, &text).unwrap();
        assert_eq!(b.words(1).unwrap().text, "hi");
        assert_eq!(b.words(2).unwrap().text, long);
        assert_eq!(b.words(7), Err(Missing(7)));
        b.index(&sent(3, "x"), torn);
        assert_eq!(b.words(3), Err(Missing(3)), "a torn line");
        b.index(&sent(4, "x"), 0);
        assert_eq!(b.words(4), Err(Missing(4)), "another event's line");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A test hub's journal in memory reads the same.
    #[test]
    fn a_kept_journal_reads_the_same() {
        let mut b = Bodies::default();
        b.keep(&json!({"type": "agent_created"}));
        b.keep(&sent(1, "one"));
        b.keep(&sent(2, "two"));
        assert_eq!(b.words(2).unwrap().text, "two");
        assert_eq!(b.words(1).unwrap().text, "one");
        assert_eq!(b.words(0), Err(Missing(0)));
    }
}
