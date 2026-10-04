//! The frame of the Switchboard screen (book §8 "The frame", BISE-98):
//! the faint rounded border with `bise :*` and the summary in its top
//! edge, the panel's rule joined to it, and the divider `├─ you → main
//! ─…─ state ─┤` over the composer pane. Lines only: no background of
//! their own (the theme ground stays, BISE-92).

use crate::gust;
use crate::layout::Cols;
use crate::theme::{self, accent, dim, faint};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// The line pieces: rounded corners, joins, rules; `+ - |` in ASCII.
struct Pieces {
    set: border::Set,
    left_join: &'static str,
    right_join: &'static str,
    top_join: &'static str,
    bottom_join: &'static str,
}

fn pieces() -> Pieces {
    if theme::ascii_mode() {
        Pieces {
            set: border::Set {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                vertical_left: "|",
                vertical_right: "|",
                horizontal_top: "-",
                horizontal_bottom: "-",
            },
            left_join: "+",
            right_join: "+",
            top_join: "+",
            bottom_join: "+",
        }
    } else {
        Pieces { set: border::ROUNDED, left_join: "├", right_join: "┤", top_join: "┬", bottom_join: "┴" }
    }
}

pub(crate) fn line_style() -> Style {
    Style::default().fg(crate::theme::rule())
}

/// Write `spans` from `x` on row `y`, never past `end` (exclusive).
fn put(buf: &mut Buffer, x: u16, y: u16, spans: &[Span], end: u16) {
    let mut x = x;
    for s in spans {
        if x >= end {
            break;
        }
        let (nx, _) = buf.set_stringn(x, y, s.content.as_ref(), usize::from(end - x), s.style);
        x = nx;
    }
}

/// `spans` cut to `room` columns, a `…` at the cut.
pub(crate) fn fit(spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let w: usize = spans.iter().map(|s| s.content.width()).sum();
    if w <= room {
        return spans;
    }
    let ell = theme::ellipsis();
    let mut left = room.saturating_sub(ell.width());
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut last = Style::default();
    for s in spans {
        last = s.style;
        if left == 0 {
            break;
        }
        let mut t = String::new();
        for ch in s.content.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if cw > left {
                left = 0;
                break;
            }
            left -= cw;
            t.push(ch);
        }
        out.push(Span::styled(t, s.style));
    }
    if room >= ell.width() {
        out.push(Span::styled(ell, last));
    }
    out
}

/// The frame on `area`'s edge, framed screens only: the rounded border,
/// the title from column 3 (1 space each side), what `edge` puts after
/// it (the path, the role line: topedge.rs) and the summary ending at
/// F − 4 in the top edge, the panel's rule from row 1 down to the divider
/// (joined `┬` on the top edge, `┴` on the divider, unless text covers
/// the join).
pub(crate) fn draw_frame(
    buf: &mut Buffer,
    area: Rect,
    cols: Cols,
    title: Vec<Span<'static>>,
    edge: impl Fn(usize) -> (Vec<Span<'static>>, Vec<Span<'static>>),
    divider_y: u16,
) {
    let area = area.intersection(buf.area);
    if area.width < 8 || area.height < 3 {
        return;
    }
    let p = pieces();
    let st = line_style();
    let (l, r, t, b) = (area.x, area.right() - 1, area.y, area.bottom() - 1);
    for x in l..=r {
        let (top, bottom) = if x == l {
            (p.set.top_left, p.set.bottom_left)
        } else if x == r {
            (p.set.top_right, p.set.bottom_right)
        } else {
            (p.set.horizontal_top, p.set.horizontal_bottom)
        };
        buf[(x, t)].set_symbol(top).set_style(st);
        buf[(x, b)].set_symbol(bottom).set_style(st);
    }
    for y in t + 1..b {
        buf[(l, y)].set_symbol(p.set.vertical_left).set_style(st);
        buf[(r, y)].set_symbol(p.set.vertical_right).set_style(st);
    }
    // the panel's rule, joined on the top edge (the title may cover it)
    if let Some(x) = cols.panel.and_then(|p| p.rule).map(|x| area.x + x).filter(|x| *x > l && *x < r) {
        for y in t + 1..divider_y.min(b) {
            buf[(x, y)].set_symbol(p.set.vertical_left).set_style(st);
        }
        buf[(x, t)].set_symbol(p.top_join).set_style(st);
    }
    // the title from column 3, 1 space each side; the path and the viewed
    // task's role line right after it (BISE-126)
    let mut head = vec![Span::raw(" ")];
    head.extend(title);
    head.push(Span::raw(" "));
    let head_w: u16 = head.iter().map(|s| s.content.width() as u16).sum();
    let hx = l + cols.margin - 1;
    // the summary: 1 space each side, ending at F − 4; at least 1 rule
    // cell between it and the title
    let end = r - (cols.margin - 1); // exclusive: F - 3 holds its space
    let start = hx + head_w + 2;
    let room = end.saturating_sub(start) as usize;
    let (after, s) = edge(room);
    let w: u16 = s.iter().map(|s| s.content.width() as u16).sum();
    let space = head.pop();
    head.extend(after);
    head.extend(space);
    put(buf, hx, t, &head, r);
    if w > 0 && w as usize <= room {
        let x = end - w;
        put(buf, x - 1, t, &[Span::raw(" ")], end);
        put(buf, x, t, &s, end);
        put(buf, end, t, &[Span::raw(" ")], end + 1);
    }
}

