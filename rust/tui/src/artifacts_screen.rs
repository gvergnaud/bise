//! `/artifacts` (site/m/artifacts; brief artifacts-folders part 6, page
//! artifacts-gallery v7 block b-term): the library, full screen. The
//! same library for every project (this hub's rows and the others',
//! `artifacts/library`), in the same folders (artifacts_tree.rs). From
//! 120 columns the folder tree is at the left (`•` in the accent:
//! something new inside); under it, a crumb, and tab opens the tree in
//! the list's place. The list of a place: newest first, grouped by day,
//! a row says only what the place does not (its agent when it is not
//! the folder's own, its folder in all artifacts); the kind stays a dim
//! word (no kind glyphs: the designer's font audit). The rows new when
//! it opened (the header's `↗ N new`) say `new` in the accent for the
//! whole visit, and so do their new versions in the versions box
//! ([`Fresh`]); the hub has them seen at once.
//!
//! Keys: the list holds them when it opens: ↑↓, ⏎ open, space Quick
//! Look, v versions, r show in Finder, c copy, @ put it in a message,
//! m move to… (artifacts_pick.rs), n new folder, p project, tab the
//! folders, a archived, esc back. The tree's: ↑↓ the place (the list
//! follows), →← open or shut a theme, ⏎ or tab back to the list. `/`
//! focuses the search: it looks in every folder (`in:<project>` filters
//! too); ⏎ or ↑↓ give the keys back to the list (the search keeps its
//! text). esc clears the search, a second esc closes.

use crate::app::App;
use crate::artifacts::{self, Artifact, Folder};
use crate::artifacts_lines::{lines, Clock};
use crate::artifacts_pick::{self as pick, Choice, Outcome, Pick};
use crate::artifacts_tree::{self as tree, Place};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use std::time::Instant;

pub(crate) use crate::artifacts_lines::cut;

/// The url of the header's `↗ 3 new` (textlayer.rs opens the screen).
pub(crate) const OPEN_URL: &str = "bise-artifacts:open";

/// The open screen.
#[derive(Default)]
pub(crate) struct Screen {
    /// the search's text
    pub(crate) query: String,
    /// the search has the keys (after `/`)
    pub(crate) typing: bool,
    /// where the list is (the tree's place)
    pub(crate) place: Place,
    /// the themes open in the tree, by id
    pub(crate) open_themes: Vec<String>,
    /// the tree has the keys (tab); under 120 columns it shows in the
    /// list's place
    pub(crate) in_tree: bool,
    /// one project only (`p`), by hub id; none: every project
    pub(crate) project: Option<String>,
    /// the box that is up (`m`, `p`, `n`)
    pub(crate) pick: Option<Pick>,
    /// the folders you moved to on this visit (`m` offers them first)
    pub(crate) moved: Vec<String>,
    /// the archived ones are shown instead (the `N archived` row, `a`)
    pub(crate) archived: bool,
    /// the selected artifact, by key ([`Artifact::key`]; stays on it
    /// when the list changes)
    pub(crate) sel: Option<String>,
    /// the first body row shown
    pub(crate) top: usize,
    /// the versions under the selected row, open: the version selected
    /// (an index, newest first)
    pub(crate) versions: Option<usize>,
    /// Quick Look is up (macOS `qlmanage -p`)
    pub(crate) look: Option<Look>,
    /// what the last frame drew where (clicks): body row → target
    pub(crate) hits: Vec<(u16, Hit)>,
    /// the screen row of the search (a click focuses it)
    pub(crate) search_y: Option<u16>,
    /// the screen column where the rows start
    pub(crate) x0: u16,
    /// the tree is at the left (the last frame was 120 columns or more)
    pub(crate) wide: bool,
    /// what was new when it opened (and what came while it is open):
    /// marked `new` for this visit, though the hub has them seen
    pub(crate) fresh: Fresh,
}

/// The rows marked `new` on this visit: their ids, and when you had
/// last looked (a version after it is new in the versions box; none
/// from an older hub: only the current version).
#[derive(Default, Clone, Debug, PartialEq)]
pub(crate) struct Fresh {
    pub(crate) ids: Vec<String>,
    pub(crate) since: Option<u64>,
}

