//! The home hub's id table of `~/bise`, the shell's side
//! (docs/plugin-apps.md A6 ids; architect m_60046, m_60075, m_60095):
//! `files/ids/subscribe` and `files/mint` on hub.sock, the window's own
//! (never a view's: daemon/views.rs refuses them on `/api/view`).
//!
//! - one thread off the loop owns the table ([`Table`]): it loads it from
//!   `Home::files_ids_file()` (a table refused on load moves aside to
//!   `view-ids.broken-<ms>`, one log line, and the walk starts empty),
//!   watches `Home::bise_root()` with FSEvents (`notify`), walks the whole
//!   root at start, then per event batch the changed folders
//!   (`crate::files_ids::covers`); dropped events, MustScanSubDirs or the
//!   watcher's own error are a whole walk again, with one log line;
//! - a walk never crosses the root's device, skips `bise_ids::skipped`
//!   names and symlinks (lstat), and stops at
//!   [`crate::files_ids::WALK_MAX`] entries (then only the folders it
//!   listed whole say what is gone);
//! - each change is saved (tmp file + rename, the one writer) and sent as
//!   one `files_ids` event to the subscribed connections, `seq` one more
//!   each; a subscribe's answer is the whole table at the current `seq`;
//!   both go to the loop in one channel, so no change falls between;
//! - `files/mint {path}` checks the path on disk name by name, exact bytes
//!   (a name the disk spells otherwise is not found), no symlink, the
//!   root's device, then mints (idempotent);
//! - a project hub has no table: it refuses both, naming the home hub.

use super::views::ViewStep;
use super::{log_line, ClientId, Msg, Shell};
use crate::files_ids::{self as ids, WALK_MAX};
use crate::util::now_ms;
use bise_ids::{skipped, tracked, Cover, Entry, Kind, Scan, Snapshot};
use bise_proto::files::{FilesIds, FilesMintResult};
use bise_proto::hub::{HubCmd, HubEv};
use bise_proto::jsonrpc::{Id, RpcError};
use bise_proto::view_rpc::ViewError;
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// Events closer than this are one batch (one apply, one event out).
const BATCH: Duration = Duration::from_millis(50);
/// A batch never waits longer than this for the events to stop.
const BATCH_MAX: Duration = Duration::from_millis(500);

/// The table's side on the loop: its thread's jobs (none: not the home
/// hub, or not started) and the connections subscribed to its changes.
#[derive(Default)]
pub(super) struct Ids {
    jobs: Option<Sender<Job>>,
    subs: BTreeSet<ClientId>,
}

/// One job of the table's thread.
enum Job {
    /// The watcher's event (or its error).
    Fs(notify::Result<notify::Event>),
    Subscribe(ClientId, Id),
    Mint(ClientId, Id, String),
}

impl Shell {
    /// Boot: the home hub starts its table's thread; a project hub has
    /// none.
    pub(super) fn ids_boot(&mut self) {
        if !crate::paths::is_home(&self.opts.paths.workspace) {
            return;
        }
        let (jobs, rx) = channel();
        let (home, paths, tx, watch) = (self.opts.home.clone(), self.opts.paths.clone(), self.tx.clone(), jobs.clone());
        self.ids.jobs = Some(jobs);
        let spawned = std::thread::Builder::new().name("files-ids".into()).spawn(move || {
            let log = |s: &str| log_line(&paths, &format!("files/ids: {s}"));
            let (table, said) = Table::open(&home.bise_root(), home.files_ids_file());
            said.iter().for_each(|s| log(s));
            run(table, rx, watch, &tx, &log);
        });
        if let Err(e) = spawned {
            self.ids.jobs = None;
            log_line(&self.opts.paths, &format!("files/ids: no table thread: {e}"));
        }
    }

    /// The window's `~/bise` methods (hub.sock): the id table's two here;
    /// `folder/list` and `file/new` are not served yet.
    pub(super) fn files_rpc(&mut self, client: ClientId, rid: Id, cmd: HubCmd) {
        let (job, method) = match cmd {
            HubCmd::FilesIdsSubscribe { .. } => (Job::Subscribe(client, rid.clone()), "files/ids/subscribe"),
            HubCmd::FilesMint { path, .. } => (Job::Mint(client, rid.clone(), path), "files/mint"),
            HubCmd::FolderList { .. } => return self.rpc_answer(client, rid, Err(ViewError::NotSupported.rpc("folder/list"))),
            HubCmd::FileNew { .. } => return self.rpc_answer(client, rid, Err(ViewError::NotSupported.rpc("file/new"))),
            _ => return,
        };
        let sent = self.ids.jobs.as_ref().is_some_and(|j| j.send(job).is_ok());
        if !sent {
            let home = bise_home::Home::from_env().bise_root();
            let why = format!("{method}: the id table of {} is the home hub's, not this project's ({})", home.display(), self.project());
            self.rpc_answer(client, rid, Err(RpcError::refused(why, Some("not_home"))));
        }
    }

