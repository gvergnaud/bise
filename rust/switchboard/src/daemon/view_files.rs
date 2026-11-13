//! A document view's file, the shell's side (docs/plugin-apps.md A4;
//! architect m_59962, m_60222): `resources/read`, `resources/url` and
//! `ai.mistral.vibe/resources/write` from a document's `view/call {doc}`
//! (daemon/views.rs `doc_call` hands them here), and `view/copy`, the
//! host's own method.
//!
//! - every request: its handle looked up for its session in the pages
//!   server's table (pages/doc_files.rs), copied out; anything else is
//!   `bad-handle`;
//! - a read runs on a thread ([`crate::view_files_disk`]): never the loop;
//! - a write: the `write` grant read fresh and checked every time, an
//!   older version refused (`no-write`, its reason), a retried `seq`
//!   answered as before ([`crate::view_files::Binding::replay`]); its
//!   bytes are staged on the one writer thread ([`DocFiles`]: fsx's temp
//!   file beside the file, fsync, hash; an artifact's next copy in the
//!   store), then committed on the loop, metadata only: the decide (an
//!   artifact's: `crate::artifacts_save::decide`, one version per sitting,
//!   the sitting the doc; a root file's: its `ifMatch` against the hash
//!   cached for its size and mtime, so a stat), the rename, the version;
//! - `dev.bise/storage/seed`: a site's kept localStorage, once (shim
//!   site-storage-seed);
//! - `view/copy`: the bytes beside the file as `<name> (copy).<ext>` (a
//!   free name from the words catalog, never over a file), added as his
//!   artifact.

use super::views::{ViewDone, ViewStep};
use super::{Msg, Shell};
use crate::artifacts::{self, Add};
use crate::artifacts_save::{decide, Decision, Save};
use crate::pages::artifact::notes::arts;
use crate::pages::Pages;
use crate::util::now_ms;
use crate::view_files::{artifact_etag, parse_artifact_etag, Binding, FileTarget, Range};
use crate::view_files_disk as disk;
use bise_peer::fsx;
use bise_proto::jsonrpc::RpcError;
use bise_proto::view_rpc::{NoWrite, ViewError, WriteOutcome, MAX_WRITE};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

pub(super) const READ: &str = "resources/read";
pub(super) const URL: &str = "resources/url";
pub(super) const WRITE: &str = "ai.mistral.vibe/resources/write";
/// A site's kept localStorage, once, for its own origin's real one (A5,
/// architect m_60421).
// TODO(site-storage-seed): goes in v2026.12.15 with artifacts_storage
pub(super) const SEED: &str = "dev.bise/storage/seed";
const COPY: &str = "view/copy";

/// Where a document view's `view/call` method goes: the hub's own file
/// methods, its opener's tools, or nowhere. Any other method is unknown,
/// the host's own `dev.bise/resources/copy` too: a view never relays it
/// (architect m_60137).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DocRoute {
    File,
    Tool,
    Unknown,
}

pub(super) fn doc_route(method: &str) -> DocRoute {
    match method {
        READ | URL | WRITE | SEED => DocRoute::File,
        "tools/call" => DocRoute::Tool,
        _ => DocRoute::Unknown,
    }
}

/// The document files' one writer thread (architect m_59962 (a)): the
/// writes' heavy half, one at a time, off the loop.
#[derive(Default)]
pub(super) struct DocFiles {
    writer: Option<Sender<Box<dyn FnOnce() + Send>>>,
}

impl DocFiles {
    /// `job` on the writer thread, started at the first write.
    fn run(&mut self, job: Box<dyn FnOnce() + Send>) {
        let tx = self.writer.get_or_insert_with(|| {
            let (tx, rx) = channel::<Box<dyn FnOnce() + Send>>();
            std::thread::spawn(move || rx.into_iter().for_each(|job| job()));
            tx
        });
        if let Err(e) = tx.send(job) {
            // the thread is gone (a panic): a new one, the job again
            self.writer = None;
            self.run(e.0);
        }
    }
}

