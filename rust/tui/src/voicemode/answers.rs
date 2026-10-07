//! Answering the inbox by voice (owner: voice-settings; design §5, plan
//! §4.7). Pure: the controller matches the turn after a question or an
//! approval here first; a match shows `heard "the first one" → 1
//! smaller` for [`HEARD_FOR`] (esc undoes), then counts; no match: the
//! words go to the agent as usual.

use std::time::Duration;

/// How long `heard "…" → 1 smaller` shows before the answer counts.
pub const HEARD_FOR: Duration = Duration::from_millis(1500);

/// A longer turn is a sentence, not an answer: sent as words.
const MAX_WORDS: usize = 8;

/// The turn's words, lowercase, without punctuation (apostrophes and
/// inner hyphens kept: "d'accord", "uh-huh").
fn words(heard: &str) -> Vec<String> {
    heard
        .to_lowercase()
        .replace('’', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .map(|w| w.trim_matches(|c| c == '\'' || c == '-').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Only the word "allow" allows ("yes", "ok", "mm" never do; "always
/// allow" is key only; "don't allow" does not).
pub fn is_allow(heard: &str) -> bool {
    let w = words(heard);
    if w.is_empty() || w.len() > MAX_WORDS || heard.contains('?') {
        return false;
    }
    let negated = |i: usize| {
        i > 0 && matches!(w[i - 1].as_str(), "don't" | "dont" | "not" | "never" | "no" | "always" | "pas" | "jamais")
    };
    let allows: Vec<usize> = (0..w.len()).filter(|&i| w[i] == "allow" || w[i] == "allowed").collect();
    !allows.is_empty() && allows.iter().all(|&i| !negated(i)) && !w.iter().any(|x| x == "always" || x == "deny")
}

/// Ordinals and the numbers said as words (English, French), 1-based.
fn ordinal(w: &str) -> Option<usize> {
    Some(match w {
        "first" | "1st" | "premier" | "première" | "premiere" => 1,
        "second" | "2nd" | "deuxième" | "deuxieme" | "seconde" => 2,
        "third" | "3rd" | "troisième" | "troisieme" => 3,
        "fourth" | "4th" | "quatrième" | "quatrieme" => 4,
        "fifth" | "5th" | "cinquième" | "cinquieme" => 5,
        "sixth" | "6th" | "sixième" | "sixieme" => 6,
        "seventh" | "7th" | "septième" | "septieme" => 7,
        "eighth" | "8th" | "huitième" | "huitieme" => 8,
        "ninth" | "9th" | "neuvième" | "neuvieme" => 9,
        _ => return None,
    })
}

/// A number said as a word ("two", "deux"); "one", "un", "une" are also
/// a pronoun and articles: only when nothing else names a choice.
fn cardinal(w: &str) -> Option<usize> {
    Some(match w {
        "one" | "un" | "une" => 1,
        "two" | "deux" => 2,
        "three" | "trois" => 3,
        "four" | "quatre" => 4,
        "five" | "cinq" => 5,
        "six" => 6,
        "seven" | "sept" => 7,
        "eight" | "huit" => 8,
        "nine" | "neuf" => 9,
        _ => w.parse::<usize>().ok().filter(|n| (1..=99).contains(n))?,
    })
}

fn weak_cardinal(w: &str) -> bool {
    matches!(w, "one" | "un" | "une")
}

/// Words that never name a choice by its label.
const STOP: &[&str] = &[
    "the", "a", "an", "one", "ones", "it", "this", "that", "of", "to", "and", "or", "with", "for", "in", "on", "please",
    "let's", "lets", "go", "take", "i", "i'd", "want", "pick", "choose", "option", "choice", "number", "le", "la", "les",
    "un", "une", "de", "des", "du", "et", "ou", "prends", "choisis", "numéro", "numero", "option", "plutôt", "plutot",
    "s'il", "te", "plaît", "plait", "vous", "c'est", "is", "be", "do", "it's",
];

/// A label's words that can name it (lowercase, ≥ 3 letters, no stop
/// words), plural `s` dropped.
fn label_words(label: &str) -> Vec<String> {
    words(label).into_iter().filter(|w| w.chars().count() >= 3 && !STOP.contains(&w.as_str())).map(|w| stem(&w)).collect()
}

fn stem(w: &str) -> String {
    match w.strip_suffix('s') {
        Some(s) if s.chars().count() >= 3 && !s.ends_with('s') => s.to_string(),
        _ => w.to_string(),
    }
}

/// The choice `heard` names, 0-based: "1", "one", "the first one",
/// "premier", "the last one", or a word of its label ("smaller"); None
/// when unsure (two choices named, a sentence, nothing): the agent asks
/// back, or the words go as they are.
pub fn pick(heard: &str, choices: &[String]) -> Option<usize> {
    let w = words(heard);
    if choices.is_empty() || w.is_empty() || w.len() > MAX_WORDS || heard.contains('?') {
        return None;
    }
    let n = choices.len();
    // every way the turn names a choice; one wins only alone
    let mut named: Vec<usize> = Vec::new();
    let mut weak: Vec<usize> = Vec::new();
    for x in &w {
        if let Some(k) = ordinal(x) {
            named.push(k);
        } else if matches!(x.as_str(), "last" | "dernier" | "dernière" | "derniere") {
            named.push(n);
        } else if let Some(k) = cardinal(x) {
            if weak_cardinal(x) {
                weak.push(k);
            } else {
                named.push(k);
            }
        }
    }
    // a word of a label, when it is in that label only
    let labels: Vec<Vec<String>> = choices.iter().map(|c| label_words(c)).collect();
    let said: Vec<String> = w.iter().map(|x| stem(x)).collect();
    for (i, lw) in labels.iter().enumerate() {
        let hit = lw.iter().any(|l| said.contains(l) && labels.iter().enumerate().all(|(j, o)| j == i || !o.contains(l)));
        if hit {
            named.push(i + 1);
        }
    }
    // "one" counts only when nothing else names a choice ("the first
    // one", "the smaller one": a pronoun)
    if named.is_empty() {
        named = weak;
    }
    named.sort_unstable();
    named.dedup();
    match named.as_slice() {
        [k] if (1..=n).contains(k) => Some(k - 1),
        _ => None,
    }
}

/// The one decision of an answer by voice (the TUI's live.rs and the
/// window's core both call it): the 0-based option the words answer and
/// the heard line, or None (the words go as words). An approval takes
/// only the word "allow" ([`is_allow`]), as its first option (allow
/// once); a question takes [`pick`].
pub fn decide(heard: &str, approval: bool, options: &[String]) -> Option<(usize, String)> {
    if approval {
        let label = options.first()?;
        is_allow(heard).then(|| (0, heard_allow(heard, label)))
    } else {
        let i = pick(heard, options)?;
        Some((i, heard_line(heard, i + 1, &options[i])))
    }
}

/// The line shown while the answer waits: `heard "the first one" → 1
/// smaller` (the turn cut to 24 characters, the label to 24).
pub fn heard_line(heard: &str, num: usize, label: &str) -> String {
    format!("heard \"{}\" → {} {}", clip(heard.trim(), 24), num, clip(label.trim(), 24))
}

/// `heard "allow" → allow once`.
pub fn heard_allow(heard: &str, label: &str) -> String {
    format!("heard \"{}\" → {}", clip(heard.trim(), 24), clip(label.trim(), 24))
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim_end_matches(['.', '!', ',']);
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('…');
    t
}

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
