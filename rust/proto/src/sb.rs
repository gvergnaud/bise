//! An agent's `sb` request, typed (issue 27, debt D10): one JSON object
//! per request on `agent.sock`, tagged `"cmd"`, declared once here. The
//! CLI (`switchboard/src/cli.rs`) builds a [`Req`] and sends its JSON;
//! the hub decodes it with [`Req::decode`] and matches on it, one arm per
//! command, never a string key. A new sb command is one variant here, its
//! CLI arm and its handler.
//!
//! Each variant's fields are the keys the CLI has always written (the
//! released ones are pinned in `fixtures/released/sb_req_*.jsonl`, read by
//! `tests/released.rs`): a Rust name that differs says its wire key with
//! `rename`, and the few wire shapes that are not plain values (a message
//! id `m_12`, `mode`, `off`, `step`/`of`, a spawn's model) have a codec
//! in [`wire`]. A tag this side doesn't know (a newer `sb` than the hub)
//! decodes to `Unknown`, never an error; a known tag with fields that
//! don't fit is the sender's error, said back to it.
//!
//! What a field means past its shape stays with its one parser in the
//! hub: a timer's schedule (`every.rs`), a watch's spec (`wake.rs`), a
//! flow mode (`devflow.rs`), a page's `went` (`pages/store.rs`).
//!
//! The wire rule binds (lib.rs): a field is only added, never renamed or
//! retyped.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod wire;

