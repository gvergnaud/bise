//! A tool call's whole output, read from the session log (the window's
//! tool row opened; ambient-lead m_14200, architect m_14218). The
//! transcript keeps only the runtime's one-line preview of a result
//! (`tool_result #N ok : <out>`, 200 flattened chars); the log keeps the
//! result itself (`tool_result {call: "call_<N>", content}`, a big text in
//! a blob).
//!
//! `call_<N>` is per REPL process (core/ev.bend `call_id`): a log can hold
//! the same id twice, and the same command can run twice with the same
//! preview (a test rerun, a polling loop). So the result is joined by time
//! first (call_<N>, the same ok/fail, the event nearest the transcript
//! line's time within a bound; the log event is written before the
//! annotation), and the caller's check (`same`: the preview is its start)
//! only confirms it; any miss is None (the caller shows the preview). The
//! cleaner fix, for the list (architect m_14228): the runtime writes a
//! unique call key (session id + N) in the `tool_result #N` annotation,
//! then no join is needed (a bend change).

use crate::reader::read_dir;
use crate::types::{Part, Payload};
use std::path::Path;

/// A result's text, cut to the caller's `max` bytes (`cut` then), and the
/// whole text's length in bytes (`total`, the unit of `max`: designer's
/// '4 KB of 18 KB shown', ambient-lead m_15631).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    pub text: String,
    pub cut: bool,
    pub total: usize,
}

/// `text` cut to `max` bytes on a char boundary.
pub fn cut_to(text: &str, max: usize) -> Out {
    let total = text.len();
    if total <= max {
        return Out { text: text.to_string(), cut: false, total };
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Out { text: text[..end].to_string(), cut: true, total }
}

/// The text of a result's parts: inline text and text blobs, in order
/// (an image or a file is named, never inlined).
fn text_of(content: &[Part], blobs: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    for p in content {
        match p {
            Part::Text { text } => out.push(text.clone()),
            Part::TextBlob { blob } => out.push(std::fs::read_to_string(crate::blob::path(blobs, &blob.sha256)).unwrap_or_default()),
            Part::Image { name, .. } => out.push(format!("[image {name}]")),
            Part::File { name, .. } => out.push(format!("[file {name}]")),
            _ => {}
        }
    }
    out.join("\n")
}

/// A logged result's output without the header the runtime puts before
/// it: the log keeps the message the model reads, `tool <name> ok: <out>`
/// or `tool <name> failed: <out>` (bend/core/session.bend
/// `tool_result_msg`; the runtime's replay strips it the same way,
/// main-pure.bend `replay.res`), while the transcript's preview is `<out>`
/// alone. A text without that header is returned whole.
pub fn result_body<'t>(text: &'t str, name: &str, ok: bool) -> &'t str {
    let head = format!("tool {name} {}: ", if ok { "ok" } else { "failed" });
    text.strip_prefix(head.as_str()).unwrap_or(text)
}

/// Which result: the transcript's `tool_result #n ok|fail` line of the
/// call `#n <name>`, at `at_ms`, in session `session`.
#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
    pub session: &'a str,
    pub n: u64,
    pub name: &'a str,
    pub ok: bool,
    pub at_ms: u64,
}

/// How far from the transcript line's time its result event may be.
pub const WITHIN_MS: u64 = 5_000;

/// The result `q` names (call_<n>, the same ok, the event nearest
/// `q.at_ms` within [`WITHIN_MS`]), if `same` accepts its whole text, cut
/// to `max` bytes; None when the log, the result or the check misses.
pub fn tool_output(sessions: &Path, blobs: &Path, q: &Query, max: usize, same: &dyn Fn(&str) -> bool) -> Option<Out> {
    if q.session.is_empty() || q.session.contains('/') || q.session.starts_with('.') {
        return None;
    }
    let log = read_dir(&sessions.join(q.session)).ok()?;
    let call = format!("call_{}", q.n);
    let r = log
        .events
        .iter()
        .filter_map(|e| match &e.payload {
            Some(Payload::ToolResult(r)) if r.call == call && r.ok == q.ok => Some((crate::writer::ms_of_iso(&e.at)?.abs_diff(q.at_ms), r)),
            _ => None,
        })
        .filter(|(d, _)| *d <= WITHIN_MS)
        .min_by_key(|(d, _)| *d)?
        .1;
    let text = text_of(&r.content, blobs);
    let body = result_body(&text, q.name, q.ok);
    same(body).then(|| cut_to(body, max))
}
