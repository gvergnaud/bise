//! `bise pty` ↔ Electron's main process (the desktop's terminal panel, bar
//! T W25/V13/K26/M16; architect m_10140): one JSON object per line on the
//! pty process's stdin/stdout. One process per shell, a child of Electron's
//! main, never inside ambient-core (a terminal's flood must not queue the
//! core's events behind it).
//!
//! - [`PtyEv`], tagged `"ev"`: what the pty process says;
//! - [`PtyCmd`], tagged `"cmd"`: what Electron's main asks it.
//!
//! Bytes travel base64 (`data`): a shell's output is not always UTF-8, and
//! a key may be any byte. A closed stdin is `kill`: the shell never
//! outlives the app.
//!
//! Draft: its names may still move until Electron's main reads them. The
//! fixtures (`fixtures/pty_ev.jsonl`, `fixtures/pty_cmd.jsonl`) are the one
//! source for the Rust round trip and the generated TS.

use crate::{decode, parse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the pty process says.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum PtyEv {
    /// the shell runs: once, first
    Started {
        /// the shell's pid (its process group's too)
        pid: u32,
        shell: String,
        cwd: String,
        cols: u16,
        rows: u16,
    },
    /// the shell's output, as it came (base64)
    Data { data: String },
    /// the answer to `attach`: escape sequences (base64) that draw the
    /// screen as it is now on a fresh terminal of `cols` × `rows`: the
    /// alternate screen if a full-screen program holds it, the cells with
    /// their colors, the cursor, the input modes (application cursor,
    /// bracketed paste, mouse) and the title
    Screen { data: String, cols: u16, rows: u16, alternate: bool },
    /// the shell ended (its exit code; none when a signal killed it); the
    /// process ends right after
    Exit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
    },
    /// a command it could not do (a bad line, base64 that doesn't decode)
    Error { text: String },
    /// a tag this version doesn't know
    #[serde(skip)]
    #[cfg_attr(feature = "ts", ts(skip))]
    Unknown { tag: String, raw: Value },
}

impl PtyEv {
    pub const TAGS: &'static [&'static str] = &["started", "data", "screen", "exit", "error"];

    pub fn decode(line: &str) -> Result<PtyEv, String> {
        decode(parse(line)?, "ev", Self::TAGS, |tag, raw| PtyEv::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            PtyEv::Unknown { raw, .. } => raw.clone(),
            e => serde_json::to_value(e).expect("a PtyEv encodes"),
        }
    }
}

/// What Electron's main asks the pty process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum PtyCmd {
    /// bytes to the shell (keys, a paste), base64
    Write { data: String },
    /// the panel's new size in cells (clamped by the pty process)
    Resize { cols: u16, rows: u16 },
    /// the screen as it is now (a panel or a window that opens again):
    /// answered by `screen`
    Attach,
    /// end the shell (SIGHUP to its group, SIGKILL after a grace): `exit` follows
    Kill,
    /// a tag this version doesn't know
    #[serde(skip)]
    #[cfg_attr(feature = "ts", ts(skip))]
    Unknown { tag: String, raw: Value },
}

impl PtyCmd {
    pub const TAGS: &'static [&'static str] = &["write", "resize", "attach", "kill"];

    pub fn decode(line: &str) -> Result<PtyCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| PtyCmd::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            PtyCmd::Unknown { raw, .. } => raw.clone(),
            c => serde_json::to_value(c).expect("a PtyCmd encodes"),
        }
    }
}
