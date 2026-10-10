//! Image attachments in the composer (docs/images.md).
//!
//! The composer shows `[Image #N]`; the app keeps what each label
//! stands for. On send, every label still in the text becomes its
//! marker (`<image name=… b64=…>`, see the `bend-images` crate) and the
//! attachments start over. Sources: a picked `@` image, a paste that is
//! only image paths (a file dragged into the terminal, iTerm2's "save to
//! temp file and paste path"), Ctrl+V, Cmd+V passed through and an empty
//! paste (the clipboard image, see `input::on_paste`). Nothing here
//! panics on any input.

use crate::app::App;
use crate::theme::{accent, dim, error, glyph, text, G_FAILED, G_IMAGE, G_PASTE, G_QUOTE};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

/// One attached image: its composer label, its marker, what the strip
/// says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    pub(crate) label: String,
    pub(crate) marker: String,
    pub(crate) info: Info,
}

/// What the strip above the composer says about an attached image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Info {
    /// where it came from: the path as picked or dropped, or `clipboard`
    pub(crate) source: String,
    /// the size the model gets (after a downscale)
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// the bytes the model gets
    pub(crate) bytes: u64,
    /// downscaled to fit the limits
    pub(crate) resized: bool,
}

/// The label of image `n`.
pub(crate) fn label(n: usize) -> String {
    format!("[Image #{n}]")
}

/// An image chip, the only attachment that sends an image: quotes,
/// pastes and artifact chips are text. Its label says it, not its size:
/// the store keeps an image whose header it cannot read with a 0x0 size.
pub(crate) fn is_image(a: &Attachment) -> bool {
    a.label.starts_with("[Image #")
}

/// The number of a chip label: 3 for `[Image #3]`, `[Quote #3]`,
/// `[Paste #3]`.
fn label_number(label: &str) -> Option<usize> {
    label.rsplit_once('#')?.1.strip_suffix(']')?.parse().ok()
}

/// Forget the attachments whose chip left the composer text; the lowest
/// number no chip uses. Images, quotes and pastes share one sequence
/// (BISE-240, designer): `❝ 1`, `❝ 2`, `▣ 3`, so a number always points
/// at one row of the box.
pub(crate) fn next_number(app: &mut App) -> usize {
    let text = &app.ed.text;
    app.attachments.retain(|a| text.contains(&a.label));
    let used: Vec<usize> = app.attachments.iter().filter_map(|a| label_number(&a.label)).collect();
    (1..).find(|n| !used.contains(n)).unwrap_or(1)
}

/// Put `path` in the image store and its label at the cursor; the
/// chip (`▣ 1`) for the flash.
pub(crate) fn attach_file(app: &mut App, path: &Path, source: &str) -> Result<String, String> {
    let stored = bend_images::store_file(path)?;
    let original = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Ok(add(app, source, source, original, &stored))
}

/// What the strip says about `stored`: the store keeps the bytes as
/// they came unless they were downscaled, so a size change means a
/// resize.
fn info_of(shown: &str, original_bytes: u64, stored: &bend_images::Stored) -> Info {
    let bytes = std::fs::metadata(&stored.file).map(|m| m.len()).unwrap_or(original_bytes);
    Info {
        source: shown.to_string(),
        width: stored.width,
        height: stored.height,
        bytes,
        resized: original_bytes != 0 && bytes != original_bytes,
    }
}

fn add(app: &mut App, source: &str, shown: &str, original: u64, stored: &bend_images::Stored) -> String {
    let n = next_number(app);
    let l = label(n);
    let marker = bend_images::marker(&l, source, stored);
    let info = info_of(shown, original, stored);
    app.attachments.push(Attachment { label: l.clone(), marker, info });
    insert_chip(&mut app.ed, &l);
    // the flash names the chip
    chip_name(&l)
}

/// Puts the chip `label` at the cursor, like a paste (it replaces the
/// selection; one undo step): a space before it when it would touch a
/// word or another chip, one after it; the cursor lands past that space,
/// so a key typed next goes right after the chip (BISE-207). Images and
/// quotes (quote.rs) alike.
pub(crate) fn insert_chip(ed: &mut crate::editor::Editor, label: &str) {
    let at = ed.selection().map_or(ed.cursor, |(a, _)| a);
    let before = ed.text.chars().nth(at.wrapping_sub(1));
    let pad = if at > 0 && before.is_some_and(|c| !c.is_whitespace()) { " " } else { "" };
    ed.paste(&format!("{pad}{label} "));
}

/// Puts the voice chip (BISE-222) at the cursor: like [`insert_chip`]
/// but bare (no space around it: the transcript that replaces it gets
/// its own, voice.rs) and not an undo step (the transcript is; see
/// [`crate::editor::Editor::put_mark`]).
pub(crate) fn insert_live_chip(ed: &mut crate::editor::Editor) {
    ed.put_mark(crate::voice::chip::LABEL);
}

/// The clipboard image. Unit tests (the fuzzers press Ctrl+V and paste
/// empty text) never touch the real clipboard: one osascript each would
/// take seconds.
fn clipboard_bytes() -> Result<Vec<u8>, String> {
    if cfg!(test) && bise_home::env::test_setting("BEND_CLIPBOARD_IMAGE_FILE").is_none() {
        return Err("no clipboard in unit tests".into());
    }
    bend_images::clipboard_image()
}

// A unit test's clipboard image, already stored (`tests::clip`): no
// osascript, nothing written to the image store.
#[cfg(test)]
thread_local! {
    static TEST_CLIP: RefCell<Option<bend_images::Stored>> = const { RefCell::new(None) };
}

/// Ctrl+V, Cmd+V, an empty paste: the clipboard image, stored as PNG,
/// its chip at the cursor (it replaces the selection).
pub(crate) fn attach_clipboard(app: &mut App) -> Result<String, String> {
    #[cfg(test)]
    if let Some(stored) = TEST_CLIP.with(|c| c.borrow_mut().take()) {
        let source = stored.file.to_string_lossy().to_string();
        return Ok(add(app, &source, CLIPBOARD, 0, &stored));
    }
    let bytes = clipboard_bytes()?;
    let original = bytes.len() as u64;
    let stored = bend_images::store_bytes(bytes)?;
    let source = stored.file.to_string_lossy().to_string();
    Ok(add(app, &source, CLIPBOARD, original, &stored))
}

