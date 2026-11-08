//! The REPL wire lines the hub needs to understand (the full vocabulary
//! is documented in `.agents/skills/bend-harness-qa/reference/`).

use crate::util::{strip_thinking, wire_unescape};
use bise_proto::thread::lines::{self, Obs, Rec};

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
    /// `tool_result #<id> ok|fail : …`: a tool call ended (les bims:
    /// the agent thinks again).
    ToolResult,
    /// `obs: compaction_started #<n> (auto|manual)`.
    CompactionStarted,
    /// `obs: compaction_done: …` or `obs: context_compaction_failed: …`.
    CompactionEnded,
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
    // the runtime's observations, either form (bise_proto's lines::read)
    if let Rec::Obs(o) = lines::read(line) {
        return match o {
            Obs::TurnStarted => Wire::TurnStarted,
            Obs::Assistant(t) => Wire::Assistant(strip_thinking(&wire_unescape(&t))),
            Obs::SteeringReceived(_) => Wire::SteeringReceived,
            Obs::Steered(_) => Wire::Steered,
            Obs::TurnDone(end) => Wire::TurnDone(end.text().trim().to_string()),
            Obs::CompactionStarted => Wire::CompactionStarted,
            Obs::CompactionDone(_) | Obs::CompactionFailed(_) => Wire::CompactionEnded,
            _ => Wire::Other,
        };
    }
    if line.starts_with("tool_result #") {
        return Wire::ToolResult;
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

/// The first answer of `f` on the lines of `path` before byte `upto`,
/// newest first: the file read backward in blocks, so a REPL's whole
/// wire.log (MBs) is not read on the hub's loop to find its last state
/// line (c4-loop-never-waits). A line cut by `upto` counts as a line.
pub fn find_back<T>(path: &std::path::Path, upto: u64, mut f: impl FnMut(&str) -> Option<T>) -> Option<T> {
    use std::io::{Read, Seek, SeekFrom};
    const BLOCK: u64 = 64 * 1024;
    let mut file = std::fs::File::open(path).ok()?;
    let mut end = upto.min(file.metadata().ok()?.len());
    // the bytes of a line not seen whole yet (its head is in an earlier block)
    let mut tail: Vec<u8> = Vec::new();
    while end > 0 {
        let start = end.saturating_sub(BLOCK);
        let mut block = vec![0u8; (end - start) as usize];
        file.seek(SeekFrom::Start(start)).ok()?;
        file.read_exact(&mut block).ok()?;
        block.extend_from_slice(&tail);
        // every line after the block's first newline is whole; before it, a
        // line goes on in the earlier block (unless the file starts here)
        let first = if start == 0 { 0 } else { block.iter().position(|b| *b == b'\n').map_or(block.len(), |i| i + 1) };
        for line in block[first..].split(|b| *b == b'\n').rev().filter(|l| !l.is_empty()) {
            if let Some(t) = f(&String::from_utf8_lossy(line)) {
                return Some(t);
            }
        }
        tail = block[..first].to_vec();
        end = start;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // the backward read sees each line once, newest first, across blocks
    // and up to `upto` only
    #[test]
    fn find_back_reads_lines_newest_first_across_blocks() {
        let p = std::env::temp_dir().join(format!("sb-find-back-{}-{}", std::process::id(), crate::util::now_ms()));
        let lines: Vec<String> = (0..20_000).map(|i| format!("line {i:05} {}", "x".repeat(i % 37))).collect();
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        let mut seen = Vec::new();
        let none: Option<()> = find_back(&p, u64::MAX, |l| {
            seen.push(l.to_string());
            None
        });
        assert!(none.is_none());
        seen.reverse();
        assert_eq!(seen, lines, "every line once, in the file's order once reversed");
        let upto = lines[..100].iter().map(|l| l.len() as u64 + 1).sum::<u64>();
        assert_eq!(find_back(&p, upto, |l| Some(l.to_string())).as_deref(), Some(lines[99].as_str()));
        assert_eq!(find_back(&p, u64::MAX, |l| l.starts_with("line 00007 ").then(|| l.to_string())).as_deref(), Some(lines[7].as_str()));
        let _ = std::fs::remove_file(&p);
    }

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
    fn tool_results_and_compactions() {
        assert_eq!(parse("tool_result #3 ok : done"), Wire::ToolResult);
        assert_eq!(parse("tool_result #4 fail : exit 1"), Wire::ToolResult);
        assert_eq!(parse("  obs: compaction_started #750 (auto)"), Wire::CompactionStarted);
        assert_eq!(parse("  obs: compaction_done: ## Who I am"), Wire::CompactionEnded);
        assert_eq!(parse("  obs: context_compaction_failed: too big"), Wire::CompactionEnded);
        assert_eq!(parse("history tool_result #3 ok : done"), Wire::History);
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
