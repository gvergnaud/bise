//! The inbox (cards v2, BISE-236; its arrows and ⏎: book screens
//! `inbox · 1-7`): the hub's questions and alerts at three levels. The
//! UI says "inbox" and the items' own nouns (a question, an approval);
//! the code keeps "card".
//!
//! One rule for the arrows: an empty composer, they belong to the inbox;
//! text in the composer, they belong to your text. In the thread they
//! never touch the inbox.
//!
//! 1. The quick look: the strip right above the divider, one row per
//!    open card, most blocking first (approvals, questions, the rest),
//!    numbered 1, 2, 3… in that order (the panel uses the same numbers).
//! 2. The card view (BISE-302: ctrl+N opens row N, a click on a row
//!    opens it; `/inbox` the top one): it takes the history's
//!    place; the cards are tabs. On an empty composer ↑↓ highlight an
//!    option (none on open: a reflex ⏎ never answers), ⏎ picks it, 1-9
//!    pick at once, ←→ move between the cards; with text the composer
//!    answers (⏎ sends it). ctrl+n / ctrl+p move, ctrl+x closes, esc
//!    goes back to the thread. Each card keeps its own draft; the
//!    thread's draft waits for the way back.
//!
//! A new card never takes the focus: a strip row, or a tab. Answering
//! moves to the next card, or back to the thread when none are left,
//! with `✓ you answered perf: both` in the history.
//!
//! Approvals (docs/approvals.md §7, approvals-plan.md round 2) plug in
//! as the kind `approval`: its text is the command (or a patch), a blank
//! line, then the reason; the options are allow once / always here /
//! deny, and ⏎ with text denies with the text as a note. The gate that
//! opens them is not built yet.

use super::*;
use crate::editor::Editor;
use crate::theme;
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct Card {
    pub(super) id: u64,
    pub(super) kind: String,
    pub(super) agent: String,
    pub(super) text: String,
    /// The card's age when the snapshot arrived, and when it arrived.
    pub(super) age_ms: u64,
    pub(super) seen_at: std::time::Instant,
    /// The hub's remark (the asker heard from main since...).
    pub(super) note: String,
    /// How the TUI's own items (setup, BISE-245) read: their words are
    /// the TUI's, not parsed from `text`.
    pub(super) look: Option<Box<Look>>,
    /// A hub item's place id and PR number (the ready-to-merge item,
    /// pr-design §6.3): its box's `↑` takes the accent, `2` opens the
    /// place's PR.
    pub(super) place: Option<String>,
    pub(super) pr: Option<u64>,
    /// update-card: the release page an update item's `3` opens (the
    /// hub's snapshot).
    pub(super) link: Option<String>,
    /// dev-flow §5.1: a feature's merge item asks once more before its
    /// `3 drop the branch` (`drop computer-use? 14 commits go.`); the
    /// TUI's, kept across snapshots by `Sb::feature_drop_ask`.
    pub(super) asking: bool,
    /// expired-ux: a `signin` item's agents, stopped on the expired
    /// ChatGPT sign-in (the hub's snapshot): ⏎ in their thread signs in.
    pub(super) waiting: Vec<String>,
}

/// A paragraph of an item the TUI writes itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Para {
    Text(String),
    /// said on the side: dim
    Dim(String),
    /// a unified diff, in the code colors
    Diff(String),
}

/// How an item of the TUI's own reads (the setup items): the strip's
/// row and its faint end, the view's title, meta, tab and body, the
/// options; a strip action and a key bar of its own when it has no
/// options (the connectors key: `⏎ paste it`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) row: String,
    pub(crate) row_note: String,
    pub(crate) title: String,
    pub(crate) meta: String,
    pub(crate) tab: String,
    pub(crate) body: Vec<Para>,
    pub(crate) options: Vec<String>,
    pub(crate) right: Option<(&'static str, &'static str)>,
    pub(crate) keys: Vec<(&'static str, &'static str)>,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            id: 0,
            kind: String::new(),
            agent: String::new(),
            text: String::new(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
            look: None,
            place: None,
            pr: None,
            link: None,
            asking: false,
            waiting: Vec::new(),
        }
    }
}

impl Card {
    /// How long ago the card opened, in ms.
    pub(super) fn age_now(&self) -> u64 {
        self.age_ms.saturating_add(self.seen_at.elapsed().as_millis() as u64)
    }
}

/// What a click on the strip or the card view does (set by the draw).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CardHit {
    /// open the card view on this card (a strip row, the panel's)
    Row(u64),
    /// answer this card with its option `i` at once
    Pick(u64, usize),
    /// show this card in the view (a tab)
    Tab(u64),
    /// open the card view on the top card
    Open,
}

/// The last answer, folded into one line on top of the box for 2 s
/// (`✓ you allowed t3: npm publish`, `✗ you said no to …: … · "note"`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fold {
    pub(super) ok: bool,
    pub(super) text: String,
    /// The user's own words under the sentence (a typed answer, a no's
    /// note); the history draws them like a message of yours (BISE-307).
    pub(super) note: String,
    /// What the item asked (its text): the history's line opens on it.
    pub(super) asked: String,
}

/// How long the fold of an answer stays on top of the box, and `✓ inbox
/// clear` on the divider.
pub(super) const FOLD_FOR: std::time::Duration = std::time::Duration::from_secs(2);

/// The cards as the user sees them: an item open or not (in place, or
/// full screen), the card in it, its option highlighted, how far it is
/// scrolled, the drafts.
#[derive(Default)]
pub(super) struct CardView {
    /// An item is open: in its box, where its row was (or full screen).
    pub(super) open: bool,
    /// ctrl+o: the open item takes the history's place, the others as
    /// tabs on top.
    pub(super) full: bool,
    /// The last answer, folded on top of the box, and when.
    pub(super) fold: Option<(Fold, std::time::Instant)>,
    /// The card in the view; its draft is in the composer while open.
    pub(super) sel: Option<u64>,
    /// The option highlighted in the view (none on open), and whether
    /// the next draw scrolls it into view.
    pub(super) opt: Option<usize>,
    pub(super) reveal: bool,
    pub(super) scroll: usize,
    /// Set by the last draw: the last scroll offset and the page size.
    pub(super) max_scroll: usize,
    pub(super) page: usize,
    /// Set by the last draw: the full-screen item (the wheel over it
    /// scrolls it) and the box.
    pub(super) area: Rect,
    pub(super) strip: Rect,
    /// The drafts of the cards out of view (esc keeps them).
    pub(super) drafts: HashMap<u64, Editor>,
    /// The thread's draft while the view is open.
    pub(super) thread: Option<Editor>,
    /// Answered or closed here, still in the hub's last snapshot: hidden.
    answered: Vec<u64>,
    /// The cards whose fold line this TUI wrote, and the feed it went
    /// to: the hub's own line for the answer (`route : you → @x (answer
    /// to card #n) : …`) stays out of that feed (one line per answer).
    folded: Vec<(u64, String)>,
    /// Set by the last draw: what a click hits.
    pub(super) hits: RefCell<Vec<(Rect, CardHit)>>,
}

