//! A bash / TypeScript call in a feed (BISE-223, designer's reco):
//! one row, `$` or `ƒ` in the lead column, the model's own description
//! of the call, its state on the right (`∿ 12s`, `✓ 0.3s`, `✗ exit 1 ·
//! 0.8s`). A failed call adds one row: its first error line. 4 or more
//! done calls in a row fold into `▸ 6 commands · <first description>`.
//! A click opens one call's box, ctrl+o every box; main and the agents'
//! views alike (an open box has the description as its title).

use crate::render::{elapsed_label, fit_chars, fmt_duration, fmt_elapsed};
use crate::theme::{self, *};
use crate::wire::*;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// A run of done calls this long folds into one row.
pub(crate) const FOLD_TOOLS: usize = 4;

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// A call drawn as one row: a closed bash / TypeScript / skill call, in
/// any view.
pub(crate) fn row_mode(td: &ToolData) -> bool {
    crate::toolbox::opens_as_box(td) && !td.opened
}

/// A skill call (BISE-283): a sentence, never the tool's JSON.
pub(crate) fn is_skill(td: &ToolData) -> bool {
    td.name.as_deref() == Some("skill")
}

/// The skill a call reads: its `name` (the `arg` of an older schema).
fn skill_name(td: &ToolData) -> Option<String> {
    let a = td.args.as_deref()?;
    crate::render::json_str_field(a, "name")
        .or_else(|| crate::render::json_str_field(a, "arg"))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
}

/// What a skill row says: `reading skill <name>` while it runs (the
/// verb dim, the name in the text color), `read skill <name>` once done
/// or failed, all dim (designer, BISE-283).
fn skill_spans(td: &ToolData) -> Vec<Span<'static>> {
    let run = matches!(td.state, ToolState::Run);
    let dim_st = Style::default().fg(dim());
    let verb = if run { "reading skill" } else { "read skill" };
    match skill_name(td) {
        Some(n) => vec![
            Span::styled(format!("{} ", verb), dim_st),
            Span::styled(n, Style::default().fg(if run { text() } else { dim() })),
        ],
        None => vec![Span::styled(verb.to_string(), dim_st)],
    }
}

/// Why a skill call failed, short: the runtime's reason up to its first
/// `:` (`unknown skill: x: call it with…` → `unknown skill`).
fn skill_reason(td: &ToolData) -> String {
    let r = td.result.as_ref().map(|(_, r)| r.trim()).unwrap_or("");
    let head = r.split(':').next().unwrap_or("").trim();
    // the usage error starts `skill: call exactly with…`
    let head = if head == "skill" || head.is_empty() { "failed" } else { head };
    fit_chars(head, 40)
}

/// The state on the right of a skill row: the pulse and time while it
/// runs, nothing once read (it takes no time worth a `✓ 0.0s`), `✗
/// <reason>` in the error color when it failed.
fn skill_state(td: &ToolData, tick: u32) -> Vec<Span<'static>> {
    match td.state {
        ToolState::Run => state_spans(td, tick),
        ToolState::Ok => Vec::new(),
        ToolState::Fail => vec![Span::styled(format!("{} {}", glyph(G_FAILED), skill_reason(td)), Style::default().fg(error()))],
    }
}

/// The kind glyph: `$` bash, `ƒ` TypeScript (`f` in ASCII).
pub(crate) fn kind_glyph(td: &ToolData) -> &'static str {
    if td.name.as_deref() == Some("bash") {
        glyph(G_BASH)
    } else {
        glyph(G_TS)
    }
}

/// `exit 1` from a failed bash result (`exit 1: <output>`).
pub(crate) fn exit_code(td: &ToolData) -> Option<String> {
    let (_, r) = td.result.as_ref()?;
    bise_proto::thread::lines::exit_code(r).map(|code| format!("exit {}", code))
}

