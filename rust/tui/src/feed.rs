//! The feed layout cache: events wrap to rows once (per width, per
//! mutation) and frames only clone the visible slice; plus the event
//! merge rules (push_event) and the scroll anchor arithmetic.

use crate::render::*;
use crate::wire::*;
use crate::feedsel;
use bise_proto::thread::lines;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// a replayed tool has no meaningful duration (the timing is the replay's)
#[cfg(test)]
pub(crate) fn hide_replayed_elapsed(events: &mut [Ev], cache: &mut [Option<EventRows>], id: u32) {
    if let Some((i, td)) = last_tool_mut(events, |td| td.id == id) {
        td.elapsed = Some(String::new());
        cache[i] = None;
    }
}

/// The newest tool event that matches, with its index.
fn last_tool_mut(events: &mut [Ev], pred: impl Fn(&ToolData) -> bool) -> Option<(usize, &mut ToolData)> {
    events.iter_mut().enumerate().rev().find_map(|(i, e)| match e {
        Ev::Tool(td) if pred(td) => Some((i, td)),
        _ => None,
    })
}

// ---- the codex-style layout cache ----
// Events render to wrapped rows ONCE (per width / per mutation); every
// frame only the visible slice is cloned into the paragraph. A running
// tool re-renders only its tool line each frame (spinner, elapsed): its
// body (a source block can be thousands of rows) stays cached.

pub(crate) struct EventRows {
    pub(crate) width: u16,
    /// built for main's feed (render::main_feed)
    pub(crate) main: bool,
    pub(crate) rows: Vec<Line<'static>>,
    /// A running tool: where its tool line sits in `rows`, and what it
    /// needs to be redrawn alone.
    pub(crate) live: Option<LiveHead>,
    /// the urls of its links, in order (links.rs: a link's tag)
    pub(crate) urls: Vec<String>,
    /// its markdown code blocks (codeblock.rs: the copy icon)
    pub(crate) blocks: Vec<crate::codeblock::Block>,
}

pub(crate) struct LiveHead {
    pub(crate) at: usize,
    pub(crate) len: usize,
    pub(crate) what: Live,
}

/// What a live row redraws each frame.
pub(crate) enum Live {
    /// a running tool's line (its pulse, its elapsed)
    Tool { name: String, args: String },
    /// a running call's row (BISE-223: its pulse, its elapsed)
    Row,
    /// the fold of the last run of level-3 lines (its pulse)
    Fold { n: usize, agents: usize, open: bool },
    /// a compaction still running (its `≡` pulses, BISE-90)
    Compacting,
}

pub(crate) fn event_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> EventRows {
    let ((mut er, urls), found) =
        crate::codeblock::collect(|| crate::links::collect(|| event_rows_of(events, i, debug, width, tick)));
    er.urls = urls;
    er.blocks = crate::codeblock::locate(&er.rows, found);
    er
}

fn event_rows_of(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> EventRows {
    // a run of ↗ lines aligns its kind column (designer, m_7220)
    if matches!(events[i], Ev::Made { .. }) {
        crate::render::set_made_pad(made_run_pad(events, i));
    }
    if is_l3(&events[i]) && ev_visible(&events[i], debug) {
        let (rows, live) = l3_rows(events, i, debug, width, tick);
        return EventRows { width: width as u16, main: main_feed(), rows, live, urls: Vec::new(), blocks: Vec::new() };
    }
    // a compaction runs until its summary arrives
    if matches!(events[i], Ev::Compact) && !events[i + 1..].iter().any(|e| matches!(e, Ev::Compacted { .. })) {
        let mut rows = build_rows(events, i, debug, width, tick);
        let at = rows.len().saturating_sub(1);
        rows.truncate(at);
        rows.extend(compacting_line(tick, true));
        return EventRows {
            width: width as u16,
            main: main_feed(),
            rows,
            live: Some(LiveHead { at, len: 1, what: Live::Compacting }),
            urls: Vec::new(), blocks: Vec::new(),
        };
    }
    let running = match &events[i] {
        Ev::Tool(td) if matches!(td.state, ToolState::Run) && ev_visible(&events[i], debug) => Some(td),
        _ => None,
    };
    let Some(td) = running else {
        return EventRows {
            width: width as u16,
            main: main_feed(),
            rows: build_rows(events, i, debug, width, tick),
            live: None,
            urls: Vec::new(), blocks: Vec::new(),
        };
    };
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    let mut rows: Vec<Line<'static>> = Vec::new();
    if wants_gap_before(&events[i], prev) {
        rows.push(Line::from(""));
    }
    let (name, args, code) = tool_meta(td);
    // a tool is code: its rows follow the code measure
    let cw = code_width(width);
    let at = rows.len();
    if crate::toolrow::row_mode(td) {
        // BISE-223: a running call is one row (pulse, time)
        rows.push(crate::toolrow::row_line(td, tick, cw));
        return EventRows {
            width: width as u16,
            main: main_feed(),
            rows,
            live: Some(LiveHead { at, len: 1, what: Live::Row }),
            urls: Vec::new(), blocks: Vec::new(),
        };
    }
    if crate::toolbox::opens_as_box(td) {
        // a running box redraws its top border only (title, pulse, time)
        rows.extend(crate::toolbox::box_lines(td, &code, &subs_of(events, i), tick, cw));
        return EventRows {
            width: width as u16,
            main: main_feed(),
            rows,
            live: Some(LiveHead { at, len: 1, what: Live::Tool { name, args } }),
            urls: Vec::new(), blocks: Vec::new(),
        };
    }
    rows.extend(wrap_line(tool_head(td, tick, &name, &args), cw));
    let len = rows.len() - at;
    for l in tool_body(td, &code, cw) {
        rows.extend(wrap_line(l, cw));
    }
    EventRows {
        width: width as u16,
        main: main_feed(),
        rows,
        live: Some(LiveHead { at, len, what: Live::Tool { name, args } }),
        urls: Vec::new(), blocks: Vec::new(),
    }
}

/// Redraw the live row of an event (a running tool's line, the pulse of
/// the last fold), keep the rest.
pub(crate) fn refresh_live(er: &mut EventRows, ev: &Ev, tick: u32) {
    let Some(lh) = er.live.as_mut() else { return };
    let head = match (&lh.what, ev) {
        (Live::Row, Ev::Tool(td)) => vec![crate::toolrow::row_line(td, tick, code_width(er.width as usize))],
        (Live::Tool { .. }, Ev::Tool(td)) if crate::toolbox::opens_as_box(td) => {
            vec![crate::toolbox::box_top(td, tick, code_width(er.width as usize))]
        }
        (Live::Tool { name, args }, Ev::Tool(td)) => {
            wrap_line(tool_head(td, tick, name, args), code_width(er.width as usize))
        }
        (Live::Fold { n, agents, open }, _) => vec![fold_line(*n, *agents, *open, true, tick, code_width(er.width as usize))],
        (Live::Compacting, _) => compacting_line(tick, true),
        _ => return,
    };
    let n = head.len();
    er.rows.splice(lh.at..lh.at + lh.len, head);
    lh.len = n;
}

pub(crate) fn line_from(cells: Vec<(char, Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cur_style: Option<Style> = None;
    let mut buf = String::new();
    for (c, st) in cells {
        match cur_style {
            Some(s) if s == st => buf.push(c),
            _ => {
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), cur_style.unwrap()));
                }
                cur_style = Some(st);
                buf.push(c);
            }
        }
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur_style.unwrap()));
    }
    Line::from(spans)
}

/// The display width of each char of `chars`, by grapheme: the first
/// char of a grapheme carries its width (at least 1), the rest 0, so an
/// emoji ZWJ sequence or a char with a variation selector counts as
/// ratatui draws it, and a row never breaks inside one.
pub(crate) fn cell_widths(chars: impl Iterator<Item = char>) -> Vec<usize> {
    let s: String = chars.collect();
    let mut out = Vec::with_capacity(s.len());
    for g in s.graphemes(true) {
        let mut first = true;
        for _ in g.chars() {
            out.push(if first { g.width().max(1) } else { 0 });
            first = false;
        }
    }
    out
}

