//! The lines of `/artifacts` (page artifacts-gallery v7, block b-term),
//! pure: from the screen's state, the library and the folders, what
//! each row says at a width. From 120 columns (114 of text): the head,
//! the search, then the tree (30 columns, `  │ `) beside the place's
//! crumb and list. Under it: the head, the crumb with `tab folders`, the
//! search, then the list, or the tree in its place. A box (`m`, `p`,
//! `n`, artifacts_pick.rs) takes the list's place. The key bar last.
//! The screen (artifacts_screen.rs) draws them and runs the keys.

use crate::artifacts::{self, Artifact, Folder, Group};
use bise_proto::words::library as words;
use crate::artifacts_pick::{self as pick};
use crate::artifacts_screen::{Fresh, Hit, Screen};
use crate::artifacts_tree::{self as tree, Place, TreeRow};
use crate::theme::{self, accent, dim, faint, text};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// From this many columns of text the tree is at the left (120 columns
/// of terminal, the frame's margins out).
pub(crate) const TREE_FROM: usize = 114;

/// The tree's column, then its `  │ ` edge.
pub(crate) const TREE_W: usize = 30;
const EDGE: &str = "  │ ";

/// The mark of a row new on this visit, after its title.
const NEW_TAG: &str = " new";

/// The time of the clock: now, and the UTC offset at a time (tests fix it).
pub(crate) struct Clock<'a> {
    pub(crate) now: u64,
    pub(crate) off: &'a dyn Fn(u64) -> i32,
}

impl Clock<'_> {
    fn group(&self, ms: u64) -> Group {
        artifacts::group(ms, (self.off)(ms), self.now, (self.off)(self.now))
    }
    /// The time column: today its time `14:02` (the group says the day),
    /// then `18:40` yesterday, `tue`, `sep 28`.
    fn time(&self, ms: u64) -> String {
        if self.group(ms) == Group::Today {
            let secs = ((ms / 1000) as i64 + (self.off)(ms) as i64).rem_euclid(86_400);
            return format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60);
        }
        artifacts::age(ms, (self.off)(ms), self.now, (self.off)(self.now), false)
    }
    fn ago(&self, ms: u64) -> String {
        artifacts::ago_at(ms, (self.off)(ms), self.now, (self.off)(self.now))
    }
}

pub(crate) fn pad(s: &str, w: usize) -> String {
    let s = cut(s, w);
    let n = w.saturating_sub(s.width());
    format!("{}{}", s, " ".repeat(n))
}

