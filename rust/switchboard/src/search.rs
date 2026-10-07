//! `sb history` and `sb show` (BISE-233): search every agent's thread
//! (main, tasks, archived tasks) and open a hit with its neighbors.
//!
//! The source is `agents/<dir>/transcript.log`, which only grows: a
//! compaction or a restart of the agent does not touch it, so a search
//! finds the raw messages from before any compaction checkpoint.
//!
//! The index is in memory, built incrementally: each thread keeps the
//! byte offset it has read up to, and a search first reads only the new
//! bytes of each transcript. A document keeps its position, time, role,
//! byte offset and folded text (lowercase, no accents); the text shown is
//! read back from the file at its offset, for the few hits shown.
//! Pure but for the file reads; the daemon holds the index.

use crate::transcript::readable;
use crate::util::{age, clip, one_line, wire_unescape};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Chars one answer may carry.
pub const BUDGET: usize = 6000;
/// Chars of one hit line.
const HIT_CLIP: usize = 240;
pub const DEFAULT_HITS: usize = 10;
pub const MAX_HITS: usize = 20;
/// Chars of an entry indexed (the rest is not searchable): tool calls
/// and results are 86 % of the entries and 64 % of the text, and their
/// head is what one looks for.
const DOC_CAP: usize = 4000;
const TOOL_CAP: usize = 1000;
/// Chars of the entry `sb show` opens (the rest: `sb inspect --at`).
const SHOW_CLIP: usize = 3500;
pub const DEFAULT_CONTEXT: usize = 3;
pub const MAX_CONTEXT: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum Role {
    User,
    Assistant,
    Message,
    Tool,
    Hub,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::User, Role::Assistant, Role::Message, Role::Tool, Role::Hub];
    pub fn name(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Message => "message",
            Role::Tool => "tool",
            Role::Hub => "hub",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == s)
    }
    /// The ranking: what the user and the agents said first.
    fn tier(self) -> u8 {
        match self {
            Role::User | Role::Assistant => 0,
            Role::Message => 1,
            Role::Tool => 2,
            Role::Hub => 3,
        }
    }
}

/// The role and the readable text of a transcript line (after its
/// `<ms>\t`): the entries of `sb inspect`, plus tool results.
pub fn classify(line: &str) -> Option<(Role, String)> {
    let tool = |r: &str, kind: &str| {
        let r = r.split_once(' ').map_or(r, |x| x.1);
        Some((Role::Tool, format!("{}: {}", kind, wire_unescape(r))))
    };
    if let Some(r) = line.strip_prefix("tool_result #") {
        return tool(r, "result");
    }
    if let Some(r) = line.strip_prefix("tool #") {
        return tool(r, "tool");
    }
    if let Some(r) = line.trim_start().strip_prefix("obs: compaction_done: ") {
        return Some((Role::Hub, format!("compaction summary: {}", wire_unescape(r))));
    }
    let text = readable(line)?;
    let role = if line.starts_with("sb you : ") {
        Role::User
    } else if line.trim_start().starts_with("obs: assistant: ") {
        Role::Assistant
    } else if ["sb msg-in : ", "sb msg : ", "sb msg-you : ", "sb answered : "]
        .iter()
        .any(|p| line.starts_with(p))
    {
        Role::Message
    } else {
        Role::Hub
    };
    Some((role, text))
}

/// One char to one char: lowercase, without the accents of latin
/// letters, so a char index in the folded text is one in the original.
fn fold_char(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        '’' => '\'',
        _ => c,
    }
}

pub fn fold(s: &str) -> String {
    s.chars().map(fold_char).collect()
}

/// The terms of a query: words, or "quoted phrases", folded.
pub fn terms_of(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, part) in query.split('"').enumerate() {
        if i % 2 == 1 {
            let p = one_line(part);
            if !p.is_empty() {
                out.push(fold(&p));
            }
        } else {
            out.extend(part.split_whitespace().map(fold));
        }
    }
    out
}

