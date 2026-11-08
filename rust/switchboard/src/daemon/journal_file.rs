//! The journal file on the hub's side (`journal.jsonl`): read back at a
//! boot and at a revive (`read`), appended with sb-core's lines (`Out`).
//! sb-core decides every line (hub/gen/journal.bend); this module only
//! moves bytes and says when they did not move.
//!
//! - `read` takes bytes and decodes each line alone: one undecodable line
//!   (a multi-byte char cut by a full disk) is one unreadable line, never
//!   an empty hub.
//! - `Out` keeps `ok`: false from the first failed write or flush to the
//!   end of the session (the file may miss a line or hold half of one),
//!   with one hub.log line. The boot photo (docs/issues/36) is never
//!   written while it is false.

use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// The journal as [`read`] finds it.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Read {
    pub events: Vec<Value>,
    /// where each event's line starts (a byte offset), by event: the
    /// mirror reads a message's words there (bodies.rs)
    pub offsets: Vec<u64>,
    /// the (1-based) numbers of the lines that are not UTF-8 or not a
    /// JSON object
    pub bad: Vec<usize>,
}

/// The journal at `path` (no file: an empty journal), as [`parse`].
pub(super) fn read(path: &Path) -> Read {
    parse(&std::fs::read(path).unwrap_or_default())
}

/// The journal's events, and the lines that are not UTF-8 or not a JSON
/// object (a half-written last line, a cut multi-byte char...): they are
/// never dropped in silence, the caller logs them. The Rust side does not
/// decode the events (hub/gen/journal.bend does), so a kind it does not
/// know still reaches sb-core.
pub(super) fn parse(bytes: &[u8]) -> Read {
    let mut r = Read::default();
    let mut off = 0u64;
    for (i, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        let at = off;
        off += raw.len() as u64 + 1;
        // as str::lines: a "\r\n" ending is one line ending
        let l = raw.strip_suffix(b"\r").unwrap_or(raw);
        let Ok(s) = std::str::from_utf8(l) else {
            r.bad.push(i + 1);
            continue;
        };
        if s.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(s) {
            Ok(v) if v.is_object() => {
                r.events.push(v);
                r.offsets.push(at);
            }
            _ => r.bad.push(i + 1),
        }
    }
    r
}

/// The message bodies of the journal at `path`, as [`read`] found it
/// (bodies.rs): each message's line, by its id.
pub(super) fn bodies(path: &Path, r: &Read) -> crate::bodies::Bodies {
    let mut b = crate::bodies::Bodies::file(path.to_path_buf());
    for (ev, off) in r.events.iter().zip(&r.offsets) {
        b.index(ev, *off);
    }
    b
}

/// The first line numbers of a list, for a log line.
pub(super) fn lines_list(ns: &[usize]) -> String {
    let mut s: Vec<String> = ns.iter().take(10).map(|n| n.to_string()).collect();
    if ns.len() > 10 {
        s.push("...".into());
    }
    s.join(", ")
}

/// The journal opened for appending, and whether every line so far
/// reached it. `ok` stays false for the session: after a failed write the
/// file may hold half a line or miss one, so no photo trusts it until a
/// restart; the next boot reads it line by line, logs the torn line, does
/// a full replay and takes a fresh photo.
pub(super) struct Out<W: Sink = std::fs::File> {
    w: W,
    ok: bool,
}

/// What the journal is written to: a writer that knows its end (where
/// the next line starts: a file in append mode writes there whatever a
/// failed write left before it).
pub(super) trait Sink: Write {
    fn end(&self) -> Option<u64>;
}

impl Sink for std::fs::File {
    fn end(&self) -> Option<u64> {
        self.metadata().ok().map(|m| m.len())
    }
}

/// One [`Out::append`]: where its line starts when it reached the file
/// (the mirror reads a message's words there, bodies.rs), and the
/// hub.log line of the session's first failed write.
#[derive(Debug, PartialEq)]
pub(super) struct Written {
    pub at: Option<u64>,
    pub log: Option<String>,
}

impl<W: Sink> Out<W> {
    pub(super) fn new(w: W) -> Self {
        Out { w, ok: true }
    }

    /// Append one line and flush it. On the session's first failed write
    /// or flush: `ok` goes false for good and the hub.log line comes back;
    /// a later failure has none (one log line, not one per event).
    pub(super) fn append(&mut self, line: &str) -> Written {
        let at = self.w.end();
        let res = writeln!(self.w, "{}", line).and_then(|_| self.w.flush());
        match res {
            Ok(()) => Written { at, log: None },
            Err(e) if self.ok => {
                self.ok = false;
                let log = format!("journal: a write failed ({e}): the journal may miss a line from here; journal_ok false until the hub restarts");
                Written { at: None, log: Some(log) }
            }
            Err(_) => Written { at: None, log: None },
        }
    }

