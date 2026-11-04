//! His message as typed parts (msg-parts, architect m_17621): what he
//! wrote, the selections he quoted, his long pastes, his images and his
//! files, apart. The one owner of the template the model reads them in.
//!
//! - [`Part`]: one piece. The clients send them (`HubCmd::Send.parts`,
//!   `HubCmd::Answer.parts`), sb-core keeps them on the message (opaque
//!   JSON), the fold puts them on his entries (`Entry.parts`), and every
//!   client draws his message from them: when an entry's `parts` is not
//!   empty, no client draws his message from its `text`.
//! - [`render`]: the parts as the text the model gets, byte for byte the
//!   tags the composers wrote before (the TUI's quote.rs / pasted.rs /
//!   the image store's marker, the desktop's message.ts):
//!
//!   ```text
//!   <selection from="main">
//!   the selected text
//!   </selection>
//!   what does <pasted n="1" lines="240">
//!   …
//!   </pasted> mean? <image name="[Image #2]" path="…" mime="image/png" b64="…">
//!   in [deck.pptx](/Users/ana/deck.pptx)
//!
//!   [files he attached:]
//!   /Users/ana/notes.md
//!   ```
//!
//!   the quotes in front (one per line), then the words with the pastes,
//!   the images and the file chips where they were (file-chips, architect
//!   m_21764: a file by its path; file-mention-wire, architect m_53247: as
//!   a markdown link with its name, ` (missing)` after it when the hub
//!   found none at send time, [`file_marker`]), then the list of his
//!   files that have no chip (a `files` list, an old journal). [`parse`]
//!   reads the old `<file name="[File #3]" path="…">` marker of stored
//!   history too, as the same part.
//! - [`chip_kind`]: the one table of what a path he attaches becomes (an
//!   image chip by [`IMAGE_EXTS`], which the window reads generated, else
//!   a file chip), an `@` pick and a drop alike.
//! - [`parse`]: an old text read back (an old journal, an older client
//!   that sends text only). Lossless: `render(&parse(t)) == t` for every
//!   text, because a tag is a part only when rendering that part again
//!   gives the same bytes; anything else stays text.
//!
//! Pure: no file system. Whether an image's `b64_file` may be read is the
//! hub's check (only inside its image store), never this module's.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One piece of his message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    /// his words (markdown, an artifact's `[title](artifact:id)` link
    /// included)
    Text { text: String },
    /// a selection he quoted: who wrote it (`main`, `you`, `t1 vs main`)
    /// and, from a diff, where (`file`, the lines `new` / `old`:
    /// `12-14`; empty: none). `n`: its chip's number (`❝ 1`, the one
    /// sequence of Paste.n, `[Image #n]`, `[File #n]`), and then it sits
    /// at its place in his words (his m_22231: "regarding ❝ 1 …"); none
    /// (an old journal, a client that puts quotes first): in front
    Quote {
        from: String,
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        file: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        new: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        old: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        n: Option<u32>,
    },
    /// a long paste, its chip number `n` (`▤ 1`) and its whole text
    Paste { n: u32, text: String },
    /// an image: its label (`[Image #2]`, or its file name), where he
    /// took it from (`source`, the chip's thumbnail), its mime and the
    /// image store's base64 file the model's marker points at (the hub
    /// accepts it only inside its store). `caption`: a fact about the
    /// image, never his words (the talk's still: `Safari · cold bench`,
    /// architect m_22000): in the marker and the model's frame, drawn
    /// under the chip; "" (every pasted image) leaves the marker as before
    Image {
        name: String,
        source: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        mime: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        b64_file: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        caption: String,
    },
    /// a file he attached by its absolute path (file-chips, architect
    /// m_21764): the model reads it with its own tools, never a copy.
    /// `name`: its chip's label (`[File #2]`, the images' number
    /// sequence), rendered in place as its marker; "" (an old journal, a
    /// `files` list): in the list after his words. `missing`: the hub
    /// found no file there when he sent it.
    File {
        path: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        name: String,
        #[serde(default, skip_serializing_if = "is_false")]
        missing: bool,
    },
    /// what he did on a page (page-answer-row, architect m_20833): his
    /// notes, picks, ticks or starts on page `page`, as the hub sends
    /// them to an agent. `text` is the model's text whole (the hub's
    /// template, its hint included), so it renders as itself; the clients
    /// draw his words from `notes`, never from `text`; `to`: the agent it
    /// went to (the page's, main when it is gone). Only the hub makes one
    /// ([`parse`] never does), one per message.
    Page {
        text: String,
        to: String,
        page: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        title: String,
        #[serde(default)]
        version: u64,
        #[serde(default)]
        notes: Vec<PageNote>,
    },
    /// a part of an artifact he selected in its viewer (office-artifacts
    /// plan §9b, architect m_23874): version `version` (and `rev`, the
    /// save he looked at; 0: not said) of artifact `artifact`, where
    /// ([`crate::anchor::Anchor`]: the agent passes it to the office SDK's
    /// `at()`), and what he saw selected (`quote`, "" none). `n`: its chip's
    /// number, placed like a [`Part::Quote`]'s (None: in front). The door
    /// (`parts_in::said`) checks the artifact and the version, then records
    /// it as a sent note of that artifact.
    Anchor {
        artifact: String,
        version: u64,
        #[serde(default, skip_serializing_if = "is_zero32")]
        rev: u32,
        anchor: crate::anchor::Anchor,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        n: Option<u32>,
    },
    /// a line the hub adds after his words for the model (ambient-lead
    /// m_22706, architect m_22715): `text` whole, its separator included,
    /// rendered after everything else; never drawn in his bubble. Only
    /// the hub makes one (`parts_in::said` refuses it from a client);
    /// `of`: which one, so a client reads 'from fn' from it, never from
    /// its words ([`via_fn`])
    Hint {
        #[serde(default)]
        of: HintKind,
        text: String,
    },
    /// a message forwarded whole (msg-forward, architect m_41547): message
    /// `id`, `from` wrote it to `to` (`user`: him), at `at` (the shell's
    /// UTC time, "" not said), and `by` forwards it. It renders to the
    /// header line the model reads, and what comes after it renders as
    /// the original message of its own ([`render`]); the clients draw it
    /// as one line and the parts after it as the original, quoted. Only
    /// sb-core makes one (bend/hub/forward.bend; `parts_in::said` refuses
    /// it from a client, [`parse`] never makes one).
    Forwarded {
        id: u64,
        from: String,
        to: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        at: String,
        by: String,
    },
    /// mcp-apps (docs/mcp-apps.md, C5; architect m_49840): what a view of
    /// `plugin` (its resource `uri`) last said of itself with
    /// `ui/update-model-context`, joined to his next send to that view's
    /// agent once. Only the hub makes one (`parts_in::said` refuses it
    /// from a client); rendered before his words, never drawn as them
    ViewContext { plugin: String, uri: String, text: String },
    /// a part a newer client sent: kept whole, never rendered
    #[serde(untagged)]
    #[cfg_attr(feature = "ts", ts(skip))]
    Unknown(Value),
}

