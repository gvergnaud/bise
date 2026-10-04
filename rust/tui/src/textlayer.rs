//! The text layer (BISE-290): one place for what the mouse does on the
//! text bise draws, whatever draws it. A screen says where its text is
//! ([`text`], one rect per frame: the inbox strip, a card, a popup, the
//! help, the key bar, the panel, the onboarding…) and marks bise's own
//! links in its copy ([`link`]: a short label, the page it opens);
//! [`finish`], at the end of the frame, reads those rects back from the
//! frame and does the rest the same way everywhere:
//!
//! - the links: a full `http(s)://` url or a path to a local file in the
//!   text (`links::bare_at`, `file_links::bare_at`; never a bare domain,
//!   the designer's call) and the marked ones, drawn as links (the feed's
//!   look; on the accent tint the underline takes the text's color), with
//!   their OSC 8 and the hand over them (`links::push_hit`);
//! - the selection ([`TextMouse`]): a drag selects (the feed's selection
//!   tint), a double click the word, a triple click the row, the release
//!   copies (`copied N chars`); a plain click on a link opens it (a file
//!   in your editor, [`open`]); any other plain click is the screen's
//!   own, given back at the release ([`Out::Click`]).
//!
//! The feed, the composer and the terminal panel keep their own text
//! models (they scroll past the screen); they share the rest: the url
//! and path finder ([`spans`]), the opener ([`open`]), the notes
//! ([`copy_note`]), the word and row rules (`feedsel`).

use crate::app::MouseState;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// A link on screen, one row of it: cells [x0, x1) of row y.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Link {
    pub(crate) y: u16,
    pub(crate) x0: u16,
    pub(crate) x1: u16,
    pub(crate) url: String,
    /// the last piece of a link whose label is not its url: the copy
    /// adds ` (url)` after it (as the feed does)
    pub(crate) tail: bool,
}

#[derive(Default)]
struct Frame {
    /// the text rects of this frame, in draw order; `false`: only bise's
    /// own links there, a url in the text stays text ([`text_own`])
    areas: Vec<(Rect, bool)>,
    /// bise's own links of this frame ([`link`]): tag k + 1 is urls[k]
    urls: Vec<String>,
}

/// What the mouse reads: the last frame as drawn.
#[derive(Default)]
struct Last {
    buf: Buffer,
    areas: Vec<Rect>,
    links: Vec<Link>,
}

/// How long a row's links are trusted (a file written meanwhile shows
/// as a link after this).
const FOUND_TTL: Duration = Duration::from_secs(2);

/// A row's links: (first char, end char, url).
type Found = Vec<(usize, usize, String)>;
/// A selection: its text rect, its first and last cells.
type Range = (Rect, (u16, u16), (u16, u16));

thread_local! {
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
    static LAST: RefCell<Last> = RefCell::new(Last::default());
    /// the links found in a row's text, by text: a frame does not look
    /// again at a row it saw
    static FOUND: RefCell<HashMap<String, (Found, Instant)>> = RefCell::new(HashMap::new());
}

// ---- the frame ----

/// A new frame: no text on screen yet.
pub(crate) fn begin_frame() {
    FRAME.with(|f| *f.borrow_mut() = Frame::default());
}

/// `r` holds text: it selects, copies and has links. Drawn over an
/// earlier text rect, it covers it.
pub(crate) fn text(r: Rect) {
    if !r.is_empty() {
        FRAME.with(|f| f.borrow_mut().areas.push((r, true)));
    }
}

/// `r` holds text whose only links are bise's own ([`link`]): a url
/// someone else wrote there stays text (the onboarding quotes a
/// provider's words under bise's own link, BISE-287).
pub(crate) fn text_own(r: Rect) {
    if !r.is_empty() {
        FRAME.with(|f| f.borrow_mut().areas.push((r, false)));
    }
}

