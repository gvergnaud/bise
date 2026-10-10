//! The hub's side of the fn context (S9, architect m_9048): an input op's
//! `context` (what was on his screen at fn) framed after his words by the
//! one render (`crate::fn_context`), and his 'you' line written as his
//! words plus an `sb context : <json>` line in one append (feed).

use super::*;

/// What an attached path is, for `crate::attached::render`: an image put in
/// the image store (its marker), another file, or left out (logged).
fn look_file(path: &str) -> crate::attached::Look {
    use crate::attached::Look;
    let p = std::path::Path::new(path);
    if !p.is_absolute() || !p.is_file() {
        eprintln!("sbd: attached file left out (not an absolute path to a file): {path}");
        return Look::Skip;
    }
    match bend_images::store_file(p) {
        Ok(stored) => {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            Look::Image(bend_images::marker(&name, path, &stored))
        }
        Err(_) => Look::File,
    }
}

/// His words with the input op's attached files rendered after them (item
/// H), once, before routing: a routed send carries the same text to the
/// other project (image-store markers are global to every hub). Never on a
/// slash command.
pub(super) fn with_files(v: &Value, text: String) -> String {
    match v.get("files").and_then(Value::as_array).filter(|_| !text.trim_start().starts_with('/')) {
        Some(files) => {
            let files: Vec<String> = files.iter().filter_map(Value::as_str).map(str::to_string).collect();
            render_files(&text, &files)
        }
        None => text,
    }
}

/// `text` with `files` rendered after it by the one render
/// (`crate::attached::render` with the image store): a send's input
/// ([`with_files`]) and a card's answer (`HubCmd::Answer.files`, R41).
pub(super) fn render_files(text: &str, files: &[String]) -> String {
    crate::attached::render(text, files, &mut look_file)
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
        self.step(Input::ClientInput { client, focus, text, queued });
        self.fn_ctx = None;
    }

    /// The lines a feed line becomes: the 'you' line of the input being
    /// stepped with a fn context as two lines, his words (the rendered
    /// block cut off, the model got it) and `sb context : <json>` (the
    /// window's thread reads it, the TUI skips it); any other line, itself.
    pub(super) fn fn_ctx_lines(&mut self, name: &str, line: &str) -> Vec<String> {
        let Some((_, json)) = self.fn_ctx.as_ref().filter(|(a, _)| a == name) else { return vec![line.to_string()] };
        let Some(said) = line.strip_prefix("sb you : ") else { return vec![line.to_string()] };
        // no block: a context with a shot only (the core's marker is in
        // his words), or a line sb-core cut before it
        let words = crate::fn_context::strip_block(said).unwrap_or(said);
        let lines = vec![format!("sb you : {words}"), format!("sb context : {json}")];
        self.fn_ctx = None;
        lines
    }
}