impl Fresh {
    /// The new rows now, kept on top of the ones already marked.
    pub(crate) fn take(&mut self, rows: &[Artifact], since: Option<u64>) {
        for a in rows {
            if !self.ids.contains(&a.id) {
                self.ids.push(a.id.clone());
            }
        }
        // the first look of the visit sets it; later ones are this visit's own
        if self.since.is_none() {
            self.since = since;
        }
    }
    pub(crate) fn row(&self, a: &Artifact) -> bool {
        self.ids.contains(&a.id)
    }
    pub(crate) fn version(&self, a: &Artifact, v: &artifacts::Version) -> bool {
        self.row(a) && self.since.map_or(v.v == a.v, |s| v.ts_ms > s)
    }
}

/// What a row of the last frame was.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hit {
    /// an artifact's row, by key; the columns of its `v3` (a click
    /// opens the versions)
    Row(String, Option<(u16, u16)>),
    /// a row of the tree (from column 0 to its edge)
    Tree(Place),
    /// a version's row in the versions box
    Version(usize),
    /// the quiet `N archived` row (a click opens them)
    Archived,
}

/// Quick Look is up (open_file.rs holds its process): closed when this
/// goes.
pub(crate) struct Look;

impl Drop for Look {
    fn drop(&mut self) {
        crate::open_file::close_preview();
    }
}

/// `/artifacts` opens the screen (`add <path or link>` adds one).
pub(crate) fn open(app: &mut App) {
    // the header's `↗ designer · pricing page` opens on that one (the
    // newest new one), else on the newest
    let new = artifacts::new_rows();
    let sel = new.first().or(artifacts::all().first()).map(|a| a.key());
    // the new ones stay marked while it is open
    let mut fresh = Fresh::default();
    fresh.take(&new, artifacts::seen_ms());
    // the theme of the new one is open in the tree
    let folders = artifacts::folders();
    let open_themes = new.first().and_then(|a| tree::theme_of(&Place::Folder(a.folder.clone().unwrap_or_default()), &folders)).into_iter().collect();
    app.artifacts = Some(Screen { sel, fresh, open_themes, ..Default::default() });
    ask_library(app);
    // you looked: the header's `↗ N new` goes
    artifacts::mark_seen();
    app.sb.call("artifacts/seen", artifacts::seen_params(), crate::sb::rpc::Then::Shown);
}

/// The hub's list came while the screen is open: what came new is
/// marked too, and seen at once (you are looking at it).
pub(crate) fn on_list(app: &mut App) {
    if app.artifacts.is_none() {
        return;
    }
    // folders.json may have moved (his edit in the app): the library again
    ask_library(app);
    let Some(sc) = app.artifacts.as_mut() else { return };
    if artifacts::new_count() == 0 {
        return;
    }
    sc.fresh.take(&artifacts::new_rows(), None);
    artifacts::mark_seen();
    app.sb.call("artifacts/seen", artifacts::seen_params(), crate::sb::rpc::Then::Shown);
}

// ---- drawing ----

/// The frame of a full screen of bise: `╭─ bise :* · ~/acme ── {name} ───╮`
/// (the path as the main screen's top edge says it, topedge.rs: its last
/// name when short, then nothing; the name always).
pub(crate) fn draw_frame(app: &App, frame: &mut Frame, full: Rect, name: &str) -> Rect {
    draw_frame_at(app, frame, full, name, false)
}