impl CardView {
    /// The fold of the last answer, while it shows (2 s).
    pub(super) fn fresh_fold(&self) -> Option<&Fold> {
        self.fold.as_ref().filter(|(_, at)| at.elapsed() < FOLD_FOR).map(|(f, _)| f)
    }

    /// Scroll by `d` rows (negative: up), within the card.
    pub(super) fn scroll_by(&mut self, d: isize) {
        self.scroll = if d < 0 {
            self.scroll.saturating_sub(d.unsigned_abs())
        } else {
            self.scroll.saturating_add(d.unsigned_abs()).min(self.max_scroll)
        };
    }

    pub(super) fn hit(&self, x: u16, y: u16) -> Option<CardHit> {
        let hits = self.hits.borrow();
        // the last drawn wins (an option over its row)
        hits.iter().rev().find(|(r, _)| x >= r.x && x < r.right() && y >= r.y && y < r.bottom()).map(|(_, h)| *h)
    }
}

/// What ⏎ does in the card view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enter {
    /// the text answers
    Answer,
    /// an approval: the text denies, as a note to the agent
    Deny,
    /// no words needed: an empty composer acknowledges (done, overlap)
    Ack,
}

/// A piece of a card's text in the view.
pub(super) enum Part {
    Text(String),
    /// a command or a patch: code colors on the raised tint
    Code(Vec<Vec<Span<'static>>>),
    /// why it runs (approvals), dim
    Reason(String),
    /// the hub's remark, dim italic
    Note(String),
    /// the facts under what it asks (a merge item's checks, its link):
    /// dim, right under the text; `[label](url)` is a link
    Evidence(String),
}

/// A card as the strip and the view show it.
pub(super) struct Shape {
    /// after the glyph, bold: `perf needs you`, `release wants to run`
    pub(super) title: String,
    /// the strip's name: `perf`, `release wants to run`
    pub(super) who: String,
    /// the strip's text: the first line, `npm publish · 13 lines`
    pub(super) summary: String,
    pub(super) parts: Vec<Part>,
    /// the options, whole (the view), and short (the strip)
    pub(super) options: Vec<String>,
    pub(super) short: Vec<String>,
    pub(super) enter: Enter,
    /// faint after the strip's text (`1 min · i ask before changing
    /// anything`)
    pub(super) note: String,
    /// the view's meta instead of `2 of 3 · 6m · ⌥1 perf`, its tab's
    /// label instead of the agent
    pub(super) meta: Option<String>,
    pub(super) tab: Option<String>,
    /// the strip's action when there are no options (`⏎ paste it`)
    pub(super) right: Option<(&'static str, &'static str)>,
    /// typed words answer it (`type to answer in your words`)
    pub(super) words: bool,
    /// the view's key bar, when the item has its own
    pub(super) keys: Vec<(&'static str, &'static str)>,
    /// each option's digit when not 1, 2, 3… (a gate's card: `no` is
    /// always 3, designer: a thumb that learned 3 = no never allows)
    pub(super) nums: Vec<usize>,
    /// the summary is a command: the box's row puts `$ ` before it
    pub(super) cmd: bool,
}

impl Shape {
    /// The digit of option `i`.
    pub(super) fn num(&self, i: usize) -> usize {
        self.nums.get(i).copied().unwrap_or(i + 1)
    }

