//! Computer use in the TUI (docs/computer-use-design.md §8, designer
//! m_3551, m_3554, m_3896): the `/computer-use` screen, and the live marks
//! of an agent that drives Chrome or an app.
//!
//! The screen: one row per step, chrome → chrome extension → live test,
//! then, under a faint "for apps", accessibility and screen recording
//! (only when the helper app is installed). The rows come from `bise
//! computer-use setup-check --json`, polled once a second on a thread; a
//! row that turns `✓` moves the cursor on. ⏎ runs the selected row's fix
//! (open the browser, the load-unpacked steps, repair, the live test, the
//! macOS prompts), ↑↓ step, ⇧⇥ the approvals mode, x stops every agent
//! that drives, esc closes.
//!
//! The marks read C6's `~/.bise/run/computer-use/state.json` (by mtime):
//! `↖` in the agent's row, `· ↖ Chrome` in its divider, `? you took the
//! wheel · ⏎ give it back` while the user holds its tab or app. Stops
//! (ctrl+c, a click on `↖`, `/stop`) call `bise computer-use stop`, the
//! user's next message to that agent `resume`.
//!
//! Tests point `BISE_COMPUTER_USE` at a fake program (it gets the
//! subcommand's args) and `BEND_RUN_DIR` at a temp run dir.

use crate::{theme, App};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use unicode_width::UnicodeWidthStr;

// ---- the commands ----

/// `bise computer-use <args>`: this executable, or `$BISE_COMPUTER_USE`
/// (a program that takes `<args>` alone: the tests' fake).
fn command(args: &[&str]) -> Command {
    let mut c = match bise_home::env::test_setting("BISE_COMPUTER_USE") {
        Some(p) => Command::new(p),
        None => {
            let mut c = Command::new(std::env::current_exe().unwrap_or_else(|_| PathBuf::from("bise")));
            c.arg("computer-use");
            c
        }
    };
    c.args(args).stdin(Stdio::null()).stderr(Stdio::null());
    c
}

/// Run it and read its JSON (None: it failed or said nothing readable).
fn json_of(args: &[&str]) -> Option<Value> {
    let out = command(args).stdout(Stdio::piped()).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

/// The opt-in (computer-use-ship.md §1): the built-in `computer` plugin
/// is off until /computer-use turns it on. The hub sees the plugins
/// change and relaunches each agent at its next idle, prompt included.
pub(crate) fn set_on(on: bool) {
    let _ = bend_plugins::state::set_enabled(&bend_plugins::state::state_path(), "computer", on);
}

/// Whether the plugin is on: named in `enabled` and not in `disabled`
/// (what `set_on` writes; the `/computer-use` menu follows it).
pub(crate) fn is_on() -> bool {
    on_in(&bend_plugins::state::state_path())
}

pub(crate) fn on_in(state: &std::path::Path) -> bool {
    let named = |l: Vec<String>| l.iter().any(|n| n == "computer");
    named(bend_plugins::state::enabled(state)) && !named(bend_plugins::state::disabled(state))
}

/// /computer-use off|uninstall: the plugin off, then `bise computer-use
/// off|uninstall` (the broker, hosts and helper quit; uninstall also
/// removes what setup wrote). The line the feed shows.
pub(crate) fn turn_off(uninstall: bool) -> String {
    set_on(false);
    let v = json_of(&[if uninstall { "uninstall" } else { "off" }]).unwrap_or(Value::Null);
    let mut s = String::from("computer use is off: agents lose it at their next idle");
    if uninstall {
        s.push_str(". removed the browser hosts and its files");
        let left: Vec<&str> = v["left_to_you"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        for l in left {
            s.push_str(&format!("\n  · {l}"));
        }
    } else {
        s.push_str(". /computer-use turns it back on");
    }
    s
}

/// Run it on a thread, its output dropped (stop, resume, drop).
pub(crate) fn fire(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let _ = command(&a).stdout(Stdio::null()).status();
    });
}

