//! A file-opened view's file on disk (docs/plugin-apps.md A4): where a
//! handle's [`FileTarget`] is, and its bytes. No `Shell`, no loop: the
//! hub's threads, the pages server's `/h/` route and `view/open` call these
//! with the artifact [`Store`] and `Home::bise_root()`. The rules are
//! `crate::view_files`' (pure); the atomic write is `bise_peer::fsx`.
//!
//! - [`resolve_artifact`]: an artifact (its newest version, or the one
//!   asked) as `view/open` binds it;
//! - [`file_at`]: where a target's bytes are now: an artifact version's
//!   copy (the newest one's file itself when the store kept no copy and it
//!   is unchanged), a root file re-checked under the root (a symlink out
//!   since: [`ViewError::BadHandle`]; gone: [`ViewError::Gone`]);
//! - [`read`]: a window of those bytes ([`crate::view_files::window`]);
//! - [`mime`]: a file's type by its ending.

use crate::artifacts::{Meta, Store, Version};
use crate::view_files::{FileAt, FileTarget, Kind, Resolved};
use bise_proto::view_rpc::ViewError;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The artifact `id` (its version `version`, else its newest) as
/// `view/open` binds it: its target, name, kind and size. An unknown id, a
/// link or a version it never had: [`ViewError::BadHandle`] (one answer,
/// as a handle's).
pub fn resolve_artifact(store: &Store, id: &str, version: Option<u64>) -> Result<Resolved, ViewError> {
    let m = store.get(id).filter(|m| !m.source.is_empty() && Path::new(&m.source).is_absolute()).ok_or(ViewError::BadHandle)?;
    let v = match version {
        Some(n) => m.versions.iter().find(|v| v.v == n),
        None => m.current(),
    }
    .ok_or(ViewError::BadHandle)?;
    let target = FileTarget::Artifact { id: m.id.clone(), version: v.v };
    let at = artifact_at(store, &m, v)?;
    let name = Path::new(&m.source).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let kind = match crate::artifacts::names_of(&at.path) {
        // a folder: its first-level names, the one rule a version keeps them by (artifacts::folder_names)
        (names, more) if at.path.is_dir() => Kind::Folder { names, more },
        _ => Kind::File,
    };
    let older = m.current().is_some_and(|c| c.v != v.v);
    Ok(Resolved { target, name, kind, size: at.size, documents_of: None, older })
}

/// What `view/open` opens, resolved (off the loop): an artifact
/// ([`resolve_artifact`]); a `folder/list` entry needs the home hub's id
/// table and is refused until it lands (`bad-handle`); an unknown kind is
/// a bad request ([`ViewError::BadHandle`] too: nothing names a file but
/// an id the hub gave).
pub fn resolve(store: &Store, _root: &Path, target: &bise_proto::files::OpenTarget) -> Result<Resolved, ViewError> {
    use bise_proto::files::OpenTarget;
    match target {
        OpenTarget::Artifact { id, version } => resolve_artifact(store, id, *version),
        OpenTarget::Entry { .. } | OpenTarget::Unknown => Err(ViewError::BadHandle),
    }
}

/// Whether the view of `b` may write its file now: its `write` grant
/// (read fresh from `plugins.json`: he may have taken it back), and not
/// an older version. A read's `_meta.writable` and every write ask it.
pub fn may_write(b: &crate::view_files::Binding) -> Result<(), bise_proto::view_rpc::NoWrite> {
    use bise_proto::view_rpc::NoWrite;
    let grants = bend_plugins::state::grants(&bend_plugins::state::state_path());
    if !bend_plugins::state::view_allows(&grants, &b.opener.plugin, &b.opener.uri, bise_proto::view_rules::ViewPerm::Write) {
        return Err(NoWrite::NoGrant);
    }
    if b.older {
        return Err(NoWrite::OlderVersion);
    }
    Ok(())
}


/// Where `target`'s bytes are now. `root`: `Home::bise_root()`.
pub fn file_at(store: &Store, root: &Path, target: &FileTarget) -> Result<FileAt, ViewError> {
    match target {
        FileTarget::Artifact { id, version } => {
            let m = store.get(id).ok_or(ViewError::Gone)?;
            let v = m.versions.iter().find(|v| v.v == *version).ok_or(ViewError::Gone)?;
            artifact_at(store, &m, v)
        }
        FileTarget::RootFile { path } => root_at(root, path),
    }
}

/// An artifact version's bytes: bise's copy, else (the newest version, no
/// copy kept: over the store's cap) its file while it is unchanged.
fn artifact_at(store: &Store, m: &Meta, v: &Version) -> Result<FileAt, ViewError> {
    let etag = crate::view_files::artifact_etag(v.v, v.rev());
    let newest = m.current().is_some_and(|c| c.v == v.v);
    let path = match store.copy_path(m, v).filter(|p| p.exists()) {
        Some(p) => p,
        None if newest && crate::artifacts::file_sig(Path::new(&m.source)).as_deref() == Some(v.sig.as_str()) => PathBuf::from(&m.source),
        None => return Err(ViewError::Gone),
    };
    let size = std::fs::metadata(&path).map_err(|_| ViewError::Gone)?.len();
    Ok(FileAt { mime: mime(&path).to_string(), path, size, etag })
}

