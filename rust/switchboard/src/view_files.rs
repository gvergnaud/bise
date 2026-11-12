//! A file-opened view's file, the pure half (docs/plugin-apps.md A4;
//! architect m_59962): what a view names its file by, and the rules of
//! reading and writing it. No disk here: the atomic write is
//! `bise_peer::fsx`; `~/bise`'s stable ids (`e:<n>`, `folder/list`'s rows)
//! are `bise_ids`, kept by the home hub.
//!
//! - **Handles** ([`Files`]): `view-file://<32 hex>`, minted by the hub
//!   for one view session (the doc id, the view's pages session) and one
//!   typed [`FileTarget`]; gone with the session ([`Files::end`]). A
//!   view names its file only by its handle: one it was not given (made
//!   up, another session's, a path) is [`ViewError::BadHandle`], one
//!   answer whatever the reason. `resources/url` mints a URL token for a
//!   handle ([`Files::url`]), served by the pages server's `/h/` on the
//!   session's origin ([`Files::by_url`], a [`FileAt`]); gone with it.
//! - **Reads** ([`window`]): 24 MiB a read, ranges for big files.
//! - **Writes**: the etags ([`artifact_etag`], a root file's content hash)
//!   and the answer kept per session for a retried `seq`
//!   ([`Binding::replay`]): the same seq, the same answer, never a second
//!   version (architect m_59962 #1).

use bise_proto::view_rpc::{ViewError, WriteOutcome, MAX_READ};
use std::collections::HashMap;
use std::path::PathBuf;

/// A handle's scheme (Vibe's Plugins v2 RFC §7).
pub const SCHEME: &str = "view-file://";
/// A folder's first-level names given to the matcher's folder rules (A2).
pub const FOLDER_NAMES: usize = 64;
/// At most this many live handles (a backstop: sessions end with their
/// tab and with the pages server's caps; past it the oldest goes).
pub const HANDLES: usize = 4096;

// ---- targets and handles ----

/// What a handle is bound to (A4): an artifact's version, or a file of
/// `~/bise` by the path the hub resolved. An entry that is an artifact's
/// current target is minted as that artifact (one document, one write
/// path).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileTarget {
    Artifact { id: String, version: u64 },
    RootFile { path: PathBuf },
}

/// A file or a folder, with a folder's first-level names (A2's folder
/// rules), at most [`FOLDER_NAMES`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder { names: Vec<String> },
}

/// A `view/open` target (`bise_proto::files::OpenTarget`) as the hub
/// resolved it: what the matcher reads (name,
/// kind, the plugin whose documents hold it) and what a handle binds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub target: FileTarget,
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    /// the plugin whose documents (`~/bise/Plugins/<name>/`) hold it
    pub documents_of: Option<String>,
    /// an artifact's version older than its newest: read only
    /// ([`Binding::older`])
    pub older: bool,
}

/// The view a handle was minted for: its plugin (the `write` grant is
/// its view's), server (a tool call to it gets the real path) and view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opener {
    pub plugin: String,
    pub server: String,
    pub uri: String,
}

/// One handle's binding.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// the view session (the doc id) it was minted for
    pub session: String,
    pub opener: Opener,
    pub target: FileTarget,
    /// opened on an artifact's older version: read only (NoWrite
    /// `older_version`)
    pub older: bool,
    /// the newest write applied, and its answer
    pub answered: Option<(u64, WriteOutcome)>,
    seq: u64,
}

impl Binding {
    pub fn new(session: &str, opener: Opener, target: FileTarget, older: bool) -> Binding {
        Binding { session: session.to_string(), opener, target, older, answered: None, seq: 0 }
    }

    /// A write of `seq` already applied (the host's retry, or an older
    /// one late): the answer it had, never a second write. None: a new
    /// write.
    pub fn replay(&self, seq: u64) -> Option<WriteOutcome> {
        let (last, answer) = self.answered.as_ref()?;
        (seq <= *last).then(|| answer.clone())
    }
}