// span-aware greedy word wrap; words wider than the row hard-split
pub(crate) fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    // a row that is already the continuation of a wrapped line stays one
    // (a second wrap, wider, never loses the soft mark the copy joins on)
    let soft = feedsel::is_soft(&line);
    let mut cells: Vec<(char, Style)> = Vec::new();
    for sp in line.spans {
        for c in sp.content.chars() {
            cells.push((c, sp.style));
        }
    }
    if cells.is_empty() {
        return vec![Line::from("")];
    }
    let widths = cell_widths(cells.iter().map(|c| c.0));
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut row_w = 0usize;
    let mut i = 0usize;
    while i < cells.len() {
        // the next word (non-space run)
        let mut word: Vec<(char, Style)> = Vec::new();
        let mut word_w = 0usize;
        while i < cells.len() && cells[i].0 != ' ' {
            word_w += widths[i];
            word.push(cells[i]);
            i += 1;
        }
        // wrap before the word if it does not fit
        if row_w > 0 && row_w + word_w > width {
            rows.push(line_from(std::mem::take(&mut row)));
            row_w = 0;
        }
        // hard-split words wider than a full row
        if row_w == 0 && word_w > width {
            let mut chunk: Vec<(char, Style)> = Vec::new();
            let mut cw = 0usize;
            let word_start = i - word.len();
            for (k, (c, st)) in word.into_iter().enumerate() {
                let cc = widths[word_start + k];
                if cc > 0 && cw > 0 && cw + cc > width {
                    rows.push(line_from(std::mem::take(&mut chunk)));
                    cw = 0;
                }
                chunk.push((c, st));
                cw += cc;
            }
            row = chunk;
            row_w = cw;
        } else {
            row.extend(word);
            row_w += word_w;
        }
        // the spaces that follow the word stay on the row
        while i < cells.len() && cells[i].0 == ' ' {
            row.push(cells[i]);
            row_w += 1;
            i += 1;
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(line_from(row));
    }
    // the rows after the first continue the line (the copy joins them)
    for r in rows.iter_mut().skip(usize::from(!soft)) {
        feedsel::mark_soft(r);
    }
    rows
}

// the rows of one event: an optional breathing gap, then the wrapped
// lines.
pub(crate) fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> Vec<Line<'static>> {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    if !ev_visible(ev, debug) {
        return rows;
    }
    if is_l3(ev) {
        return l3_rows(events, i, debug, width, tick).0;
    }
    // the previous VISIBLE event decides the gap: a debug-only
    // annotation between two blocks must not swallow the blank line
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    // a sub-call of a box is drawn inside it (book §11)
    // (computer use: an action has its own row, `↖ clicked …`, design §8)
    let own_row = matches!(ev, Ev::Sub { name, preview, .. } if crate::computer_use::sub_row(name, preview).is_some());
    if matches!(ev, Ev::Sub { .. }) && !own_row && box_owner(events, i).is_some() {
        return rows;
    }
    // BISE-223: a done call (and the thinking around it) under a closed
    // `▸ n commands` fold takes no row
    if folded_away(events, i, debug) {
        return rows;
    }
    if wants_gap_before(ev, prev) {
        rows.push(Line::from(""));
    }
    match ev {
        Ev::Tool(td) if crate::toolbox::opens_as_box(td) => {
            let cw = code_width(width);
            if let Some(f) = tool_fold(events, i, debug).filter(|f| f.carrier == i) {
                rows.push(crate::toolrow::fold_row(td, f.n, f.total, f.open, cw));
                if !f.open {
                    return rows;
                }
            }
            if crate::toolrow::row_mode(td) {
                rows.extend(crate::toolrow::rows(td, tick, cw));
            } else {
                let (_, _, code) = tool_meta(td);
                rows.extend(crate::toolbox::box_lines(td, &code, &subs_of(events, i), tick, cw));
            }
        }
        // BISE-304: 3 done edits or more in a run fold into `± ▸ 3 files`
        Ev::Tool(td) if is_edit(td) => {
            if let Some(f) = tool_fold(events, i, debug).filter(|f| f.carrier == i) {
                let files = crate::toolrow::edit_files(&fold_patches(events, &f));
                rows.push(crate::toolrow::edit_fold_row(&files, f.n, f.open, code_width(width)));
                if !f.open {
                    return rows;
                }
            }
            rows.extend(ev_rows(ev, tick, width));
        }
        _ => rows.extend(ev_rows(ev, tick, width)),
    }
    rows
}

pub(crate) fn is_message(ev: &Ev) -> bool {
    // an answer to an inbox item reads like your message (BISE-307): a
    // block with a blank row around it
    matches!(ev, Ev::You(..) | Ev::Assistant(_) | Ev::Thinking { .. } | Ev::AgentMsg { .. } | Ev::Answered { .. } | Ev::Approval { .. })
}

pub(crate) fn is_tool_block(ev: &Ev) -> bool {
    matches!(ev, Ev::Tool(_) | Ev::Sub { .. })
}

pub(crate) fn is_notice(ev: &Ev) -> bool {
    matches!(
        ev,
        Ev::Warn(_)
            | Ev::Card { .. }
            | Ev::Err(_)
            | Ev::Info(_)
            | Ev::Pr { .. }
            | Ev::Made { .. }
            | Ev::Landed { .. }
            | Ev::Compact
            | Ev::Compacted { .. }
            | Ev::Fold { .. }
            | Ev::Scheduled { .. }
            | Ev::TurnDone
            | Ev::TimeMark(_)
    ) || matches!(ev, Ev::Release(r) if !r.is_l2())
}

// the breathing rules: one blank line when the content kind switches
// (message / tool block / notice), and always above and below what's
// for you (level 2, book §9 'Emphasis'). A reply after anything but its
// thinking; the first event of the feed starts flush at the top.
pub(crate) fn wants_gap_before(ev: &Ev, prev: Option<&Ev>) -> bool {
    let Some(p) = prev else {
        return false;
    };
    // the messages of one pair of agents stack; a new pair starts after
    // a blank row (book §9, BISE-106); a time mark stands apart from
    // whatever came before
    if is_l3(ev) && is_l3(p) {
        return l3_pair(ev) != l3_pair(p);
    }
    if matches!(ev, Ev::TimeMark(_)) {
        return true;
    }
    // what's for you (level 2) has room above and below, even between
    // two level-2 blocks and after the reply's thinking (book §9
    // 'Emphasis')
    if is_l2(ev) || is_l2(p) {
        return true;
    }
    let prev_message = is_message(p);
    let prev_tool = is_tool_block(p);
    let prev_notice = is_notice(p);
    match ev {
        Ev::Assistant(_) => {
            !matches!(p, Ev::Thinking { .. })
                && (prev_message || prev_tool || prev_notice)
        }
        Ev::Thinking { .. } => prev_message || prev_tool || prev_notice,
        _ if is_tool_block(ev) => prev_message || prev_notice,
        _ if is_notice(ev) => prev_message || prev_tool,
        _ => prev_message || prev_tool || prev_notice,
    }
}

// structural annotations (turn separators, idle markers) are debug-only;
// messages, tool activity, compaction and errors always show
pub(crate) fn ev_visible(ev: &Ev, debug: bool) -> bool {
    // BISE-271: a time, never a row (its hover draws it)
    if matches!(ev, Ev::Ended(_)) {
        return false;
    }
    if debug {
        return true;
    }
    // BISE-110: a box that only sent the messages drawn below it
    if let Ev::Tool(td) = ev {
        return !td.quiet || td.expanded;
    }
    !matches!(
        ev,
        Ev::Turn
            | Ev::TurnDone
            | Ev::Idle
            | Ev::Raw(_)
            | Ev::Usage(_)
            | Ev::ToolInfo { .. }
            | Ev::ToolResult { .. }
            | Ev::ToolCode { .. }
            | Ev::ToolIntent { .. }
    )
}

