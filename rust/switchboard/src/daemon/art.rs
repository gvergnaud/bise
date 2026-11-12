//! Artifacts and diffs on the hub's side (docs/artifacts.md): the
//! `artifacts` event (artifacts/seen, artifacts/add's words), `sb
//! artifact` (add, list, archive, unarchive), the thread lines, `diff/read` and `branches/list`'s answers
//! (computed off the hub's loop; [`Shell::diff_job`] reads a diff for the guide too, daemon/guide.rs), each agent's
//! `changes` in the state, the `landed` line after a land, and what follows
//! his save of a version from a document view ([`Shell::art_saved`]; the
//! save itself: daemon/view_files.rs).

use super::*;
use crate::artifacts::{self, Add, Added, Store};
use crate::diff;

/// How often an agent's `changes` may be computed while it edits.
const CHANGES_EVERY: Duration = Duration::from_secs(5);

/// The tool calls that change files (the `changes` follow them).
fn edits(line: &str) -> bool {
    match crate::wire::parse(line) {
        crate::wire::Wire::Tool { name, .. } => {
            matches!(name.as_str(), "edit" | "write_file" | "apply_patch" | "bash")
        }
        _ => false,
    }
}

/// The artifacts and diffs state of the shell.
#[derive(Default)]
pub(super) struct Art {
    /// The last `artifacts` event sent (rows and new): an idle that finds
    /// the same sends nothing.
    last: String,
    /// Each agent's `changes` ({files, add, del} or null), by name.
    pub(super) changes: BTreeMap<String, Option<bise_proto::rows::Changes>>,
    /// When each agent's changes were last asked, and those being
    /// computed now.
    asked: BTreeMap<String, std::time::Instant>,
    busy: BTreeSet<String>,
    /// An edit came while busy or too soon: ask again at the next idle.
    stale: BTreeSet<String>,
    /// The artifacts whose newest version's turn still ran at their last
    /// live frame: id -> (maker, its turn's start). When sb-core says that
    /// turn is over, their `live` frame and the list go again.
    live: BTreeMap<String, (String, u64)>,
    /// The live words' cache was filled at the first state push after the
    /// hub started (events::seed).
    seeded: bool,
    /// The store's rows, its metas (for `live`), new and seen_ms, built
    /// off the loop when their key moves (daemon/art_rows.rs, hub-loop):
    /// the state push and the view ask for them up to 10 times a second.
    pub(super) rows: std::cell::RefCell<super::art_rows::ArtRows>,
    /// The typed rows of the last view and send-time key (artifact_rows_now).
    #[allow(clippy::type_complexity)]
    typed: std::cell::RefCell<Option<((u64, Option<std::time::SystemTime>, u64, bool), std::rc::Rc<super::art_rows::ArtView>, std::rc::Rc<Vec<bise_proto::rows::Artifact>>)>>,
    /// The library's folders, read by its mtime (daemon/art_folders.rs).
    pub(super) lib: super::art_folders::LibCache,
    /// The open PR artifacts' links, newest first, by the store's
    /// generation (daemon/pr_arts.rs: the PRs the poller follows).
    pub(super) prs: std::cell::RefCell<Option<(u64, Vec<String>)>>,
    /// His opens not written yet (id, at, version seen): marks.json is
    /// written at most once a second ([`Daemon::art_opens_flush`]).
    opens: Vec<(String, u64, Option<u64>)>,
    opens_written_ms: u64,
    /// The store's backfill was tried since the hub started
    /// ([`Daemon::art_backfill_once`]).
    backfill_tried: bool,
    /// A retention sweep runs on its thread now
    /// ([`Daemon::art_retention_sweep`]): the next waits for its end.
    sweeping: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Where an agent works and what its changes are measured against.
#[derive(Clone, Debug)]
enum Where {
    /// Its own checkout (a worktree), against this base.
    Checkout { dir: PathBuf, base: Option<String> },
    /// The shared folder: its own files only.
    Shared { dir: PathBuf, files: Vec<String> },
}

impl Shell {
    pub(super) fn art_store(&self) -> Store {
        Store::new(&self.opts.paths.state)
    }

    /// An agent's name now (its old names are aliases) and whether it is
    /// archived.
    fn art_who(&self) -> impl Fn(&str) -> Option<(String, bool)> + '_ {
        move |name: &str| {
            self.hub
                .st
                .agents
                .values()
                .find(|a| a.name == name || a.dir == name || a.aliases.iter().any(|x| x == name))
                .map(|a| (a.name.clone(), a.status() == crate::model::Status::Archived))
        }
    }

    /// The turn `agent` is in now, from sb-core's view (its `turn_ms`):
    /// the stamp of a save, never read from the request (artifacts_turns).
    fn art_turn(&self, agent: &str) -> Option<crate::artifacts_turns::Turn> {
        let a = self.hub.st.agents.get(agent)?;
        a.turn_ms.map(|started_ms| crate::artifacts_turns::Turn { agent: a.name.clone(), started_ms })
    }

    /// When the turn of the agent that made `v` started, if it runs now.
    fn art_maker_turn(&self, v: &artifacts::Version) -> Option<u64> {
        let t = v.turn.as_ref()?;
        self.hub.st.agents.get(&t.agent)?.turn_ms
    }

    /// Each stored artifact's live words, by id (the rows' `live`).
    fn art_live_rows(&self, metas: &BTreeMap<String, artifacts::Meta>, rows: &mut [Value]) {
        for r in rows.iter_mut() {
            let Some(v) = r.get("id").and_then(Value::as_str).and_then(|id| metas.get(id)).and_then(|m| m.current()) else { continue };
            if let Some(live) = crate::artifacts_turns::live(v, self.art_maker_turn(v)) {
                r["live"] = json!(live);
            }
        }
    }

    /// A new save of `m`: its `version` frame on the viewer's
    /// `/a/<id>/events` (pages/artifact/events.rs), and its turn watched
    /// for the frame that follows the turn's end.
    fn art_live_push(&mut self, m: &artifacts::Meta) {
        let Some(v) = m.current() else { return };
        let maker = self.art_maker_turn(v);
        match (&v.turn, maker) {
            (Some(t), Some(at)) if t.started_ms == at => {
                self.art.live.insert(m.id.clone(), (t.agent.clone(), at));
            }
            _ => {
                self.art.live.remove(&m.id);
            }
        }
        if let Some(pages) = self.pg.pages.clone() {
            crate::pages::artifact::events::push_version(&pages, &m.id, v, crate::artifacts_turns::live(v, maker));
        }
    }