/// A request's bad params.
fn bad(what: &str, why: &str) -> RpcError {
    RpcError { code: bise_proto::rpc::code::INVALID_PARAMS, message: format!("{what}: {why}"), data: None }
}

/// The bytes a write or a copy carries: `text`, or `blob` (base64).
fn body_of(p: &Value, what: &str) -> Result<Vec<u8>, RpcError> {
    use base64::Engine;
    match (p.get("text").and_then(Value::as_str), p.get("blob").and_then(Value::as_str)) {
        (Some(t), None) => Ok(t.as_bytes().to_vec()),
        (None, Some(b)) => base64::engine::general_purpose::STANDARD.decode(b).map_err(|_| bad(what, "blob is not base64")),
        _ => Err(bad(what, "text or blob, one of the two")),
    }
}

/// A read's answer (the MCP `resources/read` shape plus `_meta`).
fn read_answer(uri: &str, mime: &str, bytes: Vec<u8>, whole: bool, meta: bise_proto::view_rpc::ReadMeta) -> Value {
    use base64::Engine;
    let content = match (whole && disk::is_text(mime)).then(|| String::from_utf8(bytes.clone()).ok()).flatten() {
        Some(text) => json!({"uri": uri, "mimeType": mime, "text": text}),
        None => json!({"uri": uri, "mimeType": mime, "blob": base64::engine::general_purpose::STANDARD.encode(&bytes)}),
    };
    json!({"contents": [content], "_meta": meta})
}

/// A staged write, back on the loop to be committed.
struct Staged {
    doc: String,
    uri: String,
    seq: u64,
    binding: Binding,
    if_match: Option<String>,
    prepared: Option<fsx::Prepared>,
    /// an artifact's next copy in the store (deleted when it loses)
    copy: Option<PathBuf>,
}

impl Drop for Staged {
    fn drop(&mut self) {
        if let Some(c) = self.copy.take() {
            let _ = std::fs::remove_file(c);
        }
    }
}

impl Shell {
    fn doc_pages(&self) -> Option<Arc<Pages>> {
        self.pg.pages.clone()
    }

