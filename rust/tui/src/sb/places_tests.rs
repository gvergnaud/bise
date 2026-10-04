//! The worktrees in the panel (pr-design §4.1, option B): the 6 rules at the
//! panel's widths (24, 31, 44), ctrl up and held, NO_COLOR, ASCII.

use super::places::{Place, Pr};
use super::*;
use crate::ctrlhint::{Held, Hold};
use ratatui::{backend::TestBackend, Terminal};

fn agent(name: &str, status: &str, place: &str) -> Agent {
    Agent { name: name.into(), status: status.into(), place_id: place.into(), ..Agent::default() }
}

fn pr(number: u64, review: &str, checks: &str) -> Pr {
    Pr {
        number,
        url: format!("https://github.com/acme/web/pull/{number}"),
        state: "open".into(),
        review: review.into(),
        checks: checks.into(),
        ..Pr::default()
    }
}

fn place(dir: &str, agents: &[&str], pr: Option<Pr>, lid: Option<&str>) -> Place {
    Place {
        id: format!("wt:{dir}"),
        branch: Some(format!("sb/{dir}")),
        agents: agents.iter().map(|a| a.to_string()).collect(),
        pr,
        lid: lid.map(Into::into),
        ..Place::default()
    }
}

/// The mock's panel (sidebar-wt.html, option A): main and cookies in
/// your folder, dark-mode and i18n sharing a worktree with PR #412
/// (changes asked), login-fix alone (#415, checks fail), emoji-csv alone
/// (no PR yet), palette alone (draft #418), release alone (waits to
/// land), docs alone (#409, ready to merge: an inbox item asks you).
/// Numbers in creation order, so the blocks reorder the rows, not the
/// numbers.
fn mock() -> App {
    let mut app = bench::test_app_drained();
    let sb = &mut app.sb;
    sb.agents = vec![
        Agent { main: true, turn_ms: Some(60_000), ..agent("main", "working", "shared") },
        Agent { turn_ms: Some(180_000), ..agent("dark-mode", "working", "wt:dark-mode") },
        agent("cookies", "idle", "shared"),
        Agent { turn_ms: Some(300_000), ..agent("login-fix", "working", "wt:login-fix") },
        Agent { turn_ms: Some(42_000), ..agent("i18n", "working", "wt:dark-mode") },
        Agent { turn_ms: Some(120_000), ..agent("emoji-csv", "working", "wt:emoji-csv") },
        agent("palette", "idle", "wt:palette"),
        agent("release", "waiting", "wt:release"),
        agent("docs", "idle", "wt:docs"),
    ];
    let failing = Pr { failing: vec!["e2e/login".into()], ..pr(415, "none", "fail") };
    let draft = Pr { state: "draft".into(), ..pr(418, "none", "running") };
    sb.places = vec![
        place("dark-mode", &["dark-mode", "i18n"], Some(pr(412, "changes_requested", "pass")), None),
        place("login-fix", &["login-fix"], Some(failing), None),
        place("emoji-csv", &["emoji-csv"], None, Some("no PR yet · 2 commits")),
        place("palette", &["palette"], Some(draft), None),
        Place { branch: Some("sb/release-notes".into()), ..place("release", &["release"], None, Some("waits to land · 2nd")) },
        place("docs", &["docs"], Some(pr(409, "approved", "pass")), None),
    ];
    sb.flow = "pr".into();
    sb.activity.insert("i18n".into());
    sb.cards.push(cards::Card {
        id: 1,
        kind: "merge".into(),
        agent: "docs".into(),
        text: "#409 is approved, checks pass. merge it?".into(),
        age_ms: 0,
        seen_at: std::time::Instant::now(),
        note: String::new(),
        look: None,
        place: None,
        pr: None,
        link: None,
        asking: false,
    });
    app
}

fn hold(app: &mut App) {
    app.hold = Hold::of(Held::Ctrl, std::time::Instant::now() - std::time::Duration::from_secs(2));
}

/// The panel `w` wide drawn at column 1 (column 0 is the blank column
/// left of it, where the rails go), `h` rows; the rows trimmed.
fn rows(app: &App, w: u16, h: u16) -> Vec<String> {
    buffer(app, w, h).1
}