/// `s` in `w` columns at most, `…` at the cut.
pub(crate) fn cut(s: &str, w: usize) -> String {
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

fn width_of(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// What the list is now: the rows in view, the place's and the
/// project's, the search's.
pub(crate) struct View {
    /// the rows of the list, with the title's letters the search found
    pub(crate) list: Vec<(Artifact, Vec<usize>)>,
    /// every row of the library (archived or not, as the screen shows)
    pub(crate) every: Vec<Artifact>,
    /// the project shown, by hub id (`p` or the search's `in:`)
    pub(crate) project: Option<String>,
    /// the search's words, `in:` out
    pub(crate) words: String,
}

impl View {
    pub(crate) fn searching(&self) -> bool {
        !self.words.trim().is_empty()
    }
    /// The rows in the project filter (every folder).
    pub(crate) fn scoped(&self) -> Vec<&Artifact> {
        self.every.iter().filter(|a| self.project.as_deref().is_none_or(|p| a.project == p)).collect()
    }
}

/// The list of the screen's state: the archived ones or not, the
/// project, then the place (a search looks in every folder), then the
/// search.
pub(crate) fn view(sc: &Screen, all: &[Artifact], folders: &[Folder]) -> View {
    let every: Vec<Artifact> = all.iter().filter(|a| a.archived_at.is_some() == sc.archived).cloned().collect();
    let (inp, words) = tree::split_in(&sc.query);
    let refs: Vec<&Artifact> = every.iter().collect();
    let projects = tree::projects(&refs);
    let project = match inp {
        // a name no project has: nothing shows
        Some(w) => Some(tree::match_project(&w, &projects).map_or(format!("in:{w}"), str::to_string)),
        None => sc.project.clone(),
    };
    let searching = !words.trim().is_empty();
    let list = every
        .iter()
        .filter(|a| project.as_deref().is_none_or(|p| a.project == p))
        .filter(|a| searching || tree::holds(&sc.place, a, folders))
        .filter_map(|a| artifacts::find(a, &words).map(|h| (a.clone(), h)))
        .collect();
    View { list, every, project, words }
}

/// The columns of a list row: title, kind, who (the tree's width only),
/// time.
struct Cols {
    title: usize,
    kind: usize,
    who: usize,
    time: usize,
}

fn cols(list_w: usize, wide: bool, folder_words: bool) -> Cols {
    let (kind, who, time) = if wide { (12, if folder_words { 20 } else { 14 }, 6) } else { (11, 0, 6) };
    let title = list_w.saturating_sub(kind + who + time).clamp(12, 40);
    Cols { title, kind, who, time }
}

/// What a row's who column says: in a folder, its agent when it is not
/// the folder's own (`lead`); in all artifacts and a search, its folder
/// (its agent when unfiled); in unfiled, its agent.
fn who(a: &Artifact, place: &Place, searching: bool, lead: Option<&str>, folders: &[Folder]) -> String {
    match place {
        _ if searching => tree::folder_of(a, folders).map_or_else(|| tree::maker(a).to_string(), |f| f.name.clone()),
        Place::All => tree::folder_of(a, folders).map_or_else(|| tree::maker(a).to_string(), |f| f.name.clone()),
        Place::Unfiled => tree::maker(a).to_string(),
        Place::Folder(_) if Some(tree::maker(a)) == lead => String::new(),
        Place::Folder(_) => tree::maker(a).to_string(),
    }
}

/// One artifact's row: its title (` new` in the accent right after it
/// when new on this visit; the search's letters underlined; struck
/// through when its file is gone), its kind, who, its time. Selected:
/// bold, and tinted while the list has the keys.
#[allow(clippy::too_many_arguments)]
fn item_line(a: &Artifact, hits: &[usize], selected: bool, tint: bool, fresh: bool, who: &str, c: &Cols, clock: &Clock) -> Vec<Span<'static>> {
    // new: only the ` new` after it takes the accent (art-new's rule,
    // designer m_40753); the title keeps the text colour
    let mut st = Style::default().fg(text());
    if selected {
        st = st.add_modifier(Modifier::BOLD);
    }
    if a.gone {
        st = st.add_modifier(Modifier::CROSSED_OUT);
    }
    let room = c.title.saturating_sub(1 + if fresh { NEW_TAG.width() } else { 0 }).max(1);
    let shown = cut(&a.title, room);
    let n = shown.chars().count() - usize::from(shown.width() < a.title.width());
    let hit_st = st.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, ch) in shown.chars().enumerate() {
        let s = if i < n && hits.contains(&i) { hit_st } else { st };
        match spans.last_mut() {
            Some(last) if last.style == s => last.content.to_mut().push(ch),
            _ => spans.push(Span::styled(ch.to_string(), s)),
        }
    }
    if fresh {
        spans.push(Span::styled(NEW_TAG, Style::default().fg(accent()).add_modifier(Modifier::BOLD)));
    }
    let used = width_of(&spans);
    spans.push(Span::raw(" ".repeat(c.title.saturating_sub(used))));
    let d = Style::default().fg(dim());
    spans.push(Span::styled(pad(&a.kind_word(), c.kind), d));
    if c.who > 0 {
        spans.push(Span::styled(pad(who, c.who), d));
    }
    let t = cut(&clock.time(a.ts_ms), c.time);
    spans.push(Span::styled(format!("{:>w$}", t, w = c.time), Style::default().fg(faint())));
    spans.retain(|s| !s.content.is_empty());
    if selected && tint {
        spans = spans.into_iter().map(|s| Span::styled(s.content, s.style.bg(theme::selection_bg()))).collect();
    }
    spans
}

/// A body row of the list.
enum Body {
    /// the blank before a day's group (not the first)
    Gap,
    Group(Group),
    Item(usize),
    /// the versions box: its top edge, a version (index newest first),
    /// its key row (wide only), its bottom edge
    BoxTop,
    Version(usize),
    BoxKeys,
    BoxBottom,
}