/// Declares [`Req`]: the serde enum (tag `cmd`, plus `Unknown`), its
/// `TAGS` and `tag()`.
macro_rules! sb_reqs {
    (
        $(#[$em:meta])*
        pub enum $E:ident {
            $( $(#[$vm:meta])* $V:ident = $tag:literal $( { $($body:tt)* } )? $( ( $inner:ty ) )? ),* $(,)?
        }
    ) => {
        $(#[$em])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "cmd")]
        pub enum $E {
            $( $(#[$vm])* #[serde(rename = $tag)] $V $( { $($body)* } )? $( ( $inner ) )?, )*
            /// a command this hub doesn't know (a newer `sb`): kept whole
            #[serde(skip)]
            Unknown { tag: String, raw: Value },
        }

        impl $E {
            /// every known command
            pub const TAGS: &'static [&'static str] = &[$($tag),*];

            /// Its command.
            pub fn tag(&self) -> &str {
                match self {
                    $( $E::$V { .. } => $tag, )*
                    $E::Unknown { tag, .. } => tag,
                }
            }
        }
    };
}

sb_reqs! {
    /// One `sb` request.
    pub enum Req {
        List = "list",
        Tasks = "tasks",
        /// `sb send <agent> "<text>"`; with `--model`/`--effort` the task
        /// moves first (issue #4)
        Send = "send" {
            #[serde(default)]
            to: String,
            #[serde(default)]
            text: String,
            #[serde(default)]
            expect_reply: bool,
            #[serde(default, with = "wire::opt_msg_id", skip_serializing_if = "Option::is_none")]
            reply_to: Option<u64>,
            /// `--mode queued`: it waits for the end of the recipient's turn
            /// (no key reads as steer; it is written always: a released
            /// `"mode":"steer"` keeps its key)
            #[serde(default, rename = "mode", with = "wire::mode")]
            queued: bool,
            /// main answering for the user: the one-sentence why
            #[serde(default, skip_serializing_if = "String::is_empty")]
            why: String,
            /// `--model`, `--effort` (either: both keys, "" the other)
            #[serde(flatten, with = "wire::switch")]
            switch: Option<(String, String)>,
            /// send-parts: `--attach`, absolute paths or `m_<n>`
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            attach: Vec<String>,
            /// send-parts: the message's parts as the hub checked them from
            /// `attach` (msg-parts JSON, "" none; `text` is their render);
            /// the daemon's (daemon/attach.rs), never on the wire
            #[serde(skip)]
            parts: String,
            /// msg-forward: `--forward m_<n>`, the message it forwards
            /// (`text`: the forwarder's note, "" none)
            #[serde(default, with = "wire::opt_msg_id", skip_serializing_if = "Option::is_none")]
            fwd: Option<u64>,
        },
        /// `sb send <task> --model <id> [--effort <e>]` without a text
        Switch = "switch" {
            #[serde(default)]
            to: String,
            #[serde(default)]
            model: String,
            #[serde(default)]
            effort: String,
        },
        Wait = "wait" {
            #[serde(with = "wire::msg_id")]
            msg: u64,
            #[serde(default = "wire::twenty")]
            timeout_s: u64,
        },
        Ask = "ask" {
            #[serde(default)]
            to: String,
            #[serde(default)]
            text: String,
            #[serde(default = "wire::twenty")]
            timeout_s: u64,
        },
        Status = "status" {
            #[serde(with = "wire::declared")]
            status: Declared,
            #[serde(default)]
            note: String,
        },
        /// `sb worktree <path>|none` (BISE-136): "none" or "" none; the
        /// hub checks the rest
        Worktree = "worktree" {
            #[serde(default)]
            path: String,
        },
        /// `sb flow [pr|trunk]` (main): "" asks, else devflow's mode
        Flow = "flow" {
            #[serde(default, skip_serializing_if = "String::is_empty")]
            set: String,
        },
        Report = "report" {
            #[serde(deserialize_with = "wire::report_kind")]
            kind: String,
            #[serde(default)]
            summary: String,
            #[serde(default)]
            decisions: Vec<String>,
            /// S10: `--step n/m`, as `step` and `of`
            #[serde(flatten, with = "wire::step")]
            step: Option<(u32, u32)>,
            /// emitter 5: `--result`, proto's DiffResult (the CLI checks
            /// it; anything but an object reads none)
            #[serde(default, deserialize_with = "wire::object", skip_serializing_if = "Option::is_none")]
            result: Option<Value>,
        },
        /// S10: `sb follow <agent> [--off]` (main only)
        Follow = "follow" {
            #[serde(default, deserialize_with = "wire::at_name")]
            agent: String,
            #[serde(default = "wire::yes", rename = "off", with = "wire::negated")]
            on: bool,
            /// J: the hub that asked through its xfollow op ("" this hub's
            /// own main); never on an agent's wire
            #[serde(skip)]
            hub: String,
        },
        Spawn = "spawn" {
            #[serde(default)]
            name: String,
            #[serde(flatten, with = "wire::brief")]
            brief: Brief,
            #[serde(default)]
            worktree: bool,
            #[serde(default)]
            with_changes: bool,
            /// `--place <agent>|<branch>` (dev-flow §3.1): "" the shared
            /// folder, or a new worktree with `worktree`
            #[serde(default, skip_serializing_if = "String::is_empty")]
            place: String,
            /// `--feature <name>` (dev-flow §5.1): "" none
            #[serde(default)]
            feature: String,
            /// `--model`, `--effort`, `--profile` (issue #4)
            #[serde(flatten)]
            ask: Ask,
            /// send-parts: `--attach`, absolute paths or `m_<n>`
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            attach: Vec<String>,
            /// send-parts: the brief's parts from `attach`, the daemon's
            #[serde(skip)]
            parts: String,
            /// msg-forward: `--forward m_<n>`, the message the brief
            /// carries as its last section
            #[serde(default, with = "wire::opt_msg_id", skip_serializing_if = "Option::is_none")]
            fwd: Option<u64>,
        },
        /// `sb feature new|sync|ready|merge|drop|retire|list [<name>]`
        Feature = "feature" {
            #[serde(default, rename = "step")]
            op: String,
            #[serde(default, skip_serializing_if = "String::is_empty")]
            name: String,
        },
        /// `sb move <agent> new|shared|<agent>|<branch>`
        Move = "move" {
            #[serde(default)]
            agent: String,
            #[serde(default)]
            place: String,
        },
        /// `sb land [--here] [--add <path>]... "<message>"`: `add` absolute
        Land = "land" {
            #[serde(default)]
            here: bool,
            #[serde(default)]
            message: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            add: Vec<String>,
        },
        Interrupt = "interrupt" {
            #[serde(default)]
            agent: String,
        },
        Stop = "stop" {
            #[serde(default)]
            agent: String,
            #[serde(default)]
            reason: String,
        },
        Drop = "drop" {
            #[serde(default)]
            agent: String,
        },
        /// `sb notify "<question>" [--for m_<n>] [--page|--artifact
        /// <id>[#<item>]] [--open <n>=<artifact>[#<anchor>]]...`
        Card = "card" {
            #[serde(default)]
            text: String,
            #[serde(default, rename = "for", with = "wire::opt_msg_id", skip_serializing_if = "Option::is_none")]
            for_msg: Option<u64>,
            /// the page (and item) the notification opens (card_link.rs)
            #[serde(default, skip_serializing_if = "Option::is_none")]
            page: Option<String>,
            /// or the artifact (and anchor) it opens
            #[serde(default, skip_serializing_if = "Option::is_none")]
            artifact: Option<String>,
            /// an option's artifact, `<n>=<artifact>[#<anchor>]`
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            open: Vec<String>,
            /// the artifact the card is about (an ArtLink's JSON): the
            /// hub's own, resolved from `page`/`artifact`; what a client
            /// sends here is overwritten (daemon/page_cards.rs)
            #[serde(default, skip_serializing_if = "Option::is_none")]
            about: Option<Value>,
        },
        /// `sb close N ["note"]`
        Close = "close" {
            card: u64,
            #[serde(default)]
            note: String,
        },
        /// `sb notify --withdraw N "why"`
        Withdraw = "withdraw" {
            card: u64,
            #[serde(default)]
            why: String,
        },
        Rename = "rename" {
            #[serde(default)]
            agent: String,
            #[serde(default)]
            new_name: String,
        },
        Restore = "restore" {
            #[serde(default)]
            agent: String,
        },
        Isolate = "isolate" {
            #[serde(default)]
            agent: String,
        },
        /// `sb every`: the hub's timers (every.rs)
        Every = "every" (Every),
        /// `sb wake`: the caller's watches (wake.rs)
        Wake = "wake" (Wake),
        /// `sb project list` (bise's main, desktop S2)
        ProjectList = "project_list",
        /// `sb project send <project> --input m_<n>`
        ProjectSend = "project_send" {
            #[serde(default)]
            project: String,
            #[serde(with = "wire::msg_id")]
            msg: u64,
        },
        /// `sb project ask <project> "<question>"`
        ProjectAsk = "project_ask" {
            #[serde(default)]
            project: String,
            #[serde(default)]
            text: String,
        },
        /// `sb inspect <agent>` (`<p>/<agent>`: another project's)
        Inspect = "inspect" (Inspect),
        /// `sb history "<words>"`
        History = "history" (History),
        /// `sb show <agent>#<pos>`
        Show = "show" {
            #[serde(default)]
            agent: String,
            #[serde(default)]
            pos: u64,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            context: Option<u64>,
            /// msg-forward: `sb show m_<n>`: message n's entry in its
            /// recipient's thread (then `agent` and `pos` are unset)
            #[serde(default, with = "wire::opt_msg_id", skip_serializing_if = "Option::is_none")]
            msg: Option<u64>,
        },
        /// `sb taste`: ~/bise/taste.md
        Taste = "taste" (Keep),
        /// `sb people`: ~/bise/people.md
        People = "people" (Keep),
        Artifact = "artifact" (Artifact),
        PagePublish = "page_publish" (PagePublish),
        PageStart = "page_start" {
            #[serde(default)]
            id: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            title: Option<String>,
            /// its writer ("" or none: the caller)
            #[serde(default, skip_serializing_if = "Option::is_none")]
            agent: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            ask: Option<String>,
        },
        PageWaiting = "page_waiting",
        PageTick = "page_tick" {
            #[serde(default)]
            id: String,
            #[serde(default)]
            item: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            text: Option<String>,
        },
        PageList = "page_list",
        PageNotes = "page_notes" {
            #[serde(default)]
            id: String,
        },
    }
}

impl Req {
    /// One request: a known command as its variant (an error when its
    /// fields don't fit: the sender's), any other as `Unknown`.
    pub fn decode(v: Value) -> Result<Req, String> {
        crate::decode(v, "cmd", Self::TAGS, |tag, raw| Req::Unknown { tag, raw })
    }

    /// The hub's read: a command it doesn't know is the sender's error,
    /// from an sb newer than this hub.
    pub fn decode_known(v: Value) -> Result<Req, String> {
        match Req::decode(v)? {
            Req::Unknown { tag, .. } => Err(format!("unknown command: {tag} (sb is newer than this hub)")),
            r => Ok(r),
        }
    }

    /// A notification of `text` alone (no `--for`, no link): the hub's
    /// own (a page's question, a step of his).
    pub fn card(text: impl Into<String>) -> Req {
        Req::card_about(text, None)
    }

    /// The same, about an artifact the hub resolved (a page at its
    /// block: the card's `about`, an ArtLink's JSON).
    pub fn card_about(text: impl Into<String>, about: Option<Value>) -> Req {
        Req::Card { text: text.into(), for_msg: None, page: None, artifact: None, open: Vec::new(), about }
    }

    /// Its JSON (`Unknown`: the value it was read from).
    pub fn to_value(&self) -> Value {
        match self {
            Req::Unknown { raw, .. } => raw.clone(),
            r => serde_json::to_value(r).expect("an sb request always serializes"),
        }
    }
}

/// `m_12` or `12`: the message id.
pub fn parse_msg_id(s: &str) -> Option<u64> {
    s.trim().trim_start_matches("m_").parse().ok()
}

/// The status an agent declares about itself, and the task brief: the
/// state's own types (one each; on the spawn's wire `wire::brief` writes
/// a none as "").
pub use crate::core::state::{Brief, Declared};

/// What main asked at the spawn (issue #4). All "": nothing asked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub profile: String,
}

impl Ask {
    pub fn is_empty(&self) -> bool {
        self.model.trim().is_empty() && self.effort.trim().is_empty() && self.profile.trim().is_empty()
    }
}

/// `sb every`'s forms, tagged `step`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Every {
    List,
    Stop {
        id: u64,
    },
    /// `--show <id>`: its line and the words it sends
    Show {
        id: u64,
    },
    /// its schedule is `every_ms` or `daily_min` (every.rs reads them)
    Add {
        /// "" the caller
        #[serde(default, deserialize_with = "wire::at_name")]
        to: String,
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        every_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        daily_min: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        until_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        times: Option<u64>,
        #[serde(default, deserialize_with = "wire::nonempty", skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        /// none: the hub names it (every_name.rs)
        #[serde(default, deserialize_with = "wire::nonempty", skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// `sb wake`'s forms, tagged `step` (only the caller's watches).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Wake {
    List,
    /// `--stop <id>`, or `--stop bg/<n>` (`bg`: the watch of the caller's
    /// background command n)
    Stop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bg: Option<String>,
    },
    /// `spec`: wake.rs's Spec JSON; `max_ms` none: its default; `then`:
    /// `--then` / `--alarm` (none: sb-core's default by kind)
    Add {
        #[serde(default)]
        spec: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        then: Option<bool>,
    },
}

/// `sb inspect`: a page of a thread, or (`origin`) the caller's creation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inspect {
    #[serde(default)]
    pub agent: String,
    /// at most this many entries (none: the hub's default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<u64>,
    #[serde(default)]
    pub query: String,
    /// the anchors, `#<pos>`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub around: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default)]
    pub origin: bool,
}

/// `sb history`: a search of every thread.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub agents: Vec<String>,
    /// user|assistant|message|tool|hub
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
    /// "only", "no", "" both
    #[serde(default)]
    pub archived: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,
    /// S2 C (bise's home hub): another project's threads, or every one's
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub all: bool,
}

/// `sb taste` / `sb people`'s forms, tagged `step`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Keep {
    List,
    /// taste: a rule and where he said it
    Add {
        #[serde(default)]
        rule: String,
        #[serde(default)]
        source: String,
    },
    /// people: a person and who they are
    Set {
        #[serde(default)]
        name: String,
        #[serde(default)]
        who: String,
    },
    /// taste: `which` (a number or words); people: `name`
    Remove {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        which: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        name: String,
    },
}