fn buffer(app: &App, w: u16, h: u16) -> (ratatui::buffer::Buffer, Vec<String>) {
    let mut term = Terminal::new(TestBackend::new(w + 1, h)).unwrap();
    term.draw(|f| panel::draw_panel(app, f, Rect::new(1, 0, w, h))).unwrap();
    let buf = term.backend().buffer().clone();
    let rows = buf
        .content
        .chunks((w + 1) as usize)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
        .collect();
    (buf, rows)
}

fn show(rows: &[String]) -> String {
    rows.join("\n")
}

/// The panel's width on a `width`-column screen, framed.
fn panel_w(width: u16) -> u16 {
    crate::layout::cols(width, 40).panel.unwrap().w
}

/// A section's title row: ` ψ <branch>` at the titles' column.
fn is_title(l: &str) -> bool {
    l.starts_with("  ψ ") || l.starts_with("  Δ ")
}

/// Option B at 31 columns (150 wide), at rest: the `agents` section
/// holds every agent not in a shared worktree at its number, the solo
/// ones with their mark in the last column; then a section for the
/// worktree 2 agents share, its title like `agents` (no box lines, its ↑
/// in the mark column, its rows' mark column blank), the inbox.
#[test]
fn a_section_only_when_they_share() {
    let app = mock();
    let w = panel_w(150) as usize;
    let r = rows(&app, panel_w(150), 24);
    let title = format!("  ψ sb/dark-mode{}↑", " ".repeat(w - 17));
    let want = [
        "  agents",
        "",
        "  0 ∿ main :*       1m",
        "  2 ○ cookies",
        "  3 ∿ login-fix     5m       ↑",
        "  5 ∿ emoji-csv     2m       ψ",
        "  6 ○ palette                ↑",
        "  7 … release                …",
        "  8 ○ docs                   ↑",
        "",
        title.as_str(),
        "  1 ∿ dark-mode     3m",
        "  4 ∿ i18n •       42s",
        "",
        "  inbox",
    ];
    for (k, line) in want.iter().enumerate() {
        assert_eq!(r[k], *line, "row {k}:\n{}", show(&r));
    }
    // no box line anywhere, not even in the blank column left of the panel
    assert!(!r.iter().any(|l| l.contains(['╭', '│', '╰', '─'])), "{}", show(&r));
    // the mark one column from the edge, the title's in the same column
    assert_eq!(r[4].chars().count(), w);
    assert_eq!(r[10].chars().count(), w);
    let col = |l: &str, w: &str| l.char_indices().position(|(i, _)| l[i..].starts_with(w));
    assert_eq!(col(&r[2], "1m"), col(&r[11], "3m"), "{}", show(&r));
    assert_eq!(col(&r[2], "1m"), col(&r[4], "5m"), "{}", show(&r));
    // the title in the `agents` title's color, its ↑ by the PR (dim)
    let (buf, _) = buffer(&app, panel_w(150), 24);
    assert_eq!(buf[(2, 10)].symbol(), "ψ");
    assert_eq!((buf[(2, 10)].fg, buf[(4, 10)].fg), (buf[(2, 0)].fg, buf[(2, 0)].fg));
    assert_eq!((buf[(w as u16 - 1, 10)].symbol(), buf[(w as u16 - 1, 10)].fg), ("↑", dim()));
    // a long name in the section takes the mark's column back before it is cut
    let mut long = mock();
    long.sb.agents[4].name = "i18n-everywhere".into();
    long.sb.places[0].agents[1] = "i18n-everywhere".into();
    let r = rows(&long, panel_w(150), 24);
    assert!(r.iter().any(|l| l.starts_with("  4 ∿ i18n-everywhere 42s")), "{}", show(&r));
    // the selection follows the rows' order, the numbers stay
    let names: Vec<&str> = app.sb.nav().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["main", "cookies", "login-fix", "emoji-csv", "palette", "release", "docs", "dark-mode", "i18n"]);
}

