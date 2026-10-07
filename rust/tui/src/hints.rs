//! One-time hints (BISE-61, book §15 step 6, contract C4).
//!
//! No tour: each hint shows once per user, next to the thing, the first
//! time it happens: the first agent (left of the panel), the first
//! message between agents in view (under it), the first card (above it).
//! A hint is a small accent-bordered note; it goes away when used (the
//! agent looked into, the fold opened or gone, the card answered) or
//! after the next user message.
//!
//! `hints::once(app, Hint::X)` asks for one (the event handling in
//! `sb.rs` calls it); it is marked seen in the `hints` preference
//! (`{ "first_agent": true, … }`: `bise_home` keeps it in `prefs.json`, or
//! the old `hints.json`) the first time it is really drawn. `SB_ONBOARDING=off` turns the
//! hints off too (the tmux tests). Under `cargo test` they are off unless
//! a test gives a store ([`use_store`]).
//!
//! Where a hint goes is read from the drawn frame (the panel's row `1`,
//! the last fold / level-3 row, the last card title), so the layout code
//! of the other tracks stays as it is.

use crate::theme;
use crate::App;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;
use std::cell::RefCell;
use std::collections::BTreeMap;
use bise_home::Slot;
#[cfg(test)]
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hint {
    FirstAgent,
    FirstLevel3,
    FirstCard,
    /// book §15 (⚠ proposed): for the steering marks (BISE-15, track F)
    FirstSteer,
    /// approvals-design.md §8: the first launch, in yolo
    FirstYolo,
    /// the first switch to auto with a checker that sends data out: what
    /// leaves the machine ([`set_auto_text`])
    FirstAuto,
}

thread_local! {
    static AUTO_TEXT: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
}

/// The words of [`Hint::FirstAuto`] for the checker on (designer, §8):
/// what leaves the machine, named after what `auto` resolves to. Set once
/// before the hint is asked for (it shows once per user).
pub(crate) fn set_auto_text(checker: &str, who: &str) {
    let t = auto_text(checker, who, &bise_catalog::Catalog::builtin());
    AUTO_TEXT.with(|c| c.set(Box::leak(t.into_boxed_str())));
}

/// The tip's words: `who` is the hub's `checker_who` (`TypeSafe`, or the
/// chat model's `provider/id`); a provider without a key runs on this
/// machine (Ollama, LM Studio).
fn auto_text(checker: &str, who: &str, catalog: &bise_catalog::Catalog) -> String {
    if checker == "jev" {
        return format!("auto sends commands to Jev ({who}) to check them. /models changes it.");
    }
    let Some((pid, model)) = bise_catalog::split_name(who) else {
        return format!("auto sends commands to {who} to check them. /models changes it.");
    };
    match catalog.provider(pid) {
        Some(p) if p.key_env.is_empty() => format!("auto checks commands with {model}, on this machine. /models changes it."),
        Some(p) => format!("auto sends commands to {} ({model}) to check them. /models changes it.", p.name),
        None => format!("auto sends commands to {pid} ({model}) to check them. /models changes it."),
    }
}

impl Hint {
    /// Its key in `hints.json`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Hint::FirstAgent => "first_agent",
            Hint::FirstLevel3 => "first_level3",
            Hint::FirstCard => "first_card",
            Hint::FirstSteer => "first_steer",
            Hint::FirstYolo => "first_yolo",
            Hint::FirstAuto => "first_auto",
        }
    }

    /// The text (book §15); `{…}` is a key, in accent.
    pub(crate) fn text(self) -> &'static str {
        match self {
            Hint::FirstAgent => {
                "new: your agents. they work in the background. {⌥ 1} to look inside, {esc} to come back. →"
            }
            Hint::FirstLevel3 => "agents talk to each other. it stays dim: you can ignore it, or {▸} to read.",
            Hint::FirstCard => "[?] <this is your inbox.> when an agent needs you, it waits here instead of interrupting you. {ctrl+1} opens it, or click it. ↓",
            Hint::FirstSteer => "{✓} the agent got it · {✓✓} it read it.",
            Hint::FirstYolo => "you're in yolo: agents run commands without asking. {⇧⇥} changes it.",
            Hint::FirstAuto => AUTO_TEXT.with(|c| c.get()),
        }
    }
}

