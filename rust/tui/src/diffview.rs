//! The diff panel (site/m/artifacts, D): an agent's changes, or any
//! branch, against main, drawn in the TUI. It opens on the right only
//! when you ask (ctrl+g, a click on `± 3 files` under a landed line, on
//! an agent's ψ in the panel, `/diff <branch>`), takes the agents
//! panel's place (80 columns wide at 150), and closes back to it. Under
//! [`SIDE_FROM`] columns there is no right side: it takes the screen.
//!
//! What it shows comes from the hub (`{"op":"diff"}` → `ev diff`,
//! docs/artifacts.md): the branch vs main, commits and changes not
//! committed yet together. A header with the files and the +/− counts,
//! then each file with its hunks, old and new line numbers, added lines
//! tinted green, removed ones red. Many files: the first 6 on top, `f`
//! the whole list with a filter. A file over [`BIG`] changed lines shows
//! its first hunks and `▸ 212 more lines in this file · ⏎ shows them`;
//! lock and generated files fold by themselves; an image is one row
//! that opens it.
//!
//! Keys (the page's no-clash table, designer m_7291: a letter is never
//! lost and never does something you didn't mean): opened by a key
//! (ctrl+g, `/diff`) the panel has the keys, its title in the accent,
//! the composer dim with no cursor (`the diff has the keys · type to
//! write here`); opened by a click it leaves them to the composer. A
//! click in it takes them; a click out of it, a paste, or any key that
//! isn't the panel's gives them back (a letter types in the composer).
//! On the right its keys are ↑↓ pgup pgdn home end, tab / shift+tab the
//! next / previous file, ⏎ opens the line in your editor or unfolds,
//! esc closes; ctrl+g closes it from anywhere (its title row says so).
//! Full screen (no composer) also keeps j k ] [ and `f` the file list;
//! there a click or ⏎ on `all 9 files ▸` opens it too.
//!
//! A drag over the lines (or shift+↑↓) selects them; typing quotes them
//! in the composer, like a selection in the thread: diffquote.rs. The
//! `/diff` picker's branches are diffbranches.rs.

use crate::app::App;
use crate::theme::{self, accent, dim, faint, text};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use serde_json::Value;
use std::collections::HashSet;
use unicode_width::UnicodeWidthStr;

/// From this many columns the panel is on the right; under it, the
/// whole screen.
pub(crate) const SIDE_FROM: u16 = 120;
/// The panel's width at most (unified diff, never side by side).
pub(crate) const PANEL_W: u16 = 80;
/// A file with more changed lines than this shows its first hunks only.
pub(crate) const BIG: usize = 200;
/// The files listed on top before `↓ 3 more`.
const TOP_FILES: usize = 6;