/// `sb artifact`'s forms, tagged `do`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Artifact {
    /// a path (relative to `cwd`) or a link
    Add {
        #[serde(default)]
        target: String,
        #[serde(default)]
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        /// office (harness/src/office.rs): a next version of an artifact
        /// that already is one, never a new one
        #[serde(default, skip_serializing_if = "crate::is_false")]
        known_only: bool,
        /// office: what the write changed (`office::Changed::of_value`
        /// reads it; anything but a list is none)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed: Option<Value>,
        /// `--folder <name|id|theme/task>`: a library folder, made by
        /// the agent when the path is not there
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// `sb artifact move <id>... <folder> | --unfiled`
    Move {
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// `sb artifact folders`: the library's folders, one line each
    Folders,
    List {
        #[serde(default)]
        words: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default)]
        archived: bool,
    },
    Archive(ArtifactPick),
    Unarchive(ArtifactPick),
}

/// Which artifacts an archive or unarchive takes: its ids, or those of an
/// agent, a kind, made before a time.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPick {
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_ms: Option<u64>,
}

/// `sb page publish <file>`: the page's HTML, read by the CLI.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PagePublish {
    #[serde(default)]
    pub html: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// `--notes-done`: the notes it answered
    #[serde(default)]
    pub done: Vec<String>,
    /// `--note-answer n2="…"`: a line for a note it didn't do
    #[serde(default)]
    pub answers: BTreeMap<String, String>,
    /// `--went`: pages/store.rs's Went JSON, read by the hub
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub went: Vec<Value>,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub taste: bool,
    /// `--public` / `--private`; none: as it was
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}

#[cfg(test)]
mod tests;
