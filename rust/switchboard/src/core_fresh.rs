//! The tests refuse a stale sb-core (architect m_8480): a Rust test that
//! runs an sb-core built from other `bend/` sources than the tree's
//! passes or fails for the wrong reason (amb-home's S0b run: the queued-
//! input test failed on an sb-core built before that change).
//!
//! The tree's key is `scripts/bins.sh key sb-core` (its sources' hash),
//! asked once per test binary. The binary's key: the gate's cache file is
//! named `sb-core-<key>`; a copy `bins.sh` placed has `<bin>.key` next to
//! it. No key to compare (no bash or bend, an sb-core from elsewhere): no
//! check, as before. Test-only (`Hub::new`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The message of a stale sb-core.
pub const STALE: &str = "sb-core is stale: run scripts/bins.sh sb-core";

/// The key `bin` was built from: its name (`sb-core-<key>`, the cache's)
/// or its `<bin>.key` file (`key_file`'s text).
pub fn bin_key(bin: &Path, key_file: Option<&str>) -> Option<String> {
    let name = bin.file_name()?.to_string_lossy().to_string();
    match name.strip_prefix("sb-core-") {
        Some(k) if !k.is_empty() => Some(k.to_string()),
        _ => key_file.map(str::trim).filter(|k| !k.is_empty()).map(str::to_string),
    }
}

/// Stale: both keys known and different.
pub fn stale(bin_key: Option<&str>, tree_key: Option<&str>) -> bool {
    matches!((bin_key, tree_key), (Some(b), Some(t)) if b != t)
}

/// The tree's key, once: `scripts/bins.sh key sb-core` at the repo root.
fn tree_key() -> Option<String> {
    static KEY: OnceLock<Option<String>> = OnceLock::new();
    KEY.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::process::Command::new("bash").arg(root.join("scripts/bins.sh")).args(["key", "sb-core"]).output().ok()?;
        let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && !k.is_empty()).then_some(k)
    })
    .clone()
}

/// Panics with [`STALE`] when `bin` was built from other sources.
pub fn check(bin: &Path) {
    let file = std::fs::read_to_string(PathBuf::from(format!("{}.key", bin.display()))).ok();
    let b = bin_key(bin, file.as_deref());
    if b.is_none() {
        return;
    }
    if stale(b.as_deref(), tree_key().as_deref()) {
        panic!("{STALE} ({} was built from {}, the tree is {})", bin.display(), b.unwrap_or_default(), tree_key().unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binary_knows_its_key_by_name_or_key_file() {
        assert_eq!(bin_key(Path::new("/c/cache/sb-core-abc123"), None).as_deref(), Some("abc123"));
        assert_eq!(bin_key(Path::new("/repo/sb-core"), Some("def456\n")).as_deref(), Some("def456"));
        assert_eq!(bin_key(Path::new("/repo/sb-core"), None), None);
        assert_eq!(bin_key(Path::new("/repo/sb-core"), Some("  ")), None);
    }

    #[test]
    fn stale_only_when_both_keys_differ() {
        assert!(stale(Some("a"), Some("b")));
        assert!(!stale(Some("a"), Some("a")));
        assert!(!stale(None, Some("a")), "no key: no check");
        assert!(!stale(Some("a"), None), "no bins.sh: no check");
    }
}
