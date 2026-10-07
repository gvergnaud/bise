//! The worktrees in the panel (pr-design §4.1, dev-flow §3.1, option B
//! of sidebar-wt): a worktree 2 or more agents share is a section like
//! `agents` and `inbox`, git in its title, its agents' rows under it; an
//! agent alone in one carries its mark in its row's last column.
//!
//! The hub sends `places` (the worktrees only, in their first agent's
//! order, the frozen contract of switchboard's `place.rs`) and each
//! agent's `place_id`; this module reads them and draws what is git's:
//! the section's title (` ψ sb/dark-mode      ↑ `), the held lid line
//! (`changes asked · checks pass`), a solo row's mark, the PR's words
//! for the divider and the header.

use super::*;
use unicode_width::UnicodeWidthStr;

/// A worktree as the hub sends it (`PlaceView`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Place {
    pub(crate) id: String,
    pub(crate) branch: Option<String>,
    /// its agents, not archived, in the hub's order
    pub(crate) agents: Vec<String>,
    pub(crate) pr: Option<Pr>,
    /// the held line the hub writes (`waits to land · 2nd`, `no PR yet ·
    /// 2 commits`): it wins over the PR's words, shown as it is
    pub(crate) lid: Option<String>,
    /// dev-flow §5.1, pr-design §4.1 item 8: a feature branch (its agents
    /// may each have a worktree; they share the branch). `trying`: its
    /// try build builds or is on trial: `Δ` dim, else `ψ`; never `↑`.
    pub(crate) feature: bool,
    pub(crate) trying: bool,
}

/// The PR of a worktree's branch (`PrView`): the enums as their JSON
/// words (`changes_requested`), the failing checks' names.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pr {
    pub(crate) number: u64,
    pub(crate) url: String,
    /// `draft`, `open`, `merged`, `closed`
    pub(crate) state: String,
    /// `none`, `pending`, `approved`, `changes_requested`
    pub(crate) review: String,
    /// `none`, `running`, `pass`, `fail`
    pub(crate) checks: String,
    pub(crate) failing: Vec<String>,
    /// how old the forge's last answer is, when it is late
    pub(crate) stale_ms: Option<u64>,
}

