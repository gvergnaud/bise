//! Slash lines that both the TUI and the hub read (architect m_11359):
//! one parse each, so the TUI's own arm and the hub's `slash` command
//! (switchboard proto_view::slash) can't drift apart.

/// What `/artifacts …` asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Artifacts {
    /// `/artifacts` (or anything after it but `add`): the list
    List,
    /// `/artifacts add <path or link>`: that target, trimmed
    Add(String),
    /// `/artifacts add` with nothing after it: [`ARTIFACTS_ADD`]
    Usage,
}

/// The usage of `/artifacts add`.
pub const ARTIFACTS_ADD: &str = "usage: /artifacts add <path or link>";

/// A line whose first word is `/artifacts`, read once for both sides.
/// `None` when it is another line.
pub fn artifacts(line: &str) -> Option<Artifacts> {
    let rest = line.trim().strip_prefix("/artifacts")?;
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    let rest = rest.trim();
    Some(match rest.strip_prefix("add") {
        Some(target) if target.is_empty() || target.starts_with(char::is_whitespace) => match target.trim() {
            "" => Artifacts::Usage,
            t => Artifacts::Add(t.to_string()),
        },
        _ => Artifacts::List,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Law: the TUI and the hub read `/artifacts` the same way: add with a
    /// target (spaces kept), add alone is the usage, anything else the list.
    #[test]
    fn artifacts_lines_read_once() {
        assert_eq!(artifacts("/artifacts"), Some(Artifacts::List));
        assert_eq!(artifacts("  /artifacts  "), Some(Artifacts::List));
        assert_eq!(artifacts("/artifacts add https://x.dev/a b"), Some(Artifacts::Add("https://x.dev/a b".into())));
        assert_eq!(artifacts("/artifacts add   ./notes.md  "), Some(Artifacts::Add("./notes.md".into())));
        assert_eq!(artifacts("/artifacts add"), Some(Artifacts::Usage));
        assert_eq!(artifacts("/artifacts add   "), Some(Artifacts::Usage));
        assert_eq!(artifacts("/artifacts addx"), Some(Artifacts::List));
        assert_eq!(artifacts("/artifacts foo"), Some(Artifacts::List));
        assert_eq!(artifacts("/artifactsx"), None);
        assert_eq!(artifacts("/diff"), None);
    }
}
