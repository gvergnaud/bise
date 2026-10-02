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