/// The snapshot's `places` (absent from an older hub: none).
pub(super) fn parse(v: &Value) -> Vec<Place> {
    let s = |x: &Value, k: &str| x.get(k).and_then(|b| b.as_str()).map(String::from);
    let Some(all) = v.get("places").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    all.iter()
        .map(|p| Place {
            id: s(p, "id").unwrap_or_default(),
            branch: s(p, "branch"),
            agents: p
                .get("agents")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|n| n.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            pr: p.get("pr").filter(|x| x.is_object()).map(|x| {
                let checks = x.get("checks");
                Pr {
                    number: x.get("number").and_then(|n| n.as_u64()).unwrap_or(0),
                    url: s(x, "url").unwrap_or_default(),
                    state: s(x, "state").unwrap_or_default(),
                    review: s(x, "review").unwrap_or_default(),
                    checks: checks.and_then(|c| s(c, "state")).unwrap_or_default(),
                    failing: checks
                        .and_then(|c| c.get("failing"))
                        .and_then(|f| f.as_array())
                        .map(|f| f.iter().filter_map(|n| n.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    stale_ms: x.get("stale_ms").and_then(|n| n.as_u64()),
                }
            }),
            lid: s(p, "lid").filter(|l| !l.is_empty()),
            feature: p.get("feature").and_then(|b| b.as_bool()).unwrap_or(false),
            trying: p.get("trying").and_then(|b| b.as_bool()).unwrap_or(false),
        })
        .collect()
}

impl Pr {
    /// Open or draft: merged and closed draw nothing (a merged place goes
    /// away a tick later; a closed one keeps its box, not its mark).
    pub(crate) fn live(&self) -> bool {
        matches!(self.state.as_str(), "open" | "draft")
    }

    fn fails(&self) -> bool {
        self.checks == "fail"
    }

    /// `↑`'s look (pr-design §4): dim open, faint draft or late, red
    /// when checks fail (bold under `NO_COLOR`); never pink, never green.
    pub(crate) fn mark_style(&self) -> Style {
        if self.stale_ms.is_some() || self.state == "draft" {
            Style::default().fg(faint())
        } else if self.fails() && no_color() {
            Style::default().add_modifier(Modifier::BOLD)
        } else if self.fails() {
            Style::default().fg(error())
        } else {
            Style::default().fg(dim())
        }
    }

    /// The PR's state in words, ctrl held, `st` their look: `changes
    /// asked · checks pass`, `draft · checks running`, `checks fail:
    /// e2e/login` (red only on the words `checks fail`), `state from 12m
    /// ago` when the forge is late.
    pub(crate) fn words(&self, st: Style) -> Vec<Span<'static>> {
        let mut parts: Vec<Vec<Span<'static>>> = Vec::new();
        let w = |t: &str| vec![Span::styled(t.to_string(), st)];
        if self.state == "draft" {
            parts.push(w("draft"));
        }
        match self.review.as_str() {
            "changes_requested" => parts.push(w("changes asked")),
            "approved" => parts.push(w("approved")),
            "pending" => parts.push(w("in review")),
            _ => {}
        }
        match self.checks.as_str() {
            "pass" => parts.push(w("checks pass")),
            "running" => parts.push(w("checks running")),
            "fail" => {
                let red = if no_color() { st.add_modifier(Modifier::BOLD) } else { st.fg(error()) };
                let mut v = vec![Span::styled("checks fail", red)];
                if !self.failing.is_empty() {
                    v.push(Span::styled(format!(": {}", self.failing.join(", ")), st));
                }
                parts.push(v);
            }
            _ => {}
        }
        if parts.is_empty() {
            parts.push(w("open"));
        }
        if let Some(ms) = self.stale_ms {
            parts.push(w(&format!("state from {} ago", panel::short_age(ms))));
        }
        let mut out = Vec::new();
        for (k, p) in parts.into_iter().enumerate() {
            if k > 0 {
                out.push(Span::styled(" · ", st));
            }
            out.extend(p);
        }
        out
    }
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

impl Place {
    /// Its name in a border, a divider, a words line: its branch; a
    /// private worktree with none (BISE-136, `gate.sh new` detaches it,
    /// id `pt:<path>`) its folder, the path's last part (designer's call
    /// 8). None: a hub worktree with no branch.
    pub(crate) fn label(&self) -> Option<String> {
        self.branch.clone().or_else(|| self.folder())
    }

    /// The folder of a private worktree with no branch checked out.
    fn folder(&self) -> Option<String> {
        Some(folder_of(self.id.strip_prefix("pt:")?)).filter(|f| !f.is_empty())
    }

    /// The PR when it is open or a draft (a feature never has one).
    pub(crate) fn live_pr(&self) -> Option<&Pr> {
        self.pr.as_ref().filter(|p| p.live() && !self.feature)
    }

    /// The glyph of its border and of a solo row's mark: `Δ` while a
    /// feature's try build builds or is on trial, else `ψ`.
    fn glyph(&self) -> &'static str {
        if self.feature && self.trying {
            theme::glyph(G_BUILDING)
        } else {
            theme::glyph(G_WORKTREE)
        }
    }

    /// Trunk flow: its land waits in the queue (the hub's lid says
    /// `waits to land · 2nd`): `…` in its border (dev-flow §7).
    fn waits_to_land(&self) -> bool {
        self.lid.as_deref().is_some_and(|l| l.starts_with("waits to land"))
    }

    /// The title of a shared worktree's section (option B, sidebar-wt),
    /// `w` columns, like `agents` and `inbox` (their color, 1 blank
    /// before): ` ψ sb/dark-mode        ↑ `, the mark in the rows' mark
    /// column (`↑` by [`Pr::mark_style`], never the accent; `…` while its
    /// land waits; a feature none: `Δ` dim for its glyph while it
    /// tries). Ctrl held (`held`) the number joins the mark, left-packed:
    /// ` ψ sb/dark-mode ↑ #412`. Short on room the branch is cut with `…`
    /// first; the glyph, the mark and its number stay.
    pub(crate) fn title(&self, w: usize, held: bool) -> Line<'static> {
        let t = Style::default().fg(text());
        let d = Style::default().fg(dim());
        let mut right: Vec<Span<'static>> = Vec::new();
        if let Some(pr) = self.live_pr() {
            right.push(Span::styled(theme::pr_glyph(), pr.mark_style()));
            if held {
                let num = if pr.state == "draft" || pr.stale_ms.is_some() { Style::default().fg(faint()) } else { d };
                right.push(Span::styled(format!(" #{}", pr.number), num));
            }
        } else if self.waits_to_land() {
            right.push(Span::styled(theme::glyph(G_WAITING), d));
        }
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        let glyph = self.glyph();
        let gst = if self.feature && self.trying { d } else { t };
        // ` ψ ` the branch, 1 blank, the right side, 1 blank of margin
        let fixed = 1 + glyph.width() + 1 + if right.is_empty() { 0 } else { 1 + right_w } + 1;
        let room = w.saturating_sub(fixed);
        // the number goes before the branch is cut below 2 columns
        if held && room < 2 && right.len() > 1 {
            return self.title(w, false);
        }
        let branch = match &self.label() {
            Some(b) if room >= 2 => panel::fit(b, room),
            _ => String::new(),
        };
        let mut spans = vec![Span::raw(" "), Span::styled(glyph.to_string(), gst)];
        if !branch.is_empty() {
            spans.push(Span::styled(format!(" {}", branch), t));
        }
        if !right.is_empty() {
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            // at rest the mark sits in the rows' mark column (2 from the
            // right edge); held, left-packed after the branch
            let pad = if held { 1 } else { w.saturating_sub(used + right_w + 1).max(1) };
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(right);
        }
        Line::from(spans)
    }

    /// The mark of a row alone in this worktree (option A, sidebar-wt),
    /// in the row's last column, first that applies: `…` dim (its land
    /// waits), `↑` faint when the forge is late, red when checks fail,
    /// accent when an inbox item asks you about it (`asks`), faint
    /// draft, dim open; else `ψ` dim (no PR yet, or merged or closed).
    /// `NO_COLOR`: red and accent are bold.
    pub(crate) fn row_mark(&self, asks: bool) -> Span<'static> {
        let d = Style::default().fg(dim());
        if self.waits_to_land() {
            return Span::styled(theme::glyph(G_WAITING).to_string(), d);
        }
        let Some(pr) = self.live_pr() else {
            // a feature: Δ while it builds or is on trial (dev-flow §7)
            return Span::styled(self.glyph().to_string(), d);
        };
        let st = if pr.stale_ms.is_none() && !pr.fails() && asks {
            if no_color() { Style::default().add_modifier(Modifier::BOLD) } else { Style::default().fg(accent()) }
        } else {
            pr.mark_style()
        };
        Span::styled(theme::pr_glyph().to_string(), st)
    }

    /// The words line under a solo row or a worktree's row, ctrl held,
    /// `w` columns: dim, at the name's column, cut with `…`. The hub's
    /// lid as it is, else `#415 · ` and the PR's words; then ` · ψ
    /// <branch>` when the branch isn't the agent's name (`name`: the
    /// row's agent; None, the row already says the branch): last, so the
    /// cut eats the branch and the state stays. None: nothing to say.
    pub(crate) fn words_line(&self, name: Option<&str>, w: usize) -> Option<Line<'static>> {
        let d = Style::default().fg(dim());
        let mut spans = match (&self.lid, self.live_pr()) {
            (Some(l), _) => vec![Span::styled(l.clone(), d)],
            (None, Some(pr)) => {
                let mut v = vec![Span::styled(format!("#{} · ", pr.number), d)];
                v.extend(pr.words(d));
                v
            }
            (None, None) => Vec::new(),
        };
        if let (Some(n), Some(b)) = (name, &self.branch) {
            if b.rsplit('/').next() != Some(n) {
                if !spans.is_empty() {
                    spans.push(Span::styled(" · ", d));
                }
                spans.push(Span::styled(format!("{} {}", theme::glyph(G_WORKTREE), b), d));
            }
        }
        // a detached private worktree: its folder, only after something
        // to say (no commit of its own: no line, designer's call 8)
        if let (Some(_), None, Some(f)) = (name, &self.branch, self.folder()) {
            if !spans.is_empty() {
                spans.push(Span::styled(format!(" · {} {}", theme::glyph(G_WORKTREE), f), d));
            }
        }
        if spans.is_empty() {
            return None;
        }
        let mut line = vec![Span::raw("     ")];
        line.extend(fit_spans(spans, w.saturating_sub(6)));
        Some(Line::from(line))
    }

    /// The row of a worktree with no live agent but an open PR: no
    /// number, no glyph, its branch faint at the name's column, its mark
    /// in the mark column (`    sb/palette        ↑`), `w` columns.
    pub(crate) fn orphan_row(&self, asks: bool, w: usize) -> Line<'static> {
        let mark = self.row_mark(asks);
        let room = w.saturating_sub(5 + 3 + 1);
        let branch = panel::fit(&self.label().unwrap_or_else(|| self.id.clone()), room);
        let pad = w.saturating_sub(5 + branch.width() + 3 + 1);
        Line::from(vec![
            Span::raw("     "),
            Span::styled(branch, Style::default().fg(faint())),
            Span::raw(" ".repeat(pad + 2)),
            mark,
            Span::raw(" "),
        ])
    }

    /// The lid line under a section's title, ctrl held, `w` columns: dim
    /// (red only on `checks fail`), at the title's column, cut with `…`:
    /// the hub's lid, else the PR's words (the title has the number);
    /// None when nothing to say.
    pub(crate) fn lid_line(&self, w: usize) -> Option<Line<'static>> {
        let d = Style::default().fg(dim());
        let words = match (&self.lid, self.live_pr()) {
            (Some(l), _) => vec![Span::styled(l.clone(), d)],
            (None, Some(pr)) => pr.words(d),
            (None, None) => return None,
        };
        let mut spans = vec![Span::raw(" ")];
        spans.extend(fit_spans(words, w.saturating_sub(2)));
        Some(Line::from(spans))
    }
}