/// [`draw_frame`]; `folder`: the edge names the workspace's folder only
/// (a narrow screen: /running at 80 columns).
pub(crate) fn draw_frame_at(app: &App, frame: &mut Frame, full: Rect, name: &str, folder: bool) -> Rect {
    let cols = crate::layout::cols(full.width, full.height);
    let cols = crate::layout::Cols { panel: None, ..cols };
    let title = app.sb.title();
    let mut paths = crate::topedge::paths(&crate::sb::workspace(app).unwrap_or_default(), "");
    if folder {
        paths = paths.split_off(paths.len().saturating_sub(1));
    }
    let name = name.to_string();
    let edge = move |room: usize| (crate::topedge::screen_head(room, &paths, &name), Vec::new());
    if cols.framed {
        crate::chrome::draw_frame(frame.buffer_mut(), full, cols, title, edge, full.bottom());
    }
    let m = cols.margin;
    let top = u16::from(cols.framed);
    Rect {
        x: full.x + m,
        y: full.y + top,
        width: full.width.saturating_sub(2 * m),
        height: full.height.saturating_sub(2 * top),
    }
}

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    if app.artifacts.is_none() {
        return;
    }
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 10 {
        return;
    }
    let area = draw_frame(app, frame, full, "artifacts");
    let all = artifacts::library();
    let folders = artifacts::folders();
    let now = crate::when::now_ms();
    let off = |ms: u64| crate::when::offset_at(ms);
    let clock = Clock { now, off: &off };
    let Some(sc) = app.artifacts.as_mut() else { return };
    let (rows, hits, _) = lines(sc, &all, &folders, area.width as usize, area.height as usize, &clock);
    sc.hits = hits.into_iter().map(|(i, h)| (area.y + i as u16, h)).collect();
    sc.wide = area.width as usize >= crate::artifacts_lines::TREE_FROM;
    sc.search_y = Some(area.y + if sc.wide { 1 } else { 2 });
    sc.x0 = area.x;
    frame.render_widget(Paragraph::new(rows), area);
    crate::textlayer::text(area);
    // a click on a row's `v3` and the rows: the hand
    for (y, _) in &sc.hits {
        crate::pointer::region(Rect { x: area.x, y: *y, width: area.width, height: 1 }, crate::pointer::Shape::Pointer);
    }
}

// ---- the library from the hub ----

/// `artifacts/library`: the other projects' rows (the screen opens, the
/// `artifacts` event came: folders.json may have moved). Its answer
/// comes through sb/rpc.rs's `Then::Library` into [`on_library`].
fn ask_library(app: &mut App) {
    app.sb.call("artifacts/library", serde_json::json!({}), crate::sb::rpc::Then::Library);
}

/// The library came: the other projects' rows and the folders.
pub(crate) fn on_library(app: &mut App, here: String, others: Vec<Artifact>, folders: Vec<Folder>) {
    let _ = app;
    artifacts::set_here_name(&here);
    artifacts::set_others(others);
    artifacts::set_folders(folders);
}

// ---- keys and the mouse ----

/// The list as it is drawn, and the selected row's index in it.
fn selected(sc: &Screen) -> (Vec<(Artifact, Vec<usize>)>, Option<usize>) {
    let list = crate::artifacts_lines::view(sc, &artifacts::library(), &artifacts::folders()).list;
    let i = sc.sel.as_ref().and_then(|k| list.iter().position(|(a, _)| &a.key() == k));
    (list, i)
}

fn sel_row(sc: &Screen) -> Option<Artifact> {
    sc.sel.as_ref().and_then(|k| artifacts::get_key(k))
}

fn step(sc: &mut Screen, by: isize) {
    if let Some(v) = sc.versions {
        let n = sel_row(sc).map_or(0, |a| a.versions.len());
        sc.versions = Some((v as isize + by).clamp(0, n.saturating_sub(1) as isize) as usize);
        return;
    }
    let (list, i) = selected(sc);
    if list.is_empty() {
        return;
    }
    let j = match i {
        Some(i) => (i as isize + by).clamp(0, list.len() as isize - 1) as usize,
        None => 0,
    };
    sc.sel = Some(list[j].0.key());
}

fn flash(app: &mut App, note: String) {
    app.flash = Some((note, Instant::now()));
}

/// Opens the selected artifact (its version when the box is open).
fn open_selected(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sel_row(sc) else { return };
    let v = sc.versions.and_then(|k| a.versions.iter().rev().nth(k)).map(|x| x.v);
    if v.is_none() {
        if let artifacts::How::Diff(url) = artifacts::how(&a, None, true) {
            app.artifacts = None;
            crate::diffview::request(app, crate::diffview::Ask::Pr(url), crate::diffview::By::Key);
            return;
        }
    }
    let note = artifacts::open(app, &a, v);
    flash(app, note);
}

/// Quick Look (macOS): `qlmanage -p` on the file; a link, or Linux,
/// opens like ⏎.
fn quick_look(app: &mut App) {
    let Some(sc) = app.artifacts.as_mut() else { return };
    let Some(a) = sel_row(sc) else { return };
    let target = a.open_target(None);
    if !cfg!(target_os = "macos") || artifacts::is_url(&target) || target.is_empty() {
        sc.look = None;
        return open_selected(app);
    }
    // the chips' Quick Look (open_file.rs): one up at a time
    crate::open_file::preview(&target);
    sc.look = Some(Look);
}