/// Which line the hub added ([`Part::Hint`]): the capsule's (he typed it
/// with fn space: answer in one short line), the capsule's start (fn space
/// + tab: start an agent), his open steps (a capsule message).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum HintKind {
    Capsule,
    CapsuleStart,
    Steps,
    /// his notes on an artifact: how the agent answers them
    Notes,
    #[default]
    #[serde(other)]
    Unknown,
}

/// The capsule's line after his words (main's rule reads it: prompts.rs).
pub const CAPSULE_HINT: &str = "\n\n[from the capsule: answer in one short line]";
/// The same for fn space + tab: main starts an agent on it.
pub const CAPSULE_START_HINT: &str = "\n\n[from the capsule: start an agent; answer in one short line]";
/// The line after his notes on an artifact (a [`Part::Anchor`] in his
/// message): what answers them (art-notes; designer signs the words).
pub const NOTES_HINT: &str = "\n\n[notes on an artifact: answer with its next version (bise office run, or sb artifact add <path>): that marks them done. a note you won't do: say why]";

/// The notes hint when `parts` carry a part of an artifact; none
/// otherwise.
pub fn notes_hint(parts: &[Part]) -> Option<Part> {
    parts.iter().any(|p| matches!(p, Part::Anchor { .. })).then(|| Part::Hint { of: HintKind::Notes, text: NOTES_HINT.into() })
}

const STEPS_HEAD: &str = "\n[his open steps: ";
const STEPS_TAIL: &str = "; when he says one is done: sb page tick <page> <item>]";

/// The capsule's hint for an input sent `via` it (`capsule`,
/// `capsule-start`); none for any other client.
pub fn capsule_hint(via: &str) -> Option<Part> {
    match via {
        "capsule" => Some(Part::Hint { of: HintKind::Capsule, text: CAPSULE_HINT.into() }),
        "capsule-start" => Some(Part::Hint { of: HintKind::CapsuleStart, text: CAPSULE_START_HINT.into() }),
        _ => None,
    }
}

/// His open steps as one hint (`page item "text"` each, roadmap D); none
/// without any.
pub fn steps_hint(steps: &[String]) -> Option<Part> {
    match steps.is_empty() {
        true => None,
        false => Some(Part::Hint { of: HintKind::Steps, text: format!("{STEPS_HEAD}{}{STEPS_TAIL}", steps.join(", ")) }),
    }
}

/// His message came by fn space, typed (designer m_22714: 'from fn' on
/// his bubble): a capsule hint in it, and not said (a fn talk keeps its
/// 'said'). The one rule both apps follow (fixture via_fn.json).
pub fn via_fn(parts: &[Part], said: bool) -> bool {
    !said && parts.iter().any(|p| matches!(p, Part::Hint { of: HintKind::Capsule | HintKind::CapsuleStart, .. }))
}