/// `label` in `st` as a link to `url`, in bise's own copy (the label a
/// short `console.mistral.ai`, the url the page it opens), for a text
/// rect of this frame.
pub(crate) fn link(label: impl Into<String>, url: &str, st: Style) -> Span<'static> {
    let tag = FRAME.with(|f| {
        let mut f = f.borrow_mut();
        let k = match f.urls.iter().position(|u| u == url) {
            Some(k) => k,
            None => {
                f.urls.push(url.to_string());
                f.urls.len() - 1
            }
        };
        (k % 127 + 1) as u8
    });
    Span::styled(label.into(), crate::links::link_style(st, st.fg.unwrap_or(crate::theme::text()), tag))
}

/// The `[label](url)` links of bise's own copy (a card's words): (start,
/// end) in bytes of each, its label and its url.
fn marked(text: &str) -> Vec<(usize, usize, &str, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(a) = text[from..].find('[').map(|i| from + i) {
        let found = text[a + 1..].find("](").map(|i| a + 1 + i).and_then(|m| {
            let end = text[m + 2..].find(')').map(|i| m + 2 + i)?;
            let (label, url) = (&text[a + 1..m], &text[m + 2..end]);
            (!label.is_empty() && !label.contains('[') && crate::links::linkable(url)).then_some((a, end + 1, label, url))
        });
        match found {
            Some(l) => {
                from = l.1;
                out.push(l);
            }
            None => from = a + 1,
        }
    }
    out
}

/// bise's own copy as spans in `st`: each `[label](url)` its label, a
/// link to its url ([`link`]).
pub(crate) fn copy_spans(text: &str, st: Style) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut at = 0;
    for (a, b, label, url) in marked(text) {
        if a > at {
            out.push(Span::styled(text[at..a].to_string(), st));
        }
        out.push(link(label, url, st));
        at = b;
    }
    if at < text.len() || out.is_empty() {
        out.push(Span::styled(text[at..].to_string(), st));
    }
    out
}

/// bise's own copy as it reads: each `[label](url)` its label (find,
/// completion, a copy).
pub(crate) fn copy_plain(text: &str) -> String {
    let mut s = String::new();
    let mut at = 0;
    for (a, b, label, _) in marked(text) {
        s.push_str(&text[at..a]);
        s.push_str(label);
        at = b;
    }
    s.push_str(&text[at..]);
    s
}

/// Row `y` of `b` from `x0` to `x1` as chars, with the column of each
/// (a wide grapheme's hidden cells skipped).
fn row_chars(b: &Buffer, y: u16, x0: u16, x1: u16) -> (Vec<char>, Vec<u16>) {
    let (mut cs, mut xs) = (Vec::new(), Vec::new());
    let mut x = x0;
    while x < x1 {
        let sym = b[(x, y)].symbol();
        let sym = if sym.is_empty() { " " } else { sym };
        for c in sym.chars() {
            cs.push(c);
            xs.push(x);
        }
        x += sym.width().max(1) as u16;
    }
    (cs, xs)
}

/// The urls and file paths in `cs`: (first char, end char, url).
fn find(cs: &[char]) -> Vec<(usize, usize, String)> {
    let key: String = cs.iter().collect();
    let now = Instant::now();
    if let Some(v) = FOUND.with(|f| f.borrow().get(&key).filter(|(_, t)| now.duration_since(*t) < FOUND_TTL).map(|(v, _)| v.clone())) {
        return v;
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if let Some(n) = crate::links::bare_at(cs, i) {
            out.push((i, i + n, cs[i..i + n].iter().collect()));
            i += n;
        } else if let Some((n, url)) = crate::file_links::bare_at(cs, i) {
            out.push((i, i + n, url));
            i += n;
        } else {
            i += 1;
        }
    }
    FOUND.with(|f| {
        let mut f = f.borrow_mut();
        if f.len() > 4096 {
            f.clear();
        }
        f.insert(key, (out.clone(), now));
    });
    out
}

