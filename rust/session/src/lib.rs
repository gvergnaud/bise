//! The bise session log (docs/research/session-format.md, spec in
//! docs/spec/session-format.ts): one folder per session,
//! an append-only `events.jsonl` of typed events.
pub mod reader;
pub mod types;

pub use reader::{read_bytes, read_dir, Event, Loc, Log, Open};
pub use types::Payload;
pub mod blob;
pub mod writer;
pub use writer::{new_session_id, OpenError, Writer};
pub mod project;
pub mod resume;
pub mod state;
pub use state::State;
pub mod redact;
pub use redact::Redactor;
pub mod legacy;
pub mod migrate;
pub mod show;
pub mod recorder;
pub mod pairing;
pub mod usage_line;