/// The hints at the end of `text` (the steps line, the capsule's), each
/// exactly as the hub writes it: the text before them and the hints in
/// order. The same words mid-message stay his.
fn split_hints(text: &str) -> (&str, Vec<Part>) {
    let mut rest = text;
    let mut out = Vec::new();
    loop {
        if let Some(r) = rest.strip_suffix(CAPSULE_HINT) {
            out.push(Part::Hint { of: HintKind::Capsule, text: CAPSULE_HINT.into() });
            rest = r;
        } else if let Some(r) = rest.strip_suffix(CAPSULE_START_HINT) {
            out.push(Part::Hint { of: HintKind::CapsuleStart, text: CAPSULE_START_HINT.into() });
            rest = r;
        } else if let Some(i) = rest.strip_suffix(STEPS_TAIL).and_then(|r| r.rfind(STEPS_HEAD)).filter(|&i| !rest[i + 1..].contains('\n')) {
            out.push(Part::Hint { of: HintKind::Steps, text: rest[i..].to_string() });
            rest = &rest[..i];
        } else {
            break;
        }
    }
    out.reverse();
    (rest, out)
}

/// One thing he did on a page ([`Part::Page`]): on block `block`, a
/// `kind` (`pick`, `note`, `voice`, `tick`, `edit`, `keep`, `drop`,
/// `unsure`, `approve`, `skip`, `start`), his words (`text`: the option
/// he picked, his note; an edit's new words), what it is on (`quote`: the
/// words he selected, an edit's old words; "" none) and, when it answers
/// a question that had a card, that card and its question (`card` 0 and
/// `asked` "" none). `parts` (card-image-parts, architect m_35268): his
/// answer as he typed it (his words, images and files, as `Send.parts`),
/// none for a plain one; `text` is then his words alone, never a marker.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PageNote {
    pub block: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quote: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub card: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub asked: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<Part>>"))]
    pub parts: Vec<Part>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

fn is_zero32(n: &u32) -> bool {
    *n == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ---- which paths become a chip ----

/// What a path he attaches becomes (architect m_21764 B: the one table
/// both composers read; the TS gets it as `chips.gen.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipKind {
    /// an image chip (`[Image #N]`)
    Image,
    /// a file chip (`[File #N]`)
    File,
}

/// The marks that take no space after a chip: '❝ 1, which?', never
/// '❝ 1 , which?' (architect m_22426). A chip followed by a word gets one
/// space; followed by one of these, none. Both composers read it (the TUI's
/// attach::expand, the window's buildMessage through `chips.gen.ts`), so
/// one typed message gives the same parts from both.
pub const NO_SPACE_BEFORE: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']'];

/// The extensions of an image chip (bend_images' `has_image_ext`).
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// A path's extension, lower case ("" none).
fn ext_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// What a file he attaches becomes (an `@` pick, a drop, a paste, the
/// '+'): an image chip by its extension, else a file chip, code too
/// (at-chips: a picked `e2e.mjs` is a chip at the `@`, its path in the
/// attachment list, never its path as text).
pub fn chip_kind(path: &str) -> ChipKind {
    if IMAGE_EXTS.contains(&ext_of(path).as_str()) {
        ChipKind::Image
    } else {
        ChipKind::File
    }
}

/// A file chip's label: `[File #2]`.
pub fn file_label(n: usize) -> String {
    format!("[File #{n}]")
}

/// The last segment of a path (the chip's words): `deck.pptx`.
pub fn file_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

/// The most characters a file chip in the composer shows of its name
/// (designer m_27881; the window reads it generated).
pub const FILE_CHIP_CHARS: usize = 24;

/// A file chip's words in the composer (designer m_27881: its name, not
/// its number, which stays in the attachment list's row): the file's
/// name, cut at [`FILE_CHIP_CHARS`] with a glued `…`.
pub fn file_chip_name(path: &str) -> String {
    let name = file_name(path);
    if name.chars().count() <= FILE_CHIP_CHARS {
        return name.to_string();
    }
    let head: String = name.chars().take(FILE_CHIP_CHARS - 1).collect();
    format!("{head}…")
}

/// The page part of a message, when it has one (a message the hub wrote
/// from a page): who it went to, the page's title (its id when it has
/// none) and his notes.
pub fn page_of(parts: &[Part]) -> Option<PageSaid<'_>> {
    parts.iter().find_map(|p| match p {
        Part::Page { to, page, title, notes, .. } => Some(PageSaid { to, title: if title.is_empty() { page } else { title }, notes }),
        _ => None,
    })
}

/// [`page_of`]'s view of a page part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSaid<'a> {
    pub to: &'a str,
    pub title: &'a str,
    pub notes: &'a [PageNote],
}

/// The header of the list of his other files (the hub's
/// `attached::render` wrote it first).
pub const FILES_HEADER: &str = "[files he attached:]";

