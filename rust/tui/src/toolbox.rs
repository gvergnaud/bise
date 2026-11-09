//! The bash / TypeScript box (book §11 "Scripts: a box", BISE-96): a
//! rounded box the width of the code measure, its title in the top border
//! (`╭─ $ bash ∿ 12s ───╮`), the script, a faint rule, then the output,
//! dim. Closed, the inside is 15 rows at most (BISE-123), its last row
//! `▸ n more lines` when lines are hidden. Other tools stay one line.

use crate::code::*;
use crate::render::{elapsed_label, fmt_elapsed};
use crate::theme::{self, *};
use crate::wire::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// The rows inside a closed box, script + rule + output + marker
/// (BISE-123): a longer box shows `▸ n more lines` on its last row.
pub(crate) const BOX_ROWS: usize = 15;
/// The script's share of a closed box whose output needs the room.
const SCRIPT_ROWS: usize = 5;

/// A tool drawn as a box: bash and TypeScript.
pub(crate) fn is_boxed(td: &ToolData) -> bool {
    matches!(td.name.as_deref(), Some("bash") | Some("run_typescript"))
}

/// A tool drawn as a row that opens into a box (click, ctrl+o): the
/// boxed calls, and a skill call (BISE-283: `read skill <name>`, its
/// SKILL.md in the box). Only bash and TypeScript fold into `▸ n
/// commands` and carry sub-calls.
pub(crate) fn opens_as_box(td: &ToolData) -> bool {
    is_boxed(td) || crate::toolrow::is_skill(td)
}

/// A sub-call of a TypeScript run, as the box shows it.
pub(crate) struct SubCall<'a> {
    pub(crate) name: &'a str,
    pub(crate) ok: bool,
    pub(crate) preview: &'a str,
    // the connector account it used: ` · perso` after the name
    pub(crate) account: Option<&'a str>,
}

// corners, lines and joints: rounded, or plain ASCII (book §11)
pub(crate) struct Frame {
    pub(crate) tl: &'static str,
    pub(crate) tr: &'static str,
    pub(crate) bl: &'static str,
    pub(crate) br: &'static str,
    pub(crate) h: &'static str,
    v: &'static str,
    lj: &'static str,
    rj: &'static str,
}

/// The box's corners and lines (also the markdown code blocks', codeblock.rs).
pub(crate) fn frame() -> Frame {
    if theme::ascii_mode() {
        Frame { tl: "+", tr: "+", bl: "+", br: "+", h: "-", v: "|", lj: "+", rj: "+" }
    } else {
        Frame { tl: "╭", tr: "╮", bl: "╰", br: "╯", h: "─", v: "│", lj: "├", rj: "┤" }
    }
}

/// The border: text while running (not the accent), faint when done,
/// error when failed.
fn border(td: &ToolData) -> Color {
    match td.state {
        ToolState::Run => text(),
        ToolState::Ok => faint(),
        ToolState::Fail => error(),
    }
}

/// The result text lines of the box (images aside: their chip is a line).
fn result_lines(td: &ToolData) -> (Option<Vec<Span<'static>>>, Vec<String>) {
    let Some((_, preview)) = &td.result else { return (None, Vec::new()) };
    let images = crate::attach::result_spans(preview);
    let shown = if images.is_some() { crate::attach::without_markers(preview) } else { preview.clone() };
    let t = shown.trim_end();
    let lines = if t.trim().is_empty() { Vec::new() } else { t.lines().map(str::to_string).collect() };
    (images, lines)
}

/// The closed box hides lines (`▸` / ctrl+o / click open it): as last
/// drawn (the rows depend on the width and the sub-calls), or before any
/// draw, a result that alone passes the cap (its rows + the rule).
pub(crate) fn box_folds(td: &ToolData) -> bool {
    td.clips.get() || result_lines(td).1.len() >= BOX_ROWS
}

