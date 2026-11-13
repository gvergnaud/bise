//! tasks (docs/tasks.md; architect m_64423, m_64740): a land's commit
//! names the works its turn served, one `Bise-Task: <id>` git trailer per
//! work. Ids only, never names (an id stays on a rename; nothing parses a
//! name back). The one writer of these trailers: a reader, if one comes,
//! lives here next to it, on the same rows.
//!
//! As `git interpret-trailers` writes them: a message with no trailer
//! block gets a blank line, then the block; a message whose last
//! paragraph is a trailer block (`Co-authored-by:`, `Signed-off-by:`)
//! gets its lines added to that block, with no second blank line. A work
//! already named is never written twice. An empty message stays empty
//! (the land refuses it for its own reason).

/// The trailer key.
pub const KEY: &str = "Bise-Task";

/// A trailer line: `Token: value`, the token letters, digits and dashes.
fn is_trailer(line: &str) -> bool {
    let Some((token, value)) = line.split_once(": ") else { return false };
    !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !value.trim().is_empty()
}

/// `message` with a `Bise-Task: <id>` trailer for each of `works` (in
/// their order, once each, none it already has).
pub fn with_tasks(message: &str, works: &[String]) -> String {
    let message = message.trim_end();
    let mut new: Vec<String> = Vec::new();
    for w in works {
        let line = format!("{KEY}: {w}");
        if !message.lines().any(|l| l.trim() == line) && !new.contains(&line) {
            new.push(line);
        }
    }
    if new.is_empty() || message.trim().is_empty() {
        return message.to_string();
    }
    // the last paragraph is a trailer block when it follows the subject
    // and every line of it is a trailer
    let block = message.rsplit_once("\n\n").map(|(_, last)| last).filter(|last| last.lines().all(is_trailer));
    let sep = if block.is_some() { "\n" } else { "\n\n" };
    format!("{message}{sep}{}", new.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_message_with_no_trailers_gets_a_blank_line_then_the_block() {
        assert_eq!(with_tasks("feat: the icon", &w(&["native-mac", "digest"])), "feat: the icon\n\nBise-Task: native-mac\nBise-Task: digest");
        assert_eq!(with_tasks("feat: the icon\n\nwhy it changed.\n", &w(&["native-mac"])), "feat: the icon\n\nwhy it changed.\n\nBise-Task: native-mac");
    }

    #[test]
    fn a_trailer_block_gets_its_lines_with_no_second_blank_line() {
        let m = "feat: the icon\n\nCo-authored-by: Ann <ann@x.dev>\nSigned-off-by: Bob <bob@x.dev>";
        assert_eq!(with_tasks(m, &w(&["native-mac"])), format!("{m}\nBise-Task: native-mac"));
    }

    #[test]
    fn the_subject_alone_is_never_a_trailer_block() {
        assert_eq!(with_tasks("fix: x", &w(&["a"])), "fix: x\n\nBise-Task: a");
    }

    #[test]
    fn a_work_is_never_written_twice() {
        let m = "feat: the icon\n\nBise-Task: native-mac";
        assert_eq!(with_tasks(m, &w(&["native-mac", "digest", "digest"])), format!("{m}\nBise-Task: digest"));
        assert_eq!(with_tasks(m, &w(&["native-mac"])), m);
    }

    #[test]
    fn no_work_or_no_message_changes_nothing() {
        assert_eq!(with_tasks("feat: the icon", &[]), "feat: the icon");
        assert_eq!(with_tasks("  ", &w(&["a"])), "  ".trim_end());
    }
}
