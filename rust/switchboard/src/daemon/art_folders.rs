//! Artifacts in folders on the hub's side (docs/artifacts.md, "Folders"):
//! the library read for the `artifacts` event (cached by the file's
//! mtime, watched on the tick so his edit in the desktop core reaches
//! this hub's clients), `artifacts/move` (this hub's artifacts, or
//! forwarded to another project's hub as `xartmove`, started if needed),
//! `artifacts/folders` (his folder changes, `bise_home::library::run`)
//! and `artifacts/library` (the other projects' rows, read off the loop).
//!
//! This hub writes only its own store's `folder` (meta.json, a page's in
//! `artifacts/filed.json`); the library file through
//! `bise_home::library`, its one writer.
//!
//! His undo slot: a typed `artifacts/move` here records each artifact's
//! folder before; with `keep_undo` it leaves the slot to its caller (the
//! desktop core's move across projects records one slot for the whole
//! move from its items' `from`, core/library.rs).

use super::*;
use crate::artifacts::{self, Store};
use bise_home::library::{self, Library, Move};
use bise_proto::hub::HubEv;
use bise_proto::rows::{ArtifactMoved, Folder, FolderOp, LibraryProject};
use std::time::SystemTime;

/// The library as last read, and the file's mtime then.
#[derive(Default)]
pub(super) struct LibCache {
    read: std::cell::RefCell<(Option<SystemTime>, Library)>,
    /// The mtime the tick last saw (a change sends the list again).
    seen: Option<SystemTime>,
}

/// The library file's stamp (the typed artifact rows' key, daemon/art.rs).
pub(super) fn library_stamp(home: &bise_home::Home) -> Option<SystemTime> {
    mtime(home)
}

fn mtime(home: &bise_home::Home) -> Option<SystemTime> {
    let f = home.library_folders();
    std::fs::metadata(f).ok()?.modified().ok()
}

/// The library's live folders as the protocol's rows.
pub(super) fn folder_rows(lib: &Library) -> Vec<Folder> {
    library::live(lib)
        .map(|f| Folder { id: f.id.clone(), name: f.name.clone(), parent: f.parent.clone(), by: f.by.clone(), named_by_you: f.named_by_you })
        .collect()
}

/// The protocol's op as the library's.
fn op_of(op: FolderOp) -> library::Op {
    match op {
        FolderOp::Make { name, parent } => library::Op::Make { name, parent },
        FolderOp::Rename { id, name } => library::Op::Rename { id, name },
        FolderOp::Nest { id, parent } => library::Op::Nest { id, parent },
        FolderOp::Merge { from, into } => library::Op::Merge { from, into },
        FolderOp::Delete { id } => library::Op::Delete { id },
        FolderOp::Undo => library::Op::Undo,
    }
}

/// The registry's rows but this hub's: (hub id, name, workspace).
fn other_projects(home: &bise_home::Home, own: &str) -> Vec<(String, String, PathBuf)> {
    bise_home::projects::list(home, &bise_home::Home::from_env().bise_root())
        .into_iter()
        .filter(|r| r.id != own && r.path.is_dir())
        .map(|r| (r.id, r.name, r.path))
        .collect()
}

impl Shell {
    /// The library now (read again only when its file's mtime moved).
    pub(super) fn art_library(&self) -> Library {
        let m = mtime(&self.opts.home);
        let mut c = self.art.lib.read.borrow_mut();
        if c.0 != m || m.is_none() {
            *c = (m, library::read(&self.opts.home));
        }
        c.1.clone()
    }

    /// The tick (architect m_39721): while a typed client is connected,
    /// the library file changed since the last look sends the list again.
    pub(super) fn library_watch(&mut self) {
        if self.typed_ids().is_empty() {
            return;
        }
        let m = mtime(&self.opts.home);
        if m != self.art.lib.seen {
            self.art.lib.seen = m;
            self.artifacts_refresh(false);
        }
    }

    /// File this hub's `ids` in `folder` (a library folder's id or name,
    /// resolved; none: unfiled), all or none. Each one's folder before.
    pub(super) fn art_move(&mut self, ids: &[String], folder: Option<&str>) -> Result<Vec<(String, Option<String>)>, String> {
        let lib = self.art_library();
        let to = match folder {
            Some(f) => Some(library::find(&lib, f).ok_or_else(|| format!("no folder {f}"))?.id.clone()),
            None => None,
        };
        let moved = self.art_store().move_to(ids, to.as_deref())?;
        self.artifacts_refresh(true);
        Ok(moved)
    }