/// A paste that is only image paths (drag-and-drop): attach them all.
/// None when the paste is something else (it stays text).
pub(crate) fn on_paste(app: &mut App, text: &str) -> Option<Result<Vec<String>, String>> {
    let paths = bend_images::pasted_images(text)?;
    let mut labels = Vec::new();
    for p in &paths {
        match attach_file(app, p, &p.to_string_lossy()) {
            Ok(l) => labels.push(l),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(Ok(labels))
}

/// The text to send: each image label still present becomes its
/// marker; each quote label leaves the text and its tag goes in front,
/// one per line, in text order (quote.rs); the attachments start over.
pub(crate) fn expand(app: &mut App, text: &str) -> String {
    let atts = std::mem::take(&mut app.attachments);
    let mut out = text.to_string();
    let mut quotes: Vec<(usize, &Attachment)> = Vec::new();
    for a in &atts {
        if crate::quote::is_quote(&a.label) {
            if let Some(at) = text.find(&a.label) {
                quotes.push((at, a));
            }
            // the label and one space after it (else before it) go
            for pat in [format!("{} ", a.label), format!(" {}", a.label), a.label.clone()] {
                out = out.replace(&pat, "");
            }
        } else {
            out = out.replace(&a.label, &a.marker);
        }
    }
    if quotes.is_empty() {
        return out;
    }
    quotes.sort_by_key(|(at, _)| *at);
    let mut head: Vec<&str> = quotes.iter().map(|(_, a)| a.marker.as_str()).collect();
    let rest = out.trim();
    if !rest.is_empty() {
        head.push(rest);
    }
    head.join("\n")
}

/// The picked `@` entry is an image file: attach it (the `@token` goes).
/// Returns the label, or None when it is not an image.
pub(crate) fn pick_image(app: &mut App, rel: &str) -> Option<Result<String, String>> {
    if !bend_images::has_image_ext(rel) {
        return None;
    }
    let (start, _) = crate::files::token(&app.ed.text, app.ed.cursor)?;
    let cursor = app.ed.cursor;
    // the root the `@` popup searched (commands::at_items)
    let root = crate::sb::workspace(app)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let path = root.join(rel);
    if !path.is_file() {
        return None;
    }
    // drop the `@token`, then attach at its place
    let chars: Vec<char> = app.ed.text.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars.get(..start).unwrap_or(&[]).iter().collect();
    let tail: String = chars.get(cursor..).unwrap_or(&[]).iter().collect();
    let tail = tail.strip_prefix(' ').map(str::to_string).unwrap_or(tail);
    let at = head.chars().count();
    let before = app.ed.text.clone();
    app.ed.set(&format!("{head}{tail}"), at);
    match attach_file(app, &path, rel) {
        Ok(l) => Some(Ok(l)),
        Err(e) => {
            app.ed.set(&before, cursor);
            Some(Err(e))
        }
    }
}

// ---- chips (book §14): `▣ 1` in the composer, `▣ login.png` in the history ----

/// What a clipboard image is called in the strip and the history.
const CLIPBOARD: &str = "clipboard";

/// The labels `[Image #N]`, `[Quote #N]` (quote.rs), `[Paste #N]`
/// (pasted.rs) and the voice chip (voice/chip.rs) in `text`: (first char
/// index, char index past it, N), in text order. The composer draws each
/// as one chip, the cursor steps over it, a delete takes it whole.
pub(crate) fn chips(text: &str) -> Vec<(usize, usize, usize)> {
    let mut v = image_chips(text);
    v.extend(crate::quote::chips(text));
    v.extend(crate::pasted::chips(text));
    v.extend(find_labels(text, crate::voice::chip::OPEN));
    v.extend(find_labels(text, ARTIFACT_OPEN));
    v.sort_unstable();
    v
}

// ---- an artifact's chip (site/m/artifacts C, E) ----

/// How the label of an artifact's chip starts (`[Artifact #3]`): the
/// composer draws it ` ↗ pricing page `; on send it becomes
/// `[pricing page](artifact:pricing-page)`, the title and the link,
/// never `@name` (that sends to an agent).
pub(crate) const ARTIFACT_OPEN: &str = "[Artifact #";

thread_local! {
    /// the title of each artifact chip, by its label
    static ARTIFACT_TITLES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The title an artifact chip shows.
fn artifact_title(label: &str) -> Option<String> {
    label.starts_with(ARTIFACT_OPEN).then(|| ARTIFACT_TITLES.with(|t| t.borrow().get(label).cloned()).unwrap_or_default())
}

/// Puts the chip of artifact `id` at the cursor (`@` in /artifacts, a
/// pick in the `@` popup).
pub(crate) fn insert_artifact(app: &mut App, id: &str, title: &str) {
    let n = next_number(app);
    let label = format!("{ARTIFACT_OPEN}{n}]");
    let marker = format!("[{}]({})", title.replace(['[', ']'], ""), crate::artifacts::url_of(id, None));
    ARTIFACT_TITLES.with(|t| t.borrow_mut().insert(label.clone(), title.to_string()));
    app.attachments.push(Attachment { label: label.clone(), marker, info: Info { source: id.to_string(), ..Default::default() } });
    insert_chip(&mut app.ed, &label);
}

/// The draft holds an artifact's chip (the key bar says how it goes).
pub(crate) fn has_artifact(text: &str) -> bool {
    !find_labels(text, ARTIFACT_OPEN).is_empty()
}

/// The image labels `[Image #N]` in `text`.
pub(crate) fn image_chips(text: &str) -> Vec<(usize, usize, usize)> {
    find_labels(text, "[Image #")
}

/// The labels `{open}N]` in `text` (`open` ASCII): (first char index,
/// char index past it, N).
pub(crate) fn find_labels(text: &str, open: &str) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0usize; // bytes
    let mut ci = 0usize; // chars before `from`
    while let Some(i) = text.get(from..).and_then(|t| t.find(open)) {
        let at = from + i;
        ci += text[from..at].chars().count();
        let digits: String = text[at + open.len()..].chars().take_while(char::is_ascii_digit).collect();
        let close = at + open.len() + digits.len();
        let n = digits.parse::<usize>().ok().filter(|_| digits.len() <= 6);
        match n {
            Some(n) if text[close..].starts_with(']') => {
                let len = open.len() + digits.len() + 1; // ASCII: bytes = chars
                out.push((ci, ci + len, n));
                from = at + len;
                ci += len;
            }
            _ => {
                from = at + 1;
                ci += 1;
            }
        }
    }
    out
}

/// The chip holding char index `ci` strictly inside (not at its start).
pub(crate) fn chip_around(text: &str, ci: usize) -> Option<(usize, usize)> {
    chips(text).into_iter().find(|&(a, b, _)| a < ci && ci < b).map(|(a, b, _)| (a, b))
}

/// The range `[a, b)` grown to take whole every chip it touches.
pub(crate) fn chip_widen(text: &str, a: usize, b: usize) -> (usize, usize) {
    chips(text).into_iter().fold((a, b), |(a, b), (s, e, _)| {
        if s < b && a < e {
            (a.min(s), b.max(e))
        } else {
            (a, b)
        }
    })
}

/// A chip's glyph (`▣`, `❝` or `▤`) and number, from its label
/// `[Image #N]`, `[Quote #N]` or `[Paste #N]`.
fn chip_parts(label: &str) -> (&'static str, &str) {
    if let Some(n) = label.strip_prefix(crate::quote::OPEN) {
        return (glyph(G_QUOTE), n.trim_end_matches(']'));
    }
    if let Some(n) = label.strip_prefix(crate::pasted::OPEN) {
        return (glyph(G_PASTE), n.trim_end_matches(']'));
    }
    (G_IMAGE, label.trim_start_matches("[Image #").trim_end_matches(']'))
}

/// A chip named in a sentence (the flash): `▣ 1`, `❝ 1`.
pub(crate) fn chip_name(label: &str) -> String {
    if let Some(t) = artifact_title(label) {
        return format!("↗ {t}");
    }
    let (g, n) = chip_parts(label);
    format!("{g} {n}")
}

/// How the composer and the strip draw a chip (BISE-205): a pill
/// ` ❝ 1 ` (one padding cell each side, on the `pill` tint), or, with no
/// tint (`NO_COLOR`, `BISE_ASCII=1`), `[❝ 1]`. Same width both ways.
pub(crate) fn chip_text(label: &str) -> String {
    if let Some(t) = artifact_title(label) {
        return match crate::render::chip_form() {
            crate::render::ChipForm::Tinted => format!(" ↗ {t} "),
            crate::render::ChipForm::Bracketed => format!("[↗ {t}]"),
        };
    }
    let (g, n) = chip_parts(label);
    match crate::render::chip_form() {
        crate::render::ChipForm::Tinted => format!(" {g} {n} "),
        crate::render::ChipForm::Bracketed => format!("[{g} {n}]"),
    }
}

/// The columns the chip `label` takes in `inner` columns of text.
pub(crate) fn chip_width(label: &str, inner: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    if is_voice(label) {
        crate::voice::chip::width(inner)
    } else {
        chip_text(label).width()
    }
}

/// `label` is the voice chip.
pub(crate) fn is_voice(label: &str) -> bool {
    label.starts_with(crate::voice::chip::OPEN)
}

/// The spans of [`chip_text`]: the glyph accent, the number in the text
/// color, all on the pill tint (the brackets dim); `over` is patched on
/// every span (the selection's background, the cursor's REVERSED), so
/// the whole pill takes it.
pub(crate) fn chip_pill(label: &str, over: Style) -> Vec<Span<'static>> {
    let artifact = artifact_title(label);
    let (g, n) = match &artifact {
        Some(t) => ("↗", t.as_str()),
        None => chip_parts(label),
    };
    let (open, close, bg) = match crate::render::chip_form() {
        crate::render::ChipForm::Tinted => (" ", " ", Some(crate::theme::pill_bg())),
        crate::render::ChipForm::Bracketed => ("[", "]", None),
    };
    let st = |fg| {
        let s = Style::default().fg(fg);
        bg.map_or(s, |b| s.bg(b)).patch(over)
    };
    vec![
        Span::styled(open.to_string(), st(dim())),
        Span::styled(g.to_string(), st(accent())),
        Span::styled(format!(" {n}"), st(text())),
        Span::styled(close.to_string(), st(dim())),
    ]
}

/// The style of a chip in the history: accent (the background stays the
/// terminal's).
pub(crate) fn chip_style() -> Style {
    Style::default().fg(accent())
}

/// The image store holds `path` (a clipboard image is stored first,
/// its path is the store's).
fn in_store(path: &str) -> bool {
    let Some(dir) = bend_images::store_dir() else { return false };
    Path::new(path).parent().is_some_and(|p| p == dir)
}

/// The name of an image in the history: `clipboard`, else the file name.
fn short_name(path: &str) -> String {
    if path.is_empty() {
        return "image".into();
    }
    if in_store(path) {
        return CLIPBOARD.into();
    }
    Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

thread_local! {
    /// the size of each stored image, by its `.b64` path (read once)
    static SIZES: RefCell<HashMap<String, Option<(u32, u32)>>> = RefCell::new(HashMap::new());
    /// the model of the agent in view (ui.rs sets it each frame)
    static MODEL: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The size of a marker's image: the decoded copy beside its `.b64`
/// in the store (`<hash>.<ext>`), read once. None when it is gone.
fn marker_size(m: &bend_images::Marker) -> Option<(u32, u32)> {
    if let Some(s) = SIZES.with(|c| c.borrow().get(&m.b64).copied()) {
        return s;
    }
    let size = bend_images::Kind::from_mime(&m.mime).and_then(|k| {
        let file = Path::new(&m.b64).with_extension(k.ext());
        let bytes = std::fs::read(file).ok()?;
        bend_images::dimensions(&bytes)
    });
    SIZES.with(|c| c.borrow_mut().insert(m.b64.clone(), size));
    size
}

fn wxh((w, h): (u32, u32)) -> String {
    format!("{w}×{h}")
}

/// `text` as spans in `style`, each image marker an accent chip
/// `▣ login.png` (a user line of the history; one line, no `\n`).
pub(crate) fn chip_spans(text: &str, style: Style) -> Vec<Span<'static>> {
    // an artifact's chip as sent, `[pricing page](artifact:pricing-page)`:
    // the same chip as in the composer and in replies (site/m/artifacts E)
    if let Some((a, b, title, url)) = artifact_link(text) {
        let mut out = chip_spans(&text[..a], style);
        let gone = crate::artifacts::parse_url(&url).and_then(|(id, _)| crate::artifacts::get(&id)).is_some_and(|x| x.gone);
        out.extend(crate::render::artifact_chip(&title, gone, &url));
        out.extend(chip_spans(&text[b..], style));
        return out.into_iter().filter(|s| !s.content.is_empty()).collect();
    }
    // a paste's chip mark (pasted::fold): `▤ 1`, accent
    let marks = crate::pasted::marks(text);
    if marks.is_empty() {
        return image_spans(text, style);
    }
    let g = glyph(G_PASTE);
    let mut out = Vec::new();
    let mut last = 0usize;
    for (a, b, n) in marks {
        let before = text.get(last..a).unwrap_or("");
        if !before.is_empty() {
            out.extend(image_spans(before, style));
        }
        out.push(Span::styled(format!("{g} {n}"), chip_style()));
        last = b;
    }
    let rest = text.get(last..).unwrap_or("");
    if !rest.is_empty() {
        out.extend(image_spans(rest, style));
    }
    out
}

/// The first `[title](artifact:id)` in `text`: its byte range, its title
/// and its url.
fn artifact_link(text: &str) -> Option<(usize, usize, String, String)> {
    let mut from = 0;
    while let Some(m) = text[from..].find("](artifact:").map(|i| from + i) {
        let close = text[m..].find(')').map(|i| m + i);
        let open = text[..m].rfind('[');
        if let (Some(a), Some(c)) = (open, close) {
            let title = &text[a + 1..m];
            let url = &text[m + 2..c];
            if !title.contains(']') && crate::artifacts::parse_url(url).is_some() {
                return Some((a, c + 1, title.to_string(), url.to_string()));
            }
        }
        from = m + 2;
    }
    None
}

/// [`chip_spans`] for a text with no paste mark: each image marker
/// drawn `▣ name`.
fn image_spans(text: &str, style: Style) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut last = 0usize;
    for m in bend_images::markers(text) {
        let before = text.get(last..m.start).unwrap_or("");
        if !before.is_empty() {
            out.push(Span::styled(before.to_string(), style));
        }
        out.push(Span::styled(format!("{G_IMAGE} {}", short_name(&m.path)), chip_style()));
        last = m.end;
    }
    let rest = text.get(last..).unwrap_or("");
    if !rest.is_empty() || out.is_empty() {
        out.push(Span::styled(rest.to_string(), style));
    }
    out
}

/// The dim line under a user line with images: each image's size,
/// `▣ login-mobile.png 1170×2532 · ▣ clipboard 2048×1536`. None
/// without images.
pub(crate) fn sizes_line(text: &str) -> Option<String> {
    let parts: Vec<String> = bend_images::markers(text)
        .iter()
        .map(|m| match marker_size(m) {
            Some(s) => format!("{G_IMAGE} {} {}", short_name(&m.path), wxh(s)),
            None => format!("{G_IMAGE} {}", short_name(&m.path)),
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A tool result holding images: `result · ▣ screenshot.png 390×844`
/// (dim, accent chip, dim size). None without images.
pub(crate) fn result_spans(text: &str) -> Option<Vec<Span<'static>>> {
    let ms = bend_images::markers(text);
    if ms.is_empty() {
        return None;
    }
    let d = Style::default().fg(dim());
    let mut out = vec![Span::styled("result ·".to_string(), d)];
    for (i, m) in ms.iter().enumerate() {
        if i > 0 {
            out.push(Span::styled(" ·".to_string(), d));
        }
        out.push(Span::raw(" "));
        out.push(Span::styled(format!("{G_IMAGE} {}", short_name(&m.path)), chip_style()));
        if let Some(s) = marker_size(m) {
            out.push(Span::styled(format!(" {}", wxh(s)), d));
        }
    }
    Some(out)
}

/// `text` without its image markers (the text around a result's images).
pub(crate) fn without_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for m in bend_images::markers(text) {
        out.push_str(text.get(last..m.start).unwrap_or(""));
        last = m.end;
    }
    out.push_str(text.get(last..).unwrap_or(""));
    out
}

// ---- a model without vision (book §17) ----

/// The model of the agent in view, for the no-vision line (ui.rs).
pub(crate) fn set_model(model: &str) {
    MODEL.with(|m| {
        if *m.borrow() != model {
            *m.borrow_mut() = model.to_string();
        }
    });
}

/// The composer holds images (their labels) and the agent in view runs
/// a model the catalog lists without vision (BISE-150): its name, so the
/// message is not sent. A slash command, or a model the catalog does not
/// list: None (the provider decides).
pub(crate) fn refused_images(app: &App) -> Option<String> {
    let text = app.ed.text.trim_start();
    if text.starts_with('/') || !app.attachments.iter().any(|a| is_image(a) && text.contains(&a.label)) {
        return None;
    }
    let model = crate::sb::focus_model(app);
    crate::models::lacks_vision(&model).then_some(model)
}

/// The error the no-vision line is drawn from when the catalog refused
/// the images before sending ([`is_no_vision`] matches it).
pub(crate) fn no_vision_error(model: &str) -> String {
    format!("{model} does not support image input (bise's model catalog): not sent")
}

/// The provider refused images: its error says image (or vision) and
/// that it is not supported. Anthropic, Mistral and OpenAI-style
/// providers each word it their way.
pub(crate) fn is_no_vision(err: &str) -> bool {
    let e = err.to_lowercase();
    let about = ["image", "vision", "multimodal", "multi-modal"].iter().any(|w| e.contains(w));
    let refused = [
        "not support",
        "unsupported",
        "only supported",
        "not supported",
        "does not accept",
        "doesn't support",
        "not allowed",
        "not enabled",
        "not available for this model",
        "invalid content type",
        "unknown variant `image",
        "cannot read",
    ]
    .iter()
    .any(|w| e.contains(w));
    // our own attach errors ("image too large", "not a PNG…") are not it
    about && refused && !e.contains("image too large") && !e.contains("not a png")
}

/// The no-vision line: `✗ {model} can't read images.` (error) then the
/// way out (dim). None when `err` is another error.
pub(crate) fn no_vision(err: &str) -> Option<Vec<Span<'static>>> {
    if !is_no_vision(err) {
        return None;
    }
    let model = MODEL.with(|m| m.borrow().clone());
    Some(no_vision_spans(if model.is_empty() { "this model" } else { &model }))
}

fn no_vision_spans(model: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("  {G_FAILED} "), Style::default().fg(error())),
        Span::styled(format!("{model} can't read images."), Style::default().fg(error())),
        Span::styled(
            " pick a model that can (/model), or describe the screen in words.".to_string(),
            Style::default().fg(dim()),
        ),
    ]
}