    /// Hands a thread's next step to the loop.
    fn doc_step(&self) -> impl Fn(ViewStep) + Send + 'static {
        let tx = self.tx.clone();
        move |s: ViewStep| {
            let _ = tx.send(Msg::View(s));
        }
    }

    /// A document view's file request (daemon/views.rs `doc_call`).
    pub(super) fn doc_file(&mut self, doc: String, method: &str, inner: Value, done: ViewDone) {
        let Some(pages) = self.doc_pages() else { return done(Err(ViewError::Expired.rpc(method))) };
        let uri = inner.get("uri").and_then(Value::as_str).unwrap_or("").to_string();
        let b = match pages.doc_binding(&doc, &uri) {
            Ok(b) => b,
            Err(e) => return done(Err(e.rpc(method))),
        };
        match method {
            READ => doc_read(pages, b, uri, &inner, done),
            URL => done(pages.doc_url(&doc, &uri).map(|url| json!({"url": url})).map_err(|e| e.rpc(method))),
            WRITE => self.doc_write(pages, doc, b, uri, &inner, done),
            SEED => doc_seed(pages, b, done),
            m => done(Err(ViewError::NotSupported.rpc(m))),
        }
    }

    // ---- write ----

    fn doc_write(&mut self, pages: Arc<Pages>, doc: String, b: Binding, uri: String, p: &Value, done: ViewDone) {
        let Some(seq) = p.get("seq").and_then(Value::as_u64).filter(|s| *s > 0) else { return done(Err(bad(WRITE, "seq (from 1) is needed"))) };
        if let Some(a) = b.replay(seq) {
            return done(Ok(json!(a)));
        }
        if let Err(why) = disk::may_write(&b) {
            return done(Err(ViewError::no_write(why, WRITE)));
        }
        let bytes = match body_of(p, WRITE) {
            Ok(b) => b,
            Err(e) => return done(Err(e)),
        };
        if bytes.len() as u64 > MAX_WRITE {
            return done(Ok(json!(WriteOutcome::TooLarge { max_bytes: MAX_WRITE })));
        }
        let if_match = p.get("ifMatch").and_then(Value::as_str).map(str::to_string);
        let step = self.doc_step();
        self.files.run(Box::new(move || {
            let staged = stage(&pages, &b, &bytes).map(|(prepared, copy)| Staged { doc, uri, seq, binding: b, if_match, prepared: Some(prepared), copy });
            step(Box::new(move |sh: &mut Shell| match staged {
                Ok(s) => sh.doc_commit(s, done),
                Err(e) => done(Err(e.rpc(WRITE))),
            }));
        }));
    }

    /// The loop's half of a write: the decide, the rename, the version.
    fn doc_commit(&mut self, mut s: Staged, done: ViewDone) {
        let Some(pages) = self.doc_pages() else { return done(Err(ViewError::Expired.rpc(WRITE))) };
        let answer = match s.binding.target.clone() {
            FileTarget::RootFile { path } => commit_root(&pages, &path, &mut s),
            FileTarget::Artifact { id, .. } => self.commit_artifact(&pages, &id, &mut s),
        };
        match answer {
            Ok((outcome, target)) => {
                pages.doc_wrote(&s.doc, &s.uri, s.seq, &outcome, target);
                done(Ok(json!(outcome)))
            }
            Err(e) => done(Err(e.rpc(WRITE))),
        }
    }

    /// An artifact's save: the store's rule (one version per sitting, the
    /// sitting this doc), its file renamed over, the staged copy renamed
    /// in as the version's, the version's lines and frames as any save's.
    fn commit_artifact(&mut self, pages: &Pages, id: &str, s: &mut Staged) -> Result<(WriteOutcome, Option<FileTarget>), ViewError> {
        let store = self.art_store();
        let m = store.get(id).ok_or(ViewError::Gone)?;
        let latest = m.current().cloned().ok_or(ViewError::Gone)?;
        let now_etag = artifact_etag(latest.v, latest.rev());
        let FileTarget::Artifact { version, .. } = s.binding.target else { return Err(ViewError::Gone) };
        let (base, rev) = match s.if_match.as_deref().map(parse_artifact_etag) {
            Some(Some(b)) => b,
            Some(None) => return Ok((WriteOutcome::Conflict { etag: now_etag }, None)),
            None => (version, m.versions.iter().find(|v| v.v == version).map_or(1, |v| v.rev())),
        };
        let save = Save { base, rev, session: s.doc.clone(), seq: s.seq, force: false };
        // a stat, never a read: the file is the newest version's while its sig is
        let disk_same = || artifacts::file_sig(std::path::Path::new(&m.source)).as_deref() == Some(latest.sig.as_str());
        match decide(Some(&latest), &save, disk_same) {
            Decision::Already { v, rev } => return Ok((WriteOutcome::Saved { etag: artifact_etag(v, rev) }, None)),
            Decision::Stale { .. } | Decision::Disk { .. } => return Ok((WriteOutcome::Conflict { etag: now_etag }, None)),
            Decision::Write => {}
        }
        let p = s.prepared.take().ok_or(ViewError::Gone)?;
        fsx::commit(p, None, None).map_err(|_| ViewError::Gone)?;
        let cwd = std::path::Path::new(&m.source).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let sitting = crate::artifacts_turns::Sitting { by: "you".into(), session: s.doc.clone(), seq: s.seq };
        let add = Add { target: m.source.clone(), agent: m.agent.clone(), by: "you".into(), cwd, known_only: true, sitting: Some(sitting), staged: s.copy.take(), ..Default::default() };
        let a = store.add(&add, now_ms()).map_err(|_| ViewError::Gone)?;
        if let Some(c) = add.staged.filter(|c| c.exists()) {
            let _ = std::fs::remove_file(c);
        }
        let v = a.meta.current().cloned().ok_or(ViewError::Gone)?;
        if a.new_version {
            self.art_saved(pages, &a);
        }
        Ok((WriteOutcome::Saved { etag: artifact_etag(v.v, v.rev()) }, Some(FileTarget::Artifact { id: id.to_string(), version: v.v })))
    }

    // ---- view/copy ----

    /// `view/copy {doc, text|blob}` (A6b Keep mine as a copy, architect
    /// m_60222): the host's own method, never relayed from a view.
    pub(super) fn view_copy(&mut self, doc: String, p: Value, done: ViewDone) {
        let Some(pages) = self.doc_pages() else { return done(Err(ViewError::Expired.rpc(COPY))) };
        let Some(b) = pages.doc_of(&doc) else { return done(Err(ViewError::BadHandle.rpc(COPY))) };
        if let Err(NoWrite::NoGrant) = disk::may_write(&b) {
            return done(Err(ViewError::no_write(NoWrite::NoGrant, COPY)));
        }
        let bytes = match body_of(&p, COPY) {
            Ok(b) => b,
            Err(e) => return done(Err(e)),
        };
        if bytes.len() as u64 > MAX_WRITE {
            return done(Err(ViewError::TooLarge.rpc(COPY)));
        }
        let (store, root) = (self.art_store(), pages.bise_root.clone());
        let step = self.doc_step();
        self.files.run(Box::new(move || {
            let made = (|| {
                let at = match &b.target {
                    FileTarget::Artifact { id, .. } => store.get(id).map(|m| PathBuf::from(m.source)).ok_or(ViewError::Gone)?,
                    FileTarget::RootFile { .. } => disk::file_at(&store, &root, &b.target)?.path,
                };
                write_copy(&at, &bytes).map_err(|_| ViewError::Gone)
            })();
            step(Box::new(move |sh: &mut Shell| match made {
                Ok(path) => done(sh.copy_added(&b, &path).map(|id| json!(bise_proto::files::ViewCopyResult { artifact: id }))),
                Err(e) => done(Err(e.rpc(COPY))),
            }));
        }));
    }

    /// The copy as his artifact, filed beside its source's.
    fn copy_added(&mut self, b: &Binding, path: &std::path::Path) -> Result<String, RpcError> {
        let store = self.art_store();
        let folder = match &b.target {
            FileTarget::Artifact { id, .. } => store.get(id).and_then(|m| m.folder).map(artifacts::Filed::Asked),
            FileTarget::RootFile { .. } => None,
        };
        let cwd = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let add = Add { target: path.to_string_lossy().to_string(), agent: crate::model::MAIN.into(), by: "you".into(), cwd, folder, ..Default::default() };
        let a = store.add(&add, now_ms()).map_err(|e| bad(COPY, &e))?;
        self.art_lines(&a);
        self.artifacts_refresh(false);
        Ok(a.meta.id)
    }
}

