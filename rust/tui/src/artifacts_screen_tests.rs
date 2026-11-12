//! /artifacts' render tests at 150 and 80 columns (144 and 74 of text,
//! the frame's margins out), on the page's terminal mock (artifacts-
//! gallery v7, block b-term).

use super::*;
use crate::artifacts::{Artifact, Folder, Version};
use crate::artifacts_lines::lines;
use crate::artifacts_pick::Pick;
use crate::theme::accent;

// 2025-09-30 12:41:00 UTC, a tuesday; the clock in UTC
const T: u64 = 1_759_236_060_000;
const MIN: u64 = 60_000;
const DAY: u64 = 24 * 60 * MIN;

fn utc(_: u64) -> i32 {
    0
}

fn f(id: &str, name: &str, parent: Option<&str>) -> Folder {
    Folder { id: id.into(), name: name.into(), parent: parent.map(str::to_string), by: "main".into(), named_by_you: false }
}

fn art(id: &str, title: &str, kind: &str, agent: &str, ago: u64, folder: Option<&str>) -> Artifact {
    Artifact { id: id.into(), title: title.into(), kind: kind.into(), agent: agent.into(), ts_ms: T - ago, v: 1, target: format!("/w/{id}"), folder: folder.map(str::to_string), project: "h-bise".into(), project_name: "bise".into(), ..Default::default() }
}

/// The mock's folders: desktop app (agent view, inbox, page heroes,
/// plugins), bims, launch, 36 rue de Montreuil, épargne, main's reports.
fn folders() -> Vec<Folder> {
    vec![
        f("f-desk", "desktop app", None),
        f("f-view", "agent view", Some("f-desk")),
        f("f-inbox", "inbox", Some("f-desk")),
        f("f-hero", "page heroes", Some("f-desk")),
        f("f-plug", "plugins", Some("f-desk")),
        f("f-bims", "bims", None),
        f("f-launch", "launch", None),
        f("f-rue", "36 rue de Montreuil", None),
        f("f-ep", "épargne", None),
        f("f-main", "main's reports", None),
    ]
}

/// agent view's work (amb-home's, one of agent-target's, an older one
/// of amb-win's and architect's), page heroes' new one, bims' from
/// another project, two unfiled.
fn library() -> Vec<Artifact> {
    let step2 = Artifact {
        v: 2,
        versions: vec![
            Version { v: 1, ts_ms: T - 60 * MIN, target: "x".into(), copy: None, note: String::new() },
            Version { v: 2, ts_ms: T - 39 * MIN, target: "x".into(), copy: None, note: "2 notes open".into() },
        ],
        ..art("step2", "agent view step 2", "image", "amb-home", 39 * MIN, Some("f-view"))
    };
    vec![
        step2,
        art("fixes", "agent view fixes", "image", "amb-home", 61 * MIN, Some("f-view")),
        art("hero", "hero still, dark", "image", "bims-plugins", 90 * MIN, Some("f-hero")),
        art("head", "agent view head sheet", "image", "amb-home", 156 * MIN, Some("f-view")),
        art("target", "agent-target shots", "page", "agent-target", 209 * MIN, Some("f-view")),
        Artifact { project: "h-bims".into(), project_name: "bims".into(), ..art("town", "town signs", "image", "town-signs", 5 * 60 * MIN, Some("f-bims")) },
        art("mine", "q3 plan", "doc", "", 6 * 60 * MIN, None),
        art("folds", "open folds, dark", "image", "amb-win", 17 * DAY, Some("f-view")),
        art("delta", "ambient-app delta review", "doc", "architect", 17 * DAY + 10 * MIN, Some("f-view")),
        art("loose", "pricing deck", "slides", "launch", 18 * DAY, None),
    ]
}

fn render(sc: &mut Screen, all: &[Artifact], w: usize, h: usize) -> String {
    let clock = Clock { now: T, off: &utc };
    let (lines, _, _) = lines(sc, all, &folders(), w, h, &clock);
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
}

