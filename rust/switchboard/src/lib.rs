//! Switchboard: one main agent that routes the user's messages to task
//! sub-agents (RFC 0001), optional git worktrees per task (RFC 0002), and
//! agent-to-agent messaging (RFC 0003). The design docs live in
//! `docs/`.
//!
//! Layout (functional core, imperative shell):
//! - `model`, `router`, `wire`, `board`, `prompts`: pure data and parsing;
//! - `core`: the link to sb-core (the hub's decisions, in Bend,
//!   `hub/*.bend`): `Hub::handle(input) -> effects`, plus the read-only
//!   state mirror the views read; tested without REPLs or sockets;
//! - `worktree`: the git operations of RFC 0002;
//! - `sweep`: where task worktrees live, and their cleanup;
//! - `tools_env`: the agents' PATH, git and rg (BISE-166);
//! - `agents_md`: the AGENTS.md files an agent's prompt carries (BISE-232);
//! - `daemon`: the imperative shell (sockets, journal, feeds), with
//!   `daemon/repl` (REPL processes) and `daemon/versions` (`/version`);
//! - `transcript`: reading a thread (positions, cursors, origin);
//! - `cli`: the `sb` command the agents call through their bash tool;
//! - `client`: how a client (TUI, headless test) reaches the hub.

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

pub mod agents_md;
pub mod approvals;
pub mod artifacts;
pub mod board;
pub mod boot;
pub mod cli;
pub mod client;
pub mod computer_use;
pub mod core;
pub mod daemon;
pub mod devflow;
pub mod diff;
pub mod every;
pub mod feature;
pub mod flow;
pub mod forge;
pub mod idle;
pub mod land;
pub mod land_pick;
pub mod model;
pub mod paths;
pub mod peer;
pub mod peer_os;
pub mod place;
pub mod procs;
pub mod prompts;
pub mod recycle;
pub mod role;
pub mod router;
pub mod search;
pub mod sweep;
pub mod switch;
pub mod tools_env;
pub mod transcript;
pub mod trunk;
pub mod util;
pub mod wire;
pub mod worktree;