/// `text` in `st` as spans, each url and path to a file a link of the
/// feed's model (`links::add`): a text a feed row shows as it is (a
/// tool's output in its box).
pub(crate) fn spans(text: &str, st: Style) -> Vec<Span<'static>> {
    let cs: Vec<char> = text.chars().collect();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut at = 0;
    for (a, b, url) in find(&cs) {
        if a > at {
            out.push(Span::styled(cs[at..a].iter().collect::<String>(), st));
        }
        let tag = crate::links::add(&url);
        out.push(Span::styled(cs[a..b].iter().collect::<String>(), crate::links::link_style(st, st.fg.unwrap_or(crate::theme::dim()), tag)));
        at = b;
    }
    if at < cs.len() || out.is_empty() {
        out.push(Span::styled(cs[at..].iter().collect::<String>(), st));
    }
    out
}

/// The text rect on top at (x, y), if any.
fn area_at(areas: &[Rect], x: u16, y: u16) -> Option<usize> {
    areas.iter().rposition(|r| r.contains((x, y).into()))
}

/// The end of the frame: the links of its text rects found, drawn and
/// linked (OSC 8, the hand), the selection `sel` painted, the frame kept
/// for the mouse. Before the frame passes (theme, zen, ascii).
pub(crate) fn finish(buf: &mut Buffer, sel: &TextMouse) {
    let Frame { areas, urls } = FRAME.with(|f| std::mem::take(&mut *f.borrow_mut()));
    let (areas, finds): (Vec<Rect>, Vec<bool>) =
        areas.into_iter().map(|(r, f)| (r.intersection(buf.area), f)).filter(|(r, _)| !r.is_empty()).unzip();
    // the feed's links under a text drawn over it are gone
    crate::links::drop_hits(|h| areas.iter().any(|r| h.y >= r.y && h.y < r.bottom() && h.x1 > r.x && h.x0 < r.right()));
    let mut links: Vec<Link> = Vec::new();
    let mut next = urls.len();
    for (ai, r) in areas.iter().enumerate() {
        for y in r.y..r.bottom() {
            // the runs of this row where this rect is on top
            let mut x = r.x;
            while x < r.right() {
                if area_at(&areas, x, y) != Some(ai) {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < r.right() && area_at(&areas, x, y) == Some(ai) {
                    x += 1;
                }
                row_links(buf, y, start, x, &urls, finds[ai], &mut next, &mut links);
            }
        }
    }
    // the last piece of a marked link, when its label is not its url
    mark_tails(buf, &mut links);
    for (k, l) in links.iter().enumerate() {
        let tag = crate::links::tag_of(buf[(l.x0, l.y)].modifier);
        crate::links::push_hit(crate::links::Hit { y: l.y, x0: l.x0, x1: l.x1, tag, url: l.url.clone(), id: format!("t{}-{}", l.y, k) });
    }
    let last = Last { buf: buf.clone(), areas, links };
    sel.paint(buf, &last);
    LAST.with(|l| *l.borrow_mut() = last);
}

/// The links of row `y`, cells [x0, x1): the marked ones (their tag), the
/// urls and paths found in the rest; drawn as links, kept in `out`.
#[allow(clippy::too_many_arguments)]
fn row_links(buf: &mut Buffer, y: u16, x0: u16, x1: u16, urls: &[String], find_urls: bool, next: &mut usize, out: &mut Vec<Link>) {
    // bise's own: runs of cells with a tag of this frame's urls
    let mut marked: Vec<(u16, u16)> = Vec::new();
    let mut x = x0;
    while x < x1 {
        let tag = crate::links::tag_of(buf[(x, y)].modifier) as usize;
        if tag == 0 || tag > urls.len() {
            x += 1;
            continue;
        }
        let a = x;
        while x < x1 && crate::links::tag_of(buf[(x, y)].modifier) as usize == tag {
            x += 1;
        }
        marked.push((a, x));
        style_cells(buf, y, a, x, tag as u8);
        out.push(Link { y, x0: a, x1: x, url: urls[tag - 1].clone(), tail: false });
    }
    // the urls and paths in the text
    if !find_urls {
        return;
    }
    let (cs, xs) = row_chars(buf, y, x0, x1);
    for (a, b, url) in find(&cs) {
        let (xa, xb) = (xs[a], xs[b - 1] + buf[(xs[b - 1], y)].symbol().width().max(1) as u16);
        if marked.iter().any(|&(m0, m1)| xa < m1 && m0 < xb) {
            continue;
        }
        let tag = (*next % 127 + 1) as u8;
        *next += 1;
        style_cells(buf, y, xa, xb, tag);
        out.push(Link { y, x0: xa, x1: xb, url, tail: false });
    }
}

/// Cells [x0, x1) of row `y` drawn as the link `tag`: underlined, the
/// underline in the accent; on the accent tint (a selected popup row) in
/// the text's own color (the designer's call).
fn style_cells(buf: &mut Buffer, y: u16, x0: u16, x1: u16, tag: u8) {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    for x in x0..x1 {
        let c = &mut buf[(x, y)];
        c.modifier = crate::links::with_tag(c.modifier | Modifier::UNDERLINED, tag);
        c.underline_color = if no_color {
            Color::Reset
        } else if c.bg == crate::theme::accent() {
            c.fg
        } else {
            crate::theme::accent()
        };
    }
}

/// Marks the last piece of each link whose label (all its pieces, rows
/// in order) is not its url.
fn mark_tails(buf: &Buffer, links: &mut [Link]) {
    let mut i = 0;
    while i < links.len() {
        let mut j = i + 1;
        while j < links.len() && links[j].url == links[i].url && links[j].y == links[j - 1].y + 1 {
            j += 1;
        }
        let label: String = links[i..j]
            .iter()
            .map(|l| row_chars(buf, l.y, l.x0, l.x1).0.into_iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("");
        let url = &links[i].url;
        let label = label.trim();
        if label != url.as_str() && label != url.trim_start_matches("mailto:") && !crate::file_links::label_names_file(label, url) {
            links[j - 1].tail = true;
        }
        i = j;
    }
}

// ---- the mouse ----

/// What a mouse event did on the text layer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Out {
    /// not on a text rect: the screen's own
    Pass,
    /// taken (a press, a drag)
    Took,
    /// a selection made: copy it
    Copy(String),
    /// a plain click on a link: open it
    Open(String),
    /// a plain click elsewhere on the text: the screen's own click, this
    /// press then the release
    Click(MouseEvent),
}

#[derive(Clone, Copy, Debug)]
struct Sel {
    /// the text rect the press was in: the selection stays inside it
    area: Rect,
    anchor: (u16, u16),
    head: (u16, u16),
    /// it selects (a drag, a double or a triple click), not clicks
    moved: bool,
    press: MouseEvent,
}

/// The mouse on the text layer: the selection being made or made.
#[derive(Clone, Debug, Default)]
pub(crate) struct TextMouse {
    clicks: MouseState,
    sel: Option<Sel>,
    held: bool,
}

/// The columns [from, to] of `cs` that are text (not the blanks around).
fn text_cols(cs: &[char]) -> Option<(usize, usize)> {
    let from = cs.iter().position(|c| !c.is_whitespace())?;
    let to = cs.iter().rposition(|c| !c.is_whitespace())?;
    Some((from, to))
}

impl TextMouse {
    /// The button is down on the text.
    pub(crate) fn held(&self) -> bool {
        self.held
    }

    /// The selection goes (a key, a new screen).
    pub(crate) fn clear(&mut self) {
        self.sel = None;
        self.held = false;
    }

    /// A selection is shown.
    #[cfg(test)]
    pub(crate) fn selecting(&self) -> bool {
        self.sel.is_some_and(|s| s.moved)
    }

    /// One mouse event, over the last frame.
    pub(crate) fn on(&mut self, m: &MouseEvent, now: Instant) -> Out {
        LAST.with(|l| self.on_frame(m, now, &l.borrow()))
    }

    fn on_frame(&mut self, m: &MouseEvent, now: Instant, last: &Last) -> Out {
        let b = &last.buf;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(ai) = area_at(&last.areas, m.column, m.row) else {
                    self.clear();
                    return Out::Pass;
                };
                let area = last.areas[ai];
                let (x, y) = (m.column, m.row);
                let n = self.clicks.press(x, y, now);
                let (cs, xs) = row_chars(b, y, area.x, area.right());
                let ci = xs.iter().rposition(|&cx| cx <= x).unwrap_or(0);
                let span = match n {
                    2 => {
                        let s: String = cs.iter().collect();
                        let col: usize = cs[..ci].iter().map(|c| c.to_string().width()).sum();
                        let (f, t) = crate::feedsel::word_cols(&s, col);
                        Some((area.x + f as u16, area.x + t as u16))
                    }
                    3 => text_cols(&cs).map(|(f, t)| (xs[f], xs[t])),
                    _ => None,
                };
                self.held = true;
                self.sel = Some(match span {
                    Some((f, t)) => Sel { area, anchor: (f, y), head: (t, y), moved: true, press: *m },
                    None => Sel { area, anchor: (x, y), head: (x, y), moved: false, press: *m },
                });
                Out::Took
            }
            MouseEventKind::Drag(MouseButton::Left) if self.held => {
                if let Some(s) = self.sel.as_mut() {
                    let at = (
                        m.column.clamp(s.area.x, s.area.right().saturating_sub(1)),
                        m.row.clamp(s.area.y, s.area.bottom().saturating_sub(1)),
                    );
                    if s.head != at {
                        s.head = at;
                        s.moved |= s.anchor != at;
                    }
                }
                Out::Took
            }
            MouseEventKind::Up(MouseButton::Left) if self.held => {
                self.held = false;
                let Some(s) = self.sel else { return Out::Took };
                if s.moved {
                    let t = self.text_of(last);
                    return if t.is_empty() { Out::Took } else { Out::Copy(t) };
                }
                self.sel = None;
                let (px, py) = s.anchor;
                match last.links.iter().find(|l| l.y == py && px >= l.x0 && px < l.x1) {
                    Some(l) => Out::Open(l.url.clone()),
                    None => Out::Click(s.press),
                }
            }
            _ => Out::Pass,
        }
    }

    /// The ordered selection, when one is shown.
    fn range(&self) -> Option<Range> {
        let s = self.sel.filter(|s| s.moved)?;
        let (a, b) = if (s.anchor.1, s.anchor.0) <= (s.head.1, s.head.0) { (s.anchor, s.head) } else { (s.head, s.anchor) };
        Some((s.area, a, b))
    }

    /// The selected cells [from, to] of row `y`, text only.
    fn cols(&self, b: &Buffer, y: u16) -> Option<(u16, u16)> {
        let (area, a, z) = self.range()?;
        if y < a.1 || y > z.1 || y >= b.area.bottom() {
            return None;
        }
        let (cs, xs) = row_chars(b, y, area.x, area.right().min(b.area.right()));
        let (t0, t1) = text_cols(&cs)?;
        let (t0, t1) = (xs[t0], xs[t1]);
        let from = if y == a.1 { a.0.max(t0) } else { t0 };
        let to = if y == z.1 { z.0.min(t1) } else { t1 };
        (from <= to).then_some((from, to))
    }

    /// The selected text: its rows trimmed, one per line; after a link
    /// whose label is not its url, ` (url)`.
    fn text_of(&self, last: &Last) -> String {
        let Some((_, a, z)) = self.range() else { return String::new() };
        let b = &last.buf;
        let rows: Vec<String> = (a.1..=z.1)
            .map(|y| {
                let Some((f, t)) = self.cols(b, y) else { return String::new() };
                let mut s = String::new();
                let mut x = f;
                while x <= t {
                    let sym = b[(x, y)].symbol();
                    s.push_str(if sym.is_empty() { " " } else { sym });
                    x += sym.width().max(1) as u16;
                    if let Some(l) = last.links.iter().find(|l| l.tail && l.y == y && l.x1 == x) {
                        s.push_str(&format!(" ({})", l.url));
                    }
                }
                s.trim_end().to_string()
            })
            .collect();
        rows.join("\n").trim_matches('\n').to_string()
    }

    /// The selection on `b`: the feed's tint; reverse video where the
    /// cell's own tint is too close to it to see.
    fn paint(&self, b: &mut Buffer, last: &Last) {
        let Some((_, a, z)) = self.range() else { return };
        let bg = crate::theme::selection_bg();
        for y in a.1..=z.1 {
            let Some((f, t)) = self.cols(&last.buf, y) else { continue };
            for x in f..=t.min(b.area.right().saturating_sub(1)) {
                let c = &mut b[(x, y)];
                if close(c.bg, bg) && c.bg != Color::Reset {
                    c.modifier |= Modifier::REVERSED;
                } else {
                    c.bg = bg;
                }
            }
        }
    }
}