    /// A connection gone: no more changes for it.
    pub(super) fn ids_gone(&mut self, client: ClientId) {
        self.ids.subs.remove(&client);
    }

    /// A subscribe's answer, the whole table: from now on its changes.
    fn ids_subscribed(&mut self, client: ClientId, rid: Id, full: FilesIds) {
        self.ids.subs.insert(client);
        self.rpc_answer(client, rid, Ok(serde_json::to_value(full).unwrap_or_default()));
    }

    /// One change of the table, to every subscribed connection.
    fn ids_changed(&mut self, delta: FilesIds) {
        let ev = HubEv::FilesIds { project: self.project(), ids: delta };
        for c in self.ids.subs.clone() {
            self.proto_send(c, &ev);
        }
    }
}

/// The table's thread: the watcher first (so nothing falls between), the
/// whole walk, then its jobs in batches until the hub ends.
fn run(mut t: Table, jobs: Receiver<Job>, watch: Sender<Job>, tx: &Sender<Msg>, log: &dyn Fn(&str)) {
    let step = |s: ViewStep| tx.send(Msg::View(s)).is_ok();
    let _watcher = match watcher(&t.root, watch) {
        Ok(w) => Some(w),
        Err(e) => {
            log(&format!("no watcher on {}: {e} (the table changes only by files/mint)", t.root.display()));
            None
        }
    };
    let out = |t: &mut Table, changes: Vec<bise_ids::Change>| {
        if let Some(d) = t.changed(&changes, log) {
            return step(Box::new(move |sh: &mut Shell| sh.ids_changed(d)));
        }
        true
    };
    let changes = t.walk_apply(&[Cover::FULL], log);
    if !out(&mut t, changes) {
        return;
    }
    while let Ok(first) = jobs.recv() {
        let mut held = vec![first];
        if matches!(held[0], Job::Fs(_)) {
            let start = Instant::now();
            while start.elapsed() < BATCH_MAX {
                match jobs.recv_timeout(BATCH) {
                    Ok(j) => held.push(j),
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
                }
            }
        }
        let mut events = Vec::new();
        let mut asks = Vec::new();
        for j in held {
            match j {
                Job::Fs(e) => events.push(e),
                ask => asks.push(ask),
            }
        }
        if !events.is_empty() {
            let covers = t.covers_of(events, log);
            if !covers.is_empty() {
                let changes = t.walk_apply(&covers, log);
                if !out(&mut t, changes) {
                    return;
                }
            }
        }
        for ask in asks {
            let alive = match ask {
                Job::Subscribe(client, rid) => {
                    let full = ids::full(&t.snap, t.seq, &t.root_str());
                    step(Box::new(move |sh: &mut Shell| sh.ids_subscribed(client, rid, full)))
                }
                Job::Mint(client, rid, path) => {
                    let r = t.mint(&path, log);
                    let (answer, changes) = match r {
                        Ok((id, changes)) => (Ok(serde_json::to_value(FilesMintResult { id }).unwrap_or_default()), changes),
                        Err(e) => (Err(e), Vec::new()),
                    };
                    // the change goes out before the answer: a client
                    // that reads its id already has its row
                    out(&mut t, changes) && step(Box::new(move |sh: &mut Shell| sh.rpc_answer(client, rid, answer)))
                }
                Job::Fs(_) => true,
            };
            if !alive {
                return;
            }
        }
    }
}

/// FSEvents on `root`, recursive, each event a job.
fn watcher(root: &Path, jobs: Sender<Job>) -> notify::Result<notify::RecommendedWatcher> {
    use notify::Watcher;
    let mut w = notify::recommended_watcher(move |e| {
        let _ = jobs.send(Job::Fs(e));
    })?;
    w.watch(root, notify::RecursiveMode::Recursive)?;
    Ok(w)
}

/// The id table and what it needs to walk and save: owned by its thread.
pub(super) struct Table {
    /// the canonical `Home::bise_root()`
    root: PathBuf,
    /// its device: a walk never leaves it
    dev: u64,
    file: PathBuf,
    snap: Snapshot,
    /// the last event's number (0: none yet)
    seq: u64,
}

/// A path `files/mint` refuses: its reason and words.
fn refused(reason: &str, path: &str, why: &str) -> RpcError {
    RpcError::refused(format!("files/mint {path:?}: {why}"), Some(reason))
}

