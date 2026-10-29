//! Reading an agent's thread from its transcript (`sb inspect`;
//! `sb history`): the conversation entries, stable positions, cursors
//! and the origin of a task (RFC 0001 §7.5). Pure: the daemon reads the
//! file, this module decides what to show.
//!
//! A position is the line number (from 1) of the entry in
//! `transcript.log`. The file only grows, so a position never moves.

use crate::util::{age, clip, one_line, strip_thinking, wire_unescape};
use bise_proto::thread::lines::{self, Hub, Rec};
use std::path::Path;

/// Chars one answer may carry (the rest is reachable with cursors).
pub const BUDGET: usize = 6000;
/// Chars of one entry in a listing; `--at` shows it whole.
const ENTRY_CLIP: usize = 800;
/// Chars of one search hit.
const HIT_CLIP: usize = 300;
/// Chars of an entry read with `--at` (or the origin message).
const FULL_CLIP: usize = 12_000;
pub const DEFAULT_LIMIT: usize = 20;
pub const MAX_LIMIT: usize = 200;

/// One raw line of a transcript: (position, time, line).
pub type Raw = (usize, u64, String);

/// Every line of a transcript (`<ms>\t<line>` per line), with positions.
pub fn read(path: &Path) -> Vec<Raw> {
    std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn parse(text: &str) -> Vec<Raw> {
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            l.split_once('\t')
                .map(|(t, l)| (i + 1, t.parse().unwrap_or(0), l.to_string()))
        })
        .collect()
}

/// What a human would call "the conversation": user messages, messages
/// in, assistant texts and tool calls, one entry each.
pub fn readable(line: &str) -> Option<String> {
    if let lines::Rec::Obs(lines::Obs::Assistant(r)) = lines::read(line) {
        let v = strip_thinking(&wire_unescape(&r));
        return (!v.is_empty()).then(|| format!("assistant: {}", v));
    }
    if let Some(rest) = line.strip_prefix("sb ") {
        return Some(match lines::hub(rest) {
            Hub::You(t, _) => format!("user: {t}"),
            Hub::MsgIn { from, id, body, .. } if from.is_empty() => format!("message from {}", with_id(&id, &body)),
            Hub::MsgIn { id, body, view: Some(plugin), .. } => format!("message from a view via {} {}", plugin, with_id(&id, &body)),
            Hub::MsgIn { from, id, body, .. } => format!("message from {} {}", from, with_id(&id, &body)),
            // hub line protocol v2 (C2)
            Hub::Msg { from, id, body, .. } if from.is_empty() => format!("message {}", with_id(&id, &body)),
            Hub::Msg { from, to, id, body } => format!("message {} → {} {}", from, to, with_id(&id, &body)),
            Hub::MsgYou { from, body } => format!("{} to the user: {}", if from.is_empty() { "?" } else { &from }, body),
            Hub::Answered { agent, question, answer, why } => {
                let why = if why.is_empty() { String::new() } else { format!(" (why: {why})") };
                format!("main answered @{agent} for the user: {question} -> {answer}{why}")
            }
            _ => format!("hub: {}", wire_unescape(rest)),
        });
    }
    if let Some(r) = line.strip_prefix("tool #") {
        let r = r.split_once(' ').map(|x| x.1).unwrap_or(r);
        return Some(format!("tool: {}", clip(&wire_unescape(r), 300)));
    }
    None
}

/// A message's `m_<n> : <text>` (the id when it has one).
fn with_id(id: &str, body: &str) -> String {
    match id {
        "" => format!(": {body}"),
        id => format!("{id} : {body}"),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub pos: usize,
    pub ms: u64,
    pub text: String,
}

pub fn entries(raw: &[Raw]) -> Vec<Entry> {
    raw.iter()
        .filter_map(|(pos, ms, l)| {
            readable(l).map(|text| Entry {
                pos: *pos,
                ms: *ms,
                text,
            })
        })
        .collect()
}

/// Where the window sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// The latest entries.
    Tail,
    Before(usize),
    After(usize),
    Around(usize),
    /// One entry, whole.
    At(usize),
}

