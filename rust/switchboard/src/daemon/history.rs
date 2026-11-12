//! `sb inspect`, `sb history` and `sb show` (RFC 0001 §7.5, BISE-233), and
//! the transcript pages the TUI's scroll-back reads: an agent's thread
//! from its transcript, and the search index over every thread. The typed
//! thread pages read through the transcript's offset index (tindex.rs,
//! `page_lines`): a page costs its own lines, not the file's. `answer`
//! works over an agents dir and an agent list, no Shell, so a reader of
//! another hub's threads (stream C) calls it with that hub's. `sb
//! history` and `sb show` run on the history worker (history_work.rs):
//! the Shell's part only takes what they need from memory.

use super::history_work::HistoryJob;
use super::Shell;
use crate::search;
use crate::transcript::{self, Anchor};
use crate::util::now_ms;
use bise_proto::sb::{History, Inspect};
use serde_json::{json, Value};
use std::os::unix::net::UnixStream;
use std::path::Path;

/// The lines of a transcript at positions [before - count, before)
/// (positions from 1, as `transcript.rs`), each with the time it was
/// written (ms, None when the stamp does not parse), without keeping the
/// rest of the file in memory.
pub(super) fn transcript_page(path: &Path, before: usize, count: usize) -> Vec<(usize, Option<u64>, String)> {
    use std::io::BufRead;
    let Ok(f) = std::fs::File::open(path) else { return Vec::new() };
    let from = before.saturating_sub(count).max(1);
    let mut out = Vec::new();
    for (i, l) in std::io::BufReader::new(f).lines().enumerate() {
        let pos = i + 1;
        if pos >= before {
            break;
        }
        let Ok(l) = l else { break };
        if pos >= from {
            if let Some((ms, line)) = l.split_once('\t') {
                out.push((pos, ms.parse().ok(), line.to_string()));
            }
        }
    }
    out
}

/// The lines of a transcript at positions [from, from + count), as
/// [`transcript_page`]: what comes after a page (its last turn's steered
/// receipts, `thread::mark_ahead`).
pub(super) fn transcript_ahead(path: &Path, from: usize, count: usize) -> Vec<(usize, Option<u64>, String)> {
    transcript_page(path, from + count, count).into_iter().filter(|l| l.0 >= from).collect()
}

/// [`Shell::page_lines`] over the Shell's indexes alone (a clone shares
/// them): the page arm's reads back (`page_back`, thread-older) while
/// the rest of the Shell is borrowed.
pub(super) fn page_lines_in(tix: &super::tindex::Indexes, path: &Path, before: Option<usize>, count: usize) -> Vec<(usize, Option<u64>, String)> {
    let at = |ix: &super::tindex::TIndex| {
        let before = before.unwrap_or(ix.lines() + 1);
        ix.read(path, before.saturating_sub(count), before)
    };
    if tix.ensure(path) {
        if let Some(lines) = tix.with(path, at) {
            return lines;
        }
    }
    let before = before.unwrap_or_else(|| super::lines::transcript_len(path) + 1);
    transcript_page(path, before, count)
}

impl Shell {
    /// The lines of `path` at [before - count, before), `before` None: its
    /// end (the newest page). Through the transcript's offset index
    /// (tindex.rs): a page reads its own lines. A thread paged for the
    /// first time has none yet: read the old way, once, while a worker
    /// builds it.
    pub(super) fn page_lines(&self, path: &Path, before: Option<usize>, count: usize) -> Vec<(usize, Option<u64>, String)> {
        page_lines_in(&self.tix, path, before, count)
    }

    /// The lines of `path` at [from, from + count), as [`Self::page_lines`]
    /// (a page's last turn's steered receipts).
    pub(super) fn ahead_lines(&self, path: &Path, from: usize, count: usize) -> Vec<(usize, Option<u64>, String)> {
        if let Some(lines) = self.tix.with(path, |ix| ix.read(path, from, from + count)) {
            return lines;
        }
        transcript_ahead(path, from, count)
    }

    /// `sb inspect`: a bounded page of an agent's thread, with positions
    /// and cursors, or the origin of the caller (RFC 0001 §7.5).
    pub(super) fn inspect(&self, from: &str, q: &Inspect) -> Value {
        let target = &q.agent;
        let Some(name) = self.hub.st.resolve(target) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let Some(dir) = self.dir_of(&name) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let path = self.transcript(&dir);
        let now = now_ms();
        if q.origin {
            let raw = transcript::read(&path);
            let all = transcript::entries(&raw);
            let Some(me) = self.hub.st.agents.get(from) else {
                return json!({"ok": false, "error": "--origin: unknown calling agent"});
            };
            return match transcript::origin(&raw, &me.dir, me.created_ms) {
                Some(o) => {
                    json!({"ok": true, "text": transcript::render_origin(&name, from, &all, &o, now)})
                }
                None => {
                    json!({"ok": false, "error": format!("no creation of {} in the thread of {}", from, name)})
                }
            };
        }
        inspect_page(&name, &path, q, now)
    }

    /// Every agent of this hub as the search names its threads.
    fn who(&self) -> Vec<search::Who> {
        self.hub
            .st
            .agents
            .values()
            .map(|a| search::Who {
                name: a.name.clone(),
                dir: a.dir.clone(),
                aliases: a.aliases.clone(),
                archived: a.status() == crate::model::Status::Archived,
            })
            .collect()
    }

    /// `sb history` and `sb show <agent>#<pos>` (BISE-233): the history
    /// worker's job (history_work.rs), its reply written there.
    pub(super) fn search(&self, stream: UnixStream, from: &str, look: Look) {
        let job = HistoryJob::Home { agents: self.opts.paths.state.join("agents"), who: self.who(), look };
        self.history.ask(stream, from, job);
    }

