//! The boot photo's files on the hub's side (issue 36 chunk 4; the
//! decisions: crate::photo). At boot: the photo that fits the journal is
//! restored and only the events after it replayed, else the full replay
//! and one hub.log line saying why. Then a photo is taken, always between
//! two steps and only while every journal line of the session reached the
//! file (`journal_ok`): at the first tick after a boot that did a full
//! replay (outside the boot watch), at a clean
//! stop (skipped when the last one took longer than STOP_BUDGET_MS), and
//! at most once a day after IDLE_AFTER_MS with no journal event.

use super::{log_line, Shell};
use crate::core::{Hub, Revive};
use crate::paths::Paths;
use crate::photo::{self, Mark, Use};
use crate::util::now_ms;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The hub's photo bookkeeping.
pub(super) struct Photos {
    dir: PathBuf,
    /// the sb-core binary's key; none (unreadable): no photo at all
    key: Option<String>,
    /// the journal as a photo of now would hold it
    mark: Mark,
    /// when the last journal event was written, and the last photo taken (ms)
    last_event: u64,
    last_photo: u64,
    /// the events the last photo held (an idle photo of the same is useless)
    last_events: usize,
    /// how long the last photo took (ms)
    last_took: u64,
    /// a full replay's photo (`due`) or the idle one, waiting for a good
    /// moment (photo::moment): never inside the boot, where a loaded
    /// machine's slow encode read as a stuck boot and the boot watch
    /// stopped the hub (his -47 switch, keep-current m_37023: ~350 s of
    /// encode after a 703 s replay at load ~58), nor at a busy tick
    wait: photo::Wait,
    /// nothing waited in the loop's inbox behind the message being run
    /// (set by the serve loop before each one)
    pub(super) quiet: bool,
}

impl Photos {
    /// One more journal line written.
    pub(super) fn journaled(&mut self, ev: &Value) {
        self.mark.push(ev);
        self.last_event = now_ms();
    }
}

/// The photo of `key` in `dir` that fits `events`, or why none does.
fn pick(dir: &Path, key: Option<&str>, events: &[Value], migrated: bool) -> (Option<(usize, Vec<String>)>, Option<String>) {
    let Some(key) = key else { return (None, Some("sb-core's binary unreadable".into())) };
    let read = photo::read(dir, key);
    match photo::usable(read.as_ref().map(|(h, _)| h), key, events, migrated) {
        Use::Photo { k } => (read.map(|(_, st)| (k, st)), None),
        Use::Full { why } => (None, Some(why)),
    }
}

/// The boot's rebuild: from the photo when one fits, else every event.
/// A full replay takes a photo at once (the next boot starts from it).
/// sb-core dead or late on the replay: the error, the hub does not start.
pub(super) fn boot(hub: &mut Hub, paths: &Paths, core_bin: &Path, events: &[Value], migrated: bool, watch: &super::boot::Boot) -> Result<Photos, crate::core::CoreError> {
    let dir = paths.state.join("photo");
    photo::clean(&dir);
    let key = photo::key_of(core_bin);
    let t0 = now_ms();
    // each long part says its progress in hub.log (Boot::moving): the
    // photo read, the journal checked against it in one pass (its mark
    // and the photo's prefix together), its slices restored, the replay
    let read = key.as_deref().and_then(|k| photo::read(&dir, k));
    watch.step(&match &read {
        Some((h, s)) => format!("photo read ({} slices, {} events)", s.len(), h.mark.events),
        None => "no photo of this sb-core".to_string(),
    });
    let k = read.as_ref().map_or(0, |(h, _)| h.mark.events);
    let (mark, at) = Mark::of_and_at(events, k, &mut |done, total| watch.moving(&format!("journal checked {done}/{total} events")));
    let (from, why) = match key.as_deref() {
        None => (None, Some("sb-core's binary unreadable".to_string())),
        Some(key) => match photo::usable_at(read.as_ref().map(|(h, _)| h), key, events.len(), &at, migrated) {
            Use::Photo { k } => (read.map(|(_, st)| (k, st)), None),
            Use::Full { why } => (None, Some(why)),
        },
    };
    let mut ph = Photos { dir, key, mark, last_event: now_ms(), last_photo: 0, last_events: 0, last_took: 0, wait: photo::Wait::default(), quiet: false };
    let rebuilt = hub.rebuild(events, from, &mut |done, total| watch.replayed(done, total), &mut |done, total| watch.moving(&format!("photo restore {done}/{total} slices")))?;
    if !rebuilt.skipped.is_empty() {
        let mut kinds: Vec<String> = rebuilt.skipped.iter().map(|e| e["type"].to_string()).collect();
        kinds.dedup();
        let n = rebuilt.skipped.len();
        log_line(paths, &format!("journal: {n} events of a kind this hub does not know (not applied; a newer hub wrote them?): {}", kinds.join(", ")));
    }
    match rebuilt.photo {
        Ok(k) => log_line(paths, &format!("boot: from the photo of {k} events, {} replayed ({} ms)", events.len() - k, now_ms() - t0)),
        Err(e) => {
            let why = why.unwrap_or(e);
            log_line(paths, &format!("boot: full replay of {} events ({} ms): {why}", events.len(), now_ms() - t0));
            ph.wait.due = true;
        }
    }
    Ok(ph)
}

