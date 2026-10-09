//! Random input never panics: seeded random key sequences (arrows,
//! Home/End, Backspace, Tab, Enter, Esc, `@`, `$`, `/`, `:`, multi-byte
//! chars), pastes, mouse events, hub lines and terminal sizes (1..200
//! columns, 1..60 rows) into the switchboard screen: the composer, the
//! popups, the feed, the panel, the cards, the help overlay. Every step
//! draws on a TestBackend. A failure prints the seed and the step:
//! `FUZZ_SEED=<seed> FUZZ_RUNS=1 cargo test -p bend-tui fuzz_` replays it.

use super::*;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use voice::fakes::{FakeRecorder, FakeTranscriber};
use voice::Voice;
use serde_json::json;

/// xorshift64*: deterministic, no dependency.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub(crate) fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
    pub(crate) fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

/// Characters that stress the byte/char/grapheme/width arithmetic.
pub(crate) const CHARS: &[char] = &[
    'a', 'b', 'z', 'Z', '0', '9', ' ', ' ', '@', '@', '$', '$', '/', '/', ':', '"', '\'', '.', '-', '_',
    '\\', '*', '`', '#', '\t', 'é', 'à', '\u{301}', '\u{200d}', '日', '本', '語', '👍', '🎉', '👩', '\u{fe0f}',
    '🇫', '🇷', 'ﬁ', '\u{0}', '\u{7f}', '\u{1b}', '\u{a0}', '®', '´', '¨', '^',
];

/// Texts that stress the same, as pastes or hub lines.
pub(crate) const TEXTS: &[&str] = &[
    "",
    "hello world",
    "@rust/tui/src/",
    "@\"docs/my notes/",
    "$sk",
    "/help",
    "/plugins",
    ":thumbsup:",
    ":+1",
    "line one\r\nline two\rthree\n",
    "👩‍👩‍👧‍👦🇫🇷🏳️‍🌈",
    "e\u{301}e\u{301}e\u{301}",
    "日本語のテキスト、とても長い行です。日本語のテキスト、とても長い行です。",
    "```rust\nfn main() { let x = s[a..b]; }\n```",
    "a\tb\tc\t\t",
    "\u{1b}[31mred\u{1b}[0m",
    "- item\n1. one\n# Title\n**bold** *it* `code` [link](http://x)",
    "word word word word word word word word word word word word word word word word word word word word word word word word word word",
];

fn texts(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => (0..rng.below(40)).map(|_| *rng.pick(CHARS)).collect(),
        1 => rng.pick(TEXTS).repeat(1 + rng.below(3)),
        _ => rng.pick(TEXTS).to_string(),
    }
}

fn key_of(rng: &mut Rng) -> KeyEvent {
    const CODES: &[KeyCode] = &[
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::F(1),
    ];
    let mods = [
        KeyModifiers::NONE,
        KeyModifiers::NONE,
        KeyModifiers::NONE,
        KeyModifiers::SHIFT,
        KeyModifiers::ALT,
        KeyModifiers::CONTROL,
        KeyModifiers::SUPER,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ];
    let m = *rng.pick(&mods);
    let code = if rng.chance(55) {
        let c = if m == KeyModifiers::CONTROL || m == KeyModifiers::ALT {
            // the control letters and Alt+digits: nav, cards, undo...
            *rng.pick(&"abcdefghiklmnopqrstuvwxyz0123456789.,-=".chars().collect::<Vec<_>>())
        } else {
            *rng.pick(CHARS)
        };
        KeyCode::Char(c)
    } else {
        *rng.pick(CODES)
    };
    KeyEvent::new(code, m)
}

fn mouse_of(rng: &mut Rng, w: u16, h: u16) -> MouseEvent {
    let kinds = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::Moved,
    ];
    MouseEvent {
        kind: *rng.pick(&kinds),
        // sometimes off the screen: a resize races the event
        column: rng.below(w as usize + 3) as u16,
        row: rng.below(h as usize + 3) as u16,
        modifiers: if rng.chance(20) { KeyModifiers::SHIFT } else { KeyModifiers::NONE },
    }
}