/// What the panel shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ask {
    /// an agent's changes (its branch, or its files in the shared folder)
    Agent(String),
    /// a branch vs main
    Branch(String),
    /// a GitHub PR
    Pr(u64),
    /// a landed range `from..to`, by an agent
    Range(String, String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Hunk {
    pub(crate) old: u32,
    pub(crate) new: u32,
    pub(crate) head: String,
    /// ` x`, `-x`, `+x`
    pub(crate) lines: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct File {
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
    /// M A D R
    pub(crate) status: String,
    pub(crate) add: usize,
    pub(crate) del: usize,
    pub(crate) binary: bool,
    pub(crate) image: bool,
    pub(crate) generated: bool,
    /// its absolute path (⏎ opens it in your editor)
    pub(crate) abs: String,
    /// the hub cut it (over 5000 lines)
    pub(crate) cut: bool,
    pub(crate) hunks: Vec<Hunk>,
}

impl File {
    fn changed(&self) -> usize {
        self.add + self.del
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Diff {
    pub(crate) title: String,
    pub(crate) branch: String,
    pub(crate) commits: u64,
    pub(crate) uncommitted: bool,
    pub(crate) landed_ms: Option<u64>,
    pub(crate) working: bool,
    pub(crate) files: Vec<File>,
    /// a real failure (drawn with ▲)
    pub(crate) error: String,
    /// a state in plain words, not a failure: `t1 is archived and its
    /// folder is gone`, `there's no branch named sb/x.` (designer m_7393)
    pub(crate) note: String,
    /// the agent's folder is gone (its last land is offered)
    pub(crate) gone: bool,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn n(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
}
fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

impl Diff {
    /// The hub's `diff` event.
    pub(crate) fn of(v: &Value) -> Diff {
        let files = v
            .get("files")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .map(|f| File {
                        path: s(f, "path"),
                        old_path: f.get("old_path").and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from),
                        status: {
                            let st = s(f, "status");
                            if st.is_empty() { "M".into() } else { st }
                        },
                        add: n(f, "add") as usize,
                        del: n(f, "del") as usize,
                        binary: b(f, "binary"),
                        image: b(f, "image"),
                        generated: b(f, "generated"),
                        abs: s(f, "abs"),
                        cut: b(f, "cut"),
                        hunks: f
                            .get("hunks")
                            .and_then(|x| x.as_array())
                            .map(|hs| {
                                hs.iter()
                                    .map(|h| Hunk {
                                        old: n(h, "old") as u32,
                                        new: n(h, "new") as u32,
                                        head: s(h, "head"),
                                        lines: h
                                            .get("lines")
                                            .and_then(|x| x.as_array())
                                            .map(|ls| ls.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Diff {
            title: s(v, "title"),
            branch: s(v, "branch"),
            commits: n(v, "commits"),
            uncommitted: b(v, "uncommitted"),
            landed_ms: v.get("landed_ms").and_then(|x| x.as_u64()),
            working: b(v, "working"),
            files,
            error: s(v, "error"),
            note: s(v, "note"),
            gone: b(v, "gone"),
        }
    }

    pub(crate) fn add(&self) -> usize {
        self.files.iter().map(|f| f.add).sum()
    }
    pub(crate) fn del(&self) -> usize {
        self.files.iter().map(|f| f.del).sum()
    }
}

/// `+42 −18`, `+41`, `−26`.
pub(crate) fn counts(add: usize, del: usize) -> String {
    match (add, del) {
        (0, 0) => String::new(),
        (a, 0) => format!("+{}", a),
        (0, d) => format!("−{}", d),
        (a, d) => format!("+{} −{}", a, d),
    }
}

/// `1 file`, `9 files`.
pub(crate) fn files_word(n: usize) -> String {
    if n == 1 {
        "1 file".to_string()
    } else {
        format!("{} files", n)
    }
}

// ---- the panel's state ----

/// The file list (`f`): its filter and the file selected.
#[derive(Clone, Debug, Default)]
pub(crate) struct List {
    pub(crate) filter: String,
    pub(crate) sel: usize,
}

pub(crate) struct Panel {
    pub(crate) ask: Ask,
    pub(crate) req: u64,
    pub(crate) diff: Option<Diff>,
    /// the panel has the keys
    pub(crate) focused: bool,
    /// the cursor's row in the body, the first body row shown
    pub(crate) cursor: usize,
    pub(crate) top: usize,
    /// big files shown whole, folded files opened (by path)
    pub(crate) unfolded: HashSet<String>,
    /// files you folded (by path)
    pub(crate) folded: HashSet<String>,
    pub(crate) list: Option<List>,
    /// the body rows of the last frame (keys and clicks)
    pub(crate) rows: Vec<Kind>,
    /// the screen rect of the body in the last frame, and of the panel
    pub(crate) body: Rect,
    pub(crate) area: Rect,
    /// the body rows the last frame showed
    pub(crate) page: usize,
    /// the agent's changes when it was last asked (asked again when
    /// the hub's count moves)
    pub(crate) changes_seen: Option<(u64, u64, u64)>,
    /// the last frame drew it on the right (else full screen)
    pub(crate) side: bool,
    /// its title until the hub answers (`t1 vs main`, `your folder vs
    /// main`)
    pub(crate) what: String,
    /// the agent's last land in the feed (its range): an empty diff of
    /// that agent offers it
    pub(crate) last_land: Option<Ask>,
    /// the lines selected (select + type to quote, diffquote.rs)
    pub(crate) sel: Option<crate::diffquote::Sel>,
}

static REQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn ask_json(ask: &Ask, req: u64) -> Value {
    match ask {
        Ask::Agent(a) => serde_json::json!({"op": "diff", "req": req, "agent": a}),
        Ask::Branch(br) => serde_json::json!({"op": "diff", "req": req, "branch": br}),
        Ask::Pr(n) => serde_json::json!({"op": "diff", "req": req, "pr": n}),
        Ask::Range(r, a) => serde_json::json!({"op": "diff", "req": req, "range": r, "agent": a}),
    }
}

/// How the panel was opened: by a key (ctrl+g, `/diff`, ⏎ in
/// /artifacts), it takes the keys; by a click (a `± 3 files`, an agent's
/// ψ, a PR's chip), the composer keeps them (designer m_7291).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum By {
    Key,
    Click,
}

/// Opens the panel on `ask` and asks the hub for it; `by` a key it
/// takes the keys.
pub(crate) fn request(app: &mut App, ask: Ask, by: By) {
    let req = REQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    app.sb.send(ask_json(&ask, req));
    // the shared folder isn't only main's (designer m_7354)
    let what = match &ask {
        Ask::Agent(a) if app.sb.in_shared_folder(a) => "your folder vs main".to_string(),
        Ask::Agent(a) => format!("{} vs main", a),
        Ask::Branch(b) => format!("{} vs main", b),
        Ask::Pr(n) => format!("PR #{}", n),
        Ask::Range(r, a) => range_title(r, a),
    };
    // whose work this is: an empty diff then says it is on main
    let owner = match &ask {
        Ask::Agent(a) => Some(a.clone()),
        Ask::Branch(b) => app.sb.agent_of_branch(b),
        _ => None,
    };
    let last_land = owner.as_deref().and_then(|a| last_land(&app.events, a));
    app.diff = Some(Panel {
        what,
        last_land,
        ask,
        req,
        diff: None,
        focused: by == By::Key,
        cursor: 0,
        top: 0,
        unfolded: HashSet::new(),
        folded: HashSet::new(),
        list: None,
        rows: Vec::new(),
        body: Rect::default(),
        area: Rect::default(),
        page: 10,
        changes_seen: None,
        side: true,
        sel: None,
    });
}

/// A land's title: `diff-focus landed on main · e0f3df5` (the head adds
/// `· 11 files`), the range's own words without an agent.
pub(crate) fn range_title(range: &str, agent: &str) -> String {
    let to: String = range.split("..").last().unwrap_or(range).trim_start_matches('.').chars().take(7).collect();
    if agent.is_empty() {
        range.to_string()
    } else {
        format!("{} landed on main · {}", agent, to)
    }
}

/// The newest land of `agent` in the feed: its range (old main..new).
pub(crate) fn last_land(events: &[crate::wire::Ev], agent: &str) -> Option<Ask> {
    events.iter().rev().find_map(|e| match e {
        crate::wire::Ev::Landed { agent: a, from, sha, .. } if a == agent && !from.is_empty() => {
            Some(Ask::Range(format!("{}..{}", from, sha), agent.to_string()))
        }
        _ => None,
    })
}

/// Asks the hub again (the agent's changes moved): the panel keeps
/// where it is.
pub(crate) fn refresh(app: &mut App) {
    let Some(p) = app.diff.as_mut() else { return };
    let req = REQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    p.req = req;
    let v = ask_json(&p.ask, req);
    app.sb.send(v);
}

/// The hub's `diff` event: the panel's, when it answers its last ask.
pub(crate) fn event(app: &mut App, v: &Value) {
    let Some(p) = app.diff.as_mut() else { return };
    if v.get("req").and_then(|x| x.as_u64()) != Some(p.req) {
        return;
    }
    p.diff = Some(Diff::of(v));
}

/// The agent's changes moved (the hub's state): an open panel on that
/// agent asks again, so it stays live while the agent works.
pub(crate) fn on_changes(app: &mut App, agent: &str, changes: Option<(u64, u64, u64)>) {
    let Some(p) = app.diff.as_mut() else { return };
    if p.ask != Ask::Agent(agent.to_string()) {
        return;
    }
    if p.changes_seen.is_some() && p.changes_seen != changes {
        p.changes_seen = changes;
        refresh(app);
    } else {
        p.changes_seen = changes;
    }
}

/// ctrl+g: the agent in view's diff, or the panel closes.
pub(crate) fn toggle(app: &mut App) {
    if app.diff.is_some() {
        app.diff = None;
    } else {
        let a = app.sb.focus_name().to_string();
        request(app, Ask::Agent(a), By::Key);
    }
}

/// The panel is on the right and has the keys: the composer looks
/// unfocused (no cursor, `the diff has the keys · type to write here`).
pub(crate) fn has_keys(app: &App) -> bool {
    app.diff.as_ref().is_some_and(|p| p.focused && p.side)
}

/// The composer's dim line while the panel has the keys.
pub(crate) const COMPOSER_NOTE: &str = "the diff has the keys · type to write here";

/// The panel on the right gives the keys back to the composer (a key
/// that isn't the panel's, a paste, a click out of it).
pub(crate) fn give_back(app: &mut App) {
    if let Some(p) = app.diff.as_mut().filter(|p| p.side) {
        p.focused = false;
    }
}

/// The panel is on the right (else full screen) at this width.
pub(crate) fn side(width: u16) -> bool {
    width >= SIDE_FROM
}

/// The panel's width on the right of a `width`-column screen.
pub(crate) fn side_w(width: u16) -> u16 {
    PANEL_W.min(width.saturating_sub(64))
}

// ---- the rows (pure) ----

/// What a body row is (keys and clicks).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Blank,
    /// `files … all 9 files ▸` (a click or ⏎: the list)
    FilesHead,
    /// a file of the list on top: its index
    ListFile(usize),
    /// `↓ 3 more`: the file list
    More,
    /// a file's head `▾ path  +4 −6`
    FileHead(usize),
    Hunk(usize),
    /// a line of a file: the file, its numbers and text (diffquote.rs)
    Code(usize, crate::diffquote::CodeLine),
    /// `▸ 212 more lines in this file · ⏎ shows them`
    Fold(usize),
    End,
    /// `show what it landed last` (an agent's diff empty, its work on main):
    /// ⏎ or a click opens that land's range
    LastLand,
}

fn tint(add: bool) -> Option<Color> {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return None;
    }
    let dark = theme::mode() == theme::Mode::Dark;
    Some(match (add, dark) {
        (true, true) => Color::Rgb(0x1f, 0x33, 0x26),
        (false, true) => Color::Rgb(0x3d, 0x22, 0x24),
        (true, false) => Color::Rgb(0xdf, 0xf2, 0xe1),
        (false, false) => Color::Rgb(0xfa, 0xe0, 0xe0),
    })
}

fn add_st() -> Style {
    let st = Style::default().fg(theme::ok());
    tint(true).map_or(st, |c| st.bg(c))
}
fn del_st() -> Style {
    let st = Style::default().fg(theme::error());
    tint(false).map_or(st, |c| st.bg(c))
}

fn cut(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) + 1 > w {
            break;
        }
        out.push(c);
    }
    out.push_str(theme::ellipsis());
    out
}

/// `path` cut from the left when too long: `…ents/Plan.tsx`.
fn cut_left(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    crate::ui::truncate_left(s, w)
}

fn counts_spans(add: usize, del: usize) -> Vec<Span<'static>> {
    let mut v = Vec::new();
    if add > 0 {
        v.push(Span::styled(format!("+{}", add), Style::default().fg(theme::ok())));
    }
    if del > 0 {
        if add > 0 {
            v.push(Span::raw(" "));
        }
        v.push(Span::styled(format!("−{}", del), Style::default().fg(theme::error())));
    }
    v
}

/// A file row of the list: `M src/pages/pricing.tsx      +30 −12`.
fn list_row(f: &File, width: usize, sel: bool) -> Line<'static> {
    let num_w = 12;
    let path_w = width.saturating_sub(num_w + 4);
    let path = cut_left(&f.path, path_w);
    let st = if sel { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) };
    let mut spans = vec![
        Span::styled(format!("{} ", f.status), Style::default().fg(dim())),
        Span::styled(format!("{:<w$}", path, w = path_w), st),
        Span::raw("  "),
    ];
    if f.binary || f.image {
        spans.push(Span::styled("binary", Style::default().fg(dim())));
    } else {
        spans.extend(counts_spans(f.add, f.del));
    }
    Line::from(spans)
}

/// The file is folded (lock and generated files, until ⏎; a file you
/// folded).
fn is_folded(p: &Panel, f: &File) -> bool {
    if p.folded.contains(&f.path) {
        return true;
    }
    (f.generated || is_lock(&f.path)) && !p.unfolded.contains(&f.path)
}

/// Lock files fold by themselves.
pub(crate) fn is_lock(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.ends_with(".lock")
        || matches!(name, "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "Cargo.lock" | "Gemfile.lock" | "poetry.lock" | "go.sum" | "composer.lock")
}

/// The body's rows, under the 2 head rows: the file list on top (more
/// than one file), then each file.
pub(crate) fn body_rows(p: &Panel, d: &Diff, width: usize) -> Vec<(Line<'static>, Kind)> {
    let mut out: Vec<(Line<'static>, Kind)> = Vec::new();
    let d_st = Style::default().fg(dim());
    if d.files.len() > 1 {
        // a click or ⏎ on it opens the list (designer m_7291)
        let right = format!("all {} files {}", d.files.len(), theme::glyph(theme::G_CLOSED));
        let gap = width.saturating_sub(5 + right.width() + 2).max(1);
        out.push((
            Line::from(vec![Span::styled("files", Style::default().fg(text())), Span::raw(" ".repeat(gap)), Span::styled(right, d_st)]),
            Kind::FilesHead,
        ));
        for (i, f) in d.files.iter().enumerate().take(TOP_FILES) {
            out.push((list_row(f, width, false), Kind::ListFile(i)));
        }
        if d.files.len() > TOP_FILES {
            out.push((Line::from(Span::styled(format!("  ↓ {} more", d.files.len() - TOP_FILES), d_st)), Kind::More));
        }
        out.push((Line::from(""), Kind::Blank));
    }
    let num = |n: Option<u32>| n.map_or("    ".to_string(), |n| format!("{:>4}", n));
    for (i, f) in d.files.iter().enumerate() {
        let folded = is_folded(p, f);
        let glyph = if folded || f.image || f.binary { theme::G_CLOSED } else { theme::G_OPEN };
        let mut name = cut_left(&f.path, width.saturating_sub(24));
        if let Some(old) = &f.old_path {
            name = cut_left(&format!("{} → {}", old, f.path), width.saturating_sub(24));
        }
        let note = if f.image {
            " · an image, ⏎ opens it"
        } else if f.binary {
            " · binary"
        } else if folded && (f.generated || is_lock(&f.path)) {
            " · generated, folded"
        } else if folded {
            " · folded"
        } else {
            ""
        };
        let head = format!("{} {}{}", theme::glyph(glyph), name, note);
        let pad = 50usize.min(width.saturating_sub(14)).max(head.width() + 2);
        let mut spans = vec![Span::styled(format!("{:<w$}", head, w = pad), Style::default().fg(text()).add_modifier(Modifier::BOLD))];
        if !(f.binary || f.image) {
            spans.extend(counts_spans(f.add, f.del));
        }
        out.push((Line::from(spans), Kind::FileHead(i)));
        if folded || f.image || f.binary {
            out.push((Line::from(""), Kind::Blank));
            continue;
        }
        let whole = p.unfolded.contains(&f.path) || f.changed() <= BIG;
        let mut changed = 0usize;
        let mut hidden = 0usize;
        for h in &f.hunks {
            let hunk_changed = h.lines.iter().filter(|l| l.starts_with('+') || l.starts_with('-')).count();
            if !whole && changed > 0 && changed + hunk_changed > BIG / 2 {
                hidden += hunk_changed;
                continue;
            }
            changed += hunk_changed;
            let head = if h.head.is_empty() { "@@".to_string() } else { format!("@@ {} @@", h.head) };
            out.push((Line::from(Span::styled(cut(&head, width), Style::default().fg(faint()))), Kind::Hunk(i)));
            let (mut old, mut new) = (h.old, h.new);
            for l in &h.lines {
                let (mark, body) = l.split_at(l.chars().next().map_or(0, |c| c.len_utf8()));
                let (o, nn, m, st) = match mark {
                    "+" => {
                        new += 1;
                        (None, Some(new - 1), "+", add_st())
                    }
                    "-" => {
                        old += 1;
                        (Some(old - 1), None, "−", del_st())
                    }
                    _ => {
                        old += 1;
                        new += 1;
                        (Some(old - 1), Some(new - 1), " ", Style::default().fg(text()))
                    }
                };
                let body = body.replace('\t', "    ");
                let lead = format!("{} {} {} ", num(o), num(nn), m);
                let room = width.saturating_sub(lead.width());
                let code = cut(&body, room);
                let fill = room.saturating_sub(code.width());
                let num_st = if mark == " " { Style::default().fg(faint()) } else { st };
                out.push((
                    Line::from(vec![
                        Span::styled(lead, num_st),
                        Span::styled(code, st),
                        Span::styled(" ".repeat(if mark == " " { 0 } else { fill }), st),
                    ]),
                    Kind::Code(i, crate::diffquote::CodeLine { old: o, new: nn, raw: l.clone() }),
                ));
            }
        }
        if hidden > 0 {
            out.push((
                Line::from(Span::styled(format!("{} {} more lines in this file · ⏎ shows them", theme::glyph(theme::G_CLOSED), hidden), d_st)),
                Kind::Fold(i),
            ));
        }
        if f.cut {
            out.push((Line::from(Span::styled("the rest of this file is too long to show here", d_st)), Kind::Blank));
        }
        out.push((Line::from(""), Kind::Blank));
    }
    out.push((Line::from(Span::styled(format!("end of the diff · {}", files_word(d.files.len())), d_st)), Kind::End));
    out
}

/// The file a body row belongs to (the head's `file 5 of 9`).
fn file_at(rows: &[Kind], k: usize) -> Option<usize> {
    rows.get(..=k.min(rows.len().saturating_sub(1)))?.iter().rev().find_map(|r| match r {
        Kind::FileHead(i) | Kind::Hunk(i) | Kind::Code(i, _) | Kind::Fold(i) => Some(*i),
        _ => None,
    })
}

/// The 2 head rows: `pricing-page vs main · 9 files +429 −367   ∿ still
/// working`, then the branch (`branch x · 6 commits + changes not
/// committed yet`), or once scrolled `file 5 of 9 · path · 62%`.
pub(crate) fn head_rows(p: &Panel, d: &Diff, width: usize, scrolled_file: Option<(usize, usize)>, now: u64) -> Vec<Line<'static>> {
    let title_st = if p.focused { Style::default().fg(accent()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()).add_modifier(Modifier::BOLD) };
    let mut first = vec![Span::styled(format!("{} · {} ", d.title, files_word(d.files.len())), title_st)];
    first.extend(counts_spans(d.add(), d.del()));
    if d.working {
        first.push(Span::styled(format!("   {} still working", theme::glyph(theme::G_WORKING)), Style::default().fg(dim())));
    }
    if p.side {
        close_hint(&mut first, width);
    }
    let second = match scrolled_file {
        Some((i, pct)) if d.files.len() > 1 || pct > 0 => {
            let path = d.files.get(i).map_or(String::new(), |f| f.path.clone());
            format!("file {} of {} · {} · {}%", i + 1, d.files.len(), cut_left(&path, width.saturating_sub(24)), pct)
        }
        _ => {
            let mut s = if d.branch.is_empty() { String::new() } else { format!("branch {}", d.branch) };
            let commits = match d.commits {
                0 => String::new(),
                1 => "1 commit".to_string(),
                c => format!("{} commits", c),
            };
            let tail = match (commits.is_empty(), d.uncommitted) {
                (false, true) => format!("{} + changes not committed yet", commits),
                (true, true) => "changes not committed yet".to_string(),
                (false, false) => commits,
                (true, false) => String::new(),
            };
            for part in [tail, d.landed_ms.map(|ms| format!("landed {}", crate::artifacts::ago_words(ms, now))).unwrap_or_default()] {
                if !part.is_empty() {
                    if !s.is_empty() {
                        s.push_str(" · ");
                    }
                    s.push_str(&part);
                }
            }
            s
        }
    };
    vec![Line::from(first), Line::from(Span::styled(cut(&second, width), Style::default().fg(dim())))]
}

/// The title row of a diff with nothing to count (a failure, a gone
/// folder): its title, and `ctrl+g close` on the right like every
/// title row (designer m_7487).
fn bare_title(p: &Panel, d: &Diff, width: usize) -> Line<'static> {
    let mut spans = vec![Span::styled(d.title.clone(), Style::default().fg(text()).add_modifier(Modifier::BOLD))];
    if p.side {
        close_hint(&mut spans, width);
    }
    Line::from(spans)
}

/// The title row's right end on the right of the screen: `ctrl+g close`,
/// dim, focused or not (designer m_7291), when it fits.
pub(crate) const CLOSE_HINT: &str = "ctrl+g close";

fn close_hint(spans: &mut Vec<Span<'static>>, width: usize) {
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let w = CLOSE_HINT.width();
    if used + 3 + w + 2 <= width {
        spans.push(Span::raw(" ".repeat(width - used - w - 2)));
        spans.push(Span::styled(CLOSE_HINT, Style::default().fg(dim())));
    }
}

/// The file list's lines (`f`): the filter, the files, the legend.
fn list_lines(d: &Diff, l: &List, width: usize, height: usize) -> (Vec<Line<'static>>, Vec<usize>) {
    let mut out = Vec::new();
    if l.filter.is_empty() {
        out.push(Line::from(Span::styled(format!("{} type to filter the files", theme::glyph(theme::G_YOU)), Style::default().fg(faint()))));
    } else {
        out.push(Line::from(vec![
            Span::styled(format!("{} ", theme::glyph(theme::G_YOU)), Style::default().fg(accent())),
            Span::styled(l.filter.clone(), Style::default().fg(text())),
            Span::styled("▏", Style::default().fg(accent())),
        ]));
    }
    out.push(Line::from(""));
    let shown = list_matches(d, &l.filter);
    let room = height.saturating_sub(out.len() + 2);
    let sel = l.sel.min(shown.len().saturating_sub(1));
    let top = sel.saturating_sub(room.saturating_sub(1));
    for (k, &i) in shown.iter().enumerate().skip(top).take(room) {
        let mut line = list_row(&d.files[i], width.saturating_sub(2), k == sel);
        let mark = if k == sel { Span::styled(format!("{} ", theme::glyph(theme::G_YOU)), Style::default().fg(accent())) } else { Span::raw("  ") };
        line.spans.insert(0, mark);
        out.push(line);
    }
    if shown.is_empty() {
        out.push(Line::from(Span::styled("  nothing matches", Style::default().fg(dim()))));
    }
    out.push(Line::from(""));
    out.push(Line::from(Span::styled("M changed   A added   D deleted   R renamed", Style::default().fg(dim()))));
    (out, shown)
}

/// The files whose path has every word of `filter`.
pub(crate) fn list_matches(d: &Diff, filter: &str) -> Vec<usize> {
    let words: Vec<String> = filter.split_whitespace().map(|w| w.to_lowercase()).collect();
    (0..d.files.len()).filter(|&i| words.iter().all(|w| d.files[i].path.to_lowercase().contains(w.as_str()))).collect()
}

/// The panel's lines in `width` × `height`; sets its rows, cursor and
/// window.
pub(crate) fn lines(p: &mut Panel, width: usize, height: usize, now: u64) -> Vec<Line<'static>> {
    let Some(d) = p.diff.clone() else {
        let what = p.what.clone();
        p.rows.clear();
        return vec![Line::from(Span::styled(what, Style::default().fg(text()).add_modifier(Modifier::BOLD))), Line::from(Span::styled("reading the diff…", Style::default().fg(dim())))];
    };
    if !d.error.is_empty() {
        p.rows.clear();
        return vec![
            bare_title(p, &d, width),
            Line::from(vec![Span::styled("▲ ", Style::default().fg(theme::error())), Span::styled(d.error.clone(), Style::default().fg(dim()))]),
        ];
    }
    // a state, not a failure (designer m_7393): one dim line; a gone
    // folder offers what the agent landed last
    if !d.note.is_empty() {
        p.rows.clear();
        let mut line = vec![Span::styled(d.note.clone(), Style::default().fg(dim()))];
        if let (true, Some(Ask::Range(..))) = (d.gone, &p.last_land) {
            p.rows.push(Kind::LastLand);
            p.cursor = 0;
            p.top = 0;
            let link = Style::default().fg(text()).add_modifier(Modifier::UNDERLINED);
            let link = if p.focused { link.bg(theme::selection_bg()) } else { link };
            line.push(Span::styled(" · ", Style::default().fg(dim())));
            line.push(Span::styled("show what it landed last", link));
        }
        return vec![bare_title(p, &d, width), Line::from(line)];
    }
    if d.files.is_empty() {
        p.rows.clear();
        let mut v = head_rows(p, &d, width, None, now);
        v.push(Line::from(""));
        match &p.last_land {
            // its work is on main (designer): say so, and offer its last land
            Some(Ask::Range(_, a)) => {
                p.rows.push(Kind::LastLand);
                p.cursor = 0;
                p.top = 0;
                let link = Style::default().fg(text()).add_modifier(Modifier::UNDERLINED);
                let link = if p.focused { link.bg(theme::selection_bg()) } else { link };
                v.push(Line::from(vec![
                    Span::styled(format!("{}'s work is all on main already · ", a), Style::default().fg(dim())),
                    Span::styled("show what it landed last", link),
                ]));
            }
            _ => v.push(Line::from(Span::styled("no changes against main", Style::default().fg(dim())))),
        }
        return v;
    }
    if let Some(l) = p.list.clone() {
        let mut v = head_rows(p, &d, width, None, now);
        v.truncate(1);
        let (rest, _) = list_lines(&d, &l, width, height.saturating_sub(1));
        v.extend(rest);
        return v;
    }
    let rows = body_rows(p, &d, width);
    let body_h = height.saturating_sub(3).max(1);
    p.page = body_h;
    let kinds: Vec<Kind> = rows.iter().map(|(_, k)| k.clone()).collect();
    // new rows (a fold, the agent's new changes): the selection was theirs
    if kinds != p.rows {
        p.sel = None;
    }
    p.rows = kinds;
    p.cursor = p.cursor.min(rows.len().saturating_sub(1));
    if p.cursor < p.top {
        p.top = p.cursor;
    }
    if p.cursor >= p.top + body_h {
        p.top = p.cursor + 1 - body_h;
    }
    p.top = p.top.min(rows.len().saturating_sub(body_h.min(rows.len())));
    let scrolled = (p.top > 0).then(|| {
        let i = file_at(&p.rows, p.top + body_h / 2).unwrap_or(0);
        let pct = ((p.top + body_h).min(rows.len()) * 100 / rows.len().max(1)).min(100);
        (i, pct)
    });
    let mut out = head_rows(p, &d, width, scrolled, now);
    out.push(Line::from(""));
    for (k, (line, _)) in rows.into_iter().enumerate().skip(p.top).take(body_h) {
        let line = if p.sel.is_some_and(|s| s.has(k)) {
            // the thread's selection tint, the full row (designer m_7568)
            let mut spans: Vec<Span<'static>> = line.spans.into_iter().map(|s| Span::styled(s.content, s.style.bg(theme::selection_bg()))).collect();
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            spans.push(Span::styled(" ".repeat(width.saturating_sub(used)), Style::default().bg(theme::selection_bg())));
            Line::from(spans)
        } else if p.focused && k == p.cursor && p.sel.is_none() {
            Line::from(line.spans.into_iter().map(|s| Span::styled(s.content, s.style.bg(theme::selection_bg()))).collect::<Vec<_>>())
        } else {
            line
        };
        out.push(line);
    }
    out
}

/// The keys the key bar shows while the panel has them.
pub(crate) fn key_pairs(app: &App) -> Vec<(&'static str, String)> {
    let Some(p) = app.diff.as_ref() else { return Vec::new() };
    let full = !p.side;
    if p.list.is_some() {
        return vec![("⏎", "go to the file".into()), ("esc", "back to the diff".into())];
    }
    // on the right: printable keys type in the composer (designer m_7291)
    let write = ("", "type to write".to_string());
    match p.rows.get(p.cursor) {
        Some(Kind::LastLand) if !full => vec![("⏎", "show what it landed last".to_string()), ("esc", "close".into()), write],
        Some(Kind::LastLand) => vec![("⏎", "show what it landed last".to_string()), ("esc", "close".into())],
        Some(Kind::Code(i, c)) if p.focused && c.line().is_some() => {
            let line = c.line().unwrap_or(0);
            let path = p.diff.as_ref().and_then(|d| d.files.get(*i)).map(|f| f.path.clone()).unwrap_or_default();
            let mut v = vec![("⏎", format!("open {}:{} in your editor", path, line)), ("↑↓", "move".into()), ("esc", "close".into())];
            if !full {
                v.push(write);
            }
            v
        }
        _ if full => vec![
            ("↑↓", "scroll".to_string()),
            ("tab", "next file".into()),
            ("f", "files".into()),
            ("⏎", "editor".into()),
            ("esc", "close".into()),
        ],
        _ => vec![("↑↓", "scroll".to_string()), ("tab", "next file".into()), ("⏎", "open in your editor".into()), ("esc", "close".into()), write],
    }
}

// ---- drawing ----

/// The panel in `area` (the agents panel's place).
pub(crate) fn draw_side(app: &mut App, frame: &mut Frame, area: Rect) {
    let now = crate::when::now_ms();
    let Some(p) = app.diff.as_mut() else { return };
    p.side = true;
    frame.render_widget(Clear, area);
    let inner = Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area };
    let rows = lines(p, inner.width as usize, inner.height as usize, now);
    p.area = area;
    p.body = Rect { y: inner.y + 3, height: inner.height.saturating_sub(3), ..inner };
    frame.render_widget(Paragraph::new(rows), inner);
    text_rects(p, inner);
    crate::diffquote::draw_hint(app, frame);
}

/// The panel's text for the text layer (links, word selection): the
/// whole of it, but the lines of a diff, which select whole lines
/// (diffquote.rs).
fn text_rects(p: &Panel, r: Rect) {
    if p.list.is_some() || p.rows.is_empty() || p.body.is_empty() {
        crate::textlayer::text(r);
        return;
    }
    crate::textlayer::text(Rect { height: p.body.y.saturating_sub(r.y), ..r });
    crate::textlayer::text(Rect { y: p.body.bottom(), height: r.bottom().saturating_sub(p.body.bottom()), ..r });
}

/// Under [`SIDE_FROM`] columns: the whole screen, its frame `bise :* ──
/// diff`, its key bar at the bottom.
pub(crate) fn draw_full(app: &mut App, frame: &mut Frame) {
    if app.diff.is_none() || side(frame.area().width) {
        return;
    }
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 8 {
        return;
    }
    let area = crate::artifacts_screen::draw_frame(app, frame, full, "diff");
    let body = Rect { y: area.y + 1, height: area.height.saturating_sub(3), ..area };
    let now = crate::when::now_ms();
    if let Some(p) = app.diff.as_mut() {
        p.focused = true;
        p.side = false;
        let rows = lines(p, body.width as usize, body.height as usize, now);
        p.area = full;
        p.body = Rect { y: body.y + 3, height: body.height.saturating_sub(3), ..body };
        frame.render_widget(Paragraph::new(rows), body);
    }
    let pairs = key_pairs(app);
    let mut spans = Vec::new();
    for (i, (k, w)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(k.to_string(), Style::default().fg(text())));
        spans.push(Span::styled(format!(" {}", w), Style::default().fg(dim())));
    }
    let kb = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
    frame.render_widget(Paragraph::new(Line::from(spans)), kb);
    if let Some(p) = app.diff.as_ref() {
        text_rects(p, area);
    }
    crate::diffquote::draw_hint(app, frame);
}

