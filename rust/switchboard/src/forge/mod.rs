//! Pull requests (pr-design §7-§10): the hub follows the PR of each
//! place's branch by polling the forge, never with a token of its own
//! (GitHub through `gh`, which has the user's login).
//!
//! - this module: the [`Forge`] seam, the repo of the workspace, and
//!   [`diff`], what changed between two answers (the hub's PR events);
//! - [`github`]: `gh api graphql`, one query per repo, an alias per branch
//!   (25 aliases measured at 1 point, like 2);
//! - [`poll`]: the cadence and back-off (§7) and the worker that asks,
//!   on its own thread (a query takes ~0.5 s: never on the hub's loop);
//! - [`artifact`]: a PR as an artifact of kind `pr` (pr-integration), the
//!   store's ops for a snapshot; [`pr_link`], the one key of a PR's link;
//!   [`yours`], what a PR waits on him for.
//!
//! The hub's side (`core.rs`, `Input::Prs`): the snapshots by place id
//! (`Hub::prs`), the journal lines that keep each place's PR number, the
//! held line of a place with no PR yet, and a merged PR's cleanup.
//! `activity` (reviews, comments) and `merge` join [`Forge`] with pr-news
//! and pr-merge (wave 3).

pub mod artifact;
pub mod github;
pub mod news;
pub mod poll;

use crate::place::{MergeMethod, PrSnapshot, PrState};

/// A repository on a forge: `github.com/owner/name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    pub host: String,
    pub owner: String,
    pub name: String,
}

/// The repo of a remote's URL: `https://host/o/r(.git)`,
/// `git@host:o/r(.git)`, `ssh://git@host(:port)/o/r(.git)`.
pub fn repo_of_url(url: &str) -> Option<RepoRef> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.split_once("://").map(|(_, r)| r) {
        let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?.to_string(), path.to_string())
    } else {
        // scp-like: git@host:o/r
        let (user_host, path) = url.split_once(':')?;
        let host = user_host.rsplit_once('@').map_or(user_host, |(_, h)| h);
        (host.to_string(), path.to_string())
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    if host.is_empty() || owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(RepoRef { host: host.to_lowercase(), owner: owner.into(), name: name.into() })
}

/// Why the forge did not answer (pr-design §7: the hub backs off, the
/// boxes keep their last state, faint; never an inbox item).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForgeError {
    /// No `gh` on the hub's PATH.
    Missing,
    /// gh is not logged in, or its login is refused (401).
    Auth(String),
    RateLimited(String),
    /// No network, GitHub down.
    Offline(String),
    /// gh didn't answer in the time it was given: it was stopped.
    Late,
    Other(String),
}

impl ForgeError {
    /// One line for hub.log and `bise doctor` (never gh's whole output).
    pub fn describe(&self) -> String {
        match self {
            ForgeError::Missing => "gh is not installed".into(),
            ForgeError::Auth(s) => format!("gh is not logged in ({})", s),
            ForgeError::RateLimited(s) => format!("GitHub's rate limit ({})", s),
            ForgeError::Offline(s) => format!("GitHub unreachable ({})", s),
            ForgeError::Late => "GitHub didn't answer in time".into(),
            ForgeError::Other(s) => s.clone(),
        }
    }
}

/// One forge (GitHub now, GitLab later: pr-design §9).
pub trait Forge: Send {
    /// The latest PR whose head is each of `branches` (none for a branch
    /// without one), and each PR of `numbers` (pr-integration rule 2: a
    /// PR an agent only reviews, followed by its artifact): one call for
    /// them all.
    fn fetch(&self, repo: &RepoRef, branches: &[String], numbers: &[u64]) -> Result<Answer, ForgeError>;

    /// pr-news (pr-design §6.1, §6.2): the PR's reviews and comments, and
    /// for `logs` the failing checks with their log's tail. Asked only
    /// when the PR changed (`updatedAt` moved). The default: nothing (a
    /// forge that has no such details).
    fn activity(&self, _repo: &RepoRef, _pr: &PrSnapshot, _logs: bool) -> Result<Activity, ForgeError> {
        Ok(Activity::default())
    }