/// What the divider says of the agent you view while it works (book §8,
/// BISE-105, BISE-303): the gust's motion; with ctrl held (`words`),
/// `working` and the current turn's age (`42s`) after it.
pub(crate) struct Working {
    pub(crate) motion: gust::Motion,
    pub(crate) age: Option<String>,
    pub(crate) words: bool,
}

/// What the divider says after the name (BISE-135, BISE-136): the model
/// the agent runs and its reasoning effort (dim, ` · ` faint), the
/// session's approvals mode (`yolo`, dim; accent for 3 s after a switch),
/// then `ψ place` when it does not work in the shared checkout. Short on
/// room, before the state loses anything: the place's name goes (ψ
/// stays), then the long `opus 5.5 · high` becomes the tag `opus·hi`,
/// then the tag goes, then ψ; the mode goes last ([`Who::tails`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct Who {
    /// the long name, `opus 5.5`; "" = not known yet
    pub(crate) model: String,
    /// "" = the model takes none
    pub(crate) effort: String,
    /// the short form, `opus·hi`
    pub(crate) tag: String,
    pub(crate) place: Option<String>,
    /// the approvals mode, `yolo`; "" = the hub has not said yet
    pub(crate) mode: String,
    /// shift+tab switched it less than 3 s ago: the word in accent
    pub(crate) flash: bool,
    /// the others in its worktree (dev-flow §3.1): `ψ sb/dark-mode with i18n`
    pub(crate) with: Vec<String>,
    /// the PR of its worktree's branch (pr-design §4): `↑ #412`
    pub(crate) pr: Option<WhoPr>,
    /// computer use (design §8): what it drives, `↖ Chrome`; ctrl held,
    /// `↖ driving Chrome · amazon.fr` (accent)
    pub(crate) drives: Option<String>,
}

/// The PR in the divider: its number (a link to `url`), `↑`'s look,
/// and, ctrl held, its state in words.
#[derive(Clone, Debug, Default)]
pub(crate) struct WhoPr {
    pub(crate) number: u64,
    pub(crate) url: String,
    pub(crate) style: Style,
    /// ctrl held: `changes asked · checks pass` ([] at rest)
    pub(crate) words: Vec<Span<'static>>,
}

