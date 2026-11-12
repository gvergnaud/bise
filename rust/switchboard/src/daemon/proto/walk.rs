//! The hub's one transcript walker (thread-perf, architect m_26633 and
//! m_60492): a worker reads an agent's transcript through its offset index
//! (tindex.rs), folds it a chunk at a time and hands the entries to a
//! visitor, never on the hub's loop. Two walks use it:
//! - `thread/find` (⌘F over a whole thread): forward from the first line,
//!   the entries that hold the needle (`bise_proto::thread::find`'s rule),
//!   one `thread_hits`. A newer find of the same connection and agent moves
//!   the rid it watches: the older walk stops at its next chunk and sends
//!   nothing.
//! - `thread/marks` (msg-dots: the desktop's ticks beside a thread): backward
//!   from the end, his last messages (`bise_proto::thread::marks`' rule), one
//!   `thread_marks`; it stops once it has them.

use super::super::tindex::{Indexes, TIndex};
use super::*;
use bise_proto::thread::find::{hits, FindHit, MAX_HITS};
use bise_proto::thread::marks::{mark_of, Mark, MAX_MARKS};
use bise_proto::thread::Line;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Transcript lines folded at once (a fold starts at an entry's first line).
const CHUNK: usize = 4000;
/// A chunk holding a single entry (a very long tools run) grows to this.
const CHUNK_MAX: usize = 64_000;

/// Which way a walk reads the transcript.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dir {
    /// from the first line on; a chunk's entries oldest first
    Forward,
    /// from the last complete line back; a chunk's entries newest first
    Back,
}

/// What a fold gives for lines that start at an entry's first line.
pub(super) type Fold<'a> = &'a dyn Fn(&[Line]) -> Vec<pthread::Entry>;

fn lines_of(t: &TIndex, path: &Path, from: usize, before: usize) -> Vec<Line> {
    t.read(path, from, before).into_iter().map(|(p, ts, l)| (p as u64, ts.unwrap_or(0), l)).collect()
}

/// Walks the transcript at `path` up to its last complete line (returned),
/// every entry whole and once, in `dir`'s order: `visit` gets each
/// chunk's entries and says whether to go on. `stop` says a newer walk
/// replaced this one (None then). A chunk ends where an entry may go on
/// past it: that entry is folded again in the next chunk, from its first
/// line; a chunk of one entry grows (to [`CHUNK_MAX`]).
pub(super) fn walk(ix: &Indexes, path: &Path, dir: Dir, fold: Fold, visit: &mut dyn FnMut(Vec<pthread::Entry>) -> bool, stop: &dyn Fn() -> bool) -> Option<u64> {
    let t = ix.build(path);
    let upto = t.lines();
    let (mut start, mut end, mut chunk) = (1, upto + 1, CHUNK);
    while start < end {
        if stop() {
            return None;
        }
        let entries = match dir {
            Dir::Forward => {
                let stop_at = (start + chunk).min(upto + 1);
                let mut entries = fold(&lines_of(&t, path, start, stop_at));
                // the last entry may go on in the next chunk: folded again from its first line there
                let next = match (stop_at > upto, entries.len()) {
                    (true, _) | (false, 0) => stop_at,
                    (false, 1) if chunk < CHUNK_MAX => {
                        chunk *= 2;
                        continue;
                    }
                    (false, 1) => stop_at,
                    (false, _) => entries.pop().map_or(stop_at, |e| (e.pos as usize).max(start + 1)),
                };
                start = next;
                entries
            }
            Dir::Back => {
                let from = end.saturating_sub(chunk).max(1);
                let mut entries = fold(&lines_of(&t, path, from, end));
                // the first entry may have begun before the chunk: folded again, whole, in the next one back
                let next = match (from == 1, entries.len()) {
                    (true, _) | (false, 0) => from,
                    (false, 1) if chunk < CHUNK_MAX => {
                        chunk *= 2;
                        continue;
                    }
                    (false, 1) => {
                        entries.clear();
                        from
                    }
                    (false, _) => {
                        entries.remove(0);
                        entries.first().map_or(from, |e| (e.pos as usize).min(end - 1))
                    }
                };
                end = next;
                entries.reverse();
                entries
            }
        };
        chunk = CHUNK;
        if !visit(entries) {
            break;
        }
    }
    Some(upto as u64)
}

