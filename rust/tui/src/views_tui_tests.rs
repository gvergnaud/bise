use super::*;
use crate::wire::{ToolData, ToolState};
use std::time::Duration;

fn chip(call: &str, expired: bool) -> ViewChip {
    ViewChip::test(call, expired)
}

fn text(l: &Line) -> String {
    l.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn a_live_view_is_its_line_with_two_links() {
    let ls = chip_lines(&[chip("1700-3", false)], 80);
    assert_eq!(ls.len(), 1);
    let t = text(&ls[0]);
    assert!(t.starts_with(&format!("  {} counter · fake-app", glyph(G_VIEW))), "{t}");
    // the link spans carry their labels (textlayer tags them in the frame)
    assert!(t.contains(views::tui_app) && t.contains(views::tui_browser), "{t}");
    assert!(ls[0].spans.iter().all(|s| s.style.fg == Some(dim())), "all dim");
}

#[test]
fn an_expired_view_is_faint_with_no_link() {
    let ls = chip_lines(&[chip("1700-3", true), chip("1700-4", false)], 80);
    assert_eq!(ls.len(), 2, "one line per view, in line order");
    let t = text(&ls[0]);
    assert_eq!(t, format!("  {} counter · fake-app · {}", glyph(G_VIEW), views::tui_expired));
    assert!(!t.contains(views::tui_app) && ls[0].spans.iter().all(|s| s.style.fg == Some(faint())));
}

#[test]
fn a_chip_link_names_how_and_its_call() {
    assert_eq!(of_url("bise-view:app:1700-3"), Some((How::App, "1700-3".into())));
    assert_eq!(of_url("bise-view:web:1700-3"), Some((How::Web, "1700-3".into())));
    for bad in ["bise-view:app:", "bise-view:app:../x", "bise-view:web:1700", "bise-view:web:1-2-3", "bise-view:x:1-2", "https://a.b/1-2", "bise-view:app:1700-3/x"] {
        assert_eq!(of_url(bad), None, "{bad}");
    }
    assert_eq!(web_url("http://127.0.0.1:47123/", "t1", "1700-3"), "http://127.0.0.1:47123/v/t1/1700-3");
}

#[test]
fn app_open_says_how_many_apps_got_it() {
    assert_eq!(delivered(&json!({"delivered": 2})), 2);
    assert_eq!(delivered(&json!({})), 0);
    assert_eq!(delivered(&Value::Null), 0);
}

fn tool(id: u32, views: Vec<ViewChip>) -> Ev {
    let mut t = ToolData::bare(id, ToolState::Ok);
    t.views = views;
    Ev::Tool(t)
}

#[test]
fn views_lists_the_threads_views_newest_first_and_skips_expired() {
    let evs = vec![tool(1, vec![chip("1-1", false)]), Ev::Info("x".into()), tool(2, vec![chip("1-2", true), chip("1-3", false)]), tool(3, vec![])];
    let rs = rows(&evs, 0);
    let calls: Vec<&str> = rs.iter().map(|r| r.view.call.as_str()).collect();
    assert_eq!(calls, ["1-3", "1-2", "1-1"]);
    assert_eq!(first(&rs), 0);
    assert_eq!(step(&rs, 0, true), 2, "↓ skips the expired row");
    assert_eq!(step(&rs, 2, false), 0, "↑ too");
    assert_eq!(step(&rs, 2, true), 2, "the end stays");
    let only_expired = rows(&[tool(1, vec![chip("1-1", true)])], 0);
    assert_eq!(first(&only_expired), 0);
}

#[test]
fn a_view_from_the_history_keeps_the_age_of_its_call() {
    // the thread loaded now (its ToolData built this instant), its calls
    // made 130 s and 2 h before: the ages are the calls', never 0 s
    let now = 1_700_000_000_000u64;
    let evs = vec![tool(1, vec![chip(&format!("{}-1", now - 7_200_000), false)]), tool(2, vec![chip(&format!("{}-2", now - 130_000), false)])];
    let rs = rows(&evs, now);
    assert_eq!(rs.iter().map(|r| r.ago).collect::<Vec<_>>(), [Duration::from_secs(130), Duration::from_secs(7200)]);
    assert_eq!(text(&lines(&rs, 0)[2]), format!("› {} counter · fake-app · 2 min ago", glyph(G_VIEW)));
    assert_eq!(crate::wire::call_ms("1700000000000-3"), 1_700_000_000_000);
    assert_eq!(crate::wire::call_ms("x-3"), 0);
}

#[test]
fn the_screen_says_its_rows_keys_and_empty() {
    let empty = lines(&[], 0);
    assert_eq!(text(&empty[0]), views::tui_list_title);
    assert_eq!(text(&empty[2]), views::tui_list_empty);
    let rs = vec![Row { view: chip("1-3", false), ago: Duration::from_secs(130) }, Row { view: chip("1-2", true), ago: Duration::from_secs(4000) }];
    let ls = lines(&rs, 0);
    assert_eq!(text(&ls[2]), format!("› {} counter · fake-app · 2 min ago", glyph(G_VIEW)));
    assert_eq!(text(&ls[3]), format!("  {} counter · fake-app · 1 h ago", glyph(G_VIEW)));
    assert_eq!(ls[3].spans[1].style.fg, Some(faint()), "expired: faint");
    assert_eq!(text(ls.last().unwrap()), views::tui_list_keys);
}

#[test]
fn ages_read_short() {
    assert_eq!(age_words(Duration::from_secs(40)), "40s");
    assert_eq!(age_words(Duration::from_secs(120)), "2 min");
    assert_eq!(age_words(Duration::from_secs(7200)), "2 h");
    assert_eq!(age_words(Duration::from_secs(3 * 86_400)), "3 d");
}

/// A program's views: run_typescript is a box (its row closed, or the box
/// opened), never tool_lines, and every view comes from a program (a
/// tmux capture showed none, sandbox's captures for designer).
#[test]
fn a_programs_box_draws_its_views_under_it() {
    for opened in [false, true] {
        let mut t = ToolData::bare(2, ToolState::Ok);
        t.name = Some("run_typescript".into());
        t.intent = Some("show two counters".into());
        t.opened = opened;
        let bare = crate::feed::build_rows(&[crate::wire::Ev::Tool(t.clone())], 0, false, 80, 0);
        t.views = vec![chip("1-1", false), chip("1-2", false)];
        let rows = crate::feed::build_rows(&[crate::wire::Ev::Tool(t)], 0, false, 80, 0);
        assert_eq!(rows.len(), bare.len() + 2, "opened {opened}");
        let chips: Vec<String> = rows[bare.len()..].iter().map(text).collect();
        assert!(chips.iter().all(|c| c.contains("counter · fake-app") && c.contains(views::tui_app)), "opened {opened}: {chips:?}");
    }
}

/// The `⊞` lines come right under the tool row, in every form of it.
#[test]
fn the_tool_row_draws_its_views_under_it() {
    let mut t = ToolData::bare(7, ToolState::Ok);
    t.name = Some("show_counter".into());
    let bare = crate::render::tool_lines(&t, 0, 80).len();
    t.views = vec![chip("1-1", false), chip("1-2", true)];
    let ls = crate::render::tool_lines(&t, 0, 80);
    assert_eq!(ls.len(), bare + 2);
    assert!(text(&ls[bare]).contains("counter · fake-app") && text(&ls[bare]).contains(views::tui_app));
    assert!(text(&ls[bare + 1]).ends_with(views::tui_expired));
}

#[test]
fn an_artifacts_web_page_is_the_hubs_d_route() {
    // the TUI's `O` on an artifact in /artifacts (docs/plugin-apps.md A7): its /d/<id> page
    assert_eq!(doc_web_url("http://127.0.0.1:4100", "q3-budget"), "http://127.0.0.1:4100/d/q3-budget");
    assert_eq!(doc_web_url("http://127.0.0.1:4100/", "q3-budget"), "http://127.0.0.1:4100/d/q3-budget");
}
