//! The `/version` picker: the versions the hub offers (its `versions`
//! event), filtered by what follows `/version ` in the composer.

use super::*;

/// One entry of the `/version` picker.
#[derive(Clone, Debug, Default)]
pub(crate) struct VersionItem {
    pub(super) rev: String,
    subject: String,
    pub(super) marks: Vec<String>,
}

pub(super) fn parse_versions(v: &Value) -> Vec<VersionItem> {
    let s = str_of;
    v.get("items")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| VersionItem {
                    rev: s(x, "rev"),
                    subject: s(x, "subject"),
                    marks: x
                        .get("marks")
                        .and_then(|m| m.as_array())
                        .map(|m| m.iter().filter_map(|y| y.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The items matching `q` (in the revision or the subject).
fn filter_versions<'a>(items: &'a [VersionItem], q: &str) -> Vec<&'a VersionItem> {
    let q = q.trim().to_lowercase();
    items
        .iter()
        .filter(|i| {
            q.is_empty() || i.rev.to_lowercase().contains(&q) || i.subject.to_lowercase().contains(&q)
        })
        .collect()
}

/// The marks as words, and the glyph of the most telling one.
fn version_marks(marks: &[String]) -> (String, (&'static str, Color)) {
    let has = |m: &str| marks.iter().any(|x| x == m);
    let glyph = if has("building") {
        ("…", theme::accent())
    } else if has("failed") && !has("current") {
        (theme::glyph(theme::G_FAILED), theme::error())
    } else if has("current") {
        ("◉", theme::accent())
    } else if has("good") {
        ("✓", theme::dim())
    } else if has("built") {
        ("●", theme::text())
    } else {
        (theme::glyph(theme::G_IDLE), theme::dim())
    };
    let words: Vec<&str> = marks
        .iter()
        .map(|m| match m.as_str() {
            "current" => "current",
            "trial" => "on trial",
            "good" => "last good",
            "built" => "built",
            "building" => "building…",
            "failed" => "failed",
            other => other,
        })
        .collect();
    (words.join(", "), glyph)
}

/// Whether the hub runs in bise's source tree (None: not known yet).
pub(crate) fn versions_dev(app: &App) -> Option<bool> {
    app.sb.versions_dev
}

/// The versions matching `q` (the `/version` and `/restart` arguments):
/// a note while the list loads. Asks the hub for a fresh list (at most
/// every 3 s while a popup shows them).
pub(crate) fn version_choices(app: &App, q: &str) -> Vec<Choice> {
    let sb = &app.sb;
    let stale = sb
        .versions_asked
        .get()
        .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(3));
    if stale {
        sb.versions_asked.set(Some(std::time::Instant::now()));
        if let Ok(mut w) = sb.writer.lock() {
            let _ = w.write_all(b"{\"op\":\"version\",\"do\":\"items\"}\n");
        }
    }
    if sb.versions.is_empty() {
        return vec![Choice { value: String::new(), label: "…".into(), desc: "loading the versions".into(), mark: None }];
    }
    filter_versions(&sb.versions, q)
        .into_iter()
        .map(|i| {
            let (words, glyph) = version_marks(&i.marks);
            Choice {
                value: i.rev.clone(),
                label: i.rev.clone(),
                desc: if words.is_empty() {
                    i.subject.clone()
                } else {
                    format!("[{}] {}", words, i.subject)
                },
                mark: Some(glyph),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(rev: &str, subject: &str, marks: &[&str]) -> VersionItem {
        VersionItem {
            rev: rev.into(),
            subject: subject.into(),
            marks: marks.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn the_filter_matches_the_revision_or_the_subject() {
        let items = vec![
            item("back", "roll back to 1234567", &[]),
            item("tree", "the working tree", &["current"]),
            item("abc1234", "tui: faster feed", &["built"]),
            item("def5678", "hub: journal", &["good", "built"]),
        ];
        let revs = |q: &str| -> Vec<String> {
            filter_versions(&items, q).iter().map(|i| i.rev.clone()).collect()
        };
        assert_eq!(revs("").len(), 4);
        assert_eq!(revs("abc"), vec!["abc1234"]);
        assert_eq!(revs("JOURNAL"), vec!["def5678"]);
        assert_eq!(revs("back"), vec!["back"]);
    }

    #[test]
    fn marks_read_as_words_and_one_glyph() {
        let (w, g) = version_marks(&["current".into(), "trial".into()]);
        assert_eq!(w, "current, on trial");
        assert_eq!(g.0, "◉");
        assert_eq!(version_marks(&["building".into()]).1 .0, "…");
        assert_eq!(version_marks(&["failed".into()]).1 .0, "✗");
        assert_eq!(version_marks(&[]).1 .0, "○");
    }

    /// tui-parity m_13350: '/version list' ⏎ with a version whose subject
    /// holds 'list' asks the hub for the list and switches to nothing
    /// (the popup's top row was that version, and ⏎ ran it); so do the
    /// other subcommand words.
    #[test]
    fn a_subcommand_word_typed_then_enter_runs_it_never_a_switch() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use std::io::Read;
        let ops = |line: &str| -> Vec<Value> {
            let (a, mut b) = UnixStream::pair().unwrap();
            b.set_nonblocking(true).unwrap();
            let (tx, rx) = std::sync::mpsc::channel::<String>();
            std::mem::forget(tx);
            let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
            let mut app = sb_app(sb, rx, false, 100, crate::voice::Voice::live(false));
            app.sb.versions = vec![
                item("abc1234", "tui: the version list view", &["built"]),
                item("def5678", "hub: rollback restart update words", &["built"]),
            ];
            for c in line.chars() {
                crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            let (mut s, mut buf) = (String::new(), [0u8; 4096]);
            while let Ok(n) = b.read(&mut buf) {
                if n == 0 {
                    break;
                }
                s.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            s.lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter(|v| v["op"] == "version" && v["do"] != "items")
                .collect()
        };
        for (line, want) in [
            ("/version list", "list"),
            ("/version back", "rollback"),
            ("/version rollback", "rollback"),
            ("/version restart", "restart"),
            ("/version update", "update"),
        ] {
            let got = ops(line);
            assert_eq!(got.len(), 1, "{line}: {got:?}");
            assert_eq!(got[0]["do"], want, "{line}: {got:?}");
            assert!(got[0].get("to").is_none_or(|t| t == ""), "{line}: no version picked: {got:?}");
        }
        // any other word still filters and ⏎ runs the top version
        assert_eq!(ops("/version view"), vec![json!({"op": "version", "do": "switch", "to": "abc1234"})]);
    }

    #[test]
    fn the_hub_event_parses() {
        let v = json!({"ev": "versions", "items": [
            {"rev": "abc1234", "subject": "s", "marks": ["built", "good"]}]});
        let items = parse_versions(&v);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].marks, vec!["built", "good"]);
    }
}
