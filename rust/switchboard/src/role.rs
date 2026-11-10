//! The agents' role lines (BISE-126): one short line per task, what it
//! is doing now, for the header of the TUI when you view it. Pure: the
//! hub (core.rs) decides when to ask, the daemon runs the one-shot call
//! (repl-live with `BISE_ONESHOT`, runtime/oneshot.bend) and hands the
//! reply back through [`clean`].

use std::hash::{Hash, Hasher};

/// The widest line, in characters.
pub const MAX: usize = 60;

/// The mark the system prompt starts with (the fake provider of the tests
/// answers it with a fixed line).
pub const MARK: &str = "# bise role line";

/// `s` in at most `max` characters, cut on a word when it can, with `…`.
pub fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max.saturating_sub(1)).collect();
    let head = match head.rfind(' ') {
        Some(i) if i >= head.len() / 2 => head[..i].trim_end().to_string(),
        _ => head,
    };
    format!("{}…", head.trim_end_matches([',', ';', ':', '.', ' ']))
}

/// The line before any model call: the objective's first sentence, cut to
/// [`MAX`].
pub fn default_line(objective: &str) -> String {
    let o = objective.trim();
    let mut end = o.len();
    for pat in [". ", "; ", "\n", ".\t"] {
        if let Some(i) = o.find(pat) {
            end = end.min(i);
        }
    }
    let first = crate::util::one_line(&o[..end]);
    cut(first.trim_end_matches('.'), MAX)
}

/// What a line is made from; a new call only when this changes.
pub fn key(objective: &str, report: &str, note: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (objective, report, note).hash(&mut h);
    format!("{:016x}", h.finish())
}

const SYSTEM: &str = "# bise role line\n\
You write the one-line label shown above an AI agent's thread in a terminal app. \
Say what the agent is doing now, or its role, from what it was asked and what it last reported. \
Rules: one line, at most 60 characters, lowercase, plain words, no quotes, no final period, \
no agent name, no ticket numbers unless they are the point. \
Examples: fixing the safari login redirect / drafting the v2 release notes / waiting on docs for the api answer. \
Answer with the line only.";

fn field(s: &str, max: usize) -> String {
    crate::util::clip(&crate::util::one_line(s), max)
}

/// The wire request (runtime/remote.bend's format) of one call: a system
/// message and a user message, one line each (≤ ~1k tokens).
pub fn request(objective: &str, report: &str, note: &str, current: &str) -> String {
    let mut user = format!("What it was asked: {}", field(objective, 1500));
    if !report.trim().is_empty() {
        user.push_str(&format!("\nIts last report: {}", field(report, 1200)));
    }
    if !note.trim().is_empty() {
        user.push_str(&format!("\nIts status note: {}", field(note, 200)));
    }
    if !current.trim().is_empty() {
        user.push_str(&format!("\nThe line now (keep it if it is still right): {}", field(current, 100)));
    }
    format!(
        "MODEL default\nMSG system : {}\nMSG user : {}\nEND\n",
        escape(SYSTEM),
        escape(&user)
    )
}

/// One wire line: newlines as `\n` (W.unescape_nl reads them back).
fn escape(s: &str) -> String {
    s.replace('\\', "/").replace('\n', "\\n")
}

/// The line of a model's reply: its first non-empty line, without quotes,
/// a `role:` label or a final period, lowercase, cut to [`MAX`]. None:
/// nothing usable (the old line stays). A reasoning model's `<think>`
/// span is its thinking, never the line (his sidebar read '<think>',
/// main m_22156): it goes first, an unclosed one with all after it.
/// Also the reader of a stored line (daemon.rs read_role), so a line
/// saved before this rule is dropped, not shown.
pub fn clean(reply: &str) -> Option<String> {
    let reply = crate::util::strip_thinking(reply);
    let first = reply
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("```"))?;
    let mut l = first.trim_start_matches(['-', '*', '#', '>', ' ']).trim().to_string();
    for label in ["role line:", "role:", "line:", "label:"] {
        if l.to_lowercase().starts_with(label) {
            l = l[label.len()..].trim().to_string();
        }
    }
    let l = l
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '«' | '»' | '“' | '”' | '*' | '_'))
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_lowercase();
    let l = crate::util::one_line(&l);
    if l.is_empty() || l.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(cut(&l, MAX))
}

