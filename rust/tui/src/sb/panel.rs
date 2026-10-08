//! The chrome of the switchboard mode: the header row, the agents panel
//! on the right, the status row, the key hints and the composer
//! placeholder (bise book §8, §17).

use super::*;
use unicode_width::UnicodeWidthStr;

/// The status glyph of an agent and its color (book §6): the gust
/// breathes in its cell while it works (BISE-107), `·` pulses while it
/// starts; only "needs you" and a failure get a hue, and done's check is
/// accent (BISE-100).
pub(super) fn glyph(status: &str, tick: u32, motion: crate::gust::Motion) -> (&'static str, Color) {
    match status {
        "working" => crate::gust::cell(motion),
        "starting" => starting_frame(tick),
        "waiting" => (G_WAITING, text()),
        // BISE-299: a blocked task is main's to handle, not yours: the
        // same `?`, dim; the accent comes from a card in your inbox
        "blocked" => (G_NEEDS_YOU, dim()),
        "done" => (crate::theme::done_glyph(), accent()),
        "failed" => (G_FAILED, error()),
        "idle" => (G_IDLE, dim()),
        "stopped" | "archived" => (G_STOPPED, dim()),
        _ => (G_STARTING, faint()),
    }
}

/// The workspace folder (the embedded terminal starts there).
/// The model of the agent in view (the no-vision check, BISE-150).
pub(crate) fn focus_model(app: &App) -> String {
    app.sb.focus_model(app)
}

pub(crate) fn workspace(app: &App) -> Option<String> {
    Some(app.sb.workspace.clone()).filter(|w| !w.is_empty())
}

/// BISE-264: the folders a relative path in `who`'s feed resolves
/// against (file links): its private worktree, its own folder, then
/// the workspace.
pub(crate) fn feed_dirs(app: &App, who: &str) -> Vec<std::path::PathBuf> {
    let sb = &app.sb;
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    if let Some(a) = sb.agent(who) {
        out.extend([&a.place, &a.path].into_iter().filter(|p| !p.is_empty()).map(Into::into));
    }
    if !sb.workspace.is_empty() {
        out.push(sb.workspace.clone().into());
    }
    out
}

/// The feed and composer area, and the panel on the right when it fits.
pub(crate) fn split(full: Rect) -> (Rect, Option<Rect>) {
    // the screen's layout (book §8, layout.rs): under the header and its
    // blank row, above the composer pane (the divider and what the
    // smallest composer takes)
    let c = crate::layout::cols(full.width, full.height);
    let r = crate::layout::rows(full.width, full.height);
    let pane = 2 + r.pad_top + r.min_text + r.pad_bottom;
    let bottom = r.keybar.saturating_sub(pane);
    let body = Rect { y: full.y + r.body, height: bottom.saturating_sub(r.body), ..full };
    let feed = Rect { x: full.x + c.feed_x, width: c.feed_w, ..body }.intersection(full);
    let panel = c.panel.map(|p| Rect { x: full.x + p.x, width: p.w, ..body }.intersection(full));
    (feed, panel)
}

/// The panel title: `agents` alone (BISE-303: holding ⌥ writes the
/// numbers' key, `⌥1`; ctrlhint.rs writes its keys after the title).
pub(crate) const PANEL_TITLE: &str = "agents";

/// The agent waits on you: it is blocked, or one of its cards asks you
/// something.
/// BISE-299: only a card of your inbox about `a` (main escalated its
/// question, a confirmation) needs you; a blocked task is main's.
pub(super) fn needs_you(sb: &Sb, a: &Agent) -> bool {
    !matches!(a.status.as_str(), "failed" | "stopped" | "archived")
        && (a.waiting_on == "you"
            || sb
                .cards
                .iter()
                // answered here: its `?` turns back into its gust at once
                .any(|c| c.agent == a.name && !sb.answered_here(c.id) && matches!(c.kind.as_str(), "question" | "confirm" | "approval")))
}

/// A duration in the panel: `40s`, `12m`, `3h`, `2d`.
pub(crate) fn short_age(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{}s", s),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

/// An agent's state in one word, and its color, for the panel's columns
/// while ctrl is held (BISE-303): `asks you` in accent when it needs
/// you (designer: `needs you` stays the header's word).
fn state_word(sb: &Sb, a: &Agent) -> (String, Color) {
    if needs_you(sb, a) {
        return ("asks you".into(), accent());
    }
    let w = if a.status.is_empty() { "idle" } else { a.status.as_str() };
    (fit(w, STATE_W), dim())
}

/// The columns of an agent's row (BISE-303, designer's "less on
/// screen"): the turn's time (only while it works) and its context %,
/// each right-aligned in 3 columns, then `ψ` when it has a worktree, 2
/// columns between them; blank cells stay, so the rows line up. Ctrl
/// held: its state word takes the time and % columns (8 wide), the mark
/// stays. The mark (option A, sidebar-wt): an agent alone in its
/// worktree carries its git state ([`places::Place::row_mark`]: `↑`, `ψ`
/// or `…`); one in a private worktree the hub has no place for, `ψ`;
/// your folder's agents a blank cell. In a shared worktree's box
/// (`boxed`) no mark column: the border says it. Short on room (`drop`)
/// the time goes first (1), then the % (2).
fn columns(app: &App, sb: &Sb, a: &Agent, boxed: bool, drop: usize) -> Vec<Span<'static>> {
    let d = Style::default().fg(dim());
    // computer use (design §8): `↖` takes the mark's column while it
    // drives, in accent; ctrl held, the state words say what it drives
    let drives = crate::computer_use::driving(&a.name).and_then(|x| x.driving);
    let mark = match sb.solo_of(a) {
        _ if drives.is_some() => Span::styled(crate::computer_use::mark(), Style::default().fg(accent())),
        Some(p) => p.row_mark(sb.asks_merge(p)),
        None if place_label(a).is_some() => Span::styled(crate::theme::glyph(G_WORKTREE).to_string(), d),
        None => Span::raw(" "),
    };
    let mut out = if crate::ctrlhint::words(app) {
        let (w, c) = match &drives {
            Some(app) => (crate::computer_use::short_app(app), dim()),
            None => state_word(sb, a),
        };
        vec![Span::styled(format!(" {:>STATE_W$}", w), Style::default().fg(c))]
    } else {
        let time = a.turn_ms.filter(|_| a.status == "working").map(short_age).unwrap_or_default();
        let fill = sb.usage_of(app, &a.name).map(|u| u.short()).unwrap_or_default();
        // site/m/timers: a scheduled task's next run takes the time's
        // place, faint: `◷ 1m`, `◷ 07:30`
        let next = next_run(sb, &a.name, crate::when::now_ms());
        match (drop, next) {
            (0, Some(n)) => vec![
                Span::styled(format!(" {:>7}", n), Style::default().fg(faint())),
                Span::styled(format!("  {:>3}", fill), d),
            ],
            (0, None) => vec![Span::styled(format!(" {:>3}  {:>3}", time, fill), d)],
            (1, _) => vec![Span::styled(format!(" {:>3}", fill), d)],
            _ => Vec::new(),
        }
    };
    if !boxed {
        out.push(Span::styled("  ", d));
        out.push(mark);
    }
    out
}

/// The state word's columns: the time's 3, 2 between, the %'s 3.
const STATE_W: usize = 8;

/// site/m/timers: `agent`'s next scheduled run, `◷ 1m` within the hour,
/// else `◷ 07:30`; None: nothing scheduled for it.
pub(super) fn next_run(sb: &Sb, agent: &str, now: u64) -> Option<String> {
    let t = sb.timers.iter().filter(|t| t.active() && t.agent == agent).min_by_key(|t| t.next_ms)?;
    let left = t.next_ms.saturating_sub(now);
    let when = if left < 3_600_000 {
        format!("{}m", left.div_ceil(60_000).max(1))
    } else {
        crate::scheduled::ahead(t.next_ms, now).rsplit(' ').next().unwrap_or_default().to_string()
    };
    Some(format!("{} {}", crate::theme::glyph(G_SCHEDULED), when))
}

/// One row of the panel, `w` columns: ` N G name marks …… right `. The
/// name is cut to leave room for the marks and the right side; `bg`
/// paints the whole row (the selection).
#[allow(clippy::too_many_arguments)]
fn row(
    num: Option<usize>,
    g: (&str, Color),
    name: &str,
    name_style: Style,
    marks: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    w: usize,
    bg: Option<Color>,
) -> Line<'static> {
    let num = match num {
        Some(n) if n <= 9 => n.to_string(),
        _ => " ".to_string(),
    };
    let lead = format!(" {} ", num);
    let gl = format!("{} ", g.0);
    let marks_w: usize = marks.iter().map(|s| s.content.width()).sum();
    let mut right = right;
    let right_w: usize = right.iter().map(|s| s.content.width()).sum();
    // the right side's blank cells first (a blank time column) lend
    // themselves to the name and marks, 1 space kept: the columns stay
    let blank = right.first().map_or(0, |s| s.content.len() - s.content.trim_start().len());
    let lend = blank.saturating_sub(1);
    // 1 column of margin on the right
    let room = w.saturating_sub(lead.width() + gl.width() + marks_w + right_w - lend + 1);
    let name = fit(name, room);
    let used = lead.width() + gl.width() + name.width() + marks_w;
    let over = (used + right_w + 1).saturating_sub(w).min(lend);
    if over > 0 {
        if let Some(first) = right.first_mut() {
            first.content = first.content[over..].to_string().into();
        }
    }
    let right_w = right_w - over;
    let pad = w.saturating_sub(used + right_w + 1);
    let mut spans = vec![
        Span::styled(lead, Style::default().fg(faint())),
        // the glyph cell alone takes its color: a new gust frame rewrites
        // that one cell, not the space after it
        Span::styled(g.0.to_string(), Style::default().fg(g.1)),
        Span::raw(" "),
        Span::styled(name, name_style),
    ];
    spans.extend(marks);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right);
    spans.push(Span::raw(" "));
    if let Some(bg) = bg {
        spans = spans.into_iter().map(|s| { let st = s.style.bg(bg); s.style(st) }).collect();
    }
    Line::from(spans)
}

/// `s` cut to `max` display columns, `…` at the cut.
pub(super) fn fit(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + c.to_string().width() + 1 > max {
            break;
        }
        out.push(c);
    }
    if max > 0 {
        out.push('…');
    }
    out
}

/// BISE-136: where `a` works when it is not the shared checkout, for
/// `ψ {label}`: its hub worktree's branch (`sb spawn --worktree`,
/// `/isolate`), else the name of the private worktree it told the hub
/// about ([`super::places::folder_of`]: `gate.sh new`'s task folder, or
/// `/tmp/<task>-wt`'s last part). None: the shared checkout,
/// which shows nothing (quiet is normal).
pub(crate) fn place_label(a: &Agent) -> Option<String> {
    let base = |p: &str| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p).to_string();
    if let Some(b) = &a.branch {
        return Some(b.clone());
    }
    if a.mode == "worktree" {
        return Some(base(&a.path));
    }
    (!a.place.is_empty()).then(|| super::places::folder_of(&a.place))
}

/// The row of live agent `a`, entry `i` of the panel, number `num`
/// (0 main; blank after 9): ` N G name marks …… TTT  PPP  ψ ` (BISE-303:
/// no model tag, no state word at rest, the glyph says it).
fn agent_row(app: &App, sb: &Sb, a: &Agent, i: usize, num: Option<usize>, w: usize, boxed: bool) -> Line<'static> {
    let focused = a.name == sb.focus;
    let selected = sb.selected == Some(i);
    // BISE-119: main's status sits in the same column as every agent's
    // (the breathing gust while it works, `○` idle); its `:*` follows its name
    let g = if !a.main && needs_you(sb, a) {
        (G_NEEDS_YOU, accent())
    } else {
        glyph(&a.status, app.tick, app.motion_away)
    };
    let name_style = if focused {
        Style::default().fg(accent()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(text())
    };
    let mut marks = Vec::new();
    if a.main {
        marks.push(Span::styled(format!(" {}", G_MAIN), Style::default().fg(accent())));
    }
    if sb.activity.contains(&a.name) && !focused {
        marks.push(Span::styled(format!(" {}", G_UNREAD), Style::default().fg(accent())));
    }
    // BISE-299: main's inbox, dim, only when something waits there
    if a.main && a.inbox > 0 {
        marks.push(Span::styled(format!(" {} {}", crate::theme::glyph(G_MSG), a.inbox), Style::default().fg(dim())));
    }
    if a.queued > 0 {
        marks.push(Span::styled(format!(" {}{}", G_MSG, a.queued), Style::default().fg(dim())));
    }
    // BISE-89: the messages queued here for after its turn
    let mine = if focused { app.queued.len() } else { sb.views.get(&a.name).map_or(0, |v| v.queued.len()) };
    if mine > 0 {
        marks.push(Span::styled(format!(" · {} queued", mine), Style::default().fg(faint())));
    }
    let bg = selected.then(selection_bg);
    // the name takes all the room left of the marks and the columns; it
    // is cut only there (BISE-109: no fixed cap)
    let whole = |l: &Line| l.spans.get(3).is_some_and(|s| s.content == a.name);
    // short on room (the 24-column panel) the time goes, then the %,
    // before the name is cut; the mark column stays
    let fitted = |w: usize| -> Line<'static> {
        let mut l = Line::default();
        for drop in 0..3 {
            l = row(num, g, &a.name, name_style, marks.clone(), columns(app, sb, a, boxed, drop), w, bg);
            if whole(&l) {
                break;
            }
        }
        l
    };
    let mut l = if boxed {
        // in a box the columns stay where the other rows have them (the
        // mark's column blank); a name that would be cut there takes
        // those 3 cells back (pr-design §4.1)
        let mut narrow = row(num, g, &a.name, name_style, marks.clone(), columns(app, sb, a, boxed, 0), w.saturating_sub(3), bg);
        if whole(&narrow) {
            let st = bg.map_or(Style::default(), |c| Style::default().bg(c));
            narrow.spans.push(Span::styled("   ", st));
            narrow
        } else {
            fitted(w)
        }
    } else {
        fitted(w)
    };
    // option held (ctrlhint.rs): ` 1 ` reads `⌥1 `, in the accent
    if let (Some(k), Some(first)) = (num.and_then(|n| crate::ctrlhint::number(app, n)), l.spans.first_mut()) {
        *first = Span::styled(format!("{k} "), first.style.fg(accent()));
    }
    l
}

/// The live agents (main and the archived left out) by what the header
/// counts: working, waiting, needs you, done; the open PRs (pr-design
/// §4: `↑ 2 PRs`, ctrl held); then the open cards (BISE-125). None: no
/// agent and no card.
fn counts(sb: &Sb) -> Option<[usize; 6]> {
    let live: Vec<&Agent> = sb.agents.iter().filter(|a| !a.main && !a.archived()).collect();
    if live.is_empty() && sb.cards.is_empty() {
        return None;
    }
    let mut n = [0, 0, 0, 0, super::places::open_prs(&sb.places), sb.cards.len()];
    for a in live {
        let k = if needs_you(sb, a) {
            2
        } else {
            match a.status.as_str() {
                "working" => 0,
                "waiting" => 1,
                "done" => 3,
                _ => continue,
            }
        };
        n[k] += 1;
    }
    Some(n)
}

