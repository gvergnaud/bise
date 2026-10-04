//! The key bar (BISE-99, book §8 "The frame", §13 "The composer pane").
//!
//! One row under the composer: the keys of the current mode in the text
//! color, what they do dim, 3 spaces between pairs; on the right, ending
//! at the row's last column, a dim tip, one per session, only in the
//! default mode while you don't type and when at least 3 columns separate
//! it from the keys. Track F places the row (BISE-98) and calls [`line`];
//! [`render`] is the pure part. The tip changes every 5 minutes, in the
//! order of [`help::TIPS`] (BISE-104, [`TipClock`]).

use crate::{attach, commands, files, help, theme, voice, App};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// What the key bar shows: one set of keys per mode (the sets of the old
/// hint row, the default one from book §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// the terminal panel is shown: keys go to the shell
    Terminal,
    /// voice: recording
    Recording,
    /// voice: the clip is being transcribed
    Transcribing,
    /// the `@` file popup
    FilePopup,
    /// images are attached to the draft (book §14)
    Images,
    /// text is selected in the history: typing quotes it (BISE-134)
    Quote,
    /// `D` asks before dropping an agent (BISE-43)
    DropAsk,
    /// a yes / no question in the status row
    Confirm,
    /// the card view (ctrl+1-9, a click on an inbox row): the composer
    /// answers the card
    Card,
    /// ctrl+f: the find field (BISE-237)
    Find,
    /// cmd+k / ctrl+s: the agent palette (BISE-265)
    Palette,
    /// an agent is selected in the panel
    Selected,
    /// in an archived agent
    Archived,
    /// switchboard, the agent in view is working
    Steer,
    /// switchboard, idle: the default bar (with the tip)
    Default,
}

/// A key and what it does; an empty key is a plain dim label.
type Pair = (&'static str, &'static str);

/// The way home from an agent's view (book §13, BISE-103): the first
/// pair there, never dropped.
const BACK: Pair = ("esc", "back to main");

/// The inbox from the thread while it holds something (book §12): last
/// at rest, right after `⏎ steer` while the agent works; `ctrl+1 inbox`,
/// a terminal without ctrl+1-9 (BISE-302, reach.rs): `/inbox`.
const INBOX: Pair = ("ctrl+1", "inbox");
const INBOX_COMMAND: Pair = ("/inbox", "");

/// The first pair while text is selected (BISE-134): the one pair of the
/// bar in the accent, so you notice you can just type (book §13).
const ASK: Pair = ("type", "ask about it");

/// expired-ux: the agent in view waits for the ChatGPT sign-in.
pub(crate) const SIGN_IN_AGAIN: Pair = ("⏎", "sign in again");

/// The bar at rest (BISE-303, designer's "less on screen"): the three
/// characters that start something. `⏎ send`, `? help` (? still opens
/// it on an empty composer), `⌥0-9 switch` (the panel's numbers say it)
/// and `ctrl+s find agent` (held ctrl lists it) are gone.
const REST: [Pair; 3] = [("@", "file"), ("$", "skills"), ("/", "commands")];