/// The `agents` section is in number order, your folder's agents and
/// the solo ones mixed (designer: they sit at their number).
#[test]
fn solo_rows_sit_at_their_number() {
    let mut app = mock();
    // dark-mode alone (1) sits between main (0) and cookies (2)
    app.sb.agents[4].status = "archived".into();
    let names: Vec<String> = app.sb.blocks()[0].1.iter().map(|a| a.name.clone()).collect();
    assert_eq!(names, ["main", "dark-mode", "cookies", "login-fix", "emoji-csv", "palette", "release", "docs"]);
}

/// A shared section that drops to one live agent is a plain row on the
/// next draw (its ↑ in the mark column), and a section again when a
/// second one joins; sections order by their lowest number.
#[test]
fn a_section_down_to_one_agent_is_a_row() {
    let mut app = mock();
    app.sb.agents[4].status = "archived".into();
    let r = rows(&app, panel_w(150), 24);
    assert!(!r.iter().any(|l| is_title(l)), "{}", show(&r));
    let dark = r.iter().find(|l| l.contains("dark-mode")).unwrap();
    assert!(dark.starts_with("  1 ∿ dark-mode     3m") && dark.ends_with('↑'), "{}", show(&r));
    // at its number, right after main
    assert!(r[3].contains("dark-mode") && r[4].contains("cookies"), "{}", show(&r));
    // a second one joins: the section is back
    app.sb.agents[4].status = "working".into();
    app.sb.agents[6].place_id = "wt:login-fix".into();
    app.sb.places[1].agents.push("palette".into());
    app.sb.places[3].agents.clear();
    let r = rows(&app, panel_w(150), 30);
    let boxes: Vec<&String> = r.iter().filter(|l| is_title(l)).collect();
    assert_eq!(boxes.len(), 2, "{}", show(&r));
    assert!(boxes[0].contains("sb/dark-mode") && boxes[1].contains("sb/login-fix"), "{}", show(&r));
    // palette's own worktree (its draft still open, no agent): a row with
    // no number and no glyph, its mark in the mark column
    let orphan = r.iter().find(|l| l.contains("sb/palette")).unwrap();
    assert!(orphan.starts_with("      sb/palette ") && orphan.ends_with('↑'), "{}", show(&r));
}

/// Call 8: a worktree with no live agent and no open PR (a `gate.sh
/// new` scratch worktree, a merged PR) is never shown; an agent in a
/// private worktree the hub has no place for keeps its ψ.
#[test]
fn a_worktree_with_nothing_to_say_is_not_shown() {
    let mut app = mock();
    app.sb.places.push(place("scratch", &[], None, None));
    let merged = Pr { state: "merged".into(), ..pr(400, "approved", "pass") };
    app.sb.places.push(place("old", &[], Some(merged), None));
    let r = rows(&app, panel_w(150), 30);
    assert!(!r.iter().any(|l| l.contains("scratch") || l.contains("sb/old")), "{}", show(&r));
    // a merged PR, its agent still there: ψ, no ↑
    app.sb.places[1].pr.as_mut().unwrap().state = "merged".into();
    let r = rows(&app, panel_w(150), 30);
    assert!(r.iter().any(|l| l == "  3 ∿ login-fix     5m       ψ"), "{}", show(&r));
}