fn hub_line(rng: &mut Rng) -> String {
    let agent = *rng.pick(&["main", "t1", "notes-👍-agent"]);
    let t = texts(rng);
    let line = match rng.below(9) {
        0 => format!("  obs: assistant: {}", t.replace('\n', "\\n")),
        1 => format!("history you : {}", t),
        2 => format!("tool #{} run_typescript : {}", rng.below(5), t),
        3 => format!("tool_code #{} : {}", rng.below(5), t),
        4 => format!("tool_result #{} ok : {}", rng.below(5), t),
        5 => "  obs: turn_started".into(),
        6 => "--- idle".into(),
        7 => format!("  obs: assistant: <think>{t}</think>{t}"),
        _ => t.clone(),
    };
    match rng.below(11) {
        0 => json!({"ev": "notice", "text": t}).to_string(),
        // BISE-235: a release's plan, steps and result
        4 => json!({"ev": "release", "state": *rng.pick(&["plan", "step", "running", "done", "failed", "error"]),
            "tag": t, "text": t, "commits": [[t, t]], "count": rng.below(30), "tail": [t], "elapsed": rng.below(5000)})
        .to_string(),
        1 => json!({"jsonrpc": "2.0", "method": "confirm/ask", "params": {"project": "p", "id": rng.below(3), "text": t}}).to_string(),
        2 => json!({"ev": "focus", "focus": agent}).to_string(),
        // hub/agents or hub/cards, as the hub sends them typed (P4c-4b)
        3 => {
            let mut t1 = crate::sb::hub_reads::rows_for_tests::agent("t1", "working", &t);
            t1.note = t.clone();
            let [agents, cards] = crate::sb::hub_reads::rows_for_tests::lines(
                vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", &t), t1],
                vec![crate::sb::hub_reads::rows_for_tests::card(1, "question", "t1", &t), crate::sb::hub_reads::rows_for_tests::card(2, "report", "main", &t)],
            );
            if rng.below(2) == 0 { agents } else { cards }
        }
        // a thread's entry as the hub sends it (P4d): the fold of the
        // line at a pos that may be known (a changed entry replaces)
        _ => sb::entries_for_tests::entry_note(agent, rng.below(40) as u64 + 1, &line),
    }
}

/// The line the key would hand to `handle_input`: `/voice` saves the
/// user's settings file, the fuzz never sends it.
fn would_toggle_voice(app: &App, k: &KeyEvent) -> bool {
    if !matches!(k.code, KeyCode::Enter | KeyCode::Tab) {
        return false;
    }
    let items = commands::popup_items(app);
    let sel = items.get(app.popup_sel.min(items.len().saturating_sub(1)));
    app.ed.text.contains("/voice") || sel.is_some_and(|c| c.run.as_deref().is_some_and(|r| r.contains("/voice")) || c.fill.contains("/voice"))
}

fn fuzz_app() -> App {
    let mut app = sb::bench::test_app_drained();
    app.voice = Voice::new(true, Box::new(FakeRecorder::ok(true)), Box::new(FakeTranscriber::default()));
    sb::bench::set_workspace(&mut app, at_popup_tests::ws());
    sb::bench::add_agent(&mut app, "t1", "an objective");
    sb::bench::add_agent(&mut app, "notes-👍-agent", "an objective with émojis 🎉 ".repeat(5).as_str());
    sb::entries_for_tests::subscribed(&mut app, &["main", "t1", "notes-👍-agent"]);
    app
}

/// One seeded run of `steps` random events; panics carry the seed.
fn run_one(seed: u64, steps: usize) {
    let mut rng = Rng::new(seed);
    let mut app = fuzz_app();
    let (mut w, mut h) = (1 + rng.below(200) as u16, 1 + rng.below(60) as u16);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    for step in 0..steps {
        let what = rng.below(100);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match what {
                0..=64 => {
                    let k = key_of(&mut rng);
                    // the terminal pane spawns a real shell: not fuzzed
                    if !would_toggle_voice(&app, &k) && !term::is_toggle(&k) {
                        input::on_key(&mut app, &k);
                    }
                }
                65..=69 => {
                    let t = texts(&mut rng);
                    input::on_paste(&mut app, &t);
                }
                70..=79 => {
                    let m = mouse_of(&mut rng, w, h);
                    input::on_mouse(&mut app, &m, h);
                }
                80..=91 => {
                    let l = hub_line(&mut rng);
                    sb::dispatch(&mut app, &l);
                }
                92..=95 => {
                    // a resize, sometimes to the smallest sizes
                    w = if rng.chance(30) { 1 + rng.below(4) as u16 } else { 1 + rng.below(200) as u16 };
                    h = if rng.chance(30) { 1 + rng.below(4) as u16 } else { 1 + rng.below(60) as u16 };
                    term.backend_mut().resize(w, h);
                }
                _ => pump_voice(&mut app),
            }
            app.should_quit = false;
            term.draw(|f| sb::draw_sb(&mut app, f)).unwrap();
        }));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            panic!(
                "fuzz panic: seed {seed} step {step} ({w}x{h}) text {:?} cursor {}: {msg}",
                app.ed.text, app.ed.cursor
            );
        }
    }
}