// ---- the store ----

/// The store: the `hints` preference (none with `SB_ONBOARDING=off`).
pub(crate) fn store() -> Option<Slot> {
    #[cfg(test)]
    {
        STORE.with(|s| s.borrow().clone())
    }
    #[cfg(not(test))]
    {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        if env(crate::onboarding::ENV).is_some_and(|v| matches!(v.trim(), "off" | "0" | "no")) {
            return None;
        }
        Some(crate::onboarding::home_of(&env).pref(bise_home::Pref::Hints))
    }
}

/// The keys marked in a store value.
pub(crate) fn seen_of(v: Option<serde_json::Value>) -> BTreeMap<String, bool> {
    v.and_then(|v| serde_json::from_value::<BTreeMap<String, serde_json::Value>>(v).ok())
        .map(|m| m.into_iter().map(|(k, v)| (k, v.as_bool().unwrap_or(false))).collect())
        .unwrap_or_default()
}

/// The keys marked in a store text.
#[cfg(test)]
pub(crate) fn seen_in(text: &str) -> BTreeMap<String, bool> {
    seen_of(serde_json::from_str(text).ok())
}

/// Mark `key` in the store (other keys kept).
pub(crate) fn mark_in(store: &Slot, key: &str) -> std::io::Result<()> {
    let mut m = seen_of(store.get());
    m.insert(key.to_string(), true);
    store.set(serde_json::to_value(m).unwrap_or_default())
}

// ---- the live hints (UI thread only) ----

#[derive(Default)]
struct State {
    /// the store, read once
    seen: Option<BTreeMap<String, bool>>,
    /// asked for, waiting for their thing to be on screen
    pending: Vec<Hint>,
    /// the one drawn now (marked seen when it came up)
    active: Option<Hint>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    #[cfg(test)]
    static STORE: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

/// Tests: hints on, with this store (per thread).
#[cfg(test)]
pub(crate) fn use_store(p: Option<PathBuf>) {
    STORE.with(|s| *s.borrow_mut() = p.map(Slot::file));
    STATE.with(|s| *s.borrow_mut() = State::default());
}

fn is_seen(st: &mut State, store: &Slot, h: Hint) -> bool {
    let seen = st.seen.get_or_insert_with(|| seen_of(store.get()));
    seen.get(h.key()).copied().unwrap_or(false)
}

/// Ask for hint `h` (contract C4). A hint never seen waits until its
/// thing is on screen and no other hint is up, then shows (and counts as
/// seen). True when it is waiting or up.
pub(crate) fn once(_app: &App, h: Hint) -> bool {
    request(h)
}

fn request(h: Hint) -> bool {
    let Some(path) = store() else { return false };
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if st.active == Some(h) || st.pending.contains(&h) {
            return true;
        }
        if is_seen(&mut st, &path, h) {
            return false;
        }
        // the mode's tip (seen once drawn) gives way to the thing's own
        if matches!(st.active, Some(Hint::FirstYolo | Hint::FirstAuto)) && !matches!(h, Hint::FirstYolo | Hint::FirstAuto) {
            st.active = None;
        }
        st.pending.push(h);
        true
    })
}

/// The hint up now.
#[cfg(test)]
pub(crate) fn active() -> Option<Hint> {
    STATE.with(|s| s.borrow().active)
}

/// `h` was used, or its thing is gone: it goes away (up or waiting).
pub(crate) fn used(h: Hint) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.pending.retain(|p| *p != h);
        if st.active == Some(h) {
            st.active = None;
        }
    });
}