// ---- the strip above the composer (book §14) ----

/// `310 kB`, `1.1 MB` (decimal units, like Finder).
pub(crate) fn size_text(bytes: u64) -> String {
    if bytes < 1_000 {
        format!("{bytes} B")
    } else if bytes < 999_500 {
        format!("{} kB", (bytes + 500) / 1_000)
    } else {
        let tenths = (bytes + 50_000) / 100_000;
        format!("{}.{} MB", tenths / 10, tenths % 10)
    }
}

/// One strip row: `▣ 1 shots/login-mobile.png` and, dim on the right,
/// `1170×2532 · 310 kB` (` → resized to fit 2048` after a downscale).
pub(crate) fn strip_row(n: usize, info: &Info) -> (String, String) {
    let left = format!("{G_IMAGE} {n} {}", file_name(&info.source));
    let mut right = format!("{} · {}", wxh((info.width, info.height)), size_text(info.bytes));
    if info.resized {
        right.push_str(&format!(" → resized to fit {}", bend_images::MAX_DIMENSION));
    }
    (left, right)
}

/// The box's title, in its top border (book §13 "The attachments box").
pub(crate) const BOX_TITLE: &str = "attached";
/// The box's tip, in its bottom border on the right.
pub(crate) const BOX_TIP: &str = "backspace on a chip removes it";
/// The box's width bounds: as wide as its longest row, at least 44
/// columns, at most the reading width (full width when narrower).
const BOX_MIN: usize = 44;
const BOX_MAX: usize = 91;
/// A preview keeps at least this many columns before the source on its
/// right goes.
const PREVIEW_MIN: usize = 16;
/// Between the preview and the source, at least.
const ROW_GAP: usize = 4;