// ---- keys and the mouse ----

fn next_file(p: &mut Panel, fwd: bool) {
    let heads: Vec<usize> = p.rows.iter().enumerate().filter(|(_, k)| matches!(k, Kind::FileHead(_))).map(|(i, _)| i).collect();
    let to = if fwd { heads.iter().find(|&&h| h > p.cursor) } else { heads.iter().rev().find(|&&h| h < p.cursor) };
    if let Some(&h) = to {
        p.cursor = h;
        p.top = h;
    }
}

fn go_to_file(p: &mut Panel, i: usize) {
    if let Some(h) = p.rows.iter().position(|k| *k == Kind::FileHead(i)) {
        p.cursor = h;
        p.top = h;
    } else {
        // the rows are built at the next frame: aim at it then
        p.pending_file(i);
    }
}

impl Panel {
    fn pending_file(&mut self, i: usize) {
        // the body rows of a diff always hold every file's head; built
        // from the diff now
        if let Some(d) = self.diff.clone() {
            let rows = body_rows(self, &d, 80);
            self.rows = rows.into_iter().map(|(_, k)| k).collect();
            if let Some(h) = self.rows.iter().position(|k| *k == Kind::FileHead(i)) {
                self.cursor = h;
                self.top = h;
            }
        }
    }
}