/// The user sent a message: the hint up goes away.
pub(crate) fn user_message() {
    STATE.with(|s| s.borrow_mut().active = None);
}

/// `h` comes up: out of the queue, marked seen in the store.
fn bring_up(h: Hint) {
    let Some(path) = store() else { return };
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.pending.retain(|p| *p != h);
        st.active = Some(h);
        st.seen.get_or_insert_with(BTreeMap::new).insert(h.key().to_string(), true);
        let _ = mark_in(&path, h.key());
    });
}

// ---- drawing ----

/// Inner text width of a hint box (the mockup's 36ch).
const TEXT_W: usize = 36;

/// The words of `text`, wrapped at `w` columns: `{…}` a key (it never
/// splits), `[…]` a glyph in accent (never splits), `<…>` bold words;
/// the rest plain. A mark right after a word (`{⏎}.`) sticks to it. The
/// keys are in accent and the rest in the text color, except the first
/// item's hint (BISE-248): the words dim, the keys in the text color.
pub(crate) fn wrap(text: &str, w: usize) -> Vec<Line<'static>> {
    wrap_in(text, w, Style::default().fg(theme::text()), Style::default().fg(theme::accent()))
}

fn wrap_in(text: &str, w: usize, plain: Style, key: Style) -> Vec<Line<'static>> {
    // (the word, its style, glued to the one before)
    let mut words: Vec<(String, Style, bool)> = Vec::new();
    let mut rest = text;
    let mut glue = false;
    while !rest.is_empty() {
        let at = rest.find(['{', '[', '<']).unwrap_or(rest.len());
        let (head, tail) = rest.split_at(at);
        for (i, wd) in head.split(' ').enumerate() {
            if !wd.is_empty() {
                words.push((wd.to_string(), plain, glue && i == 0));
            }
        }
        glue = !head.is_empty() && !head.ends_with(' ') || head.is_empty() && glue;
        if tail.is_empty() {
            break;
        }
        let close = match tail.as_bytes()[0] {
            b'{' => '}',
            b'[' => ']',
            _ => '>',
        };
        let end = tail.find(close).unwrap_or(tail.len());
        let inner = &tail[1..end];
        match close {
            '}' => words.push((inner.to_string(), key, glue)),
            ']' => words.push((inner.to_string(), Style::default().fg(theme::accent()), glue)),
            _ => {
                let bold = Style::default().fg(theme::text()).add_modifier(Modifier::BOLD);
                for (i, wd) in inner.split(' ').filter(|x| !x.is_empty()).enumerate() {
                    words.push((wd.to_string(), bold, glue && i == 0));
                }
            }
        }
        rest = tail.get(end + 1..).unwrap_or("");
        glue = !rest.starts_with(' ');
    }
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut col = 0usize;
    for (wd, st, glued) in words {
        let ww = wd.width();
        let gap = usize::from(col > 0 && !glued);
        if col > 0 && col + gap + ww > w && !glued {
            lines.push(Vec::new());
            col = 0;
        }
        let line = lines.last_mut().expect("one line");
        if col > 0 && !glued {
            line.push(Span::raw(" "));
            col += 1;
        }
        line.push(Span::styled(wd, st));
        col += ww;
    }
    lines.into_iter().map(Line::from).collect()
}

/// The first item's hint where the terminal sends no ctrl+1-9 (BISE-302,
/// reach.rs).
const FIRST_CARD_CLICK: &str = "[?] <this is your inbox.> when an agent needs you, it waits here instead of interrupting you. click it, or type {/inbox}. ↓";

/// Hint `h`'s lines at `w` columns, in its styles; `digits`: the
/// terminal sends ctrl+1-9.
fn hint_lines(h: Hint, w: usize, digits: bool) -> Vec<Line<'static>> {
    match h {
        Hint::FirstCard => {
            let t = if digits { h.text() } else { FIRST_CARD_CLICK };
            wrap_in(t, w, Style::default().fg(theme::dim()), Style::default().fg(theme::text()))
        }
        _ => wrap(h.text(), w),
    }
}