/// The top border with the title, the only row a running box redraws.
pub(crate) fn box_top(td: &ToolData, tick: u32, width: usize) -> Line<'static> {
    let f = frame();
    let bst = Style::default().fg(border(td));
    // BISE-223: the model's description is the title (`$ weighing the
    // hero image`), cut before the state; else `$ bash` / `ƒ typescript`
    let title = crate::toolrow::box_title(td);
    let mut spans = vec![
        Span::styled(format!("{}{} ", f.tl, f.h), bst),
        Span::styled(title, Style::default().fg(text())),
    ];
    let dim_st = Style::default().fg(dim());
    match td.state {
        _ if td.paused => {
            spans.push(Span::raw(" "));
            spans.extend(crate::toolrow::state_spans(td, tick));
        }
        ToolState::Run => {
            let (p, c) = working_frame(tick);
            spans.push(Span::styled(format!(" {}", p), Style::default().fg(c)));
            spans.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            let ok = if theme::ascii_mode() { "ok" } else { G_RECEIVED };
            spans.push(Span::styled(format!(" {}{}", ok, elapsed_label(&td.elapsed)), dim_st));
        }
        ToolState::Fail => {
            // `✗ exit 1 · 0.8s` (BISE-223: the row's state)
            spans.push(Span::raw(" "));
            spans.extend(crate::toolrow::state_spans(td, tick));
        }
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    // a long title is cut with `…`: the state and one fill cell stay
    if used + 3 > width {
        let over = used + 3 - width;
        let t = spans[1].content.to_string();
        let room = t.width().saturating_sub(over);
        spans[1].content = crate::render::fit_chars(&t, room).into();
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let fill = width.saturating_sub(used + 2);
    spans.push(Span::styled(format!(" {}{}", f.h.repeat(fill), f.tr), bst));
    Line::from(spans)
}

/// One inner row: `│ content … │`, padded to the box.
fn inner_row(content: Vec<Span<'static>>, inner: usize, bst: Style, soft: bool) -> Line<'static> {
    let f = frame();
    let used: usize = content.iter().map(|s| s.content.width()).sum();
    let mut spans = vec![Span::styled(format!("{} ", f.v), bst)];
    spans.extend(content);
    spans.push(Span::raw(" ".repeat(inner.saturating_sub(used))));
    spans.push(Span::styled(format!(" {}", f.v), bst));
    let mut l = Line::from(spans);
    if soft {
        crate::feedsel::mark_soft(&mut l);
    }
    l
}

/// One styled logical line wrapped inside the box (a hanging `»` on the
/// continuation rows, like the code blocks).
fn wrap_one(spans: &[Span<'static>], inner: usize, bst: Style) -> Vec<Line<'static>> {
    let hang = Span::styled(format!("{} ", glyph(G_WRAP)), Style::default().fg(faint()));
    wrap_one_with(spans, inner, bst, hang)
}

/// [`wrap_one`] with `hang` in front of each continuation row (a
/// markdown code block in ASCII: two blanks, codeblock.rs).
pub(crate) fn wrap_one_with(spans: &[Span<'static>], inner: usize, bst: Style, hang: Span<'static>) -> Vec<Line<'static>> {
    if spans.iter().all(|s| s.content.is_empty()) {
        return vec![inner_row(Vec::new(), inner, bst, false)];
    }
    let rest = inner.saturating_sub(hang.content.width()).max(4);
    wrap_code_line_hanging(spans, inner.max(4), rest)
        .into_iter()
        .enumerate()
        .map(|(r, (content, _))| {
            let mut c = Vec::new();
            if r > 0 {
                c.push(hang.clone());
            }
            c.extend(content);
            inner_row(c, inner, bst, r > 0)
        })
        .collect()
}

/// Logical lines wrapped on demand: a closed box wraps only the rows it
/// counts or shows, never a long output in full.
struct Wrapped<'a> {
    lines: &'a [Vec<Span<'static>>],
    rows: Vec<Option<Vec<Line<'static>>>>,
    inner: usize,
    bst: Style,
}

impl<'a> Wrapped<'a> {
    fn new(lines: &'a [Vec<Span<'static>>], inner: usize, bst: Style) -> Self {
        Wrapped { lines, rows: vec![None; lines.len()], inner, bst }
    }

    fn get(&mut self, i: usize) -> &Vec<Line<'static>> {
        let (lines, inner, bst) = (self.lines, self.inner, self.bst);
        self.rows[i].get_or_insert_with(|| wrap_one(&lines[i], inner, bst))
    }

    /// The rows of all the lines, counted up to `cap` (then `cap + 1`).
    fn count_upto(&mut self, cap: usize) -> usize {
        let mut n = 0;
        for i in 0..self.lines.len() {
            n += self.get(i).len();
            if n > cap {
                return cap + 1;
            }
        }
        n
    }

    fn all(mut self) -> Vec<Line<'static>> {
        (0..self.lines.len())
            .flat_map(|i| {
                self.get(i);
                self.rows[i].take().unwrap_or_default()
            })
            .collect()
    }

    /// Whole lines from the start (or the end) within `budget` rows, and
    /// how many lines show whole. When not even one fits, the first (the
    /// last) line cut to the budget, not counted as shown.
    fn pick(&mut self, budget: usize, from_end: bool) -> (Vec<Line<'static>>, usize) {
        let n = self.lines.len();
        let (mut used, mut k) = (0, 0);
        while k < n {
            let i = if from_end { n - 1 - k } else { k };
            let h = self.get(i).len();
            if used + h > budget {
                break;
            }
            used += h;
            k += 1;
        }
        if k == 0 && n > 0 && budget > 0 {
            let rows = self.get(if from_end { n - 1 } else { 0 });
            let cut = if from_end { rows[rows.len() - budget..].to_vec() } else { rows[..budget].to_vec() };
            return (cut, 0);
        }
        let idx: Vec<usize> = if from_end { (n - k..n).collect() } else { (0..k).collect() };
        (idx.into_iter().flat_map(|i| self.get(i).clone()).collect(), k)
    }
}