/// The header counts that fit in `room` columns (QA 14): all of them with
/// their words when they fit (not `short`), else the numbers only; still
/// too wide, the least important counts go first ("needs you" stays, then
/// cards, working, waiting, done, PRs), shown in the §8 order. `gust` leads the
/// working count (BISE-107). The open PRs (`↑ 2 PRs`) before the open
/// cards (`# 3 in the inbox`, dim), which come last, so what waits is
/// counted even when the panel is hidden (BISE-125).
/// The panel numbers the items as the strip does (BISE-302).
fn fit_counts(n: [usize; 6], short: bool, room: usize, gust: &[Span<'static>]) -> Vec<Span<'static>> {
    // (glyph, word, glyph color, text color): done's check is accent on
    // dim words (BISE-100)
    let prs = if n[4] == 1 { "PR" } else { "PRs" };
    let parts = [
        (G_WORKING, "working", dim(), dim()),
        (G_WAITING, "waiting", dim(), dim()),
        (G_NEEDS_YOU, "needs you", accent(), accent()),
        (crate::theme::done_glyph(), "done", accent(), dim()),
        (crate::theme::pr_glyph(), prs, dim(), dim()),
        ("#", "in the inbox", dim(), dim()),
    ];
    let spans = |keep: &[usize], words: bool| -> Vec<Span<'static>> {
        let mut out: Vec<Span<'static>> = Vec::new();
        for k in (0..6).filter(|k| keep.contains(k)) {
            if !out.is_empty() {
                out.push(Span::styled(" · ", Style::default().fg(dim())));
            }
            let (g, word, g_color, color) = parts[k];
            let t = if words { format!(" {} {}", n[k], word) } else { format!(" {}", n[k]) };
            if k == 0 {
                out.extend(gust.iter().cloned());
            } else {
                out.push(Span::styled(g, Style::default().fg(g_color)));
            }
            out.push(Span::styled(t, Style::default().fg(color)));
        }
        out
    };
    let by_importance: Vec<usize> = [2, 5, 0, 1, 3, 4].into_iter().filter(|&k| n[k] > 0).collect();
    let fits = |out: &Vec<Span<'static>>| out.iter().map(|s| s.content.width()).sum::<usize>() <= room;
    if !short {
        let out = spans(&by_importance, true);
        if fits(&out) {
            return out;
        }
    }
    for keep in (1..=by_importance.len()).rev() {
        let out = spans(&by_importance[..keep], false);
        if fits(&out) {
            return out;
        }
    }
    Vec::new()
}

impl Sb {
    /// The header row (book §8): `bise :*` on the left; on the right the
    /// non-zero counts `∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1
    /// done` (`? … needs you` in accent), shortened to `∿ 3 · ? 1` when
    /// `short` (no panel), or `no agents yet`. A method, so `ui.rs` reaches
    /// it through `app.sb` (the `panel` module is private to `sb`). `gust`
    /// leads the working count (BISE-107).
    pub(crate) fn header(&self, width: u16, short: bool, words: bool, gust: &[Span<'static>]) -> Line<'static> {
        let mut spans = vec![Span::raw(" ")];
        spans.extend(self.title());
        let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
        // one column of margin on the right, two of gap after `bise :*`
        let room = (width as usize).saturating_sub(left_w + 3);
        let (after, right) = self.edge(room, words, |r| self.summary(r, short, words, gust));
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        spans.extend(after);
        let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
        let pad = (width as usize).saturating_sub(left_w + right_w + 1);
        if pad >= 2 {
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(right);
        }
        Line::from(spans)
    }

    /// The role line of the task you view (BISE-126), dim, after the
    /// title: ` · fixing the safari login`; nothing in main's view or
    /// before the hub sends one.
    pub(crate) fn role_spans(&self) -> Vec<Span<'static>> {
        let Some(a) = self.agents.iter().find(|a| a.name == self.focus && !a.main) else {
            return Vec::new();
        };
        let role = a.role.split_whitespace().collect::<Vec<_>>().join(" ");
        if role.is_empty() {
            return Vec::new();
        }
        let st = Style::default().fg(dim());
        vec![Span::styled(format!(" {} ", crate::theme::glyph("·")), st), Span::styled(role, st)]
    }

    /// The title: `bise` bold in the text color, `:*` in accent.
    pub(crate) fn title(&self) -> Vec<Span<'static>> {
        vec![
            Span::styled("bise ", Style::default().fg(text()).add_modifier(Modifier::BOLD)),
            Span::styled(crate::theme::glyph(G_MAIN), Style::default().fg(accent())),
        ]
    }

    /// The top edge in `room` columns (book §8 "The frame", topedge.rs):
    /// after the logo the workspace (`· ~/acme`, dim; ctrl held, what
    /// happens to the work after it, dev-flow §7) and the role line; on
    /// the right the counts (`counts`: [`Sb::summary`], maybe with the
    /// voice before them) then what came new in /artifacts since you
    /// last looked (`↗ designer · pricing page`, a click opens the screen).
    pub(crate) fn edge(
        &self,
        room: usize,
        words: bool,
        counts: impl Fn(usize) -> Vec<Span<'static>>,
    ) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
        let flow = if words { super::places::flow_words(&self.flow) } else { "" };
        let paths = crate::topedge::paths(&self.workspace, flow);
        let link = |t: String, st: Style| crate::textlayer::link(t, crate::artifacts_screen::OPEN_URL, st);
        let notice = crate::topedge::notice(crate::artifacts::new_count(), &crate::artifacts::new_rows(), &link);
        crate::topedge::lay(room, &paths, self.role_spans(), counts, &notice)
    }

    /// The counts in at most `room` columns (book §8 "The frame"): not
    /// enough room, they shorten (`∿ 3 · ? 1`). BISE-303: with the panel
    /// shown (not `short`) the agents' counts are its job, the header
    /// keeps the inbox's (`# 1 in the inbox`); all of them with ctrl held
    /// (`words`). A release running: one more item after them.
    pub(crate) fn summary(&self, room: usize, short: bool, words: bool, gust: &[Span<'static>]) -> Vec<Span<'static>> {
        self.summary_of(room, short, words, gust)
    }

    fn summary_of(&self, room: usize, short: bool, words: bool, gust: &[Span<'static>]) -> Vec<Span<'static>> {
        // a release running (BISE-235): one more item after the counts
        let item = super::release::header_item(self, gust);
        let item_w: usize = item.iter().map(|s| s.content.width()).sum();
        if item.is_empty() || item_w + 3 > room {
            return self.counts_summary(room, short, words, gust);
        }
        let mut out = self.counts_summary(room - item_w - 3, short, words, gust);
        if !out.is_empty() {
            out.push(Span::styled(" · ", Style::default().fg(dim())));
        }
        out.extend(item);
        out
    }

    fn counts_summary(&self, room: usize, short: bool, words: bool, gust: &[Span<'static>]) -> Vec<Span<'static>> {
        let fitted = |room: usize| -> Vec<Span<'static>> {
            match counts(self) {
                None => vec![Span::styled("no agents yet", Style::default().fg(dim()))],
                Some(n) if !short && !words => fit_counts([0, 0, 0, 0, 0, n[5]], short, room, gust),
                // the PRs only with ctrl held (pr-design §4: nothing new at rest)
                Some(n) if !words => fit_counts([n[0], n[1], n[2], n[3], 0, n[5]], short, room, gust),
                Some(n) => fit_counts(n, short, room, gust),
            }
        };
        fitted(room)
    }
}

pub(crate) fn draw_panel(app: &App, frame: &mut Frame, area: Rect) {
    let sb = &app.sb;
    // no rule on its left: whitespace and alignment do the job (book §8)
    let w = area.width as usize;
    let title = Line::from(Span::styled(format!(" {}", PANEL_TITLE), Style::default().fg(text())));
    let mut lines: Vec<Line> = Vec::new();
    // the shared worktrees' sections, rows [start, end): never split by
    // the scroll (pr-design §4.1)
    let mut boxes: Vec<(usize, usize)> = Vec::new();
    let numbers = sb.numbers();
    let held = crate::ctrlhint::words(app);
    let mut owners: Vec<(usize, Hit)> = Vec::new();
    // the first row of the selected entry (the panel scrolls to it)
    let mut sel_row = None;
    let mut i = 0;
    for (place, agents) in sb.blocks() {
        let start = lines.len();
        if let Some(p) = place {
            // a shared worktree's section (option B, sidebar-wt): 1 blank
            // row, its title, held its lid; git's, never a row (no hit)
            lines.push(Line::from(""));
            // site/m/artifacts D: a click on its label opens its diff
            if let Some(b) = &p.branch {
                owners.push((lines.len(), Hit::Place(b.clone())));
            }
            lines.push(p.title(w, held));
            if let Some(lid) = p.lid_line(w).filter(|_| held) {
                lines.push(lid);
            }
        }
        for a in agents {
            if sb.selected == Some(i) {
                sel_row = Some(lines.len());
            }
            owners.push((lines.len(), Hit::Agent(a.name.clone())));
            let n = numbers.iter().find(|(name, _)| *name == a.name).map(|(_, n)| *n);
            lines.push(agent_row(app, sb, a, i, n, w, place.is_some()));
            // ctrl held, alone in its worktree: its git state in words
            // under its row (sidebar-wt)
            if let Some(words) = sb.solo_of(a).filter(|_| held && place.is_none()).and_then(|p| p.words_line(Some(&a.name), w)) {
                owners.push((lines.len(), Hit::Agent(a.name.clone())));
                lines.push(words);
            }
            // the selected agent: what it is for and its last note, under its row
            if sb.selected == Some(i) && !a.main {
                for t in [&a.objective, &a.note].into_iter().filter(|t| !t.is_empty()) {
                    owners.push((lines.len(), Hit::Agent(a.name.clone())));
                    lines.push(Line::from(Span::styled(
                        format!("     {}", fit(t, w.saturating_sub(6))),
                        Style::default().fg(dim()),
                    )));
                }
            }
            i += 1;
        }
        if place.is_none() {
            // the worktrees with an open PR and no live agent: a row with
            // no number and no glyph each, after the solo rows
            for p in sb.orphans() {
                lines.push(p.orphan_row(sb.asks_merge(p), w));
                if let Some(words) = p.words_line(None, w).filter(|_| held) {
                    lines.push(words);
                }
            }
        } else {
            boxes.push((start + 1, lines.len()));
        }
    }
    let live = i;
    cards_lines(app, w, &mut lines, &mut owners, &mut sel_row);
    archived_lines(sb, live, w, &mut lines, &mut owners, &mut sel_row);
    // the body under the title: scrolled to keep the selection in view,
    // or where the wheel or a `↑`/`↓` row put it (sidebar-more); what
    // does not fit ends in `↓ {n} more`, what is hidden over it starts
    // with `↑ {n} more`
    let h = (area.height as usize).saturating_sub(2);
    let wanted = sb.panel_hits.try_borrow().ok().and_then(|p| p.scroll).filter(|(_, s)| *s == sel_row).map(|(t, _)| t);
    let archived = sb.archived().len();
    let weigh = |hit: &Hit| match hit {
        Hit::Agent(name) => {
            let a = sb.agent(name).filter(|a| !a.archived());
            Hidden {
                n: 1,
                ask: a.is_some_and(|a| needs_you(sb, a)) as usize,
                failed: a.is_some_and(|a| a.status == "failed") as usize,
            }
        }
        Hit::Card(_) => Hidden { n: 1, ..Hidden::default() },
        Hit::Archived if !sb.archived_open => Hidden { n: archived, ..Hidden::default() },
        _ => Hidden::default(),
    };
    let win = window(lines.len(), &owners, &boxes, sel_row, h, wanted, weigh);
    let (top, shown) = (win.top, win.shown);
    let up = win.up.is_some() as usize;
    let mut body: Vec<Line> = Vec::new();
    if let Some(n) = win.up {
        body.push(more_line(n, true, w));
    }
    body.extend(lines.into_iter().skip(top).take(shown));
    if let Some(n) = win.down {
        body.push(more_line(n, false, w));
    }
    if let Ok(mut hits) = sb.panel_hits.try_borrow_mut() {
        hits.area = area;
        hits.rows = owners
            .into_iter()
            .filter(|(r, _)| *r >= top && *r - top < shown)
            .map(|(r, hit)| (area.y.saturating_add((2 + up + r - top) as u16), hit))
            .collect();
        let body_y = area.y.saturating_add(2);
        if win.up.is_some() {
            hits.rows.push((body_y, Hit::Up));
        }
        if win.down.is_some() {
            hits.rows.push((body_y.saturating_add((up + shown) as u16), Hit::Down));
        }
        // the scroll the wheel or a click asked for, as drawn (clamped)
        hits.scroll = hits.scroll.filter(|(_, s)| *s == sel_row).map(|_| (top, sel_row));
        hits.view = View { top, shown, sel_row, overflow: win.up.is_some() || win.down.is_some() };
        // BISE-272: the hand over the rows a click opens
        for (y, _) in &hits.rows {
            crate::pointer::region(Rect { y: *y, height: 1, ..area }.intersection(area), crate::pointer::Shape::Pointer);
        }
    }
    // the title, then 1 blank row (book §8)
    let mut all = vec![title, Line::from("")];
    all.extend(body);
    frame.render_widget(Paragraph::new(all), area);
    // BISE-290: its text selects, copies and has links
    crate::textlayer::text(area);
}

/// What a `↑`/`↓ n more` row hides (sidebar-more): `n` items (agents,
/// cards, the agents of a folded archived section), how many of those
/// agents need you and how many failed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Hidden {
    n: usize,
    ask: usize,
    failed: usize,
}

/// The panel's body in a window: its first row, how many rows show, and
/// what the `↑ n more` row over them and the `↓ n more` row under them
/// hide (None: no such row).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Win {
    top: usize,
    shown: usize,
    up: Option<Hidden>,
    down: Option<Hidden>,
}

/// The window of `len` body rows in `h` rows. Its top: `wanted` (the
/// wheel, a click on a `↑`/`↓` row), else where the selection (and the
/// row under it) stays in view over the `↓` row. Rows hidden over it
/// start with `↑ n more`, rows left under it end in `↓ n more`; `weigh`
/// says what a hidden row's target counts for. A box (`boxes`, rows
/// [start, end)) never splits (pr-design §4.1 rule 6): cut at the top it
/// goes up whole (down whole when it holds the selection; not when you
/// scrolled), cut at the bottom it goes under `↓ n more` whole; a box
/// taller than the window splits.
fn window(
    len: usize,
    owners: &[(usize, Hit)],
    boxes: &[(usize, usize)],
    sel_row: Option<usize>,
    h: usize,
    wanted: Option<usize>,
    weigh: impl Fn(&Hit) -> Hidden,
) -> Win {
    if len <= h || h < 2 {
        return Win { top: 0, shown: len, up: None, down: None };
    }
    let hidden = |rows: &mut dyn Iterator<Item = &(usize, Hit)>| {
        let mut seen: Vec<&Hit> = Vec::new();
        let mut out = Hidden::default();
        for (_, hit) in rows {
            if !seen.contains(&hit) {
                seen.push(hit);
                let w = weigh(hit);
                out = Hidden { n: out.n + w.n, ask: out.ask + w.ask, failed: out.failed + w.failed };
            }
        }
        out
    };
    // `rows`: how many list rows a window from `top` has above the `↓` row
    let top_for = |rows: usize| wanted.unwrap_or_else(|| sel_row.map_or(0, |r| (r + 2).saturating_sub(rows)));
    let cut = |at: usize| boxes.iter().copied().find(|(s, e)| *s < at && at < *e);
    // `up_ok`: a `↑` row may take the first row
    let place = |up_ok: bool| {
        let mut top = top_for(h - 1).min(len - h);
        if top > 0 && up_ok {
            // a `↑` row takes the first row: one less for the list
            top = top_for(h - 2).min(len - (h - 1));
        }
        if let Some((s, e)) = cut(top).filter(|_| wanted.is_none()) {
            let holds_sel = sel_row.is_some_and(|r| r >= s && r < e);
            top = if holds_sel { s } else { e };
        }
        let above = |top: usize| Some(hidden(&mut owners.iter().filter(|(r, _)| *r < top))).filter(|n| up_ok && n.n > 0);
        let up = above(top);
        let room = h - up.is_some() as usize;
        if top + room >= len {
            // the end of the list fits: no `↓ n more`
            return Win { top, shown: len - top, up, down: None };
        }
        let mut end = top + room - 1;
        if let Some((s, _)) = cut(end) {
            // a box that starts after a blank row: the blank goes too
            if s > top + 1 {
                end = s - 1;
            }
        }
        let down = hidden(&mut owners.iter().filter(|(r, _)| *r >= end));
        if down.n == 0 {
            // only rows with no target below (a blank, a worktree): the end
            let top = len - room;
            return Win { top, shown: room, up: up.and_then(|_| above(top)), down: None };
        }
        Win { top, shown: end - top, up, down: Some(down) }
    };
    let win = place(h >= 3);
    // the selection (its whole box when it is in one) not in view with a
    // `↑` row: none, the rows over it go unmarked (a short panel)
    let in_view = |w: &Win| {
        let Some(r) = sel_row.filter(|_| wanted.is_none()) else { return true };
        let (s, e) = boxes.iter().copied().find(|(s, e)| *s <= r && r < *e && e - s < h).unwrap_or((r, r + 1));
        s >= w.top && e <= w.top + w.shown
    };
    if win.up.is_some() && !in_view(&win) {
        return place(false);
    }
    win
}

/// A `↑ 6 more` / `↓ 24 more` row (sidebar-more, designer): dim; when
/// what it hides needs you, ` · ? 2` in accent (bold under NO_COLOR),
/// then a failure ` · ✗ 1` if it fits (the `?` wins).
fn more_line(n: Hidden, up: bool, w: usize) -> Line<'static> {
    let d = Style::default().fg(dim());
    let arrow = crate::theme::glyph(if up { "↑" } else { "↓" });
    let mut spans = vec![Span::styled(format!(" {} {} more", arrow, n.n), d)];
    let bold = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let ask_st = if bold { Style::default().add_modifier(Modifier::BOLD) } else { Style::default().fg(accent()) };
    let marks = [(n.ask, G_NEEDS_YOU, ask_st), (n.failed, G_FAILED, Style::default().fg(error()))];
    for (k, g, st) in marks.into_iter().filter(|(k, _, _)| *k > 0) {
        let tail = vec![Span::styled(" · ", d), Span::styled(format!("{} {}", crate::theme::glyph(g), k), st)];
        let wd: usize = spans.iter().chain(&tail).map(|s| s.content.width()).sum();
        // one column of margin on the right
        if wd < w {
            spans.extend(tail);
        }
    }
    Line::from(spans)
}

/// The cards section, under the live agents (BISE-125): a title row
/// `inbox`, then one row per open card in the strip's order with the
/// strip's number (BISE-302, designer: ctrl+1 opens the row that says 1
/// everywhere): ` 1 ? perf  its first line…` (the kind's glyph in its
/// color, the agent, the text cut to the row). The card in the box is on
/// the selection color; with no agent selected, the panel scrolls to it.
/// Nothing while no card is open.
fn cards_lines(
    app: &App,
    w: usize,
    lines: &mut Vec<Line<'static>>,
    owners: &mut Vec<(usize, Hit)>,
    sel_row: &mut Option<usize>,
) {
    let sb = &app.sb;
    if sb.cards.is_empty() {
        return;
    }
    let shown = sb.card.open.then(|| sb.current_card().map(|c| c.id)).flatten();
    lines.push(Line::from(""));
    owners.push((lines.len(), Hit::Cards));
    lines.push(Line::from(Span::styled(" inbox", Style::default().fg(text()))));
    let num = super::card_draw::number_style(app, dim());
    for (id, n) in super::card_draw::row_numbers(sb) {
        let Some(c) = sb.cards.iter().find(|c| c.id == id) else { continue };
        if shown == Some(c.id) && sel_row.is_none() {
            *sel_row = Some(lines.len());
        }
        owners.push((lines.len(), Hit::Card(c.id)));
        lines.push(card_row(c, n, num, w, (shown == Some(c.id)).then(selection_bg)));
    }
}

/// One card's row, `w` columns: ` 1 ? perf  first line…`, 1 column of
/// margin on the right. The agent keeps its whole name while 6 columns
/// are left for the text, then it is cut too.
fn card_row(c: &Card, n: usize, num_style: Style, w: usize, bg: Option<Color>) -> Line<'static> {
    let num = format!(" {n} ");
    let g = super::cards::kind_look(&c.kind).1;
    let lead = num.width() + g.width() + 1;
    let room = w.saturating_sub(lead + 1);
    let first = c.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    // update-card: bise's own news, `bise  v0.0.2 is out`
    let (who, first) = match c.kind.as_str() {
        "update" => ("bise", first.strip_prefix("bise ").unwrap_or(first)),
        // expired-ux: bise's own item, not an agent's
        "signin" => ("bise", first),
        _ => (c.agent.as_str(), first),
    };
    let agent = if first.is_empty() || room < 12 {
        fit(who, room)
    } else {
        fit(who, room.saturating_sub(7).max(room / 2))
    };
    let rest = room.saturating_sub(agent.width() + 2);
    let title = if rest >= 3 { fit(first, rest) } else { String::new() };
    let mut spans = vec![
        Span::styled(num, num_style),
        Span::styled(g.to_string(), Style::default().fg(super::cards::glyph_color(&c.kind))),
        Span::raw(" "),
        Span::styled(agent, Style::default().fg(text())),
    ];
    if !title.is_empty() {
        spans.push(Span::styled(format!("  {}", title), Style::default().fg(dim())));
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    spans.push(Span::raw(" ".repeat(w.saturating_sub(used))));
    if let Some(bg) = bg {
        spans = spans.into_iter().map(|s| { let st = s.style.bg(bg); s.style(st) }).collect();
    }
    Line::from(spans)
}

/// The archived section, at the bottom: a dim folded row `▸ {n}
/// archived` (`▾` open), then, open, one row per agent, newest first:
/// its name and how long ago it was last heard of; the selected or
/// focused one also shows its last report (or its objective). Folded,
/// only the archived agent in focus is listed, so the view in focus is
/// always found in the panel.
fn archived_lines(
    sb: &Sb,
    live: usize,
    w: usize,
    lines: &mut Vec<Line<'static>>,
    owners: &mut Vec<(usize, Hit)>,
    sel_row: &mut Option<usize>,
) {
    let all = sb.archived();
    if all.is_empty() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    lines.push(Line::from(""));
    owners.push((lines.len(), Hit::Archived));
    let arrow = if sb.archived_open { G_OPEN } else { G_CLOSED };
    lines.push(Line::from(Span::styled(
        format!(" {} {} archived", arrow, all.len()),
        Style::default().fg(dim()),
    )));
    for (k, a) in all.iter().enumerate() {
        let focused = a.name == sb.focus;
        if !sb.archived_open && !focused {
            continue;
        }
        let selected = sb.archived_open && sb.selected == Some(live + k);
        if selected {
            *sel_row = Some(lines.len());
        }
        let age = a.report_ms.map(|t| short_age(now.saturating_sub(t))).unwrap_or_default();
        let name_style = Style::default().fg(if focused { accent() } else { dim() });
        let first = lines.len();
        let bg = selected.then(selection_bg);
        let right = if age.is_empty() { Vec::new() } else { vec![Span::styled(format!(" {age}"), Style::default().fg(dim()))] };
        lines.push(row(None, (G_STOPPED, faint()), &a.name, name_style, Vec::new(), right, w, bg));
        if selected || focused {
            let what = if a.report.is_empty() { &a.objective } else { &a.report };
            lines.push(Line::from(Span::styled(
                format!("     {}", fit(what, w.saturating_sub(6))),
                Style::default().fg(dim()),
            )));
        }
        owners.extend((first..lines.len()).map(|r| (r, Hit::Agent(a.name.clone()))));
    }
}

/// What a panel row leads to when clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hit {
    /// Focus this agent (an archived one opens read-only).
    Agent(String),
    /// The header of the archived section: expand / collapse.
    Archived,
    /// An open card: shown in the card box (BISE-125).
    Card(u64),
    /// The title of the cards section: the card view on the top card.
    Cards,
    /// `↑ n more`: the panel scrolls a page up (sidebar-more).
    Up,
    /// `↓ n more`: the panel scrolls a page down.
    Down,
    /// A shared worktree's label `ψ sculpt`: its branch's diff.
    Place(String),
}

/// The panel's window as the last frame drew it (sidebar-more): its
/// first body row, how many list rows it showed, the selection's row
/// then, and whether rows were hidden (the wheel scrolls the panel only
/// then).
#[derive(Debug, Default, Clone, Copy)]
struct View {
    top: usize,
    shown: usize,
    sel_row: Option<usize>,
    overflow: bool,
}

/// Where the last frame drew the panel, and what each of its rows leads
/// to (screen row, target): a click on an agent row focuses the agent.
#[derive(Debug, Default, Clone)]
pub(crate) struct PanelHits {
    area: Rect,
    rows: Vec<(u16, Hit)>,
    /// The panel's numbers (Alt+N): agent name → number, kept while the
    /// agent lives (see [`Sb::numbers`]).
    slots: Vec<(String, usize)>,
    /// The first body row you scrolled to (the wheel, a `↑`/`↓` row),
    /// and the selection's row then: a new selection (Alt+↑/↓, a card
    /// shown) drops it, the panel follows the selection again.
    scroll: Option<(usize, Option<usize>)>,
    view: View,
}

impl PanelHits {
    /// Scroll the panel's body to `top` (clamped at the next draw).
    fn scroll_to(&mut self, top: usize) {
        self.scroll = Some((top, self.view.sel_row));
    }
}

impl Sb {
    /// The number of every live agent: main 0; an agent keeps its number
    /// while it lives (a drop does not renumber the others); a new one
    /// takes the smallest free number, so the first agents get 1, 2, 3 in
    /// creation order. Only 0-9 are shown and reachable with Alt+N.
    pub(crate) fn numbers(&self) -> Vec<(String, usize)> {
        let live: Vec<&Agent> = self.agents.iter().filter(|a| !a.archived()).collect();
        let Ok(mut hits) = self.panel_hits.try_borrow_mut() else {
            return assign(Vec::new(), &live);
        };
        let slots = assign(std::mem::take(&mut hits.slots), &live);
        hits.slots = slots.clone();
        slots
    }

    /// The live agent with number `n`, if any.
    pub(crate) fn agent_numbered(&self, n: usize) -> Option<String> {
        self.numbers().into_iter().find(|(_, k)| *k == n).map(|(name, _)| name)
    }
}

/// `slots` brought up to date with the `live` agents (hub order): the
/// gone ones free their number, main is 0, a newcomer takes the smallest
/// free number from 1.
fn assign(mut slots: Vec<(String, usize)>, live: &[&Agent]) -> Vec<(String, usize)> {
    slots.retain(|(name, n)| live.iter().any(|a| a.name == *name && (a.main == (*n == 0))));
    for a in live {
        if slots.iter().any(|(name, _)| *name == a.name) {
            continue;
        }
        let n = if a.main {
            0
        } else {
            (1..).find(|k| !slots.iter().any(|(_, n)| n == k)).unwrap_or(1)
        };
        slots.push((a.name.clone(), n));
    }
    slots
}

impl PanelHits {
    /// What is drawn at screen cell (`x`, `y`), if anything.
    fn hit_at(&self, x: u16, y: u16) -> Option<&Hit> {
        if !self.contains(x, y) {
            return None;
        }
        self.rows.iter().find(|(r, _)| *r == y).map(|(_, h)| h)
    }

    fn contains(&self, x: u16, y: u16) -> bool {
        let a = self.area;
        x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height)
    }
}