/// `open <args>` (an app, a URL), detached.
fn open(args: &[String]) {
    if bise_home::env::test_setting("BISE_COMPUTER_USE").is_some() {
        // the tests' fake: never the user's apps; the fake logs it
        let mut a = vec!["open".to_string()];
        a.extend(args.iter().cloned());
        let a: Vec<&str> = a.iter().map(String::as_str).collect();
        fire(&a);
        return;
    }
    let _ = Command::new("open").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}

// ---- the live state (C6 state.json) ----

/// One agent in state.json.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Driver {
    /// "Chrome", "TextEdit"; None: it holds nothing now
    pub(crate) driving: Option<String>,
    /// "amazon.fr"
    pub(crate) place: Option<String>,
    pub(crate) paused: bool,
    pub(crate) stopped: bool,
}

/// The agents of state.json, by name.
pub(crate) type Drivers = BTreeMap<String, Driver>;

pub(crate) fn parse_state(v: &Value) -> Drivers {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(String::from);
    let b = |x: &Value, k: &str| x.get(k).and_then(Value::as_bool).unwrap_or(false);
    v.get("agents")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(n, a)| (n.clone(), Driver { driving: s(a, "driving"), place: s(a, "where"), paused: b(a, "paused"), stopped: b(a, "stopped") }))
                .collect()
        })
        .unwrap_or_default()
}

fn state_file() -> PathBuf {
    bise_home::Home::from_env().run_dir().join("computer-use").join("state.json")
}

struct Cache {
    checked: Option<Instant>,
    mtime: Option<SystemTime>,
    drivers: Drivers,
}

thread_local! {
    static CACHE: std::cell::RefCell<Cache> = const { std::cell::RefCell::new(Cache { checked: None, mtime: None, drivers: BTreeMap::new() }) };
}

/// The agents that drive (or are held) now: state.json, read again when
/// its mtime moves, looked at at most every 250 ms.
pub(crate) fn drivers() -> Drivers {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.checked.is_none_or(|t| t.elapsed() >= Duration::from_millis(250)) {
            c.checked = Some(Instant::now());
            let f = state_file();
            let m = std::fs::metadata(&f).and_then(|m| m.modified()).ok();
            if m != c.mtime {
                c.mtime = m;
                c.drivers = std::fs::read_to_string(&f)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .map(|v| parse_state(&v))
                    .unwrap_or_default();
            }
        }
        c.drivers.clone()
    })
}

/// What `agent` drives now (not paused, not stopped).
pub(crate) fn driving(agent: &str) -> Option<Driver> {
    drivers().remove(agent).filter(|d| d.driving.is_some() && !d.stopped)
}

/// `agent` waits for the user to give its tab or app back.
pub(crate) fn paused(agent: &str) -> bool {
    drivers().get(agent).is_some_and(|d| d.paused && !d.stopped)
}

/// The mark: `↖`, `C` in ASCII (designer m_3551).
pub(crate) fn mark() -> &'static str {
    if theme::ascii_mode() {
        "C"
    } else {
        "↖"
    }
}

/// `app` in 8 columns at most, cut with `…` (`Calcula…`, m_3896).
pub(crate) fn short_app(app: &str) -> String {
    cut(app, 8)
}

/// Stop `agent` (ctrl+c in its view, a click on `↖`, `/stop`): it lets
/// go until [`resume_if_stopped`].
pub(crate) fn stop(agent: &str) {
    fire(&["stop", agent]);
}

/// The user writes to `agent` again: a stopped one may start again
/// (C6, m_3893: his message is the go-ahead the stop asked for).
pub(crate) fn resume_if_stopped(agent: &str) {
    if drivers().get(agent).is_some_and(|d| d.stopped || d.paused) {
        fire(&["resume", agent]);
    }
}

/// `? you took the wheel · ⏎ give it back`: the user gives it back.
pub(crate) fn give_back(agent: &str) {
    fire(&["resume", agent]);
}

// ---- the tool rows ----

