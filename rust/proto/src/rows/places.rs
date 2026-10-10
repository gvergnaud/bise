//! The project's places: merged commits, dev servers, worktrees,
//! features, PRs and the panel's places.

use serde::{Deserialize, Serialize};

/// A commit that reached the project's trunk today (the changes tab's
/// "merged today", amb-web m_8974).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Merged {
    /// the full sha
    pub sha: String,
    /// its subject line
    pub title: String,
    /// the agent whose land brought it (sb land, the merge path); absent:
    /// not landed through bise (his own commit), or not known
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// its commit time
    pub at_ms: u64,
}

/// A server an agent runs in the background (`pnpm dev`, a local
/// grafana): its job listening on a port, or a known server command not
/// listening yet (no port, no url).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DevServer {
    pub agent: String,
    /// what it is ("dev server", "http server")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `http://localhost:<port>` once it listens
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// the job's command line
    pub cmd: String,
    /// its job runs
    pub up: bool,
}

/// A worktree of the project's repo: an agent's, or one of his own (the
/// main checkout included).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Worktree {
    pub path: String,
    /// its branch ("" when detached)
    pub branch: String,
    /// the full sha it sits at ("detached at e0f3df5", amb-web m_8951)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// the agent working there (none: his own)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// changes not committed, untracked files included
    pub dirty: bool,
    /// commits ahead of and behind the trunk
    pub ahead: u32,
    pub behind: u32,
}

/// A feature's last try build (`sb feature` / the try card's "try it").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeatureTry {
    /// the tip built (short)
    pub sha: String,
    /// what to run, in his words (`~/.bise/dev/versions/fd25c45/bise`,
    /// or `git checkout <name>` when the repo has no try command)
    pub run: String,
    pub at_ms: u64,
}

/// A feature branch (dev-flow §5.1): several agents land on it, he tries
/// it, it merges into the trunk only on his answer. Its actions are the
/// answers of its open card (`card`, kind `feature_try` or
/// `feature_merge`) through `answer`, as in the TUI: no command of its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Feature {
    pub name: String,
    /// its local branch (never pushed)
    pub branch: String,
    /// the trunk it leaves and merges into (`main`)
    pub base: String,
    /// its live agents, in the hub's order
    pub agents: Vec<String>,
    /// commits it has that the trunk lacks, and the other way
    pub ahead: u32,
    pub behind: u32,
    /// lines added and removed against the trunk
    pub adds: u32,
    pub dels: u32,
    /// its tip (full sha; "" before git was read)
    pub tip: String,
    /// the repo's check passed on this tip
    pub checked: bool,
    /// its last try build
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tried: Option<FeatureTry>,
    /// on trial: built, and he hasn't said merge, keep working or drop yet
    pub trial: bool,
    /// a try build runs now
    pub building: bool,
    /// an existing local branch `sb feature new` took as it was
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub adopted: bool,
    pub created_ms: u64,
    /// its open card (`feature_try`: try it / show the diff / not yet;
    /// `feature_merge`: merge / keep working / drop the branch, the drop
    /// asked once more as the TUI does), the id `answer` takes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<u64>,
}

/// An open pull request's state (merged and closed ones aren't listed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    Draft,
    /// P4c-4a: a place's PR only (`/prs` lists open and draft ones): a
    /// merged place goes a tick later, a closed one keeps its box
    Merged,
    Closed,
    /// a state this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// Its checks on the head commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrChecks {
    Pass,
    Fail,
    Running,
    /// none reported yet
    None,
    #[serde(other)]
    Unknown,
}

/// Its review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrReview {
    Approved,
    Changes,
    None,
    #[serde(other)]
    Unknown,
}

/// An open pull request of the project's places (the TUI's `/prs` row):
/// what it means, and the hub's own words for it (never rebuilt by a
/// client).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Pr {
    pub number: u64,
    pub url: String,
    pub branch: String,
    /// the agents of its place, in the hub's order
    pub agents: Vec<String>,
    pub state: PrState,
    pub checks: PrChecks,
    /// the failing checks' names, when `checks` is `fail`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing: Vec<String>,
    pub review: PrReview,
    /// its state in words: `changes asked · checks pass`, `draft · checks running`
    pub words: String,
    /// its `/prs` line after `#<number>`: `sb/x · x, y · <words>`
    pub text: String,
    /// P4c-4a: a review was asked and none came yet (`review` stays
    /// `none` then, as it always was: the wire rule)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub in_review: bool,
    /// a place's PR (`HubEv::Agents.places`): how old the forge's last
    /// answer is, when it is late (offline, rate limit); none when fresh,
    /// and always none in `/prs`'s rows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_ms: Option<u64>,
}

/// A worktree of the project as the terminal's panel draws it (dev-flow
/// §3.1, the `places` of the hub's snapshot, P4c-4a): a worktree its
/// agents share, a feature branch, a solo agent's own worktree. Never
/// the shared checkout. In its first agent's order; an agent's
/// `place_id` is its `id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Place {
    /// `wt:<branch>`, `pt:<path>` (a private worktree), `feature:<name>`
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// its agents, not archived, in the hub's order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// the PR of its branch (merged and closed ones too, until it goes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<Pr>,
    /// the held line the hub writes (`waits to land · 2nd`, `no PR yet ·
    /// 2 commits`): it wins over the PR's words
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,
    /// a feature branch's place (dev-flow §5.1): never a PR
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub feature: bool,
    /// its try build builds or is on trial
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub trying: bool,
}