/// A left click in the panel: on an agent's rows it focuses that agent,
/// the same path as Alt+N. `true` when the click was the panel's.
pub(crate) fn panel_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    if matches!(m.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
        // sidebar-more: the wheel over a panel taller than its room
        // scrolls it 3 rows; else it goes on to the history
        let Ok(mut hits) = app.sb.panel_hits.try_borrow_mut() else { return false };
        if !hits.contains(m.column, m.row) || !hits.view.overflow {
            return false;
        }
        let top = hits.view.top;
        hits.scroll_to(if m.kind == MouseEventKind::ScrollUp { top.saturating_sub(3) } else { top + 3 });
        return true;
    }
    if m.kind != MouseEventKind::Down(MouseButton::Left) {
        return false;
    }
    let sb = &app.sb;
    let (target, right) = {
        let Ok(hits) = sb.panel_hits.try_borrow() else { return false };
        if !hits.contains(m.column, m.row) {
            return false;
        }
        (hits.hit_at(m.column, m.row).cloned(), hits.area.right().saturating_sub(m.column) <= 3)
    };
    match target {
        // computer use (design §7.3): a click on `↖` stops it, from anywhere
        Some(Hit::Agent(name)) if right && crate::computer_use::driving(&name).is_some() => {
            app.sb.call("turn/interrupt", serde_json::json!({"agent": name}), super::rpc::Then::Shown);
            crate::computer_use::stop(&app.sb.dir_of(&name));
        }
        // site/m/artifacts D: a click on an agent's ψ opens its diff
        Some(Hit::Agent(name)) if right && sb.agent(&name).is_some_and(|a| !a.archived() && place_label(a).is_some()) => {
            crate::diffview::request(app, crate::diffview::Ask::Agent(name), crate::diffview::By::Click);
        }
        Some(Hit::Agent(name)) if sb.agent(&name).is_some() => focus(app, &name),
        Some(Hit::Archived) => {
            let sb = &mut app.sb;
            sb.toggle_archived();
        }
        Some(Hit::Place(branch)) => crate::diffview::request(app, crate::diffview::Ask::Branch(branch), crate::diffview::By::Click),
        Some(Hit::Card(id)) => super::cards::open_view(app, Some(id)),
        Some(Hit::Cards) if app.sb.card.open => super::cards::close_view(app),
        Some(Hit::Cards) => super::cards::open_view(app, None),
        // a page that way, less one row: the last row you saw stays in
        // view (designer)
        Some(Hit::Up | Hit::Down) => {
            if let Ok(mut hits) = app.sb.panel_hits.try_borrow_mut() {
                let View { top, shown, .. } = hits.view;
                let page = shown.saturating_sub(1).max(1);
                hits.scroll_to(if target == Some(Hit::Up) { top.saturating_sub(page) } else { top + page });
            }
        }
        _ => {}
    }
    true
}