/// One entry of a thread in the index.
#[derive(Debug)]
struct Doc {
    pos: usize,
    ms: u64,
    role: Role,
    /// Byte offset of its line in the transcript.
    off: u64,
    folded: Box<str>,
}

#[derive(Debug)]
struct Thread {
    path: PathBuf,
    /// Bytes read (whole lines only).
    read: u64,
    lines: usize,
    docs: Vec<Doc>,
}

impl Thread {
    fn new(path: PathBuf) -> Thread {
        Thread { path, read: 0, lines: 0, docs: Vec::new() }
    }

    /// Index the lines written since the last call.
    fn refresh(&mut self) {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.read {
            // rewritten (never by the hub): start over
            *self = Thread::new(self.path.clone());
        }
        if len == self.read || f.seek(SeekFrom::Start(self.read)).is_err() {
            return;
        }
        let mut buf = Vec::with_capacity((len - self.read) as usize);
        if f.read_to_end(&mut buf).is_err() {
            return;
        }
        let Some(end) = buf.iter().rposition(|b| *b == b'\n') else { return };
        let mut off = self.read;
        for chunk in buf[..=end].split_inclusive(|b| *b == b'\n') {
            self.lines += 1;
            let line = String::from_utf8_lossy(chunk);
            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            if let Some((t, l)) = line.split_once('\t') {
                if let Some((role, text)) = classify(l) {
                    self.docs.push(Doc {
                        pos: self.lines,
                        ms: t.parse().unwrap_or(0),
                        role,
                        off,
                        folded: fold(&clip(&text, if role == Role::Tool { TOOL_CAP } else { DOC_CAP })).into_boxed_str(),
                    });
                }
            }
            off += chunk.len() as u64;
        }
        self.read += end as u64 + 1;
    }

    /// The readable text of a doc, read back from the file.
    fn text(&self, d: &Doc) -> String {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return String::new() };
        if f.seek(SeekFrom::Start(d.off)).is_err() {
            return String::new();
        }
        let mut line = String::new();
        let _ = BufReader::new(f).read_line(&mut line);
        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        line.split_once('\t')
            .and_then(|(_, l)| classify(l))
            .map(|(_, t)| t)
            .unwrap_or_default()
    }
}

/// An agent the hub knows (archived ones included).
#[derive(Clone, Debug)]
pub struct Who {
    pub name: String,
    pub dir: String,
    pub aliases: Vec<String>,
    pub archived: bool,
}

/// Which threads: all, only archived ones, or only live ones.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Archived {
    #[default]
    Any,
    Only,
    No,
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,
    pub agents: Vec<String>,
    pub roles: Vec<Role>,
    /// ms since the epoch, inclusive
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub archived: Archived,
    pub limit: usize,
    /// from 1
    pub page: usize,
    /// stream C (S2 step 3): only the threads of this project (their
    /// keys are `<project>/<dir>`: `Index::refresh_as`)
    pub scope: Option<String>,
    /// `--all`: home and every project, ranked together (for the flags)
    pub all: bool,
}

impl Query {
    /// The flags that reproduce this query, for the "next page" line.
    fn flags(&self) -> String {
        let mut f = String::new();
        if let Some(p) = &self.scope {
            f.push_str(&format!(" --project {}", p));
        }
        if self.all {
            f.push_str(" --all");
        }
        for a in &self.agents {
            f.push_str(&format!(" --agent {}", a));
        }
        for r in &self.roles {
            f.push_str(&format!(" --role {}", r.name()));
        }
        if let Some(s) = self.since {
            f.push_str(&format!(" --since {}", fmt_iso(s)));
        }
        if let Some(u) = self.until {
            f.push_str(&format!(" --until {}", fmt_iso(u)));
        }
        match self.archived {
            Archived::Only => f.push_str(" --archived"),
            Archived::No => f.push_str(" --live"),
            Archived::Any => {}
        }
        if self.limit != DEFAULT_HITS {
            f.push_str(&format!(" --limit {}", self.limit));
        }
        f
    }
}