/// A read, on its own thread.
/// `dev.bise/storage/seed {uri}`: the handle's artifact's kept
/// localStorage, once ([`crate::artifacts_storage::seed`], under the
/// pages lock, off the loop); `{local: null}` for anything else.
// TODO(site-storage-seed): goes in v2026.12.15 with artifacts_storage
fn doc_seed(pages: Arc<Pages>, b: Binding, done: ViewDone) {
    std::thread::spawn(move || {
        let local = match &b.target {
            FileTarget::Artifact { id, .. } => pages.locked(|_| crate::artifacts_storage::seed(&arts(&pages), id)),
            FileTarget::RootFile { .. } => None,
        };
        done(Ok(json!({"local": local})))
    });
}

fn doc_read(pages: Arc<Pages>, b: Binding, uri: String, p: &Value, done: ViewDone) {
    let range = p.get("range").map(|r| Range { offset: r.get("offset").and_then(Value::as_u64).unwrap_or(0), length: r.get("length").and_then(Value::as_u64).unwrap_or(0) });
    let writable = disk::may_write(&b).is_ok();
    std::thread::spawn(move || {
        let r = (|| {
            let at = disk::file_at(&arts(&pages), &pages.bise_root, &b.target)?;
            let (offset, length) = crate::view_files::window(at.size, range)?;
            let bytes = disk::read(&at, offset, length)?;
            let whole = offset == 0 && length == at.size;
            let etag = match &b.target {
                FileTarget::Artifact { .. } => at.etag.clone(),
                FileTarget::RootFile { .. } if whole => fsx::hash_bytes(&bytes),
                FileTarget::RootFile { .. } => pages.root_hash(&at.path).ok_or(ViewError::Gone)?,
            };
            let meta = bise_proto::view_rpc::ReadMeta { etag, writable, size: at.size };
            Ok(read_answer(&uri, &at.mime, bytes, whole, meta))
        })();
        done(r.map_err(|e: ViewError| e.rpc(READ)))
    });
}