impl Mode {
    fn pairs(self) -> &'static [Pair] {
        match self {
            Mode::Terminal => &[
                ("", "terminal: keys go to the shell"),
                ("ctrl+`", "hide"),
                ("wheel/shift+pgup", "scroll"),
                ("drag", "select"),
                ("drag the border", "resize"),
            ],
            // BISE-222: the chip in the text says recording / transcribing
            Mode::Recording => &[("any key", "stop"), ("esc", "cancel")],
            Mode::Transcribing => &[("esc", "cancel")],
            Mode::FilePopup => &[
                ("⏎/tab", "insert"),
                ("⏎/tab/→", "open a folder"),
                ("←", "up"),
                ("↑↓", "select"),
                ("esc", "close"),
            ],
            // the image key, then the default keys (BISE-108: the bar
            // once lost its keys with an image)
            Mode::Images => &[("ctrl+v", "paste image"), REST[0], REST[1], REST[2]],
            Mode::Quote => &[ASK, ("cmd+c", "copy"), ("esc", "drop")],
            Mode::DropAsk => &[("y", "archive"), ("n or esc", "keep")],
            Mode::Confirm => &[("y", "yes"), ("n", "no"), ("esc", "cancel")],
            Mode::Find => &[("⏎", "older"), ("shift+⏎", "newer"), ("esc", "close")],
            Mode::Palette => &[("↑↓", "choose"), ("⏎", "open"), ("esc", "close")],
            // the view's own keys come from `sb::card_key_pairs` (`1-2 pick`…)
            Mode::Card => &[("⏎", "answer"), ("ctrl+x", "close"), ("esc", "back")],
            Mode::Selected => &[("⏎", "enter"), ("space", "preview"), ("D", "archive"), ("esc", "close")],
            // the placeholder says /restore (designer: once)
            Mode::Archived => &[BACK],
            Mode::Steer => &[("tab", "queue"), ("⏎", "steer"), ("ctrl+c", "interrupt")],
            Mode::Default => &REST,
        }
    }

    /// The keys of this mode in an agent's view (book §13 "Key bar in an
    /// agent's view"): `esc back to main` first; working, the book's set
    /// `esc back to main   tab queue   ⏎ steer   ctrl+c interrupt` (tab
    /// and ⏎ only with text in the composer, [`render_with`]).
    fn agent_pairs(self) -> Vec<Pair> {
        match self {
            Mode::Default | Mode::Images => std::iter::once(BACK).chain(self.pairs().iter().copied()).collect(),
            Mode::Steer => vec![BACK, ("tab", "queue"), ("⏎", "steer"), ("ctrl+c", "interrupt")],
            m => m.pairs().to_vec(),
        }
    }
}

/// The mode of `app` (the order of the old hint row).
pub(crate) fn mode(app: &App) -> Mode {
    if app.term.shown() {
        Mode::Terminal
    } else if app.voice.state() == voice::VoiceState::Recording {
        Mode::Recording
    } else if app.voice.state() == voice::VoiceState::Flushing {
        Mode::Transcribing
    } else if crate::sb::palette::is_open(app) {
        Mode::Palette
    } else if app.find.is_some() {
        Mode::Find
    } else if commands::popup_open(app) && files::token(&app.ed.text, app.ed.cursor).is_some() {
        Mode::FilePopup
    } else if app.feed_sel.is_some() && app.mouse.drag.is_none() {
        Mode::Quote
    } else if !app.pending && attach::strip_height(app) > 0 {
        Mode::Images
    } else {
        crate::sb::key_mode(app)
    }
}

/// How long the key bar says a switch of mode.
const FLASH: Duration = Duration::from_secs(3);

/// The flash of a switch (designer, §8): the mode word, then its sentence.
pub(crate) fn flash_words(mode: &str, checker: &str, env: bool) -> (String, String) {
    let what = match mode {
        "auto" if checker == "off" => "edits run, commands ask you",
        "auto" => "safe calls run, risky ones ask you",
        _ => "everything runs, nothing asks",
    };
    let only = if env { " (this session only: BISE_APPROVALS)" } else { "" };
    (mode.to_string(), format!(" · {what}{only}"))
}

/// A switch of mode less than 3 s ago: the key bar says it, the
/// divider's mode word is in accent (designer, approvals-design.md §8).
pub(crate) fn flashing(a: &crate::sb::Approvals) -> bool {
    a.flash.is_some_and(|t| t.elapsed() < FLASH)
}