    /// The option digit `d` picks.
    pub(super) fn by_digit(&self, d: usize) -> Option<usize> {
        (0..self.options.len()).find(|&i| self.num(i) == d)
    }
}

impl Shape {
    fn plain(title: String, who: String, summary: String, parts: Vec<Part>, options: Vec<String>, short: Vec<String>, enter: Enter) -> Shape {
        Shape {
            title,
            who,
            summary,
            parts,
            options,
            short,
            enter,
            note: String::new(),
            meta: None,
            tab: None,
            right: None,
            words: true,
            keys: Vec::new(),
            nums: Vec::new(),
            cmd: false,
        }
    }
}

/// The options of an approval (approvals.md §7), whole and short.
const ALLOW: [&str; 3] = ["allow once", "always here", "deny"];
const ALLOW_SHORT: [&str; 3] = ["allow", "always", "deny"];

/// The shape of card `c` (with `heard "…" → 1` while an answer by voice
/// waits on it).
pub(super) fn shape(c: &Card) -> Shape {
    with_heard(c, base_shape(c))
}

fn base_shape(c: &Card) -> Shape {
    if c.kind == "approval" {
        return approval_shape(c);
    }
    if c.kind == "confirm" {
        return confirm_shape(c);
    }
    if c.kind == "setup" {
        return setup_shape(c);
    }
    if choice_kind(&c.kind) {
        return choice_shape(c);
    }
    let (body, options) = if no_words(&c.kind) { (c.text.clone(), Vec::new()) } else { split_choices(&c.text) };
    let summary = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
    let mut parts = vec![Part::Text(body)];
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    Shape::plain(
        kind_title(&c.kind, &c.agent),
        if c.kind == "question" { c.agent.clone() } else { kind_title(&c.kind, &c.agent) },
        summary,
        parts,
        options.clone(),
        short_labels(&options),
        if no_words(&c.kind) { Enter::Ack } else { Enter::Answer },
    )
}

/// An approval: the command (or a patch) up to the first blank line,
/// then the reason.
fn approval_shape(c: &Card) -> Shape {
    let text = c.text.trim_matches('\n');
    let (head, reason) = match text.find("\n\n") {
        Some(i) => (&text[..i], text[i + 2..].trim()),
        None => (text, ""),
    };
    let lines: Vec<&str> = head.lines().collect();
    let patch = head.starts_with("diff --git") || head.starts_with("--- ");
    let (title, who, summary, code) = if patch {
        let files: Vec<&str> =
            lines.iter().filter_map(|l| l.strip_prefix("+++ ")).map(|f| f.trim().trim_start_matches("b/")).collect();
        let plus = lines.iter().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
        let minus = lines.iter().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();
        let first = files.first().copied().unwrap_or("a file");
        let more = match files.len() {
            0 | 1 => String::new(),
            n => format!(" +{} file{} ·", n - 1, if n > 2 { "s" } else { "" }),
        };
        let minus_sign = if theme::ascii_mode() { "-" } else { "−" };
        (
            format!("{} wants to edit {}", c.agent, first),
            format!("{} wants to edit", c.agent),
            format!("{first}{more} +{plus} {minus_sign}{minus}"),
            crate::code::highlight_patch(head),
        )
    } else {
        let first = lines.first().map_or("", |l| l.trim());
        let summary = if lines.len() > 1 { format!("{first} · {} lines", lines.len()) } else { first.to_string() };
        // the bash mark in accent before the command, like a gate's card
        let mut rows = crate::code::highlight_bash(head);
        if let Some(first) = rows.first_mut() {
            first.insert(0, Span::styled("$ ", Style::default().fg(theme::accent())));
        }
        (format!("{} wants to run", c.agent), format!("{} wants to run", c.agent), summary, rows)
    };
    let mut parts = vec![Part::Code(code)];
    if !reason.is_empty() {
        parts.push(Part::Reason(reason.to_string()));
    }
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    let mut s = Shape::plain(
        title,
        who,
        summary,
        parts,
        ALLOW.iter().map(|s| s.to_string()).collect(),
        ALLOW_SHORT.iter().map(|s| s.to_string()).collect(),
        Enter::Deny,
    );
    s.cmd = !patch;
    s
}

/// A gate's card (approvals-design.md §9), as the hub writes it: the
/// title (`wants to run`), `= ` the parts already allowed (dim, above),
/// `| ` what needs you, `reason: `, one `always: ` per rule it saves (none:
/// a hard rule, no option 2). Options: allow, always allow … here, no;
/// typed words say no, with the words as the note.
fn confirm_shape(c: &Card) -> Shape {
    let (mut head, mut done, mut body, mut reason, mut always) = (String::new(), vec![], vec![], String::new(), vec![]);
    for (i, l) in c.text.lines().enumerate() {
        if i == 0 {
            head = l.to_string();
        } else if let Some(x) = l.strip_prefix("= ") {
            done.push(x.to_string());
        } else if let Some(x) = l.strip_prefix("| ").or(l.strip_prefix("|")) {
            body.push(x.to_string());
        } else if let Some(x) = l.strip_prefix("reason: ") {
            reason = x.to_string();
        } else if let Some(x) = l.strip_prefix("always: ") {
            always.push(x.to_string());
        }
    }
    let n = c.agent.split(", ").count();
    let title = if n > 1 {
        format!("{} agents {}", n, head.replacen("wants", "want", 1))
    } else {
        format!("{} {}", c.agent, head)
    };
    // `wants to run it outside the sandbox`: a sandbox card (brief 1e)
    let bash = head.starts_with("wants to run");
    let rerun = head.ends_with("outside the sandbox");
    let edit = head.starts_with("wants to edit");
    // an edit or a connector: its first 3 lines (after the path), the rest counted
    let (shown, more) = if bash {
        (body.clone(), 0)
    } else {
        let keep = if edit { 4 } else { 3 };
        (body.iter().take(keep).cloned().collect::<Vec<_>>(), body.len().saturating_sub(keep))
    };
    let mut parts: Vec<Part> = done.iter().map(|d| Part::Reason(format!("$ {d}"))).collect();
    let text = shown.join("\n");
    parts.push(Part::Code(if bash {
        // the bash mark in accent before the command (designer)
        let mut rows = crate::code::highlight_bash(&text);
        if let Some(first) = rows.first_mut() {
            first.insert(0, Span::styled("$ ", Style::default().fg(theme::accent())));
        }
        rows
    } else if edit {
        crate::code::highlight_patch(&text)
    } else {
        text.lines().map(|l| vec![Span::raw(l.to_string())]).collect()
    }));
    if more > 0 {
        parts.push(Part::Reason(format!("{} {} more line{}", theme::glyph("▸"), more, if more == 1 { "" } else { "s" })));
    }
    if !reason.is_empty() {
        parts.push(Part::Reason(reason));
    }
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    let first = body.first().map_or("", |l| l.trim());
    let summary = if body.len() > 1 && bash { format!("{first} · {} lines", body.len()) } else { first.to_string() };
    let mut options = vec![if rerun { "run it again without the sandbox" } else { "allow" }.to_string()];
    let mut short = vec![if rerun { "run again" } else { "allow" }.to_string()];
    if !always.is_empty() {
        options.push(always_option(rerun, &always, &body));
        short.push("always".into());
    }
    options.push("no".into());
    short.push("no".into());
    let who = title.clone();
    let hard = always.is_empty();
    let mut s = Shape::plain(title, who, summary, parts, options, short, Enter::Deny);
    s.note = String::new();
    s.cmd = bash;
    // `no` is always 3 (designer): a hard rule has no 2
    if hard {
        s.nums = vec![1, 3];
    }
    s
}

/// A confirm card's option 2. A sandbox card's saves a "run outside the
/// sandbox" rule, and says so (designer): `it` when the rule is the
/// command shown, else the rule.
fn always_option(rerun: bool, always: &[String], body: &[String]) -> String {
    let rules = always.join(", ");
    if !rerun {
        format!("always allow {rules} here")
    } else if rules == body.join("\n").trim() {
        "always run it outside the sandbox here".to_string()
    } else {
        format!("always run {rules} outside the sandbox here")
    }
}

impl super::Sb {
    /// expired-ux: the agent in view stopped on the expired ChatGPT
    /// sign-in (the open `signin` item names it): ⏎ on an empty composer
    /// signs in again.
    pub(crate) fn waits_for_sign_in(&self) -> bool {
        let me = self.focus_name();
        self.cards.iter().any(|c| c.kind == "signin" && c.waiting.iter().any(|a| a == me))
    }
}

/// The hub's items with numbered options, about a place (the hub's
/// `choice_kind`): a digit answers them (the hub acts, then closes the
/// item); typed words go to main.
pub(super) fn choice_kind(kind: &str) -> bool {
    matches!(kind, "merge" | "feature_try" | "feature_merge" | "update" | "signin")
}

/// A hub item, as the hub writes it: its head line (`#409 is ready to
/// merge`), then its body (a merge: the PR's title, its facts dim, its
/// link), a blank line, the numbered options.
fn choice_shape(c: &Card) -> Shape {
    if c.asking && c.kind == "feature_merge" {
        return drop_ask_shape(c);
    }
    let (body, options) = split_choices(&c.text);
    let mut lines = body.trim_end().lines();
    let head = lines.next().unwrap_or("").trim().to_string();
    let rest: Vec<&str> = lines.collect();
    let mut parts = Vec::new();
    if c.kind == "merge" {
        // designer: the title is what you decide on (text), the rest is
        // evidence (dim): the facts, the link (clickable)
        let minus = if theme::ascii_mode() { "-" } else { "−" };
        let rest: Vec<&str> = rest.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        let (title, evidence) = match rest.len() {
            0..=2 => (None, &rest[..]),
            _ => (Some(rest[0]), &rest[1..]),
        };
        if let Some(t) = title {
            parts.push(Part::Text(t.to_string()));
        }
        for l in evidence {
            parts.push(Part::Evidence(if l.contains("/pull/") && !l.contains(' ') {
                let url = if l.starts_with("http") { l.to_string() } else { format!("https://{l}") };
                format!("[{l}]({url})")
            } else {
                l.replace('−', minus)
            }));
        }
    } else if c.kind == "update" {
        // update-card (designer): the notes are text, the running version
        // (the last line) is dim; no link line, `3` opens it
        let rest: Vec<&str> = rest.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if let Some((on, notes)) = rest.split_last() {
            if !notes.is_empty() {
                parts.push(Part::Text(notes.join("\n")));
            }
            parts.push(Part::Evidence(on.to_string()));
        }
    } else {
        parts.push(Part::Text(rest.join("\n").trim().to_string()));
    }
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    let options: Vec<String> = if theme::ascii_mode() { options.iter().map(|o| o.replace('↗', "->")).collect() } else { options };
    let short = short_labels(&options);
    if c.kind == "update" {
        // the hub's own news, not an agent's: the head alone
        // the row: `bise · v0.0.2 is out`
        let summary = head.strip_prefix("bise ").unwrap_or(&head).to_string();
        return Shape::plain(head.clone(), "bise".into(), summary, parts, options, short, Enter::Answer);
    }
    if c.kind == "signin" {
        // expired-ux: bise's own item too, the head alone
        return Shape::plain(head.clone(), "bise".into(), head, parts, options, short, Enter::Answer);
    }
    Shape::plain(format!("{}: {}", c.agent, head), c.agent.clone(), head, parts, options, short, Enter::Answer)
}

/// A feature merge item after `3 drop the branch` (designer, d4172e6):
/// it asks once more, on the same item, `drop computer-use? 14 commits
/// go.` `1 drop it` / `2 keep it`. The name from its place, the count
/// from its text (`14 commits · the check passes.`).
fn drop_ask_shape(c: &Card) -> Shape {
    let name = c
        .place
        .as_deref()
        .and_then(|p| p.strip_prefix("feature:"))
        .unwrap_or("the branch")
        .to_string();
    let commits = c
        .text
        .lines()
        .find_map(|l| {
            let mut w = l.split_whitespace();
            let n = w.next()?.parse::<usize>().ok()?;
            w.next().filter(|x| x.starts_with("commit")).map(|_| n)
        })
        .unwrap_or(0);
    let head = format!("drop {}? {} commit{} go.", name, commits, if commits == 1 { "" } else { "s" });
    let options = vec!["drop it".to_string(), "keep it".to_string()];
    let short = short_labels(&options);
    Shape::plain(format!("{}: {}", c.agent, head), c.agent.clone(), head, Vec::new(), options, short, Enter::Answer)
}

/// The link a merge item's `2` opens: its PR's, from the place's box
/// (the hub's snapshot), else the link line of its text.
pub(super) fn merge_link(sb: &Sb, c: &Card) -> Option<String> {
    let from_place = c
        .place
        .as_ref()
        .and_then(|id| sb.places.iter().find(|p| p.id == *id))
        .and_then(|p| p.pr.as_ref())
        .filter(|pr| c.pr.is_none_or(|n| n == pr.number))
        .map(|pr| pr.url.clone());
    from_place.or_else(|| {
        let (body, _) = split_choices(&c.text);
        let tag = format!("/pull/{}", c.pr?);
        body.lines().map(str::trim).find(|l| l.ends_with(&tag)).map(|l| {
            if l.starts_with("http") {
                l.to_string()
            } else {
                format!("https://{l}")
            }
        })
    })
}

/// A setup item (BISE-245): its look, written by setup.rs; without one
/// (never from setup.rs), its first line in the strip, a diff in code
/// colors, the rest as text.
fn setup_shape(c: &Card) -> Shape {
    let Some(l) = c.look.as_deref() else {
        let (body, options) = split_choices(&c.text);
        let summary = body.lines().next().unwrap_or("").trim().to_string();
        let parts = body
            .split("\n\n")
            .map(|p| if p.starts_with("--- ") { Part::Code(crate::code::highlight_patch(p)) } else { Part::Text(p.to_string()) })
            .collect();
        let mut s = Shape::plain(c.agent.clone(), c.agent.clone(), summary, parts, options.clone(), options, Enter::Answer);
        s.words = false;
        return s;
    };
    let parts = l
        .body
        .iter()
        .map(|p| match p {
            Para::Text(t) => Part::Text(t.clone()),
            Para::Dim(t) => Part::Reason(t.clone()),
            Para::Diff(t) => Part::Code(crate::code::highlight_patch(t)),
        })
        .collect();
    let mut s = Shape::plain(l.title.clone(), c.agent.clone(), l.row.clone(), parts, l.options.clone(), l.options.clone(), Enter::Answer);
    s.note = l.row_note.clone();
    s.meta = Some(l.meta.clone()).filter(|m| !m.is_empty());
    s.tab = Some(l.tab.clone()).filter(|t| !t.is_empty());
    s.right = l.right;
    s.words = false;
    s.keys = l.keys.clone();
    s
}

/// The strip's labels of `options`: their first words when those differ
/// (`compress`, `both`), else the start of each, cut.
fn short_labels(options: &[String]) -> Vec<String> {
    let first: Vec<String> = options
        .iter()
        .map(|o| o.split_whitespace().next().unwrap_or("").trim_end_matches([':', ',', '.', ';']).to_string())
        .collect();
    let distinct = first.iter().enumerate().all(|(i, a)| !a.is_empty() && !first[..i].contains(a));
    if distinct {
        first
    } else {
        options.iter().map(|o| cut(o, 16)).collect()
    }
}

/// `s` in at most `w` columns, cut with `…`.
pub(super) fn cut(s: &str, w: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if s.width() <= w {
        return s.to_string();
    }
    let e = theme::ellipsis();
    let room = w.saturating_sub(e.width());
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > room {
            break;
        }
        used += cw;
        out.push(ch);
    }
    let out = out.trim_end().to_string();
    if w >= e.width() {
        out + e
    } else {
        out
    }
}

