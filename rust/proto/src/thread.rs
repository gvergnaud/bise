//! An agent's thread as entries: his messages, its replies, messages from
//! other agents, its cards, reports and pages, and its tool calls folded
//! (consecutive calls are one `tools` entry at the first call's pos). The
//! hub folds its transcript lines (`<pos, ms, line>`) with [`fold`]; a
//! live line refolds the newest lines, and an entry whose JSON changed is
//! sent again at the same pos.
//!
//! Here: the entry types and [`page`]. [`lines`] reads each transcript
//! line kind into a typed record, the one parser the TUI calls too;
//! [`words`] makes the words both sides show; [`fold`] turns the records
//! into entries.

use crate::context::FnContext;
use crate::rows::{Opt, ReportKind};
use crate::{is_false, Pos};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// his message
    You,
    /// the agent's own words
    Agent,
    /// a message from another agent (`from`)
    FromAgent,
    /// its tool calls in a row
    Tools,
    Card,
    Page,
    Report,
    /// its turn was stopped (an interrupt: sb-core's `stopped` line, written
    /// in the same step as the interrupt)
    Stopped,
    /// a message this agent sent another one (`to`; `asks`: it waits
    /// for the reply, its question), sb-core's `sent` line
    ToAgent,
    /// the model's reasoning with no words after it (a tool-call-only
    /// turn; `thinking`: how long, the text), folded as "thought for
    /// 3.2s". A reply's own thinking rides on its `agent` entry.
    Thinking,
    /// a compaction started (`≡ compacting`)
    Compacting,
    /// a compaction's summary (the text), folded
    Compacted,
    /// a line of the runtime or the hub for him (`notice`: info, warn or
    /// err, the words): a failed turn and why, a retry, a warning
    Notice,
    /// his message didn't reach its agent (`not_delivered`: to whom, his
    /// text, never lost)
    NotDelivered,
    /// an agent's commits landed (`landed`: on what, the sha, the diff's
    /// size), the text its words: `3 files +42 −18`
    Landed,
    /// news of a pull request (`pr`: its number, link, words, state)
    Pr,
    /// an agent made or changed an artifact (`made`)
    Artifact,
    /// main answered an agent for him (`answered`: the question, the
    /// answer, why)
    Answered,
    /// his answer to an approval card or an item, folded (`approval`:
    /// yes or no, the sentence, the words under it)
    Approval,
    /// a scheduled task set, ended, or one of its runs (`scheduled`: its
    /// id, the line's head, the task's words)
    Scheduled,
    /// R12: its turn failed (`turn_failed`: why, as the runtime said it);
    /// the text the TUI's words for it. An interrupt is not one (a
    /// `stopped` entry or a notice, as before)
    TurnFailed,
    /// a kind this reader doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// A `turn_failed` entry (R12): why the turn failed, the runtime's words
/// (a provider's error, a missing key, a budget).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TurnFailed {
    pub why: String,
}

/// A `thinking` entry: how long the model thought (ms, 0 unknown: a
/// replay) and what.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Thinking {
    pub ms: u64,
    pub text: String,
}

/// How a notice reads: info (dim), warn (▲, what to do), err (✗).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    Info,
    Warn,
    Err,
    #[serde(other)]
    Unknown,
}

/// A `notice` entry: its level and its words ([`words::notice`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Notice {
    pub level: NoticeLevel,
    pub text: String,
}

/// A `not_delivered` entry: the agent his message didn't reach, and his
/// text (BISE-86: send again or drop it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotDelivered {
    pub to: String,
    pub text: String,
}

/// A `landed` entry (site/m/artifacts D): `agent`'s commits from `from`
/// landed on `target` at `sha`; the diff's size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Landed {
    pub agent: String,
    pub target: String,
    pub from: String,
    pub sha: String,
    pub files: u64,
    pub add: u64,
    pub del: u64,
}

/// What a PR's news means for him (pr-design §4), never a color: `news`
/// (opened, reviewed, merged...), `done` (nothing for him to do),
/// `failing` (its checks fail). The TUI maps it to its look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrNewsState {
    News,
    Done,
    Failing,
    /// a state this reader doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// A `pr` entry: the PR's number, its link, the news's words.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrNews {
    pub number: u64,
    pub url: String,
    pub text: String,
    pub state: PrNewsState,
}

/// An `artifact` entry (site/m/artifacts C): what was made; `kind_word`
/// as the TUI's chip says it (`page`, `PR`, `file`), `url` when it is a
/// page the hub serves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Made {
    pub id: String,
    pub agent: String,
    pub title: String,
    pub kind: String,
    pub kind_word: String,
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// An `answered` entry: main answered `agent`'s question for him.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Answered {
    pub agent: String,
    pub question: String,
    /// his words only, never an image marker or the file list (R41)
    pub answer: String,
    pub why: String,
    /// the images he pasted with it: the window shows each as its chip
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageRef>,
    /// his other files (absolute paths)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// An image he attached: its label (`[Image #1]`, or its file name) and
