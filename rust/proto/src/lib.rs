//! bise's typed client protocol (bise desktop S3a; architect's review of
//! the desktop plan, decision D, and of the S3a plan, m_8220).
//!
//! The messages a client and a hub exchange on `hub.sock`:
//!
//! - [`hub::HubEv`], tagged `"ev"`: what a hub tells a client (its
//!   agents, its cards, a thread's pages and live entries, an agent's
//!   current step, errors);
//! - [`hub::HubCmd`], tagged `"cmd"`: what a client asks a hub (subscribe
//!   to a thread, an older page, send, answer, stop, archive, unarchive).
//!
//! [`rpc`] carries them as JSON-RPC 2.0 (client-protocol): `initialize`,
//! one method per `HubCmd`, one notification per `HubEv` (tables over
//! their tags), numbered hub-wide notifications. The older `cmd` door
//! (a `hello` command) went once client-protocol's release (v2026.10.2-30)
//! had shipped its one release after (debt-t3).
//!
//! The crate holds every JSON-line contract bise has, each in its own
//! module (one TS generator, one fixtures convention): the hub protocol
//! ([`hub`]), the desktop core ↔ app lines ([`core_ev`], [`app_cmd`],
//! their rows in [`app_rows`], their JSON-RPC in [`app_rpc`]), and
//! bise-mac-helper ↔ Electron's main ([`helper`]), `bise pty` ↔ Electron's
//! main ([`pty`], the terminal panel).
//!
//! Every event and command carries `project`: the hub's id
//! (`bise_home`'s hub id of the workspace, e.g. `harness-3abb2bd8`), a
//! plain string here so this crate depends on nothing of bise. Positions
//! are transcript line numbers, the `#pos` of `sb inspect`/`sb history`.
//!
//! v1's rule: a name is added, never renamed or retyped. A reader ignores
//! unknown fields, and an unknown tag decodes to `Unknown { tag, raw }`,
//! never an error, so an older client survives a newer hub and the
//! reverse.
//!
//! THE WIRE RULE (architect m_14382): on the wire an existing field, list
//! or enum value never changes meaning, and a line that made an entry of
//! one kind still makes one of that kind. A new fact goes in a new
//! optional field or list; a new enum value only in an enum whose
//! released version already has `Unknown` (an older reader of one without
//! it fails on the whole event). Its law: `tests/released.rs` reads what
//! v2026.10.2-28 wrote (`fixtures/released/`) with today's types and
//! writes it back with every released key and value unchanged. Its one
//! exception: a field a released client sends that the hub no longer
//! reads ([`REMOVED`]) is refused by name when it carries data, never
//! read with its data dropped. A tag or method bise retired ([`RETIRED`])
//! reads as `Unknown` (a method: refused by name), and is never reused.
//! What has no producer yet lives in [`draft`]: its fixtures
//! are there for the window's fake core, its names may still move until
//! its stream emits it.
//!
//! [`thread::fold`] turns an agent's transcript lines into entries (the
//! hub folds, clients don't). It reads them with [`thread::lines`], the
//! one parser of each line kind, which the TUI's `wire.rs`/`sb.rs` call
//! too, and [`thread::words`] makes the words both show.
//!
//! [`parts`] is his message as typed parts (a quote, a paste, an image,
//! a file, his words) and the one owner of the template the model reads
//! them in (render, and parse for an old text).
//!
//! [`ops`] holds the rows of the answers that were the terminal's older
//! untyped events (`/diff`'s branches, `/version`'s picker), typed for
//! their methods (client-protocol step 3).
//!
//! [`commands`] is the slash commands' catalog (name, usage, arguments,
//! `client` for the screen ones): the TUI's popup and `/help` read it, a
//! client over the wire gets it serialized.
//!
//! [`words`] is the UI words' catalog: the TUI, the desktop (generated TS)
//! and the tests (generated JSON) read every word they show from it.
//!
//! The fixtures (`fixtures/*.jsonl`) are the one source for the Rust
//! tests, the native helper's laws and the Python e2e.

pub mod activity;
pub mod anchor;
pub mod app_cmd;
pub mod app_rows;
pub mod app_rpc;
pub mod approvals;
pub mod ascii;
pub mod core_ev;
pub mod commands;
pub mod context;
pub mod core;
pub mod diff;
pub mod draft;
pub mod envelope;
pub mod helper;
pub mod hub;
pub mod inbox;
pub mod log;
pub mod office;
pub mod ops;
pub mod parts;
pub mod pty;
pub mod rows;
pub mod jsonrpc;
pub mod rpc;
pub mod sb;
pub mod slash;
pub mod socket;
pub mod thread;
pub mod view_rpc;
pub mod view_rules;
pub mod words;

use serde::de::DeserializeOwned;
use serde_json::Value;

