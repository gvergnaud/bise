//! `/log`: the entries of a small log, the model view after a
//! compaction, search, filters, folds, redaction.
use super::model::{self, Body, Role};
use super::*;
use bise_session::read_bytes;

fn line(seq: u64, turn: Option<u64>, typ: &str, data: serde_json::Value) -> String {
    let mut v = serde_json::json!({"seq": seq, "at": format!("2026-10-02T14:02:{:02}.000Z", seq % 60), "type": typ, "v": 1, "data": data});
    if let Some(t) = turn {
        v["turn"] = t.into();
    }
    v.to_string()
}

/// A session: system prompt, a turn with thinking, a bash call, an
/// edit, an injected status, then a compaction and a second turn.
fn sample() -> Log {
    use serde_json::json;
    let text = |t: &str| json!([{"kind": "text", "text": t}]);
    let lines = [
        line(1, None, "session_start", json!({"session": "s-1", "format": 1, "created_by": "bise", "cwd": "/w"})),
        line(2, None, "context_set", json!({"system": {"text": "You are a precise assistant.\n## Tools\nbash"}, "tools": [{"name": "bash", "description": "run"}]})),
        line(3, Some(1), "turn_started", json!({"cause": "user"})),
        line(4, Some(1), "user_message", json!({"content": text("fix the **signup** test"), "delivery": "prompt"})),
        line(5, Some(1), "assistant_message", json!({"req": 1, "model": "m", "parts": [{"kind": "thinking", "text": "look at the test"}],
            "calls": [{"id": "call_1", "name": "bash", "args": "cargo test signup"},
                      {"id": "call_2", "name": "edit", "args": "{\"file_path\":\"src/a.rs\",\"old_string\":\"let a = 1;\",\"new_string\":\"let a = 2;\\nlet b = 3;\"}"}]})),
        line(6, Some(1), "usage", json!({"req": 1, "model": "m", "input": 8400, "output": 1100, "cache_read": 0})),
        line(7, Some(1), "tool_result", json!({"call": "call_1", "ok": false, "content": text("test failed: sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789"), "exit": 101})),
        line(8, Some(1), "tool_result", json!({"call": "call_2", "ok": true, "content": text("{\"ok\":true}")})),
        line(9, Some(1), "user_message", json!({"content": text("<task_status>\ndesigner idle</task_status>"), "delivery": "steer"})),
        line(10, Some(1), "assistant_message", json!({"req": 2, "model": "m", "parts": [{"kind": "text", "text": "Fixed: `a` is 2 now."}], "calls": []})),
        line(11, Some(1), "usage", json!({"req": 2, "model": "m", "input": 9000, "output": 50})),
        line(12, Some(1), "turn_ended", json!({"outcome": "done"})),
        line(13, None, "compaction_started", json!({"id": 1, "trigger": "user"})),
        line(14, None, "compaction_done", json!({"id": 1, "summary": text("<summary>signup fixed</summary>"), "replaces": {"from": 4, "to": 12}, "kept": []})),
        line(15, Some(2), "turn_started", json!({"cause": "user"})),
        line(16, Some(2), "user_message", json!({"content": text("thanks"), "delivery": "prompt"})),
        line(17, Some(2), "assistant_message", json!({"req": 3, "model": "m", "parts": [{"kind": "text", "text": "You're welcome."}], "calls": []})),
        line(18, Some(2), "usage", json!({"req": 3, "model": "m", "input": 1400, "output": 10})),
        line(19, Some(2), "turn_ended", json!({"outcome": "done"})),
    ];
    let bytes = lines.join("\n") + "\n";
    read_bytes(&[("events.jsonl".into(), bytes.into_bytes())])
}

fn view() -> View {
    View::of("main".into(), sample(), PathBuf::from("/nonexistent"), Redactor::default())
}

