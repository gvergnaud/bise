//! A tool row's whole output for the window (`HubCmd::ToolOut`;
//! ambient-lead m_14200, architect m_14218/m_14228). The transcript keeps
//! the runtime's 200-char preview of a result; the agent's session log
//! keeps the result itself. Joined in `bise_session::tool_output` (call,
//! ok, time); the preview confirms the match and is the answer when the
//! log has no such result (an older REPL's session, no log).

use bise_proto::thread::lines::{self, Rec};
use bise_proto::thread::{cap, TOOL_TEXT_CAP};
use bise_session::tool_output::{tool_output, Query};
use std::path::Path;

/// Transcript lines read after the call for its result.
const AFTER: usize = 2000;

/// Where the logs are: the session logs and their blobs.
pub(super) struct Logs<'a> {
    pub sessions: &'a Path,
    pub blobs: &'a Path,
}

/// The output of the tool call at line `pos` of `transcript` (the agent's
/// folder `adir` names its session): `(out, cut, total)` (total: the whole
/// output's bytes, known only when the log answered), or why there is none.
pub(super) fn answer(transcript: &Path, adir: &Path, pos: u64, logs: &Logs) -> Result<(String, bool, Option<u64>), String> {
    let page = super::super::history::transcript_page(transcript, pos as usize + AFTER + 1, AFTER + 1);
    let mut after = page.into_iter().skip_while(|(p, _, _)| (*p as u64) < pos);
    let call = after.next().filter(|(p, _, _)| *p as u64 == pos).map(|(_, _, l)| l);
    let Some(Rec::Tool { id: n, name, .. }) = call.as_deref().map(lines::read) else {
        return Err(format!("line {pos} is no tool call"));
    };
    let found = after.find_map(|(_, ts, l)| match lines::read(&l) {
        Rec::ToolResult { id, ok, preview } if id == n => Some((ts, ok, preview)),
        _ => None,
    });
    let Some((ts, ok, preview)) = found else { return Err("the call has no result yet".into()) };
    let preview = lines::wire_decode(&preview);
    let session = super::super::session_log::session_of(adir).unwrap_or_default();
    let q = Query { session: &session, n: u64::from(n), name: &name, ok, at_ms: ts.unwrap_or(0) };
    Ok(match tool_output(logs.sessions, logs.blobs, &q, TOOL_TEXT_CAP, &|full| starts_alike(full, &preview)) {
        Some(o) => (o.text, o.cut, Some(o.total as u64)),
        None => (cap(&preview), true, None),
    })
}

/// The preview is the result's start (the runtime's preview is the
/// output flattened to one line and cut at 200 chars): whitespace aside.
pub(super) fn starts_alike(full: &str, preview: &str) -> bool {
    let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let (full, preview) = (words(full), words(preview.trim_end_matches('…')));
    // the preview's last word may be cut
    let head = preview.rsplit_once(' ').map_or("", |(h, _)| h);
    full.starts_with(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_is_its_results_start_whitespace_aside() {
        let full = "1\n2\n3\n  running 40 tests\nok";
        assert!(starts_alike(full, "1 2 3 running 40 te"));
        assert!(starts_alike(full, ""));
        assert!(!starts_alike(full, "2 3 running"));
        assert!(!starts_alike("first run: 40 passed", "second run: 39 pass"));
    }
}