/// The images still in the composer text, by number.
fn shown(app: &App) -> Vec<(usize, &Attachment)> {
    let mut v: Vec<(usize, &Attachment)> = image_chips(&app.ed.text)
        .into_iter()
        .filter_map(|(_, _, n)| app.attachments.iter().find(|a| a.label == label(n)).map(|a| (n, a)))
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v.dedup_by_key(|(n, _)| *n);
    v
}

/// The quotes still in the composer text, by number.
fn shown_quotes(app: &App) -> Vec<(usize, crate::quote::Quote)> {
    let mut v: Vec<(usize, crate::quote::Quote)> = crate::quote::chips(&app.ed.text)
        .into_iter()
        .filter_map(|(_, _, n)| {
            let l = crate::quote::label(n);
            app.attachments.iter().find(|a| a.label == l).and_then(crate::quote::of).map(|q| (n, q))
        })
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v.dedup_by_key(|(n, _)| *n);
    v
}

/// How many rows the attachments box takes (0: nothing attached): its
/// two borders and one row per attachment.
pub(crate) fn strip_height(app: &App) -> u16 {
    match shown_quotes(app).len() + shown(app).len() + shown_pastes(app).len() {
        0 => 0,
        n => n as u16 + 2,
    }
}

/// The pastes still in the composer text, by number (pasted.rs).
fn shown_pastes(app: &App) -> Vec<crate::pasted::Pasted> {
    let mut v: Vec<crate::pasted::Pasted> = crate::pasted::chips(&app.ed.text)
        .into_iter()
        .filter_map(|(_, _, n)| {
            let l = crate::pasted::label(n);
            app.attachments.iter().find(|a| a.label == l).and_then(crate::pasted::of)
        })
        .collect();
    v.sort_by_key(|p| p.n);
    v.dedup_by_key(|p| p.n);
    v
}

