use super::*;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mcp-apps")
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bp-views-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn the_extension_is_announced_with_its_mime_type() {
    let c = client_capabilities();
    assert_eq!(c["extensions"][EXT]["mimeTypes"], json!([MIME]));
    // the stdio and remote clients send it (and the tools cache's key holds it)
    assert_eq!(crate::stdio::init_params()["capabilities"], c);
}

#[test]
fn a_tools_meta_ui_is_read_with_the_specs_defaults() {
    let plain = tool_ui(&json!({"name": "a"}));
    assert_eq!(plain, ToolUi { uri: None, model: true, app: true });
    let view = tool_ui(&json!({"name": "a", "_meta": {"ui": {"resourceUri": "ui://x/counter"}}}));
    assert_eq!(view, ToolUi { uri: Some("ui://x/counter".into()), model: true, app: true });
    let app = tool_ui(&json!({"name": "a", "_meta": {"ui": {"visibility": ["app"]}}}));
    assert_eq!((app.model, app.app), (false, true));
    let model = tool_ui(&json!({"name": "a", "_meta": {"ui": {"visibility": ["model"]}}}));
    assert_eq!((model.model, model.app), (true, false));
    let none = tool_ui(&json!({"name": "a", "_meta": {"ui": {"visibility": []}}}));
    assert_eq!((none.model, none.app), (false, false));
    // the older flat key
    let flat = tool_ui(&json!({"name": "a", "_meta": {"ui/resourceUri": "ui://x/old"}}));
    assert_eq!(flat.uri.as_deref(), Some("ui://x/old"));
    // only ui:// URIs are views
    for bad in [json!("https://x/y"), json!("ui://"), json!(3)] {
        assert_eq!(tool_ui(&json!({"_meta": {"ui": {"resourceUri": bad}}})).uri, None);
    }
}

#[test]
fn the_ui_field_has_no_space_and_stays_one_line() {
    assert_eq!(ui_field("ui://fake-app/counter"), " | ui: ui://fake-app/counter");
    assert_eq!(ui_field("ui://a b/c|d\n%"), " | ui: ui://a%20b/c%7Cd%0A%25");
    assert!(!ui_field("ui://x/é y").trim_start_matches(" | ui: ").contains(' '));
}

#[test]
fn the_fixtures_static_view_loads_as_a_tool() {
    let root = fixtures().join("fake-views");
    let (views, diags) = load("fake-views", &root);
    assert!(diags.is_empty(), "{:?}", diags);
    assert_eq!(views.len(), 1);
    let v = &views[0];
    assert_eq!((v.name.as_str(), v.tool.as_str(), v.uri.as_str()), ("hello", "hello", "ui://fake-views/views/hello"));
    assert!(v.description.starts_with("Greet someone"), "{}", v.description);
    assert_eq!(v.input_schema["required"], json!(["name"]));
    assert!(v.ui["csp"].is_object());
    assert!(v.html.ends_with("hello/index.html"));
}

