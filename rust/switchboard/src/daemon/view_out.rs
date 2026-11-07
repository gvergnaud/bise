//! view.json written off the loop, and only when it changes
//! (c4-loop-never-waits): the loop builds the view (`crate::view::of`, from
//! the state it owns) and hands it to this worker, which renders it
//! (pretty JSON: his was 2.4 MB) and writes it (a temp file, then a
//! rename). A view equal to the last one written but for its times
//! (`written_ms`, `last_activity_ms`) is not written: a push that changed
//! nothing a reader sees leaves the file and its activity time as they
//! were. Views that queue while it writes: only the newest is written.
//!
//! The stop's view (`stopped_ms`) is the last one: [`ViewOut::finish`]
//! sends it and waits for the worker to write it, so nothing writes
//! after it.
//!
//! Owns `<state>/view.json` and its temp file (the only writer).

use super::log_line;
use crate::paths::Paths;
use bise_proto::draft::ProjectView;
use std::sync::mpsc::{channel, Receiver, Sender};

pub(super) struct ViewOut {
    tx: Option<Sender<ProjectView>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl ViewOut {
    /// The worker, writing into `paths.state`.
    pub(super) fn start(paths: Paths) -> ViewOut {
        let (tx, rx) = channel();
        let worker = std::thread::Builder::new().name("view-out".into()).spawn(move || run(&paths, rx)).ok();
        ViewOut { tx: Some(tx), worker }
    }

    /// A view to write (not when it is the last one written, times aside).
    pub(super) fn send(&self, v: ProjectView) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(v);
        }
    }

    /// The last view (the stop's): written, then the worker ends.
    pub(super) fn finish(&mut self, v: ProjectView) {
        self.send(v);
        self.tx = None;
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// Two views a reader can't tell apart: equal but for when they were
/// written and the activity time that comes with each push.
pub(super) fn same(a: &ProjectView, b: &ProjectView) -> bool {
    let bare = |v: &ProjectView| ProjectView { written_ms: 0, last_activity_ms: 0, ..v.clone() };
    bare(a) == bare(b)
}

fn run(paths: &Paths, rx: Receiver<ProjectView>) {
    let mut last: Option<ProjectView> = None;
    while let Ok(mut v) = rx.recv() {
        // the newest of those that queued meanwhile
        while let Ok(newer) = rx.try_recv() {
            v = newer;
        }
        take(&mut last, v, &mut |v| write(paths, v));
    }
}

/// One view for the file: written unless it is `last` but for its times;
/// `last` is the one written.
fn take(last: &mut Option<ProjectView>, v: ProjectView, write: &mut dyn FnMut(&ProjectView) -> bool) {
    if last.as_ref().is_some_and(|l| same(l, &v)) {
        return;
    }
    if write(&v) {
        *last = Some(v);
    }
}

/// view.json whole or not at all: its temp file, then a rename.
fn write(paths: &Paths, v: &ProjectView) -> bool {
    let (tmp, file) = (paths.state.join("view.json.tmp"), paths.state.join(crate::view::FILE));
    if let Err(e) = std::fs::write(&tmp, crate::view::render(v)) {
        log_line(paths, &format!("cannot write {}: {}", tmp.display(), e));
        return false;
    }
    if let Err(e) = std::fs::rename(&tmp, &file) {
        log_line(paths, &format!("cannot rename {} to {}: {}", tmp.display(), file.display(), e));
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view written at `written`, active at `activity`, with `arts`
    /// artifacts (the part a reader sees).
    fn view(written: u64, activity: u64, arts: u32) -> ProjectView {
        ProjectView {
            v: crate::view::V,
            project: "p-1".into(),
            written_ms: written,
            stopped_ms: None,
            last_activity_ms: activity,
            agents: vec![],
            cards: vec![],
            artifacts: vec![],
            artifacts_total: arts,
            scheduled: vec![],
            growth: None,
            pulse: None,
        }
    }

    #[test]
    fn a_view_differing_only_in_its_times_is_the_same() {
        assert!(same(&view(1, 1, 3), &view(9, 8, 3)));
        assert!(!same(&view(1, 1, 3), &view(1, 1, 4)));
        let stopped = ProjectView { stopped_ms: Some(5), ..view(1, 1, 3) };
        assert!(!same(&view(1, 1, 3), &stopped), "the stop is a change");
    }

    /// A change is written, a push that changed nothing a reader sees is
    /// not, and a failed write is tried again with the next view.
    #[test]
    fn only_a_change_is_written() {
        let (mut last, mut wrote) = (None, Vec::new());
        let mut ok = true;
        for v in [view(1, 1, 3), view(2, 2, 3), view(3, 3, 4)] {
            take(&mut last, v, &mut |v| {
                wrote.push(v.written_ms);
                ok
            });
        }
        assert_eq!(wrote, [1, 3]);
        ok = false;
        take(&mut last, view(4, 4, 5), &mut |_| ok);
        assert_eq!(last.as_ref().map(|l| l.written_ms), Some(3), "not written: the next one is compared to the file's");
    }

    /// The stop's view is the last write: finish waits for it.
    #[test]
    fn the_stops_view_is_written_last() {
        let d = std::env::temp_dir().join(format!("sb-view-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mut out = ViewOut::start(Paths { workspace: d.clone(), state: d.clone(), worktrees: d.clone(), run: d.clone() });
        out.send(view(1, 1, 3));
        out.send(view(2, 2, 4));
        out.finish(ProjectView { stopped_ms: Some(7), ..view(3, 3, 4) });
        let got = crate::view::read(&d).unwrap();
        assert_eq!((got.written_ms, got.stopped_ms), (3, Some(7)));
        let _ = std::fs::remove_dir_all(&d);
    }
}