/// Designer's call 8 (BISE-136): a private worktree (`gate.sh new`, id
/// `pt:<path>`) follows the one rule. Alone: a row with `ψ` dim; held,
/// its count then `ψ <task folder>` (detached, no branch); no commit of
/// its own: no held line. Shared by two agents: a box named by the
/// folder. A branch checked out there replaces the folder everywhere,
/// and its PR gets the `↑` like any worktree's.
#[test]
fn a_private_worktree_is_a_worktree() {
    let path = "/u/.bise/worktrees/harness-3abb/fix/harness";
    assert_eq!(places::folder_of(path), "fix");
    assert_eq!(places::folder_of("/tmp/fix-wt/"), "fix-wt");
    let private = |agents: &[&str], lid: Option<&str>| Place {
        id: format!("pt:{path}"),
        branch: None,
        agents: agents.iter().map(|a| a.to_string()).collect(),
        pr: None,
        lid: lid.map(Into::into),
        ..Place::default()
    };
    let mut app = mock();
    app.sb.agents.push(Agent { place: path.into(), ..agent("fix", "idle", &format!("pt:{path}")) });
    app.sb.places.push(private(&["fix"], Some("no PR yet · 2 commits")));
    let r = rows(&app, panel_w(150), 30);
    let fix = r.iter().position(|l| l.contains(" fix ")).unwrap_or_else(|| panic!("{}", show(&r)));
    assert_eq!(r[fix], "  9 ○ fix                    ψ", "{}", show(&r));
    assert!(r[fix - 1].contains("docs"), "with the solo rows, by number: {}", show(&r));
    let (buf, _) = buffer(&app, panel_w(150), 30);
    assert_eq!(buf[(panel_w(150) - 1, fix as u16)].fg, dim());
    hold(&mut app);
    let r = rows(&app, panel_w(150), 32);
    let fix = r.iter().position(|l| l.contains(" fix ")).unwrap();
    assert!(r[fix + 1].starts_with("      no PR yet · 2 commits"), "{}", show(&r));
    let words = app.sb.places[6].words_line(Some("fix"), 60).unwrap();
    let t: String = words.spans.iter().map(|s| s.content.to_string()).collect();
    assert_eq!(t, "     no PR yet · 2 commits · ψ fix");
    // detached, no commit of its own: nothing to say
    assert!(private(&["fix"], None).words_line(Some("fix"), 60).is_none());
    // the divider: the folder
    app.sb.focus = "fix".into();
    assert_eq!(panel::viewed_who(&app).place.as_deref(), Some("fix"));
    // two agents in it: one section, the folder in its title
    let mut app = mock();
    for n in ["fix", "fix-2"] {
        app.sb.agents.push(Agent { place: path.into(), ..agent(n, "working", &format!("pt:{path}")) });
    }
    app.sb.places.push(private(&["fix", "fix-2"], None));
    let r = rows(&app, panel_w(150), 30);
    let top = r.iter().position(|l| l == "  ψ fix").unwrap_or_else(|| panic!("{}", show(&r)));
    assert!(r[top - 1].is_empty(), "{}", show(&r));
    assert!(r[top + 1].starts_with("  9 ∿ fix") && r[top + 2].contains("fix-2"), "{}", show(&r));
    assert!(!r.iter().any(|l| l.ends_with('ψ') && l.contains("fix")), "no ψ in the section's rows: {}", show(&r));
    // a branch checked out there, with a PR: its name and its ↑
    app.sb.places[6].branch = Some("feat/login".into());
    app.sb.places[6].pr = Some(pr(420, "pending", "pass"));
    let r = rows(&app, panel_w(150), 30);
    assert!(r.iter().any(|l| l.starts_with("  ψ feat/login ") && l.ends_with('↑')), "{}", show(&r));
    app.sb.focus = "fix".into();
    let who = panel::viewed_who(&app);
    assert_eq!((who.place.as_deref(), who.with.clone()), (Some("feat/login"), vec!["fix-2".to_string()]));
}