/// ⏎: the line in your editor, or unfold, or the image opens; on
/// `show what it landed last`, that land's diff.
fn enter(app: &mut App) {
    if let Some((Some(Kind::LastLand), Some(ask), focused)) = app.diff.as_ref().map(|p| (p.rows.get(p.cursor).cloned(), p.last_land.clone(), p.focused)) {
        request(app, ask, if focused { By::Key } else { By::Click });
        return;
    }
    let Some(p) = app.diff.as_mut() else { return };
    let Some(d) = p.diff.clone() else { return };
    match p.rows.get(p.cursor).cloned() {
        Some(Kind::Fold(i)) => {
            if let Some(f) = d.files.get(i) {
                p.unfolded.insert(f.path.clone());
            }
        }
        Some(Kind::FileHead(i)) => {
            let Some(f) = d.files.get(i) else { return };
            if f.image || f.binary {
                let t = crate::file_links::Target { path: f.abs.clone().into(), line: None, col: None };
                let url = crate::file_links::url_of(&t);
                let ok = crate::links::open(&url);
                app.flash = Some((if ok { format!("opening {}", f.path) } else { format!("could not open {}", f.path) }, std::time::Instant::now()));
            } else if is_folded(p, f) {
                p.folded.remove(&f.path);
                p.unfolded.insert(f.path.clone());
            } else {
                p.folded.insert(f.path.clone());
                p.unfolded.remove(&f.path);
            }
        }
        Some(Kind::ListFile(i)) => go_to_file(p, i),
        Some(Kind::More) | Some(Kind::FilesHead) => p.list = Some(List::default()),
        Some(Kind::Code(i, c)) => {
            let line = c.line();
            let Some(f) = d.files.get(i) else { return };
            if f.abs.is_empty() {
                return;
            }
            let t = crate::file_links::Target { path: f.abs.clone().into(), line, col: None };
            let note = crate::file_links::open(app, &t);
            app.flash = Some((note, std::time::Instant::now()));
        }
        _ => {}
    }
}

