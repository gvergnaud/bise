//! What the frame's top edge says (book §8 "The frame"): after the logo,
//! where you are (` · ~/lab/acme`, then the task's role line); on the
//! right, the counts and what came new in /artifacts, with who made it
//! and what it is when it fits: `↗ designer · features update`,
//! `↗ 3 new artifacts · designer, art-demo`.
//!
//! Short on room the edge gives up, in order: the role line past
//! [`ROLE_KEEP`] columns, then [`ROLE_MIN`]; the notice keeps its words
//! (who, what) while the path shortens to the repo's name
//! (`~/lab/acme` → `acme`), then the title or the names are cut, then the
//! notice says only `↗ 1 new artifact`; then the path goes and the notice
//! shortens to `↗ 1 new`, `↗ 1`. The counts are never lost: they shorten
//! last (`∿ 3 · ? 1`).

use crate::artifacts::Artifact;
use crate::chrome::fit;
use crate::theme::{self, accent, dim};
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// The role line keeps at least this many columns (its ` · ` included)
/// before the rest shortens for it; under it, it goes (BISE-126).
pub(crate) const ROLE_MIN: usize = 12;
/// What the edge leaves to the role line at most before the path and the
/// notice shorten for it.
pub(crate) const ROLE_KEEP: usize = 27;
/// A cut title or list of names keeps at least this many columns.
const CUT_MIN: usize = 6;

fn width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

fn sep() -> String {
    format!(" {} ", theme::glyph("·"))
}

/// One way to say the notice: its spans, and whether it still says who
/// or what (`rich`: kept over the path's long form).
#[derive(Clone, Debug)]
pub(crate) struct Form {
    pub(crate) spans: Vec<Span<'static>>,
    /// the spans may be cut down to this width (a `…` at the cut)
    pub(crate) cut_to: Option<usize>,
    pub(crate) rich: bool,
}

/// The notice's forms, richest first, for the hub's `new` count and the
/// new rows (newest first). `link` makes a span (the notice is one link:
/// textlayer opens /artifacts). Nothing new: no forms.
pub(crate) fn notice(new: u64, rows: &[Artifact], link: &dyn Fn(String, Style) -> Span<'static>) -> Vec<Form> {
    if new == 0 {
        return Vec::new();
    }
    let st = Style::default().fg(accent());
    let quiet = Style::default().fg(dim());
    let arrow = || link("↗ ".to_string(), st);
    let words = |n: u64| if n == 1 { "1 new artifact".to_string() } else { format!("{} new artifacts", n) };
    let mut out = Vec::new();
    let one = |spans: Vec<Span<'static>>, cut_to: Option<usize>, rich: bool| Form { spans, cut_to, rich };
    let names = who_made(rows);
    if new == 1 {
        if let Some(a) = rows.first() {
            let title = a.title.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut spans = vec![arrow()];
            if let Some(who) = names.first() {
                spans.push(link(who.clone(), st));
                spans.push(link(sep(), quiet));
            }
            let head = width(&spans);
            spans.push(link(title, st));
            out.push(one(spans, Some(head + CUT_MIN), true));
        }
    } else if !names.is_empty() {
        let mut spans = vec![arrow(), link(words(new), st), link(sep(), quiet)];
        let head = width(&spans);
        spans.push(link(names.join(", "), st));
        out.push(one(spans.clone(), None, true));
        if names.len() > 1 {
            spans.pop();
            spans.push(link(format!("{} +{}", names[0], names.len() - 1), st));
            out.push(one(spans, Some(head + CUT_MIN), true));
        }
    }
    out.push(one(vec![arrow(), link(words(new), st)], None, true));
    out.push(one(vec![arrow(), link(format!("{} new", new), st)], None, false));
    out.push(one(vec![link(format!("↗ {}", new), st)], None, false));
    out
}

/// Who made the new rows, newest first, each once: the agent, else who
/// added it (never `you`).
fn who_made(rows: &[Artifact]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in rows {
        let who = if a.agent.is_empty() { a.by.as_str() } else { a.agent.as_str() };
        if !who.is_empty() && who != "you" && who != "page" && !out.iter().any(|w| w == who) {
            out.push(who.to_string());
        }
    }
    out
}

/// The path's forms, richest first: the folder with `~` (and, ctrl held,
/// `flow`: `~/acme · lands via PRs`), then its last name.
pub(crate) fn paths(workspace: &str, flow: &str) -> Vec<String> {
    let full = home_path(workspace);
    if full.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if !flow.is_empty() {
        out.push(format!("{}{}{}", full, sep(), flow));
    }
    out.push(full.clone());
    let last = std::path::Path::new(workspace).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    if !last.is_empty() && last != full {
        out.push(last);
    }
    out
}