    /// pr-design §6.3: merge PR `number` with `method`, only while its
    /// head is still `head` (a push since the user said yes: no merge),
    /// with the user's login. Only the user's `1` on the ready-to-merge
    /// item calls it: no agent ever merges.
    fn merge(&self, _repo: &RepoRef, _number: u64, _head: &str, _method: MergeMethod) -> Result<(), ForgeError> {
        Err(ForgeError::Other("this forge can't merge".into()))
    }

    /// pr-integration: close PR `number` with the user's login. Only his
    /// click on the PR's tab calls it.
    fn close(&self, _repo: &RepoRef, _number: u64) -> Result<(), ForgeError> {
        Err(ForgeError::Other("this forge can't close".into()))
    }
}

/// One answer of [`Forge::fetch`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    /// The branches' PRs.
    pub prs: Vec<PrSnapshot>,
    /// Each number asked, its PR; None: it no longer resolves (gone, no
    /// access: the forge's typed not-found), it stops being followed.
    pub numbered: Vec<(u64, Option<PrSnapshot>)>,
}

/// What people and CI said on a PR (pr-news): all of it, the hub keeps
/// what it already passed on ([`news`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// Reviews, review-thread comments (unresolved threads only) and the
    /// PR's own comments, oldest first.
    pub notes: Vec<Note>,
    /// The failing checks of the head commit (with `logs`).
    pub failed: Vec<FailedCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteKind {
    /// A review: `CHANGES_REQUESTED`, `COMMENTED`, `APPROVED`, `DISMISSED`.
    Review(String),
    /// A comment in a review thread, on a file's line.
    Thread { path: String, line: Option<u64> },
    /// A comment on the PR itself.
    Comment,
}

/// One review or comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// The forge's id (unique per kind; `r12`, `t34`, `c56`).
    pub id: String,
    pub author: String,
    /// The forge's `authorAssociation` (`OWNER`, `MEMBER`,
    /// `COLLABORATOR`, `CONTRIBUTOR`, `NONE`…) and whether the author is
    /// a bot: [`news::trusted`] decides whose words are passed on.
    pub association: String,
    pub bot: bool,
    pub kind: NoteKind,
    pub body: String,
    /// The commit it is about (a review's, a thread comment's).
    pub commit: Option<String>,
    /// ISO 8601 (`2026-10-01T12:00:00Z`).
    pub at: String,
}

/// A check that failed on the PR's head, with the end of its log
/// (`gh run view --log-failed`, the last [`LOG_TAIL`] lines; empty when
/// the CI is not GitHub Actions or the log is gone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedCheck {
    pub name: String,
    pub url: String,
    pub tail: String,
}

/// How many lines of a failing check's log reach the agent (pr-design §6.1).
pub const LOG_TAIL: usize = 60;

/// The hub's PR events (pr-design §10). `Seen`, `Merged` and `Closed`
/// are journaled (the number survives a restart, so a restart never
/// sees a PR for the first time again); `Changed` and `Unreachable` go
/// to hub.log, and to pr-news (wave 3) for the agents and main.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrEvent {
    Seen { place: String, pr: PrSnapshot },
    Changed { place: String, from: PrSnapshot, to: PrSnapshot },
    Merged { place: String, pr: PrSnapshot },
    Closed { place: String, pr: PrSnapshot },
    Unreachable { error: ForgeError },
}

/// What the journal says about a place's PR: its number, and whether its
/// end (merged, closed) was journaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub number: u64,
    pub ended: bool,
}

/// Merged or closed: the PR's end.
pub(crate) fn ended(s: PrState) -> bool {
    matches!(s, PrState::Merged | PrState::Closed)
}

fn end_event(place: &str, pr: &PrSnapshot) -> Option<PrEvent> {
    match pr.state {
        PrState::Merged => Some(PrEvent::Merged { place: place.into(), pr: pr.clone() }),
        PrState::Closed => Some(PrEvent::Closed { place: place.into(), pr: pr.clone() }),
        _ => None,
    }
}

/// Whether a PR the forge gives for a place's branch is the place's: a
/// merged or closed PR this place never knew, whose head is not the
/// branch's tip, is an older PR of a branch name used again (a new task
/// `x` on `sb/x` after an old one's PR was merged): not this place's.
pub fn belongs(pr: &PrSnapshot, known: Option<Known>, tip: Option<&str>) -> bool {
    !ended(pr.state) || known.is_some_and(|k| k.number == pr.number) || tip == Some(pr.head_oid.as_str())
}