/// Keys: ctrl+g everywhere (open, close); the rest while the panel has
/// the focus. True when taken.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    if k.kind != KeyEventKind::Press {
        return false;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && k.code == KeyCode::Char('g') {
        toggle(app);
        return true;
    }
    let Some(p) = app.diff.as_mut() else { return false };
    let full = !p.side;
    if !p.focused && !full {
        return false;
    }
    // the file list: its filter takes the letters (it shows them)
    if let Some(l) = p.list.as_mut() {
        let Some(d) = p.diff.clone() else { return true };
        let shown = list_matches(&d, &l.filter);
        match k.code {
            KeyCode::Esc => p.list = None,
            KeyCode::Up => l.sel = l.sel.saturating_sub(1),
            KeyCode::Down => l.sel = (l.sel + 1).min(shown.len().saturating_sub(1)),
            KeyCode::Enter => {
                let pick = shown.get(l.sel.min(shown.len().saturating_sub(1))).copied();
                p.list = None;
                if let Some(i) = pick {
                    go_to_file(p, i);
                }
            }
            KeyCode::Backspace => {
                l.filter.pop();
                l.sel = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                l.filter.push(c);
                l.sel = 0;
            }
            _ => {}
        }
        return true;
    }
    // lines selected: shift+↑↓, esc, a letter quotes (diffquote.rs)
    if crate::diffquote::on_key(app, k) {
        return true;
    }
    let Some(p) = app.diff.as_mut() else { return false };
    let n = p.rows.len();
    let page = p.page.max(3);
    match panel_key(k, full) {
        Some(PanelKey::Close) => app.diff = None,
        Some(PanelKey::Up) => p.cursor = p.cursor.saturating_sub(1),
        Some(PanelKey::Down) => p.cursor = (p.cursor + 1).min(n.saturating_sub(1)),
        Some(PanelKey::PageUp) => p.cursor = p.cursor.saturating_sub(page),
        Some(PanelKey::PageDown) => p.cursor = (p.cursor + page).min(n.saturating_sub(1)),
        Some(PanelKey::Top) => p.cursor = 0,
        Some(PanelKey::Bottom) => p.cursor = n.saturating_sub(1),
        Some(PanelKey::NextFile) => next_file(p, true),
        Some(PanelKey::PrevFile) => next_file(p, false),
        Some(PanelKey::Files) => p.list = Some(List::default()),
        Some(PanelKey::Enter) => enter(app),
        // ctrl+c interrupts, wherever the keys are
        None if ctrl && k.code == KeyCode::Char('c') => return false,
        // full screen: no composer to give it to
        None if full => {}
        // on the right: a key that isn't the panel's gives the keys back
        // and acts in the composer (a letter is never lost)
        None => {
            p.focused = false;
            return false;
        }
    }
    true
}

