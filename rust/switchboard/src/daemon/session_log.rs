//! Each agent's session log (BISE-196): the hub owns the writer. A fresh
//! REPL gets the log's projection (BEND-SESSION 2) in
//! `agents/<dir>/session.resume.txt` and `BEND_CONTINUE`; its `ev:` lines
//! (BISE-195) are appended by a `Recorder`. `agents/<dir>/session` holds
//! the session id; the log lives in `~/.bise/sessions/<id>/`. An agent
//! still on its `session.txt` moves once, at its first fresh REPL
//! (BISE-197: checked byte for byte; the `.txt` stays as the backup; a
//! failed move keeps the agent on the `.txt`, as before).
use super::{log_line, Shell};
use crate::model::Agent;
use bise_session::migrate::{migrate_txt, Outcome, Source};
use bise_session::recorder::Recorder;
use bise_session::types::AgentRef;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The file the REPL loads from and saves to once its session has a log.
pub(super) const RESUME_FILE: &str = "session.resume.txt";
/// The wire offset of the last `ev:` line recorded (an adopted REPL's
/// wire log is read again from `wire.offset`: no event twice).
const EV_OFFSET: &str = "session.offset";

fn writer_name() -> String {
    format!("bise {}", env!("CARGO_PKG_VERSION"))
}