/// The key bar of `app` for a row `width` columns wide: the mode's keys
/// and the tip of the moment (none in an agent's view); for 3 s after a
/// switch of approvals mode, the switch's line (the mode itself is on the
/// divider, after the model: chrome.rs [`crate::chrome::Who`]).
pub(crate) fn line(app: &App, width: u16) -> Line<'static> {
    let dimmed = Style::default().fg(theme::dim());
    // the diff panel has the keys (site/m/artifacts D)
    if app.diff.as_ref().is_some_and(|p| p.focused) {
        let pairs = crate::diffview::key_pairs(app);
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, w)| (*k, w.as_str())).collect();
        return pairs_line(&pairs, usize::from(width)).0;
    }
    // an artifact's chip under the mouse (site/m/artifacts E): what it is
    if let Some(words) = app
        .hover
        .and_then(|(x, y)| crate::links::hit_url(x, y))
        .and_then(|u| crate::artifacts::hover_words(&u, crate::when::now_ms()))
    {
        return Line::from(Span::styled(cut(&words, usize::from(width)), dimmed));
    }
    // a draft with an artifact's chip: how it goes
    if crate::attach::has_artifact(&app.ed.text) && !app.ed.text.trim().is_empty() {
        let pairs = [("⏎", "send"), ("", "the ↗ chips go as the artifact's link and title, never as @name")];
        return pairs_line(&pairs, usize::from(width)).0;
    }
    let a = &app.sb.approvals;
    let dim = Style::default().fg(theme::dim());
    if flashing(a) {
        let (word, rest) = flash_words(a.word(), &a.checker, a.env);
        let w = usize::from(width);
        let rest = cut(&rest, w.saturating_sub(word.width()));
        return Line::from(vec![Span::styled(word, Style::default().fg(theme::accent())), Span::styled(rest, dim)]);
    }
    keys_line(app, width)
}

fn keys_line(app: &App, width: u16) -> Line<'static> {
    let agent = !app.sb.is_main_focus();
    let typing = !app.ed.text.is_empty();
    // ctrl, option or cmd held (ctrlhint.rs): every key of that modifier
    // of the moment, same row
    if crate::ctrlhint::on(app)
        && matches!(mode(app), Mode::Default | Mode::Steer | Mode::Selected | Mode::Archived | Mode::Images | Mode::Quote | Mode::Card)
    {
        return pairs_line(&crate::ctrlhint::pairs(app), usize::from(width)).0;
    }
    if mode(app) == Mode::Card {
        let pairs = crate::sb::card_key_pairs(app);
        let pairs = crate::sb::fit_card_pairs(pairs, usize::from(width));
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, l)| (*k, l.as_str())).collect();
        return pairs_line(&pairs, usize::from(width)).0;
    }
    let inbox = (crate::sb::ctrl_view(app).cards > 0).then_some(if app.ctrl_digits { INBOX } else { INBOX_COMMAND });
    // expired-ux: the sign-in again waits for the browser (the first
    // run's words, designer m_7456)
    if let Some(r) = app.resign.as_ref() {
        let copy = if r.just_copied() { ("c", "copied") } else { ("c", "copy the link") };
        let pairs = [("", crate::resign::WAITING), copy, ("esc", "cancel")];
        return pairs_line(&pairs, usize::from(width)).0;
    }
    // expired-ux: the agent in view stopped on the expired ChatGPT
    // sign-in: ⏎ signs in again, first
    if mode(app) == Mode::Default && !typing && app.diff.is_none() && app.sb.waits_for_sign_in() {
        let mut pairs: Vec<Pair> = if agent { vec![BACK] } else { Vec::new() };
        pairs.push(SIGN_IN_AGAIN);
        pairs.extend(REST);
        pairs.extend(inbox);
        return pairs_line(&pairs, usize::from(width)).0;
    }
    let bar = Bar { typing, agent, inbox };
    // the diff panel open, the composer with the keys: its bar, then
    // `ctrl+g close` (designer m_7291), no tip
    if app.diff.is_some() {
        let close = format!("   {}", crate::diffview::CLOSE_HINT);
        let room = usize::from(width).saturating_sub(close.width());
        let Line { mut spans, .. } = render_with(mode(app), room as u16, None, bar);
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        if used + close.width() <= usize::from(width) {
            spans.push(Span::raw("   "));
            spans.push(Span::styled("ctrl+g", Style::default().fg(theme::text())));
            spans.push(Span::styled(" close", Style::default().fg(theme::dim())));
        }
        return Line::from(spans);
    }
    render_with(mode(app), width, Some(current_tip(typing)), bar)
}