/// The result of a one-shot run: the text after the last `ONESHOT_OK `
/// (newlines unescaped; a reasoning model's `<think>…</think>` before
/// its answer dropped: GLM 5.3 Flash on Mistral sends one), or the error.
pub fn oneshot_reply(stdout: &str) -> Result<String, String> {
    for l in stdout.lines().rev() {
        if let Some(t) = l.strip_prefix("ONESHOT_OK ") {
            let t = t.replace("\\n", "\n");
            return Ok(t.rsplit_once("</think>").map_or(t.as_str(), |(_, answer)| answer.trim_start()).to_string());
        }
        if let Some(e) = l.strip_prefix("ONESHOT_ERR ") {
            return Err(e.to_string());
        }
    }
    Err("no answer".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_line_is_the_first_sentence_cut() {
        assert_eq!(default_line("Fix the login. Then the docs."), "Fix the login");
        assert_eq!(default_line("write the docs"), "write the docs");
        let long = "BISE-126: when the user views a sub-agent, the header shows a one-line description of what it does";
        let d = default_line(long);
        assert!(d.chars().count() <= MAX, "{}", d);
        assert!(d.ends_with('…'), "{}", d);
        assert!(d.starts_with("BISE-126: when the user views a sub-agent"), "{}", d);
        assert_eq!(default_line(""), "");
    }

    /// main m_22156: a reasoning model's thinking never becomes the line.
    #[test]
    fn a_think_span_is_never_the_line() {
        assert_eq!(clean("<think>
the user wants a label
</think>
Fixing the login.").as_deref(), Some("fixing the login"));
        assert_eq!(clean("<think>a</think> drafting the notes").as_deref(), Some("drafting the notes"));
        assert_eq!(clean("<think>
still thinking"), None, "unclosed: no line, the old one stays");
        assert_eq!(clean("<think>"), None, "a line stored before the rule is dropped on read");
        assert_eq!(clean("reviewing the parser"), Some("reviewing the parser".into()), "a stored line reads back as itself");
    }

    #[test]
    fn a_reply_is_cleaned_to_one_short_lowercase_line() {
        assert_eq!(clean("Fixing the Safari login.\n").as_deref(), Some("fixing the safari login"));
        assert_eq!(clean("\"drafting the v2 notes\"").as_deref(), Some("drafting the v2 notes"));
        assert_eq!(clean("Role: waiting on docs").as_deref(), Some("waiting on docs"));
        assert_eq!(clean("\n\n  - reviewing the parser\nmore").as_deref(), Some("reviewing the parser"));
        assert_eq!(clean("   "), None);
        assert_eq!(clean("\"\""), None);
        let l = clean(&"word ".repeat(40)).unwrap();
        assert!(l.chars().count() <= MAX && l.ends_with('…'), "{}", l);
    }

    #[test]
    fn the_key_follows_the_inputs() {
        assert_eq!(key("a", "b", "c"), key("a", "b", "c"));
        assert_ne!(key("a", "b", "c"), key("a", "b2", "c"));
        assert_ne!(key("ab", "", "c"), key("a", "b", "c"));
    }

    #[test]
    fn the_request_is_two_wire_lines_and_bounded() {
        let big = "x".repeat(10_000);
        let r = request(&big, &big, &big, "old line");
        let msgs: Vec<&str> = r.lines().filter(|l| l.starts_with("MSG ")).collect();
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].starts_with("MSG system : # bise role line\\n"));
        assert!(msgs[1].contains("The line now (keep it if it is still right): old line"));
        // about 1k tokens at most (4 chars a token)
        assert!(r.len() < 4600, "{}", r.len());
        let r = request("line one\nline two", "", "", "");
        assert!(!r.contains("Its last report") && !r.contains("Its status note") && !r.contains("The line now"));
        assert!(r.contains("line one line two"));
    }

    #[test]
    fn the_oneshot_answer_is_its_last_marker_line() {
        assert_eq!(oneshot_reply("usage: 1 2\nONESHOT_OK a\\nb\n"), Ok("a\nb".to_string()));
        assert_eq!(oneshot_reply("ONESHOT_ERR ANTHROPIC_API_KEY is not set\n"), Err("ANTHROPIC_API_KEY is not set".to_string()));
        assert!(oneshot_reply("").is_err());
        // a reasoning model's thinking, braces and all, never reaches the caller's parse
        assert_eq!(oneshot_reply("ONESHOT_OK <think>answer {\"ok\": true} only\\n</think>\\n{\"ok\": true}\n"), Ok("{\"ok\": true}".to_string()));
    }
}
