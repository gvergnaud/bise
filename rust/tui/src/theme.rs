//! The bise theme (book §5-6, contract C1): roles, not colors.
//!
//! Two palettes, dark and light, switched by [`set_mode`] (default dark).
//! BISE-92: bise paints its own background ([`bg`], the palette's ground)
//! on every cell, so the text reads whatever the terminal's colors or a
//! wrong theme pick: [`paint`] turns every cell left at `Color::Reset`
//! into the theme's ground and text, once per frame. The tints on that
//! ground are [`selection_bg`], [`card_tint`] and [`raised`] (the
//! composer pane, BISE-102).
//!
//! Color means attention: only "needs you" (accent) and errors get a hue;
//! everything else is text, dim or faint. `faint` is the quietest text
//! (hints, keys, timestamps), still ~4.5:1 on the ground (BISE-279);
//! the lines (rails, borders, rules) are [`rule`], quieter still.
//!
//! The old OpenCode constants (`BRAND`, `DIM`, …) were `#[deprecated]`
//! aliases of the dark palette until BISE-83; new code calls the roles.

use ratatui::style::Color;
use ratatui::symbols::border;

// ---- mode ----

/// Which palette the roles read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Dark,
    Light,
}

#[cfg(not(test))]
mod mode_cell {
    //! The process-wide mode: detection (BISE-02) may run on any thread.
    use super::Mode;
    use std::sync::atomic::{AtomicBool, Ordering};

    static LIGHT: AtomicBool = AtomicBool::new(false);

    pub(super) fn set(m: Mode) {
        LIGHT.store(m == Mode::Light, Ordering::Relaxed);
    }
    pub(super) fn get() -> Mode {
        if LIGHT.load(Ordering::Relaxed) {
            Mode::Light
        } else {
            Mode::Dark
        }
    }
}

#[cfg(test)]
mod mode_cell {
    //! Under `cargo test` the mode is per thread (each test runs on its own
    //! thread), so a test that switches to light never repaints a render
    //! test running next to it.
    use super::Mode;
    use std::cell::Cell;

    thread_local!(static MODE: Cell<Mode> = const { Cell::new(Mode::Dark) });

    pub(super) fn set(m: Mode) {
        MODE.with(|c| c.set(m));
    }
    pub(super) fn get() -> Mode {
        MODE.with(|c| c.get())
    }
}

/// Switch every role to the dark or the light palette.
pub(crate) fn set_mode(m: Mode) {
    mode_cell::set(m);
}

/// The current mode (default dark).
pub(crate) fn mode() -> Mode {
    mode_cell::get()
}

// ---- palettes ----

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// One palette: the value of every role in one mode.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Palette {
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    /// rails, borders, rules: the lines, never text (BISE-279)
    pub rule: Color,
    pub accent: Color,
    pub error: Color,
    pub ok: Color,
    /// text drawn on an accent background (a chip, the popup selection)
    pub on_accent: Color,
    pub selection_bg: Color,
    pub card_tint: Color,
    /// the composer pane, raised a little above the ground (BISE-102)
    pub raised: Color,
    /// an inbox item open in its box: one step above the composer's
    pub item: Color,
    /// the level-3 message chip (BISE-106)
    pub chip: Color,
    /// the pill of a quote or image chip in the composer (BISE-205)
    pub pill: Color,
    /// the ground every cell is painted with (BISE-92)
    pub bg: Color,
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_comment: Color,
    pub syntax_number: Color,
    pub syntax_call: Color,
    pub syntax_type: Color,
}

/// Dark: for a dark terminal (checked on black, `#141211`, `#282c34`).
pub(crate) const DARK: Palette = Palette {
    text: rgb(0xece6da),
    dim: rgb(0xa39c90),
    // BISE-279 (user: faint text was hard to read): was #4a4540 (1.97:1),
    // now the site's gray, 4.6:1 on the ground, 4.2:1 on raised; the old
    // value is `rule`
    faint: rgb(0x857d72),
    rule: rgb(0x4a4540),
    accent: rgb(0xf4a6b0), // pale pink
    error: rgb(0xff5a52),
    ok: rgb(0xb9d99a),
    on_accent: rgb(0x1b1917),
    selection_bg: rgb(0x33292c),
    card_tint: rgb(0x211d1b),
    raised: rgb(0x1f1c1a),
    item: rgb(0x26221f),
    chip: rgb(0x231f1d),
    pill: rgb(0x3a2530),
    bg: rgb(0x141211),
    syntax_keyword: rgb(0xd7a6f0),
    syntax_string: rgb(0xb9d99a),
    // the book's #857e74 is 3.5:1 on #282c34: lifted to pass 4.5:1
    syntax_comment: rgb(0x99928a),
    syntax_number: rgb(0xf0b27a),
    syntax_call: rgb(0x8fc4f0),
    syntax_type: rgb(0xe8cf9a),
};

/// Light: for a light terminal (checked on white and our cream `#f7f4ee`).
pub(crate) const LIGHT: Palette = Palette {
    text: rgb(0x1b1917),
    dim: rgb(0x6b645a),
    // BISE-279: was #cfc8bd (1.61:1), now the landing demo's light gray,
    // 4.3:1 on the ground, 3.9:1 on raised; the old value is `rule`
    faint: rgb(0x7d766c),
    rule: rgb(0xcfc8bd),
    accent: rgb(0xb8416b), // raspberry
    error: rgb(0xb3261e),
    ok: rgb(0x3f7a2a),
    on_accent: rgb(0xffffff),
    // on the painted ground (a lighter cream than #f7f4ee, so both tints
    // show on it and every role still reads on them): a pink selection,
    // a sand card
    selection_bg: rgb(0xfdeef2),
    card_tint: rgb(0xf1eee6),
    raised: rgb(0xf4f0e8),
    // the deepest sand where the accent still reads at 4.5:1
    item: rgb(0xf2ede6),
    chip: rgb(0xefe9df),
    pill: rgb(0xf0d3dc),
    bg: rgb(0xfdfbf7),
    syntax_keyword: rgb(0x8a3fb0),
    syntax_string: rgb(0x44782a),
    syntax_comment: rgb(0x726b60),
    syntax_number: rgb(0x9a4a0c),
    syntax_call: rgb(0x1f63a8),
    syntax_type: rgb(0x7a5c00),
};