/// What a key does in the panel while it has the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelKey {
    Close,
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    NextFile,
    PrevFile,
    Files,
    Enter,
}

/// The focus rules (designer m_7291): on the right the panel has no
/// letter keys (every printable key types in the composer): ↑↓ pgup
/// pgdn home end, tab / shift+tab the next / previous file, ⏎, esc
/// closes. Full screen (no composer) also has j k ] [ and f. None: not
/// the panel's.
pub(crate) fn panel_key(k: &KeyEvent, full: bool) -> Option<PanelKey> {
    let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
    Some(match k.code {
        KeyCode::Esc => PanelKey::Close,
        KeyCode::Up if plain => PanelKey::Up,
        KeyCode::Down if plain => PanelKey::Down,
        KeyCode::PageUp => PanelKey::PageUp,
        KeyCode::PageDown => PanelKey::PageDown,
        KeyCode::Home => PanelKey::Top,
        KeyCode::End => PanelKey::Bottom,
        KeyCode::Tab if plain && !k.modifiers.contains(KeyModifiers::SHIFT) => PanelKey::NextFile,
        KeyCode::BackTab | KeyCode::Tab if plain => PanelKey::PrevFile,
        KeyCode::Enter if plain && !k.modifiers.contains(KeyModifiers::SHIFT) => PanelKey::Enter,
        KeyCode::Char('k') if full && plain => PanelKey::Up,
        KeyCode::Char('j') if full && plain => PanelKey::Down,
        KeyCode::Char(']') if full && plain => PanelKey::NextFile,
        KeyCode::Char('[') if full && plain => PanelKey::PrevFile,
        KeyCode::Char('f') if full && plain => PanelKey::Files,
        _ => return None,
    })
}