/// In agent view, its theme open, the step-2 shots and the hero new.
fn screen() -> Screen {
    let mut fresh = Fresh::default();
    fresh.take(&library()[..1], Some(T - 50 * MIN));
    fresh.take(&library()[2..3], None);
    Screen { place: Place::Folder("f-view".into()), open_themes: vec!["f-desk".into()], sel: Some("h-bise/fixes".into()), fresh, ..Default::default() }
}

fn assert_fits(out: &str, w: usize) {
    for l in out.lines() {
        assert!(unicode_width::UnicodeWidthStr::width(l) <= w, "too wide at {w}: {l}\n{out}");
    }
}

#[test]
fn at_150_the_tree_is_at_the_left_of_the_crumb_and_the_list() {
    let all = library();
    let mut sc = screen();
    let out = render(&mut sc, &all, 144, 24);
    let want = [
        "artifacts  10 from 2 projects · 10 folders · 2 new · 2 unfiled",
        "",
        "  all artifacts             10  │ desktop app / agent view  6 · amb-home and 3 more",
        "  unfiled                    2  │",
        "                                │ today",
        "▾ desktop app                •  │ agent view step 2 new                   image                      12:02",
        "    agent view               6  │ agent view fixes                        image                      11:40",
        "    inbox                       │ agent view head sheet                   image                      10:05",
        "    page heroes              •  │ agent-target shots                      page        agent-target   09:12",
        "    plugins                     │",
        "  bims                          │ earlier",
        "  36 rue de Montreuil           │ open folds, dark                        image       amb-win       sep 13",
        "  launch                        │ ambient-app delta review                doc         architect     sep 13",
    ];
    let got: Vec<&str> = out.lines().collect();
    for (i, w) in want.iter().enumerate() {
        assert_eq!(got[i], *w, "line {i}\n{out}");
    }
    assert!(out.contains("  main's reports                │"), "{out}");
    assert!(out.ends_with("⏎ open   space look   m move to…   n new folder   tab folders   p project   / search   esc back"), "{out}");
    assert_fits(&out, 144);
}

#[test]
fn at_80_a_crumb_and_tab_opens_the_tree_in_the_lists_place() {
    let all = library();
    let mut sc = screen();
    let out = render(&mut sc, &all, 74, 22);
    let want = [
        "artifacts · 10 · 2 new",
        "desktop app / agent view  6            tab folders",
        "",
        "today",
        "agent view step 2 new                   image       12:02",
        "agent view fixes                        image       11:40",
        "agent view head sheet                   image       10:05",
        "agent-target shots                      page        09:12",
        "",
        "earlier",
        "open folds, dark                        image      sep 13",
        "ambient-app delta review                doc        sep 13",
    ];
    let got: Vec<&str> = out.lines().collect();
    for (i, w) in want.iter().enumerate() {
        assert_eq!(got[i], *w, "line {i}\n{out}");
    }
    assert!(out.ends_with("⏎ open   m move   tab folders   p project   esc back"), "{out}");
    assert_fits(&out, 74);
    sc.in_tree = true;
    let out = render(&mut sc, &all, 74, 22);
    assert!(out.contains("\n  all artifacts             10\n  unfiled                    2\n\n▾ desktop app                •\n    agent view               6"), "{out}");
    assert!(!out.contains("agent view fixes"), "the tree takes the list's place\n{out}");
    assert!(out.ends_with("↑↓ folders   →← open, shut   ⏎ show it   n new folder   esc back"), "{out}");
    assert_fits(&out, 74);
}