/// The revival's photo (crate::core::Revive): the same pick, on the
/// journal the revival read.
pub(super) fn revive(paths: &Paths, ph: &Photos, journal: Box<dyn FnMut() -> Vec<Value>>) -> Revive {
    let lp = paths.clone();
    let (dir, key) = (ph.dir.clone(), ph.key.clone());
    Revive::new(journal, Box::new(move |s| log_line(&lp, s))).with_photo(Box::new(move |events| pick(&dir, key.as_deref(), events, false).0))
}

/// Take a photo now (between two steps): `journal_ok` false, none.
fn take(hub: &mut Hub, ph: &mut Photos, paths: &Paths, journal_ok: bool, why: &str) {
    let Some(key) = ph.key.clone().filter(|_| journal_ok) else { return };
    let t0 = now_ms();
    let Some(st) = hub.photo() else {
        return log_line(paths, &format!("photo ({why}): sb-core has none now (a step waits on the hub)"));
    };
    let res = photo::write(&ph.dir, &key, &ph.mark, &st);
    ph.last_took = now_ms() - t0;
    match res {
        Ok(()) => {
            (ph.last_photo, ph.last_events) = (now_ms(), ph.mark.events);
            log_line(paths, &format!("photo ({why}): {} events, {} ms", ph.mark.events, ph.last_took));
        }
        Err(e) => log_line(paths, &format!("photo ({why}) not written: {e}")),
    }
}

impl Shell {
    /// At each tick: a full replay's photo or the idle one, when the tick
    /// is a good moment (photo::Wait); else it stays due, one hub.log line.
    pub(super) fn photo_idle(&mut self) {
        let ph = &self.photos;
        let why = if ph.wait.due { "after a full replay" } else { "idle" };
        let idle = ph.mark.events != ph.last_events && photo::idle_due(now_ms(), ph.last_event, ph.last_photo);
        let lands = !self.lands.ranks().is_empty();
        let quiet = ph.quiet;
        match self.photos.wait.tick(idle, quiet, lands, crate::load::factor()) {
            Some(photo::Act::Take) => take(&mut self.hub, &mut self.photos, &self.opts.paths, self.journal.ok(), why),
            Some(photo::Act::Say(s)) => log_line(&self.opts.paths, &s),
            None => {}
        }
    }

    /// At a clean stop: the photo, unless the last one was slow or the
    /// machine is loaded (the switcher does not wait long for a hub that
    /// goes, and a stop under load must not stretch).
    pub(super) fn photo_stop(&mut self) {
        let ph = &self.photos;
        if ph.mark.events == ph.last_events {
            return;
        }
        if ph.last_took > photo::STOP_BUDGET_MS {
            return log_line(&self.opts.paths, &format!("photo at stop skipped: the last took {} ms", ph.last_took));
        }
        let f = crate::load::factor();
        if crate::load::is_loaded(f) {
            return log_line(&self.opts.paths, &format!("photo at stop skipped: load x{f:.1}"));
        }
        take(&mut self.hub, &mut self.photos, &self.opts.paths, self.journal.ok(), "stop");
    }
}

#[cfg(test)]
#[path = "photo_take_tests.rs"]
mod tests;