/// A root file, re-checked: its realpath still under the root's, a file.
fn root_at(root: &Path, path: &Path) -> Result<FileAt, ViewError> {
    let root = root.canonicalize().map_err(|_| ViewError::Gone)?;
    let real = path.canonicalize().map_err(|_| ViewError::Gone)?;
    if !real.starts_with(&root) {
        return Err(ViewError::BadHandle);
    }
    let meta = std::fs::metadata(&real).map_err(|_| ViewError::Gone)?;
    if !meta.is_file() {
        return Err(ViewError::Gone);
    }
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
    // an HTTP validator for /h/ (a read's etag is the content hash)
    let etag = format!("{:x}-{mtime:x}", meta.len());
    Ok(FileAt { mime: mime(&real).to_string(), path: real, size: meta.len(), etag })
}

/// The bytes of `at` in the window `(offset, length)`.
pub fn read(at: &FileAt, offset: u64, length: u64) -> Result<Vec<u8>, ViewError> {
    let mut f = std::fs::File::open(&at.path).map_err(|_| ViewError::Gone)?;
    f.seek(SeekFrom::Start(offset)).map_err(|_| ViewError::Gone)?;
    let mut out = Vec::with_capacity(length as usize);
    f.take(length).read_to_end(&mut out).map_err(|_| ViewError::Gone)?;
    Ok(out)
}

/// A file's MIME type by its ending; unknown: `application/octet-stream`.
pub fn mime(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

/// Whether a read of `mime` answers `text` (else a base64 `blob`).
pub fn is_text(mime: &str) -> bool {
    mime.starts_with("text/") || mime == "application/json" || mime == "image/svg+xml"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-vfd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn a_root_file_is_rechecked_under_the_root_on_every_use() {
        let d = tmp("root");
        let (root, out) = (d.join("bise"), d.join("elsewhere"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(root.join("a.md"), b"# hi").unwrap();
        std::fs::write(out.join("secret.txt"), b"no").unwrap();
        let at = root_at(&root, &root.join("a.md")).unwrap();
        assert_eq!((at.size, at.mime.as_str()), (4, "text/markdown"));
        assert_eq!(read(&at, 2, 10).unwrap(), b"hi");
        // a symlink out of the root, made after the open
        std::os::unix::fs::symlink(out.join("secret.txt"), root.join("b.md")).unwrap();
        assert_eq!(root_at(&root, &root.join("b.md")).err(), Some(ViewError::BadHandle));
        assert_eq!(root_at(&root, &out.join("secret.txt")).err(), Some(ViewError::BadHandle));
        std::fs::rename(root.join("a.md"), root.join("c.md")).unwrap();
        assert_eq!(root_at(&root, &root.join("a.md")).err(), Some(ViewError::Gone), "renamed: gone");
        std::fs::create_dir_all(root.join("dir")).unwrap();
        assert_eq!(root_at(&root, &root.join("dir")).err(), Some(ViewError::Gone), "a folder is no file");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_artifact_reads_its_versions_copy_and_names_it() {
        let d = tmp("art");
        let work = d.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let f = work.join("q3.csv");
        std::fs::write(&f, b"a,b\n").unwrap();
        let store = Store::new(&d.join("state"));
        let add = crate::artifacts::Add { target: f.to_string_lossy().to_string(), agent: "perf".into(), by: "perf".into(), cwd: work.to_string_lossy().to_string(), ..Default::default() };
        let id = store.add(&add, 10).unwrap().meta.id;
        std::fs::write(&f, b"a,b\n1,2\n").unwrap();
        store.add(&add, 20).unwrap();
        let r = resolve_artifact(&store, &id, None).unwrap();
        assert_eq!((r.target.clone(), r.name.as_str(), r.size), (FileTarget::Artifact { id: id.clone(), version: 2 }, "q3.csv", 8));
        let v1 = file_at(&store, &d, &FileTarget::Artifact { id: id.clone(), version: 1 }).unwrap();
        assert_eq!((read(&v1, 0, 100).unwrap(), v1.etag.as_str(), v1.mime.as_str()), (b"a,b\n".to_vec(), "1.1", "text/csv"));
        assert_eq!(resolve_artifact(&store, &id, Some(7)).err(), Some(ViewError::BadHandle), "a version it never had");
        assert_eq!(resolve_artifact(&store, "nope", None).err(), Some(ViewError::BadHandle));
        assert_eq!(file_at(&store, &d, &FileTarget::Artifact { id: "nope".into(), version: 1 }).err(), Some(ViewError::Gone));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn types_by_ending() {
        assert_eq!(mime(Path::new("a/B.XLSX")), "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
        assert_eq!(mime(Path::new("noext")), "application/octet-stream");
        assert!(is_text("text/markdown") && is_text("application/json") && !is_text("video/mp4"));
    }
}
