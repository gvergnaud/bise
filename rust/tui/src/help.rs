//! /help and /shortcuts: a scrollable overlay built from ONE table of
//! (section, keys, action) rows. A new feature adds one row to `ROWS`.
//!
//! /help shows the commands and the essential keys (rows marked `top`);
//! /shortcuts shows every row. Both end with the symbols, from
//! [`theme::LEGEND`]. Typing filters, Tab switches the page, Esc clears
//! the filter, then closes.

use crate::{theme, App};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// One line of the table. `keys` holds the alternatives separated by
/// `|`; ` then ` inside one alternative is a sequence (Option+e then e).
/// Empty `keys`: a note written across the whole width.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row {
    pub section: &'static str,
    pub keys: &'static str,
    pub action: &'static str,
    /// shown by /help too, not only by /shortcuts
    pub top: bool,
}

const fn r(section: &'static str, keys: &'static str, action: &'static str) -> Row {
    Row { section, keys, action, top: false }
}

impl Row {
    const fn top(mut self) -> Row {
        self.top = true;
        self
    }
}

const TALK: &str = "talk to agents";
const AGENTS: &str = "agents (empty composer)";
const CARDS: &str = "inbox";
const EDIT: &str = "composer editing";
const ACCENTS: &str = "accents & symbols";
const SELECT: &str = "selection & copy";
const FEED: &str = "feed";
const VOICE: &str = "voice";
const TERM: &str = "terminal panel";
const GHOSTTY: &str = "Ghostty tips";

/// The tips of the key bar (BISE-99, book §8): one per session, shown dim
/// on the right while idle. Each starts with a key of [`ROWS`] (or a
/// command); the key bar prefixes "tip · ".
pub(crate) const TIPS: &[&str] = &[
    "ctrl+o opens everything folded",
    "$ calls a skill, tab completes",
    "ctrl+` opens a terminal in the workspace",
    "ctrl+1-9 open the items waiting in your inbox",
    "ctrl+s finds an agent by name, ⏎ opens it",
    "esc puts your draft away, ↑ brings it back",
    "shift+⏎ adds a new line",
    "ctrl+r speaks into the composer (turn it on with /voice)",
    "ctrl+r twice starts voice mode: talk with the agent in view",
    "/theme switches between light and dark",
];

/// ctrl+f; once a cmd key reached us (the terminal passes them,
/// `App::cmd_keys`) the help shows [`FIND_CMD`] in its place.
const FIND: Row = r(FEED, "ctrl+f", FIND_WHAT).top();
const FIND_CMD: Row = r(FEED, "cmd+f|ctrl+f", FIND_WHAT).top();
const FIND_WHAT: &str = "find in the history: ⏎ or ↑ older, shift+⏎ or ↓ newer, esc close (the view stays on the match)";
/// ctrl+s (any terminal) and /switch; cmd+k first once a cmd key reached
/// us, like [`FIND_CMD`] (BISE-265).
const SWITCH: Row = r(AGENTS, "ctrl+s|/switch", SWITCH_WHAT).top();
const SWITCH_CMD: Row = r(AGENTS, "cmd+k|ctrl+s|/switch", SWITCH_WHAT).top();
/// ctrl+1-9 open inbox item N (BISE-302); a terminal without them
/// (`App::ctrl_digits`, reach.rs): /inbox in their place.
const INBOX: Row = r(CARDS, "ctrl+1-9", "open the inbox item with that number (or click it)").top();
const INBOX_COMMAND: Row = r(CARDS, "/inbox", "open the inbox (or click a row)").top();
const SWITCH_WHAT: &str = "find an agent by name (archived ones too) and open it: type part of its name, ↑↓ choose, ⏎ open, esc close";

