//! The envelopes bise puts around what it adds to an agent's context, once
//! (architect m_41684): the writers (`switchboard::prompts`, `board`)
//! build them from these constants and `/debug completion`'s classifier
//! (`bise_session::completion::groups`) recognizes them by the same ones,
//! so renaming a tag renames both. The compaction's summary is written by
//! the runtime (Bend): its prefixes are pinned to `bend/core/history.bend`
//! and `bend/runtime/main-pure.bend` by a test of the classifier.

/// his words: `<user_message id="m_<n>">…</user_message>`
pub const USER_MESSAGE: &str = "user_message";
/// another agent's message: `<agent_message from="…" …>…</agent_message>`
pub const AGENT_MESSAGE: &str = "agent_message";
/// an MCP App view's message (docs/mcp-apps.md, C5): `<view_message
/// plugin="…" id="m_<n>">…</view_message>`, never his words
pub const VIEW_MESSAGE: &str = "view_message";
/// the live state before each call: `<bise_state>…</bise_state>`
pub const BISE_STATE: &str = "bise_state";
/// the head of an agent's role preamble (`# Your role: …`)
pub const ROLE_PREAMBLE: &str = "# Your role";
/// the compaction's summary message (bend/core/history.bend)
pub const SUMMARY_PREFIX: &str = "Summary of the earlier conversation:";
/// the compaction's note before the kept messages (bend/runtime/main-pure.bend)
pub const COMPACTED_PREFIX: &str = "The earlier conversation was compacted.";
/// a line of bise's own to the agent (a resume, a fact)
pub const BISE_NOTE_PREFIX: &str = "[bise]";
