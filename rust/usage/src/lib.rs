//! bise's usage counts (page usage-metrics-plan, step 1; architect
//! m_53623, m_53687): what a process may count, the day's packet the
//! machine sends, and the one rule that says whether anything is counted
//! or sent at all.
//!
//! Pure, like bise_peer: the time (a [`Day`], unix seconds), the paths
//! (from `bise_home::Home::usage_*`), the random bytes and the network (a
//! [`send::Transport`]) come in as arguments. The edges are two small
//! functions: [`sharing::Facts::read`] (prefs.json and the environment) and
//! [`spool`] (a source's file, written whole, tmp then rename).
//!
//! - [`key`]: the closed keys ([`CountKey`], [`MinutesKey`], [`HistKey`],
//!   [`Milestone`]): never a key made of a string from outside;
//! - [`day`]: UTC days and ISO weeks (no time zone in the data);
//! - [`tally`]: what one process counted in a day, and the sum of sources;
//! - [`packet`]: the day's packet, with its caps (999 a count, minutes
//!   rounded to 5) and its closed build fields;
//! - [`state`]: `state.json` (cohort, active days, milestones);
//! - [`install_id`]: the random id, renewed after 13 months at most; its
//!   `Debug` never prints it;
//! - [`sharing`]: [`sharing_allowed`], the one gate; its [`Permit`] is
//!   what writing a source and sending a packet ask for;
//! - [`encode`]: the bytes of a body (the packets as plain JSON, the
//!   delete request): the PII law reads these bytes;
//! - [`send`]: the endpoint, the send fn through a transport port, the
//!   backoff;
//! - [`schema`]: the exported schema (snapshot `schema.json`, the doc
//!   `docs/telemetry.md`, the server's allow list);
//! - [`law`]: the PII law on a body and the no-wildcard check of a
//!   mapping, for this crate's tests and the collectors' (steps 2 to 4).
//!
//! Not here: who counts what (the hub's journal projection, the core's
//! AppCmd match, the TUI), the one sender's flock and its retries, the
//! curl transport. Those are steps 2 to 5 of the plan.
//!
//! [`SENDING_ON`] is the global switch, off: until step 9 (Gabriel's go)
//! nothing is counted, written or sent, whatever the settings say.

#[macro_use]
mod closed;
pub mod day;
pub mod encode;
pub mod install_id;
pub mod key;
pub mod law;
pub mod packet;
pub mod schema;
pub mod send;
pub mod sharing;
pub mod spool;
pub mod state;
pub mod tally;

pub use day::{Day, IsoWeek};
pub use install_id::InstallId;
pub use key::{CountKey, Delay, HistKey, Milestone, MinutesKey};
pub use packet::{Arch, Build, Channel, Packet, Surface};
pub use sharing::{sharing_allowed, Facts, Off, Permit};
pub use tally::Tally;

bise_home::test_home!();

/// The global switch (plan step 9, Gabriel's go): while false,
/// [`sharing_allowed`] refuses everything, so nothing is counted, written
/// or sent.
pub const SENDING_ON: bool = false;
