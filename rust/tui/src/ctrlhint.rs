//! Ctrl, option or cmd held: the key hints (book §8 "The frame", §16).
//!
//! While you hold ctrl alone for [`DELAY`], the places where a ctrl
//! shortcut changes something show it: the folds (`▸ 12 more lines` →
//! `▸ ctrl+o expand`), the inbox rows' numbers (in accent: ctrl+N opens
//! row N, BISE-302, card_draw.rs) and the key bar (every ctrl key of the
//! moment); every word the screen leaves out at rest comes back in its
//! place (BISE-303, [`words`]: the divider's `working · 1m` and long
//! context, the panel's state words, the header's counts). Released, or
//! any other key:
//! back at once. A hint only writes over cells the frame already drew
//! (text it replaces, padded with spaces, or the blank cells after a
//! fold's mark): no row or column moves.
//!
//! Option (⌥) held alone the same way (BISE-277): the panel's numbers
//! read `⌥1` (⌥0-9 go to that agent), the panel title `⌥↑↓ select` on an
//! empty composer, the key bar every ⌥ key of the moment. When Option
//! types characters (a layout, or `macos-option-as-alt = false`), ⌥c
//! comes as `ç` without alt: the hints go at that key and it types.
//! Cmd held alone: the panel title `cmd+k find`, the key bar every cmd
//! key; only once a cmd key reached bise this session (`App::cmd_keys`,
//! the terminal passes them), else nothing (cmd alone says nothing).
//!
//! Only a terminal speaking the kitty keyboard protocol reports ctrl
//! (option, cmd) alone (flag 8, "report all keys as escape codes":
//! Ghostty, kitty, WezTerm); with it, the typed
//! text comes as the associated text (flag 16, parsed by our crossterm
//! patch, rust/vendor/crossterm). [`FLAGS`] are pushed at start and kept
//! only when the terminal's `CSI ? u` reply confirms them ([`confirmed`]);
//! else the old flag 1 alone, and no hint ever shows (tmux, most
//! terminals). `BISE_CTRL_HINTS=0` turns it off.

use crate::{theme, App};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, ModifierKeyCode};
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// How long ctrl is held alone before the hints show: a ctrl+x combo
/// never flashes them (a combo's key comes well within 150 ms; BISE-231,
/// was 250 ms, then 80 ms).
pub(crate) const DELAY: Duration = Duration::from_millis(150);

/// The kitty keyboard flags for the hints: disambiguate, event types
/// (press, repeat, release), all keys as escape codes (ctrl alone), and
/// the associated text (what a key types: accents, caps lock).
pub(crate) const FLAGS: KeyboardEnhancementFlags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
    .union(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
    .union(KeyboardEnhancementFlags::REPORT_ASSOCIATED_TEXT);

/// Off by the environment (`BISE_CTRL_HINTS=0`).
pub(crate) fn wanted() -> bool {
    std::env::var("BISE_CTRL_HINTS").map_or(true, |v| v != "0")
}

/// The flags the terminal answered to `CSI ? u` (`ESC [ ? flags u`) in
/// `reply`, if it answered.
pub(crate) fn reply_flags(reply: &[u8]) -> Option<u8> {
    let s = String::from_utf8_lossy(reply);
    let at = s.find("\x1b[?")?;
    let rest = &s[at + 3..];
    let n = rest.find(|c: char| !c.is_ascii_digit())?;
    (rest[n..].starts_with('u') && n > 0).then(|| rest[..n].parse().ok()).flatten()
}

/// The terminal keeps every flag of [`FLAGS`].
pub(crate) fn confirmed(reply: &[u8]) -> bool {
    reply_flags(reply).is_some_and(|f| f & FLAGS.bits() == FLAGS.bits())
}

/// An input event as the handlers take it: a repeat is a press again; a
/// release, a modifier or lock key alone is none (a terminal without the
/// protocol never sends them).
pub(crate) fn for_handlers(ev: Event) -> Option<Event> {
    match ev {
        Event::Key(k) if k.kind == KeyEventKind::Release => None,
        Event::Key(k) if lone(&k) => None,
        Event::Key(k) if k.kind == KeyEventKind::Repeat => Some(Event::Key(KeyEvent { kind: KeyEventKind::Press, ..k })),
        ev => Some(ev),
    }
}

fn lone(k: &KeyEvent) -> bool {
    matches!(k.code, KeyCode::Modifier(_) | KeyCode::CapsLock | KeyCode::NumLock | KeyCode::ScrollLock)
}

/// The modifier whose hints show: ctrl, option (alt), cmd (super).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Held {
    Ctrl,
    Alt,
    Cmd,
}

impl Held {
    /// The modifier key `k` is, alone (left or right).
    fn of(k: &KeyEvent) -> Option<Held> {
        use ModifierKeyCode::*;
        match k.code {
            KeyCode::Modifier(LeftControl | RightControl) => Some(Held::Ctrl),
            KeyCode::Modifier(LeftAlt | RightAlt) => Some(Held::Alt),
            KeyCode::Modifier(LeftSuper | RightSuper) => Some(Held::Cmd),
            _ => None,
        }
    }

    fn bit(self) -> KeyModifiers {
        match self {
            Held::Ctrl => KeyModifiers::CONTROL,
            Held::Alt => KeyModifiers::ALT,
            Held::Cmd => KeyModifiers::SUPER,
        }
    }
}

/// A modifier held alone: which, since when, and whether another key
/// came since.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Hold {
    down: Option<(Held, Instant)>,
    spoiled: bool,
}

impl Hold {
    /// Ctrl held alone since `at` (tests).
    #[cfg(test)]
    pub(crate) fn since(at: Instant) -> Hold {
        Hold::of(Held::Ctrl, at)
    }

    /// `h` held alone since `at` (tests).
    #[cfg(test)]
    pub(crate) fn of(h: Held, at: Instant) -> Hold {
        Hold { down: Some((h, at)), spoiled: false }
    }

