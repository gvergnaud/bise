//! `bise pty --cwd <dir> --cols C --rows R [--shell <path>]`: one shell in a
//! pseudo-terminal, for the desktop's terminal panel (bar T W25/V13/K26/M16;
//! architect m_10140). A child of Electron's main, never inside
//! ambient-core: a terminal's flood must not queue the core's events.
//!
//! It speaks bise-proto's [`bise_proto::pty`] lines: [`PtyCmd`] on stdin
//! (write, resize, attach, kill), [`PtyEv`] on stdout (started, data,
//! screen, exit, error). A closed stdin kills the shell: it never outlives
//! the app. The process ends once the shell has ended and said `exit`.
//!
//! - The shell's environment is [`bise_home::env::for_child`] with
//!   [`Child::Shell`]: his settings, nothing of bise's internals or of a
//!   test run (the same rule as every child bise starts).
//! - Its folder must exist and, under `BISE_TEST_HOME`, be inside it
//!   ([`bise_home::test_home::jail`]).
//! - A vt100 parser follows the screen, so `attach` answers a snapshot that
//!   redraws it whatever runs (vim's alternate screen, htop, less), not a
//!   tail of raw bytes.
//!
//! [`spawn`] is bise's one pty spawn: `bise pty` and the TUI's own
//! terminal panel (term.rs) both run their shell through it, so both get
//! the same environment rule.
//!
//! Pure parts (tested): [`parse_args`], [`clamp`], [`shell_env`],
//! [`snapshot`], [`exit_code`]. The rest is the pty plumbing.

use base64::Engine;
use bise_home::env::{Child as EnvChild, ChildEnv};
use bise_proto::pty::{PtyCmd, PtyEv};
use portable_pty::{native_pty_system, Child as PtyChild, CommandBuilder, ExitStatus, MasterPty, PtySize};
use std::ffi::OsString;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// the history the parser keeps (the snapshot draws the screen only)
const SCROLLBACK: usize = 1000;
/// at most this many bytes per `data` line
const CHUNK: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Args {
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub shell: Option<String>,
}

/// `--cwd <dir> --cols C --rows R [--shell <path>]`; the size clamped.
pub(crate) fn parse_args(args: &[String]) -> Result<Args, String> {
    let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let cwd = val("--cwd").ok_or("--cwd <dir> is needed")?;
    if !Path::new(&cwd).is_absolute() {
        return Err(format!("--cwd must be absolute: {cwd}"));
    }
    let num = |k: &str, d: u16| -> Result<u16, String> {
        match val(k) {
            None => Ok(d),
            Some(v) => v.parse::<u16>().map_err(|_| format!("{k}: not a number: {v}")),
        }
    };
    let (cols, rows) = clamp(num("--cols", 80)?, num("--rows", 24)?);
    let shell = val("--shell").filter(|s| !s.is_empty());
    Ok(Args { cwd: PathBuf::from(cwd), cols, rows, shell })
}

/// A size a terminal can have: 2..=500 columns, 1..=200 rows.
pub(crate) fn clamp(cols: u16, rows: u16) -> (u16, u16) {
    (cols.clamp(2, 500), rows.clamp(1, 200))
}

/// The shell's whole environment from its parent's: [`Child::Shell`]'s
/// rule, plus a terminal type xterm.js draws.
pub(crate) fn shell_env(parent: impl IntoIterator<Item = (OsString, OsString)>) -> ChildEnv {
    bise_home::env::env_for(EnvChild::Shell, parent, [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")])
}

/// Escape sequences that draw `screen` on a fresh terminal of its size:
/// the alternate screen first when a full-screen program holds it, then
/// the cells (clear, contents, cursor), the input modes and the title.
pub(crate) fn snapshot(screen: &vt100::Screen) -> Vec<u8> {
    let mut out = Vec::new();
    if screen.alternate_screen() {
        out.extend_from_slice(b"\x1b[?1049h");
    }
    out.extend_from_slice(&screen.state_formatted());
    out
}

/// The shell's code; none when a signal ended it.
pub(crate) fn exit_code(s: &ExitStatus) -> Option<i32> {
    if s.signal().is_some() {
        None
    } else {
        i32::try_from(s.exit_code()).ok()
    }
}

/// One event, one line, never interleaved with another.
#[derive(Clone)]
struct Out(Arc<Mutex<std::io::Stdout>>);

impl Out {
    fn send(&self, ev: &PtyEv) {
        if let Ok(mut o) = self.0.lock() {
            let _ = writeln!(o, "{}", ev.to_value());
            let _ = o.flush();
        }
    }
}

/// `bise pty`: its exit code (2 for bad arguments or a refused folder).
pub fn main(args: &[String]) -> i32 {
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bise pty: {e}");
            return 2;
        }
    };
    if !a.cwd.is_dir() {
        eprintln!("bise pty: not a folder: {}", a.cwd.display());
        return 2;
    }
    if let Err(e) = bise_home::test_home::jail(&a.cwd) {
        eprintln!("bise pty: refused: {e}");
        return 2;
    }
    match run(a) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("bise pty: {e}");
            1
        }
    }
}