/// A computer call's user-facing line from its result: `summary`, else
/// `error.summary` (C1; the MCP text puts it first, so the runtime's
/// 200-character preview holds it). `(ok, line)`.
pub(crate) fn summary_of(preview: &str) -> Option<(bool, String)> {
    let p = preview.trim_start();
    let (ok, rest) = match p.strip_prefix("{\"error\":{\"summary\":") {
        Some(r) => (false, r),
        None => (true, p.strip_prefix("{\"summary\":")?),
    };
    // a JSON string, maybe cut by the preview: read up to its closing quote
    let mut out = String::new();
    let mut chars = rest.strip_prefix('"')?.chars();
    loop {
        match chars.next()? {
            '"' => break,
            '\\' => match chars.next()? {
                'n' | 't' => out.push(' '),
                'u' => {
                    let h: String = chars.by_ref().take(4).collect();
                    out.push(u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).unwrap_or('?'));
                }
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    (!out.trim().is_empty()).then(|| (ok, out.trim().to_string()))
}

/// How a computer call's row reads (designer m_3774).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Did {
    /// `↖ clicked "Add to cart" · amazon.fr`
    Done,
    /// `✗ couldn't find "Email"` (err)
    Failed,
    /// the user paused or stopped it: a dim line, no `✗`
    Held,
}

/// The row of a `computer.*` sub-call that says what it did (an action
/// with a `summary`); None: a quiet call (`snapshot`, `tabs`…), drawn
/// inside its TypeScript box as any sub-call.
pub(crate) fn sub_row(name: &str, preview: &str) -> Option<(Did, String)> {
    if !name.starts_with("computer.") {
        return None;
    }
    let (ok, line) = summary_of(preview)?;
    let held = ["\"code\":\"paused\"", "\"code\":\"stopped\""].iter().any(|c| preview.contains(c));
    Some((
        match (ok, held) {
            (true, _) => Did::Done,
            (false, true) => Did::Held,
            (false, false) => Did::Failed,
        },
        line,
    ))
}

// ---- the screen ----

/// A row's state glyph (designer m_3551).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum St {
    Done,
    Waits,
    Checking,
    Failed,
    NotYet,
}

/// One row as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) st: St,
    pub(crate) detail: String,
    /// the selected row's dim lines under it
    pub(crate) help: Vec<String>,
    /// what ⏎ does, as the key bar says it; None: nothing
    pub(crate) action: Option<String>,
    /// the setup-check fix code
    pub(crate) fix: Option<String>,
    /// under "for apps"
    pub(crate) apps: bool,
}

/// What runs now, started from this screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Busy {
    /// the live test runs
    pub(crate) live_test: bool,
    /// Screen Recording granted: the helper reopens (since when)
    pub(crate) reopening: Option<Instant>,
}

