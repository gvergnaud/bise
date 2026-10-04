//! Links in the feed (BISE-211): what the markdown parser found, drawn as
//! OSC 8 hyperlinks and opened by a plain click.
//!
//! A link span carries a tag, 1..=127, in the 7 bits of its
//! `add_modifier` above ratatui's 9 modifiers: the tag survives the wrap
//! (styles are copied per cell), the selection highlight and every frame
//! pass, and it lands in the buffer cells, so a link cell never equals a
//! plain cell that looks the same (the diff redraws it). The crossterm
//! backend ignores these bits. The tag of the event's k-th link is
//! `k % 127 + 1`; the event keeps its urls in order ([`collect`] around
//! the build of its rows), so a tag and the rows give the url back.
//!
//! Each frame the feed says where its visible links are ([`begin_frame`],
//! [`push_hit`]); [`LinkBackend`] wraps the cells the diff writes there in
//! `ESC ]8;id=…;url ESC \` … `ESC ]8;; ESC \`. Only the drawn cells get the
//! escape: widths and the diff are ratatui's own, and a link wrapped on 2
//! rows is 2 pieces with the same id (one hover, one link).

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::cell::RefCell;
use std::rc::Rc;
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

const SHIFT: u16 = 9;
const MASK: u16 = 0x7f << SHIFT;
/// How many tags: 7 bits, 0 is "no link".
const TAGS: usize = 127;

// ---- the tag in a style ----

/// The link tag of a style (0: not a link).
pub(crate) fn tag_of(m: Modifier) -> u8 {
    ((m.bits() & MASK) >> SHIFT) as u8
}

/// `st` tagged as the link `tag` (1..=127).
fn tagged(st: Style, tag: u8) -> Style {
    let mut st = st;
    st.add_modifier = with_tag(st.add_modifier, tag);
    st
}

/// `m` with the link tag `tag` (a cell of the text layer, textlayer.rs).
pub(crate) fn with_tag(m: Modifier, tag: u8) -> Modifier {
    Modifier::from_bits_retain((m.bits() & !MASK) | ((tag as u16 & 0x7f) << SHIFT))
}