/// The events between the last answer (`old`, this run of the hub) and
/// this one (`new`), given what the journal knows. A PR that disappears
/// (its branch renamed on the forge) makes no event.
pub fn diff(place: &str, old: Option<&PrSnapshot>, new: Option<&PrSnapshot>, known: Option<Known>) -> Vec<PrEvent> {
    let Some(new) = new else { return Vec::new() };
    let mut out = Vec::new();
    match old.filter(|o| o.number == new.number) {
        Some(old) => {
            if old != new {
                out.push(PrEvent::Changed { place: place.into(), from: old.clone(), to: new.clone() });
            }
            if ended(old.state) && !ended(new.state) {
                // reopened: seen again (the journal forgets its end)
                out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
            } else if !ended(old.state) {
                out.extend(end_event(place, new));
            }
        }
        None => match known.filter(|k| k.number == new.number) {
            // the hub restarted: the journal saw it; only an end missed
            Some(k) => {
                if !k.ended {
                    out.extend(end_event(place, new));
                } else if !ended(new.state) {
                    out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
                }
            }
            None => {
                out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
                out.extend(end_event(place, new));
            }
        },
    }
    out
}

/// The journal line of an event, if it is journaled (hub-owned: the
/// hub reads it back at its start, sb-core never sees it).
pub fn journal_line(e: &PrEvent) -> Option<serde_json::Value> {
    let (t, place, pr) = match e {
        PrEvent::Seen { place, pr } => ("pr_seen", place, pr),
        PrEvent::Merged { place, pr } => ("pr_merged", place, pr),
        PrEvent::Closed { place, pr } => ("pr_closed", place, pr),
        _ => return None,
    };
    Some(serde_json::json!({"type": t, "place": place, "branch": pr.branch, "number": pr.number}))
}

/// The hub.log line of an event.
pub fn log_line(e: &PrEvent) -> String {
    let at = |place: &str, pr: &PrSnapshot| format!("{} #{} ({})", place, pr.number, pr.branch);
    match e {
        PrEvent::Seen { place, pr } => format!("PR seen: {} {}", at(place, pr), pr.url),
        PrEvent::Changed { place, from, to } => format!(
            "PR changed: {}: {:?}/{:?}/{:?} -> {:?}/{:?}/{:?}",
            at(place, to),
            from.state,
            from.review,
            from.checks,
            to.state,
            to.review,
            to.checks
        ),
        PrEvent::Merged { place, pr } => format!("PR merged: {}", at(place, pr)),
        PrEvent::Closed { place, pr } => format!("PR closed: {}", at(place, pr)),
        PrEvent::Unreachable { error } => format!("PRs: the forge did not answer, backing off: {}", error.describe()),
    }
}

/// Whether a journal event is one of the hub's PR lines: [`journal_line`]'s
/// and pr-news' read marks (`pr_read`, [`news::News::read_journal`]).
pub fn is_pr_line(ev: &serde_json::Value) -> bool {
    matches!(ev["type"].as_str(), Some("pr_seen" | "pr_merged" | "pr_closed" | "pr_read"))
}

/// Read one of [`journal_line`]'s lines into what the journal knows.
pub fn read_line(known: &mut std::collections::BTreeMap<String, Known>, ev: &serde_json::Value) {
    let (Some(place), Some(number)) = (ev["place"].as_str(), ev["number"].as_u64()) else { return };
    match ev["type"].as_str() {
        Some("pr_seen") => {
            known.insert(place.into(), Known { number, ended: false });
        }
        Some("pr_merged" | "pr_closed") => {
            known.insert(place.into(), Known { number, ended: true });
        }
        _ => {}
    }
}

/// pr-integration rule 6: what a PR waits on him for, the one owner of
/// it (the row's pink and the inbox item read this): his review asked
/// (open or draft), else his merge, when the ready-to-merge item would
/// open (`core::merge::ready`: approved or no review needed, checks
/// pass, the forge would merge it). `place`: a place's PR, the only ones
/// the merge item opens for; a followed PR (an agent only reviews it)
/// never waits on his merge.
pub fn yours(pr: &PrSnapshot, place: bool) -> Option<bise_proto::rows::PrYours> {
    use bise_proto::rows::PrYours;
    let live = matches!(pr.state, PrState::Open | PrState::Draft);
    if live && pr.facts.asked_you {
        Some(PrYours::Review)
    } else if place && crate::core::merge::ready(pr) {
        Some(PrYours::Merge)
    } else {
        None
    }
}