impl Sb {
    /// The cards in reading order, the ones answered here left out: what
    /// blocks an agent first (approvals, questions), then the oldest.
    pub(super) fn sorted_cards(&self) -> Vec<&Card> {
        let mut v: Vec<&Card> = self.cards.iter().filter(|c| !self.card.answered.contains(&c.id)).collect();
        v.sort_by_key(|c| (kind_look(&c.kind).0, c.id));
        v
    }

    /// Card `id` was answered or closed here (the hub's snapshot still
    /// has it): its agent no longer waits on you.
    pub(super) fn answered_here(&self, id: u64) -> bool {
        self.card.answered.contains(&id)
    }

    /// This TUI wrote card `id`'s fold line in `agent`'s feed.
    pub(super) fn folded_in(&self, id: u64, agent: &str) -> bool {
        self.card.folded.iter().any(|(i, a)| *i == id && a == agent)
    }

    pub(super) fn card_ids(&self) -> Vec<u64> {
        self.sorted_cards().iter().map(|c| c.id).collect()
    }

    /// The card in the view while it is open, else the top card.
    pub(super) fn current_card(&self) -> Option<&Card> {
        let v = self.sorted_cards();
        self.card
            .sel
            .filter(|_| self.card.open)
            .and_then(|id| v.iter().find(|c| c.id == id).copied())
            .or_else(|| v.first().copied())
    }

    pub(super) fn card_by_id(&self, id: u64) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }
}

