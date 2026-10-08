//! `/release-bise` in the TUI (BISE-235): offered only when the hub's
//! workspace is bise's source tree (the `dev` of its `versions` event;
//! the hub refuses it anywhere else too). The hub's plan shows in the
//! feed in view with a y/n; `y` asks the hub to run it; its steps and
//! result go to main's feed, and `∿ releasing v… · 12m` to the header.

use super::*;
use crate::commands::{Arg, Cmd};
use crate::release_row::Row;

/// The commands of the dev build only: never in the popup or `/help`
/// of another workspace or an installed bise.
pub(crate) const DEV_COMMANDS: &[Cmd] = &[Cmd {
    name: "/release-bise",
    desc: "release bise: tag HEAD, CI builds it, publish (asks first): /release-bise [dry-run]",
    args: &[Arg::Words(&[("dry-run", "say what it would do, push and publish nothing")])],
    client: true,
}];

/// The dev build: the hub said its workspace is bise's source tree.
pub(crate) fn dev(app: &App) -> bool {
    app.sb.versions_dev == Some(true)
}

/// The dev commands when this is the dev build, else none (`/log` is
/// not one: shipped to everyone, logview.rs).
pub(crate) fn dev_commands(app: &App) -> &'static [Cmd] {
    if dev(app) {
        DEV_COMMANDS
    } else {
        &[]
    }
}

/// The plan waiting for your y/n.
#[derive(Clone, Debug)]
pub(super) struct Ask {
    tag: String,
    commit: String,
    dry: bool,
}

/// `/release-bise [dry-run]`: ask the hub for the plan (it refuses
/// outside the dev build).
pub(super) fn command(sb: &mut Sb, typed: &str) {
    let dry = matches!(typed.split_whitespace().nth(1), Some("dry-run" | "--dry-run" | "dry"));
    sb.send(json!({"op": "release", "do": "plan", "dry": dry}));
}

/// Your line while a plan waits: `y` runs it, anything else cancels it
/// (a later `y` never releases by surprise). True when the line was the
/// answer (it goes nowhere else).
pub(super) fn answer(app: &mut App, typed: &str) -> bool {
    let Some(ask) = app.sb.release_ask.take() else {
        return false;
    };
    let t = typed.trim().to_lowercase();
    if matches!(t.as_str(), "y" | "yes" | "o" | "oui") {
        app.sb.send(json!({"op": "release", "do": "run", "tag": ask.tag, "commit": ask.commit, "dry": ask.dry}));
        return true;
    }
    push_event(&mut app.events, &mut app.cache, Ev::Info(format!("release {} cancelled", ask.tag)));
    matches!(t.as_str(), "n" | "no" | "non")
}

fn row_of(v: &Value) -> Option<Row> {
    let s = |k: &str| str_of(v, k);
    Some(match s("state").as_str() {
        "plan" => Row::Plan {
            tag: s("tag"),
            short: s("short"),
            subject: s("subject"),
            since: s("since"),
            count: v.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            commits: v
                .get("commits")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .map(|c| {
                            let f = |i: usize| c.get(i).and_then(|x| x.as_str()).unwrap_or("").to_string();
                            (f(0), f(1))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            dry: v.get("dry").and_then(|x| x.as_bool()).unwrap_or(false),
        },
        "step" => Row::Step(s("text")),
        "running" => Row::Running(s("text")),
        "done" => Row::Done(s("text")),
        "failed" => Row::Failed {
            text: s("text"),
            tail: v
                .get("tail")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            open: false,
        },
        "error" => Row::Failed { text: format!("no release · {}", s("text")), tail: Vec::new(), open: false },
        _ => return None,
    })
}

/// A `release` event of the hub.
pub(super) fn event(app: &mut App, v: &Value) {
    let Some(row) = row_of(v) else { return };
    let s = |k: &str| str_of(v, k);
    let elapsed = v.get("elapsed").and_then(|x| x.as_u64()).unwrap_or(0);
    match &row {
        Row::Plan { tag, dry, .. } => {
            // the plan and its question where you typed the command
            let q = if *dry {
                format!("dry run of {}: nothing is pushed or published. go?", tag)
            } else {
                format!("push tag {} and publish it? everyone on bise gets it at their next start", tag)
            };
            for ev in [Ev::Release(row.clone()), Ev::Warn(q), Ev::Info("y yes · n no, then ⏎".into())] {
                push_event(&mut app.events, &mut app.cache, ev);
            }
            let sb = &mut app.sb;
            sb.release_ask = Some(Ask { tag: tag.clone(), commit: s("commit"), dry: *dry });
            sb.calls += 1;
            return;
        }
        Row::Step(_) | Row::Running(_) => {
            let since = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(elapsed));
            app.sb.release = Some((s("tag"), since.unwrap_or_else(std::time::Instant::now)));
        }
        Row::Done(_) | Row::Failed { .. } | Row::Warned { .. } => {
            app.sb.release = None;
            app.sb.calls += 1;
        }
    }
    // a plan that failed shows where you asked; a run's rows in main's feed
    if s("state") == "error" {
        push_event(&mut app.events, &mut app.cache, Ev::Release(row));
        return;
    }
    feed::with_feed(app, "main", |app| {
        push_event(&mut app.events, &mut app.cache, Ev::Release(row));
    });
}

/// An `update` event of the hub (dev-update: `/update` in bise's source
/// tree builds HEAD): `building` puts `∿ building <sha> · 2m` in the
/// header; `built` clears it (the switch says the rest in main's
/// thread); `failed` clears it and puts `▲ couldn't build …` in main's
/// feed, the build's last lines folded under it.
pub(super) fn update_event(app: &mut App, v: &Value) {
    let s = |k: &str| str_of(v, k);
    match s("state").as_str() {
        "building" => {
            let elapsed = v.get("elapsed").and_then(|x| x.as_u64()).unwrap_or(0);
            let since = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(elapsed));
            app.sb.updating = Some((s("rev"), since.unwrap_or_else(std::time::Instant::now)));
        }
        "built" => {
            app.sb.updating = None;
            app.sb.calls += 1;
        }
        "failed" => {
            app.sb.updating = None;
            app.sb.calls += 1;
            let tail = v
                .get("tail")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let row = Row::Warned { text: s("text"), tail, open: false };
            feed::with_feed(app, "main", |app| {
                push_event(&mut app.events, &mut app.cache, Ev::Release(row));
            });
        }
        _ => {}
    }
}

