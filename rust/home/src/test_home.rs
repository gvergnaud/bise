//! Tests never see the user's real HOME (fake data only).
//!
//! A test that reads `~/.vibe/skills`, `~/.agents/plugins` or
//! `~/.bise/config.toml` reads the user's own files: they change under it,
//! and from a launchd job a file there that links into `~/Documents` makes
//! `open()` wait on macOS's privacy prompt forever (it hung the release
//! gate twice). [`test_home!`] gives each test binary of a crate a temp
//! HOME before its first test runs: `HOME` points at an empty folder of
//! its own, and the variables that name bise's real places (`BISE_*`,
//! `BEND_*`, `SB_*`, `XDG_*_HOME`) are unset. A test that needs a home
//! still makes its own (an injected root or an env map).
//!
//! `CARGO_HOME` and `RUSTUP_HOME` keep the real ones (a test that runs
//! cargo still finds its toolchain): they are paths, nothing reads in them
//! but cargo.

use std::path::{Path, PathBuf};

/// The prefix of the temp homes, in the temp folder.
pub const PREFIX: &str = "bise-test-home-";

/// Whether `k` names one of bise's places (or the hub): unset in tests.
pub fn is_place_var(k: &str) -> bool {
    k.starts_with("BISE_")
        || k.starts_with("BEND_")
        || k.starts_with("SB_")
        || (k.starts_with("XDG_") && k.ends_with("_HOME"))
}

/// Points this process at a fresh temp HOME and unsets the place
/// variables. Called by [`test_home!`] before `main` (one thread, no test
/// running yet); never panics (a panic there aborts).
pub fn enter() {
    let real = std::env::var_os("HOME").map(PathBuf::from);
    let tmp = std::env::temp_dir();
    sweep(&tmp);
    let home = tmp.join(format!("{PREFIX}{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::create_dir_all(&home);
    if let Some(real) = real.filter(|r| !r.as_os_str().is_empty()) {
        for (var, dir) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            if std::env::var_os(var).is_none() {
                std::env::set_var(var, real.join(dir));
            }
        }
    }
    let place: Vec<_> = std::env::vars_os()
        .filter_map(|(k, _)| k.to_str().filter(|k| is_place_var(k)).map(String::from))
        .collect();
    for k in place {
        std::env::remove_var(k);
    }
    std::env::set_var("HOME", &home);
}

/// Whether this process runs on a temp HOME from [`enter`].
pub fn active() -> bool {
    std::env::var_os("HOME").map(PathBuf::from).is_some_and(|h| {
        h.parent() == Some(std::env::temp_dir().as_path())
            && h.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(PREFIX))
    })
}

/// Removes the temp homes of earlier runs (older than a day: a run of
/// another crate may still be using a newer one).
fn sweep(tmp: &Path) {
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in std::fs::read_dir(tmp).into_iter().flatten().flatten() {
        let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|a| a > day);
        if old && e.file_name().to_str().is_some_and(|n| n.starts_with(PREFIX)) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// In a crate root: its test binary runs on a temp HOME ([`enter`], before
/// any test). Nothing in a normal build.
#[macro_export]
macro_rules! test_home {
    () => {
        #[cfg(test)]
        #[used]
        #[cfg_attr(target_vendor = "apple", link_section = "__DATA,__mod_init_func")]
        #[cfg_attr(not(target_vendor = "apple"), link_section = ".init_array")]
        static BISE_TEST_HOME: extern "C" fn() = {
            extern "C" fn enter() {
                $crate::test_home::enter()
            }
            enter
        };
    };
}