/// A hub's id (the workspace's), the `project` of every message.
pub type Project = String;
/// A transcript line number: an entry's position (an entry folding several
/// lines has its first line's).
pub type Pos = u64;

/// The protocol's version, said in `hello` both ways.
pub const PROTO: u32 = 1;

/// A tagged message: the known tags decode as `T`, any other as
/// `unknown(tag, raw)`. A missing tag or a known tag with bad fields is an
/// error (a bug of the sender, not a newer version).
pub(crate) fn decode<T: DeserializeOwned>(v: Value, key: &str, tags: &[&str], unknown: impl FnOnce(String, Value) -> T) -> Result<T, String> {
    let tag = v.get(key).and_then(Value::as_str).ok_or_else(|| format!("no \"{key}\""))?.to_string();
    if !tags.contains(&tag.as_str()) {
        return Ok(unknown(tag, v));
    }
    serde_json::from_value(v).map_err(|e| format!("{tag}: {e}"))
}

/// A field a released client sends that the hub no longer reads: its
/// command's tag, the field, where what it carried goes now. The wire
/// rule's one exception (architect m_34507): ignoring it would drop his
/// work silently (an older app's `files`: his pasted image lost, only
/// the text arriving), so a command carrying it with data is refused by
/// name and nothing is sent. Empty or absent: read as any line. A row is
/// never deleted: an old client's error stays clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removed {
    pub cmd: &'static str,
    pub field: &'static str,
    pub went: &'static str,
}

/// Every removed field (issue 31: the desktops' `files` list became the
/// typed `parts`, shims-due ff9c5cdd5).
pub const REMOVED: &[Removed] = &[
    Removed { cmd: "send", field: "files", went: "parts (issue 31)" },
    Removed { cmd: "answer", field: "files", went: "parts (issue 31)" },
    Removed { cmd: "slash", field: "files", went: "parts (issue 31)" },
];

/// The removed field a command (its tag under `key`) carries with data.
pub fn removed_in(v: &Value, key: &str) -> Option<&'static Removed> {
    let tag = v.get(key)?.as_str()?;
    let data = |x: &Value| match x {
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        Value::String(s) => !s.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    };
    REMOVED.iter().find(|r| r.cmd == tag && v.get(r.field).is_some_and(data))
}

/// The refusal of a command that carries a removed field, for a log or a
/// test: his words, then the field and where it went.
pub fn removed_refusal(r: &Removed) -> String {
    format!("{} ({} removed: {})", words::common::removed_field, r.field, r.went)
}

/// What a [`Retired`] row names: a hub event's tag, a hub command's tag,
/// or an rpc method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetiredKind {
    HubEv,
    HubCmd,
    RpcMethod,
}

/// A tag or method a released bise used that this one retired
/// (architect m_48959): its tag is out of its kind's `TAGS`, so a line
/// carrying it reads as `Unknown` (a client ignores it), and a retired
/// method is refused by name ([`jsonrpc::RpcError::retired`]), never
/// method-not-found. A row is never deleted and its tag never comes back
/// with another meaning (laws in `lib_tests`, `tests/released.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retired {
    pub kind: RetiredKind,
    pub tag: &'static str,
    pub went: &'static str,
}

/// Every retired tag and method.
pub const RETIRED: &[Retired] = &[
    // an older hub's /archive yes/no: his /archive asks with the 1/2 drop card
    Retired { kind: RetiredKind::HubEv, tag: "confirm", went: "the drop card (drop-card, architect m_37545)" },
    Retired { kind: RetiredKind::HubCmd, tag: "confirm", went: "the drop card (drop-card, architect m_37545)" },
    Retired { kind: RetiredKind::RpcMethod, tag: "confirm/answer", went: "the drop card (drop-card, architect m_37545)" },
    // an older desktop's Environments screen: the processes list replaced it
    Retired { kind: RetiredKind::HubEv, tag: "dev_servers", went: "processes (docs/processes.md)" },
    Retired { kind: RetiredKind::HubCmd, tag: "dev_servers", went: "processes (docs/processes.md)" },
    Retired { kind: RetiredKind::RpcMethod, tag: "devServers/list", went: "processes (docs/processes.md)" },
];

/// The row of a retired tag or method of that kind.
pub fn retired(kind: RetiredKind, tag: &str) -> Option<&'static Retired> {
    RETIRED.iter().find(|r| r.kind == kind && r.tag == tag)
}

/// One line as a JSON value.
pub(crate) fn parse(line: &str) -> Result<Value, String> {
    serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))
}

/// serde's default for a flag that is true unless said
pub(crate) fn yes() -> bool {
    true
}

/// skip a flag at its default (true)
pub(crate) fn is_true(b: &bool) -> bool {
    *b
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}