/// `#156` or `156`.
pub fn parse_pos(s: &str) -> Option<usize> {
    s.trim().trim_start_matches('#').parse().ok()
}

/// All the words, case-insensitive (empty: everything matches).
fn matches(text: &str, words: &[String]) -> bool {
    let low = text.to_lowercase();
    words.iter().all(|w| low.contains(w.as_str()))
}

pub fn words_of(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// A search hit on one line, starting a little before the first word.
fn snippet(text: &str, words: &[String]) -> String {
    // keep the kind ("user:", "assistant:"...) in front of the cut
    let (kind, text) = match text.split_once(": ") {
        Some((k, rest)) if k.len() <= 30 => (format!("{}: ", k), rest),
        _ => (String::new(), text),
    };
    let flat = one_line(text);
    let low = flat.to_lowercase();
    let start = words
        .iter()
        .filter_map(|w| low.find(w.as_str()))
        .min()
        .map(|b| low[..b].chars().count())
        .unwrap_or(0);
    let from = start.saturating_sub(80);
    let body: String = flat.chars().skip(from).collect();
    let head = if from > 0 { "…" } else { "" };
    format!("{}{}{}", kind, head, clip(&body, HIT_CLIP))
}

/// What one `sb inspect` shows: the entries in order, and whether there
/// is more on each side.
#[derive(Debug, PartialEq)]
pub struct Page {
    pub shown: Vec<(Entry, String)>,
    pub earlier: bool,
    pub later: bool,
}

/// Pick the entries of a window: at most `limit`, at most `budget`
/// chars, the nearest to the anchor first.
pub fn window(
    all: &[Entry],
    words: &[String],
    anchor: Anchor,
    limit: usize,
    budget: usize,
) -> Page {
    let search = !words.is_empty();
    let render = |e: &Entry| {
        if search {
            snippet(&e.text, words)
        } else {
            clip(&e.text, ENTRY_CLIP)
        }
    };
    let limit = limit.clamp(1, MAX_LIMIT);
    if let Anchor::At(p) = anchor {
        let shown: Vec<(Entry, String)> = all
            .iter()
            .filter(|e| e.pos == p)
            .map(|e| (e.clone(), clip(&e.text, FULL_CLIP)))
            .collect();
        return Page {
            earlier: all.first().is_some_and(|e| e.pos < p),
            later: all.last().is_some_and(|e| e.pos > p),
            shown,
        };
    }
    let pool: Vec<&Entry> = all.iter().filter(|e| matches(&e.text, words)).collect();
    // the split point: entries before it are "older", from it "newer"
    let (split, take_before, take_after) = match anchor {
        Anchor::Tail => (pool.len(), limit, 0),
        Anchor::Before(p) => (pool.partition_point(|e| e.pos < p), limit, 0),
        Anchor::After(p) => (pool.partition_point(|e| e.pos <= p), 0, limit),
        Anchor::Around(p) => {
            let s = pool.partition_point(|e| e.pos < p);
            (s, limit / 2, limit - limit / 2)
        }
        Anchor::At(_) => unreachable!(),
    };
    let (mut lo, mut hi) = (split, split);
    let mut used = 0usize;
    // grow alternately (newer first): the anchor itself is at `split`
    loop {
        let mut grew = false;
        if hi < pool.len() && hi - split < take_after {
            let r = render(pool[hi]);
            if used + r.len() > budget && lo < hi {
                break;
            }
            used += r.len();
            hi += 1;
            grew = true;
        }
        if lo > 0 && split - lo < take_before {
            let r = render(pool[lo - 1]);
            if used + r.len() > budget && lo < hi {
                break;
            }
            used += r.len();
            lo -= 1;
            grew = true;
        }
        if !grew {
            break;
        }
    }
    Page {
        shown: pool[lo..hi]
            .iter()
            .map(|e| ((*e).clone(), render(e)))
            .collect(),
        earlier: lo > 0,
        later: hi < pool.len(),
    }
}

/// The text of a page, with its cursors to go on.
pub fn render_page(agent: &str, query: &str, page: &Page, now: u64) -> String {
    if page.shown.is_empty() {
        return if query.is_empty() {
            format!("no entry there in {}'s thread", agent)
        } else {
            format!("no match for \"{}\" there in {}'s thread", query, agent)
        };
    }
    let mut out: Vec<String> = page
        .shown
        .iter()
        .map(|(e, r)| format!("#{} ({} ago) {}", e.pos, age(e.ms, now), r))
        .collect();
    let q = if query.is_empty() {
        String::new()
    } else {
        format!(" --query \"{}\"", query)
    };
    let mut more: Vec<String> = Vec::new();
    if page.earlier {
        more.push(format!(
            "earlier: sb inspect {}{} --before #{}",
            agent, q, page.shown[0].0.pos
        ));
    }
    if page.later {
        more.push(format!(
            "later: sb inspect {}{} --after #{}",
            agent,
            q,
            page.shown[page.shown.len() - 1].0.pos
        ));
    }
    let clipped = page
        .shown
        .iter()
        .any(|(e, r)| r.chars().count() < e.text.chars().count());
    if clipped {
        more.push(format!("whole entry: sb inspect {} --at #<pos>", agent));
    }
    if !query.is_empty() {
        more.push(format!(
            "context of a hit: sb inspect {} --around #<pos>",
            agent
        ));
    }
    if !more.is_empty() {
        out.push(format!("-- {}", more.join(" | ")));
    }
    out.join("\n")
}

/// The line where the hub recorded the spawn of `task` (by its first
/// name), the one nearest `created_ms` when the name was reused.
fn spawn_line(raw: &[Raw], task: &str, created_ms: u64) -> Option<(usize, bool)> {
    let tag = format!("@{}", task);
    raw.iter()
        .filter_map(|(pos, ms, l)| match lines::read(l) {
            Rec::Hub(Hub::Spawn(text)) => Some((*pos, *ms, text)),
            _ => None,
        })
        .filter(|(_, _, r)| {
            // " → new agent " (older journals: " → new task ", " → nouvelle tâche ")
            r.split_once(" → new agent ")
                .or_else(|| r.split_once(" → new task "))
                .or_else(|| r.split_once(" → nouvelle tâche "))
                .map(|(_, t)| {
                    t.strip_prefix(&tag).is_some_and(|rest| {
                        !rest
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
                    })
                })
                .unwrap_or(false)
        })
        .min_by_key(|(_, ms, _)| ms.abs_diff(created_ms))
        .map(|(pos, _, r)| (pos, (r.starts_with("you →") || r.starts_with("toi →"))))
}

/// Where a task came from in its parent's thread.
#[derive(Debug, PartialEq)]
pub struct Origin {
    /// The user message that led to the spawn (None: the user created
    /// the task directly, or no user message precedes it).
    pub user: Option<usize>,
    pub spawn: usize,
}

pub fn origin(raw: &[Raw], task: &str, created_ms: u64) -> Option<Origin> {
    let (spawn, by_user) = spawn_line(raw, task, created_ms)?;
    let user = if by_user {
        None
    } else {
        raw.iter()
            .rev()
            .filter(|(p, _, l)| *p < spawn && matches!(lines::read(l), Rec::Hub(Hub::You(..))))
            .map(|(p, _, _)| *p)
            .next()
    };
    Some(Origin { user, spawn })
}

/// The origin as the task reads it: the user message whole, then the
/// parent's turn up to the spawn.
pub fn render_origin(agent: &str, task: &str, all: &[Entry], o: &Origin, now: u64) -> String {
    let mut out: Vec<String> = Vec::new();
    let from = match o.user {
        None => {
            out.push(format!(
                "the user created `{}` directly (#{}): the brief is the user's own words.",
                task, o.spawn
            ));
            o.spawn
        }
        Some(u) => {
            out.push(format!(
                "origin of `{}` in {}'s thread: the user message #{} (verbatim), then {}'s turn up to the spawn #{}.",
                task, agent, u, agent, o.spawn
            ));
            u
        }
    };
    let turn: Vec<Entry> = all
        .iter()
        .filter(|e| e.pos >= from && e.pos <= o.spawn)
        .cloned()
        .collect();
    let mut used = 0usize;
    let mut cut = false;
    for (i, e) in turn.iter().enumerate() {
        let r = if i == 0 {
            clip(&e.text, FULL_CLIP)
        } else {
            clip(&e.text, ENTRY_CLIP)
        };
        if i > 0 && used + r.len() > BUDGET {
            cut = true;
            break;
        }
        used += r.len();
        out.push(format!("#{} ({} ago) {}", e.pos, age(e.ms, now), r));
    }
    let mut more = vec![format!("earlier: sb inspect {} --before #{}", agent, from)];
    if cut {
        more.push(format!(
            "rest of the turn: sb inspect {} --after #{}",
            agent, from
        ));
    }
    more.push(format!(
        "after the spawn: sb inspect {} --after #{}",
        agent, o.spawn
    ));
    out.push(format!("-- {}", more.join(" | ")));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The v2 kinds (C2) read back as conversation entries; v1 lines
    /// still read as before.
    #[test]
    fn protocol_v2_kinds_are_readable() {
        assert_eq!(readable("sb msg : a → b : hi\\nthere").unwrap(), "message a → b : hi\nthere");
        assert_eq!(readable("sb msg-you : docs : la v2").unwrap(), "docs to the user: la v2");
        assert_eq!(
            readable("sb answered : docs : v1 \\: v2? : v2 : the brief says v2").unwrap(),
            "main answered @docs for the user: v1 : v2? -> v2 (why: the brief says v2)"
        );
        assert_eq!(readable("sb answered : docs : q : a : ").unwrap(), "main answered @docs for the user: q -> a");
        assert_eq!(readable("sb msg-in : docs m_3 : x").unwrap(), "message from docs m_3 : x");
        assert_eq!(readable("sb msg-in : @docs : la v2").unwrap(), "message from @docs : la v2");
    }

    fn log(lines: &[&str]) -> Vec<Raw> {
        let text: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}\t{}", 1000 + i as u64, l))
            .collect();
        parse(&text.join("\n"))
    }

    fn main_log() -> Vec<Raw> {
        log(&[
            "sb you : bonjour",                                   // 1
            "  obs: turn_started",                                // 2
            "  obs: assistant: <think>hm</think>salut",           // 3
            "  obs: turn_done: completed",                        // 4
            "sb you : ajoute le mode sombre\\n\\navec un toggle", // 5
            "  obs: assistant: je crée une tâche",                // 6
            "tool #1 bash : sb spawn dark --objective x",         // 7
            "sb spawn : main → new task @dark : x",               // 8
            "sb spawn : main → new task @dark-2 : y",             // 9
            "sb you : et les tests ?",                            // 10
            "sb spawn : toi → nouvelle tâche @solo : z",          // 11 (older French journal)
        ])
    }

    fn pos(p: &Page) -> Vec<usize> {
        p.shown.iter().map(|(e, _)| e.pos).collect()
    }

    #[test]
    fn positions_are_line_numbers_and_skip_noise() {
        let e = entries(&main_log());
        assert_eq!(e[0].pos, 1);
        assert_eq!(
            e[1],
            Entry {
                pos: 3,
                ms: 1002,
                text: "assistant: salut".into()
            }
        );
        assert!(e.iter().all(|x| x.pos != 2 && x.pos != 4));
        assert_eq!(e[2].text, "user: ajoute le mode sombre\n\navec un toggle");
    }

    #[test]
    fn cursors_page_both_ways() {
        let e = entries(&main_log());
        let w: Vec<String> = Vec::new();
        let tail = window(&e, &w, Anchor::Tail, 2, BUDGET);
        assert_eq!(pos(&tail), vec![10, 11]);
        assert!(tail.earlier && !tail.later);
        let before = window(&e, &w, Anchor::Before(10), 3, BUDGET);
        assert_eq!(pos(&before), vec![7, 8, 9]);
        let after = window(&e, &w, Anchor::After(5), 2, BUDGET);
        assert_eq!(pos(&after), vec![6, 7]);
        assert!(after.earlier && after.later);
        let around = window(&e, &w, Anchor::Around(7), 4, BUDGET);
        assert_eq!(pos(&around), vec![5, 6, 7, 8]);
        let at = window(&e, &w, Anchor::At(5), 20, BUDGET);
        assert_eq!(pos(&at), vec![5]);
        let end = window(&e, &w, Anchor::After(11), 5, BUDGET);
        assert!(end.shown.is_empty() && !end.later);
    }

    #[test]
    fn the_budget_bounds_the_output_and_keeps_the_nearest() {
        let big = format!("sb you : {}", "x".repeat(700));
        let lines: Vec<&str> = (0..30).map(|_| big.as_str()).collect();
        let e = entries(&log(&lines));
        let p = window(&e, &[], Anchor::Tail, 30, BUDGET);
        let chars: usize = p.shown.iter().map(|(_, r)| r.len()).sum();
        assert!(chars <= BUDGET && !p.shown.is_empty());
        assert_eq!(p.shown.last().unwrap().0.pos, 30);
        assert!(p.earlier);
        let text = render_page("main", "", &p, 5000);
        assert!(text.contains(&format!("--before #{}", p.shown[0].0.pos)));
    }

    #[test]
    fn search_returns_positions() {
        let e = entries(&main_log());
        let w = words_of("Mode SOMBRE");
        let p = window(&e, &w, Anchor::Tail, 20, BUDGET);
        assert_eq!(pos(&p), vec![5]);
        let text = render_page("main", "Mode SOMBRE", &p, 5000);
        assert!(text.starts_with("#5 "));
        assert!(text.contains("--around #<pos>"));
        let spawns = window(
            &e,
            &words_of("new task"),
            Anchor::Before(11),
            20,
            BUDGET,
        );
        assert_eq!(pos(&spawns), vec![8, 9]);
    }

    #[test]
    fn the_origin_is_the_user_message_before_the_spawn() {
        let raw = main_log();
        assert_eq!(
            origin(&raw, "dark", 1007),
            Some(Origin {
                user: Some(5),
                spawn: 8
            })
        );
        assert_eq!(
            origin(&raw, "dark-2", 1008),
            Some(Origin {
                user: Some(5),
                spawn: 9
            })
        );
        assert_eq!(
            origin(&raw, "solo", 1010),
            Some(Origin {
                user: None,
                spawn: 11
            })
        );
        assert_eq!(origin(&raw, "nope", 0), None);
        let o = origin(&raw, "dark", 1007).unwrap();
        let text = render_origin("main", "dark", &entries(&raw), &o, 5000);
        assert!(text.contains("#5 (") && text.contains("avec un toggle"));
        assert!(text.contains("#8 (") && !text.contains("#9 ("));
        assert!(text.contains("--before #5"));
    }

    #[test]
    fn a_reused_name_takes_the_nearest_spawn() {
        let raw = log(&[
            "sb you : a",
            "sb spawn : main → new task @t : a",
            "sb you : b",
            "sb spawn : main → new task @t : b",
        ]);
        assert_eq!(
            origin(&raw, "t", 1003),
            Some(Origin {
                user: Some(3),
                spawn: 4
            })
        );
        assert_eq!(
            origin(&raw, "t", 1001),
            Some(Origin {
                user: Some(1),
                spawn: 2
            })
        );
    }
}