/// The mouse over the panel: the wheel scrolls it, a click takes the
/// keys and moves the cursor there (⏎ on what it clicked: a fold, a
/// file of the list).
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(p) = app.diff.as_mut() else { return false };
    // a drag that selects lines keeps its events, out of the panel too
    if crate::diffquote::dragging(p) && matches!(m.kind, MouseEventKind::Drag(_) | MouseEventKind::Up(_)) {
        if let Some(t) = crate::diffquote::mouse(p, m) {
            crate::input::copy_text(app, &t);
        }
        return true;
    }
    // the popup over selected lines: a press quotes or copies (diffquote.rs)
    if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) && crate::diffquote::hint_press(app, m.column, m.row) {
        return true;
    }
    let Some(p) = app.diff.as_mut() else { return true };
    let full = !p.side;
    let inside = full || (m.column >= p.area.x && m.column < p.area.right() && m.row >= p.area.y && m.row < p.area.bottom());
    if !inside {
        // a click out of it (the composer, the thread) gives the keys
        // back; the click goes on
        if matches!(m.kind, MouseEventKind::Down(_)) {
            p.focused = false;
        }
        return false;
    }
    match m.kind {
        MouseEventKind::ScrollUp => {
            p.top = p.top.saturating_sub(3);
            p.cursor = p.cursor.min(p.top + p.page.saturating_sub(1)).max(p.top);
        }
        MouseEventKind::ScrollDown => {
            let max = p.rows.len().saturating_sub(p.page);
            p.top = (p.top + 3).min(max);
            p.cursor = p.cursor.max(p.top);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            p.focused = true;
            // a gone folder's line (`t1's folder is gone · show what it
            // landed last`) sits right under the title, above the body:
            // its only row, a click under the title opens it
            if matches!(p.rows.as_slice(), [Kind::LastLand]) && p.diff.as_ref().is_some_and(|d| !d.note.is_empty()) && m.row > p.body.y.saturating_sub(3) && m.row < p.body.bottom() {
                p.cursor = 0;
                enter(app);
            } else if m.row >= p.body.y && m.row < p.body.bottom() && p.list.is_none() {
                let k = p.top + (m.row - p.body.y) as usize;
                p.sel = None;
                if k < p.rows.len() {
                    p.cursor = k;
                    if matches!(p.rows[k], Kind::Fold(_) | Kind::ListFile(_) | Kind::More | Kind::FilesHead | Kind::FileHead(_) | Kind::LastLand) {
                        enter(app);
                    } else {
                        // a line: a drag from it selects (diffquote.rs)
                        crate::diffquote::mouse(p, m);
                    }
                }
            }
        }
        _ => {}
    }
    true
}