    /// `artifacts/move` (his): this hub's artifacts, or `to`'s, forwarded
    /// (off the loop) to its hub; his undo gets their folders before.
    /// `undo`: his undo slot gets the move (the core keeps it itself).
    pub(super) fn art_move_typed(&mut self, id: ClientId, tag: &str, ids: Vec<String>, folder: Option<String>, to: Option<String>, undo: bool) {
        let own = self.project();
        let target = match to.filter(|t| !t.is_empty() && *t != own) {
            None => None,
            Some(t) => {
                let rows = bise_home::projects::list(&self.opts.home, &bise_home::Home::from_env().bise_root());
                match bise_home::projects::target(&rows, &t, &own) {
                    Ok(hub) => Some(hub),
                    Err(e) => return self.proto_error(id, tag, &e),
                }
            }
        };
        let Some(hub) = target else {
            return match self.art_move(&ids, folder.as_deref()) {
                Ok(moved) => {
                    if undo {
                        remember(&self.opts.home, &own, &moved);
                    }
                    self.proto_send(id, &moved_ev(&own, moved));
                }
                Err(e) => self.proto_error(id, tag, &e),
            };
        };
        let (tx, exe, root, tag, home) = (self.tx.clone(), self.opts.exe.clone(), self.opts.app_root.clone(), tag.to_string(), self.opts.home.clone());
        std::thread::spawn(move || {
            let req = json!({"op": "xartmove", "ids": ids, "folder": folder});
            let ev = match super::xhub::call_hub(&home, &hub, &exe, &root, &req) {
                Ok(v) => {
                    let moved: Vec<(String, Option<String>)> = serde_json::from_value(v["moved"].clone()).unwrap_or_default();
                    if undo {
                        remember(&home, &hub, &moved);
                    }
                    moved_ev(&own, moved).to_value()
                }
                Err(e) => json!({"ev": "error", "project": own, "cmd": tag, "text": e}),
            };
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: ev });
        });
    }

    /// `{"op":"xartmove"}` from another hub (his move there of this
    /// hub's artifacts): `{ok, moved}` or `{ok: false, error}`.
    pub(super) fn xartmove(&mut self, mut stream: std::os::unix::net::UnixStream, v: &Value) {
        let ids: Vec<String> = serde_json::from_value(v["ids"].clone()).unwrap_or_default();
        let folder = v.get("folder").and_then(Value::as_str);
        let body = match self.art_move(&ids, folder) {
            Ok(moved) => json!({"ok": true, "moved": moved}),
            Err(e) => json!({"ok": false, "error": e}),
        };
        write_json(&mut stream, &body);
    }

    /// `artifacts/folders` (his change in the terminal): the library's one
    /// writer; an undo's moves of this hub's artifacts done here, the
    /// others' forwarded. `folders` answers.
    pub(super) fn art_folders_typed(&mut self, id: ClientId, tag: &str, op: FolderOp) {
        match library::run(&self.opts.home, op_of(op)) {
            Ok(done) => {
                self.undo_moves(done.moves);
                self.artifacts_refresh(false);
                let folders = folder_rows(&self.art_library());
                self.proto_send(id, &HubEv::Folders { project: self.project(), folders, made: done.made });
            }
            Err(e) => self.proto_error(id, tag, &e),
        }
    }

    /// An undo's moves back: this hub's here, each other project's to
    /// its hub (off the loop; a failure is logged, the rest goes on).
    fn undo_moves(&mut self, moves: Vec<Move>) {
        let own = self.project();
        let mut by: BTreeMap<String, BTreeMap<Option<String>, Vec<String>>> = BTreeMap::new();
        for m in moves {
            by.entry(m.project).or_default().entry(m.folder).or_default().push(m.id);
        }
        let mut away = Vec::new();
        for (project, groups) in by {
            for (folder, ids) in groups {
                if project == own {
                    if let Err(e) = self.store_move_raw(&ids, folder.as_deref()) {
                        log_line(&self.opts.paths, &format!("artifacts: undo of a move: {e}"));
                    }
                } else {
                    away.push((project.clone(), json!({"op": "xartmove", "ids": ids, "folder": folder})));
                }
            }
        }
        if away.is_empty() {
            return;
        }
        let (exe, root, log, home) = (self.opts.exe.clone(), self.opts.app_root.clone(), self.opts.paths.clone(), self.opts.home.clone());
        std::thread::spawn(move || {
            for (hub, req) in away {
                if let Err(e) = super::xhub::call_hub(&home, &hub, &exe, &root, &req) {
                    log_line(&log, &format!("artifacts: undo of a move in {hub}: {e}"));
                }
            }
        });
    }

    /// An undo's move: the folder id as it was (it may be a redirect now:
    /// the readers resolve it), never looked up by name.
    fn store_move_raw(&mut self, ids: &[String], folder: Option<&str>) -> Result<(), String> {
        self.art_store().move_to(ids, folder)?;
        self.artifacts_refresh(true);
        Ok(())
    }

    /// `sb artifact list --project <p>`: this hub's own (its name or id)
    /// at once; another registered project's store read once off the loop
    /// (like artifacts/library, never watched), its answer written from
    /// that thread.
    pub(super) fn art_list(&mut self, mut stream: std::os::unix::net::UnixStream, from: &str, r: bise_proto::sb::Artifact) {
        let away = match &r {
            bise_proto::sb::Artifact::List { project: Some(p), .. } => {
                let own = self.project();
                let rows = bise_home::projects::list(&self.opts.home, &bise_home::Home::from_env().bise_root());
                let p = p.trim().trim_start_matches('@');
                match rows.iter().find(|x| x.name == p).or_else(|| rows.iter().find(|x| x.id == p)) {
                    _ if p.is_empty() => None,
                    Some(row) if row.id == own => None,
                    Some(row) => Some(Ok(row.path.clone())),
                    None => Some(Err(format!("no project named {p} (sb project list)"))),
                }
            }
            _ => None,
        };
        let ws = match away {
            None => {
                write_json(&mut stream, &self.artifact_cmd(from, r));
                return;
            }
            Some(Err(e)) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
                return;
            }
            Some(Ok(ws)) => ws,
        };
        let bise_proto::sb::Artifact::List { words, agent, mut agents, since, archived, .. } = r else { return };
        // TODO(artifacts-list-agent): an older sb's one --agent, goes in a later release
        agents.extend(agent);
        let f = match crate::artifacts_filter::of_list(&words, agents, since) {
            Ok(f) => f,
            Err(e) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
                return;
            }
        };
        let state = crate::paths::Paths::for_workspace(&ws).state;
        std::thread::spawn(move || {
            let text = Store::new(&state).list_text(&f, archived, now_ms(), &crate::artifacts_filter::local_offset_min);
            write_json(&mut stream, &json!({"ok": true, "text": text}));
        });
    }

    /// `artifacts/library`: the other registered projects' rows, read
    /// from their stores off the loop (never watched), and the folders.
    pub(super) fn art_library_typed(&mut self, id: ClientId) {
        let (own, tx, lib, home) = (self.project(), self.tx.clone(), self.art_library(), self.opts.home.clone());
        let here = super::xhub::own_name(&self.opts.home, &self.opts.paths.workspace);
        std::thread::spawn(move || {
            let projects = other_projects(&home, &own)
                .into_iter()
                .map(|(hub, name, ws)| {
                    let paths = crate::paths::Paths::for_workspace(&ws);
                    let rows = artifacts::rows_of(&paths.state, &ws.to_string_lossy(), &lib);
                    let base = Store::new(&paths.state).pages_base();
                    let items = crate::proto_view::artifacts(&json!({"rows": rows}), |_| None, base.as_deref(), |_| Default::default());
                    LibraryProject { project: hub, name, items }
                })
                .filter(|p| !p.items.is_empty())
                .collect();
            let ev = HubEv::Library { project: own, here, projects, folders: folder_rows(&lib) };
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: ev.to_value() });
        });
    }
}