/// The state on the right of a row, and its style.
pub(crate) fn state_spans(td: &ToolData, tick: u32) -> Vec<Span<'static>> {
    let dim_st = Style::default().fg(dim());
    match td.state {
        // approvals-design.md §3.1, §10: the gate holds it
        ToolState::Run if matches!(td.gate, Some((crate::wire::Gate::Card, _))) => {
            vec![Span::styled("? ", Style::default().fg(accent())), Span::styled("waiting for you", dim_st)]
        }
        ToolState::Run if td.gate.is_some_and(|(g, t)| g == crate::wire::Gate::Check && t.elapsed().as_millis() >= 250) => {
            vec![Span::styled(format!("checking{}", theme::ellipsis()), dim_st)]
        }
        ToolState::Run => {
            let (g, c) = working_frame(tick);
            let c = if no_color() { text() } else { c };
            vec![Span::styled(g.to_string(), Style::default().fg(c)), Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st)]
        }
        ToolState::Ok => {
            let ok = if theme::ascii_mode() { "ok" } else { G_RECEIVED };
            vec![Span::styled(format!("{}{}", ok, elapsed_label(&td.elapsed)), dim_st)]
        }
        ToolState::Fail => {
            let t = match (exit_code(td), td.elapsed.as_deref()) {
                (Some(x), Some(e)) if !e.is_empty() => format!("{} {} · {}", glyph(G_FAILED), x, e),
                (Some(x), _) => format!("{} {}", glyph(G_FAILED), x),
                (None, _) => format!("{}{}", glyph(G_FAILED), elapsed_label(&td.elapsed)),
            };
            vec![Span::styled(t, Style::default().fg(error()))]
        }
    }
}

/// The first non-empty line of the call's script, highlighted.
fn first_code_line(td: &ToolData) -> Option<Vec<Span<'static>>> {
    let (_, _, code) = crate::render::tool_meta(td);
    let (lang, src) = code?;
    let lines = match lang {
        crate::code::CodeLang::Bash => crate::code::highlight_bash(&src),
        _ => crate::code::highlight_ts(&src),
    };
    lines.into_iter().find(|l| l.iter().any(|s| !s.content.trim().is_empty())).map(|l| {
        // the lead spaces go: the row has its own column
        let mut l = l;
        if let Some(s) = l.first_mut() {
            s.content = s.content.trim_start().to_string().into();
        }
        l
    })
}

/// What the row says: the model's description; without one, the
/// script's first line in its code colors; else the tool's kind.
fn desc_spans(td: &ToolData) -> Vec<Span<'static>> {
    let st = Style::default().fg(if matches!(td.state, ToolState::Run) { text() } else { dim() });
    if let Some(d) = td.intent.as_deref().filter(|d| !d.trim().is_empty()) {
        // a done row's paths are links (BISE-264); a running row is
        // redrawn each frame, outside its event's links
        if matches!(td.state, ToolState::Run) {
            return vec![Span::styled(d.trim().to_string(), st)];
        }
        return crate::file_links::plain_spans(d.trim(), st);
    }
    if let Some(l) = first_code_line(td) {
        return l;
    }
    let label = if td.name.as_deref() == Some("bash") { "bash" } else { "typescript" };
    vec![Span::styled(label.to_string(), st)]
}

/// Spans cut to `room` cells, `…` at the cut.
fn fit_spans(spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(|s| s.content.width()).sum();
    if total <= room {
        return spans;
    }
    let e = ellipsis();
    let mut left = room.saturating_sub(e.width());
    let mut out = Vec::new();
    let mut last = Style::default();
    for s in spans {
        if left == 0 {
            break;
        }
        last = s.style;
        let mut take = String::new();
        for c in s.content.chars() {
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if w > left {
                left = 0;
                break;
            }
            left -= w;
            take.push(c);
        }
        out.push(Span::styled(take, s.style));
    }
    out.push(Span::styled(e.to_string(), last));
    out
}

