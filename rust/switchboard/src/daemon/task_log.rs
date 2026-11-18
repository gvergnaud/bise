//! A task's Log on the hub's side (roma idea 3; architect m_65317): the
//! `task_log` ask of a Task tab and `sb task show --log` read the journal
//! from its end, on a thread (never on the hub's loop), until the task's
//! birth, and the land records; crate::task_log makes the lines. Reads
//! only, nothing kept.

use super::*;
use bise_proto::hub::HubEv;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A line's end.
const NL: u8 = 10;

/// How much of the file one read takes, from its end.
const CHUNK: u64 = 1 << 20;

/// Every line of `path`, newest first, to `f` until it says stop; the
/// file read from its end in chunks. A line not valid UTF-8 is skipped.
pub(crate) fn rev_lines(path: &Path, mut f: impl FnMut(&str) -> bool) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut end = file.metadata()?.len();
    // the bytes of a line cut by the chunk's start, kept for the next read
    let mut tail: Vec<u8> = Vec::new();
    while end > 0 {
        let start = end.saturating_sub(CHUNK);
        let mut buf = vec![0u8; (end - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buf)?;
        buf.extend_from_slice(&tail);
        let mut stop = buf.len();
        // the lines after the first newline are whole; the head may be cut
        while let Some(nl) = buf[..stop].iter().rposition(|b| *b == NL) {
            let line = &buf[nl + 1..stop];
            if !line.is_empty() {
                if let Ok(s) = std::str::from_utf8(line) {
                    if !f(s) {
                        return Ok(());
                    }
                }
            }
            stop = nl;
        }
        tail = buf[..stop].to_vec();
        end = start;
    }
    if !tail.is_empty() {
        if let Ok(s) = std::str::from_utf8(&tail) {
            f(s);
        }
    }
    Ok(())
}

/// The Log of the task `ids` names (the task, then the ones merged into
/// it): the journal from its end, the land records, the artifacts' and
/// pages' versions made under it (`state`: the hub's state folder).
pub(crate) fn read_log(journal: &Path, lands: &Path, state: &Path, ids: &[String], limit: usize) -> Result<Vec<bise_proto::rows::LogLine>, String> {
    let mut scan = crate::task_log::Scan::new(ids);
    rev_lines(journal, |l| scan.line(l)).map_err(|e| e.to_string())?;
    Ok(scan.log(&crate::land_record::read(lands), &arts_of(state, ids), limit))
}

/// The artifacts and pages one of whose versions (or, before the stamp,
/// whose Meta) names one of `ids`, as the Log reads them.
pub(crate) fn arts_of(state: &Path, ids: &[String]) -> Vec<crate::task_log::ArtLog> {
    use crate::task_log::ArtLog;
    let names = |ws: &[String]| ws.iter().any(|w| ids.contains(w));
    let mut out = Vec::new();
    for m in crate::artifacts::Store::new(state).stored() {
        let versions: Vec<_> = m.versions.iter().map(|v| (v.v, v.at_ms, v.turn.as_ref().map_or_else(|| m.agent.clone(), |t| t.agent.clone()), v.works.clone())).collect();
        if names(m.works.works()) || versions.iter().any(|(_, _, _, w)| w.as_deref().is_some_and(names)) {
            out.push(ArtLog { art: m.id.clone(), title: m.title.clone(), page: false, works: m.works.works().to_vec(), versions });
        }
    }
    for m in crate::pages::store::Store::new(state).list() {
        let versions: Vec<_> = m.versions.iter().map(|v| (v.n, v.at_ms, m.agent.clone(), v.works.clone())).collect();
        if names(m.works.works()) || versions.iter().any(|(_, _, _, w)| w.as_deref().is_some_and(names)) {
            out.push(ArtLog { art: m.id.clone(), title: m.title.clone(), page: true, works: m.works.works().to_vec(), versions });
        }
    }
    out
}