/// The state of the agent you talk to (book §8 "The frame": the right of
/// What the divider says after the name of the agent you view
/// (BISE-135, BISE-136): its model and effort, the session's approvals
/// mode, where it works.
pub(crate) fn viewed_who(app: &App) -> crate::chrome::Who {
    let sb = &app.sb;
    let Some(a) = sb.agent(&sb.focus) else {
        return crate::chrome::Who::default();
    };
    let others: Vec<&str> = sb.agents.iter().filter(|x| !x.archived()).map(|x| x.model.as_str()).collect();
    // its worktree (pr-design §4): the branch, who else is there, the PR
    let box_of = sb.place_of(a);
    crate::chrome::Who {
        model: if a.model.is_empty() { String::new() } else { crate::models::long_name(&a.model) },
        effort: a.effort.clone(),
        tag: crate::models::tag(&a.model, &a.effort, &others),
        place: box_of.and_then(|p| p.label()).or_else(|| place_label(a)),
        mode: sb.approvals.mode.clone(),
        flash: crate::keybar::flashing(&sb.approvals),
        with: box_of
            .map(|p| p.agents.iter().filter(|n| **n != a.name && sb.agent(n).is_some_and(|x| !x.archived())).cloned().collect())
            .unwrap_or_default(),
        pr: box_of.and_then(|p| p.live_pr()).map(|pr| crate::chrome::WhoPr {
            number: pr.number,
            url: pr.url.clone(),
            style: pr.mark_style(),
            words: if crate::ctrlhint::words(app) { pr.words(Style::default().fg(dim())) } else { Vec::new() },
        }),
        drives: crate::computer_use::driving(&a.name).and_then(|d| {
            let m = crate::computer_use::mark();
            let app_name = d.driving?;
            Some(match (crate::ctrlhint::words(app), d.place) {
                (true, Some(p)) if p != app_name => format!("{m} driving {app_name} · {p}"),
                (true, _) => format!("{m} driving {app_name}"),
                (false, _) => format!("{m} {app_name}"),
            })
        }),
    }
}

/// The model of the agent in view, its effort and the efforts its model
/// takes (the `/model` and `/reasoning` popups, BISE-135): (name,
/// model, effort, efforts).
pub(crate) fn viewed_model(app: &App) -> (String, String, String, Vec<String>) {
    let sb = &app.sb;
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let model = if a.model.is_empty() { sb.focus_model(app) } else { a.model.clone() };
    let efforts = if a.efforts.is_empty() && a.model.is_empty() { crate::models::efforts(&model).0 } else { a.efforts.clone() };
    (sb.focus.clone(), model, a.effort, efforts)
}

/// The agent you view, when it works (book §8, BISE-105): the gust's
/// motion and the current turn's age, for the divider's label; the age
/// and the word only with ctrl held (BISE-303).
pub(crate) fn viewed_working(app: &App) -> Option<crate::chrome::Working> {
    let sb = &app.sb;
    let a = sb.agent(&sb.focus).filter(|a| a.status == "working")?;
    Some(crate::chrome::Working {
        motion: app.motion,
        age: a.turn_age_ms().map(short_age),
        words: crate::ctrlhint::words(app),
    })
}

/// The state of the agent you talk to (book §8 "The frame": the right of
/// the divider; it was the status row), dim, richest form first
/// (BISE-303). At rest its context, short (`58k · 22%`); ctrl held, the
/// long form, after its state and the turn's age when it doesn't work
/// (`idle · 210k / 1M tokens · 21%`; working, the label says `working ·
/// 42s`). Then the notes (preview, read-only, the hub's version, the hub
/// disconnected); or the `D` question, in accent.
pub(crate) fn status_state(app: &App) -> Vec<Line<'static>> {
    let sb = &app.sb;
    if let Some(name) = &sb.drop_ask {
        return vec![Line::from(Span::styled(drop_question(name), Style::default().fg(accent())))];
    }
    // computer use (design §7.3, designer m_16125): it drives, you took
    // the wheel (it waits), or it was stopped until you write to it
    if let Some(line) = crate::computer_use::status_line(&sb.focus, crate::when::now_ms()) {
        return vec![line];
    }
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let d = |t: String| Span::styled(format!(" · {}", t), Style::default().fg(dim()));
    let usage = crate::usage::current(&app.events);
    let mut held: Vec<Span<'static>> = Vec::new();
    if a.status != "working" {
        if !a.status.is_empty() {
            held.push(d(a.status.clone()));
        }
        if let Some(ms) = a.turn_age_ms().filter(|_| app.pending) {
            held.push(d(short_age(ms)));
        }
    }
    held.extend(usage.as_ref().map(|u| d(u.label())));
    let rest: Vec<Span<'static>> = usage.as_ref().map(|u| d(u.compact())).into_iter().collect();
    let mut notes: Vec<Span<'static>> = Vec::new();
    if a.archived() {
        // the placeholder says /restore: here only the state
        notes.push(d("archived".into()));
    }
    if sb.preview {
        if let Some(sel) = sb.selected_agent().map(|a| a.name.clone()) {
            notes.push(d(format!("preview of {}", sel)));
        }
    }
    for i in &sb.versions {
        if i.marks.iter().any(|m| m == "building") {
            notes.push(d(format!("{} building {}", G_BUILDING, i.rev)));
        }
        if i.marks.iter().any(|m| m == "trial") {
            notes.push(d(format!("{} {} on trial", G_BUILDING, i.rev)));
        }
    }
    // a feature's trial (dev-flow §7) is a place's `trying`, not a version mark
    notes.extend(super::places::trial_notes(&sb.places).into_iter().map(d));
    if !sb.version.is_empty() {
        notes.push(d(format!("v {}", sb.version.chars().take(24).collect::<String>())));
    }
    if !app.connected {
        notes.push(Span::styled(" · ", Style::default().fg(dim())));
        notes.push(Span::styled(
            format!("{} hub disconnected · reconnecting…", G_IDLE),
            Style::default().fg(error()),
        ));
    }
    // the first note has no ` · ` before it
    let form = |head: &[Span<'static>]| -> Line<'static> {
        let mut spans: Vec<Span<'static>> = head.iter().chain(notes.iter()).cloned().collect();
        if let Some(first) = spans.first_mut() {
            if let Some(t) = first.content.strip_prefix(" · ") {
                first.content = t.to_string().into();
            } else if first.content == " · " {
                spans.remove(0);
            }
        }
        Line::from(spans)
    };
    if crate::ctrlhint::words(app) {
        vec![form(&held), form(&rest)]
    } else {
        vec![form(&rest)]
    }
}

/// The divider's text as one string, `name · state` (tests).
/// The header's gust without motion: one `∿` (tests draw still).
#[cfg(test)]
fn still_gust() -> Vec<Span<'static>> {
    crate::gust::mark(crate::gust::Motion::Still, crate::gust::Size::Five)
}

#[cfg(test)]
pub(crate) fn status_text(app: &App) -> String {
    let sb = &app.sb;
    let state: String = status_state(app)[0].spans.iter().map(|s| s.content.as_ref()).collect();
    if sb.drop_ask.is_some() {
        state
    } else if state.is_empty() {
        sb.focus.clone()
    } else {
        format!("{} · {}", sb.focus, state)
    }
}

/// What `D` asks in the status row (book §16, BISE-43).
pub(crate) fn drop_question(name: &str) -> String {
    format!("archive {}? /restore brings it back. y / n", name)
}

/// The key bar's mode once no overlay or popup has it (BISE-99,
/// [`crate::keybar`]).
pub(crate) fn key_mode(app: &App) -> crate::keybar::Mode {
    use crate::keybar::Mode;
    let sb = &app.sb;
    if sb.drop_ask.is_some() {
        Mode::DropAsk
    } else if sb.confirm.is_some() {
        Mode::Confirm
    } else if sb.card.open {
        Mode::Card
    } else if sb.selected.is_some() {
        Mode::Selected
    } else if sb.focus_archived() {
        Mode::Archived
    } else if app.pending {
        Mode::Steer
    } else {
        Mode::Default
    }
}

/// What the empty composer shows after the cursor, dim (book §8 "The
/// frame"): `what's on your mind?` to main, `talk to auth-fix directly`
/// inside an agent, `/restore` in an archived agent's history (it reads
/// nothing: a plain message stays in the composer, `archived_refusal`).
pub(crate) fn placeholder(app: &App) -> Option<String> {
    let sb = &app.sb;
    Some(if sb.focus_archived() {
        format!("{} is archived · /restore to talk to it", sb.focus)
    } else if sb.is_main_focus() {
        PLACEHOLDER_MAIN.to_string()
    } else {
        format!("talk to {} directly", sb.focus)
    })
}

/// The line an archived agent's view says when ⏎ would send it a plain
/// message (designer's words): it is not sent.
pub(crate) fn archived_warn(name: &str) -> String {
    format!("{name} is archived. /restore brings it back · esc → main")
}

/// ⏎ on `text` in the view in focus: an archived agent reads nothing,
/// so a plain message (not a `/command`) is refused with its warn line
/// and stays in the composer. None: it goes.
pub(crate) fn archived_refusal(app: &App, text: &str) -> Option<String> {
    let sb = &app.sb;
    (sb.focus_archived() && !text.trim_start().starts_with('/')).then(|| archived_warn(&sb.focus))
}

/// The empty composer's question, to main (copy deck §17).
pub(crate) const PLACEHOLDER_MAIN: &str = "what's on your mind?";

/// The first-run text (book §8, §17): shown dim in main's empty feed
/// while there are no agents yet.
pub(crate) const FIRST_RUN: [&str; 3] = [
    "what's on your mind?",
    "say it and keep talking. the work runs in the background, i'm always here.",
    "try: \"show me what you can do\"",
];

/// The first-run text's suggestion (BISE-284): a click on it fills the
/// composer, and the first open after the onboarding starts with it.
pub(crate) const DEMO: &str = "show me what you can do";

/// What the first-run text's last line says while the composer holds
/// [`DEMO`] (designer, BISE-284): the key, then the words.
pub(crate) const DEMO_READY: (&str, &str) = ("⏎", " try it · or just type your own");

/// The composer holds `show me what you can do`, selected: typing
/// replaces it, enter sends it, an arrow or esc keeps it (BISE-284).
pub(crate) fn fill_demo(app: &mut App) {
    app.ed.set(DEMO, DEMO.chars().count());
    app.ed.select_all();
    app.feed_sel = None;
}

/// The first open of main's thread after the first-run onboarding: the
/// composer starts with [`DEMO`] (BISE-284), when the first-run text
/// shows and nothing is typed yet.
pub(crate) fn prefill_demo(app: &mut App) {
    let said = app.events.iter().any(|e| matches!(e, crate::Ev::You(..)));
    if app.sb.first_run().is_some() && app.ed.is_empty() && !said {
        fill_demo(app);
    }
}

/// The composer holds [`DEMO`] as it came: the first-run text says
/// `⏎ try it` (BISE-284).
pub(crate) fn demo_ready(app: &App) -> bool {
    app.ed.text == DEMO
}

impl Sb {
    /// No agents yet and main in view: the first-run text, which shows
    /// until your first message (the setup card may wait in the strip,
    /// BISE-245).
    pub(crate) fn first_run(&self) -> Option<[&'static str; 3]> {
        let live = self.agents.iter().any(|a| !a.main && !a.archived());
        (self.focus == "main" && !live && !self.preview).then_some(FIRST_RUN)
    }