/// The label of a thread and whether it is archived: its agent, or its
/// directory when no agent holds it any more.
fn label_of<'a>(who: &'a [Who], dir: &'a str) -> (&'a str, bool) {
    who.iter()
        .find(|w| w.dir == dir)
        .map_or((dir, true), |w| (w.name.as_str(), w.archived))
}

/// The directory of an agent named by its name, an old name or its dir.
fn resolve<'a>(who: &'a [Who], dirs: &'a BTreeMap<String, Thread>, name: &str) -> Option<&'a str> {
    let name = name.trim_start_matches('@');
    who.iter()
        .find(|w| w.name == name)
        .or_else(|| who.iter().find(|w| w.aliases.iter().any(|a| a == name)))
        .map(|w| w.dir.as_str())
        .or_else(|| dirs.get_key_value(name).map(|(k, _)| k.as_str()))
}

#[derive(Debug, Default)]
pub struct Index {
    threads: BTreeMap<String, Thread>,
}

/// What one refresh did (for the perf log).
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub threads: usize,
    pub docs: usize,
    pub bytes: u64,
    pub folded_bytes: usize,
}

impl Index {
    /// Read what was written since the last refresh, in every
    /// `<agents_dir>/<dir>/transcript.log`.
    pub fn refresh(&mut self, agents_dir: &Path) {
        let Ok(rd) = std::fs::read_dir(agents_dir) else { return };
        for e in rd.flatten() {
            let path = e.path().join("transcript.log");
            if !path.is_file() {
                continue;
            }
            let dir = e.file_name().to_string_lossy().to_string();
            self.threads
                .entry(dir)
                .or_insert_with(|| Thread::new(path))
                .refresh();
        }
    }

    /// The same as `refresh` for another hub's `agents_dir` (stream C, S2
    /// step 3): its threads are keyed `<prefix>/<dir>`, so one index holds
    /// several projects and one query ranks them together. Called only
    /// when a query runs (never on a tick: core-idle-cpu's burn).
    pub fn refresh_as(&mut self, agents_dir: &Path, prefix: &str) {
        let Ok(rd) = std::fs::read_dir(agents_dir) else { return };
        for e in rd.flatten() {
            let path = e.path().join("transcript.log");
            if !path.is_file() {
                continue;
            }
            let dir = format!("{}/{}", prefix, e.file_name().to_string_lossy());
            self.threads.entry(dir).or_insert_with(|| Thread::new(path)).refresh();
        }
    }

    /// Drops the threads whose `<prefix>/` is not in `keep` (a project
    /// that left the registry).
    pub fn retain_prefixes(&mut self, keep: &[String]) {
        self.threads.retain(|k, _| k.split_once('/').is_some_and(|(p, _)| keep.iter().any(|x| x == p)));
    }

    pub fn stats(&self) -> Stats {
        let mut s = Stats { threads: self.threads.len(), ..Stats::default() };
        for t in self.threads.values() {
            s.docs += t.docs.len();
            s.bytes += t.read;
            s.folded_bytes += t.docs.iter().map(|d| d.folded.len()).sum::<usize>();
        }
        s
    }

