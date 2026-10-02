//! bend-plugins: Agent Plugins 1.0 (the portable base) for the Bend
//! harness. Design: docs/plugins.md.
//!
//! - `resolve`: discovery, manifest validation, precedence, components,
//!   diagnostics (pure over the file system, no process started);
//! - `state`: the enable/disable file;
//! - `report`: the human and JSON listings;
//! - `stdio`: a stdio MCP client (one child process, JSON-RPC lines);
//! - `http`, `remote`: a remote MCP client (Streamable HTTP, SSE);
//! - `oauth`: the browser login and the tokens of remote servers;
//! - `login`: `/plugins login`, `bise plugins login|logout`;
//! - `status`: each remote server's last state, for `/plugins`;
//! - `bridge`: the per-session loopback HTTP bridge the Bend REPL calls;
//! - `cli`: the `bise plugins ...` subcommand;
//! - `import`: `bise plugins import-mcp`, another agent's MCP servers as
//!   one plugin (BISE-273).

pub mod bridge;
pub mod cli;
pub mod http;
pub mod import;
pub mod login;
pub mod oauth;
pub mod remote;
pub mod report;
pub mod resolve;
pub mod state;
pub mod status;
pub mod stdio;