    /// The line pinned on top of the feed: the preview of the selected
    /// agent, or, inside an agent, that main is out of the loop.
    pub(crate) fn feed_banner(&self) -> Option<Line<'static>> {
        let dim = Style::default().fg(dim());
        if self.preview {
            let name = self.selected_agent().map(|a| a.name.clone()).filter(|n| *n != self.focus)?;
            return Some(Line::from(vec![
                Span::styled("preview · ", dim),
                Span::styled(name, Style::default().fg(text())),
                Span::styled(" · ⏎ enter · esc close", dim),
            ]));
        }
        if self.focus == "main" || self.focus_archived() || self.agent(&self.focus).is_none() {
            return None;
        }
        Some(Line::from(Span::styled(
            format!("you're talking to {} directly. main isn't in the loop. esc back to main.", self.focus),
            dim,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        let w = buf.area.width as usize;
        buf.content
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect()
    }

    fn click(app: &mut App, column: u16, row: u16) {
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        crate::input::on_mouse(app, &m, 0);
    }

    /// The screen row and column where `label` shows in the panel.
    fn find(rows: &[String], panel_x: u16, label: &str) -> (u16, u16) {
        rows.iter()
            .enumerate()
            .find_map(|(y, r)| {
                let tail: String = r.chars().skip(panel_x as usize).collect();
                tail.find(label).map(|_| (panel_x + 3, y as u16))
            })
            .unwrap_or_else(|| panic!("{} not in the panel:\n{}", label, rows.join("\n")))
    }

    /// A click on an agent's row focuses it, like Alt+N (the objective
    /// under the selected row too); a click on the title changes nothing.
    #[test]
    fn a_click_on_an_agent_row_focuses_it() {
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        bench::add_agent(&mut app, "alpha", "first objective");
        bench::add_agent(&mut app, "beta", "second objective");
        bench::add_agent(&mut app, "gamma", "third objective");
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let draw = |app: &mut App, term: &mut Terminal<TestBackend>| {
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        };
        draw(&mut app, &mut term);
        let panel_x = app.sb.panel_hits.borrow().area.x;
        for (label, name) in [("alpha", "alpha"), ("beta", "beta")] {
            let (x, y) = find(&screen(&term), panel_x, label);
            click(&mut app, x, y);
            assert_eq!(app.sb.focus, name, "click on {:?}", label);
            draw(&mut app, &mut term);
        }
        // the selected agent shows its objective under its row
        let sb = &mut app.sb;
        sb.selected = Some(3);
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "third objective");
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "gamma");
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "main");
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "main");
        // the title row: nothing happens
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, PANEL_TITLE);
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "main");
        // a click in the feed is not the panel's
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!panel_mouse(&mut app, &m));
    }

    fn agent(name: &str, status: &str) -> Agent {
        Agent { name: name.into(), status: status.into(), ..Agent::default() }
    }

    /// main and one agent in every state, as in the mockup.
    fn every_state() -> App {
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        sb.agents.push(Agent { turn_ms: Some(12 * 60_000), ..agent("auth-fix", "working") });
        sb.agents.push(agent("tests", "starting"));
        sb.agents.push(agent("docs", "waiting"));
        sb.agents.push(Agent { waiting_on: "docs".into(), ..agent("api-v2", "waiting") });
        sb.agents.push(agent("bench", "done"));
        sb.agents.push(agent("deploy", "failed"));
        sb.agents.push(agent("ideas", "idle"));
        sb.agents.push(agent("old-spike", "stopped"));
        sb.agents.push(Agent { branch: Some("sb/big".into()), mode: "worktree".into(), ..agent("big-refactor-of-auth", "working") });
        sb.agents.push(agent("eleventh", "idle"));
        sb.cards.push(Card {
            id: 1,
            kind: "question".into(),
            agent: "docs".into(),
            text: "v1 or v2?".into(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
            look: None,
            place: None,
            pr: None,
            link: None,
            asking: false,
            waiting: Vec::new(),
        });
        sb.activity.insert("auth-fix".into());
        app
    }

    /// The panel alone, `w` columns wide, `h` rows high.
    fn panel_rows(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_panel(app, f, f.area())).unwrap();
        screen(&term)
    }

    fn trimmed(rows: &[String]) -> Vec<String> {
        rows.iter().map(|r| r.trim_end().to_string()).collect()
    }

    /// The row layout at the two panel widths (BISE-303): number (0-9,
    /// blank after), status glyph, name, marks, then the columns: the
    /// turn's time (working only) and the context %, right-aligned in 3
    /// columns each, ψ, one column of margin; no state word, no model
    /// tag; the title `agents` alone.
    #[test]
    fn panel_rows_at_28_and_40() {
        let mut app = every_state();
        app.sb.agents.push(Agent { model: "foundry/claude-opus-5-5".into(), effort: "high".into(), ..agent("tagged", "idle") });
        for w in [28u16, 40] {
            let rows = panel_rows(&app, w, 16);
            let t = trimmed(&rows);
            let row = |n: &str| t.iter().find(|r| r.contains(n)).unwrap_or_else(|| panic!("{} missing:\n{}", n, t.join("\n"))).clone();
            assert_eq!(t[0], format!(" {}", PANEL_TITLE), "the title alone at {}", w);
            assert_eq!(row("main"), format!(" 0 {} main {}", G_IDLE, G_MAIN));
            // the time column ends 9 columns from the right edge
            let time_end = |n: &str, time: &str| {
                let r = row(n);
                assert!(r.ends_with(time), "{:?} ends with {:?} at {}", r, time, w);
                assert_eq!(r.chars().count(), w as usize - 9, "{:?} in the time column at {}", r, w);
                assert_eq!(rows.iter().find(|x| x.contains(n)).unwrap().chars().count(), w as usize);
            };
            time_end("auth-fix", "12m");
            assert!(row("auth-fix").starts_with(&format!(" 1 {} auth-fix {}", G_WORKING, G_UNREAD)));
            // the glyph says the state: no word
            for (n, g) in [("tests", ""), ("docs", G_NEEDS_YOU), ("api-v2", G_WAITING), ("bench", G_DONE), ("deploy", G_FAILED), ("ideas", G_IDLE), ("old-spike", G_STOPPED)] {
                assert!(row(n).ends_with(n), "{:?}: no word at {}", row(n), w);
                assert!(row(n).contains(&format!("{g} {n}")), "{:?}", row(n));
            }
            assert!(row("docs").starts_with(&format!(" 3 {} docs", G_NEEDS_YOU)));
            // ψ in its column, one from the edge
            let big = row("big-re");
            assert!(big.starts_with(&format!(" 9 {} big-re", G_WORKING)) && big.ends_with(G_WORKTREE), "{big:?}");
            assert_eq!(big.chars().count(), w as usize - 1);
            // no model tag at rest
            assert!(row("tagged").ends_with("tagged") && !t.iter().any(|r| r.contains("opus")), "{t:?}");
            // no number after 9
            assert!(row("eleventh").starts_with(&format!("   {} eleventh", G_IDLE)), "{:?}", row("eleventh"));
            assert!(!t.iter().any(|r| r.to_lowercase().contains("task")), "no \"task\" in the panel");
        }
        // 28 columns: the time goes, then the %, before the name is cut;
        // ψ kept in its column (sidebar-wt, option A)
        let t = trimmed(&panel_rows(&app, 28, 16));
        let big = t.iter().find(|r| r.contains("big-")).unwrap();
        assert_eq!(big, &format!(" 9 {} big-refactor-of-auth {}", G_WORKING, G_WORKTREE));
        // narrower still: the name is cut, ψ kept
        let t = trimmed(&panel_rows(&app, 24, 16));
        let big = t.iter().find(|r| r.contains("big-")).unwrap();
        assert!(big.contains("…") && big.ends_with(G_WORKTREE) && big.chars().count() == 23, "{:?}", big);
        // with room, the whole name: no fixed cap (BISE-109)
        let t = trimmed(&panel_rows(&app, 40, 16));
        assert!(t.iter().any(|r| r.contains("big-refactor-of-auth ")), "{}", t.join("\n"));
    }

    /// BISE-303: ctrl held, the state word takes the time and % columns
    /// (8 wide, right-aligned; `you` in accent), ψ stays; nothing else
    /// on the row moves.
    #[test]
    fn ctrl_held_writes_the_state_words_in_the_columns() {
        use crate::ctrlhint::{Held, Hold};
        let mut app = every_state();
        let rest = panel_rows(&app, 40, 16);
        app.hold = Hold::of(Held::Ctrl, std::time::Instant::now() - std::time::Duration::from_secs(2));
        let held = panel_rows(&app, 40, 16);
        let t = trimmed(&held);
        let row = |n: &str| t.iter().find(|r| r.contains(n)).cloned().unwrap_or_default();
        for (n, word) in [("main", "idle"), ("auth-fix", "working"), ("tests", "starting"), ("docs", "you"), ("api-v2", "waiting"), ("bench", "done"), ("deploy", "failed"), ("ideas", "idle"), ("old-spike", "stopped")] {
            assert!(row(n).ends_with(word), "{:?} says {word}", row(n));
            assert_eq!(row(n).chars().count(), 40 - 4, "{:?}: the word ends at the % column", row(n));
        }
        let big = row("big-re");
        assert!(big.ends_with(&format!("working  {}", G_WORKTREE)), "{big:?}");
        // the left part of every row is the same
        for (a, b) in rest.iter().zip(&held) {
            let left = |r: &str| r.chars().take(20).collect::<String>();
            assert_eq!(left(a), left(b));
        }
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let y = t.iter().position(|r| r.contains("docs")).unwrap();
        assert_eq!(term.backend().buffer()[(32, y as u16)].fg, accent(), "{:?}", t[y]);
    }

    /// BISE-135: the model and effort are on the divider (no tag in the
    /// panel since BISE-303).
    #[test]
    fn the_divider_names_the_viewed_model() {
        let mut app = bench::test_app_drained();
        let with = |a: Agent, m: &str, e: &str| Agent { model: m.into(), effort: e.into(), ..a };
        app.sb.agents = vec![
            with(Agent { main: true, ..agent("main", "idle") }, "foundry/claude-opus-5-5", "high"),
            with(agent("release", "working"), "anthropic/claude-sonnet-4-5", "low"),
        ];
        app.sb.focus = "release".into();
        let w = viewed_who(&app);
        assert_eq!((w.model.as_str(), w.effort.as_str(), w.tag.as_str()), ("sonnet 4.5", "low", "sonnet·lo"));
    }

    /// BISE-136: ψ marks an agent out of the shared checkout (a hub
    /// worktree or a private one), with its branch or worktree name in
    /// the divider; the shared checkout shows nothing.
    #[test]
    fn where_an_agent_works() {
        let hub = Agent { branch: Some("sb/big".into()), mode: "worktree".into(), ..agent("big", "working") };
        let private = Agent { mode: "shared".into(), place: "/tmp/fix-wt/".into(), ..agent("fix", "working") };
        let shared = Agent { mode: "shared".into(), path: "/ws".into(), ..agent("docs", "idle") };
        assert_eq!(place_label(&hub).as_deref(), Some("sb/big"));
        assert_eq!(place_label(&private).as_deref(), Some("fix-wt"));
        assert_eq!(place_label(&shared), None);
        let mut app = bench::test_app_drained();
        app.sb.agents = vec![Agent { main: true, ..agent("main", "idle") }, private, shared];
        let t = trimmed(&panel_rows(&app, 40, 8));
        assert!(t.iter().any(|r| r.contains("fix") && r.ends_with(G_WORKTREE)), "{t:?}");
        assert!(!t.iter().any(|r| r.contains("docs") && r.contains(G_WORKTREE)), "{t:?}");
        // the place is in the divider's label (BISE-135: after the model),
        // not in the state
        app.sb.focus = "fix".into();
        assert_eq!(viewed_who(&app).place.as_deref(), Some("fix-wt"));
        let state: String = status_state(&app)[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains(G_WORKTREE), "{state:?}");
        app.sb.focus = "docs".into();
        assert_eq!(viewed_who(&app).place, None);
        let state: String = status_state(&app)[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains(G_WORKTREE) && !state.contains("shared"), "{state:?}");
    }

    /// Colors: the number faint, the agent in view in accent, "needs
    /// you" in accent, a failure in error, the right side dim.
    #[test]
    fn panel_colors() {
        let mut app = every_state();
        app.sb.focus = "bench".into();
        // ctrl held: the state words (BISE-303)
        app.hold = crate::ctrlhint::Hold::of(crate::ctrlhint::Held::Ctrl, std::time::Instant::now() - std::time::Duration::from_secs(2));
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let rows = screen(&term);
        let buf = term.backend().buffer();
        let at = |n: &str, what: &str| {
            let y = rows.iter().position(|r| r.contains(n)).unwrap();
            let x = rows[y].find(what).map(|b| rows[y][..b].chars().count()).unwrap();
            buf.cell((x as u16, y as u16)).unwrap().fg
        };
        assert_eq!(at("main", "0"), faint());
        assert_eq!(at("main", G_MAIN), accent());
        assert_eq!(at("bench", "bench"), accent(), "the agent in view");
        assert_eq!(at("auth-fix", "auth-fix"), text());
        assert_eq!(at("docs", G_NEEDS_YOU), accent());
        assert_eq!(at("docs", "you"), accent());
        assert_eq!(at("deploy", G_FAILED), error());
        assert_eq!(at("deploy", "failed"), dim());
        assert_eq!(at("auth-fix", G_UNREAD), accent());
    }

    /// BISE-119: main's row lines up with the others: its status in the
    /// agents' glyph column (the breath while it works, a new frame
    /// rewrites that one cell only; `○` idle; no motion, one static `∿`),
    /// its name in their name column, `:*` (accent, still) after it.
    #[test]
    fn main_status_sits_in_the_glyph_column() {
        use crate::gust::{Motion, W1};
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "working".into(), ..Agent::default() });
        sb.agents.push(agent("ideas", "idle"));
        let mut term = Terminal::new(TestBackend::new(28, 6)).unwrap();
        let mut draw = |app: &mut App, m: Motion| {
            (app.motion, app.motion_away) = (m, m);
            term.draw(|f| draw_panel(app, f, f.area())).unwrap();
            (screen(&term), term.backend().buffer().clone())
        };
        let row_of = |rows: &[String], n: &str| rows.iter().find(|r| r.contains(n)).unwrap().trim_end().to_string();
        let mark_x = " 0 ∿ main ".chars().count() as u16;
        let mut last = None;
        for i in 0..W1.breath.len() as u64 {
            let (rows, buf) = draw(&mut app, Motion::Frame(i));
            let breath = W1.breath[i as usize];
            assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", breath, G_MAIN), "frame {i}");
            // the same columns as an agent's glyph and name
            let ideas = row_of(&rows, "ideas");
            assert_eq!(ideas.chars().position(|c| c == 'i'), row_of(&rows, "main").chars().position(|c| c == 'm'));
            let y = rows.iter().position(|r| r.contains("main")).unwrap() as u16;
            assert_eq!(buf.cell((3, y)).unwrap().fg, crate::gust::cell(Motion::Frame(i)).1, "frame {i}");
            assert_eq!(buf.cell((mark_x, y)).unwrap().fg, accent(), "`:*` stays accent");
            // the next frame rewrites main's gust cell, nothing else
            if let Some(prev) = last.replace(buf.clone()) {
                let d = prev.diff(&buf);
                assert_eq!(d.iter().map(|(x, y, _)| (*x, *y)).collect::<Vec<_>>(), vec![(3, y)], "frame {i}");
            }
        }
        // no motion (focus lost, BISE_REDUCE_MOTION, a slow draw): one still `∿`
        let (rows, a) = draw(&mut app, Motion::Still);
        assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", W1.still.0, G_MAIN));
        let (_, b) = draw(&mut app, Motion::Still);
        assert!(a.diff(&b).is_empty());
        // idle: an idle agent's glyph, and the frames change no cell
        app.sb.agents[0].status = "idle".into();
        let (rows, a) = draw(&mut app, Motion::Frame(0));
        assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", G_IDLE, G_MAIN));
        assert!(row_of(&rows, "ideas").starts_with(&format!(" 1 {} ideas", G_IDLE)));
        let (_, b) = draw(&mut app, Motion::Frame(1));
        assert!(a.diff(&b).is_empty());
        // zen (BISE-132): the panel's gust stands still while the one of
        // the agent in view (the divider's label) is calm
        app.sb.agents[0].status = "working".into();
        app.motion = Motion::Calm(3);
        app.motion_away = Motion::Still;
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        assert_eq!(row_of(&screen(&term), "main"), format!(" 0 {} main {}", W1.still.0, G_MAIN));
        app.sb.focus = "main".into();
        assert_eq!(viewed_working(&app).map(|w| w.motion), Some(Motion::Calm(3)));
    }

    /// Numbers stay while an agent lives: a drop does not renumber the
    /// others, Alt+N follows the number shown; a newcomer takes the free
    /// number, and the rows go in number order (QA M).
    #[test]
    fn numbers_survive_a_drop() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut app = bench::test_app_drained();
        {
            let sb = &mut app.sb;
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for n in ["a", "b", "c"] {
                sb.agents.push(agent(n, "working"));
            }
        }
        let num = |app: &App, n: &str| app.sb.numbers().into_iter().find(|(x, _)| x == n).map(|(_, k)| k);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!((num(&app, "a"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        // a is dropped (archived): b and c keep 2 and 3
        app.sb.agents[1].status = "archived".into();
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 2 {} b", G_WORKING))), "{}", t.join("\n"));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!(num(&app, "a"), None);
        // Alt+3 goes to c, Alt+1 to no one, Alt+0 to main
        key(&mut app, &KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "c");
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "c", "no agent 1 any more");
        key(&mut app, &KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "main");
        // a newcomer takes the free number 1; the others keep theirs
        app.sb.agents.push(agent("d", "starting"));
        assert_eq!((num(&app, "d"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        // QA M: its row is at its number, before b and c (not last)
        let t = trimmed(&panel_rows(&app, 28, 10));
        let row_of = |p: &str| t.iter().position(|r| r.starts_with(p)).unwrap_or(usize::MAX);
        assert!(row_of(" 1 ") < row_of(" 2 ") && row_of(" 2 ") < row_of(" 3 "), "{}", t.join("\n"));
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "d");
        // restored, a comes back with a free number (4)
        app.sb.agents[1].status = "idle".into();
        assert_eq!(num(&app, "a"), Some(4));
    }

    /// sidebar-more: a click on `↓ n more` scrolls a page less one row
    /// (the last row seen stays), `↑ n more` back; the wheel 3 rows; the
    /// rows say when a hidden agent needs you or failed; a new selection
    /// takes the scroll back; every agent is reached.
    #[test]
    fn the_more_rows_scroll_the_panel() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut app = bench::test_app_drained();
        {
            let sb = &mut app.sb;
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for i in 1..=30 {
                sb.agents.push(agent(&format!("a{:02}", i), "working"));
            }
            sb.agents[25].waiting_on = "you".into();
            sb.agents[28].waiting_on = "you".into();
            sb.agents[29].status = "failed".into();
        }
        let ev = |kind, row| MouseEvent { kind, column: 3, row, modifiers: KeyModifiers::NONE };
        let click = |app: &mut App, row| panel_mouse(app, &ev(MouseEventKind::Down(MouseButton::Left), row));
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert_eq!(t[9], format!(" ↓ 24 more · {G_NEEDS_YOU} 2 · {G_FAILED} 1"), "{}", t.join("\n"));
        let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let x = t[9].chars().position(|c| c == '?').unwrap() as u16;
        assert_eq!(term.backend().buffer().cell((x, 9)).unwrap().fg, accent());
        // narrow: the failure goes, the question stays
        let t = trimmed(&panel_rows(&app, 22, 10));
        assert_eq!(t[9], format!(" ↓ 24 more · {G_NEEDS_YOU} 2"), "{}", t.join("\n"));
        // a click on `↓`: a06 (the last row seen) is now under `↑ 6 more`
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert!(t[8].contains("a06"), "{}", t.join("\n"));
        assert!(click(&mut app, 9));
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert_eq!(t[2], " ↑ 6 more", "{}", t.join("\n"));
        assert!(t[3].contains("a06"), "{}", t.join("\n"));
        // down to the end: the last agent shows, no `↓` row
        for _ in 0..10 {
            if !trimmed(&panel_rows(&app, 40, 10))[9].contains('↓') {
                break;
            }
            assert!(click(&mut app, 9));
        }
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert!(t[9].contains("a30"), "{}", t.join("\n"));
        assert!(t[2].starts_with(" ↑ ") && t[2].contains(" more"), "{}", t.join("\n"));
        // a click on the last agent focuses it
        assert!(click(&mut app, 9));
        assert_eq!(app.sb.focus, "a30");
        // `↑` back up, then the wheel: 3 rows a notch
        for _ in 0..10 {
            if !trimmed(&panel_rows(&app, 40, 10))[2].contains('↑') {
                break;
            }
            assert!(click(&mut app, 2));
        }
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert!(t[2].contains("main"), "{}", t.join("\n"));
        assert!(panel_mouse(&mut app, &ev(MouseEventKind::ScrollDown, 5)));
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert_eq!(t[2], " ↑ 3 more", "main, a01, a02 went up: {}", t.join("\n"));
        assert!(t[3].contains("a03"), "{}", t.join("\n"));
        // a selection (Alt+↓) takes the scroll back
        app.sb.selected = Some(0);
        let t = trimmed(&panel_rows(&app, 40, 10));
        assert!(t[2].contains("main"), "{}", t.join("\n"));
        // a panel with room: the wheel goes on to the history
        let t = trimmed(&panel_rows(&app, 40, 60));
        assert!(!t.iter().any(|r| r.contains("more")));
        assert!(!panel_mouse(&mut app, &ev(MouseEventKind::ScrollDown, 5)));
    }

    /// More agents than rows: the list ends with `↓ {n} more`, and
    /// scrolls to keep the selection in view.
    #[test]
    fn overflow_ends_with_more() {
        let mut app = bench::test_app_drained();
        {
            let sb = &mut app.sb;
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for i in 1..=30 {
                sb.agents.push(agent(&format!("a{:02}", i), "working"));
            }
        }
        let t = trimmed(&panel_rows(&app, 28, 10));
        // title + 1 blank row + 7 agents + the more row
        assert_eq!(t[9], " ↓ 24 more", "{}", t.join("\n"));
        app.sb.selected = Some(30);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.contains("a30")), "{}", t.join("\n"));
        assert!(!t.iter().any(|r| r.contains("↓")), "nothing left below");
        // sidebar-more: what is hidden over it is counted on the first row
        assert_eq!(t[2], " ↑ 24 more", "{}", t.join("\n"));
        app.sb.selected = Some(15);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.contains("a15")), "{}", t.join("\n"));
        assert!(t[9].contains("more"), "{}", t.join("\n"));
    }
}