/// The session id an agent's folder names (its `session` file), trimmed;
/// None when there is none yet. The one reader of that file (architect
/// m_14498): the session log here and the window's `tool_out`.
pub(super) fn session_of(adir: &Path) -> Option<String> {
    std::fs::read_to_string(adir.join("session")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn write_small(path: &Path, text: &str) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// `sessions/migrated.json`: `{<txt path>: <session id>}`.
fn note_migrated(sessions: &Path, txt: &Path, id: &str) {
    let path = sessions.join("migrated.json");
    let mut m: serde_json::Map<String, Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    m.insert(txt.to_string_lossy().to_string(), json!(id));
    write_small(&path, &serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_default());
}

impl Shell {
    fn session_start(&self, a: &Agent, id: &str) -> Value {
        let hub = self.opts.paths.state.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut agent = json!({"hub": hub, "name": a.name});
        if let Some(p) = &a.parent {
            agent["parent"] = json!(p);
        }
        json!({"session": id, "format": 1, "created_by": writer_name(),
               "cwd": a.ws.path, "agent": agent})
    }

    fn keep(&mut self, dir: &str, mut r: Recorder) {
        let home = &self.opts.home;
        r.set_redactor(bise_session::Redactor::from_home(&home.auth_file(), &home.env_files()));
        self.recorders.insert(dir.to_string(), r);
    }

    /// Before a fresh REPL of `a`: its session log ready, and the file it
    /// loads. Returns (BEND_SESSION_FILE, whether to continue).
    pub(super) fn prepare_session(&mut self, a: &Agent, dir: &str, adir: &Path, resume: bool) -> (PathBuf, bool) {
        self.recorders.remove(dir); // the old process's writer, if any
        let (sessions, blobs) = (self.opts.home.sessions_dir(), self.opts.home.blobs_dir());
        let idf = adir.join("session");
        let resume_file = adir.join(RESUME_FILE);
        let legacy = adir.join("session.txt");
        write_small(&adir.join(EV_OFFSET), "0");
        let id = session_of(adir);
        let log = |sh: &Self, s: String| log_line(&sh.opts.paths, &format!("session log of {}: {}", a.name, s));
        // a session.txt newer than the id: an older REPL (adopted by this
        // hub) went on saving it after the move: move it again
        let stale = |idf: &Path| {
            let m = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            matches!((m(&legacy), m(idf)), (Some(t), Some(i)) if t > i)
        };
        if resume {
            if legacy.exists() && (id.is_none() || stale(&idf)) {
                let src = Source { txt: legacy.clone(), cwd: a.ws.path.clone(),
                    agent: serde_json::from_value::<AgentRef>(self.session_start(a, "")["agent"].clone()).ok(), model: None };
                match migrate_txt(&src, &sessions, &blobs, &writer_name()) {
                    Outcome::Migrated { id: new, messages, dropped_lines, .. } => {
                        write_small(&idf, &new);
                        note_migrated(&sessions, &legacy, &new);
                        log(self, format!("session.txt moved to {new} ({messages} messages, {dropped_lines} lines today's loader skips; the .txt stays)"));
                        return self.resume_log(a, dir, &sessions.join(&new), &blobs, &resume_file, &legacy);
                    }
                    Outcome::Unloadable => log(self, "session.txt holds no session: a new log".into()),
                    Outcome::Failed(why) => {
                        log(self, format!("session.txt NOT moved, the agent stays on it: {why}"));
                        return (legacy, true);
                    }
                }
            } else if let Some(id) = &id {
                return self.resume_log(a, dir, &sessions.join(id), &blobs, &resume_file, &legacy);
            }
        }
        let new = bise_session::new_session_id();
        match Recorder::create(&sessions.join(&new), &blobs, self.session_start(a, &new), &writer_name()) {
            Ok(r) => {
                write_small(&idf, &new);
                let _ = std::fs::remove_file(&resume_file);
                self.keep(dir, r);
                (resume_file, false)
            }
            Err(e) => {
                log(self, format!("no session log ({e}): the REPL saves session.txt"));
                (legacy, false)
            }
        }
    }

    fn resume_log(&mut self, a: &Agent, dir: &str, sdir: &Path, blobs: &Path, resume_file: &Path, legacy: &Path) -> (PathBuf, bool) {
        match Recorder::resume(sdir, blobs, &writer_name()) {
            Ok(r) => {
                if !r.repaired.is_empty() {
                    log_line(&self.opts.paths, &format!("session log of {}: a turn cut by a crash closed ({} events)", a.name, r.repaired.len()));
                }
                let cont = r.has_context();
                if let Err(e) = r.project_to(resume_file) {
                    log_line(&self.opts.paths, &format!("session log of {}: projection failed ({e}): the REPL's own checkpoint is used", a.name));
                }
                self.keep(dir, r);
                (resume_file.to_path_buf(), cont)
            }
            Err(e) => {
                // locked or read-only: the REPL's own checkpoint, else the .txt
                log_line(&self.opts.paths, &format!("session log of {}: {e}", a.name));
                let f = if resume_file.exists() { resume_file.to_path_buf() } else { legacy.to_path_buf() };
                let cont = f.exists();
                (f, cont)
            }
        }
    }

    /// After the boot: every session still on a `.txt` that no live REPL
    /// uses moves now, in the background (BISE-197: the user's move is
    /// once and whole): the solo `sessions/*.txt` and the agents with no
    /// REPL (done, archived). A live REPL's agent moves at its next fresh
    /// REPL (prepare_session).
    pub(super) fn migrate_the_rest(&self) {
        let live: std::collections::BTreeSet<String> = self.pids.keys().cloned().collect();
        let agents = self.opts.paths.state.join("agents");
        let paths = self.opts.paths.clone();
        let hub = paths.state.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let home = self.opts.home.clone();
        std::thread::spawn(move || {
            let (sessions, blobs) = (home.sessions_dir(), home.blobs_dir());
            let done: serde_json::Map<String, Value> = std::fs::read_to_string(sessions.join("migrated.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
            let mut todo: Vec<(PathBuf, Option<(PathBuf, String)>)> = Vec::new();
            for e in std::fs::read_dir(&sessions).into_iter().flatten().flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "txt") && !done.contains_key(&*p.to_string_lossy()) {
                    todo.push((p, None));
                }
            }
            for e in std::fs::read_dir(&agents).into_iter().flatten().flatten() {
                let dir = e.file_name().to_string_lossy().to_string();
                let txt = e.path().join("session.txt");
                if !live.contains(&dir) && txt.exists() && !e.path().join("session").exists() {
                    todo.push((txt, Some((e.path(), dir))));
                }
            }
            let (mut ok, mut failed) = (0, 0);
            for (txt, agent) in todo {
                let agent_ref = agent.as_ref().map(|(_, d)| AgentRef { hub: hub.clone(), name: d.clone(), parent: None });
                let src = Source { txt: txt.clone(), cwd: String::new(), agent: agent_ref, model: None };
                match migrate_txt(&src, &sessions, &blobs, &writer_name()) {
                    Outcome::Migrated { id, .. } => {
                        if let Some((adir, _)) = &agent {
                            write_small(&adir.join("session"), &id);
                        }
                        note_migrated(&sessions, &txt, &id);
                        ok += 1;
                    }
                    Outcome::Unloadable => {}
                    Outcome::Failed(why) => {
                        failed += 1;
                        log_line(&paths, &format!("session log: {} NOT moved (kept on its .txt): {why}", txt.display()));
                    }
                }
            }
            if ok + failed > 0 {
                log_line(&paths, &format!("session log: {ok} sessions moved to logs, {failed} kept on their .txt"));
            }
        });
    }

    /// An adopted REPL (a hub restart): its log goes on, no repair.
    pub(super) fn attach_session(&mut self, a: &Agent, dir: &str, adir: &Path) {
        let Some(id) = session_of(adir) else { return };
        let home = &self.opts.home;
        match Recorder::attach(&home.sessions_dir().join(&id), &home.blobs_dir()) {
            Ok(r) => self.keep(dir, r),
            Err(e) => log_line(&self.opts.paths, &format!("session log of {}: {e}", a.name)),
        }
    }

    /// Before a fresh REPL resets the wire log: the `ev:` lines of the
    /// old process the hub never took (a stop ahead of queued lines, a
    /// killed hub) go into its session log first, so the record and the
    /// next resume have them. Returns how many.
    pub(super) fn catch_up_wire(&mut self, dir: &str, adir: &Path) -> usize {
        let Ok(bytes) = std::fs::read(adir.join("wire.log")) else { return 0 };
        let done: u64 = std::fs::read_to_string(adir.join(EV_OFFSET)).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        let evs = pending_evs(&bytes, done);
        if evs.is_empty() {
            return 0;
        }
        if !self.recorders.contains_key(dir) {
            let Some(id) = session_of(adir) else { return 0 };
            let home = &self.opts.home;
            match Recorder::attach(&home.sessions_dir().join(&id), &home.blobs_dir()) {
                Ok(r) => self.keep(dir, r),
                Err(e) => {
                    log_line(&self.opts.paths, &format!("session log of {dir}: {e}"));
                    return 0;
                }
            }
        }
        let n = evs.len();
        for (ev, offset) in evs {
            self.on_ev_line(dir, &ev, offset);
        }
        n
    }

    /// A wire line; true when it was an `ev:` line (not for the feed).
    pub(super) fn on_ev_line(&mut self, dir: &str, line: &str, offset: u64) -> bool {
        let Some(ev) = line.strip_prefix("  ev: ") else { return false };
        let adir = self.opts.paths.agent_dir(dir);
        let done: u64 = std::fs::read_to_string(adir.join(EV_OFFSET)).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        if offset <= done {
            return true; // recorded before a hub restart
        }
        if let Some(r) = self.recorders.get_mut(dir) {
            if let Err(e) = r.on_ev(ev) {
                log_line(&self.opts.paths, &format!("session log of {dir}: {e}"));
            }
            write_small(&adir.join(EV_OFFSET), &offset.to_string());
        }
        true
    }
}

/// The `ev:` lines of a wire log after `done` (the offset of the last one
/// recorded), each with the offset just after it (`pump_wire`'s). A last
/// line without its newline is not whole yet: left out.
fn pending_evs(wire: &[u8], done: u64) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut offset = 0u64;
    for raw in wire.split_inclusive(|b| *b == b'\n') {
        offset += raw.len() as u64;
        if !raw.ends_with(b"\n") || offset <= done {
            continue;
        }
        let line = String::from_utf8_lossy(raw).trim_end_matches(['\n', '\r']).to_string();
        if line.starts_with("  ev: ") {
            out.push((line, offset));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::pending_evs;

    /// A stop that went ahead of queued lines (2026-10-11): the `ev:`
    /// lines after the recorded offset are found again, with pump_wire's
    /// offsets; the feed lines and a half-written line are not.
    #[test]
    fn the_evs_the_hub_never_took_are_found_again() {
        let wire = b"  ev: {\"a\":1}\nhello\n  ev: {\"a\":2}\n  ev: {\"a\":3}\n  ev: {\"a\"";
        let first = "  ev: {\"a\":1}\n".len() as u64;
        assert_eq!(pending_evs(wire, 0).len(), 3);
        let after = pending_evs(wire, first);
        assert_eq!(after.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["  ev: {\"a\":2}", "  ev: {\"a\":3}"]);
        assert_eq!(after[0].1, first + "hello\n".len() as u64 + "  ev: {\"a\":2}\n".len() as u64);
        assert!(pending_evs(wire, wire.len() as u64).is_empty());
    }
}