/// The palette of a mode.
pub(crate) const fn palette_of(m: Mode) -> &'static Palette {
    match m {
        Mode::Dark => &DARK,
        Mode::Light => &LIGHT,
    }
}

/// The palette of the current mode.
pub(crate) fn palette() -> &'static Palette {
    palette_of(mode())
}

// ---- roles ----

/// Everything you read.
pub(crate) fn text() -> Color {
    palette().text
}
/// Secondary text, level 3, durations.
pub(crate) fn dim() -> Color {
    palette().dim
}
/// The quietest text: hints, keys, timestamps, numbers (4.3-4.6:1 on the
/// ground, BISE-279). Not for lines: [`rule`].
pub(crate) fn faint() -> Color {
    palette().faint
}
/// Rails, borders, rules: the lines, quieter than any text (the old
/// `faint`, BISE-279). Never for text.
pub(crate) fn rule() -> Color {
    palette().rule
}
/// The `:*`, "needs you", the agent you talk to, `✓✓` read.
pub(crate) fn accent() -> Color {
    palette().accent
}
/// Failures only.
pub(crate) fn error() -> Color {
    palette().error
}
/// Diff additions only.
pub(crate) fn ok() -> Color {
    palette().ok
}
/// Text on an accent background (a chip, a selected popup row).
pub(crate) fn on_accent() -> Color {
    palette().on_accent
}
/// The ground: painted on every cell (BISE-92).
pub(crate) fn bg() -> Color {
    palette().bg
}

/// The frame pass (BISE-92): every cell left at the terminal's default
/// (`Color::Reset`) gets the theme's ground, or text color for the
/// foreground. Runs after everything is drawn, before `asciify`.
pub(crate) fn paint(buf: &mut ratatui::buffer::Buffer) {
    let (ground, ink) = (bg(), text());
    for cell in buf.content.iter_mut() {
        if cell.bg == Color::Reset {
            cell.bg = ground;
        }
        if cell.fg == Color::Reset {
            cell.fg = ink;
        }
    }
}
/// The light tint under selected text.
pub(crate) fn selection_bg() -> Color {
    palette().selection_bg
}
/// The light tint of the card box.
pub(crate) fn card_tint() -> Color {
    palette().card_tint
}
/// The composer pane's tint (book §5 `raised`, §13): everything under the
/// divider. Under `NO_COLOR`, none (`Reset`: the ground).
pub(crate) fn raised() -> Color {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        Color::Reset
    } else {
        palette().raised
    }
}
/// The tint of an inbox item open in its box (one step above the
/// composer's). Under `NO_COLOR`, none.
pub(crate) fn item_tint() -> Color {
    if raised() == Color::Reset {
        Color::Reset
    } else {
        palette().item
    }
}
/// The tint of a level-3 message chip (book §5 `chip`, §9; BISE-106).
/// Under `NO_COLOR`, none (`Reset`): the chip is drawn in brackets.
pub(crate) fn chip_bg() -> Color {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        Color::Reset
    } else {
        palette().chip
    }
}
/// The pink pill under a quote or image chip in the composer and the
/// strip (book §5 `pill`, §13; BISE-205). Under `NO_COLOR`, none
/// (`Reset`): the chip is drawn in brackets.
pub(crate) fn pill_bg() -> Color {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        Color::Reset
    } else {
        palette().pill
    }
}
pub(crate) fn syntax_keyword() -> Color {
    palette().syntax_keyword
}
pub(crate) fn syntax_string() -> Color {
    palette().syntax_string
}
pub(crate) fn syntax_comment() -> Color {
    palette().syntax_comment
}
pub(crate) fn syntax_number() -> Color {
    palette().syntax_number
}
/// Function and method calls.
pub(crate) fn syntax_call() -> Color {
    palette().syntax_call
}
pub(crate) fn syntax_type() -> Color {
    palette().syntax_type
}

// ---- glyphs (book §6): one glyph per entity and per status ----
// After the glyph audit (BISE-03, BISE-84): only glyphs a fallback font
// draws at width 1; the breakers are replaced (✉ → @, ⟳ → ≡ pulsing,
// ⧗ → Δ, ⎇ → ψ, ↪ → »). Under `BISE_ASCII=1` every glyph has a plain
// ASCII form: call [`glyph`] (the constants are the Unicode forms), and
// [`asciify`] catches what is drawn without it.