/// Every shortcut, in display order (sections appear in first-row order).
/// Lowercase, "agent" never "task" (book §4, §16).
#[rustfmt::skip]
pub(crate) const ROWS: &[Row] = &[
    r(TALK, "⏎", "send to the agent in view (main, or the agent you entered); while it works, steer its turn").top(),
    r(TALK, "@agent …", "a direct message to an agent without leaving main; @main … from inside an agent").top(),
    r(TALK, "ctrl+c", "interrupt the turn of the agent in view; again (or at idle) quit, the agents keep running").top(),
    r(TALK, "/", "the commands, then their arguments (agents, inbox items, versions…): tab completes, ⏎ runs").top(),
    r(TALK, "⇧⇥", "switch the approvals mode: yolo / auto (the word after the model, on the divider)").top(),
    r(TALK, "$", "a skill: the popup lists them, tab completes; the agent reads the $name mention").top(),
    r(TALK, "hold ctrl|hold ⌥|hold cmd", "show that key's shortcuts where they act (Ghostty, kitty; cmd once a cmd key reached bise; typing a ⌥ character hides them)").top(),
    r(AGENTS, "⌥ + 0…9", "go to main (0) or to the agent with that number in the panel").top(),
    SWITCH,
    r(AGENTS, "alt+↓", "select the next agent").top(),
    r(AGENTS, "alt+↑", "select the previous agent"),
    r(AGENTS, "⏎", "enter the selected agent").top(),
    r(AGENTS, "space", "preview the selected agent without entering it"),
    r(AGENTS, "D", "archive the selected agent (stop it, keep its history)"),
    r(AGENTS, "A|/archived", "show or hide the archived agents (read-only history, newest first)"),
    r(AGENTS, "esc", "close the selection; in an agent, back to main").top(),
    r(AGENTS, "click an agent", "in the right panel: go to that agent (main: back to main)").top(),
    r(AGENTS, "click ▸ archived", "in the right panel: show or hide the archived agents"),
    INBOX,
    r(CARDS, "↑|↓|⏎", "an item open, empty composer: choose an option, ⏎ picks it (nothing chosen: ⏎ does nothing)").top(),
    r(CARDS, "1-9", "an item open, empty composer: pick an option at once (once you type, digits are text)"),
    r(CARDS, "←|→|ctrl+n|ctrl+p", "an item open: the previous / next one (←→ on an empty composer)"),
    r(CARDS, "type|⏎", "an item open: your text answers it (an approval: denies, your text as a note)"),
    r(CARDS, "ctrl+x", "an item open: close it without answering"),
    r(CARDS, "esc", "an item open: back to your thread; each item keeps its draft"),
    r(CARDS, "pgup|pgdn|wheel", "an item open: scroll a long one"),
    r(CARDS, "y|n|esc", "a confirmation: yes / no / not now"),
    r(FEED, "click ▸|space", "open or close one folded item: thinking, an output, a diff, a report (space: the item selected in the feed, composer empty)"),
    r(FEED, "ctrl+o", "open or close everything folded").top(),
    FIND,
    r(FEED, "/artifacts", "what your agents made, full screen: / find, ⏎ open, space quick look, v versions, @ put it in a message").top(),
    r(FEED, "click ↗", "an artifact's chip: open it (a page in the browser, a .md in your editor, a file in its app)"),
    r(FEED, "ctrl+g|/diff", "the diff panel: the agent in view's changes against main (/diff <branch>: any branch); ctrl+g again closes it"),
    r(FEED, "click ± 3 files|click ψ", "the diff of what an agent landed, or of its branch (the composer keeps the keys; a click in the panel takes them)"),
    r(FEED, "↑↓|tab|⏎|esc", "in the diff panel with the keys: move, next file (shift+tab: previous), open the line in your editor or unfold, esc closes; typing writes in the composer"),
    r(FEED, "pgup|pgdn|wheel", "scroll the feed"),
    r(FEED, "end", "back to the bottom"),
    r(FEED, "ctrl+l", "clear the display (/clear); scroll up to see the lines again"),
    r(FEED, "ctrl+y|click copy", "copy a code block: the one under the mouse (its copy on hover), else the newest on screen"),
    r(EDIT, "shift+⏎|alt+⏎|ctrl+j", "new line").top(),
    r(EDIT, "option+←|option+→", "word left / right"),
    r(EDIT, "ctrl+option+←|ctrl+option+→", "subword left / right (camelCase, snake_case, kebab-case, digits)"),
    r(EDIT, "cmd+←|cmd+→|ctrl+a|ctrl+e|home|end", "line start / end"),
    r(EDIT, "ctrl+home|ctrl+end|cmd+↑|cmd+↓", "text start / end; with shift, select to there (cmd+↑/↓: see Ghostty tips)"),
    r(EDIT, "↑|↓", "move between rows, then through the history (↓ past the newest brings the draft back)"),
    r(EDIT, "option+backspace|ctrl+w", "delete the word before"),
    r(EDIT, "option+delete", "delete the word after"),
    r(EDIT, "ctrl+option+backspace|ctrl+option+delete", "delete a subword before / after"),
    r(EDIT, "cmd+backspace|ctrl+u", "delete to the line start"),
    r(EDIT, "ctrl+k", "delete to the line end"),
    r(EDIT, "ctrl+/|cmd+z", "undo your typing (only the composer: sent messages have no undo)"),
    r(EDIT, "alt+/|ctrl+shift+/|cmd+shift+z", "redo"),
    r(EDIT, "esc", "put the draft away in the history (↑ brings it back)"),
    r(EDIT, "tab|⏎", "pick from the /, @ or $ popup (esc closes it)"),
    r(EDIT, ":name:", "typed, becomes its emoji (:tada: → 🎉)").top(),
    r(EDIT, "typing", "zen: the edges fade while you type, back 5 s after your last key (or at once on ⏎, esc, a shortcut)"),
    r(ACCENTS, "option+` then e", "è (grave)"),
    r(ACCENTS, "option+e then e", "é (acute)"),
    r(ACCENTS, "option+i then o", "ô (circumflex)"),
    r(ACCENTS, "option+u then u", "ü (umlaut)"),
    r(ACCENTS, "option+n then n", "ñ (tilde)"),
    r(ACCENTS, "option+c|option+q|option+\\|option+shift+\\", "ç œ « » — every macOS U.S. option character works"),
    r(SELECT, "shift + any move", "extend the composer selection"),
    r(SELECT, "cmd+a", "select the whole composer text (see Ghostty tips)"),
    r(SELECT, "click|drag|shift+click", "composer: place the cursor, select, extend"),
    r(SELECT, "double click|triple click", "select a word / everything (feed: the word / the row)"),
    r(SELECT, "drag in the feed", "select; the release copies it"),
    r(SELECT, "select, then type", "ask about it: the selection goes in as a quote ❝ (backspace on it removes it)"),
    r(SELECT, "ctrl+shift+c|cmd+c", "copy the composer selection, else the feed's").top(),
    r(SELECT, "ctrl+shift+x|cmd+x", "cut"),
    r(SELECT, "esc", "drop the selection"),
    r(SELECT, "shift+drag", "the terminal's own selection (outside the app)"),
    r(VOICE, "ctrl+r", "dictation into the composer (turn it on with /voice)").top(),
    r(VOICE, "any key", "while recording: stop, keep the text"),
    r(VOICE, "esc|ctrl+c", "while recording: stop and drop the text"),
    r(VOICE, "ctrl+r twice", "voice mode: talk with the agent in view").top(),
    r(VOICE, "space", "in voice mode: send now"),
    r(VOICE, "hold space", "in voice mode: keep the floor, or cut in on speakers"),
    r(VOICE, "m", "in voice mode: mute"),
    r(VOICE, "tab", "in voice mode: type instead"),
    r(VOICE, "esc|ctrl+c", "leave voice mode"),
    r(TERM, "ctrl+`|ctrl+space", "show or hide the terminal panel: a shell in the workspace, kept running while hidden").top(),
    r(TERM, "any key", "while shown: goes to the shell, ctrl+c included"),
    r(TERM, "shift+pgup|shift+pgdn|wheel", "scroll its history"),
    r(TERM, "drag|double click|triple click", "select (the word, the row); the release copies it"),
    r(TERM, "cmd+c|ctrl+shift+c", "copy the selection (ctrl+shift+c with none: the shell's ctrl+c)"),
    r(TERM, "shift+drag", "select when the program in it takes the mouse (vim, less, htop)"),
    r(TERM, "drag the top border", "resize it"),
    r(GHOSTTY, "", "Ghostty keeps cmd+↑/↓, cmd+a, cmd+c and cmd+z by default. to get them in the composer, add to ~/Library/Application Support/com.mitchellh.ghostty/config:"),
    r(GHOSTTY, "", "keybind = super+a=unbind (cmd+a selects the whole composer text; Ghostty's selects the screen)"),
    r(GHOSTTY, "", "keybind = super+arrow_up=unbind (cmd+↑ goes to the composer text's start; Ghostty's jumps to the previous shell prompt)"),
    r(GHOSTTY, "", "keybind = super+arrow_down=unbind"),
    r(GHOSTTY, "", "keybind = super+shift+arrow_up=unbind (cmd+shift+↑ selects to the start)"),
    r(GHOSTTY, "", "keybind = super+shift+arrow_down=unbind"),
    r(GHOSTTY, "", "keybind = super+z=unbind"),
    r(GHOSTTY, "", "keybind = super+shift+z=unbind"),
    r(GHOSTTY, "", "keybind = super+c=performable:copy_to_clipboard (cmd+c copies Ghostty's selection if any, else the app's)"),
    r(GHOSTTY, "", "keybind = super+f=unbind (cmd+f finds in the history; Ghostty's own find keeps its menu item, not cmd+f)"),
    r(GHOSTTY, "", "keybind = super+k=unbind (cmd+k finds an agent by name; Ghostty's cmd+k clears the screen)"),
    r(GHOSTTY, "", "check what reaches the app: bise keyprobe"),
];