/// The whole box: top border, script, rule, output, bottom border.
/// `subs` are the TypeScript sub-calls (before the result). Closed, the
/// inside (script, rule, output) is BOX_ROWS rows at most, the last one
/// `▸ n more lines` when lines are hidden (see [`split`]).
pub(crate) fn box_lines(
    td: &ToolData,
    code: &Option<(CodeLang, String)>,
    subs: &[SubCall],
    tick: u32,
    width: usize,
) -> Vec<Line<'static>> {
    let f = frame();
    let width = width.max(12);
    let inner = width - 4;
    let bst = Style::default().fg(border(td));
    let dim_st = Style::default().fg(dim());
    let faint_st = Style::default().fg(faint());
    let mut out = vec![box_top(td, tick, width)];
    let script: Vec<Vec<Span<'static>>> = match code {
        Some((CodeLang::Bash, src)) => highlight_bash(src),
        Some((CodeLang::TypeScript, src)) => highlight_ts(src),
        // a skill's args are its name, already in the title
        _ if crate::toolrow::is_skill(td) => Vec::new(),
        _ => td
            .args
            .as_deref()
            .map(|a| vec![vec![Span::styled(crate::render::args_preview(td.name.as_deref().unwrap_or(""), a), dim_st)]])
            .unwrap_or_default(),
    };
    // the output: sub-calls, the image chip, then the result
    let mut shown: Vec<Vec<Span<'static>>> = subs
        .iter()
        .map(|s| {
            let mut l = vec![Span::styled(format!("{} {}{}", glyph(G_SUBCALL), s.name, crate::render::account_suffix(s.account)), dim_st)];
            if s.ok {
                l.push(Span::styled(format!(" {}", glyph(G_RECEIVED)), dim_st));
            } else {
                let why = crate::render::fit_chars(s.preview.trim(), 80);
                l.push(Span::styled(format!(" {} {}", glyph(G_FAILED), why).trim_end().to_string(), Style::default().fg(error())));
            }
            l
        })
        .collect();
    let (images, lines) = result_lines(td);
    if let Some(chip) = images {
        shown.push(chip);
    }
    // BISE-290: its urls and file paths are links
    shown.extend(lines.iter().map(|t| crate::textlayer::spans(t, dim_st)));
    let rule = || Line::from(Span::styled(format!("{}{}{}", f.lj, f.h.repeat(width - 2), f.rj), faint_st));
    let mut sw = Wrapped::new(&script, inner, bst);
    let mut ow = Wrapped::new(&shown, inner, bst);
    let s_rows = sw.count_upto(BOX_ROWS);
    let o_rows = ow.count_upto(BOX_ROWS);
    let whole = s_rows + if shown.is_empty() { 0 } else { 1 + o_rows };
    td.clips.set(whole > BOX_ROWS);
    if td.expanded || whole <= BOX_ROWS {
        out.extend(sw.all());
        if !shown.is_empty() {
            // no script (a skill), no rule under the title
            if !script.is_empty() {
                out.push(rule());
            }
            out.extend(ow.all());
        }
    } else {
        let (s_budget, o_budget) = split(s_rows, if shown.is_empty() { None } else { Some(o_rows) });
        let (s_part, s_whole) = sw.pick(s_budget, false);
        let o_budget = o_budget.map(|b| b + s_budget - s_part.len());
        out.extend(s_part);
        let mut hidden = script.len() - s_whole;
        if let Some(b) = o_budget {
            // done: from the top, the rest is below; running or failed:
            // the latest lines, where the news (or the error) is
            let (o_part, o_whole) = ow.pick(b, !matches!(td.state, ToolState::Ok));
            if !script.is_empty() {
                out.push(rule());
            }
            out.extend(o_part);
            hidden += shown.len() - o_whole;
        }
        out.push(inner_row(vec![Span::styled(more_label(hidden), dim_st)], inner, bst, false));
    }
    out.push(Line::from(Span::styled(format!("{}{}{}", f.bl, f.h.repeat(width - 2), f.br), bst)));
    out
}