/// The versions box's rows under the selected row.
fn box_line(a: &Artifact, row: &Body, vsel: usize, fresh: &Fresh, clock: &Clock, wide: bool) -> Vec<Span<'static>> {
    let w: usize = if wide { 58 } else { 44 };
    let edge = Style::default().fg(faint());
    let lead = Span::raw("  ");
    let versions: Vec<&artifacts::Version> = a.versions.iter().rev().collect();
    match row {
        Body::BoxTop => {
            let head = format!("{} · {} versions", a.title, a.versions.len().max(a.v as usize));
            let head = cut(&head, w.saturating_sub(4));
            let fill = w.saturating_sub(head.width() + 3);
            vec![
                lead,
                Span::styled("╭─ ", edge),
                Span::styled(head, Style::default().fg(text())),
                Span::styled(format!(" {}╮", "─".repeat(fill.saturating_sub(1))), edge),
            ]
        }
        Body::BoxBottom => vec![lead, Span::styled(format!("╰{}╯", "─".repeat(w)), edge)],
        Body::BoxKeys => {
            let k = Style::default().fg(text());
            let words = vec![
                Span::styled("⏎", k),
                Span::styled(" open this one   ", Style::default().fg(dim())),
                Span::styled("esc", k),
                Span::styled(" back", Style::default().fg(dim())),
            ];
            let used = width_of(&words);
            let mut spans = vec![lead, Span::styled("│ ", edge)];
            spans.extend(words);
            spans.push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
            spans.push(Span::styled("│", edge));
            spans
        }
        Body::Version(i) => {
            let Some(v) = versions.get(*i) else { return Vec::new() };
            let sel = *i == vsel;
            let mark = if sel { Span::styled(format!("{} ", theme::glyph(theme::G_YOU)), Style::default().fg(accent())) } else { Span::raw("  ") };
            let st = if sel { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) };
            // a new version: `new` right after its name, as the list has
            // it (designer m_7579); the column only when one is new
            let any_new = a.versions.iter().any(|x| fresh.version(a, x));
            let mut body = vec![mark, Span::styled(pad(&format!("v{}", v.v), if any_new { 4 } else { 5 }), st)];
            if any_new {
                let tag = if fresh.version(a, v) { NEW_TAG.trim_start() } else { "" };
                body.push(Span::styled(pad(tag, 6), Style::default().fg(accent()).add_modifier(Modifier::BOLD)));
            }
            body.push(Span::styled(pad(&clock.ago(v.ts_ms), 13), Style::default().fg(dim())));
            body.push(Span::styled(pad(&v.note, 15), Style::default().fg(dim())));
            if wide && v.v == a.v {
                body.push(Span::styled("the current one", Style::default().fg(faint())));
            }
            let used = width_of(&body);
            let mut spans = vec![lead, Span::styled("│ ", edge)];
            spans.extend(body);
            spans.push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
            spans.push(Span::styled("│", edge));
            spans
        }
        _ => Vec::new(),
    }
}

/// One key and its words, for the key bar.
fn keys<K: AsRef<str>>(pairs: &[(K, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (k, w)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(k.as_ref().to_string(), Style::default().fg(text())));
        spans.push(Span::styled(format!(" {}", w), Style::default().fg(dim())));
    }
    Line::from(spans)
}