/// BISE-90 (book §9, §12, mockup "reports in main"): an agent's report
/// and the card the hub opens for it are one entry. Blocked keeps the
/// card (level 1, it fades once answered); done and failed keep the
/// report line (`✓ bench: … ▸ report`). Whichever comes second takes the
/// place of the first, or is dropped; `Some(false)`: nothing appended.
fn merge_report_and_card(events: &mut [Ev], cache: &mut [Option<EventRows>], ev: &Ev) -> Option<bool> {
    fn kind_of(k: &str) -> Option<&'static str> {
        match k {
            "done" => Some("done"),
            "blocked" => Some("blocked"),
            k if k.contains("fail") => Some("failed"),
            _ => None,
        }
    }
    // (agent, kind, is_card) of a report or a card
    fn key(ev: &Ev) -> Option<(String, &'static str, bool)> {
        match ev {
            Ev::AgentMsg { from, text, .. } => {
                let (k, _) = report_parts(text)?;
                Some((from.clone(), kind_of(k)?, false))
            }
            Ev::Card { card, .. } => Some((card.agent.clone(), kind_of(&card.kind)?, true)),
            _ => None,
        }
    }
    let (agent, kind, is_card) = key(ev)?;
    // its twin: the other one, among the last few events
    let twin = events
        .iter()
        .enumerate()
        .rev()
        .take(8)
        .take_while(|(_, e)| !matches!(e, Ev::You(..)))
        .find(|(_, e)| key(e).is_some_and(|(a, k, c)| a == agent && k == kind && c != is_card))
        .map(|(i, _)| i)?;
    // the card for blocked, the report line for done and failed
    let keep_new = (kind == "blocked") == is_card;
    if keep_new {
        events[twin] = ev.clone();
        if let Some(c) = cache.get_mut(twin) {
            *c = None;
        }
    }
    Some(false)
}

/// The widest title (cut at 28) of the run of ↗ lines `events[i]` is in.
fn made_run_pad(events: &[Ev], i: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    let made = |e: &Ev| match e {
        Ev::Made { title, .. } => Some(title.width().min(crate::render::MADE_TITLE_MAX)),
        _ => None,
    };
    let back = events[..=i].iter().rev().map_while(made);
    let fwd = events[i + 1..].iter().map_while(made);
    back.chain(fwd).max().unwrap_or(0)
}

pub(crate) fn push_event(events: &mut Vec<Ev>, cache: &mut Vec<Option<EventRows>>, ev: Ev) -> bool {
    // a new ↗ line may widen its run: the run's rows are built again
    if matches!(ev, Ev::Made { .. }) {
        for k in (0..events.len()).rev() {
            if !matches!(events[k], Ev::Made { .. }) {
                break;
            }
            if let Some(c) = cache.get_mut(k) {
                *c = None;
            }
        }
    }
    if let Some(appended) = merge_report_and_card(events, cache, &ev) {
        return appended;
    }
    // BISE-293: a turn that fails for the reason its candidate was
    // discarded says it once: the failure takes the warning's place
    if let Ev::Err(t) = &ev {
        let why = t.strip_prefix("turn failed: ");
        if let (Some(why), Some(Ev::Warn(w))) = (why, events.last()) {
            if w.strip_prefix("candidate discarded: ") == Some(why) {
                let i = events.len() - 1;
                events[i] = ev;
                if let Some(c) = cache.get_mut(i) {
                    *c = None;
                }
                return false;
            }
        }
    }
    // annotations enrich the matching tool event instead of stacking
    match &ev {
        Ev::ToolInfo { id, name, args } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.name = Some(name.clone());
                td.args = Some(args.clone());
                cache[i] = None;
            }
            return false;
        }
        Ev::ToolResult { id, ok, preview } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                // tabs expanded, escapes dropped: the columns the terminal shows
                td.result = Some((*ok, crate::sanitize::clean(preview, crate::sanitize::TAB_OUTPUT).into_owned()));
                cache[i] = None;
                settle_quiet(events, cache, i);
                forget_work_run(events, cache, i);
            }
            return false;
        }
        // the approvals gate holds the running call (approvals-design.md §3.1)
        Ev::Gate(g) => {
            if let Some((i, td)) = last_tool_mut(events, |td| matches!(td.state, ToolState::Run)) {
                td.gate = (*g != crate::wire::Gate::Done).then(|| (*g, std::time::Instant::now()));
                cache[i] = None;
            }
            return false;
        }
        Ev::ToolIntent { id, text } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.intent = Some(text.clone());
                cache[i] = None;
                forget_work_run(events, cache, i);
            }
            return false;
        }
        Ev::ToolCode { id, code } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.code = Some(code.clone());
                cache[i] = None;
                settle_quiet(events, cache, i);
            }
            return false;
        }
        Ev::Tool(done) if !matches!(done.state, ToolState::Run) => {
            // a tool finishing rewrites its running line
            let running = |td: &ToolData| td.id == done.id && matches!(td.state, ToolState::Run);
            if let Some((i, td)) = last_tool_mut(events, running) {
                td.state = done.state.clone();
                td.elapsed = Some(fmt_elapsed(td.started));
                td.took = Some(td.started.elapsed());
                if td.result.is_none() {
                    td.result = done.result.clone();
                }
                cache[i] = None;
                settle_quiet(events, cache, i);
                forget_work_run(events, cache, i);
                return false;
            }
            events.push(ev);
            cache.push(None);
            after_append(events, cache);
            return true;
        }
        // BISE-235: a running release row gives its place to the next one
        Ev::Release(_) => {
            let last = events.iter().rposition(|e| matches!(e, Ev::Release(_)));
            if let Some(i) = last.filter(|&i| matches!(events[i], Ev::Release(crate::release_row::Row::Running(_)))) {
                events[i] = ev;
                cache[i] = None;
                return false;
            }
        }
        // BISE-86: the hub could not deliver your message: its line gets
        // `✗`; only the newest `not delivered` line keeps its question
        Ev::Undelivered { name, text, .. } => {
            for (i, e) in events.iter_mut().enumerate().rev() {
                if let Ev::Undelivered { open, .. } = e {
                    if *open {
                        *open = false;
                        cache[i] = None;
                    }
                }
            }
            // your line (the text, or `@name text` from another view) gets
            // `✗` by bise-proto's one rule (G1, the hub's fold too); not in
            // this feed (an `@name` line from another view shows only once
            // delivered): your line comes back, marked
            let m = lines::Mark::Failed { to: name.clone(), text: text.clone() };
            if !deliver(events, cache, &m) {
                let t = format!("@{} {}", name, text);
                push_event(events, cache, Ev::You(t, Mark::Failed, false));
            }
        }
        // C3: a steering line moves the mark of your message (bise-proto's
        // one rule, G1: none with its words raises what you sent this turn,
        // BISE-90); an injected notification with no message of yours
        // shows its info line
        Ev::MarkYou { mark, or } => {
            if !deliver(events, cache, mark) {
                if let Some(ev) = or {
                    return push_event(events, cache, (**ev).clone());
                }
            }
            return false;
        }
        // BISE-31: a closed card fades in place (its last line in this
        // feed); not in the feed (an older page): an info line as before
        Ev::CardClosed { id, res } => {
            let found = events.iter_mut().enumerate().rev().find_map(|(i, e)| match e {
                Ev::Card { card, closed } if card.id == Some(*id) => Some((i, closed)),
                _ => None,
            });
            match found {
                Some((i, closed)) => {
                    *closed = res.clone();
                    if let Some(c) = cache.get_mut(i) {
                        *c = None;
                    }
                    return false;
                }
                // not in the feed (an older page, or a done card that
                // became its report line, BISE-90): nothing to show
                None => return false,
            }
        }
        // the summary ends the compaction: its line stops pulsing
        Ev::Compacted { .. } => {
            if let Some(i) = events.iter().rposition(|e| matches!(e, Ev::Compact)) {
                if let Some(c) = cache.get_mut(i) {
                    *c = None;
                }
            }
        }
        // a turn starts: the model reads what you sent since the last one
        // (a message at idle goes straight to ✓✓; G1, bise-proto's rule)
        Ev::Turn => {
            deliver(events, cache, &lines::Mark::Turn);
        }
        // the turn ended: a tool still shown as running was abandoned
        // (interrupt or failed turn) — freeze it so the elapsed stops
        Ev::TurnDone | Ev::Idle => {
            // back to the previous end of turn: the tools before it were
            // frozen then (O(turn), not O(history))
            for (i, e) in events.iter_mut().enumerate().rev() {
                if matches!(e, Ev::TurnDone | Ev::Idle) {
                    break;
                }
                if let Ev::Tool(td) = e {
                    if matches!(td.state, ToolState::Run) {
                        td.state = ToolState::Fail;
                        td.elapsed = Some(fmt_elapsed(td.started));
                        if td.result.is_none() {
                            td.result = Some((false, "interrupted".to_string()));
                        }
                        cache[i] = None;
                    }
                }
            }
        }
        _ => {}
    }
    // a sub-call is an output line of its TypeScript box: the box rebuilds
    if matches!(ev, Ev::Sub { .. }) {
        if let Some(j) = box_owner(events, events.len()) {
            cache[j] = None;
        }
    }
    events.push(ev);
    cache.push(None);
    after_append(events, cache);
    // BISE-110: the message a box sent shows: the box can hide
    if let Some(Ev::AgentMsg { id, .. }) = events.last().filter(|e| matches!(e, Ev::AgentMsg { id, .. } if !id.is_empty())) {
        let id = id.clone();
        quiet_back(events, cache, &id);
    }
    true
}

