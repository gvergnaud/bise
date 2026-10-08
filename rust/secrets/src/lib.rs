//! Where bise's secrets live (option A of page secrets-keychain, the
//! user's decision of 2026-10-08): `auth.json` (API keys, the ChatGPT and
//! OpenRouter sign-ins) and each MCP server's OAuth file in
//! `secrets/mcp-oauth/`. Their own files (the default), or the macOS
//! keychain through Apple's /usr/bin/security.
//!
//! A secret is named by the path of its file. **The file says where the
//! secret is**: a secret's own text, or a [`stub::Stub`] (no secret in it)
//! that means "in the keychain, generation G, N items". Reads follow the
//! file ([`read`]); writes follow the setting, config.toml's `[secrets]
//! store` ([`setting`], macOS only): [`write`]. So a process started
//! before `bise secrets keychain on` sees the move at its next read, the
//! mtime watches on these files (the hub's sign-in card, the MCP bridge's
//! login watch) keep working, and a reader can tell a write under way.
//!
//! - **Keychain items**: service "bise", account = the file's path (then
//!   `<path> #2`...), label "bise: <file name>"; the secret in base64, cut
//!   into parts ([`parts`]), written with `security -i` on standard input
//!   in hex, never in an argument ([`security`], [`keychain`]).
//! - **Which keychain** (issue 19 step B): bise's own file,
//!   `~/.bise/secrets/bise.keychain-db`, inside the agents' sandbox read
//!   deny (so an agent's `security` can't read its items), its password one
//!   item of the login keychain ([`keychain::Keychains`]). The stub's head
//!   says which ([`stub::At`]): a stub of v2026.10.2-28 (the login
//!   keychain) is read there and moves on its next write, or on `bise
//!   secrets keychain on` again.
//! - **Never a dialog**: before any `security` call, both keychains' states
//!   are read with no window ([`status`], Security.framework) and
//!   [`ready::next_step`] decides: a locked keychain is
//!   [`ReadError::Locked`], never a call that would make macOS ask.
//! - **Cache**: each process keeps the last secret it read or wrote with
//!   its generation; a read whose stub has that generation costs one small
//!   file read and no `security` call. Every write, by any process, makes a
//!   new generation.
//! - **Locked keychain**: [`ReadError::Locked`], never "no secret": a
//!   locked keychain is never signed out.
//! - **Locks**: the callers' (`auth.json.lock`, `mcp-oauth/<x>.lock`) stay
//!   files, held around a read-modify-write; this crate takes none.
//!
//! Only this crate opens these files (the callers' locks aside): see
//! `tests::law_only_bise_secrets_opens_the_secrets`.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub mod keychain;
pub mod parts;
pub mod ready;
pub mod security;
pub mod setting;
pub mod status;
pub mod stub;

pub use keychain::{Keychain, Keychains};
pub use setting::Store;
use ready::Intent;
use stub::{At, Stub};

bise_home::test_home!();

/// Why a secret can't be read now. Never "there is none": that is
/// `Ok(None)`.
#[derive(Debug)]
pub enum ReadError {
    /// the file couldn't be read
    Io(std::io::Error),
    /// the keychain is locked
    Locked,
    /// `security` failed otherwise (its first line)
    Keychain(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(e) => write!(f, "{e}"),
            ReadError::Locked => write!(f, "the keychain is locked"),
            ReadError::Keychain(e) => write!(f, "the keychain: {e}"),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<ReadError> for std::io::Error {
    fn from(e: ReadError) -> std::io::Error {
        match e {
            ReadError::Io(e) => e,
            e => std::io::Error::other(e),
        }
    }
}

/// A write's error that is a locked keychain.
pub fn is_locked(e: &std::io::Error) -> bool {
    matches!(e.get_ref().and_then(|i| i.downcast_ref::<ReadError>()), Some(ReadError::Locked))
}

/// Where a secret is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Missing,
    File,
    Keychain(Stub),
}

/// path -> (generation, secret): the last one this process read or wrote.
static CACHE: Mutex<BTreeMap<PathBuf, (String, String)>> = Mutex::new(BTreeMap::new());

fn cached(path: &Path, gen: &str) -> Option<String> {
    let c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    c.get(path).filter(|(g, _)| g == gen).map(|(_, s)| s.clone())
}

fn keep(path: &Path, gen: &str, secret: &str) {
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_path_buf(), (gen.to_string(), secret.to_string()));
}

fn read_file(path: &Path) -> Result<Option<String>, ReadError> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ReadError::Io(e)),
    }
}