    /// Every input event, before the handlers.
    pub(crate) fn event(&mut self, ev: &Event, now: Instant) {
        let held = self.down.map(|(h, _)| h);
        match ev {
            Event::Key(k) => match (Held::of(k), k.kind) {
                (Some(h), KeyEventKind::Press) if held.is_none() => {
                    *self = Hold { down: Some((h, now)), spoiled: k.modifiers.intersects(!h.bit()) };
                }
                (Some(h), KeyEventKind::Release) if held == Some(h) => *self = Hold::default(),
                // the held one again (the terminal repeats it, the other
                // side's key), another one let go: nothing changes
                (Some(h), _) if held == Some(h) => {}
                (Some(_), KeyEventKind::Release) => {}
                // another modifier with it (ctrl+shift, ⌥+cmd): no hints
                (Some(_), _) => self.spoiled = true,
                // a key whose modifiers say the held one is up: its
                // release was lost; or Option typed a character (ç, no
                // alt): the hints go, the key types
                _ if held.is_none_or(|h| !k.modifiers.contains(h.bit())) && !lone(k) => *self = Hold::default(),
                _ if k.kind == KeyEventKind::Release => {}
                _ => self.spoiled = true,
            },
            Event::FocusLost | Event::FocusGained => *self = Hold::default(),
            Event::Resize(..) => {}
            _ => self.spoiled = true,
        }
    }

    /// The hints show at `now` (tests).
    #[cfg(test)]
    pub(crate) fn shown(&self, now: Instant) -> bool {
        self.which(now).is_some()
    }

    /// Whose hints show at `now`.
    pub(crate) fn which(&self, now: Instant) -> Option<Held> {
        let (h, t) = self.down.filter(|_| !self.spoiled)?;
        (now.saturating_duration_since(t) >= DELAY).then_some(h)
    }

    /// How long until they show (the loop wakes then), if they will.
    pub(crate) fn due(&self, now: Instant) -> Option<Duration> {
        let (_, t) = self.down.filter(|_| !self.spoiled)?;
        Some((t + DELAY).saturating_duration_since(now)).filter(|d| !d.is_zero())
    }
}

/// A key and what it does.
pub(crate) type Pair = (&'static str, &'static str);

/// No hints at all: the terminal pane (ctrl goes to the shell), the help,
/// voice at work (its keys are in the bar).
fn quiet(app: &App) -> bool {
    app.term.shown() || app.help.is_some() || app.voice.active()
}

/// The hints show in `app` now.
pub(crate) fn on(app: &App) -> bool {
    held(app).is_some()
}

/// Whose hints show in `app` now: cmd's only once a cmd key reached us
/// (the terminal passes them; cmd alone says nothing).
pub(crate) fn held(app: &App) -> Option<Held> {
    let h = app.hold.which(Instant::now())?;
    (!quiet(app) && (h != Held::Cmd || app.cmd_keys)).then_some(h)
}

/// Ctrl held alone (BISE-303): every word the screen leaves out at rest
/// comes back in its place (the divider's `working · 1m` and long
/// context, the panel's state words, the header's counts).
pub(crate) fn words(app: &App) -> bool {
    held(app) == Some(Held::Ctrl)
}

/// Every key of the held modifier that does something now, for the key
/// bar.
pub(crate) fn pairs(app: &App) -> Vec<Pair> {
    match held(app) {
        Some(Held::Alt) => alt_pairs(app),
        Some(Held::Cmd) => cmd_pairs(app),
        _ => ctrl_pairs(app),
    }
}

/// The ctrl keys (ctrl+f, ctrl+s: cmd+f, cmd+k are cmd's hints).
fn ctrl_pairs(app: &App) -> Vec<Pair> {
    let mut p: Vec<Pair> = Vec::new();
    p.push(if app.pending && !app.interrupt_requested { ("ctrl+c", "interrupt") } else { ("ctrl+c", "quit") });
    // BISE-297: the find box has the keys (find.rs `on_key`)
    if app.find.is_some() {
        p.extend([("ctrl+f", "older"), ("ctrl+w", "delete a word"), ("ctrl+u", "clear")]);
        return p;
    }
    if let Some(w) = fold_word(app) {
        p.push(("ctrl+o", w));
    }
    // BISE-237: find in the history
    if !app.events.is_empty() {
        p.push(("ctrl+f", "find"));
    }
    let v = crate::sb::ctrl_view(app);
    if v.cards > 0 {
        // BISE-302: ctrl+N opens inbox row N (from the thread and the
        // card view); the rest in the card view
        if app.ctrl_digits {
            p.push((open_keys(crate::sb::strip_rows(app)), "open an inbox item"));
        }
        if v.card_open {
            if v.cards > 1 {
                p.push(("ctrl+n/p", "next item"));
            }
            p.push(("ctrl+x", "close without answering"));
        }
    }
    // BISE-265: the agent palette
    if crate::sb::palette::has_agents(app) {
        p.push(("ctrl+s", "find agent"));
    }
    // voice (designer): dictation when it is on; voice mode always (two
    // ctrl+r open it with dictation off too). Before the generic keys
    // below: a full line drops those first
    if app.voice.enabled {
        p.push(("ctrl+r", "dictate"));
    }
    p.push(("ctrl+r twice", "voice mode"));
    // a code block on screen: ctrl+y copies it (codeblock.rs)
    if crate::codeblock::any_on_screen(app) {
        p.push(("ctrl+y", "copy code"));
    }
    p.push(("ctrl+v", "paste image"));
    p.push(("ctrl+j", "newline"));
    p.push(("ctrl+`", "terminal"));
    p.push(("ctrl+l", "clear"));
    p
}

/// `ctrl+1`, `ctrl+1-2` … `ctrl+1-9`: the keys of `rows` inbox rows.
pub(crate) fn open_keys(rows: usize) -> &'static str {
    const KEYS: [&str; 9] = ["ctrl+1", "ctrl+1-2", "ctrl+1-3", "ctrl+1-4", "ctrl+1-5", "ctrl+1-6", "ctrl+1-7", "ctrl+1-8", "ctrl+1-9"];
    KEYS[rows.clamp(1, 9) - 1]
}

/// The ⌥ keys (sb/keys.rs `nav_key`, the editor's word keys).
fn alt_pairs(app: &App) -> Vec<Pair> {
    let mut p: Vec<Pair> = Vec::new();
    if app.find.is_some() {
        p.push(("⌥⌫", "delete a word"));
        return p;
    }
    let v = crate::sb::ctrl_view(app);
    let empty = app.ed.text.is_empty();
    if v.agents > 1 {
        p.push(("⌥0-9", "go to an agent"));
        if empty {
            p.push(("⌥↑↓", "select an agent"));
        }
    }
    if !empty {
        p.push(("⌥←→", "word"));
        p.push(("⌥⌫", "delete a word"));
    }
    p.push(("⌥⏎", "newline"));
    p
}