/// A PR's link read once, typed (architect m_54220): the one form the
/// hub keys a PR by (its artifact's source, the poller's followed PRs),
/// its forge, its repo and number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrLink {
    /// `https://<host>/<owner>/<repo>/pull/<n>` (GitLab:
    /// `…/<group>/<repo>/-/merge_requests/<n>`), lowercase: GitHub's
    /// owner and repo are case-insensitive, and GitLab's paths resolve
    /// whatever their case, so one PR is one key.
    pub url: String,
    pub provider: bise_proto::rows::PrProvider,
    pub host: String,
    /// `owner/name` (GitLab: `group/sub/name`), lowercase
    pub repo: String,
    pub number: u64,
}

impl PrLink {
    /// `#412`, GitLab's `!412`.
    pub fn tag(&self) -> String {
        tag(self.provider, self.number)
    }
}

/// A PR's link: its tab, query and trailing parts dropped
/// (`…/pull/6/files#x` is `…/pull/6`), lowercase. None: not a PR.
pub fn pr_link(url: &str) -> Option<PrLink> {
    use bise_proto::rows::PrProvider;
    let bare = url.trim().trim_start_matches("https://").trim_start_matches("http://");
    let bare = bare.split(['?', '#']).next()?.to_lowercase();
    let parts: Vec<&str> = bare.split('/').filter(|p| !p.is_empty()).collect();
    let host = parts.first()?.to_string();
    let num = |s: Option<&&str>| s.and_then(|s| s.parse::<u64>().ok()).filter(|n| *n > 0);
    if parts.len() >= 5 && parts[3] == "pull" {
        let number = num(parts.get(4))?;
        let repo = format!("{}/{}", parts[1], parts[2]);
        let url = format!("https://{}/{}/pull/{}", host, repo, number);
        return Some(PrLink { url, provider: PrProvider::Github, host, repo, number });
    }
    // GitLab: group/(sub/)*repo/-/merge_requests/n
    let dash = parts.iter().position(|p| *p == "-")?;
    if dash >= 3 && parts.get(dash + 1) == Some(&"merge_requests") {
        let number = num(parts.get(dash + 2))?;
        let repo = parts[1..dash].join("/");
        let url = format!("https://{}/{}/-/merge_requests/{}", host, repo, number);
        return Some(PrLink { url, provider: PrProvider::Gitlab, host, repo, number });
    }
    None
}

/// The forge's time (`2026-10-01T12:00:00Z`, or with ms) as ms since
/// the epoch; None for anything else.
pub fn iso_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    match s.len() {
        20 if s.ends_with('Z') => bise_session::writer::ms_of_iso(&format!("{}.000Z", &s[..19])),
        _ => bise_session::writer::ms_of_iso(s),
    }
}

/// A PR's number with its forge's sigil: `#412`, GitLab's `!412`.
pub fn tag(provider: bise_proto::rows::PrProvider, number: u64) -> String {
    match provider {
        bise_proto::rows::PrProvider::Gitlab => format!("!{}", number),
        _ => format!("#{}", number),
    }
}

