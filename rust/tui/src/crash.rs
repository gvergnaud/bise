//! Crashes never leave a broken terminal and always leave a trace.
//!
//! [`install`] sets a panic hook (once per process) that:
//! - writes the panic (thread, message, location, backtrace) to a log
//!   file: `$BEND_DEBUG_DIR` (the harness session), else bise's
//!   `crashes/` (`bise_home`; unit tests: always the temp dir);
//! - on the UI thread, outside a [`guarded`] handler: restores the
//!   terminal (kitty keyboard flags, bracketed paste, mouse capture,
//!   alternate screen, cursor, raw mode) BEFORE the report is printed,
//!   then lets the previous hooks run (the harness debug log, the
//!   default report);
//! - inside a [`guarded`] handler, or on a background thread while the
//!   UI runs: only logs (a report on stderr would scribble over the
//!   screen). The run loop shows the error in the feed ([`take_notes`],
//!   [`guarded`]'s `Err`): visible, never swallowed.
//!
//! [`guarded`] is the run loop's last line of defence around ONE key,
//! paste, mouse event, hub line or frame: the bug is reported in the
//! feed and logged, and the UI keeps running. It is not a fix: every
//! panic found is fixed at its cause (total functions, clamped ranges).

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, Once};
use std::thread::ThreadId;

/// The UI thread while a TUI owns the terminal (raw mode, alternate
/// screen); None otherwise.
static UI_THREAD: Mutex<Option<ThreadId>> = Mutex::new(None);
/// Background-thread panics while the UI runs, for the feed.
static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Log files written by this process (capped: a crash loop never fills
/// the disk).
static LOGS: AtomicUsize = AtomicUsize::new(0);
const MAX_LOGS: usize = 20;
/// Tests: never touch the real terminal.
static DRY_RUN: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Inside `guarded` on this thread.
    static GUARDED: Cell<bool> = const { Cell::new(false) };
    /// The last panic caught on this thread (for `guarded`).
    static LAST: RefCell<Option<Crash>> = const { RefCell::new(None) };
}

/// A panic, as reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Crash {
    pub(crate) message: String,
    pub(crate) location: String,
    /// the log file, when one was written
    pub(crate) log: Option<PathBuf>,
}

impl Crash {
    /// One line for the feed.
    pub(crate) fn line(&self, what: &str) -> String {
        let log = self.log.as_ref().map_or("no log written".to_string(), |p| format!("details: {}", p.display()));
        format!("internal error while handling {what} (a bug, please report): {} at {} — the UI kept running; {log}", self.message, self.location)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // a panic while holding it must not poison the crash path itself
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The panic message of a payload (`&str` or `String`, else a type note).
pub(crate) fn payload_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(non-string panic payload)".into())
}

fn log_dir() -> PathBuf {
    // unit tests: a failing test never lands in the user's crash logs
    if cfg!(test) {
        return std::env::temp_dir().join("bend-tui-test-crashes");
    }
    if let Some(d) = std::env::var_os("BEND_DEBUG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    bise_home::Home::from_env().crashes_dir()
}

/// The text of a log file.
pub(crate) fn report_text(thread: &str, crash: &Crash, context: &str, backtrace: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "bend-tui panic\ntime: {secs} (unix)\npid: {}\nversion: {}\nthread: {thread}\ncontext: {context}\nmessage: {}\nlocation: {}\n\nbacktrace:\n{backtrace}\n",
        std::process::id(),
        env!("CARGO_PKG_VERSION"),
        crash.message,
        crash.location,
    )
}

/// Writes the log file; None when capped or not writable.
fn write_log(text: &str) -> Option<PathBuf> {
    let n = LOGS.fetch_add(1, Ordering::SeqCst);
    if n >= MAX_LOGS {
        return None;
    }
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("tui-panic-{secs}-{}-{n}.log", std::process::id()));
    let mut f = std::fs::File::create(&path).ok()?;
    f.write_all(text.as_bytes()).ok()?;
    Some(path)
}

/// Gives the terminal back: every mode the TUI turned on, off. Safe to
/// call twice, and on a terminal where some were never enabled.
pub(crate) fn restore_terminal() {
    if DRY_RUN.load(Ordering::SeqCst) {
        return;
    }
    use crossterm::event::{DisableBracketedPaste, DisableMouseCapture, PopKeyboardEnhancementFlags};
    use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
    // BISE-92: the terminal's own background back first
    crate::theme_detect::restore_terminal_bg();
    // term-title: the tab title you had
    crate::termtitle::restore();
    let mut out = std::io::stdout();
    // BISE-272: the default mouse pointer, if bise changed it
    crate::pointer::restore(&mut out);
    let _ = crossterm::execute!(out, PopKeyboardEnhancementFlags);
    let _ = crossterm::execute!(out, DisableBracketedPaste);
    let _ = crossterm::execute!(out, DisableMouseCapture);
    let _ = crossterm::execute!(out, crossterm::event::DisableFocusChange);
    let _ = crossterm::execute!(out, LeaveAlternateScreen);
    let _ = crossterm::execute!(out, crossterm::cursor::Show);
    let _ = disable_raw_mode();
    let _ = out.flush();
}