    /// Every line of this session reached the file (read by the boot
    /// photo, docs/issues/36 chunk 4).
    pub(super) fn ok(&self) -> bool {
        self.ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("sb-journal-file-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A line that is not a JSON object (a half-written last line) is
    /// counted with its number, never dropped in silence; a kind the Rust
    /// side does not know is kept (sb-core decodes the events).
    #[test]
    fn the_journal_keeps_unknown_kinds_and_counts_unreadable_lines() {
        let text = "{\"type\":\"main_notes_flushed\"}\n{\"type\":\"from_a_newer_hub\",\"x\":1}\n\n42\n{\"type\":\"main_no";
        let r = parse(text.as_bytes());
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[1]["type"], "from_a_newer_hub");
        assert_eq!(r.bad, vec![4, 5]);
        // each event's line starts where the bodies read it (bodies.rs)
        assert_eq!(r.offsets, vec![0, text.find("{\"type\":\"from").unwrap() as u64]);
        assert_eq!(lines_list(&(1..=12).collect::<Vec<_>>()), "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, ...");
    }

    /// A full disk cut "é" (0xC3 0xA9) after its first byte, and the hub
    /// wrote on: the boot read the whole file as one string, failed, and
    /// started an EMPTY hub. Now that line alone is unreadable; every
    /// other line is replayed, the non-ASCII ones too.
    #[test]
    fn a_cut_utf8_char_costs_its_line_only() {
        let d = tmp("cut");
        let p = d.join("journal.jsonl");
        let mut bytes = b"{\"type\":\"a\",\"text\":\"caf\xC3\xA9\"}\r\n".to_vec();
        bytes.extend_from_slice(b"{\"type\":\"b\",\"text\":\"caf\xC3");
        bytes.extend_from_slice(b"\n{\"type\":\"c\"}\n");
        std::fs::write(&p, &bytes).unwrap();
        let r = read(&p);
        let kinds: Vec<&str> = r.events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, vec!["a", "c"]);
        assert_eq!(r.events[0]["text"], "café");
        assert_eq!(r.bad, vec![2]);
        // a "\r\n" line and a torn one count their bytes: "c" starts after them
        assert_eq!(r.offsets[1] as usize, bytes.len() - "{\"type\":\"c\"}\n".len());
        // no file: an empty journal, as before
        assert_eq!(read(&d.join("none.jsonl")), Read::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A write to a closed fd (here: a file opened read-only, its writes
    /// fail with EBADF) flips `ok` and gives one log line; the next
    /// failure gives none, and `ok` stays false after a write that works.
    #[test]
    fn a_failed_write_flips_ok_and_logs_once() {
        let d = tmp("write");
        let p = d.join("journal.jsonl");
        std::fs::write(&p, "").unwrap();
        let mut good = Out::new(std::fs::OpenOptions::new().append(true).open(&p).unwrap());
        assert_eq!(good.append("{\"type\":\"a\"}"), Written { at: Some(0), log: None });
        // the next line starts at the file's end
        assert_eq!(good.append("{\"type\":\"b\"}").at, Some(13));
        assert!(good.ok());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{\"type\":\"a\"}\n{\"type\":\"b\"}\n");

        let mut out = Out::new(std::fs::File::open(&p).unwrap());
        let w = out.append("{\"type\":\"b\"}");
        let log = w.log.expect("a failed write is logged");
        assert!(log.starts_with("journal: a write failed ("), "{log}");
        assert_eq!(w.at, None, "a line that never reached the file has no place");
        assert!(!out.ok());
        assert_eq!(out.append("{\"type\":\"c\"}"), Written { at: None, log: None }, "one log line per session");
        assert!(!out.ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A write that lands but whose flush fails is a failed write too.
    #[test]
    fn a_failed_flush_flips_ok() {
        struct NoFlush(Vec<u8>, bool);
        impl Write for NoFlush {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                if self.1 { Err(std::io::Error::other("disk full")) } else { Ok(()) }
            }
        }
        impl Sink for NoFlush {
            fn end(&self) -> Option<u64> {
                Some(self.0.len() as u64)
            }
        }
        let mut out = Out::new(NoFlush(vec![], false));
        assert_eq!(out.append("x"), Written { at: Some(0), log: None });
        assert!(out.ok());
        out.w.1 = true;
        assert!(out.append("y").log.unwrap().contains("disk full"));
        assert!(!out.ok());
    }
}