/// The cmd keys that reach bise (the ones Ghostty keeps by default,
/// cmd+z and cmd+↑↓, only in the help).
fn cmd_pairs(app: &App) -> Vec<Pair> {
    let mut p: Vec<Pair> = Vec::new();
    if app.find.is_some() {
        p.push(("cmd+f", "older"));
        return p;
    }
    let empty = app.ed.text.is_empty();
    if crate::sb::palette::has_agents(app) {
        p.push(("cmd+k", "find agent"));
    }
    if !app.events.is_empty() {
        p.push(("cmd+f", "find"));
    }
    if app.ed.anchor.is_some() && app.ed.selected_text().is_some() {
        p.push(("cmd+c", "copy"));
        p.push(("cmd+x", "cut"));
    } else if app.feed_sel.is_some() {
        p.push(("cmd+c", "copy"));
    }
    if !empty {
        p.push(("cmd+a", "select all"));
        p.push(("cmd+←→", "line start/end"));
        p.push(("cmd+⌫", "delete to line start"));
    }
    p.push(("cmd+v", "paste"));
    p
}

/// What ctrl+o does now: open everything closed, else close what is open.
fn fold_word(app: &App) -> Option<&'static str> {
    if crate::feed::anything_closed(&app.events) {
        Some("expand")
    } else if (0..app.events.len()).any(|i| crate::feed::is_open_at(&app.events, i)) {
        Some("collapse")
    } else {
        None
    }
}

/// `key label` in `w` columns: the label cut (`key label…`), else the
/// key alone, else none; padded with spaces to `w`.
pub(crate) fn fit((k, l): Pair, w: usize) -> Option<String> {
    let full = format!("{k} {l}");
    let s = if full.width() <= w {
        full
    } else if k.width() + 2 + theme::ellipsis().width() <= w {
        let mut s = format!("{k} ");
        for c in l.chars() {
            if s.width() + c.to_string().width() + theme::ellipsis().width() > w {
                break;
            }
            s.push(c);
        }
        s + theme::ellipsis()
    } else if k.width() <= w {
        k.to_string()
    } else {
        return None;
    };
    let pad = w - s.width();
    Some(s + &" ".repeat(pad))
}

/// Write `text` (a hint made by [`fit`]) at `x, y`: the key in accent,
/// the rest dim, one cell per column (the cells after a wide char too).
fn put(buf: &mut Buffer, x: u16, y: u16, key: &str, text: &str) {
    let area = buf.area;
    let mut cx = x;
    let mut used = 0;
    for c in text.chars() {
        let cw = c.to_string().width() as u16;
        if cw == 0 || cx + cw > area.right() || y >= area.bottom() {
            break;
        }
        let st = if used < key.len() { Style::default().fg(theme::accent()) } else { Style::default().fg(theme::dim()) };
        buf[(cx, y)].set_symbol(&c.to_string()).set_style(st);
        for i in 1..cw {
            buf[(cx + i, y)].set_symbol("");
        }
        used += c.len_utf8();
        cx += cw;
    }
}