// ---- the overlay state ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Page {
    Help,
    Shortcuts,
}

#[derive(Debug)]
pub(crate) struct Overlay {
    page: Page,
    pub(crate) filter: String,
    scroll: usize,
    // set by the last draw
    max_scroll: usize,
    visible: usize,
}

impl Overlay {
    pub(crate) fn new(page: Page) -> Overlay {
        Overlay { page, filter: String::new(), scroll: 0, max_scroll: 0, visible: 1 }
    }
}

/// The page a command opens: /help, /shortcuts and its aliases.
pub(crate) fn page_of(cmd: &str) -> Option<Page> {
    match cmd {
        "/help" => Some(Page::Help),
        "/shortcuts" | "/shortcut" | "/keys" => Some(Page::Shortcuts),
        _ => None,
    }
}

/// Keys while the overlay is open: it takes them all. `true` when
/// handled (always, while open).
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(o) = app.help.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    // cmd+a, cmd+c…: never typed into the filter
    let cmd = k.modifiers.contains(KeyModifiers::SUPER);
    let page = o.visible.saturating_sub(1).max(1);
    match k.code {
        KeyCode::Esc if !o.filter.is_empty() => {
            o.filter.clear();
            o.scroll = 0;
        }
        KeyCode::Esc => app.help = None,
        KeyCode::Char('c') if ctrl => app.help = None,
        KeyCode::Tab | KeyCode::BackTab => {
            o.page = match o.page {
                Page::Help => Page::Shortcuts,
                Page::Shortcuts => Page::Help,
            };
            o.scroll = 0;
        }
        KeyCode::Up => o.scroll = o.scroll.saturating_sub(1),
        KeyCode::Down => o.scroll = (o.scroll + 1).min(o.max_scroll),
        KeyCode::PageUp => o.scroll = o.scroll.saturating_sub(page),
        KeyCode::PageDown => o.scroll = (o.scroll + page).min(o.max_scroll),
        KeyCode::Home => o.scroll = 0,
        KeyCode::End => o.scroll = o.max_scroll,
        KeyCode::Backspace => {
            o.filter.pop();
            o.scroll = 0;
        }
        KeyCode::Char(c) if !ctrl && !cmd => {
            o.filter.push(c);
            o.scroll = 0;
        }
        _ => {}
    }
    true
}