/// The marks' colors: ↑ red when checks fail, accent only through an
/// inbox item (ready to merge), faint draft, dim open; ψ and … dim; a
/// section's title in the titles' color, its ↑ by the same rules.
#[test]
fn the_marks_colors() {
    let app = mock();
    let (buf, r) = buffer(&app, panel_w(150), 24);
    let y = |s: &str| r.iter().position(|l| l.contains(s)).unwrap() as u16;
    let at = |x: u16, y: u16| buf[(x, y)].clone();
    let mark = |row: u16| at(panel_w(150) - 1, row);
    assert_eq!(mark(y("login-fix")).symbol(), "↑");
    assert_eq!(mark(y("login-fix")).fg, error());
    assert_eq!((mark(y("emoji-csv")).symbol(), mark(y("emoji-csv")).fg), ("ψ", dim()));
    assert_eq!(mark(y("palette")).fg, faint());
    assert_eq!((mark(y("release")).symbol(), mark(y("release")).fg), ("…", dim()));
    assert_eq!(mark(y("docs")).fg, accent());
    // no inbox item: dim
    let mut quiet = mock();
    quiet.sb.cards.clear();
    let (qbuf, qr) = buffer(&quiet, panel_w(150), 24);
    let docs = qr.iter().position(|l| l.contains("docs")).unwrap() as u16;
    assert_eq!(qbuf[(panel_w(150) - 1, docs)].fg, dim());
    // the section: no line, ψ in the titles' color, its ↑ dim (never
    // accent), red when its checks fail
    let dark = y("sb/dark-mode");
    assert_eq!(at(0, dark).symbol(), " ");
    assert_eq!((at(2, dark).symbol(), at(2, dark).fg), ("ψ", text()));
    assert_eq!((mark(dark).symbol(), mark(dark).fg), ("↑", dim()));
    let mut red = mock();
    red.sb.places[0].pr.as_mut().unwrap().checks = "fail".into();
    red.sb.cards[0].text = "#412 merge it?".into();
    let (rbuf, _) = buffer(&red, panel_w(150), 24);
    assert_eq!(rbuf[(panel_w(150) - 1, dark)].fg, error());
}

/// Ctrl held: the rows' state words, the mark stays; under each solo
/// row its git state in words at the name's column, dim (`checks fail`
/// red), cut with `…`; `ψ <branch>` first only when the branch isn't the
/// agent's name; your folder's rows get nothing; the box keeps its
/// number and lid. The header adds the PRs and the flow.
#[test]
fn ctrl_held_says_the_words() {
    let mut app = mock();
    hold(&mut app);
    let r = rows(&app, panel_w(150), 32);
    let at = |s: &str| r.iter().position(|l| l.contains(s)).unwrap_or_else(|| panic!("{s}:\n{}", show(&r)));
    assert_eq!(r[at("cookies") + 1], "  3 ∿ login-fix     working  ↑", "{}", show(&r));
    let want = [
        ("login-fix", "      #415 · checks fail: e2e…"),
        ("emoji-csv", "      no PR yet · 2 commits"),
        ("palette", "      #418 · draft · checks r…"),
        ("release", "      waits to land · 2nd · ψ…"),
        ("docs", "      #409 · approved · check…"),
    ];
    for (name, words) in want {
        assert_eq!(r[at(name) + 1], words, "{}", show(&r));
    }
    assert!(r[at("main :*") + 1].contains("cookies"), "{}", show(&r));
    let dark = at("sb/dark-mode");
    assert_eq!(r[dark], "  ψ sb/dark-mode ↑ #412", "{}", show(&r));
    assert_eq!(r[dark + 1], "  changes asked · checks pass");
    assert!(r[dark - 1].is_empty() && r[dark + 2].starts_with("  1 ∿ dark-mode"), "{}", show(&r));
    // no PR: the title alone, the hub's lid under it
    let mut none = mock();
    none.sb.places[0].pr = None;
    none.sb.places[0].lid = Some("no PR yet · 2 commits".into());
    hold(&mut none);
    let nr = rows(&none, panel_w(150), 32);
    let d = nr.iter().position(|l| l.contains("sb/dark-mode")).unwrap();
    assert_eq!((nr[d].as_str(), nr[d + 1].as_str()), ("  ψ sb/dark-mode", "  no PR yet · 2 commits"), "{}", show(&nr));
    // red only on `checks fail`
    let (buf, _) = buffer(&app, panel_w(150), 32);
    let y = at("#415") as u16;
    let red: String = (0..=panel_w(150)).filter(|x| buf[(*x, y)].fg == error()).map(|x| buf[(x, y)].symbol().to_string()).collect();
    assert_eq!(red.trim(), "checks fail");
    // stale: the words say how old, the ↑ faint
    app.sb.places[5].pr.as_mut().unwrap().stale_ms = Some(12 * 60_000);
    let words = app.sb.places[5].words_line(Some("docs"), 60).unwrap();
    let t: String = words.spans.iter().map(|s| s.content.to_string()).collect();
    assert_eq!(t, "     #409 · approved · checks pass · state from 12m ago");
    assert_eq!(app.sb.places[5].row_mark(true).style.fg, Some(faint()));
    // the header: `↑ 4 PRs` with the counts, the flow after the folder
    app.sb.workspace = "/w/acme".into();
    let text = |sb: &crate::sb::Sb, words: bool| -> (String, String) {
        let (l, r) = sb.edge(200, words, |room| sb.summary(room, false, words, &[]));
        (l.iter().map(|s| s.content.to_string()).collect(), r.iter().map(|s| s.content.to_string()).collect())
    };
    let (l, r) = text(&app.sb, true);
    assert_eq!(l, " · /w/acme · lands via PRs");
    assert!(r.contains(&format!("{} 4 PRs · # 1 in the inbox", G_PR)), "{r}");
    let rest = mock();
    let (l, r) = text(&rest.sb, false);
    assert!(!r.contains("PR") && !l.contains("lands"), "{l} {r}");
}

