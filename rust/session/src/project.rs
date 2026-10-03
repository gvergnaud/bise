//! Projection to the Core's `BEND-SESSION 2` text (§7 step 7): what the
//! REPL loads with BEND_CONTINUE. It mirrors core/checkpoint.bend's
//! to_text byte for byte (escape_nl for texts, wire_encode for call args
//! and the queue).
use crate::blob;
use crate::reader::Log;
use crate::state::State;
use crate::types::*;
use std::path::Path;

/// core/wire.bend escape_nl: LF → backslash-n; nothing else changes.
pub fn escape_nl(s: &str) -> String {
    s.replace('\n', "\\n")
}

/// core/wire.bend unescape_nl: backslash-n → LF; any other backslash stays.
pub fn unescape_nl(s: &str) -> String {
    // a scan, like the Bend one: a backslash takes the next character
    // with it, so "\\\\n" stays as it is
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('n') => o.push('\n'),
            Some(c2) => {
                o.push('\\');
                o.push(c2);
            }
            None => o.push('\\'),
        }
    }
    o
}

/// core/wire.bend wire_encode: backslash doubled, LF → \N, CR → \R.
pub fn wire_encode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\N"),
            '\r' => o.push_str("\\R"),
            c => o.push(c),
        }
    }
    o
}

/// core/wire.bend wire_decode: the inverse of wire_encode; a backslash
/// before anything else is kept as it is (legacy content).
pub fn wire_decode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.peek() {
            Some('\\') => {
                it.next();
                o.push('\\');
            }
            Some('N') => {
                it.next();
                o.push('\n');
            }
            Some('R') => {
                it.next();
                o.push('\r');
            }
            _ => o.push('\\'),
        }
    }
    o
}

fn blob_text(blobs: &Path, r: &BlobRef) -> Result<String, String> {
    let b = blob::get(blobs, r).map_err(|e| format!("blob {}: {e}", r.sha256))?;
    String::from_utf8(b).map_err(|_| format!("blob {} is not text", r.sha256))
}

pub fn image_marker(name: &str, path: &str, mime: &str, b64: &str) -> String {
    format!("<image name=\"{name}\" path=\"{path}\" mime=\"{mime}\" b64=\"{b64}\">")
}

/// The Core's text of some parts: their concatenation (spec: Part).
pub fn parts_text(parts: &[Part], blobs: &Path) -> Result<String, String> {
    let mut o = String::new();
    for p in parts {
        match p {
            Part::Text { text } => o.push_str(text),
            Part::TextBlob { blob } => o.push_str(&blob_text(blobs, blob)?),
            Part::Image { image, name, path, b64 } => o.push_str(&image_marker(name, path, &image.mime, b64)),
            Part::Thinking { text, signature, .. } => {
                o.push_str("<think>");
                o.push_str(text);
                // a signature a redactor rewrote (a log written before
                // redact::opaque_key) no longer checks: the span goes
                // unsigned and the Core drops it from the request (the
                // model thinks again) instead of a 400 on every turn
                let sig = signature.as_deref().filter(|s| !s.is_empty());
                if sig.is_some_and(crate::redact::has_marker) {
                    eprintln!("session projection: a thinking signature carries a redaction marker: the block goes unsigned");
                }
                if let Some(sig) = sig.filter(|s| !crate::redact::has_marker(s)) {
                    o.push_str("\nBENDSIG::");
                    o.push_str(sig);
                }
                o.push_str("</think>");
            }
            Part::File { .. } | Part::RedactedThinking { .. } | Part::Other => {
                return Err("a part the Core cannot hold (file, redacted thinking or unknown)".into())
            }
        }
    }
    Ok(o)
}