/// The mouse while the overlay is open: the wheel scrolls it, the
/// rest is swallowed. `true` when handled.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let Some(o) = app.help.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => o.scroll = o.scroll.saturating_sub(3),
        MouseEventKind::ScrollDown => o.scroll = (o.scroll + 3).min(o.max_scroll),
        _ => {}
    }
    true
}

// ---- rendering (pure: rows in, lines out) ----

/// A key cap: accent and bold, no painted background (book §5).
fn chip() -> Style {
    Style::new().fg(theme::accent()).add_modifier(Modifier::BOLD)
}

/// A config line to copy (the Ghostty tips): the code string color.
fn code() -> Style {
    Style::new().fg(theme::syntax_string())
}

/// The rows of a page, filtered (case-insensitive, over the
/// section, the keys and the action). `cmd`: the terminal passes cmd
/// keys, find reads cmd+f. `digits`: it passes ctrl+1-9 (else /inbox).
pub(crate) fn rows(page: Page, filter: &str, cmd: bool, digits: bool) -> Vec<&'static Row> {
    let f = filter.to_lowercase();
    ROWS.iter()
        .map(|r| {
            if cmd && r.keys == FIND.keys {
                &FIND_CMD
            } else if cmd && r.keys == SWITCH.keys {
                &SWITCH_CMD
            } else if !digits && r.keys == INBOX.keys {
                &INBOX_COMMAND
            } else {
                r
            }
        })
        .filter(|r| page == Page::Shortcuts || r.top)
        .filter(|r| {
            f.is_empty()
                || [r.section, r.keys, r.action]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&f))
        })
        .collect()
}