/// his source path (the chip's thumbnail). Never the store's b64 path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImageRef {
    pub name: String,
    pub path: String,
}

/// A text with his attached files rendered in it, read back (the hub's
/// `attached::split`, given to the fold as [`Ctx::attached`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attached {
    pub words: String,
    pub images: Vec<ImageRef>,
    pub files: Vec<String>,
}

impl Attached {
    /// A text with nothing attached: its words (tests, a client with no
    /// image store).
    pub fn plain(text: &str) -> Attached {
        Attached { words: text.to_string(), ..Attached::default() }
    }
}

/// An `approval` entry: his answer, folded (`you allowed api: rm -rf
/// target`, `you answered perf: both`); `note` the words under it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ApprovalFold {
    pub ok: bool,
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// an answer's pasted images (`you answered gift-ui` + his words,
    /// R41): each the window's chip, never a marker in `text`/`note`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageRef>,
    /// an answer's other files (absolute paths)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// A `scheduled` entry (batch 3b): a task set or ended (the hub's
/// `scheduled` line) or a run (bise's wake), as the TUI's ◷ line says it:
/// `head` (`perf scheduled #48 · every 2m · 6 times · next 14:22`,
/// `scheduled #48 · 2 of 6 · check the build`, `scheduled #48 ended ·
/// stopped by you`), `words` the task's words under it ("" when ended).
/// Times are read at the line's own time, so a replay says the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Scheduled {
    pub id: u64,
    pub head: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub words: String,
}

/// Where a tool call is: running, done, failed (R11, amb-win S9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    Run,
    Ok,
    Err,
    /// a state this version doesn't know (a newer hub), or none sent (an
    /// older hub: never drawn as running forever, architect m_13326)
    #[default]
    #[serde(other)]
    Unknown,
}

/// A file an edit touched, with its lines added and removed (the TUI's
/// `± file +18 −6`; a move reads `old → new`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileCount {
    pub path: String,
    pub add: u32,
    pub del: u32,
}

/// At most this many bytes of a tool call's `code` and `out` go in an
/// entry (a write_file's args can be a whole file, in every event).
pub const TOOL_TEXT_CAP: usize = 4096;

/// `s` cut to [`TOOL_TEXT_CAP`] bytes on a char boundary, `…` after a cut.
pub fn cap(s: &str) -> String {
    if s.len() <= TOOL_TEXT_CAP {
        return s.to_string();
    }
    let mut end = TOOL_TEXT_CAP;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// What a tool call did, for the counted summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit,
    Run,
    Search,
    Other,
}