/// Installs the hook (once per process; later calls do nothing). The
/// harness calls it first thing in every mode (TUI, hub daemon, CLI):
/// any panic of the process leaves a log with its backtrace.
pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let crash = Crash {
                message: payload_text(info.payload()),
                location: info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default(),
                log: None,
            };
            let me = std::thread::current();
            let thread = me.name().unwrap_or("unnamed").to_string();
            let guarded = GUARDED.with(|g| g.get());
            let ui = *lock(&UI_THREAD);
            let context = match (guarded, ui) {
                (true, _) => "caught by the run loop (the UI kept running)",
                (false, Some(t)) if t == me.id() => "UI thread: the terminal was restored, the UI exits",
                (false, Some(_)) => "background thread while the UI runs",
                (false, None) => "no UI running",
            };
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let log = write_log(&report_text(&thread, &crash, context, &backtrace));
            let crash = Crash { log, ..crash };
            if guarded {
                LAST.with(|l| *l.borrow_mut() = Some(crash));
                return;
            }
            match ui {
                Some(t) if t == me.id() => {
                    *lock(&UI_THREAD) = None;
                    restore_terminal();
                    if let Some(p) = &crash.log {
                        eprintln!("bend-tui crashed — the panic and its backtrace: {}", p.display());
                    }
                    previous(info);
                }
                Some(_) => lock(&NOTES).push(Crash::line(&crash, &format!("thread {thread}"))),
                None => previous(info),
            }
        }));
    });
}

/// This thread owns the terminal from now on (`true`), or gave it back.
pub(crate) fn set_ui_thread(active: bool) {
    *lock(&UI_THREAD) = active.then(|| std::thread::current().id());
}

/// A line for the feed from outside the UI flow (a draft not saved,
/// sb/drafts.rs): shown with the background panics.
pub(crate) fn note(line: String) {
    lock(&NOTES).push(line);
}

/// The background panics since the last call, as feed lines.
pub(crate) fn take_notes() -> Vec<String> {
    std::mem::take(&mut *lock(&NOTES))
}

/// Runs `f`; a panic inside it is logged (not printed) and returned.
pub(crate) fn guarded<R>(f: impl FnOnce() -> R) -> Result<R, Crash> {
    let was = GUARDED.with(|g| g.replace(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    GUARDED.with(|g| g.set(was));
    r.map_err(|p| {
        LAST.with(|l| l.borrow_mut().take()).unwrap_or_else(|| Crash {
            // no hook installed (a test): the payload is all we have
            message: payload_text(p.as_ref()),
            location: String::new(),
            log: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guarded_panic_is_returned_logged_and_not_rethrown() {
        DRY_RUN.store(true, Ordering::SeqCst);
        install();
        let r = guarded(|| -> u8 {
            let v: Vec<u8> = Vec::new();
            let i = v.len() + 3; // an out-of-range index, on purpose
            v[i]
        });
        let c = r.expect_err("the panic comes back as an Err");
        assert!(c.message.contains("index out of bounds"), "{c:?}");
        assert!(c.location.contains("crash.rs"), "{c:?}");
        if let Some(p) = &c.log {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            assert!(text.contains("index out of bounds") && text.contains("backtrace:"), "{text}");
            let _ = std::fs::remove_file(p);
        }
        assert!(c.line("a key").contains("internal error while handling a key"));
        // the loop goes on: the next call works
        assert_eq!(guarded(|| 7), Ok(7));
    }

    #[test]
    fn report_text_carries_the_fields() {
        let c = Crash { message: "boom".into(), location: "x.rs:1:2".into(), log: None };
        let t = report_text("main", &c, "ctx", "BT");
        for want in ["thread: main", "context: ctx", "message: boom", "location: x.rs:1:2", "BT"] {
            assert!(t.contains(want), "{want} in {t}");
        }
    }

    #[test]
    fn payloads_of_both_kinds_read() {
        let a: Box<dyn std::any::Any + Send> = Box::new("s");
        let b: Box<dyn std::any::Any + Send> = Box::new(String::from("t"));
        let c: Box<dyn std::any::Any + Send> = Box::new(3u8);
        assert_eq!(payload_text(a.as_ref()), "s");
        assert_eq!(payload_text(b.as_ref()), "t");
        assert!(payload_text(c.as_ref()).contains("non-string"));
    }
}
