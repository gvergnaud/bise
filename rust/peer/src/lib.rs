//! Who is at the other end of a socket (docs/issues/16, 18): an agent's
//! process or the user's. One home for the parts the hub's sockets
//! (`switchboard::daemon::accept`) and the computer-use broker both need,
//! with no copy:
//!
//! - [`tags`]: the `BISE_OWNERS` variable every agent's process carries,
//!   its tags `<hub>.<dir>.<ms>` and a hub's id;
//! - [`table`]: the process table of this user, each process with its
//!   tags (`ps -E` on macOS, `/proc` on Linux), whole ([`table::snapshot`],
//!   the cleanup sweeps) or only what a judge walks ([`table::around`],
//!   the hub's sockets, per request);
//! - [`judge`]: pure, a peer pid and a table in, who it is out
//!   ([`judge::judge`] for one hub, [`judge::judge_any`] for a
//!   machine-wide service);
//! - [`os`]: the peer pid of a unix socket;
//! - [`fsx`]: the one atomic replace of a `~/bise` file with its `ifMatch`
//!   (plugin-apps A4), shared by the hub's view writes and the desktop
//!   core's Finder writes (architect m_60037).
//!
//! The rest of the process tracking (which processes to kill, the REPLs'
//! sessions, spawning) stays in `switchboard::procs`, which re-exports
//! these.

pub mod fsx;
pub mod judge;
pub mod os;
pub mod table;
pub mod tags;