impl ToolKind {
    pub fn of(name: &str) -> ToolKind {
        match name {
            "read_file" | "read" | "view" | "read_image" => ToolKind::Read,
            "edit" | "write_file" | "write" | "apply_patch" | "multi_edit" => ToolKind::Edit,
            "bash" | "run_typescript" | "shell" => ToolKind::Run,
            "grep" | "rg" | "glob" | "search" | "find" | "web_search" | "search_tool_functions" => ToolKind::Search,
            _ => ToolKind::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ToolItem {
    pub pos: Pos,
    pub at_ms: u64,
    /// its intent ("running the tests"), else `name: first line of args`
    pub text: String,
    pub kind: ToolKind,
    /// a `sb land`
    #[serde(default, skip_serializing_if = "is_false")]
    pub land: bool,
    /// running until its result line, then ok or err
    #[serde(default)]
    pub state: ToolState,
    /// its duration: the result line's time minus the call line's
    /// ([`words::tool_ms`]); none while it runs or on a replay
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    /// a failed bash call's exit code (`exit 1: …`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// a failed call's first error line, the TUI's row under it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    /// the full command or args (the runtime's tool_code), [`cap`]ped
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// its output as the result line gives it, [`cap`]ped
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    /// an edit's files with their line counts (the window formats
    /// `+18 −6` from them: counts, like the folds)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileCount>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Tools {
    pub count: u32,
    /// counted words: "read 6 files, ran 4 commands"
    pub summary: String,
    pub items: Vec<ToolItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EntryCard {
    pub id: u64,
    pub question: String,
    pub options: Vec<Opt>,
    pub answered: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PageRef {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReportRef {
    pub kind: ReportKind,
}

/// One entry of a thread.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Entry {
    /// its first line's transcript position (`<agent>#<pos>`)
    pub pos: Pos,
    pub at_ms: u64,
    pub kind: EntryKind,
    /// markdown
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Tools>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<EntryCard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<PageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportRef>,
    /// his message's fn context (S9): what was on his screen when he
    /// spoke, from the `sb context : <json>` line right after his 'you'
    /// line (the window draws the shot and the app on his message)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<FnContext>,
    /// a `to_agent` entry: who it went to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// a `to_agent` entry: it waits for the reply (its question)
    #[serde(default, skip_serializing_if = "is_false")]
    pub asks: bool,
    /// a `to_agent` entry: its message id (`m_12` is 12; the card main
    /// opens for it says `for` the same)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<u64>,
    /// a `thinking` entry's, or an `agent` reply's own: the reasoning
    /// before it ("thought for 3.2s", folded)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<Notice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_delivered: Option<NotDelivered>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed: Option<Landed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrNews>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<Made>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered: Option<Answered>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalFold>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<Scheduled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_failed: Option<TurnFailed>,
}

impl Entry {
    fn new(pos: Pos, at_ms: u64, kind: EntryKind, text: String) -> Entry {
        Entry {
            pos,
            at_ms,
            kind,
            text,
            from: None,
            tools: None,
            card: None,
            page: None,
            report: None,
            context: None,
            to: None,
            asks: false,
            msg: None,
            thinking: None,
            notice: None,
            not_delivered: None,
            landed: None,
            pr: None,
            made: None,
            answered: None,
            approval: None,
            scheduled: None,
            turn_failed: None,
        }
    }

    /// Law (architect m_10476): an entry of a kind with a payload carries
    /// exactly that payload, and no other kind's: `pr` never comes with
    /// `pr: None`. The kinds without a payload carry none, but for an
    /// `agent` reply's own thinking (one line, one entry: pos is the key).
    pub fn payload_matches_kind(&self) -> bool {
        let has = [
            (EntryKind::Tools, self.tools.is_some()),
            (EntryKind::Card, self.card.is_some()),
            (EntryKind::Page, self.page.is_some()),
            (EntryKind::Report, self.report.is_some()),
            (EntryKind::Notice, self.notice.is_some()),
            (EntryKind::NotDelivered, self.not_delivered.is_some()),
            (EntryKind::Landed, self.landed.is_some()),
            (EntryKind::Pr, self.pr.is_some()),
            (EntryKind::Artifact, self.made.is_some()),
            (EntryKind::Answered, self.answered.is_some()),
            (EntryKind::Approval, self.approval.is_some()),
            (EntryKind::Scheduled, self.scheduled.is_some()),
            (EntryKind::TurnFailed, self.turn_failed.is_some()),
        ];
        let thinking = match self.kind {
            EntryKind::Thinking => self.thinking.is_some(),
            EntryKind::Agent => true,
            _ => self.thinking.is_none(),
        };
        thinking && has.iter().all(|(k, set)| *set == (self.kind == *k))
    }
}

/// One transcript line: its position, its time (ms, 0 unknown), the line.
pub type Line = (Pos, u64, String);

/// What the fold needs from the hub: the open cards (an entry's card says
/// answered when it is not open), the pages (a publish's title, version
/// and url).
pub struct Ctx<'a> {
    pub open_cards: &'a [u64],
    pub page: &'a dyn Fn(&str) -> Option<PageRef>,
    /// a provider's name for people, by its id or its key variable (a
    /// missing key's notice; the catalog's `provider_name`)
    pub provider: &'a dyn Fn(&str, &str) -> String,
    /// a string's display width (unicode-width; this crate stays pure
    /// std): where an answer stays on its line (`words::answered_split`)
    pub width: &'a dyn Fn(&str) -> usize,
    /// the local UTC offset (seconds east) at a moment: a scheduled
    /// task's clock times (`thread::when`; this crate has no time zone)
    pub offset: &'a dyn Fn(u64) -> i32,
    /// a text with his attached files rendered in it read back (the hub's
    /// `attached::split` over the image markers; this crate has no image
    /// parser): an answer's words, images and files (R41)
    pub attached: &'a dyn Fn(&str) -> Attached,
}

/// A page of a thread: the newest `limit` entries of `lines`. `more`: the
/// lines don't start the transcript (the oldest entry, maybe cut, is
/// dropped) or entries were left out; `before`: the oldest entry's pos
/// when there is more (ask `page {before}` for the ones before it).
pub fn page(lines: &[Line], ctx: &Ctx, limit: usize) -> (Vec<Entry>, Option<Pos>, bool) {
    let older = lines.first().is_some_and(|l| l.0 > 1);
    let mut entries = fold(lines, ctx);
    if older && entries.len() > 1 {
        entries.remove(0);
    }
    let more = older || entries.len() > limit;
    let entries = entries.split_off(entries.len().saturating_sub(limit));
    let before = entries.first().map(|e| e.pos).filter(|_| more);
    (entries, before, more)
}

pub mod fold;
pub mod lines;
pub mod scheduled;
pub mod when;
pub mod words;
pub use fold::fold;