/// Where the secret at `path` is (its file read, no keychain call).
pub fn place(path: &Path) -> Result<Place, ReadError> {
    Ok(match read_file(path)? {
        None => Place::Missing,
        Some(t) => Stub::parse(&t).map_or(Place::File, Place::Keychain),
    })
}

/// The secret at `path`: its file's text, or the keychain's copy when the
/// file is a stub. `Ok(None)`: no secret (no file, or the keychain has no
/// item for the stub: deleted by hand). A locked keychain is an `Err`.
pub fn read(path: &Path) -> Result<Option<String>, ReadError> {
    let Some(text) = read_file(path)? else { return Ok(None) };
    let Some(mut stub) = Stub::parse(&text) else { return Ok(Some(text)) };
    if let Some(s) = cached(path, &stub.gen) {
        return Ok(Some(s));
    }
    let kcs = Keychains::here().map_err(ReadError::Keychain)?;
    // the stub says which keychain; never a call on a locked one
    let ready = match stub.at {
        At::Bise => kcs.ready(Intent::Read)?,
        At::Login => kcs.login_ready()?,
    };
    let Some(kc) = ready else { return Ok(None) };
    // a write under way: its parts carry the next generation until its
    // stub lands (a write takes ~50-100 ms)
    for _ in 0..20 {
        let datas = find_parts(kc, path, stub.parts)?;
        match datas {
            None => {}
            Some(d) => match parts::join(&stub.gen, &d) {
                Ok(s) => {
                    keep(path, &stub.gen, &s);
                    return Ok(Some(s));
                }
                Err(parts::Torn::Bad) => return Err(ReadError::Keychain("bise's item in the keychain is not readable".into())),
                Err(parts::Torn::Gen) => {}
            },
        }
        std::thread::sleep(Duration::from_millis(30));
        let again = match read_file(path)? {
            None => return Ok(None),
            Some(t) => match Stub::parse(&t) {
                None => return Ok(Some(t)),
                Some(s) => s,
            },
        };
        if again.at != stub.at {
            // moved to bise's keychain meanwhile: read it there
            return read(path);
        }
        if again == stub && datas_absent(kc, path)? {
            // the stub didn't change and its first item is gone
            return Ok(None);
        }
        stub = again;
    }
    Err(ReadError::Keychain("bise's item keeps changing: try again".into()))
}

fn datas_absent(kc: &Keychain, path: &Path) -> Result<bool, ReadError> {
    Ok(kc.find(&security::account(path, 0))?.is_none())
}