// ---- the doors ----

/// A `bise-diff:` url (the `± 3 files` under a landed line):
/// `bise-diff:range/<from>..<to>?agent=<a>`, `bise-diff:agent/<a>`,
/// `bise-diff:branch/<b>`.
pub(crate) fn ask_of_url(url: &str) -> Option<Ask> {
    let rest = url.strip_prefix("bise-diff:")?;
    let (kind, arg) = rest.split_once('/')?;
    match kind {
        "agent" => Some(Ask::Agent(arg.to_string())),
        "branch" => Some(Ask::Branch(arg.to_string())),
        "pr" => arg.parse().ok().map(Ask::Pr),
        "range" => {
            let (range, agent) = arg.split_once("?agent=").unwrap_or((arg, ""));
            Some(Ask::Range(range.to_string(), agent.to_string()))
        }
        _ => None,
    }
}

pub(crate) fn url_of(ask: &Ask) -> String {
    match ask {
        Ask::Agent(a) => format!("bise-diff:agent/{}", a),
        Ask::Branch(b) => format!("bise-diff:branch/{}", b),
        Ask::Pr(n) => format!("bise-diff:pr/{}", n),
        Ask::Range(r, a) => format!("bise-diff:range/{}?agent={}", r, a),
    }
}

#[cfg(test)]
#[path = "diffview_tests.rs"]
mod tests;