/// The 24-column panel (95 wide): the same blocks; a row drops its time
/// first, then its %, the mark column never; the name is cut before it
/// touches the mark; the words line cut with `…`; a section's branch
/// cut first, its mark (held, `↑ #412`) stays. The widest panel (44):
/// the whole branch.
#[test]
fn at_24_and_44_columns() {
    let mut app = mock();
    app.sb.places[0].branch = Some("sb/dark-mode-everywhere".into());
    app.sb.agents[3].name = "login-fix-everywhere".into();
    app.sb.places[1].agents[0] = "login-fix-everywhere".into();
    let narrow = 24;
    let r = rows(&app, narrow, 24);
    assert!(r.iter().all(|l| l.chars().count() <= narrow as usize + 1), "{}", show(&r));
    // emoji-csv's time goes (its % is blank here), the mark stays in its column
    assert!(r.iter().any(|l| l == "  5 ∿ emoji-csv        ψ"), "{}", show(&r));
    let login = r.iter().find(|l| l.contains("login-fix")).unwrap();
    assert_eq!(login, "  3 ∿ login-fix-every… ↑", "{}", show(&r));
    let dark = r.iter().find(|l| l.contains("sb/dark")).unwrap();
    assert_eq!(dark, "  ψ sb/dark-mode-ever… ↑", "{}", show(&r));
    hold(&mut app);
    let r = rows(&app, narrow, 30);
    let at = |s: &str| r.iter().position(|l| l.contains(s)).unwrap_or_else(|| panic!("{s}:\n{}", show(&r)));
    // held, the state word stays: the name is cut, the mark stays
    assert_eq!(r[at("5 ∿ emoji")], "  5 ∿ emoji-… working  ψ", "{}", show(&r));
    assert_eq!(r[at("5 ∿ emoji") + 1], "      no PR yet · 2 com…", "{}", show(&r));
    // a branch that isn't the agent's name comes last: the cut eats it
    assert_eq!(r[at("3 ∿ login") + 1], "      #415 · checks fail", "{}", show(&r));
    assert_eq!(r[at("7 … release") + 1], "      waits to land · 2…", "{}", show(&r));
    let words = app.sb.places[1].words_line(Some("login-fix-everywhere"), 60).unwrap();
    let t: String = words.spans.iter().map(|s| s.content.to_string()).collect();
    assert_eq!(t, "     #415 · checks fail: e2e/login · ψ sb/login-fix");
    assert_eq!(r[at("sb/dark")], "  ψ sb/dark-mode… ↑ #412", "{}", show(&r));
    assert_eq!(r[at("sb/dark") + 1], "  changes asked · check…", "{}", show(&r));
    let wide = panel_w(400);
    let r = rows(&app, wide, 30);
    let dark = r.iter().position(|l| l.contains("sb/dark")).unwrap();
    assert_eq!(r[dark], "  ψ sb/dark-mode-everywhere ↑ #412", "{}", show(&r));
}