/// The text of row `y` of `buf` between columns `x0..x1`.
fn row_text(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    (x0..x1).map(|x| buf[(x, y)].symbol()).collect()
}

/// Where hint `h` points at in the drawn frame: the anchor row, if any.
fn anchor(buf: &Buffer, h: Hint, feed: Rect, panel: Option<Rect>) -> Option<u16> {
    let rows = |r: Rect| (r.y..r.bottom()).map(move |y| (y, row_text(buf, y, r.x, r.right())));
    match h {
        Hint::FirstAgent => {
            panel_row(buf, panel?, 1)
        }
        Hint::FirstLevel3 => {
            // a level-3 chip (BISE-106): its envelope then its arrow
            let (env, arrow) = (crate::render::envelope(), theme::glyph("→"));
            let chip = |t: &str| t.find(env).is_some_and(|at| t[at..].contains(arrow));
            rows(feed)
                .filter(|(_, t)| t.contains("messages between") || chip(t))
                .map(|(y, _)| y)
                .next_back()
        }
        Hint::FirstCard => card_row(buf, feed),
        Hint::FirstSteer => {
            let read = theme::glyph(theme::G_READ);
            rows(feed).filter(|(_, t)| t.trim_start().starts_with(theme::glyph(theme::G_YOU)) && t.contains(read)).map(|(y, _)| y).next_back()
        }
        // the divider's mode word (approvals-design.md §8, designer):
        // the box sits right above it, never over your text
        Hint::FirstYolo | Hint::FirstAuto => mode_word(buf, h).map(|(_, y)| y),
    }
}

/// Where the divider says the mode of hint `h` (`you → main · opus 5.5 ·
/// high · yolo`): the word's first column and its row, the lowest such row.
fn mode_word(buf: &Buffer, h: Hint) -> Option<(u16, u16)> {
    let word = if h == Hint::FirstAuto { " · auto" } else { " · yolo" };
    let area = buf.area;
    let arrow = theme::glyph("→");
    (area.y..area.bottom()).rev().find_map(|y| {
        let cells: Vec<&str> = (area.x..area.right()).map(|x| buf[(x, y)].symbol()).collect();
        let t: String = cells.concat();
        if !t.contains(&format!("you {arrow} ")) {
            return None;
        }
        let at = t.find(word)?;
        // the byte offset to a column: count the cells before it
        let mut len = 0;
        let col = cells.iter().position(|c| {
            let here = len >= at;
            len += c.len();
            here
        })?;
        Some((area.x + col as u16 + 3, y))
    })
}

/// The row of the last card title in `feed`, else the inbox's label.
pub(crate) fn card_row(buf: &Buffer, feed: Rect) -> Option<u16> {
    let rows = |r: Rect| (r.y..r.bottom()).map(move |y| (y, row_text(buf, y, r.x, r.right())));
    let title = format!("┃ {} ", theme::glyph(theme::G_CARD));
    rows(feed)
        .filter(|(_, t)| t.contains(&title) && t.contains("needs you"))
        .map(|(y, _)| y)
        .next_back()
        // an item with no row in the history (the setup item,
        // BISE-245): the inbox's label `inbox · 1 waiting for you
        // … ctrl+1 open`
        .or_else(|| rows(feed).filter(|(_, t)| t.contains(" waiting for you ")).map(|(y, _)| y).next_back())
}

/// The row of the panel's entry numbered `n` (`1 name`), if drawn.
pub(crate) fn panel_row(buf: &Buffer, panel: Rect, n: usize) -> Option<u16> {
    let head = format!("{n} ");
    (panel.y..panel.bottom()).find(|&y| row_text(buf, y, panel.x, panel.right()).trim_start_matches(['│', ' ']).starts_with(&head))
}