/// The text of row `y` from column `x0` to `x1` (a wide char's trailing
/// cells are empty strings).
fn row(buf: &Buffer, y: u16, x0: u16, x1: u16) -> Vec<String> {
    (x0..x1).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

/// The hints over the drawn frame (after the one-time hints, before the
/// frame passes): the folds, the panel title. The key bar
/// draws its own ([`pairs`]), the panel its numbers (`⌥1`, sb/panel.rs).
pub(crate) fn draw(app: &App, buf: &mut Buffer) {
    // the find box has the keys: ctrl+o, ⌥↑↓ wait (BISE-297)
    let empty = app.ed.text.is_empty() && app.find.is_none();
    match held(app) {
        Some(Held::Ctrl) if app.find.is_some() => {}
        Some(Held::Ctrl) => folds(app, buf),
        Some(Held::Alt) if empty => panel(app, buf, ("⌥↑↓", "select")),
        Some(Held::Cmd) => panel(app, buf, ("cmd+k", "find")),
        _ => {}
    }
}

/// Each fold in view: `▸ n more lines` becomes `▸ ctrl+o expand`; another
/// mark gets the hint in the blank cells after its row's text.
fn folds(app: &App, buf: &mut Buffer) {
    let Some(word) = fold_word(app) else { return };
    let (mark, want_open) = if word == "expand" { (theme::G_CLOSED, false) } else { (theme::G_OPEN, true) };
    let x0 = app.feed_x;
    let x1 = (x0 as usize + app.area_w).min(buf.area.right() as usize) as u16;
    let mut last: Option<usize> = None;
    for (r, &i) in app.vis_events.iter().enumerate() {
        let y = app.feed_y + r as u16;
        if y >= buf.area.bottom() || i >= app.events.len() || last == Some(i) {
            continue;
        }
        let this = if want_open { crate::feed::is_open_at(&app.events, i) } else { crate::feed::is_closed_at(&app.events, i) };
        if !this {
            continue;
        }
        let cells = row(buf, y, x0, x1);
        let Some(at) = cells.iter().position(|c| c == mark) else { continue };
        last = Some(i);
        let pair = ("ctrl+o", word);
        // `▸ 12 more lines`: the words after the mark are replaced
        let after: String = cells[at + 1..].concat();
        let t = after.trim_start();
        let lead = after.len() - t.len();
        let words = t.split("  ").next().unwrap_or("").trim_end();
        // your folded message has its mark after: `▸ 12 more lines ✓✓`
        let words = more_lines_head(words).unwrap_or(words);
        if more_lines(words) {
            let w = words.width();
            let x = x0 + (at + 1 + lead) as u16;
            if let Some(s) = fit(pair, w) {
                put(buf, x, y, pair.0, &s);
            }
            continue;
        }
        // else: after the row's text, one blank between
        let end = cells.iter().rposition(|c| !c.trim().is_empty()).map_or(0, |e| e + 1);
        let room = cells.len().saturating_sub(end + 1);
        if end > at {
            if let Some(s) = fit(pair, room).filter(|_| room >= pair.0.width()) {
                put(buf, x0 + (end + 1) as u16, y, pair.0, s.trim_end());
            }
        } else if let Some(s) = fit(pair, room) {
            put(buf, x0 + (end + 1) as u16, y, pair.0, s.trim_end());
        }
    }
}

/// `12 more lines ✓✓` (a folded message of yours, BISE-239): its
/// `12 more lines`.
fn more_lines_head(s: &str) -> Option<&str> {
    let end = s.match_indices(' ').nth(2).map(|(i, _)| i)?;
    more_lines(&s[..end]).then(|| &s[..end])
}

/// `12 more lines`, `1 more line` (a closed box's last row).
fn more_lines(s: &str) -> bool {
    let mut w = s.split(' ');
    matches!((w.next(), w.next(), w.next(), w.next()), (Some(n), Some("more"), Some("line" | "lines"), None) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// After the panel title, in its blank cells: ` · ⌥↑↓ select` (they
/// move the selection while the composer is empty) or ` · cmd+k find`.
fn panel(app: &App, buf: &mut Buffer, pair: Pair) {
    if crate::sb::ctrl_view(app).agents < 2 {
        return;
    }
    let (_, Some(p)) = crate::sb::split(buf.area) else { return };
    let title = format!(" {}", crate::sb::PANEL_TITLE);
    for y in p.y..p.bottom().min(buf.area.bottom()) {
        let cells = row(buf, y, p.x, p.right().min(buf.area.right()));
        let text = cells.concat();
        let Some(at) = text.find(&title) else { continue };
        let start = text[..at + title.len()].width();
        // the blank cells after the title, its margin column kept
        if !text[at + title.len()..].trim().is_empty() {
            return;
        }
        let sep = if theme::ascii_mode() { " . " } else { " · " };
        let w = (p.width as usize).saturating_sub(start + sep.width() + 1);
        if let Some(s) = fit(pair, w) {
            let x = p.x + start as u16;
            for (i, c) in sep.chars().enumerate() {
                buf[(x + i as u16, y)].set_symbol(&c.to_string()).set_style(Style::default().fg(theme::faint()));
            }
            put(buf, x + sep.width() as u16, y, pair.0, s.trim_end());
        }
        return;
    }
}

/// The panel's number `n` while option is held: `⌥1` over its ` 1`
/// (⌥1 goes to that agent; ASCII mode: the number alone, in the accent).
pub(crate) fn number(app: &App, n: usize) -> Option<String> {
    (n <= 9 && held(app) == Some(Held::Alt)).then(|| if theme::ascii_mode() { format!(" {n}") } else { format!("⌥{n}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, MouseEvent, MouseEventKind};

    fn ev(code: KeyCode, mods: KeyModifiers, kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent { code, modifiers: mods, kind, state: KeyEventState::NONE })
    }
    fn ctrl(kind: KeyEventKind) -> Event {
        ev(KeyCode::Modifier(ModifierKeyCode::LeftControl), KeyModifiers::CONTROL, kind)
    }

    #[test]
    fn shown_after_the_delay_hidden_at_release_or_any_key() {
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        let mut h = Hold::default();
        h.event(&ctrl(KeyEventKind::Press), t);
        assert!(!h.shown(ms(149)) && h.shown(ms(150)));
        assert_eq!(h.due(ms(30)), Some(Duration::from_millis(120)));
        // the terminal repeats the held modifier: nothing changes
        h.event(&ctrl(KeyEventKind::Repeat), ms(300));
        assert!(h.shown(ms(301)));
        h.event(&ctrl(KeyEventKind::Release), ms(400));
        assert!(!h.shown(ms(401)) && h.due(ms(401)).is_none());
        // ctrl+o: the combo never shows the hints, even held long after
        h.event(&ctrl(KeyEventKind::Press), ms(500));
        h.event(&ev(KeyCode::Char('o'), KeyModifiers::CONTROL, KeyEventKind::Press), ms(550));
        h.event(&ev(KeyCode::Char('o'), KeyModifiers::CONTROL, KeyEventKind::Release), ms(600));
        assert!(!h.shown(ms(2000)) && h.due(ms(2000)).is_none());
        h.event(&ctrl(KeyEventKind::Release), ms(2100));
        // a fast ctrl+o, all within the delay: never shown, before or after
        h.event(&ctrl(KeyEventKind::Press), ms(3000));
        h.event(&ev(KeyCode::Char('o'), KeyModifiers::CONTROL, KeyEventKind::Press), ms(3030));
        assert!(!h.shown(ms(3149)) && !h.shown(ms(3150)));
        h.event(&ctrl(KeyEventKind::Release), ms(3060));
        assert!(!h.shown(ms(4000)) && h.due(ms(4000)).is_none());
        // ctrl+shift, a click, a paste: no hints
        for other in [
            ev(KeyCode::Modifier(ModifierKeyCode::LeftShift), KeyModifiers::CONTROL | KeyModifiers::SHIFT, KeyEventKind::Press),
            Event::Mouse(MouseEvent { kind: MouseEventKind::Moved, column: 0, row: 0, modifiers: KeyModifiers::CONTROL }),
            Event::Paste("x".into()),
        ] {
            let mut h = Hold::default();
            h.event(&ctrl(KeyEventKind::Press), t);
            h.event(&other, ms(10));
            assert!(!h.shown(ms(1000)), "{other:?}");
        }
        // the release was lost (the window changed): the next key without
        // ctrl, or the focus, ends it
        for end in [ev(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press), Event::FocusLost] {
            let mut h = Hold::default();
            h.event(&ctrl(KeyEventKind::Press), t);
            h.event(&end, ms(10));
            assert_eq!(h, Hold::default());
        }
    }

    /// BISE-277: option and cmd alone, as ctrl: shown after the delay,
    /// gone at release, at any other key, or with another modifier.
    #[test]
    fn option_and_cmd_held_alone_show_theirs() {
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        let m = |c, b, kind| ev(KeyCode::Modifier(c), b, kind);
        let (alt, sup) = (KeyModifiers::ALT, KeyModifiers::SUPER);
        for (code, bit, held) in [
            (ModifierKeyCode::LeftAlt, alt, Held::Alt),
            (ModifierKeyCode::RightAlt, alt, Held::Alt),
            (ModifierKeyCode::LeftSuper, sup, Held::Cmd),
            (ModifierKeyCode::RightSuper, sup, Held::Cmd),
        ] {
            let mut h = Hold::default();
            h.event(&m(code, bit, KeyEventKind::Press), t);
            assert_eq!((h.which(ms(149)), h.which(ms(150))), (None, Some(held)));
            h.event(&m(code, bit, KeyEventKind::Repeat), ms(200));
            assert_eq!(h.which(ms(201)), Some(held));
            h.event(&m(code, KeyModifiers::NONE, KeyEventKind::Release), ms(300));
            assert_eq!(h, Hold::default());
        }
        // option with cmd, ctrl with option: none
        let mut h = Hold::default();
        h.event(&m(ModifierKeyCode::LeftAlt, alt, KeyEventKind::Press), t);
        h.event(&m(ModifierKeyCode::LeftSuper, alt | sup, KeyEventKind::Press), ms(10));
        assert!(!h.shown(ms(1000)));
        let mut h = Hold::default();
        h.event(&ctrl(KeyEventKind::Press), t);
        h.event(&m(ModifierKeyCode::LeftAlt, KeyModifiers::CONTROL | alt, KeyEventKind::Press), ms(10));
        h.event(&m(ModifierKeyCode::LeftAlt, KeyModifiers::CONTROL, KeyEventKind::Release), ms(20));
        assert!(!h.shown(ms(1000)));
        // option as alt: ⌥1, ⌥c keep alt: the combo hides them
        for k in [KeyCode::Char('1'), KeyCode::Char('c'), KeyCode::Down] {
            let mut h = Hold::default();
            h.event(&m(ModifierKeyCode::LeftAlt, alt, KeyEventKind::Press), t);
            h.event(&ev(k, alt, KeyEventKind::Press), ms(500));
            assert!(!h.shown(ms(501)), "{k:?}");
        }
        // option typing characters (the crossterm patch drops alt when
        // the text is not the key): ç ends the hold at once
        let mut h = Hold::default();
        h.event(&m(ModifierKeyCode::LeftAlt, alt, KeyEventKind::Press), t);
        assert!(h.shown(ms(500)));
        h.event(&ev(KeyCode::Char('ç'), KeyModifiers::NONE, KeyEventKind::Press), ms(600));
        assert_eq!(h, Hold::default());
        h.event(&m(ModifierKeyCode::LeftAlt, KeyModifiers::NONE, KeyEventKind::Release), ms(700));
        assert_eq!(h, Hold::default());
        // cmd+tab: the window goes, the release never comes
        let mut h = Hold::default();
        h.event(&m(ModifierKeyCode::LeftSuper, sup, KeyEventKind::Press), t);
        h.event(&Event::FocusLost, ms(400));
        assert_eq!(h, Hold::default());
    }

    #[test]
    fn the_handlers_get_presses_and_repeats_only() {
        let a = |kind| ev(KeyCode::Char('a'), KeyModifiers::NONE, kind);
        assert_eq!(for_handlers(a(KeyEventKind::Press)), Some(a(KeyEventKind::Press)));
        // a held backspace / letter repeats as presses
        assert_eq!(for_handlers(a(KeyEventKind::Repeat)), Some(a(KeyEventKind::Press)));
        assert_eq!(for_handlers(a(KeyEventKind::Release)), None);
        assert_eq!(for_handlers(ctrl(KeyEventKind::Press)), None);
        assert_eq!(for_handlers(ev(KeyCode::CapsLock, KeyModifiers::NONE, KeyEventKind::Press)), None);
        assert_eq!(for_handlers(Event::Paste("é".into())), Some(Event::Paste("é".into())));
    }

    #[test]
    fn on_only_when_the_terminal_confirms_flags_8_and_16() {
        let da1 = "\x1b[?62;22c";
        assert_eq!(FLAGS.bits(), 1 | 2 | 8 | 16);
        assert!(confirmed(format!("\x1b[?27u{da1}").as_bytes()));
        assert!(confirmed(format!("\x1b[?31u{da1}").as_bytes()));
        // flag 16 (the text) refused: the accents would be lost, off
        assert!(!confirmed(format!("\x1b[?11u{da1}").as_bytes()));
        assert!(!confirmed(format!("\x1b[?1u{da1}").as_bytes()));
        // no kitty protocol (tmux, Terminal.app): only DA1 comes back
        assert!(!confirmed(da1.as_bytes()));
        assert!(!confirmed(b""));
        assert_eq!(reply_flags(b"\x1b]11;rgb:0/0/0\x07\x1b[?27u"), Some(27));
    }

    #[test]
    fn fit_cuts_the_label_then_keeps_the_key() {
        let p = ("ctrl+o", "expand");
        assert_eq!(fit(p, 15).as_deref(), Some("ctrl+o expand  "));
        assert_eq!(fit(p, 13).as_deref(), Some("ctrl+o expand"));
        assert_eq!(fit(p, 11).as_deref(), Some(format!("ctrl+o exp{}", theme::ellipsis()).as_str()));
        assert_eq!(fit(p, 8).as_deref(), Some("ctrl+o  "));
        assert_eq!(fit(p, 5), None);
        assert!(more_lines("12 more lines") && more_lines("1 more line") && !more_lines("more lines") && !more_lines("12 more lines x"));
        assert_eq!(more_lines_head("12 more lines ✓✓"), Some("12 more lines"));
        assert_eq!(more_lines_head("12 more lines"), None);
        assert_eq!(more_lines_head("12 more things ✓"), None);
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use crate::run::draw_frame;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use serde_json::json;

    fn line(app: &mut App, l: &str) {
        crate::sb::dispatch(app, &json!({"ev": "line", "agent": "main", "line": l}).to_string());
    }

    /// main working, a long bash call (a closed box), two agents, a card.
    pub(super) fn busy_app() -> App {
        let mut app = crate::sb::bench::test_app();
        crate::sb::hub_reads::rows_for_tests::apply(
            &mut app,
            vec![crate::sb::hub_reads::rows_for_tests::agent("main", "working", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")],
            vec![crate::sb::hub_reads::rows_for_tests::card(7, "question", "docs", "v1 or v2?")],
        );
        crate::sb::dispatch(&mut app, &json!({"ev": "ready"}).to_string());
        line(&mut app, "sb you : ship it");
        let cmd: String = (1..=60).map(|i| format!("echo {i}")).collect::<Vec<_>>().join("\n");
        line(&mut app, "  obs: tool_started #4");
        line(&mut app, "tool #4 bash : echo");
        line(&mut app, &format!("tool_code #4 : {}", cmd.replace('\\', "\\\\").replace('\n', "\\N")));
        line(&mut app, "  obs: tool_finished #4 ok");
        // BISE-223: a call is one row; opened, its box (and its
        // `▸ n more lines`) is what the hints cover
        for e in app.events.iter_mut() {
            if let crate::wire::Ev::Tool(td) = e {
                td.opened = true;
            }
        }
        app.cache.iter_mut().for_each(|c| *c = None);
        app.pending = true;
        app
    }

    pub(super) fn screen(app: &mut App, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw_frame(app, f)).unwrap();
        t.backend().buffer().clone()
    }

    pub(super) fn text(b: &Buffer) -> Vec<String> {
        (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>()).collect()
    }

    /// The frame's lines (box and frame borders, joins) by row: where
    /// they are is the layout.
    fn skeleton(b: &Buffer) -> Vec<Vec<(u16, String)>> {
        let lines = ["│", "╭", "╮", "╰", "╯", "├", "┤", "┴", "┬", "┃"];
        (0..b.area.height)
            .map(|y| (0..b.area.width).filter(|&x| lines.contains(&b[(x, y)].symbol())).map(|x| (x, b[(x, y)].symbol().to_string())).collect())
            .collect()
    }

    /// What the last frame placed: the feed rows, the composer.
    fn geometry(app: &App) -> String {
        format!("{:?} {:?} {} {} {:?}", app.vis_events, app.vis_rows, app.feed_x, app.feed_y, app.composer)
    }

    fn row_of(t: &[String], s: &str) -> usize {
        t.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} missing:\n{}", t.join("\n")))
    }

    #[test]
    fn the_hints_replace_cells_and_move_nothing() {
        let mut app = busy_app();
        {
            app.hold = Hold::default();
            let off = screen(&mut app, 120, 40);
            let g_off = geometry(&app);
            app.hold = Hold::since(Instant::now() - Duration::from_secs(1));
            let on = screen(&mut app, 120, 40);
            assert_eq!(geometry(&app), g_off, "the hints moved the layout");
            let (a, b) = (text(&off), text(&on));
            // same cells, same frame and box lines at the same columns
            // (the top edge: its corners; the header's counts may cover
            // the panel's join, as the summary always could)
            assert_eq!(off.area, on.area);
            let (mut s_off, mut s_on) = (skeleton(&off), skeleton(&on));
            for s in [&mut s_off, &mut s_on] {
                s[0].retain(|(_, c)| c != "┬");
            }
            assert_eq!(s_off, s_on, "\n{}\n---\n{}", a.join("\n"), b.join("\n"));
            // the rows that change: the fold, the header's counts, the
            // panel's state words, the divider's words, the key bar,
            // nothing else (BISE-303)
            let fold = row_of(&a, "▸ 46 more lines");
            assert!(b[fold].contains("│ ▸ ctrl+o expand          "), "{}", b[fold]);
            assert!(!a[0].contains("needs you") && b[0].contains("? 1 needs you"), "{}\n{}", a[0], b[0]);
            let main = row_of(&a, " 0 ");
            let docs = row_of(&a, " 1 ? docs   ");
            assert!(b[main].contains(" working ") && b[docs].contains(" you "), "{}\n{}", b[main], b[docs]);
            let div = row_of(&a, "you → main");
            assert!(!a[div].contains("working") && b[div].contains(" working "), "{}\n{}", a[div], b[div]);
            let bar = a.len() - 2;
            assert!(b[bar].contains("ctrl+c interrupt   ctrl+o expand   ctrl+f find   ctrl+1 open an inbox item"), "{}", b[bar]);
            assert!(!b[bar].contains("ctrl+k/j") && !b[bar].contains("ctrl+g"), "{}", b[bar]);
            let mut changed = vec![0, fold, main, docs, div, bar];
            changed.sort();
            let diff: Vec<usize> = (0..a.len()).filter(|&y| a[y] != b[y]).collect();
            assert_eq!(diff, changed, "\n{}\n---\n{}", a.join("\n"), b.join("\n"));
            // the key in accent, what it does dim
            let x = b[fold].find("ctrl+o").map(|i| b[fold][..i].chars().count()).unwrap() as u16;
            assert_eq!(on[(x, fold as u16)].fg, theme::accent());
            assert_eq!(on[(x + 8, fold as u16)].fg, theme::dim());
            // BISE-302: the inbox rows' numbers light up, nothing moves
            // (the strip's row, the panel's)
            let item = |r: &str| r.contains("1 ? docs  v1") || r.contains("1 ? docs · v1");
            for r in (0..a.len()).filter(|&y| item(&a[y])) {
                let x = a[r].find("1 ? docs").map(|i| a[r][..i].chars().count()).unwrap() as u16;
                assert_ne!(off[(x, r as u16)].fg, theme::accent(), "{}", a[r]);
                assert_eq!(on[(x, r as u16)].fg, theme::accent(), "{}", b[r]);
            }
            assert_eq!(a.iter().filter(|r| item(r)).count(), 2, "{}", a.join("\n"));
            // released: the frame as before
            app.hold = Hold::default();
            assert_eq!(text(&screen(&mut app, 120, 40)), a);
        }
    }

    /// A fold whose mark ends its row (a closed thinking section): the
    /// hint in the blank cells after it, one blank between.
    #[test]
    fn a_mark_at_the_end_of_its_row_gets_the_hint_after_it() {
        let mut app = crate::sb::bench::test_app();
        crate::feed::push_event(&mut app.events, &mut app.cache, crate::Ev::Thinking { ms: 3000, text: "hmm".into(), open: false });
        let a = text(&screen(&mut app, 100, 30));
        app.hold = Hold::since(Instant::now() - Duration::from_secs(1));
        let b = text(&screen(&mut app, 100, 30));
        let y = row_of(&a, theme::G_CLOSED);
        let end = a[y].find(theme::G_CLOSED).unwrap() + theme::G_CLOSED.len();
        assert_eq!(&b[y][..end], &a[y][..end], "the row's text stays");
        assert!(b[y][end..].starts_with(" ctrl+o expand"), "{}", b[y]);
        assert_eq!(b[y].chars().count(), a[y].chars().count());
    }

    /// No kitty protocol (tmux, most terminals): no ctrl event ever comes,
    /// the hold never starts, the frame never changes. The terminal pane
    /// and voice at work keep theirs too.
    #[test]
    fn no_support_or_the_terminal_pane_no_hints() {
        let mut app = busy_app();
        let before = text(&screen(&mut app, 120, 40));
        // a legacy terminal's ctrl+o: a press with ctrl, no ctrl alone
        let mut h = Hold::default();
        h.event(&Event::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)), Instant::now());
        assert!(!h.shown(Instant::now() + Duration::from_secs(5)) && h.due(Instant::now()).is_none());
        app.hold = h;
        assert_eq!(text(&screen(&mut app, 120, 40)), before);
    }

    /// The loop's path for one event: the hold, then the handlers.
    fn feed(app: &mut App, evs: &[Event]) {
        for ev in evs {
            app.hold.event(ev, Instant::now());
            if let Some(Event::Key(k)) = for_handlers(ev.clone()) {
                crate::input::on_key(app, &k);
            }
        }
    }

    fn k(code: KeyCode, m: KeyModifiers, kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent { code, modifiers: m, kind, state: crossterm::event::KeyEventState::NONE })
    }

    /// With the flags on, typing gives the same text as before: every key
    /// comes as press + release (and repeats), shift and ctrl alone too;
    /// the text of an accent or caps lock is the key (our crossterm patch,
    /// its tests in rust/vendor/crossterm).
    #[test]
    fn typing_with_the_flags_on_is_unchanged() {
        use KeyEventKind::{Press, Release, Repeat};
        let (n, s, c) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::CONTROL);
        let shift = |kind| k(KeyCode::Modifier(ModifierKeyCode::LeftShift), s, kind);
        let key = |ch: char, m, kind| k(KeyCode::Char(ch), m, kind);
        let tap = |ch: char, m| vec![key(ch, m, Press), key(ch, m, Release)];
        let mut evs = vec![shift(Press)];
        evs.extend(tap('É', s));
        evs.push(shift(Release));
        evs.extend(tap('t', n));
        evs.extend(tap('\u{e9}', n)); // é: a dead key's text (option+e, e)
        evs.extend(tap('\u{e5}', n)); // å: option+a
        evs.push(k(KeyCode::CapsLock, n, Press));
        evs.push(k(KeyCode::CapsLock, n, Release));
        evs.extend(tap('A', s)); // caps lock's capital
        evs.push(key('x', n, Press));
        evs.push(key('x', n, Repeat)); // held: repeats type
        evs.push(key('x', n, Repeat));
        evs.push(key('x', n, Release));
        evs.push(k(KeyCode::Backspace, n, Press));
        evs.push(k(KeyCode::Backspace, n, Release));
        // ctrl alone, then ctrl+a / ctrl+e (line start, end) still act
        evs.push(k(KeyCode::Modifier(ModifierKeyCode::LeftControl), c, Press));
        evs.push(key('a', c, Press));
        evs.push(key('a', c, Release));
        evs.push(k(KeyCode::Modifier(ModifierKeyCode::LeftControl), c, Release));
        evs.extend(tap('>', n));
        let mut app = crate::sb::bench::test_app();
        feed(&mut app, &evs);
        assert_eq!(app.ed.text, ">Ét\u{e9}\u{e5}Axx");
        assert_eq!(app.hold, Hold::default());
    }

    /// Option, then cmd, held: the key bar lists their keys (`panel`
    /// the panel's rows as drawn, the bar the row above the last).
    fn held_screen(app: &mut App, h: Held) -> (Vec<String>, Vec<String>, String) {
        app.hold = Hold::default();
        let a = text(&screen(app, 120, 40));
        let g = geometry(app);
        app.hold = Hold::of(h, Instant::now() - Duration::from_secs(1));
        let b = text(&screen(app, 120, 40));
        assert_eq!(geometry(app), g, "the hints moved the layout");
        app.hold = Hold::default();
        let bar = b[b.len() - 2].clone();
        (a, b, bar)
    }

    /// Two cards, ctrl+1: the first in the card view.
    fn inbox(app: &mut App) {
        crate::sb::hub_reads::rows_for_tests::apply(
            app,
            vec![crate::sb::hub_reads::rows_for_tests::agent("main", "working", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")],
            vec![crate::sb::hub_reads::rows_for_tests::card(7, "question", "docs", "v1 or v2?"), crate::sb::hub_reads::rows_for_tests::card(8, "question", "docs", "ship?")],
        );
        crate::input::on_key(app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL));
        assert!(crate::sb::ctrl_view(app).card_open);
    }

    fn screen_held(app: &mut App, h: Held) -> Buffer {
        screen_held_at(app, h, 120)
    }

    fn screen_held_at(app: &mut App, h: Held, w: u16) -> Buffer {
        app.hold = Hold::of(h, Instant::now() - Duration::from_secs(1));
        let b = screen(app, w, 40);
        app.hold = Hold::default();
        b
    }

    /// Ctrl held: `ctrl+r dictate` when dictation is on, `ctrl+r twice
    /// voice mode` always (designer's words), before the generic keys: a
    /// full line drops those first.
    #[test]
    fn ctrl_held_shows_the_voice_keys() {
        let mut app = busy_app();
        let bar = |app: &mut App, w: u16| {
            let t = text(&screen_held_at(app, Held::Ctrl, w));
            t.into_iter().rev().find(|l| l.contains("ctrl+c")).unwrap_or_default()
        };
        app.voice.enabled = true;
        let p = ctrl_pairs(&app);
        let at = |k: &str| p.iter().position(|x| x.0 == k).unwrap_or(usize::MAX);
        assert!(p.contains(&("ctrl+r", "dictate")) && p.contains(&("ctrl+r twice", "voice mode")), "{p:?}");
        assert!(at("ctrl+r") < at("ctrl+r twice") && at("ctrl+r twice") < at("ctrl+v") && at("ctrl+s") < at("ctrl+r"), "{p:?}");
        let wide = bar(&mut app, 200);
        assert!(wide.contains("ctrl+r dictate") && wide.contains("ctrl+r twice voice mode"), "{wide}");
        // dictation off: voice mode only (two ctrl+r open it all the same)
        app.voice.enabled = false;
        let p = ctrl_pairs(&app);
        assert!(!p.iter().any(|x| x.0 == "ctrl+r") && p.contains(&("ctrl+r twice", "voice mode")), "{p:?}");
        let off = bar(&mut app, 200);
        assert!(!off.contains("dictate") && off.contains("ctrl+r twice voice mode"), "{off}");
    }

    /// BISE-277: option held in the thread: `⌥0 main`, `⌥1 docs` in the
    /// panel, the title `⌥↑↓ select`, the key bar the ⌥ keys; nothing
    /// else changes, nothing moves.
    #[test]
    fn option_held_in_the_thread() {
        let mut app = busy_app();
        let (a, b, bar) = held_screen(&mut app, Held::Alt);
        let main = row_of(&a, " 0 ");
        assert!(b[main].contains("⌥0 "), "{}", b[main]);
        let docs = row_of(&a, "docs");
        assert!(b[docs].contains("⌥1 "), "{}", b[docs]);
        let title = row_of(&a, " agents ");
        assert!(b[title].contains("agents · ⌥↑↓ select"), "{}", b[title]);
        assert!(bar.contains("⌥0-9 go to an agent   ⌥↑↓ select an agent   ⌥⏎ newline"), "{bar}");
        let mut changed = vec![main, docs, title, a.len() - 2];
        changed.sort();
        changed.dedup();
        let diff: Vec<usize> = (0..a.len()).filter(|&y| a[y] != b[y]).collect();
        assert_eq!(diff, changed, "\n{}\n---\n{}", a.join("\n"), b.join("\n"));
        // the key in accent
        let x = b[main].find("⌥0").map(|i| b[main][..i].chars().count()).unwrap() as u16;
        let on = screen_held(&mut app, Held::Alt);
        assert_eq!(on[(x, main as u16)].fg, theme::accent());
        assert_eq!(on[(x + 1, main as u16)].fg, theme::accent());
        // a draft: the word keys, the title alone
        app.ed.insert("ship it");
        let (a, b, bar) = held_screen(&mut app, Held::Alt);
        let title = row_of(&a, " agents ");
        assert_eq!(b[title], a[title]);
        assert!(bar.contains("⌥0-9 go to an agent   ⌥←→ word   ⌥⌫ delete a word   ⌥⏎ newline"), "{bar}");
        assert!(!bar.contains("⌥↑↓"), "{bar}");
    }

    /// Option held in an agent's view and in the card view (ctrl+1): the
    /// key bar the ⌥ keys, which still do their job there.
    #[test]
    fn option_held_in_an_agent_and_the_inbox() {
        let mut app = busy_app();
        crate::sb::focus(&mut app, "docs");
        let (_, b, bar) = held_screen(&mut app, Held::Alt);
        assert!(bar.contains("⌥0-9 go to an agent"), "{bar}");
        assert!(b.iter().any(|r| r.contains("⌥0 ")), "{}", b.join("\n"));
        crate::sb::focus(&mut app, "main");
        inbox(&mut app);
        let (_, _, bar) = held_screen(&mut app, Held::Alt);
        assert!(bar.contains("⌥0-9 go to an agent   ⌥↑↓ select an agent"), "{bar}");
        let (_, _, bar) = held_screen(&mut app, Held::Ctrl);
        assert!(bar.contains("ctrl+1-2 open an inbox item   ctrl+n/p next item"), "{bar}");
        // ⌥1 from the inbox: to docs
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT));
        assert_eq!(app.sb.focus_name(), "docs");
    }

    /// Cmd held: nothing until a cmd key reached us; then the title
    /// `cmd+k find` and the cmd keys in the bar, in the thread, an
    /// agent's view and the inbox.
    #[test]
    fn cmd_held_once_a_cmd_key_came() {
        let mut app = busy_app();
        let (a, b, _) = held_screen(&mut app, Held::Cmd);
        assert_eq!(a, b, "no cmd key yet: nothing");
        app.cmd_keys = true;
        let (a, b, bar) = held_screen(&mut app, Held::Cmd);
        let title = row_of(&a, " agents ");
        assert!(b[title].contains("agents · cmd+k find"), "{}", b[title]);
        assert!(bar.contains("cmd+k find agent   cmd+f find   cmd+v paste"), "{bar}");
        let diff: Vec<usize> = (0..a.len()).filter(|&y| a[y] != b[y]).collect();
        assert_eq!(diff, vec![title, a.len() - 2]);
        app.ed.insert("ship it");
        let (_, _, bar) = held_screen(&mut app, Held::Cmd);
        assert!(bar.contains("cmd+a select all   cmd+←→ line start/end"), "{bar}");
        crate::sb::focus(&mut app, "docs");
        let (_, _, bar) = held_screen(&mut app, Held::Cmd);
        assert!(bar.contains("cmd+k find agent"), "{bar}");
        crate::sb::focus(&mut app, "main");
        inbox(&mut app);
        let (_, _, bar) = held_screen(&mut app, Held::Cmd);
        assert!(bar.contains("cmd+k find agent"), "{bar}");
    }

    /// Option held, then a character: the hints go and it types, both
    /// ways Ghostty sends Option (as alt: ⌥c with alt, the editor's
    /// option layer; composing: `ç` itself, alt dropped by the crossterm
    /// patch).
    #[test]
    fn an_option_character_hides_the_hints_and_types() {
        use KeyEventKind::{Press, Release};
        let (n, a) = (KeyModifiers::NONE, KeyModifiers::ALT);
        let opt = |kind, m| k(KeyCode::Modifier(ModifierKeyCode::LeftAlt), m, kind);
        for (typed, m) in [('c', a), ('ç', n)] {
            let mut app = busy_app();
            let before = text(&screen(&mut app, 120, 40));
            feed(&mut app, &[opt(Press, a)]);
            // held a while: the hints are up
            app.hold = Hold::of(Held::Alt, Instant::now() - Duration::from_secs(1));
            assert!(on(&app));
            assert_ne!(text(&screen(&mut app, 120, 40)), before);
            feed(&mut app, &[k(KeyCode::Char(typed), m, Press)]);
            assert!(!on(&app), "{typed}");
            feed(&mut app, &[k(KeyCode::Char(typed), m, Release), opt(Release, n)]);
            assert_eq!(app.ed.text, "ç");
            assert_eq!(app.hold, Hold::default());
        }
    }
}