/// `r`: the file in Finder (its folder elsewhere); `o`: a PR on GitHub.
fn reveal(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sel_row(sc) else { return };
    if let Some(pr) = &a.pr {
        let url = if a.is_link() { a.target.clone() } else { format!("https://github.com/{}/pull/{}", pr.repo, pr.number) };
        let ok = crate::links::open(&url);
        return flash(app, if ok { format!("opening {}", artifacts::short_target(&url)) } else { format!("could not open {}", url) });
    }
    if a.is_link() {
        return;
    }
    let path = a.open_target(None);
    // the chips' ⌥ click does the same (open_file.rs)
    if !crate::open_file::reveal(&path) {
        flash(app, format!("could not show {}", path));
    }
}

/// `c`: the path or the link.
fn copy(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sel_row(sc) else { return };
    if crate::clipboard::copy(&a.open_target(None)) {
        flash(app, "copied".to_string());
    }
}

/// `@`: the screen closes, the artifact's chip goes in your message.
fn put_in_message(app: &mut App) {
    let Some(sc) = app.artifacts.as_ref() else { return };
    let Some(a) = sel_row(sc) else { return };
    app.artifacts = None;
    crate::attach::insert_artifact(app, &a.id, &a.title);
}

/// The box a key opens: `m` on the selected row, `p`, `n` in the
/// place's theme.
fn open_pick(sc: &mut Screen, which: char) {
    let folders = artifacts::folders();
    sc.versions = None;
    sc.pick = match which {
        'm' => sel_row(sc).map(|a| Pick::Move { key: a.key(), title: a.title.clone(), from: tree::folder_of(&a, &folders).map(|f| f.id.clone()), query: String::new(), sel: 0 }),
        'p' => Some(Pick::Project { query: String::new(), sel: 0 }),
        _ => Some(Pick::NewFolder { parent: tree::theme_of(&sc.place, &folders), name: String::new() }),
    };
}

/// What a box chose: the hub moves the row (its own project's hub
/// writes it), makes the folder (then moves into it), or the list shows
/// one project.
fn run_choice(app: &mut App, c: Choice) {
    let Some(sc) = app.artifacts.as_mut() else { return };
    let moving = match &sc.pick {
        Some(Pick::Move { key, .. }) => artifacts::get_key(key),
        _ => None,
    };
    sc.pick = None;
    match c {
        Choice::Project(p) => {
            sc.project = p;
            sc.top = 0;
        }
        Choice::Folder(id) => {
            if let Some(a) = moving {
                sc.moved.retain(|m| m != &id);
                sc.moved.push(id.clone());
                move_to(app, &a, Some(id));
            }
        }
        Choice::Unfiled => {
            if let Some(a) = moving {
                move_to(app, &a, None);
            }
        }
        Choice::Make { name, parent, then_move } => {
            let then = moving.filter(|_| then_move).map(|a| (a.project.clone(), a.id.clone()));
            make_folder(app, name, parent, then);
        }
    }
}

/// `artifacts/move` to this hub with the row's project: the hub forwards
/// it to that project's hub (projects::target), the only writer of its
/// meta.json. The new list comes in the `artifacts` event.
fn move_to(app: &mut App, a: &Artifact, folder: Option<String>) {
    app.sb.call("artifacts/move", move_params(&a.project, &a.id, folder.as_deref()), crate::sb::rpc::Then::Shown);
}

/// `artifacts/move`'s params: always the row's hub id, never a name
/// (architect m_40803).
pub(crate) fn move_params(project: &str, id: &str, folder: Option<&str>) -> serde_json::Value {
    serde_json::json!({"project": project, "ids": [id], "folder": folder})
}

/// `artifacts/folders` make (his edit: the hub writes folders.json);
/// `then`: the row to move into it once it is made (its answer's
/// `made`). A parent that is a name, not an id: the theme is made first
/// by the hub (`theme/task`).
fn make_folder(app: &mut App, name: String, parent: Option<String>, then: Option<(String, String)>) {
    let folders = artifacts::folders();
    let parent = parent.filter(|p| folders.iter().any(|f| &f.id == p));
    let params = serde_json::json!({"op": {"make": {"name": name, "parent": parent}}});
    app.sb.call("artifacts/folders", params, crate::sb::rpc::Then::FolderMade(then));
}