/// Every part's data, read in parallel (one round); None when one is
/// missing.
fn find_parts(kc: &Keychain, path: &Path, n: usize) -> Result<Option<Vec<String>>, ReadError> {
    let results: Vec<Result<Option<String>, ReadError>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..n).map(|i| s.spawn(move || kc.find(&security::account(path, i)))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(ReadError::Keychain("a read failed".into())))).collect()
    });
    let mut out = Vec::with_capacity(n);
    for r in results {
        match r? {
            Some(d) => out.push(d),
            None => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// Write the secret at `path` where the setting says ([`setting::current`]).
/// `to_file` writes it as its own file, the caller's way (unchanged from
/// before the keychain, byte for byte).
pub fn write(path: &Path, secret: &str, to_file: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<()> {
    write_to(setting::current(), path, secret, to_file)
}

/// Write the secret at `path` to `store`. The keychain (always bise's
/// own): its parts, read back, then the stub (atomic), then the old parts
/// deleted (beyond the new count, or all of them in the login keychain:
/// a stub of before moves on its next write). A file: `to_file`, then the
/// keychain items of the stub it replaced.
pub fn write_to(store: Store, path: &Path, secret: &str, to_file: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<()> {
    let old = match place(path) {
        Ok(Place::Keychain(s)) => Some(s),
        _ => None,
    };
    match store {
        Store::File => {
            to_file()?;
            if let Some(s) = old {
                forget_items(path, &s);
            }
            Ok(())
        }
        Store::Keychain => {
            let kcs = Keychains::here().map_err(std::io::Error::other)?;
            let kc = kcs.ready(Intent::Write)?.ok_or_else(|| std::io::Error::other("bise's keychain is missing"))?;
            let on_bise = old.as_ref().filter(|s| s.at == At::Bise).map_or(0, |s| s.parts);
            to_keychain(kc, path, secret, on_bise)?;
            if let Some(s) = old.filter(|s| s.at == At::Login) {
                if let Ok(Some(login)) = kcs.login_ready() {
                    delete_parts(login, path, 0, s.parts);
                }
            }
            Ok(())
        }
    }
}

/// Delete the items of the stub `s` of `path`, in its keychain; best
/// effort, never on a locked one.
fn forget_items(path: &Path, s: &Stub) {
    let Ok(kcs) = Keychains::here() else { return };
    let kc = match s.at {
        At::Bise => kcs.ready(Intent::Read),
        At::Login => kcs.login_ready(),
    };
    if let Ok(Some(kc)) = kc {
        delete_parts(kc, path, 0, s.parts);
    }
}

fn to_keychain(kc: &Keychain, path: &Path, secret: &str, old_parts: usize) -> std::io::Result<()> {
    let room = security::room(path, kc.file()).ok_or_else(|| std::io::Error::other("the secret's path is too long for the keychain"))?;
    let gen = new_gen();
    let payload = parts::encode(secret);
    // the generation and its ':' go in each part's data
    let pieces = parts::split(&payload, room - gen.len() - 1);
    if pieces.len() > security::MAX_PARTS {
        return Err(std::io::Error::other("the secret is too big for the keychain"));
    }
    let label = security::label(path);
    let lines: Vec<String> = pieces
        .iter()
        .enumerate()
        .map(|(i, p)| security::add_line(&security::account(path, i), &label, &parts::data(&gen, p), kc.file()))
        .collect::<Option<_>>()
        .ok_or_else(|| std::io::Error::other("the secret's path holds a control character"))?;
    kc.run(&lines)?;
    // read back before the stub replaces the file: a secret is never in
    // neither place
    match find_parts(kc, path, pieces.len())? {
        Some(d) if parts::join(&gen, &d).as_deref() == Ok(secret) => {}
        _ => return Err(std::io::Error::other("the keychain didn't keep the secret")),
    }
    write_stub(path, &Stub { gen: gen.clone(), parts: pieces.len(), at: At::Bise })?;
    delete_parts(kc, path, pieces.len(), old_parts);
    keep(path, &gen, secret);
    Ok(())
}

fn delete_parts(kc: &Keychain, path: &Path, from: usize, to: usize) {
    for i in from..to {
        kc.delete(&security::account(path, i));
    }
}

/// The stub, 0600, by rename (its folder made 0700 when missing).
fn write_stub(path: &Path, s: &Stub) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !dir.exists() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("secret");
    let tmp = dir.join(format!(".{name}.{}.stub", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let res = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(s.to_text().as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Move the secret at `path` to `store` (the caller holds its lock): read
/// where it is, write it there; its own file is written as the secret's
/// text, 0600, by rename. Whether it moved (false: already there, or no
/// secret).
pub fn move_to(store: Store, path: &Path) -> Result<bool, ReadError> {
    let here = place(path)?;
    // a stub of before (the login keychain) moves to bise's keychain too
    let moves = match (&here, store) {
        (Place::File, Store::Keychain) | (Place::Keychain(_), Store::File) => true,
        (Place::Keychain(s), Store::Keychain) => s.at == At::Login,
        _ => false,
    };
    if !moves {
        return Ok(false);
    }
    // from the keychain itself, not this process's cache: a locked
    // keychain stops the move (its items could not be deleted either)
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).remove(path);
    let Some(secret) = read(path)? else { return Ok(false) };
    write_to(store, path, &secret, &|| write_private(path, &secret)).map_err(|e| if is_locked(&e) { ReadError::Locked } else { ReadError::Io(e) })?;
    Ok(true)
}

/// `text` at `path`, 0600, by rename (a temp file beside it).
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("secret");
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let res = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Remove the secret at `path`: its keychain items when it is there,
/// then its file.
pub fn remove(path: &Path) -> std::io::Result<()> {
    if let Ok(Place::Keychain(s)) = place(path) {
        forget_items(path, &s);
    }
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).remove(path);
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// 16 hex digits from /dev/urandom (the clock's nanoseconds and the pid
/// if it can't be read).
fn new_gen() -> String {
    let mut b = [0u8; 8];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_ok();
    if !ok {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
        b = (t ^ ((std::process::id() as u64) << 32)).to_le_bytes();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `n` random bytes from /dev/urandom, in hex (bise's keychain password:
/// never made without it).
pub(crate) fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        b = (0..n).map(|_| new_gen()).collect::<String>().into_bytes();
        b.truncate(n);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests;