/// The key bar for the state the screen is in.
pub(crate) fn key_bar(sc: &Screen, sel: Option<&Artifact>, wide: bool, box_keys: Option<Vec<(String, &'static str)>>) -> Line<'static> {
    if let Some(pairs) = box_keys {
        return keys(&pairs);
    }
    if sc.typing {
        return keys(&[("⏎", "done"), ("↑↓", "choose"), ("esc", "clear the search")]);
    }
    if sc.versions.is_some() {
        return keys(&[("⏎", "open this version"), ("↑↓", "choose"), ("esc", "back to the list")]);
    }
    if sc.look.is_some() {
        return keys(&[("space", "close the preview"), ("↑↓", "the next one, previewed"), ("⏎", "open"), ("esc", "close")]);
    }
    let esc = match (sc.query.is_empty(), sc.archived) {
        (false, _) => "clear the search",
        (true, true) => "back to the list",
        (true, false) => words::key_back,
    };
    if sc.in_tree {
        let tab = if wide { ("tab", "the list") } else { ("⏎", "show it") };
        return keys(&[("↑↓", "folders"), ("→←", "open, shut"), tab, ("n", "new folder"), ("esc", esc)]);
    }
    let Some(_) = sel else {
        return if wide {
            keys(&[("n", "new folder"), ("tab", "folders"), ("p", "project"), ("/", "search"), ("esc", esc)])
        } else {
            keys(&[("tab", "folders"), ("p", "project"), ("esc", esc)])
        };
    };
    if !wide {
        return keys(&[("⏎", "open"), ("m", "move"), ("tab", "folders"), ("p", "project"), ("esc", esc)]);
    }
    keys(&[
        ("⏎", "open"),
        ("space", "look"),
        ("m", "move to…"),
        ("n", "new folder"),
        ("tab", "folders"),
        ("p", "project"),
        ("/", "search"),
        ("esc", esc),
    ])
}

/// The head: `artifacts  893 from 6 projects · 9 folders · 12 new · 23
/// unfiled`; filtered `artifacts  ai-docs · 74 of 893 · …`; under 120
/// `artifacts · 893 · 12 new`.
fn head(sc: &Screen, v: &View, folders: &[Folder], wide: bool) -> Line<'static> {
    let b = Style::default().fg(text()).add_modifier(Modifier::BOLD);
    let d = Style::default().fg(dim());
    let every: Vec<&Artifact> = v.every.iter().collect();
    let scoped = v.scoped();
    let n_projects = tree::projects(&every).len();
    let count = match &v.project {
        Some(p) => format!("{} · {}", tree::name_of(p, &v.every), words::of_total(scoped.len(), every.len())),
        None if wide && n_projects > 1 => format!("{} from {}", every.len(), words::projects_n(n_projects)),
        None => every.len().to_string(),
    };
    let title = if sc.archived { "artifacts · archived" } else { "artifacts" };
    let new = scoped.iter().filter(|a| sc.fresh.row(a)).count();
    let unfiled = scoped.iter().filter(|a| tree::folder_of(a, folders).is_none()).count();
    let mut spans = vec![Span::styled(title.to_string(), b)];
    spans.push(Span::styled(if wide { format!("  {}", count) } else { format!(" · {}", count) }, d));
    if wide && !folders.is_empty() {
        let f = folders.len();
        spans.push(Span::styled(format!(" · {} folder{}", f, if f == 1 { "" } else { "s" }), d));
    }
    if new > 0 {
        spans.push(Span::styled(" · ", d));
        spans.push(Span::styled(format!("{} new", new), Style::default().fg(accent())));
    }
    if wide && unfiled > 0 && !sc.archived {
        spans.push(Span::styled(format!(" · {}", words::unfiled_n(unfiled)), d));
    }
    Line::from(spans)
}

/// The search's line, or blank.
fn search_line(sc: &Screen, v: &View) -> Line<'static> {
    if sc.query.is_empty() && !sc.typing {
        return Line::from("");
    }
    let mut out = vec![
        Span::styled("/ ", Style::default().fg(if sc.typing { accent() } else { dim() })),
        Span::styled(sc.query.clone(), Style::default().fg(text())),
    ];
    if sc.typing {
        out.push(Span::styled("▏", Style::default().fg(accent())));
    }
    if !sc.query.is_empty() {
        out.push(Span::styled(format!("   {} in every folder", v.list.len()), Style::default().fg(dim())));
    }
    Line::from(out)
}