/// The folder was made: the tree opens its theme, the list goes there,
/// and the row `m` was moving goes in.
pub(crate) fn on_folder_made(app: &mut App, id: String, folders: Vec<Folder>, then: Option<(String, String)>) {
    artifacts::set_folders(folders.clone());
    if let Some(sc) = app.artifacts.as_mut() {
        if let Some(t) = tree::theme_of(&Place::Folder(id.clone()), &folders) {
            if !sc.open_themes.contains(&t) {
                sc.open_themes.push(t);
            }
        }
        sc.moved.push(id.clone());
        if then.is_none() {
            sc.place = Place::Folder(id.clone());
        }
    }
    if let Some((project, aid)) = then {
        app.sb.call("artifacts/move", move_params(&project, &aid, Some(&id)), crate::sb::rpc::Then::Shown);
    }
}

/// The tree's rows now (the keys move in them).
fn tree_rows(sc: &Screen) -> Vec<tree::TreeRow> {
    let all = artifacts::library();
    let folders = artifacts::folders();
    let v = crate::artifacts_lines::view(sc, &all, &folders);
    let scoped = v.scoped();
    let every: Vec<&Artifact> = v.every.iter().collect();
    tree::tree(&folders, &scoped, &every, &sc.open_themes, &|a| sc.fresh.row(a))
}

/// A key while the tree has them: ↑↓ the place (the list follows), →
/// opens a theme, ← shuts it (or goes up to it), ⏎ and tab give the
/// keys back to the list.
fn tree_key(sc: &mut Screen, k: &KeyEvent) -> bool {
    let rows = tree_rows(sc);
    let i = rows.iter().position(|r| r.place == sc.place).unwrap_or(0);
    let go = |sc: &mut Screen, j: usize| {
        if let Some(r) = rows.get(j) {
            sc.place = r.place.clone();
            sc.sel = None;
            sc.top = 0;
        }
    };
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => go(sc, i.saturating_sub(1)),
        KeyCode::Down | KeyCode::Char('j') => go(sc, (i + 1).min(rows.len().saturating_sub(1))),
        KeyCode::Right | KeyCode::Char('l') => {
            if let (Some(true | false), Place::Folder(id)) = (rows[i].open, &rows[i].place) {
                if !sc.open_themes.contains(id) {
                    sc.open_themes.push(id.clone());
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let folders = artifacts::folders();
            match (&rows[i].place, rows[i].open) {
                (Place::Folder(id), Some(true)) => sc.open_themes.retain(|t| t != id),
                (p @ Place::Folder(_), None) if rows[i].depth == 1 => {
                    if let Some(t) = tree::theme_of(p, &folders) {
                        sc.place = Place::Folder(t);
                        sc.sel = None;
                    }
                }
                _ => {}
            }
        }
        KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab => sc.in_tree = false,
        KeyCode::Esc => sc.in_tree = false,
        KeyCode::Char('n') => open_pick(sc, 'n'),
        KeyCode::Char('p') => open_pick(sc, 'p'),
        _ => return false,
    }
    true
}

/// Keys while the screen is open: it takes them all.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(sc) = app.artifacts.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('g')) {
        app.artifacts = None;
        return true;
    }
    // a box has the keys
    if let Some(mut p) = sc.pick.clone() {
        let folders = artifacts::folders();
        let lib = artifacts::library();
        let refs: Vec<&Artifact> = lib.iter().collect();
        let choices = pick::rows(&p, &folders, &refs, &sc.moved);
        match pick::key(&mut p, k, &choices, &folders) {
            Outcome::Stay => sc.pick = Some(p),
            Outcome::Close => sc.pick = None,
            Outcome::Chose(c) => {
                sc.pick = Some(p);
                run_choice(app, c);
            }
        }
        return true;
    }
    // the search has the keys
    if sc.typing {
        match k.code {
            KeyCode::Esc => {
                sc.query.clear();
                sc.typing = false;
            }
            KeyCode::Enter => sc.typing = false,
            KeyCode::Up => {
                sc.typing = false;
                step(sc, -1);
            }
            KeyCode::Down => {
                sc.typing = false;
                step(sc, 1);
            }
            KeyCode::Backspace => {
                sc.query.pop();
            }
            KeyCode::Char(c) if !ctrl => sc.query.push(c),
            _ => {}
        }
        return true;
    }
    // Quick Look up: space closes it, ↑↓ preview the next one
    if sc.look.is_some() {
        match k.code {
            KeyCode::Char(' ') => sc.look = None,
            KeyCode::Up | KeyCode::Down => {
                step(sc, if k.code == KeyCode::Up { -1 } else { 1 });
                quick_look(app);
            }
            KeyCode::Enter => {
                sc.look = None;
                open_selected(app);
            }
            KeyCode::Esc => app.artifacts = None,
            _ => {}
        }
        return true;
    }
    if sc.in_tree && !ctrl && tree_key(sc, k) {
        return true;
    }
    match k.code {
        KeyCode::Esc if sc.versions.is_some() => sc.versions = None,
        KeyCode::Esc if !sc.query.is_empty() => sc.query.clear(),
        KeyCode::Esc if sc.archived => show_archived(sc, false),
        KeyCode::Esc => app.artifacts = None,
        KeyCode::Char('a') if !ctrl => show_archived(sc, !sc.archived),
        KeyCode::Up | KeyCode::Char('k') if !ctrl => step(sc, -1),
        KeyCode::Down | KeyCode::Char('j') if !ctrl => step(sc, 1),
        KeyCode::PageUp => step(sc, -10),
        KeyCode::PageDown => step(sc, 10),
        KeyCode::Home => step(sc, -100_000),
        KeyCode::End => step(sc, 100_000),
        KeyCode::Enter => open_selected(app),
        KeyCode::Char('/') => {
            sc.typing = true;
            sc.versions = None;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            sc.in_tree = true;
            sc.versions = None;
        }
        KeyCode::Char(c @ ('m' | 'n' | 'p')) if !ctrl => open_pick(sc, c),
        KeyCode::Char('v') if sc.versions.is_none() => {
            let n = sel_row(sc).map_or(0, |a| a.versions.len());
            if n > 0 {
                sc.versions = Some(0);
            } else {
                flash(app, "it has one version".to_string());
            }
        }
        KeyCode::Char('v') => sc.versions = None,
        KeyCode::Char(' ') if sc.versions.is_none() => quick_look(app),
        KeyCode::Char('r') | KeyCode::Char('o') => reveal(app),
        KeyCode::Char('c') if !ctrl => copy(app),
        KeyCode::Char('@') => put_in_message(app),
        _ => {}
    }
    true
}