impl Who {
    /// The tails the label may end with, richest first.
    fn tails(&self) -> Vec<Vec<Span<'static>>> {
        let mut out: Vec<Vec<Span<'static>>> = Vec::new();
        for t in self.forms() {
            if out.last().is_none_or(|l| width_of(l) != width_of(&t)) {
                out.push(t);
            }
        }
        out
    }

    /// The short tail, `· opus·hi · yolo · ψ · ↑ #412`: what the label
    /// keeps when the key bar shares the divider.
    fn short_tail(&self) -> Vec<Span<'static>> {
        self.forms().swap_remove(SHORT_FORM)
    }

    /// Every form of the tail, richest first (pr-design §4, short on
    /// room): the PR's held words go, then who else is in the worktree,
    /// the branch's name (ψ stays), the long model becomes the tag, the
    /// tag goes, then ψ, the PR's number (`↑` stays), `↑`; the mode last.
    fn forms(&self) -> Vec<Vec<Span<'static>>> {
        let sep = || Span::styled(" · ", Style::default().fg(faint()));
        let d = |t: &str| Span::styled(t.to_string(), Style::default().fg(dim()));
        let psi = theme::glyph(theme::G_WORKTREE);
        let place = |full: bool, with: bool| -> Vec<Span<'static>> {
            match &self.place {
                Some(p) if full => {
                    let mut v = vec![sep(), d(&format!("{} {}", psi, p))];
                    if with && !self.with.is_empty() {
                        v.push(d(&format!(" with {}", self.with.join(", "))));
                    }
                    v
                }
                Some(_) => vec![sep(), d(psi)],
                None => Vec::new(),
            }
        };
        let pr = |num: bool, words: bool| -> Vec<Span<'static>> {
            let Some(p) = &self.pr else { return Vec::new() };
            let mut v = vec![sep(), Span::styled(theme::pr_glyph(), p.style)];
            if num {
                let label = format!("#{}", p.number);
                v.push(Span::raw(" "));
                v.push(if crate::links::linkable(&p.url) {
                    crate::textlayer::link(label, &p.url, Style::default().fg(dim()))
                } else {
                    d(&label)
                });
            }
            if words && !p.words.is_empty() {
                v.push(Span::raw(" "));
                v.extend(p.words.iter().cloned());
            }
            v
        };
        let long = || -> Vec<Span<'static>> {
            if self.model.is_empty() {
                return Vec::new();
            }
            let mut v = vec![sep(), d(&self.model)];
            if !self.effort.is_empty() {
                v.extend([sep(), d(&self.effort)]);
            }
            v
        };
        let short = || -> Vec<Span<'static>> {
            if self.tag.is_empty() {
                Vec::new()
            } else {
                vec![sep(), d(&self.tag)]
            }
        };
        // approvals-design.md §8: the word that says whether commands ask
        let mode = || -> Vec<Span<'static>> {
            if self.mode.is_empty() {
                return Vec::new();
            }
            let st = match self.flash {
                false => Style::default().fg(dim()),
                true if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) => Style::default().add_modifier(Modifier::BOLD),
                true => Style::default().fg(accent()),
            };
            vec![sep(), Span::styled(self.mode.clone(), st)]
        };
        // it drives Chrome or an app: live, it stays in every form
        let mode = || -> Vec<Span<'static>> {
            let mut v = mode();
            if let Some(d) = &self.drives {
                v.extend([sep(), Span::styled(d.clone(), Style::default().fg(accent()))]);
            }
            v
        };
        vec![
            [long(), mode(), place(true, true), pr(true, true)].concat(),
            [long(), mode(), place(true, true), pr(true, false)].concat(),
            [long(), mode(), place(true, false), pr(true, false)].concat(),
            [long(), mode(), place(false, false), pr(true, false)].concat(),
            [short(), mode(), place(false, false), pr(true, false)].concat(),
            [mode(), place(false, false), pr(true, false)].concat(),
            [mode(), pr(true, false)].concat(),
            [mode(), pr(false, false)].concat(),
            mode(),
        ]
    }
}

/// The index of the short tail in [`Who::forms`].
const SHORT_FORM: usize = 4;

