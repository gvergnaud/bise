//! The REPL wire lines the hub needs to understand (the full vocabulary
//! is documented in `.agents/skills/bend-harness-qa/reference/`).

use crate::util::{strip_thinking, wire_unescape};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wire {
    TurnStarted,
    /// The batch ended: the REPL is idle and reads its socket again.
    Idle,
    /// Visible assistant text (reasoning removed), unescaped.
    Assistant(String),
    TurnDone(String),
    /// A steering entry reached the Core (committed to the turn).
    SteeringReceived,
    /// A steering entry went into a model request.
    Steered,
    /// `tool #<id> <name> : <args>` (a live tool call annotation).
    Tool {
        name: String,
        args: String,
    },
    /// `tool_intent #<id> : <text>` (BISE-223): the model's one-line
    /// description of a bash or run_typescript call.
    Intent(String),
    /// `bg_handoff : {"slot", "cmd"}` (event-wake): the bash tool handed a
    /// command off to the background; the JSON, as written.
    BgHandoff(String),
    /// A line replayed from a restored session.
    History,
    Other,
}

pub fn parse(line: &str) -> Wire {
    if line.starts_with("history ") {
        return Wire::History;
    }
    if line == "--- idle" {
        return Wire::Idle;
    }
    let t = line.trim_start();
    if t == "obs: turn_started" {
        return Wire::TurnStarted;
    }
    if let Some(rest) = t.strip_prefix("obs: assistant: ") {
        return Wire::Assistant(strip_thinking(&wire_unescape(rest)));
    }
    if t.starts_with("obs: steering_received: ") {
        return Wire::SteeringReceived;
    }
    if t.starts_with("obs: steered: ") {
        return Wire::Steered;
    }
    if let Some(rest) = t.strip_prefix("obs: turn_done: ") {
        return Wire::TurnDone(rest.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("bg_handoff : ") {
        return Wire::BgHandoff(rest.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("tool_intent #") {
        if let Some((_, text)) = rest.split_once(" : ") {
            return Wire::Intent(text.trim().to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("tool #") {
        if let Some((_, rest)) = rest.split_once(' ') {
            let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
            return Wire::Tool {
                name: name.trim().to_string(),
                args: args.to_string(),
            };
        }
    }
    Wire::Other
}

/// The file an `edit` or `write_file` call acts on: the `file_path` of
/// its feed line's JSON (the runtime puts it first, so a line cut at
/// 200 chars still holds it when the path is shorter).
pub fn edit_file(args: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(args) {
        return v.get("file_path").and_then(|p| p.as_str()).map(str::to_string).filter(|p| !p.is_empty());
    }
    let rest = &args[args.find("\"file_path\"")? + 11..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"').unwrap_or(rest.len());
    Some(rest[..end].to_string()).filter(|p| !p.is_empty())
}

/// The files an `apply_patch` call touches (its V4A headers). The
/// annotation carries the JSON args with escaped newlines.
pub fn patch_files(args: &str) -> Vec<String> {
    let text = args.replace("\\\\n", "\n").replace("\\n", "\n");
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches('"');
        for marker in [
            "*** Update File: ",
            "*** Add File: ",
            "*** Delete File: ",
            "*** Move to: ",
        ] {
            if let Some(p) = l.strip_prefix(marker) {
                let p = p.trim().trim_end_matches('"').trim_end_matches(',').trim();
                if !p.is_empty() && !out.iter().any(|x| x == p) {
                    out.push(p.to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_file_reads_the_path_of_a_whole_or_cut_line() {
        assert_eq!(edit_file("{\"file_path\":\"src/a.rs\"}").as_deref(), Some("src/a.rs"));
        assert_eq!(edit_file("{\"file_path\": \"/w/b.md\", \"content\": \"cut her").as_deref(), Some("/w/b.md"));
        assert_eq!(edit_file("{\"content\":\"x\"}"), None);
        assert_eq!(edit_file("garbage"), None);
    }

    #[test]
    fn turn_markers() {
        assert_eq!(parse("  obs: turn_started"), Wire::TurnStarted);
        assert_eq!(parse("--- idle"), Wire::Idle);
        assert_eq!(
            parse("  obs: turn_done: completed"),
            Wire::TurnDone("completed".into())
        );
        assert_eq!(parse("history   obs: turn_started"), Wire::History);
    }

    #[test]
    fn assistant_text_is_unescaped_without_reasoning() {
        assert_eq!(
            parse("  obs: assistant: <think>hmm</think>\\nFait.\\nOK"),
            Wire::Assistant("Fait.\nOK".into())
        );
    }

    #[test]
    fn tool_annotation() {
        assert_eq!(
            parse("tool #3 bash : {\"arg\":\"sb list\"}"),
            Wire::Tool {
                name: "bash".into(),
                args: "{\"arg\":\"sb list\"}".into()
            }
        );
    }

    #[test]
    fn bg_handoff_line() {
        let l = "bg_handoff : {\"slot\":\"/t/bg/3\",\"cmd\":\"cargo test\"}";
        assert_eq!(parse(l), Wire::BgHandoff("{\"slot\":\"/t/bg/3\",\"cmd\":\"cargo test\"}".into()));
        assert_eq!(parse(&format!("history {}", l)), Wire::History);
    }

    #[test]
    fn intent_annotation() {
        assert_eq!(parse("tool_intent #4 : je lance les tests"), Wire::Intent("je lance les tests".into()));
        assert_eq!(parse("history tool_intent #4 : x"), Wire::History);
    }

    #[test]
    fn patch_headers() {
        let args = "{\"arg\":\"*** Begin Patch\\n*** Update File: src/a.rs\\n@@\\n-x\\n+y\\n*** Add File: b.md\\n+z\\n*** End Patch\"}";
        assert_eq!(
            patch_files(args),
            vec!["src/a.rs".to_string(), "b.md".to_string()]
        );
    }
}