#[cfg(test)]
mod archived_tests {
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }

    /// main, one live task, three archived ones (`old` 5 h ago, `mid`
    /// 2 h, `new` 10 min: listed new, mid, old).
    fn app() -> App {
        let mut app = bench::test_app_drained();
        let now = now_ms();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        bench::add_agent(&mut app, "alpha", "live objective");
        let sb = &mut app.sb;
        for (name, h_ago) in [("old", 300u64), ("new", 10), ("mid", 120)] {
            sb.agents.push(Agent {
                name: name.into(),
                status: "archived".into(),
                objective: format!("{} objective", name),
                report: format!("{} did its job", name),
                report_ms: Some(now - h_ago * 60_000),
                ..Agent::default()
            });
        }
        app
    }

    fn draw(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<String> {
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        let buf = term.backend().buffer();
        let w = buf.area.width as usize;
        buf.content
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect()
    }

    fn panel(rows: &[String], x: u16) -> Vec<String> {
        rows.iter().map(|r| r.chars().skip(x as usize).collect::<String>()).collect()
    }

    fn row_of(rows: &[String], label: &str) -> Option<usize> {
        rows.iter().position(|r| r.contains(label))
    }

    fn click(app: &mut App, column: u16, row: u16) {
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        crate::input::on_mouse(app, &m, 0);
    }

    fn press(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
        key(app, &KeyEvent::new(code, m), false)
    }

    /// Collapsed: one dim header, no archived name. A click on it
    /// expands the list, newest first, dim; a click on a row opens that
    /// task's history read-only (status row, placeholder, typed text not
    /// sent); a second click on the header folds the list, the task in
    /// focus stays listed.
    #[test]
    fn archived_section_folds_expands_and_opens_read_only() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let rows = draw(&mut app, &mut term);
        let x = app.sb.panel_hits.borrow().area.x;
        let p = panel(&rows, x);
        let head = row_of(&p, "▸ 3 archived").unwrap_or_else(|| panic!("{}", p.join("\n")));
        for n in [&format!("{} old", G_STOPPED), &format!("{} mid", G_STOPPED), &format!("{} new", G_STOPPED)] {
            assert!(row_of(&p, n).is_none(), "{} shown while folded", n);
        }
        let buf = term.backend().buffer().clone();
        let cell = buf.cell((x + 3, head as u16)).unwrap();
        assert_eq!(cell.fg, dim(), "the header is dim");

        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▾ 3 archived").is_some(), "{}", p.join("\n"));
        let (n, m, o) = (
            row_of(&p, &format!("{} new", G_STOPPED)).unwrap(),
            row_of(&p, &format!("{} mid", G_STOPPED)).unwrap(),
            row_of(&p, &format!("{} old", G_STOPPED)).unwrap(),
        );
        assert!(n < m && m < o, "newest first:\n{}", p.join("\n"));
        assert!(p[n].contains("10m") && p[o].contains("5h"), "{}", p.join("\n"));
        assert!(row_of(&p, "did its job").is_none(), "no report line when not selected");
        let buf = term.backend().buffer().clone();
        let name_x = x + p[m].find("mid").map(|b| p[m][..b].chars().count()).unwrap() as u16;
        assert_eq!(buf.cell((name_x, m as u16)).unwrap().fg, dim(), "archived names are dim");

        click(&mut app, x + 5, m as u16);
        assert_eq!(app.sb.focus, "mid");
        let rows = draw(&mut app, &mut term);
        let all = rows.join("\n");
        assert!(all.contains("archived ─"), "the divider says archived: {}", all);
        assert_eq!(all.matches("/restore").count(), 1, "the placeholder alone says /restore: {}", all);
        assert!(panel(&rows, x).iter().any(|r| r.contains("mid did its job")), "{}", all);
        assert_eq!(placeholder(&app).unwrap(), "mid is archived · /restore to talk to it");
        let out = handle_input(&mut app, "hello");
        assert!(matches!(&out[..], [Ev::Warn(w)] if w == "mid is archived. /restore brings it back · esc → main"), "not sent");
        // ⏎ in the composer: the warn line, the message stays there;
        // a command still runs
        assert_eq!(archived_refusal(&app, "hello").as_deref(), Some("mid is archived. /restore brings it back · esc → main"));
        assert_eq!(archived_refusal(&app, "/restore mid"), None);
        app.ed.text = "hello".into();
        app.ed.cursor = 5;
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.ed.text, "hello", "the message stays in the composer");
        assert!(app.events.iter().any(|e| matches!(e, Ev::Warn(w) if w.starts_with("mid is archived."))));
        app.ed.text.clear();
        app.ed.cursor = 0;

        let p = panel(&rows, x);
        let head = row_of(&p, "▾ 3 archived").unwrap();
        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▸ 3 archived").is_some());
        assert!(row_of(&p, &format!("{} mid", G_STOPPED)).is_some(), "the focused archived task stays listed");
        assert!(row_of(&p, &format!("{} new", G_STOPPED)).is_none());
    }

    /// Keys: A expands from a selection, ⌥↑↓ walk into the archived
    /// rows (the selected one shows its report), ⏎ opens it; Alt+N never
    /// lands on an archived task; D does not drop one.
    #[test]
    fn archived_keys() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        press(&mut app, KeyCode::Down, KeyModifiers::ALT);
        assert_eq!(app.sb.nav().len(), 2);
        press(&mut app, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(app.sb.archived_open);
        assert_eq!(app.sb.selected, Some(0), "selection kept");
        press(&mut app, KeyCode::Up, KeyModifiers::ALT);
        let sb = &app.sb;
        assert_eq!(sb.selected_agent().map(|a| a.name.as_str()), Some("old"));
        let x = sb.panel_hits.borrow().area.x;
        let p = panel(&draw(&mut app, &mut term), x.max(90));
        assert!(row_of(&p, "old did its job").is_some(), "{}", p.join("\n"));
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.focus, "old");
        press(&mut app, KeyCode::Char('2'), KeyModifiers::ALT);
        assert_eq!(app.sb.focus, "old", "Alt+2: no live task 2");
        press(&mut app, KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(app.sb.focus, "alpha");
    }

    /// Hundreds of archived tasks, expanded: the panel scrolls to keep
    /// the selected row in view, and clicks still hit the right row.
    #[test]
    fn a_long_archived_list_scrolls_to_the_selection() {
        let mut app = app();
        let sb = &mut app.sb;
        for i in 0..300u64 {
            sb.agents.push(Agent {
                name: format!("t{:03}", i),
                status: "archived".into(),
                report_ms: Some(1_000 + i),
                ..Agent::default()
            });
        }
        sb.archived_open = true;
        // the oldest: t000, last of the list
        sb.selected = sb.nav().iter().position(|a| a.name == "t000");
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let rows = draw(&mut app, &mut term);
        let x = app.sb.panel_hits.borrow().area.x;
        let p = panel(&rows, x);
        let y = row_of(&p, &format!("{} t000", G_STOPPED)).unwrap_or_else(|| panic!("{}", p.join("\n")));
        click(&mut app, x + 5, y as u16);
        assert_eq!(app.sb.focus, "t000");
    }
}

#[cfg(test)]
mod chrome_tests {
    use super::super::bench;
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    /// Ctrl held alone for 2 s: the words (BISE-303).
    fn held_ctrl() -> crate::ctrlhint::Hold {
        crate::ctrlhint::Hold::of(crate::ctrlhint::Held::Ctrl, std::time::Instant::now() - std::time::Duration::from_secs(2))
    }

    fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        let buf = term.backend().buffer();
        buf.content
            .chunks(w as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect()
    }

    fn with_main() -> App {
        let mut app = bench::test_app_drained();
        app.sb.agents.push(Agent {
            name: "main".into(),
            main: true,
            status: "idle".into(),
            ..Agent::default()
        });
        app
    }

    fn busy() -> App {
        let mut app = with_main();
        let sb = &mut app.sb;
        for (n, st) in [("auth-fix", "working"), ("release", "working"), ("big", "working"), ("api-v2", "waiting"), ("docs", "blocked"), ("bench", "done"), ("ideas", "idle")] {
            sb.agents.push(Agent { name: n.into(), status: st.into(), ..Agent::default() });
        }
        // BISE-299: docs needs you through its card in your inbox (main
        // escalated its question), not through its blocked status
        sb.cards.push(Card { id: 7, kind: "question".into(), agent: "docs".into(), ..Card::default() });
        app
    }

    /// BISE-299: a blocked task without a card in your inbox is main's:
    /// its `?` is dim, the header counts no "needs you"; main's inbox
    /// shows as a dim `@ 2` on main's row.
    #[test]
    fn a_blocked_task_is_mains_not_yours() {
        let mut app = busy();
        app.sb.cards.clear();
        let docs = app.sb.agents.iter().find(|a| a.name == "docs").unwrap();
        assert!(!needs_you(&app.sb, docs));
        assert_eq!(glyph("blocked", 0, crate::gust::Motion::Still), (G_NEEDS_YOU, dim()));
        let main = app.sb.agents.iter_mut().find(|a| a.main).unwrap();
        main.inbox = 2;
        let main = app.sb.agents.iter().find(|a| a.main).unwrap().clone();
        let t = draw(&mut app, 120, 20);
        let row = t.iter().find(|r| r.contains(" 0 ")).unwrap().trim_end().trim_end_matches('│').trim_end();
        assert!(row.ends_with(&format!("main {} {} 2", G_MAIN, crate::theme::glyph(G_MSG))), "{row:?}");
        let _ = main;
    }