// ---- a box that only sends a message (BISE-110, book §9) ----

/// How far back a message looks for the box that sent it.
const QUIET_BACK: usize = 64;

/// Message `id` is drawn in this feed after event `i`.
fn drawn_after(events: &[Ev], i: usize, id: &str) -> bool {
    events[i + 1..].iter().take(QUIET_BACK).any(|e| matches!(e, Ev::AgentMsg { id: m, .. } if m == id) && ev_visible(e, false))
}

/// Box `i` hides iff it only sent messages that are drawn below it
/// (toolbox::sent_ids); when that changes, the rows around it rebuild
/// (the gap after it, the run of level-3 lines it joins or splits).
fn settle_quiet(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) {
    let Some(Ev::Tool(td)) = events.get(i) else { return };
    let ids = crate::toolbox::sent_ids(td);
    let quiet = !ids.is_empty() && ids.iter().all(|id| drawn_after(events, i, id));
    if td.quiet == quiet {
        return;
    }
    if let Some(Ev::Tool(td)) = events.get_mut(i) {
        td.quiet = quiet;
    }
    forget_around(events, cache, i);
}

/// Event `i` shows or hides: its rows, the rows after it and the run of
/// level-3 lines before it rebuild.
fn forget_around(events: &[Ev], cache: &mut [Option<EventRows>], i: usize) {
    let start = match prev_visible(events, i, false) {
        Some(p) if is_l3(&events[p]) => run_back(events, p, false).0,
        _ => i,
    };
    forget(cache, start..=events.len().saturating_sub(1));
}

/// A message with id `id` was appended: the box that sent it, among the
/// last events, may hide.
fn quiet_back(events: &mut [Ev], cache: &mut [Option<EventRows>], id: &str) {
    let n = events.len();
    let found = (n.saturating_sub(QUIET_BACK)..n).rev().find(|&j| {
        // the output names the id (a substring test) before the script
        // is read: replaying a long feed stays as fast
        matches!(&events[j], Ev::Tool(td) if !td.quiet
            && td.result.as_ref().is_some_and(|(ok, out)| *ok && out.contains(id))
            && crate::toolbox::sent_ids(td).iter().any(|s| s == id))
    });
    if let Some(j) = found {
        settle_quiet(events, cache, j);
    }
}

/// The bash / TypeScript box that event `i` (a sub-call) belongs to: the
/// tool right before it, sub-calls in between.
pub(crate) fn box_owner(events: &[Ev], i: usize) -> Option<usize> {
    for j in (0..i).rev() {
        match &events[j] {
            Ev::Sub { .. } => continue,
            Ev::Tool(td) if crate::toolbox::is_boxed(td) => return Some(j),
            _ => return None,
        }
    }
    None
}

/// The sub-calls right after the box at `i` (its output lines).
fn subs_of(events: &[Ev], i: usize) -> Vec<crate::toolbox::SubCall<'_>> {
    events[i + 1..]
        .iter()
        .map_while(|e| match e {
            Ev::Sub { name, ok, preview } => Some(crate::toolbox::SubCall { name, ok: *ok, preview }),
            _ => None,
        })
        // a computer action has its own row under the box (design §8)
        .filter(|s| crate::computer_use::sub_row(s.name, s.preview).is_none())
        .collect()
}

/// The rows of event `i` at this width, built when missing (a running
/// tool redraws its tool line). Returns how many rows it has.
pub(crate) fn ensure_rows(
    events: &[Ev],
    cache: &mut Vec<Option<EventRows>>,
    i: usize,
    debug: bool,
    width: usize,
    tick: u32,
) -> usize {
    // total: no event there, no rows; a cache shorter than the events
    // (a feed swapped or trimmed since the last frame) grows first
    if i >= events.len() {
        return 0;
    }
    if cache.len() < events.len() {
        cache.resize_with(events.len(), || None);
    }
    match cache[i].as_mut() {
        Some(c) if c.width == width as u16 && c.main == main_feed() => {
            refresh_live(c, &events[i], tick);
        }
        _ => cache[i] = Some(event_rows(events, i, debug, width, tick)),
    }
    cache[i].as_ref().map_or(0, |c| c.rows.len())
}

/// The anchor that shows the last `h` rows.
pub(crate) fn bottom_anchor(n: usize, h: usize, rows_of: &mut dyn FnMut(usize) -> usize) -> (usize, usize) {
    let mut need = h;
    let mut i = n;
    while i > 0 {
        i -= 1;
        let len = rows_of(i);
        if len >= need {
            return (i, len - need);
        }
        need -= len;
    }
    (0, 0)
}