    /// msg-forward: `sb show m_<n>`: its recipient from memory (a cold
    /// message's row has it), then the worker finds its entry
    /// ([`show_msg`]); unknown here: answered now.
    pub(super) fn show_msg(&self, mut stream: UnixStream, from: &str, n: u64, context: Option<u64>) {
        let Some(m) = self.hub.st.peek(n) else {
            super::write_json(&mut stream, &json!({"ok": false, "error": format!("m_{n} unknown")}));
            return;
        };
        let to = self.hub.st.resolve(m.to).unwrap_or_else(|| m.to.to_string());
        let job = HistoryJob::Msg { agents: self.opts.paths.state.join("agents"), who: self.who(), n, to, context };
        self.history.ask(stream, from, job);
    }
}

/// `sb show m_<n>` on the history worker: the message's entry in the
/// thread of its recipient `to` (`forward::msg_pos` over the transcript
/// as a stream), then `sb show <to>#<pos>`; not found: one line naming
/// the recipient.
pub(super) fn show_msg(index: &mut search::Index, agents_dir: &Path, who: &[search::Who], n: u64, to: &str, context: Option<u64>) -> Value {
    let Some(dir) = who.iter().find(|w| w.name == to).map(|w| w.dir.clone()) else {
        return json!({"ok": false, "error": format!("m_{n} went to {to}, which has no thread here")});
    };
    let path = agents_dir.join(&dir).join("transcript.log");
    let pos = crate::store_io::open(&path).ok().and_then(|f| {
        use std::io::BufRead;
        crate::forward::msg_pos(std::io::BufReader::new(f).lines().map_while(Result::ok), n)
    });
    match pos {
        Some(pos) => answer(index, agents_dir, who, &Look::show(to, pos as u64, context), now_ms()),
        None => json!({"ok": false, "error": format!("m_{n} is not in {to}'s thread (its transcript was rotated or deleted)")}),
    }
}

/// `sb inspect`'s page of the thread at `path` (agent `name`): the
/// anchor and limit of request `q` (stream C reads another hub's the same
/// way).
pub(super) fn inspect_page(name: &str, path: &Path, q: &Inspect, now: u64) -> Value {
    let raw = transcript::read(path);
    let all = transcript::entries(&raw);
    let pos = |p: &Option<String>| p.as_deref().and_then(transcript::parse_pos);
    let anchor = if let Some(p) = pos(&q.at) {
        Anchor::At(p)
    } else if let Some(p) = pos(&q.around) {
        Anchor::Around(p)
    } else if let Some(p) = pos(&q.before) {
        Anchor::Before(p)
    } else if let Some(p) = pos(&q.after) {
        Anchor::After(p)
    } else {
        Anchor::Tail
    };
    let limit = q.last.map_or(transcript::DEFAULT_LIMIT, |n| n as usize);
    let words = transcript::words_of(&q.query);
    let page = transcript::window(&all, &words, anchor, limit, transcript::BUDGET);
    json!({"ok": true, "text": transcript::render_page(name, &q.query, &page, now)})
}

/// What `sb show` or `sb history` asks of a search index.
pub(super) enum Look {
    Show { agent: String, pos: usize, context: usize },
    Search(search::Query),
}

impl Look {
    /// `sb show <agent>#<pos> [--context <n>]`.
    pub(super) fn show(agent: &str, pos: u64, context: Option<u64>) -> Look {
        Look::Show { agent: agent.to_string(), pos: pos as usize, context: context.map_or(search::DEFAULT_CONTEXT, |n| n as usize) }
    }

    /// `sb history`'s request as the index's query (an unknown role is
    /// left out: the CLI refuses it first).
    pub(super) fn history(h: &History) -> Look {
        Look::Search(search::Query {
            text: h.query.clone(),
            agents: h.agents.clone(),
            roles: h.roles.iter().filter_map(|r| search::Role::parse(r)).collect(),
            since: h.since,
            until: h.until,
            archived: match h.archived.as_str() {
                "only" => search::Archived::Only,
                "no" => search::Archived::No,
                _ => search::Archived::Any,
            },
            limit: h.limit.map_or(search::DEFAULT_HITS, |n| n as usize),
            page: h.page.map_or(1, |n| n as usize),
            scope: None,
            all: h.all,
        })
    }

    fn cmd(&self) -> &'static str {
        match self {
            Look::Show { .. } => "show",
            Look::Search(_) => "history",
        }
    }
}

/// `sb history` and `sb show` over the threads of `agents_dir` (a hub's
/// `<state>/agents`) and its agents `who`: `index` reads what the
/// transcripts got since its last refresh, then answers `look`.
pub(super) fn answer(index: &mut search::Index, agents_dir: &Path, who: &[search::Who], look: &Look, now: u64) -> Value {
    let t0 = std::time::Instant::now();
    index.refresh(agents_dir);
    respond(index, who, look, now, t0)
}

/// `answer` on an index already refreshed (stream C refreshes its own,
/// prefixed: xread.rs).
pub(super) fn respond(index: &search::Index, who: &[search::Who], look: &Look, now: u64, t0: std::time::Instant) -> Value {
    let r = match look {
        Look::Show { agent, pos, context } => index.show(who, agent, *pos, *context, now),
        Look::Search(q) => index.search(who, q, now),
    };
    let ms = t0.elapsed().as_millis();
    if ms > 200 {
        let st = index.stats();
        eprintln!("sb {}: {} ms ({} threads, {} entries)", look.cmd(), ms, st.threads, st.docs);
    }
    match r {
        Ok(text) => json!({"ok": true, "text": text}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}