impl Shell {
    /// An agent's `--folder` (`sb artifact add`): the library folder it
    /// names, its id, else a path `theme/task` (or a theme) made by that
    /// agent. None asked: its task's folder (`task_folder`), a new
    /// artifact only.
    pub(super) fn art_folder(&mut self, agent: &str, asked: Option<&str>, for_you: bool) -> Result<Option<artifacts::Filed>, String> {
        let Some(asked) = asked.map(str::trim).filter(|a| !a.is_empty()) else { return Ok(self.task_folder(agent, for_you).map(artifacts::Filed::Default)) };
        Ok(Some(artifacts::Filed::Asked(self.agent_folder(agent, asked)?)))
    }

    /// The folder main gave `agent` at its spawn (its brief's, by name):
    /// made by main at its first add, then the one its last artifact is
    /// in (artifacts_filing, pure). None: unfiled; a library it cannot
    /// write files nothing rather than fail the add.
    fn task_folder(&mut self, agent: &str, for_you: bool) -> Option<String> {
        use crate::artifacts_filing::{derived_folder, filing, Filing};
        let a = self.hub.st.agents.get(agent)?;
        // the brief's folder, else its lead's theme or its own (architect m_62392)
        let path = a.brief.folder.clone().or_else(|| derived_folder(agent, a.parent.as_deref()))?;
        // its last artifact for him: what carries the folder on (an agents' one never makes it)
        let last = self
            .art_store()
            .stored()
            .into_iter()
            .filter(|m| m.agent == agent && m.by == agent && m.audience() == crate::artifacts_marks::Audience::You)
            .max_by_key(|m| m.created_ms)
            .map(|m| m.folder);
        let lib = self.art_library();
        match filing(Some(&path), last.as_ref().map(|f| f.as_deref()), for_you, |id| library::resolve(&lib, id).map(|f| f.id.clone())) {
            Filing::Use(id) => Some(id),
            Filing::Ensure(path) => library::update(&self.opts.home, false, |l| library::make_path(l, &path, crate::model::MAIN)).ok(),
            Filing::Unfiled => None,
        }
    }