/// The box of hint `h` next to its anchor row `y` (None: no room).
pub(crate) fn place(h: Hint, y: u16, lines: u16, area: Rect, feed: Rect, panel: Option<Rect>) -> Option<Rect> {
    let bw = (TEXT_W as u16 + 4).min(feed.width.saturating_sub(4));
    let bh = lines + 2;
    if bw < 16 || bh > area.height {
        return None;
    }
    let (x, y) = match h {
        // left of the panel, level with agent 1, the arrow pointing at it
        Hint::FirstAgent => {
            let p = panel?;
            (p.x.checked_sub(bw + 1)?, y.saturating_sub(1).max(area.y))
        }
        // under the line, else above it
        Hint::FirstLevel3 | Hint::FirstSteer => {
            let limit = area.bottom().saturating_sub(4);
            let y = if y + 1 + bh <= limit { y + 1 } else { y.checked_sub(bh)? };
            (feed.x + 4, y)
        }
        // above the card, the arrow pointing down at it
        Hint::FirstCard => (feed.x + 4, y.checked_sub(bh)?.max(area.y)),
        // above the divider's mode word, at the right end ([`draw`]
        // moves it over the word)
        Hint::FirstYolo | Hint::FirstAuto => (area.right().checked_sub(bw + 1)?, y.checked_sub(bh)?.max(area.y)),
    };
    Some(Rect { x, y, width: bw, height: bh }.intersection(area))
}

/// Draw the hint up, else the first waiting one whose thing is on screen
/// (after the frame is drawn: the anchor is read from it). Coming up marks
/// it seen; a hint up whose thing left the screen goes away.
pub(crate) fn draw(f: &mut Frame, digits: bool) {
    let (active, pending) = STATE.with(|s| {
        let st = s.borrow();
        (st.active, st.pending.clone())
    });
    if active.is_none() && pending.is_empty() {
        return;
    }
    let area = f.area();
    let (feed, panel) = crate::sb::split(area);
    let found = match active {
        Some(h) => match anchor(f.buffer_mut(), h, feed, panel) {
            Some(y) => Some((h, y)),
            None => {
                used(h);
                None
            }
        },
        None => pending.into_iter().find_map(|h| anchor(f.buffer_mut(), h, feed, panel).map(|y| (h, y))),
    };
    let Some((h, y)) = found else { return };
    let lines = hint_lines(h, TEXT_W, digits);
    let Some(mut r) = place(h, y, lines.len() as u16, area, feed, panel) else { return };
    // the mode's tip: its left edge 2 columns before the word, as far
    // right as the screen lets it
    if let (Hint::FirstYolo | Hint::FirstAuto, Some((x, _))) = (h, mode_word(f.buffer_mut(), h)) {
        r.x = x.saturating_sub(2).max(area.x).min(r.x);
    }
    draw_box(f, r, lines);
    if active.is_none() {
        bring_up(h);
    }
}

/// A hint's box at `r`: accent rounded border on the card tint.
pub(crate) fn draw_box(f: &mut Frame, r: Rect, lines: Vec<Line<'static>>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .style(Style::default().bg(theme::card_tint()))
        .padding(Padding::horizontal(1));
    f.render_widget(Clear, r);
    crate::pointer::region(r, crate::pointer::Shape::Default); // BISE-272: over what it covers
    let inner = block.inner(r);
    f.render_widget(Paragraph::new(lines).block(block), r);
    crate::textlayer::text(inner); // BISE-290: its text selects, copies and has links
}