/// How much of the label the divider shows, while the agent works (book
/// §9 "Short on room (the gust)", BISE-303): the steps in the order they
/// are dropped.
#[derive(Clone, Copy)]
struct Step {
    /// `working · 42s` after the gust (ctrl held)
    words: bool,
    size: gust::Size,
    name_cut: Option<usize>,
}

const STEPS: [Step; 6] = {
    use gust::Size::{Five, One, Three};
    let full = Step { words: true, size: Five, name_cut: None };
    [
        full,
        Step { words: false, ..full },
        Step { words: false, size: Three, ..full },
        Step { words: false, size: One, ..full },
        Step { words: false, size: One, name_cut: Some(20) },
        Step { words: false, size: One, name_cut: Some(12) },
    ]
};

/// The divider's label: ` you → name` and the tail (model, effort, mode,
/// ψ); while the agent works, a dim ` · ` and the gust, then, ctrl held,
/// ` working · 42s` dim (as much of it as `step` keeps).
fn label(name: &str, tail: &[Span<'static>], working: Option<(&Working, Step)>) -> Vec<Span<'static>> {
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    let name = match working.and_then(|(_, s)| s.name_cut) {
        Some(n) => fit(vec![Span::raw(name.to_string())], n).into_iter().map(|s| s.content.into_owned()).collect(),
        None => name.to_string(),
    };
    let mut out = vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), Style::default().fg(dim())),
        Span::styled(name, Style::default().fg(accent())),
    ];
    out.extend(tail.iter().cloned());
    if let Some((w, step)) = working {
        out.push(Span::styled(" · ", Style::default().fg(dim())));
        out.extend(gust::mark(w.motion, step.size));
        if step.words && w.words {
            let words = match &w.age {
                Some(age) => format!(" working · {}", age),
                None => " working".to_string(),
            };
            out.push(Span::styled(words, Style::default().fg(dim())));
        }
    }
    out.push(Span::raw(" "));
    out
}

fn width_of(spans: &[Span]) -> u16 {
    spans.iter().map(|s| s.content.width() as u16).sum()
}

/// The columns the divider leaves for its right side on a screen `width`
/// wide: from 1 rule cell and a space after the label to the state's end.
/// While the agent works, the label keeps its 5-cell gust, without the
/// held words. With the key bar in the divider, the label keeps the
/// short tail (`· opus·hi · ψ`).
pub(crate) fn divider_room(width: u16, cols: Cols, name: &str, who: &Who, working: Option<&Working>) -> u16 {
    let tail = who.short_tail();
    let label_w = width_of(&label(name, &tail, working.map(|w| (w, STEPS[1]))));
    let start = cols.margin - 1 + label_w + 2;
    let end = width.saturating_sub(cols.margin);
    end.saturating_sub(start)
}

/// The divider's rule on row `y` (its joins when framed); false when
/// there is no room.
fn divider_rule(buf: &mut Buffer, area: Rect, cols: Cols, y: u16) -> bool {
    if area.width < 4 || y < area.y || y >= area.bottom() {
        return false;
    }
    let p = pieces();
    let st = line_style();
    let (l, r) = (area.x, area.right() - 1);
    for x in l..=r {
        buf[(x, y)].set_symbol(p.set.horizontal_top).set_style(st);
    }
    if cols.framed {
        buf[(l, y)].set_symbol(p.left_join).set_style(st);
        buf[(r, y)].set_symbol(p.right_join).set_style(st);
        if let Some(x) = cols.panel.and_then(|p| p.rule).map(|x| area.x + x).filter(|x| *x > l && *x < r) {
            buf[(x, y)].set_symbol(p.bottom_join).set_style(st);
        }
    }
    true
}