// ---- the tip clock (BISE-104, book §8) ----

/// How long a tip stays: 5 minutes.
const TIP_EVERY: Duration = Duration::from_secs(5 * 60);

/// Which tip shows, and since when. The tips go in the order of
/// [`help::TIPS`], so you meet more of them over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TipClock {
    index: usize,
    since: Instant,
}

impl TipClock {
    /// A session's first tip: the one after where the last session
    /// stopped (`saved`), else `seed`.
    fn start(saved: Option<usize>, seed: usize, now: Instant) -> TipClock {
        let index = saved.map_or(seed, |i| i + 1) % help::TIPS.len();
        TipClock { index, since: now }
    }

    /// At `now`: the next tip once 5 minutes are up, never while you type
    /// (it waits for an empty composer). True when it changed.
    fn tick(&mut self, now: Instant, typing: bool) -> bool {
        if typing || now.saturating_duration_since(self.since) < TIP_EVERY {
            return false;
        }
        self.index = (self.index + 1) % help::TIPS.len();
        self.since = now;
        true
    }
}

thread_local! {
    static CLOCK: std::cell::Cell<Option<TipClock>> = const { std::cell::Cell::new(None) };
}

/// Where the last tip shown is kept: the `tip` preference, when the
/// one-time hints have a store (none with `SB_ONBOARDING=off`, and under
/// `cargo test`; a test store keeps it in a `tip` file next to it).
fn tip_slot() -> Option<bise_home::Slot> {
    crate::hints::store().map(|s| match s.key {
        Some(_) => bise_home::Slot { key: Some(bise_home::Pref::Tip.key()), ..s },
        None => bise_home::Slot::file(s.file.with_file_name("tip")),
    })
}

fn save_tip(i: usize) {
    if let Some(s) = tip_slot() {
        let _ = s.set(i.into());
    }
}

/// The tip of the moment. Read at each draw (the loop draws anyway): no
/// timer and no redraw of its own; the file is written only when the tip
/// changes (and once at the start).
fn current_tip(typing: bool) -> &'static str {
    let now = Instant::now();
    let mut clock = CLOCK.with(|c| c.get()).unwrap_or_else(|| {
        let saved = tip_slot().and_then(|s| s.get()).and_then(|v| v.as_u64()).map(|i| i as usize);
        // no saved tip: a random one (the clock's nanoseconds), 0 in tests
        let seed = if cfg!(test) {
            0
        } else {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize)
        };
        let c = TipClock::start(saved, seed, now);
        save_tip(c.index);
        c
    });
    if clock.tick(now, typing) {
        save_tip(clock.index);
    }
    CLOCK.with(|c| c.set(Some(clock)));
    help::TIPS[clock.index]
}

/// A key's ASCII form (book §6): `alt+` for `⌥`, words for the arrows
/// and `⏎`; the key itself outside ASCII mode.
fn key_text(k: &str) -> String {
    if !theme::ascii_mode() {
        return k.to_string();
    }
    k.replace("↑↓", "up/down")
        .replace('↑', "up")
        .replace('↓', "down")
        .replace('⏎', "enter")
        .replace('⌥', "alt+")
        .replace('←', "left")
        .replace('→', "right")
}

fn label_text(l: &str) -> String {
    l.replace('…', theme::ellipsis())
}

/// The width of a pair drawn: the key, a space, what it does.
fn pair_width((k, l): Pair) -> usize {
    let (k, l) = (key_text(k), label_text(l));
    k.width() + l.width() + usize::from(!k.is_empty() && !l.is_empty())
}