/// The rows of a closed box that does not fit, from the rows its script
/// and its output (None: no output, no rule) would take: the script
/// gets up to SCRIPT_ROWS (more when the output is short), the rule 1,
/// the output the rest, the marker the last row. Script + rule + output
/// + marker is BOX_ROWS.
fn split(s_rows: usize, o_rows: Option<usize>) -> (usize, Option<usize>) {
    let room = BOX_ROWS - 1;
    match o_rows {
        None => (s_rows.min(room), None),
        Some(o) => {
            let s = s_rows.min(SCRIPT_ROWS.max((room - 1).saturating_sub(o)));
            (s, Some(room - 1 - s))
        }
    }
}

/// The marker on the last row of a closed box: `▸ n more lines`.
pub(crate) fn more_label(n: usize) -> String {
    let mark = if theme::ascii_mode() { ">" } else { G_CLOSED };
    format!("{} {}", mark, bise_proto::words::common::more_lines(n))
}

// ---- a box that only sends a message (BISE-110, book §9) ----

/// A shell token of a script this module reads: a word (quotes removed)
/// or `&&`.
#[derive(PartialEq)]
enum Tok {
    Word(String),
    And,
}

/// The tokens of a script made of plain words and `&&`: None when it
/// holds anything else a shell would act on (`;`, `|`, `&`, a newline,
/// a redirection, `$(`, a backtick, a glob, a group, a comment, an open
/// quote).
fn plain_tokens(src: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = src.trim().chars().collect();
    let mut toks = Vec::new();
    let mut word: Option<String> = None;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            ' ' | '\t' => {
                toks.extend(word.take().map(Tok::Word));
            }
            '&' if cs.get(i + 1) == Some(&'&') => {
                toks.extend(word.take().map(Tok::Word));
                toks.push(Tok::And);
                i += 1;
            }
            '\\' => {
                match *cs.get(i + 1)? {
                    // a line continuation
                    '\n' => toks.extend(word.take().map(Tok::Word)),
                    n => word.get_or_insert_with(String::new).push(n),
                }
                i += 1;
            }
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                let end = cs[i + 1..].iter().position(|&q| q == '\'')?;
                w.extend(&cs[i + 1..i + 1 + end]);
                i += end + 1;
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                i += 1;
                loop {
                    match *cs.get(i)? {
                        '"' => break,
                        '`' => return None,
                        '$' if cs.get(i + 1) == Some(&'(') => return None,
                        '\\' => {
                            let n = *cs.get(i + 1)?;
                            if !matches!(n, '"' | '\\' | '$' | '`' | '\n') {
                                w.push('\\');
                            }
                            if n != '\n' {
                                w.push(n);
                            }
                            i += 1;
                        }
                        q => w.push(q),
                    }
                    i += 1;
                }
            }
            '$' if cs.get(i + 1) == Some(&'(') => return None,
            ';' | '|' | '&' | '<' | '>' | '(' | ')' | '`' | '\n' | '\r' | '{' | '}' | '*' | '?' | '[' => return None,
            '#' if word.is_none() => return None,
            c => word.get_or_insert_with(String::new).push(c),
        }
        i += 1;
    }
    toks.extend(word.take().map(Tok::Word));
    Some(toks)
}

