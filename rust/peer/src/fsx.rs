//! One atomic replace of a file of `~/bise`, with its `ifMatch` (docs/
//! plugin-apps.md A4, architect m_59962 (a), m_60037): the hub's view
//! writes (a root file a view saves) and the desktop core's Finder writes
//! (shared-kb's File Provider domain) call this one code, never a copy.
//!
//! The write is split so the costly half runs off a loop:
//!
//! - [`prepare`]: the bytes to a temp file in the target's own folder
//!   (`.<name>.bise-<pid>-<n>`, so the rename never crosses a volume),
//!   hashed in the same pass, fsync'd. Its [`Prepared`] deletes the temp
//!   file when dropped uncommitted (a loser of the decide, an error).
//! - [`commit`]: cheap: the caller says what the file's hash is now (its
//!   cache keyed by size and mtime, or [`content_hash`]); `ifMatch` set and
//!   not that hash: a conflict that says it; set and no file: gone; else
//!   the temp renamed over it, its mode kept, the folder fsync'd.
//! - [`replace`]: both at once, for a caller with no loop to spare.
//!
//! The hash is sha256 of the bytes, 64 lowercase hex: a view's `etag` and
//! `ifMatch`, the same on both sides. Paths are the bytes on disk, never
//! Unicode-normalised (architect m_60075).
//!
//! One copy on main (architect m_61167): any change to fsx goes to main
//! first, never to a branch; the branches take it by rebasing.

use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A file's content hash: sha256, 64 lowercase hex, read in one pass.
pub fn content_hash(path: &Path) -> io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(hex(&h.finalize()));
        }
        h.update(&buf[..n]);
    }
}

/// The content hash of `bytes`.
pub fn hash_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// A file written beside its target, not in place yet.
#[derive(Debug)]
pub struct Prepared {
    /// the file it replaces
    pub path: PathBuf,
    /// the new bytes' content hash
    pub hash: String,
    pub size: u64,
    tmp: Option<PathBuf>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Some(t) = self.tmp.take() {
            let _ = std::fs::remove_file(t);
        }
    }
}

/// What a commit wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub hash: String,
    pub size: u64,
    pub mtime_ms: u64,
}

/// Why a commit wrote nothing.
#[derive(Debug)]
pub enum FsxErr {
    /// the file is not the one `ifMatch` named: `hash`, what it is now
    Conflict { hash: String },
    /// `ifMatch` named a file that is no more
    Gone,
    Io(io::Error),
}

impl From<io::Error> for FsxErr {
    fn from(e: io::Error) -> FsxErr {
        FsxErr::Io(e)
    }
}

impl std::fmt::Display for FsxErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsxErr::Conflict { hash } => write!(f, "it changed (now {hash})"),
            FsxErr::Gone => f.write_str("the file is gone"),
            FsxErr::Io(e) => write!(f, "{e}"),
        }
    }
}

static TMP: AtomicU64 = AtomicU64::new(0);

/// `content` to a temp file in `path`'s folder, hashed, fsync'd.
pub fn prepare(path: &Path, content: &mut impl Read) -> io::Result<Prepared> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no folder"))?;
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".bise-{}-{}", std::process::id(), TMP.fetch_add(1, Ordering::Relaxed)));
    let tmp = dir.join(tmp_name);
    let mut p = Prepared { path: path.to_path_buf(), hash: String::new(), size: 0, tmp: None };
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    p.tmp = Some(tmp);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = content.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        f.write_all(&buf[..n])?;
        p.size += n as u64;
    }
    f.sync_all()?;
    p.hash = hex(&h.finalize());
    Ok(p)
}

/// `p` in place of its file, unless `if_match` says otherwise. `now`: the
/// file's content hash now (None: no file there).
pub fn commit(mut p: Prepared, if_match: Option<&str>, now: Option<&str>) -> Result<Written, FsxErr> {
    match (if_match, now) {
        (Some(_), None) => return Err(FsxErr::Gone),
        (Some(m), Some(n)) if m != n => return Err(FsxErr::Conflict { hash: n.to_string() }),
        _ => {}
    }
    let tmp = p.tmp.clone().ok_or_else(|| io::Error::other("already committed"))?;
    if let Ok(meta) = std::fs::metadata(&p.path) {
        std::fs::set_permissions(&tmp, meta.permissions())?;
    }
    std::fs::rename(&tmp, &p.path)?;
    p.tmp = None;
    if let Some(dir) = p.path.parent() {
        // the rename itself durable (best effort: some file systems refuse)
        let _ = std::fs::File::open(dir).and_then(|d| d.sync_all());
    }
    let mtime_ms = std::fs::metadata(&p.path)?.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
    Ok(Written { hash: std::mem::take(&mut p.hash), size: p.size, mtime_ms })
}

