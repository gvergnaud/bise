//! The hub's side of the fn context (S9, architect m_9048): an input op's
//! `context` (what was on his screen at fn) framed after his words by the
//! one render (`crate::fn_context`), and his 'you' line written as his
//! words plus an `sb context : <json>` line in one append (feed).

use super::*;

/// Whether a file part's path is a file now (`crate::parts_in::said`).
pub(super) fn file_exists(p: &std::path::Path) -> bool {
    p.is_file()
}

/// An image part sent with its source only, put in the image store
/// (`bend_images::store_file`: one FNV
/// name, one downscale), for `crate::parts_in::said`.
pub(super) fn store_image(p: &std::path::Path) -> Result<crate::parts_in::Stored, String> {
    let stored = bend_images::store_file(p)?;
    Ok(crate::parts_in::Stored { b64: stored.b64.to_string_lossy().into_owned(), mime: stored.kind.mime().into() })
}

/// A routed image whose source is gone, put in this hub's store from the
/// sender's `.b64` (its base64 text decoded, then `store_bytes`: the same
/// FNV name and downscale), for `crate::parts_in::restored` (R41 (6)).
pub(super) fn store_b64(p: &std::path::Path) -> Result<crate::parts_in::Stored, String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let bytes = bend_images::base64_decode(text.trim()).ok_or_else(|| format!("{}: not base64", p.display()))?;
    let stored = bend_images::store_bytes(bytes)?;
    Ok(crate::parts_in::Stored { b64: stored.b64.to_string_lossy().into_owned(), mime: stored.kind.mime().into() })
}


impl Shell {
    /// His input stepped into sb-core, with its fn context when the op
    /// has one (never on a slash command): the block after his words for
    /// the model, the context kept for the 'you' line this step writes.
    /// `text` already has his attached files ([`with_files`]).
    pub(super) fn step_input(&mut self, client: ClientId, v: &Value, focus: String, text: String, queued: bool) {
        // computer use: a stopped or paused agent goes on first
        self.resume_computer_use(&focus, &text);
        let slash = text.trim_start().starts_with('/');
        let ctx = crate::fn_context::of(v.get("context")).filter(|_| !slash);
        let text = match &ctx {
            Some(c) => crate::fn_context::with_context(&text, c),
            None => text,
        };
        self.fn_ctx = ctx
            .filter(|c| !queued && *c != bise_proto::context::FnContext::default())
            .and_then(|c| serde_json::to_string(&c).ok())
            .map(|j| (self.hub.st.resolve(&focus).unwrap_or_else(|| MAIN.to_string()), j));
        // msg-parts: his parts' JSON as the op carries it (proto_input),
        // never on a slash command
        let parts = v.get("parts").and_then(Value::as_str).filter(|_| !slash).unwrap_or_default().to_string();
        // hub-tick: he talks to main: its own recycle waits (crate::recycle)
        if self.hub.st.resolve(&focus).unwrap_or_else(|| MAIN.to_string()) == MAIN {
            self.recycle.his_line_to_main(now_ms());
        }
        // at-start: the thread he typed it in, when not focus's (a trace there)
        let from = v.get("from").and_then(Value::as_str).filter(|_| !slash).unwrap_or_default().to_string();
        // his explicit tasks (SendOpts.works, a Task tab's '+ Agent'), never on a slash command
        let works = v.get("works").and_then(Value::as_array).filter(|_| !slash).map_or_else(Vec::new, |w| w.iter().filter_map(|x| x.as_str().map(String::from)).collect());
        self.step(Input::ClientInput { client, focus, text, queued, parts, from, works });
        self.fn_ctx = None;
    }

    /// The lines a feed line becomes: the 'you' line of the input being
    /// stepped with a fn context as two records, his words (the rendered
    /// block cut off, the model got it) and his `context` (the window's
    /// thread reads it, the TUI skips it); any other line, itself.
    pub(super) fn fn_ctx_lines(&mut self, name: &str, line: &str) -> Vec<String> {
        use bise_proto::core::{feed, Feed};
        use bise_proto::thread::lines::{read, Hub, Rec};
        let Some((_, json)) = self.fn_ctx.as_ref().filter(|(a, _)| a == name) else { return vec![line.to_string()] };
        let Rec::Hub(Hub::You(said, via)) = read(line) else { return vec![line.to_string()] };
        // no block: a context with a shot only (the core's marker is in
        // his words), or a line sb-core cut before it
        let words = crate::fn_context::strip_block(&said).unwrap_or(&said).to_string();
        let context = serde_json::from_str(json).unwrap_or_default();
        let lines = vec![Feed::You(feed::You { text: words, via }).line(), Feed::Context(feed::Context { context }).line()];
        self.fn_ctx = None;
        lines
    }
}