/// The pairs that fit in `width` (book §13): in an agent's view
/// (`agent`), `/ commands` first, then the others from the right, `esc
/// back to main` never; in the thread from the right.
fn fit(mut pairs: Vec<Pair>, width: usize, agent: bool) -> Vec<Pair> {
    let total = |p: &[Pair]| p.iter().map(|&x| pair_width(x)).sum::<usize>() + 3 * p.len().saturating_sub(1);
    while agent && pairs.len() > 1 && total(&pairs) > width {
        match pairs.iter().position(|p| p.0 == "/") {
            Some(i) => pairs.remove(i),
            None => pairs.pop().unwrap_or(BACK),
        };
    }
    // the thread: pairs_line drops from the end (and cuts the first)
    pairs
}

/// What the key bar depends on besides its mode.
#[derive(Clone, Copy, Debug, Default)]
struct Bar {
    /// the composer holds text
    typing: bool,
    /// an agent's view, not main's
    agent: bool,
    /// the inbox holds something: its pair (`ctrl+1 inbox`, `/inbox`),
    /// last at rest, after `⏎ steer` while the agent works
    inbox: Option<Pair>,
}

/// The key bar for `mode` in a row `width` columns wide. Pairs that don't
/// fit are dropped from the end (the first is cut if it alone doesn't
/// fit). In an agent's view (`agent`), `esc back to main` comes first
/// and stays, `/ commands` drops first. The tip shows only in [`Mode::Default`] out of an agent's
/// view, while not `typing`, right-aligned, with at least 3 columns
/// between it and the keys.
#[cfg(test)]
pub(crate) fn render(mode: Mode, width: u16, typing: bool, agent: bool, tip: Option<&str>) -> Line<'static> {
    render_with(mode, width, tip, Bar { typing, agent, ..Bar::default() })
}

/// [`render`], with the inbox pair ([`Bar`]).
fn render_with(mode: Mode, width: u16, tip: Option<&str>, bar: Bar) -> Line<'static> {
    let Bar { typing, agent, inbox } = bar;
    let width = usize::from(width);
    let dim = Style::default().fg(theme::dim());
    let mut pairs = if agent { mode.agent_pairs() } else { mode.pairs().to_vec() };
    if let Some(pair) = inbox.filter(|_| matches!(mode, Mode::Default | Mode::Steer | Mode::Images)) {
        let at = pairs.iter().position(|p| p.0 == "⏎").map_or(pairs.len(), |i| i + 1);
        pairs.insert(at, pair);
    }
    // designer: `tab queue` and `⏎ steer` only with text to queue or
    // steer (both do nothing on an empty composer)
    if mode == Mode::Steer && !typing {
        pairs.retain(|p| p.0 != "tab" && p.0 != "⏎");
    }
    let pairs = fit(pairs, width, agent);
    let (Line { mut spans, .. }, used) = pairs_line(&pairs, width);
    if let Some(t) = tip.filter(|_| mode == Mode::Default && !agent && !typing).map(key_text) {
        let t = if theme::ascii_mode() { format!("tip: {t}") } else { format!("tip · {t}") };
        let tw = t.width();
        if used + 3 + tw <= width {
            spans.push(Span::raw(" ".repeat(width - used - tw)));
            spans.push(Span::styled(t, dim));
        }
    }
    Line::from(spans)
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// The styles of [`ASK`], key and label: the accent, the key bold; under
/// `NO_COLOR`, the whole pair bold in the text color.
fn ask_styles(no_color: bool) -> (Style, Style) {
    if no_color {
        let s = Style::default().fg(theme::text()).add_modifier(Modifier::BOLD);
        return (s, s);
    }
    let label = Style::default().fg(theme::accent());
    (label.add_modifier(Modifier::BOLD), label)
}

