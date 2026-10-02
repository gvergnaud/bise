//! The rows of a `/release-bise` in main's feed (BISE-235, designer's
//! reco): the preview (`· release v2026.10.3 · abc1234 subject · 12
//! commits since v2026.10.1`, then up to 10 commits), a dim `✓` per step,
//! one `∿ CI building · 12m` row the next row replaces in place, then
//! for you (level 2) `✓ released v2026.10.3 · …` or `✗ release … failed ·
//! <why>` with the script's tail folded under `▸ n more lines`.

use crate::render::{hung_rows, G_NOTE};
use crate::theme::*;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Plan {
        tag: String,
        short: String,
        subject: String,
        since: String,
        count: usize,
        /// (short hash, subject), the newest first
        commits: Vec<(String, String)>,
        dry: bool,
    },
    /// a step is over: dim `✓`
    Step(String),
    /// in progress: `∿`, replaced by the next release row
    Running(String),
    /// released (level 2): accent `✓`
    Done(String),
    /// the script failed: `✗` and why, its last lines under a fold
    Failed { text: String, tail: Vec<String>, open: bool },
    /// dev-update: `/update` could not build, the running version stays:
    /// `▲` and why, the build's last lines under a fold
    Warned { text: String, tail: Vec<String>, open: bool },
}

impl Row {
    /// For you (level 2): the result.
    pub(crate) fn is_l2(&self) -> bool {
        matches!(self, Row::Done(_) | Row::Failed { .. } | Row::Warned { .. })
    }

    /// Something to open (ctrl+o, a click): a failure's tail.
    pub(crate) fn discloses(&self) -> bool {
        matches!(self, Row::Failed { tail, .. } | Row::Warned { tail, .. } if !tail.is_empty())
    }

    pub(crate) fn open_mut(&mut self) -> Option<&mut bool> {
        match self {
            Row::Failed { open, .. } | Row::Warned { open, .. } => Some(open),
            _ => None,
        }
    }
}

fn lead(glyph: &'static str, st: Style) -> Span<'static> {
    Span::styled(format!(" {} ", crate::theme::glyph(glyph)), st)
}

fn one(glyph: Span<'static>, spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    hung_rows(&glyph, &Span::raw("   "), [Line::from(spans)], width)
}

pub(crate) fn lines(row: &Row, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let faint_st = Style::default().fg(faint());
    let text_st = Style::default().fg(text());
    let sep = || Span::styled(" · ", dim_st);
    match row {
        Row::Plan { tag, short, subject, since, count, commits, dry } => {
            let what = match (*count, since.is_empty()) {
                (_, true) => "the first release".to_string(),
                (1, false) => format!("1 commit since {}", since),
                (n, false) => format!("{} commits since {}", n, since),
            };
            let mut head = vec![
                Span::styled("release ", text_st),
                Span::styled(tag.clone(), text_st.add_modifier(Modifier::BOLD)),
                sep(),
                Span::styled(format!("{} ", short), faint_st),
                Span::styled(subject.clone(), text_st),
                sep(),
                Span::styled(what, text_st),
            ];
            if *dry {
                head.push(Span::styled(" · dry run", dim_st));
            }
            let mut out = one(lead(G_NOTE, faint_st), head, width);
            for (h, s) in commits {
                out.extend(one(
                    Span::raw("   "),
                    vec![Span::styled(format!("{} ", h), faint_st), Span::styled(s.clone(), text_st)],
                    width,
                ));
            }
            let more = count.saturating_sub(commits.len());
            if more > 0 {
                out.push(Line::from(vec![Span::raw("   "), Span::styled(format!("and {} more", more), dim_st)]));
            }
            out
        }
        Row::Step(t) => one(lead(G_RECEIVED, dim_st), vec![Span::styled(t.clone(), dim_st)], width),
        Row::Running(t) => one(lead(G_WORKING, dim_st), vec![Span::styled(t.clone(), dim_st)], width),
        Row::Done(t) => one(
            Span::styled(format!(" {} ", done_glyph()), Style::default().fg(accent())),
            vec![Span::styled(t.clone(), text_st)],
            width,
        ),
        Row::Failed { text: t, tail, open } | Row::Warned { text: t, tail, open } => {
            let glyph = match row {
                Row::Failed { .. } => lead(G_FAILED, Style::default().fg(error())),
                _ => lead(G_INTERRUPTED, dim_st),
            };
            let mut out = one(glyph, vec![Span::styled(t.clone(), text_st)], width);
            if tail.is_empty() {
                return out;
            }
            if *open {
                for l in tail {
                    out.extend(one(Span::styled("   │ ", faint_st), vec![Span::styled(l.clone(), dim_st)], width));
                }
            } else {
                let n = tail.len();
                let word = if n == 1 { "line" } else { "lines" };
                out.push(Line::from(vec![Span::raw("   "), Span::styled(format!("▸ {} more {}", n, word), dim_st)]));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(row: &Row, width: usize) -> Vec<String> {
        lines(row, width).iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>()).collect()
    }

    #[test]
    fn the_preview_lists_ten_commits_then_a_count() {
        let commits: Vec<(String, String)> = (0..10).map(|i| (format!("abc12{:02}", i), format!("fix {}", i))).collect();
        let row = Row::Plan {
            tag: "v2026.10.3".into(),
            short: "abc1200".into(),
            subject: "fix 0".into(),
            since: "v2026.10.1".into(),
            count: 12,
            commits,
            dry: false,
        };
        let t = text(&row, 120);
        assert_eq!(t[0], " · release v2026.10.3 · abc1200 fix 0 · 12 commits since v2026.10.1");
        assert_eq!(t[1], "   abc1200 fix 0");
        assert_eq!(t.len(), 12);
        assert_eq!(t[11], "   and 2 more");
    }

    #[test]
    fn a_step_a_result_and_a_folded_failure() {
        assert_eq!(text(&Row::Step("tag v1 pushed".into()), 80), [" ✓ tag v1 pushed"]);
        assert_eq!(text(&Row::Running("CI building · 12m".into()), 80), [" ∿ CI building · 12m"]);
        let mut f = Row::Failed { text: "release v1 failed · no gh".into(), tail: vec!["a".into(), "b".into()], open: false };
        assert_eq!(text(&f, 80), [" ✗ release v1 failed · no gh", "   ▸ 2 more lines"]);
        assert!(f.discloses() && f.is_l2());
        *f.open_mut().unwrap() = true;
        assert_eq!(text(&f, 80), [" ✗ release v1 failed · no gh", "   │ a", "   │ b"]);
    }
}
