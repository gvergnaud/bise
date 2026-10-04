use super::*;
use crate::artifacts::{Artifact, Version};

// 2025-09-30 12:41:00 UTC, a tuesday; the clock in UTC
const T: u64 = 1_759_236_060_000;
const MIN: u64 = 60_000;
const DAY: u64 = 24 * 60 * MIN;

fn utc(_: u64) -> i32 {
    0
}

fn art(id: &str, title: &str, kind: &str, agent: &str, ago: u64) -> Artifact {
    Artifact { id: id.into(), title: title.into(), kind: kind.into(), agent: agent.into(), ts_ms: T - ago, v: 1, target: format!("/w/{id}"), ..Default::default() }
}

/// The afternoon of the page: pricing-page's page v3 and sheet, the
/// deck, the site, the PR, yesterday's doc, the archived design doc.
pub(crate) fn afternoon() -> Vec<Artifact> {
    let page = Artifact {
        v: 3,
        target: "http://127.0.0.1:47438/p/pricing-page".into(),
        detail: "2 notes open".into(),
        versions: vec![
            Version { v: 1, ts_ms: T - 60 * MIN, target: "x".into(), copy: None, note: String::new() },
            Version { v: 2, ts_ms: T - 40 * MIN, target: "x".into(), copy: None, note: "3 notes done".into() },
            Version { v: 3, ts_ms: T - 12 * MIN, target: "x".into(), copy: None, note: "2 notes open".into() },
        ],
        ..art("pricing-page", "pricing page", "page", "pricing-page", 12 * MIN)
    };
    let pr = Artifact {
        detail: "open".into(),
        target: "https://github.com/acme/web/pull/6".into(),
        pr: Some(crate::artifacts::Pr { repo: "acme/web".into(), number: 6, branch: "gateway".into() }),
        ..art("pr6", "PR #6 · gateway head checks", "pr", "pr-review", 5 * 60 * MIN)
    };
    let subs = Artifact {
        archived: true,
        gone: true,
        copy: Some("/store/subs.md".into()),
        ..art("subs", "subscriptions design", "doc", "subs-lead", 2 * DAY)
    };
    vec![
        page,
        art("plans", "pricing-plans.xlsx", "sheet", "pricing-page", 14 * MIN),
        Artifact { v: 5, ..art("everyone", "bise for everyone", "page", "ambient-pm", 60 * MIN) },
        art("deck", "onboarding deck", "slides", "launch", 3 * 60 * MIN),
        Artifact { detail: "127.0.0.1:4747".into(), ..art("capsule", "capsule, round 9", "site", "ambient", 4 * 60 * MIN) },
        pr,
        art("providers", "docs: providers", "doc", "docs-site", 18 * 60 * MIN),
        subs,
        art("investor", "investor deck, draft", "slides", "designer", 2 * DAY + 60 * MIN),
    ]
}

fn render(sc: &mut Screen, all: &[Artifact], w: usize, h: usize) -> String {
    let clock = Clock { now: T, off: &utc };
    let (lines, _, _) = lines(sc, all, w, h, &clock);
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
}

fn screen() -> Screen {
    Screen { agent: "main".into(), ..Default::default() }
}

#[test]
fn the_list_at_150_has_every_column_and_the_key_bar_as_drawn() {
    let all = afternoon();
    let mut sc = screen();
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("artifacts · what your agents made"), "{out}");
    assert!(out.contains("all agents · 9   tab this agent"), "{out}");
    assert!(out.contains("/ find: a title, an agent, a kind"), "{out}");
    assert!(out.contains("  today\n› pricing page"), "{out}");
    assert!(out.contains("pricing page                      page      pricing-page            12 min     v3 · 2 notes open"), "{out}");
    assert!(out.contains("yesterday"), "{out}");
    assert!(out.contains("this week"), "{out}");
    assert!(out.contains("subs-lead · archived    sun        ▲ gone from disk · bise kept a copy"), "{out}");
    assert!(out.contains("pricing page · page · v3 · by pricing-page, 12 min ago · 127.0.0.1:47438/p/pricing-page"), "{out}");
    assert!(out.ends_with("⏎ open   space quick look   v versions   c copy   @ put it in a message   esc close"), "{out}");
}