const QUOTE_OPEN: &str = "<selection from=\"";
const QUOTE_CLOSE: &str = "</selection>";
const PASTE_OPEN: &str = "<pasted n=\"";
const PASTE_CLOSE: &str = "</pasted>";
const IMAGE_OPEN: &str = "<image name=\"";
const FILE_OPEN: &str = "<file name=\"";

// ---- render ----

/// The text the model gets for `parts` (see the module's header). A
/// [`Part::Forwarded`] splits them: what comes before it, its header,
/// then what comes after it rendered as a message of its own (its quotes
/// in its own front), as sb-core writes the forward's text.
pub fn render(parts: &[Part]) -> String {
    if let Some(i) = parts.iter().position(|p| matches!(p, Part::Forwarded { .. })) {
        let Part::Forwarded { id, from, to, at, by } = &parts[i] else { unreachable!() };
        return format!("{}{}{}", render_one(&parts[..i]), forward_header(*id, from, to, at, by), render(&parts[i + 1..]));
    }
    render_one(parts)
}

/// The header line of a forward (with its newline), byte for byte
/// bend/hub/forward.bend's `header`: who wrote message `id` to whom,
/// when ("" left out), who forwards it, and where to open it.
pub fn forward_header(id: u64, from: &str, to: &str, at: &str, by: &str) -> String {
    let who = |n: &str| if n == "user" { "the user".to_string() } else { n.to_string() };
    let at = if at.is_empty() { String::new() } else { format!(" · {at}") };
    format!("[forwarded m_{id} · from {} to {}{at} · forwarded by {by} · its place: sb show m_{id}]\n", who(from), who(to))
}

/// [`render`] of parts with no [`Part::Forwarded`].
fn render_one(parts: &[Part]) -> String {
    let mut head: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut files: Vec<&str> = Vec::new();
    let mut hints = String::new();
    for p in parts {
        match p {
            // after everything, his files' list too: where the hub put it
            Part::Hint { text, .. } => hints.push_str(text),
            Part::Quote { from, text, file, new, old, n: None } => head.push(quote_tag(from, file, new, old, None, text)),
            Part::Quote { from, text, file, new, old, n: Some(n) } => body.push_str(&quote_tag(from, file, new, old, Some(*n), text)),
            Part::Text { text } => body.push_str(text),
            Part::Paste { n, text } => body.push_str(&paste_tag(*n, text)),
            Part::Image { name, source, mime, b64_file, caption } => body.push_str(&image_marker(name, source, mime, b64_file, caption)),
            Part::File { path, name, missing } if !name.is_empty() => body.push_str(&file_marker(path, *missing)),
            Part::File { path, .. } => files.push(path),
            Part::Page { text, .. } => body.push_str(text),
            Part::Anchor { artifact, version, rev, anchor, quote, n: None } => head.push(anchor_tag(artifact, *version, *rev, anchor, quote, None)),
            Part::Anchor { artifact, version, rev, anchor, quote, n: Some(n) } => body.push_str(&anchor_tag(artifact, *version, *rev, anchor, quote, Some(*n))),
            Part::ViewContext { plugin, uri, text } => head.push(view_context_tag(plugin, uri, text)),
            Part::Unknown(_) | Part::Forwarded { .. } => {}
        }
    }
    if !body.is_empty() {
        head.push(body);
    }
    let mut out = head.join("\n");
    if !files.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(FILES_HEADER);
        for f in files {
            out.push('\n');
            out.push_str(f);
        }
    }
    out.push_str(&hints);
    out
}

/// Lines in `text` (a last newline not counted; at least 1): a paste
/// tag's `lines`, a quote's `3 lines`.
pub fn line_count(text: &str) -> usize {
    text.trim_end_matches('\n').split('\n').count().max(1)
}

/// A quote's tag: `<selection from="t1 vs main" n="1" file="src/a.rs"
/// new="12-14" old="11">`, an attribute only when it has a value (`n`:
/// an inline quote, at its place); a `</selection>` inside the text is
/// broken ([`unclosed`]) so the tag ends where it should, a `"` or
/// newline in an attribute becomes `'` or a space. The header quote and
/// the inline one share this grammar: only `n` tells them apart.
pub fn quote_tag(from: &str, file: &str, new: &str, old: &str, n: Option<u32>, text: &str) -> String {
    let attr = |s: &str| s.replace('"', "'").replace('\n', " ");
    let mut head = format!("{QUOTE_OPEN}{}\"", attr(from));
    if let Some(n) = n {
        head.push_str(&format!(" n=\"{n}\""));
    }
    for (k, v) in [("file", file), ("new", new), ("old", old)] {
        if !v.is_empty() {
            head.push_str(&format!(" {k}=\"{}\"", attr(v)));
        }
    }
    let text = unclosed(text, QUOTE_CLOSE);
    format!("{head}>\n{text}\n{QUOTE_CLOSE}")
}