    /// `sb history`: the hits of a query, ranked (user and assistant
    /// first, then messages, tool calls and results, hub lines; newest
    /// first in each), one page of at most `limit`, within `BUDGET`.
    pub fn search(&self, who: &[Who], q: &Query, now: u64) -> Result<String, String> {
        let terms = terms_of(&q.text);
        if terms.is_empty() {
            return Err("sb history needs a query: sb history \"<words>\" [--agent a] [--role r] [--since 2w]".into());
        }
        let mut only: Option<BTreeSet<&str>> = None;
        for a in &q.agents {
            match resolve(who, &self.threads, a) {
                Some(d) => {
                    only.get_or_insert_with(BTreeSet::new).insert(d);
                }
                None => return Err(format!("no agent named {}", a)),
            }
        }
        let mut hits: Vec<(&str, &Thread, &Doc)> = Vec::new();
        for (dir, t) in &self.threads {
            if only.as_ref().is_some_and(|o| !o.contains(dir.as_str())) {
                continue;
            }
            if q.scope.as_ref().is_some_and(|p| dir.split_once('/').map(|x| x.0) != Some(p.as_str())) {
                continue;
            }
            let archived = label_of(who, dir).1;
            match q.archived {
                Archived::Only if !archived => continue,
                Archived::No if archived => continue,
                _ => {}
            }
            for d in &t.docs {
                if (!q.roles.is_empty() && !q.roles.contains(&d.role))
                    || q.since.is_some_and(|s| d.ms < s)
                    || q.until.is_some_and(|u| d.ms > u)
                {
                    continue;
                }
                if terms.iter().all(|w| d.folded.contains(w.as_str())) {
                    hits.push((dir.as_str(), t, d));
                }
            }
        }
        hits.sort_by(|a, b| {
            (a.2.role.tier(), std::cmp::Reverse(a.2.ms), a.0, a.2.pos)
                .cmp(&(b.2.role.tier(), std::cmp::Reverse(b.2.ms), b.0, b.2.pos))
        });
        // a message is in the thread of its sender and of its receiver:
        // one hit
        let mut seen: HashSet<&str> = HashSet::new();
        hits.retain(|(_, _, d)| {
            if d.role != Role::Message {
                return true;
            }
            let body = d.folded.split_once(" : ").map_or(&*d.folded, |x| x.1);
            let key = &body[..floor_boundary(body, 300)];
            seen.insert(key)
        });
        let q_shown = one_line(&q.text);
        if hits.is_empty() {
            return Ok(format!(
                "no match for \"{}\"{} in {} threads (every word must appear; accents and case do not matter)",
                q_shown,
                q.flags(),
                self.threads.len()
            ));
        }
        let limit = q.limit.clamp(1, MAX_HITS);
        let pages = hits.len().div_ceil(limit);
        let page = q.page.clamp(1, pages);
        let mut by: BTreeMap<&str, usize> = BTreeMap::new();
        for (dir, _, _) in &hits {
            *by.entry(label_of(who, dir).0).or_default() += 1;
        }
        let mut by: Vec<(&str, usize)> = by.into_iter().collect();
        by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut by_s: Vec<String> = by.iter().take(6).map(|(a, n)| format!("{} {}", a, n)).collect();
        if by.len() > 6 {
            by_s.push(format!("+{} more", by.len() - 6));
        }
        let mut out = vec![format!(
            "{} hit{} for \"{}\"{} (page {}/{}; by thread: {})",
            hits.len(),
            if hits.len() == 1 { "" } else { "s" },
            q_shown,
            q.flags(),
            page,
            pages,
            by_s.join(", ")
        )];
        let mut used = out[0].len();
        for (dir, t, d) in hits.iter().skip((page - 1) * limit).take(limit) {
            let (label, archived) = label_of(who, dir);
            let line = format!(
                "{}#{} · {} ago{} · {}",
                label,
                d.pos,
                age(d.ms, now),
                if archived { " · archived" } else { "" },
                snippet(&t.text(d), &terms)
            );
            used += line.len();
            if used > BUDGET {
                break;
            }
            out.push(line);
        }
        let mut more = Vec::new();
        if page < pages {
            more.push(format!("next: sb history \"{}\"{} --page {}", q_shown.replace('"', "'"), q.flags(), page + 1));
        }
        more.push("open a hit with its neighbors: sb show <agent>#<pos>".to_string());
        if q.agents.is_empty() && q.roles.is_empty() && q.since.is_none() && pages > 1 {
            more.push("narrow: --agent <a> --role user|assistant|message|tool|hub --since 2w --until <date> --archived|--live".into());
        }
        out.push(format!("-- {}", more.join(" | ")));
        Ok(out.join("\n"))
    }