#[test]
fn a_bad_view_is_dropped_with_a_warning() {
    let root = tmp("bad");
    let views = root.join(VIBE_EXTENSION).join("views");
    // good, with a dash: its tool is an identifier
    write(&views.join("my-view/view.json"), r#"{"name":"my-view","description":"d"}"#);
    write(&views.join("my-view/index.html"), "<p>hi</p>");
    // name differs from its folder
    write(&views.join("other/view.json"), r#"{"name":"wrong"}"#);
    write(&views.join("other/index.html"), "x");
    // no index.html
    write(&views.join("nohtml/view.json"), r#"{"name":"nohtml"}"#);
    // not JSON
    write(&views.join("broken/view.json"), "{");
    write(&views.join("broken/index.html"), "x");
    // a bad schema
    write(&views.join("schema/view.json"), r#"{"name":"schema","inputSchema":3}"#);
    write(&views.join("schema/index.html"), "x");
    // a folder outside the plugin
    #[cfg(unix)]
    {
        let out = tmp("bad-outside");
        write(&out.join("view.json"), r#"{"name":"escape"}"#);
        write(&out.join("index.html"), "x");
        std::os::unix::fs::symlink(&out, views.join("escape")).unwrap();
    }
    let (got, diags) = load("p", &root);
    assert_eq!(got.iter().map(|v| v.tool.as_str()).collect::<Vec<_>>(), vec!["my_view"]);
    assert_eq!(got[0].input_schema, json!({"type": "object"}));
    let n = if cfg!(unix) { 5 } else { 4 };
    assert_eq!(diags.len(), n, "{:?}", diags);
    assert!(diags.iter().all(|d| d.code == "plugin.view.invalid" && d.severity == Severity::Warning));
    assert!(diags.iter().any(|d| d.message.contains("outside")) || !cfg!(unix));
}

#[test]
fn the_virtual_server_answers_like_a_server() {
    let (views, _) = load("fake-views", &fixtures().join("fake-views"));
    let v = Virtual { plugin: "fake-views".into(), namespace: "fake_views".into(), opt_in: false, views };
    assert_eq!(v.key(), "fake-views/~views");
    let init = v.answer("initialize", &json!({}));
    assert!(init["result"]["capabilities"]["resources"].is_object());
    let list = v.answer("tools/list", &json!({}));
    let t = &list["result"]["tools"][0];
    assert_eq!(t["name"], "hello");
    assert_eq!(t["_meta"]["ui"]["resourceUri"], "ui://fake-views/views/hello");
    assert_eq!(tool_ui(t).uri.as_deref(), Some("ui://fake-views/views/hello"));
    let call = v.answer("tools/call", &json!({"name": "hello", "arguments": {"name": "Ada"}}));
    assert_eq!(call["result"]["structuredContent"], json!({"name": "Ada"}));
    assert_eq!(call["result"]["content"][0]["type"], "text");
    assert_eq!(v.answer("tools/call", &json!({"name": "nope"}))["error"]["code"], -32602);
    let read = v.answer("resources/read", &json!({"uri": "ui://fake-views/views/hello"}));
    let c = &read["result"]["contents"][0];
    assert_eq!(c["mimeType"], MIME);
    assert!(c["text"].as_str().unwrap().contains("<"));
    assert!(c["_meta"]["ui"]["csp"].is_object());
    assert_eq!(v.answer("resources/read", &json!({"uri": "ui://other"}))["error"]["code"], -32602);
    assert_eq!(v.answer("resources/list", &json!({}))["result"]["resources"][0]["mimeType"], MIME);
    assert_eq!(v.answer("sampling/createMessage", &json!({}))["error"]["code"], -32601);
}

#[test]
fn the_resource_cache_keeps_ui_results_only() {
    assert_eq!(ui_read("resources/read", &json!({"uri": "ui://a/b"})).as_deref(), Some("ui://a/b"));
    assert_eq!(ui_read("resources/read", &json!({"uri": "file:///etc"})), None);
    assert_eq!(ui_read("tools/call", &json!({"uri": "ui://a/b"})), None);
    let c = ResourceCache::default();
    c.put("ui://a/err", &json!({"error": {"code": 1}}));
    assert!(c.is_empty());
    c.put("ui://a/big", &json!({"result": {"text": "x".repeat(MAX_HTML as usize + 1)}}));
    assert!(c.is_empty());
    let ok = json!({"result": {"contents": []}});
    c.put("ui://a/ok", &ok);
    assert_eq!(c.get("ui://a/ok"), Some(ok));
    for i in 0..MAX_CACHED + 5 {
        c.put(&format!("ui://a/{}", i), &json!({"result": {}}));
    }
    assert_eq!(c.len(), MAX_CACHED);
    c.clear();
    assert!(c.is_empty());
}

#[test]
fn a_call_id_is_digits_dash_digits_only() {
    for ok in ["1730000000000-3", "0-0"] {
        assert!(call_ok(ok), "{ok}");
    }
    for bad in ["", "1", "-1", "1-", "a-1", "1-2-3", "../1-2", "1-2/x", "1 -2", &"9".repeat(41)] {
        assert!(!call_ok(bad), "{bad}");
    }
}

#[test]
fn the_views_file_keeps_the_whole_result_or_says_truncated() {
    let args = json!({"start": 3});
    let answer = json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": "counter at 3"}], "structuredContent": {"count": 3}}});
    let r = Record { plugin: "fake-app", server: "fake-app", uri: "ui://fake-app/counter", tool: "show_counter", args: &args, answer: &answer, at_ms: 7 };
    let v: Value = serde_json::from_str(&file_text(&r)).unwrap();
    assert_eq!(v, json!({"v": 1, "plugin": "fake-app", "server": "fake-app", "uri": "ui://fake-app/counter", "tool": "show_counter",
        "args": {"start": 3}, "result": answer["result"], "at_ms": 7}));
    // an error answer: its error, no result
    let err = json!({"error": {"code": -32000, "message": "boom"}});
    let v: Value = serde_json::from_str(&file_text(&Record { answer: &err, ..r })).unwrap();
    assert_eq!((v.get("result"), v["error"]["message"].clone()), (None, json!("boom")));
    // too big: the result says so, the file stays under the cap
    let big = json!({"result": {"structuredContent": {"blob": "x".repeat(MAX_FILE)}}});
    let text = file_text(&Record { answer: &big, ..r });
    assert!(text.len() <= MAX_FILE);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!((v["result"].clone(), v["args"].clone()), (json!({"truncated": true}), json!({"start": 3})));
    let huge_args = json!({"blob": "y".repeat(MAX_FILE)});
    let v: Value = serde_json::from_str(&file_text(&Record { args: &huge_args, ..r })).unwrap();
    assert_eq!(v["args"], json!({"truncated": true}));
}

#[test]
fn each_kept_call_gets_its_own_id_and_file() {
    let d = tmp("file");
    let args = json!({});
    let answer = json!({"result": {}});
    let r = Record { plugin: "p", server: "s", uri: "ui://p/v", tool: "t", args: &args, answer: &answer, at_ms: 12 };
    let a = keep(&d.join("views"), &r).unwrap();
    let b = keep(&d.join("views"), &r).unwrap();
    assert_eq!((a.as_str(), b.as_str()), ("12-0", "12-1"), "two calls of one ms");
    assert!(call_ok(&a) && call_ok(&b));
    let mut names: Vec<String> = std::fs::read_dir(d.join("views")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, vec!["12-0.json", "12-1.json"], "no temp file left");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(d.join("views/12-1.json")).unwrap()).unwrap();
    assert_eq!(v["uri"], "ui://p/v");
}

#[test]
fn the_view_header_is_one_ascii_line_of_json() {
    let h = view_header("p", "s", "ui://p/vue é\n", "12-0");
    assert!(h.is_ascii() && !h.contains('\n') && !h.contains('\r'), "{h}");
    let v: Value = serde_json::from_str(&h).unwrap();
    assert_eq!(v, json!({"plugin": "p", "server": "s", "uri": "ui://p/vue é\n", "call": "12-0"}));
}