/// Word wrap `text` into rows of at most `width` columns (a word longer
/// than a row is cut).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if !cur.is_empty() && cur.width() + 1 + word.width() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
        while cur.width() > width {
            let mut head = String::new();
            for ch in cur.chars() {
                if head.width() + ch.width().unwrap_or(0) > width {
                    break;
                }
                head.push(ch);
            }
            if head.is_empty() {
                break;
            }
            cur = cur[head.len()..].to_string();
            out.push(head);
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

fn spans_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// A key as drawn: in ASCII mode `⌥` reads `alt`, `⇧⇥` `shift+tab` and `…` reads `...`
/// (QA 12: the cell-by-cell net would give `M + 0;9`).
fn key_text(step: &str) -> String {
    if theme::ascii_mode() {
        step.replace('⌥', "alt").replace('…', "...").replace("⇧⇥", "shift+tab")
    } else {
        step.to_string()
    }
}

/// The chips of one keys field, as rows of spans at most `width` wide
/// (an alternative never splits).
fn chip_rows(keys: &str, width: usize) -> Vec<Vec<Span<'static>>> {
    let units = keys.split('|').map(|alt| {
        let mut u = Vec::new();
        for (i, step) in alt.split(" then ").enumerate() {
            if i > 0 {
                u.push(Span::styled(" then ", Style::default().fg(theme::dim())));
            }
            u.push(Span::styled(format!(" {} ", key_text(step)), chip()));
        }
        u
    });
    let mut out: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    for u in units {
        let line = out.last_mut().unwrap();
        let used = spans_width(line);
        if used > 0 && used + 1 + spans_width(&u) > width {
            out.push(u);
        } else {
            if used > 0 {
                line.push(Span::raw(" "));
            }
            line.extend(u);
        }
    }
    out
}

fn header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(theme::accent()).add_modifier(Modifier::BOLD),
    ))
}

/// The two-column body (keys | action) of `rows`, `width` columns wide,
/// a header per section.
pub(crate) fn table_lines(rows: &[&Row], width: usize) -> Vec<Line<'static>> {
    let width = width.max(20);
    // the key column: its chips wrap at a third of the width (30 at
    // most); one alternative wider than that widens the column
    let cap = (width / 3).clamp(16, 30);
    let key_w = rows
        .iter()
        .filter(|r| !r.keys.is_empty())
        .flat_map(|r| chip_rows(r.keys, cap))
        .map(|l| spans_width(&l))
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(10));
    let act_w = width.saturating_sub(key_w + 2).max(8);
    let mut out = Vec::new();
    let mut section = "";
    for r in rows {
        if r.section != section {
            if !out.is_empty() {
                out.push(Line::default());
            }
            out.push(header(r.section));
            section = r.section;
        }
        if r.keys.is_empty() {
            let style = if r.action.starts_with("keybind") { code() } else { Style::default() };
            for l in wrap(r.action, width.saturating_sub(2)) {
                out.push(Line::from(vec![Span::raw("  "), Span::styled(l, style)]));
            }
            continue;
        }
        let keys = chip_rows(r.keys, key_w);
        let acts = wrap(r.action, act_w);
        for i in 0..keys.len().max(acts.len()) {
            let mut spans = keys.get(i).cloned().unwrap_or_default();
            let used = spans_width(&spans);
            spans.push(Span::raw(" ".repeat(key_w + 2 - used.min(key_w + 2))));
            if let Some(a) = acts.get(i) {
                spans.push(Span::raw(a.clone()));
            }
            out.push(Line::from(spans));
        }
    }
    out
}