/// A write's heavy half (the writer thread): the bytes beside the file
/// (fsx), and an artifact's next copy staged in the store. Its root file
/// is re-checked under the root, and its hash now cached for the decide.
fn stage(pages: &Pages, b: &Binding, bytes: &[u8]) -> Result<(fsx::Prepared, Option<PathBuf>), ViewError> {
    match &b.target {
        FileTarget::RootFile { .. } => {
            let at = disk::file_at(&arts(pages), &pages.bise_root, &b.target)?;
            pages.root_hash(&at.path).ok_or(ViewError::Gone)?;
            Ok((fsx::prepare(&at.path, &mut &bytes[..]).map_err(|_| ViewError::Gone)?, None))
        }
        FileTarget::Artifact { id, .. } => {
            let store = arts(pages);
            let m = store.get(id).ok_or(ViewError::Gone)?;
            let p = fsx::prepare(std::path::Path::new(&m.source), &mut &bytes[..]).map_err(|_| ViewError::Gone)?;
            let copy = store.stage_copy(id, bytes).map_err(|_| ViewError::Gone)?;
            Ok((p, Some(copy)))
        }
    }
}

/// A root file's commit: its `ifMatch` against its hash now (the cache's,
/// for its size and mtime now; none: it changed since it was staged, a
/// conflict), then the rename.
fn commit_root(pages: &Pages, path: &std::path::Path, s: &mut Staged) -> Result<(WriteOutcome, Option<FileTarget>), ViewError> {
    let meta = std::fs::metadata(path).map_err(|_| ViewError::Gone)?;
    let stamp = (meta.len(), meta.modified().map_err(|_| ViewError::Gone)?);
    let now = pages.files.lock().unwrap_or_else(|e| e.into_inner()).hash_of(path, stamp);
    let Some(now) = now else { return Ok((WriteOutcome::Conflict { etag: String::new() }, None)) };
    if let Err(c) = crate::view_files::root_decide(s.if_match.as_deref(), &now) {
        return Ok((c, None));
    }
    let p = s.prepared.take().ok_or(ViewError::Gone)?;
    let w = fsx::commit(p, None, Some(&now)).map_err(|_| ViewError::Gone)?;
    if let Ok(m) = std::fs::metadata(path) {
        if let Ok(t) = m.modified() {
            pages.files.lock().unwrap_or_else(|e| e.into_inner()).keep_hash(path, (m.len(), t), &w.hash);
        }
    }
    Ok((WriteOutcome::Saved { etag: w.hash }, None))
}

/// `bytes` beside `source` as `<name> (copy).<ext>`, a free name
/// ([`crate::view_files_disk::free_name`]), never over a file. Its path.
fn write_copy(source: &std::path::Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    use std::io::Write;
    let dir = source.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    let stem = source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = source.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let nth = |n: u32| bise_proto::words::views::copy_nth(n);
    let (p, mut f) = crate::view_files_disk::free_name(dir, &stem, &ext, Some(bise_proto::words::views::copy_first), &nth)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(p)
}
