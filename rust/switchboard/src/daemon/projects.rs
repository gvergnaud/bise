//! The hub's side of the projects (bise desktop S1).
//!
//! - At its start the hub adds its workspace to the projects registry
//!   (`bise_home::projects`, the user's decision: `bise` run in a repo
//!   lists it), unless it is the home workspace (row 0 already) or a test
//!   hub (`SB_STATE_DIR`: its workspace is a throwaway).
//! - It keeps `<state>/view.json` (`crate::view`): written at its start,
//!   then when its state changed (debounced, checked at each tick), and
//!   with `stopped_ms` when it stops for good (idle-exit or `stop_hub`),
//!   with the hub's pulse (daemon/pulse.rs: its last value, a fresh one
//!   at the stop, so a stopped project's sidebar row keeps it).
//!   Only this hub writes it; the window, `bise project list` and bise's
//!   reads of other projects read it.

use super::{log_line, rename_logged, write_logged, Shell};
use crate::paths::Paths;
use crate::proto_view::Since;
use crate::snapshot::Snapshot;
use crate::util::now_ms;
use crate::view::{self, Writer};

/// The hub's view writer (one field of the Shell).
#[derive(Default)]
pub(super) struct Projects {
    writer: Writer,
    since: Since,
    /// The last pushed snapshot not written yet: the tick writes it
    /// (send-fast: the tick built a whole new snapshot for it, pages and
    /// artifacts read from disk, 12% of his loop's time).
    pending: Option<Snapshot>,
}

impl Projects {
    /// At the hub's start: register its workspace (see the header). A side
    /// effect on purpose: it is the Shell field's initializer in
    /// `daemon::run`, so the registration costs run no line of its own.
    pub(super) fn boot(home: &bise_home::Home, paths: &Paths) -> Projects {
        if bise_home::env::test_setting("SB_STATE_DIR").is_none() {
            let home_ws = crate::paths::home_workspace();
            if bise_home::projects::canonical(&paths.workspace) != bise_home::projects::canonical(&home_ws) {
                match bise_home::projects::add_path(&home, &home_ws, &paths.workspace, None, now_ms()) {
                    Ok(bise_home::projects::Added::New(p)) => log_line(paths, &format!("projects: added as {}", p.name)),
                    Ok(bise_home::projects::Added::Already(_)) => {}
                    Err(e) => log_line(paths, &format!("projects: not added ({e})")),
                }
            }
        }
        Projects::default()
    }
}

impl Shell {
    /// The hub's state changed (sb-core's `state` effect): `snap` is the
    /// snapshot it just broadcast (written now when due, else at a tick).
    pub(super) fn view_changed(&mut self, snap: &Snapshot) {
        let now = now_ms();
        self.projects.writer.changed(now);
        if self.projects.writer.due(now) {
            self.projects.pending = None;
            self.view_write(snap, None);
        } else {
            self.projects.pending = Some(snap.clone());
        }
    }

    /// At each tick: a change not written yet, once the debounce allows:
    /// the last pushed snapshot (no new build on the loop).
    pub(super) fn view_tick(&mut self) {
        if self.projects.writer.due(now_ms()) {
            let snap = match self.projects.pending.take() {
                Some(s) => s,
                None => self.snapshot(),
            };
            self.view_write(&snap, None);
        }
    }

    /// The hub stops for good: the last write, with when.
    pub(super) fn view_stopped(&mut self) {
        let snap = self.snapshot();
        self.view_write(&snap, Some(now_ms()));
    }

    fn view_write(&mut self, snap: &Snapshot, stopped_ms: Option<u64>) {
        let now = now_ms();
        let project = crate::paths::workspace_id(&self.opts.paths.workspace);
        // ⌘K's index: the same rows as the typed artifacts event
        let arts = self.artifact_rows(&self.artifacts_ev());
        // les bims: the same cached growth as the typed event
        let growth = self.growth_of(Some(snap), Some(arts.len() as u32));
        // and the same rows as the typed scheduled event
        let sched = crate::proto_view::scheduled(self.hub.timers());
        // the H1 sidebar: the pulse the typed event has (fresh at a stop)
        let pulse = self.pulse_view(stopped_ms.is_some());
        let cat = Some(&self.config.setup().catalog);
        let vision = |m: &str| cat.and_then(|c| c.vision(m));
        let v = view::of(snap, &project, now, self.projects.writer.last_activity_ms, stopped_ms, &mut self.projects.since, &vision, arts, sched, growth, pulse);
        let paths = self.opts.paths.clone();
        let (tmp, file) = (paths.state.join("view.json.tmp"), paths.state.join(view::FILE));
        if write_logged(&paths, &tmp, &view::render(&v)) {
            rename_logged(&paths, &tmp, &file);
        }
        self.projects.writer.wrote(now);
    }
}