/// The divider with a label of its own and no state (the card view:
/// `you → ? perf · your answer`); returns (no state, the label).
pub(crate) fn draw_divider_label(buf: &mut Buffer, area: Rect, cols: Cols, y: u16, label: Vec<Span<'static>>) -> (Rect, Rect) {
    let area = area.intersection(buf.area);
    if !divider_rule(buf, area, cols, y) {
        return (Rect::default(), Rect::default());
    }
    let (l, r) = (area.x, area.right() - 1);
    let lx = l + cols.margin - 1;
    let end = r + 1 - cols.margin;
    let label = fit(label, (end + 1).saturating_sub(lx) as usize);
    put(buf, lx, y, &label, end + 1);
    let label_rect = Rect { x: lx, y, width: width_of(&label).min((end + 1).saturating_sub(lx)), height: 1 };
    (Rect::default(), label_rect)
}

/// The divider on row `y` (book §8): framed, a rule joining the frame
/// (`├ … ┤`, `┴` under the panel's rule); bare, a plain rule. The label
/// ` you → name · model · effort · mode ` from the margin; while the
/// agent works, ` · ≈∿~·` after it (and ` working · 42s` with ctrl
/// held). The right side, ending 1 column before the right margin's
/// space, 3 columns at least from the label: the first of `states`
/// (richest first, BISE-303: the long context `58k / 262k tokens ·
/// 22%` with ctrl held, then the short `58k · 22%`) that fits. Short on
/// room ([`ladder`]): the held words go, the long state becomes the
/// short one, the tail shrinks ([`Who::tails`]), then the state goes,
/// then the gust shrinks and the name is cut; the mode goes last. The
/// state's rect is returned (a click on `↓ back to the bottom` jumps to
/// the tail), then the label's (zen keeps it, BISE-121).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_divider(
    buf: &mut Buffer,
    area: Rect,
    cols: Cols,
    y: u16,
    name: &str,
    who: &Who,
    working: Option<&Working>,
    states: Vec<Vec<Span<'static>>>,
) -> (Rect, Rect) {
    let area = area.intersection(buf.area);
    if !divider_rule(buf, area, cols, y) {
        return (Rect::default(), Rect::default());
    }
    let (l, r) = (area.x, area.right() - 1);
    let lx = l + cols.margin - 1;
    // the state's end: framed F − 4 (F − 3 its space), bare the last column
    let end = r + 1 - cols.margin;
    let fits = |label: &[Span], state: &[Span]| {
        let w = u32::from(lx) + u32::from(width_of(label));
        let state_w = width_of(state);
        if state_w > 0 {
            w + 2 + u32::from(state_w) <= u32::from(end)
        } else {
            w <= u32::from(end) + 1
        }
    };
    let states: Vec<Vec<Span<'static>>> = states.into_iter().filter(|s| width_of(s) > 0).collect();
    let tails = who.tails();
    let pick = ladder(states.len(), tails.len())
        .into_iter()
        .map(|(si, ti, step)| {
            let tail = tails.get(ti).cloned().unwrap_or_default();
            let state = si.and_then(|i| states.get(i)).cloned().unwrap_or_default();
            (label(name, &tail, working.map(|w| (w, STEPS[step]))), state, si.is_none())
        })
        .find(|(label, state, _)| fits(label, state));
    let (label, state) = match pick {
        Some((label, state, _)) => (label, state),
        // nothing fits: the smallest label, the state cut to what is left
        None => {
            let tail = tails.last().cloned().unwrap_or_default();
            let label = label(name, &tail, working.map(|w| (w, STEPS[STEPS.len() - 1])));
            let room = end.saturating_sub(lx + width_of(&label) + 2);
            let state = match (working, states.last()) {
                (None, Some(s)) => fit(s.clone(), room as usize),
                _ => Vec::new(),
            };
            (label, state)
        }
    };
    put(buf, lx, y, &label, end + 1);
    let label_rect = Rect { x: lx, y, width: width_of(&label).min((end + 1).saturating_sub(lx)), height: 1 };
    let w = width_of(&state);
    if w == 0 {
        return (Rect::default(), label_rect);
    }
    let x = end - w;
    put(buf, x - 1, y, &[Span::raw(" ")], end);
    put(buf, x, y, &state, end);
    if end <= r {
        put(buf, end, y, &[Span::raw(" ")], end + 1);
    }
    (Rect { x, y, width: w, height: 1 }, label_rect)
}