/// A card kind: its rank in reading order (what blocks an agent first:
/// bise-proto's `card_rank`, the one table, which the desktop's inbox
/// reads too), its glyph and its color. Color means attention (book §5):
/// needs you in accent, failures in error, the rest plain text.
pub(super) fn kind_look(kind: &str) -> (u8, &'static str, Color) {
    let (glyph, color) = match kind {
        // a PR ready to merge (pr-design §6.3): yours to act on, pink
        "approval" | "confirm" | "question" | "merge" | "feature_try" | "feature_merge" | "signin" | "blocked" | "setup" => (theme::G_NEEDS_YOU, theme::accent()),
        "failed" => (theme::G_FAILED, theme::error()),
        "restart" => (theme::G_RESTART_FAILED, theme::error()),
        "drop" => (theme::G_STOPPED, theme::text()),
        "overlap" => (theme::G_OVERLAP, theme::text()),
        "done" => (theme::done_glyph(), theme::text()),
        _ => (theme::G_CARD, theme::accent()),
    };
    (bise_proto::rows::card_rank(kind), glyph, color)
}

/// The color of a card kind's glyph: its hue, except done's check, in
/// accent on a plain title (BISE-100, book §6).
pub(super) fn glyph_color(kind: &str) -> Color {
    match kind {
        "done" => theme::accent(),
        k => kind_look(k).2,
    }
}

/// A card's title, after its glyph (copy deck §17: `{name} needs you`).
fn kind_title(kind: &str, agent: &str) -> String {
    match kind {
        "question" => format!("{agent} asks"),
        "blocked" => format!("{agent} is blocked"),
        "failed" => format!("{agent} failed"),
        "restart" => "restart failed".into(),
        "drop" => format!("drop {agent}?"),
        "overlap" => "overlap".into(),
        "done" => format!("{agent} is done"),
        k => format!("{agent}: {k}"),
    }
}

/// The cards answered with no words: ⏎ on an empty composer.
fn no_words(kind: &str) -> bool {
    matches!(kind, "done" | "overlap")
}

// ---- the actions ----

/// Open card `id` in place, in its box (none, or gone: the top card).
/// The thread's draft steps aside (kept, its cursor too) until esc.
pub(super) fn open_view(app: &mut App, id: Option<u64>) {
    let ids = app.sb.card_ids();
    let Some(id) = id.filter(|i| ids.contains(i)).or_else(|| ids.first().copied()) else { return };
    if !app.sb.card.open {
        app.sb.card.thread = Some(std::mem::take(&mut app.ed));
        app.sb.card.open = true;
        app.sb.card.full = false;
        app.sb.card.sel = None;
    }
    show(app, id);
}

/// Back to your message: the item folds back to its row (its draft
/// kept), the thread's draft comes back.
pub(super) fn close_view(app: &mut App) {
    if !app.sb.card.open {
        return;
    }
    park_draft(app);
    let cv = &mut app.sb.card;
    app.ed = cv.thread.take().unwrap_or_default();
    cv.open = false;
    cv.full = false;
    cv.fold = None;
    cv.scroll = 0;
}

/// The composer's draft goes to the card in view's slot.
fn park_draft(app: &mut App) {
    let cv = &mut app.sb.card;
    if let Some(old) = cv.sel.take() {
        let d = std::mem::take(&mut app.ed);
        if d.text.is_empty() {
            cv.drafts.remove(&old);
        } else {
            cv.drafts.insert(old, d);
        }
    }
}

/// Card `id` in the view, with its draft in the composer.
fn show(app: &mut App, id: u64) {
    if app.sb.card.sel == Some(id) {
        return;
    }
    park_draft(app);
    let cv = &mut app.sb.card;
    app.ed = cv.drafts.remove(&id).unwrap_or_default();
    cv.sel = Some(id);
    cv.opt = None;
    cv.reveal = false;
    cv.scroll = 0;
    cv.max_scroll = 0;
}

/// ↑↓ (no wrap) and ctrl+n / ctrl+p (around): the next or previous
/// item.
fn step(app: &mut App, d: isize, wrap: bool) {
    let ids = app.sb.card_ids();
    if ids.is_empty() {
        return;
    }
    let i = app.sb.card.sel.and_then(|s| ids.iter().position(|x| *x == s)).unwrap_or(0) as isize;
    let j = if wrap { (i + d).rem_euclid(ids.len() as isize) } else { (i + d).clamp(0, ids.len() as isize - 1) } as usize;
    show(app, ids[j]);
}

/// Answer card `id` with `reply`; the history says the box's fold line
/// (`✓ you answered perf: both`); the view moves on.
/// `picked`: the short label of the option picked (on the line).
fn answer(app: &mut App, id: u64, reply: &str, picked: Option<&str>) {
    // the TUI's own cards: answered here, their own result row
    if super::setup::is_local(id) {
        if super::setup::valid(app, id, reply) {
            retire(app, id);
            super::setup::answer(app, id, reply);
        }
        return;
    }
    let Some(c) = app.sb.card_by_id(id).cloned() else { return };
    app.sb.send_input(format!("/answer {} {}", id, reply));
    // the thread says the box's fold line (designer: one sentence); a
    // gate's card folds with the hub's own (`✓ you allowed …`)
    let fold = fold_of(&c, reply, picked);
    if c.kind != "confirm" {
        let ev = Ev::Approval { ok: fold.ok, text: fold.text.clone(), note: fold.note.clone(), asked: fold.asked.clone(), open: false };
        push_event(&mut app.events, &mut app.cache, ev);
        app.sb.card.folded.push((id, app.sb.focus.clone()));
    }
    let open = app.sb.card.open && app.sb.card.sel == Some(id);
    retire(app, id);
    if !open {
        return;
    }
    // the next item opened: the answer folds on top of the box for 2 s;
    // the last one: the box goes, the divider says `✓ inbox clear`
    if app.sb.card.open {
        app.sb.card.fold = Some((fold, std::time::Instant::now()));
    } else {
        app.flash = Some(("inbox clear".into(), std::time::Instant::now()));
    }
}

/// The one line an answer folds into (the hub's words for a gate's
/// card, approvals-design.md §9): `you allowed t3: npm publish`, `you
/// said no to t3: npm publish` and the note, `you answered cookies:
/// smaller`.
pub(super) fn fold_of(c: &Card, reply: &str, picked: Option<&str>) -> Fold {
    let gate = matches!(c.kind.as_str(), "confirm" | "approval");
    if !gate {
        // a picked option: its short label on the line (`both`)
        let (text, note) = match picked {
            Some(p) => (format!("you answered {}: {}", c.agent, p), String::new()),
            None => answered(&c.agent, reply),
        };
        return Fold { ok: true, text, note, asked: c.text.trim().to_string() };
    }
    let what = shape(c).summary;
    let no = reply == "no" || reply == "deny" || reply.starts_with("deny: ");
    if no {
        let note = reply.strip_prefix("deny: ").unwrap_or("").trim().to_string();
        return Fold { ok: false, text: format!("you said no to {}: {}", c.agent, what), note, asked: String::new() };
    }
    let outside = c.text.lines().next().is_some_and(|h| h.ends_with("outside the sandbox"));
    let text = if outside {
        format!("you let {} run it outside the sandbox: {}", c.agent, what)
    } else {
        format!("you allowed {}: {}", c.agent, what)
    };
    Fold { ok: true, text, note: String::new(), asked: String::new() }
}