/// The look of a link (the designer's call): its label in `fg`,
/// underlined; the underline in the accent where the terminal draws
/// underline colors (SGR 58), a plain underline under `NO_COLOR`.
pub(crate) fn link_style(base: Style, fg: Color, tag: u8) -> Style {
    let mut st = base.fg(fg).add_modifier(Modifier::UNDERLINED);
    if !no_color() {
        st = st.underline_color(crate::theme::accent());
    }
    tagged(st, tag)
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// OSC 8 on (default). `BISE_HYPERLINKS=0` turns it off: a labelled
/// link then shows its url after the label, `label (url)`.
pub(crate) fn osc8() -> bool {
    #[cfg(test)]
    {
        OSC8_OFF.with(|c| !c.get())
    }
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| {
            !std::env::var("BISE_HYPERLINKS").is_ok_and(|v| matches!(v.trim(), "0" | "false" | "no" | "off"))
        })
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static OSC8_OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// ---- which urls: parsing ----

/// A url the TUI links: http(s), mailto, file; printable ASCII or
/// letters, no blank, no control character.
pub(crate) fn linkable(url: &str) -> bool {
    let scheme = ["http://", "https://", "mailto:", "file://"];
    scheme.iter().any(|s| url.len() > s.len() && url[..s.len()].eq_ignore_ascii_case(s))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// A bare url starting at `cs[i]` (`http://` or `https://`, not inside
/// a word): its length in chars, what [`trim_end`] cuts left out. The
/// one url finder of the TUI: the feed (markdown.rs) and the
/// onboarding's links (onboarding.rs) both ask it.
pub(crate) fn bare_at(cs: &[char], i: usize) -> Option<usize> {
    let starts = |p: &str| p.chars().enumerate().all(|(k, c)| cs.get(i + k).is_some_and(|x| x.eq_ignore_ascii_case(&c)));
    let head = if starts("https://") {
        8
    } else if starts("http://") {
        7
    } else {
        return None;
    };
    if i > 0 && (cs[i - 1].is_alphanumeric() || matches!(cs[i - 1], '/' | ':' | '_' | '-' | '.' | '@')) {
        return None;
    }
    let mut end = i;
    while end < cs.len() && !cs[end].is_whitespace() && !cs[end].is_control() && !matches!(cs[end], '<' | '>' | '"' | '`')
    {
        end += 1;
    }
    let end = trim_end(cs, i, end);
    (end - i > head).then_some(end - i)
}

/// `s` is one bare url, whole: nothing before it, nothing [`trim_end`]
/// cuts after it.
pub(crate) fn is_bare_url(s: &str) -> bool {
    let cs: Vec<char> = s.chars().collect();
    bare_at(&cs, 0) == Some(cs.len())
}

/// Where a url or a path met in text ends (BISE-287), like GitHub and
/// Slack: `cs[from..end]` less its trailing sentence punctuation
/// (`. , ; : ! ?`), closing quotes (`" ' ” ’ »`), `>`, markdown's
/// emphasis (`*`, `_`) and a `)` `]` `}` that closes nothing inside it;
/// one that does stays (`https://en.wikipedia.org/wiki/A_(b)`).
pub(crate) fn trim_end(cs: &[char], from: usize, mut end: usize) -> usize {
    while end > from {
        let s = &cs[from..end];
        let closes_nothing = |o: char, c: char| s.iter().filter(|x| **x == c).count() > s.iter().filter(|x| **x == o).count();
        let cut = match cs[end - 1] {
            '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '>' | '*' | '_' | '”' | '’' | '»' => true,
            ')' => closes_nothing('(', ')'),
            ']' => closes_nothing('[', ']'),
            '}' => closes_nothing('{', '}'),
            _ => false,
        };
        if !cut {
            break;
        }
        end -= 1;
    }
    end
}

// ---- the urls of one event ----

thread_local! {
    static COLLECT: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    static FRAME: RefCell<Vec<Hit>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` (the build of one event's rows) and returns the urls its
/// links took, in order: the k-th has the tag `k % 127 + 1`.
pub(crate) fn collect<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
    let saved = COLLECT.with(|c| c.borrow_mut().replace(Vec::new()));
    let out = f();
    let urls = COLLECT.with(|c| std::mem::replace(&mut *c.borrow_mut(), saved)).unwrap_or_default();
    (out, urls)
}

/// A new link: its tag. Outside [`collect`] the link is drawn, but has
/// no url to open.
pub(crate) fn add(url: &str) -> u8 {
    COLLECT.with(|c| match c.borrow_mut().as_mut() {
        Some(v) => {
            v.push(url.to_string());
            ((v.len() - 1) % TAGS + 1) as u8
        }
        None => 1,
    })
}

/// The links of row `row` of an event: (first column, end column, index
/// of the url in `urls`). With at most 127 links the tag is the index;
/// past that, the rows before are read in order (two links in a row
/// never share a tag, so a piece with the tag of the link before
/// continues it).
pub(crate) fn row_links(rows: &[Line], urls: &[String], row: usize) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    if urls.is_empty() || row >= rows.len() {
        return out;
    }
    let direct = urls.len() <= TAGS;
    let mut last: Option<(u8, usize)> = None;
    let from = if direct { row } else { 0 };
    for (r, line) in rows.iter().enumerate().take(row + 1).skip(from) {
        let mut col = 0usize;
        for sp in &line.spans {
            let w = sp.content.width();
            let tag = tag_of(sp.style.add_modifier);
            if tag > 0 {
                let idx = if direct {
                    Some(tag as usize - 1)
                } else {
                    match last {
                        Some((t, k)) if t == tag => Some(k),
                        _ => {
                            let start = last.map_or(0, |(_, k)| k + 1);
                            (start..urls.len()).find(|k| k % TAGS + 1 == tag as usize)
                        }
                    }
                };
                if let Some(k) = idx.filter(|k| *k < urls.len()) {
                    last = Some((tag, k));
                    if r == row {
                        match out.last_mut() {
                            Some((_, e, j)) if *e == col && *j == k => *e = col + w,
                            _ => out.push((col, col + w, k)),
                        }
                    }
                }
            }
            col += w;
        }
    }
    out
}

/// The url under column `col` of row `row` of an event.
pub(crate) fn url_at(rows: &[Line], urls: &[String], row: usize, col: usize) -> Option<String> {
    row_links(rows, urls, row)
        .into_iter()
        .find(|(a, b, _)| col >= *a && col < *b)
        .map(|(_, _, k)| urls[k].clone())
}

// ---- the copy ----

/// The selected rows (`from` on the first, `to` on the last) for the
/// copy: after the last selected cell of each link whose label is not
/// its url, ` (url)`, so the url stays readable once pasted. `rows` go
/// with the index of their event's first row and the event's urls.
/// Returns the rows and the new `to`.
pub(crate) fn with_urls(
    rows: Vec<Line<'static>>,
    events: &[(usize, usize, &[Line<'static>], &[String])],
    from: usize,
    to: usize,
) -> (Vec<Line<'static>>, usize) {
    // events: (rows taken, first row index in the event, the event's rows, urls)
    let n = rows.len();
    // every selected link piece: (row in `rows`, end column of the selected part, url index, event)
    let mut pieces: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut at = 0usize;
    for (e, (count, first, erows, urls)) in events.iter().enumerate() {
        for k in 0..*count {
            let i = at + k;
            let a = if i == 0 { from } else { 0 };
            let b = if i + 1 == n { to } else { usize::MAX };
            for (c0, c1, u) in row_links(erows, urls, first + k) {
                let (s, t) = (c0.max(a), c1.min(b));
                if s < t {
                    pieces.push((i, t, u, e));
                }
            }
        }
        at += count;
    }
    // the last piece of each link occurrence: the next piece is another link
    let mut inserts: Vec<(usize, usize, String)> = Vec::new();
    for (p, &(i, t, u, e)) in pieces.iter().enumerate() {
        let next_same = pieces.get(p + 1).is_some_and(|&(_, _, u2, e2)| u2 == u && e2 == e);
        if next_same {
            continue;
        }
        let url = &events[e].3[u];
        // an artifact's chip copies as `artifacts mock (bise.dev/m/artifacts)`
        if let Some(link) = crate::artifacts::copy_link(url) {
            inserts.push((i, t, format!(" ({})", link)));
            continue;
        }
        if url.starts_with("artifact:") || url.starts_with("bise-") {
            continue;
        }
        let label = link_label(events[e].2, events[e].3, u);
        if label.trim() != url.as_str()
            && label.trim() != url.trim_start_matches("mailto:")
            && !crate::file_links::label_names_file(&label, url)
        {
            inserts.push((i, t, format!(" ({})", url)));
        }
    }
    let mut rows = rows;
    let mut to = to;
    for (i, col, text) in inserts.into_iter().rev() {
        if i + 1 == n && col <= to {
            to = to.saturating_add(text.width());
        }
        insert_at(&mut rows[i], col, text);
    }
    (rows, to)
}

/// The label of the link `u`: the text of all its pieces.
fn link_label(rows: &[Line], urls: &[String], u: usize) -> String {
    let mut s = String::new();
    for r in 0..rows.len() {
        for (a, b, k) in row_links(rows, urls, r) {
            if k == u {
                s.push_str(&crate::feedsel::slice_cols(&crate::feedsel::line_text(&rows[r]), a, b));
            }
        }
    }
    s
}

fn insert_at(line: &mut Line<'static>, col: usize, text: String) {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 1);
    let mut c = 0usize;
    let mut done = false;
    for sp in std::mem::take(&mut line.spans) {
        let w = sp.content.width();
        if !done && col < c + w {
            let left = crate::feedsel::slice_cols(&sp.content, 0, col - c);
            let right = crate::feedsel::slice_cols(&sp.content, col - c, usize::MAX);
            if !left.is_empty() {
                spans.push(Span::styled(left, sp.style));
            }
            spans.push(Span::raw(text.clone()));
            if !right.is_empty() {
                spans.push(Span::styled(right, sp.style));
            }
            done = true;
        } else {
            spans.push(sp);
        }
        c += w;
    }
    if !done {
        spans.push(Span::raw(text));
    }
    line.spans = spans;
}

// ---- the frame: where the links are on screen ----

/// A visible link piece: row, columns [x0, x1), its tag, its url, and
/// the OSC 8 id shared by its pieces.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) y: u16,
    pub(crate) x0: u16,
    pub(crate) x1: u16,
    pub(crate) tag: u8,
    pub(crate) url: String,
    pub(crate) id: String,
}

/// A new frame: no link on screen yet.
pub(crate) fn begin_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

pub(crate) fn push_hit(h: Hit) {
    // BISE-272: the hand over it (the same frame's hit map)
    crate::pointer::region(ratatui::layout::Rect::new(h.x0, h.y, h.x1.saturating_sub(h.x0), 1), crate::pointer::Shape::Pointer);
    FRAME.with(|f| f.borrow_mut().push(h));
}

#[cfg(test)]
pub(crate) fn frame_hits() -> Vec<Hit> {
    FRAME.with(|f| f.borrow().clone())
}

/// The url of the link drawn at `(x, y)` so far this frame (the key bar
/// says what an artifact's chip under the mouse is).
pub(crate) fn hit_url(x: u16, y: u16) -> Option<String> {
    FRAME.with(|f| f.borrow().iter().find(|h| h.y == y && h.x0 <= x && x < h.x1).map(|h| h.url.clone()))
}

/// The frame's links that `gone` says a later text covered (textlayer.rs).
pub(crate) fn drop_hits(gone: impl Fn(&Hit) -> bool) {
    FRAME.with(|f| f.borrow_mut().retain(|h| !gone(h)));
}

/// The OSC 8 opening of `url`: its bytes outside printable ASCII
/// percent-encoded (the spec's 32-126), never an escape.
pub(crate) fn osc8_open(id: &str, url: &str) -> String {
    // a file link's line is the click's (file_links.rs), not the terminal's
    let url = crate::file_links::without_line(url);
    let mut u = String::with_capacity(url.len());
    for b in url.bytes() {
        if (0x21..0x7f).contains(&b) {
            u.push(b as char);
        } else {
            u.push_str(&format!("%{:02X}", b));
        }
    }
    format!("\x1b]8;id={};{}\x1b\\", id, u)
}

pub(crate) const OSC8_CLOSE: &str = "\x1b]8;;\x1b\\";

/// The crossterm backend, with the cells of the frame's links wrapped in
/// OSC 8. No link on screen: the plain backend, nothing more.
pub(crate) struct LinkBackend<W: Write> {
    out: Rc<RefCell<W>>,
    inner: CrosstermBackend<Shared<W>>,
    /// the pointer shape last written (BISE-272; the terminal's own at start)
    pointer: crate::pointer::Shape,
}

/// The one writer, shared by the crossterm backend and the OSC 8 around
/// its cells (crossterm writes through at once: the order holds).
pub(crate) struct Shared<W: Write>(Rc<RefCell<W>>);

impl<W: Write> Write for Shared<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl<W: Write> LinkBackend<W> {
    pub(crate) fn new(w: W) -> Self {
        let out = Rc::new(RefCell::new(w));
        Self { inner: CrosstermBackend::new(Shared(out.clone())), out, pointer: crate::pointer::Shape::Default }
    }

    /// BISE-272: the mouse pointer takes the shape `s` (OSC 22), written
    /// only when it changes and where the terminal has it.
    pub(crate) fn set_pointer(&mut self, s: crate::pointer::Shape) -> io::Result<()> {
        if s == self.pointer || !crate::pointer::enabled() {
            return Ok(());
        }
        let mut out = self.out.borrow_mut();
        out.write_all(crate::pointer::osc22(s).as_bytes())?;
        out.flush()?;
        self.pointer = s;
        crate::pointer::wrote(s);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn take_output(&self) -> W
    where
        W: Default,
    {
        std::mem::take(&mut *self.out.borrow_mut())
    }
}

/// The hit that holds the cell at (x, y), if the cell is still that
/// link's (a popup drawn over a link has no tag).
fn hit_of(hits: &[Hit], x: u16, y: u16, cell: &Cell) -> Option<usize> {
    let tag = tag_of(cell.modifier);
    if tag == 0 {
        return None;
    }
    hits.iter().position(|h| h.y == y && x >= h.x0 && x < h.x1 && h.tag == tag)
}

impl<W: Write> LinkBackend<W> {
    /// The cells, each link inside its OSC 8.
    fn draw_cells<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if !osc8() {
            return self.inner.draw(content);
        }

        FRAME.with(|f| {
            let hits = f.borrow();
            if hits.is_empty() {
                return self.inner.draw(content);
            }
            let cells: Vec<(u16, u16, &Cell)> = content.collect();
            let mut i = 0;
            while i < cells.len() {
                let (x, y, c) = cells[i];
                let h = hit_of(&hits, x, y, c);
                let mut j = i + 1;
                while j < cells.len() {
                    let (x2, y2, c2) = cells[j];
                    let h2 = hit_of(&hits, x2, y2, c2);
                    let same = match (h, h2) {
                        (None, None) => true,
                        (Some(a), Some(b)) => hits[a].url == hits[b].url && hits[a].id == hits[b].id,
                        _ => false,
                    };
                    if !same {
                        break;
                    }
                    j += 1;
                }
                match h {
                    Some(k) => {
                        write!(self.out.borrow_mut(), "{}", osc8_open(&hits[k].id, &hits[k].url))?;
                        self.inner.draw(cells[i..j].iter().copied())?;
                        write!(self.out.borrow_mut(), "{}", OSC8_CLOSE)?;
                    }
                    None => self.inner.draw(cells[i..j].iter().copied())?,
                }
                i = j;
            }
            Ok(())
        })
    }
}

impl<W: Write> Backend for LinkBackend<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // NO_COLOR: crossterm writes every color change as `ESC[;m`, a
        // reset that also drops the modifiers just set (a find match's
        // underline, BISE-237). Colorless cells change no color.
        if no_color() {
            let plain: Vec<(u16, u16, Cell)> = content
                .map(|(x, y, c)| {
                    let mut c = c.clone();
                    c.fg = Color::Reset;
                    c.bg = Color::Reset;
                    (x, y, c)
                })
                .collect();
            return self.draw_cells(plain.iter().map(|(x, y, c)| (*x, *y, c)));
        }
        self.draw_cells(content)
    }
    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }
    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }
    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }
    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }
    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }
    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }
    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }
    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }
    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

/// The terminal the TUI draws on.
pub(crate) type Tui = ratatui::Terminal<LinkBackend<io::Stdout>>;

// ---- the click ----

/// Opens `url` in the default app, detached: `BISE_OPEN` (a command
/// given the url) when set, else `open` (macOS) or `xdg-open`. Only a
/// [`linkable`] url.
pub(crate) fn open(url: &str) -> bool {
    if !linkable(url) {
        return false;
    }
    #[cfg(test)]
    {
        OPENED.with(|o| o.borrow_mut().push(url.to_string()));
        true
    }
    #[cfg(not(test))]
    {
        let cmd = std::env::var("BISE_OPEN").ok().filter(|c| !c.trim().is_empty()).unwrap_or_else(|| {
            if cfg!(target_os = "macos") { "open" } else { "xdg-open" }.to_string()
        });
        std::process::Command::new(cmd)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static OPENED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}