/// Move a (event, row) anchor by `d` rows (negative: up), building only
/// the rows it walks over.
pub(crate) fn move_anchor(
    anchor: (usize, usize),
    d: isize,
    n: usize,
    rows_of: &mut dyn FnMut(usize) -> usize,
) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }
    let (mut i, mut r) = anchor;
    if i >= n {
        i = n - 1;
        r = usize::MAX;
    }
    r = r.min(rows_of(i).saturating_sub(1));
    let mut k = d.unsigned_abs();
    if d < 0 {
        while k > 0 {
            if r >= k {
                r -= k;
                break;
            }
            k -= r;
            r = 0;
            // one row up: the last row of the previous event with rows
            let mut j = i;
            let mut moved = false;
            while j > 0 {
                j -= 1;
                let len = rows_of(j);
                if len > 0 {
                    (i, r) = (j, len - 1);
                    k -= 1;
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
        }
    } else {
        while k > 0 {
            let len = rows_of(i);
            if r + k < len {
                r += k;
                break;
            }
            // to the first row of the next event with rows
            k -= len.saturating_sub(r);
            let mut j = i + 1;
            while j < n && rows_of(j) == 0 {
                j += 1;
            }
            if j >= n {
                r = len.saturating_sub(1);
                break;
            }
            (i, r) = (j, 0);
        }
    }
    (i, r)
}

// ---- progressive disclosure (book §11, BISE-12) ----

/// A tool has something behind its `▸`: an output, or an edit's diff.
fn tool_discloses(td: &ToolData) -> bool {
    if crate::toolbox::opens_as_box(td) {
        // BISE-223: a call's row opens into its box (every view)
        return true;
    }
    td.result.as_ref().is_some_and(|(_, r)| !r.trim().is_empty())
        || (matches!(td.name.as_deref(), Some("apply_patch" | "edit" | "write_file")) && td.code.is_some())
}

/// Whether event `ev` opens and closes: a thinking section, a tool with
/// an output or a diff, a report, a brief.
pub(crate) fn discloses(ev: &Ev) -> bool {
    match ev {
        Ev::Thinking { .. } => true,
        Ev::Tool(td) => tool_discloses(td),
        Ev::AgentMsg { text, level: 3, .. } if !is_brief(text) && report_parts(text).is_none() => l3_long(text),
        Ev::AgentMsg { text, .. } => is_brief(text) || report_parts(text).is_some(),
        // a why, or a question or answer longer than its gist
        Ev::Answered { agent, question, answer, why, .. } => crate::answered::answered_opens(agent, question, answer, why),
        Ev::Compacted { text, .. } | Ev::Fold { text, .. } | Ev::Scheduled { words: text, .. } => !text.trim().is_empty(),
        Ev::Release(r) => r.discloses(),
        // a long message of yours folds (BISE-239)
        Ev::You(t, ..) => crate::render::you_folds(t),
        // an answer to an inbox item opens on its full question and
        // words (BISE-307)
        Ev::Approval { text, note, asked, .. } => crate::render::answer_opens(text, note, asked),
        _ => false,
    }
}

/// Open or close event `i` in place (its rows rebuild). False when it
/// has nothing to disclose.
pub(crate) fn toggle_event(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    // the first call of a `▸ n commands` fold: the fold (BISE-223)
    if tool_fold(events, i, false).is_some_and(|f| f.carrier == i) {
        return toggle_tool_fold(events, cache, i);
    }
    // the first line of a folded run: the fold opens or closes
    if events.get(i).is_some_and(is_l3) && folded_run(events, i, false) == Some(i) {
        return toggle_fold(events, cache, i);
    }
    toggle_own(events, cache, i)
}

/// Open or close the fold starting at `start` (its lines rebuild).
fn toggle_fold(events: &mut [Ev], cache: &mut [Option<EventRows>], start: usize) -> bool {
    let end = run_from(events, start, false).end;
    if let Some(Ev::AgentMsg { fold, .. }) = events.get_mut(start) {
        *fold = !*fold;
    }
    forget(cache, start..=end);
    true
}

/// Open or close event `i` in place from row `row` of its rows: on the
/// first line of an open fold, its fold row toggles the fold and its
/// message row the message; elsewhere as [`toggle_event`].
pub(crate) fn toggle_at(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize, row: usize) -> bool {
    // an open `▾ n commands` fold: its row closes it, the call's own
    // rows open the call (BISE-223)
    if tool_fold(events, i, false).is_some_and(|f| f.carrier == i && f.open) {
        let prev = prev_visible(events, i, false).map(|p| &events[p]);
        let fold_row = usize::from(wants_gap_before(&events[i], prev));
        if row > fold_row {
            return toggle_own(events, cache, i);
        }
    }
    if events.get(i).is_some_and(|e| is_l3(e) && fold_open(e)) && folded_run(events, i, false) == Some(i) {
        let prev = prev_visible(events, i, false).map(|p| &events[p]);
        let fold_row = usize::from(wants_gap_before(&events[i], prev));
        if row > fold_row {
            return toggle_own(events, cache, i);
        }
    }
    // a message of yours: only its last row (`▸ n more lines`, or the one
    // ending in `▾`) folds it; its other rows stay for reading (BISE-239).
    // Its long pastes' rows under it too (BISE-240: `▤ 1 “…” · 240
    // lines` opens it, the open text closes it)
    if you_row_stays(events, cache, i, row) {
        return false;
    }
    toggle_event(events, cache, i)
}

/// Row `row` of event `i` is a row of your message a click leaves alone:
/// its text, or the sizes of its images under it (one row per image
/// list that fits, several when a few screenshots wrap). Only its hint
/// row (`▸ n more lines` / `▾`) and its long pastes' rows toggle.
fn you_row_stays(events: &[Ev], cache: &[Option<EventRows>], i: usize, row: usize) -> bool {
    let (Some(Ev::You(t, _, open)), Some(Some(c))) = (events.get(i), cache.get(i)) else { return false };
    let width = usize::from(c.width);
    let sizes = crate::render::you_sizes_rows(t, width);
    let pastes = crate::render::you_paste_rows(t, *open, width);
    row + 1 + sizes + pastes < c.rows.len() || row + sizes >= c.rows.len()
}

/// A click on row `row` of event `i` opens or closes something
/// ([`toggle_at`] without the change): the pointer's hand (BISE-272).
pub(crate) fn toggles_at(events: &[Ev], cache: &[Option<EventRows>], i: usize, row: usize) -> bool {
    let fold_row = || {
        let prev = prev_visible(events, i, false).map(|p| &events[p]);
        usize::from(wants_gap_before(&events[i], prev))
    };
    if tool_fold(events, i, false).is_some_and(|f| f.carrier == i && f.open) && row > fold_row() {
        return own_toggles(&events[i]);
    }
    if events.get(i).is_some_and(|e| is_l3(e) && fold_open(e)) && folded_run(events, i, false) == Some(i) && row > fold_row() {
        return own_toggles(&events[i]);
    }
    if you_row_stays(events, cache, i, row) {
        return false;
    }
    if tool_fold(events, i, false).is_some_and(|f| f.carrier == i) {
        return true;
    }
    if events.get(i).is_some_and(is_l3) && folded_run(events, i, false) == Some(i) {
        return true;
    }
    events.get(i).is_some_and(own_toggles)
}

/// [`toggle_own`] would change `ev`.
fn own_toggles(ev: &Ev) -> bool {
    match ev {
        Ev::Release(r) => discloses(ev) && r.clone().open_mut().is_some(),
        _ => discloses(ev),
    }
}

/// Open or close event `i` itself (not a fold).
fn toggle_own(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    let Some(ev) = events.get_mut(i).filter(|e| discloses(e)) else {
        return false;
    };
    match ev {
        Ev::Thinking { open, .. }
        | Ev::AgentMsg { open, .. }
        | Ev::Answered { open, .. }
        | Ev::Compacted { open, .. }
        | Ev::Fold { open, .. }
        | Ev::Scheduled { open, .. }
        | Ev::Approval { open, .. }
        | Ev::You(_, _, open) => *open = !*open,
        Ev::Tool(td) if crate::toolbox::opens_as_box(td) => {
            // BISE-223: the row opens into its box (15 rows), a box that
            // hides lines opens whole, then back to the row
            if !td.opened {
                td.opened = true;
                td.expanded = false;
            } else if !td.expanded && crate::toolbox::box_folds(td) {
                td.expanded = true;
            } else {
                td.opened = false;
                td.expanded = false;
            }
        }
        Ev::Tool(td) => td.expanded = !td.expanded,
        Ev::Release(r) => match r.open_mut() {
            Some(open) => *open = !*open,
            None => return false,
        },
        _ => return false,
    }
    if let Some(c) = cache.get_mut(i) {
        *c = None;
    }
    if matches!(&events[i], Ev::Tool(_)) {
        forget_work_run(events, cache, i);
    }
    // a hidden box shows or hides again (BISE-110)
    if matches!(&events[i], Ev::Tool(td) if td.quiet) {
        forget_around(events, cache, i);
    }
    true
}

// ---- the history: runs of level 3, folds, time marks (book §10, BISE-14) ----

/// A run of level-3 lines longer than this folds into one line.
pub(crate) const FOLD_AFTER: usize = 3;
/// A pause this long without a line gets a time mark.
pub(crate) const PAUSE_MS: u128 = 5 * 60 * 1000;

/// A message between agents (level 3), not a brief or a report (they
/// have their own lines).
/// What's for you (level 2, book §9): a reply, main answering for you,
/// an agent writing to you, a report (not a brief, not level 3).
pub(crate) fn is_l2(ev: &Ev) -> bool {
    match ev {
        Ev::Assistant(_) | Ev::Answered { .. } => true,
        Ev::Release(r) => r.is_l2(),
        Ev::AgentMsg { text, .. } => !is_brief(text) && !is_l3(ev),
        _ => false,
    }
}

pub(crate) fn is_l3(ev: &Ev) -> bool {
    matches!(ev, Ev::AgentMsg { level: 3, text, .. } if !is_brief(text) && report_parts(text).is_none())
}

/// The two agents of a level-3 message, either way round (`auth-fix →
/// release` and `release → auth-fix` are one pair).
fn l3_pair(ev: &Ev) -> Option<(String, String)> {
    match ev {
        Ev::AgentMsg { from, to, id, .. } => {
            let to = crate::render::l3_receiver(to, id);
            Some(if *from <= to { (from.clone(), to) } else { (to, from.clone()) })
        }
        _ => None,
    }
}

/// The visible event before `i`.
fn prev_visible(events: &[Ev], i: usize, debug: bool) -> Option<usize> {
    (0..i).rev().find(|&j| ev_visible(&events[j], debug))
}

/// The first line of the run that holds level-3 line `i`, and how many
/// of its lines come up to `i` (included).
fn run_back(events: &[Ev], i: usize, debug: bool) -> (usize, usize) {
    let (mut start, mut n) = (i, 1);
    let mut j = i;
    while let Some(p) = prev_visible(events, j, debug) {
        if !is_l3(&events[p]) {
            break;
        }
        (start, n, j) = (p, n + 1, p);
    }
    (start, n)
}

/// A run of level-3 lines: consecutive among the visible events. Any
/// other visible line ends it (a level-1 or level-2 line, a tool, a time
/// mark), so a closed run never changes: only the last one grows.
pub(crate) struct Run {
    /// its last line
    pub(crate) end: usize,
    pub(crate) n: usize,
    /// the agents it names (senders and receivers)
    pub(crate) agents: usize,
    /// nothing visible after it yet: it may still grow
    pub(crate) live: bool,
}

/// The run that starts at level-3 line `start`, counted forward.
fn run_from(events: &[Ev], start: usize, debug: bool) -> Run {
    let mut names: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let (mut n, mut end, mut live) = (0, start, true);
    for (j, e) in events.iter().enumerate().skip(start) {
        if !ev_visible(e, debug) {
            continue;
        }
        let Ev::AgentMsg { from, to, .. } = e else {
            live = false;
            break;
        };
        if !is_l3(e) {
            live = false;
            break;
        }
        names.insert(from);
        if !to.is_empty() {
            names.insert(to);
        }
        n += 1;
        end = j;
    }
    Run { end, n, agents: names.len(), live }
}

/// Whether the run of level-3 line `i` folds, and where it starts:
/// counted back to its start, then forward only as far as needed.
fn folded_run(events: &[Ev], i: usize, debug: bool) -> Option<usize> {
    let (start, mut n) = run_back(events, i, debug);
    let mut j = i + 1;
    while n <= FOLD_AFTER && j < events.len() {
        let e = &events[j];
        if ev_visible(e, debug) {
            if !is_l3(e) {
                break;
            }
            n += 1;
        }
        j += 1;
    }
    (n > FOLD_AFTER).then_some(start)
}

fn fold_open(ev: &Ev) -> bool {
    matches!(ev, Ev::AgentMsg { fold: true, .. })
}

/// The rows of level-3 line `i`: its own line in a short run; in a
/// folded one the first line carries the fold (`▸ 12 messages between 5
/// agents`), the others show only when it is open, in place, in order.
/// The sender and receiver of a level-3 message, in that order.
fn l3_way(ev: &Ev) -> Option<(&str, String)> {
    match ev {
        Ev::AgentMsg { from, to, id, .. } => Some((from.as_str(), crate::render::l3_receiver(to, id))),
        _ => None,
    }
}

/// The rows of level-3 line `i`: its chip then its text; right after a
/// line with the same sender and receiver, its text alone (the run
/// shares one chip, book §9, BISE-127).
fn l3_ev_rows(events: &[Ev], i: usize, debug: bool, tick: u32, width: usize) -> Vec<Line<'static>> {
    let ev = &events[i];
    let same = prev_visible(events, i, debug).is_some_and(|p| is_l3(&events[p]) && l3_way(&events[p]) == l3_way(ev));
    if same {
        crate::render::l3_text_only_rows(ev, width)
    } else {
        ev_rows(ev, tick, width)
    }
}

