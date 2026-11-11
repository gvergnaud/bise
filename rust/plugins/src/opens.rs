//! Plugin apps (docs/plugin-apps.md A1, A2): the files a plugin's openers
//! open.
//!
//! - [`type_key`]: the one canonical form of a type, the key of his picks
//!   (`plugins.json` `openers`, [`crate::state::picks`]): Vibe's
//!   extension `.XLSX` and the glob `*.xlsx` are both `*.xlsx`. Pure.

/// The longest type pattern kept (a glob or an extension).
pub const MAX_PATTERN: usize = 128;

/// The canonical key of a type pattern, lowercase: an extension (`.xlsx`,
/// Vibe's `entrypoints[].extensions`) is the glob of its files
/// (`*.xlsx`); a glob (`*.board.json`, `*/index.html`,
/// `@documents/boards/*.board.json`) stays itself. None for a pattern no
/// entry may claim: empty, too long, a bare `*` or `**`, absolute, a
/// `..`, an empty segment, a backslash or a control character, an
/// `@documents` scope with nothing after it.
pub fn type_key(pattern: &str) -> Option<String> {
    let p = pattern.trim().to_lowercase();
    let p = match p.strip_prefix('.') {
        Some(ext) if !ext.is_empty() && !ext.contains(['/', '*', '?']) => format!("*.{ext}"),
        _ => p,
    };
    let body = p.strip_prefix("@documents/").unwrap_or(&p);
    let bad = p.is_empty()
        || p.len() > MAX_PATTERN
        || p.chars().any(|c| c.is_control() || c == '\\')
        || body.starts_with('/')
        || body.split('/').any(|s| s.is_empty() || s == "." || s == "..")
        || body.split('/').all(|s| s == "*" || s == "**");
    (!bad).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_and_its_glob_are_one_key() {
        assert_eq!(type_key(".xlsx").as_deref(), Some("*.xlsx"));
        assert_eq!(type_key(".XLSX").as_deref(), Some("*.xlsx"));
        assert_eq!(type_key("*.xlsx").as_deref(), Some("*.xlsx"));
        assert_eq!(type_key(" *.XLSX ").as_deref(), Some("*.xlsx"));
        assert_eq!(type_key(".board.json").as_deref(), Some("*.board.json"));
        assert_eq!(type_key("*/index.html").as_deref(), Some("*/index.html"));
        assert_eq!(type_key("@documents/boards/*.board.json").as_deref(), Some("@documents/boards/*.board.json"));
        assert_eq!(type_key("report-*.csv").as_deref(), Some("report-*.csv"));
    }

    #[test]
    fn a_pattern_no_entry_may_claim_has_no_key() {
        for p in ["", " ", ".", "*", "**", "*/*", "**/*", "/etc/*.conf", "../*.xlsx", "a/../*.x", "a//b.x", "@documents/", "@documents/*", "a\\b.x", "a\u{0}.x"] {
            assert_eq!(type_key(p), None, "{p:?}");
        }
        assert_eq!(type_key(&format!("*.{}", "x".repeat(MAX_PATTERN))), None);
    }
}
