//! bise-computer-use: the Rust side of computer use (design:
//! docs/computer-use-design.md; contracts C1-C6: docs/computer-use-briefs.md).
//!
//! ```text
//! agent session ── `bise computer-use mcp` (stdio MCP, C1) ──┐
//!                                                             │ C3: ~/.bise/run/computer-use.sock
//!   Chrome ── `bise computer-use chrome-host` (C4 relay) ─────┤
//!                                                             ▼
//!                                   broker (`bise computer-use broker`, one per machine)
//!                                     ├─ browsers: one link per extension (C4)
//!                                     ├─ helper: `bise Computer Use.app` (C5)
//!                                     └─ state.json + events.jsonl (C6)
//! ```
//!
//! - `paths`: every file and socket, from one bise home;
//! - `proto`: the C1 error codes and targets;
//! - `refuse`: the hard refusals (design §5.1, §5.2);
//! - `nm`: native messaging frames; `b64` and `image`: screenshots;
//! - `state`: C6's state.json and events.jsonl;
//! - `browsers`: the Chromium family, native host manifests, the shim;
//! - `broker`: the broker; `client`: connecting to it (and starting it);
//! - `mcp`: the MCP server; `host`: the native host relay;
//! - `cli`: `bise computer-use ...`.

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

pub mod b64;
pub mod broker;
pub mod browsers;
pub mod cli;
pub mod client;
#[cfg(test)]
mod e2e;
pub mod host;
pub mod image;
pub mod mcp;
pub mod nm;
pub mod paths;
pub mod policy;
pub mod proto;
pub mod refuse;
pub mod state;
pub mod who;

/// Milliseconds since the epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
