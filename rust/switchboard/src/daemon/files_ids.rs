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

use super::art_rows::Links;
use super::views::ViewStep;
use super::{log_line, ClientId, Msg, Shell};
use crate::files_ids::{self as ids, WALK_MAX};
use crate::util::now_ms;
use bend_plugins::opens::{self, Target};
use bise_ids::{skipped, tracked, Cover, Entry, Kind, Scan, Snapshot};
use bise_proto::files::{Crumb, EntryArtifact, FileKind, FilesIds, FilesMintResult, FolderList, FolderRow, Names};
use bise_proto::hub::{HubCmd, HubEv};
use bise_proto::jsonrpc::{Id, RpcError};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
    /// `folder/list`: the folder's id (none or the root's: the root) and
    /// the artifacts' current files of the last rows' build.
    List(ClientId, Id, Option<String>, Links),
    /// `view/open {kind: entry}`: where an id is now (none: unknown),
    /// answered to the asker's thread at once, a whole walk or not.
    Path(String, Sender<Option<PathBuf>>),
    /// `file/new`: its make found and its template read off the loop
    /// (daemon/file_new.rs); the file, its id and the store's copy here.
    New(ClientId, Id, Box<super::file_new::NewFile>),
}

/// How long `view/open` waits for an id's path before it answers busy
/// (architect m_63160: never a guess, never a stale path).
pub(super) const PATH_WAIT: Duration = Duration::from_secs(2);

/// The root's crumb id in `folder/list`'s answer: the root has no row in
/// the table; sent back as `folder`, it lists the root (as none does).
const ROOT_CRUMB: &str = "";

/// Asks the table's thread where an entry is, from any thread
/// (`view/open`'s, off the loop).
#[derive(Clone)]
pub(super) struct IdsAsk(Sender<Job>);

/// Why an entry has no path: unknown (or not this hub's), or the table's
/// thread did not answer in [`PATH_WAIT`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum NoPath {
    Unknown,
    Busy,
}

impl IdsAsk {
    /// `file/new`'s file to make, from its thread; false: the table's
    /// thread is gone.
    pub(super) fn new_file(&self, client: ClientId, rid: Id, n: super::file_new::NewFile) -> bool {
        self.0.send(Job::New(client, rid, Box::new(n))).is_ok()
    }

    /// The absolute path of entry `id` now.
    pub(super) fn path(&self, id: &str, wait: Duration) -> Result<PathBuf, NoPath> {
        let (tx, rx) = channel();
        self.0.send(Job::Path(id.to_string(), tx)).map_err(|_| NoPath::Busy)?;
        match rx.recv_timeout(wait) {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Err(NoPath::Unknown),
            Err(_) => Err(NoPath::Busy),
        }
    }
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

    /// The window's `~/bise` methods (hub.sock): the id table's two and
    /// `folder/list`; `file/new` is not served yet.
    pub(super) fn files_rpc(&mut self, client: ClientId, rid: Id, cmd: HubCmd) {
        let (job, method) = match cmd {
            HubCmd::FilesIdsSubscribe { .. } => (Job::Subscribe(client, rid.clone()), "files/ids/subscribe"),
            HubCmd::FilesMint { path, .. } => (Job::Mint(client, rid.clone(), path), "files/mint"),
            // no dotfile rows until there is a use, never a quiet false (architect m_63160)
            HubCmd::FolderList { hidden: true, .. } => {
                let why = format!("folder/list: {}", bise_proto::words::views::hidden_not_listed);
                return self.rpc_answer(client, rid, Err(RpcError::refused(why, Some("bad_request"))));
            }
            HubCmd::FolderList { folder, .. } => (Job::List(client, rid.clone(), folder, self.art_links()), "folder/list"),
            HubCmd::FileNew { make, library_folder, in_folder, .. } => return self.file_new(client, rid, make, library_folder, in_folder),
            _ => return,
        };
        let sent = self.ids.jobs.as_ref().is_some_and(|j| j.send(job).is_ok());
        if !sent {
            self.rpc_answer(client, rid, Err(self.not_home(method)));
        }
    }