/// The sentence of an answer to `who` and the words under it (BISE-307,
/// designer): a picked option or any short one-line answer stays on the
/// line (`you answered perf: both`), anything longer goes under it, whole
/// (`you answered main` + the words). The same split for the box's own
/// fold and the hub's line (another view, a reload).
pub(super) fn answered(who: &str, words: &str) -> (String, String) {
    // bise_proto's words (the hub's fold of the route line says the same)
    bise_proto::thread::words::answered_split(who, words, &unicode_width::UnicodeWidthStr::width)
}

/// Card `id` is answered or closed: hidden until the hub drops it; the
/// view goes to the next card, or back to the thread.
fn retire(app: &mut App, id: u64) {
    let ids = app.sb.card_ids();
    let cv = &mut app.sb.card;
    cv.answered.push(id);
    cv.drafts.remove(&id);
    if !(cv.open && cv.sel == Some(id)) {
        return;
    }
    // the answer is sent: its text is gone from the composer
    app.ed = Editor::default();
    cv.sel = None;
    let i = ids.iter().position(|x| *x == id).unwrap_or(0);
    let next = ids.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| ids.get(j))).copied();
    match next {
        Some(n) => show(app, n),
        None => close_view(app),
    }
}

/// Option `i` of card `id` answers it (a digit, a click).
/// The option of card `id` whose digit is `d`.
fn pick_digit(app: &mut App, id: u64, d: usize) -> bool {
    match app.sb.card_by_id(id).map(shape).and_then(|s| s.by_digit(d)) {
        Some(i) => pick(app, id, i),
        None => false,
    }
}

fn pick(app: &mut App, id: u64, i: usize) -> bool {
    let Some(c) = app.sb.card_by_id(id).cloned() else { return false };
    let s = shape(&c);
    let (Some(reply), Some(said)) = (s.options.get(i), s.short.get(i)) else { return false };
    // an approval answers with its digit (the composer rule: sb-core takes
    // only 1, 2 or 3 on it, never words), the history says the option's
    if matches!(c.kind.as_str(), "confirm" | "approval") {
        answer(app, id, &s.num(i).to_string(), Some(said));
        return true;
    }
    if !choice_kind(&c.kind) {
        answer(app, id, reply, Some(said));
        return true;
    }
    // a hub item: its digit answers (the hub acts on it), the history
    // says the option's words
    let n = s.num(i);
    // dev-flow §5.1: `3 drop the branch` deletes work, so it asks once
    // more on the same item; `1 drop it` sends the 3, `2 keep it` goes back
    if c.kind == "feature_merge" {
        let ask = |app: &mut App, on: bool| {
            app.sb.feature_drop_ask = on.then_some(id);
            if let Some(x) = app.sb.cards.iter_mut().find(|x| x.id == id) {
                x.asking = on;
            }
        };
        match (c.asking, n) {
            (false, 3) => {
                ask(app, true);
                return true;
            }
            (true, 1) => {
                ask(app, false);
                answer(app, id, "3", Some("drop the branch"));
                return true;
            }
            (true, _) => {
                ask(app, false);
                return true;
            }
            _ => {}
        }
    }
    if c.kind == "merge" && n == 2 {
        // `open it on GitHub`: here, and the item stays (pr-design §6.3)
        if let Some(url) = merge_link(&app.sb, &c) {
            crate::links::open(&url);
        }
        return true;
    }
    if c.kind == "signin" && n == 1 {
        // expired-ux `sign in again`: the TUI's own ChatGPT sign-in (the
        // browser, resign.rs); the hub closes the item once auth.json
        // says it's back
        crate::resign::start(app);
        return true;
    }
    if c.kind == "update" && n == 3 {
        // update-card `release notes ↗`: here, and the item stays
        if let Some(url) = c.link.as_deref() {
            crate::links::open(url);
        }
        return true;
    }
    answer(app, id, &n.to_string(), Some(reply));
    true
}

/// ⏎ in the card view: the text answers (an approval: denies with the
/// text as a note); an empty composer acknowledges a card that needs no
/// words, else does nothing.
fn submit(app: &mut App) {
    let Some((id, enter)) = app.sb.current_card().map(|c| (c.id, shape(c).enter)) else { return };
    let text = app.ed.text.trim().to_string();
    // a setup card: never in the history (the key card's text is a key)
    if super::setup::is_local(id) {
        if !text.is_empty() {
            answer(app, id, &text, None);
        }
        return;
    }
    if text.is_empty() {
        if enter == Enter::Ack {
            answer(app, id, "seen", Some("seen"));
        }
        return;
    }
    app.history.insert(0, app.ed.text.clone());
    // the chips go out whole, as a message to the thread would
    // (input.rs): a paste's text inline in its tag, an image's marker
    // with its saved path; a bare `[Paste #1]` reached the asker empty
    let text = crate::attach::expand(app, &text);
    match enter {
        Enter::Deny => answer(app, id, &format!("deny: {text}"), None),
        _ => answer(app, id, &text, None),
    }
}

/// ctrl+x: close card `id` without answering.
fn close_card(app: &mut App, id: u64) {
    if app.sb.card_by_id(id).is_none() {
        return;
    }
    if super::setup::is_local(id) {
        retire(app, id);
        super::setup::close(app, id);
        return;
    }
    app.sb.send_input(format!("/close {}", id));
    retire(app, id);
}

/// A new snapshot: forget what the hub closed; the card in view went
/// away (answered elsewhere): its draft goes to the history (↑ brings it
/// back) and the view moves on.
pub(super) fn sync(app: &mut App) {
    let live: Vec<u64> = app.sb.cards.iter().map(|c| c.id).collect();
    let cv = &mut app.sb.card;
    cv.answered.retain(|x| live.contains(x));
    cv.drafts.retain(|k, _| live.contains(k));
    if !cv.open {
        return;
    }
    let ids = app.sb.card_ids();
    let cv = &mut app.sb.card;
    if cv.sel.is_some_and(|s| ids.contains(&s)) {
        return;
    }
    let d = std::mem::take(&mut app.ed);
    if !d.text.trim().is_empty() {
        app.history.insert(0, d.text);
    }
    cv.sel = None;
    match ids.first() {
        Some(&n) => show(app, n),
        None => close_view(app),
    }
}