/// The symbols section (book §6): one row per [`theme::LEGEND`] entry,
/// the glyph in its screen color (its ASCII form under `BISE_ASCII=1`),
/// then its meaning; a faint title per group. Filtered like the keys.
pub(crate) fn symbol_lines(filter: &str, width: usize) -> Vec<Line<'static>> {
    let f = filter.to_lowercase();
    let rows: Vec<_> = theme::LEGEND
        .iter()
        .filter(|s| {
            f.is_empty()
                || ["symbols", s.group, s.glyph, &s.shown(), s.meaning]
                    .iter()
                    .any(|t| t.to_lowercase().contains(&f))
        })
        .collect();
    if rows.is_empty() {
        return Vec::new();
    }
    // one column for every row, filtered or not: the width of the longest + 2
    let glyph_w = theme::LEGEND.iter().map(|s| s.shown().width()).max().unwrap_or(1) + 2;
    let mean_w = width.saturating_sub(2 + glyph_w).max(8);
    let mut out = vec![header("symbols")];
    let mut group = "";
    for s in rows {
        if s.group != group {
            out.push(Line::from(Span::styled(format!("  {}", s.group), Style::default().fg(theme::faint()))));
            group = s.group;
        }
        let shown = s.shown();
        for (i, l) in wrap(s.meaning, mean_w).into_iter().enumerate() {
            let g = if i == 0 { shown.as_str() } else { "" };
            out.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:<w$}", g, w = glyph_w), Style::default().fg(s.tone.color())),
                Span::raw(l),
            ]));
        }
    }
    out
}

/// The whole text of a page: /help = the commands + the essential keys,
/// /shortcuts = the full table; both end with the symbols.
pub(crate) fn page_lines(
    page: Page,
    filter: &str,
    commands: &[(&'static str, &'static str)],
    width: usize,
    cmd: bool,
    digits: bool,
) -> Vec<Line<'static>> {
    let rows = rows(page, filter, cmd, digits);
    let mut out = Vec::new();
    if page == Page::Help {
        let f = filter.to_lowercase();
        let cmds: Vec<_> = commands
            .iter()
            .filter(|(n, d)| f.is_empty() || n.contains(&f) || d.to_lowercase().contains(&f))
            .collect();
        // where every key is, first: the list below outgrows a small
        // screen (BISE-135 added /model and /reasoning)
        if !rows.is_empty() {
            let hint = Style::default().fg(theme::dim()).add_modifier(Modifier::ITALIC);
            for l in wrap("essential keys · every key: /shortcuts (tab here) · symbols at the end", width) {
                out.push(Line::from(Span::styled(l, hint)));
            }
        }
        if !cmds.is_empty() {
            out.push(header("commands"));
            let name_w = cmds.iter().map(|(n, _)| n.width()).max().unwrap_or(0);
            let desc_w = width.saturating_sub(name_w + 2).max(8);
            for (n, d) in cmds {
                for (i, l) in wrap(d, desc_w).into_iter().enumerate() {
                    let name = if i == 0 { *n } else { "" };
                    out.push(Line::from(vec![
                        Span::styled(format!("{:<w$}  ", name, w = name_w), Style::default().fg(theme::accent())),
                        Span::raw(l),
                    ]));
                }
            }
            out.push(Line::default());
        }
    }
    out.extend(table_lines(&rows, width));
    let symbols = symbol_lines(filter, width);
    if !symbols.is_empty() && !out.is_empty() {
        out.push(Line::default());
    }
    out.extend(symbols);
    if out.is_empty() {
        out.push(Line::from(Span::styled(
            format!("nothing matches “{}” · backspace or esc", filter),
            Style::default().fg(theme::dim()),
        )));
    }
    out
}

/// The overlay, over the whole frame, when open.
pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let dev_cmds = crate::sb::release::dev_commands(app);
    let (cmd, digits) = (app.cmd_keys, app.ctrl_digits);
    let Some(o) = app.help.as_mut() else { return };
    let full = frame.area();
    // BISE-272: open, it takes the mouse (`mouse`): no click does anything
    crate::pointer::region(full, crate::pointer::Shape::Default);
    if full.width < 24 || full.height < 6 {
        return;
    }
    let w = full.width.saturating_sub(2).min(110);
    let h = full.height.saturating_sub(2);
    let area = Rect { x: full.x + (full.width - w) / 2, y: full.y + 1, width: w, height: h };
    let commands: Vec<(&'static str, &'static str)> = crate::commands::COMMANDS
        .iter()
        .chain(dev_cmds)
        .map(|c| (c.name, c.desc))
        .collect();
    let lines = page_lines(o.page, &o.filter, &commands, (w as usize).saturating_sub(4), cmd, digits);
    let visible = (h as usize).saturating_sub(2).max(1);
    o.visible = visible;
    o.max_scroll = lines.len().saturating_sub(visible);
    o.scroll = o.scroll.min(o.max_scroll);
    let tab = |p: Page, name: &str| {
        let style = if o.page == p {
            Style::default().fg(Color::Black).bg(theme::accent()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::dim())
        };
        Span::styled(format!(" {} ", name), style)
    };
    let mut title = vec![
        Span::raw(" "),
        tab(Page::Help, "/help"),
        Span::raw(" "),
        tab(Page::Shortcuts, "/shortcuts"),
        Span::raw(" "),
    ];
    if !o.filter.is_empty() {
        title.push(Span::styled(format!(" filter: {}▏ ", o.filter), Style::default().fg(theme::accent())));
    }
    let pos = if o.max_scroll > 0 {
        format!("{} ↑↓ PgUp/PgDn · ", rows_shown(o.scroll, visible, lines.len()))
    } else {
        String::new()
    };
    let foot = format!(" {}type to filter · tab switch · esc close ", pos);
    scroll_box(frame, area, theme::accent(), Line::from(title), foot, lines, o.scroll);
}