/// A row: the lead column (3 cells), the text cut before the state,
/// the state right-aligned at `width`.
fn row_of(lead: Span<'static>, text: Vec<Span<'static>>, state: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let state_w: usize = state.iter().map(|s| s.content.width()).sum();
    let room = width.saturating_sub(3 + 1 + state_w);
    let text = fit_spans(text, room);
    let used: usize = text.iter().map(|s| s.content.width()).sum();
    let mut spans = vec![lead];
    spans.extend(text);
    spans.push(Span::raw(" ".repeat(width.saturating_sub(3 + used + state_w).max(1))));
    spans.extend(state);
    Line::from(spans)
}

fn lead_span(td: &ToolData) -> Span<'static> {
    let c = if matches!(td.state, ToolState::Fail) { error() } else { dim() };
    Span::styled(format!(" {} ", kind_glyph(td)), Style::default().fg(c))
}

/// The row of one call.
pub(crate) fn row_line(td: &ToolData, tick: u32, width: usize) -> Line<'static> {
    if is_skill(td) {
        // no glyph: a skill read is quiet plumbing (designer, BISE-283)
        return row_of(Span::raw("   "), skill_spans(td), skill_state(td, tick), width);
    }
    row_of(lead_span(td), desc_spans(td), state_spans(td, tick), width)
}

/// The first error line of a failed call's output. The runtime sends
/// the output as one line (its newlines as spaces): the text from the
/// first word that reads like an error (`UnicodeDecodeError: …`), else
/// all of it.
pub(crate) fn error_line(td: &ToolData) -> Option<String> {
    if !matches!(td.state, ToolState::Fail) {
        return None;
    }
    // bise_proto's reading, the one the hub's typed tool items carry
    let (_, r) = td.result.as_ref()?;
    bise_proto::thread::lines::error_line(r)
}

/// The row under a failed call: its first error line, in the error
/// color, from column 3, cut with `…` (NO_COLOR: `✗ ` leads it).
pub(crate) fn error_row(td: &ToolData, width: usize) -> Option<Line<'static>> {
    // a skill's reason is on its row
    if is_skill(td) {
        return None;
    }
    let line = error_line(td)?;
    let lead = if no_color() { format!("{} ", glyph(G_FAILED)) } else { String::new() };
    let room = width.saturating_sub(3 + lead.width());
    // its paths are links (BISE-264): `src/x.rs:12:5` opens there
    let mut spans = vec![Span::raw("   ")];
    spans.extend(crate::file_links::plain_spans(&format!("{}{}", lead, fit_chars(&line, room)), Style::default().fg(error())));
    Some(Line::from(spans))
}

/// The rows of one call: its row, and the error row of a failure.
pub(crate) fn rows(td: &ToolData, tick: u32, width: usize) -> Vec<Line<'static>> {
    let mut out = vec![row_line(td, tick, width)];
    out.extend(error_row(td, width));
    out
}

/// The fold of a run of done calls: `$ ▸ 6 commands · <first
/// description>  ✓ 3.2s` (the total time).
pub(crate) fn fold_row(first: &ToolData, n: usize, total: std::time::Duration, open: bool, width: usize) -> Line<'static> {
    let dim_st = Style::default().fg(dim());
    let mark = if theme::ascii_mode() {
        if open { "v" } else { ">" }
    } else if open {
        G_OPEN
    } else {
        G_CLOSED
    };
    let mut text = vec![Span::styled(format!("{} {} commands · ", mark, n), dim_st)];
    text.extend(desc_spans(first).into_iter().map(|s| Span::styled(s.content, dim_st)));
    let ok = if theme::ascii_mode() { "ok" } else { G_RECEIVED };
    let state = vec![Span::styled(format!("{} {}", ok, fmt_duration(total)), dim_st)];
    row_of(Span::styled(format!(" {} ", kind_glyph(first)), dim_st), text, state, width)
}

/// A run of done edits this long folds into one row (BISE-304).
pub(crate) const FOLD_EDITS: usize = 3;