    /// The folder `asked` names, found or made by `agent` (never his
    /// undo's: an agent's change).
    fn agent_folder(&mut self, agent: &str, asked: &str) -> Result<String, String> {
        if let Some(f) = library::find(&self.art_library(), asked) {
            return Ok(f.id.clone());
        }
        library::update(&self.opts.home, false, |l| library::make_path(l, asked, agent))
    }

    /// `sb artifact move <id>... <folder> | --unfiled` (an agent's).
    pub(super) fn artifact_move_cmd(&mut self, agent: &str, ids: &[String], folder: Option<&str>) -> Value {
        let to = match folder.map(|f| self.agent_folder(agent, f)).transpose() {
            Ok(to) => to,
            Err(e) => return json!({"ok": false, "error": e}),
        };
        match self.art_move(ids, to.as_deref()) {
            Ok(moved) => {
                let place = to.and_then(|t| library::path_of(&self.art_library(), &t)).unwrap_or_else(|| "unfiled".into());
                json!({"ok": true, "text": moved_text(&moved.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), &place)})
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    }
}

/// `moved plan to docs/pricing`, `moved 3 artifacts to unfiled`.
pub(super) fn moved_text(ids: &[String], place: &str) -> String {
    match ids {
        [one] => format!("moved {one} to {place}"),
        many => format!("moved {} artifacts to {place}", many.len()),
    }
}

/// `sb artifact folders`: each live folder (themes, then their tasks
/// under them) with its id and its count here, then the unfiled ones.
pub(super) fn folders_text(lib: &Library, metas: &[artifacts::Meta]) -> String {
    let count = |id: Option<&str>| {
        metas.iter().filter(|m| m.folder.as_deref().and_then(|f| library::resolve(lib, f)).map(|f| f.id.as_str()) == id).count()
    };
    let n = |c: usize| if c == 1 { "1 artifact".to_string() } else { format!("{c} artifacts") };
    let mut out = Vec::new();
    for t in library::live(lib).filter(|f| f.parent.is_none()) {
        out.push(format!("{} · {} · {}", t.name, t.id, n(count(Some(&t.id)))));
        for k in library::children(lib, &t.id) {
            out.push(format!("{}/{} · {} · {}", t.name, k.name, k.id, n(count(Some(&k.id)))));
        }
    }
    out.push(format!("unfiled · {}", n(count(None))));
    out.join("\n")
}

fn moved_ev(project: &str, moved: Vec<(String, Option<String>)>) -> HubEv {
    HubEv::ArtifactsMoved { project: project.to_string(), moved: moved.into_iter().map(|(id, from)| ArtifactMoved { id, from }).collect() }
}

/// His move takes the undo slot: each artifact's folder before.
fn remember(home: &bise_home::Home, project: &str, moved: &[(String, Option<String>)]) {
    let moves = moved.iter().map(|(id, from)| Move { project: project.to_string(), id: id.clone(), folder: from.clone() }).collect();
    let _ = library::remember_moves(home, moves);
}
