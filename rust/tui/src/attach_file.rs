//! File chips (file-chips: architect m_21764, designer m_21765): any file
//! on disk attached like an image, by its absolute path, never copied. The
//! model reads it with its own tools from the `[deck.pptx](/u/deck.pptx)`
//! link the chip becomes where it was (`bise_proto::parts::file_marker`).
//!
//! - in the composer: the label `[File #N]` (the images', quotes' and
//!   pastes' one number sequence, `attach::next_number`), drawn with
//!   its name `▯ deck.pptx` (designer m_27881, `parts::file_chip_name`);
//!   the strip's row keeps its number `▯ 2  deck.pptx  2.1 MB  ~/Desktop`;
//! - sources ([`on_paste`], [`pick`]): a paste or a drop of paths that are
//!   all files (an image by its extension goes to the image store, any
//!   other file is a file chip; an image the store refuses too); an `@`
//!   pick of any file the same way (`parts::chip_kind`: the one table),
//!   code too, its chip where the `@token` was and the cursor after it
//!   (at-chips);
//! - in the history ([`spans`]): `▯ deck.pptx`, `▯ deck.pptx · missing`
//!   (only `missing` in the error tone) when the hub found no file at send
//!   time; the chip is a link (`open_file::chip_url`): a click previews,
//!   ⌥ click shows it in Finder.

use crate::app::App;
use crate::attach::{self, Attachment, Info};
use crate::theme::{error, glyph, G_FILE};
use bise_proto::parts::{self, ChipKind, Part};
use ratatui::style::Style;
use ratatui::text::Span;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

/// How a file chip's label starts: `[File #2]`.
pub(crate) const OPEN: &str = "[File #";

// the words each file chip shows, by its label (designer m_27881: its
// name, `parts::file_chip_name`), as artifact chips keep their titles
thread_local! {
    static NAMES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The chip `label` stands for the file at `path`: it shows its name.
pub(crate) fn name_chip(label: &str, path: &str) {
    NAMES.with(|t| t.borrow_mut().insert(label.to_string(), parts::file_chip_name(path)));
}

/// The name a file chip shows (None: not a file chip, or not known).
pub(crate) fn chip_words(label: &str) -> Option<String> {
    label.starts_with(OPEN).then(|| NAMES.with(|t| t.borrow().get(label).cloned())).flatten()
}

/// The file labels `[File #N]` in `text` (`attach::chips`).
pub(crate) fn chips(text: &str) -> Vec<(usize, usize, usize)> {
    attach::find_labels(text, OPEN)
}

/// The path a file chip's attachment stands for.
pub(crate) fn path_of(a: &Attachment) -> Option<String> {
    match parts::file_marker_at(&a.marker) {
        Some((Part::File { path, .. }, _)) => Some(path),
        _ => None,
    }
}

/// Attach the file at `path` (absolute) as a chip at the cursor: the chip
/// for the flash (`▯ deck.pptx`).
pub(crate) fn attach_path(app: &mut App, path: &Path) -> String {
    let n = attach::next_number(app);
    let label = parts::file_label(n);
    let abs = path.to_string_lossy().to_string();
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let marker = parts::file_marker(&abs, false);
    name_chip(&label, &abs);
    app.attachments.push(Attachment { label: label.clone(), marker, info: Info { source: abs, bytes, ..Info::default() } });
    attach::insert_chip(&mut app.ed, &label);
    attach::chip_name(&label)
}

/// A paste that is only paths of files (a drag and drop): each attached,
/// an image to the image store, any other file as a file chip. None when
/// the paste is something else (it stays text).
pub(crate) fn on_paste(app: &mut App, text: &str) -> Option<Result<Vec<String>, String>> {
    let paths = bend_images::pasted_paths(text)?;
    if !paths.iter().all(|p| p.is_absolute() && p.is_file()) {
        return None;
    }
    let mut labels = Vec::new();
    for p in &paths {
        labels.push(attach_one(app, p, &p.to_string_lossy()));
    }
    Some(Ok(labels))
}

/// `path` attached: an image chip when it is an image the store takes,
/// else a file chip.
fn attach_one(app: &mut App, path: &Path, shown: &str) -> String {
    if parts::chip_kind(&path.to_string_lossy()) == ChipKind::Image {
        if let Ok(l) = attach::attach_file(app, path, shown) {
            return l;
        }
    }
    attach_path(app, path)
}

/// The picked `@` file attached where the `@token` was, the cursor after
/// its chip (at-chips): an image to the store (a file chip when the store
/// refuses it), any other file, code too, a file chip. Its chip, or None
/// when there is no `@token` or no such file (the pick stays its text).
pub(crate) fn pick(app: &mut App, rel: &str) -> Option<String> {
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
    app.ed.set(&format!("{head}{tail}"), at);
    Some(attach_one(app, &path, rel))
}

/// The strip's right side for a file: `2.1 MB  ~/Desktop` (designer
/// m_21765: no kind label, the name has its extension).
pub(crate) fn strip_source(a: &Attachment) -> String {
    let path = path_of(a).unwrap_or_default();
    let dir = Path::new(&path).parent().map(|d| crate::open_file::tilde(&d.to_string_lossy())).unwrap_or_default();
    format!("{}  {dir}", attach::size_text(a.info.bytes))
}

/// The file chips still in the composer text, by number.
pub(crate) fn shown(app: &App) -> Vec<(usize, &Attachment)> {
    let mut v: Vec<(usize, &Attachment)> = chips(&app.ed.text)
        .into_iter()
        .filter_map(|(_, _, n)| app.attachments.iter().find(|a| a.label == parts::file_label(n)).map(|a| (n, a)))
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v.dedup_by_key(|(n, _)| *n);
    v
}

// ---- the history ----

/// The first file marker in `text`: its byte range, its path and
/// whether the hub found it missing.
pub(crate) fn first_marker(text: &str) -> Option<(usize, usize, String, bool)> {
    match parts::find_file_marker(text)? {
        (at, Part::File { path, missing, .. }, used) => Some((at, at + used, path, missing)),
        _ => None,
    }
}

/// A file chip in the history: `▯ deck.pptx` (accent, a link: a click
/// previews it), ` · missing` after it when the hub found no file there.
pub(crate) fn spans(path: &str, missing: bool) -> Vec<Span<'static>> {
    let tag = crate::links::add(&crate::open_file::chip_url(path));
    let base = attach::chip_style();
    let st = Style { add_modifier: crate::links::with_tag(base.add_modifier, tag), ..base };
    let name = parts::file_name(path).replace(' ', "\u{a0}");
    let mut out = vec![Span::styled(format!("{}\u{a0}{name}", glyph(G_FILE)), st)];
    if missing {
        out.push(Span::styled(" · ".to_string(), Style::default().fg(crate::theme::dim())));
        out.push(Span::styled("missing".to_string(), Style::default().fg(error())));
    }
    out
}

#[cfg(test)]
#[path = "attach_file_tests.rs"]
mod tests;