/// The files of a run of edits, each once, in run order, with their
/// lines added and removed over every call.
pub(crate) fn edit_files(patches: &[String]) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    for src in patches {
        for (p, a, d) in crate::code::patch_files(src) {
            match out.iter_mut().find(|f| f.0 == p) {
                Some(f) => (f.1, f.2) = (f.1 + a, f.2 + d),
                None => out.push((p, a, d)),
            }
        }
    }
    out
}

/// What the fold names each file: its basename, with its parent when
/// another file of the run has the same basename (`auth/index.ts`,
/// `ui/index.ts`).
fn short_names(paths: &[&str]) -> Vec<String> {
    let base = |p: &str| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p).to_string();
    let tail2 = |p: &str| {
        let parts: Vec<&str> = p.trim_end_matches('/').rsplit('/').take(2).collect();
        parts.into_iter().rev().collect::<Vec<_>>().join("/")
    };
    paths
        .iter()
        .map(|p| {
            let b = base(p);
            if paths.iter().filter(|q| base(q) == b).count() > 1 {
                tail2(p)
            } else {
                b
            }
        })
        .collect()
}

/// The names that fit in `room` cells: whole names, then `+n more`; a
/// name is cut (`…`) only when not even one fits.
fn names_in(names: &[String], room: usize) -> String {
    for k in (1..=names.len()).rev() {
        let more = names.len() - k;
        let mut s = names[..k].join(", ");
        if more > 0 {
            s.push_str(&format!(" +{} more", more));
        }
        if s.width() <= room {
            return s;
        }
    }
    let more = names.len().saturating_sub(1);
    let tail = if more > 0 { format!(" +{} more", more) } else { String::new() };
    let first = names.first().map(String::as_str).unwrap_or("");
    format!("{}{}", fit_chars(first, room.saturating_sub(tail.width())), tail)
}

/// The fold of a run of done edits (BISE-304, designer): `± ▸ 3 files ·
/// session.ts, token.ts, README.md   ✓ +7 −2`; one file edited several
/// times counts the calls, `± ▸ 4 edits · session.ts   ✓ +9 −3`. All dim,
/// like the commands fold.
pub(crate) fn edit_fold_row(files: &[(String, usize, usize)], calls: usize, open: bool, width: usize) -> Line<'static> {
    let dim_st = Style::default().fg(dim());
    let ascii = theme::ascii_mode();
    let mark = match (ascii, open) {
        (true, true) => "v",
        (true, false) => ">",
        (false, true) => G_OPEN,
        (false, false) => G_CLOSED,
    };
    let head = if files.len() == 1 {
        format!("{} {} edits · ", mark, calls)
    } else {
        format!("{} {} files · ", mark, files.len())
    };
    let (adds, dels) = files.iter().fold((0, 0), |(a, d), f| (a + f.1, d + f.2));
    let mut state = (if ascii { "ok" } else { G_RECEIVED }).to_string();
    if adds > 0 {
        state.push_str(&format!(" +{}", adds));
    }
    if dels > 0 {
        state.push_str(&format!(" {}{}", if ascii { "-" } else { "−" }, dels));
    }
    let paths: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    let room = width.saturating_sub(3 + 1 + state.width() + head.width());
    let text = vec![Span::styled(head, dim_st), Span::styled(names_in(&short_names(&paths), room), dim_st)];
    row_of(Span::styled(format!(" {} ", glyph(G_PATCH)), dim_st), text, vec![Span::styled(state, dim_st)], width)
}

/// The title of a box: the kind and the description (`$ weighing the
/// hero image`); without one, the kind's word (`$ bash`, `ƒ typescript`).
pub(crate) fn box_title(td: &ToolData) -> String {
    if is_skill(td) {
        return skill_spans(td).into_iter().map(|s| s.content.into_owned()).collect();
    }
    match td.intent.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => format!("{} {}", kind_glyph(td), d),
        None => {
            let label = if td.name.as_deref() == Some("bash") { "bash" } else { "typescript" };
            format!("{} {}", kind_glyph(td), label)
        }
    }
}