/// "call_12" → 12 (the Core's numeric id); anything else → 0, like the
/// Core's reader of a CALL line.
pub fn call_num(id: &str) -> u32 {
    id.strip_prefix("call_").and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn system_text(t: &Text, blobs: &Path) -> Result<String, String> {
    match t {
        Text::Inline { text } => Ok(text.clone()),
        Text::Blob { blob } => blob_text(blobs, blob),
    }
}

/// One MSG line (with its CALL lines) of a context event.
fn msg(p: &Payload, blobs: &Path) -> Result<String, String> {
    let flag = |b: bool| if b { "True" } else { "False" };
    let (inj, role, text, calls): (bool, &str, String, &[ToolCall]) = match p {
        Payload::UserMessage(m) => (m.injected.unwrap_or(false), "user", parts_text(&m.content, blobs)?, &[]),
        Payload::ContextInjected(m) if m.role.as_deref() == Some("system") => (false, "system", parts_text(&m.content, blobs)?, &[]),
        Payload::ContextInjected(m) => (m.injected.unwrap_or(true), "user", parts_text(&m.content, blobs)?, &[]),
        Payload::AgentMessage(m) => (m.injected.unwrap_or(true), "user", parts_text(&m.content, blobs)?, &[]),
        Payload::AssistantMessage(m) => (false, "assistant", parts_text(&m.parts, blobs)?, &m.calls),
        Payload::ToolResult(m) => (false, "tool", parts_text(&m.content, blobs)?, &[]),
        Payload::CompactionDone(m) => (true, "user", parts_text(&m.summary, blobs)?, &[]),
        _ => return Err("not a context event".into()),
    };
    let mut o = format!("MSG {} {role} : {}", flag(inj), escape_nl(&text));
    for c in calls {
        o.push_str(&format!("\n  CALL {} {} : {}", call_num(&c.id), c.name, wire_encode(&c.args)));
    }
    o.push('\n');
    Ok(o)
}

/// The BEND-SESSION 2 text of a state. Err names what cannot be
/// projected (a missing blob, a context seq not in the log).
pub fn project(log: &Log, st: &State, blobs: &Path) -> Result<String, String> {
    let mut o = String::from("BEND-SESSION 2\n");
    for t in &st.tools {
        o.push_str(&format!("TOOL {} : {}\n", t.name, t.description));
    }
    let l = &st.limits;
    o.push_str(&format!(
        "CFG {} {} {} {}\n",
        l.compact_threshold.unwrap_or(0),
        l.select_budget.unwrap_or(0),
        l.max_nulls.unwrap_or(0),
        escape_nl(&system_text(&st.system, blobs)?)
    ));
    o.push_str(&format!("COUNT {} {}\n", st.counters.inputs, st.counters.actions));
    let mut notifs = String::new();
    for &q in &st.queue {
        let e = log.by_seq(q).ok_or(format!("queued seq {q} not in the log"))?;
        let Some(Payload::InputQueued(iq)) = &e.payload else {
            return Err(format!("queued seq {q} is not input_queued"));
        };
        let t = wire_encode(&parts_text(&iq.content, blobs)?);
        match iq.kind {
            QueuedKind::Notification => notifs.push_str(&format!("NOTIF {t}\n")),
            _ => o.push_str(&format!("QUEUE {t}\n")),
        }
    }
    o.push_str(&notifs);
    // the last resort of BISE-242 (resume and the recorder already wrote
    // a result for every cut call): a call with no result gets a failed
    // one before the next message, a result no call waits for is left
    // out, so the REPL never loads an unpaired history
    let mut owed: Vec<(String, String)> = Vec::new();
    let close = |owed: &mut Vec<(String, String)>, o: &mut String| {
        for (c, name) in owed.drain(..) {
            eprintln!("session projection: call {c} has no result: a failed one is projected");
            o.push_str(&format!("MSG False tool : {}\n", escape_nl(&crate::pairing::no_result_text(&name))));
        }
    };
    for &s in &st.context {
        let e = log.by_seq(s).ok_or(format!("context seq {s} not in the log"))?;
        let p = e.payload.as_ref().ok_or(format!("context seq {s} is not readable"))?;
        match p {
            Payload::ToolResult(r) if !owed.iter().any(|(c, _)| *c == r.call) => {
                eprintln!("session projection: seq {s}: result of {} with no call waiting: left out", r.call);
                continue;
            }
            Payload::ToolResult(r) => owed.retain(|(c, _)| *c != r.call),
            _ => close(&mut owed, &mut o),
        }
        if let Payload::AssistantMessage(m) = p {
            owed = m.calls.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
        }
        o.push_str(&msg(p, blobs).map_err(|w| format!("seq {s}: {w}"))?);
    }
    close(&mut owed, &mut o);
    Ok(o)
}

/// Write the base64 file of every image part of the context whose file
/// is missing (the Core reads the image from the marker's b64 path).
pub fn materialize_images(log: &Log, st: &State, blobs: &Path) -> Vec<String> {
    let mut errs = Vec::new();
    for &s in &st.context {
        let Some(e) = log.by_seq(s) else { continue };
        let parts: &[Part] = match &e.payload {
            Some(Payload::UserMessage(m)) => &m.content,
            Some(Payload::ContextInjected(m)) => &m.content,
            Some(Payload::AgentMessage(m)) => &m.content,
            Some(Payload::ToolResult(m)) => &m.content,
            _ => continue,
        };
        for p in parts {
            if let Part::Image { image, b64, .. } = p {
                let path = Path::new(b64);
                if path.is_absolute() && !path.exists() {
                    let r = blob::get(blobs, image).and_then(|b| {
                        if let Some(d) = path.parent() {
                            std::fs::create_dir_all(d)?;
                        }
                        std::fs::write(path, base64(&b))
                    });
                    if let Err(e) = r {
                        errs.push(format!("{b64}: {e}"));
                    }
                }
            }
        }
    }
    errs
}

pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                o.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}
