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
//!
//! [`jail`] is the other fence, for whole test runs (the desktop harness,
//! main m_9883, architect m_9892): with `BISE_TEST_HOME` set, every bise
//! of the run refuses a workspace, a state dir or a BISE_HOME outside it,
//! and refuses outright when it is his real home. A desktop test run at
//! 22:33 had his real HOME: its `bise ambient-core --home` started his real
//! global hub (`client::start_hub`, a detached process group that launchd
//! adopted when the core exited). Under `BISE_TEST_HOME` that start, the
//! version switch's, the cross-hub start and sbd itself call [`jail`]
//! first.

use std::path::{Component, Path, PathBuf};

/// The jail of a test run: set by its harness (the desktop app sets it to
/// its test HOME), passed to every hub (a `Test` variable).
pub const JAIL_VAR: &str = "BISE_TEST_HOME";

/// Under `BISE_TEST_HOME`: Ok only when `path` is inside it (and it isn't
/// his real home). Unset: Ok, nothing to check. Err: one line saying why.
pub fn jail(path: &Path) -> Result<(), String> {
    match crate::env::test_setting(JAIL_VAR) {
        None => Ok(()),
        Some(j) => jail_in(path, Path::new(&j), &real_home().ok_or_else(|| "no home in the user database".to_string())?),
    }
}

/// The pure rule of [`jail`]: both sides resolved first (symlinks, macOS's
/// /var -> /private/var); a side that can't be resolved is refused (fail
/// closed).
pub fn jail_in(path: &Path, jail: &Path, real_home: &Path) -> Result<(), String> {
    let j = resolved(jail).ok_or_else(|| format!("{JAIL_VAR} ({}) can't be resolved", jail.display()))?;
    let home = resolved(real_home).unwrap_or_else(|| real_home.to_path_buf());
    if home.starts_with(&j) {
        return Err(format!("{JAIL_VAR} ({}) is his real home or holds it ({})", jail.display(), home.display()));
    }
    let p = resolved(path).ok_or_else(|| format!("{} can't be resolved under {JAIL_VAR}", path.display()))?;
    if p.starts_with(&j) {
        Ok(())
    } else {
        Err(format!("{} is outside {JAIL_VAR} ({})", path.display(), jail.display()))
    }
}

/// `p` absolute with its existing part canonicalized and the rest (not
/// made yet) appended; None for a relative path, a `..` in the rest, or
/// nothing that exists.
fn resolved(p: &Path) -> Option<PathBuf> {
    if !p.is_absolute() {
        return None;
    }
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    let mut at = p;
    loop {
        if let Ok(c) = at.canonicalize() {
            let mut out = c;
            for r in rest.iter().rev() {
                out.push(r);
            }
            return Some(out);
        }
        let name = match at.components().next_back()? {
            Component::Normal(n) => n,
            _ => return None,
        };
        rest.push(name);
        at = at.parent()?;
    }
}

/// His home from the user database (getpwuid), which HOME can't fake.
pub fn real_home() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0 as libc::c_char; 16 * 1024];
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: the buffers outlive the call; out is null or points at pw
    let rc = unsafe { libc::getpwuid_r(libc::getuid(), &mut pw, buf.as_mut_ptr(), buf.len(), &mut out) };
    if rc != 0 || out.is_null() || pw.pw_dir.is_null() {
        return None;
    }
    // SAFETY: pw_dir is a NUL-terminated string inside buf
    let dir = unsafe { CStr::from_ptr(pw.pw_dir) };
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes())))
}

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
    // a test run's own inputs stay (a bench's journal, the gate's
    // sb-core: env::Var::kept_in_tests)
    let place: Vec<_> = std::env::vars_os()
        .filter_map(|(k, _)| k.to_str().filter(|k| is_place_var(k) && !crate::env::kept_in_tests(k)).map(String::from))
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

#[cfg(test)]
mod jail_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-jail-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn unset_jails_nothing() {
        // the test binary runs on a temp HOME with every BISE_* unset
        assert!(jail(Path::new("/anywhere/at/all")).is_ok());
    }

    #[test]
    fn inside_passes_outside_is_refused() {
        let j = tmp("in");
        let real = Path::new("/Users/nobody-real");
        assert!(jail_in(&j.join("bise"), &j, real).is_ok(), "a workspace not made yet, inside");
        assert!(jail_in(&j.join(".bise/hubs/x"), &j, real).is_ok());
        let e = jail_in(Path::new("/Users/nobody-real/bise"), &j, real).unwrap_err();
        assert!(e.contains("outside"), "{e}");
        assert!(jail_in(&j.join("a/../../escape"), &j, real).is_err(), "a .. in the part not made yet");
        assert!(jail_in(Path::new("relative/ws"), &j, real).is_err());
    }

    #[test]
    fn his_real_home_is_refused_even_as_the_jail() {
        let j = tmp("real");
        let e = jail_in(&j.join("bise"), &j, &j).unwrap_err();
        assert!(e.contains("real home"), "{e}");
        // a jail that holds his home: refused too
        let e = jail_in(&j.join("x"), &j, &j.join("me")).unwrap_err();
        assert!(e.contains("real home"), "{e}");
    }

    #[test]
    fn the_var_alias_of_macos_is_the_same_folder() {
        let j = tmp("alias");
        let canon = j.canonicalize().unwrap();
        // on macOS the temp dir is under /var, which is /private/var
        assert!(jail_in(&canon.join("ws"), &j, Path::new("/Users/nobody-real")).is_ok());
        assert!(jail_in(&j.join("ws"), &canon, Path::new("/Users/nobody-real")).is_ok());
    }

    #[test]
    fn a_symlink_that_escapes_is_outside() {
        let j = tmp("link");
        let out = tmp("link-out");
        std::os::unix::fs::symlink(&out, j.join("door")).unwrap();
        let e = jail_in(&j.join("door/ws"), &j, Path::new("/Users/nobody-real")).unwrap_err();
        assert!(e.contains("outside"), "{e}");
    }

    #[test]
    fn a_jail_that_does_not_resolve_refuses_everything() {
        let real = Path::new("/Users/nobody-real");
        assert!(jail_in(Path::new("/tmp/x"), Path::new("relative"), real).is_err());
    }

    #[test]
    fn the_user_database_names_a_home() {
        assert!(real_home().is_some_and(|h| h.is_absolute()));
    }
}