// entities
pub(crate) const G_YOU: &str = "›"; // you, and the composer prompt
pub(crate) const G_MAIN: &str = ":*"; // main (accent)
pub(crate) const G_BRIEF: &str = "◇"; // an agent's brief
pub(crate) const G_THINK: &str = "∴"; // thinking (dim)
pub(crate) const G_BASH: &str = "$"; // a bash call
pub(crate) const G_TS: &str = "ƒ"; // a TypeScript call (BISE-223: the user's pick)
pub(crate) const G_SUBCALL: &str = "↳"; // a sub-call inside a TypeScript run
pub(crate) const G_PATCH: &str = "±"; // a file edit
pub(crate) const G_MSG: &str = "@"; // a message between agents, or to you (was ✉)
pub(crate) const G_IMAGE: &str = "▣"; // an image (accent chip)
pub(crate) const G_QUOTE: &str = "❝"; // a quote of the history (accent chip, BISE-134)
pub(crate) const G_PASTE: &str = "▤"; // a long paste (accent chip, BISE-240)
pub(crate) const G_CARD: &str = "?"; // a card: a decision that needs you (accent)
pub(crate) const G_COMPACTING: &str = "≡"; // compaction running (dim, pulsing; was ⟳)
pub(crate) const G_SUMMARY: &str = "≡"; // compaction summary (dim, still)
pub(crate) const G_INTERRUPTED: &str = "▲"; // turn interrupted (dim)
pub(crate) const G_WRAP: &str = "»"; // a wrapped code row continues (faint; was ↪)
pub(crate) const G_SCHEDULED: &str = "◷"; // a scheduled task (sb every), its lines and next run (faint; site/m/timers)

// agent status
pub(crate) const G_STARTING: &str = "·"; // dim, pulsing
pub(crate) const G_WORKING: &str = "∿"; // pulsing: a breeze
pub(crate) const G_WAITING: &str = "…"; // waiting on another agent
pub(crate) const G_NEEDS_YOU: &str = "?"; // accent
pub(crate) const G_DONE: &str = "✓"; // accent (BISE-100, was ♡); draw it with [`done_glyph`]
pub(crate) const G_FAILED: &str = "✗"; // error
pub(crate) const G_IDLE: &str = "○"; // dim
pub(crate) const G_STOPPED: &str = "–"; // dim

// marks
pub(crate) const G_SENDING: &str = "·"; // your message: sending
pub(crate) const G_RECEIVED: &str = "✓"; // the agent got it
pub(crate) const G_READ: &str = "✓✓"; // the model read it (accent)
pub(crate) const G_UNREAD: &str = "•"; // unread activity (accent)
pub(crate) const G_WORKTREE: &str = "ψ"; // the agent has its own worktree (was ⎇)
pub(crate) const G_OVERLAP: &str = "⇄"; // two agents changed the same file
pub(crate) const G_RESTART_FAILED: &str = "↻"; // error
pub(crate) const G_BUILDING: &str = "Δ"; // a version building or on trial (was ⧗)
pub(crate) const G_CLOSED: &str = "▸"; // progressive disclosure: closed
pub(crate) const G_OPEN: &str = "▾"; // progressive disclosure: open
pub(crate) const G_PR: &str = "↑"; // a pull request, in a worktree's border (pr-design §4); draw it with [`pr_glyph`]

// ---- the legend (the symbols section of /help, book §6) ----

/// The color a legend glyph is drawn in: its color on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Text,
    Dim,
    Faint,
    Accent,
    Error,
}

impl Tone {
    pub(crate) fn color(self) -> Color {
        match self {
            Tone::Text => text(),
            Tone::Dim => dim(),
            Tone::Faint => faint(),
            Tone::Accent => accent(),
            Tone::Error => error(),
        }
    }
}

/// One row of the legend: a glyph (or a short sample of what is drawn,
/// space-separated), its color, its ASCII form when [`ASCII`] does not
/// give it ("" = from the table, like [`asciify`]), and a few words.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Symbol {
    pub group: &'static str,
    pub glyph: &'static str,
    pub tone: Tone,
    pub ascii: &'static str,
    pub meaning: &'static str,
}

const fn sym(group: &'static str, glyph: &'static str, tone: Tone, meaning: &'static str) -> Symbol {
    Symbol { group, glyph, tone, ascii: "", meaning }
}

impl Symbol {
    /// The glyph as drawn in the current mode.
    pub(crate) fn shown(&self) -> String {
        match (ascii_mode(), self.ascii) {
            (false, _) => self.glyph.to_string(),
            (true, "") => ascii_text(self.glyph),
            (true, a) => a.to_string(),
        }
    }
}

const AGENTS: &str = "agents";
const MESSAGES: &str = "messages";
const HISTORY: &str = "history";

