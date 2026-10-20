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
//! - `parts_in`: his message's typed parts as a client sends them, checked
//!   and rendered to the model's text (msg-parts);
//! - `attach`: an agent's `sb send/spawn --attach` as its message's parts
//!   (send-parts), the pure half of `daemon/attach.rs`;
//! - `daemon`: the imperative shell (sockets, journal, feeds), with
//!   `daemon/repl` (REPL processes) and `daemon/versions` (`/version`);
//! - `doing`, `activity`: les bims' facts, pure: an agent's `doing` from
//!   its REPL lines, `hub/activity` from sb-core's journal effects (their
//!   hub side: `core_doing.rs`);
//! - `transcript`: reading a thread (positions, cursors, origin);
//! - `cli`: the `sb` command the agents call through their bash tool;
//! - `client`: how a client (TUI, headless test) reaches the hub.

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

pub mod agents_md;
pub mod approvals;
pub mod artifacts;
pub mod artifacts_assets;
pub mod artifacts_turns;
pub mod attach;
pub mod attached;
pub mod board;
pub mod boot;
pub mod card_link;
pub mod cli;
pub mod client;
pub mod activity;
pub mod computer_use;
pub mod contexts;
pub mod core;
pub mod core_proc;
#[cfg(test)]
mod core_fresh;
mod core_turn;
pub mod core_num;
pub mod daemon;
pub mod devflow;
pub mod doing;
pub mod diff;
pub mod every;
pub mod every_name;
pub mod feature;
pub mod first_run;
pub mod flow;
pub mod fn_context;
pub mod forge;
pub mod idle;
pub mod image_care;
pub mod keeps;
pub mod land;
pub mod land_check;
pub mod land_pick;
pub mod land_queue;
pub mod model;
pub mod pages;
pub mod parts_in;
pub mod paths;
pub mod peer;
pub use bise_peer::os as peer_os;
pub mod place;
pub mod place_job;
pub mod procs;
pub mod proto_view;
pub mod prompts;
pub mod recycle;
pub mod repl_start;
pub mod role;
pub mod router;
pub mod search;
pub mod snapshot;
pub mod snapshot_rows;
pub mod scratch;
pub mod sweep;
pub mod trash;
pub mod switch;
pub mod tools_env;
pub mod transcript;
pub mod trunk;
pub mod spent;
pub mod usage_ledger;
pub mod util;
pub mod view;
pub mod wake;
pub mod watch;
pub mod wire;
pub mod worktree;