#[test]
fn the_new_ones_are_in_the_accent_with_new_right_after_and_the_tree_dots_them() {
    let all = library();
    let mut sc = screen();
    let clock = Clock { now: T, off: &utc };
    let (ls, _, _) = lines(&mut sc, &all, &folders(), 144, 24, &clock);
    let row = ls.iter().find(|l| l.spans.iter().any(|s| s.content.contains("agent view step 2"))).unwrap();
    let title = row.spans.iter().find(|s| s.content.contains("agent view step 2")).unwrap();
    assert_eq!(title.style.fg, Some(crate::theme::text()), "only `new` takes the accent");
    let tag = row.spans.iter().find(|s| s.content == " new").unwrap();
    assert_eq!(tag.style.fg, Some(accent()));
    let dot = row.spans.iter().find(|s| s.content.trim() == "•").unwrap();
    assert_eq!(dot.style.fg, Some(accent()));
    // the selected row is tinted while the list has the keys; the tree's place when it has them
    let sel = ls.iter().find(|l| l.spans.iter().any(|s| s.content.contains("agent view fixes"))).unwrap();
    assert!(sel.spans.iter().any(|s| s.style.bg == Some(crate::theme::selection_bg())));
    sc.in_tree = true;
    let (ls, _, _) = lines(&mut sc, &all, &folders(), 144, 24, &clock);
    let sel = ls.iter().find(|l| l.spans.iter().any(|s| s.content.contains("agent view fixes"))).unwrap();
    let tinted: Vec<&str> = sel.spans.iter().filter(|s| s.style.bg == Some(crate::theme::selection_bg())).map(|s| s.content.as_ref()).collect();
    assert!(tinted.concat().contains("agent view") && !tinted.concat().contains("fixes"), "{tinted:?}");
}

#[test]
fn in_all_artifacts_a_row_says_its_folder() {
    let all = library();
    let mut sc = Screen { place: Place::All, ..screen() };
    let out = render(&mut sc, &all, 144, 26);
    assert!(out.contains("│ all artifacts  10"), "{out}");
    assert!(out.contains("hero still, dark new                    image       page heroes          11:11"), "{out}");
    assert!(out.contains("q3 plan                                 doc                              06:41"), "{out}");
    assert!(out.contains("pricing deck                            slides      launch"), "{out}");
}

#[test]
fn p_shows_one_project_and_the_counts_say_of() {
    let all = library();
    let mut sc = Screen { project: Some("h-bise".into()), ..screen() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.starts_with("artifacts  bise · 9 of 10 · 10 folders · 2 new · 2 unfiled"), "{out}");
    assert!(out.contains("│ desktop app / agent view  6 of 6"), "{out}");
    sc.place = Place::Folder("f-bims".into());
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("nothing from bise here. p shows every project."), "{out}");
    let mut sc = Screen { query: "in:bims".into(), ..screen() };
    let out = render(&mut sc, &all, 74, 24);
    assert!(out.starts_with("artifacts · bims · 1 of 10"), "{out}");
}

#[test]
fn the_search_looks_in_every_folder() {
    let all = library();
    let mut sc = Screen { typing: true, query: "deck".into(), ..screen() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("\n/ deck▏   1 in every folder\n"), "{out}");
    assert!(out.contains("│ every folder  1"), "{out}");
    assert!(out.contains("pricing deck") && !out.contains("agent view fixes"), "{out}");
    assert!(out.ends_with("⏎ done   ↑↓ choose   esc clear the search"), "{out}");
    sc.query = "zzz".into();
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("nothing matches. esc clears the search."), "{out}");
}

#[test]
fn m_opens_the_picker_in_the_lists_place() {
    let all = library();
    let mut sc = screen();
    sc.pick = Some(Pick::Move { key: "h-bise/fixes".into(), title: "agent view fixes".into(), from: Some("f-view".into()), query: "pa".into(), sel: 0 });
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("  all artifacts             10  │ ╭─ move \"agent view fixes\" to… ─"), "{out}");
    assert!(out.contains("│ │ 1 page heroes  desktop app"), "{out}");
    assert!(out.contains("│ │ + new folder \"pa\""), "{out}");
    assert!(out.ends_with("1-2 pick   ⏎ move   ctrl+⏎ new folder   esc back"), "{out}");
    let out = render(&mut sc, &all, 74, 24);
    assert!(out.contains("\n╭─ move \"agent view fixes\" to… ─"), "{out}");
    assert_fits(&out, 74);
}

