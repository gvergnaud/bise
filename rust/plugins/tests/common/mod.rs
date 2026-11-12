//! Shared by this crate's integration tests (each its own crate: `mod common;`).

use std::path::PathBuf;

/// The first python3 on PATH that is no version manager's shim (pyenv, asdf:
/// `.../shims/python3`), for the tests' fake servers. A pyenv shim is `pyenv exec` in bash:
/// under the full suite it took ~20 s to start the interpreter (loop audit, 2026-10-14), at
/// load ~30 10-17 s against the real one's 0.1-0.2 s (fast-loop, 2026-11-12), past every
/// start bound here. No other one: `python3` as found.
pub fn python3() -> PathBuf {
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .filter(|d| d.file_name().is_none_or(|n| n != "shims"))
        .map(|d| d.join("python3"))
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("python3"))
}