/// The held line of a worktree with no PR yet (pr-design §4.1 rule 5).
pub fn no_pr_lid(commits: u32) -> String {
    match commits {
        0 => "no PR yet".into(),
        1 => "no PR yet · 1 commit".into(),
        n => format!("no PR yet · {} commits", n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::{Checks, Review};

    pub(crate) fn pr(n: u64, state: PrState) -> PrSnapshot {
        PrSnapshot {
            number: n,
            url: format!("https://github.com/o/r/pull/{}", n),
            branch: "sb/a".into(),
            head_oid: "h1".into(),
            state,
            review: Review::None,
            checks: Checks::None,
            updated_at: "t1".into(),
            facts: Default::default(),
        }
    }

    fn kinds(es: &[PrEvent]) -> Vec<&'static str> {
        es.iter()
            .map(|e| match e {
                PrEvent::Seen { .. } => "seen",
                PrEvent::Changed { .. } => "changed",
                PrEvent::Merged { .. } => "merged",
                PrEvent::Closed { .. } => "closed",
                PrEvent::Unreachable { .. } => "unreachable",
            })
            .collect()
    }

    #[test]
    fn repo_urls() {
        let r = |u| repo_of_url(u).map(|r| format!("{} {} {}", r.host, r.owner, r.name));
        assert_eq!(r("https://github.com/gvergnaud/bise.git").as_deref(), Some("github.com gvergnaud bise"));
        assert_eq!(r("https://github.com/o/r").as_deref(), Some("github.com o r"));
        assert_eq!(r("git@github.com:o/r.git").as_deref(), Some("github.com o r"));
        assert_eq!(r("ssh://git@ghe.corp:2222/o/r.git").as_deref(), Some("ghe.corp o r"));
        assert_eq!(r("https://user@GitHub.com/o/r/").as_deref(), Some("github.com o r"));
        assert_eq!(r("/local/path/repo"), None);
        assert_eq!(r("https://gitlab.com/group/sub/r.git"), None);
    }

    #[test]
    fn the_events_of_a_pr_life() {
        let open = pr(7, PrState::Open);
        assert_eq!(kinds(&diff("p", None, Some(&open), None)), ["seen"]);
        assert_eq!(kinds(&diff("p", Some(&open), Some(&open), None)), Vec::<&str>::new());
        let mut red = open.clone();
        red.checks = Checks::Fail { failing: vec!["ci".into()] };
        assert_eq!(kinds(&diff("p", Some(&open), Some(&red), None)), ["changed"]);
        let merged = pr(7, PrState::Merged);
        assert_eq!(kinds(&diff("p", Some(&red), Some(&merged), None)), ["changed", "merged"]);
        // merged and seen again: nothing new
        assert_eq!(kinds(&diff("p", Some(&merged), Some(&merged), None)), Vec::<&str>::new());
        // no answer for the branch: nothing
        assert!(diff("p", Some(&open), None, None).is_empty());
        // a new PR on the same branch: seen
        assert_eq!(kinds(&diff("p", Some(&open), Some(&pr(8, PrState::Open)), None)), ["seen"]);
        // closed, then reopened
        let closed = pr(7, PrState::Closed);
        assert_eq!(kinds(&diff("p", Some(&open), Some(&closed), None)), ["changed", "closed"]);
        assert_eq!(kinds(&diff("p", Some(&closed), Some(&open), None)), ["changed", "seen"]);
    }

    #[test]
    fn a_restart_never_sees_a_pr_twice() {
        let open = pr(7, PrState::Open);
        let k = Some(Known { number: 7, ended: false });
        assert!(diff("p", None, Some(&open), k).is_empty());
        // merged while the hub was down
        assert_eq!(kinds(&diff("p", None, Some(&pr(7, PrState::Merged)), k)), ["merged"]);
        let k = Some(Known { number: 7, ended: true });
        assert!(diff("p", None, Some(&pr(7, PrState::Merged)), k).is_empty());
        // a first sight already merged: seen, then merged
        assert_eq!(kinds(&diff("p", None, Some(&pr(9, PrState::Merged)), None)), ["seen", "merged"]);
    }

    #[test]
    fn an_old_pr_of_a_reused_branch_name_is_not_the_place_s() {
        let old = pr(3, PrState::Merged);
        assert!(!belongs(&old, None, Some("fresh-tip")));
        assert!(belongs(&old, None, Some("h1")));
        assert!(belongs(&old, Some(Known { number: 3, ended: false }), None));
        assert!(belongs(&pr(3, PrState::Open), None, None));
    }

    /// pr-integration rule 6: his review first, else his merge exactly
    /// when the ready-to-merge item would open (one owner: the row and
    /// the card never disagree).
    #[test]
    fn what_waits_on_him() {
        use bise_proto::rows::PrYours;
        let yours = |p: &PrSnapshot| yours(p, true);
        let mut p = pr(7, PrState::Open);
        assert_eq!(yours(&p), None);
        p.facts.asked_you = true;
        assert_eq!(yours(&p), Some(PrYours::Review));
        // a followed PR (no place): his review too
        assert_eq!(super::yours(&p, false), Some(PrYours::Review));
        p.facts.asked_you = false;
        p.review = Review::Approved;
        p.checks = Checks::Pass;
        p.facts.checks = 3;
        p.facts.mergeable = true;
        p.facts.methods = vec![crate::place::MergeMethod::Squash];
        assert_eq!(yours(&p), Some(PrYours::Merge));
        // a followed PR never waits on his merge (no merge item for it)
        assert_eq!(super::yours(&p, false), None);
        for (i, f) in [
            (|p: &mut PrSnapshot| p.facts.mergeable = false) as fn(&mut PrSnapshot),
            |p| p.checks = Checks::Running,
            |p| p.review = Review::ChangesRequested,
            |p| p.state = PrState::Draft,
            |p| p.state = PrState::Merged,
        ]
        .into_iter()
        .enumerate()
        {
            let mut q = p.clone();
            f(&mut q);
            assert_eq!(yours(&q), None, "case {i}");
            assert_eq!(yours(&q).is_some(), crate::core::merge::ready(&q), "case {i}");
        }
        assert!(crate::core::merge::ready(&p));
        // a merged PR asks nothing of him, even with a request left
        let mut m = pr(7, PrState::Merged);
        m.facts.asked_you = true;
        assert_eq!(yours(&m), None);
    }

    #[test]
    fn one_form_of_a_pr_link() {
        use bise_proto::rows::PrProvider;
        let u = |s| pr_link(s).map(|l| l.url);
        let want = Some("https://github.com/o/r/pull/6".to_string());
        assert_eq!(u("https://github.com/o/r/pull/6"), want);
        assert_eq!(u("https://GitHub.com/o/r/pull/6/files#diff-1"), want);
        assert_eq!(u("github.com/o/r/pull/6?w=1"), want);
        assert_eq!(u("http://github.com/o/r/pull/6/"), want);
        // GitHub's owner and repo are case-insensitive: one key
        assert_eq!(u("https://github.com/O/R/pull/6"), want);
        assert_eq!(u("https://github.com/o/r/issues/6"), None);
        assert_eq!(u("https://github.com/o/r/pull/x"), None);
        assert_eq!(u("https://github.com/o/r/pull/0"), None);
        assert_eq!(u("https://example.com"), None);
        let gl = pr_link("https://GitLab.com/G/Sub/R/-/merge_requests/12/diffs").unwrap();
        assert_eq!(gl.url, "https://gitlab.com/g/sub/r/-/merge_requests/12");
        assert_eq!((gl.provider, gl.repo.as_str(), gl.number, gl.tag()), (PrProvider::Gitlab, "g/sub/r", 12, "!12".to_string()));
        let gh = pr_link("https://github.com/Mistralai/Dashboard/pull/6/files").unwrap();
        assert_eq!((gh.provider, gh.host.as_str(), gh.repo.as_str(), gh.tag()), (PrProvider::Github, "github.com", "mistralai/dashboard", "#6".to_string()));
        assert_eq!(tag(PrProvider::Github, 412), "#412");
        assert_eq!(tag(PrProvider::Gitlab, 412), "!412");
        assert_eq!(iso_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(iso_ms("1970-01-01T00:00:01.500Z"), Some(1500));
        assert_eq!(iso_ms(""), None);
    }

    #[test]
    fn the_journal_lines_round_trip() {
        let mut known = std::collections::BTreeMap::new();
        for e in [
            PrEvent::Seen { place: "wt:a".into(), pr: pr(7, PrState::Open) },
            PrEvent::Seen { place: "wt:b".into(), pr: pr(8, PrState::Open) },
            PrEvent::Merged { place: "wt:b".into(), pr: pr(8, PrState::Merged) },
        ] {
            let l = journal_line(&e).unwrap();
            assert!(is_pr_line(&l));
            read_line(&mut known, &l);
        }
        assert_eq!(known["wt:a"], Known { number: 7, ended: false });
        assert_eq!(known["wt:b"], Known { number: 8, ended: true });
        let changed = PrEvent::Changed { place: "p".into(), from: pr(7, PrState::Open), to: pr(7, PrState::Open) };
        assert_eq!(journal_line(&changed), None);
        assert_eq!(no_pr_lid(0), "no PR yet");
        assert_eq!(no_pr_lid(1), "no PR yet · 1 commit");
        assert_eq!(no_pr_lid(2), "no PR yet · 2 commits");
    }
}