#[test]
fn v_opens_the_versions_under_the_row() {
    let all = library();
    let mut sc = Screen { sel: Some("h-bise/step2".into()), versions: Some(0), ..screen() };
    let out = render(&mut sc, &all, 144, 26);
    assert!(out.contains("│   ╭─ agent view step 2 · 2 versions"), "{out}");
    assert!(out.contains("│   │ › v2  new   39 min ago   2 notes open   the current one"), "{out}");
    assert!(out.ends_with("⏎ open this version   ↑↓ choose   esc back to the list"), "{out}");
}

#[test]
fn empty_places_say_what_to_do() {
    let mut sc = Screen { place: Place::All, ..screen() };
    // hub-loop: before the hub's list came (built off its loop), an empty
    // list is loading, never "nothing yet"
    let out = render(&mut sc, &[], 144, 20);
    assert!(out.contains("loading the list…"), "{out}");
    crate::artifacts::set_rows(&[], None, "p");
    let out = render(&mut sc, &[], 144, 20);
    assert!(out.contains("nothing yet. when an agent makes a page, a doc or a file for you, it lands here."), "{out}");
    let all: Vec<Artifact> = library().into_iter().filter(|a| a.folder.is_some()).collect();
    let mut sc = Screen { place: Place::Unfiled, ..screen() };
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("nothing unfiled: every artifact has its folder."), "{out}");
    let mut sc = Screen { place: Place::Folder("f-plug".into()), ..screen() };
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("nothing here yet. m on an artifact moves it here."), "{out}");
    assert!(out.ends_with("n new folder   tab folders   p project   / search   esc back"), "{out}");
    // a folder deleted while it is open: back to all artifacts
    let mut sc = Screen { place: Place::Folder("f-gone".into()), ..screen() };
    render(&mut sc, &all, 144, 20);
    assert_eq!(sc.place, Place::All);
}

#[test]
fn a_long_list_keeps_the_selection_in_view_and_says_how_many_more() {
    let mut all = library();
    for i in 0..30 {
        all.push(art(&format!("old{i}"), &format!("old thing {i}"), "doc", "amb-home", 20 * DAY + i * MIN, Some("f-view")));
    }
    let mut sc = screen();
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("│ ↓ ") && out.contains(" more"), "{out}");
    sc.sel = Some("h-bise/old29".into());
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("old thing 29"), "{out}");
    let out = render(&mut sc, &all, 74, 20);
    assert!(out.contains("old thing 29"), "{out}");
}

#[test]
fn archived_ones_leave_the_list_behind_a_faint_row_and_a_opens_them() {
    let mut all = library();
    for a in all.iter_mut().filter(|a| a.id == "head" || a.id == "target") {
        a.archived_at = Some(T);
    }
    let mut sc = screen();
    let out = render(&mut sc, &all, 144, 30);
    assert!(!out.contains("agent view head sheet") && !out.contains("agent-target shots"), "{out}");
    assert!(out.contains("│ 2 archived\n"), "{out}");
    sc.archived = true;
    let out = render(&mut sc, &all, 144, 30);
    assert!(out.starts_with("artifacts · archived  2"), "{out}");
    assert!(out.contains("agent view head sheet") && !out.contains("agent view fixes"), "{out}");
    assert!(out.ends_with("esc back to the list"), "{out}");
}

/// An older hub sends no `seen_ms`: a new row's current version only.
#[test]
fn without_the_hubs_seen_only_the_current_version_is_new() {
    let all = library();
    let mut fresh = Fresh::default();
    fresh.take(&all[..1], None);
    let p = &all[0];
    let new: Vec<u32> = p.versions.iter().filter(|v| fresh.version(p, v)).map(|v| v.v).collect();
    assert_eq!(new, [2]);
    assert!(!fresh.version(&all[1], &p.versions[0]), "a row not new has no new version");
}

/// The library's answer: the other projects' rows with their project's
/// name and their folder.
#[test]
fn the_library_answer_reads_rows_and_folders() {
    let row: bise_proto::rows::Artifact = serde_json::from_value(serde_json::json!(
        {"id": "a1", "kind": "image", "title": "town signs", "agent": "town-signs", "version": 1, "at_ms": 5, "url": "/x", "new": false, "folder": "f-bims"}
    ))
    .unwrap();
    let projects = vec![bise_proto::rows::LibraryProject { project: "h-1".into(), name: "bims".into(), items: vec![row] }];
    let rows = crate::artifacts::of_library(&projects);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].project.as_str(), rows[0].project_name.as_str(), rows[0].folder.as_deref(), rows[0].key().as_str()), ("h-1", "bims", Some("f-bims"), "h-1/a1"));
}

