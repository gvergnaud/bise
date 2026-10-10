//! The open cards (what waits on him), their options and rank, and the
//! split of a question's trailing numbered options.

use crate::Project;
use serde::{Deserialize, Serialize};

/// A numbered option of a card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Opt {
    pub n: u32,
    pub label: String,
}

/// Where a card points on a page (a question block, a step of a plan).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardPage {
    pub id: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}

/// One open card: what waits on him.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Card {
    pub id: u64,
    pub project: Project,
    /// the hub's kind (`question`, `confirm`, `merge`, …): an open set
    pub kind: String,
    pub agent: String,
    /// the question without its options
    pub question: String,
    pub options: Vec<Opt>,
    /// it holds running work now (a tool call waiting for a yes)
    pub urgent: bool,
    pub since_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<CardPage>,
    /// it acts or leaves (a tool call's yes, a merge, a release...): only
    /// one of its options answers it, never typed words (the composer
    /// rule, lead m_8777); false: a question, words answer it
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub approval: bool,
    /// its place in reading order, [`card_rank`] of its kind (0 = the most
    /// blocking): the inbox sorts on (rank, id) inside one hub, as the
    /// TUI's, and has no table of its own (architect m_9549). Absent from
    /// an older hub: a reader ranks it as `card_rank("")`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u8>,
    /// the `signin` card only (bar V14): the agents the expired ChatGPT
    /// sign-in stopped, which go on once he signs in
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Vec<String>>,
    /// client-protocol step 4 (P4c-4a): the card's words as the hub wrote
    /// them, options included (`question` and `options` are parsed from
    /// them); "" from an older hub
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// the hub's second line under it (a merge's `approved · checks
    /// pass`, a question answered another way...)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// the place it is about (a `places` row's id: a merge, a feature)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// the number of the PR it is about
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// the page its link opens (the update card's release page)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// the message it answers (`sb card --for`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_msg: Option<u64>,
    /// a page's drafts batched in one card (docs/ambient-pages.md): its
    /// fields for the capsule's words; none on any other card
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<CardBatch>,
}

/// A drafts-batch card's fields (the hub's pages/drafts.rs `info`, amb-web
/// m_6061): `count` drafts titled `title`, `what` they are (`replies`,
/// `actions`...), `names` each recipient once, `topics` one per draft (or
/// none), `line` the card's second line as is, `actions` the words of its
/// actions to close (none when it has none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardBatch {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub what: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<String>,
}

/// A card kind's rank in reading order, what blocks an agent first: an
/// approval, then what only he decides (a question, a merge, a sign-in),
/// blocked, failed, drop, overlap, done (and any kind nobody taught us),
/// the setup offers last. The one table: the TUI's inbox (`kind_look`)
/// and the hub's `Card.rank` read it.
pub fn card_rank(kind: &str) -> u8 {
    match kind {
        "approval" | "confirm" => 0,
        "question" | "merge" | "feature_try" | "feature_merge" | "signin" => 1,
        "blocked" => 2,
        "failed" | "restart" => 3,
        "drop" => 4,
        "overlap" => 5,
        // the setup card and its offers (BISE-245): they block nothing
        "setup" => 7,
        _ => 6,
    }
}

impl Card {
    /// A kind answered only by one of its options: every kind but
    /// `question` and `drop` (sb-core keeps yes/no words there, and a drop
    /// never reaches a window: bise's bookkeeping): the hub's `confirm`,
    /// `merge`, `feature_try`, `feature_merge`, `update`, `signin`, and any
    /// newer one (a kind nobody taught the clients is safer picked than
    /// typed). sb-core's refusal of words says the same (amb-hub's
    /// agreement test, architect m_8848).
    pub fn approval_kind(kind: &str) -> bool {
        !matches!(kind, "question" | "drop")
    }
}

/// A question and its options: the trailing numbered lines (`1. v1`,
/// `1) v1`, `1 - v1`, `1 v1`, two to nine, numbered from 1) or a list
/// inline at the end of its last line (`Que fais-tu ? 1. a 2. b`). None
/// found: the text as is and no options. A copy of the TUI's
/// `sb/cards.rs split_choices` (bend-tui tests that they agree); the
/// TUI takes this one at the ambient merge.
pub fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))
            .or_else(|| rest.strip_prefix(' ').filter(|r| !r.starts_with(['-', '–', ' '])))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return split_inline(text).unwrap_or_else(|| (text.to_string(), Vec::new()));
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}

fn split_inline(text: &str) -> Option<(String, Vec<String>)> {
    let t = text.trim_end();
    let line_at = t.rfind('\n').map_or(0, |i| i + 1);
    let line = &t[line_at..];
    for sep in [". ", ") "] {
        let mut at: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        for n in 1..=9 {
            let m = format!("{n}{sep}");
            let found = line[from..].match_indices(&m).map(|(i, _)| from + i).find(|&i| i == 0 || line[..i].ends_with(' '));
            match found {
                Some(i) => {
                    at.push((i, i + m.len()));
                    from = i + m.len();
                }
                None => break,
            }
        }
        if at.len() < 2 {
            continue;
        }
        let options: Vec<String> = at
            .iter()
            .enumerate()
            .map(|(k, &(_, start))| {
                let end = at.get(k + 1).map_or(line.len(), |&(i, _)| i);
                line[start..end].trim().trim_end_matches([',', ';']).trim().to_string()
            })
            .collect();
        if options.iter().any(|o| o.is_empty()) {
            continue;
        }
        let body = format!("{}{}", &t[..line_at], line[..at[0].0].trim_end()).trim_end().to_string();
        return Some((body, options));
    }
    None
}

/// A question's text as a card shows it: the body and its options.
pub fn question(text: &str) -> (String, Vec<Opt>) {
    let (body, options) = split_choices(text);
    let opts = options.into_iter().enumerate().map(|(i, label)| Opt { n: i as u32 + 1, label }).collect();
    (body.trim().to_string(), opts)
}