fn l3_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> (Vec<Line<'static>>, Option<LiveHead>) {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    let Some(start) = folded_run(events, i, debug) else {
        let prev = prev_visible(events, i, debug).map(|p| &events[p]);
        if wants_gap_before(ev, prev) {
            rows.push(Line::from(""));
        }
        rows.extend(l3_ev_rows(events, i, debug, tick, width));
        return (rows, None);
    };
    let open = fold_open(&events[start]);
    if i != start {
        if open {
            let prev = prev_visible(events, i, debug).map(|p| &events[p]);
            if wants_gap_before(ev, prev) {
                rows.push(Line::from(""));
            }
            rows.extend(l3_ev_rows(events, i, debug, tick, width));
        }
        return (rows, None);
    }
    let prev = prev_visible(events, i, debug).map(|p| &events[p]);
    if wants_gap_before(ev, prev) {
        rows.push(Line::from(""));
    }
    let run = run_from(events, start, debug);
    let at = rows.len();
    rows.push(fold_line(run.n, run.agents, open, run.live, tick, code_width(width)));
    if open {
        rows.extend(l3_ev_rows(events, i, debug, tick, width));
    }
    let live = run.live.then_some(LiveHead {
        at,
        len: 1,
        what: Live::Fold { n: run.n, agents: run.agents, open },
    });
    (rows, live)
}

fn forget(cache: &mut [Option<EventRows>], range: std::ops::RangeInclusive<usize>) {
    for c in cache.iter_mut().take(range.end() + 1).skip(*range.start()) {
        *c = None;
    }
}

// ---- find (BISE-237): open what hides a match, close it after ----

/// What find opened to show its current match, to close it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Undo {
    /// event `i` itself (a report, a brief, a `▸ why`, an output)
    Own(usize),
    /// a call's row or box: how it was (opened, expanded)
    Box(usize, bool, bool),
    /// the `▸ n commands` fold whose first call is this one
    ToolFold(usize),
    /// the fold of level-3 lines that starts here
    L3Fold(usize),
}

