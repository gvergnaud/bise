//! `bise computer-use ...` (C6 commands, and the processes the plugin and
//! the browsers start).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::broker::{self, Opts, StartError};
use crate::client::{self, Conn};
use crate::paths::Paths;
use crate::{browsers, host, mcp, state};

pub const USAGE: &str = "usage:
  bise computer-use setup-check [--json]   the /computer-use rows (browser, extension, live test)
  bise computer-use repair                 write the browsers' native host manifests and the shim
  bise computer-use live-test [--json]     open a test page in a background tab, click, type, screenshot, close
  bise computer-use status                 the broker's view: browsers, helper, agents
  bise computer-use request accessibility|screen_recording
                                           the helper shows the macOS prompt and opens the pane
                                           (screen_recording: relaunching:true when macOS reopens it)
  bise computer-use stop <agent>|--all     the agent lets go of the browser and apps until resume
  bise computer-use resume <agent>
  bise computer-use release <agent>        end of turn: detach, keep the tabs
  bise computer-use drop <agent>           close its tab group (unless the user touched a tab)
internal:
  bise computer-use mcp                    the `computer` plugin's MCP server (stdio)
  bise computer-use chrome-host            the browsers' native messaging host
  bise computer-use broker [--idle-exit S] [--no-helper-launch]";

fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("bise"))
}