/// The screen's state while open.
pub(crate) struct Screen {
    pub(crate) sel: usize,
    /// the last setup-check (the poll thread writes it)
    check: Arc<Mutex<Option<Value>>>,
    busy: Arc<Mutex<Busy>>,
    /// what went wrong with a fix (error color, under the rows)
    said: Arc<Mutex<Option<String>>>,
    /// the key bar's flash: `opened · path copied` (3 s)
    flash: Option<(String, Instant)>,
    /// the live test ran by itself once (never again on its own)
    auto_ran: bool,
    /// the rows of the last frame (the cursor follows a row that turns ✓)
    last: Vec<Row>,
    stop: Arc<AtomicBool>,
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Screen {
    /// Open: the poll starts (one setup-check a second until closed).
    pub(crate) fn open() -> Screen {
        let s = Screen {
            sel: 0,
            check: Arc::new(Mutex::new(None)),
            busy: Arc::default(),
            said: Arc::default(),
            flash: None,
            auto_ran: false,
            last: Vec::new(),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let (check, stop) = (s.check.clone(), s.stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let t0 = Instant::now();
                if let Some(v) = json_of(&["setup-check", "--json"]) {
                    *lock(&check) = Some(v);
                }
                while !stop.load(Ordering::SeqCst) && t0.elapsed() < Duration::from_secs(1) {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        });
        s
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- the words (pure) ----

fn st_of(s: &str) -> St {
    match s {
        "done" => St::Done,
        "waits" => St::Waits,
        "checking" => St::Checking,
        "failed" => St::Failed,
        _ => St::NotYet,
    }
}

fn str_at<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The extension's folder as the user reads it (`~/...`).
fn tilde(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// A failure line with contractions (m_3904): `did not` → `didn't`.
fn contract(t: &str) -> String {
    [("did not", "didn't"), ("does not", "doesn't"), ("is not", "isn't"), ("cannot", "can't"), ("could not", "couldn't")]
        .iter()
        .fold(t.to_string(), |acc, (a, b)| acc.replace(a, b))
}

/// The rows from a setup-check and what runs now.
pub(crate) fn rows(check: &Value, busy: &Busy) -> Vec<Row> {
    let browser = str_at(check, "browser");
    let browser = if browser.is_empty() { "Chrome" } else { browser };
    // brand names keep their case in the label column (m_3904)
    let low = browser.to_string();
    let min = check.get("min_major").and_then(Value::as_u64).unwrap_or(116);
    let mut out: Vec<Row> = Vec::new();
    let Some(list) = check.get("rows").and_then(Value::as_array) else { return out };
    // no helper app: no "for apps" rows at all (m_3896)
    let helper = list.iter().any(|r| str_at(r, "id") == "accessibility" && str_at(r, "fix") != "install_helper");
    for r in list {
        let id = str_at(r, "id");
        let mut st = st_of(str_at(r, "state"));
        let given = str_at(r, "detail").to_string();
        let fix = r.get("fix").and_then(Value::as_str).map(String::from);
        let prev = out.last().map(|p: &Row| p.label.clone()).unwrap_or_default();
        let after = || if prev.is_empty() { String::new() } else { format!("after {prev}") };
        let o = |s: &str| Some(s.to_string());
        let (label, detail, help, action, apps): (String, String, Vec<String>, Option<String>, bool) = match id {
            "browser" => {
                let (d, h, a) = match (st, fix.as_deref()) {
                    (St::Done, _) => (given, vec![], None),
                    (_, Some("install_browser")) => (format!("{browser} isn't installed"), vec![format!("install it, i'll wait")], o("open its download page")),
                    (_, Some("update_browser")) => (format!("{given} is too old. bise needs {min}"), vec![], o("open chrome://settings/help")),
                    (_, Some("open_browser")) => (format!("{browser} isn't open. open it, i'll wait"), vec![], Some(format!("open {browser}"))),
                    _ => (given, vec![], None),
                };
                (low.clone(), d, h, a, false)
            }
            "extension" => {
                let label = format!("{low} extension");
                let (d, h, a) = match (st, fix.as_deref()) {
                    (St::Done, _) => (given, vec![], None),
                    (St::NotYet, _) => (after(), vec![], None),
                    (_, Some("add_extension")) => {
                        let dir = check.get("extension").map(|e| str_at(e, "dir")).filter(|d| !d.is_empty()).map(tilde);
                        let mut h = vec!["1  ⏎ opens chrome://extensions · turn on Developer mode, top right".to_string()];
                        h.push(match &dir {
                            Some(_) => "2  Load unpacked · cmd+shift+g, cmd+v (i copied the folder's path), Open".to_string(),
                            None => "2  Load unpacked, then pick bise's extension folder".to_string(),
                        });
                        h.push("3  i'll see it here, no key needed".to_string());
                        (format!("add bise to {browser}"), h, o("open chrome://extensions"))
                    }
                    (_, Some("repair")) => ("the extension can't reach bise".to_string(), vec!["⏎ rewrites the native host".to_string()], o("repair it")),
                    // a newer build is in its folder and Chrome didn't reload it
                    (_, Some("reload_extension")) => (
                        "an update is ready".to_string(),
                        vec![format!("⏎ opens chrome://extensions · click ↻ on bise computer use"), "i'll see it here".to_string()],
                        o("open chrome://extensions"),
                    ),
                    _ => (given, vec![], None),
                };
                (label, d, h, a, false)
            }
            "live_test" => {
                if busy.live_test {
                    st = St::Checking;
                }
                let (d, a) = match st {
                    St::Checking => (format!("opening a tab in the background{}", theme::ellipsis()), None),
                    St::Done => ("opened a tab, clicked a button. all set".to_string(), None),
                    St::Failed => (contract(&given), o("try again")),
                    St::Waits => ("opens a tab in the background and clicks".to_string(), o("run it")),
                    St::NotYet => ("last".to_string(), None),
                };
                ("live test".to_string(), d, vec![], a, false)
            }
            "accessibility" if helper => {
                let (d, h, a) = match st {
                    St::Done => ("bise can click and type in apps".to_string(), vec![], None),
                    St::Waits | St::Failed => (
                        "bise can click and type in apps".to_string(),
                        vec!["turn on bise Computer Use in the list. i'll see it".to_string()],
                        o("open System Settings"),
                    ),
                    _ => (format!("asking bise Computer Use{}", theme::ellipsis()), vec![], None),
                };
                ("accessibility".to_string(), d, h, a, true)
            }
            "screen_recording" if helper => {
                if busy.reopening.is_some() && st != St::Done {
                    st = St::Checking;
                }
                let (d, h, a) = match st {
                    St::Done => ("bise can see app windows".to_string(), vec![], None),
                    St::Checking if busy.reopening.is_some() => (format!("bise Computer Use reopens{}", theme::ellipsis()), vec![], None),
                    St::Waits | St::Failed => (
                        "optional · for screenshots of apps".to_string(),
                        vec!["turn on bise Computer Use in the list. macOS reopens it, i'll wait".to_string()],
                        o("open System Settings"),
                    ),
                    _ => (format!("asking bise Computer Use{}", theme::ellipsis()), vec![], None),
                };
                ("screen recording".to_string(), d, h, a, true)
            }
            _ => continue,
        };
        out.push(Row { id: id.to_string(), label, st, detail, help, action, fix, apps });
    }
    out
}

/// The browser, extension and live test are all `✓`.
pub(crate) fn ready(rows: &[Row]) -> bool {
    let web: Vec<&Row> = rows.iter().filter(|r| !r.apps).collect();
    !web.is_empty() && web.iter().all(|r| r.st == St::Done)
}

/// `↖ api-v2 drives Chrome`, `↖ api-v2 and perf drive Chrome`, `↖ 3
/// agents drive Chrome` (m_3896); None: nobody drives.
pub(crate) fn drivers_line(d: &Drivers) -> Option<String> {
    let live: Vec<(&String, &Driver)> = d.iter().filter(|(_, x)| x.driving.is_some() && !x.stopped).collect();
    let mut apps: Vec<&str> = live.iter().filter_map(|(_, x)| x.driving.as_deref()).collect();
    apps.dedup();
    let what = apps.join(" and ");
    let m = mark();
    match live.len() {
        0 => None,
        1 => Some(format!("{m} {} drives {what}", live[0].0)),
        2 => Some(format!("{m} {} and {} drive {what}", live[0].0, live[1].0)),
        n => Some(format!("{m} {n} agents drive {what}")),
    }
}

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

fn s(t: impl Into<String>, c: ratatui::style::Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c))
}

fn pad(t: &str, w: usize) -> String {
    format!("{t}{}", " ".repeat(w.saturating_sub(t.width())))
}

/// `t` cut to `w` columns, with `…`.
fn cut(t: &str, w: usize) -> String {
    if t.width() <= w {
        return t.to_string();
    }
    let e = theme::ellipsis();
    let mut out = String::new();
    for c in t.chars() {
        if out.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) + e.width() > w {
            break;
        }
        out.push(c);
    }
    out.push_str(e);
    out
}

fn glyph_of(st: St) -> Span<'static> {
    match st {
        St::Done => s(theme::done_glyph(), theme::accent()),
        St::Waits => s(theme::glyph(theme::G_NEEDS_YOU), theme::accent()),
        St::Checking => s(theme::glyph(theme::G_WORKING), theme::dim()),
        St::Failed => s(theme::glyph(theme::G_FAILED), theme::error()),
        St::NotYet => s(theme::glyph(theme::G_STARTING), theme::faint()),
    }
}

/// The label column: `screen recording` and 4 spaces (the mock's 20).
const LABEL_W: usize = 20;

/// What the screen says beside the rows.
#[derive(Clone, Debug, Default)]
pub(crate) struct Around {
    /// the agents' provider, `Anthropic`
    pub(crate) provider: String,
    /// the approvals mode: `yolo`, `auto`, "" unknown
    pub(crate) mode: String,
    pub(crate) drivers: Option<String>,
    pub(crate) said: Option<String>,
    pub(crate) flash: Option<String>,
    /// the helper app is installed: the title names apps
    pub(crate) apps: bool,
}

/// The screen's lines for a column `w` wide.
pub(crate) fn lines(rows: &[Row], sel: usize, a: &Around, w: usize) -> Vec<Line<'static>> {
    let bold = Style::default().fg(theme::text()).add_modifier(Modifier::BOLD);
    let what = if a.apps { "agents can drive your apps and Chrome" } else { "agents can drive Chrome" };
    let mut v = vec![
        Line::from(vec![Span::raw(" "), Span::styled("computer use", bold), s(format!(" {} {what}", dot()), theme::dim())]),
        Line::raw(""),
    ];
    if rows.is_empty() {
        v.push(Line::from(s(format!(" {} checking{}", theme::glyph(theme::G_WORKING), theme::ellipsis()), theme::dim())));
    }
    let detail_w = w.saturating_sub(3 + LABEL_W);
    let mut apps_label = false;
    for (i, r) in rows.iter().enumerate() {
        if r.apps && !apps_label {
            apps_label = true;
            v.push(Line::raw(""));
            v.push(Line::from(s(format!("   {}", "for apps"), theme::faint())));
        }
        let on = i == sel;
        let label = pad(&r.label, LABEL_W);
        let label = if on { Span::styled(label, bold) } else { s(label, theme::text()) };
        let dc = match r.st {
            St::Failed => theme::error(),
            St::NotYet => theme::faint(),
            St::Done => theme::text(),
            _ => theme::dim(),
        };
        v.push(Line::from(vec![Span::raw(" "), glyph_of(r.st), Span::raw(" "), label, s(cut(&r.detail, detail_w), dc)]));
        if on {
            for h in &r.help {
                v.push(Line::from(s(format!("{}{}", " ".repeat(3 + LABEL_W), cut(h, detail_w)), theme::dim())));
            }
        }
    }
    if ready(rows) {
        // only what's true (m_3904): apps too once their rows are ✓
        let apps: Vec<&Row> = rows.iter().filter(|r| r.apps).collect();
        let said = if !apps.is_empty() && apps.iter().all(|r| r.st == St::Done) {
            " ready. ask any agent to use Chrome or an app.".to_string()
        } else {
            format!(" {} is ready. ask any agent to use it.", rows.first().map_or("Chrome", |r| r.label.as_str()))
        };
        v.push(Line::raw(""));
        v.push(Line::from(vec![Span::raw(" "), s(theme::done_glyph(), theme::accent()), s(cut(&said, w.saturating_sub(2)), theme::text())]));
    }
    if let Some(e) = &a.said {
        v.push(Line::raw(""));
        v.push(Line::from(s(cut(&format!(" {e}"), w), theme::error())));
    }
    v.push(Line::raw(""));
    if !a.provider.is_empty() {
        v.push(Line::from(s(cut(&format!(" what the agents see goes only to their model: {}.", a.provider), w), theme::dim())));
    }
    let st = if theme::ascii_mode() { "shift+tab" } else { "⇧⇥" };
    match a.mode.as_str() {
        "yolo" => v.push(Line::from(s(cut(&format!(" in yolo, agents act without asking, purchases included. {st} for auto."), w), theme::dim()))),
        // honest until cu-approvals (ship plan §6): no card yet, the skill
        // tells agents to ask in their thread; passwords are always refused
        "auto" => v.push(Line::from(s(cut(" in auto, agents ask you in their thread before buying, sending or posting; they never type passwords.", w), theme::dim()))),
        _ => {}
    }
    if let Some(d) = &a.drivers {
        v.push(Line::from(vec![Span::raw(" "), s(cut(d, w.saturating_sub(1)), theme::accent())]));
    }
    v.push(Line::raw(""));
    if let Some(f) = &a.flash {
        v.push(Line::from(vec![Span::raw(" "), s(theme::done_glyph(), theme::accent()), s(format!(" {f}"), theme::text())]));
        return v;
    }
    let mut keys: Vec<(String, String)> = Vec::new();
    if let Some(act) = rows.get(sel).and_then(|r| r.action.clone()) {
        keys.push(("⏎".into(), act));
    }
    if a.drivers.is_some() {
        keys.push(("x".into(), "stop all".into()));
    }
    keys.push(("↑↓".into(), "step".into()));
    keys.push(("esc".into(), "later".into()));
    let mut spans = vec![Span::raw(" ")];
    for (i, (k, t)) in keys.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(s(k, theme::text()));
        spans.push(s(format!(" {t}"), theme::dim()));
    }
    v.push(Line::from(spans));
    v
}