/// ctrl+1-9 (BISE-302): the digit, for inbox row N; any other key: none.
/// Ctrl with or without shift (a layout where digits need shift). On a
/// French layout the kitty protocol reports the top row's own character
/// (`&é"'(-è_çà`, a Mac's `§` and `!`): it counts as its digit.
pub(crate) fn ctrl_digit(k: &crossterm::event::KeyEvent) -> Option<usize> {
    if k.modifiers != KeyModifiers::CONTROL && k.modifiers != KeyModifiers::CONTROL | KeyModifiers::SHIFT {
        return None;
    }
    let KeyCode::Char(c) = k.code else { return None };
    let d = match c {
        '1'..='9' => c as usize - '0' as usize,
        '&' => 1,
        'é' => 2,
        '"' => 3,
        '\'' => 4,
        '(' => 5,
        '-' | '§' => 6,
        'è' => 7,
        '_' | '!' => 8,
        'ç' => 9,
        _ => return None,
    };
    Some(d)
}

/// A key only a terminal that reports ctrl+digits sends (BISE-302): one
/// without them turns ctrl+1 into `1`, ctrl+2 into ctrl+space, ctrl+3
/// into esc, ctrl+8 into backspace and ctrl+9 into `9`; ctrl+4-7 come as
/// they are from any terminal (the bytes 0x1c-0x1f), so they prove
/// nothing.
pub(crate) fn proves_ctrl_digits(k: &crossterm::event::KeyEvent) -> bool {
    matches!(ctrl_digit(k), Some(1 | 2 | 3 | 8 | 9))
}

/// Inbox row `n` (1 = the most blocking, the box's numbers) opens in
/// place; with an item open, it jumps there (full screen stays full
/// screen); past the last row: nothing. `true` when it opened.
pub(super) fn open_row(app: &mut App, n: usize) -> bool {
    let rows = super::card_draw::strip_ids(&app.sb);
    let Some(&id) = n.checked_sub(1).and_then(|i| rows.get(i)) else { return false };
    if app.sb.card.open {
        show(app, id);
    } else {
        open_view(app, Some(id));
    }
    true
}

/// ←→: highlight the option before or after, no wrap: the first →
/// lands on option 1, the first ← on the last.
fn choose(app: &mut App, right: bool, n: usize) {
    if n == 0 {
        return;
    }
    let cv = &mut app.sb.card;
    cv.opt = Some(match cv.opt.map(|i| i.min(n - 1)) {
        None if right => 0,
        None => n - 1,
        Some(i) if right => (i + 1).min(n - 1),
        Some(i) => i.saturating_sub(1),
    });
    cv.reveal = true;
}

/// The card keys; `true` when handled. From the thread only ctrl+1-9
/// (open inbox row N in place, BISE-302); the rest with an item open.
pub(super) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    if let Some(n) = ctrl_digit(k) {
        return open_row(app, n);
    }
    if !app.sb.card.open {
        return false;
    }
    let empty = app.ed.text.is_empty();
    let sel = app.sb.current_card().map(|c| c.id);
    let n = app.sb.current_card().map_or(0, |c| shape(c).options.len());
    let full = app.sb.card.full;
    // the arrows are the inbox's on an empty composer, else your text's;
    // ctrl+arrows are silent aliases (macOS keeps them for Spaces)
    let arrows = empty && !popup_open;
    let arrow_mods = k.modifiers == KeyModifiers::NONE || k.modifiers == KeyModifiers::CONTROL;
    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) if !popup_open => close_view(app),
        // full screen and back in place
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
            let cv = &mut app.sb.card;
            cv.full = !cv.full;
            cv.scroll = 0;
            cv.reveal = cv.opt.is_some();
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => step(app, 1, true),
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => step(app, -1, true),
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            if let Some(id) = sel {
                close_card(app, id);
            }
        }
        // an empty composer picks the option highlighted (none: nothing,
        // a reflex ⏎ never answers); text answers
        (KeyCode::Enter, KeyModifiers::NONE) if !popup_open => match (sel, app.sb.card.opt.filter(|_| empty)) {
            (Some(id), Some(i)) if i < n => {
                pick(app, id, i);
            }
            _ => submit(app),
        },
        // 1-9 picks on an empty composer; once you typed, digits are text
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) if empty => {
            return sel.is_some_and(|id| pick_digit(app, id, c as usize - '0' as usize));
        }
        // full screen scrolls; in place the thread's pgup/pgdn stay
        (KeyCode::PageUp, _) if full && !popup_open => {
            let page = app.sb.card.page.max(1) as isize;
            app.sb.card.scroll_by(-page);
        }
        (KeyCode::PageDown, _) if full && !popup_open => {
            let page = app.sb.card.page.max(1) as isize;
            app.sb.card.scroll_by(page);
        }
        // ↑↓ the previous / next item, stacked like the rows (no wrap;
        // never the thread's history recalled into an answer)
        (KeyCode::Up | KeyCode::Down, _) if arrows && arrow_mods => step(app, if k.code == KeyCode::Down { 1 } else { -1 }, false),
        // ←→ walk the options, on one line like them
        (KeyCode::Left | KeyCode::Right, _) if arrows && arrow_mods => choose(app, k.code == KeyCode::Right, n),
        // no queue for an answer
        (KeyCode::Tab, _) if !popup_open => {}
        _ => return false,
    }
    true
}

/// The mouse on the box or the full-screen item; `true` when handled.
pub(crate) fn card_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let cv = &app.sb.card;
    let a = cv.area;
    let over = |r: Rect| m.column >= r.x && m.column < r.right() && m.row >= r.y && m.row < r.bottom();
    match m.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if cv.open && cv.full && over(a) => {
            let d = if m.kind == MouseEventKind::ScrollUp { -3 } else { 3 };
            app.sb.card.scroll_by(d);
            true
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(hit) = cv.hit(m.column, m.row) else { return false };
            match hit {
                CardHit::Row(id) => open_view(app, Some(id)),
                CardHit::Open => open_view(app, None),
                CardHit::Tab(id) => show(app, id),
                CardHit::Pick(id, i) => {
                    pick(app, id, i);
                }
            }
            true
        }
        _ => false,
    }
}

/// The choices an agent gives at the end of its question: its last
/// lines `1. v1` / `1) v1` / `1 - v1`, numbered 1, 2, … (two to nine).
/// Returns the text without them, and the choices (none: the text as is).
pub(crate) fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))
            // '1 a PR per task' (pm's C fail 41): a bare space too; only
            // a run of 2+ such lines numbered from 1 ends a card
            .or_else(|| rest.strip_prefix(' ').filter(|r| !r.starts_with(['-', '–', ' '])))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return split_inline(text).unwrap_or_else(|| (text.to_string(), Vec::new()));
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}