/// A shell on a new pty: what [`spawn`] gives.
pub(crate) struct Spawned {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn PtyChild + Send + Sync>,
}

/// `argv` on a new pty of `rows` × `cols`, in `cwd` when it is a folder,
/// with exactly [`shell_env`] of this process's environment: the one pty
/// spawn of bise, for `bise pty` and the TUI's own panel (term.rs).
pub(crate) fn spawn(argv: &[&str], cwd: &Path, rows: u16, cols: u16) -> Result<Spawned, String> {
    let Some(prog) = argv.first() else { return Err("nothing to run".into()) };
    let pair = native_pty_system()
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new(prog);
    cmd.args(&argv[1..]);
    if cwd.is_dir() {
        cmd.cwd(cwd);
    }
    cmd.env_clear();
    for (k, v) in shell_env(std::env::vars_os()).iter() {
        cmd.env(k, v);
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    Ok(Spawned { master: pair.master, child })
}

fn run(a: Args) -> Result<(), String> {
    let shell = a.shell.clone().or_else(|| std::env::var("SHELL").ok().filter(|s| !s.is_empty())).unwrap_or_else(|| "/bin/zsh".into());
    // a login shell, as Terminal.app opens one
    let Spawned { master, mut child } = spawn(&[&shell, "-l"], &a.cwd, a.rows, a.cols)?;
    let pid = child.process_id().unwrap_or(0);
    let mut reader = master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut writer = master.take_writer().map_err(|e| e.to_string())?;
    let out = Out(Arc::new(Mutex::new(std::io::stdout())));
    let parser = Arc::new(Mutex::new(vt100::Parser::new(a.rows, a.cols, SCROLLBACK)));
    out.send(&PtyEv::Started { pid, shell, cwd: a.cwd.display().to_string(), cols: a.cols, rows: a.rows });

    // the shell's output: into the parser and out, under the parser's lock,
    // so a `screen` and the `data` after it never overlap or miss a byte
    let (p, o) = (parser.clone(), out.clone());
    let reading = std::thread::spawn(move || {
        let mut buf = vec![0u8; CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut p) = p.lock() {
                        p.process(&buf[..n]);
                        o.send(&PtyEv::Data { data: B64.encode(&buf[..n]) });
                    }
                }
            }
        }
    });

    // his commands, until stdin closes (the app quit or died): then kill
    let (p, o) = (parser.clone(), out.clone());
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match PtyCmd::decode(&line) {
                Ok(PtyCmd::Write { data }) => match B64.decode(data.as_bytes()) {
                    Ok(bytes) => {
                        let _ = writer.write_all(&bytes);
                        let _ = writer.flush();
                    }
                    Err(e) => o.send(&PtyEv::Error { text: format!("write: base64: {e}") }),
                },
                Ok(PtyCmd::Resize { cols, rows }) => {
                    let (cols, rows) = clamp(cols, rows);
                    let _ = master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
                    if let Ok(mut p) = p.lock() {
                        p.set_size(rows, cols);
                    }
                }
                Ok(PtyCmd::Attach) => {
                    if let Ok(p) = p.lock() {
                        let s = p.screen();
                        let (rows, cols) = s.size();
                        o.send(&PtyEv::Screen { data: B64.encode(snapshot(s)), cols, rows, alternate: s.alternate_screen() });
                    }
                }
                Ok(PtyCmd::Kill) => break,
                Ok(PtyCmd::Unknown { tag, .. }) => o.send(&PtyEv::Error { text: format!("unknown cmd {tag}") }),
                Err(e) => o.send(&PtyEv::Error { text: e }),
            }
        }
        hang_up(pid);
    });

    let _ = reading.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    out.send(&PtyEv::Exit { code: exit_code(&status) });
    Ok(())
}

/// SIGHUP to the shell's group (what closing a terminal does), SIGKILL
/// after a short grace if it is still there. The shell leads its own
/// session (portable-pty's setsid), so its pid is its group's.
fn hang_up(pid: u32) {
    if pid == 0 {
        return;
    }
    let group = format!("-{pid}");
    let _ = std::process::Command::new("kill").args(["-HUP", "--", &group]).status();
    for _ in 0..30 {
        let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status();
        if !matches!(alive, Ok(s) if s.success()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = std::process::Command::new("kill").args(["-KILL", "--", &group]).status();
}

#[cfg(test)]
#[path = "pty_tests.rs"]
mod tests;