/// A project he renames keeps its rows, its kept place, its pick and
/// where a move goes: all of them go by hub id, the name only shows
/// (architect m_40803).
#[test]
fn a_renamed_project_keeps_its_rows_its_place_and_its_moves() {
    let mut all = library();
    let mut sc = Screen { project: Some("h-bims".into()), place: Place::Folder("f-bims".into()), sel: Some("h-bims/town".into()), ..screen() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.starts_with("artifacts  bims · 1 of 10"), "{out}");
    assert!(out.contains("town signs"), "{out}");
    for a in all.iter_mut().filter(|a| a.project == "h-bims") {
        a.project_name = "bims and towns".into();
    }
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.starts_with("artifacts  bims and towns · 1 of 10"), "{out}");
    assert!(out.contains("town signs"), "{out}");
    assert_eq!(sc.sel.as_deref(), Some("h-bims/town"), "the selection stays on it");
    // the kept screen (sb/keep.rs) says the place and the project by id
    assert_eq!(crate::artifacts_tree::place_of(&crate::artifacts_tree::place_word(&sc.place)), sc.place);
    assert_eq!(sc.project.as_deref(), Some("h-bims"));
    // in:<name> finds it by its new name
    let mut s2 = Screen { query: "in:towns".into(), ..screen() };
    assert!(render(&mut s2, &all, 144, 24).starts_with("artifacts  bims and towns · 1 of 10"));
    // a move goes to its hub id
    let town = all.iter().find(|a| a.id == "town").unwrap();
    assert_eq!(move_params(&town.project, &town.id, Some("f-view")), serde_json::json!({"project": "h-bims", "ids": ["town"], "folder": "f-view"}));
}