/// `pairs` in a row `width` columns wide, and the columns taken: the
/// pairs that don't fit are dropped from the end (the first is cut).
/// Keys in the text color, labels dim, [`ASK`] in the accent.
fn pairs_line(pairs: &[(&str, &str)], width: usize) -> (Line<'static>, usize) {
    let (text, dim) = (Style::default().fg(theme::text()), Style::default().fg(theme::dim()));
    let ask = ask_styles(no_color());
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (i, &pair) in pairs.iter().enumerate() {
        let (key, what) = if pair == ASK { ask } else { (text, dim) };
        let (k, l) = (key_text(pair.0), label_text(pair.1));
        let gap = if i == 0 { 0 } else { 3 };
        let w = k.width() + l.width() + usize::from(!k.is_empty() && !l.is_empty());
        if used + gap + w > width {
            if i == 0 {
                // not even the first pair: cut it, as dim text
                let s = if k.is_empty() { l } else { format!("{k} {l}") };
                spans.push(Span::styled(cut(&s, width), what));
                used = spans[0].width();
            }
            break;
        }
        if gap > 0 {
            spans.push(Span::raw("   "));
        }
        if !k.is_empty() {
            spans.push(Span::styled(k.clone(), key));
        }
        if !l.is_empty() {
            let sep = if k.is_empty() { "" } else { " " };
            spans.push(Span::styled(format!("{sep}{l}"), what));
        }
        used += gap + w;
    }
    (Line::from(spans), used)
}

