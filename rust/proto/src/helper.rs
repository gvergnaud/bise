//! bise-mac-helper ↔ Electron's main process (the desktop reset, lead
//! m_9434; architect m_9461): one JSON object per line on the helper's
//! stdin/stdout. The helper (`apps/desktop/native/mac`) does only what
//! Electron can't: the fn key tap, the front app's context through
//! Accessibility, the permission states, quiet moments, the screenshot.
//! Away (lock, sleep) is Electron's own powerMonitor; the mic is the core's.
//!
//! - [`HelperEv`], tagged `"ev"`: what the helper says;
//! - [`HelperCmd`], tagged `"cmd"`: what Electron's main asks it.
//!
//! No `project`: the helper knows no hub. A closed stdin is `quit`: the
//! helper never outlives the app.
//!
//! Frozen since Electron's main decodes them (src/main/helper/lines.ts): the
//! v1 rule holds, a name is added, never renamed or retyped. The fixtures
//! (`fixtures/helper_ev.jsonl`, `fixtures/helper_cmd.jsonl`) are the one
//! source: the Rust round trip here, the Swift helper's own tests (it
//! decodes every command line and encodes every event line back), the
//! test stub that stands in for the helper in Playwright runs.

use crate::context::FnContext;
use crate::{decode, is_false, parse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the helper says.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum HelperEv {
    /// at start, once
    Hello {
        proto: u32,
        version: String,
        pid: u32,
        /// the process macOS charges the helper's permissions to (TCC's
        /// responsible process): bise's app when Electron spawned it.
        /// Absent when macOS won't say.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        responsible: Option<Responsible>,
    },
    /// what fn (or right option) meant: the key machine's outputs
    Fn {
        act: FnAct,
        /// who holds the talk (talk_start only)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<TalkKey>,
        /// the bise key after fn (`space`, `1`..`0`, `i`, `enter`, `tab`...),
        /// act `key` only
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    /// the front app's context: read at every talk_start and on
    /// `read_context` (its `id` then); an excluded app gives an empty
    /// context and `excluded`
    Context {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        context: FnContext,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bundle: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        excluded: bool,
    },
    /// the front app changed (its name only, no read), and after each
    /// context read: the spotlight's 'in front' row. `app` null: excluded.
    Front {
        app: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bundle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// the permissions, at start and whenever one changes
    Perms { accessibility: Grant, mic: Grant, screen: Grant, calendar: Grant },
    /// a quiet moment began or ended (his 'quiet for 1 h' is the tray's:
    /// Electron ORs it in)
    Quiet { on: bool, why: QuietWhy },
    /// the answer to `shot`: the front window as a PNG
    Shot {
        id: String,
        /// none: no front window, no screen permission, or skipped
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// an excluded app or a private window
        #[serde(default, skip_serializing_if = "is_false")]
        skipped: bool,
    },
    /// a command it could not do (a bad line, an unknown tag)
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        message: String,
    },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Responsible {
    pub pid: u32,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FnAct {
    /// held past the tap threshold: talk
    TalkStart,
    /// released: the talk ends
    TalkEnd,
    /// another key came: the hold is off
    Cancel,
    Tap,
    /// two taps: hands-free until the next tap
    Double,
    /// fn + a bise key (swallowed: the front app never sees it)
    Key,
    /// his plain esc (passed to the front app): bise hides
    Esc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TalkKey {
    Fn,
    RightOption,
}

/// macOS's answer for one permission: `ask` = not decided yet (or, for
/// Accessibility and the screen, which macOS never calls refused: not
/// granted)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    Allowed,
    Denied,
    Ask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum QuietWhy {
    /// the mic or a camera runs somewhere (not bise's)
    Call,
    /// his screen is shared or recorded
    Sharing,
    /// a meeting in his calendar (when `config.meetings`)
    Meeting,
    /// the front window fills a screen
    FullScreen,
    /// not quiet
    #[serde(rename = "")]
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Perm {
    Accessibility,
    Mic,
    Screen,
    Calendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PermAction {
    /// macOS's own prompt when it can (else its pane)
    Ask,
    /// its System Settings pane
    Open,
}

impl HelperEv {
    pub const TAGS: &'static [&'static str] = &["hello", "fn", "context", "front", "perms", "quiet", "shot", "error"];

    pub fn decode(line: &str) -> Result<HelperEv, String> {
        decode(parse(line)?, "ev", Self::TAGS, |tag, raw| HelperEv::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            HelperEv::Unknown { raw, .. } => raw.clone(),
            e => serde_json::to_value(e).expect("a HelperEv encodes"),
        }
    }
}

/// What Electron's main asks the helper.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HelperCmd {
    /// at start and when his prefs change: the apps never read (bundle
    /// ids, on top of the helper's built-in list), right option as the fn
    /// fallback, quiet during calendar meetings
    Config {
        exclude: Vec<String>,
        right_option: bool,
        meetings: bool,
    },
    /// the spotlight's route line is up: a plain tab is bise's
    Route { open: bool },
    /// bise's own talk holds the mic (the core's): quiet's mic sample
    /// ignores it, and 3 s after
    OwnMic { open: bool },
    /// read the front app now -> `context {id}`
    ReadContext { id: String },
    /// -> `perms` when the answer changes
    Perm { perm: Perm, action: PermAction },
    /// the front window as a PNG in `dir` -> `shot {id}`
    Shot { id: String, dir: String },
    /// exit 0
    Quit,
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl HelperCmd {
    pub const TAGS: &'static [&'static str] = &["config", "route", "own_mic", "read_context", "perm", "shot", "quit"];

    pub fn decode(line: &str) -> Result<HelperCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| HelperCmd::Unknown { tag, raw })
    }

    pub fn to_value(&self) -> Value {
        match self {
            HelperCmd::Unknown { raw, .. } => raw.clone(),
            c => serde_json::to_value(c).expect("a HelperCmd encodes"),
        }
    }
}