/// [`prepare`] then [`commit`] against the file's hash now.
pub fn replace(path: &Path, content: &mut impl Read, if_match: Option<&str>) -> Result<Written, FsxErr> {
    let p = prepare(path, content)?;
    let now = match content_hash(path) {
        Ok(h) => Some(h),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    commit(p, if_match, now.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-fsx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn temps(d: &Path) -> Vec<String> {
        std::fs::read_dir(d).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.contains(".bise-")).collect()
    }

    /// Both callers' cases (the hub's view write, the core's Finder
    /// write): what lands, what conflicts, and no temp file left.
    #[test]
    fn a_replace_writes_only_on_the_hash_it_was_given() {
        let d = dir("table");
        let f = d.join("q3 plan.md");
        let old = hash_bytes(b"old");
        // (what's on disk, ifMatch, the bytes) -> (outcome, what's on disk after)
        type Case<'a> = (Option<&'a [u8]>, Option<String>, &'a [u8], &'a str, Option<&'a [u8]>);
        let cases: [Case; 6] = [
            (Some(b"old"), Some(old.clone()), b"new", "written", Some(b"new")),
            (Some(b"old"), None, b"new", "written", Some(b"new")),
            (None, None, b"new", "written", Some(b"new")),
            (Some(b"other"), Some(old.clone()), b"new", "conflict", Some(b"other")),
            (None, Some(old.clone()), b"new", "gone", None),
            (Some(b"old"), Some(old.clone()), b"", "written", Some(b"")),
        ];
        for (i, (before, if_match, bytes, want, after)) in cases.into_iter().enumerate() {
            let _ = std::fs::remove_file(&f);
            if let Some(b) = before {
                std::fs::write(&f, b).unwrap();
            }
            let got = replace(&f, &mut &bytes[..], if_match.as_deref());
            let outcome = match &got {
                Ok(w) => {
                    assert_eq!((w.hash.as_str(), w.size), (hash_bytes(bytes).as_str(), bytes.len() as u64), "case {i}");
                    "written"
                }
                Err(FsxErr::Conflict { hash }) => {
                    assert_eq!(*hash, hash_bytes(before.unwrap()), "case {i}: says what it is now");
                    "conflict"
                }
                Err(FsxErr::Gone) => "gone",
                Err(FsxErr::Io(e)) => panic!("case {i}: {e}"),
            };
            assert_eq!(outcome, want, "case {i}");
            assert_eq!(std::fs::read(&f).ok().as_deref(), after, "case {i}");
            assert!(temps(&d).is_empty(), "case {i}: no temp file left: {:?}", temps(&d));
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_prepared_write_commits_by_the_callers_hash_and_keeps_the_mode() {
        let d = dir("split");
        let f = d.join("été.xlsx");
        std::fs::write(&f, b"v1").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
        let now = content_hash(&f).unwrap();
        assert_eq!(now, hash_bytes(b"v1"));
        let p = prepare(&f, &mut &b"v2"[..]).unwrap();
        assert_eq!(temps(&d).len(), 1, "beside its file");
        assert_eq!(std::fs::read(&f).unwrap(), b"v1", "nothing in place before the commit");
        let w = commit(p, Some(&now), Some(&now)).unwrap();
        assert_eq!((std::fs::read(&f).unwrap(), w.hash), (b"v2".to_vec(), hash_bytes(b"v2")));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o640);
        assert!(w.mtime_ms > 0);
        // a loser: its temp file goes
        let p = prepare(&f, &mut &b"v3"[..]).unwrap();
        assert!(matches!(commit(p, Some("stale"), Some(&hash_bytes(b"v2"))), Err(FsxErr::Conflict { .. })));
        let dropped = prepare(&f, &mut &b"v4"[..]).unwrap();
        drop(dropped);
        assert!(temps(&d).is_empty());
        assert_eq!(std::fs::read(&f).unwrap(), b"v2");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_hash_is_sha256_hex() {
        assert_eq!(hash_bytes(b"hello"), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }
}