    /// A `~/bise` method asked of a project hub: refused, naming the home.
    pub(super) fn not_home(&self, method: &str) -> RpcError {
        let home = bise_home::Home::from_env().bise_root();
        let why = format!("{method}: the id table of {} is the home hub's, not this project's ({})", home.display(), self.project());
        RpcError::refused(why, Some("not_home"))
    }

    /// Where `view/open` asks an entry's path (none: not the home hub).
    pub(super) fn ids_ask(&self) -> Option<IdsAsk> {
        self.ids.jobs.clone().map(IdsAsk)
    }

    /// A folder's rows from the table's thread, each with the app that
    /// opens it and the others (his picks, from memory), then the answer.
    fn folder_listed(&mut self, client: ClientId, rid: Id, mut list: FolderList, targets: Vec<Target>) {
        let project = self.project();
        for (row, target) in list.rows.iter_mut().zip(&targets) {
            let o = opens::pick(target, self.apps.openers.openers(), self.apps.openers.picks());
            row.openers = o.opener.iter().cloned().chain(o.others).collect();
            row.opener = o.opener;
            if let Some(a) = &mut row.artifact {
                a.project = project.clone();
            }
        }
        self.rpc_answer(client, rid, Ok(serde_json::to_value(list).unwrap_or_default()));
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
    let _watcher = match watcher(&t.root, watch) {
        Ok(w) => Some(w),
        Err(e) => {
            log(&format!("no watcher on {}: {e} (the table changes only by files/mint)", t.root.display()));
            None
        }
    };
    let o = Out { tx, log };
    // jobs a walk held back (between its folders it answers Path and
    // List only): they go first after it
    let mut later: VecDeque<Job> = VecDeque::new();
    let walk = |t: &mut Table, covers: &[Cover], later: &mut VecDeque<Job>| {
        let changes = t.walk_apply(covers, log, &mut |t: &mut Table| between(t, &jobs, &o, later));
        o.changes(t, changes)
    };
    if !walk(&mut t, &[Cover::FULL], &mut later) {
        return;
    }
    loop {
        let first = match later.pop_front() {
            Some(j) => j,
            None => match jobs.recv() {
                Ok(j) => j,
                Err(_) => return,
            },
        };
        let mut held = vec![first];
        held.extend(later.drain(..));
        if held.iter().any(|j| matches!(j, Job::Fs(_))) {
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
            if !covers.is_empty() && !walk(&mut t, &covers, &mut later) {
                return;
            }
        }
        for j in asks {
            if !ask(&mut t, j, &o) {
                return;
            }
        }
    }
}

/// Between two folders of a walk: the jobs that can't wait a whole walk
/// (`Path`, `List`) answered now, the others held in `later` for after
/// it (architect m_63160: an open during a 20k rescan never times out).
fn between(t: &mut Table, jobs: &Receiver<Job>, o: &Out, later: &mut VecDeque<Job>) {
    while let Ok(j) = jobs.try_recv() {
        match j {
            j @ (Job::Path(..) | Job::List(..)) => {
                ask(t, j, o);
            }
            j => later.push_back(j),
        }
    }
}

/// The thread's way to the loop: its steps, its log.
struct Out<'a> {
    tx: &'a Sender<Msg>,
    log: &'a dyn Fn(&str),
}

impl Out<'_> {
    /// A step for the loop; false: the hub ends.
    fn step(&self, s: ViewStep) -> bool {
        self.tx.send(Msg::View(s)).is_ok()
    }

    /// The table's changes saved and sent (nothing changed: nothing).
    fn changes(&self, t: &mut Table, changes: Vec<bise_ids::Change>) -> bool {
        match t.changed(&changes, self.log) {
            Some(d) => self.step(Box::new(move |sh: &mut Shell| sh.ids_changed(d))),
            None => true,
        }
    }
}

