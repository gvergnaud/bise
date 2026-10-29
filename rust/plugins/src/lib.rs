//! bend-plugins: Agent Plugins 1.0 (the portable base) for the Bend
//! harness. Design: docs/plugins.md.
//!
//! - `id`: [`id::PluginId`], the one typed id of a plugin or a part;
//! - `model`: the plain data the modules share (connectors, tools,
//!   skills, the on/off file as read);
//! - `resolve`: discovery, manifest validation, precedence, components,
//!   diagnostics (pure over the file system, no process started);
//! - `skills`: the one scanner of the loose skill folders (roots, scan,
//!   on/off, the session's skills index, the hub's fingerprint);
//! - `state`: the on/off file (`plugins.json`);
//! - `connectors`: his Mistral connectors' catalog, its cache and index
//!   lines;
//! - `catalog`: the one builder of the unified Plugins list (the wire
//!   rows, `bise_proto::app_rows::Plugin`);
//! - `report`: the human and JSON listings;
//! - `stdio`: a stdio MCP client (one child process, JSON-RPC lines);
//! - `http`, `remote`: a remote MCP client (Streamable HTTP, SSE);
//! - `oauth`: the browser login and the tokens of remote servers;
//! - `login`: `/plugins login`, `bise plugins login|logout`;
//! - `status`: each remote server's last state, for `/plugins`;
//! - `bridge`: the per-session loopback HTTP bridge the Bend REPL calls;
//! - `cli`: the `bise plugins ...` subcommand;
//! - `import`: `bise plugins import-mcp`, another agent's MCP servers as
//!   one plugin (BISE-273);
//! - `add`: the add menu (an MCP server, an import found on disk, a
//!   folder, a git URL): staged, previewed, installed on his click;
//! - `remove`: a plugin he added removed, with its secrets, logins,
//!   statuses and on/off ids;
//! - `marketplace`: his plugin marketplaces and what they offer;
//! - `signals`: what his projects name (remotes, file names, env example
//!   keys; names only), the evidence the 'connect your tools' band ranks
//!   and says why by;
//! - `secrets`: the values of an MCP server's `${VAR}` he typed in;
//! - `visibility`: whether a view's call may run (its own server's
//!   app-visible tools only; docs/mcp-apps.md, risk 1).

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

pub mod add;
pub mod bridge;
pub mod catalog;
pub mod cli;
pub mod connectors;
pub mod http;
pub mod id;
pub mod import;
pub mod login;
pub mod marketplace;
pub mod model;
pub mod oauth;
pub mod parent;
pub mod remote;
pub mod remove;
pub mod report;
pub mod resolve;
pub mod secrets;
pub mod signals;
pub mod skills;
pub mod state;
pub mod status;
pub mod stdio;
pub mod title;
pub mod tools_cache;
pub mod visibility;
