//! Unix socket paths that fit (macOS: `sun_path` is 104 bytes, NUL
//! included).
//!
//! A socket keeps its natural place (`<hub dir>/hub.sock`,
//! `~/.bise/run/computer-use.sock`) when that path fits. When it does not
//! (a long `HOME`, a long project folder name), the socket is reached
//! through a short folder: `/tmp/bise-<uid>/<16 hex>/<file name>`, where
//! `<16 hex>` is a hash of the natural folder and is a symlink to it. The
//! socket file itself stays in the natural folder (bind and connect go
//! through the link), so `hub.pid` next to it, `exists()` checks on the
//! natural path and the state's moves keep working. Every client derives
//! the same short path from the same natural path ([`socket_path`]); the
//! side that binds makes the link first ([`prepare_socket`]).

use std::path::{Path, PathBuf};

/// The longest unix socket path (macOS: `sun_path` is 104 bytes, NUL
/// included).
pub const SOCKET_PATH_MAX: usize = 103;

/// Whether `p` fits in a unix socket address.
pub fn fits(p: &Path) -> bool {
    p.as_os_str().len() <= SOCKET_PATH_MAX
}

/// FNV-1a, 64 bits.
fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The private folder of the short links: `/tmp/bise-<uid>`.
pub fn short_root() -> PathBuf {
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/bise-{}", uid))
}

/// The short path for `natural` under `root`: `<root>/<16 hex of its
/// folder>/<its file name>`.
pub fn short_path_in(root: &Path, natural: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let dir = natural.parent().unwrap_or(Path::new("/"));
    let name = natural.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "s.sock".into());
    root.join(format!("{:016x}", fnv1a(dir.as_os_str().as_bytes()))).join(name)
}

/// Where a socket whose natural place is `natural` is bound and reached:
/// `natural` when it fits, else its short path under [`short_root`].
/// Pure: the link is made by [`prepare_socket`].
pub fn socket_path(natural: &Path) -> PathBuf {
    if fits(natural) {
        natural.to_path_buf()
    } else {
        short_path_in(&short_root(), natural)
    }
}

/// Make [`socket_path`] usable before a bind: nothing when `natural`
/// fits; else the private root (0700, ours, a real folder) and the link
/// from the short folder to `natural`'s folder (made or fixed). Returns
/// the path to bind.
pub fn prepare_socket(natural: &Path) -> std::io::Result<PathBuf> {
    if fits(natural) {
        return Ok(natural.to_path_buf());
    }
    prepare_socket_in(&short_root(), natural)
}

/// [`prepare_socket`] under `root` (tests use a temp root).
pub fn prepare_socket_in(root: &Path, natural: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let err = |m: String| std::io::Error::other(m);
    let short = short_path_in(root, natural);
    if !fits(&short) {
        return Err(err(format!("{} is too long for a unix socket, even the short {}", natural.display(), short.display())));
    }
    match std::fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(err(format!("cannot create {}: {}", root.display(), e))),
    }
    // Another user could have made it first (in /tmp): only ours, a real
    // folder, private.
    let m = std::fs::symlink_metadata(root)?;
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    if !m.is_dir() || m.uid() != uid {
        return Err(err(format!("{} is not a folder of yours: remove it", root.display())));
    }
    if m.mode() & 0o077 != 0 {
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    }
    let dir = natural.parent().unwrap_or(Path::new("/"));
    let link = short.parent().expect("short path has a folder");
    if std::fs::read_link(link).is_ok_and(|t| t == dir) {
        return Ok(short);
    }
    // A stale link (or anything else) under our private root: replace it
    // atomically.
    let tmp = root.join(format!(".{}.{}", link.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(dir, &tmp)?;
    std::fs::rename(&tmp, link).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(short)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::{UnixListener, UnixStream};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-sock-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn a_path_that_fits_stays() {
        let p = Path::new("/Users/me/.bise/hubs/harness-af1b2326/hub.sock");
        assert_eq!(socket_path(p), p);
        assert_eq!(prepare_socket(p).unwrap(), p);
        let edge = PathBuf::from(format!("/{}", "a".repeat(SOCKET_PATH_MAX - 1)));
        assert_eq!(socket_path(&edge), edge);
    }

    /// The user's case: a mktemp HOME under /var/folders and a hub named
    /// after a temp project folder (107 bytes).
    #[test]
    fn a_long_path_gets_a_short_stable_one() {
        let p = Path::new("/var/folders/xy/abcdefghijklmnopqrstuvwxyz0123/T/tmp.AbCdEfGh/.bise/hubs/tmp-mkV4VlMCXd-ee85d2ab/hub.sock");
        assert!(!fits(p));
        let s = socket_path(p);
        assert!(fits(&s), "{}", s.display());
        assert_eq!(s, socket_path(p), "every client derives the same path");
        assert!(s.starts_with(short_root()));
        assert_eq!(s.file_name().unwrap(), "hub.sock");
        let other = Path::new("/var/folders/xy/abcdefghijklmnopqrstuvwxyz0123/T/tmp.AbCdEfGh/.bise/hubs/tmp-mkV4VlMCXd-ee85d2ac/hub.sock");
        assert_ne!(s, socket_path(other), "two hubs, two paths");
        assert_eq!(short_root(), PathBuf::from(format!("/tmp/bise-{}", unsafe { libc::getuid() })));
    }

    #[test]
    fn the_link_reaches_the_natural_folder_and_is_private() {
        let base = tmp("link");
        let dir = base.join("d".repeat(120));
        std::fs::create_dir_all(&dir).unwrap();
        let natural = dir.join("hub.sock");
        // a short root, like /tmp/bise-<uid> (an agent's TMPDIR is too deep)
        let root = PathBuf::from(format!("/tmp/bise-t{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(prepare_socket_in(&base.join("r".repeat(80)), &natural).is_err(), "a root too long is refused");
        let short = prepare_socket_in(&root, &natural).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
        let l = UnixListener::bind(&short).unwrap();
        assert!(natural.exists(), "the socket file is in the natural folder");
        UnixStream::connect(&short).unwrap();
        drop(l);
        // idempotent, and a stale link is fixed
        assert_eq!(prepare_socket_in(&root, &natural).unwrap(), short);
        std::fs::remove_file(short.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&base, short.parent().unwrap()).unwrap();
        prepare_socket_in(&root, &natural).unwrap();
        assert_eq!(std::fs::read_link(short.parent().unwrap()).unwrap(), dir);
        // a root that is a symlink is refused
        let evil = base.join("evil");
        std::os::unix::fs::symlink(&base, &evil).unwrap();
        assert!(prepare_socket_in(&evil, &natural).is_err());
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&root);
    }
}