/// `s` cut to `w` columns, ending with the ellipsis when cut.
fn cut(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let e = theme::ellipsis();
    let room = w.saturating_sub(e.width());
    let mut out = String::new();
    for c in s.chars() {
        if (out.clone() + &c.to_string()).width() > room {
            break;
        }
        out.push(c);
    }
    if w >= e.width() {
        out.push_str(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    const TIP: &str = "ctrl+o opens everything folded";

    #[test]
    fn default_bar_and_tip_at_the_right_edge() {
        let l = render(Mode::Default, 120, false, false, Some(TIP));
        let s = text(&l);
        assert!(s.starts_with("@ file   $ skills   / commands   "), "{s}");
        assert!(!s.contains("send") && !s.contains("help") && !s.contains("⌥0-9"), "{s}");
        assert!(s.ends_with("tip · ctrl+o opens everything folded"), "{s}");
        assert_eq!(s.width(), 120, "the tip ends at the row's last column");
    }

    #[test]
    fn keys_in_text_color_what_they_do_dim() {
        let l = render(Mode::Default, 100, false, false, Some(TIP));
        let key = l.spans.iter().find(|s| s.content == "$").unwrap();
        assert_eq!(key.style.fg, Some(theme::text()));
        let what = l.spans.iter().find(|s| s.content == " skills").unwrap();
        assert_eq!(what.style.fg, Some(theme::dim()));
        let tip = l.spans.last().unwrap();
        assert_eq!(tip.style.fg, Some(theme::dim()));
    }

    #[test]
    fn a_selection_pops_type_ask_about_it_in_the_accent() {
        let l = render(Mode::Quote, 100, false, false, None);
        assert_eq!(text(&l), "type ask about it   cmd+c copy   esc drop");
        let style = |c: &str| l.spans.iter().find(|s| s.content == c).unwrap().style;
        // the pair drawn with ask_styles (NO_COLOR read from the env)
        assert_eq!((style("type"), style(" ask about it")), ask_styles(no_color()));
        // the others keep their style
        assert_eq!(style("cmd+c").fg, Some(theme::text()));
        assert_eq!(style(" copy").fg, Some(theme::dim()));
        assert!(!style("cmd+c").add_modifier.contains(Modifier::BOLD));
        // color: the whole pair in the accent, the key bold
        let (k, w) = ask_styles(false);
        assert_eq!((k.fg, w.fg), (Some(theme::accent()), Some(theme::accent())));
        assert!(k.add_modifier.contains(Modifier::BOLD) && !w.add_modifier.contains(Modifier::BOLD));
        // NO_COLOR: no accent, the whole pair bold
        let (k, w) = ask_styles(true);
        for s in [k, w] {
            assert_eq!(s.fg, Some(theme::text()));
            assert!(s.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn no_tip_while_typing_or_in_another_mode() {
        // (designer) working: tab queue / ⏎ steer only with text, in
        // main's view and an agent's
        assert_eq!(text(&render(Mode::Steer, 120, true, false, None)), "tab queue   ⏎ steer   ctrl+c interrupt");
        assert_eq!(text(&render(Mode::Steer, 120, false, false, None)), "ctrl+c interrupt");
        assert!(!text(&render(Mode::Default, 100, true, false, Some(TIP))).contains("tip"));
        assert!(!text(&render(Mode::Steer, 100, false, false, Some(TIP))).contains("tip"));
    }

    #[test]
    fn no_tip_under_3_free_columns() {
        let keys = text(&render(Mode::Default, 200, false, false, None)).width();
        let tip = "tip · ".width() + TIP.width();
        let fits = render(Mode::Default, (keys + 3 + tip) as u16, false, false, Some(TIP));
        assert!(text(&fits).ends_with(TIP));
        let close = render(Mode::Default, (keys + 2 + tip) as u16, false, false, Some(TIP));
        assert!(!text(&close).contains("tip"));
    }

    #[test]
    fn narrow_drops_pairs_from_the_end() {
        let s = text(&render(Mode::Default, 25, false, false, Some(TIP)));
        assert_eq!(s, "@ file   $ skills");
        let s = text(&render(Mode::Default, 4, false, false, None));
        assert!(s.width() <= 4 && s.ends_with('…'), "{s:?}");
    }

    /// BISE-303: the inbox pair last at rest (`ctrl+1 inbox`, or
    /// `/inbox` without ctrl+1-9, BISE-302), after `⏎ steer` while the
    /// agent works; with images, `ctrl+v paste image` first.
    #[test]
    fn the_inbox_pair_comes_last() {
        let bar = |inbox, agent| Bar { inbox: Some(inbox), agent, ..Bar::default() };
        let s = text(&render_with(Mode::Default, 120, None, bar(INBOX, false)));
        assert_eq!(s, "@ file   $ skills   / commands   ctrl+1 inbox");
        let s = text(&render_with(Mode::Default, 120, None, bar(INBOX_COMMAND, false)));
        assert_eq!(s, "@ file   $ skills   / commands   /inbox");
        let s = text(&render_with(Mode::Default, 120, None, bar(INBOX, true)));
        assert_eq!(s, "esc back to main   @ file   $ skills   / commands   ctrl+1 inbox");
        let typing = |inbox, agent| Bar { typing: true, ..bar(inbox, agent) };
        let s = text(&render_with(Mode::Steer, 120, None, typing(INBOX, false)));
        assert_eq!(s, "tab queue   ⏎ steer   ctrl+1 inbox   ctrl+c interrupt");
        let s = text(&render_with(Mode::Steer, 120, None, bar(INBOX, false)));
        assert_eq!(s, "ctrl+1 inbox   ctrl+c interrupt");
        let s = text(&render_with(Mode::Images, 120, None, bar(INBOX, false)));
        assert_eq!(s, "ctrl+v paste image   @ file   $ skills   / commands   ctrl+1 inbox");
        // nothing in the inbox: no pair
        let s = text(&render_with(Mode::Default, 120, None, Bar::default()));
        assert_eq!(s, "@ file   $ skills   / commands");
    }

    #[test]
    fn ascii_forms() {
        theme::set_ascii_for_tests(true);
        let s = text(&render(Mode::Default, 120, false, false, Some(TIP)));
        let f = text(&render(Mode::FilePopup, 100, false, false, None));
        let t = text(&render(Mode::Transcribing, 100, false, false, None));
        theme::set_ascii_for_tests(false);
        assert!(s.starts_with("@ file   $ skills   / commands"), "{s}");
        assert!(s.ends_with("tip: ctrl+o opens everything folded"), "{s}");
        assert!(f.contains("enter/tab/right open a folder   left up   up/down select"), "{f}");
        assert!(t.starts_with("esc cancel"), "{t}");
        for x in [s, f, t] {
            assert!(x.is_ascii(), "{x}");
        }
    }

    #[test]
    fn an_agents_view_starts_with_esc_back_to_main() {
        let idle = render(Mode::Default, 120, false, true, Some(TIP));
        let s = text(&idle);
        assert_eq!(s, "esc back to main   @ file   $ skills   / commands");
        let esc = idle.spans.iter().find(|x| x.content == "esc").unwrap();
        assert_eq!(esc.style.fg, Some(theme::text()));
        let what = idle.spans.iter().find(|x| x.content == " back to main").unwrap();
        assert_eq!(what.style.fg, Some(theme::dim()));
        assert!(!s.contains("tip"), "no tip in an agent's view");
        let working = text(&render(Mode::Steer, 120, true, true, None));
        assert_eq!(working, "esc back to main   tab queue   ⏎ steer   ctrl+c interrupt");
        let working = text(&render(Mode::Steer, 120, false, true, None));
        assert_eq!(working, "esc back to main   ctrl+c interrupt");
        let archived = text(&render(Mode::Archived, 120, false, true, None));
        assert_eq!(archived, "esc back to main", "the placeholder says /restore");
    }

    #[test]
    fn in_an_agents_view_commands_drop_first_and_esc_never() {
        let all = "esc back to main   @ file   $ skills   / commands".width();
        let s = text(&render(Mode::Default, (all - 1) as u16, false, true, None));
        assert_eq!(s, "esc back to main   @ file   $ skills");
        let s = text(&render(Mode::Default, 30, false, true, None));
        assert_eq!(s, "esc back to main   @ file");
        let s = text(&render(Mode::Steer, 20, false, true, None));
        assert_eq!(s, "esc back to main");
        let s = text(&render(Mode::Default, 10, false, true, None));
        assert!(s.starts_with("esc") && s.width() <= 10, "{s:?}");
        // other modes keep their own keys (esc does something else there)
        let s = text(&render(Mode::Selected, 120, false, true, None));
        assert!(s.starts_with("⏎ enter"), "{s}");
    }

    #[test]
    fn the_tip_changes_every_5_minutes_in_order() {
        let t0 = Instant::now();
        let mut c = TipClock::start(None, 0, t0);
        assert_eq!(c.index, 0);
        assert!(!c.tick(t0 + Duration::from_secs(299), false), "not before 5 minutes");
        assert!(c.tick(t0 + TIP_EVERY, false));
        assert_eq!(c.index, 1);
        // the next change is 5 minutes after this one
        assert!(!c.tick(t0 + TIP_EVERY + Duration::from_secs(60), false));
        // never while you type: it waits for an empty composer
        let later = t0 + 2 * TIP_EVERY + Duration::from_secs(30);
        assert!(!c.tick(later, true));
        assert_eq!(c.index, 1);
        assert!(c.tick(later, false));
        assert_eq!(c.index, 2);
        // it goes round
        let mut c = TipClock::start(Some(help::TIPS.len() - 2), 0, t0);
        assert_eq!(c.index, help::TIPS.len() - 1, "starts after the last session's tip");
        assert!(c.tick(t0 + TIP_EVERY, false));
        assert_eq!(c.index, 0);
        // a saved index out of range (fewer tips now) wraps
        assert!(TipClock::start(Some(99), 0, t0).index < help::TIPS.len());
        assert_eq!(TipClock::start(None, help::TIPS.len() + 3, t0).index, 3);
    }

    #[test]
    fn every_tip_names_a_key_of_the_help() {
        for t in help::TIPS {
            theme::set_ascii_for_tests(true);
            let a = text(&render(Mode::Default, 200, false, false, Some(t)));
            theme::set_ascii_for_tests(false);
            assert!(a.is_ascii() && a.contains("tip: "), "{a}");
            let k = t.split(' ').next().unwrap();
            assert!(
                help::ROWS.iter().any(|r| r.keys.split('|').any(|x| x == k)) || k.starts_with('/'),
                "{t}: {k} is not in the help"
            );
        }
    }
}
