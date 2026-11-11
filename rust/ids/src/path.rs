//! The paths the ids are kept for: relative to the bise root, `/`-joined,
//! compared as bytes. One rule for the hub's walk, its `files/mint` and
//! the desktop core's Finder edge, so they never drift.
//!
//! - [`valid_rel`]: the shape of a relative path (`""` is the root);
//! - [`skipped`]: a name no walk records (hidden, an editor's temp);
//! - [`tracked`]: a path that gets an id: valid, not the root, no skipped
//!   component.
//!
//! Paths are never Unicode-normalised: `é` as one char and `e` + accent
//! are two names, as the disk says (architect m_60074).

/// A relative path under the root: `""` (the root itself), or names joined
/// by `/` with no empty name, no `.`, no `..`, no leading or trailing `/`
/// and no NUL.
pub fn valid_rel(rel: &str) -> bool {
    rel.is_empty()
        || rel
            .split('/')
            .all(|name| !name.is_empty() && name != "." && name != ".." && !name.contains('\0'))
}

/// A name the walk skips, and `files/mint` refuses: hidden (a leading
/// `.`: `.DS_Store`, `._x`, the shared write's `.<name>.bise-<pid>-<n>`),
/// an editor's backup (`x~`), Emacs's autosave (`#x#`), Office's lock
/// (`~$x`), or vim's write probe (digits only, 4913 + k*123).
pub fn skipped(name: &str) -> bool {
    name.starts_with('.')
        || name.ends_with('~')
        || (name.len() > 1 && name.starts_with('#') && name.ends_with('#'))
        || name.starts_with("~$")
        || vim_probe(name)
}

/// A path that gets an id: valid, not the root, and none of its names
/// skipped.
pub fn tracked(rel: &str) -> bool {
    !rel.is_empty() && valid_rel(rel) && !rel.split('/').any(skipped)
}

/// The folder that holds `rel` (`""` for a name at the root).
pub(crate) fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `rel` with its leading `from` replaced by `to`: `from` is `rel` or one
/// of its folders.
pub(crate) fn rebase(rel: &str, from: &str, to: &str) -> String {
    format!("{to}{}", &rel[from.len()..])
}

/// vim's `:w` probe: it creates `4913` (then 4913 + 123, ...) in the
/// folder to test that it can write there, and deletes it at once.
fn vim_probe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 19
        && name.bytes().all(|b| b.is_ascii_digit())
        && !name.starts_with('0')
        && name
            .parse::<u64>()
            .is_ok_and(|n| n >= 4913 && (n - 4913) % 123 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_rel_table() {
        let rows: &[(&str, bool)] = &[
            ("", true),
            ("a.md", true),
            ("notes/a.md", true),
            ("notes/deep/a.md", true),
            (".hidden", true), // valid shape; tracked() refuses it
            ("/a.md", false),
            ("a.md/", false),
            ("a//b", false),
            ("./a", false),
            ("a/./b", false),
            ("../a", false),
            ("a/..", false),
            ("a\0b", false),
            ("/", false),
        ];
        for (rel, want) in rows {
            assert_eq!(valid_rel(rel), *want, "valid_rel({rel:?})");
        }
    }

    #[test]
    fn skipped_table() {
        let rows: &[(&str, bool)] = &[
            ("a.md", false),
            (".DS_Store", true),
            ("._a.md", true),
            (".a.md.bise-123-4", true),
            ("a.md~", true),
            ("#a.md#", true),
            ("#", false),
            ("#tag", false),
            ("~$report.docx", true),
            ("~notes", false),
            ("4913", true),
            ("5036", true),  // 4913 + 123
            ("5037", false), // not on the probe's step
            ("4790", false), // below the first probe
            ("2024", false),
            ("04913", false),
            ("99999999999999999999999", false),
        ];
        for (name, want) in rows {
            assert_eq!(skipped(name), *want, "skipped({name:?})");
        }
    }

    #[test]
    fn tracked_table() {
        let rows: &[(&str, bool)] = &[
            ("", false),
            ("a.md", true),
            ("notes/a.md", true),
            (".git/config", false),
            ("notes/.a.md.bise-1-2", false),
            ("notes/a.md~", false),
            ("a/../b", false),
            ("/a.md", false),
            ("caf\u{e9}.md", true),
            ("cafe\u{301}.md", true),
        ];
        for (rel, want) in rows {
            assert_eq!(tracked(rel), *want, "tracked({rel:?})");
        }
    }

    #[test]
    fn parent_and_rebase() {
        assert_eq!(parent("a.md"), "");
        assert_eq!(parent("x/y/a.md"), "x/y");
        assert_eq!(rebase("x/y/a.md", "x", "z"), "z/y/a.md");
        assert_eq!(rebase("x", "x", "z/w"), "z/w");
    }
}