/// The demo's tour shows `h`'s lesson its own way (tour.rs): the
/// one-time hint counts as seen and does not come up after it.
pub(crate) fn covered(h: Hint) {
    let Some(path) = store() else { return };
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if is_seen(&mut st, &path, h) {
            return;
        }
        st.pending.retain(|p| *p != h);
        if st.active == Some(h) {
            st.active = None;
        }
        st.seen.get_or_insert_with(BTreeMap::new).insert(h.key().to_string(), true);
        let _ = mark_in(&path, h.key());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// designer: the first-auto tip names what `auto` resolves to and
    /// says what leaves the machine
    #[test]
    fn the_auto_tip_names_the_checker() {
        let c = bise_catalog::Catalog::builtin();
        assert_eq!(
            auto_text("model", "mistral/mistral-small-latest", &c),
            "auto sends commands to Mistral (mistral-small-latest) to check them. /models changes it."
        );
        assert_eq!(auto_text("model", "ollama/qwen3:8b", &c), "auto checks commands with qwen3:8b, on this machine. /models changes it.");
        assert_eq!(auto_text("jev", "TypeSafe", &c), "auto sends commands to Jev (TypeSafe) to check them. /models changes it.");
    }

    /// S36: the window marks seen exactly the TUI's hints, by the same
    /// keys (bise_home prefs::WINDOW_HINTS), so one seen in either never
    /// shows again.
    #[test]
    fn the_window_marks_the_tuis_own_hints() {
        let all = [Hint::FirstAgent, Hint::FirstLevel3, Hint::FirstCard, Hint::FirstSteer, Hint::FirstYolo, Hint::FirstAuto];
        let keys: Vec<&str> = all.iter().map(|h| h.key()).collect();
        assert_eq!(keys, bise_home::prefs::WINDOW_HINTS, "a new Hint: add its key to bise_home's prefs::WINDOW_HINTS too (rust/home/src/prefs.rs), the window's list");
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-hints-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("hints.json")
    }

    fn text_of(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_store_keeps_other_keys() {
        let p = tmp("store");
        assert!(seen_in("junk").is_empty());
        std::fs::write(&p, "{\"other\": true}").unwrap();
        mark_in(&Slot::file(&p), "first_card").unwrap();
        let m = seen_in(&std::fs::read_to_string(&p).unwrap());
        assert_eq!(m.get("other"), Some(&true));
        assert_eq!(m.get("first_card"), Some(&true));
    }

    #[test]
    fn off_without_a_store() {
        use_store(None);
        assert!(!request(Hint::FirstCard));
        assert_eq!(active(), None);
    }

    #[test]
    fn once_then_seen_when_it_comes_up_across_restarts() {
        let p = tmp("once");
        use_store(Some(p.clone()));
        assert!(request(Hint::FirstCard));
        assert!(request(Hint::FirstAgent));
        // waiting, not up: nothing is seen yet
        assert_eq!(active(), None);
        assert!(!p.exists());
        bring_up(Hint::FirstCard);
        assert_eq!(active(), Some(Hint::FirstCard));
        assert!(seen_in(&std::fs::read_to_string(&p).unwrap())["first_card"]);
        assert!(request(Hint::FirstCard));
        user_message();
        assert_eq!(active(), None);
        assert!(!request(Hint::FirstCard));
        // a restart reads the store again; the one never shown can come
        use_store(Some(p.clone()));
        assert!(!request(Hint::FirstCard));
        assert!(request(Hint::FirstAgent));
        used(Hint::FirstAgent);
        assert!(STATE.with(|s| s.borrow().pending.is_empty()));
    }

    #[test]
    fn it_comes_up_when_its_thing_is_drawn_and_goes_with_it() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let p = tmp("draw");
        use_store(Some(p.clone()));
        request(Hint::FirstLevel3);
        request(Hint::FirstCard);
        let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let screen = |t: &Terminal<TestBackend>| {
            let b = t.backend().buffer();
            (0..30).map(|y| row_text(b, y, 0, 120)).collect::<Vec<_>>().join("\n")
        };
        // nothing to point at: nothing shows
        t.draw(|f| draw(f, true)).unwrap();
        assert_eq!(active(), None);
        // a card title in the feed: its hint comes up above it
        let card = format!("  ┃ {} t1 needs you", theme::glyph(theme::G_CARD));
        t.draw(|f| {
            f.render_widget(Paragraph::new(card.as_str()), Rect::new(3, 18, 80, 1));
            draw(f, true)
        })
        .unwrap();
        assert_eq!(active(), Some(Hint::FirstCard));
        let sc = screen(&t);
        assert!(sc.contains("this is your inbox."), "{sc}");
        assert!(seen_in(&std::fs::read_to_string(&p).unwrap())["first_card"]);
        // the card is gone: so is the hint; the level-3 one waits its turn
        t.draw(|f| draw(f, true)).unwrap();
        assert_eq!(active(), None);
        let l3 = format!("   {} t1 → t2  v1 or v2?", crate::render::envelope());
        t.draw(|f| {
            f.render_widget(Paragraph::new(l3.as_str()), Rect::new(3, 5, 80, 1));
            draw(f, true)
        })
        .unwrap();
        assert_eq!(active(), Some(Hint::FirstLevel3));
        assert!(screen(&t).contains("agents talk to each other."));
    }

    #[test]
    fn keys_are_accent_and_lines_fit() {
        let ls = wrap(Hint::FirstAgent.text(), TEXT_W);
        let all: Vec<String> = ls.iter().map(text_of).collect();
        assert_eq!(
            all.join(" "),
            "new: your agents. they work in the background. ⌥ 1 to look inside, esc to come back. →"
        );
        assert!(all.iter().all(|l| l.width() <= TEXT_W), "{all:?}");
        let accent: Vec<&str> = ls
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.fg == Some(theme::accent()))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(accent, vec!["⌥ 1", "esc"]);
        let l3: Vec<String> = wrap(Hint::FirstLevel3.text(), TEXT_W).iter().map(text_of).collect();
        assert_eq!(l3.join(" "), "agents talk to each other. it stays dim: you can ignore it, or ▸ to read.");
        let card = hint_lines(Hint::FirstCard, TEXT_W, true);
        let words: Vec<String> = card.iter().map(text_of).collect();
        assert_eq!(
            words.join(" "),
            "? this is your inbox. when an agent needs you, it waits here instead of interrupting you. ctrl+1 opens it, or click it. ↓"
        );
        assert!(words.iter().all(|l| l.width() <= TEXT_W), "{words:?}");
        // the title bold, the keys in the text color, the rest dim
        let spans: Vec<&Span> = card.iter().flat_map(|l| l.spans.iter()).collect();
        let style_of = |w: &str| spans.iter().find(|s| s.content == w).map(|s| s.style).unwrap();
        assert!(style_of("inbox.").add_modifier.contains(Modifier::BOLD));
        assert_eq!(style_of("ctrl+1").fg, Some(theme::text()));
        assert_eq!(style_of("?").fg, Some(theme::accent()));
        assert_eq!(style_of("waits").fg, Some(theme::dim()));
        // BISE-302: no ctrl+1-9 from the terminal
        let words: Vec<String> = hint_lines(Hint::FirstCard, TEXT_W, false).iter().map(text_of).collect();
        assert!(words.join(" ").ends_with("instead of interrupting you. click it, or type /inbox. ↓"), "{words:?}");
    }

    #[test]
    fn boxes_sit_next_to_their_thing() {
        let area = Rect::new(0, 0, 120, 40);
        let feed = Rect::new(0, 0, 90, 40);
        let panel = Some(Rect::new(90, 0, 30, 40));
        // left of the panel, level with its row
        let r = place(Hint::FirstAgent, 3, 3, area, feed, panel).unwrap();
        assert_eq!((r.right(), r.y, r.height), (89, 2, 5));
        assert_eq!(place(Hint::FirstAgent, 3, 3, area, feed, None), None);
        // under a level-3 row, above it near the bottom
        assert_eq!(place(Hint::FirstLevel3, 10, 3, area, feed, panel).unwrap().y, 11);
        assert_eq!(place(Hint::FirstLevel3, 34, 3, area, feed, panel).unwrap().y, 29);
        // above a card
        assert_eq!(place(Hint::FirstCard, 30, 3, area, feed, panel).unwrap().bottom(), 30);
    }
}