    /// `sb show <agent>#<pos>`: the entry at (or just after) a position,
    /// whole up to `SHOW_CLIP`, with `context` entries on each side, and
    /// where to go next.
    pub fn show(&self, who: &[Who], agent: &str, pos: usize, context: usize, now: u64) -> Result<String, String> {
        let dir = resolve(who, &self.threads, agent).ok_or_else(|| format!("no agent named {}", agent))?;
        let label = label_of(who, dir).0;
        let Some(t) = self.threads.get(dir) else {
            return Err(format!("{} has no thread yet", label));
        };
        let i = t.docs.partition_point(|d| d.pos < pos);
        let Some(d) = t.docs.get(i) else {
            return Err(format!("no entry at or after #{} in {}'s thread (last: #{})", pos, label, t.docs.last().map_or(0, |d| d.pos)));
        };
        let n = context.min(MAX_CONTEXT);
        let (lo, hi) = (i.saturating_sub(n), (i + n + 1).min(t.docs.len()));
        let target = clip(&t.text(d), SHOW_CLIP);
        let side = ((BUDGET.saturating_sub(target.len() + 400)) / (2 * n).max(1)).clamp(80, 400);
        let line = |d: &Doc, text: &str| format!("#{} · {} ago · {}", d.pos, age(d.ms, now), text);
        let mut out = vec![format!(
            "{}'s thread, #{} ({} UTC, {}):",
            label,
            d.pos,
            fmt_iso(d.ms).replace('T', " "),
            d.role.name()
        )];
        for e in &t.docs[lo..i] {
            out.push(line(e, &clip(&one_line(&t.text(e)), side)));
        }
        out.push(format!(">> {}", line(d, &target)));
        for e in &t.docs[i + 1..hi] {
            out.push(line(e, &clip(&one_line(&t.text(e)), side)));
        }
        let mut more = Vec::new();
        if lo > 0 {
            more.push(format!("earlier: sb show {}#{}", label, t.docs[lo - 1].pos));
        }
        if hi < t.docs.len() {
            more.push(format!("later: sb show {}#{}", label, t.docs[hi].pos));
        }
        if target.chars().count() < t.text(d).chars().count() {
            more.push(format!("whole entry: sb inspect {} --at #{}", label, d.pos));
        }
        let links = links(who, label, &t.text(d));
        if !links.is_empty() {
            more.push(links);
        }
        out.push(format!("-- {}", more.join(" | ")));
        Ok(out.join("\n"))
    }
}

/// Where the text points: agents, messages, commits.
fn links(who: &[Who], own: &str, text: &str) -> String {
    let toks: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .filter(|t| !t.is_empty())
        .collect();
    let mut agents: Vec<&str> = Vec::new();
    let mut msgs: Vec<&str> = Vec::new();
    let mut commits: Vec<&str> = Vec::new();
    for t in toks {
        if t != own && !agents.contains(&t) && who.iter().any(|w| w.name == t) {
            agents.push(t);
        } else if t.len() > 2 && t.starts_with("m_") && t[2..].bytes().all(|b| b.is_ascii_digit()) {
            if !msgs.contains(&t) {
                msgs.push(t);
            }
        } else if (7..=40).contains(&t.len())
            && t.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && t.bytes().any(|b| b.is_ascii_digit())
            && t.bytes().any(|b| b.is_ascii_alphabetic())
            && !commits.contains(&t)
        {
            commits.push(t);
        }
    }
    let mut out = Vec::new();
    if !agents.is_empty() {
        agents.truncate(5);
        out.push(format!("agents: {} (sb history \"<words>\" --agent <a>)", agents.join(", ")));
    }
    if !msgs.is_empty() {
        msgs.truncate(5);
        out.push(format!("messages: {} (sb history <id>)", msgs.join(", ")));
    }
    if !commits.is_empty() {
        commits.truncate(5);
        out.push(format!("commits: {} (git show <hash>)", commits.join(", ")));
    }
    out.join(" | ")
}