/// The designer's shots (m_40704): every state at 150 and 80 columns,
/// dark and light, from the test buffer, as one HTML page in a mono
/// stack (a glyph wider than its cell breaks the grid), and every
/// non-ASCII glyph the screen drew with its width in the code. Writes
/// only with `ART_TUI_SHOTS=<file.html>`.
#[test]
fn shots_for_the_designer() {
    let Some(path) = std::env::var_os("ART_TUI_SHOTS") else { return };
    use crate::theme::{set_mode, Mode};
    use ratatui::style::{Color, Modifier};
    let css = |c: Option<Color>| match c {
        Some(Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "inherit".into(),
    };
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let all = library();
    let pick_move = Some(Pick::Move { key: "h-bise/fixes".into(), title: "agent view fixes".into(), from: Some("f-view".into()), query: "pa".into(), sel: 0 });
    let pick_proj = Some(Pick::Project { query: String::new(), sel: 1 });
    let pick_new = Some(Pick::NewFolder { parent: Some("f-desk".into()), name: "plugins v2".into() });
    let filed: Vec<Artifact> = library().into_iter().filter(|a| a.folder.is_some()).collect();
    let states: Vec<(&str, Screen, &[Artifact])> = vec![
        ("a task folder, the list has the keys", screen(), &all),
        ("tab: the tree has the keys", Screen { in_tree: true, ..screen() }, &all),
        ("all artifacts", Screen { place: Place::All, sel: Some("h-bise/hero".into()), ..screen() }, &all),
        ("m: move to…, typing \"pa\"", Screen { pick: pick_move, ..screen() }, &all),
        ("p: a project", Screen { pick: pick_proj, ..screen() }, &all),
        ("p picked bise: counts say of", Screen { project: Some("h-bise".into()), ..screen() }, &all),
        ("n: new folder in desktop app", Screen { pick: pick_new, ..screen() }, &all),
        ("/ search across folders", Screen { typing: true, query: "dark".into(), ..screen() }, &all),
        ("v: versions", Screen { sel: Some("h-bise/step2".into()), versions: Some(0), ..screen() }, &all),
        ("unfiled, empty", Screen { place: Place::Unfiled, ..screen() }, &filed),
        ("first run", Screen { place: Place::All, ..screen() }, &[]),
    ];
    let mut glyphs: Vec<char> = Vec::new();
    let mut body = String::new();
    for mode in [Mode::Dark, Mode::Light] {
        set_mode(mode);
        let (bg, ink) = (css(Some(crate::theme::bg())), css(Some(crate::theme::text())));
        body.push_str(&format!("<h2>{}</h2>", if mode == Mode::Dark { "dark" } else { "light" }));
        for (name, sc, rows) in &states {
            for (cols, w) in [(150usize, 144usize), (80, 74)] {
                let mut sc = Screen { pick: sc.pick.clone(), fresh: sc.fresh.clone(), open_themes: sc.open_themes.clone(), sel: sc.sel.clone(), place: sc.place.clone(), project: sc.project.clone(), query: sc.query.clone(), ..Default::default() };
                let st = states.iter().find(|s| s.0 == *name).unwrap();
                sc.in_tree = st.1.in_tree;
                sc.typing = st.1.typing;
                sc.versions = st.1.versions;
                let clock = Clock { now: T, off: &utc };
                let (ls, _, _) = lines(&mut sc, rows, &folders(), w, 26, &clock);
                body.push_str(&format!("<h3>{name} · {cols} columns</h3><pre style=\"background:{bg};color:{ink};width:{w}ch\">"));
                for l in &ls {
                    for s in &l.spans {
                        for c in s.content.chars().filter(|c| !c.is_ascii()) {
                            if !glyphs.contains(&c) {
                                glyphs.push(c);
                            }
                        }
                        let m = s.style.add_modifier;
                        let mut style = format!("color:{};background:{}", css(s.style.fg), css(s.style.bg));
                        if m.contains(Modifier::BOLD) {
                            style.push_str(";font-weight:700");
                        }
                        let deco: Vec<&str> = [(Modifier::UNDERLINED, "underline"), (Modifier::CROSSED_OUT, "line-through")].iter().filter(|(k, _)| m.contains(*k)).map(|(_, v)| *v).collect();
                        if !deco.is_empty() {
                            style.push_str(&format!(";text-decoration:{}", deco.join(" ")));
                        }
                        body.push_str(&format!("<span style=\"{style}\">{}</span>", esc(&s.content)));
                    }
                    body.push('\n');
                }
                body.push_str("</pre>");
            }
        }
    }
    set_mode(Mode::Dark);
    glyphs.sort();
    let mut table = String::from("<h2>every non-ASCII glyph the screen drew, with its width in the code (unicode-width)</h2><table><tr><th>glyph</th><th>U+</th><th>width</th><th>in a grid of 1-cell x</th></tr>");
    for c in &glyphs {
        let w = unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0);
        table.push_str(&format!("<tr><td class=g>{}</td><td>{:04X}</td><td>{w}</td><td class=g>x{}x</td></tr>", esc(&c.to_string()), *c as u32, esc(&c.to_string())));
    }
    table.push_str("</table><p>no kind glyph is drawn: the kind is a dim word in its column (page, image, doc, slides…).</p>");
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>/artifacts in the terminal · art-tui shots</title><style>body{{font:14px -apple-system,sans-serif;margin:24px;background:#888}}pre,.g{{font-family:\"SF Mono\",\"JetBrains Mono\",monospace;font-size:13px;line-height:1.35}}pre{{padding:8px 0;margin:0 0 20px;white-space:pre;overflow:hidden}}h3{{font-weight:500;margin:8px 0 4px}}table{{background:#fff;border-collapse:collapse}}td,th{{border:1px solid #ccc;padding:2px 8px}}</style><h1>/artifacts in the terminal (artifacts-folders part 6)</h1><p>from the render tests' buffer (artifacts_screen_tests.rs), 144 and 74 columns of text (150 and 80 of terminal, the frame's margins out), 26 rows; fake library.</p>{table}{body}"
    );
    std::fs::write(path, html).unwrap();
}