    /// The header at 120 columns (panel shown): `bise :*` left, the long
    /// counts flush right; at 60 (no panel) the short counts.
    /// QA 14: a narrow header keeps what fits, "needs you" first, in
    /// the §8 order; the words go before the counts do.
    #[test]
    fn a_narrow_header_keeps_needs_you_first() {
        let n = [3, 1, 1, 2, 0, 0];
        let text = |room| fit_counts(n, false, room, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
        let all = text(200);
        assert!(all.contains("working") && all.contains("needs you"), "{all}");
        let numbers = text(30);
        assert!(!numbers.contains("working"), "{numbers}");
        assert!(numbers.contains("? 1") && numbers.contains("∿ 3"), "{numbers}");
        let two = text(9);
        assert_eq!(two, "∿ 3 · ? 1");
        assert_eq!(text(3), "? 1");
        assert_eq!(text(2), "");
    }

    /// The raised pane (book §13, BISE-102, BISE-212): the grey fills the
    /// inside of the frame under the divider, edge to edge between the
    /// side edges; the lines (the divider, the side and bottom edges) stay
    /// on the history's ground, outside the grey. 6 rows at rest (the
    /// divider, a blank bar row, 1 text row, a blank bar row, the key
    /// bar, the frame; BISE-219), 4 under 20 rows; under 14 rows the key bar takes the divider's right
    /// side.
    #[test]
    fn the_pane_under_the_divider_is_raised() {
        let mut app = with_main();
        let screen = |app: &mut App, w: u16, h: u16| {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
            term.backend().buffer().clone()
        };
        let row = |b: &ratatui::buffer::Buffer, y: u16| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>();
        for (h, pane) in [(40u16, 6u16), (30, 6), (24, 6), (20, 6), (19, 4), (16, 4)] {
            let b = screen(&mut app, 120, h);
            let div = (0..h).find(|&y| row(&b, y).starts_with("├─ you → main")).unwrap_or_else(|| panic!("{h}: no divider"));
            assert_eq!(h - div, pane, "{h} rows: the pane takes {pane}");
            // the history's ground, and the grey is another color
            let ground = b[(60, div - 1)].bg;
            assert_ne!(ground, raised(), "{h} rows");
            for y in div + 1..h - 1 {
                for x in 1..119 {
                    assert_eq!(b[(x, y)].bg, raised(), "{h} rows: ({x}, {y}) is raised");
                }
                // the side edges: on the ground, outside the grey
                assert_eq!((b[(0, y)].symbol(), b[(0, y)].bg), ("│", ground), "{h} rows: the left edge");
                assert_eq!((b[(119, y)].symbol(), b[(119, y)].bg), ("│", ground), "{h} rows: the right edge");
            }
            // the divider (its corners, labels and the panel's join) and
            // the bottom edge: on the ground too, like the history above
            for y in [div - 1, div, h - 1] {
                assert!((0..120).all(|x| b[(x, y)].bg == ground), "{h} rows: row {y} is on the ground: {:?}", row(&b, y));
            }
            assert!(row(&b, h - 1).starts_with('╰'), "{h} rows: {:?}", row(&b, h - 1));
            // the panel's rule stops at the divider: nothing under its join
            let join = row(&b, div).chars().position(|c| c == '┴').unwrap_or_else(|| panic!("{h}: no join"));
            assert!((div + 1..h - 1).all(|y| !["│", "┃"].contains(&b[(join as u16, y)].symbol())), "{h} rows: under the join");
            // the text keeps its colors on the tint: the bar a quiet line, the placeholder dim
            let t = (div + 1..h).find(|&y| row(&b, y).contains(PLACEHOLDER_MAIN)).unwrap();
            assert_eq!((b[(3, t)].symbol(), b[(3, t)].fg), ("│", crate::theme::rule()));
        }
        // bare (under 16 rows): the full width under the divider, down
        // to the last row
        let b = screen(&mut app, 120, 15);
        let div = (0..15).find(|&y| row(&b, y).contains("you → main")).unwrap();
        assert!((div + 1..15).all(|y| (0..120).all(|x| b[(x, y)].bg == raised())));
        assert!((0..120).all(|x| b[(x, div)].bg != raised()), "the divider stays on the ground");
        // under 14 rows the key bar is on the divider's right, no row of its own
        let b = screen(&mut app, 120, 13);
        let div = (0..13).find(|&y| row(&b, y).contains("you → main")).unwrap();
        assert!(row(&b, div).contains("@ file   $ skills"), "{:?}", row(&b, div));
        assert_eq!(13 - div, 2, "the divider and 1 text row");
    }

    #[test]
    fn header_at_60_and_120() {
        let mut app = busy();
        let rows = draw(&mut app, 120, 20);
        // framed (book §8 "The frame"): the title from column 3 in the
        // top edge, the path after it, the counts ending at F - 4
        let head = &rows[0];
        assert!(head.starts_with("╭─ bise :* · bench ─"), "{:?}", head);
        // BISE-303: the panel counts the agents, the header the inbox
        assert!(head.ends_with("─ # 1 in the inbox ─╮"), "{:?}", head);
        assert_eq!(head.chars().count(), 120, "{:?}", head);
        // ctrl held: every count
        app.hold = held_ctrl();
        let rows = draw(&mut app, 120, 20);
        let right = "∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done · # 1 in the inbox";
        assert!(rows[0].starts_with("╭─ bise :* · bench ─") && rows[0].ends_with(&format!("─ {} ─╮", right)), "{:?}", rows[0]);
        app.hold = crate::ctrlhint::Hold::default();
        assert!(!rows.iter().any(|r| r.contains("Switchboard")));
        // 60 columns, no panel: the short counts
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].starts_with("╭─ bise :* · bench ─") && rows[0].ends_with("─ ∿ 3 · … 1 · ? 1 · ✓ 1 · # 1 ─╮"), "{:?}", rows[0]);
        // under 60 columns: no frame, the header row with margins of 1
        let rows = draw(&mut app, 59, 20);
        let summary = "∿ 3 · … 1 · ? 1 · ✓ 1 · # 1";
        assert_eq!(rows[0], format!(" bise :* · bench{}{}", " ".repeat(59 - 16 - summary.chars().count() - 1), summary));
        // no agents: the words
        let mut app = with_main();
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :*") && rows[0].ends_with("no agents yet ─╮"), "{:?}", rows[0]);
        let rows = draw(&mut app, 59, 20);
        assert!(rows[0].ends_with("no agents yet"), "{:?}", rows[0]);
    }

    /// QA F: only idle agents, no count to show: the path after the logo,
    /// nothing on the right, no lone ` · `.
    #[test]
    fn idle_agents_leave_no_lone_separator_in_the_header() {
        let mut app = busy();
        for a in app.sb.agents.iter_mut().filter(|a| !a.main) {
            a.status = "idle".into();
        }
        app.sb.cards.clear();
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :* · bench ──") && rows[0].ends_with("───╮"), "{:?}", rows[0]);
        let rows = draw(&mut app, 59, 20);
        assert!(!rows[0].trim_end().ends_with('·'), "{:?}", rows[0]);
    }

    /// BISE-126: viewing a task, its role line follows the path, dim; it
    /// keeps ROLE_KEEP columns, then ROLE_MIN before the path goes; too
    /// narrow, it is cut, then it goes; main's view shows none.
    #[test]
    fn the_viewed_task_s_role_line_follows_the_title() {
        let mut app = busy();
        for a in app.sb.agents.iter_mut().filter(|a| a.name == "auth-fix") {
            a.role = "fixing the safari login redirect".into();
        }
        // ctrl held: the header's every count (BISE-303)
        app.hold = held_ctrl();
        let rows = draw(&mut app, 120, 20);
        assert!(!rows[0].contains("safari"), "main's view: {:?}", rows[0]);
        app.sb.focus = "auth-fix".into();
        let rows = draw(&mut app, 136, 20);
        let full = "∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done · # 1 in the inbox";
        assert!(rows[0].starts_with("╭─ bise :* · bench · fixing the safari login redirect ─"), "{:?}", rows[0]);
        assert!(rows[0].ends_with(&format!(" {} ─╮", full)), "{:?}", rows[0]);
        assert_eq!(rows[0].chars().count(), 136);
        // shorter: the path goes before the line is cut under ROLE_KEEP
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :* · fixing the safari login redir… ─"), "{:?}", rows[0]);
        assert!(rows[0].ends_with(&format!(" {} ─╮", full)), "{:?}", rows[0]);
        // dim, like the summary
        let line = app.sb.header(200, false, true, &super::still_gust());
        let role = line.spans.iter().find(|s| s.content.contains("safari")).unwrap();
        assert_eq!(role.style.fg, Some(crate::theme::dim()));
        // wide: the whole line
        let rows = draw(&mut app, 160, 20);
        assert!(rows[0].starts_with("╭─ bise :* · bench · fixing the safari login redirect ─"), "{:?}", rows[0]);
        // narrow: the counts shorten, the path goes, the line is cut with …
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].starts_with("╭─ bise :* · fixing"), "{:?}", rows[0]);
        assert!(rows[0].contains('…') && rows[0].ends_with("∿ 3 · … 1 · ? 1 · ✓ 1 · # 1 ─╮"), "{:?}", rows[0]);
        assert_eq!(rows[0].chars().count(), 60);
        // no frame: the header row, the same order
        let rows = draw(&mut app, 59, 20);
        assert!(rows[0].starts_with(" bise :* · bench · fixing"), "{:?}", rows[0]);
        assert!(rows[0].ends_with("∿ 3 · … 1 · ? 1 · ✓ 1 · # 1"), "{:?}", rows[0]);
        // no room left: no line at all, never a lone "·"
        let rows = draw(&mut app, 50, 20);
        assert!(rows[0].starts_with(" bise :* · fixing t…  ∿ 3"), "at least 12 columns: {:?}", rows[0]);
        let rows = draw(&mut app, 46, 20);
        assert!(!rows[0].contains("fix") && !rows[0].contains(" · f"), "{:?}", rows[0]);
        // a task without a line yet (an old hub): nothing
        for a in app.sb.agents.iter_mut() {
            a.role.clear();
        }
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :* · bench ─"), "{:?}", rows[0]);
    }

    /// The summary (the counts) shortens its words.
    #[test]
    fn summary_shortens_its_words() {
        let app = busy();
        let sb = &app.sb;
        let text = |room: usize, short: bool| sb.summary(room, short, true, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
        let full = "∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done · # 1 in the inbox";
        assert_eq!(text(100, false), full);
        assert_eq!(text(full.chars().count(), false), full);
        assert_eq!(text(full.chars().count() - 1, false), "∿ 3 · … 1 · ? 1 · ✓ 1 · # 1");
    }

    /// Colors of the header: `:*` and "needs you" in accent, the rest dim.
    #[test]
    fn header_colors() {
        let app = busy();
        let line = app.sb.header(120, false, true, &super::still_gust());
        let color_of = |t: &str| line.spans.iter().find(|s| s.content.contains(t)).map(|s| s.style.fg);
        assert_eq!(color_of(":*"), Some(Some(accent())));
        assert_eq!(color_of("needs you"), Some(Some(accent())));
        assert_eq!(color_of("working"), Some(Some(dim())));
    }

    /// First run: the header says `no agents yet`, the feed the three
    /// dim lines of the copy deck, the status row `main · idle`, the
    /// composer `›` with the main hints (mockup "first run").
    #[test]
    fn first_run_screen() {
        let mut app = with_main();
        let rows = draw(&mut app, 120, 24);
        let all = rows.join("\n");
        for l in FIRST_RUN {
            assert!(rows.iter().any(|r| r.contains(l)), "{:?} missing:\n{}", l, all);
        }
        assert_eq!(FIRST_RUN[0], "what's on your mind?");
        assert_eq!(FIRST_RUN[1], "say it and keep talking. the work runs in the background, i'm always here.");
        assert_eq!(FIRST_RUN[2], "try: \"show me what you can do\"");
        // the composer pane (book §8 "The frame"): the divider says who
        // you talk to and what it does
        let at = rows.iter().position(|r| r.starts_with("├─ you → main ─")).unwrap_or_else(|| panic!("{}", all));
        // BISE-303: no state word at rest, no context yet
        assert!(rows[at].ends_with("──┤") && !rows[at].contains("idle"), "{:?}", rows[at]);
        assert!(rows[at].contains('┴'), "the panel's rule joins it: {:?}", rows[at]);
        // then the raised pane (book §13): the composer, its bar at x0
        // (column 3 here) on a blank row, the text row, a blank row
        // (BISE-219), then the key bar from the text's column, the
        // frame's bottom edge
        assert_eq!(rows.len() - at, 6, "6 rows at rest: {}", all);
        assert!(rows[at + 1..at + 4].iter().all(|r| r.starts_with("│  │")), "{}", all);
        assert_eq!(rows[at + 1].trim_end_matches(['│', ' ']), "", "{:?}", rows[at + 1]);
        assert_eq!(rows[at + 3].trim_end_matches(['│', ' ']), "", "{:?}", rows[at + 3]);
        // empty, the composer asks; the text at x0 + 4 (BISE-XPAD)
        assert!(rows[at + 2].starts_with(&format!("│  │     {}", PLACEHOLDER_MAIN)), "{:?}", rows[at + 2]);
        let keys = &rows[rows.len() - 2];
        assert!(keys.starts_with("│      @ file   $ skills   / commands"), "{:?}", keys);
        assert!(rows.last().unwrap().starts_with("╰─"), "{}", all);
        // a panel with main only
        assert!(rows.iter().any(|r| r.contains(&format!("│  0 {} main {}", G_IDLE, G_MAIN))), "{}", all);
        // the block sits at 2/5 of the history's free rows (designer):
        // blank rows above it, more below it; the text stays left, at
        // the feed's indent
        let big = draw(&mut app, 120, 48);
        let at48 = big.iter().position(|r| r.starts_with("├─ you → main ─")).unwrap();
        let top = big.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        let bottom = big.iter().position(|r| r.contains(FIRST_RUN[2])).unwrap();
        let (above, below) = (top - 1, at48 - bottom - 1);
        assert!(above >= 8 && below > above && below - above <= above, "{above} above, {below} below:\n{}", big.join("\n"));
        let t = rows.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        assert_eq!(big[top].find(FIRST_RUN[0]), rows[t].find(FIRST_RUN[0]), "same column");
        assert!(t > 4, "centered at 24 rows too: {}", all);
        // a short history (under 12 rows): on top, after one blank row
        let small = draw(&mut app, 120, 16);
        let t = small.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        assert!(t <= 3 && small[t - 1].chars().take(60).all(|c| c == '│' || c == ' '), "{}", small.join("\n"));
        // once there is an agent, the first-run text goes
        bench::add_agent(&mut app, "auth-fix", "the safari login");
        let rows = draw(&mut app, 120, 24);
        assert!(!rows.iter().any(|r| r.contains(FIRST_RUN[1])));
    }

    /// The styled cells of a draw (the underline, the colors).
    fn cells(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        term.backend().buffer().clone()
    }

    fn mouse(app: &mut App, kind: crossterm::event::MouseEventKind, column: u16, row: u16) {
        let m = crossterm::event::MouseEvent { kind, column, row, modifiers: crossterm::event::KeyModifiers::NONE };
        crate::input::on_mouse(app, &m, 0);
    }

    fn type_key(app: &mut App, code: crossterm::event::KeyCode) {
        crate::input::on_key(app, &crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE));
    }

    /// BISE-284: the first-run text's `show me what you can do` is a
    /// link: the hand and an accent underline under the mouse; a click
    /// fills the composer with it, selected, and sends nothing; `try:`
    /// is no link.
    #[test]
    fn a_click_on_the_suggestion_fills_the_composer() {
        use crossterm::event::{MouseButton, MouseEventKind};
        use crate::pointer::{at, Shape};
        let mut app = with_main();
        assert_eq!(FIRST_RUN[2], format!("try: \"{DEMO}\""));
        let rows = draw(&mut app, 120, 24);
        let y = rows.iter().position(|r| r.contains(FIRST_RUN[2])).unwrap() as u16;
        let row = &rows[y as usize];
        let x = row[..row.find(DEMO).unwrap()].chars().count() as u16;
        let r = app.demo_rect.expect("the suggestion's place");
        assert_eq!((r.x, r.y, r.width, r.height), (x, y, DEMO.len() as u16, 1));
        assert_eq!(at(x, y), Shape::Pointer);
        assert_eq!(at(x + DEMO.len() as u16 - 1, y), Shape::Pointer);
        assert_eq!(at(x - 3, y), Shape::Default, "`try:` is no link");
        assert_eq!(at(x + DEMO.len() as u16, y), Shape::Default, "the closing quote is no link");
        // at rest: dim, no underline; under the mouse: accent, underlined
        let b = cells(&mut app, 120, 24);
        assert_eq!(b[(x, y)].fg, crate::theme::dim());
        assert!(!b[(x, y)].modifier.contains(Modifier::UNDERLINED));
        mouse(&mut app, MouseEventKind::Moved, x + 2, y);
        let b = cells(&mut app, 120, 24);
        assert_eq!(b[(x, y)].fg, crate::theme::accent());
        assert!(b[(x, y)].modifier.contains(Modifier::UNDERLINED));
        assert!(!b[(x - 3, y)].modifier.contains(Modifier::UNDERLINED), "only the sentence");
        // a click on `try:`: nothing
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x - 4, y);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x - 4, y);
        assert_eq!(app.ed.text, "");
        // a click on the sentence: the composer holds it, all selected
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 5, y);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x + 5, y);
        assert_eq!(app.ed.text, DEMO);
        assert_eq!(app.ed.selection(), Some((0, DEMO.chars().count())));
        assert!(!app.events.iter().any(|e| matches!(e, crate::Ev::You(..))), "not sent");
        // the last line now says what enter does
        let rows = draw(&mut app, 120, 24);
        let ready = format!("{}{}", DEMO_READY.0, DEMO_READY.1);
        assert_eq!(ready, "⏎ try it · or just type your own");
        assert!(rows.iter().any(|r| r.contains(&ready)), "{}", rows.join("\n"));
        assert!(!rows.iter().any(|r| r.contains(FIRST_RUN[2])));
        assert!(app.demo_rect.is_none(), "no link while it says ⏎ try it");
    }

    /// BISE-284: the first open after the onboarding: the composer holds
    /// the suggestion, selected, and the first-run text says `⏎ try it`;
    /// a typed key replaces it and the text says `try:` again; esc keeps
    /// it (unselected). Not in an agent, not over a draft, not once
    /// there are agents.
    #[test]
    fn the_first_open_after_the_onboarding_holds_the_suggestion() {
        use crossterm::event::KeyCode;
        let mut app = with_main();
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, DEMO);
        assert_eq!(app.ed.selection(), Some((0, DEMO.chars().count())));
        let rows = draw(&mut app, 120, 24);
        let all = rows.join("\n");
        let at = rows.iter().position(|r| r.contains("⏎ try it · or just type your own")).unwrap_or_else(|| panic!("{}", all));
        assert!(rows[..at].iter().any(|r| r.contains(FIRST_RUN[0])), "{}", all);
        assert!(rows.iter().any(|r| r.starts_with("│  │") && r.contains(DEMO)), "the composer holds it:\n{}", all);
        let b = cells(&mut app, 120, 24);
        let x = rows[at].find('⏎').map(|i| rows[at][..i].chars().count() as u16).unwrap();
        assert_eq!(b[(x, at as u16)].fg, crate::theme::accent(), "the key in the accent");
        assert_eq!(b[(x + 3, at as u16)].fg, crate::theme::dim());
        // esc: kept, not selected
        type_key(&mut app, KeyCode::Esc);
        assert_eq!((app.ed.text.as_str(), app.ed.selection()), (DEMO, None));
        // selected again, a key replaces it: back to `try:`
        fill_demo(&mut app);
        type_key(&mut app, KeyCode::Char('h'));
        assert_eq!(app.ed.text, "h");
        let rows = draw(&mut app, 120, 24);
        assert!(rows.iter().any(|r| r.contains(FIRST_RUN[2])), "{}", rows.join("\n"));
        // a draft stays as it is
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "h");
        // not inside an agent, not once there are agents
        let mut app = with_main();
        bench::add_agent(&mut app, "auth-fix", "the safari login");
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "");
        app.sb.focus = "auth-fix".into();
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "");
    }

    /// Inside an agent (mockup "inside an agent"): the pinned line of the
    /// copy deck on top of the feed, the status row starts with the
    /// agent's name in accent, then dim; the shared checkout shows no
    /// place (BISE-136).
    #[test]
    fn inside_an_agent_screen() {
        let mut app = with_main();
        app.sb.agents.push(Agent {
            name: "auth-fix".into(),
            status: "idle".into(),
            mode: "shared".into(),
            path: "/ws".into(),
            ..Agent::default()
        });
        focus(&mut app, "auth-fix");
        let mut term = Terminal::new(TestBackend::new(112, 24)).unwrap();
        term.draw(|f| super::super::draw_sb(&mut app, f)).unwrap();
        let buf = term.backend().buffer().clone();
        let rows: Vec<String> = buf
            .content
            .chunks(112)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect();
        let all = rows.join("\n");
        let line = "you're talking to auth-fix directly. main isn't in the loop. esc back to main.";
        // the header row, 1 blank row, then the pinned line (book §8),
        // wrapped in the column when it is narrower
        let left = |r: &String| r.chars().skip(1).take(76).collect::<String>().trim().to_string();
        assert!(format!("{} {}", left(&rows[2]), left(&rows[3])).trim().contains(line), "{}", all);
        // the divider: `you →` dim, the name in accent, the state dim
        let y = rows.iter().position(|r| r.starts_with("├─ you → auth-fix ─")).unwrap_or_else(|| panic!("{}", all));
        assert!(!rows[y].contains("idle"), "{:?}", rows[y]);
        let cell = |x: usize| buf.cell((x as u16, y as u16)).unwrap().fg;
        assert_eq!(cell(3), dim(), "you → dim");
        assert_eq!(cell(9), accent(), "the name in accent");
        assert!(!all.contains("task"), "no \"task\" in the chrome:\n{}", all);
    }

    /// A click lands on the feed row drawn under it: the feed starts
    /// under the header (and, inside an agent, under the pinned line).
    #[test]
    fn feed_clicks_land_on_the_row_under_the_header() {
        for inside in [false, true] {
            let mut app = with_main();
            if inside {
                bench::add_agent(&mut app, "auth-fix", "the safari login");
                focus(&mut app, "auth-fix");
            }
            for k in 0..5 {
                push_event(&mut app.events, &mut app.cache, Ev::Info(format!("event {}", k)));
            }
            let rows = draw(&mut app, 100, 24);
            assert!(app.feed_y >= 1, "the header is above the feed");
            if inside {
                assert!(rows[2].contains("you're talking to auth-fix"));
                // under the pinned line (1 or 2 rows) and a blank row
                assert!(app.feed_y >= 4, "{}", app.feed_y);
            }
            for k in 0..5 {
                let label = format!("event {}", k);
                let y = rows.iter().position(|r| r.contains(&label)).unwrap_or_else(|| panic!("{}", rows.join("\n")));
                let x = rows[y].find(&label).unwrap() as u16;
                let pos = crate::input::feed_pos(&app, x, y as u16, false).expect("a feed row");
                assert_eq!(pos.0, k, "inside {}: {} at screen row {}", inside, label, y);
            }
            // the header row is not the feed
            assert!(crate::input::feed_pos(&app, 5, 0, false).is_none());
        }
    }

    /// The status row: lowercase, the context, a preview note, the
    /// steer hints during a turn.
    #[test]
    fn status_row_and_hints() {
        let mut app = busy();
        // BISE-303: the glyph says idle; ctrl held, the word
        assert_eq!(status_text(&app), "main");
        app.hold = held_ctrl();
        assert_eq!(status_text(&app), "main · idle");
        app.hold = crate::ctrlhint::Hold::default();
        assert_eq!(key_mode(&app), crate::keybar::Mode::Default);
        app.pending = true;
        assert_eq!(key_mode(&app), crate::keybar::Mode::Steer);
        app.pending = false;
        let sb = &mut app.sb;
        sb.selected = Some(1);
        sb.preview = true;
        let text = status_text(&app);
        assert!(text.ends_with("preview of auth-fix"), "{:?}", text);
    }

    /// QA N: viewing a working agent, the label says `working · 12m`; the
    /// right side does not say `working · 0s` again. Another state keeps
    /// its word and the turn's age.
    #[test]
    fn a_working_agent_has_one_status_and_one_timer() {
        let mut app = busy();
        app.sb.agents.retain(|a| a.name != "auth-fix");
        app.sb.agents.push(Agent {
            name: "auth-fix".into(),
            status: "working".into(),
            turn_ms: Some(12 * 60_000),
            ..Agent::default()
        });
        app.sb.focus = "auth-fix".into();
        app.pending = true;
        assert_eq!(viewed_working(&app).and_then(|w| w.age).as_deref(), Some("12m"));
        let state: String = status_state(&app)[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains("working") && !state.contains("12m") && !state.contains("0s"), "{state:?}");
        for a in app.sb.agents.iter_mut().filter(|a| a.name == "auth-fix") {
            a.status = "blocked".into();
        }
        let state: String = status_state(&app)[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains("blocked"), "at rest the glyph says it: {state:?}");
        app.hold = held_ctrl();
        let state: String = status_state(&app)[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(state.starts_with("blocked · 12m"), "{state:?}");
    }
}