/// Two colors a reader cannot tell apart as tints.
fn close(a: Color, b: Color) -> bool {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            (r1 as i32 - r2 as i32).abs() + (g1 as i32 - g2 as i32).abs() + (b1 as i32 - b2 as i32).abs() < 16
        }
        _ => a == b,
    }
}

// ---- what a click does ----

/// The note after a copy: `copied N chars`.
pub(crate) fn copy_note(text: &str) -> String {
    let n = text.chars().count();
    if crate::clipboard::copy(text) {
        format!("copied {} char{}", n, if n == 1 { "" } else { "s" })
    } else {
        "copy failed (no pbcopy, and the terminal refused OSC 52)".to_string()
    }
}

/// Opens `url` (a click on a link): a local file in your editor
/// (BISE-264), the rest in the default app. The note for the status row.
pub(crate) fn open(app: &mut crate::App, url: &str) -> String {
    // an artifact's chip (site/m/artifacts E): it opens like ⏎ in
    // /artifacts (a PR: its diff)
    if let Some((id, v)) = crate::artifacts::parse_url(url) {
        let Some(a) = crate::artifacts::get(&id) else {
            return format!("no artifact {} (it was removed?)", id);
        };
        if let (None, crate::artifacts::How::Diff(n)) = (v, crate::artifacts::how(&a, None, true)) {
            crate::diffview::request(app, crate::diffview::Ask::Pr(n), crate::diffview::By::Click);
            return format!("PR #{} · its diff", n);
        }
        return crate::artifacts::open(app, &a, v);
    }
    // `± 3 files` under a landed line (site/m/artifacts D): the diff panel
    if let Some(ask) = crate::diffview::ask_of_url(url) {
        // a click (a `± 3 files`): the composer keeps the keys
        crate::diffview::request(app, ask, crate::diffview::By::Click);
        return String::new();
    }
    if url == crate::artifacts_screen::OPEN_URL {
        crate::artifacts_screen::open(app);
        return String::new();
    }
    if let Some(t) = crate::file_links::target_of_url(url) {
        crate::file_links::open(app, &t)
    } else if crate::links::open(url) {
        format!("opening {}", url)
    } else {
        format!("could not open {}", url)
    }
}

/// The links of the last frame (tests).
#[cfg(test)]
pub(crate) fn last_links() -> Vec<Link> {
    LAST.with(|l| l.borrow().links.clone())
}

#[cfg(test)]
#[path = "textlayer_tests.rs"]
mod tests;