/// Every glyph the TUI shows, one row each, in display order (groups
/// in first-row order). A new `G_*` constant needs its row: the test
/// `every_glyph_constant_has_a_legend_row` reads the declarations.
/// Lowercase, one line, no "X, not Y" (designer, BISE-137).
#[rustfmt::skip]
pub(crate) const LEGEND: &[Symbol] = &[
    sym(AGENTS, G_MAIN, Tone::Accent, "main, the agent you talk to"),
    sym(AGENTS, G_WORKING, Tone::Text, "working (it moves)"),
    sym(AGENTS, G_STARTING, Tone::Dim, "starting"),
    sym(AGENTS, G_WAITING, Tone::Text, "waiting on another agent"),
    sym(AGENTS, G_NEEDS_YOU, Tone::Accent, "needs you: a card in your inbox (dim: a blocked task, main's to handle)"),
    Symbol { group: AGENTS, glyph: G_DONE, tone: Tone::Accent, ascii: "*", meaning: "done" },
    sym(AGENTS, G_FAILED, Tone::Error, "failed"),
    sym(AGENTS, G_IDLE, Tone::Dim, "idle"),
    sym(AGENTS, G_STOPPED, Tone::Dim, "stopped"),
    sym(AGENTS, G_UNREAD, Tone::Accent, "unread activity in that agent"),
    sym(AGENTS, G_WORKTREE, Tone::Dim, "has its own worktree (its own copy of the files)"),
    Symbol { group: AGENTS, glyph: G_PR, tone: Tone::Dim, ascii: "P", meaning: "a pull request (dim open, faint draft, red checks fail)" },
    sym(AGENTS, "opus·hi", Tone::Dim, "model · reasoning (dim: not the same as main's)"),
    sym(AGENTS, G_OVERLAP, Tone::Text, "two agents changed the same file"),
    sym(AGENTS, G_RESTART_FAILED, Tone::Error, "a restart failed"),
    sym(AGENTS, G_BUILDING, Tone::Text, "a version is building or on trial"),
    sym(AGENTS, "# 3", Tone::Dim, "items in your inbox: ctrl+1-9 or a click opens them"),
    sym(AGENTS, "+ 2 more", Tone::Dim, "rows that do not fit"),
    sym(MESSAGES, G_YOU, Tone::Text, "the composer, and your queued messages"),
    sym(MESSAGES, "│", Tone::Accent, "your message"),
    sym(MESSAGES, "· ✓ ✓✓", Tone::Dim, "at its end: sending, the agent got it, the model read it"),
    sym(MESSAGES, "┃", Tone::Accent, "something that needs you"),
    sym(MESSAGES, G_MSG, Tone::Text, "an agent writes to you"),
    Symbol { group: MESSAGES, glyph: "\u{2709}\u{FE0E} a → b", tone: Tone::Dim, ascii: "@ a > b", meaning: "a message between two agents" },
    sym(MESSAGES, G_IMAGE, Tone::Accent, "an image"),
    sym(MESSAGES, G_QUOTE, Tone::Accent, "a quote of the history"),
    sym(MESSAGES, G_PASTE, Tone::Accent, "a long paste"),
    sym(MESSAGES, "●", Tone::Accent, "recording your voice"),
    sym(HISTORY, G_BRIEF, Tone::Text, "an agent's brief"),
    sym(HISTORY, G_THINK, Tone::Dim, "thinking"),
    sym(HISTORY, G_BASH, Tone::Text, "a bash call"),
    sym(HISTORY, G_TS, Tone::Text, "a TypeScript call"),
    sym(HISTORY, G_SUBCALL, Tone::Text, "a call inside a TypeScript run"),
    sym(HISTORY, G_PATCH, Tone::Text, "a file edit"),
    sym(HISTORY, "·", Tone::Dim, "a note from the app"),
    sym(HISTORY, G_COMPACTING, Tone::Dim, "compaction: it pulses while running, then its summary"),
    sym(HISTORY, G_INTERRUPTED, Tone::Dim, "turn interrupted"),
    sym(HISTORY, G_WRAP, Tone::Faint, "a long code row goes on"),
    sym(HISTORY, G_SCHEDULED, Tone::Faint, "a scheduled task: set, a run, ended; on an agent's row, its next run (/scheduled)"),
    sym(HISTORY, "▸ ▾", Tone::Text, "folded / open: click or space"),
    sym(HISTORY, "▸ 3 more lines", Tone::Dim, "folded lines: click, space or ctrl+o"),
];