impl Table {
    /// The table of `root` saved at `file`, and what to log: a table
    /// refused on load is moved aside and the table starts empty.
    pub(super) fn open(root: &Path, file: PathBuf) -> (Table, Vec<String>) {
        let mut said = Vec::new();
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let dev = std::fs::metadata(&root).map(|m| m.dev()).unwrap_or(0);
        let text = std::fs::read_to_string(&file).ok();
        let snap = match ids::load(text.as_deref()) {
            Ok(s) => s,
            Err(why) => {
                let aside = file.with_file_name(format!("view-ids.broken-{}", now_ms()));
                let moved = std::fs::rename(&file, &aside).map_or_else(|e| format!("not moved: {e}"), |_| format!("moved to {}", aside.display()));
                said.push(format!("the saved table was refused ({why}), {moved}; a whole walk starts it again"));
                Snapshot::new()
            }
        };
        (Table { root, dev, file, snap, seq: 0 }, said)
    }

    fn root_str(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// A batch of watcher events as the folders to read again.
    fn covers_of(&self, events: Vec<notify::Result<notify::Event>>, log: &dyn Fn(&str)) -> Vec<Cover> {
        let mut rescan = None;
        let mut seen = Vec::new();
        for e in events {
            match e {
                Err(e) => rescan = Some(format!("the watcher failed: {e}")),
                Ok(ev) if ev.need_rescan() => rescan = Some(format!("events dropped ({})", ev.info().unwrap_or("must scan subdirs"))),
                Ok(ev) => {
                    for p in &ev.paths {
                        if let Some(rel) = ids::rel_of(&self.root, p) {
                            let folder = std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_dir());
                            seen.push((rel, folder));
                        }
                    }
                }
            }
        }
        if let Some(why) = &rescan {
            log(&format!("{why}: a whole walk of {}", self.root.display()));
        }
        ids::covers(&seen, rescan.is_some())
    }

    /// Reads what `covers` name and takes it in: the changes. A walk that
    /// stops at [`WALK_MAX`] says so in one log line.
    pub(super) fn walk_apply(&mut self, covers: &[Cover], log: &dyn Fn(&str)) -> Vec<bise_ids::Change> {
        let mut w = Walk { root: &self.root, dev: self.dev, scan: Scan::new(), n: 0, capped: false };
        let mut listed = Vec::new();
        for c in covers {
            match c {
                Cover::Dir(d) => {
                    let d = w.existing(d);
                    let mut done = Vec::new();
                    if w.list(&d, false, &mut done) {
                        listed.push(Cover::Dir(d));
                    }
                }
                Cover::Tree(t) if t.is_empty() || w.is_folder(t) => {
                    let mut done = Vec::new();
                    if w.list(t, true, &mut done) {
                        listed.push(Cover::Tree(t.clone()));
                    } else {
                        listed.extend(done.into_iter().map(Cover::Dir));
                    }
                }
                // a folder gone before its walk: its parent says so
                Cover::Tree(t) => {
                    let d = w.existing(ids::parent(t));
                    let mut done = Vec::new();
                    if w.list(&d, false, &mut done) {
                        listed.push(Cover::Dir(d));
                    }
                }
            }
        }
        if w.capped {
            log(&format!("a walk of {} stopped at {} entries: only the folders it listed whole lose rows", self.root.display(), w.n));
        }
        let scan = w.scan;
        self.snap.apply(&scan, &listed)
    }

    /// The id of `rel` (exact bytes under the root), minted if the table
    /// has none, and the changes. Its folders the table lacks are read
    /// in the same job (architect m_63071): no file whose folders have no
    /// id, waiting on an event that may never come.
    pub(super) fn mint(&mut self, rel: &str, log: &dyn Fn(&str)) -> Result<(String, Vec<bise_ids::Change>), RpcError> {
        if !tracked(rel) {
            return Err(refused("bad_request", rel, "not a tracked path under ~/bise"));
        }
        let entry = self.on_disk(rel)?;
        let (id, mut changes) = self.snap.mint(rel, entry).map_err(|e| refused("bad_request", rel, &e.to_string()))?;
        let missing = folders_missing(&self.snap, rel);
        if !missing.is_empty() {
            changes.extend(self.walk_apply(&missing, log));
        }
        Ok((id.to_string(), changes))
    }

    /// `rel` on disk, name by name as its folder lists it (exact bytes:
    /// the disk may find a name spelled otherwise, the table never), no
    /// symlink, on the root's device.
    fn on_disk(&self, rel: &str) -> Result<Entry, RpcError> {
        let gone = || refused("not_found", rel, "no such file under ~/bise");
        let mut at = self.root.clone();
        let names: Vec<&str> = rel.split('/').collect();
        let mut entry = None;
        for (i, name) in names.iter().enumerate() {
            let listed = std::fs::read_dir(&at).map_err(|_| gone())?.flatten().any(|e| e.file_name().as_encoded_bytes() == name.as_bytes());
            if !listed {
                return Err(gone());
            }
            at.push(name);
            let meta = std::fs::symlink_metadata(&at).map_err(|_| gone())?;
            let e = entry_of(&meta).ok_or_else(|| refused("bad_request", rel, "a symlink or a special file"))?;
            if e.dev != self.dev {
                return Err(refused("bad_request", rel, "on another disk than ~/bise"));
            }
            if i + 1 < names.len() && e.kind != Kind::Folder {
                return Err(gone());
            }
            entry = Some(e);
        }
        entry.ok_or_else(gone)
    }

