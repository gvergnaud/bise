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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    #[default]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrChecks {
    Pass,
    Fail,
    Running,
    /// none reported yet
    #[default]
    None,
    #[serde(other)]
    Unknown,
}

/// Its review.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrReview {
    Approved,
    Changes,
    #[default]
    None,
    #[serde(other)]
    Unknown,
}

/// The forge a PR lives on (pr-integration rule 14): its words and
/// sigil (`PR #412` / `MR !412`, checks / pipeline, Open on GitHub /
/// GitLab) are picked by this, never by a host the client reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrProvider {
    #[default]
    Github,
    Gitlab,
    #[serde(other)]
    Unknown,
}

impl PrProvider {
    pub fn is_github(&self) -> bool {
        *self == PrProvider::Github
    }
}

/// What a PR waits on him for (pr-integration rule 6): his review is
/// asked, or only his merge is missing (approved or no review needed,
/// checks pass, the forge would merge it). The row's pink and the
/// inbox item read this one field (`forge::yours`, its one owner).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrYours {
    Review,
    Merge,
    #[serde(other)]
    Unknown,
}

/// What a part of a PR's words says, for its tone: `yours` waits on him
/// (the accent), `fail` a failing check (err), `plain` the rest (dim).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrTone {
    Yours,
    Fail,
    Plain,
    #[serde(other)]
    Unknown,
}

/// One part of a PR's words (`Pr.pieces`): its text, from the catalog
/// as `words` has it, and its tone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrPiece {
    pub text: String,
    pub tone: PrTone,
}

/// A pull request the hub follows (the TUI's `/prs` row, a place's PR,
/// a PR artifact's status: one builder, `forge::news::pr_row`): what it
/// means, and the hub's own words for it (never rebuilt by a client).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// the same words as typed parts, in the order a client draws them
    /// (what waits on him first, designer m_54748): a client tones them
    /// by `tone`, never by reading their text (architect m_54755). Empty
    /// from an older hub: the client draws `words` as they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<PrPiece>,
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
    /// its title on the forge ("" before the forge said it)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// the branch it merges into (`main`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base: String,
    /// its repo, `owner/name`
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String,
    /// its forge; absent: GitHub (the default, as before GitLab)
    #[serde(default, skip_serializing_if = "PrProvider::is_github")]
    pub provider: PrProvider,
    /// its number with the forge's sigil: `#412` (GitHub), `!412` (GitLab)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,
    /// the forge would merge it now (the Merge button shows only then)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub mergeable: bool,
    /// it conflicts with its base
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub conflicts: bool,
    /// its head's checks, and how many of them finished
    #[serde(default, skip_serializing_if = "super::is_zero")]
    pub checks_n: u32,
    #[serde(default, skip_serializing_if = "super::is_zero")]
    pub checks_done: u32,
    /// it waits on him: his review, or his merge
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yours: Option<PrYours>,
    /// when its head commit was made (ms): its last push
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_ms: Option<u64>,
    /// its head commit as the forge said: his Merge or Close on the tab
    /// is for this one (`pr_act.head`; a push since is refused)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head_oid: String,
}

/// What a PR's tab reads besides its row (`pr_detail`): its description,
/// its head's checks one by one, what people said, newest first. Never
/// its state, its checks' sum or whose turn it is: those are the row's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrDetail {
    /// its description, markdown from the forge (untrusted: drawn with
    /// HTML skipped), cut at the hub's cap
    pub body: String,
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub body_cut: bool,
    pub checks: Vec<PrCheck>,
    /// reviews and comments, newest first, at most the hub's cap
    pub notes: Vec<PrNote>,
    /// how many more the forge has (`N more on GitHub`)
    #[serde(default, skip_serializing_if = "super::is_zero")]
    pub more_notes: u32,
}

/// One check of the PR's head.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrCheck {
    pub name: String,
    pub state: PrCheckState,
    /// its page on the forge
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrCheckState {
    Pass,
    Fail,
    Running,
    /// skipped, neutral, cancelled: not a failure
    Skipped,
    #[default]
    #[serde(other)]
    Unknown,
}

/// A review or a comment on the PR.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrNote {
    /// the forge login
    pub author: String,
    /// markdown from the forge (untrusted), cut at the hub's cap
    pub body: String,
    pub at_ms: u64,
    /// a review's verdict; none: a comment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<PrVerdict>,
}

/// What a review said.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrVerdict {
    Approved,
    Changes,
    Commented,
    #[serde(other)]
    Unknown,
}

/// Why a PR's detail or a check's log didn't come (`pr_detail.err`,
/// `pr_check_log.err`), typed: the client says it in its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PrErr {
    /// the forge didn't answer in the hub's time
    Late,
    /// gh is missing or logged out
    NoForge,
    /// the forge refused or failed: its words in `why`
    Forge,
    /// that check has no log the hub can read (not a GitHub Actions run)
    NoLog,
    #[serde(other)]
    Unknown,
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
    /// its 1-based rank in the land queue's line (1: it lands now, its
    /// lid `landing`; 2: `waits to land · 2nd`); none when not in line
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<u32>,
    /// a feature branch's place (dev-flow §5.1): never a PR
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub feature: bool,
    /// its try build builds or is on trial
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub trying: bool,
    /// the worktree's folder (the ψ's hover names it); none for a feature
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// page pr-guided V2: its commits not on its open PR yet (`sb/perf
    /// has 2 commits not on #412`); none when 0, unknown, or no open PR
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead_of_pr: Option<u32>,
}

#[cfg(test)]
mod place_tests {
    use super::Place;

    /// page pr-guided V2 (architect m_58480): `ahead_of_pr` reads and
    /// writes back; None is never written (the hub sends a count > 0
    /// only); a Place of an older hub, without the key, reads None.
    #[test]
    fn ahead_of_pr_round_trips_and_an_older_place_reads_none() {
        let old = r#"{"id":"wt:dark","branch":"sb/dark","agents":["perf"]}"#;
        let p: Place = serde_json::from_str(old).unwrap();
        assert_eq!(p.ahead_of_pr, None);
        assert_eq!(serde_json::to_string(&p).unwrap(), old, "None is skipped");
        let two = Place { ahead_of_pr: Some(2), ..p };
        let j = serde_json::to_value(&two).unwrap();
        assert_eq!(j["ahead_of_pr"], 2);
        assert_eq!(serde_json::from_value::<Place>(j).unwrap(), two);
    }
}