/// `spans` cut to `max` columns, `…` at the cut (in the cut span's look).
pub(crate) fn fit_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(|s| s.content.width()).sum();
    if total <= max {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    for s in spans {
        let w = s.content.width();
        if used + w < max {
            used += w;
            out.push(s);
            continue;
        }
        let cut = panel::fit(&s.content, max - used);
        out.push(Span::styled(cut, s.style));
        break;
    }
    out
}

/// The name of a private worktree's folder (BISE-136): the task's name
/// for `gate.sh new`'s `<bise home>/worktrees/<project>/<name>/<repo>`
/// (its last part is the repo's, the same for every task), else the
/// path's last part (`/tmp/fix-wt`).
pub(crate) fn folder_of(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    match parts.iter().rposition(|p| *p == "worktrees") {
        Some(i) if parts.len() == i + 4 => parts[i + 2].to_string(),
        _ => parts.last().copied().unwrap_or(path).to_string(),
    }
}

/// The header's held count: the worktrees with an open PR (`↑ 2 PRs`).
/// The panel's notes for features whose try build builds or is on trial
/// (dev-flow §7): `Δ <branch> on trial`. A feature trial reaches the TUI
/// as its place's `trying`, never as a version's `trial` mark (that one
/// is the hub's own version on probation).
pub(crate) fn trial_notes(places: &[Place]) -> Vec<String> {
    places
        .iter()
        .filter(|p| p.feature && p.trying)
        .map(|p| format!("{} {} on trial", G_BUILDING, p.branch.as_deref().unwrap_or(&p.id)))
        .collect()
}