/// "12–40/85": the rows a scrolled box shows, of how many.
pub(crate) fn rows_shown(scroll: usize, visible: usize, total: usize) -> String {
    format!("{}–{}/{}", scroll + 1, (scroll + visible).min(total), total)
}

/// A box over what is under it: `lines` scrolled by `scroll` rows, the
/// border and `title` in `color`, `foot` dim at the bottom right.
pub(crate) fn scroll_box(
    frame: &mut Frame,
    area: Rect,
    color: Color,
    title: Line<'static>,
    foot: String,
    lines: Vec<Line<'static>>,
    scroll: usize,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(title)
        .title_bottom(Line::from(Span::styled(foot, Style::default().fg(theme::dim()))).right_aligned())
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll as u16, 0)), area);
    // BISE-290: its text selects, copies and has links
    crate::textlayer::text(inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every entry of the table renders,
    /// narrow and wide, and no line overflows the width.
    #[test]
    fn every_row_renders() {
        for width in [40usize, 76, 106] {
            {
                let lines = page_lines(Page::Shortcuts, "", &[], width, false, true);
                let all = text(&lines);
                for l in &lines {
                    let w = spans_width(&l.spans);
                    assert!(w <= width, "overflow {} > {}: {:?}", w, width, l);
                }
                for r in ROWS.iter() {
                    assert!(all.contains(r.section), "section {}", r.section);
                    for alt in r.keys.split('|').filter(|k| !k.is_empty()) {
                        for step in alt.split(" then ") {
                            assert!(all.contains(&format!(" {} ", step)), "key {} at {}", step, width);
                        }
                    }
                    // the action, rewrapped: every word is there
                    for word in r.action.split(' ').filter(|w| w.width() <= width - 2) {
                        assert!(all.contains(word), "action {} ({})", r.action, word);
                    }
                }
            }
        }
    }

    #[test]
    fn help_is_commands_and_essentials() {
        let lines = page_lines(Page::Help, "", &[("/help", "commands and keys")], 80, false, true);
        let all = text(&lines);
        assert!(all.contains("commands") && all.contains("/help"));
        assert!(all.contains(" ctrl+1-9 "), "a top row");
        // ctrl+j stays a new line, ctrl+k the line end (/shortcuts)
        for gone in ["ctrl+g", "ctrl+k", "select an agent"] {
            assert!(!all.contains(gone), "BISE-302: {gone} in\n{all}");
        }
        // a terminal without ctrl+1-9: /inbox in their place
        let all = text(&page_lines(Page::Help, "", &[], 80, false, false));
        assert!(all.contains(" /inbox ") && !all.contains("open the inbox item with that number"), "{all}");
        assert!(all.contains(" ⌥ + 0…9 "), "the panel numbers");
        assert!(!all.contains(" ctrl+x "), "a /shortcuts-only row");
    }

    /// The voice keys: dictation, ctrl+r twice for voice mode (in /help
    /// too), and voice mode's own keys (designer's words).
    #[test]
    fn help_says_ctrl_r_twice_and_the_voice_mode_keys() {
        let v: Vec<(&str, &str)> = ROWS.iter().filter(|r| r.section == VOICE).map(|r| (r.keys, r.action)).collect();
        for want in [
            ("ctrl+r", "dictation into the composer (turn it on with /voice)"),
            ("ctrl+r twice", "voice mode: talk with the agent in view"),
            ("space", "in voice mode: send now"),
            ("hold space", "in voice mode: keep the floor, or cut in on speakers"),
            ("m", "in voice mode: mute"),
            ("tab", "in voice mode: type instead"),
            ("esc|ctrl+c", "leave voice mode"),
        ] {
            assert!(v.contains(&want), "{want:?} in {v:?}");
        }
        let all = text(&page_lines(Page::Help, "", &[], 80, false, true));
        assert!(all.contains(" ctrl+r twice ") && all.contains("voice mode: talk with the agent in view"), "{all}");
        assert!(TIPS.contains(&"ctrl+r twice starts voice mode: talk with the agent in view"));
    }

    /// Book §4 and §16: lowercase words (keys may name a capital letter,
    /// proper nouns keep theirs), "agent" never "task", no undo row.
    #[test]
    fn rows_read_lowercase_and_say_agent() {
        let proper = ["Ghostty", "macOS", "U.S.", "Library/Application", "Support/com", "Camel", "D", "A"];
        for r in ROWS {
            for text in [r.section, r.keys, r.action] {
                for w in text.split(|c: char| c.is_whitespace() || c == '|' || c == '(' || c == ',') {
                    let first = w.chars().next().unwrap_or(' ');
                    let ok = !first.is_uppercase() || proper.iter().any(|p| w.starts_with(p));
                    assert!(ok, "capital in {:?}: {:?}", text, w);
                }
                assert!(!text.to_lowercase().contains("task"), "task in {:?}", text);
            }
        }
        assert!(!ROWS.iter().any(|r| r.keys.contains("ctrl+z")), "no undo (book §13)");
        let o: Vec<_> = ROWS.iter().filter(|r| r.keys.split('|').any(|k| k == "ctrl+o")).collect();
        assert_eq!(o.len(), 1, "ctrl+o is only the folds (no shell)");
        assert_eq!(o[0].action, "open or close everything folded");
        assert!(!ROWS.iter().any(|r| r.keys.split('|').any(|k| k == "ctrl+t")), "ctrl+t is gone");
    }

    /// The symbols end both pages, one row per legend entry, the ψ row
    /// says worktree, in both modes; nothing overflows.
    #[test]
    fn both_pages_end_with_the_symbols() {
        for ascii in [false, true] {
            theme::set_ascii_for_tests(ascii);
            for page in [Page::Help, Page::Shortcuts] {
                for width in [40usize, 106] {
                    let lines = page_lines(page, "", &[], width, false, true);
                    for l in &lines {
                        assert!(spans_width(&l.spans) <= width, "overflow at {width}: {l:?}");
                    }
                    let all = text(&lines);
                    let at = all.find("\nsymbols\n").expect("a symbols section");
                    let sym = &all[at..];
                    for s in theme::LEGEND {
                        assert!(sym.contains(&format!("  {}  ", s.shown())) || sym.contains(&format!("  {} ", s.shown())), "{:?}", s.glyph);
                    }
                    let psi = if ascii { "Y" } else { "ψ" };
                    let row = sym.lines().find(|l| l.trim_start().starts_with(psi)).unwrap();
                    assert!(row.contains("worktree"), "{row}");
                    for g in ["agents", "messages", "history"] {
                        assert!(sym.contains(&format!("\n  {g}\n")), "group {g}");
                    }
                }
            }
        }
        theme::set_ascii_for_tests(false);
        // the filter finds a symbol by its meaning, its glyph or its ASCII form
        let only = text(&page_lines(Page::Help, "worktree", &[], 80, false, true));
        assert!(only.contains("symbols") && only.contains("ψ"), "{only}");
        assert!(text(&page_lines(Page::Shortcuts, "symbols", &[], 80, false, true)).contains("✓✓"));
    }

    #[test]
    fn filter_keeps_matching_rows() {
        let r = rows(Page::Shortcuts, "subword", false, true);
        assert!(r.len() >= 2 && r.iter().all(|r| r.action.contains("subword")));
        assert!(rows(Page::Shortcuts, "zzzz", false, true).is_empty());
    }

    #[test]
    fn aliases_open_shortcuts() {
        for c in ["/shortcuts", "/shortcut", "/keys"] {
            assert_eq!(page_of(c), Some(Page::Shortcuts));
        }
        assert_eq!(page_of("/help"), Some(Page::Help));
        assert_eq!(page_of("/h"), None);
    }
}