/// The tag of a part of an artifact he selected ([`Part::Anchor`]): the
/// anchor's JSON on its own line (the agent passes it to `at()` as is),
/// then what he saw selected, when anything:
///
/// ```text
/// <artifact-selection artifact="q3-xlsx" version="4" n="1">
/// {"kind":"cell","sheet":"Q3","range":"B4:D9"}
/// 1200 1350 1410
/// </artifact-selection>
/// ```
///
/// `rev` and `n` only when they have a value; the quote's own closing
/// tag is broken, like a selection's.
/// A view's context as the model reads it, before his words.
pub fn view_context_tag(plugin: &str, uri: &str, text: &str) -> String {
    format!("<view_context plugin=\"{}\" uri=\"{}\">\n{}\n</view_context>\n", marker_attr(plugin), marker_attr(uri), unclosed(text.trim_end_matches('\n'), "</view_context>"))
}

pub fn anchor_tag(artifact: &str, version: u64, rev: u32, anchor: &crate::anchor::Anchor, quote: &str, n: Option<u32>) -> String {
    let mut head = format!("{ANCHOR_OPEN}{}\" version=\"{version}\"", marker_attr(artifact));
    if rev > 0 {
        head.push_str(&format!(" rev=\"{rev}\""));
    }
    if let Some(n) = n {
        head.push_str(&format!(" n=\"{n}\""));
    }
    let json = serde_json::to_string(anchor).unwrap_or_else(|_| "{}".into());
    let quote = match quote.trim().is_empty() {
        true => String::new(),
        false => format!("{}\n", unclosed(quote.trim_end_matches('\n'), ANCHOR_CLOSE)),
    };
    format!("{head}>\n{json}\n{quote}{ANCHOR_CLOSE}")
}

const ANCHOR_OPEN: &str = "<artifact-selection artifact=\"";
const ANCHOR_CLOSE: &str = "</artifact-selection>";

/// A block's text with its closing tag broken (`</pasted>` becomes
/// `</pasted >`), so the block ends where its own closing tag is: the
/// one rule of the paste and quote blocks (architect m_22248).
fn unclosed(text: &str, close: &str) -> String {
    text.replace(close, &format!("{} >", &close[..close.len() - 1]))
}

/// A paste's tag: `<pasted n="1" lines="240">\n{text}\n</pasted>`; a
/// `</pasted>` inside the text is broken (`</pasted >`).
pub fn paste_tag(n: u32, text: &str) -> String {
    let lines = line_count(text);
    let text = unclosed(text, PASTE_CLOSE);
    format!("{PASTE_OPEN}{n}\" lines=\"{lines}\">\n{text}\n{PASTE_CLOSE}")
}

/// An image's marker, the image store's format (`bend_images::marker`):
/// a `"` becomes `'`, a `>` `)`, a newline a space. A caption comes last,
/// ` caption="…"`, only when there is one: an image without one has the
/// marker it always had, byte for byte.
pub fn image_marker(name: &str, source: &str, mime: &str, b64_file: &str, caption: &str) -> String {
    let caption = match caption.is_empty() {
        true => String::new(),
        false => format!("{CAPTION_ATTR}{}", marker_attr(caption)),
    };
    format!("{IMAGE_OPEN}{}\" path=\"{}\" mime=\"{}\" b64=\"{}{caption}\">", marker_attr(name), marker_attr(source), marker_attr(mime), marker_attr(b64_file))
}

/// The optional last attribute of an image marker (bend/core/image.bend
/// reads the same).
const CAPTION_ATTR: &str = "\" caption=\"";

/// A file chip's mention in place in his words, as the model reads it
/// (file-mention-wire, architect m_53247): model-facing text, so plain
/// markdown, a link with the file's name and its absolute path,
/// `[The Bot.md](/u/The Bot.md)`, ` (missing)` after it when the hub found
/// no file there. The name's `\`, `[` and `]` are backslash-escaped; the
/// path stays raw (spaces kept, the model reads it as-is), in the angle
/// form `[x](</u/a (1).md>)` only when it holds `(`, `)`, `<` or `>`; a
/// newline in either becomes a space. The chip's label (`[File #2]`) is
/// the clients' number, never in this text.
pub fn file_marker(path: &str, missing: bool) -> String {
    let gone = if missing { FILE_MISSING } else { "" };
    format!("[{}]({}){gone}", link_text(&file_mention_name(path)), link_dest(path))
}

/// What follows a file's link when the hub found no file there.
const FILE_MISSING: &str = " (missing)";

/// A file's name in its mention: its last segment, the path itself when
/// it has none (`/`).
fn file_mention_name(path: &str) -> String {
    match file_name(path) {
        "" => path.to_string(),
        n => n.to_string(),
    }
}

/// A link's text: `\`, `[`, `]` escaped, a newline a space.
fn link_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A link's destination: the path raw, in `<…>` (its `<`, `>` escaped)
/// when it holds a paren or an angle bracket; a newline a space.
fn link_dest(path: &str) -> String {
    let p: String = path.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
    match p.contains(['(', ')', '<', '>']) {
        false => p,
        true => format!("<{}>", p.replace('<', "\\<").replace('>', "\\>")),
    }
}