/// One job that is not the watcher's, answered; false: the hub ends.
fn ask(t: &mut Table, job: Job, o: &Out) -> bool {
    match job {
        Job::Subscribe(client, rid) => {
            let full = ids::full(&t.snap, t.seq, &t.root_str());
            o.step(Box::new(move |sh: &mut Shell| sh.ids_subscribed(client, rid, full)))
        }
        Job::Mint(client, rid, path) => {
            let (answer, changes) = match t.mint(&path, o.log) {
                Ok((id, changes)) => (Ok(serde_json::to_value(FilesMintResult { id }).unwrap_or_default()), changes),
                Err(e) => (Err(e), Vec::new()),
            };
            // the change goes out before the answer: a client that reads
            // its id already has its row
            o.changes(t, changes) && o.step(Box::new(move |sh: &mut Shell| sh.rpc_answer(client, rid, answer)))
        }
        Job::List(client, rid, folder, links) => {
            let (listed, changes) = t.list(folder.as_deref(), &links, o.log);
            o.changes(t, changes)
                && o.step(Box::new(move |sh: &mut Shell| match listed {
                    Ok((list, targets)) => sh.folder_listed(client, rid, list, targets),
                    Err(e) => sh.rpc_answer(client, rid, Err(e)),
                }))
        }
        Job::Path(id, reply) => {
            let _ = reply.send(t.path(&id));
            true
        }
        Job::New(client, rid, n) => {
            let made = t.new_file(n.folder.as_deref(), &n.title, &n.ending, &n.bytes, o.log);
            let (made, changes) = match made {
                Ok((path, entry, changes)) => (Ok((path, entry)), changes),
                Err(e) => (Err(e), Vec::new()),
            };
            // the store's copy staged here, so the loop only renames it in
            let made = made.map(|(path, entry)| {
                let staged = crate::artifacts::Store::new(&n.state).stage_new(&n.bytes).ok();
                (path, entry, staged)
            });
            o.changes(t, changes)
                && o.step(Box::new(move |sh: &mut Shell| match made {
                    Ok((path, entry, staged)) => sh.file_made(client, rid, &path, entry, staged, n.library_folder),
                    Err(e) => sh.rpc_answer(client, rid, Err(e)),
                }))
        }
        Job::Fs(_) => true,
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
    /// The newest listing wins ([`fresher_wins`]): one number per apply,
    /// and while a walk runs, the folders a walk inside it (a `List`
    /// between its folders) applied, at that apply's number. Emptied when
    /// the outer walk ends; `depth`: the walks running.
    listing: u64,
    fresh: BTreeMap<String, u64>,
    depth: u32,
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
        (Table { root, dev, file, snap, seq: 0, listing: 0, fresh: BTreeMap::new(), depth: 0 }, said)
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
    ///
    /// `between` runs before each folder it reads, with the table as it
    /// was before this walk: the thread answers the jobs that can't wait
    /// a whole walk there (architect m_63160: an open during a 20k
    /// rescan). A `folder/list` answered there applies its own folder at
    /// once; this walk's apply comes after it, and in each folder the
    /// newest listing wins ([`fresher_wins`], architect m_63320).
    pub(super) fn walk_apply(&mut self, covers: &[Cover], log: &dyn Fn(&str), between: &mut dyn FnMut(&mut Table)) -> Vec<bise_ids::Change> {
        self.depth += 1;
        let mut w = Walk { root: self.root.clone(), dev: self.dev, read: BTreeMap::new(), n: 0, capped: false };
        let mut listed = Vec::new();
        for c in covers {
            match c {
                Cover::Dir(d) => {
                    let d = w.existing(d);
                    let mut done = Vec::new();
                    if w.list(&d, false, &mut done, self, between) {
                        listed.push(Cover::Dir(d));
                    }
                }
                Cover::Tree(t) if t.is_empty() || w.is_folder(t) => {
                    let mut done = Vec::new();
                    if w.list(t, true, &mut done, self, between) {
                        listed.push(Cover::Tree(t.clone()));
                    } else {
                        listed.extend(done.into_iter().map(Cover::Dir));
                    }
                }
                // a folder gone before its walk: its parent says so
                Cover::Tree(t) => {
                    let d = w.existing(ids::parent(t));
                    let mut done = Vec::new();
                    if w.list(&d, false, &mut done, self, between) {
                        listed.push(Cover::Dir(d));
                    }
                }
            }
        }
        if w.capped {
            log(&format!("a walk of {} stopped at {} entries: only the folders it listed whole lose rows", self.root.display(), w.n));
        }
        let snap = &self.snap;
        let (scan, listed, read) = fresher_wins(listed, w.read, &self.fresh, &|d| snap.id_of(d).is_some());
        let changes = self.snap.apply(&scan, &listed);
        self.depth -= 1;
        self.listing += 1;
        if self.depth == 0 {
            self.fresh.clear();
        } else {
            for d in read {
                self.fresh.insert(d, self.listing);
            }
        }
        changes
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
            changes.extend(self.walk_apply(&missing, log, &mut |_| {}));
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

    /// Where entry `id` is now (absolute; none: not an id of this table).
    pub(super) fn path(&self, id: &str) -> Option<PathBuf> {
        let rel = self.snap.path_of(id.parse().ok()?)?;
        Some(self.root.join(rel))
    }

    /// The table path of folder `folder` (an id; none or the root's crumb:
    /// the root, `""`); an unknown id or a file: refused (bad_request).
    fn folder_rel(&self, method: &str, folder: Option<&str>) -> Result<String, RpcError> {
        let bad = |f: &str, why: &str| RpcError::refused(format!("{method} {f:?}: {why}"), Some("bad_request"));
        let Some(f) = folder.filter(|f| *f != ROOT_CRUMB) else { return Ok(String::new()) };
        let at = f.parse().ok().and_then(|id| self.snap.path_of(id)).map(str::to_string);
        match at.as_deref().and_then(|r| self.snap.get(r)) {
            Some((_, e)) if e.kind == Kind::Folder => Ok(at.unwrap_or_default()),
            Some(_) => Err(bad(f, "not a folder")),
            None => Err(bad(f, "no such folder of ~/bise")),
        }
    }

    /// `file/new`'s file (A4b): `bytes` (its template's, or none) in
    /// folder `folder` (as [`Table::folder_rel`]) under the first free
    /// name of `title` + `ending` (never over a file), then its id. Its
    /// absolute path, its id and the changes (they go out first).
    pub(super) fn new_file(&mut self, folder: Option<&str>, title: &str, ending: &str, bytes: &[u8], log: &dyn Fn(&str)) -> Result<(PathBuf, String, Vec<bise_ids::Change>), RpcError> {
        use std::io::Write;
        const WHAT: &str = "file/new";
        let rel = self.folder_rel(WHAT, folder)?;
        let gone = || RpcError::refused(format!("{WHAT}: {rel:?} is gone from ~/bise"), Some("not_found"));
        let dir = if rel.is_empty() { self.root.clone() } else { self.root.join(&rel) };
        let nth = |n: u32| bise_proto::words::views::new_nth(n);
        let (path, mut f) = crate::view_files_disk::free_name(&dir, title, ending, None, &nth).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => gone(),
            _ => RpcError::refused(format!("{WHAT}: {e}"), Some("server_error")),
        })?;
        let wrote = f.write_all(bytes).and_then(|()| f.sync_all());
        if let Err(e) = wrote {
            let _ = std::fs::remove_file(&path);
            return Err(RpcError::refused(format!("{WHAT}: {e}"), Some("server_error")));
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let at = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let (id, changes) = self.mint(&at, log)?;
        Ok((path, id, changes))
    }

    /// `folder/list`: folder `folder` (an id; none or the root's crumb:
    /// the root) read again from disk, then its rows from the table, each
    /// with its matcher's target (the loop adds the apps), and the
    /// changes that read made (they go out first). `links`: the
    /// artifacts' current files, a row that is one says so.
    #[allow(clippy::type_complexity)]
    pub(super) fn list(&mut self, folder: Option<&str>, links: &Links, log: &dyn Fn(&str)) -> (Result<(FolderList, Vec<Target>), RpcError>, Vec<bise_ids::Change>) {
        let rel = match self.folder_rel("folder/list", folder) {
            Ok(r) => r,
            Err(e) => return (Err(e), Vec::new()),
        };
        let changes = self.walk_apply(&[Cover::Dir(rel.clone())], log, &mut |_| {});
        if !rel.is_empty() && self.snap.id_of(&rel).is_none() {
            let gone = RpcError::refused(format!("folder/list: {rel:?} is gone from ~/bise"), Some("not_found"));
            return (Err(gone), changes);
        }
        let crumb_of = |r: &str| Crumb { id: self.snap.id_of(r).map(|i| i.to_string()).unwrap_or_default(), name: name_of(r).to_string() };
        let root = Crumb { id: ROOT_CRUMB.to_string(), name: String::new() };
        let (folder, crumb) = if rel.is_empty() {
            (root, Vec::new())
        } else {
            let mut up = Vec::new();
            let mut d = ids::parent(&rel);
            while !d.is_empty() {
                up.push(crumb_of(d));
                d = ids::parent(d);
            }
            up.push(root);
            up.reverse();
            (crumb_of(&rel), up)
        };
        let mut rows: Vec<(FolderRow, Target)> = self.snap.children(&rel).filter(|(p, _, _)| !skipped(name_of(p))).map(|(p, id, e)| self.row(p, id, e, links)).collect();
        rows.sort_by(|(a, _), (b, _)| (a.kind != FileKind::Folder, a.name.to_lowercase(), &a.name).cmp(&(b.kind != FileKind::Folder, b.name.to_lowercase(), &b.name)));
        let (rows, targets) = rows.into_iter().unzip();
        (Ok((FolderList { folder, crumb, rows }, targets)), changes)
    }

    /// One row of `folder/list` and its matcher's target (A2): a folder
    /// with its first-level names as the table has them, a file under a
    /// plugin's documents (`Plugins/<plugin>/…`) with that plugin.
    fn row(&self, rel: &str, id: bise_ids::Id, e: Entry, links: &Links) -> (FolderRow, Target) {
        let name = name_of(rel).to_string();
        let artifact = links.get(&self.root.join(rel)).map(|a| EntryArtifact { project: String::new(), id: a.clone() });
        let documents = rel.strip_prefix("Plugins/").and_then(|r| r.split_once('/')).map(|(p, under)| (p.to_string(), under.to_string()));
        let (kind, names, matched) = if e.kind == Kind::Folder {
            let names = Names::of(self.snap.children(rel).map(|(c, _, _)| name_of(c).to_string()), skipped);
            let matched = if names.more { opens::Kind::FolderCut(names.names.clone()) } else { opens::Kind::Folder(names.names.clone()) };
            (FileKind::Folder, names, matched)
        } else {
            (FileKind::File, Names::default(), opens::Kind::File)
        };
        let size = if kind == FileKind::File { e.size } else { 0 };
        let row = FolderRow { id: id.to_string(), name: name.clone(), kind, size, mtime: e.mtime_ms, artifact, names, ..Default::default() };
        (row, Target { name, kind: matched, documents })
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

/// A table path's last name.
fn name_of(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
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

/// What one walk read: folder -> (the table's listing number when it read
/// it, the tracked entries it saw in it).
type Read = BTreeMap<String, (u64, Vec<(String, Entry)>)>;

/// Per folder, the newest listing wins (architect m_63320). A walk's
/// apply comes after the `List`s answered between its folders: a folder
/// one of them applied after this walk read it (`fresh` newer than its
/// listing number) is fresher in the table than in this walk's read, so
/// its entries and its cover are dropped, else a file made since would
/// lose its id (removed, then added again) and one deleted since would
/// come back. A folder under a dropped one that the table no longer has
/// (`known`: has a row) is dropped too. A `Tree` cover with a dropped
/// folder under it becomes its folders' `Dir`s. The scan to apply, its
/// covers, and the folders they list (each one's listing is now this
/// apply's).
fn fresher_wins(covers: Vec<Cover>, read: Read, fresh: &BTreeMap<String, u64>, known: &dyn Fn(&str) -> bool) -> (Scan, Vec<Cover>, Vec<String>) {
    let mut stale = BTreeSet::new();
    // parents sort before their folders: a folder's parent is decided first
    for (d, (at, _)) in &read {
        let newer = fresh.get(d).is_some_and(|f| f > at);
        let lost = !d.is_empty() && stale.contains(ids::parent(d)) && !known(d);
        if newer || lost {
            stale.insert(d.clone());
        }
    }
    let under = |t: &str, d: &str| t.is_empty() || d == t || d.strip_prefix(t).is_some_and(|r| r.starts_with('/'));
    let mut out = Vec::new();
    for c in covers {
        match c {
            Cover::Dir(d) if stale.contains(&d) => {}
            Cover::Tree(t) if stale.iter().any(|s| under(&t, s)) => {
                out.extend(read.keys().filter(|d| under(&t, d) && !stale.contains(*d)).map(|d| Cover::Dir(d.clone())));
            }
            c => out.push(c),
        }
    }
    let listed = |d: &str| out.iter().any(|c| matches!(c, Cover::Dir(x) if x == d) || matches!(c, Cover::Tree(t) if under(t, d)));
    let mut scan = Scan::new();
    let mut folders = Vec::new();
    for (d, (_, seen)) in read {
        if stale.contains(&d) {
            continue;
        }
        if listed(&d) {
            folders.push(d);
        }
        for (rel, e) in seen {
            // tracked already: the walk checked
            let _ = scan.insert(&rel, e);
        }
    }
    (scan, out, folders)
}

/// One walk: what it read, up to [`WALK_MAX`] entries.
struct Walk {
    /// the table's root (a copy: the walk lends the table to `between`)
    root: PathBuf,
    dev: u64,
    /// each folder read: the table's listing number when it was read and
    /// the entries seen in it ([`fresher_wins`] makes the scan)
    read: Read,
    n: usize,
    /// it stopped at [`WALK_MAX`]
    capped: bool,
}

impl Walk {
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
    /// `between(t)` runs before each folder (see [`Table::walk_apply`]).
    fn list(&mut self, dir: &str, deep: bool, done: &mut Vec<String>, t: &mut Table, between: &mut dyn FnMut(&mut Table)) -> bool {
        between(t);
        let Ok(rd) = std::fs::read_dir(self.abs(dir)) else { return false };
        let mut whole = true;
        let mut sub = Vec::new();
        let (at, seen) = self.read.entry(dir.to_string()).or_default();
        *at = t.listing;
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
            if e.dev != self.dev || !tracked(&rel) {
                continue;
            }
            seen.push((rel.clone(), e));
            self.n += 1;
            if deep && e.kind == Kind::Folder {
                sub.push(rel);
            }
        }
        done.push(dir.to_string());
        for s in sub {
            whole &= self.list(&s, true, done, t, between);
        }
        whole
    }
}

#[cfg(test)]
#[path = "files_ids_tests.rs"]
mod tests;