    /// The state changed (push_state): an artifact whose maker's turn
    /// ended since its last frame gets its `live` frame (the line now)
    /// and the list goes again. Read from sb-core's view, never a line.
    /// The first time with a page server: the words' cache seeded for
    /// every artifact (a stream opened after a restart starts right).
    pub(super) fn art_live_watch(&mut self) {
        if !self.art.seeded {
            if let Some(pages) = self.pg.pages.clone() {
                self.art.seeded = true;
                let running = crate::pages::artifact::events::seed(&pages, &self.art_store(), |v| self.art_maker_turn(v));
                for (id, agent, at) in running {
                    self.art.live.insert(id, (agent, at));
                }
            }
        }
        let ended: Vec<String> = self
            .art
            .live
            .iter()
            .filter(|(_, (agent, at))| self.hub.st.agents.get(agent).and_then(|a| a.turn_ms) != Some(*at))
            .map(|(id, _)| id.clone())
            .collect();
        if ended.is_empty() {
            return;
        }
        let store = self.art_store();
        for id in &ended {
            self.art.live.remove(id);
            let Some(m) = store.get(id) else { continue };
            let Some(v) = m.current() else { continue };
            if let Some(pages) = self.pg.pages.clone() {
                crate::pages::artifact::events::push_live(&pages, id, crate::artifacts_turns::live(v, self.art_maker_turn(v)));
            }
        }
        self.artifacts_refresh(false);
    }

    /// What the artifact rows read, as one key (daemon/art_rows.rs): the
    /// stores' disk generation, the agents' names, the backstop's bucket
    /// and the folders' stamps (a page or an artifact written by another
    /// process).
    fn art_key(&self, now: u64) -> super::art_rows::ArtKey {
        let st = &self.opts.paths.state;
        let disk = crate::store_gen::stamp(&st.join("pages")) ^ crate::store_gen::stamp(&st.join("artifacts")).rotate_left(1);
        (crate::store_gen::disk_now(), self.art_who_key(), now / super::art_rows::BACKSTOP_MS, disk)
    }

    /// The last rows built, and a build for `key` started off the loop
    /// when it moved (one at a time; its view comes back as
    /// `Msg::ArtBuilt`). None before the first build lands.
    fn art_view(&self, now: u64) -> Option<std::rc::Rc<super::art_rows::ArtView>> {
        let key = self.art_key(now);
        let (view, start) = self.art.rows.borrow_mut().ask(key);
        if start {
            let job = super::art_rows::ArtJob {
                state: self.opts.paths.state.clone(),
                workspace: self.hub.workspace.clone(),
                who: self
                    .hub
                    .st
                    .agents
                    .values()
                    .map(|a| super::art_rows::WhoRow { name: a.name.clone(), dir: a.dir.clone(), aliases: a.aliases.clone(), archived: a.status() == crate::model::Status::Archived })
                    .collect(),
                now,
                // the pages waiting on him are read by the build, off the loop (hub-loop m_61273)
                pages: self.pg.pages.clone(),
            };
            let tx = self.tx.clone();
            let spawned = std::thread::Builder::new().name("art-rows".into()).spawn(move || {
                let view = job.build();
                let _ = tx.send(Msg::ArtBuilt(Box::new((key, view))));
            });
            if let Err(e) = spawned {
                self.art.rows.borrow_mut().failed();
                log_line(&self.opts.paths, &format!("artifacts: the rows' build did not start: {e}"));
            }
        }
        view
    }

    /// The artifacts' current files of the last build (empty before the
    /// first): a pointer copy, for the files thread (`folder/list`,
    /// `view/open {kind: entry}`).
    pub(super) fn art_links(&self) -> super::art_rows::Links {
        self.art_view(now_ms()).map(|v| std::sync::Arc::clone(&v.links)).unwrap_or_default()
    }

    /// A build of the rows landed (`Msg::ArtBuilt`): kept, and the list to
    /// the clients when it changed (a key that moved meanwhile starts the
    /// next build there).
    pub(super) fn art_built(&mut self, key: super::art_rows::ArtKey, view: super::art_rows::ArtView) {
        if self.art.rows.borrow_mut().built(key, view) {
            log_line(&self.opts.paths, "artifacts: the 5 min backstop found a change no stamp saw");
        }
        self.state_gate.changed();
        self.artifacts_refresh(false);
    }

    /// `{"ev":"artifacts","rows":[…],"new":N,"seen_ms":T}`; None until
    /// the first build of the rows lands (no event yet: the clients keep
    /// their loading state).
    pub(super) fn artifacts_ev(&self) -> Option<Value> {
        let view = self.art_view(now_ms())?;
        Some(self.artifacts_ev_of(&view))
    }

    /// The typed rows now (⌘K's index in view.json, the pulse, the
    /// growth count): built again only when the view or what is joined
    /// to it at send time moved (hub-loop: their conversion and copy of
    /// 1550 rows held the loop 1-3 s at each state push at load 30).
    /// None before the rows' first build lands.
    pub(super) fn artifact_rows_now(&self) -> Option<std::rc::Rc<Vec<bise_proto::rows::Artifact>>> {
        let view = self.art_view(now_ms())?;
        let key = self.art_typed_key();
        if let Some((k, v, rows)) = self.art.typed.borrow().as_ref() {
            if *k == key && std::rc::Rc::ptr_eq(v, &view) {
                return Some(rows.clone());
            }
        }
        let rows = std::rc::Rc::new(self.artifact_rows(&self.artifacts_ev_of(&view)));
        *self.art.typed.borrow_mut() = Some((key, view, rows.clone()));
        Some(rows)
    }