/// The cursor after a poll: a selected row that turned `✓` moves it to
/// the next row that isn't; at first, the first row that isn't.
pub(crate) fn follow(before: &[Row], now: &[Row], sel: usize) -> usize {
    let open = |from: usize| now.iter().enumerate().skip(from).find(|(_, r)| r.st != St::Done).map(|(i, _)| i);
    if now.is_empty() {
        return 0;
    }
    if before.is_empty() {
        return open(0).unwrap_or(0);
    }
    let was = before.get(sel);
    let is = now.get(sel);
    match (was, is) {
        (Some(b), Some(n)) if b.id == n.id && b.st != St::Done && n.st == St::Done => open(sel + 1).or_else(|| open(0)).unwrap_or(sel),
        _ => sel.min(now.len() - 1),
    }
}

// ---- the actions ----

/// ⏎ on the selected row.
fn act(sc: &mut Screen, check: &Value, row: &Row) {
    *lock(&sc.said) = None;
    let pick = check
        .get("browsers")
        .and_then(Value::as_array)
        .and_then(|l| l.iter().find(|b| str_at(b, "name") == str_at(check, "browser")))
        .cloned()
        .unwrap_or(Value::Null);
    let app = str_at(&pick, "app").to_string();
    let with_app = |url: &str| -> Vec<String> {
        if app.is_empty() {
            vec![url.to_string()]
        } else {
            vec!["-a".into(), app.clone(), url.to_string()]
        }
    };
    match row.fix.as_deref() {
        Some("install_browser") => open(&["https://www.google.com/chrome/".to_string()]),
        Some("open_browser") if !app.is_empty() => open(&["-a".into(), app.clone()]),
        Some("update_browser") => open(&with_app("chrome://settings/help")),
        Some("add_extension") => {
            let dir = check.get("extension").map(|e| str_at(e, "dir").to_string()).unwrap_or_default();
            let copied = !dir.is_empty() && crate::clipboard::copy(&dir);
            open(&with_app("chrome://extensions"));
            sc.flash = Some((if copied { format!("opened {} path copied", dot()) } else { "opened".into() }, Instant::now()));
        }
        Some("reload_extension") => open(&with_app("chrome://extensions")),
        Some("repair") => {
            let said = sc.said.clone();
            std::thread::spawn(move || {
                let ok = command(&["repair"]).stdout(Stdio::null()).status().is_ok_and(|s| s.success());
                if !ok {
                    *lock(&said) = Some("repair failed: bise computer-use repair says why".into());
                }
            });
        }
        Some("run_live_test") => run_live_test(sc),
        Some(f @ ("request_accessibility" | "request_screen_recording")) => {
            let what = f.trim_start_matches("request_").to_string();
            let (busy, said) = (sc.busy.clone(), sc.said.clone());
            std::thread::spawn(move || {
                let v = json_of(&["request", &what]);
                if what == "screen_recording" && v.as_ref().is_some_and(|v| v["relaunching"] == true) {
                    lock(&busy).reopening = Some(Instant::now());
                } else if v.is_none() {
                    *lock(&said) = Some("bise Computer Use didn't answer. try again".into());
                }
            });
        }
        _ => {}
    }
}