pub(crate) fn open_prs(places: &[Place]) -> usize {
    places.iter().filter(|p| p.live_pr().is_some()).count()
}

/// What the header says of the flow, held, after the folder (dev-flow
/// §7): `lands via PRs`, `lands on main`; "" when the hub has not said.
pub(crate) fn flow_words(flow: &str) -> &'static str {
    match flow {
        "pr" => "lands via PRs",
        "trunk" => "lands on main",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn text_color() -> Color {
        crate::theme::text()
    }

    fn pr(state: &str, review: &str, checks: &str) -> Pr {
        Pr { number: 412, url: "https://github.com/acme/web/pull/412".into(), state: state.into(), review: review.into(), checks: checks.into(), ..Pr::default() }
    }

    fn place(branch: &str, pr: Option<Pr>, lid: Option<&str>) -> Place {
        Place { id: "wt:x".into(), branch: Some(branch.into()), agents: vec!["x".into()], pr, lid: lid.map(Into::into), ..Place::default() }
    }

    /// dev-flow §7, pr-design §4.1 item 8: a feature's title glyph and
    /// solo mark are `Δ` (dim) while its try build builds or is on
    /// trial, else `ψ`; never `↑`, its title's mark column blank.
    #[test]
    fn a_feature_is_psi_then_delta_while_trying() {
        let v = serde_json::json!({"places": [
            {"id": "feature:computer-use", "branch": "computer-use", "agents": ["cu-a", "cu-b"], "pr": null,
             "lid": "feature · 14 commits · 3 behind main · not tried", "feature": true, "trying": false}
        ]});
        let mut p = parse(&v).remove(0);
        assert!(p.feature && !p.trying);
        assert_eq!(text(&p.title(31, false)), " ψ computer-use");
        assert_eq!(p.title(31, false).spans[1].style.fg, Some(text_color()));
        assert_eq!(p.row_mark(false).content, "ψ");
        assert_eq!(p.lid_line(60).map(|l| text(&l)).unwrap(), " feature · 14 commits · 3 behind main · not tried");
        p.trying = true;
        assert_eq!(text(&p.title(31, false)), " Δ computer-use");
        assert_eq!(p.title(31, false).spans[1].style.fg, Some(dim()));
        assert_eq!(p.row_mark(true).content, "Δ");
        assert_eq!(p.row_mark(true).style.fg, Some(dim()));
        // never ↑, even if a PR were sent for its branch
        p.pr = Some(pr("open", "approved", "pass"));
        assert!(!text(&p.title(31, true)).contains('↑'));
        // an older hub: no feature key
        let old = parse(&serde_json::json!({"places": [{"id": "wt:x", "agents": []}]}));
        assert!(!old[0].feature && !old[0].trying);
    }

    /// A feature on trial gives the panel its `on trial` note; a plain
    /// worktree or a feature not trying gives none.
    #[test]
    fn a_feature_on_trial_gives_the_panel_note() {
        let v = serde_json::json!({"places": [
            {"id": "feature:gift-cards", "branch": "gift-cards", "agents": ["g"], "feature": true, "trying": true},
            {"id": "feature:idle", "branch": "idle", "agents": [], "feature": true, "trying": false},
            {"id": "wt:a", "branch": "sb/a", "agents": ["a"], "trying": true}
        ]});
        assert_eq!(trial_notes(&parse(&v)), vec!["Δ gift-cards on trial".to_string()]);
    }

    /// The contract's JSON (switchboard place.rs, `the_contract_json`)
    /// read back: the tagged checks, the snake_case words.
    #[test]
    fn reads_the_hubs_places() {
        let v = serde_json::json!({"places": [
            {"id": "wt:dark", "branch": "sb/dark", "agents": ["dark", "i18n"], "lid": null,
             "pr": {"number": 412, "url": "u", "state": "open", "review": "changes_requested",
                    "checks": {"state": "fail", "failing": ["ci/test"]}, "stale_ms": null}},
            {"id": "wt:csv", "branch": null, "agents": [], "pr": null, "lid": "waits to land · 2nd"}
        ]});
        let ps = parse(&v);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].agents, ["dark", "i18n"]);
        let p = ps[0].pr.as_ref().unwrap();
        assert_eq!((p.number, p.review.as_str(), p.checks.as_str()), (412, "changes_requested", "fail"));
        assert_eq!(p.failing, ["ci/test"]);
        assert_eq!(ps[1].pr, None);
        assert!(ps[1].waits_to_land());
        assert!(parse(&serde_json::json!({})).is_empty());
    }

    /// Option B: the title is ` ψ <branch>` in the section titles' color,
    /// the mark in the rows' mark column (the 2nd column from the right);
    /// held, `↑ #412` left-packed after the branch.
    #[test]
    fn the_title_carries_the_branch_and_the_mark() {
        let p = place("sb/dark-mode", Some(pr("open", "pending", "pass")), None);
        let rest = text(&p.title(31, false));
        assert_eq!(rest, " ψ sb/dark-mode              ↑");
        assert_eq!(rest.width(), 30, "the mark in the rows' mark column, 1 blank after");
        assert_eq!(text(&p.title(31, true)), " ψ sb/dark-mode ↑ #412");
        let t = p.title(31, false);
        assert!(t.spans.iter().filter(|s| s.content.contains("ψ") || s.content.contains("sb/")).all(|s| s.style.fg == Some(text_color())));
        // 24 columns: the branch is cut first, ψ, ↑ and its number stay
        let p = place("sb/dark-mode-everywhere", Some(pr("open", "pending", "pass")), None);
        assert_eq!(text(&p.title(24, true)), " ψ sb/dark-mode… ↑ #412");
        assert_eq!(text(&p.title(24, false)), " ψ sb/dark-mode-ever… ↑");
        assert_eq!(text(&p.title(24, false)).width(), 23);
        // no PR: no mark
        let none = place("sb/emoji-csv", None, None);
        assert_eq!(text(&none.title(31, false)), " ψ sb/emoji-csv");
        // a merged or closed PR: no mark
        let closed = place("sb/x", Some(pr("closed", "none", "none")), None);
        assert!(!text(&closed.title(31, true)).contains('↑'));
        // trunk: a land waiting
        let land = place("sb/emoji-csv", None, Some("waits to land · 2nd"));
        assert_eq!(text(&land.title(31, false)), " ψ sb/emoji-csv              …");
        // never wider than its room, whatever the branch
        for w in [12, 18, 24, 31, 44] {
            let long = place("sb/a-very-long-branch-name-for-sure", Some(pr("open", "none", "none")), None);
            assert!(text(&long.title(w, true)).width() < w, "{w}");
            assert!(text(&long.title(w, false)).width() < w, "{w}");
        }
    }

    #[test]
    fn the_lid_says_the_prs_state() {
        let lid = |p: &Place| p.lid_line(31).map(|l| text(&l));
        assert_eq!(lid(&place("b", Some(pr("open", "changes_requested", "pass")), None)).unwrap(), " changes asked · checks pass");
        assert_eq!(lid(&place("b", Some(pr("draft", "none", "running")), None)).unwrap(), " draft · checks running");
        assert_eq!(lid(&place("b", Some(pr("open", "none", "none")), None)).unwrap(), " open");
        let failing = Pr { failing: vec!["e2e/login".into()], ..pr("open", "none", "fail") };
        assert_eq!(lid(&place("b", Some(failing.clone()), None)).unwrap(), " checks fail: e2e/login");
        let stale = Pr { stale_ms: Some(12 * 60_000), ..pr("open", "approved", "pass") };
        assert_eq!(lid(&place("b", Some(stale), None)).unwrap(), " approved · checks pass · sta…");
        // the hub's lid wins, as it is
        assert_eq!(lid(&place("b", None, Some("no PR yet · 2 commits"))).unwrap(), " no PR yet · 2 commits");
        assert_eq!(lid(&place("b", Some(pr("open", "none", "pass")), Some("waits to land · 2nd"))).unwrap(), " waits to land · 2nd");
        assert_eq!(lid(&place("b", None, None)), None);
        // red only on the words `checks fail`
        let l = place("b", Some(failing), None).lid_line(31).unwrap();
        let red: Vec<&str> = l.spans.iter().filter(|s| s.style.fg == Some(error())).map(|s| s.content.as_ref()).collect();
        assert_eq!(red, ["checks fail"]);
    }
}