    /// What the event joins to the built view at send time, as one key:
    /// the makers' turns (`live`), the library's stamp (folders), the
    /// PRs' rows (their status) and whether a page server runs (links).
    fn art_typed_key(&self) -> (u64, Option<std::time::SystemTime>, u64, bool) {
        use std::hash::{Hash, Hasher};
        let mut live = std::collections::hash_map::DefaultHasher::new();
        for a in self.hub.st.agents.values().filter(|a| a.turn_ms.is_some()) {
            (&a.name, a.turn_ms).hash(&mut live);
        }
        let mut pr = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&self.hub.pr_rows_by_url(now_ms())).unwrap_or_default().hash(&mut pr);
        (live.finish(), super::art_folders::library_stamp(&self.opts.home), pr.finish(), self.pg.pages.is_some())
    }

    /// The event of a built view: its rows joined with what changes
    /// without a store write (live turns, folders, PR status).
    fn artifacts_ev_of(&self, view: &super::art_rows::ArtView) -> Value {
        let mut rows = view.rows.clone();
        self.art_live_rows(&view.stored, &mut rows);
        // the library joined at send time (his rename in the desktop core
        // moves no store stamp): each row's folder resolved, the folders
        let lib = self.art_library();
        artifacts::resolve_folders(&mut rows, &lib);
        // each PR's status, the hub's row of it (a change re-sends the event)
        artifacts::join_pr_status(&mut rows, &self.hub.pr_rows_by_url(now_ms()));
        // each row's done and section (artifacts_marks), once its PR's state is in;
        // a page with something waiting on him stays open work (the set the build read off the loop)
        artifacts::join_sections(&mut rows, &view.waiting);
        json!({
            "ev": "artifacts",
            "rows": rows,
            "new": view.new,
            "seen_ms": view.seen_ms,
            "agents_bytes": view.agents_bytes,
            "folders": super::art_folders::folder_rows(&lib),
        })
    }

    /// One number for what `art_who` answers: each agent's names and
    /// whether it is archived (a rename or an /archive moves the rows).
    fn art_who_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for a in self.hub.st.agents.values() {
            (&a.name, &a.dir, &a.aliases, a.status() == crate::model::Status::Archived).hash(&mut h);
        }
        h.finish()
    }

    /// Send the list when it changed since the last one sent (`force`:
    /// always).
    pub(super) fn artifacts_refresh(&mut self, force: bool) {
        let Some(ev) = self.artifacts_ev() else { return };
        let s = ev.to_string();
        if force || s != self.art.last {
            self.art.last = s;
            self.broadcast(&ev);
        }
    }

    /// artifacts/seen (the TUI's `/artifacts` screen opened): `new` = 0,
    /// the list to every client.
    /// `at_ms`: when he looked (none: now); seen moves to it, never past
    /// now, never back (art-seen 1cfbc07d)
    pub(super) fn artifacts_seen(&mut self, at_ms: Option<u64>) {
        let now = now_ms();
        let _ = self.art_store().saw(at_ms.unwrap_or(now), now);
        self.artifacts_refresh(true);
    }

    /// `/artifacts add <path or link>` by the user in `agent`'s view
    /// (artifacts/add and command/run): the store's add,
    /// its thread lines, and the words for the one who asked (`↗ added:
    /// <title>`) or the store's refusal. The caller sends them and then
    /// `artifacts_refresh(true)`. `folder`: a library folder (id or name)
    /// he picked: it goes there.
    pub(super) fn art_add(&mut self, agent: &str, target: &str, title: Option<String>, folder: Option<String>) -> Result<String, String> {
        let lib = self.art_library();
        let folder = match folder.filter(|f| !f.trim().is_empty()) {
            Some(f) => Some(artifacts::Filed::Asked(bise_home::library::find(&lib, &f).ok_or_else(|| format!("no folder {f}"))?.id.clone())),
            None => None,
        };
        let agent = match agent {
            "" => MAIN.to_string(),
            a => a.to_string(),
        };
        let cwd = self
            .hub
            .st
            .agents
            .get(&agent)
            .map(|a| self.art_where(a))
            .map(|w| match w {
                Where::Checkout { dir, .. } | Where::Shared { dir, .. } => dir.to_string_lossy().to_string(),
            })
            .unwrap_or_else(|| self.hub.workspace.clone());
        let add = Add { target: target.to_string(), title, kind: None, agent, by: "you".into(), cwd, known_only: false, folder, ..Default::default() };
        let a = self.art_store().add(&add, now_ms())?;
        if a.new_version {
            self.art_lines(&a);
        }
        self.serve_ask(&a);
        Ok(format!("↗ added: {}", a.meta.title))
    }

    /// His save made a version (a document view's write, daemon/view_files.rs:
    /// decided there, the store's only writer, one version per sitting): its
    /// frame on the viewer's stream, the list, and on the save that makes
    /// the version, its line and its maker told once (an `edit` note, the
    /// notes' Sent path).
    pub(super) fn art_saved(&mut self, pages: &crate::pages::Pages, a: &Added) {
        let Some(v) = a.meta.current() else { return };
        self.art_live_push(&a.meta);
        if !a.replaced {
            self.art_lines(a);
            if let Some((text, parts, _)) = crate::pages::artifact::notes::record_edit(pages, &a.meta.id, &a.meta.title, v.v, now_ms()) {
                let to = if self.page_agent_gone(&a.meta.agent) { MAIN.to_string() } else { a.meta.agent.clone() };
                self.step(Input::ClientInput { client: 0, focus: to, text, queued: true, parts, from: String::new() });
            }
        }
        self.artifacts_refresh(false);
    }

    /// `sb artifact add|list` (an agent's request).
    pub(super) fn artifact_cmd(&mut self, from: &str, r: bise_proto::sb::Artifact) -> Value {
        use bise_proto::sb::Artifact;
        let store = self.art_store();
        match r {
            Artifact::Add { target, cwd, title, kind, known_only, changed, folder, for_you } => {
                // its audience as the add will read it (artifacts_marks::audience_at_add): only what
                // is for him makes a task's folder
                let him = for_you || from == crate::model::MAIN || crate::forge::pr_link(&target).is_some();
                let folder = match self.art_folder(from, folder.as_deref(), him) {
                    Ok(f) => f,
                    Err(e) => return json!({"ok": false, "error": e}),
                };
                let add = Add {
                    target,
                    title,
                    kind,
                    agent: from.to_string(),
                    by: from.to_string(),
                    cwd,
                    known_only,
                    // the turn from sb-core's fact, never from the request
                    turn: self.art_turn(from),
                    sitting: None,
                    changed: changed.as_ref().and_then(bise_proto::office::Changed::of_value).map(|c| c.0),
                    folder,
                    id: None,
                    for_you,
                    staged: None,
                };
                match store.add(&add, now_ms()) {
                    Ok(a) => {
                        if a.new_version {
                            // a save in the same turn is the same version: no new line
                            if !a.replaced {
                                self.art_lines(&a);
                            }
                            self.art_live_push(&a.meta);
                        }
                        self.serve_ask(&a);
                        self.artifacts_refresh(false);
                        // a feature's diff added by hand: its try item opens it
                        if a.meta.source.ends_with(".diff") {
                            self.feature_diffs_refresh();
                        }
                        json!({"ok": true, "text": artifacts::added_text(&a), "id": a.meta.id})
                    }
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
            Artifact::List { words, agent, mut agents, project: _, since, archived } => {
                // TODO(artifacts-list-agent): an older sb's one --agent, goes in a later release
                agents.extend(agent);
                match crate::artifacts_filter::of_list(&words, agents, since) {
                    Ok(f) => json!({"ok": true, "text": store.list_text(&f, archived, now_ms(), &crate::artifacts_filter::local_offset_min)}),
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
            Artifact::Keep { ids } => self.artifact_keep(from, &ids, true),
            Artifact::Unkeep { ids } => self.artifact_keep(from, &ids, false),
            Artifact::Archive(p) => self.artifact_pick(true, p),
            Artifact::Unarchive(p) => self.artifact_pick(false, p),
            Artifact::Move { ids, folder } => self.artifact_move_cmd(from, &ids, folder.as_deref()),
            Artifact::Folders => {
                let lib = self.art_library();
                json!({"ok": true, "text": super::art_folders::folders_text(&lib, &store.all())})
            }
        }
    }

    /// `sb artifact keep|unkeep` (an agent's): kept in his Kept section
    /// under its name, or let go; his own keep stands
    /// (crate::artifacts_marks). One line per artifact.
    fn artifact_keep(&mut self, from: &str, ids: &[String], keep: bool) -> Value {
        let store = self.art_store();
        let title = self.art_titles();
        match store.keep(ids, &|id: &str| title.get(id).cloned(), crate::artifacts_marks::Who::Agent(from), keep, now_ms()) {
            Ok(done) => {
                let marks = store.marks();
                let text = done
                    .iter()
                    .map(|(id, t, o)| crate::artifacts_marks::keep_text(t, from, keep, o, marks.kept(id).map(|k| k.by.as_str())))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.artifacts_refresh(false);
                json!({"ok": true, "text": text})
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    /// `artifacts_keep` (his, from a window): kept or let go, whoever kept
    /// it before; the list to every client.
    pub(super) fn artifacts_keep_his(&mut self, ids: &[String], keep: bool) -> Result<(), String> {
        let title = self.art_titles();
        self.art_store().keep(ids, &|id: &str| title.get(id).cloned(), crate::artifacts_marks::Who::You, keep, now_ms())?;
        self.artifacts_refresh(false);
        Ok(())
    }

    /// Each artifact's title by id, from the last rows built (hub-loop's
    /// rule: no store read on the loop); before the first build lands,
    /// the store's own list (once, at a hub's start).
    fn art_titles(&self) -> BTreeMap<String, String> {
        match self.art_view(now_ms()) {
            Some(v) => v.rows.iter().filter_map(|r| Some((r["id"].as_str()?.to_string(), r["title"].as_str().unwrap_or_default().to_string()))).collect(),
            None => self.art_store().all().into_iter().map(|m| (m.id, m.title)).collect(),
        }
    }

    /// `artifacts_opened` (his, from a window): its `opened_ms` and the
    /// version he saw, gathered and written at most once a second
    /// ([`Self::art_opens_flush`], also on the tick): never a write per
    /// click on the loop (architect m_60685).
    pub(super) fn artifacts_opened(&mut self, id: &str, v: Option<u64>) {
        self.art.opens.push((id.to_string(), now_ms(), v));
        self.art_opens_flush(false);
    }

    /// Write the opens gathered, when the last write is a second old (or
    /// `force`); the list again when one moved.
    pub(super) fn art_opens_flush(&mut self, force: bool) {
        let now = now_ms();
        if self.art.opens.is_empty() || (!force && now < self.art.opens_written_ms + 1000) {
            return;
        }
        let opens = std::mem::take(&mut self.art.opens);
        self.art.opens_written_ms = now;
        match self.art_store().opened(&opens) {
            Ok(true) => self.artifacts_refresh(false),
            Ok(false) => {}
            Err(e) => log_line(&self.opts.paths, &format!("artifacts: his opens not kept: {e}")),
        }
    }

    /// The store's one-shot backfill (TODO(artifacts-backfill): goes in a
    /// later release, tests/shims.txt), started once per hub start on its
    /// own thread, off the loop (hub-loop m_61273); it runs once per store
    /// (artifacts_keep.rs, each write guarded against the loop's). Its
    /// writes bump store_gen: the rows rebuild off the loop by themselves.
    /// One log line with its counts.
    pub(super) fn art_backfill_once(&mut self) {
        if std::mem::replace(&mut self.art.backfill_tried, true) {
            return;
        }
        let (state, paths) = (self.opts.paths.state.clone(), self.opts.paths.clone());
        std::thread::spawn(move || match artifacts::Store::new(&state).backfill() {
            Ok(Some(n)) => log_line(&paths, &format!("artifacts: backfilled · {} for you · {} between agents · {} kept", n.you, n.agents, n.kept)),
            Ok(None) => {}
            Err(e) => log_line(&paths, &format!("artifacts: backfill not done: {e}")),
        });
    }

    /// The retention sweep (crate::artifacts_retention, architect
    /// m_62392), at the hub's start and then daily: the archived agents'
    /// names are copied here (no store read on the loop), the rest runs
    /// on its own thread: ended.json noted, the expired copies deleted,
    /// one log line when it expired something. Its writes bump store_gen:
    /// the rows rebuild off the loop by themselves.
    pub(super) fn art_retention_sweep(&mut self) {
        use std::sync::atomic::Ordering;
        if self.art.sweeping.swap(true, Ordering::AcqRel) {
            return;
        }
        let archived: BTreeSet<String> = self
            .hub
            .st
            .agents
            .values()
            .filter(|a| a.status() == crate::model::Status::Archived)
            .flat_map(|a| std::iter::once(a.name.clone()).chain(std::iter::once(a.dir.clone())).chain(a.aliases.iter().cloned()))
            .collect();
        let (state, paths, flag) = (self.opts.paths.state.clone(), self.opts.paths.clone(), self.art.sweeping.clone());
        std::thread::spawn(move || {
            let store = artifacts::Store::new(&state);
            let now = now_ms();
            let done = crate::artifacts_retention::note_ended(&store, &archived, now)
                .and_then(|ended| crate::artifacts_retention::sweep(&store, &ended, now));
            match done {
                Ok(s) if s.rows > 0 => log_line(&paths, &format!("artifacts: retention · {} expired · {} bytes freed", s.rows, s.bytes)),
                Ok(_) => {}
                Err(e) => log_line(&paths, &format!("artifacts: retention sweep stopped: {e}")),
            }
            flag.store(false, Ordering::Release);
        });
    }

    /// `sb artifact archive|unarchive`: those picked out of the list, or
    /// back in it.
    fn artifact_pick(&mut self, archive: bool, p: bise_proto::sb::ArtifactPick) -> Value {
        let store = self.art_store();
        let bise_proto::sb::ArtifactPick { ids, agent, kind, before_ms } = p;
        let some = |x: Option<String>| x.filter(|x| !x.is_empty());
        let pick = artifacts::Pick { ids, agent: some(agent), before_ms, kind: some(kind) };
        let text = if archive {
            store.archive(&pick, now_ms()).map(|a| artifacts::archived_text(&a))
        } else {
            store.unarchive(&pick).map(|(back, not)| artifacts::unarchived_text(&back, &not))
        };
        match text {
            Ok(text) => {
                self.artifacts_refresh(false);
                // an archived feature diff opens no more (and back again)
                self.feature_diffs_refresh();
                json!({"ok": true, "text": text})
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    /// The ↗ line of a new artifact or version: in the maker's thread and
    /// in main's.
    pub(super) fn art_lines(&mut self, a: &Added) {
        let line = artifacts::thread_rec(&a.meta).line();
        let maker = self.art_who()(&a.meta.agent).map(|(n, _)| n);
        if let Some(m) = &maker {
            if m != MAIN {
                self.feed(m, &line);
            }
        }
        self.feed(MAIN, &line);
    }

    /// An agent's turn ended: the list (a page may have been published)
    /// and its changes.
    pub(super) fn art_on_line(&mut self, name: &str, line: &str) {
        if line == "--- idle" {
            self.artifacts_refresh(false);
            self.changes_ask(name, true);
        } else if edits(line) {
            self.changes_ask(name, false);
        }
    }

    fn art_where(&self, a: &crate::model::Agent) -> Where {
        if let Some(p) = &a.place {
            return Where::Checkout { dir: PathBuf::from(p), base: None };
        }
        match a.ws.mode {
            crate::model::Mode::Worktree if !a.ws.dropped => Where::Checkout {
                dir: PathBuf::from(&a.ws.path),
                base: a.ws.feature().map(String::from),
            },
            _ => Where::Shared {
                dir: PathBuf::from(&a.ws.path),
                files: a.files.to_vec(),
            },
        }
    }

    /// Compute an agent's `changes` off the loop: at its idle (`now`), or
    /// after an edit at most every 5 s (else at its next idle).
    fn changes_ask(&mut self, name: &str, now: bool) {
        let Some(a) = self.hub.st.agents.get(name) else { return };
        if a.is_main {
            return;
        }
        let soon = self.art.asked.get(name).is_some_and(|t| t.elapsed() < CHANGES_EVERY);
        if self.art.busy.contains(name) || (!now && soon) {
            self.art.stale.insert(name.to_string());
            return;
        }
        self.art.stale.remove(name);
        self.art.busy.insert(name.to_string());
        self.art.asked.insert(name.to_string(), std::time::Instant::now());
        let place = self.art_where(a);
        let (tx, name, how) = (self.tx.clone(), name.to_string(), self.base_how());
        let shared = PathBuf::from(&self.hub.workspace);
        std::thread::spawn(move || {
            let v = changes_of(&place, &how.base(&shared));
            let _ = tx.send(Msg::Changes { name, v });
        });
    }

    /// After the add of a local link (a dev server, artifacts_serve): who
    /// listens on its port, found off the loop (lsof, its argv, `GET /`);
    /// the answer comes back as `Msg::Serve`. Nothing listens: nothing
    /// comes, the link stays a link.
    fn serve_ask(&self, a: &Added) {
        let Some(v) = a.meta.current().filter(|_| a.new_version && !a.page) else { return };
        if crate::artifacts_serve::local_port(&v.target).is_none() {
            return;
        }
        let (tx, id, n, link) = (self.tx.clone(), a.meta.id.clone(), v.v, v.target.clone());
        std::thread::spawn(move || {
            if let Some((serve, root)) = crate::artifacts_probe::recipe(&link) {
                let _ = tx.send(Msg::Serve { id, v: n, serve, root });
            }
        });
    }

    /// A local site's recipe was found: on its version (and a static
    /// site's copy), then the list again (its row's `url` is its door).
    pub(super) fn on_serve(&mut self, id: &str, v: u64, serve: crate::artifacts_serve::Serve, root: Option<PathBuf>) {
        match self.art_store().set_serve(id, v, serve, root.as_deref()) {
            Ok(true) => self.artifacts_refresh(false),
            Ok(false) => {}
            Err(e) => log_line(&self.opts.paths, &format!("artifact {id}: its recipe not kept: {e}")),
        }
    }

    /// An agent's changes are computed: the state again when they moved.
    pub(super) fn on_changes(&mut self, name: String, v: Option<bise_proto::rows::Changes>) {
        self.art.busy.remove(&name);
        let moved = self.art.changes.get(&name) != Some(&v);
        self.art.changes.insert(name.clone(), v);
        if moved {
            self.state_now();
        }
        if self.art.stale.contains(&name) {
            self.changes_ask(&name, false);
        }
    }

    /// The typed `diff` (desktop S7; a branch, a PR or a range since
    /// client-protocol step 3): the same answer as the op, in bise-proto's
    /// shape (`proto_view::diff`), to that client. `d` is checked
    /// (`DiffAsk::check`).
    pub(super) fn diff_typed(&mut self, id: ClientId, project: String, d: crate::proto_view::DiffAsk) {
        let (job, tx) = (self.diff_job(project, d), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: job().to_value() });
        });
    }

    /// The typed `diff` event `d` asks (or its `error`), read by the
    /// returned job off the hub's loop: the review's answer, and the
    /// diff a guide reads (daemon/guide.rs), the same way.
    pub(super) fn diff_job(&mut self, project: String, d: crate::proto_view::DiffAsk) -> impl FnOnce() -> bise_proto::hub::HubEv + Send + 'static {
        let shared = PathBuf::from(&self.hub.workspace);
        // one commit: the range ask the TUI's door under a land uses
        let ask = self.diff_ask(&d.op());
        let agent = d.agent.clone().unwrap_or_default();
        let commit = d.commit.clone();
        // emitter 5: what its change measured (`sb report --result`), from
        // its latest report (an agent's own change only)
        let mine = d.range.is_none() && d.commit.is_none();
        let result = self.hub.st.agents.get(&agent).filter(|_| mine).and_then(|a| a.last_report.as_ref()).and_then(|r| r.result.clone());
        let req = d.req.map_or(Value::Null, |r| json!(r));
        let how = self.base_how();
        move || {
            let mut ev = diff_answer(ask, &shared, &how, req);
            // a binary file's size, read here (the mapper stays pure):
            // its file in the agent's checkout, when it is still there
            for f in ev.get_mut("files").and_then(Value::as_array_mut).into_iter().flatten() {
                let abs = f.get("abs").and_then(Value::as_str).map(PathBuf::from);
                if f["binary"] == true {
                    if let Some(m) = abs.and_then(|p| std::fs::metadata(p).ok()).filter(|m| m.is_file()) {
                        f["size"] = json!(m.len());
                    }
                }
            }
            let mut typed = crate::proto_view::diff(&ev, &project, &agent);
            // the answer says which commit it is (not the live change)
            if let bise_proto::hub::HubEv::Diff { commit: c, result: r, .. } = &mut typed {
                *c = commit;
                *r = result.and_then(|v| serde_json::from_value(v).ok()).map(Box::new);
            }
            typed
        }
    }

    /// How the diffs pick their base now: `[worktree] base` and PR flow
    /// (read on the loop, used off it: `BaseHow::base`).
    pub(super) fn base_how(&mut self) -> BaseHow {
        let pr = self.flow_now().is_some_and(|f| f.mode == crate::flow::FlowMode::Pr);
        BaseHow { configured: self.env.config.base.clone(), pr }
    }

    /// What a diff request asks: an agent's changes, a branch, a range or
    /// a PR.
    fn diff_ask(&self, v: &Value) -> Ask {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let shared = PathBuf::from(&self.hub.workspace);
        // a range first: the door under a landed line names its agent
        // too (for the title), and must show that land, never the
        // agent's branch vs today's main (empty once landed)
        if !s("range").is_empty() {
            Ask::Range(s("range"), s("agent"))
        } else if !s("agent").is_empty() {
            match self.hub.st.agents.get(&s("agent")).or_else(|| {
                let n = self.art_who()(&s("agent")).map(|(n, _)| n)?;
                self.hub.st.agents.get(&n)
            }) {
                Some(a) => {
                    let branch = a.ws.branch.clone().or_else(|| a.place_branch.clone());
                    let archived = a.status() == crate::model::Status::Archived;
                    // an archived or dropped agent's worktree is removed (or
                    // soon): not the shared folder
                    let place = match self.art_where(a) {
                        Where::Shared { dir, .. } if dir != shared && (archived || !dir.is_dir()) => Where::Checkout { dir, base: None },
                        w => w,
                    };
                    Ask::Agent(a.name.clone(), place, branch, archived)
                }
                None => Ask::Bad(format!("no agent {}", s("agent"))),
            }
        } else if !s("branch").is_empty() {
            Ask::Branch(s("branch"))
        } else if let Some(at) = pr_at(v) {
            // a place's checkout of that PR, its base: read in the thread
            let link = |u: &str| crate::forge::pr_link(u).map(|l| l.url);
            let local = match &at {
                PrAt::Url(u) => crate::place::places(&self.hub.st, &self.hub.prs).into_iter().find_map(|p| {
                    let pr = p.pr.filter(|pr| link(&pr.url).as_deref() == Some(u.as_str()))?;
                    Some(diff::PrLocal { dir: PathBuf::from(&p.path), base: pr.facts.base.clone(), head: pr.head_oid.clone() })
                }),
                PrAt::Number(_) => None,
            };
            Ask::Pr(at, local)
        } else if !s("artifact").is_empty() {
            let version = v.get("version").and_then(Value::as_u64);
            artifact_ask(&self.art_store(), &s("artifact"), version)
        } else {
            Ask::Bad("diff: agent, branch, range, pr or artifact".into())
        }
    }

    /// The /diff picker's rows to client `id`: `branches/list`'s answer.
    pub(super) fn branches_scan(&mut self, id: ClientId) {
        let project = self.project();
        let shared = PathBuf::from(&self.hub.workspace);
        // who works on which branch, and the PRs the poller knows
        let mut on: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for a in self.hub.st.agents.values() {
            if a.status() == crate::model::Status::Archived {
                continue;
            }
            if let Some(b) = a.ws.branch.clone().or_else(|| a.place_branch.clone()) {
                on.entry(b).or_default().push(a.name.clone());
            }
        }
        let tx = self.tx.clone();
        let how = self.base_how();
        std::thread::spawn(move || {
            let base = how.base(&shared);
            let rows: Vec<Value> = diff::branches(&shared, &base)
                .into_iter()
                .map(|(b, commits, add, del)| {
                    json!({"branch": b, "agents": on.get(&b).cloned().unwrap_or_default(), "commits": commits,
                           "uncommitted": false, "pr": null, "landed_ms": null, "add": add, "del": del})
                })
                .collect();
            let v = json!({"ev": "branches", "project": project, "base": base, "rows": rows});
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v });
        });
    }
}

/// An agent's `changes` in the state: `{files, add, del}` of its
/// checkout against its base (commits and uncommitted edits, untracked
/// files included) or of its own files in the shared folder; null when
/// there are none or git cannot say (a folder that is gone: diff.rs
/// never runs git in it).
fn changes_of(place: &Where, start: &str) -> Option<bise_proto::rows::Changes> {
    let files = match place {
        Where::Checkout { dir, base } => {
            let base = base.clone().unwrap_or_else(|| start.to_string());
            diff::checkout(dir, &base).map(|(f, _, _)| f)
        }
        Where::Shared { dir, files } => diff::own_files(dir, files),
    };
    match files {
        Ok(f) if !f.is_empty() => {
            let (files, add, del) = diff::stat(&f);
            Some(bise_proto::rows::Changes { files: files as u64, add, del })
        }
        _ => None,
    }
}

/// Where an agent's work started, as a new worktree picks it
/// (`trunk::start_ref`): its checkout, a branch and the worktrees list
/// are compared to that, never to a local trunk PR flow does not pull.
#[derive(Clone, Debug, Default)]
pub(super) struct BaseHow {
    /// `[worktree] base`
    pub configured: Option<String>,
    /// the repo ships through PRs (origin's copy of the trunk)
    pub pr: bool,
}

impl BaseHow {
    /// The base's short name (`main`, `origin/main`); runs git.
    pub fn base(&self, shared: &Path) -> String {
        crate::trunk::diff_base(shared, self.configured.as_deref(), self.pr)
    }
}

/// What a `diff/read` asks for.
#[derive(Clone, Debug)]
enum Ask {
    /// name, where, branch, archived
    Agent(String, Where, Option<String>, bool),
    Branch(String),
    Range(String, String),
    /// a PR, and the place that has it checked out
    Pr(PrAt, Option<diff::PrLocal>),
    /// a `.diff` kept as an artifact ([`artifact_ask`])
    Artifact(ArtDiff),
    Bad(String),
}

/// Which PR a diff asks: its link (pr_link's one form), or an older
/// client's number in the hub's own repo (made a link in the thread:
/// `origin`'s repo is read by git, never on the loop).
#[derive(Clone, Debug, PartialEq, Eq)]
enum PrAt {
    Url(String),
    Number(u64),
}

/// The PR a diff op asks (`pr_url` wins over `pr`); none: not a PR's.
fn pr_at(v: &Value) -> Option<PrAt> {
    match v.get("pr_url").and_then(Value::as_str) {
        Some(u) => crate::forge::pr_link(u).map(|l| PrAt::Url(l.url)),
        None => v.get("pr").and_then(Value::as_u64).map(PrAt::Number),
    }
}

/// An older client's PR number as its link: that number in `origin`'s
/// repo (`remote` its url).
fn pr_url_in(remote: &str, n: u64) -> Option<String> {
    let r = crate::forge::repo_of_url(remote)?;
    crate::forge::pr_link(&format!("https://{}/{}/{}/pull/{}", r.host, r.owner, r.name, n)).map(|l| l.url)
}

/// A `.diff` artifact's version to read: its id, title and version, and
/// bise's copy of that version (read in the diff's thread, never on the
/// loop).
#[derive(Clone, Debug)]
struct ArtDiff {
    id: String,
    title: String,
    v: u64,
    copy: PathBuf,
}

/// What a diff of artifact `id` (its `version`, none: the newest) reads:
/// bise's copy, through the store (its owner); a page, a link or a
/// version with no copy is refused in words.
fn artifact_ask(store: &crate::artifacts::Store, id: &str, version: Option<u64>) -> Ask {
    let Some(m) = store.get(id) else { return Ask::Bad(format!("no artifact {id}")) };
    let v = match version {
        Some(n) => m.versions.iter().find(|x| x.v == n),
        None => m.current(),
    };
    let Some(v) = v else { return Ask::Bad(format!("{id} has no version {}", version.unwrap_or(0))) };
    match store.copy_path(&m, v) {
        Some(copy) => Ask::Artifact(ArtDiff { id: m.id.clone(), title: m.title.clone(), v: v.v, copy }),
        None => Ask::Bad(format!("{id} v{} has no copy to read", v.v)),
    }
}

/// The `diff` event answering `ask` (git runs in `shared` or the
/// agent's own folder, never in one that is gone).
fn diff_answer(ask: Ask, shared: &Path, how: &BaseHow, req: Value) -> Value {
    let shared = shared.to_path_buf();
    // where the agent's work started (PR flow: origin's copy), and where
    // lands go (a land's title)
    let start = how.base(&shared);
    let trunk = || crate::trunk::trunk_short(&shared);
    let mut ev = json!({"ev": "diff", "req": req, "working": false, "uncommitted": false, "landed_ms": null});
    let out: Result<(Vec<diff::File>, Option<PathBuf>), String> = match ask {
        // its folder is gone (archived, or removed by hand): never
        // git in it; its branch vs main if the branch is still
        // there, else one plain line (designer m_7393)
        Ask::Agent(name, Where::Checkout { dir, .. }, branch, archived) if archived || !dir.is_dir() => {
            let base = start.clone();
            ev["title"] = json!(format!("{} vs {}", name, base));
            ev["base"] = json!(base);
            ev["branch"] = json!(branch);
            match branch.filter(|b| diff::has_ref(&shared, b)) {
                Some(b) => diff::branch(&shared, &base, &b).map(|(f, c)| {
                    ev["commits"] = json!(c);
                    (f, Some(shared.clone()))
                }),
                None => {
                    ev["gone"] = json!(true);
                    ev["note"] = json!(if archived {
                        format!("{} is archived and its folder is gone", name)
                    } else {
                        format!("{}'s folder is gone", name)
                    });
                    Ok((Vec::new(), None))
                }
            }
        }
        Ask::Agent(name, place, branch, _) => match place {
            Where::Checkout { dir, base } => {
                let base = base.unwrap_or_else(|| start.clone());
                ev["title"] = json!(format!("{} vs {}", name, base));
                ev["base"] = json!(base);
                ev["branch"] = json!(branch);
                diff::checkout(&dir, &base).map(|(f, c, u)| {
                    ev["commits"] = json!(c);
                    ev["uncommitted"] = json!(u);
                    ev["working"] = json!(true);
                    (f, Some(dir))
                })
            }
            Where::Shared { dir, files } => {
                // the shared folder isn't only main's (designer
                // m_7354): `your folder vs main`, the same shape
                // as a branch's; the files are still the agent's
                ev["title"] = json!(format!("your folder vs {}", trunk()));
                ev["base"] = json!("HEAD");
                ev["branch"] = json!(null);
                ev["commits"] = json!(0);
                ev["uncommitted"] = json!(true);
                ev["working"] = json!(true);
                diff::own_files(&dir, &files).map(|f| (f, Some(dir)))
            }
        },
        Ask::Branch(b) => {
            let base = start.clone();
            ev["title"] = json!(format!("{} vs {}", b, base));
            ev["base"] = json!(base);
            ev["branch"] = json!(b);
            if diff::has_ref(&shared, &b) {
                diff::branch(&shared, &base, &b).map(|(f, c)| {
                    ev["commits"] = json!(c);
                    (f, Some(shared.clone()))
                })
            } else {
                ev["note"] = json!(format!("there's no branch named {}.", b));
                Ok((Vec::new(), None))
            }
        }
        Ask::Range(r, agent) => {
            // `diff-focus landed on main · e0f3df5` (the TUI's
            // head adds `· 11 files`)
            let to: String = r.split("..").last().unwrap_or(&r).trim_start_matches('.').chars().take(7).collect();
            let title = if agent.is_empty() { r.clone() } else { format!("{} landed on {} · {}", agent, trunk(), to) };
            ev["title"] = json!(title);
            ev["branch"] = json!(null);
            ev["base"] = json!(r.split("..").next().unwrap_or(""));
            diff::range(&shared, &r).map(|(f, c)| {
                ev["commits"] = json!(c);
                (f, Some(shared.clone()))
            })
        }
        Ask::Pr(at, local) => {
            let url = match at {
                PrAt::Url(u) => Some(u),
                PrAt::Number(n) => crate::worktree::git(&shared, &["remote", "get-url", "origin"]).ok().and_then(|r| pr_url_in(r.trim(), n)),
            };
            match url.and_then(|u| crate::forge::pr_link(&u)) {
                Some(l) => {
                    ev["title"] = json!(format!("PR #{}", l.number));
                    ev["pr"] = json!(l.number);
                    diff::pr(&l.url, local.as_ref())
                }
                None => Err("this PR isn't on this project's forge".into()),
            }
        }
        // its text is a diff already: the one parser, no git
        Ask::Artifact(a) => {
            ev["title"] = json!(a.title);
            ev["artifact"] = json!(a.id);
            ev["version"] = json!(a.v);
            std::fs::read(&a.copy).map(|b| (diff::parse(&String::from_utf8_lossy(&b)), None)).map_err(|e| format!("{} v{}: {}", a.id, a.v, e))
        }
        Ask::Bad(e) => Err(e),
    };
    match out {
        Ok((mut files, root)) => {
            ev["cut_files"] = json!(diff::cap(&mut files, diff::MAX_DIFF_LINES));
            let (n, add, del) = diff::stat(&files);
            ev["stat"] = json!({"files": n, "add": add, "del": del});
            ev["files"] = json!(files.iter().map(|f| diff::file_json(f, root.as_deref())).collect::<Vec<_>>());
        }
        Err(e) => {
            ev["files"] = json!([]);
            ev["error"] = json!(e);
        }
    }
    ev
}

/// The `landed` record of a land, after its `info` line in main's feed
/// (from = the target's tip before, short). None when git cannot say.
pub(super) fn landed_rec(shared: &Path, agent: &str, target: &str, sha: &str, commits: usize) -> Option<Feed> {
    if commits == 0 {
        return None;
    }
    let out = crate::tools_env::git_command()
        .ok()?
        .arg("-C")
        .arg(shared)
        .args(["rev-parse", "--short", &format!("{}~{}", sha, commits)])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let from = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (files, add, del) = diff::range_stat(shared, &from, sha);
    Some(Feed::Landed(bise_proto::core::feed::Landed {
        v: 1,
        agent: agent.to_string(),
        target: target.to_string(),
        from,
        sha: sha.to_string(),
        files: files as u64,
        add,
        del,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An older client's `"pr":31` and a new one's link ask the same PR
    /// (architect m_54680): the number is made a link in origin's repo;
    /// the link wins when both come.
    #[test]
    fn an_old_pr_number_and_its_link_ask_the_same_pr() {
        let url = "https://github.com/acme/web/pull/31";
        assert_eq!(pr_at(&json!({"pr": 31})), Some(PrAt::Number(31)));
        assert_eq!(pr_at(&json!({"pr_url": "https://GitHub.com/Acme/Web/pull/31"})), Some(PrAt::Url(url.into())));
        assert_eq!(pr_at(&json!({"pr": 9, "pr_url": url})), Some(PrAt::Url(url.into())));
        for remote in ["git@github.com:acme/web.git", "https://github.com/acme/web.git", "https://github.com/acme/web"] {
            assert_eq!(pr_url_in(remote, 31).as_deref(), Some(url), "{remote}");
        }
        assert_eq!(pr_at(&json!({"branch": "x"})), None);
        assert_eq!(pr_url_in("/a/local/folder", 31), None);
    }

    fn sh(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(ok.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&ok.stderr));
    }

    /// A repo with an agent's worktree on sb/t1 (one commit), then the
    /// worktree removed as an archive does.
    fn gone(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("sb-art-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let shared = root.join("repo");
        let wt = root.join("wt-t1");
        std::fs::create_dir_all(&shared).unwrap();
        sh(&shared, &["init", "-q", "-b", "main"]);
        std::fs::write(shared.join("a.txt"), "one\n").unwrap();
        sh(&shared, &["add", "."]);
        sh(&shared, &["commit", "-q", "-m", "base"]);
        sh(&shared, &["worktree", "add", "-q", "-b", "sb/t1", wt.to_str().unwrap()]);
        std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap();
        sh(&wt, &["commit", "-q", "-am", "t1"]);
        sh(&shared, &["worktree", "remove", "--force", wt.to_str().unwrap()]);
        assert!(!wt.exists());
        (shared, wt)
    }

    fn agent(wt: &Path, branch: Option<&str>, archived: bool) -> Ask {
        Ask::Agent("t1".into(), Where::Checkout { dir: wt.to_path_buf(), base: None }, branch.map(String::from), archived)
    }

    fn no_git_fatal(ev: &Value) {
        let s = ev.to_string();
        assert!(!s.contains("fatal") && !s.contains("cannot change to"), "{s}");
    }

    /// try-diff-open (architect m_52963): a feature's diff kept at `sb
    /// feature ready` (its artifact), read back through the store, is the
    /// same files and lines as the branch's diff at that tip; an older
    /// version stays readable, an unknown id or version is refused.
    #[test]
    fn a_diff_artifact_reads_as_the_branchs_diff() {
        let (shared, _) = gone("artdiff");
        let state = shared.parent().unwrap().join("state");
        sh(&shared, &["branch", "-q", "-m", "sb/t1", "cu"]);
        let file = state.join("features/cu.diff");
        let store = crate::artifacts::Store::new(&state);
        crate::feature::write_diff(&shared, "cu", &file).unwrap();
        let add = crate::feature::diff_artifact("cu", "abc1234", &file);
        let id = store.add(&add, 1).unwrap().meta.id;
        assert_eq!(id, "cu-diff");
        let strip = |mut ev: Value| {
            for f in ev["files"].as_array_mut().unwrap() {
                f.as_object_mut().unwrap().remove("abs");
            }
            (ev["files"].clone(), ev["stat"].clone())
        };
        let read = diff_answer(artifact_ask(&store, &id, None), &shared, &BaseHow::default(), json!(1));
        assert_eq!((read["title"].clone(), read["artifact"].clone(), read["version"].clone()), (json!("cu at abc1234"), json!("cu-diff"), json!(1)));
        assert_eq!(read["error"], Value::Null);
        let branch = diff_answer(Ask::Branch("cu".into()), &shared, &BaseHow::default(), json!(1));
        assert_eq!(strip(read), strip(branch.clone()));
        assert_eq!(branch["files"][0]["hunks"][0]["lines"], json!([" one", "+two"]));
        // the next ready: a next version, the first still read
        std::fs::create_dir_all(shared.join("b")).unwrap();
        sh(&shared, &["checkout", "-q", "cu"]);
        std::fs::write(shared.join("b/c.txt"), "c\n").unwrap();
        sh(&shared, &["add", "."]);
        sh(&shared, &["commit", "-q", "-m", "c"]);
        sh(&shared, &["checkout", "-q", "main"]);
        crate::feature::write_diff(&shared, "cu", &file).unwrap();
        let next = store.add(&crate::feature::diff_artifact("cu", "def5678", &file), 2).unwrap().meta;
        assert_eq!((next.id.as_str(), next.title.as_str(), next.versions.len()), ("cu-diff", "cu at def5678", 2));
        assert_eq!(diff_answer(artifact_ask(&store, "cu-diff", None), &shared, &BaseHow::default(), json!(1))["files"].as_array().unwrap().len(), 2);
        assert_eq!(diff_answer(artifact_ask(&store, "cu-diff", Some(1)), &shared, &BaseHow::default(), json!(1))["files"].as_array().unwrap().len(), 1);
        assert_eq!(diff_answer(artifact_ask(&store, "cu-diff", Some(9)), &shared, &BaseHow::default(), json!(1))["error"], "cu-diff has no version 9");
        assert_eq!(diff_answer(artifact_ask(&store, "nope", None), &shared, &BaseHow::default(), json!(1))["error"], "no artifact nope");
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }

    #[test]
    fn a_gone_worktree_shows_its_branch_vs_main() {
        let (shared, wt) = gone("branch");
        for archived in [true, false] {
            let ev = diff_answer(agent(&wt, Some("sb/t1"), archived), &shared, &BaseHow::default(), json!(1));
            no_git_fatal(&ev);
            assert_eq!(ev["title"], "t1 vs main");
            assert_eq!(ev["error"], Value::Null);
            assert_eq!(ev["note"], Value::Null);
            assert_eq!(ev["commits"], 1);
            assert_eq!(ev["files"][0]["path"], "a.txt");
        }
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }

    #[test]
    fn a_gone_worktree_without_its_branch_says_so_in_one_line() {
        let (shared, wt) = gone("nobranch");
        sh(&shared, &["branch", "-q", "-D", "sb/t1"]);
        let ev = diff_answer(agent(&wt, Some("sb/t1"), true), &shared, &BaseHow::default(), json!(1));
        no_git_fatal(&ev);
        assert_eq!(ev["note"], "t1 is archived and its folder is gone");
        assert_eq!(ev["gone"], true);
        assert_eq!(ev["error"], Value::Null);
        assert_eq!(ev["files"], json!([]));
        let ev = diff_answer(agent(&wt, None, false), &shared, &BaseHow::default(), json!(1));
        assert_eq!(ev["note"], "t1's folder is gone");
        // the same folder seen as a shared-folder agent's: never git in it
        let ev = diff_answer(Ask::Agent("t1".into(), Where::Shared { dir: wt.clone(), files: vec!["a.txt".into()] }, None, false), &shared, &BaseHow::default(), json!(1));
        no_git_fatal(&ev);
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }

    #[test]
    fn changes_count_a_commit_and_an_uncommitted_edit() {
        let root = std::env::temp_dir().join(format!("sb-art-changes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (shared, wt) = (root.join("repo"), root.join("wt-t1"));
        std::fs::create_dir_all(&shared).unwrap();
        sh(&shared, &["init", "-q", "-b", "main"]);
        std::fs::write(shared.join("README"), "hello\n").unwrap();
        sh(&shared, &["add", "."]);
        sh(&shared, &["commit", "-q", "-m", "init"]);
        sh(&shared, &["worktree", "add", "-q", "-b", "sb/t1", wt.to_str().unwrap()]);
        let place = Where::Checkout { dir: wt.clone(), base: None };
        assert_eq!(changes_of(&place, "main"), None, "nothing yet");
        std::fs::create_dir_all(wt.join("out")).unwrap();
        std::fs::write(wt.join("out/plans.csv"), "plan,price\nfree,0\npro,20\n").unwrap();
        sh(&wt, &["add", "out"]);
        sh(&wt, &["commit", "-q", "-m", "plans"]);
        std::fs::write(wt.join("README"), "hello\nmore\n").unwrap();
        std::fs::write(wt.join("new.txt"), "x\n").unwrap();
        assert_eq!(changes_of(&place, "main"), Some(bise_proto::rows::Changes { files: 3, add: 5, del: 0 }));
        // its folder gone: null, and no git in it
        sh(&shared, &["worktree", "remove", "--force", wt.to_str().unwrap()]);
        assert_eq!(changes_of(&place, "main"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn other_git_errors_read_as_one_line() {
        let (shared, _) = gone("errors");
        let ev = diff_answer(Ask::Branch("sb/nope".into()), &shared, &BaseHow::default(), json!(1));
        assert_eq!(ev["note"], "there's no branch named sb/nope.");
        let ev = diff_answer(Ask::Range("abc1234..def5678".into(), String::new()), &shared, &BaseHow::default(), json!(1));
        let e = ev["error"].as_str().unwrap();
        assert!(e.starts_with("git couldn't read this diff: ") && !e.contains('\n') && !e.contains("fatal"), "{e}");
        let _ = std::fs::remove_dir_all(shared.parent().unwrap());
    }
}