/// The choices written inline at the end of the last line (ambient-lead
/// m_6486, main's real shape: 'Que fais-tu ? 1. regarde … 2. arrête …
/// 3. laisse …'): ' 1. ' (or ' 1) ') then ' 2. ' … in order, two to
/// nine, the question before the first. None: no such list.
fn split_inline(text: &str) -> Option<(String, Vec<String>)> {
    let t = text.trim_end();
    let line_at = t.rfind('\n').map_or(0, |i| i + 1);
    let line = &t[line_at..];
    for sep in [". ", ") "] {
        // the markers ' n. ' in order, each after a space (or at the start)
        let mut at: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        for n in 1..=9 {
            let m = format!("{n}{sep}");
            let found = line[from..].match_indices(&m).map(|(i, _)| from + i).find(|&i| i == 0 || line[..i].ends_with(' '));
            match found {
                Some(i) => {
                    at.push((i, i + m.len()));
                    from = i + m.len();
                }
                None => break,
            }
        }
        if at.len() < 2 {
            continue;
        }
        let options: Vec<String> = at
            .iter()
            .enumerate()
            .map(|(k, &(_, start))| {
                let end = at.get(k + 1).map_or(line.len(), |&(i, _)| i);
                line[start..end].trim().trim_end_matches([',', ';']).trim().to_string()
            })
            .collect();
        if options.iter().any(|o| o.is_empty()) {
            continue;
        }
        let body = format!("{}{}", &t[..line_at], line[..at[0].0].trim_end()).trim_end().to_string();
        return Some((body, options));
    }
    None
}

/// The open cards matching `q` (their number, kind, agent or text):
/// the card argument of `/close` and `/answer`.
pub(crate) fn card_choices(app: &App, q: &str) -> Vec<Choice> {
    app.sb
        .sorted_cards()
        .into_iter()
        .filter(|c| card_matches(q, c.id, &[&c.kind, &c.agent, &c.text]))
        .map(|c| {
            let (_, icon, _) = kind_look(&c.kind);
            Choice {
                value: c.id.to_string(),
                label: if super::setup::is_local(c.id) { "setup".into() } else { format!("#{}", c.id) },
                desc: format!("{} @{} · {}", c.kind, c.agent, truncate_chars(&one_line(&c.text), 80)),
                mark: Some((icon, glyph_color(&c.kind))),
            }
        })
        .collect()
}

/// A card for the typed `q`: a number is the start of the card's number
/// (`99` finds #994, never #1999), and a setup card's number (2^50 + n,
/// setup::LOCAL) is not the user's: it matches `setup` instead (`/close
/// 999` had picked a setup card whose id holds "999", and the hub then
/// answered `no open card #1125899906842624`).
fn card_matches(q: &str, id: u64, fields: &[&str]) -> bool {
    let q = q.trim_start_matches('#');
    let local = super::setup::is_local(id);
    if !q.is_empty() && q.bytes().all(|b| b.is_ascii_digit()) {
        return !local && id.to_string().starts_with(q);
    }
    let num = if local { "setup".to_string() } else { id.to_string() };
    crate::commands::matches(q, &[&num, fields[0], fields[1], fields[2]])
}

/// The text on one line: runs of whitespace become one space.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- answers by voice (voice mode, design §5) ----
//
// In voice mode the turn after a question or an approval is matched
// first (`voicemode::answers`): a match shows `heard "the first one" →
// 1 smaller` on the item for 1.5 s (esc undoes), then counts.
// (allow(dead_code) on the entry points until voice mode's integration
// calls them; voicemode/mod.rs has the same.)

thread_local! {
    /// The item an answer was heard for, the line, and since when.
    static HEARD: RefCell<Option<(u64, String, std::time::Instant)>> = const { RefCell::new(None) };
}

/// What voice mode can answer: the item in the view (else the top one),
/// an approval or not, its options' words.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VoiceQuestion {
    pub(crate) id: u64,
    pub(crate) approval: bool,
    pub(crate) options: Vec<String>,
}

#[allow(dead_code)]
pub(crate) fn voice_question(app: &App) -> Option<VoiceQuestion> {
    let c = app.sb.current_card()?;
    let s = shape(c);
    (!s.options.is_empty()).then(|| VoiceQuestion { id: c.id, approval: c.kind == "approval", options: s.options })
}

/// `heard "…" → 1 smaller` on item `id` from `now` for
/// [`crate::voicemode::answers::HEARD_FOR`].
#[allow(dead_code)]
pub(crate) fn show_heard(id: u64, line: String, now: std::time::Instant) {
    HEARD.with(|h| *h.borrow_mut() = Some((id, line, now)));
}

/// esc: the heard answer does not count.
#[allow(dead_code)]
pub(crate) fn undo_heard() {
    HEARD.with(|h| *h.borrow_mut() = None);
}

/// The heard line on item `id`, while it shows.
pub(crate) fn heard_on(id: u64, now: std::time::Instant) -> Option<String> {
    HEARD.with(|h| {
        h.borrow().as_ref().and_then(|(at_id, line, at)| {
            (*at_id == id && now.saturating_duration_since(*at) < crate::voicemode::answers::HEARD_FOR).then(|| line.clone())
        })
    })
}

/// The heard answer counts: option `i` of item `id`, as a key would
/// pick it; the line goes.
#[allow(dead_code)]
pub(crate) fn answer_by_voice(app: &mut App, id: u64, i: usize) -> bool {
    undo_heard();
    pick(app, id, i)
}

/// The item's shape with the heard line under its body, while it shows.
fn with_heard(c: &Card, mut s: Shape) -> Shape {
    if let Some(line) = heard_on(c.id, std::time::Instant::now()) {
        s.parts.push(Part::Note(line));
    }
    s
}

// ---- bise ambient (docs/ambient-app.md §4): the same options and replies ----

/// A hub card as the ambient core gets it (no TUI state on it).
fn ambient_card(kind: &str, agent: &str, text: &str) -> Card {
    Card {
        id: 0,
        kind: kind.into(),
        agent: agent.into(),
        text: text.into(),
        age_ms: 0,
        seen_at: std::time::Instant::now(),
        note: String::new(),
        look: None,
        place: None,
        pr: None,
        link: None,
        asking: false,
        waiting: Vec::new(),
    }
}

/// A card's numbered options as the inbox box shows them: `(digit,
/// whole label)` (a hard confirm's `no` is 3, like the box's).
pub(crate) fn ambient_options(kind: &str, agent: &str, text: &str) -> Vec<(usize, String)> {
    let s = base_shape(&ambient_card(kind, agent, text));
    s.options.iter().enumerate().map(|(i, o)| (s.num(i), o.clone())).collect()
}

/// What digit `d` sends as `/answer <id> <reply>`, as the box's `pick`
/// does: a hub item's digit itself, else the option's words. None: no
/// such option, or one the box acts on locally (a PR or release page to
/// open, the feature drop's second ask).
pub(crate) fn ambient_reply(kind: &str, agent: &str, text: &str, d: usize) -> Option<String> {
    let s = base_shape(&ambient_card(kind, agent, text));
    let i = s.by_digit(d)?;
    if !choice_kind(kind) {
        return s.options.get(i).cloned();
    }
    (!ambient_local(kind, d)).then(|| d.to_string())
}

/// Digit `d` of a `kind` card is one the box acts on itself (a PR or
/// release page to open, the feature drop's second ask).
pub(crate) fn ambient_local(kind: &str, d: usize) -> bool {
    matches!((kind, d), ("merge", 2) | ("update", 3) | ("feature_merge", 3))
}


#[cfg(test)]
#[path = "cards_tests.rs"]
mod tests;