/// `12s`, `12m`, `1h 5m`.
fn took(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{}s", s),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

/// The header's item while a release runs: the gust, then `releasing
/// v2026.10.3 · 12m`; else while `/update` builds (dev-update): the
/// gust, then `building 1152b33 · 2m` (empty when neither runs).
pub(super) fn header_item(sb: &Sb, gust: &[Span<'static>]) -> Vec<Span<'static>> {
    let text = match (&sb.release, &sb.updating) {
        (Some((tag, since)), _) => format!(" releasing {} · {}", tag, took(since.elapsed().as_secs())),
        (None, Some((rev, since))) => format!(" building {} · {}", rev, took(since.elapsed().as_secs())),
        (None, None) => return Vec::new(),
    };
    let mut out: Vec<Span<'static>> = gust.to_vec();
    out.push(Span::styled(text, Style::default().fg(dim())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sb::bench::{set_versions_dev, test_app};
    use std::io::Read;

    /// A test app and the hub's end of its socket.
    fn app_and_hub() -> (App, UnixStream) {
        let (a, b) = UnixStream::pair().unwrap();
        let (tx, rx) = mpsc::channel::<String>();
        std::mem::forget(tx);
        let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
        (sb_app(sb, rx, false, 100, crate::voice::Voice::live(false)), b)
    }

    /// What the TUI sent to the hub so far.
    fn sent(hub: &mut UnixStream) -> Vec<Value> {
        hub.set_nonblocking(true).unwrap();
        let mut buf = String::new();
        let _ = hub.read_to_string(&mut buf);
        buf.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    fn feed_text(app: &App) -> Vec<String> {
        app.events
            .iter()
            .flat_map(|e| crate::render::ev_lines(e, 120))
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>())
            .collect()
    }

    #[test]
    fn only_the_dev_build_offers_it() {
        let mut app = test_app();
        assert!(dev_commands(&app).is_empty(), "before the hub says");
        set_versions_dev(&mut app, false);
        assert!(dev_commands(&app).is_empty());
        set_versions_dev(&mut app, true);
        assert_eq!(dev_commands(&app)[0].name, "/release-bise");
        assert!(!dev_commands(&app).iter().any(|c| c.name == "/log"), "/log is shipped, not a dev command");
    }

    /// `/log` is shipped: an installed bise (not the dev build, no
    /// `BISE_DEV`) offers it in the popup.
    #[test]
    fn a_release_build_offers_log() {
        let mut app = test_app();
        set_versions_dev(&mut app, false);
        if std::env::var("BISE_DEV").is_ok_and(|v| !v.is_empty() && v != "0") {
            return; // the env says dev: this test proves nothing here
        }
        assert!(crate::logview::enabled(&app));
        app.ed.text = "/lo".into();
        assert!(crate::commands::popup_items(&app).iter().any(|i| i.label == "/log"));
    }

    fn plan(dry: bool) -> String {
        json!({"ev": "release", "state": "plan", "tag": "v2026.10.3", "commit": "abcdef1234", "short": "abcdef1",
            "subject": "fix the voice chip", "since": "v2026.10.1", "count": 2,
            "commits": [["abcdef1", "fix the voice chip"], ["1234567", "more room"]], "dry": dry})
        .to_string()
    }

    #[test]
    fn a_plan_asks_then_y_runs_it_and_the_steps_land_in_mains_feed() {
        let (mut app, mut hub) = app_and_hub();
        set_versions_dev(&mut app, true);
        dispatch(&mut app, &plan(true));
        let t = feed_text(&app);
        assert!(t.iter().any(|l| l == " · release v2026.10.3 · abcdef1 fix the voice chip · 2 commits since v2026.10.1 · dry run"), "{t:#?}");
        assert!(t.iter().any(|l| l.contains("dry run of v2026.10.3: nothing is pushed or published. go?")), "{t:#?}");
        assert!(answer(&mut app, "y"));
        assert!(app.sb.release_ask.is_none());
        let sent = sent(&mut hub);
        assert!(sent.iter().any(|v| v["op"] == "release" && v["do"] == "run" && v["tag"] == "v2026.10.3" && v["commit"] == "abcdef1234" && v["dry"] == true), "{sent:?}");
        let ev = |state: &str, text: &str| json!({"ev": "release", "state": state, "tag": "v2026.10.3", "text": text, "elapsed": 90}).to_string();
        dispatch(&mut app, &ev("running", "CI building · 0s"));
        assert!(app.sb.release.is_some());
        let head: String = header_item(&app.sb, &[]).iter().map(|s| s.content.to_string()).collect();
        assert_eq!(head, " releasing v2026.10.3 · 1m");
        dispatch(&mut app, &ev("running", "CI building · 1m"));
        dispatch(&mut app, &ev("step", "draft checked"));
        dispatch(&mut app, &ev("done", "dry run of v2026.10.3 · nothing pushed, nothing published"));
        let t = feed_text(&app);
        assert!(!t.iter().any(|l| l.contains("CI building")), "the running row gave its place: {t:#?}");
        let at = t.iter().position(|l| l == " ✓ draft checked").expect("the step");
        assert!(t[at + 1..].iter().any(|l| l == " ✓ dry run of v2026.10.3 · nothing pushed, nothing published"), "{t:#?}");
        assert!(app.sb.release.is_none() && header_item(&app.sb, &[]).is_empty());
    }

    #[test]
    fn dev_update_shows_its_build_in_the_header_then_a_failure_in_mains_feed() {
        let (mut app, _hub) = app_and_hub();
        dispatch(&mut app, &json!({"ev": "update", "state": "building", "rev": "1152b33", "elapsed": 130}).to_string());
        let head: String = header_item(&app.sb, &[]).iter().map(|s| s.content.to_string()).collect();
        assert_eq!(head, " building 1152b33 · 2m");
        // a release running wins the header
        app.sb.release = Some(("v2026.10.3".into(), std::time::Instant::now()));
        let head: String = header_item(&app.sb, &[]).iter().map(|s| s.content.to_string()).collect();
        assert_eq!(head, " releasing v2026.10.3 · 0s");
        app.sb.release = None;
        let tail: Vec<String> = (1..=20).map(|i| format!("line {}", i)).collect();
        let mut app2 = app;
        dispatch(
            &mut app2,
            &json!({"ev": "update", "state": "failed", "rev": "1152b33", "tail": tail,
                "text": "couldn't build 1152b33, you're still on 9f0e2aa: error: could not compile `bise`"})
            .to_string(),
        );
        assert!(app2.sb.updating.is_none() && header_item(&app2.sb, &[]).is_empty());
        let t = feed_text(&app2);
        let at = t
            .iter()
            .position(|l| l == " ▲ couldn't build 1152b33, you're still on 9f0e2aa: error: could not compile `bise`")
            .unwrap_or_else(|| panic!("{t:#?}"));
        assert_eq!(t[at + 1], "   ▸ 20 more lines");
        // built: the header clears, the switch speaks in main's thread
        dispatch(&mut app2, &json!({"ev": "update", "state": "building", "rev": "1152b33"}).to_string());
        assert!(app2.sb.updating.is_some());
        dispatch(&mut app2, &json!({"ev": "update", "state": "built", "rev": "1152b33"}).to_string());
        assert!(app2.sb.updating.is_none());
    }

    #[test]
    fn anything_but_y_cancels_the_plan() {
        let (mut app, mut hub) = app_and_hub();
        dispatch(&mut app, &plan(false));
        assert!(feed_text(&app).iter().any(|l| l.contains("push tag v2026.10.3 and publish it?")));
        assert!(!answer(&mut app, "what does it do?"), "the line still goes to main");
        assert!(app.sb.release_ask.is_none());
        assert!(!answer(&mut app, "y"), "nothing to answer any more");
        assert!(!sent(&mut hub).iter().any(|v| v["do"] == "run"));
    }
}