/// `path` with the home folder as `~`.
pub(crate) fn home_path(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && (path == h || path.starts_with(&format!("{}/", h))) => format!("~{}", &path[h.len()..]),
        _ => path.to_string(),
    }
}

/// ` · ~/acme`, dim; nothing for no path.
fn path_spans(p: Option<&String>) -> Vec<Span<'static>> {
    match p {
        Some(p) => vec![Span::styled(format!("{}{}", sep(), p), Style::default().fg(dim()))],
        None => Vec::new(),
    }
}

/// What follows the logo on a full screen of bise (/artifacts, the diff
/// under 120 columns) in `room` columns: ` · ~/acme ── artifacts`; the
/// path's shortest form that fits (`acme`, then none), the name always.
pub(crate) fn screen_head(room: usize, paths: &[String], name: &str) -> Vec<Span<'static>> {
    let tail = vec![
        Span::styled(" ── ", crate::chrome::line_style()),
        Span::styled(name.to_string(), Style::default().fg(theme::text())),
    ];
    let tw = width(&tail);
    let path = paths.iter().find(|p| sep().width() + p.width() + tw <= room);
    let mut out = path_spans(path);
    out.extend(tail);
    out
}

/// The role line in `room` columns: cut with `…`, or nothing under
/// ROLE_MIN.
fn fit_role(role: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let w = width(&role);
    if w == 0 || room < ROLE_MIN.min(w) {
        return Vec::new();
    }
    fit(role, room)
}

/// The edge in `room` columns: what goes after the logo (the path, the
/// role line) and what ends the edge (the counts, then the notice).
/// `counts` fits the counts in a room (`usize::MAX`: as they are).
pub(crate) fn lay(
    room: usize,
    paths: &[String],
    role: Vec<Span<'static>>,
    counts: impl Fn(usize) -> Vec<Span<'static>>,
    notice: &[Form],
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let whole = counts(usize::MAX);
    let cw = width(&whole);
    let role_w = width(&role);
    let joined = |mut c: Vec<Span<'static>>, n: Vec<Span<'static>>| {
        if !c.is_empty() && !n.is_empty() {
            c.push(Span::styled(sep(), Style::default().fg(dim())));
        }
        c.extend(n);
        c
    };
    // the ladder: (form, cut, path), the first that fits wins
    let mut ladder: Vec<(Option<&Form>, bool, Option<&String>)> = Vec::new();
    if notice.is_empty() {
        ladder.extend(paths.iter().map(|p| (None, false, Some(p))));
        ladder.push((None, false, None));
    } else {
        for f in notice.iter().filter(|f| f.rich) {
            ladder.extend(paths.iter().map(|p| (Some(f), false, Some(p))));
            if f.cut_to.is_some() {
                ladder.extend(paths.iter().map(|p| (Some(f), true, Some(p))));
            }
        }
        for f in notice {
            ladder.push((Some(f), false, None));
            if f.cut_to.is_some() {
                ladder.push((Some(f), true, None));
            }
        }
    }
    let mut keeps = vec![role_w.min(ROLE_KEEP), ROLE_MIN.min(role_w), 0];
    keeps.dedup();
    for keep in keeps {
        for &(form, cut, path) in &ladder {
            let left = path_spans(path);
            let used = width(&left) + keep + cw + if cw > 0 && form.is_some() { sep().width() } else { 0 };
            let Some(free) = room.checked_sub(used) else { continue };
            let n = match form {
                None => Vec::new(),
                Some(f) if !cut => {
                    if width(&f.spans) > free {
                        continue;
                    }
                    f.spans.clone()
                }
                Some(f) => {
                    if free < f.cut_to.unwrap_or(usize::MAX) || width(&f.spans) <= free {
                        continue;
                    }
                    fit(f.spans.clone(), free)
                }
            };
            let right = joined(whole.clone(), n);
            let mut left = left;
            let rest = room.saturating_sub(width(&left) + width(&right));
            left.extend(fit_role(role.clone(), rest));
            return (left, right);
        }
    }
    // no room for the counts as they are: they shorten, the notice's
    // shortest form after them when it fits
    if let Some(f) = notice.last() {
        let w = width(&f.spans);
        if w + sep().width() + CUT_MIN <= room {
            let c = counts(room - w - sep().width());
            return (Vec::new(), joined(c, f.spans.clone()));
        }
        if w <= room && whole.is_empty() {
            return (Vec::new(), f.spans.clone());
        }
    }
    (Vec::new(), counts(room))
}

#[cfg(test)]
#[path = "topedge_tests.rs"]
mod tests;