/// `s` as `BISE_ASCII=1` draws it: each glyph of [`ASCII`] to its
/// form, the longest match first (`✓✓` before `✓`).
pub(crate) fn ascii_text(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        match ASCII.iter().filter(|(u, _)| rest.starts_with(u)).max_by_key(|(u, _)| u.len()) {
            Some((u, a)) => {
                out.push_str(a);
                rest = &rest[u.len()..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Every `G_*` glyph with its ASCII form, then the other glyphs the TUI
/// draws today (chrome, hints, old feed glyphs, the braille spinner): the
/// table [`glyph`] and [`asciify`] read. One cell each, except `✓✓`.
/// Box drawing (`│ ┃ ─ ╮ …`) is not in it: [`asciify`] draws it `+ - |`
/// like the frame (QA E). Block elements (`▁ █ ▏ …`) stay. User text is never in it (no letters, accents, CJK,
/// emoji, quotes).
pub(crate) const ASCII: &[(&str, &str)] = &[
    // §6 glyphs
    ("›", ">"),
    ("◇", "&"),
    ("∴", ":"),
    ("ƒ", "f"),
    ("↳", "L"),
    ("±", "%"),
    ("▣", "#"),
    ("❝", "\""),
    ("▤", "T"),
    ("≡", "="),
    ("▲", "^"),
    ("»", "}"),
    ("·", "."),
    ("∿", "~"),
    ("…", ";"),
    ("✗", "x"),
    ("○", "o"),
    ("–", "_"),
    ("✓✓", "vv"),
    ("✓", "v"),
    ("•", "!"),
    ("ψ", "Y"),
    ("⇄", "/"),
    ("↻", "("),
    ("Δ", "A"),
    ("◷", "@"),
    ("▸", "+"),
    ("▾", "-"),
    // the replaced ones, while old code still draws them
    ("✉", "@"),
    ("⟳", "="),
    ("⧗", "A"),
    ("⎇", "Y"),
    ("↪", "}"),
    ("♡", "*"),
    // chrome and hints drawn outside the G_* constants (BISE-83 moves them)
    ("✦", "*"),
    ("◀", "<"),
    ("▶", ">"),
    ("●", "*"),
    ("⌕", "/"),
    ("◉", "@"),
    ("◆", "*"),
    ("✚", "+"),
    ("▪", "*"),
    ("×", "x"),
    ("⏎", "<"),
    ("→", ">"),
    ("←", "<"),
    ("↑", "^"),
    ("↓", "v"),
    ("⇧", "S"),
    ("⌥", "M"),
    ("—", "-"),
    ("−", "-"),
    // the old braille spinner
    ("⠋", "~"),
    ("⠙", "~"),
    ("⠹", "~"),
    ("⠸", "~"),
    ("⠼", "~"),
    ("⠴", "~"),
    ("⠦", "~"),
    ("⠧", "~"),
    ("⠇", "~"),
    ("⠏", "~"),
];

#[cfg(not(test))]
mod ascii_cell {
    //! `BISE_ASCII=1` (or `true`), read once.
    use std::sync::OnceLock;

    static ASCII: OnceLock<bool> = OnceLock::new();

    pub(super) fn get() -> bool {
        *ASCII.get_or_init(|| {
            std::env::var("BISE_ASCII").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
        })
    }
}

#[cfg(test)]
mod ascii_cell {
    //! Per thread under `cargo test`, like the mode; off by default.
    use std::cell::Cell;

    thread_local!(static ASCII: Cell<bool> = const { Cell::new(false) });

    pub(super) fn get() -> bool {
        ASCII.with(|c| c.get())
    }
    pub(super) fn set(on: bool) {
        ASCII.with(|c| c.set(on));
    }
}

/// True under `BISE_ASCII=1`: every glyph is drawn in plain ASCII.
pub(crate) fn ascii_mode() -> bool {
    ascii_cell::get()
}

/// Tests of other modules switch ASCII mode for their thread.
#[cfg(test)]
pub(crate) fn set_ascii_for_tests(on: bool) {
    ascii_cell::set(on);
}

/// The glyph to draw for `g` (a `G_*` constant, or any glyph of the
/// table): its ASCII form under `BISE_ASCII=1`, else `g` itself.
pub(crate) fn glyph(g: &'static str) -> &'static str {
    if !ascii_mode() {
        return g;
    }
    ASCII.iter().find(|(u, _)| *u == g).map_or(g, |(_, a)| *a)
}

/// The PR glyph: `↑`, `P` under `BISE_ASCII=1`. Not `glyph(G_PR)`: the
/// table's `↑` is the key hints' arrow (`^`), and every ASCII form of a
/// mark is its own (`#` is `▣`'s, `^` is `▲`'s; pr-design §4).
pub(crate) fn pr_glyph() -> &'static str {
    if ascii_mode() {
        "P"
    } else {
        G_PR
    }
}

/// The done glyph: `✓` (accent), `*` under `BISE_ASCII=1`. Not
/// `glyph(G_DONE)`: the table's `✓` is your read mark (`v`), and done
/// keeps its own ASCII form (BISE-100; book §6).
pub(crate) fn done_glyph() -> &'static str {
    if ascii_mode() {
        "*"
    } else {
        G_DONE
    }
}

/// The mark of a cut text: `…`, or `...` under `BISE_ASCII=1` (QA 12: the
/// one-cell `;` of the table is for the waiting status, not for prose).
pub(crate) fn ellipsis() -> &'static str {
    if ascii_mode() {
        "..."
    } else {
        "…"
    }
}

/// Under `BISE_ASCII=1`, rewrite every cell of `buf` holding a glyph of the
/// table to its ASCII form (one cell to one cell, layout unchanged).
/// Box drawing becomes `+ - |` ([`box_ascii`]). Nothing else is touched:
/// letters, accents, CJK, emoji, block elements.
/// Called after each draw; free when the mode is off.
pub(crate) fn asciify(buf: &mut ratatui::buffer::Buffer) {
    if !ascii_mode() {
        return;
    }
    for cell in buf.content.iter_mut() {
        let sym = cell.symbol();
        if sym.is_ascii() {
            continue;
        }
        if let Some((_, a)) = ASCII.iter().find(|(u, a)| *u == sym && a.len() == 1) {
            cell.set_symbol(a);
        } else if let Some(a) = box_ascii(sym) {
            cell.set_symbol(a);
        }
    }
}

/// The ASCII form of a box-drawing cell (U+2500–U+257F), like the frame's
/// `+ - |` (QA E): the card box, the card bar, the rails. Lines are `-` and
/// `|`, corners and joins `+`, diagonals `/ \ x`. Block elements stay.
fn box_ascii(sym: &str) -> Option<&'static str> {
    let mut chars = sym.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if !('\u{2500}'..='\u{257f}').contains(&c) {
        return None;
    }
    Some(match c {
        '─' | '━' | '┄' | '┅' | '┈' | '┉' | '╌' | '╍' | '═' | '╴' | '╶' | '╸' | '╺' | '╼' | '╾' => "-",
        '│' | '┃' | '┆' | '┇' | '┊' | '┋' | '╎' | '╏' | '║' | '╵' | '╷' | '╹' | '╻' | '╽' | '╿' => "|",
        '╱' => "/",
        '╲' => "\\",
        '╳' => "x",
        _ => "+",
    })
}

/// The working pulse: `∿` in text, then dim, then text… (one phase every
/// 4 ticks). Replaces the braille spinner.
pub(crate) fn working_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { text() } else { dim() };
    (glyph(G_WORKING), color)
}

/// The starting pulse: `·`, dim then faint, same rhythm as [`working_frame`].
pub(crate) fn starting_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { dim() } else { faint() };
    (glyph(G_STARTING), color)
}