/// The marker a file chip had before file-mention-wire,
/// `<file name="[File #2]" path="/u/deck.pptx"[ missing="true"]>`: read
/// back from stored history forever ([`file_marker_at`]), never written.
fn old_file_marker(name: &str, path: &str, missing: bool) -> String {
    let gone = if missing { " missing=\"true\"" } else { "" };
    format!("{FILE_OPEN}{}\" path=\"{}\"{gone}>", marker_attr(name), marker_attr(path))
}

/// A marker's attribute: a `"` becomes `'`, a `>` `)`, a newline a space.
fn marker_attr(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => '\'',
            '>' => ')',
            '\n' | '\r' => ' ',
            c => c,
        })
        .collect()
}

// ---- parse ----

/// `text` read back into parts (see the module's header): the quotes in
/// front, the pastes and images in the words, the files' list at the
/// end. `render(&parse(t)) == t` for every `t`; a text with none of them
/// is one text part (none for an empty text).
pub fn parse(text: &str) -> Vec<Part> {
    // the hub's hints at the very end (an old journal's line, hidden too)
    let (body, hints) = split_hints(text);
    let mut out = parse_body(body);
    out.extend(hints);
    out
}

/// The parts of `text` when they render back to it (an old file marker
/// counts as its new form, [`words_parts`]); else the words alone; else
/// one text part.
fn parse_body(text: &str) -> Vec<Part> {
    let (full, norm) = parse_all(text);
    if render(&full) == norm {
        return full;
    }
    // the quotes or the list read a way that doesn't render back (a tag
    // that looks like one but is not the composer's): the words alone
    let (words, norm) = words_parts(text);
    if render(&words) == norm {
        return words;
    }
    text_part(text)
}

fn text_part(text: &str) -> Vec<Part> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![Part::Text { text: text.to_string() }]
    }
}

/// The quotes in front, the words, the files' list; and `text` with its
/// words as [`words_parts`] writes them back.
fn parse_all(text: &str) -> (Vec<Part>, String) {
    let (head, files) = split_files(text);
    let (mut out, body) = split_quotes(head);
    // body is the end of head, head the start of text
    let at = head.len() - body.len();
    let (words, norm_body) = words_parts(body);
    out.extend(words);
    out.extend(files.into_iter().map(|path| Part::File { path, name: String::new(), missing: false }));
    (out, format!("{}{norm_body}{}", &text[..at], &text[head.len()..]))
}

/// The files' list at the end of `text` (`FILES_HEADER` at the start or
/// after a blank line, then one absolute path per line): the text before
/// it and the paths. None: the text, no paths.
fn split_files(text: &str) -> (&str, Vec<String>) {
    let Some(i) = text.rfind(FILES_HEADER) else { return (text, Vec::new()) };
    let before = if i == 0 {
        ""
    } else {
        match text[..i].strip_suffix("\n\n") {
            Some(b) => b,
            None => return (text, Vec::new()),
        }
    };
    let rest = &text[i + FILES_HEADER.len()..];
    let Some(list) = rest.strip_prefix('\n') else { return (text, Vec::new()) };
    let paths: Vec<String> = list.split('\n').map(str::to_string).collect();
    if paths.iter().any(|p| !p.starts_with('/')) {
        return (text, Vec::new());
    }
    (before, paths)
}

/// The quote tags in front of `text`, one per line, and the words after
/// them (the newline after the last tag taken). A header quote has no
/// `n`; one with `n` is inline, read with the words.
fn split_quotes(text: &str) -> (Vec<Part>, &str) {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some((q, used)) = header_quote_at(rest) {
        out.push(q);
        rest = &rest[used..];
        match rest.strip_prefix('\n') {
            Some(r) if r.starts_with(QUOTE_OPEN) && header_quote_at(r).is_some() => rest = r,
            Some(r) if !r.is_empty() => return (out, r),
            _ => return (out, rest),
        }
    }
    (out, rest)
}

/// A header quote (no `n`) at the very start of `s`.
fn header_quote_at(s: &str) -> Option<(Part, usize)> {
    quote_at(s).filter(|(q, _)| matches!(q, Part::Quote { n: None, .. }))
}

/// An inline quote (with `n`) at the very start of `s`.
fn inline_quote_at(s: &str) -> Option<(Part, usize)> {
    quote_at(s).filter(|(q, _)| matches!(q, Part::Quote { n: Some(_), .. }))
}

