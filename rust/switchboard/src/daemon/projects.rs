//! The hub's side of the projects (bise desktop S1).
//!
//! - At its start the hub adds its workspace to the projects registry
//!   (`bise_home::projects`, the user's decision: `bise` run in a repo
//!   lists it), unless it is the home workspace (row 0 already) or a test
//!   hub (`SB_STATE_DIR`: its workspace is a throwaway).
//! - It keeps `<state>/view.json` (`crate::view`): written at its start,
//!   then when its state changed (debounced, checked at each tick), and
//!   with `stopped_ms` when it stops for good (idle-exit or `stop_hub`).
//!   Only this hub writes it; the window, `bise project list` and bise's
//!   reads of other projects read it.

use super::{log_line, rename_logged, write_logged, Shell};
use crate::paths::Paths;
use crate::proto_view::Since;
use crate::util::now_ms;
use crate::view::{self, Writer};
use serde_json::Value;

/// The hub's view writer (one field of the Shell).
#[derive(Default)]
pub(super) struct Projects {
    writer: Writer,
    since: Since,
}

impl Projects {
    /// At the hub's start: register its workspace (see the header). A side
    /// effect on purpose: it is the Shell field's initializer in
    /// `daemon::run`, so the registration costs run no line of its own.
    pub(super) fn boot(paths: &Paths) -> Projects {
        if bise_home::env::test_setting("SB_STATE_DIR").is_none() {
            let home = bise_home::Home::from_env();
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
    pub(super) fn view_changed(&mut self, snap: &Value) {
        let now = now_ms();
        self.projects.writer.changed(now);
        if self.projects.writer.due(now) {
            self.view_write(snap, None);
        }
    }

    /// At each tick: a change not written yet, once the debounce allows.
    pub(super) fn view_tick(&mut self) {
        if self.projects.writer.due(now_ms()) {
            let snap = self.snapshot();
            self.view_write(&snap, None);
        }
    }

    /// The hub stops for good: the last write, with when.
    pub(super) fn view_stopped(&mut self) {
        let snap = self.snapshot();
        self.view_write(&snap, Some(now_ms()));
    }

    fn view_write(&mut self, snap: &Value, stopped_ms: Option<u64>) {
        let now = now_ms();
        let project = crate::paths::workspace_id(&self.opts.paths.workspace);
        self.setup();
        let cat = self.setup.as_ref().map(|(_, s)| &s.catalog);
        let vision = |m: &str| cat.and_then(|c| c.vision(m));
        // ⌘K's index: the same rows as the typed artifacts event
        let arts = self.artifact_rows(&self.artifacts_ev());
        // and the same rows as the typed scheduled event
        let sched = crate::proto_view::scheduled(self.hub.timers());
        let v = view::of(snap, &project, now, self.projects.writer.last_activity_ms, stopped_ms, &mut self.projects.since, &vision, arts, sched);
        let paths = self.opts.paths.clone();
        let (tmp, file) = (paths.state.join("view.json.tmp"), paths.state.join(view::FILE));
        if write_logged(&paths, &tmp, &view::render(&v)) {
            rename_logged(&paths, &tmp, &file);
        }
        self.projects.writer.wrote(now);
    }
}