/// What the strip names an image by: the file name of a dropped or
/// picked path (never the whole path, user request on 6df3967),
/// `clipboard` as is.
pub(crate) fn file_name(source: &str) -> &str {
    let s = source.trim_end_matches(['/', '\\']);
    s.rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).unwrap_or(source)
}

/// `s` in at most `max` columns: cut at its end with `…`.
fn cut_end(s: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 1; // the `…`
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > max {
            break;
        }
        used += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

/// One row of the box before it is cut: its chip label, its preview
/// (a quote's words in `“”`, an image's file name), its source (`main ·
/// 1 line`, `1170×2532 · 310 kB`).
struct BoxRow {
    label: String,
    preview: Preview,
    source: String,
    /// a quote from a diff: its place (`src/a.rs:12-14`) stays, the
    /// preview gives way first
    place: bool,
}

enum Preview {
    Quote(String),
    File(String),
}

impl Preview {
    fn natural(&self) -> String {
        match self {
            Preview::Quote(t) => format!("“{}”", t.split_whitespace().collect::<Vec<_>>().join(" ")),
            Preview::File(f) => f.clone(),
        }
    }
    /// At most `max` columns, cut with `…`.
    fn cut(&self, max: usize) -> String {
        match self {
            Preview::Quote(t) if max >= 3 => format!("“{}”", crate::quote::preview(t, max - 2)),
            Preview::Quote(_) => String::new(),
            Preview::File(f) => cut_end(f, max),
        }
    }
}

/// The attachments in the text, by number (quotes, images and pastes
/// share one sequence).
fn box_rows(app: &App) -> Vec<BoxRow> {
    let mut rows: Vec<(usize, BoxRow)> = shown_quotes(app)
        .into_iter()
        .map(|(n, q)| {
            let source = crate::quote::about(&q);
            let place = !q.at.file.is_empty();
            (n, BoxRow { label: crate::quote::label(n), preview: Preview::Quote(crate::quote::words(&q)), source, place })
        })
        .collect();
    rows.extend(shown(app).into_iter().map(|(n, a)| {
        let (_, source) = strip_row(n, &a.info);
        (n, BoxRow { label: a.label.clone(), preview: Preview::File(file_name(&a.info.source).to_string()), source, place: false })
    }));
    // a paste: its first words like a quote's, `240 lines · 9.8 kB`
    rows.extend(shown_pastes(app).into_iter().map(|p| {
        let source = crate::pasted::about(&p.text);
        (p.n, BoxRow { label: crate::pasted::label(p.n), preview: Preview::Quote(p.text), source, place: false })
    }));
    rows.sort_by_key(|(n, _)| *n);
    rows.into_iter().map(|(_, r)| r).collect()
}

/// A row's content in `iw` columns: the pill, a blank, the preview (dim)
/// and, flush right, the source (faint). Short on room, the preview is
/// cut with `…` down to [`PREVIEW_MIN`]; then the source goes and the
/// preview takes the whole row.
fn box_row(r: &BoxRow, iw: usize) -> Vec<Span<'static>> {
    use unicode_width::UnicodeWidthStr;
    let chip = chip_text(&r.label).width() + 1;
    let natural = r.preview.natural();
    let with_source = iw.saturating_sub(chip + ROW_GAP + r.source.width());
    let (preview, source) = if natural.width() <= with_source {
        (natural, r.source.clone())
    } else if with_source >= PREVIEW_MIN || (r.place && with_source >= 4) {
        (r.preview.cut(with_source), r.source.clone())
    } else {
        (r.preview.cut(iw.saturating_sub(chip)), String::new())
    };
    let pad = iw.saturating_sub(chip + preview.width() + source.width());
    let mut spans = chip_pill(&r.label, Style::default());
    spans.extend([
        Span::styled(format!(" {preview}"), Style::default().fg(dim())),
        Span::raw(" ".repeat(pad)),
        Span::styled(source, Style::default().fg(crate::theme::faint())),
    ]);
    spans
}

/// The box's width in at most `room` columns: its longest row and its
/// frame (2 borders, 2 blank columns each side), at least [`BOX_MIN`],
/// at most [`BOX_MAX`]; all of `room` when narrower.
fn box_width(rows: &[BoxRow], room: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    let longest = rows
        .iter()
        .map(|r| chip_text(&r.label).width() + 1 + r.preview.natural().width() + ROW_GAP + r.source.width())
        .max()
        .unwrap_or(0);
    (longest + 6).clamp(BOX_MIN, BOX_MAX).min(room)
}