/// A section never splits across the scroll: it goes under `+ n more`
/// whole; a selected agent in a section brings its whole section into
/// view.
#[test]
fn a_section_never_splits() {
    let mut app = mock();
    for h in 6..24 {
        for sel in [None, Some(0), Some(4), Some(7), Some(8)] {
            app.sb.selected = sel;
            let r = rows(&app, panel_w(150), h);
            let has = |s: &str| r.iter().any(|l| l.contains(s));
            let (title, first, last) = (has("sb/dark-mode"), has("1 ∿ dark-mode"), has("4 ∿ i18n"));
            assert!(title == first && first == last, "h {h} sel {sel:?}: split\n{}", show(&r));
        }
    }
    // the selection on i18n: its section whole on screen
    app.sb.selected = Some(8);
    let r = rows(&app, panel_w(150), 12);
    let y = r.iter().position(|l| l.contains("sb/dark-mode")).unwrap_or_else(|| panic!("{}", show(&r)));
    assert!(r[y + 2].starts_with("  4 ∿ i18n"), "{}", show(&r));
}

/// `NO_COLOR`: the red and the accent ↑ are bold, the others plain,
/// `checks fail` bold; `BISE_ASCII=1`: ↑ is `P`, never `^`, ψ and … the
/// table's fallbacks.
#[test]
fn no_color_and_ascii() {
    let app = mock();
    let mark = |k: usize, asks: bool| app.sb.places[k].row_mark(asks);
    crate::theme::set_ascii_for_tests(true);
    let ascii: Vec<String> = [mark(1, false), mark(2, false), mark(4, false)].iter().map(|s| s.content.to_string()).collect();
    let border: String = app.sb.places[0].title(31, true).spans.iter().map(|s| s.content.to_string()).collect();
    crate::theme::set_ascii_for_tests(false);
    assert_eq!(ascii[0], "P");
    assert!(ascii[1].is_ascii() && !ascii[1].is_empty() && ascii[2].is_ascii() && !ascii[2].is_empty(), "{ascii:?}");
    assert!(border.contains("P #412") && !border.contains('^') && !border.contains('↑'), "{border}");
    let was = std::env::var_os("NO_COLOR");
    std::env::set_var("NO_COLOR", "1");
    let bold = |s: Span| s.style.add_modifier.contains(Modifier::BOLD);
    let (fail, asks, draft, open) = (bold(mark(1, false)), bold(mark(5, true)), bold(mark(3, false)), bold(mark(5, false)));
    let words = app.sb.places[1].words_line(Some("login-fix"), 40).unwrap();
    match was {
        Some(v) => std::env::set_var("NO_COLOR", v),
        None => std::env::remove_var("NO_COLOR"),
    }
    assert!(fail && asks && !draft && !open);
    let cf = words.spans.iter().find(|s| s.content == "checks fail").unwrap();
    assert!(cf.style.add_modifier.contains(Modifier::BOLD) && cf.style.fg != Some(error()));
}

/// The divider of an agent in a worktree (pr-design §4): `ψ branch with
/// i18n · ↑ #412`, the number a link; ctrl held, the PR's words after
/// the number.
#[test]
fn the_divider_says_the_branch_and_the_pr() {
    let mut app = mock();
    app.sb.focus = "dark-mode".into();
    let who = panel::viewed_who(&app);
    assert_eq!(who.place.as_deref(), Some("sb/dark-mode"));
    assert_eq!(who.with, ["i18n"]);
    let pr = who.pr.as_ref().unwrap();
    assert_eq!((pr.number, pr.url.as_str()), (412, "https://github.com/acme/web/pull/412"));
    assert!(pr.words.is_empty());
    hold(&mut app);
    let held = panel::viewed_who(&app);
    let words: String = held.pr.unwrap().words.iter().map(|s| s.content.to_string()).collect();
    assert_eq!(words, "changes asked · checks pass");
    // an agent alone in its worktree: no `with`; in your folder: nothing
    app.sb.focus = "login-fix".into();
    assert!(panel::viewed_who(&app).with.is_empty());
    app.sb.focus = "cookies".into();
    let who = panel::viewed_who(&app);
    assert!(who.place.is_none() && who.pr.is_none());
}