fn text(lines: &[Line]) -> String {
    lines.iter().map(crate::feedsel::line_text).map(|l| l.trim_end().to_string()).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_history_has_every_entry_with_its_role() {
    let v = view();
    let roles: Vec<String> = v
        .history
        .iter()
        .map(|i| match &i.rule {
            Some(r) => format!("rule {r}"),
            None => format!("{} {}", i.role.key(), i.tool),
        })
        .collect();
    assert_eq!(
        roles,
        [
            "event ",
            "system ",
            "system ",
            "rule turn 1 · 14:02 · user · 9.0k in · 1.1k out",
            "you ",
            "thinking ",
            "call bash",
            "call edit",
            "result bash",
            "result edit",
            "injected ",
            "assistant ",
            "rule compacted · 9.0k → 1.4k tokens",
            "summary ",
            "rule turn 2 · 14:02 · user · 1.4k in · 10 out",
            "you ",
            "assistant ",
        ]
    );
    let bash = &v.history[6];
    assert_eq!(bash.body, Body::Code { lang: "bash", text: "cargo test signup".into() });
    let edit = &v.history[7];
    assert_eq!(edit.head, "src/a.rs  +2 −1");
    assert_eq!(edit.body.text(), "--- src/a.rs\n+++ src/a.rs\n-let a = 1;\n+let a = 2;\n+let b = 3;\n");
    assert_eq!(v.history[8].role, Role::Result { ok: false });
    assert!(matches!(&v.history[9].body, Body::Code { lang: "json", text } if text.contains("\"ok\": true")));
    assert_eq!(v.history[10].head, "<task_status> (steer)");
    // a result's header is its output; the call id goes over the body
    assert_eq!(v.history[8].head, "test failed: «redacted:anthropic»");
    assert_eq!(v.history[8].meta, "call_1 failed · exit 101");
}

#[test]
fn a_key_never_shows() {
    let v = view();
    let r = &v.history[8];
    assert!(!r.body.text().contains("sk-ant-api03"), "{}", r.body.text());
    assert!(r.body.text().contains("«redacted:anthropic»"));
}

/// A compaction written before the summary opened the replacement: its
/// compaction_done holds the preamble, the summary comes after the kept
/// message. The compaction's row shows the real summary.
#[test]
fn an_old_compaction_shows_its_real_summary() {
    use serde_json::json;
    let text = |t: &str| json!([{"kind": "text", "text": t}]);
    let lines = [
        line(1, None, "session_start", json!({"session": "s-1", "format": 1, "created_by": "bise", "cwd": "/w"})),
        line(2, Some(1), "turn_started", json!({"cause": "user"})),
        line(3, Some(1), "user_message", json!({"content": text("fix signup"), "delivery": "prompt"})),
        line(4, Some(1), "assistant_message", json!({"req": 1, "model": "m", "parts": [{"kind": "text", "text": "done"}], "calls": []})),
        line(5, Some(1), "turn_ended", json!({"outcome": "done"})),
        line(6, None, "compaction_started", json!({"id": 1, "trigger": "user"})),
        line(7, None, "compaction_done", json!({"id": 1, "summary": text("The earlier conversation was compacted. A summary replaces it; continue from the preserved user messages."), "replaces": {"from": 3, "to": 4}, "kept": [3]})),
        line(8, None, "context_injected", json!({"kind": "summary", "content": text("Summary of the earlier conversation:\n<summary>signup fixed, tests green</summary>")})),
    ];
    let bytes = lines.join("\n") + "\n";
    let log = read_bytes(&[("events.jsonl".into(), bytes.into_bytes())]);
    let v = View::of("main".into(), log, PathBuf::from("/nonexistent"), Redactor::default());
    let row = v.history.iter().find(|i| i.role == Role::Summary).expect("a summary row");
    assert!(row.body.text().contains("signup fixed, tests green"), "{}", row.body.text());
    // a new compaction already carries its summary: shown as it is
    let s = view();
    let row = s.history.iter().find(|i| i.role == Role::Summary).expect("a summary row");
    assert!(row.body.text().contains("signup fixed"), "{}", row.body.text());
}

#[test]
fn what_the_model_got_after_a_compaction() {
    let mut v = view();
    assert_eq!(v.requests.iter().map(|r| r.req).collect::<Vec<_>>(), [1, 2, 3]);
    v.req = 2;
    v.set_mode(Mode::Model);
    let got: Vec<String> = v
        .model
        .iter()
        .map(|i| match &i.rule {
            Some(r) => format!("rule {r}"),
            None => i.role.key().to_string(),
        })
        .collect();
    assert_eq!(got, ["system", "system", "rule 6 entries before this were compacted (0 kept)", "summary", "you"]);
    // request 2: the whole first turn so far, no compaction yet
    v.step_request(-1);
    let got: Vec<&str> = v.model.iter().map(|i| i.role.key()).collect();
    assert_eq!(got, ["system", "system", "you", "thinking", "call", "call", "result", "result", "injected"]);
    let t = text(&lines(&mut v, 120, 30));
    assert!(t.contains("what the model got · request 2 of 3"), "{t}");
    assert!(t.contains("missing: each call's <bise_state>"), "{t}");
    assert!(t.contains("~"), "tokens are estimated: {t}");
}

#[test]
fn the_screen_at_120_columns() {
    let mut v = view();
    v.sel = 0;
    v.top = (0, 0);
    let t = text(&lines(&mut v, 120, 60));
    assert!(t.starts_with("log of main   full history   what the model got · request 3 of 3"), "{t}");
    assert!(t.contains("#7  14:02:07    ✗ result"), "{t}");
    assert!(t.contains("── turn 1 · 14:02 · user · 9.0k in · 1.1k out ──"), "{t}");
    assert!(t.contains("── compacted · 9.0k → 1.4k tokens ──"), "{t}");
    assert!(t.contains("$ bash") && t.contains(":* main"), "{t}");
    assert!(t.contains("cargo test signup"), "{t}");
    // the system prompt is folded
    assert!(t.contains("▸ 3 lines · "), "{t}");
    assert!(t.ends_with("esc close"), "{t}");
    // boxes stop at the code measure
    assert!(t.lines().all(|l| !l.contains('╭') || l.chars().count() <= 4 + crate::render::CODE_MAX), "{t}");
}

#[test]
fn narrow_drops_the_time() {
    let mut v = view();
    v.sel = 0;
    v.top = (0, 0);
    let t = text(&lines(&mut v, 70, 60));
    assert!(!t.contains("14:02:07"), "{t}");
    assert!(t.contains("#7    ✗ result"), "{t}");
}

#[test]
fn search_and_jump() {
    let mut v = view();
    v.search = Some(Search { text: "signup".into(), typing: false });
    v.rehit();
    // the user's message, the bash call, the summary
    assert_eq!(v.hits.len(), 3);
    v.sel = 0;
    v.jump(true);
    assert_eq!(v.items()[v.vis[v.sel]].role, Role::You);
    v.jump(true);
    assert_eq!(v.items()[v.vis[v.sel]].tool, "bash");
    assert_eq!(v.hit_label(), "2 of 3");
    v.jump(false);
    assert_eq!(v.hit_label(), "1 of 3");
}

#[test]
fn filters_hide_roles_and_tools() {
    let mut v = view();
    v.off.insert("tool:bash".into());
    v.off.insert("thinking".into());
    v.refilter();
    let shown: Vec<&Item> = v.vis.iter().map(|&i| &v.history[i]).collect();
    assert!(shown.iter().all(|i| i.tool != "bash" && i.role != Role::Thinking));
    assert!(shown.iter().any(|i| i.tool == "edit"));
    // rules go while filtered
    assert!(shown.iter().all(|i| i.rule.is_none()));
}

#[test]
fn folds_open_and_close() {
    let mut v = view();
    // the system prompt: folded by default
    assert!(!v.is_open(1));
    v.toggle(1);
    assert!(v.is_open(1));
    v.toggle(1);
    assert!(!v.is_open(1));
    v.toggle_all();
    assert!(v.is_open(1));
    // a toggle while all are open closes that one only
    v.toggle(1);
    assert!(!v.is_open(1) && v.is_open(2));
}

#[test]
fn the_calls_show_their_code() {
    let (h, b) = model::call_view("run_typescript", r#"{"code":"async function main() { return 1 }","description":"count"}"#);
    assert_eq!(h, "count");
    assert_eq!(b, Body::Code { lang: "ts", text: "async function main() { return 1 }".into() });
    let (h, b) = model::call_view("bash", r#"{"arg":"ls -la","description":"listing"}"#);
    assert_eq!((h.as_str(), b), ("listing", Body::Code { lang: "bash", text: "ls -la".into() }));
    let (h, b) = model::call_view("write_file", r#"{"file_path":"/a/b.py","content":"x = 1\n"}"#);
    assert_eq!((h.as_str(), b), ("/a/b.py  1 lines", Body::Code { lang: "python", text: "x = 1\n".into() }));
    let (_, b) = model::call_view("slack.send", r#"{"channel":"c","text":"hi"}"#);
    assert!(matches!(b, Body::Code { lang: "json", .. }));
}

/// A real session as text (dev check, not run by the gate):
/// `LOGVIEW_SESSION=~/.bise/sessions/<id> LOGVIEW_OUT=<file> cargo test -p
/// bend-tui real_session -- --ignored`: the history at 150 and 80
/// columns, the model view of the last request, a search.
#[test]
#[ignore]
fn real_session() {
    use std::fmt::Write;
    let dir = std::env::var("LOGVIEW_SESSION").expect("LOGVIEW_SESSION");
    let out = std::env::var("LOGVIEW_OUT").expect("LOGVIEW_OUT");
    let t0 = std::time::Instant::now();
    let mut v = View::load("main".into(), Path::new(&dir)).unwrap();
    let mut o = String::new();
    let _ = writeln!(o, "loaded {} items, {} requests in {:?}", v.history.len(), v.requests.len(), t0.elapsed());
    let find = |v: &View, pred: &dyn Fn(&Item) -> bool| v.vis.iter().position(|&i| pred(&v.history[i]));
    // somewhere with a compaction rule in view
    if let Some(k) = find(&v, &|i| i.rule.as_deref().is_some_and(|r| r.starts_with("compacted"))) {
        v.sel = k.saturating_sub(12);
        v.top = (v.sel, 0);
    }
    for w in [150, 80] {
        let t1 = std::time::Instant::now();
        let s = text(&lines(&mut v, w, 60));
        let _ = writeln!(o, "\n===== full history at {w} ({:?}) =====\n{s}", t1.elapsed());
    }
    v.bottom();
    let s = text(&lines(&mut v, 150, 60));
    let _ = writeln!(o, "\n===== bottom at 150 =====\n{s}");
    let t2 = std::time::Instant::now();
    v.set_mode(Mode::Model);
    let s = text(&lines(&mut v, 150, 60));
    let _ = writeln!(o, "\n===== what the model got, last request ({:?}) =====\n{s}", t2.elapsed());
    v.set_mode(Mode::History);
    v.search = Some(Search { text: "compact".into(), typing: false });
    v.rehit();
    v.jump(true);
    let s = text(&lines(&mut v, 150, 40));
    let _ = writeln!(o, "\n===== search 'compact' =====\n{s}");
    // the filter picker, over a filtered history
    v.search = None;
    v.off.insert("thinking".into());
    v.refilter();
    let mut rows: Vec<String> = model::ROLE_KEYS.iter().map(|s| s.to_string()).collect();
    rows.extend(v.tools().into_iter().map(|t| format!("tool:{t}")));
    let p = Picker { rows, sel: 2 };
    v.picker = Some(p.clone());
    let s = text(&picker_rows(&v, &p));
    let k = text(&[key_row(&v, 150)]);
    let top = text(&top_rows(&v, 150));
    let _ = writeln!(o, "\n===== filter picker =====\n{top}\n{s}\n{k}");
    std::fs::write(out, o).unwrap();
}

#[test]
fn an_image_is_a_line_never_its_base64() {
    let t = model::image_tags("look <image name=\"[Image #1]\" path=\"/tmp/a b.png\" mime=\"image/png\" b64=\"/nonexistent.b64\"> here");
    assert_eq!(t, "look [▣ [Image #1]: /tmp/a b.png] here");
    assert_eq!(model::image_tags("no image"), "no image");
}

#[test]
fn the_exact_body_when_logged() {
    let dir = std::env::temp_dir().join(format!("logview-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("123-0.json");
    std::fs::write(&f, r#"{"model":"m","messages":[{"role":"user","content":"hi"}],"img":"QUFB"}"#).unwrap();
    let mut v = view();
    // written before request 3's answer (14:02:17), after request 2's
    let at = model::iso_ms("2026-10-02T14:02:16.000Z").unwrap();
    v.dumps = vec![(at, f.clone())];
    v.req = 2;
    v.set_mode(Mode::Model);
    assert!(v.exact);
    assert!(v.model[0].head.starts_with("the exact request body · 123-0.json"), "{}", v.model[0].head);
    assert!(v.model[0].head.ends_with("1 messages"));
    v.step_request(-1);
    assert!(!v.exact);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(model::iso_ms("1970-01-01T00:00:01.500Z"), Some(1500));
    assert_eq!(model::iso_ms("2026-10-02T00:00:00.000Z"), Some(1_790_899_200_000));
}


/// A blank row after each entry, so they read apart (the user: "ça
/// manque d'espace entre les messages").
#[test]
fn a_blank_row_between_entries() {
    let mut v = view();
    v.sel = 0;
    v.top = (0, 0);
    let t = text(&lines(&mut v, 120, 80));
    let rows: Vec<&str> = t.lines().collect();
    // every header (a `#n` at the start) but the first has a blank row above
    let heads: Vec<usize> = (0..rows.len()).filter(|&y| rows[y].trim_start().starts_with('#') || rows[y].starts_with("›")).collect();
    assert!(heads.len() > 5, "{t}");
    for &y in &heads[1..] {
        assert_eq!(rows[y - 1], "", "no blank row above row {y}:\n{t}");
    }
    // a turn's rule has a blank row above and below
    let r = rows.iter().position(|l| l.starts_with("── turn 2")).expect("turn 2's rule");
    assert!(rows[r - 1].is_empty() && rows[r + 1].is_empty(), "{t}");
}

/// Wide screens: prose wraps at the prose measure, nothing passes the
/// entry measure (the user: "c'est full width, difficile à lire").
#[test]
fn bodies_stop_at_the_measure() {
    use serde_json::json;
    let long = ["the system prompt is long"; 40].join(" ");
    let text_of = |t: &str| json!([{"kind": "text", "text": t}]);
    let ls = [
        line(1, None, "session_start", json!({"session": "s-1", "format": 1, "created_by": "bise", "cwd": "/w"})),
        line(2, Some(1), "turn_started", json!({"cause": "user"})),
        line(3, Some(1), "user_message", json!({"content": text_of(&long), "delivery": "prompt"})),
    ];
    let log = read_bytes(&[("events.jsonl".into(), (ls.join("\n") + "\n").into_bytes())]);
    let mut v = View::of("main".into(), log, PathBuf::from("/nonexistent"), Redactor::default());
    v.toggle_all();
    let t = text(&lines(&mut v, 220, 60));
    let body: Vec<&str> = t.lines().filter(|l| l.contains("prompt is long") && !l.contains("› you")).collect();
    assert!(body.len() > 3, "{t}");
    // the rail and the indent (4), then at most 91 columns of prose
    assert!(body.iter().all(|l| l.chars().count() <= 4 + crate::render::PROSE_MAX), "{t}");
    assert!(body.iter().all(|l| l.starts_with("  ▎ ")), "{t}");
    // the header's size sits at the measure, not at the screen's edge
    let head = t.lines().find(|l| l.contains("› you")).unwrap();
    assert!(head.chars().count() <= MEASURE, "{head}");
}

/// One color per kind, on the label and the rail (designer's pick);
/// the labels keep the thread's glyphs.
#[test]
fn each_kind_has_its_color_and_label() {
    let v = view();
    let label = |pred: &dyn Fn(&Item) -> bool| {
        let it = v.history.iter().find(|i| pred(i)).expect("an entry");
        role_label(it, "main")
    };
    type Case<'a> = (&'a dyn Fn(&Item) -> bool, &'a str, ratatui::style::Color);
    let cases: [Case; 8] = [
        (&|i| i.role == Role::You, "› you", theme::accent()),
        (&|i| i.role == Role::Assistant, ":* main", theme::text()),
        (&|i| i.role == Role::Thinking, "∴ thinking", theme::dim()),
        (&|i| i.role == Role::Call && i.tool == "bash", "$ bash", theme::syntax_call()),
        (&|i| i.role == Role::Result { ok: false }, "  ✗ result", theme::error()),
        (&|i| i.role == Role::Injected, "⊕ bise", theme::syntax_type()),
        (&|i| i.role == Role::System, "§ system", theme::syntax_number()),
        (&|i| i.role == Role::Summary, "≡ summary", theme::syntax_string()),
    ];
    for (pred, word, color) in cases {
        let (w, st) = label(pred);
        assert_eq!((w.as_str(), st.fg), (word, Some(color)));
    }
    assert_eq!(label(&|i| i.role == Role::Result { ok: true }).0, "  └ result");
    // the rail of an open body: its kind's color; none for events
    let call = v.history.iter().find(|i| i.tool == "bash").unwrap();
    let rows = entry_rows(call, 120, true, Mode::History, "main");
    assert_eq!(rows[1].spans[0].content, "  ▎ ");
    assert_eq!(rows[1].spans[0].style.fg, Some(theme::syntax_call()));
    assert_eq!(rail(&Role::Event).content, "    ");
}

#[test]
fn the_key_bar_says_top_and_bottom() {
    let mut v = view();
    let k = |v: &View, w| text(&[key_row(v, w)]);
    assert_eq!(
        k(&v, 150),
        "/ search   f filter   g top   G bottom   tab what the model got   space open   ctrl+o open all   esc close"
    );
    // narrow: from the right, never `/ search` nor `esc close`
    assert_eq!(k(&v, 60), "/ search   f filter   g top   G bottom   esc close");
    assert_eq!(k(&v, 20), "/ search   esc close");
    v.set_mode(Mode::Model);
    assert!(k(&v, 160).starts_with("/ search   f filter   [ ] request   g top   G bottom   tab full history"), "{}", k(&v, 160));
    v.set_mode(Mode::History);
    v.search = Some(Search { text: "signup".into(), typing: false });
    v.rehit();
    v.sel = 0;
    v.jump(true);
    let bar = k(&v, 120);
    assert!(bar.starts_with("/ search   n next   N previous   esc clear search"), "{bar}");
    assert!(bar.ends_with("1 of 3"), "{bar}");
}

/// Typing in `/` moves the cursor to the first match from where it was;
/// esc puts it back; n and N go round the ends and say so.
#[test]
fn incsearch_wraps_and_esc_goes_back() {
    let mut v = view();
    let at = v.sel;
    let role = |v: &View| v.items()[v.vis[v.sel]].role.clone();
    v.start_search();
    for c in "signup".chars() {
        v.search.as_mut().unwrap().text.push(c);
        v.incsearch();
    }
    // the cursor was at the end: round to the first match
    assert_eq!(role(&v), Role::You);
    v.search_cancel();
    assert_eq!(v.sel, at);
    assert!(v.search.is_none());
    // from the top
    (v.sel, v.top) = (0, (0, 0));
    v.start_search();
    v.search.as_mut().unwrap().text = "signup".into();
    v.incsearch();
    v.search_done();
    assert_eq!(role(&v), Role::You);
    assert!(v.is_open(v.vis[v.sel]));
    assert_eq!(v.hit_label(), "1 of 3");
    v.jump(false);
    assert_eq!(v.hit_label(), "back to the last · 3 of 3");
    v.wrapped = "";
    v.jump(true);
    assert_eq!(v.hit_label(), "back to the first · 1 of 3");
    // g and G: the top and the bottom
    v.search = None;
    let bottom = v.vis.len() - 1;
    v.bottom();
    assert_eq!(v.sel, bottom);
}

/// The top bar counts the requests as the subtitle does (by position: a
/// REPL restart numbers its requests from 1 again), in the singular for
/// one, and says when there is none.
#[test]
fn the_top_bar_counts_like_the_subtitle() {
    use serde_json::json;
    let text_of = |t: &str| json!([{"kind": "text", "text": t}]);
    let reply = |seq, turn| line(seq, Some(turn), "assistant_message", json!({"req": 1, "model": "m", "parts": [{"kind": "text", "text": "ok"}], "calls": []}));
    let mk = |ls: &[String]| {
        let log = read_bytes(&[("events.jsonl".into(), (ls.join("\n") + "\n").into_bytes())]);
        View::of("main".into(), log, PathBuf::from("/nonexistent"), Redactor::default())
    };
    let start = line(1, None, "session_start", json!({"session": "s-1", "format": 1, "created_by": "bise", "cwd": "/w"}));
    let ask = |seq, turn| line(seq, Some(turn), "user_message", json!({"content": text_of("hi"), "delivery": "prompt"}));
    // two REPLs, each with its request 1
    let mut v = mk(&[start.clone(), ask(2, 1), reply(3, 1), ask(4, 2), reply(5, 2)]);
    let t = text(&lines(&mut v, 150, 30));
    assert!(t.contains("what the model got · request 2 of 2"), "{t}");
    assert!(t.contains("· 5 entries · 2 requests"), "{t}");
    let mut v = mk(&[start.clone(), ask(2, 1), reply(3, 1)]);
    let t = text(&lines(&mut v, 150, 30));
    assert!(t.contains("request 1 of 1") && t.contains("· 1 request"), "{t}");
    let mut v = mk(&[start]);
    let t = text(&lines(&mut v, 150, 30));
    assert!(t.contains("what the model got · no request yet") && t.contains("· 1 entry · 0 requests"), "{t}");
}