fn run_live_test(sc: &mut Screen) {
    let busy = sc.busy.clone();
    {
        let mut b = lock(&busy);
        if b.live_test {
            return;
        }
        b.live_test = true;
    }
    std::thread::spawn(move || {
        let _ = command(&["live-test", "--json"]).stdout(Stdio::null()).status();
        lock(&busy).live_test = false;
    });
}

/// The rows now (and the cursor and the live test that runs by itself).
fn refresh(sc: &mut Screen) -> (Value, Vec<Row>) {
    let check = lock(&sc.check).clone().unwrap_or(Value::Null);
    {
        // the reopened helper said yes, or it never came back (a minute)
        let mut b = lock(&sc.busy);
        let granted = check.get("rows").and_then(Value::as_array).is_some_and(|l| {
            l.iter().any(|r| str_at(r, "id") == "screen_recording" && str_at(r, "state") == "done")
        });
        if b.reopening.is_some_and(|t| granted || t.elapsed() > Duration::from_secs(60)) {
            b.reopening = None;
        }
    }
    let busy = lock(&sc.busy).clone();
    let now = rows(&check, &busy);
    sc.sel = follow(&sc.last, &now, sc.sel);
    // the live test runs by itself once, as soon as it can (m_3896)
    if !sc.auto_ran && now.iter().any(|r| r.id == "live_test" && r.st == St::Waits) {
        sc.auto_ran = true;
        run_live_test(sc);
    }
    // the extension is in: the step-2 flash has done its job (m_3904)
    if now.iter().any(|r| r.id == "extension" && r.st == St::Done) {
        sc.flash = None;
    }
    sc.last = now.clone();
    (check, now)
}