/// The live handles, by their 32 hex, and the URLs minted for them
/// (`resources/url`), by their token. Held by the pages server beside the
/// view sessions (`Pages.files`): its `/h/` route resolves a URL on its
/// own thread, and the one place a session ends ends its handles.
#[derive(Debug, Default)]
pub struct Files {
    by: HashMap<String, Binding>,
    /// a URL's token -> its handle's 32 hex
    urls: HashMap<String, String>,
    /// root files' content hashes by (size, mtime) when hashed, so a
    /// video's ranges and a write's decide hash it once
    hashes: HashMap<PathBuf, (Stamp, String)>,
    seq: u64,
}

/// A file's (size, mtime): what a cached content hash stays true for.
pub type Stamp = (u64, std::time::SystemTime);

/// At most this many content hashes kept.
pub const HASHES: usize = 256;

/// Where a URL's file is on disk, for the pages server's `/h/` route
/// (ranges and headers are its own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAt {
    pub path: PathBuf,
    pub size: u64,
    pub mime: String,
    pub etag: String,
}

/// The 32 hex of a handle: exactly `view-file://` + 32 lowercase hex.
pub fn handle_key(uri: &str) -> Option<&str> {
    let k = uri.strip_prefix(SCHEME)?;
    (k.len() == 32 && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then_some(k)
}

impl Files {
    /// A new handle for `b`, `token` its 32 hex (the system's random
    /// source, `pages::new_token`). Past [`HANDLES`] the oldest goes.
    pub fn mint(&mut self, token: &str, mut b: Binding) -> String {
        self.seq += 1;
        b.seq = self.seq;
        self.by.insert(token.to_string(), b);
        if self.by.len() > HANDLES {
            if let Some(old) = self.by.iter().min_by_key(|(_, b)| b.seq).map(|(k, _)| k.clone()) {
                self.by.remove(&old);
                self.urls.retain(|_, k| *k != old);
            }
        }
        format!("{SCHEME}{token}")
    }

    /// The session ended (its tab closed, unloaded, swapped, or dropped by
    /// the pages server's caps): its handles and their URLs go.
    pub fn end(&mut self, session: &str) {
        self.by.retain(|_, b| b.session != session);
        let by = &self.by;
        self.urls.retain(|_, k| by.contains_key(k));
    }

    /// `resources/url`: a URL token for `uri`, `token` its 32 hex (one per
    /// call; each lives as long as the session). Refused as a read is.
    pub fn url(&mut self, session: &str, uri: &str, token: &str) -> Result<String, ViewError> {
        self.get(session, uri)?;
        let k = handle_key(uri).ok_or(ViewError::BadHandle)?.to_string();
        self.urls.insert(token.to_string(), k);
        Ok(token.to_string())
    }

    /// The binding a URL's token names (the `/h/` route, which checks the
    /// request's origin is the session's).
    pub fn by_url(&self, token: &str) -> Option<&Binding> {
        self.by.get(self.urls.get(token)?)
    }

    /// `path`'s content hash, when the one kept is for `stamp`.
    pub fn hash_of(&self, path: &std::path::Path, stamp: Stamp) -> Option<String> {
        self.hashes.get(path).filter(|(s, _)| *s == stamp).map(|(_, h)| h.clone())
    }

    /// Keep `path`'s content hash for `stamp`; past [`HASHES`], all go
    /// (the next read hashes again).
    pub fn keep_hash(&mut self, path: &std::path::Path, stamp: Stamp, hash: &str) {
        if self.hashes.len() >= HASHES && !self.hashes.contains_key(path) {
            self.hashes.clear();
        }
        self.hashes.insert(path.to_path_buf(), (stamp, hash.to_string()));
    }

    /// The binding of `uri` for a request of `session`; anything else is
    /// [`ViewError::BadHandle`].
    pub fn get(&self, session: &str, uri: &str) -> Result<&Binding, ViewError> {
        let k = handle_key(uri).ok_or(ViewError::BadHandle)?;
        self.by.get(k).filter(|b| b.session == session && !session.is_empty()).ok_or(ViewError::BadHandle)
    }

    /// [`Files::get`], to record a write.
    pub fn get_mut(&mut self, session: &str, uri: &str) -> Result<&mut Binding, ViewError> {
        let k = handle_key(uri).ok_or(ViewError::BadHandle)?;
        self.by.get_mut(k).filter(|b| b.session == session && !session.is_empty()).ok_or(ViewError::BadHandle)
    }

    /// The file session `session` was opened on (its notes, apps-viewkit).
    pub fn of_session(&self, session: &str) -> Option<&Binding> {
        self.by.values().filter(|b| b.session == session).min_by_key(|b| b.seq)
    }

    pub fn len(&self) -> usize {
        self.by.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by.is_empty()
    }
}

// ---- reads ----

/// A read's byte range, as the view asks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub offset: u64,
    pub length: u64,
}