/// The archived ones instead of the list (or back).
fn show_archived(sc: &mut Screen, on: bool) {
    sc.archived = on;
    sc.sel = None;
    sc.versions = None;
    sc.top = 0;
}

/// The mouse while open: the wheel scrolls, a click opens, a click on
/// a folder of the tree goes there, on the search focuses it.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(sc) = app.artifacts.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => step(sc, -3),
        MouseEventKind::ScrollDown => step(sc, 3),
        MouseEventKind::Down(MouseButton::Left) => {
            if sc.search_y == Some(m.row) {
                sc.typing = true;
                return true;
            }
            // the tree's rows from column 0 to its edge; the list's after it
            let on_tree = (sc.wide || sc.in_tree) && m.column < sc.x0 + crate::artifacts_lines::TREE_W as u16;
            let hit = sc.hits.iter().find(|(y, h)| *y == m.row && matches!(h, Hit::Tree(_)) == on_tree).map(|(_, h)| h.clone());
            match hit {
                Some(Hit::Tree(p)) => {
                    if let Place::Folder(id) = &p {
                        if sc.place == p && !sc.open_themes.contains(id) {
                            sc.open_themes.push(id.clone());
                        } else if sc.place == p {
                            sc.open_themes.retain(|t| t != id);
                        }
                    }
                    sc.place = p;
                    sc.sel = None;
                    sc.top = 0;
                    sc.in_tree = sc.wide && sc.in_tree;
                }
                Some(Hit::Row(key, _)) => {
                    sc.sel = Some(key);
                    sc.typing = false;
                    sc.in_tree = false;
                    sc.versions = None;
                    open_selected(app);
                }
                Some(Hit::Version(k)) => {
                    sc.versions = Some(k);
                    open_selected(app);
                }
                Some(Hit::Archived) => show_archived(sc, true),
                None => {}
            }
        }
        _ => {}
    }
    true
}

#[cfg(test)]
#[path = "artifacts_screen_tests.rs"]
mod tests;