fn env_num(k: &str, default: u64) -> u64 {
    std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Random sequences (FUZZ_RUNS, default 300, 60 events each; the full
/// gate runs FUZZ_RUNS=2000) from FUZZ_SEED (default: fixed, so CI is reproducible).
/// Run i goes to shard i % FUZZ_SHARDS: the shards are separate tests, so
/// the test runner spreads them over the cores (loop-speed).
fn fuzz_shard(shard: u64) {
    let runs = env_num("FUZZ_RUNS", 300);
    let base = env_num("FUZZ_SEED", 0x5eed);
    let steps = env_num("FUZZ_STEPS", 60) as usize;
    for i in (shard..runs).step_by(FUZZ_SHARDS as usize) {
        run_one(base.wrapping_add(i), steps);
    }
}

const FUZZ_SHARDS: u64 = 8;

macro_rules! fuzz_shards {
    ($($name:ident = $k:literal),*) => {$(
        #[test]
        fn $name() {
            fuzz_shard($k);
        }
    )*};
}

fuzz_shards!(
    fuzz_random_input_never_panics_0 = 0,
    fuzz_random_input_never_panics_1 = 1,
    fuzz_random_input_never_panics_2 = 2,
    fuzz_random_input_never_panics_3 = 3,
    fuzz_random_input_never_panics_4 = 4,
    fuzz_random_input_never_panics_5 = 5,
    fuzz_random_input_never_panics_6 = 6,
    fuzz_random_input_never_panics_7 = 7
);

// ---- one regression test per panic class fixed ----

/// A popup on a short terminal: the prompt near the top, the popup rect
/// (n + 2 rows above it) once hung below the 5-row buffer.
#[test]
fn popup_on_a_short_terminal_stays_in_the_buffer() {
    let mut app = fuzz_app();
    app.ed.set("/", 1);
    for h in 1..12u16 {
        for w in [1u16, 2, 5, 40, 111] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| sb::draw_sb(&mut app, f)).unwrap();
        }
    }
}

/// A 1..3-column feed: its 1-column margin put the text rect outside.
#[test]
fn feed_narrower_than_its_margins_draws() {
    let mut app = fuzz_app();
    sb::entries_for_tests::lines(&mut app, "main", &["  obs: assistant: hello 👍 world"]);
    for w in 1..6u16 {
        let mut term = Terminal::new(TestBackend::new(w, 20)).unwrap();
        term.draw(|f| sb::draw_sb(&mut app, f)).unwrap();
    }
}

/// Token splices with a stale start past the cursor, or a range past
/// the text: clamped, never sliced out of range.
#[test]
fn token_completions_are_total() {
    assert_eq!(skills::complete("ab", 5, 1, "x"), ("a$x b".to_string(), 4));
    assert_eq!(skills::complete("", 3, 9, "x"), ("$x ".to_string(), 3));
    assert_eq!(emoji::complete("ab", 5, 9, "👍"), ("ab👍".to_string(), 3));
    assert_eq!(emoji::complete("abc", 2, 1, "👍"), ("a👍bc".to_string(), 2));
}

/// The history changed under a browse (a stale index): an empty entry,
/// no index out of range.
#[test]
fn history_browse_is_total() {
    let mut ed = editor::Editor::default();
    let hist: Vec<String> = ["one", "two", "three"].map(String::from).to_vec();
    for _ in 0..3 {
        assert!(ed.history_up(&hist));
    }
    // the list emptied while browsing entry 2: entry 1 is gone
    assert!(ed.history_down(&[]));
    assert_eq!(ed.text, "");
    assert!(ed.history_down(&[])); // entry 0, gone too
    assert!(ed.history_down(&[])); // back to the draft
    assert!(!ed.history_down(&[]));
}

/// A composer hit on a zero-size area: the row clamp never inverts.
#[test]
fn composer_hit_on_an_empty_area_is_total() {
    let area = ComposerArea::default();
    assert_eq!(area.hit("", 0, 0, true), Some(0));
    assert!(area.hit("abc\ndef", 9, 9, true).is_some());
}

/// A feed selection copied after the feed changed under it (the cache
/// shorter than the events): rows are built, never indexed past.
#[test]
fn feed_rows_are_total() {
    let mut app = fuzz_app();
    sb::entries_for_tests::lines(&mut app, "main", &["  obs: assistant: a", "  obs: assistant: b 👍", "  obs: assistant: c"]);
    app.cache.clear();
    assert!(feed::ensure_rows(&app.events, &mut app.cache, 1, false, 20, 0) > 0);
    assert_eq!(feed::ensure_rows(&app.events, &mut app.cache, 99, false, 20, 0), 0);
    app.cache.clear();
    app.feed_sel = Some(feedsel::FeedSel { anchor: (0, 0, 0), head: (9, 9, 9) });
    assert!(input::feed_selection_text(&mut app).is_some());
}