// the prompt/autocomplete borders: only a vertical bar
pub(crate) const SPLIT: border::Set = border::Set {
    top_left: "",
    top_right: "",
    bottom_left: "",
    bottom_right: "",
    vertical_left: "┃",
    vertical_right: "┃",
    horizontal_top: " ",
    horizontal_bottom: " ",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Color) -> f64 {
        match c {
            Color::Rgb(r, g, b) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b),
            other => panic!("not an rgb color: {other:?}"),
        }
    }

    /// WCAG 2 contrast ratio.
    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Every role you must be able to read.
    fn readable(p: &Palette) -> [(&'static str, Color); 11] {
        [
            ("text", p.text),
            ("dim", p.dim),
            ("accent", p.accent),
            ("error", p.error),
            ("ok", p.ok),
            ("syntax_keyword", p.syntax_keyword),
            ("syntax_string", p.syntax_string),
            ("syntax_comment", p.syntax_comment),
            ("syntax_number", p.syntax_number),
            ("syntax_call", p.syntax_call),
            ("syntax_type", p.syntax_type),
        ]
    }

    fn check(p: &Palette, backgrounds: &[(&str, Color)]) -> Vec<String> {
        let mut fails = vec![];
        for (bg_name, bg) in backgrounds {
            for (name, fg) in readable(p) {
                let r = contrast(fg, *bg);
                if r < 4.5 {
                    fails.push(format!("{name} on {bg_name}: {r:.2}"));
                }
            }
        }
        fails
    }

    #[test]
    fn dark_roles_read_on_dark_backgrounds() {
        let bgs = [("black", rgb(0x000000)), ("#141211", rgb(0x141211)), ("#282c34", rgb(0x282c34))];
        let fails = check(&DARK, &bgs);
        assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
    }

    #[test]
    fn light_roles_read_on_light_backgrounds() {
        let bgs = [("white", rgb(0xffffff)), ("#f7f4ee", rgb(0xf7f4ee))];
        let fails = check(&LIGHT, &bgs);
        assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
    }

    #[test]
    fn text_reads_on_the_tints() {
        for p in [&DARK, &LIGHT] {
            for (tint_name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised), ("item", p.item)] {
                for (name, fg) in [("text", p.text), ("dim", p.dim), ("accent", p.accent)] {
                    let r = contrast(fg, tint);
                    assert!(r >= 4.5, "{name} on {tint_name} tint: {r:.2}");
                }
            }
            let r = contrast(p.on_accent, p.accent);
            assert!(r >= 4.5, "on_accent on accent: {r:.2}");
        }
    }

    #[test]
    fn roles_read_on_the_painted_ground_and_the_tints_show_on_it() {
        for p in [&DARK, &LIGHT] {
            let fails = check(p, &[("ground", p.bg)]);
            assert!(fails.is_empty(), "below 4.5:1 on the ground: {fails:?}");
            // a tint must be seen on the ground, and apart from the other one
            for (name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised), ("item", p.item)] {
                let r = contrast(tint, p.bg);
                assert!(r >= 1.08, "{name} tint vs ground: {r:.3}");
            }
            assert!(contrast(p.selection_bg, p.card_tint) >= 1.03);
            // the open inbox item shows on the composer's tint
            assert!(contrast(p.item, p.raised) >= 1.02, "item vs raised: {:.3}", contrast(p.item, p.raised));
        }
    }

    #[test]
    fn the_pill_reads_and_stands_out_on_the_composer() {
        // on the pill: the number (text) and the glyph (accent, a symbol:
        // 3:1 for graphics); it must stand out from the raised pane and
        // from the selection
        for p in [&DARK, &LIGHT] {
            let r = contrast(p.text, p.pill);
            assert!(r >= 4.5, "text on the pill: {r:.2}");
            let r = contrast(p.accent, p.pill);
            assert!(r >= 3.0, "accent on the pill: {r:.2}");
            let r = contrast(p.pill, p.raised);
            assert!(r >= 1.15, "pill vs raised: {r:.3}");
        }
    }

    #[test]
    fn the_chip_reads_and_shows() {
        // what sits on the chip: the sender (text), the envelope and the
        // receiver (dim); its `→` is faint, never read alone (book §5)
        for p in [&DARK, &LIGHT] {
            for (name, fg) in [("text", p.text), ("dim", p.dim)] {
                let r = contrast(fg, p.chip);
                assert!(r >= 4.5, "{name} on the chip: {r:.2}");
            }
            let r = contrast(p.chip, p.bg);
            assert!(r >= 1.08, "chip vs ground: {r:.3}");
            assert!(contrast(p.faint, p.chip) < contrast(p.dim, p.chip), "faint stays quieter than dim on the chip");
        }
        assert!(contrast(DARK.text, DARK.chip) >= 12.0 && contrast(DARK.dim, DARK.chip) >= 5.5);
        assert!(contrast(LIGHT.dim, LIGHT.chip) >= 4.7);
    }

    #[test]
    fn paint_leaves_no_reset_cell() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::Style;
        let mut b = Buffer::empty(Rect::new(0, 0, 4, 1));
        b[(1, 0)].set_style(Style::default().bg(card_tint()).fg(accent()));
        paint(&mut b);
        assert!(b.content.iter().all(|c| c.bg != Color::Reset && c.fg != Color::Reset));
        assert_eq!((b[(0, 0)].bg, b[(0, 0)].fg), (bg(), text()));
        assert_eq!((b[(1, 0)].bg, b[(1, 0)].fg), (card_tint(), accent()), "set colors stay");
    }

    #[test]
    fn faint_is_quieter_than_dim() {
        // text > dim > faint > rule on the ground, in both modes, each a
        // visible step; faint still reads (BISE-279: >= 4.3:1 on the
        // ground, >= 3.9 on the composer's raised pane, where the queued
        // hint sits)
        for p in [&DARK, &LIGHT] {
            let on = |c| contrast(c, p.bg);
            let (t, d, f, r) = (on(p.text), on(p.dim), on(p.faint), on(p.rule));
            assert!(t > d * 1.8 && d > f * 1.3 && f > r * 2.0, "steps: {t:.2} {d:.2} {f:.2} {r:.2}");
            assert!(f >= 4.3, "faint on the ground: {f:.2}");
            let fr = contrast(p.faint, p.raised);
            assert!(fr >= 3.9, "faint on raised: {fr:.2}");
            assert!(r >= 1.5, "a rule still shows: {r:.2}");
        }
    }

    fn roles() -> Vec<Color> {
        vec![
            text(),
            dim(),
            faint(),
            accent(),
            error(),
            ok(),
            on_accent(),
            selection_bg(),
            card_tint(),
            bg(),
            syntax_keyword(),
            syntax_string(),
            syntax_comment(),
            syntax_number(),
            syntax_call(),
            syntax_type(),
        ]
    }

    #[test]
    fn set_mode_switches_every_role() {
        assert_eq!(mode(), Mode::Dark, "default is dark");
        let dark = roles();
        set_mode(Mode::Light);
        assert_eq!(mode(), Mode::Light);
        let light = roles();
        set_mode(Mode::Dark);
        assert_eq!(roles(), dark);
        for (i, (d, l)) in dark.iter().zip(&light).enumerate() {
            assert_ne!(d, l, "role #{i} is the same in both modes");
        }
        assert_eq!(bg(), DARK.bg);
    }

    #[test]
    fn working_pulses_between_text_and_dim() {
        let frames: Vec<_> = (0..8).map(working_frame).collect();
        assert!(frames.iter().all(|(g, _)| *g == G_WORKING));
        assert_eq!(frames[0].1, text());
        assert_eq!(frames[4].1, dim());
    }

    const ALL_GLYPHS: &[&str] = &[
        G_YOU, G_MAIN, G_BRIEF, G_THINK, G_BASH, G_TS, G_SUBCALL, G_PATCH, G_MSG, G_IMAGE, G_QUOTE, G_PASTE,
        G_CARD, G_COMPACTING, G_SUMMARY, G_INTERRUPTED, G_WRAP, G_STARTING, G_WORKING,
        G_WAITING, G_NEEDS_YOU, G_DONE, G_FAILED, G_IDLE, G_STOPPED, G_SENDING, G_RECEIVED,
        G_READ, G_UNREAD, G_WORKTREE, G_OVERLAP, G_RESTART_FAILED, G_BUILDING, G_CLOSED, G_OPEN,
    ];

    /// The documented widths: `:*` and `✓✓` are two cells, the rest one.
    fn documented_width(g: &str) -> usize {
        if g == G_MAIN || g == G_READ {
            2
        } else {
            1
        }
    }

    #[test]
    fn every_glyph_is_one_cell_in_both_modes() {
        use unicode_width::UnicodeWidthStr;
        for ascii in [false, true] {
            ascii_cell::set(ascii);
            for g in ALL_GLYPHS {
                let shown = glyph(g);
                assert_eq!(shown.width(), documented_width(g), "{g:?} → {shown:?} (ascii {ascii})");
                // no ambiguous-width surprise: the CJK width agrees for ASCII forms
                if ascii {
                    assert!(shown.is_ascii(), "{g:?} → {shown:?} is not ASCII");
                    assert_eq!(shown.width_cjk(), shown.width());
                }
            }
            let (w, _) = working_frame(0);
            assert_eq!(w, if ascii { "~" } else { "∿" });
        }
        ascii_cell::set(false);
    }

    #[test]
    fn the_ascii_table_is_one_cell_to_one_cell() {
        use unicode_width::UnicodeWidthStr;
        let mut seen = std::collections::HashSet::new();
        for (u, a) in ASCII {
            assert!(seen.insert(*u), "{u:?} twice in the table");
            assert!(a.is_ascii() && !a.is_empty(), "{u:?} → {a:?}");
            assert_eq!(u.width(), a.width(), "{u:?} → {a:?} changes the width");
            assert!(u.chars().all(|c| !c.is_alphanumeric() || c == 'ƒ' || c == 'ψ' || c == 'Δ'),
                "{u:?}: letters are user text");
        }
        // every G_* glyph that is not ASCII has its form
        for g in ALL_GLYPHS.iter().filter(|g| !g.is_ascii()) {
            assert!(ASCII.iter().any(|(u, _)| u == g), "{g:?} has no ASCII form");
        }
        // the replaced glyphs are gone from the constants
        for gone in ["✉", "⟳", "⧗", "⎇", "↪"] {
            assert!(!ALL_GLYPHS.contains(&gone), "{gone} is back");
        }
    }

    /// QA 12: two different glyphs never share an ASCII form, and the
    /// cut-text mark is `...` in ASCII mode.
    #[test]
    fn every_entity_has_its_own_ascii_form() {
        ascii_cell::set(true);
        let mut by_ascii: std::collections::HashMap<&str, &str> = Default::default();
        for g in ALL_GLYPHS {
            if let Some(other) = by_ascii.insert(glyph(g), g) {
                assert_eq!(other, *g, "{other:?} and {g:?} both read {:?}", glyph(g));
            }
        }
        // done is the read mark's `✓`, drawn `*` in ASCII (BISE-100)
        assert_eq!(done_glyph(), "*");
        assert!(ALL_GLYPHS.iter().all(|g| glyph(g) != "*"), "* is done's");
        assert_eq!(ellipsis(), "...");
        ascii_cell::set(false);
        assert_eq!(done_glyph(), "✓");
        assert_eq!(ellipsis(), "…");
    }

    /// Every `G_*` glyph constant of the crate (the declarations, read
    /// from the sources: a new one cannot be forgotten) is on a legend
    /// row, and the legend's ASCII forms are ASCII (box drawing aside).
    #[test]
    fn every_glyph_constant_has_a_legend_row() {
        let named: &[(&str, &str)] = &[
            ("G_YOU", G_YOU), ("G_MAIN", G_MAIN), ("G_BRIEF", G_BRIEF), ("G_THINK", G_THINK),
            ("G_BASH", G_BASH), ("G_TS", G_TS), ("G_SUBCALL", G_SUBCALL), ("G_PATCH", G_PATCH),
            ("G_MSG", G_MSG), ("G_IMAGE", G_IMAGE), ("G_QUOTE", G_QUOTE), ("G_PASTE", G_PASTE),
            ("G_CARD", G_CARD),
            ("G_COMPACTING", G_COMPACTING), ("G_SUMMARY", G_SUMMARY), ("G_INTERRUPTED", G_INTERRUPTED),
            ("G_WRAP", G_WRAP), ("G_SCHEDULED", G_SCHEDULED), ("G_STARTING", G_STARTING), ("G_WORKING", G_WORKING),
            ("G_WAITING", G_WAITING), ("G_NEEDS_YOU", G_NEEDS_YOU), ("G_DONE", G_DONE),
            ("G_FAILED", G_FAILED), ("G_IDLE", G_IDLE), ("G_STOPPED", G_STOPPED),
            ("G_SENDING", G_SENDING), ("G_RECEIVED", G_RECEIVED), ("G_READ", G_READ),
            ("G_UNREAD", G_UNREAD), ("G_WORKTREE", G_WORKTREE), ("G_OVERLAP", G_OVERLAP),
            ("G_RESTART_FAILED", G_RESTART_FAILED), ("G_BUILDING", G_BUILDING),
            ("G_CLOSED", G_CLOSED), ("G_OPEN", G_OPEN), ("G_PR", G_PR),
            ("G_ENVELOPE", crate::render::G_ENVELOPE), ("G_NOTE", crate::render::G_NOTE),
        ];
        // the declared ones: a `G_…` constant of type `&str` in any source file
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut declared = std::collections::BTreeSet::new();
        let mut stack = vec![std::path::PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    for l in src.lines() {
                        let Some(at) = l.find("const G_") else { continue };
                        let rest = &l[at + 6..];
                        let name: String = rest.chars().take_while(|c| c.is_ascii_uppercase() || *c == '_').collect();
                        if rest[name.len()..].starts_with(": &str") {
                            declared.insert(name);
                        }
                    }
                }
            }
        }
        let listed: std::collections::BTreeSet<String> = named.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(declared, listed, "a G_* glyph constant is missing from this test's list");
        for (name, g) in named {
            assert!(
                LEGEND.iter().any(|s| s.glyph.split(' ').any(|t| t == *g)),
                "{name} ({g:?}) has no row in the legend (theme::LEGEND)"
            );
        }
        for ascii in [false, true] {
            ascii_cell::set(ascii);
            for s in LEGEND {
                let shown = s.shown();
                if ascii {
                    let odd: String = shown.chars().filter(|c| !c.is_ascii() && !('\u{2500}'..='\u{257f}').contains(c)).collect();
                    assert!(odd.is_empty(), "{:?} → {shown:?} under BISE_ASCII", s.glyph);
                } else {
                    assert_eq!(shown, s.glyph);
                }
            }
        }
        ascii_cell::set(true);
        let of = |g: &str| LEGEND.iter().find(|s| s.glyph == g).unwrap().shown();
        assert_eq!((of(G_WORKTREE).as_str(), of(G_DONE).as_str(), of("· ✓ ✓✓").as_str()), ("Y", "*", ". v vv"));
        assert_eq!(of("opus·hi"), "opus.hi");
        ascii_cell::set(false);
    }

    #[test]
    fn glyph_is_the_identity_when_off() {
        assert!(!ascii_mode());
        for g in ALL_GLYPHS {
            assert_eq!(glyph(g), *g);
        }
    }

    fn buffer_of(text: &str) -> ratatui::buffer::Buffer {
        use ratatui::layout::Rect;
        use unicode_width::UnicodeWidthStr;
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, text.width() as u16, 1));
        buf.set_string(0, 0, text, ratatui::style::Style::default());
        buf
    }

    fn row(buf: &ratatui::buffer::Buffer) -> String {
        buf.content.iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn asciify_maps_only_the_table() {
        let text = "› ∿ ♡ ✓ ψ Δ … · ─│┃ é ñ ü 漢字 👍 « » “q” ✦ ⠋";
        let mut buf = buffer_of(text);
        let before = row(&buf);
        asciify(&mut buf);
        assert_eq!(row(&buf), before, "off: nothing changes");
        ascii_cell::set(true);
        asciify(&mut buf);
        ascii_cell::set(false);
        let after = row(&buf);
        // (wide characters keep their continuation cell: compare buffers)
        assert_eq!(after, row(&buffer_of("> ~ * v Y A ; . -|| é ñ ü 漢字 👍 « } “q” * ~")));
        let non_ascii: String = after.chars().filter(|c| !c.is_ascii()).collect();
        assert_eq!(non_ascii, "éñü漢字👍«“”", "only user text survives");
    }

    /// QA E: the card box (`┎ ┃ ┖ ─ ╮ │ ╯`), the card bar and the rails
    /// turn to `+ - |` like the frame; block elements stay.
    #[test]
    fn asciify_draws_box_drawing_like_the_frame() {
        let text = "┎─╮┃│┖─╯╭╰├┤┬┴┼═║╌╱╲╳▁█";
        let mut buf = buffer_of(text);
        ascii_cell::set(true);
        asciify(&mut buf);
        ascii_cell::set(false);
        assert_eq!(row(&buf), "+-+||+-++++++++-|-/\\x▁█");
    }
}