/// The crumb: `desktop app / agent view  48 · amb-home and 2 more` (the
/// agents only from 120 columns); filtered `14 of 48`.
fn crumb(sc: &Screen, v: &View, folders: &[Folder], wide: bool) -> Vec<Span<'static>> {
    let f = Style::default().fg(faint());
    if v.searching() {
        return vec![Span::styled("every folder".to_string(), Style::default().fg(dim())), Span::styled(format!("  {}", v.list.len()), f)];
    }
    let parts = tree::crumb(&sc.place, folders);
    let mut spans = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if i + 1 < parts.len() {
            spans.push(Span::styled(format!("{} / ", p), Style::default().fg(dim())));
        } else {
            spans.push(Span::styled(p.clone(), Style::default().fg(text()).add_modifier(Modifier::BOLD)));
        }
    }
    let total = v.every.iter().filter(|a| tree::holds(&sc.place, a, folders)).count();
    let n = v.list.len();
    let count = if v.project.is_some() { format!("  {}", words::of_total(n, total)) } else { format!("  {}", n) };
    spans.push(Span::styled(count, f));
    if wide && matches!(sc.place, Place::Folder(_)) {
        let rows: Vec<&Artifact> = v.list.iter().map(|(a, _)| a).collect();
        let who = tree::agents(&rows);
        let words = match who.len() {
            0 => String::new(),
            1 => who[0].clone(),
            k => format!("{} and {} more", who[0], k - 1),
        };
        if !words.is_empty() {
            spans.push(Span::styled(format!(" · {}", words), f));
        }
    }
    spans
}

/// The tree's rows as lines `TREE_W` wide: the current place bold
/// (tinted while the tree has the keys), its count; another place, `•`
/// in the accent when something new is inside; a blank after unfiled.
fn tree_lines(sc: &Screen, rows: &[TreeRow]) -> Vec<(Vec<Span<'static>>, Option<Place>)> {
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let current = r.place == sc.place;
        let lead = match (r.depth, r.open) {
            (1, _) => "    ",
            (_, Some(true)) => "▾ ",
            (_, Some(false)) => "▸ ",
            _ => "  ",
        };
        let theme = r.depth == 0 && matches!(r.place, Place::Folder(_));
        let mut st = Style::default().fg(if theme || current { text() } else { dim() });
        if current {
            st = st.add_modifier(Modifier::BOLD);
        }
        let name_w = TREE_W - 4;
        let mut spans = vec![Span::styled(pad(&format!("{}{}", lead, r.name), name_w), st)];
        let always = matches!(r.place, Place::All | Place::Unfiled);
        if current || always {
            let n = if r.count == 0 && !always { String::new() } else { r.count.to_string() };
            spans.push(Span::styled(format!("{:>4}", n), Style::default().fg(faint())));
        } else if r.fresh {
            spans.push(Span::styled("   •", Style::default().fg(accent())));
        } else {
            spans.push(Span::raw("    "));
        }
        if current && sc.in_tree {
            spans = spans.into_iter().map(|s| Span::styled(s.content, s.style.bg(theme::selection_bg()))).collect();
        }
        out.push((spans, Some(r.place.clone())));
        if i == 1 {
            out.push((vec![Span::raw(" ".repeat(TREE_W))], None));
        }
    }
    out
}

