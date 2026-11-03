//! The files an agent may attach to a web page (C1 `set_input_files`,
//! task cu-upload, architect m_52652). The broker runs outside the
//! agent's sandbox, so it must never send a file that sandbox denies the
//! agent: everything `bise_secrets::held` holds is refused (an agent that
//! can't `cat ~/.ssh/id_ed25519` can't attach it either), plus what only
//! an upload needs to keep: all of `~/.ssh` and `~/.bise`, the cloud and
//! tool credentials (`~/.aws`, `~/.config`...), `~/Library` (Keychains,
//! Cookies, Mail, Messages, app data) but iCloud Drive and the cloud
//! drives the user keeps documents in, the system's folders, and key or
//! keychain files by their names wherever they are.
//!
//! Every path is absolute, made real first (a link from Documents into
//! `~/.ssh` is judged where it leads) and the real path is what goes to
//! the browser. At most [`MAX_FILES`] files of [`MAX_BYTES`] each.
//! Pure but for the disk reads of `canonicalize` and `metadata`.

use std::path::{Component, Path, PathBuf};

use bise_secrets::held::{Held, Secrets};
use serde_json::Value;

use crate::proto::err;

/// The largest file an agent attaches.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// The most files in one call.
pub const MAX_FILES: usize = 20;

/// Under the user's home, refused whole (an upload's own list, on top of
/// the sandbox's): the keys and sign-ins of other tools, and bise's home.
const HOME_DENIED: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".docker",
    ".config",
    ".password-store",
    ".netrc",
    ".bise",
    "Library",
];
/// Inside `~/Library`, the folders that hold his documents: iCloud Drive,
/// and the cloud drives (Dropbox, Google Drive, OneDrive).
const LIBRARY_OPEN: &[&str] = &["Mobile Documents", "CloudStorage"];
/// Outside his home: the system's folders (real paths: `/etc` is
/// `/private/etc`).
const SYSTEM_DENIED: &[&str] = &[
    "/etc", "/private", "/System", "/Library", "/usr", "/bin", "/sbin", "/dev", "/var", "/cores",
    "/opt",
];
/// `/private`'s one shared folder a user's file can be in.
const SYSTEM_OPEN: &[&str] = &["/private/tmp"];
/// File names that are keys or keychains wherever they are.
const KEY_PREFIXES: &[&str] = &["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"];
const KEY_SUFFIXES: &[&str] = &[".keychain", ".keychain-db"];
const KEY_NAMES: &[&str] = &[".netrc", ".env", ".pgpass", ".git-credentials"];

/// One file that may go: its real path, its name, its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
}

/// Where the user's and bise's homes are (real paths are taken here).
#[derive(Clone, Debug)]
pub struct Homes {
    pub home: PathBuf,
    pub bise: PathBuf,
}

impl Homes {
    fn real(&self) -> (PathBuf, PathBuf) {
        let r = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        (r(&self.home), r(&self.bise))
    }
}

/// The files of a `set_input_files` call, checked; a C1 error otherwise.
pub fn check(files: &Value, homes: &Homes) -> Result<Vec<File>, Value> {
    let list: Vec<&str> = match files {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| err("bad_args", "files is a list of absolute paths (strings)"))
            })
            .collect::<Result<_, _>>()?,
        _ => {
            return Err(err(
                "bad_args",
                "set_input_files needs files: a list of absolute paths",
            ))
        }
    };
    if list.is_empty() {
        return Err(err(
            "bad_args",
            "files is empty: give at least one absolute path",
        ));
    }
    if list.len() > MAX_FILES {
        return Err(err(
            "bad_args",
            format!(
                "{} files in one call; at most {}: attach them in several calls",
                list.len(),
                MAX_FILES
            ),
        ));
    }
    let (home, bise) = homes.real();
    let secrets = Secrets::of(&bise, &home);
    list.iter()
        .map(|p| one(p, &home, &bise, &secrets))
        .collect()
}

fn one(raw: &str, home: &Path, bise: &Path, secrets: &Secrets) -> Result<File, Value> {
    let p = Path::new(raw);
    if !p.is_absolute() {
        return Err(err("bad_args", format!("{:?} isn't an absolute path; give the full path (/Users/…/file.pdf; ~ isn't expanded)", raw)));
    }
    let real = std::fs::canonicalize(p).map_err(|_| {
        err(
            "file_not_found",
            format!("{} doesn't exist; check the path (ls in bash)", raw),
        )
    })?;
    // the written path and the one it leads to: either may be a secret
    for q in [p, real.as_path()] {
        if let Some(why) = refused(&fold(q), home, bise, secrets) {
            return Err(err("refused", format!("{}: {}", raw, why)));
        }
    }
    let meta = std::fs::metadata(&real)
        .map_err(|_| err("file_not_found", format!("{} can't be read", raw)))?;
    if meta.is_dir() {
        return Err(err(
            "bad_args",
            format!("{} is a folder; attach the files in it one by one", raw),
        ));
    }
    if !meta.is_file() {
        return Err(err("bad_args", format!("{} isn't a regular file", raw)));
    }
    if meta.len() > MAX_BYTES {
        return Err(err(
            "file_too_large",
            format!(
                "{} is {} MB; an agent attaches files of {} MB at most: ask the user to attach it",
                raw,
                meta.len() / (1024 * 1024),
                MAX_BYTES / (1024 * 1024)
            ),
        ));
    }
    let name = real
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(File {
        path: real,
        name,
        size: meta.len(),
    })
}

/// `..` and `.` folded without the disk.
fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// Why `p` (absolute, folded) never goes to a page; None when it may.
pub fn refused(p: &Path, home: &Path, bise: &Path, secrets: &Secrets) -> Option<String> {
    match secrets.held(p) {
        Some(Held::Ssh) => return Some("an ssh private key: agents can't send it".into()),
        Some(Held::Bise) => return Some("bise's keys and sign-ins: agents can't send them".into()),
        None => {}
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if KEY_PREFIXES.iter().any(|k| name.starts_with(k))
        || KEY_SUFFIXES.iter().any(|k| name.ends_with(k))
        || KEY_NAMES.contains(&name.as_str())
    {
        return Some("a key, a keychain or a credentials file: agents can't send it".into());
    }
    if p.starts_with(bise) {
        return Some("bise's own files: agents can't send them".into());
    }
    if let Ok(rel) = p.strip_prefix(home) {
        let mut parts = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        let first = parts.next().unwrap_or_default();
        if first == "Library" {
            let second = parts.next().unwrap_or_default();
            if LIBRARY_OPEN.contains(&second.as_str()) {
                return None;
            }
            return Some("~/Library holds the Mac's keychains, cookies, mail and app data: agents can't send from it (iCloud Drive and the cloud drives are fine)".into());
        }
        if HOME_DENIED.contains(&first.as_str()) {
            return Some(format!(
                "~/{} holds keys and sign-ins: agents can't send from it",
                first
            ));
        }
        return None;
    }
    if SYSTEM_OPEN.iter().any(|o| p.starts_with(o)) {
        return None;
    }
    if SYSTEM_DENIED.iter().any(|d| p.starts_with(d)) {
        return Some("a system folder: agents can't send from it".into());
    }
    None
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod files_tests;