/// Open what hides event `i`'s text: the fold it is in, then the event
/// itself (a call opens whole). Thinking stays closed. Returns what
/// changed, for [`unreveal`].
pub(crate) fn reveal(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> Vec<Undo> {
    let mut out = Vec::new();
    if i >= events.len() || matches!(events[i], Ev::Thinking { .. }) {
        return out;
    }
    if let Some(f) = tool_fold(events, i, false) {
        if !f.open && (f.carrier..=f.last).contains(&i) {
            toggle_tool_fold(events, cache, f.carrier);
            out.push(Undo::ToolFold(f.carrier));
        }
    }
    if is_l3(&events[i]) {
        if let Some(s) = folded_run(events, i, false).filter(|&s| !fold_open(&events[s])) {
            toggle_fold(events, cache, s);
            out.push(Undo::L3Fold(s));
        }
    }
    if own_open(&events[i]) == Some(false) {
        if let Ev::Tool(td) = &mut events[i] {
            if crate::toolbox::opens_as_box(td) {
                out.push(Undo::Box(i, td.opened, td.expanded));
                (td.opened, td.expanded) = (true, true);
                forget(cache, i..=i);
                forget_work_run(events, cache, i);
                return out;
            }
        }
        if toggle_own(events, cache, i) {
            out.push(Undo::Own(i));
        }
    }
    out
}

/// Close again what [`reveal`] opened (last opened, first closed).
pub(crate) fn unreveal(events: &mut [Ev], cache: &mut [Option<EventRows>], undo: Vec<Undo>) {
    for u in undo.into_iter().rev() {
        match u {
            Undo::Own(i) if i < events.len() => {
                toggle_own(events, cache, i);
            }
            Undo::Box(i, opened, expanded) if matches!(events.get(i), Some(Ev::Tool(_))) => {
                if let Ev::Tool(td) = &mut events[i] {
                    (td.opened, td.expanded) = (opened, expanded);
                }
                forget(cache, i..=i);
                forget_work_run(events, cache, i);
            }
            Undo::ToolFold(c) if matches!(events.get(c), Some(Ev::Tool(td)) if td.fold_open) => {
                toggle_tool_fold(events, cache, c);
            }
            Undo::L3Fold(s) if events.get(s).is_some_and(fold_open) => {
                toggle_fold(events, cache, s);
            }
            _ => {}
        }
    }
}

/// Older lines came in front of the feed: every index moves by `d`.
pub(crate) fn shift_undo(undo: &mut [Undo], d: usize) {
    for u in undo.iter_mut() {
        match u {
            Undo::Own(i) | Undo::Box(i, _, _) | Undo::ToolFold(i) | Undo::L3Fold(i) => *i += d,
        }
    }
}

/// A line was appended: the run before it changes (its count, the lines
/// that fold at the fourth, its pulse when it closes). Nothing else does.
fn after_append(events: &[Ev], cache: &mut [Option<EventRows>]) {
    let e = events.len() - 1;
    if !ev_visible(&events[e], false) {
        return;
    }
    if is_work(&events[e]) {
        forget_work_run(events, cache, e);
    }
    let Some(p) = prev_visible(events, e, false).filter(|&p| is_l3(&events[p])) else {
        return;
    };
    let (start, n) = run_back(events, p, false);
    if is_l3(&events[e]) && n + 1 == FOLD_AFTER + 1 {
        forget(cache, start..=e);
    } else if n + usize::from(is_l3(&events[e])) > FOLD_AFTER {
        forget(cache, start..=start);
    }
}

/// A live line after a pause of `gap_ms`: first a time mark `· 14:31 ·`
/// (`now` gives the time). Not at the top of a feed, not twice.
pub(crate) fn pause_mark(
    events: &mut Vec<Ev>,
    cache: &mut Vec<Option<EventRows>>,
    gap_ms: u128,
    now: impl FnOnce() -> String,
) -> bool {
    if gap_ms < PAUSE_MS {
        return false;
    }
    match events.iter().rev().find(|e| ev_visible(e, false)) {
        None | Some(Ev::TimeMark(_)) => false,
        Some(_) => push_event(events, cache, Ev::TimeMark(now())),
    }
}

/// BISE-271: when the turn that event `i` belongs to ended (ms since
/// the epoch), for a reply of the turn (its thinking included) or the
/// row that ends it; None for anything else, a turn still running, a
/// turn whose end has no time (a hub that does not say, a replay).
pub(crate) fn turn_end_of(events: &[Ev], i: usize) -> Option<u64> {
    let ends_turn = |e: &Ev| match e {
        Ev::TurnDone => true,
        // "turn interrupted by main": a stop mid-answer (wire.rs)
        Ev::Warn(t) => t == "turn interrupted" || t.starts_with("turn interrupted by ") || crate::wire::is_plan_line(t),
        Ev::Err(t) => t.starts_with("turn failed: ") || t.starts_with("turn stopped: "),
        _ => false,
    };
    let e = events.get(i)?;
    if !matches!(e, Ev::Assistant(_) | Ev::Thinking { .. }) && !ends_turn(e) {
        return None;
    }
    events[i + 1..].iter().find_map(|e| match e {
        Ev::Ended(t) => Some(Some(*t)),
        Ev::Turn => Some(None),
        _ => None,
    })?
}

/// The local time, `14:31`: the zone from `when::offset_at` (one `date`
/// per hour, cached; UTC when it fails), not a `date` process per call
/// (BISE-292).
#[cfg(test)]
pub(crate) fn local_hhmm() -> String {
    let ms = crate::when::now_ms();
    let s = (ms / 1000) as i64 + i64::from(crate::when::offset_at(ms));
    let s = s.rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, (s / 60) % 60)
}

// ---- everything at once (ctrl+o, input.rs) ----

/// Whether event `ev` itself is open; None when it has nothing to
/// disclose.
fn own_open(ev: &Ev) -> Option<bool> {
    if !discloses(ev) {
        return None;
    }
    match ev {
        Ev::Thinking { open, .. }
        | Ev::AgentMsg { open, .. }
        | Ev::Answered { open, .. }
        | Ev::Compacted { open, .. }
        | Ev::Fold { open, .. }
        | Ev::Scheduled { open, .. }
        | Ev::Approval { open, .. }
        | Ev::You(_, _, open) => Some(*open),
        // a row, or an open box that still hides lines, is closed
        Ev::Tool(td) if crate::toolbox::opens_as_box(td) => {
            Some(td.opened && (td.expanded || !crate::toolbox::box_folds(td)))
        }
        Ev::Tool(td) => Some(td.expanded),
        Ev::Release(crate::release_row::Row::Failed { open, .. } | crate::release_row::Row::Warned { open, .. }) => Some(*open),
        _ => None,
    }
}

/// Whether event `i` is closed and can open: a thinking section, an
/// output, a report, a brief, a long level-3 line, a `▸ why`, a fold.
pub(crate) fn is_closed_at(events: &[Ev], i: usize) -> bool {
    let ev = &events[i];
    own_open(ev) == Some(false)
        || (is_l3(ev) && !fold_open(ev) && folded_run(events, i, false) == Some(i))
        || tool_fold(events, i, false).is_some_and(|f| f.carrier == i && !f.open)
}

/// Event `i` is open and ctrl+o would close it (the ctrl hints).
pub(crate) fn is_open_at(events: &[Ev], i: usize) -> bool {
    let ev = &events[i];
    own_open(ev) == Some(true) || (is_l3(ev) && fold_open(ev) && folded_run(events, i, false) == Some(i))
}

/// Anything closed in the feed.
pub(crate) fn anything_closed(events: &[Ev]) -> bool {
    (0..events.len()).any(|i| is_closed_at(events, i))
}

/// Open (or close) everything that discloses, folds included.
pub(crate) fn set_everything(events: &mut [Ev], cache: &mut [Option<EventRows>], open: bool) {
    // BISE-223: every call opens whole (its fold too), or every call is
    // a row again
    let mut changed = false;
    for e in events.iter_mut() {
        if let Ev::Tool(td) = e {
            if crate::toolbox::opens_as_box(td) && (td.opened != open || td.expanded != open || td.fold_open != open) {
                (td.opened, td.expanded, td.fold_open) = (open, open, open);
                changed = true;
            }
            // BISE-304: an edits fold opens or closes with its diffs
            if is_edit(td) && td.fold_open != open {
                td.fold_open = open;
                changed = true;
            }
        }
    }
    if changed {
        forget(cache, 0..=cache.len().saturating_sub(1));
    }
    for i in 0..events.len() {
        if is_l3(&events[i]) && fold_open(&events[i]) != open && folded_run(events, i, false) == Some(i) {
            toggle_fold(events, cache, i);
        }
        if own_open(&events[i]).is_some_and(|o| o != open) {
            toggle_own(events, cache, i);
        }
    }
}