/// What a read of a file of `size` bytes answers: (offset, length). A
/// whole read past [`MAX_READ`]: [`ViewError::TooLarge`] (read it by
/// ranges); a range is cut at the file's end and at [`MAX_READ`].
pub fn window(size: u64, range: Option<Range>) -> Result<(u64, u64), ViewError> {
    match range {
        None if size > MAX_READ => Err(ViewError::TooLarge),
        None => Ok((0, size)),
        Some(r) => {
            let offset = r.offset.min(size);
            Ok((offset, r.length.min(MAX_READ).min(size - offset)))
        }
    }
}

// ---- etags ----

/// An artifact version's etag: `<v>.<rev>`.
pub fn artifact_etag(v: u64, rev: u32) -> String {
    format!("{v}.{rev}")
}

/// The (version, rev) an artifact etag names.
pub fn parse_artifact_etag(e: &str) -> Option<(u64, u32)> {
    let (v, r) = e.split_once('.')?;
    Some((v.parse().ok()?, r.parse().ok()?))
}

/// A root file's etag: the sha256 of its bytes, 64 hex.
pub fn content_etag(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

/// 64 hex of a digest.
pub fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// A root file's write against what it is now: `ifMatch` absent, or its
/// etag now, writes; else a conflict that says what it is now.
pub fn root_decide(if_match: Option<&str>, now: &str) -> Result<(), WriteOutcome> {
    match if_match {
        Some(m) if m != now => Err(WriteOutcome::Conflict { etag: now.to_string() }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opener() -> Opener {
        Opener { plugin: "fake-opener".into(), server: "~views".into(), uri: "ui://fake-opener/fake".into() }
    }

    fn root(p: &str) -> FileTarget {
        FileTarget::RootFile { path: PathBuf::from(p) }
    }

    #[test]
    fn a_handle_is_its_sessions_only() {
        let mut h = Files::default();
        let t = "0123456789abcdef0123456789abcdef";
        let uri = h.mint(t, Binding::new("s1", opener(), root("/r/a.fake"), false));
        assert_eq!(uri, format!("view-file://{t}"));
        assert_eq!(h.get("s1", &uri).map(|b| b.target.clone()), Ok(root("/r/a.fake")));
        let other = h.mint("fedcba9876543210fedcba9876543210", Binding::new("s2", opener(), FileTarget::Artifact { id: "q3".into(), version: 2 }, false));
        // every refusal is the same answer
        for (session, uri) in [
            ("s2", uri.as_str()),                                    // another session's
            ("s1", other.as_str()),                                  // another file's
            ("s1", "view-file://00000000000000000000000000000000"), // made up
            ("s1", "/Users/x/bise/a.fake"),                          // a path
            ("s1", "file:///Users/x/bise/a.fake"),
            ("s1", "view-file://0123456789ABCDEF0123456789ABCDEF"),
            ("s1", "view-file://0123456789abcdef0123456789abcdef/../x"),
            ("", uri.as_str()),
        ] {
            assert_eq!(h.get(session, uri).err(), Some(ViewError::BadHandle), "{session} {uri}");
        }
        assert_eq!(h.of_session("s2").map(|b| b.target.clone()), Some(FileTarget::Artifact { id: "q3".into(), version: 2 }));
        h.end("s1");
        assert_eq!(h.get("s1", &uri).err(), Some(ViewError::BadHandle), "gone with its session");
        assert!(h.get("s2", &other).is_ok() && h.len() == 1);
    }

    /// `resources/url`: a URL for its own handle only, gone with its session.
    #[test]
    fn a_url_names_its_handle_until_the_session_ends() {
        let mut f = Files::default();
        let uri = f.mint("0123456789abcdef0123456789abcdef", Binding::new("s1", opener(), root("/r/a.mp4"), false));
        let tok = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(f.url("s2", &uri, tok).err(), Some(ViewError::BadHandle), "another session's handle");
        assert_eq!(f.url("s1", "view-file://00000000000000000000000000000000", tok).err(), Some(ViewError::BadHandle));
        assert_eq!(f.by_url(tok), None, "a refused url mints nothing");
        assert_eq!(f.url("s1", &uri, tok), Ok(tok.to_string()));
        assert_eq!(f.by_url(tok).map(|b| b.target.clone()), Some(root("/r/a.mp4")));
        assert_eq!(f.by_url("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), None, "a made-up token");
        f.end("s1");
        assert_eq!(f.by_url(tok), None, "gone with its session");
    }

    #[test]
    fn a_content_hash_is_kept_for_its_stamp_only() {
        let mut f = Files::default();
        let t = std::time::UNIX_EPOCH;
        let p = std::path::Path::new("/r/v.mp4");
        f.keep_hash(p, (10, t), "h1");
        assert_eq!(f.hash_of(p, (10, t)).as_deref(), Some("h1"));
        assert_eq!(f.hash_of(p, (11, t)), None, "its size moved");
        assert_eq!(f.hash_of(p, (10, t + std::time::Duration::from_millis(1))), None, "its mtime moved");
        for i in 0..HASHES + 1 {
            f.keep_hash(&std::path::PathBuf::from(format!("/r/{i}")), (1, t), "h");
        }
        assert!(f.hashes.len() <= HASHES, "bounded");
    }

    #[test]
    fn the_handles_kept_are_bounded_oldest_out() {
        let mut h = Files::default();
        let tok = |i: usize| format!("{i:032x}");
        for i in 0..HANDLES + 2 {
            h.mint(&tok(i), Binding::new(&format!("s{i}"), opener(), root("/r/a"), false));
        }
        assert_eq!(h.len(), HANDLES);
        assert!(h.get("s0", &format!("{SCHEME}{}", tok(0))).is_err() && h.get("s1", &format!("{SCHEME}{}", tok(1))).is_err());
        assert!(h.get("s2", &format!("{SCHEME}{}", tok(2))).is_ok());
    }

    #[test]
    fn a_retried_write_gets_the_answer_it_had() {
        let mut b = Binding::new("s1", opener(), root("/r/a"), false);
        assert_eq!(b.replay(1), None, "a new write");
        b.answered = Some((2, WriteOutcome::Saved { etag: "e2".into() }));
        assert_eq!(b.replay(2), Some(WriteOutcome::Saved { etag: "e2".into() }), "the same seq");
        assert_eq!(b.replay(1), Some(WriteOutcome::Saved { etag: "e2".into() }), "an older one, late");
        assert_eq!(b.replay(3), None);
    }

    #[test]
    fn a_read_is_capped_and_ranged() {
        assert_eq!(window(10, None), Ok((0, 10)));
        assert_eq!(window(MAX_READ, None), Ok((0, MAX_READ)));
        assert_eq!(window(MAX_READ + 1, None), Err(ViewError::TooLarge), "a whole read past the cap");
        let big = 1 << 30;
        assert_eq!(window(big, Some(Range { offset: 5, length: big })), Ok((5, MAX_READ)), "a range past the cap is cut");
        assert_eq!(window(10, Some(Range { offset: 8, length: 5 })), Ok((8, 2)), "cut at the end");
        assert_eq!(window(10, Some(Range { offset: 20, length: 5 })), Ok((10, 0)), "past the end: nothing");
    }

    #[test]
    fn etags_and_the_root_files_rule() {
        assert_eq!(artifact_etag(3, 2), "3.2");
        assert_eq!(parse_artifact_etag("3.2"), Some((3, 2)));
        for bad in ["3", "a.2", "3.", ".2", "3.2.1"] {
            assert_eq!(parse_artifact_etag(bad), None, "{bad}");
        }
        let e = content_etag(b"hello");
        assert_eq!(e, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert_eq!(root_decide(Some(&e), &e), Ok(()));
        assert_eq!(root_decide(None, &e), Ok(()), "no ifMatch: his write wins");
        assert_eq!(root_decide(Some("stale"), &e), Err(WriteOutcome::Conflict { etag: e.clone() }));
    }
}