/// The attachments box (book §13, the user's pick "d"), in at most
/// `room` columns, drawn from the composer's bar column: a thin rounded
/// frame (dim), `attached` in its top border (dim), one row per
/// attachment in number order at the text's column, the backspace tip
/// in its bottom border on the right (faint; dropped first when short).
/// No bar: the bar marks your message. `NO_COLOR` keeps the frame (it
/// is glyphs); `BISE_ASCII=1` turns it to `+ - |` (asciify).
pub(crate) fn strip_lines(app: &App, room: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let rows = box_rows(app);
    if rows.is_empty() || room < 2 {
        return Vec::new();
    }
    let w = box_width(&rows, room);
    let iw = w.saturating_sub(6);
    let edge = Style::default().fg(dim());
    let line = |n: usize| "─".repeat(n);
    // ╭─ attached ───╮
    let top = if w >= BOX_TITLE.width() + 5 {
        vec![
            Span::styled("╭─ ", edge),
            Span::styled(BOX_TITLE, edge),
            Span::styled(format!(" {}╮", line(w - BOX_TITLE.width() - 5)), edge),
        ]
    } else {
        vec![Span::styled(format!("╭{}╮", line(w - 2)), edge)]
    };
    // ╰──── backspace on a chip removes it ─╯
    let tip = crate::theme::faint();
    let bottom = if w >= BOX_TIP.width() + 7 {
        vec![
            Span::styled(format!("╰{} ", line(w - BOX_TIP.width() - 5)), edge),
            Span::styled(BOX_TIP, Style::default().fg(tip)),
            Span::styled(" ─╯", edge),
        ]
    } else {
        vec![Span::styled(format!("╰{}╯", line(w - 2)), edge)]
    };
    let mut out = vec![Line::from(top)];
    for r in &rows {
        let (lead, trail) = if w >= 6 { ("│  ", "  │") } else { ("│", "│") };
        let inner = if w >= 6 { iw } else { w.saturating_sub(2) };
        let mut spans = vec![Span::styled(lead, edge)];
        if inner > chip_text(&r.label).width() {
            spans.extend(box_row(r, inner));
        } else {
            // no room for the pill: the frame stays whole
            spans.push(Span::raw(" ".repeat(inner)));
        }
        spans.push(Span::styled(trail, edge));
        out.push(Line::from(spans));
    }
    out.push(Line::from(bottom));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn att(n: usize, m: &str) -> Attachment {
        Attachment { label: label(n), marker: m.into(), info: Info { width: 1, ..Default::default() } }
    }

    /// The clipboard holds an image until the next read.
    fn clip() {
        let stored = bend_images::Stored {
            kind: bend_images::Kind::Png,
            width: 2048,
            height: 1536,
            file: "/nowhere/clip.png".into(),
            b64: "/nowhere/clip.b64".into(),
        };
        TEST_CLIP.with(|c| *c.borrow_mut() = Some(stored));
    }

    fn clip_left() -> bool {
        TEST_CLIP.with(|c| c.borrow_mut().take().is_some())
    }

    /// The composer holds `text`, the cursor at `at`.
    fn composer(text: &str, at: usize) -> App {
        let mut app = crate::sb::bench::test_app();
        app.ed.paste(text);
        app.ed.cursor = at;
        app
    }

    #[test]
    fn an_empty_paste_puts_the_clipboard_image_at_the_cursor() {
        // Cmd+V on an image in a terminal that sends an empty paste
        let mut app = composer("hello world", 5);
        clip();
        crate::input::on_paste(&mut app, "");
        assert_eq!(app.ed.text, "hello [Image #1]  world");
        assert_eq!(app.ed.cursor, 17, "past the chip and its space");
        assert_eq!(shown(&app).len(), 1);
        assert_eq!(app.attachments[0].info.source, "clipboard");
        assert!(app.flash.as_ref().is_some_and(|(f, _)| f == "attached ▣ 1"), "{:?}", app.flash);
        // one undo takes the chip and its attachment away
        assert!(app.ed.undo());
        assert_eq!(app.ed.text, "hello world");
        assert!(shown(&app).is_empty());
        // a paste of blanks is empty too
        clip();
        crate::input::on_paste(&mut app, " \r\n");
        assert_eq!(app.ed.text, "hello [Image #1]  world");
    }

    #[test]
    fn a_pasted_image_replaces_the_selection() {
        let mut app = composer("hello world", 0);
        app.ed.select_range(6, 11);
        clip();
        crate::input::on_paste(&mut app, "");
        assert_eq!(app.ed.text, "hello [Image #1] ");
        assert_eq!(app.ed.selection(), None);
        assert!(app.ed.undo());
        assert_eq!(app.ed.text, "hello world");
    }

    #[test]
    fn a_text_paste_never_reads_the_clipboard() {
        let mut app = composer("", 0);
        clip();
        crate::input::on_paste(&mut app, "abc");
        assert_eq!(app.ed.text, "abc");
        assert!(app.attachments.is_empty());
        assert!(clip_left(), "the clipboard was not read");
    }

    #[test]
    fn an_empty_paste_without_an_image_does_nothing() {
        let mut app = composer("hello", 5);
        crate::input::on_paste(&mut app, "");
        assert_eq!(app.ed.text, "hello");
        assert!(app.attachments.is_empty());
        assert_eq!(app.flash, None, "a silent no-op, like the terminal's own empty paste");
    }

    #[test]
    fn ctrl_v_and_a_passed_through_cmd_v_attach_the_clipboard_image() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        for m in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
            let mut app = composer("see", 3);
            clip();
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('v'), m));
            assert_eq!(app.ed.text, "see [Image #1] ", "{m:?}");
            // no image: the key says so
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('v'), m));
            assert_eq!(app.ed.text, "see [Image #1] ", "{m:?}");
            assert!(app.flash.as_ref().is_some_and(|(f, _)| f.contains("clipboard")), "{m:?} {:?}", app.flash);
        }
    }

    #[test]
    fn numbers_reuse_freed_labels() {
        let mut app = composer("hello [Image #2]", 0);
        app.attachments = vec![att(1, "m1"), att(2, "m2")];
        // [Image #1] was deleted from the text: it is forgotten, 1 is free
        assert_eq!(next_number(&mut app), 1);
        assert_eq!(app.attachments.len(), 1);
        app.ed.set("", 0);
        assert_eq!(next_number(&mut app), 1);
        assert!(app.attachments.is_empty());
    }

    /// BISE-240 (designer): quotes, images and pastes count in one
    /// sequence, so a number points at one row of the box.
    #[test]
    fn quotes_images_and_pastes_share_one_number_sequence() {
        let mut app = composer("", 0);
        crate::quote::add(&mut app, "main", "a quote").unwrap();
        clip();
        attach_clipboard(&mut app).unwrap();
        crate::pasted::add(&mut app, &long(20));
        let labels: Vec<&str> = app.attachments.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["[Quote #1]", "[Image #2]", "[Paste #3]"]);
        assert_eq!(app.ed.text, "[Quote #1] [Image #2] [Paste #3] ");
        // the image's chip goes: 2 is free for the next one, of any kind
        app.ed.set("[Quote #1] [Paste #3] ", 22);
        crate::pasted::add(&mut app, &long(20));
        assert!(app.ed.text.ends_with("[Paste #2] "), "{}", app.ed.text);
    }

    fn long(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    /// A long bracketed paste: a chip at the cursor, replacing the
    /// selection, its text an attachment; one undo puts the text inline,
    /// a second takes it away.
    #[test]
    fn a_long_paste_is_a_chip_and_undo_gives_the_text_back() {
        let text = long(300);
        let mut app = composer("see XX now", 4);
        app.ed.select_range(4, 6);
        crate::input::on_paste(&mut app, &text);
        assert_eq!(app.ed.text, "see [Paste #1]  now");
        assert_eq!(app.attachments.len(), 1);
        assert!(app.flash.as_ref().is_some_and(|(f, _)| f == "attached \u{25a4} 1"), "{:?}", app.flash);
        // the box row: its first words, lines and size
        let rows: Vec<String> =
            strip_lines(&app, 100).iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert!(rows[1].contains("\u{25a4} 1  \u{201c}line 1 line 2"), "{rows:?}");
        assert!(rows[1].contains("300 lines \u{b7} 3 kB"), "{rows:?}");
        // sent: the whole text, in its tag, where the chip was
        let mut sent = composer("", 0);
        sent.attachments = app.attachments.clone();
        let out = expand(&mut sent, &app.ed.text);
        assert_eq!(out, format!("see {}  now", crate::pasted::tag(1, text.trim_end())));
        assert!(out.contains("line 300\n</pasted>"));
        // undo: the text inline; again: the text before the paste
        assert!(app.ed.undo());
        assert_eq!(app.ed.text, format!("see {text} now"));
        assert!(app.ed.undo());
        assert_eq!(app.ed.text, "see XX now");
    }

    #[test]
    fn a_short_paste_and_typed_text_stay_inline() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = composer("", 0);
        crate::input::on_paste(&mut app, &long(11));
        assert_eq!(app.ed.text, long(11));
        assert!(app.attachments.is_empty());
        // typed, even a lot: never a chip
        let mut app = composer("", 0);
        for c in "x".repeat(1300).chars() {
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(app.ed.text.len(), 1300);
        assert!(app.attachments.is_empty());
    }

    #[test]
    fn chips_are_found_by_char_index() {
        // `é` is 2 bytes, 1 char: indices are chars
        let t = "é [Image #1] x [Image #12][Image #] [Image #3";
        assert_eq!(chips(t), vec![(2, 12, 1), (15, 26, 12)]);
        assert!(chips("").is_empty());
        assert_eq!(chip_around(t, 5), Some((2, 12)));
        assert_eq!(chip_around(t, 2), None);
        assert_eq!(chip_around(t, 12), None);
        // a range touching a chip takes it whole
        assert_eq!(chip_widen(t, 11, 12), (2, 12));
        assert_eq!(chip_widen(t, 0, 3), (0, 12));
        assert_eq!(chip_widen(t, 12, 13), (12, 13));
        assert_eq!(chip_name("[Image #12]"), "▣ 12");
        assert_eq!(chip_name("[Quote #3]"), "❝ 3");
    }

    #[test]
    fn strip_text_sizes_and_resize() {
        assert_eq!(size_text(999), "999 B");
        assert_eq!(size_text(310_400), "310 kB");
        assert_eq!(size_text(1_100_000), "1.1 MB");
        assert_eq!(size_text(1_149_999), "1.1 MB");
        let shot = Info { source: "shots/login-mobile.png".into(), width: 1170, height: 2532, bytes: 310_000, resized: false };
        assert_eq!(
            strip_row(1, &shot),
            ("▣ 1 login-mobile.png".to_string(), "1170×2532 · 310 kB".to_string())
        );
        let clip = Info { source: "clipboard".into(), width: 2048, height: 1536, bytes: 1_100_000, resized: true };
        assert_eq!(
            strip_row(2, &clip),
            ("▣ 2 clipboard".to_string(), "2048×1536 · 1.1 MB → resized to fit 2048".to_string())
        );
    }

    fn line_text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn width(s: &str) -> usize {
        unicode_width::UnicodeWidthStr::width(s)
    }

    fn quote(n: usize, text: &str) -> Attachment {
        Attachment { label: crate::quote::label(n), marker: crate::quote::tag("main", text), info: Info::default() }
    }

    /// The box (the user's pick "d"): a rounded frame, `attached` in its
    /// top border, one row per attachment in number order, the tip in
    /// its bottom border; as wide as its longest row, at least 44.
    #[test]
    fn the_box_lists_the_chips_still_in_the_text() {
        let mut app = crate::sb::bench::test_app();
        app.attachments = vec![
            Attachment { info: Info { source: "clipboard".into(), width: 2048, height: 1536, bytes: 1_100_000, resized: true }, ..att(2, "m2") },
            Attachment { info: Info { source: "shots/a.png".into(), width: 10, height: 20, bytes: 300, resized: false }, ..att(1, "m1") },
            Attachment { info: Info { source: "gone.png".into(), ..Info::default() }, ..att(3, "m3") },
        ];
        app.ed.set("compare [Image #1] with [Image #2]", 0);
        assert_eq!(strip_height(&app), 4);
        let ls: Vec<String> = strip_lines(&app, 91).iter().map(line_text).collect();
        assert_eq!(ls.len(), 4, "{ls:?}");
        let w = width(&ls[0]);
        // the longest row: pill 5, blank, `clipboard`, 4 blanks, the size
        assert_eq!(w, 6 + 5 + 1 + 9 + 4 + width("2048×1536 · 1.1 MB → resized to fit 2048"));
        assert!(ls.iter().all(|l| width(l) == w), "{ls:?}");
        assert_eq!(ls[0], format!("╭─ attached {}╮", "─".repeat(w - 13)));
        assert!(ls[1].starts_with("│   ▣ 1  a.png ") && ls[1].ends_with("10×20 · 300 B  │"), "{ls:?}");
        assert!(ls[2].starts_with("│   ▣ 2  clipboard    2048×1536") && ls[2].ends_with("→ resized to fit 2048  │"), "{ls:?}");
        assert_eq!(ls[3], format!("╰{} backspace on a chip removes it ─╯", "─".repeat(w - 35)));
        // short rows: the box is 44 wide
        app.ed.set("[Image #1]", 0);
        let ls: Vec<String> = strip_lines(&app, 91).iter().map(line_text).collect();
        assert!(ls.iter().all(|l| width(l) == 44), "{ls:?}");
        app.ed.set("no images", 0);
        assert_eq!(strip_height(&app), 0);
        assert!(strip_lines(&app, 60).is_empty());
    }

    /// The styles: the frame and `attached` dim, the tip and the sources
    /// faint, the previews dim, the pill on its tint; no bar.
    #[test]
    fn the_box_styles() {
        let mut app = crate::sb::bench::test_app();
        app.attachments = vec![quote(1, "la licence du repo,")];
        app.ed.set("pour [Quote #1] tu recommande quoi?", 0);
        let ls = strip_lines(&app, 91);
        let find = |l: &Line<'static>, t: &str| l.spans.iter().find(|s| s.content.as_ref() == t).map(|s| s.style.fg);
        assert_eq!(find(&ls[0], "attached"), Some(Some(dim())));
        assert_eq!(find(&ls[0], "╭─ "), Some(Some(dim())));
        assert_eq!(find(&ls[2], BOX_TIP), Some(Some(crate::theme::faint())));
        assert_eq!(find(&ls[1], " “la licence du repo,”"), Some(Some(dim())));
        assert_eq!(find(&ls[1], "main · 1 line"), Some(Some(crate::theme::faint())));
        assert!(line_text(&ls[1]).starts_with("│   ❝ 1  “la licence du repo,”"), "{:?}", line_text(&ls[1]));
    }

    /// Quotes and images in number order, whatever their kind.
    #[test]
    fn the_box_rows_go_in_number_order() {
        let mut app = crate::sb::bench::test_app();
        app.attachments = vec![
            Attachment { info: Info { source: "shots/readme-dark.png".into(), width: 1600, height: 900, bytes: 240_000, resized: false }, ..att(3, "m3") },
            quote(2, "le README n'est pas prêt"),
            quote(1, "la licence du repo,"),
        ];
        app.ed.set("pour [Quote #1] tu recommande quoi? et pour [Quote #2], voilà : [Image #3] c'est trop long non?", 0);
        let ls: Vec<String> = strip_lines(&app, 91).iter().map(line_text).collect();
        assert!(ls[1].contains("❝ 1  “la licence du repo,”") && ls[1].ends_with("main · 1 line  │"), "{ls:?}");
        assert!(ls[2].contains("❝ 2  “le README n'est pas prêt”"), "{ls:?}");
        assert!(ls[3].contains("▣ 3  readme-dark.png") && ls[3].ends_with("1600×900 · 240 kB  │"), "{ls:?}");
        // the longest row: pill, blank, the 2nd quote's 26 columns, 4, source
        assert_eq!(width(&ls[0]), 6 + 5 + 1 + 26 + 4 + 13);
    }

    /// Narrow: the box takes the full width; a long preview is cut with
    /// `…` next to its source, then the source goes first; the tip
    /// goes before the title.
    #[test]
    fn the_box_when_narrow() {
        let mut app = crate::sb::bench::test_app();
        let deep = "/var/folders/c5/T/TemporaryItems/NSIRD_screencaptureui_iClIqj/Screenshot 2026-09-29 at 09.56.06.png";
        app.attachments = vec![
            Attachment { info: Info { source: deep.into(), width: 1788, height: 542, bytes: 83_000, resized: false }, ..att(1, "m1") },
        ];
        app.ed.set("look [Image #1] ", 0);
        // wide: the file name only, never its path
        let ls: Vec<String> = strip_lines(&app, 91).iter().map(line_text).collect();
        assert!(ls[1].starts_with("│   ▣ 1  Screenshot 2026-09-29 at 09.56.06.png    1788×542 · 83 kB  │"), "{ls:?}");
        assert!(!ls[1].contains('/'), "{ls:?}");
        // 50: the name cut, the size whole, full width
        let ls: Vec<String> = strip_lines(&app, 50).iter().map(line_text).collect();
        assert!(ls.iter().all(|l| width(l) == 50), "{ls:?}");
        assert!(ls[1].starts_with("│   ▣ 1  Screenshot 20") && ls[1].contains('…') && ls[1].ends_with("1788×542 · 83 kB  │"), "{ls:?}");
        // 36: no room for the size: it goes, the name takes the row;
        // the tip goes too, the title stays
        let ls: Vec<String> = strip_lines(&app, 36).iter().map(line_text).collect();
        assert!(ls.iter().all(|l| width(l) == 36), "{ls:?}");
        assert!(ls[1].starts_with("│   ▣ 1  Screenshot 2026-09-29 a…  │"), "{ls:?}");
        assert!(!ls[1].contains("83 kB"), "{ls:?}");
        assert!(ls[0].starts_with("╭─ attached ─"), "{ls:?}");
        assert_eq!(ls[2], format!("╰{}╯", "─".repeat(34)));
        // tiny: nothing panics, the frame stays whole
        for w in 0..20 {
            for l in strip_lines(&app, w) {
                assert_eq!(width(&line_text(&l)), w, "{w}");
            }
        }
        assert_eq!(file_name("shots/a.png"), "a.png");
        assert_eq!(file_name("C:\\shots\\a.png"), "a.png");
        assert_eq!(file_name("clipboard"), "clipboard");
    }

    #[test]
    fn history_chips_sizes_and_results() {
        let dir = std::env::temp_dir().join(format!("bise-i-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a PNG header is enough for its size
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&390u32.to_be_bytes());
        png.extend_from_slice(&844u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        std::fs::write(dir.join("abc.png"), &png).unwrap();
        let b64 = dir.join("abc.b64").to_string_lossy().to_string();
        let mk = |name: &str, path: &str| format!("<image name=\"{name}\" path=\"{path}\" mime=\"image/png\" b64=\"{b64}\">");
        let t = format!("compare {} with {}, off", mk("[Image #1]", "shots/login-mobile.png"), mk("[Image #2]", "/nowhere/x/y.png"));
        let spans = chip_spans(&t, Style::default());
        let s: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(s, "compare ▣ login-mobile.png with ▣ y.png, off");
        assert_eq!(spans[1].style, chip_style());
        assert_eq!(sizes_line(&t).as_deref(), Some("▣ login-mobile.png 390×844 · ▣ y.png 390×844"));
        assert_eq!(sizes_line("plain"), None);
        assert_eq!(chip_spans("plain", Style::default()).len(), 1);
        let r = result_spans(&format!("{}\nok", mk("[Image #1]", "/tmp/screenshot.png"))).unwrap();
        let r: String = r.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(r, "result · ▣ screenshot.png 390×844");
        assert_eq!(without_markers(&format!("a{}b", mk("n", "p"))), "ab");
        assert!(result_spans("no image").is_none());
        // a marker whose image left the store: no size, still a chip
        let gone = t.replace("abc.b64", "zzz.b64");
        assert_eq!(sizes_line(&gone).as_deref(), Some("▣ login-mobile.png · ▣ y.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A quote, a paste and an artifact chip are attachments but not
    /// images: none of them ever had a width, so a model the catalog
    /// lists without vision must not refuse the message.
    #[test]
    fn quotes_pastes_and_artifacts_are_not_images() {
        let mut app = composer("", 0);
        let usage = |m: &str| {
            crate::Ev::Usage(crate::usage::Usage { model: m.into(), input: 10, ..Default::default() })
        };
        app.events.push(usage("mistral/codestral-latest"));
        app.attachments.push(Attachment {
            label: crate::quote::label(1),
            marker: "<selection from=\"main\">q</selection>".into(),
            info: Default::default(),
        });
        app.attachments.push(Attachment {
            label: crate::pasted::label(2),
            marker: "<pasted n=\"2\" lines=\"12\">p</pasted>".into(),
            info: Default::default(),
        });
        let artifact = format!("{ARTIFACT_OPEN}3]");
        app.attachments.push(Attachment {
            label: artifact.clone(),
            marker: "[deck](artifact:d)".into(),
            info: Default::default(),
        });
        app.ed.insert(&format!(
            "{} fix this {} and {artifact}",
            crate::quote::label(1),
            crate::pasted::label(2),
        ));
        assert_eq!(refused_images(&app), None, "a quote, a paste and an artifact are not images");
        // a real image chip still refuses
        app.attachments.push(att(1, "<image name=\"[Image #1]\" b64=\"/x.b64\">"));
        app.ed.insert(" look at [Image #1]");
        assert_eq!(refused_images(&app), Some("mistral/codestral-latest".into()));
    }

    /// An image whose header the store could not read is kept with a
    /// 0x0 size (bend_images::store_bytes): it is still an image, and a
    /// model the catalog lists without vision still refuses it.
    #[test]
    fn an_image_of_unknown_size_is_still_an_image() {
        let mut app = composer("", 0);
        app.events.push(crate::Ev::Usage(crate::usage::Usage {
            model: "mistral/codestral-latest".into(),
            input: 10,
            ..Default::default()
        }));
        app.attachments.push(Attachment { info: Info::default(), ..att(1, "<image name=\"[Image #1]\" b64=\"/x.b64\">") });
        app.ed.insert("look at [Image #1]");
        assert_eq!(refused_images(&app), Some("mistral/codestral-latest".into()));
    }

    #[test]
    fn no_vision_matches_provider_errors_only() {
        for e in [
            "turn failed: the model provider answered 400: Image input is not supported for this model",
            "turn failed: 400 {\"message\":\"Invalid content type. image_url is only supported by certain models.\"}",
            "turn failed: model does not support vision",
            "turn failed: this model doesn't support images",
        ] {
            assert!(is_no_vision(e), "{e}");
        }
        for e in [
            "turn failed: rate limited",
            "image not attached: image too large (9000x9000, 1 bytes) and it could not be downscaled",
            "shots/a.txt: not a PNG, JPEG, GIF or WebP image",
            "unsupported tool call",
        ] {
            assert!(!is_no_vision(e), "{e}");
        }
        set_model("glm-5");
        let l: String = no_vision("turn failed: 400 image input not supported").unwrap().iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(l, "  ✗ glm-5 can't read images. pick a model that can (/model), or describe the screen in words.");
        assert!(no_vision("turn failed: timeout").is_none());
    }
}