#[cfg(test)]
mod cards_tests {
    //! BISE-125: the open cards in the panel, under the agents.
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn card(id: u64, kind: &str, agent: &str, text: &str) -> Card {
        Card { id, kind: kind.into(), agent: agent.into(), text: text.into(), ..Card::default() }
    }

    /// main, two agents, three open cards (ids 12, 153, 40).
    fn app() -> App {
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        sb.agents.push(Agent { name: "debt-solo".into(), status: "done".into(), ..Agent::default() });
        sb.agents.push(Agent { name: "docs".into(), status: "working".into(), ..Agent::default() });
        sb.cards.push(card(12, "question", "docs", "\nv1 or v2?\n1. v1\n2. v2"));
        sb.cards.push(card(153, "done", "debt-solo", "the debt list is cleared, 14 items closed and two left for later"));
        sb.cards.push(card(40, "blocked", "docs", "no access to the wiki"));
        app
    }

    fn rows(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_panel(app, f, f.area())).unwrap();
        let buf = term.backend().buffer();
        buf.content
            .chunks(w as usize)
            .map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect()
    }

    /// Under the agents: a blank row, `inbox`, then one row per card in
    /// the strip's order with its number (BISE-302: ctrl+N opens the row
    /// that says N), `N glyph agent  first line`; the text cut to the
    /// panel with `…`, never past its width.
    #[test]
    fn the_open_cards_list_under_the_agents() {
        let app = app();
        let t = rows(&app, 40, 14);
        let at = |s: &str| t.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} missing:\n{}", t.join("\n")));
        let title = t.iter().position(|r| r == " inbox").unwrap_or_else(|| panic!("{}", t.join("\n")));
        assert!(at("docs") < title, "{}", t.join("\n"));
        assert_eq!(t[title - 1], "");
        let first = title + 1;
        // the first line of the text, not the blank one before it
        assert_eq!(t[first], format!(" 1 {} docs  v1 or v2?", G_NEEDS_YOU));
        assert_eq!(t[first + 1], format!(" 2 {} docs  no access to the wiki", G_NEEDS_YOU));
        assert_eq!(t[first + 2], format!(" 3 {} debt-solo  the debt list is clear…", crate::theme::done_glyph()));
        // 28 columns: still one row each, cut at the width
        for w in [28u16, 24] {
            let t = rows(&app, w, 14);
            let r = t.iter().find(|r| r.starts_with(" 3 ")).unwrap_or_else(|| panic!("{}", t.join("\n")));
            assert!(r.chars().count() < w as usize, "{r:?} at {w}");
            assert!(r.contains("debt-solo") && r.ends_with('…'), "{r:?} at {w}");
        }
        // a long agent name is cut too, a few columns of text kept
        let mut app = app;
        app.sb.cards.push(card(200, "question", "a-very-long-agent-name-indeed", "which one?"));
        let t = rows(&app, 28, 14);
        let r = t.iter().find(|r| r.contains(" 2 ? a-very")).unwrap_or_else(|| panic!("{}", t.join("\n")));
        assert!(r.contains("…  wh"), "{r:?}");
        assert!(r.chars().count() < 28);
        // colors: the number dim, the glyph in its kind's color, the text dim
        let mut term = Terminal::new(TestBackend::new(40, 14)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let t = rows(&app, 40, 14);
        let y = t.iter().position(|r| r.contains("no access")).unwrap() as u16;
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((1, y)).unwrap().fg, dim());
        assert_eq!(buf.cell((3, y)).unwrap().fg, accent(), "blocked: needs you, accent");
        assert_eq!(buf.cell((5, y)).unwrap().fg, text());
        assert_eq!(buf.cell((11, y)).unwrap().fg, dim());
        // no card: no section
        app.sb.cards.clear();
        assert!(!rows(&app, 40, 14).iter().any(|r| r == " inbox"));
    }

    /// Many cards: the panel ends with `+ n more`; the card in the box
    /// is on the selection color and scrolled into view.
    #[test]
    fn many_cards_end_with_more_and_the_shown_one_stays_in_view() {
        let mut app = app();
        for i in 0..30 {
            app.sb.cards.push(card(1000 + i, "done", "debt-solo", &format!("report {i}")));
        }
        let t = rows(&app, 28, 14);
        assert!(t[13].starts_with(" ↓ ") && t[13].ends_with(" more"), "{}", t.join("\n"));
        // 33 cards; title, blank, 3 agents, blank, cards title, 6 cards: 27 below
        assert_eq!(t[13], " ↓ 27 more", "{}", t.join("\n"));
        assert!(t.iter().any(|r| r.starts_with(" 1 ? docs  v1")), "the strip's order: {}", t.join("\n"));
        // a card far down shown in the box: the panel scrolls to it
        super::super::cards::open_view(&mut app, Some(1029));
        let t = rows(&app, 28, 14);
        assert!(t.iter().any(|r| r.contains("report 29")), "{}", t.join("\n"));
        let mut term = Terminal::new(TestBackend::new(28, 14)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let y = t.iter().position(|r| r.contains("report 29")).unwrap() as u16;
        assert_eq!(term.backend().buffer().cell((20, y)).unwrap().bg, selection_bg());
    }

    fn click(app: &mut App, column: u16, row: u16) -> bool {
        let m = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE };
        panel_mouse(app, &m)
    }

    /// A click on a card row opens the card view on it (cards v2); on
    /// another card the view follows; the section title toggles the view.
    #[test]
    fn a_click_on_a_card_opens_it() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let mut draw = |app: &mut App| {
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
            let buf = term.backend().buffer();
            buf.content.chunks(120).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>()
        };
        let screen = draw(&mut app);
        let x = app.sb.panel_hits.borrow().area.x;
        let y_of = |s: &[String], l: &str| s.iter().position(|r| r.chars().skip(x as usize).collect::<String>().contains(l)).unwrap_or_else(|| panic!("{l}:\n{}", s.join("\n"))) as u16;
        assert!(!app.sb.card.open);
        assert!(click(&mut app, x + 3, y_of(&screen, "no access")));
        assert!(app.sb.card.open);
        assert_eq!(app.sb.current_card().map(|c| c.id), Some(40));
        let screen = draw(&mut app);
        assert!(screen.iter().any(|r| r.contains("docs is blocked")), "the view shows #40:\n{}", screen.join("\n"));
        // another card: the view follows
        click(&mut app, x + 3, y_of(&screen, "3 ✓ debt-solo"));
        assert_eq!((app.sb.card.open, app.sb.current_card().map(|c| c.id)), (true, Some(153)));
        // the section title: back to the thread, then the view again
        let screen = draw(&mut app);
        click(&mut app, x + 3, y_of(&screen, " inbox  "));
        assert!(!app.sb.card.open);
        let screen = draw(&mut app);
        click(&mut app, x + 3, y_of(&screen, " inbox  "));
        assert!(app.sb.card.open);
        assert_eq!(app.sb.focus, "main", "a card click does not change the view");
    }

    /// The header counts the open cards, last (`# 3 in the inbox`), and keeps
    /// them when the panel is hidden (`# 3`), right after needs you.
    #[test]
    fn the_header_counts_the_cards() {
        let app = app();
        let text = |room: usize, short: bool| app.sb.summary(room, short, true, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
        let t = text(200, false);
        assert!(t.ends_with(&format!("{} 1 done · # 3 in the inbox", crate::theme::done_glyph())), "{t:?}");
        assert!(text(200, true).ends_with("· # 3"), "{:?}", text(200, true));
        // short on room: needs you, then the cards, before the rest
        assert_eq!(text(14, true), format!("{} 1 · # 3", G_NEEDS_YOU));
        let one = [0, 0, 0, 0, 0, 1];
        let s: String = fit_counts(one, false, 100, &[]).iter().map(|s| s.content.to_string()).collect();
        assert_eq!(s, "# 1 in the inbox");
    }
}