    /// After an apply or a mint: the table saved and the next event, or
    /// none when nothing changed.
    fn changed(&mut self, changes: &[bise_ids::Change], log: &dyn Fn(&str)) -> Option<FilesIds> {
        let delta = ids::delta(&self.snap, changes, self.seq + 1)?;
        self.seq += 1;
        if let Err(e) = self.save() {
            log(&format!("not saved to {}: {e}", self.file.display()));
        }
        Some(delta)
    }

    /// The table to its file: a temp file beside it, then a rename.
    fn save(&self) -> std::io::Result<()> {
        if let Some(d) = self.file.parent() {
            std::fs::create_dir_all(d)?;
        }
        let text = serde_json::to_string(&self.snap).map_err(std::io::Error::other)?;
        let tmp = self.file.with_extension(format!("tmp-{}", std::process::id()));
        // synced before the rename: a table lost to a power cut would give
        // every file a new id (architect m_63071)
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()
        };
        write().inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        std::fs::rename(&tmp, &self.file).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

/// A file's or a folder's lstat as a table entry; a symlink or anything
/// else: none.
fn entry_of(m: &std::fs::Metadata) -> Option<Entry> {
    let ft = m.file_type();
    let kind = if ft.is_dir() {
        Kind::Folder
    } else if ft.is_file() {
        Kind::File
    } else {
        return None;
    };
    let mtime_ms = (m.mtime().max(0) as u64).saturating_mul(1000) + (m.mtime_nsec().max(0) as u64) / 1_000_000;
    let size = if kind == Kind::File { m.size() } else { 0 };
    Some(Entry { dev: m.dev(), ino: m.ino(), kind, size, mtime_ms })
}

/// The folders of `rel` the table lacks, as the covers that read them:
/// each one's parent listed (so `a/b/c.md` with no `a` reads `""` and
/// `a`).
fn folders_missing(snap: &Snapshot, rel: &str) -> Vec<Cover> {
    let mut covers = Vec::new();
    let mut d = ids::parent(rel);
    while !d.is_empty() && snap.id_of(d).is_none() {
        covers.push(Cover::Dir(ids::parent(d).to_string()));
        d = ids::parent(d);
    }
    covers.reverse();
    covers
}

/// One walk: what it read, up to [`WALK_MAX`] entries.
struct Walk<'a> {
    root: &'a Path,
    dev: u64,
    scan: Scan,
    n: usize,
    /// it stopped at [`WALK_MAX`]
    capped: bool,
}

impl Walk<'_> {
    fn abs(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.to_path_buf()
        } else {
            self.root.join(rel)
        }
    }

    fn is_folder(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.abs(rel)).is_ok_and(|m| m.file_type().is_dir())
    }

    /// `rel`, or its nearest folder that still exists (the root last).
    fn existing(&self, rel: &str) -> String {
        let mut d = rel;
        while !d.is_empty() && !self.is_folder(d) {
            d = ids::parent(d);
        }
        d.to_string()
    }

    /// Lists folder `dir` (`deep`: and every folder under it) into the
    /// scan; `done` gets each folder listed whole. Whether all of it was.
    fn list(&mut self, dir: &str, deep: bool, done: &mut Vec<String>) -> bool {
        let Ok(rd) = std::fs::read_dir(self.abs(dir)) else { return false };
        let mut whole = true;
        let mut sub = Vec::new();
        for ent in rd {
            if self.n >= WALK_MAX {
                self.capped = true;
                return false;
            }
            let Ok(ent) = ent else {
                whole = false;
                continue;
            };
            let name = ent.file_name();
            let Some(name) = name.to_str() else { continue };
            if skipped(name) {
                continue;
            }
            let rel = if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
            let Some(e) = std::fs::symlink_metadata(ent.path()).ok().as_ref().and_then(entry_of) else { continue };
            if e.dev != self.dev || self.scan.insert(&rel, e).is_err() {
                continue;
            }
            self.n += 1;
            if deep && e.kind == Kind::Folder {
                sub.push(rel);
            }
        }
        done.push(dir.to_string());
        for s in sub {
            whole &= self.list(&s, true, done);
        }
        whole
    }
}

#[cfg(test)]
#[path = "files_ids_tests.rs"]
mod tests;