/// The list's lines (`list_w` wide, `room` rows): its day groups, its
/// rows, the versions box under the selected one, `↓ N more`, the quiet
/// `N archived` row; or what an empty place says.
#[allow(clippy::too_many_arguments)]
fn list_lines(sc: &mut Screen, v: &View, folders: &[Folder], put_away: usize, wide: bool, list_w: usize, room: usize, clock: &Clock) -> Vec<(Vec<Span<'static>>, Option<Hit>)> {
    let mut out: Vec<(Vec<Span<'static>>, Option<Hit>)> = Vec::new();
    let list = &v.list;
    let txt = |s: String| (vec![Span::styled(s, Style::default().fg(text()))], None);
    if list.is_empty() {
        let words = if v.every.is_empty() && !crate::artifacts::came() {
            // the hub builds its list off its loop after a start
            "loading the list…".to_string()
        } else if sc.archived {
            words::none_archived.to_string()
        } else if v.every.is_empty() {
            "nothing yet. when an agent makes a page, a doc or a file for you, it lands here.".to_string()
        } else if v.searching() {
            "nothing matches. esc clears the search.".to_string()
        } else if let (Some(p), true) = (&v.project, v.every.iter().any(|a| tree::holds(&sc.place, a, folders))) {
            format!("nothing from {} here. p shows every project.", tree::name_of(p, &v.every))
        } else if sc.place == Place::Unfiled {
            "nothing unfiled: every artifact has its folder.".to_string()
        } else {
            "nothing here yet. m on an artifact moves it here.".to_string()
        };
        out.push(txt(words));
        return out;
    }
    let sel_i = sc.sel.as_ref().and_then(|k| list.iter().position(|(a, _)| &a.key() == k));
    let folder_words = v.searching() || sc.place == Place::All;
    let c = cols(list_w, wide, folder_words);
    let rows: Vec<&Artifact> = list.iter().map(|(a, _)| a).collect();
    let lead = match sc.place {
        Place::Folder(_) => tree::agents(&rows).into_iter().next(),
        _ => None,
    };
    let mut body: Vec<Body> = Vec::new();
    let mut last: Option<Group> = None;
    let (mut sel_row, mut sel_end) = (0usize, 0usize);
    for (i, (a, _)) in list.iter().enumerate() {
        let g = clock.group(a.ts_ms);
        if last != Some(g) {
            // a blank between two days, as the mock has it
            if last.is_some() {
                body.push(Body::Gap);
            }
            body.push(Body::Group(g));
            last = Some(g);
        }
        if Some(i) == sel_i {
            sel_row = body.len();
        }
        body.push(Body::Item(i));
        if Some(i) == sel_i && sc.versions.is_some() {
            body.push(Body::BoxTop);
            body.extend((0..a.versions.len()).map(Body::Version));
            if wide {
                body.push(Body::BoxKeys);
            }
            body.push(Body::BoxBottom);
        }
        if Some(i) == sel_i {
            sel_end = body.len() - 1;
        }
    }
    // keep the selection (and its open box) in view; the group above the
    // first row with it
    let more = body.len() > room;
    let fit = if more { room.saturating_sub(1).max(1) } else { room };
    if more {
        if sel_row < sc.top {
            sc.top = sel_row.saturating_sub(usize::from(sel_row > 0 && matches!(body[sel_row - 1], Body::Group(_))));
        }
        if sel_end >= sc.top + fit {
            sc.top = (sel_end + 1).saturating_sub(fit).min(sel_row);
        }
        sc.top = sc.top.min(body.len().saturating_sub(fit));
    } else {
        sc.top = 0;
    }
    let sel = sel_i.map(|i| &list[i].0);
    let tint = !sc.in_tree && sc.pick.is_none();
    for row in body.iter().skip(sc.top).take(fit) {
        let line = match row {
            Body::Gap => (Vec::new(), None),
            Body::Group(g) => (vec![Span::styled(g.words().to_string(), Style::default().fg(faint()))], None),
            Body::Item(i) => {
                let (a, h) = &list[*i];
                let w = who(a, &sc.place, v.searching(), lead.as_deref(), folders);
                (item_line(a, h, Some(*i) == sel_i, tint, sc.fresh.row(a), &w, &c, clock), Some(Hit::Row(a.key(), None)))
            }
            Body::Version(k) => {
                let s = sel.expect("a box is under the selection");
                (box_line(s, row, sc.versions.unwrap_or(0), &sc.fresh, clock, wide), Some(Hit::Version(*k)))
            }
            _ => (box_line(sel.expect("a box is under the selection"), row, sc.versions.unwrap_or(0), &sc.fresh, clock, wide), None),
        };
        out.push(line);
    }
    if more {
        let below = list.len().saturating_sub(body.iter().take(sc.top + fit).filter(|b| matches!(b, Body::Item(_))).count());
        if below > 0 {
            out.push((vec![Span::styled(format!("↓ {} more", below), Style::default().fg(dim()))], None));
        }
    }
    // the quiet row that opens the archived ones, under the list
    if !sc.archived && put_away > 0 && out.len() + 2 <= room {
        out.push((Vec::new(), None));
        out.push((vec![Span::styled(format!("{} archived", put_away), Style::default().fg(faint()))], Some(Hit::Archived)));
    }
    out
}

/// The screen's lines in `width` × `height` (inside the frame's padding)
/// and the body rows' targets (by line index; a tree row's from column
/// 0 to the tree's edge), and the number of rows in the list.
pub(crate) fn lines(sc: &mut Screen, all: &[Artifact], folders: &[Folder], width: usize, height: usize, clock: &Clock) -> (Vec<Line<'static>>, Vec<(usize, Hit)>, usize) {
    let wide = width >= TREE_FROM;
    if !tree::exists(&sc.place, folders) {
        sc.place = Place::All;
    }
    let v = view(sc, all, folders);
    if sc.sel.as_ref().is_none_or(|k| !v.list.iter().any(|(a, _)| &a.key() == k)) {
        sc.sel = v.list.first().map(|(a, _)| a.key());
        sc.versions = None;
    }
    let sel = sc.sel.as_ref().and_then(|k| v.list.iter().find(|(a, _)| &a.key() == k)).map(|(a, _)| a.clone());
    let put_away = all.iter().filter(|a| a.archived_at.is_some()).count();
    let mut out: Vec<Line<'static>> = vec![head(sc, &v, folders, wide)];
    let mut hits: Vec<(usize, Hit)> = Vec::new();
    let crumb_spans = crumb(sc, &v, folders, wide);
    if wide {
        out.push(search_line(sc, &v));
    } else {
        let mut l = crumb_spans.clone();
        if !sc.in_tree && sc.pick.is_none() {
            let gap = 39usize.saturating_sub(width_of(&l)).max(3);
            l.push(Span::styled(format!("{}tab folders", " ".repeat(gap)), Style::default().fg(faint())));
        }
        out.push(Line::from(l));
        out.push(search_line(sc, &v));
    }
    // the body, then a blank and the key bar
    let foot = 2;
    let body_h = height.saturating_sub(out.len() + foot).max(1);
    let scoped = v.scoped();
    let every: Vec<&Artifact> = v.every.iter().collect();
    let fresh = |a: &Artifact| sc.fresh.row(a);
    let trows = tree::tree(folders, &scoped, &every, &sc.open_themes, &fresh);
    let tlines = tree_lines(sc, &trows);
    // the tree scrolls to keep the place in view
    let cur = tlines.iter().position(|(_, p)| p.as_ref() == Some(&sc.place)).unwrap_or(0);
    let ttop = (cur + 1).saturating_sub(body_h);
    // the right side: a box, or the crumb and the list
    let list_w = if wide { width.saturating_sub(TREE_W + EDGE.width()) } else { width };
    let mut right: Vec<(Vec<Span<'static>>, Option<Hit>)> = Vec::new();
    let mut box_keys = None;
    if let Some(p) = sc.pick.clone() {
        let lib: Vec<&Artifact> = all.iter().collect();
        let choices = pick::rows(&p, folders, &lib, &sc.moved);
        let theme_name = match &p {
            pick::Pick::NewFolder { parent: Some(id), .. } => folders.iter().find(|f| &f.id == id).map(|f| f.name.clone()),
            _ => None,
        };
        right.extend(pick::lines(&p, &choices, folders, theme_name.as_deref(), list_w).into_iter().map(|l| (l.spans, None)));
        box_keys = Some(pick::key_pairs(&p, choices.len(), pick::can_make(&p, folders)));
    } else if wide || !sc.in_tree {
        if wide {
            right.push((crumb_spans, None));
            right.push((Vec::new(), None));
        }
        let room = body_h.saturating_sub(right.len());
        right.extend(list_lines(sc, &v, folders, put_away, wide, list_w, room, clock));
    }
    for k in 0..body_h {
        let y = out.len();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let tree_here = wide || (sc.in_tree && sc.pick.is_none());
        if tree_here {
            match tlines.get(ttop + k) {
                Some((s, p)) => {
                    spans.extend(s.iter().cloned());
                    if let Some(p) = p {
                        hits.push((y, Hit::Tree(p.clone())));
                    }
                }
                None => spans.push(Span::raw(" ".repeat(TREE_W))),
            }
            if wide {
                spans.push(Span::styled(EDGE, Style::default().fg(faint())));
            }
        }
        if let Some((s, h)) = right.get(k) {
            spans.extend(s.iter().cloned());
            if let Some(h) = h {
                hits.push((y, h.clone()));
            }
        }
        // no trailing blanks past the text
        while spans.last().is_some_and(|s| s.content.trim().is_empty() && s.style.bg.is_none()) {
            spans.pop();
        }
        out.push(Line::from(spans));
    }
    out.push(Line::from(""));
    out.push(key_bar(sc, sel.as_ref(), wide, box_keys));
    (out, hits, v.list.len())
}