fn print(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

pub fn main(args: &[String]) -> i32 {
    let paths = Paths::from_env();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.first().copied().unwrap_or("help") {
        "broker" => run_broker(&paths, &a[1..]),
        "mcp" => {
            let server = mcp::Server::new(paths, Arc::from(client::spawn_broker(exe())), mcp::Me::from_env());
            let stdin = std::io::stdin();
            mcp::run(server, stdin.lock(), std::io::stdout())
        }
        "chrome-host" => {
            // SAFETY: getppid has no preconditions
            let parent = host::exe_of(unsafe { libc::getppid() });
            host::run(&paths, Arc::from(client::spawn_broker(exe())), std::io::stdin(), std::io::stdout(), parent)
        }
        "setup-check" => {
            let start: Arc<client::Starter> = Arc::from(client::spawn_broker(exe()));
            let helper = broker::find_helper(&paths.home);
            print(&setup_check(&paths, helper.as_deref(), Some(&*start)));
            0
        }
        "repair" => {
            let (exe, current) = browsers::exe_and_current();
            match browsers::repair(&paths, &exe, current.as_deref()) {
                Ok(v) => {
                    print(&v);
                    0
                }
                Err(e) => {
                    eprintln!("repair: {}", e);
                    1
                }
            }
        }
        "live-test" => {
            let v = live_test(&paths, Arc::from(client::spawn_broker(exe())));
            print(&v);
            if v["ok"] == true {
                0
            } else {
                1
            }
        }
        "status" => match Conn::ctl(&paths).and_then(|mut c| c.call("status", &json!({}))) {
            Ok(Ok(v)) => {
                print(&v);
                0
            }
            Ok(Err(e)) => {
                print(&e);
                1
            }
            Err(_) => {
                print(&json!({"broker": "not running", "state": state::read(&paths)}));
                0
            }
        },
        "request" => {
            let Some(what) = a.get(1).filter(|w| matches!(**w, "accessibility" | "screen_recording")) else {
                eprintln!("{}", USAGE);
                return 2;
            };
            match Conn::ctl(&paths).and_then(|mut c| c.call("request", &json!({"what": what}))) {
                Ok(Ok(v)) => {
                    print(&v);
                    0
                }
                Ok(Err(e)) => {
                    print(&e);
                    1
                }
                Err(e) => {
                    eprintln!("request: the broker isn't running ({})", e);
                    1
                }
            }
        }
        cmd @ ("stop" | "resume" | "release" | "drop") => {
            let all = a.get(1) == Some(&"--all");
            let agent = a.get(1).filter(|_| !all).map(|s| s.to_string());
            if agent.is_none() && !(all && cmd == "stop") {
                eprintln!("{}", USAGE);
                return 2;
            }
            let args = json!({"agent": agent, "all": all});
            match control(&paths, cmd, &args) {
                Ok(v) => {
                    print(&v);
                    0
                }
                Err(e) => {
                    eprintln!("{}: {}", cmd, e);
                    1
                }
            }
        }
        // turned off (/computer-use off): the broker and the helper exit;
        // the plugin's enable state is the caller's (the TUI)
        "off" => {
            print(&off(&paths));
            0
        }
        // off, then nothing of it left on disk but the app bundle; what
        // only the user can remove is said in `left_to_you`
        "uninstall" => {
            let mut v = off(&paths);
            let _ = std::fs::remove_dir_all(paths.dir());
            let _ = std::fs::remove_dir_all(stable_extension_dir(&paths));
            v["left_to_you"] = json!([
                "remove the bise extension in each browser (Extensions page)",
                "remove \"bise Computer Use\" in System Settings > Privacy & Security > Accessibility and > Screen Recording",
            ]);
            print(&v);
            0
        }
        "help" | "-h" | "--help" => {
            println!("{}", USAGE);
            0
        }
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}

/// Computer use off: the host manifests and the shim go (a browser with
/// the extension can no longer start the host, so nothing starts the
/// broker again; setup's repair writes them back), the broker lets every
/// agent go and exits, the native hosts and the helper app quit.
fn off(paths: &Paths) -> Value {
    let removed = browsers::unrepair(paths);
    let broker = match Conn::ctl(paths).and_then(|mut c| c.call("quit", &json!({}))) {
        Ok(_) => "stopped",
        Err(_) => "not running",
    };
    // by their exact command lines: never another program
    let pkill = |args: &[&str]| {
        std::process::Command::new("pkill")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    let hosts = pkill(&["-f", "computer-use chrome-host chrome-extension://"]);
    let helper = pkill(&["-x", broker::HELPER_EXE]);
    json!({"off": true, "removed": removed, "broker": broker, "hosts_quit": hosts, "helper_quit": helper})
}

fn run_broker(paths: &Paths, a: &[&str]) -> i32 {
    // a new bise version brings its extension to the folder Chrome loads
    // (the extension then reloads itself): only once set up, never before
    if stable_extension_dir(paths).join("manifest.json").is_file() {
        let _ = extension_dir(paths);
    }
    // set up (the shim exists): point the browsers' host at this bise, the
    // one in use, so a pruned version never leaves Chrome without a host
    if paths.shim().exists() {
        let (exe, current) = browsers::exe_and_current();
        let _ = browsers::repair(paths, &exe, current.as_deref());
    }
    let mut opts = Opts::new(paths.clone());
    if let Some(i) = a.iter().position(|x| *x == "--idle-exit") {
        opts.idle_exit = a.get(i + 1).and_then(|s| s.parse().ok()).map(Duration::from_secs).filter(|d| !d.is_zero());
    }
    if a.contains(&"--no-helper-launch") {
        opts.launch_helper = false;
    }
    match broker::start(opts) {
        Ok(h) => {
            h.wait();
            0
        }
        Err(StartError::Running) => 0,
        Err(e) => {
            eprintln!("computer-use broker: {}", e);
            1
        }
    }
}

/// stop / resume / release / drop: through the broker, or on the files
/// when none runs (a later broker reads state.json).
pub fn control(paths: &Paths, cmd: &str, args: &Value) -> Result<Value, String> {
    let mut args = args.clone();
    if let Some(a) = args["agent"].as_str().filter(|a| !a.is_empty()) {
        args["agent"] = json!(resolve(&state::read(paths), a)?);
    }
    let args = &args;
    if let Ok(mut c) = Conn::ctl(paths) {
        return match c.call(cmd, args) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e["message"].as_str().unwrap_or("failed").to_string()),
            Err(e) => Err(e.to_string()),
        };
    }
    let mut st = state::read(paths);
    let agent = args["agent"].as_str().unwrap_or("").to_string();
    let names: Vec<String> = if args["all"] == true {
        st["agents"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
    } else {
        vec![agent.clone()]
    };
    let out = match cmd {
        "stop" => {
            for n in &names {
                let (hub, name) = crate::who::split(n);
                st["agents"][n] = json!({"driving": null, "where": null, "since_ms": null, "paused": false, "stopped": true, "name": name, "hub": hub});
                state::event(paths, n, "stopped", "you").map_err(|e| e.to_string())?;
            }
            json!({"stopped": names})
        }
        "resume" => {
            if let Some(m) = st["agents"].as_object_mut() {
                m.remove(&agent);
            }
            state::event(paths, &agent, "resumed", "you").map_err(|e| e.to_string())?;
            json!({"resumed": agent})
        }
        "drop" | "release" => {
            if let Some(m) = st["agents"].as_object_mut().filter(|_| cmd == "drop") {
                m.remove(&agent);
            }
            json!({ if cmd == "drop" { "dropped" } else { "released" }: agent })
        }
        _ => return Err("unknown command".into()),
    };
    if st.get("agents").is_none_or(|a| !a.is_object()) {
        st["agents"] = json!({});
    }
    st["v"] = json!(2);
    state::write(paths, &st).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The key a command names (docs/issues/18): a key of state.json as is;
/// else the agent of that name, refused when two projects' agents share
/// it; else the word itself (an untagged agent's key is its name).
fn resolve(st: &Value, word: &str) -> Result<String, String> {
    let agents = st["agents"].as_object();
    if agents.is_some_and(|m| m.contains_key(word)) {
        return Ok(word.to_string());
    }
    let hits: Vec<&String> = agents.into_iter().flatten().filter(|(_, a)| a["name"] == word).map(|(k, _)| k).collect();
    match hits.as_slice() {
        [] => Ok(word.to_string()),
        [k] => Ok(k.to_string()),
        _ => Err(format!(
            "{} agents are named {}, in different projects: name one by its key ({})",
            hits.len(),
            word,
            hits.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

// ---- setup-check ----

/// The `/computer-use` rows (design §8): browser → extension → live test,
/// then, for apps, accessibility and screen recording (C6, m_3893).
/// Each row: `state` (`done`, `waits`, `checking`, `failed`, `not_yet`),
/// `detail`, and `fix` (what ⏎ does: `install_browser`, `open_browser`,
/// `update_browser`, `add_extension`, `repair`, `run_live_test`,
/// `request_accessibility`, `request_screen_recording`, `install_helper`).
/// `helper`: the helper app when installed; then the broker is started
/// (`start`) and asked `permissions` (never `request`: the macOS prompt
/// only on the user's ⏎).
pub fn setup_check(paths: &Paths, helper: Option<&std::path::Path>, start: Option<&client::Starter>) -> Value {
    let mut v = browser_rows(paths);
    let perms: Option<Value> = helper.and_then(|_| {
        // started when none runs (a plain connect to the agents' socket),
        // then asked on the commands' socket
        drop(client::connect(paths, start).ok()?);
        let mut c = Conn::ctl(paths).ok()?;
        c.call("permissions", &json!({})).ok()?.ok()
    });
    let row = |id: &str, st: &str, detail: &str, fix: Option<&str>| json!({"id": id, "state": st, "detail": detail, "fix": fix});
    let app_row = |id: &str| -> Value {
        let fix = format!("request_{}", id);
        match perms.as_ref().map(|p| &p[id]) {
            _ if helper.is_none() => row(id, "not_yet", "", Some("install_helper")),
            Some(Value::Bool(true)) => row(id, "done", "", None),
            Some(Value::Bool(false)) => row(id, "waits", "", Some(&fix)),
            // the helper doesn't answer yet (starting, or reopening after a grant)
            _ => row(id, "checking", "", None),
        }
    };
    let apps = [app_row("accessibility"), app_row("screen_recording")];
    if let Some(rows) = v["rows"].as_array_mut() {
        rows.extend(apps);
    }
    v["helper"] = json!(helper.map(|h| h.display().to_string()));
    v
}

/// Where the unpacked extension is, for the "load unpacked" steps (before
/// the Web Store): `$BISE_CU_EXTENSION`, the app root's
/// `computer-use/extension` (`$BISE_APP_ROOT`, else the executable's
/// folder when it holds VERSION), then the try folder
/// `~/.bise/dev/try/computer-use/extension`.
pub fn extension_dir(paths: &Paths) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut c: Vec<PathBuf> = Vec::new();
    c.extend(bise_home::env::test_setting("BISE_CU_EXTENSION").map(PathBuf::from));
    c.extend(env("BISE_APP_ROOT").map(|r| r.join("computer-use").join("extension")));
    // a version dir or a bundle: the executable's folder holds VERSION
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.canonicalize().ok()).and_then(|e| e.parent().map(PathBuf::from)) {
        if dir.join("VERSION").is_file() {
            c.push(dir.join("computer-use").join("extension"));
        }
    }
    c.push(paths.root.join("dev").join("try").join("computer-use").join("extension"));
    let src = c.into_iter().find(|p| p.join("manifest.json").is_file())?;
    // the folder the user loads once: every version syncs into it and the
    // extension reloads itself (browsers::sync_extension)
    let stable = stable_extension_dir(paths);
    match browsers::sync_extension(&src, &stable) {
        Ok(_) => Some(stable),
        Err(_) => Some(src),
    }
}

/// The connected extension runs another build than its folder holds (an
/// extension that says no build, or no stamp on disk: not stale).
fn stale_build(paths: &Paths, running: &Value) -> bool {
    let disk = std::fs::read_to_string(stable_extension_dir(paths).join("build.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["build"].as_str().map(String::from));
    match (running.as_str(), disk) {
        (Some(r), Some(d)) => r != d,
        _ => false,
    }
}

/// `~/.bise/computer-use/extension`: where "Load unpacked" points.
pub fn stable_extension_dir(paths: &Paths) -> PathBuf {
    paths.root.join("computer-use").join("extension")
}

fn browser_rows(paths: &Paths) -> Value {
    let live = Conn::ctl(paths).ok().and_then(|mut c| c.call("status", &json!({})).ok()).and_then(Result::ok);
    let connected = |key: &str| -> Option<Value> {
        live.as_ref()?["browsers"].as_array()?.iter().find(|b| {
            b["connected"] == true && browsers::by_key(b["name"].as_str().unwrap_or("")).map(|x| x.key) == Some(key)
        }).cloned()
    };
    let list: Vec<Value> = browsers::ALL
        .iter()
        .map(|b| {
            let app = browsers::app_path(b, paths);
            let version = app.as_deref().and_then(browsers::app_version);
            let major = version.as_deref().and_then(browsers::major);
            let conn = connected(b.key);
            json!({
                "key": b.key,
                "name": b.name,
                "installed": app.is_some() || b.installed(paths),
                "app": app.as_ref().map(|a| a.display().to_string()),
                "version": version,
                "too_old": major.is_some_and(|m| m < browsers::MIN_MAJOR),
                "running": conn.is_some() || (app.is_some() && browsers::running(b)),
                "manifest": browsers::manifest_state(b, paths),
                "connected": conn.is_some(),
                "extension_version": conn.as_ref().map(|c| c["extension_version"].clone()),
                "extension_build": conn.as_ref().map(|c| c["extension_build"].clone()),
            })
        })
        .collect();
    let pick = list
        .iter()
        .find(|b| b["connected"] == true)
        .or_else(|| list.iter().find(|b| b["running"] == true))
        .or_else(|| list.iter().find(|b| b["installed"] == true))
        .unwrap_or(&list[0])
        .clone();
    let name = pick["name"].as_str().unwrap_or("Chrome").to_string();
    let major = pick["version"].as_str().and_then(browsers::major);
    let row = |id: &str, st: &str, detail: String, fix: Option<&str>| json!({"id": id, "state": st, "detail": detail, "fix": fix});
    let browser_row = if pick["installed"] != true {
        row("browser", "failed", format!("{} isn't installed", name), Some("install_browser"))
    } else if pick["too_old"] == true {
        row("browser", "failed", format!("{} {}", name, major.unwrap_or(0)), Some("update_browser"))
    } else if pick["running"] != true {
        row("browser", "waits", format!("{} isn't open", name), Some("open_browser"))
    } else {
        row("browser", "done", major.map(|m| format!("{} {}", name, m)).unwrap_or(name.clone()), None)
    };
    // a work profile: the organisation's policies can block it; say which
    // instead of waiting for an extension that can never connect
    let blocked = browsers::ALL
        .iter()
        .find(|b| b.name == name)
        .and_then(|b| crate::policy::blocks(&crate::policy::read(b, &crate::policy::managed_dirs()), &name));
    let ext_row = if browser_row["state"] != "done" {
        row("extension", "not_yet", String::new(), None)
    } else if let (Some(why), false) = (&blocked, pick["connected"] == true) {
        row("extension", "failed", why.clone(), None)
    } else if pick["connected"] == true && stale_build(paths, &pick["extension_build"]) {
        // Chrome runs an older build than the folder it loaded: it reloads
        // itself (sw.js checkBuild); until then the user can click ↻
        row("extension", "waits", "an update is ready".into(), Some("reload_extension"))
    } else if pick["connected"] == true {
        row("extension", "done", format!("v{}", pick["extension_version"].as_str().unwrap_or("?")), None)
    } else if pick["manifest"] != "ok" {
        row("extension", "failed", "the extension can't reach bise".into(), Some("repair"))
    } else {
        row("extension", "waits", format!("add bise to {}", name), Some("add_extension"))
    };
    let last: Option<Value> = std::fs::read_to_string(paths.live_test_file()).ok().and_then(|t| serde_json::from_str(&t).ok());
    let test_row = if ext_row["state"] != "done" {
        row("live_test", "not_yet", String::new(), None)
    } else {
        match &last {
            Some(l) if l["ok"] == true => row("live_test", "done", "ready".into(), None),
            Some(l) => row("live_test", "failed", l["error"].as_str().unwrap_or("failed").to_string(), Some("run_live_test")),
            None => row("live_test", "waits", "last".into(), Some("run_live_test")),
        }
    };
    json!({
        "browser": name,
        "min_major": browsers::MIN_MAJOR,
        "browsers": list,
        "shim": paths.shim().exists(),
        "broker": {"running": live.is_some()},
        "apps": live.as_ref().map(|l| l["apps"].clone()).unwrap_or(json!({"helper": "unknown"})),
        "live_test": last,
        "extension": {"id": browsers::EXTENSION_ID, "dir": extension_dir(paths).map(|d| d.display().to_string())},
        "rows": [browser_row, ext_row, test_row],
    })
}

// ---- live test ----

const TEST_PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>bise live test</title></head>
<body><h1>bise live test</h1>
<label for="name">Your name</label> <input id="name" type="text">
<button id="go" onclick="document.getElementById('out').textContent='clicked '+document.getElementById('name').value">Click me</button>
<p id="out">not clicked</p></body></html>"#;

/// Serve the test page on 127.0.0.1 (any path) until the process ends.
fn serve_test_page() -> std::io::Result<String> {
    let l = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://127.0.0.1:{}/bise-live-test", l.local_addr()?.port());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().expect("clone"));
            let mut line = String::new();
            while r.read_line(&mut line).map(|n| n > 2).unwrap_or(false) {
                line.clear();
            }
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", TEST_PAGE.len());
            let _ = s.write_all(head.as_bytes()).and_then(|_| s.write_all(TEST_PAGE.as_bytes()));
        }
    });
    Ok(url)
}

/// Open the test page in `bise · setup`, click, type, screenshot, close
/// (design §8). The result is kept for setup-check.
pub fn live_test(paths: &Paths, start: Arc<client::Starter>) -> Value {
    let t0 = Instant::now();
    let mut steps = Vec::new();
    let mut fail: Option<String> = None;
    let tmp = std::env::temp_dir().join(format!("bise-live-test-{}", std::process::id()));
    let hello = json!({"op": "hello", "agent": "setup", "session": "live-test", "tmpdir": tmp});
    let mut run = || -> Result<(), String> {
        let url = serve_test_page().map_err(|e| format!("test page: {}", e))?;
        let mut c = Conn::open(paths, Some(&*start), &hello).map_err(|e| format!("broker: {}", e))?;
        let mut step = |name: &str, op: &str, args: Value, steps: &mut Vec<Value>| -> Result<Value, String> {
            let t = Instant::now();
            let r = c.call(op, &args).map_err(|e| e.to_string())?;
            let ms = t.elapsed().as_millis() as u64;
            match r {
                Ok(v) => {
                    steps.push(json!({"step": name, "ok": true, "ms": ms}));
                    Ok(v)
                }
                Err(e) => {
                    let m = e["message"].as_str().unwrap_or("failed").to_string();
                    steps.push(json!({"step": name, "ok": false, "ms": ms, "error": e}));
                    Err(m)
                }
            }
        };
        let tab = step("open", "open", json!({"url": url}), &mut steps)?;
        let target = tab["target"].as_str().unwrap_or("").to_string();
        let snap = step("snapshot", "snapshot", json!({"target": target}), &mut steps)?;
        if !snap["text"].as_str().unwrap_or("").contains("Click me") {
            return Err("the snapshot has no \"Click me\" button".into());
        }
        step("type", "act", json!({"target": target, "action": "fill", "locator": {"role": "textbox", "name": "Your name"}, "text": "bise"}), &mut steps)?;
        step("click", "act", json!({"target": target, "action": "click", "locator": {"role": "button", "name": "Click me"}}), &mut steps)?;
        let read = step("check", "act", json!({"target": target, "action": "read", "locator": {"text": "clicked"}}), &mut steps)?;
        if !read.to_string().contains("clicked bise") {
            return Err("the click didn't land".into());
        }
        let shot = step("screenshot", "screenshot", json!({"target": target}), &mut steps)?;
        if let Some(p) = shot["path"].as_str() {
            let _ = std::fs::remove_file(p);
        }
        step("close", "act", json!({"target": target, "action": "close"}), &mut steps)?;
        Ok(())
    };
    if let Err(e) = run() {
        fail = Some(e);
    }
    let _ = control(paths, "drop", &json!({"agent": "setup"}));
    let _ = std::fs::remove_dir_all(&tmp);
    let v = json!({"ok": fail.is_none(), "error": fail, "steps": steps, "ms": t0.elapsed().as_millis() as u64, "at_ms": crate::now_ms()});
    if paths.ensure().is_ok() {
        let _ = std::fs::write(paths.live_test_file(), v.to_string());
    }
    v
}