/// Every hit of `needle` in the transcript at `path` up to its last
/// complete line, oldest first (the newest [`MAX_HITS`]), and that line.
pub(super) fn search(ix: &Indexes, path: &Path, needle: &str, fold: Fold, stop: &dyn Fn() -> bool) -> Option<(Vec<FindHit>, u64)> {
    let mut out: Vec<FindHit> = Vec::new();
    let upto = walk(ix, path, Dir::Forward, fold, &mut |entries| {
        out.extend(hits(&entries, needle));
        if out.len() > 2 * MAX_HITS {
            out.drain(..out.len() - MAX_HITS);
        }
        true
    }, stop)?;
    if out.len() > MAX_HITS {
        out.drain(..out.len() - MAX_HITS);
    }
    Some((out, upto))
}

/// His last `limit` (at most [`MAX_MARKS`]) messages in the transcript at
/// `path`, oldest first, and its last complete line.
pub(super) fn marks(ix: &Indexes, path: &Path, limit: usize, fold: Fold) -> (Vec<Mark>, u64) {
    let limit = limit.min(MAX_MARKS);
    let mut out: Vec<Mark> = Vec::new();
    let upto = walk(ix, path, Dir::Back, fold, &mut |entries| {
        out.extend(entries.iter().filter_map(mark_of).take(limit - out.len()));
        out.len() < limit
    }, &|| false)
    .unwrap_or(0);
    out.reverse();
    (out, upto)
}

impl Shell {
    /// A worker's fold: the hub's facts taken now; a notice's provider is
    /// named by its id (no catalog off the loop): never in a walk's words.
    fn walk_fold(&self) -> impl Fn(&[Line]) -> Vec<pthread::Entry> + Send + 'static {
        let facts = self.facts();
        move |lines: &[Line]| {
            let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| if i.is_empty() { k.to_string() } else { i.to_string() }, width: &width, offset: &offset, attached: &crate::attached::split };
            pthread::fold(lines, &ctx)
        }
    }

    /// `thread/find {agent, needle, rid}` of connection `id`: the search
    /// goes to a worker; its `thread_hits` comes back through the loop
    /// (`Msg::Typed`).
    pub(in crate::daemon) fn proto_find(&mut self, id: ClientId, tag: &str, project: String, agent: String, needle: String, rid: u64) {
        let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, tag, &format!("no agent {agent}")) };
        let path = self.transcript(&dir);
        let cur = self.finds.entry((id, agent.clone())).or_insert_with(|| Arc::new(AtomicU64::new(0))).clone();
        cur.store(rid, Ordering::SeqCst);
        let fold = self.walk_fold();
        let (ix, tx) = (self.tix.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let stop = || cur.load(Ordering::SeqCst) != rid;
            let Some((hits, upto)) = search(&ix, &path, &needle, &fold, &stop) else { return };
            let v = HubEv::ThreadHits { project, agent, rid, needle, hits, upto }.to_value();
            let _ = tx.send(Msg::Typed { to: Typed::Answer(id), v });
        });
    }

    /// `thread/marks {agent, limit}` of connection `id`: his last messages,
    /// read back on a worker; its `thread_marks` comes back through the
    /// loop (`Msg::Typed`).
    pub(in crate::daemon) fn proto_marks(&mut self, id: ClientId, tag: &str, project: String, agent: String, limit: usize) {
        let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, tag, &format!("no agent {agent}")) };
        let path = self.transcript(&dir);
        let fold = self.walk_fold();
        let (ix, tx) = (self.tix.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let (marks, upto) = marks(&ix, &path, limit, &fold);
            let v = HubEv::ThreadMarks { project, agent, marks, upto }.to_value();
            let _ = tx.send(Msg::Typed { to: Typed::Answer(id), v });
        });
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
