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
    /// a bare fence's block (``` with no language): `text` 90 % toward
    /// the ground (oklab), gray enough to say "code" (code-fences)
    pub syntax_bare: Color,
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
    syntax_bare: rgb(0xd3cec3),
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
    // string and comment ≥ 4.5:1 on the desktop diff's +/- tints too (diff-syntax, designer m_53174)
    syntax_string: rgb(0x417228),
    syntax_comment: rgb(0x6c665b),
    syntax_number: rgb(0x9a4a0c),
    syntax_call: rgb(0x1f63a8),
    syntax_type: rgb(0x7a5c00),
    syntax_bare: rgb(0x2e2b29),
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
/// A bare fence's block (``` with no language): gray text.
pub(crate) fn syntax_bare() -> Color {
    palette().syntax_bare
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
pub(crate) const G_VIEW: &str = "⊞"; // an MCP Apps view under its tool row (dim; designer m_50266)
pub(crate) const G_QUOTE: &str = "❝"; // a quote of the history (accent chip, BISE-134)
pub(crate) const G_PASTE: &str = "▤"; // a long paste (accent chip, BISE-240)
pub(crate) const G_FILE: &str = "▯"; // a file he attached (accent chip, file-chips: designer m_21765)
pub(crate) const G_PLUGIN: &str = "◆"; // a plugin he picked with @ (accent chip and row, at-plugins)
pub(crate) const G_CARD: &str = "?"; // a card: a decision that needs you (accent)
pub(crate) const G_COMPACTING: &str = "≡"; // compaction running (dim, pulsing; was ⟳)
pub(crate) const G_SUMMARY: &str = "≡"; // compaction summary (dim, still)
pub(crate) const G_INTERRUPTED: &str = "▲"; // turn interrupted (dim)
pub(crate) const G_WRAP: &str = "»"; // a wrapped code row continues (faint; was ↪)
pub(crate) const G_SCHEDULED: &str = "◷"; // a scheduled task (sb every), its lines and next run (faint; site/m/timers)

// agent status
pub(crate) const G_STARTING: &str = "·"; // dim, pulsing
pub(crate) const G_WORKING: &str = "∿"; // pulsing: a breeze
pub(crate) const G_WAITING: &str = "…"; // waiting on another agent, or paused on an event
pub(crate) const G_WATCHING: &str = "◌"; // an alarm: bise wakes the agent if an event comes (faint; agent-waiting-states A)
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
    sym(AGENTS, G_WAITING, Tone::Text, "waiting on another agent, or paused until a command ends"),
    sym(AGENTS, G_WATCHING, Tone::Faint, "watching: bise wakes the agent if something happens (/scheduled)"),
    sym(AGENTS, G_NEEDS_YOU, Tone::Accent, "needs you: a notification in your inbox (dim: a blocked task, main's to handle)"),
    Symbol { group: AGENTS, glyph: G_DONE, tone: Tone::Accent, ascii: "*", meaning: "done" },
    sym(AGENTS, G_FAILED, Tone::Error, "failed"),
    sym(AGENTS, G_IDLE, Tone::Dim, "idle"),
    sym(AGENTS, G_STOPPED, Tone::Dim, "stopped"),
    sym(AGENTS, G_UNREAD, Tone::Accent, "unread activity in that agent"),
    sym(AGENTS, G_WORKTREE, Tone::Dim, "worktree: an agent's own copy of the repo, on its own branch (hover a ψ to see which)"),
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
    sym(MESSAGES, G_FILE, Tone::Accent, "a file you attached"),
    sym(MESSAGES, "●", Tone::Accent, "recording your voice"),
    sym(HISTORY, G_BRIEF, Tone::Text, "an agent's brief"),
    sym(HISTORY, G_THINK, Tone::Dim, "thinking"),
    sym(HISTORY, G_BASH, Tone::Text, "a bash call"),
    sym(HISTORY, G_TS, Tone::Text, "a TypeScript call"),
    sym(HISTORY, G_SUBCALL, Tone::Text, "a call inside a TypeScript run"),
    sym(HISTORY, G_PATCH, Tone::Text, "a file edit"),
    sym(HISTORY, G_VIEW, Tone::Dim, "a tool's view: open it in the app or your browser (/views)"),
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
    ("⊞", "V"),
    ("❝", "\""),
    ("▤", "T"),
    ("▯", "F"),
    ("≡", "="),
    ("▲", "^"),
    ("»", "}"),
    ("·", "."),
    ("∿", "~"),
    ("…", ";"),
    ("✗", "x"),
    ("○", "o"),
    ("◌", "O"),
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
mod tests;