#[test]
fn the_list_at_80_drops_columns() {
    let all = afternoon();
    let mut sc = screen();
    let out = render(&mut sc, &all, 74, 24);
    assert!(out.contains("artifacts") && !out.contains("what your agents made"), "{out}");
    assert!(out.contains("all agents · 9") && !out.contains("tab this agent"), "{out}");
    assert!(out.contains("› pricing page") && out.contains("12m") && out.contains("v3"), "{out}");
    assert!(out.ends_with("⏎ open   space look   v versions   esc close"), "{out}");
    for l in out.lines() {
        assert!(unicode_width::UnicodeWidthStr::width(l) <= 74, "too wide: {l}");
    }
}

#[test]
fn slash_searches_and_enter_gives_the_keys_back() {
    let all = afternoon();
    let mut sc = screen();
    sc.typing = true;
    sc.query = "deck".into();
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("/ deck▏   2 of 9"), "{out}");
    assert!(out.contains("› onboarding deck") && out.contains("investor deck, draft") && !out.contains("pricing page "), "{out}");
    assert!(out.ends_with("⏎ done   ↑↓ choose   esc clear the search"), "{out}");
    sc.typing = false;
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("/ deck   2 of 9"), "{out}");
    assert!(out.ends_with("@ put it in a message   esc clear the search"), "{out}");
    sc.query = "zzz".into();
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("nothing matches. esc clears the search."), "{out}");
}

#[test]
fn v_opens_the_versions_under_the_row() {
    let all = afternoon();
    let mut sc = Screen { sel: Some("pricing-page".into()), ..screen() };
    sc.versions = Some(1);
    let out = render(&mut sc, &all, 144, 26);
    assert!(out.contains("╭─ pricing page · 3 versions"), "{out}");
    assert!(out.contains("│   v3   12 min ago   2 notes open   the current one"), "{out}");
    assert!(out.contains("│ › v2   40 min ago   3 notes done"), "{out}");
    assert!(out.contains("│ ⏎ open this one   esc back"), "{out}");
    assert!(out.ends_with("⏎ open this version   ↑↓ choose   esc back to the list"), "{out}");
}

#[test]
fn the_empty_list_says_how_things_get_in() {
    let mut sc = screen();
    let out = render(&mut sc, &[], 144, 20);
    assert!(out.contains("nothing yet. when an agent makes a page, a doc or a file for you, it lands here."), "{out}");
    assert!(out.contains("agents add what they make with sb artifact add. bise pages come in by themselves."), "{out}");
    assert!(out.ends_with("esc close"), "{out}");
}

#[test]
fn tab_shows_the_agent_in_view_only() {
    let all = afternoon();
    let mut sc = Screen { agent: "pricing-page".into(), this_agent: true, ..Default::default() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("pricing-page · 2   tab all agents"), "{out}");
    assert!(!out.contains("onboarding deck"), "{out}");
}

#[test]
fn a_long_list_keeps_the_selection_in_view_and_says_how_many_more() {
    let mut all = afternoon();
    for i in 0..30 {
        all.push(art(&format!("old{i}"), &format!("old thing {i}"), "doc", "launch", 10 * DAY + i * MIN));
    }
    let mut sc = screen();
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("↓ ") && out.contains(" more"), "{out}");
    sc.sel = Some("old29".into());
    let out = render(&mut sc, &all, 144, 20);
    assert!(out.contains("› old thing 29"), "{out}");
}

#[test]
fn the_pr_row_offers_github_and_a_gone_file_says_the_copy_opens() {
    let all = afternoon();
    let mut sc = Screen { sel: Some("pr6".into()), ..screen() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("o GitHub"), "{out}");
    let mut sc = Screen { sel: Some("subs".into()), ..screen() };
    let out = render(&mut sc, &all, 144, 24);
    assert!(out.contains("subscriptions design · doc · by subs-lead (archived), sun 12:41 · its worktree is gone: ⏎ opens the copy bise kept"), "{out}");
    assert!(out.contains("r show in Finder"), "{out}");
}