/// Keys while the screen is open: it takes them all.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(sc) = app.computer_use.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let (check, now) = refresh(sc);
    let n = now.len();
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Esc => app.computer_use = None,
        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => app.computer_use = None,
        KeyCode::Up | KeyCode::Char('k') => sc.sel = sc.sel.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => sc.sel = (sc.sel + 1).min(n.saturating_sub(1)),
        KeyCode::Enter => {
            if let Some(r) = now.get(sc.sel).cloned() {
                act(sc, &check, &r);
            }
        }
        KeyCode::Char('x') if drivers_line(&drivers()).is_some() => fire(&["stop", "--all"]),
        KeyCode::BackTab => crate::sb::toggle_approvals(app),
        _ => {}
    }
    true
}

/// The mouse while open: the wheel moves the cursor, the rest is
/// swallowed.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let Some(sc) = app.computer_use.as_mut() else { return false };
    let n = sc.last.len();
    match m.kind {
        MouseEventKind::ScrollUp => sc.sel = sc.sel.saturating_sub(1),
        MouseEventKind::ScrollDown => sc.sel = (sc.sel + 1).min(n.saturating_sub(1)),
        _ => {}
    }
    true
}

/// The agents' provider for the privacy line: the provider of the
/// agents' model, by its name for people.
fn provider() -> String {
    let m = crate::models::model_for(false);
    match m.split_once('/') {
        Some((id, _)) if !id.is_empty() => crate::models::provider_name(id, ""),
        _ => String::new(),
    }
}

/// The screen, over the whole frame, when open: one column (96 at most:
/// the load-unpacked steps hold on one line each), its block at 2/5 of
/// the free rows, like `/approvals`.
pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let mode = app.sb.approvals.mode.clone();
    let Some(sc) = app.computer_use.as_mut() else { return };
    let (_, now) = refresh(sc);
    if sc.flash.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(3)) {
        sc.flash = None;
    }
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 24 || full.height < 6 {
        return;
    }
    let around = Around {
        provider: provider(),
        mode,
        drivers: drivers_line(&drivers()),
        said: lock(&sc.said).clone(),
        flash: sc.flash.as_ref().map(|(f, _)| f.clone()),
        apps: now.iter().any(|r| r.apps),
    };
    let w = full.width.saturating_sub(4).min(96);
    let lines = lines(&now, sc.sel, &around, w as usize);
    let h = (lines.len() as u16).min(full.height);
    let y = full.y + (full.height - h) * 2 / 5;
    let area = Rect { x: full.x + (full.width - w) / 2, y, width: w, height: h };
    frame.render_widget(Paragraph::new(lines), area);
    crate::textlayer::text(area);
}

#[cfg(test)]
#[path = "computer_use_tests.rs"]
mod tests;