/// The quote tag at the very start of `s`, when it renders back to the
/// same bytes: the part and the bytes it takes.
fn quote_at(s: &str) -> Option<(Part, usize)> {
    let rest = s.strip_prefix(QUOTE_OPEN)?;
    let (head, rest) = rest.split_once("\">\n")?;
    if head.contains('\n') {
        return None;
    }
    let (from, mut attrs) = head.split_once('"').unwrap_or((head, ""));
    let (mut file, mut new, mut old) = (String::new(), String::new(), String::new());
    let mut n: Option<u32> = None;
    while !attrs.is_empty() {
        let (k, v) = attrs.strip_prefix(' ')?.split_once("=\"")?;
        let (v, more) = v.split_once('"').unwrap_or((v, ""));
        match k {
            "n" if v.len() <= 9 && !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => n = Some(v.parse().ok()?),
            "file" => file = v.to_string(),
            "new" => new = v.to_string(),
            "old" => old = v.to_string(),
            _ => return None,
        }
        attrs = more;
    }
    let end = rest.find(&format!("\n{QUOTE_CLOSE}"))?;
    let text = &rest[..end];
    let tag = quote_tag(from, &file, &new, &old, n, text);
    if !s.starts_with(&tag) {
        return None;
    }
    let q = Part::Quote { from: from.to_string(), text: text.to_string(), file, new, old, n };
    Some((q, tag.len()))
}

/// `body` cut at its paste tags, image markers and file mentions, in
/// order (texts between them), and `body` as the renderer writes it
/// today: each old file marker in the new form ([`file_marker_at`]), the
/// rest byte for byte.
fn words_parts(body: &str) -> (Vec<Part>, String) {
    let mut out = Vec::new();
    let mut norm = String::with_capacity(body.len());
    let mut from = 0usize;
    let mut scan = 0usize;
    while let Some(at) = next_tag(body, scan) {
        match paste_at(&body[at..]).or_else(|| image_at(&body[at..])).or_else(|| file_marker_at(&body[at..])).or_else(|| inline_quote_at(&body[at..])) {
            Some((p, used)) => {
                if at > from {
                    out.push(Part::Text { text: body[from..at].to_string() });
                }
                norm.push_str(&body[from..at]);
                match &p {
                    Part::File { path, missing, .. } => norm.push_str(&file_marker(path, *missing)),
                    _ => norm.push_str(&body[at..at + used]),
                }
                out.push(p);
                from = at + used;
                scan = from;
            }
            None => scan = at + 1,
        }
    }
    if from < body.len() {
        out.push(Part::Text { text: body[from..].to_string() });
        norm.push_str(&body[from..]);
    }
    (out, norm)
}

/// The byte where the next paste tag, image marker, file mention (its
/// `[`, or the old `<file name="`) or inline quote may start.
fn next_tag(s: &str, from: usize) -> Option<usize> {
    let rest = s.get(from..)?;
    [rest.find(PASTE_OPEN), rest.find(IMAGE_OPEN), rest.find(FILE_OPEN), rest.find('['), rest.find(QUOTE_OPEN)].into_iter().flatten().min().map(|i| from + i)
}

/// The file mention at the very start of `s`, when it renders back the
/// same: the part and the bytes it takes (the TUI's history draws each
/// as its chip). Reads the link [`file_marker`] writes and, from stored
/// history forever, the old `<file name="[File #2]" path="…">` marker
/// (architect m_53247: not a shim): both give the same part, its `name`
/// the file's name (the old label's number is not in the new text).
pub fn file_marker_at(s: &str) -> Option<(Part, usize)> {
    file_link_at(s).or_else(|| old_file_marker_at(s))
}

/// The first file mention in `s`: where it starts, its part and the
/// bytes it takes.
pub fn find_file_marker(s: &str) -> Option<(usize, Part, usize)> {
    let mut scan = 0usize;
    while let Some(at) = next_tag(s, scan) {
        if let Some((p, used)) = file_marker_at(&s[at..]) {
            return Some((at, p, used));
        }
        scan = at + 1;
    }
    None
}

/// A file's link (`[name](path)`, ` (missing)` after it) at the very
/// start of `s`.
fn file_link_at(s: &str) -> Option<(Part, usize)> {
    let rest = s.strip_prefix('[')?;
    let close = unescaped(rest, ']')?;
    let dest = rest[close + 1..].strip_prefix('(')?;
    let path = match dest.strip_prefix('<') {
        Some(d) => {
            let end = unescaped(d, '>')?;
            d[end + 1..].strip_prefix(')')?;
            d[..end].replace("\\<", "<").replace("\\>", ">")
        }
        None => dest[..dest.find(')')?].to_string(),
    };
    if !path.starts_with('/') {
        return None;
    }
    let link = file_marker(&path, false);
    if !s.starts_with(&link) {
        return None;
    }
    let missing = s[link.len()..].starts_with(FILE_MISSING);
    let used = link.len() + if missing { FILE_MISSING.len() } else { 0 };
    Some((Part::File { name: file_mention_name(&path), path, missing }, used))
}

/// The byte of the first `c` in `s` that no `\` escapes, before any
/// newline.
fn unescaped(s: &str, c: char) -> Option<usize> {
    let mut esc = false;
    for (i, ch) in s.char_indices() {
        match ch {
            '\n' => return None,
            _ if esc => esc = false,
            '\\' => esc = true,
            _ if ch == c => return Some(i),
            _ => {}
        }
    }
    None
}

