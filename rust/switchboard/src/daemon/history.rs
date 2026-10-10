//! `sb inspect`, `sb history` and `sb show` (RFC 0001 §7.5, BISE-233), and
//! the transcript pages the TUI's scroll-back reads: an agent's thread
//! from its transcript, and the search index over every thread. `answer`
//! works over an agents dir and an agent list, no Shell, so a reader of
//! another hub's threads (stream C) calls it with that hub's.

use super::Shell;
use crate::search;
use crate::transcript::{self, Anchor};
use crate::util::now_ms;
use serde_json::{json, Value};
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

impl Shell {
    /// `sb inspect`: a bounded page of an agent's thread, with positions
    /// and cursors, or the origin of the caller (RFC 0001 §7.5).
    pub(super) fn inspect(&self, from: &str, v: &Value) -> Value {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let target = s("agent");
        let Some(name) = self.hub.st.resolve(&target) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let Some(dir) = self.dir_of(&name) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let path = self.transcript(&dir);
        let now = now_ms();
        if v.get("origin") == Some(&json!(true)) {
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
        inspect_page(&name, &path, v, now)
    }

    /// `sb history` and `sb show` (BISE-233): the index reads what the
    /// transcripts got since the last search, then answers.
    pub(super) fn search(&mut self, cmd: &str, v: &Value) -> Value {
        let who: Vec<search::Who> = self
            .hub
            .st
            .agents
            .values()
            .map(|a| search::Who {
                name: a.name.clone(),
                dir: a.dir.clone(),
                aliases: a.aliases.clone(),
                archived: a.status() == crate::model::Status::Archived,
            })
            .collect();
        answer(&mut self.search, &self.opts.paths.state.join("agents"), &who, cmd, v, now_ms())
    }
}

/// `sb inspect`'s page of the thread at `path` (agent `name`): the
/// anchor and limit of request `v` (stream C reads another hub's the same
/// way).
pub(super) fn inspect_page(name: &str, path: &Path, v: &Value, now: u64) -> Value {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let raw = transcript::read(path);
    let all = transcript::entries(&raw);
    {
        let pos = |k: &str| transcript::parse_pos(&s(k));
        let anchor = if let Some(p) = pos("at") {
            Anchor::At(p)
        } else if let Some(p) = pos("around") {
            Anchor::Around(p)
        } else if let Some(p) = pos("before") {
            Anchor::Before(p)
        } else if let Some(p) = pos("after") {
            Anchor::After(p)
        } else {
            Anchor::Tail
        };
        let limit = v
            .get("last")
            .and_then(|x| x.as_u64())
            .map(|n| n as usize)
            .unwrap_or(transcript::DEFAULT_LIMIT);
        let query = s("query");
        let words = transcript::words_of(&query);
        let page = transcript::window(&all, &words, anchor, limit, transcript::BUDGET);
        json!({"ok": true, "text": transcript::render_page(name, &query, &page, now)})
    }
}

/// `sb history` and `sb show` over the threads of `agents_dir` (a hub's
/// `<state>/agents`) and its agents `who`: `index` reads what the
/// transcripts got since its last refresh, then answers `cmd` ("show",
/// else a search) for request `v`.
pub(super) fn answer(index: &mut search::Index, agents_dir: &Path, who: &[search::Who], cmd: &str, v: &Value, now: u64) -> Value {
    let t0 = std::time::Instant::now();
    index.refresh(agents_dir);
    respond(index, who, cmd, v, now, t0)
}

/// `answer` on an index already refreshed (stream C refreshes its own,
/// prefixed: xread.rs).
pub(super) fn respond(index: &search::Index, who: &[search::Who], cmd: &str, v: &Value, now: u64, t0: std::time::Instant) -> Value {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let n = |k: &str, d: usize| v.get(k).and_then(|x| x.as_u64()).map_or(d, |x| x as usize);
    let strs = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let r = if cmd == "show" {
        index.show(who, &s("agent"), n("pos", 0), n("context", search::DEFAULT_CONTEXT), now)
    } else {
        let q = search::Query {
            text: s("query"),
            agents: strs("agents"),
            roles: strs("roles").iter().filter_map(|r| search::Role::parse(r)).collect(),
            since: v.get("since").and_then(|x| x.as_u64()),
            until: v.get("until").and_then(|x| x.as_u64()),
            archived: match s("archived").as_str() {
                "only" => search::Archived::Only,
                "no" => search::Archived::No,
                _ => search::Archived::Any,
            },
            limit: n("limit", search::DEFAULT_HITS),
            page: n("page", 1),
            scope: Some(s("scope")).filter(|p| !p.is_empty()),
            all: v.get("all") == Some(&json!(true)),
        };
        index.search(who, &q, now)
    };
    let ms = t0.elapsed().as_millis();
    if ms > 200 {
        let st = index.stats();
        eprintln!("sb {}: {} ms ({} threads, {} entries)", cmd, ms, st.threads, st.docs);
    }
    match r {
        Ok(text) => json!({"ok": true, "text": text}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}
