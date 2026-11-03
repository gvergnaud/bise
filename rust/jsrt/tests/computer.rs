// The `computer` object of code mode (src/computer.ts) against recorded
// C1 results: each test runs the real bend-jsrt the way the runtime does
// (program file + results file, exit 42 = one tool request, 0 = the
// result, 43 = the error), one round per tool call, so every request is
// checked byte for byte and the replay of the earlier ones is exercised.
// The SDK is in the prelude only when the session's plugin index lists the
// `computer` tools: each test writes one into a temp BEND_RUN_DIR.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

const INDEX: &str = "http://127.0.0.1:1/computer/computer computer status : #status
http://127.0.0.1:1/computer/computer computer open : #open
http://127.0.0.1:1/computer/computer computer act : #act
";

// a 1x1 PNG: what a screenshot file holds in these tests
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x60, 0x60, 0xf8, 0x0f, 0x00, 0x01, 0x04, 0x01, 0x00, 0x5f, 0xe5, 0xc3,
    0x4b, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

struct Box {
    dir: PathBuf,
    loaded: bool,
}

impl Box {
    fn new(loaded: bool) -> Box {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("jsrt-computer-{}-{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("run/7/plugins")).unwrap();
        std::fs::create_dir_all(dir.join("images")).unwrap();
        if loaded {
            std::fs::write(dir.join("run/7/plugins/mcp-index.txt"), INDEX).unwrap();
        }
        Box { dir, loaded }
    }

    // one round: (exit code, stdout trimmed)
    fn run(&self, program: &str, results: &[&str]) -> (i32, String) {
        let prog = self.dir.join("prog.ts");
        let res = self.dir.join("res.json");
        std::fs::write(&prog, program).unwrap();
        let arr: Vec<String> = results.iter().map(|r| json_str(r)).collect();
        std::fs::write(&res, format!("[{}]", arr.join(","))).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_bend-jsrt"))
            .arg(&prog)
            .arg(&res)
            .env("BEND_RUN_DIR", self.dir.join("run"))
            .env("BEND_REPL_PORT", "7")
            .env("BEND_IMAGE_DIR", self.dir.join("images"))
            .env("BISE_HOME", &self.dir)
            .output()
            .unwrap();
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    // every round of a program: each step is (tool, args JSON the SDK must
    // send, the recorded result); then the final round's (code, stdout)
    fn drive(&self, program: &str, steps: &[(&str, &str, &str)]) -> (i32, String) {
        for k in 0..steps.len() {
            let results: Vec<&str> = steps[..k].iter().map(|s| s.2).collect();
            let (code, out) = self.run(program, &results);
            let want = format!("{{\"tool\": \"computer.{}\", \"args\": {}}}", steps[k].0, steps[k].1);
            assert_eq!((code, out.as_str()), (42, want.as_str()), "round {} (loaded: {})", k + 1, self.loaded);
        }
        let results: Vec<&str> = steps.iter().map(|s| s.2).collect();
        self.run(program, &results)
    }

    fn ok(&self, program: &str, steps: &[(&str, &str, &str)]) -> String {
        let (code, out) = self.drive(program, steps);
        assert_eq!(code, 0, "the program failed: {out}");
        out
    }

    // the program fails: its error message
    fn err(&self, program: &str, steps: &[(&str, &str, &str)]) -> String {
        let (code, out) = self.drive(program, steps);
        assert_eq!(code, 43, "the program did not fail: {out}");
        let msg = out.strip_prefix("{\"error\": ").and_then(|s| s.strip_suffix('}')).unwrap_or(&out).to_string();
        unjson(&msg)
    }
}

impl Drop for Box {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

// a JSON string literal -> its text (the escapes JSON.stringify makes)
fn unjson(s: &str) -> String {
    let s = s.trim();
    let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) else { return s.to_string() };
    let mut o = String::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('n') => o.push('\n'),
            Some('t') => o.push('\t'),
            Some('u') => {
                let h: String = it.by_ref().take(4).collect();
                o.push(char::from_u32(u32::from_str_radix(&h, 16).unwrap_or(63)).unwrap_or('?'));
            }
            Some(x) => o.push(x),
            None => {}
        }
    }
    o
}