/// The old file marker ([`old_file_marker`]) at the very start of `s`.
fn old_file_marker_at(s: &str) -> Option<(Part, usize)> {
    let rest = s.strip_prefix(FILE_OPEN)?;
    let (name, rest) = rest.split_once("\" path=\"")?;
    let (path, tail) = rest.split_once('"')?;
    let missing = tail.starts_with(" missing=\"true\">");
    let m = old_file_marker(name, path, missing);
    if !s.starts_with(&m) {
        return None;
    }
    Some((Part::File { path: path.into(), name: file_mention_name(path), missing }, m.len()))
}

/// The paste tag at the very start of `s`, when it renders back the same.
fn paste_at(s: &str) -> Option<(Part, usize)> {
    let rest = s.strip_prefix(PASTE_OPEN)?;
    let (n, rest) = rest.split_once('"')?;
    if n.is_empty() || n.len() > 9 || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = n.parse().ok()?;
    let rest = rest.strip_prefix(" lines=\"")?;
    let (_, rest) = rest.split_once("\">\n")?;
    let end = rest.find(&format!("\n{PASTE_CLOSE}"))?;
    let text = &rest[..end];
    let tag = paste_tag(n, text);
    if !s.starts_with(&tag) {
        return None;
    }
    Some((Part::Paste { n, text: text.to_string() }, tag.len()))
}

/// The image marker at the very start of `s`, when it renders back the
/// same.
fn image_at(s: &str) -> Option<(Part, usize)> {
    let rest = s.strip_prefix(IMAGE_OPEN)?;
    let (name, rest) = rest.split_once("\" path=\"")?;
    let (source, rest) = rest.split_once("\" mime=\"")?;
    let (mime, rest) = rest.split_once("\" b64=\"")?;
    let (b64, _) = rest.split_once("\">")?;
    let (b64, caption) = b64.split_once(CAPTION_ATTR).unwrap_or((b64, ""));
    let m = image_marker(name, source, mime, b64, caption);
    if !s.starts_with(&m) {
        return None;
    }
    Some((Part::Image { name: name.into(), source: source.into(), mime: mime.into(), b64_file: b64.into(), caption: caption.into() }, m.len()))
}

// ---- reading parts ----

/// The parts of an old text (`parse`), only when it holds more than words
/// (a quote, a paste, an image, a file): none for a plain text, which is
/// drawn from itself. The fold's read of an entry with no parts line, and
/// a client's own event of a text it has (a resend, a history line).
pub fn of_text(text: &str) -> Vec<Part> {
    let parts = parse(text);
    match parts.iter().all(|p| matches!(p, Part::Text { .. })) {
        true => Vec::new(),
        false => parts,
    }
}

/// `parts` with each page note typed (card-image-parts, architect
/// m_35268): a note an older hub stored, his answer's images rendered as
/// markers in its text and no parts, gets its parts read back from that
/// text ([`of_text`]) and its text becomes his words alone, so every
/// client draws chips, never a tag. A note with parts, or a plain one, is
/// kept. Part of the old-text reader, never a shim: stored transcripts
/// keep those notes forever (architect m_48962). The fold's one read of a
/// page part.
pub fn typed_page_notes(mut parts: Vec<Part>) -> Vec<Part> {
    for p in &mut parts {
        let Part::Page { notes, .. } = p else { continue };
        for n in notes.iter_mut().filter(|n| n.parts.is_empty()) {
            let read = of_text(&n.text);
            if read.iter().any(|q| matches!(q, Part::Image { .. })) {
                n.text = words(&read).trim().to_string();
                n.parts = read;
            }
        }
    }
    parts
}

/// His words: the text parts, the pastes and images as nothing (the
/// gist of a row, a notification).
pub fn words(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Parts as the composers make them: adjacent texts joined, empty texts
/// dropped, a `</selection>` / `</pasted>` inside a quote or a paste
/// broken as its tag breaks it. `parse(&render(p)) == canonical(p)`.
pub fn canonical(parts: &[Part]) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    for p in parts {
        let p = match p {
            Part::Quote { from, text, file, new, old, n } => {
                let a = |s: &str| s.replace('"', "'").replace('\n', " ");
                Part::Quote { from: a(from), text: unclosed(text, QUOTE_CLOSE), file: a(file), new: a(new), old: a(old), n: *n }
            }
            Part::Paste { n, text } => Part::Paste { n: *n, text: unclosed(text, PASTE_CLOSE) },
            Part::Text { text } if text.is_empty() => continue,
            Part::Unknown(_) => continue,
            p => p.clone(),
        };
        match (out.last_mut(), &p) {
            (Some(Part::Text { text: a }), Part::Text { text: b }) => a.push_str(b),
            _ => out.push(p),
        }
    }
    out
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