/// The divider's forms, richest first (BISE-303): (the state, the tail,
/// the step) for `states` states and `tails` tails. The long state goes
/// for the short one, then the held words, then the tail shrinks
/// with the short state; then the state goes (the smallest tail, the
/// mode, stays), the gust shrinks, the name is cut; the mode goes last
/// (tail `tails`: none).
fn ladder(states: usize, tails: usize) -> Vec<(Option<usize>, usize, usize)> {
    let mut out = Vec::new();
    let last_tail = tails.saturating_sub(1);
    if states > 0 {
        let short = states - 1;
        out.extend((0..states).map(|s| (Some(s), 0, 0)));
        out.extend((0..tails.max(1)).map(|t| (Some(short), t, 1)));
    } else {
        out.push((None, 0, 0));
    }
    out.extend((0..tails.max(1)).map(|t| (None, t, 1)));
    out.extend((2..STEPS.len()).map(|step| (None, last_tail, step)));
    out.push((None, tails, STEPS.len() - 1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        let s = vec![Span::raw("idle · "), Span::raw("210k / 1M tokens")];
        let out = fit(s.clone(), 100);
        assert_eq!(out.len(), 2);
        let out: String = fit(s, 10).iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(out, "idle · 21…");
        assert_eq!(out.width(), 10);
    }

    fn divider_row_of(width: u16, name: &str, working: Option<&Working>, state: &str) -> String {
        divider_row_who(width, name, &Who::default(), working, state)
    }

    fn divider_row_who(width: u16, name: &str, who: &Who, working: Option<&Working>, state: &str) -> String {
        divider_row_forms(width, name, who, working, &[state])
    }

    fn divider_row_forms(width: u16, name: &str, who: &Who, working: Option<&Working>, states: &[&str]) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let cols = crate::layout::cols(width, 40);
        let states = states.iter().map(|s| vec![Span::raw(s.to_string())]).collect();
        draw_divider(&mut buf, area, cols, 0, name, who, working, states);
        (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
    }

    /// BISE-135: the model and effort after the name, `ψ place` after
    /// them (BISE-136); short on room, the place's name goes first (ψ
    /// stays), then the long form becomes the tag, then the tag goes,
    /// all before the state loses anything.
    #[test]
    fn the_divider_names_the_model_and_the_effort() {
        let who = Who {
            model: "opus 5.5".into(),
            effort: "high".into(),
            tag: "opus·hi".into(),
            place: Some("fix-login".into()),
            ..Who::default()
        };
        let state = "idle · 42k";
        let row = |w| divider_row_who(w, "auth-fix", &who, None, state);
        assert!(row(90).contains("you → auth-fix · opus 5.5 · high · ψ fix-login ─"), "{}", row(90));
        assert!(row(90).ends_with(" idle · 42k ─┤"), "{}", row(90));
        assert!(row(58).contains("you → auth-fix · opus 5.5 · high · ψ ─"), "{}", row(58));
        assert!(row(58).contains(" idle · 42k "), "{}", row(58));
        assert!(row(46).contains("you → auth-fix · opus·hi · ψ ─"), "{}", row(46));
        assert!(row(46).contains(" idle · 42k "), "{}", row(46));
        assert!(row(36).contains("you → auth-fix · ψ ─"), "{}", row(36));
        assert!(row(36).contains(" idle · 42k "), "{}", row(36));
        // the shared checkout: nothing after the effort
        let main = Who { place: None, ..who.clone() };
        let r = divider_row_who(90, "main", &main, None, state);
        assert!(r.contains("you → main · opus 5.5 · high ─"), "{r}");
        // a model with no effort: the model alone
        let plain = Who { model: "gpt-4.1".into(), tag: "gpt-4.1".into(), ..Who::default() };
        assert!(divider_row_who(90, "docs", &plain, None, state).contains("you → docs · gpt-4.1 ─"));
        // while it works, the gust after the tail (ctrl held: the words)
        let w = Working { motion: crate::gust::Motion::Still, age: Some("42s".into()), words: true };
        let r = divider_row_who(90, "auth-fix", &who, Some(&w), "31%");
        assert!(r.contains("auth-fix · opus 5.5 · high · ψ fix-login · ∿ working · 42s ─"), "{r}");
    }

    /// The user's feedback on approvals (item 1): the mode after the
    /// model, `· yolo`, dim; short on room it goes last, after ψ.
    #[test]
    fn the_divider_says_the_approvals_mode_after_the_model() {
        let who = Who {
            model: "opus 5.5".into(),
            effort: "high".into(),
            tag: "opus·hi".into(),
            place: Some("fix-login".into()),
            mode: "yolo".into(),
            ..Who::default()
        };
        let state = "idle · 42k";
        let row = |w| divider_row_who(w, "auth-fix", &who, None, state);
        assert!(row(90).contains("you → auth-fix · opus 5.5 · high · yolo · ψ fix-login ─"), "{}", row(90));
        assert!(row(65).contains("you → auth-fix · opus 5.5 · high · yolo · ψ ─"), "{}", row(65));
        assert!(row(52).contains("you → auth-fix · opus·hi · yolo · ψ ─"), "{}", row(52));
        assert!(row(42).contains("you → auth-fix · yolo · ψ ─"), "{}", row(42));
        assert!(row(38).contains("you → auth-fix · yolo ─"), "{}", row(38));
        for w in [90, 65, 52, 42, 38] {
            assert!(row(w).contains(" idle · 42k "), "{}", row(w));
        }
        let main = Who { place: None, ..who.clone() };
        assert!(divider_row_who(90, "main", &main, None, state).contains("you → main · opus 5.5 · high · yolo ─"));
        // the word's style: dim at rest, accent for the switch's 3 s
        let style = |who: &Who| {
            let t = who.tails().remove(0);
            t.iter().find(|s| s.content == "yolo").map(|s| s.style.fg).expect("the mode")
        };
        assert_eq!(style(&who), Some(dim()));
        assert_eq!(style(&Who { flash: true, ..who.clone() }), Some(accent()));
        // the hub has not said the mode yet: no word
        assert!(!divider_row_who(90, "main", &Who { mode: String::new(), ..main }, None, state).contains("yolo"));
    }

    fn divider_row(width: u16, working: Option<&Working>, state: &str) -> String {
        divider_row_of(width, "marketing", working, state)
    }

    /// Frame 3 of the gust: 5 cells `·~∿≈ `, 3 cells `·~∿`, 1 cell `≈`;
    /// `words`: ctrl held.
    fn working(age: Option<&str>, words: bool) -> Working {
        Working { motion: gust::Motion::Frame(3), age: age.map(String::from), words }
    }

    /// BISE-303: at rest, the gust after a dim ` · ` and nothing else;
    /// ctrl held, `working · 42s` after it. The right side as given.
    #[test]
    fn the_divider_says_the_viewed_agent_works() {
        let row = divider_row(100, Some(&working(Some("42s"), false)), "18k · 2%");
        assert!(row.starts_with("├─ you → marketing · ·~∿≈  ─"), "{row}");
        assert!(row.ends_with("─ 18k · 2% ─┤"), "{row}");
        let row = divider_row(100, Some(&working(Some("42s"), true)), "18k · 2%");
        assert!(row.starts_with("├─ you → marketing · ·~∿≈  working · 42s ─"), "{row}");
        // no age yet: the word alone
        let row = divider_row(100, Some(&working(None, true)), "");
        assert!(row.starts_with("├─ you → marketing · ·~∿≈  working ─"), "{row}");
        // no motion: one static wave
        let still = Working { motion: gust::Motion::Still, age: Some("42s".into()), words: true };
        let row = divider_row(100, Some(&still), "");
        assert!(row.starts_with("├─ you → marketing · ∿ working · 42s ─"), "{row}");
        // the gust's ` · ` is dim
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        draw_divider(&mut buf, area, crate::layout::cols(100, 40), 0, "m", &Who::default(), Some(&still), Vec::new());
        assert_eq!(buf[(11, 0)].symbol(), "·");
        assert_eq!(buf[(11, 0)].style().fg, Some(dim()));
    }

    #[test]
    fn the_divider_says_nothing_after_the_name_when_idle() {
        let row = divider_row(100, None, "18k · 2%");
        assert!(row.starts_with("├─ you → marketing ───"), "{row}");
        assert!(row.ends_with("─ 18k · 2% ─┤"), "{row}");
    }

    /// BISE-303: short on room, ctrl held: the words go, the long
    /// context becomes the short one, the tail shrinks, the context goes,
    /// then the gust shrinks and the name is cut; the mode goes last.
    #[test]
    fn short_on_room_the_divider_drops_in_order() {
        let who = Who { model: "opus 5.5".into(), effort: "high".into(), tag: "opus·hi".into(), mode: "yolo".into(), ..Who::default() };
        let w = working(Some("42s"), true);
        let forms = ["18k / 1M tokens · 2%", "18k · 2%"];
        // the steps, richest first: what each one shows
        let level = |r: &str| -> usize {
            let has = |t: &str| r.contains(t);
            match () {
                _ if has("working · 42s") && has("tokens") => 0,
                _ if has("working · 42s") && has("18k · 2%") => 1,
                _ if has("tokens") => 2,
                _ if has("opus 5.5") && has("18k · 2%") => 3,
                _ if has("opus·hi") && has("18k · 2%") => 4,
                _ if has("18k · 2%") => 5,
                _ if has("·~∿≈") => 6,
                _ if has("·~∿") => 7,
                _ => 8,
            }
        };
        let mut last = 0;
        let mut seen = Vec::new();
        for width in (20..140).rev() {
            let r = divider_row_forms(width, "marketing", &who, Some(&w), &forms);
            assert_eq!(r.chars().count(), width as usize, "{width}: {r}");
            let l = level(&r);
            assert!(l >= last, "{width}: level {l} after {last}: {r}");
            // the mode stays to the end, the gust always
            assert!(r.contains("yolo") || width < 30, "{width}: {r}");
            assert!(r.contains('≈') || r.contains('∿') || width < 21, "{width}: {r}");
            last = l;
            if !seen.contains(&l) {
                seen.push(l);
            }
        }
        assert_eq!(seen, vec![0, 1, 3, 4, 5, 6, 7, 8], "every step shows on the way down");
        // at rest: the short context only, never `working`
        let rest = working(Some("42s"), false);
        for width in (20..140).rev() {
            let r = divider_row_forms(width, "marketing", &who, Some(&rest), &["18k · 2%"]);
            assert!(!r.contains("working") && !r.contains("42s"), "{width}: {r}");
        }
        // the name is cut last: at 20, then 12 (BISE-109)
        let name = "release-notes-writer-v2";
        let rest = working(Some("42s"), false);
        assert!(divider_row_of(35, name, Some(&rest), "").starts_with(" you → release-notes-writer-v2 · ≈ "));
        assert!(divider_row_of(34, name, Some(&rest), "").starts_with(" you → release-notes-write… · ≈ "));
        assert_eq!(divider_row_of(31, name, Some(&rest), ""), format!(" you → release-not… · ≈ {}", "─".repeat(7)));
    }

    #[test]
    fn the_gust_stays_while_the_agent_works_at_any_width() {
        let w = working(Some("42s"), true);
        for width in 20..130 {
            let r = divider_row_forms(width, "marketing", &Who::default(), Some(&w), &["18k / 1M tokens · 2%", "18k · 2%"]);
            assert_eq!(r.chars().count(), width as usize, "{width}: {r}");
            assert!(r.contains("marketing · ≈") || r.contains("marketing · ·~∿"), "{width}: {r}");
            // the state is whole or gone, never cut
            assert!(r.contains("2%") || !r.contains("18k"), "{width}: {r}");
        }
    }
}