/// The `sb` command of a script that is only one `sb send|ask|report`
/// (any flags), after one optional `cd <dir> &&`.
fn lone_sb_send(src: &str) -> Option<&'static str> {
    let toks = plain_tokens(src)?;
    let word = |t: &Tok, w: &str| matches!(t, Tok::Word(x) if x == w);
    let rest = match toks.as_slice() {
        [cd, Tok::Word(_), Tok::And, rest @ ..] if word(cd, "cd") => rest,
        all => all,
    };
    let [sb, Tok::Word(cmd), args @ ..] = rest else { return None };
    if !word(sb, "sb") || args.contains(&Tok::And) {
        return None;
    }
    ["send", "ask", "report"].into_iter().find(|c| c == cmd)
}

/// The message id at the start of `s` (`m_12`), and what follows it.
fn lead_id(s: &str) -> Option<(&str, &str)> {
    let n = s.strip_prefix("m_")?;
    let d = n.bytes().take_while(u8::is_ascii_digit).count();
    (d > 0).then(|| s.split_at(2 + d))
}

/// BISE-110: the ids of the messages a bash box only sends: its script
/// is one `sb send|ask|report` (after one optional `cd <dir> &&`), it
/// succeeded, and its output names them (`sent m_12 to …`, `reported
/// (m_12)`, an ask's `reply from x (m_13, answers m_12…)`: the question
/// and the reply). Empty for any other box: it always shows.
pub(crate) fn sent_ids(td: &ToolData) -> Vec<String> {
    let none = Vec::new();
    if td.name.as_deref() != Some("bash") || !matches!(td.state, ToolState::Ok) {
        return none;
    }
    let (Some(raw), Some((true, out))) = (&td.code, &td.result) else { return none };
    // the preview is one line (the runtime flattens it): the whole of it
    // must be the command's own words, nothing after. Read first: most
    // outputs are not an sb one, and then the script is never decoded.
    let out = out.trim();
    let kind = ["sent ", "reported (", "reply from "].into_iter().position(|p| out.starts_with(p));
    let Some(kind) = kind else { return none };
    let Some(cmd) = lone_sb_send(&wire_decode(raw)) else { return none };
    if ["send", "report", "ask"][kind] != cmd {
        return none;
    }
    let ids = match cmd {
        // `sent m_12 to docs (delivered, thread t_12)`
        "send" => out
            .strip_prefix("sent ")
            .and_then(lead_id)
            .filter(|(_, r)| {
                let tail = r.strip_prefix(" to ").and_then(|r| r.split_once(" (")).and_then(|(to, st)| Some((to, st.strip_suffix(')')?)));
                tail.is_some_and(|(to, st)| !to.is_empty() && !to.contains(' ') && !st.contains(['(', ')']) && st.contains(", thread t_"))
            })
            .map(|(id, _)| vec![id]),
        "report" => out
            .strip_prefix("reported (")
            .and_then(lead_id)
            .filter(|(_, r)| *r == ")")
            .map(|(id, _)| vec![id]),
        _ => out.strip_prefix("reply from ").and_then(|r| {
            let head = r.split_once("):")?.0;
            let (reply, r) = lead_id(head.rsplit_once(" (")?.1)?;
            let (asked, _) = lead_id(r.strip_prefix(", answers ")?)?;
            Some(vec![asked, reply])
        }),
    };
    ids.map(|v| v.into_iter().map(str::to_string).collect()).unwrap_or(none)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, state: ToolState, out: usize) -> ToolData {
        let mut td = ToolData::bare(1, state);
        td.name = Some(name.into());
        td.code = Some("cargo test".into());
        td.elapsed = Some("0.9s".into());
        if out > 0 {
            let ok = !matches!(td.state, ToolState::Fail);
            td.result = Some((ok, (1..=out).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")));
        }
        td
    }

    fn text(td: &ToolData, subs: &[SubCall]) -> Vec<String> {
        let code = Some((CodeLang::Bash, "cargo test".to_string()));
        box_lines(td, &code, subs, 0, 60)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect()
    }

    fn inner(rows: &[String]) -> Vec<String> {
        rows.iter().map(|r| r.trim_start_matches("│ ").trim_end_matches([' ', '│']).to_string()).collect()
    }

    /// Rounded, the title in the top border, every row the box's width.
    #[test]
    fn a_box_has_its_title_in_the_border() {
        let rows = text(&tool("bash", ToolState::Ok, 3), &[]);
        assert!(rows[0].starts_with("╭─ $ bash ✓ 0.9s ─") && rows[0].ends_with('╮'), "{rows:#?}");
        assert!(rows[1].starts_with("│ cargo test"), "{rows:#?}");
        assert!(rows[2].starts_with('├') && rows.last().unwrap().starts_with('╰'), "{rows:#?}");
        assert!(rows.iter().all(|r| r.width() == 60), "{rows:#?}");
        assert!(!box_folds(&tool("bash", ToolState::Ok, 14)));
        assert!(box_folds(&tool("bash", ToolState::Ok, 15)));
    }

    fn text_of(td: &ToolData, script: &str, width: usize) -> Vec<String> {
        let code = Some((CodeLang::Bash, script.to_string()));
        let rows: Vec<String> = box_lines(td, &code, &[], 0, width)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect();
        inner(&rows)
    }

    fn script(n: usize) -> String {
        (1..=n).map(|i| format!("echo {i}")).collect::<Vec<_>>().join("\n")
    }

    /// The inside rows (script, rule, output, marker), borders aside.
    fn inside(rows: &[String]) -> &[String] {
        &rows[1..rows.len() - 1]
    }

    /// Done, long output: the script, the rule, the first lines, and
    /// `▸ n more lines` on the last row; 15 inside rows. Open: all.
    #[test]
    fn a_long_output_keeps_its_head_and_says_the_rest_at_the_bottom() {
        let mut td = tool("bash", ToolState::Ok, 42);
        let rows = text_of(&td, "cargo test", 60);
        let ins = inside(&rows);
        assert_eq!(ins.len(), BOX_ROWS, "{rows:#?}");
        assert_eq!(ins[0], "cargo test");
        assert!(ins[1].starts_with('├'), "{rows:#?}");
        assert_eq!(ins[2], "line 1");
        assert_eq!(ins[13], "line 12");
        assert_eq!(ins[14], "▸ 30 more lines");
        assert!(box_folds(&td));
        td.expanded = true;
        let open = text_of(&td, "cargo test", 60);
        assert_eq!(inside(&open).len(), 1 + 1 + 42, "{open:#?}");
        assert!(open.iter().any(|r| r == "line 42") && !open.iter().any(|r| r.contains("more line")));
        // opened, it still folds: the click closes it again
        assert!(box_folds(&td));
    }

    /// A long script, no output: its first 14 rows and the marker.
    #[test]
    fn a_long_script_alone_is_cut_too() {
        let td = tool("bash", ToolState::Run, 0);
        let rows = text_of(&td, &script(30), 60);
        let ins = inside(&rows);
        assert_eq!(ins.len(), BOX_ROWS, "{rows:#?}");
        assert_eq!(ins[0], "echo 1");
        assert_eq!(ins[13], "echo 14");
        assert_eq!(ins[14], "▸ 16 more lines");
        assert!(box_folds(&td));
    }

    /// A long script and a short output: the output shows whole, the
    /// script takes the rest; one marker counts the script's hidden lines.
    #[test]
    fn a_long_script_leaves_room_for_a_short_output() {
        let td = tool("bash", ToolState::Ok, 2);
        let rows = text_of(&td, &script(30), 60);
        let ins = inside(&rows);
        assert_eq!(ins.len(), BOX_ROWS, "{rows:#?}");
        assert_eq!(ins[10], "echo 11");
        assert!(ins[11].starts_with('├'));
        assert_eq!(&ins[12..14], ["line 1", "line 2"]);
        assert_eq!(ins[14], "▸ 19 more lines");
    }

    /// Both long: 5 script rows, the rule, 8 output rows, the marker
    /// counting both.
    #[test]
    fn both_long_split_five_and_eight() {
        let td = tool("bash", ToolState::Ok, 40);
        let rows = text_of(&td, &script(20), 60);
        let ins = inside(&rows);
        assert_eq!(ins.len(), BOX_ROWS, "{rows:#?}");
        assert_eq!(ins[4], "echo 5");
        assert!(ins[5].starts_with('├'));
        assert_eq!(ins[6], "line 1");
        assert_eq!(ins[13], "line 8");
        assert_eq!(ins[14], "▸ 47 more lines");
    }

    /// Running or failed: the latest output lines, the marker at the
    /// bottom; the border is the error color when it failed.
    #[test]
    fn a_running_or_failed_box_keeps_its_end() {
        for state in [ToolState::Run, ToolState::Fail] {
            let td = tool("bash", state, 40);
            let rows = text_of(&td, &script(8), 60);
            let ins = inside(&rows);
            assert_eq!(ins.len(), BOX_ROWS, "{rows:#?}");
            assert_eq!(ins[4], "echo 5");
            assert_eq!(ins[6], "line 33");
            assert_eq!(ins[13], "line 40");
            assert_eq!(ins[14], "▸ 35 more lines");
        }
        let lines = box_lines(&tool("bash", ToolState::Fail, 1), &None, &[], 0, 40);
        assert_eq!(lines[0].spans[0].style.fg, Some(error()));
        let lines = box_lines(&tool("bash", ToolState::Ok, 1), &None, &[], 0, 40);
        assert_eq!(lines[0].spans[0].style.fg, Some(faint()));
    }

    /// Exactly 15 inside rows: no marker, no fold; one more: the fold.
    /// Rows, not lines: a short output that wraps past the cap folds.
    #[test]
    fn the_cap_counts_wrapped_rows() {
        let td = tool("bash", ToolState::Ok, 13);
        let rows = text_of(&td, "cargo test", 60);
        assert_eq!(inside(&rows).len(), 15);
        assert!(!rows.iter().any(|r| r.contains("more line")) && !box_folds(&td));
        let td = tool("bash", ToolState::Ok, 14);
        let rows = text_of(&td, "cargo test", 60);
        assert_eq!(inside(&rows).len(), 15);
        assert_eq!(inside(&rows)[14], "▸ 2 more lines");
        let mut td = tool("bash", ToolState::Ok, 0);
        td.result = Some((true, "word ".repeat(300)));
        assert!(!box_folds(&td), "not drawn yet: one line");
        let rows = text_of(&td, "cargo test", 60);
        assert_eq!(inside(&rows).len(), 15, "{rows:#?}");
        assert_eq!(inside(&rows)[14], "▸ 1 more line");
        assert!(box_folds(&td), "drawn: it clips");
        // wide enough, nothing hidden
        let rows = text_of(&td, "cargo test", 2000);
        assert!(!rows.iter().any(|r| r.contains("more line")) && !box_folds(&td));
    }

    /// TypeScript sub-calls are output lines inside the box.
    #[test]
    fn sub_calls_are_inside() {
        let td = tool("run_typescript", ToolState::Ok, 1);
        let subs = [
            SubCall { name: "github.search_issues", ok: true, preview: "", account: None },
            SubCall { name: "gmail.search", ok: true, preview: "", account: Some("perso") },
        ];
        let rows = inner(&text(&td, &subs));
        assert!(rows[0].starts_with("╭─ ƒ typescript"), "{rows:#?}");
        let out: Vec<&String> = rows.iter().skip_while(|r| !r.starts_with('├')).skip(1).collect();
        assert_eq!(out[0], "↳ github.search_issues ✓");
        // the account it used follows the name (multi-accounts piece 5)
        assert_eq!(out[1], "↳ gmail.search · perso ✓");
        assert_eq!(out[2], "line 1");
    }

    /// ASCII: `+- $ bash ok 0.9s ---+`, `|`, `> n more lines`.
    #[test]
    fn the_ascii_box() {
        theme::set_ascii_for_tests(true);
        let rows = text(&tool("bash", ToolState::Ok, 20), &[]);
        theme::set_ascii_for_tests(false);
        assert!(rows[0].starts_with("+- $ bash ok 0.9s -") && rows[0].ends_with('+'), "{rows:#?}");
        assert!(rows[1].starts_with("| cargo test") && rows[1].ends_with(" |"));
        assert!(rows.last().is_some_and(|r| r.starts_with("+-")), "{rows:#?}");
        assert!(rows[rows.len() - 2].starts_with("| > 8 more lines"), "{rows:#?}");
        assert!(rows.iter().all(|r| r.is_ascii()), "{rows:#?}");
    }
}