// ---- message marks (C3, book §13, BISE-15) ----

/// Mark `m` on the feed by bise-proto's one rule (`lines::deliver`, G1):
/// the rows whose mark moved are drawn again. False: no message of yours
/// matched.
fn deliver(events: &mut [Ev], cache: &mut [Option<EventRows>], m: &lines::Mark) -> bool {
    let Some(moved) = lines::deliver(events, m) else { return false };
    for i in moved {
        if let Some(c) = cache.get_mut(i) {
            *c = None;
        }
    }
    true
}

/// Your messages and the turns in the feed, as bise-proto's rule reads
/// them.
impl lines::Delivered for Ev {
    fn yours(&self) -> Option<(&str, Mark)> {
        match self {
            Ev::You(t, m, ..) => Some((t.as_str(), *m)),
            _ => None,
        }
    }

    fn set_mark(&mut self, to: Mark) {
        if let Ev::You(_, m, ..) = self {
            *m = to;
        }
    }

    fn turn_start(&self) -> bool {
        matches!(self, Ev::Turn)
    }
}

// ---- tool rows: the `▸ n commands` fold (BISE-223) and the edits fold ----

/// What a fold of calls gathers: bash / TypeScript calls (`▸ 6
/// commands`, BISE-223) or file edits (`▸ edited 4 files`, BISE-304).
/// The two never mix: a command ends a run of edits and the reverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FoldKind {
    Commands,
    Edits,
}

/// A file edit: Vibe's edit and write_file, OpenAI's apply_patch.
pub(crate) fn is_edit(td: &ToolData) -> bool {
    matches!(td.name.as_deref(), Some("apply_patch" | "edit" | "write_file"))
}

fn call_kind(td: &ToolData) -> Option<FoldKind> {
    if crate::toolbox::is_boxed(td) {
        Some(FoldKind::Commands)
    } else if is_edit(td) {
        Some(FoldKind::Edits)
    } else {
        None
    }
}

/// The work a fold of `kind` holds: its calls, their sub-calls (the
/// commands'), the thinking between them.
fn in_run(ev: &Ev, kind: FoldKind) -> bool {
    match ev {
        Ev::Tool(td) => call_kind(td) == Some(kind),
        Ev::Sub { .. } => kind == FoldKind::Commands,
        Ev::Thinking { .. } => true,
        _ => false,
    }
}

/// The kinds of fold `ev` can sit in: a call its own, the thinking
/// between calls either.
fn kinds_of(ev: &Ev) -> &'static [FoldKind] {
    match ev {
        Ev::Tool(td) => match call_kind(td) {
            Some(FoldKind::Commands) => &[FoldKind::Commands],
            Some(FoldKind::Edits) => &[FoldKind::Edits],
            None => &[],
        },
        Ev::Sub { .. } => &[FoldKind::Commands],
        Ev::Thinking { .. } => &[FoldKind::Commands, FoldKind::Edits],
        _ => &[],
    }
}

fn is_work(ev: &Ev) -> bool {
    !kinds_of(ev).is_empty()
}

/// The first and last event of the run of `kind` that holds `i` (the
/// visible events only; the hidden ones are skipped).
fn work_run(events: &[Ev], i: usize, debug: bool, kind: FoldKind) -> (usize, usize) {
    let (mut a, mut b) = (i, i);
    for j in (0..i).rev() {
        let e = &events[j];
        if !ev_visible(e, debug) {
            continue;
        }
        if !in_run(e, kind) {
            break;
        }
        a = j;
    }
    for (j, e) in events.iter().enumerate().skip(i + 1) {
        if !ev_visible(e, debug) {
            continue;
        }
        if !in_run(e, kind) {
            break;
        }
        b = j;
    }
    (a, b)
}

/// A fold of done calls: its first call (which draws the fold row), its
/// last one, its calls, their total time, open or not.
pub(crate) struct ToolFold {
    pub(crate) carrier: usize,
    pub(crate) last: usize,
    pub(crate) n: usize,
    /// the done calls it folds, in order
    pub(crate) calls: Vec<usize>,
    pub(crate) total: std::time::Duration,
    pub(crate) open: bool,
}

/// A done call of `kind`: ok, and for an edit, its patch known (the
/// fold counts its files and lines).
fn done_call(ev: &Ev, kind: FoldKind) -> Option<&ToolData> {
    match ev {
        Ev::Tool(td) if call_kind(td) == Some(kind) && matches!(td.state, ToolState::Ok) => {
            (kind == FoldKind::Commands || td.code.is_some()).then_some(td)
        }
        _ => None,
    }
}

/// How many done calls a run needs to fold.
fn fold_at(kind: FoldKind) -> usize {
    match kind {
        FoldKind::Commands => crate::toolrow::FOLD_TOOLS,
        FoldKind::Edits => crate::toolrow::FOLD_EDITS,
    }
}

fn fold_of(events: &[Ev], i: usize, debug: bool, kind: FoldKind) -> Option<ToolFold> {
    let (a, b) = work_run(events, i, debug, kind);
    let calls: Vec<usize> =
        (a..=b).filter(|&j| ev_visible(&events[j], debug) && done_call(&events[j], kind).is_some()).collect();
    if calls.len() < fold_at(kind) {
        return None;
    }
    let total = calls.iter().filter_map(|&j| done_call(&events[j], kind).and_then(|td| td.took)).sum();
    let carrier = calls[0];
    let open = done_call(&events[carrier], kind).is_some_and(|td| td.fold_open);
    Some(ToolFold { carrier, last: *calls.last()?, n: calls.len(), calls, total, open })
}

/// The fold of the run that holds `i`, in any view, when it has
/// [`fold_at`] done calls or more. Failed and running calls keep their
/// own rows; nothing moves. A thinking between calls: the fold it sits
/// inside, if any.
pub(crate) fn tool_fold(events: &[Ev], i: usize, debug: bool) -> Option<ToolFold> {
    let kinds = kinds_of(events.get(i)?);
    if let [kind] = kinds {
        return fold_of(events, i, debug, *kind);
    }
    kinds
        .iter()
        .filter_map(|&k| fold_of(events, i, debug, k))
        .find(|f| (f.carrier..=f.last).contains(&i))
}

/// The patches of a fold's edits, decoded, in order.
fn fold_patches(events: &[Ev], f: &ToolFold) -> Vec<String> {
    f.calls
        .iter()
        .filter_map(|&j| match &events[j] {
            Ev::Tool(td) => td.code.as_deref().map(|raw| crate::code::tool_source(crate::code::CodeLang::Patch, wire_decode(raw))),
            _ => None,
        })
        .collect()
}

/// Hidden by a closed fold: a done call after its first, or the
/// thinking between its calls.
fn folded_away(events: &[Ev], i: usize, debug: bool) -> bool {
    let ev = &events[i];
    let done = kinds_of(ev).iter().any(|&k| done_call(ev, k).is_some());
    if !done && !matches!(ev, Ev::Thinking { .. }) {
        return false;
    }
    tool_fold(events, i, debug).is_some_and(|f| !f.open && i > f.carrier && i <= f.last)
}

/// Open or close the fold whose first call is `i`.
fn toggle_tool_fold(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    if let Some(Ev::Tool(td)) = events.get_mut(i) {
        td.fold_open = !td.fold_open;
    }
    forget_work_run(events, cache, i);
    true
}

/// The rows of a run of work depend on each other (the fold): a change
/// to one call rebuilds them all.
fn forget_work_run(events: &[Ev], cache: &mut [Option<EventRows>], i: usize) {
    for &kind in kinds_of(&events[i]) {
        let (a, b) = work_run(events, i, false, kind);
        forget(cache, a..=b);
    }
}