/// The ids a task's Log reads: the live task an id names (a merged one:
/// the task it went into), then every work merged into it. Err: no such
/// task. Pure over the view's works.
pub(crate) fn ids_of(st: &crate::model::State, id: &str) -> Result<(String, Vec<String>), String> {
    let mut id = id.to_string();
    for _ in 0..=st.works.len() {
        match st.works.get(&id).and_then(|w| w.work.merged_into.clone()) {
            Some(into) => id = into,
            None => break,
        }
    }
    if !st.works.contains_key(&id) && !st.tombs.contains_key(&id) {
        return Err(format!("no task {} (sb task list --all)", id));
    }
    let mut ids = vec![id.clone()];
    // merged into it, at any depth
    let mut i = 0;
    while i < ids.len() {
        let into = ids[i].clone();
        ids.extend(st.works.values().filter(|w| w.work.merged_into.as_deref() == Some(into.as_str())).map(|w| w.work.id.clone()));
        ids.extend(st.tombs.values().filter(|t| t.merged_into.as_deref() == Some(into.as_str())).map(|t| t.id.clone()));
        i += 1;
        if ids.len() > st.works.len() + st.tombs.len() + 1 {
            break;
        }
    }
    Ok((id, ids))
}

impl Shell {
    /// `sb task show <id> --log`: the same lines in words, read on a
    /// thread, written to the asker's stream.
    pub(super) fn task_log_text(&mut self, mut stream: std::os::unix::net::UnixStream, task: String) {
        let (journal, lands, state) = (self.opts.paths.journal(), self.opts.paths.lands(), self.opts.paths.state.clone());
        let found = ids_of(&self.hub.st, &task);
        std::thread::spawn(move || {
            let now = crate::util::now_ms();
            let body = match found.and_then(|(live, ids)| read_log(&journal, &lands, &state, &ids, crate::task_log::LIMIT).map(|l| (live, l))) {
                Ok((live, lines)) if lines.is_empty() => serde_json::json!({"ok": true, "text": format!("{}: {}", live, bise_proto::words::tasks::log_none)}),
                Ok((live, lines)) => {
                    let mut out = vec![bise_proto::words::tasks::log_head(&live)];
                    out.extend(crate::task_log::words(&lines, |t| crate::every::past_label(t, now)));
                    serde_json::json!({"ok": true, "text": out.join(&char::from(NL).to_string())})
                }
                Err(e) => serde_json::json!({"ok": false, "error": e}),
            };
            super::write_json(&mut stream, &body);
        });
    }

    /// `task_log` for client `id`: read on a thread, answered with `req`.
    pub(super) fn task_log(&mut self, id: ClientId, task: String, req: u64, limit: u64) {
        let (tx, project) = (self.tx.clone(), self.project());
        let (journal, lands, state) = (self.opts.paths.journal(), self.opts.paths.lands(), self.opts.paths.state.clone());
        let limit = if limit == 0 { crate::task_log::LIMIT } else { limit as usize };
        let found = ids_of(&self.hub.st, &task);
        std::thread::spawn(move || {
            let ev = match found.and_then(|(live, ids)| read_log(&journal, &lands, &state, &ids, limit).map(|l| (live, l))) {
                Ok((live, lines)) => HubEv::TaskLog { project, req, id: live, lines, error: None },
                Err(e) => HubEv::TaskLog { project, req, id: task, lines: Vec::new(), error: Some(e) },
            };
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: ev.to_value() });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_journal_is_read_newest_first_across_chunks() {
        let dir = std::env::temp_dir().join(format!("task-log-rev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("j.jsonl");
        // lines longer than nothing in particular, enough to cross a chunk
        let lines: Vec<String> = (0..3000).map(|i| serde_json::json!({"n": i, "pad": "x".repeat(700)}).to_string()).collect();
        let nl = char::from(NL).to_string();
        std::fs::write(&p, lines.join(&nl) + &nl).unwrap();
        let mut got = Vec::new();
        rev_lines(&p, |l| {
            got.push(l.to_string());
            true
        })
        .unwrap();
        let want: Vec<String> = lines.iter().rev().cloned().collect();
        assert_eq!(got, want);
        // it stops when asked
        let mut n = 0;
        rev_lines(&p, |_| {
            n += 1;
            n < 5
        })
        .unwrap();
        assert_eq!(n, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