/// One line around the first term, with the kind ("user:", ...) kept.
fn snippet(text: &str, terms: &[String]) -> String {
    let (kind, body) = match text.split_once(": ") {
        Some((k, rest)) if k.chars().count() <= 40 => (format!("{}: ", k), rest),
        _ => (String::new(), text),
    };
    let flat: Vec<char> = one_line(body).chars().collect();
    let folded: String = flat.iter().map(|c| fold_char(*c)).collect();
    let start = terms
        .iter()
        .filter_map(|w| folded.find(w.as_str()))
        .min()
        .map_or(0, |b| folded[..b].chars().count());
    let from = start.saturating_sub(60);
    let body: String = flat[from..].iter().collect();
    let head = if from > 0 { "…" } else { "" };
    clip(&format!("{}{}{}", kind, head, body), HIT_CLIP)
}

/// The largest char boundary of `s` at most `i`.
fn floor_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    (0..=i).rev().find(|&j| s.is_char_boundary(j)).unwrap_or(0)
}

// ---- time: `--since 2w`, `--until 2026-09-30` ----

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

/// `2026-09-30T14:03` (UTC).
pub fn fmt_iso(ms: u64) -> String {
    let s = (ms / 1000) as i64;
    let (y, m, d) = civil_from_days(s.div_euclid(86_400));
    let r = s.rem_euclid(86_400);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}", y, m, d, r / 3600, r % 3600 / 60)
}

/// A time for `--since`/`--until`: an age (`30m`, `5h`, `2d`, `2j`,
/// `2w`) before `now`, or a UTC date `YYYY-MM-DD[THH:MM]`.
pub fn parse_time(s: &str, now: u64) -> Result<u64, String> {
    let s = s.trim();
    let bad = || format!("bad time: {} (30m, 5h, 2d, 2w, or 2026-09-30, 2026-09-30T14:00 UTC)", s);
    if let Some(unit) = s.chars().last().filter(|c| c.is_ascii_alphabetic()) {
        let n: u64 = s[..s.len() - 1].parse().map_err(|_| bad())?;
        let per = match unit {
            'm' => 60_000,
            'h' => 3_600_000,
            'd' | 'j' => 86_400_000,
            'w' => 7 * 86_400_000,
            _ => return Err(bad()),
        };
        return Ok(now.saturating_sub(n * per));
    }
    let (date, time) = s.split_once(['T', ' ']).unwrap_or((s, "00:00"));
    let p: Vec<i64> = date.split('-').map(|x| x.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    let t: Vec<i64> = time.split(':').map(|x| x.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    if p.len() != 3 || !(1..=12).contains(&p[1]) || !(1..=31).contains(&p[2]) || t.len() > 3 || t[0] > 23 {
        return Err(bad());
    }
    let secs = days_from_civil(p[0], p[1], p[2]) * 86_400 + t[0] * 3600 + t.get(1).unwrap_or(&0) * 60 + t.get(2).unwrap_or(&0);
    u64::try_from(secs * 1000).map_err(|_| bad())
}

/// `main#123`, `main #123`, `main 123`: an agent and a position.
pub fn parse_ref(args: &[String]) -> Option<(String, usize)> {
    let joined = args.join(" ");
    let (a, p) = joined
        .split_once('#')
        .or_else(|| joined.split_once(' '))?;
    let a = a.trim().trim_start_matches('@');
    let p = p.trim().trim_start_matches('#').parse().ok()?;
    (!a.is_empty() && !a.contains(' ')).then(|| (a.to_string(), p))
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
