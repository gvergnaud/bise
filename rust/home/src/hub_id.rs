//! The id of a workspace's hub: `<name>-<8 hex>`, the folder name of its
//! state (`Home::hub_dir`) and of its task worktrees. One owner: the
//! projects registry, `switchboard::paths`, the CLI and the ambient core
//! all ask [`hub_id`]. The Python copies (tests/gate.sh `new`/`done`) are
//! pinned by `paths::tests::ids_match_the_python_copies`.

use std::path::Path;

/// FNV-1a, 64 bits.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The hub id of `workspace`: its folder name (32 chars at most, other
/// than `[A-Za-z0-9_-]` turned to `-`; `root` for `/`), a dash, then the
/// low 32 bits of the FNV-1a hash of the whole path.
pub fn hub_id(workspace: &Path) -> String {
    let base = workspace.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "root".to_string());
    let base: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .take(32)
        .collect();
    format!("{}-{:08x}", base, fnv1a(&workspace.to_string_lossy()) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_the_folder_and_a_hash_of_the_path() {
        assert_eq!(hub_id(Path::new("/Users/me/lab/harness")), "harness-af1b2326");
        assert_eq!(hub_id(Path::new("/tmp/my repo")), "my-repo-b50e38fe");
        assert_eq!(hub_id(Path::new("/")), "root-860189fe");
        assert_ne!(hub_id(Path::new("/Users/me/my repo")), hub_id(Path::new("/Users/you/my repo")));
    }
}