const ACT_OK: &str = r#"{"ok":true,"url":"https://shop.test/cart","title":"Cart","changed":"+ text \"1 item\"","summary":"clicked \"Add to cart\" · shop.test"}"#;
const SNAP: &str = r##"{"target":"tab:7","url":"https://shop.test/","title":"Shop","text":"# Shop · shop.test\n- main\n  - searchbox \"Search\" [e1]\n  - button \"Add to cart\" [e2]\n  - button \"Add to list\" [e3]\n  - link \"Anker cable 2 m\" [e4]\n  - link \"Anker cable 1 m\" [e5]\n  - checkbox \"Gift\" [e6]\n  - group\n    - text \"Price 12 €\"","refs":6,"truncated":false}"##;

fn act(args: &str) -> (&'static str, String, &'static str) {
    ("act", args.to_string(), ACT_OK)
}

fn steps(v: &[(&'static str, String, &'static str)]) -> Vec<(&'static str, String, &'static str)> {
    v.to_vec()
}

fn borrow<'a>(v: &'a [(&'static str, String, &'static str)]) -> Vec<(&'a str, &'a str, &'a str)> {
    v.iter().map(|(a, b, c)| (*a, b.as_str(), *c)).collect()
}

// ---- the prelude ----

#[test]
fn the_sdk_is_there_only_when_the_plugin_is_loaded() {
    let p = "return [typeof computer.browser.open, typeof ComputerError].join(' ')";
    assert_eq!(Box::new(true).ok(p, &[]), "function function");
    // without the plugin, `computer` is the generic tool proxy and
    // ComputerError a free identifier (a tool call of its own)
    let (code, out) = Box::new(false).run("return typeof ComputerError.x", &[]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_program_without_the_sdk_still_runs() {
    assert_eq!(Box::new(true).ok("async function main() { return 1 + 1 }", &[]), "2");
}

// ---- computer ----

#[test]
fn status() {
    let r = r#"{"browsers":[{"name":"Chrome","version":"154","connected":true,"extension_version":"0.1.0"}],"apps":{"helper":"absent","accessibility":false,"screen_recording":false},"me":{"stopped":false,"paused":[]}}"#;
    let out = Box::new(true).ok("const s = await computer.status(); return s.browsers[0].name + ' ' + s.apps.helper", &[("status", "{}", r)]);
    assert_eq!(out, "Chrome absent");
}

#[test]
fn browser_open() {
    let b = Box::new(true);
    let p = "const t = await computer.browser.open('https://shop.test'); return [t.target, t.id, t.url, t.title, t.userTouched].join('|')";
    let r = r#"{"target":"tab:7","url":"https://shop.test/","title":"Shop"}"#;
    // userTouched is false on a tab just opened, not undefined (launch #6)
    assert_eq!(b.ok(p, &[("open", r#"{"url":"https://shop.test"}"#, r)]), "tab:7|7|https://shop.test/|Shop|false");
    let p2 = "const t = await computer.browser.open('https://shop.test', { browser: 'edge' }); return t.target";
    assert_eq!(b.ok(p2, &[("open", r#"{"url":"https://shop.test","browser":"edge"}"#, r)]), "tab:7");
}

#[test]
fn browser_tabs() {
    let r = r#"[{"target":"tab:7","url":"https://a.test/","title":"A","user_touched":false},{"target":"tab:9","url":"https://b.test/","title":"B","user_touched":true}]"#;
    let p = "const ts = await computer.browser.tabs(); return ts.map((t) => t.target + ':' + t.userTouched).join(' ')";
    assert_eq!(Box::new(true).ok(p, &[("tabs", "{}", r)]), "tab:7:false tab:9:true");
}

#[test]
fn browser_tab_makes_a_handle_without_a_call() {
    let p = "const a = computer.browser.tab(7); const b = computer.browser.tab('tab:9'); await a.goto('https://x.test'); return a.target + ' ' + b.target + ' ' + a.url";
    let out = Box::new(true).ok(p, &[("act", r#"{"target":"tab:7","action":"goto","url":"https://x.test"}"#, ACT_OK)]);
    assert_eq!(out, "tab:7 tab:9 https://shop.test/cart");
}

const APPS: &str = r#"[{"target":"app:com.figma.Desktop","name":"Figma","pid":10,"windows":[{"title":"Design","focused":true}]},{"target":"app:com.apple.TextEdit","name":"TextEdit","pid":11,"windows":[]},{"target":"app:com.apple.Notes","name":"Notes","pid":12,"windows":[]},{"target":"app:com.example.NotesPlus","name":"Notes Plus","pid":13,"windows":[]}]"#;

#[test]
fn apps() {
    let p = "const xs = await computer.apps(); return xs.map((a) => a.bundleId + '/' + a.pid).join(' ')";
    let out = Box::new(true).ok(p, &[("apps", "{}", APPS)]);
    assert_eq!(out, "com.figma.Desktop/10 com.apple.TextEdit/11 com.apple.Notes/12 com.example.NotesPlus/13");
}

#[test]
fn app_by_name_bundle_id_substring_and_target() {
    let b = Box::new(true);
    let one = |p: &str| b.ok(p, &[("apps", "{}", APPS)]);
    assert_eq!(one("return (await computer.app('figma')).target"), "app:com.figma.Desktop");
    assert_eq!(one("return (await computer.app('com.apple.TextEdit')).name"), "TextEdit");
    // an exact name wins over a longer one that contains it
    assert_eq!(one("return (await computer.app('Notes')).target"), "app:com.apple.Notes");
    assert_eq!(one("return (await computer.app('text')).target"), "app:com.apple.TextEdit");
    // a target string: no call
    assert_eq!(b.ok("return (await computer.app('app:com.x.Y')).bundleId", &[]), "com.x.Y");
}

#[test]
fn app_not_found_and_ambiguous_list_the_apps() {
    let b = Box::new(true);
    let e = b.err("await computer.app('Photoshop')", &[("apps", "{}", APPS)]);
    assert!(e.starts_with("not_found: no running app called \"Photoshop\""), "{e}");
    assert!(e.contains("\ncandidates:\n  Figma (app:com.figma.Desktop)"), "{e}");
    let e = b.err("await computer.app('e')", &[("apps", "{}", APPS)]);
    assert!(e.starts_with("ambiguous: "), "{e}");
}

#[test]
fn app_window_rides_on_every_call() {
    let p = "const f = await computer.app('Figma', { window: 'Design' }); await f.snapshot(); await f.getByRole('button', { name: 'Share' }).click(); return f.windowTitle";
    let out = Box::new(true).ok(p, &[
        ("apps", "{}", APPS),
        ("snapshot", r#"{"target":"app:com.figma.Desktop","window":"Design"}"#, SNAP),
        ("act", r#"{"target":"app:com.figma.Desktop","window":"Design","action":"click","locator":{"role":"button","name":"Share"}}"#, ACT_OK),
    ]);
    assert_eq!(out, "Design");
    let p2 = "const f = (await computer.app('app:com.figma.Desktop')).window('Untitled'); return (await f.read())";
    let out = Box::new(true).ok(p2, &[("act", r#"{"target":"app:com.figma.Desktop","window":"Untitled","action":"read"}"#, ACT_OK)]);
    assert_eq!(out, "+ text \"1 item\"");
}

#[test]
fn app_goto_is_bad_args_without_a_call() {
    let e = Box::new(true).err("await (await computer.app('app:com.apple.TextEdit')).goto('https://x')", &[]);
    assert_eq!(e, "bad_args: goto works on tabs only");
}

// ---- tab / app methods ----

#[test]
fn snapshot_returns_the_text() {
    let b = Box::new(true);
    let t = "const t = computer.browser.tab(7);";
    let out = b.ok(&format!("{t} return await t.snapshot()"), &[("snapshot", r#"{"target":"tab:7"}"#, SNAP)]);
    assert!(out.starts_with("# Shop · shop.test\n- main\n  - searchbox \"Search\" [e1]"), "{out}");
    let out = b.ok(&format!("{t} await t.snapshot({{ maxNodes: 50 }}); return t.title"), &[("snapshot", r#"{"target":"tab:7","max_nodes":50}"#, SNAP)]);
    assert_eq!(out, "Shop");
}

const SHOT: &str = r#"{"path":"__PATH__","mime":"image/jpeg","width":1,"height":1}"#;

#[test]
fn screenshot_is_an_image_block() {
    let b = Box::new(true);
    let png = b.dir.join("shot.png");
    std::fs::write(&png, PNG).unwrap();
    let shot = SHOT.replace("__PATH__", &png.to_string_lossy());
    let out = b.ok("const t = computer.browser.tab(7); return [{ type: 'text', text: 'look:' }, await t.screenshot()]", &[("screenshot", r#"{"target":"tab:7"}"#, &shot)]);
    assert!(out.starts_with("look:\n<image name=\"[Image #1]\" path=\""), "{out}");
    let out = b.ok("const t = computer.browser.tab(7); const s = await t.screenshot({ maxWidth: 640 }); return s.type + ' ' + s.mimeType", &[("screenshot", r#"{"target":"tab:7","max_width":640}"#, &shot)]);
    assert_eq!(out, "image image/jpeg");
}

#[test]
fn screenshot_of_an_element() {
    let b = Box::new(true);
    let png = b.dir.join("shot.png");
    std::fs::write(&png, PNG).unwrap();
    let shot = SHOT.replace("__PATH__", &png.to_string_lossy());
    // a ref goes as is
    let p = "const t = computer.browser.tab(7); return (await t.ref('e4').screenshot()).type";
    assert_eq!(b.ok(p, &[("screenshot", r#"{"target":"tab:7","ref":"e4"}"#, &shot)]), "image");
    // a locator: one snapshot finds its ref
    let p = "const t = computer.browser.tab(7); return (await t.screenshot({ element: t.getByRole('link', { name: /2 m/ }) })).type";
    assert_eq!(
        b.ok(p, &[("snapshot", r#"{"target":"tab:7","max_nodes":2000}"#, SNAP), ("screenshot", r#"{"target":"tab:7","ref":"e4"}"#, &shot)]),
        "image"
    );
    // several matches: ambiguous, with the lines
    let p = "const t = computer.browser.tab(7); await t.getByRole('link').screenshot()";
    let e = b.err(p, &[("snapshot", r#"{"target":"tab:7","max_nodes":2000}"#, SNAP)]);
    assert!(e.starts_with("ambiguous: the locator matches 2 elements"), "{e}");
    assert!(e.contains("  - link \"Anker cable 2 m\" [e4]\n  - link \"Anker cable 1 m\" [e5]"), "{e}");
}

#[test]
fn page_press_type_scroll_read_close() {
    let s = steps(&[
        act(r#"{"target":"tab:7","action":"press","keys":"Control+A Delete"}"#),
        act(r#"{"target":"tab:7","action":"type","text":"hello"}"#),
        act(r#"{"target":"tab:7","action":"scroll"}"#),
        act(r#"{"target":"tab:7","action":"scroll","direction":"up","amount":300}"#),
        act(r#"{"target":"tab:7","action":"read"}"#),
        act(r#"{"target":"tab:7","action":"close"}"#),
    ]);
    let p = "const t = computer.browser.tab(7); await t.press('Control+A Delete'); await t.type('hello'); await t.scroll(); await t.scroll('up', 300); const text = await t.read(); const r = await t.close(); return text + ' / ' + r.summary";
    assert_eq!(Box::new(true).ok(p, &borrow(&s)), "+ text \"1 item\" / clicked \"Add to cart\" · shop.test");
}

#[test]
fn page_wait_for_text_locator_or_time() {
    let s = steps(&[
        act(r#"{"target":"tab:7","action":"wait","text":"Order placed"}"#),
        act(r#"{"target":"tab:7","action":"wait","timeout_ms":9000,"locator":{"role":"button","name":"Pay"}}"#),
        act(r#"{"target":"tab:7","action":"wait","amount":500}"#),
    ]);
    let p = "const t = computer.browser.tab(7); await t.waitFor('Order placed'); await t.waitFor(t.getByRole('button', { name: 'Pay' }), { timeout: 9000 }); await t.waitFor(500); return 'ok'";
    assert_eq!(Box::new(true).ok(p, &borrow(&s)), "ok");
}

#[test]
fn the_raw_act_stays_callable() {
    let p = "const t = computer.browser.tab(7); return (await t.act({ action: 'hover', ref: 'e2' })).ok";
    assert_eq!(Box::new(true).ok(p, &[("act", r#"{"target":"tab:7","action":"hover","ref":"e2"}"#, ACT_OK)]), "true");
}

// ---- locators ----

#[test]
fn locator_actions() {
    let s = steps(&[
        act(r#"{"target":"tab:7","action":"fill","locator":{"role":"searchbox"},"text":"usb-c cable 2m"}"#),
        act(r#"{"target":"tab:7","action":"click","locator":{"role":"button","name":"Add to cart","exact":true}}"#),
        act(r#"{"target":"tab:7","action":"type","locator":{"label":"Email"},"text":"ana@example.com"}"#),
        act(r#"{"target":"tab:7","action":"press","locator":{"label":"Email"},"keys":"Enter"}"#),
        act(r#"{"target":"tab:7","action":"check","ref":"e6","value":true}"#),
        act(r#"{"target":"tab:7","action":"check","ref":"e6","value":false}"#),
        act(r#"{"target":"tab:7","action":"select","locator":{"role":"combobox","name":"Size"},"value":"XL"}"#),
        act(r#"{"target":"tab:7","action":"hover","locator":{"text":"Price"}}"#),
        act(r#"{"target":"tab:7","action":"scroll","locator":{"role":"list"},"direction":"down","amount":200}"#),
        act(r#"{"target":"tab:7","action":"click","locator":{"role":"link","name_re":"Anker.*2 m/i","nth":0},"timeout_ms":2000}"#),
        act(r#"{"target":"tab:7","action":"click","locator":{"text_re":"€$","nth":-1}}"#),
        act(r#"{"target":"tab:7","action":"click","locator":{"role":"link","nth":3}}"#),
        act(r#"{"target":"tab:7","action":"wait","locator":{"role":"dialog"}}"#),
        act(r#"{"target":"tab:7","action":"click","locator":{"role":"button","name":"OK"}}"#),
    ]);
    let p = "
      const t = computer.browser.tab(7);
      await t.getByRole('searchbox').fill('usb-c cable 2m');
      await t.getByRole('button', { name: 'Add to cart', exact: true }).click();
      await t.getByLabel('Email').type('ana@example.com');
      await t.getByLabel('Email').press('Enter');
      await t.ref('[e6]').check();
      await t.ref('6').uncheck();
      await t.getByRole('combobox', { name: 'Size' }).select('XL');
      await t.getByText('Price').hover();
      await t.getByRole('list').scroll('down', 200);
      await t.getByRole('link', { name: /Anker.*2 m/gi }).first().click({ timeout: 2000 });
      await t.getByText(/€$/).last().click();
      await t.getByRole('link').nth(3).click();
      await t.getByRole('dialog').waitFor();
      await t.locator({ role: 'button', name: 'OK' }).click();
      return t.url;";
    assert_eq!(Box::new(true).ok(p, &borrow(&s)), "https://shop.test/cart");
}

#[test]
fn locator_text_content() {
    let r = r#"{"ok":true,"title":"Shop","changed":"12,99 €","summary":"read the price · shop.test"}"#;
    let p = "const t = computer.browser.tab(7); return await t.getByText(/€/).first().textContent()";
    assert_eq!(Box::new(true).ok(p, &[("act", r#"{"target":"tab:7","action":"read","locator":{"text_re":"€","nth":0}}"#, r)]), "12,99 €");
}

#[test]
fn locator_count_reads_one_snapshot() {
    let b = Box::new(true);
    let snap = ("snapshot", r#"{"target":"tab:7","max_nodes":2000}"#, SNAP);
    let count = |expr: &str| b.ok(&format!("const t = computer.browser.tab(7); return await {expr}.count()"), &[snap]);
    assert_eq!(count("t.getByRole('button')"), "2");
    assert_eq!(count("t.getByRole('button', { name: 'add to' })"), "2");
    assert_eq!(count("t.getByRole('button', { name: 'add to', exact: true })"), "0");
    assert_eq!(count("t.getByRole('link', { name: /2 m$/ })"), "1");
    assert_eq!(count("t.getByText('Price')"), "1");
    assert_eq!(count("t.getByLabel('Gift')"), "1");
    assert_eq!(count("t.getByRole('link').nth(5)"), "0");
    assert_eq!(count("t.ref('e3')"), "1");
    assert_eq!(count("t.ref('e99')"), "0");
}

#[test]
fn locator_ref_finds_the_element() {
    let p = "const t = computer.browser.tab(7); return await t.getByRole('searchbox').ref()";
    assert_eq!(Box::new(true).ok(p, &[("snapshot", r#"{"target":"tab:7","max_nodes":2000}"#, SNAP)]), "e1");
}

#[test]
fn locator_bad_args_never_call() {
    let b = Box::new(true);
    assert_eq!(b.err("computer.browser.tab(7).ref('Buy')", &[]), "bad_args: a ref looks like e14, from snapshot(); got \"Buy\"");
    assert!(b.err("computer.browser.tab(7).ref('e1').first()", &[]).starts_with("bad_args: nth() needs a locator"));
    assert!(b.err("computer.browser.tab(7).getByLabel(/x/)", &[]).starts_with("bad_args: getByLabel takes a string"));
    assert!(b.err("await computer.browser.tab(7).waitFor(/x/)", &[]).starts_with("bad_args: waitFor takes a text or a locator"));
    assert!(b.err("computer.browser.tab(7).locator('button')", &[]).starts_with("bad_args: locator takes an object"));
}

// ---- errors ----

#[test]
fn a_c1_error_is_thrown_with_its_candidates() {
    let r = r#"{"error":{"code":"not_found","message":"couldn't find button \"Buy\"","candidates":["- button \"Add to cart\" [e2]","- button \"Add to list\" [e3]"],"summary":"couldn't find \"Buy\""}}"#;
    let e = Box::new(true).err(
        "await computer.browser.tab(7).getByRole('button', { name: 'Buy' }).click()",
        &[("act", r#"{"target":"tab:7","action":"click","locator":{"role":"button","name":"Buy"}}"#, r)],
    );
    assert_eq!(e, "not_found: couldn't find button \"Buy\"\ncandidates:\n  - button \"Add to cart\" [e2]\n  - button \"Add to list\" [e3]");
}

#[test]
fn every_c1_error_code_can_be_caught() {
    let b = Box::new(true);
    for code in [
        "not_set_up", "no_browser", "no_helper", "no_permission", "not_found", "ambiguous", "stale_ref", "stopped", "paused",
        "refused", "timeout", "needs_front", "bad_args",
    ] {
        let r = format!(r#"{{"error":{{"code":"{code}","message":"m","candidates":["- a","- b"],"summary":"couldn't click"}}}}"#);
        let p = "try { await computer.browser.tab(7).ref('e2').click() } catch (e) { return [e instanceof ComputerError, e.name, e.code, e.candidates.length, e.summary].join(' ') }";
        let out = b.ok(p, &[("act", r#"{"target":"tab:7","action":"click","ref":"e2"}"#, &r)]);
        assert_eq!(out, format!("true ComputerError {code} 2 couldn't click"));
    }
}

#[test]
fn errors_of_every_tool_are_thrown() {
    let b = Box::new(true);
    let r = r#"{"error":{"code":"no_browser","message":"no browser has the bise extension; ask the user to run /computer-use"}}"#;
    for (p, tool, args) in [
        ("await computer.status()", "status", "{}"),
        ("await computer.browser.open('https://x.test')", "open", r#"{"url":"https://x.test"}"#),
        ("await computer.browser.tabs()", "tabs", "{}"),
        ("await computer.apps()", "apps", "{}"),
        ("await computer.browser.tab(7).snapshot()", "snapshot", r#"{"target":"tab:7"}"#),
        ("await computer.browser.tab(7).screenshot()", "screenshot", r#"{"target":"tab:7"}"#),
    ] {
        let e = b.err(p, &[(tool, args, r)]);
        assert_eq!(e, "no_browser: no browser has the bise extension; ask the user to run /computer-use", "{p}");
    }
}

#[test]
fn a_non_json_answer_is_not_set_up() {
    let e = Box::new(true).err("await computer.status()", &[("status", "{}", "unknown tool: computer.status")]);
    assert!(e.starts_with("not_set_up: computer.status answered: unknown tool: computer.status"), "{e}");
}

#[test]
fn a_caught_error_lets_the_program_go_on() {
    let r = r#"{"error":{"code":"timeout","message":"no \"Accept cookies\" after 2000 ms"}}"#;
    let p = "const t = computer.browser.tab(7);
      try { await t.getByRole('button', { name: 'Accept cookies' }).click({ timeout: 2000 }) } catch (e) { if (e.code !== 'timeout') throw e }
      return (await t.getByRole('searchbox').fill('cable')).ok";
    let out = Box::new(true).ok(p, &[
        ("act", r#"{"target":"tab:7","action":"click","locator":{"role":"button","name":"Accept cookies"},"timeout_ms":2000}"#, r),
        ("act", r#"{"target":"tab:7","action":"fill","locator":{"role":"searchbox"},"text":"cable"}"#, ACT_OK),
    ]);
    assert_eq!(out, "true");
}

// ---- the design's example (docs/computer-use-design.md §6) ----

#[test]
fn the_design_example_runs() {
    let b = Box::new(true);
    let png = b.dir.join("shot.png");
    std::fs::write(&png, PNG).unwrap();
    let shot = SHOT.replace("__PATH__", &png.to_string_lossy());
    let p = r#"async function main() {
  const tab = await computer.browser.open("https://www.amazon.fr");
  await tab.getByRole("searchbox").fill("usb-c cable 2m");
  await tab.press("Enter");
  await tab.getByRole("link", { name: /Anker.*2 m/ }).first().click();
  const price = await tab.getByText(/€/).first().textContent();
  return [{ type: "text", text: price }, await tab.screenshot()];
}"#;
    let read = r#"{"ok":true,"title":"Anker","changed":"12,99 €","summary":"read · amazon.fr"}"#;
    let out = b.ok(p, &[
        ("open", r#"{"url":"https://www.amazon.fr"}"#, r#"{"target":"tab:42","url":"https://www.amazon.fr/","title":"Amazon"}"#),
        ("act", r#"{"target":"tab:42","action":"fill","locator":{"role":"searchbox"},"text":"usb-c cable 2m"}"#, ACT_OK),
        ("act", r#"{"target":"tab:42","action":"press","keys":"Enter"}"#, ACT_OK),
        ("act", r#"{"target":"tab:42","action":"click","locator":{"role":"link","name_re":"Anker.*2 m","nth":0}}"#, ACT_OK),
        ("act", r#"{"target":"tab:42","action":"read","locator":{"text_re":"€","nth":0}}"#, read),
        ("screenshot", r#"{"target":"tab:42"}"#, &shot),
    ]);
    assert!(out.starts_with("12,99 €\n<image name=\"[Image #1]\""), "{out}");
}

// ---- uploads (task cu-upload) ----

#[test]
fn set_input_files_is_one_act_with_the_paths() {
    let b = Box::new(true);
    let p = "const t = computer.browser.tab(7); return (await t.getByRole('button', { name: /upload/i }).setInputFiles(['/Users/me/a.pdf', '/Users/me/b.pdf'])).summary";
    let want = r#"{"target":"tab:7","action":"set_input_files","locator":{"role":"button","name_re":"upload/i"},"files":["/Users/me/a.pdf","/Users/me/b.pdf"]}"#;
    assert_eq!(b.ok(p, &[("act", want, ACT_OK)]), "clicked \"Add to cart\" · shop.test");
    // one path is a list of one; a ref and a timeout ride along
    let p = "const t = computer.browser.tab(7); await t.ref('e4').setInputFiles('/Users/me/a.pdf', { timeout: 9000 }); return 'ok'";
    let want = r#"{"target":"tab:7","action":"set_input_files","ref":"e4","files":["/Users/me/a.pdf"],"timeout_ms":9000}"#;
    assert_eq!(b.ok(p, &[("act", want, ACT_OK)]), "ok");
    // no files: bad_args, no call
    let e = b.err("const t = computer.browser.tab(7); await t.ref('e4').setInputFiles([])", &[]);
    assert!(e.starts_with("bad_args: setInputFiles takes an absolute path"), "{e}");
}

#[test]
fn wait_for_file_chooser_arms_clicks_waits_then_sets() {
    let b = Box::new(true);
    let p = "const t = computer.browser.tab(7);
const chooser = await t.waitForFileChooser(() => t.getByText('Justificatif').click());
const r = await chooser.setFiles(['/Users/me/id.pdf']);
return [chooser.multiple, chooser.id, r.ok].join(' ')";
    let waited = r#"{"ok":true,"url":"https://shop.test/","title":"Shop","changed":"","summary":"a file chooser opened · shop.test","chooser":3,"multiple":false,"kind":"input"}"#;
    let out = b.ok(
        p,
        &[
            ("act", r#"{"target":"tab:7","action":"expect_chooser"}"#, ACT_OK),
            ("act", r#"{"target":"tab:7","action":"click","locator":{"text":"Justificatif"}}"#, ACT_OK),
            ("act", r#"{"target":"tab:7","action":"wait_chooser"}"#, waited),
            ("act", r#"{"target":"tab:7","action":"set_input_files","chooser":3,"files":["/Users/me/id.pdf"]}"#, ACT_OK),
        ],
    );
    assert_eq!(out, "false 3 true");
    // no chooser: the C1 error is thrown, catchable by its code
    let p = "const t = computer.browser.tab(7);
try { await t.waitForFileChooser(() => t.ref('e2').click(), { timeout: 1000 }); } catch (e) { return e.code; }";
    let none = r#"{"error":{"code":"chooser_not_opened","message":"no file chooser opened within 1000 ms"}}"#;
    let out = b.ok(
        p,
        &[
            ("act", r#"{"target":"tab:7","action":"expect_chooser"}"#, ACT_OK),
            ("act", r#"{"target":"tab:7","action":"click","ref":"e2"}"#, ACT_OK),
            ("act", r#"{"target":"tab:7","action":"wait_chooser","timeout_ms":1000}"#, none),
        ],
    );
    assert_eq!(out, "chooser_not_opened");
}
