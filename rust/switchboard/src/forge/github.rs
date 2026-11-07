//! GitHub through `gh api graphql` (pr-design §7, §8): one query per repo
//! per tick, an alias per branch (`b0`, `b1`…), gh's own login. bise
//! never reads, stores or prints a token: gh does.

use super::{Activity, FailedCheck, Forge, ForgeError, Note, NoteKind, RepoRef};
use crate::place::{Checks, MergeMethod, PrFacts, PrSnapshot, PrState, Review};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What one alias asks: the 3 latest PRs of the branch (a fork's PR on a
/// branch of the same name is skipped by its owner), the last commit's
/// checks with the names of the failing ones; for the ready-to-merge
/// item (pr-design §6.3): the title, the size, the reviewers' latest
/// reviews, whether GitHub would merge it now.
const FIELDS: &str = "number url state isDraft reviewDecision headRefName baseRefName headRefOid updatedAt \
headRepositoryOwner { login } title additions deletions mergeStateStatus total: commits { totalCount } latestReviews(first: 10) { nodes { state author { login } } } reviewRequests(first: 10) { nodes { requestedReviewer { __typename ... on User { login } } } } \
commits(last: 1) { nodes { commit { committedDate statusCheckRollup { state \
contexts(first: 50) { totalCount nodes { __typename \
... on CheckRun { name status conclusion } \
... on StatusContext { context state } } } } } } }";

/// What the repo is asked once per query: the merge methods it allows.
const REPO_FIELDS: &str = "squashMergeAllowed mergeCommitAllowed rebaseMergeAllowed";

/// A GraphQL string literal.
fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// The query for `branches` of `repo` and the PRs it follows by number
/// (pr-integration rule 2: an alias `n<i>` each, in the same query;
/// measured: 25 aliases cost 1 point).
pub fn query(repo: &RepoRef, branches: &[String], numbers: &[u64]) -> String {
    let aliases: Vec<String> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "b{}: pullRequests(headRefName: {}, first: 3, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{ nodes {{ {} }} }}",
                i,
                lit(b),
                FIELDS
            )
        })
        .chain(numbers.iter().enumerate().map(|(i, n)| format!("n{}: pullRequest(number: {}) {{ {} }}", i, n, FIELDS)))
        .collect();
    format!(
        "query {{ viewer {{ login }} repository(owner: {}, name: {}) {{ {} {} }} }}",
        lit(&repo.owner),
        lit(&repo.name),
        REPO_FIELDS,
        aliases.join(" ")
    )
}

/// The repo's allowed merge methods, squash first (pr-design §6.3).
fn methods_of(r: &Value) -> Vec<MergeMethod> {
    [
        ("squashMergeAllowed", MergeMethod::Squash),
        ("mergeCommitAllowed", MergeMethod::Merge),
        ("rebaseMergeAllowed", MergeMethod::Rebase),
    ]
    .into_iter()
    .filter(|(k, _)| r[*k].as_bool() == Some(true))
    .map(|(_, m)| m)
    .collect()
}

/// The facts of PR node `n` for the ready-to-merge item. Mergeable:
/// GitHub's `mergeStateStatus` says the merge would go through now
/// (`CLEAN`; `HAS_HOOKS`: clean with pre-receive hooks; `UNSTABLE`:
/// only checks that are not required fail). Conflicts: `DIRTY` (the one
/// place `mergeStateStatus` becomes a conflict). `viewer`: gh's login,
/// whose direct review request is `asked_you` (a team's is not, v1).
fn facts_of(n: &Value, methods: &[MergeMethod], viewer: &str) -> PrFacts {
    let num = |v: &Value| v.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32;
    let mut approved_by: Vec<String> = n["latestReviews"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["state"].as_str() == Some("APPROVED"))
        .filter_map(|r| r["author"]["login"].as_str().map(str::to_string))
        .collect();
    approved_by.dedup();
    let commit = &n["commits"]["nodes"][0]["commit"];
    let rollup = &commit["statusCheckRollup"];
    let asked_you = !viewer.is_empty()
        && n["reviewRequests"]["nodes"].as_array().into_iter().flatten().any(|r| {
            r["requestedReviewer"]["__typename"].as_str() == Some("User")
                && r["requestedReviewer"]["login"].as_str().is_some_and(|l| l.eq_ignore_ascii_case(viewer))
        });
    let checks_done = rollup["contexts"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| match c["__typename"].as_str() {
            Some("CheckRun") => c["status"].as_str() == Some("COMPLETED"),
            Some("StatusContext") => !matches!(c["state"].as_str(), Some("PENDING" | "EXPECTED") | None),
            _ => false,
        })
        .count() as u32;
    PrFacts {
        base: n["baseRefName"].as_str().unwrap_or_default().to_string(),
        conflicts: n["mergeStateStatus"].as_str() == Some("DIRTY"),
        asked_you,
        pushed_at: commit["committedDate"].as_str().unwrap_or_default().to_string(),
        checks_done,
        title: n["title"].as_str().unwrap_or_default().to_string(),
        approved_by,
        commits: num(&n["total"]["totalCount"]),
        additions: num(&n["additions"]),
        deletions: num(&n["deletions"]),
        checks: num(&rollup["contexts"]["totalCount"]),
        mergeable: matches!(n["mergeStateStatus"].as_str(), Some("CLEAN" | "HAS_HOOKS" | "UNSTABLE")),
        methods: methods.to_vec(),
    }
}

fn checks_of(rollup: &Value) -> Checks {
    if rollup.is_null() {
        return Checks::None;
    }
    let mut failing: Vec<String> = Vec::new();
    for c in rollup["contexts"]["nodes"].as_array().into_iter().flatten() {
        let (name, bad) = match c["__typename"].as_str() {
            Some("CheckRun") => (
                c["name"].as_str(),
                matches!(
                    c["conclusion"].as_str(),
                    Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE")
                ),
            ),
            Some("StatusContext") => (c["context"].as_str(), matches!(c["state"].as_str(), Some("FAILURE" | "ERROR"))),
            _ => (None, false),
        };
        if let (Some(n), true) = (name, bad) {
            if !failing.iter().any(|f| f == n) {
                failing.push(n.to_string());
            }
        }
    }
    match rollup["state"].as_str() {
        _ if !failing.is_empty() => Checks::Fail { failing },
        Some("FAILURE" | "ERROR") => Checks::Fail { failing },
        Some("SUCCESS") => Checks::Pass,
        Some("PENDING" | "EXPECTED") => Checks::Running,
        _ => Checks::None,
    }
}

/// gh's PR url in the hub's one key form (`pr_link`: lowercased, its
/// tab and query dropped): GitHub answers with the repo's own casing
/// (`Owner/Repo`), so without it the feed line, the place's PR and the
/// artifact would each key the same PR differently (architect m_54725).
/// One that doesn't read as a PR is kept as is, said once on stderr.
fn key_url(raw: &str) -> String {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SAID: AtomicBool = AtomicBool::new(false);
    match super::pr_link(raw) {
        Some(l) => l.url,
        None => {
            if !SAID.swap(true, Ordering::Relaxed) {
                eprintln!("forge: gh gave a PR url that is not a PR link, kept as is: {}", raw);
            }
            raw.to_string()
        }
    }
}

fn snapshot(n: &Value, branch: &str, methods: &[MergeMethod], viewer: &str) -> Option<PrSnapshot> {
    let state = match (n["state"].as_str()?, n["isDraft"].as_bool() == Some(true)) {
        ("OPEN", true) => PrState::Draft,
        ("OPEN", false) => PrState::Open,
        ("MERGED", _) => PrState::Merged,
        ("CLOSED", _) => PrState::Closed,
        _ => return None,
    };
    let review = match n["reviewDecision"].as_str() {
        Some("APPROVED") => Review::Approved,
        Some("CHANGES_REQUESTED") => Review::ChangesRequested,
        Some("REVIEW_REQUIRED") => Review::Pending,
        _ => Review::None,
    };
    let rollup = &n["commits"]["nodes"][0]["commit"]["statusCheckRollup"];
    Some(PrSnapshot {
        number: n["number"].as_u64()?,
        url: key_url(n["url"].as_str()?),
        branch: branch.to_string(),
        head_oid: n["headRefOid"].as_str().unwrap_or_default().to_string(),
        state,
        review,
        checks: checks_of(rollup),
        updated_at: n["updatedAt"].as_str().unwrap_or_default().to_string(),
        facts: Box::new(facts_of(n, methods, viewer)),
    })
}

/// An error of gh's answer that only says a followed PR (alias `n<i>`)
/// does not resolve (deleted, no access): its index. GitHub's typed
/// `NOT_FOUND` at that alias's path, never its message.
fn gone_alias(e: &Value) -> Option<usize> {
    if e["type"].as_str() != Some("NOT_FOUND") {
        return None;
    }
    let path = e["path"].as_array()?;
    let alias = path.get(1)?.as_str()?;
    (path.first()?.as_str() == Some("repository") && path.len() == 2).then_some(())?;
    alias.strip_prefix('n')?.parse().ok()
}

/// The PRs in gh's answer to [`query`]: per branch alias, the latest PR
/// from the repo's owner (not a fork's); per followed number, its PR, or
/// none when it no longer resolves.
pub fn parse(out: &str, repo: &RepoRef, branches: &[String], numbers: &[u64]) -> Result<super::Answer, ForgeError> {
    let v: Value = serde_json::from_str(out).map_err(|e| ForgeError::Other(format!("gh answered no JSON: {}", e)))?;
    let errs = v["errors"].as_array().cloned().unwrap_or_default();
    let mut gone = std::collections::BTreeSet::new();
    for e in &errs {
        match gone_alias(e) {
            Some(i) => {
                gone.insert(i);
            }
            None => {
                let msg = e["message"].as_str().unwrap_or("error").to_string();
                return Err(match e["type"].as_str() {
                    Some("RATE_LIMITED") => ForgeError::RateLimited(msg),
                    _ => ForgeError::Other(msg),
                });
            }
        }
    }
    let r = &v["data"]["repository"];
    if r.is_null() {
        return Err(ForgeError::Other(format!("no repository {}/{} on {}", repo.owner, repo.name, repo.host)));
    }
    let methods = methods_of(r);
    let viewer = v["data"]["viewer"]["login"].as_str().unwrap_or_default();
    let mut prs = Vec::new();
    for (i, b) in branches.iter().enumerate() {
        let ours = r[format!("b{}", i)]["nodes"].as_array().into_iter().flatten().find(|n| {
            n["headRepositoryOwner"]["login"].as_str().is_none_or(|o| o.eq_ignore_ascii_case(&repo.owner))
        });
        if let Some(pr) = ours.and_then(|n| snapshot(n, b, &methods, viewer)) {
            prs.push(pr);
        }
    }
    let numbered = numbers
        .iter()
        .enumerate()
        .map(|(i, num)| {
            let n = &r[format!("n{}", i)];
            let pr = if gone.contains(&i) || n.is_null() {
                None
            } else {
                snapshot(n, n["headRefName"].as_str().unwrap_or_default(), &methods, viewer)
            };
            (*num, pr)
        })
        .collect();
    Ok(super::Answer { prs, numbered })
}

// ---- pr-news: a PR's reviews, comments and failing checks ----

const WHO: &str = "authorAssociation author { login __typename }";

/// The details of PR `number` (pr-news; asked only when its `updatedAt`
/// moved): the last reviews, the review threads with their comments, the
/// PR's comments, the head commit's checks with their Actions run.
pub fn activity_query(repo: &RepoRef, number: u64) -> String {
    format!(
        "query {{ repository(owner: {}, name: {}) {{ pullRequest(number: {}) {{ \
         reviews(last: 30) {{ nodes {{ databaseId state body submittedAt {who} commit {{ oid }} }} }} \
         reviewThreads(last: 50) {{ nodes {{ isResolved comments(last: 20) {{ nodes {{ databaseId body createdAt path line originalLine {who} commit {{ oid }} }} }} }} }} \
         comments(last: 30) {{ nodes {{ databaseId body createdAt {who} }} }} \
         commits(last: 1) {{ nodes {{ commit {{ statusCheckRollup {{ contexts(first: 50) {{ nodes {{ __typename \
         ... on CheckRun {{ name conclusion detailsUrl checkSuite {{ workflowRun {{ databaseId }} }} }} \
         ... on StatusContext {{ context state targetUrl }} }} }} }} }} }} }} }} }} }}",
        lit(&repo.owner),
        lit(&repo.name),
        number,
        who = WHO
    )
}

fn note_of(n: &Value, id: String, kind: NoteKind, at: &str) -> Option<Note> {
    Some(Note {
        id,
        author: n["author"]["login"].as_str().unwrap_or("ghost").to_string(),
        association: n["authorAssociation"].as_str().unwrap_or("NONE").to_string(),
        bot: n["author"]["__typename"].as_str() == Some("Bot"),
        kind,
        body: n["body"].as_str().unwrap_or_default().trim().to_string(),
        commit: n["commit"]["oid"].as_str().map(str::to_string),
        at: n[at].as_str()?.to_string(),
    })
}

/// A failing check of the activity answer, and its Actions run id (for
/// the log), if any.
fn failing_of(c: &Value) -> Option<(FailedCheck, Option<u64>)> {
    match c["__typename"].as_str()? {
        "CheckRun" => {
            let bad = matches!(
                c["conclusion"].as_str(),
                Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE")
            );
            bad.then(|| {
                (
                    FailedCheck {
                        name: c["name"].as_str().unwrap_or("check").to_string(),
                        url: c["detailsUrl"].as_str().unwrap_or_default().to_string(),
                        tail: String::new(),
                    },
                    c["checkSuite"]["workflowRun"]["databaseId"].as_u64(),
                )
            })
        }
        "StatusContext" => matches!(c["state"].as_str(), Some("FAILURE" | "ERROR")).then(|| {
            (
                FailedCheck {
                    name: c["context"].as_str().unwrap_or("status").to_string(),
                    url: c["targetUrl"].as_str().unwrap_or_default().to_string(),
                    tail: String::new(),
                },
                None,
            )
        }),
        _ => None,
    }
}

/// gh's answer to [`activity_query`]: the notes oldest first (a COMMENTED
/// review with no words is only the envelope of its thread comments:
/// skipped; resolved threads skipped), and the failing checks with their
/// Actions run ids.
pub fn parse_activity(out: &str) -> Result<(Activity, Vec<Option<u64>>), ForgeError> {
    let v: Value = serde_json::from_str(out).map_err(|e| ForgeError::Other(format!("gh answered no JSON: {}", e)))?;
    if let Some(errs) = v["errors"].as_array().filter(|e| !e.is_empty()) {
        let e = &errs[0];
        let msg = e["message"].as_str().unwrap_or("error").to_string();
        return Err(match e["type"].as_str() {
            Some("RATE_LIMITED") => ForgeError::RateLimited(msg),
            _ => ForgeError::Other(msg),
        });
    }
    let pr = &v["data"]["repository"]["pullRequest"];
    if pr.is_null() {
        return Err(ForgeError::Other("no such pull request".into()));
    }
    let mut notes = Vec::new();
    for r in pr["reviews"]["nodes"].as_array().into_iter().flatten() {
        let state = r["state"].as_str().unwrap_or("COMMENTED").to_string();
        if state == "PENDING" || (state == "COMMENTED" && r["body"].as_str().is_none_or(|b| b.trim().is_empty())) {
            continue;
        }
        let id = format!("r{}", r["databaseId"].as_u64().unwrap_or(0));
        notes.extend(note_of(r, id, NoteKind::Review(state), "submittedAt"));
    }
    for t in pr["reviewThreads"]["nodes"].as_array().into_iter().flatten() {
        if t["isResolved"].as_bool() == Some(true) {
            continue;
        }
        for c in t["comments"]["nodes"].as_array().into_iter().flatten() {
            let kind = NoteKind::Thread {
                path: c["path"].as_str().unwrap_or_default().to_string(),
                line: c["line"].as_u64().or(c["originalLine"].as_u64()),
            };
            let id = format!("t{}", c["databaseId"].as_u64().unwrap_or(0));
            notes.extend(note_of(c, id, kind, "createdAt"));
        }
    }
    for c in pr["comments"]["nodes"].as_array().into_iter().flatten() {
        let id = format!("c{}", c["databaseId"].as_u64().unwrap_or(0));
        notes.extend(note_of(c, id, NoteKind::Comment, "createdAt"));
    }
    notes.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    let rollup = &pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"];
    let (failed, runs): (Vec<FailedCheck>, Vec<Option<u64>>) =
        rollup["contexts"]["nodes"].as_array().into_iter().flatten().filter_map(failing_of).unzip();
    Ok((Activity { notes, failed }, runs))
}

/// The last `n` lines of a log, each cut at 300 characters (the agent
/// reads the end of the failure, not megabytes).
pub fn tail(log: &str, n: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let from = lines.len().saturating_sub(n);
    lines[from..]
        .iter()
        .map(|l| {
            let l = l.trim_end();
            if l.chars().count() > 300 {
                format!("{}…", l.chars().take(300).collect::<String>())
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A PR's description past this many characters is cut (`body_cut`).
pub const DETAIL_BODY_CAP: usize = 8_000;
/// A note past this many characters is cut.
pub const NOTE_CAP: usize = 4_000;
/// The notes a PR's tab gets, newest first; the rest are `more_notes`.
pub const NOTES_CAP: usize = 30;
/// The lines of a check's log his "ask to fix" gets.
pub const CHECK_LOG_LINES: usize = 80;

/// `gh pr view`'s fields for a PR's tab.
pub const DETAIL_FIELDS: &str = "body,statusCheckRollup,reviews,comments";

fn cut(s: &str, n: usize) -> (String, bool) {
    match s.char_indices().nth(n) {
        Some((i, _)) => (format!("{}…", &s[..i]), true),
        None => (s.to_string(), false),
    }
}

/// One check of `statusCheckRollup` (a check run or a commit status),
/// typed: anything new is `Unknown`, never a pass.
fn check_of(c: &Value) -> bise_proto::rows::PrCheck {
    use bise_proto::rows::{PrCheck, PrCheckState as S};
    let s = |k: &str| c[k].as_str().unwrap_or_default().to_string();
    let run = c["__typename"] == "CheckRun" || c.get("conclusion").is_some();
    let state = if run {
        match (s("status").as_str(), s("conclusion").as_str()) {
            ("COMPLETED", "SUCCESS") => S::Pass,
            ("COMPLETED", "FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED") => S::Fail,
            ("COMPLETED", "SKIPPED" | "NEUTRAL" | "CANCELLED" | "STALE") => S::Skipped,
            ("QUEUED" | "IN_PROGRESS" | "WAITING" | "PENDING" | "REQUESTED", _) => S::Running,
            _ => S::Unknown,
        }
    } else {
        match s("state").as_str() {
            "SUCCESS" => S::Pass,
            "FAILURE" | "ERROR" => S::Fail,
            "PENDING" | "EXPECTED" => S::Running,
            _ => S::Unknown,
        }
    };
    let name = if run { s("name") } else { s("context") };
    let url = if run { s("detailsUrl") } else { s("targetUrl") };
    PrCheck { name, state, url }
}

/// `gh pr view --json body,statusCheckRollup,reviews,comments` as a PR's
/// tab reads it: its description cut at the cap, its checks typed, its
/// reviews and comments newest first, at most [`NOTES_CAP`] (a review
/// with no words and no verdict is skipped).
pub fn parse_detail(out: &str) -> Result<bise_proto::rows::PrDetail, ForgeError> {
    use bise_proto::rows::{PrDetail, PrNote, PrVerdict};
    let v: Value = serde_json::from_str(out).map_err(|e| ForgeError::Other(format!("gh pr view: {}", e)))?;
    let (body, body_cut) = cut(v["body"].as_str().unwrap_or_default(), DETAIL_BODY_CAP);
    let checks = v["statusCheckRollup"].as_array().map(|a| a.iter().map(check_of).collect()).unwrap_or_default();
    let note = |n: &Value, at: &str, verdict: Option<PrVerdict>| PrNote {
        author: n["author"]["login"].as_str().unwrap_or("someone").to_string(),
        body: cut(n["body"].as_str().unwrap_or_default(), NOTE_CAP).0,
        at_ms: n[at].as_str().and_then(super::iso_ms).unwrap_or(0),
        verdict,
    };
    let mut notes: Vec<PrNote> = Vec::new();
    for r in v["reviews"].as_array().into_iter().flatten() {
        let verdict = match r["state"].as_str().unwrap_or_default() {
            "APPROVED" => PrVerdict::Approved,
            "CHANGES_REQUESTED" => PrVerdict::Changes,
            "COMMENTED" => PrVerdict::Commented,
            // a pending review is the reviewer's draft, a dismissed one gone
            "PENDING" | "DISMISSED" => continue,
            _ => PrVerdict::Unknown,
        };
        if verdict == PrVerdict::Commented && r["body"].as_str().unwrap_or_default().trim().is_empty() {
            continue;
        }
        notes.push(note(r, "submittedAt", Some(verdict)));
    }
    for c in v["comments"].as_array().into_iter().flatten() {
        notes.push(note(c, "createdAt", None));
    }
    notes.sort_by_key(|n| std::cmp::Reverse(n.at_ms));
    let more_notes = u32::try_from(notes.len().saturating_sub(NOTES_CAP)).unwrap_or(u32::MAX);
    notes.truncate(NOTES_CAP);
    Ok(PrDetail { body, body_cut, checks, notes, more_notes })
}

/// A GitHub Actions check's run: `-R`'s repo (`host/owner/name`) and the
/// run id from its details link (`…/owner/name/actions/runs/<id>/job/<j>`).
/// None: not an Actions run (another CI): no log the hub can read.
pub fn run_of(details_url: &str) -> Option<(String, u64)> {
    let bare = details_url.trim().strip_prefix("https://")?;
    let parts: Vec<&str> = bare.split(['?', '#']).next()?.split('/').collect();
    let at = parts.iter().position(|p| *p == "actions")?;
    if at != 3 || parts.get(4) != Some(&"runs") {
        return None;
    }
    let run = parts.get(5)?.parse().ok()?;
    Some((format!("{}/{}/{}", parts[0], parts[1], parts[2]), run))
}

/// The last lines of check `name` in its run's `--log-failed` output, at
/// most [`CHECK_LOG_LINES`], each line's `job\tstep\t` prefix dropped.
pub fn check_log_lines(log: &str, name: &str) -> Vec<String> {
    let job = name.rsplit(" / ").next().unwrap_or(name);
    let mine: Vec<&str> = log.lines().filter(|l| l.split('\t').next() == Some(job)).collect();
    let all: Vec<&str> = if mine.is_empty() { log.lines().collect() } else { mine };
    let from = all.len().saturating_sub(CHECK_LOG_LINES);
    all[from..].iter().map(|l| cut(l.splitn(3, '\t').last().unwrap_or(l).trim_end(), 300).0).collect()
}

/// The tail of check `name`'s lines in a run's `--log-failed` output: a
/// run's log holds all its failed jobs, each line `job\tstep\tline`; the
/// check `ci / test` is the job `test`. None of them: the whole log's tail.
pub fn check_tail(log: &str, name: &str) -> String {
    let job = name.rsplit(" / ").next().unwrap_or(name);
    let mine: Vec<&str> = log.lines().filter(|l| l.split('\t').next() == Some(job)).collect();
    if mine.is_empty() {
        tail(log, super::LOG_TAIL)
    } else {
        tail(&mine.join("\n"), super::LOG_TAIL)
    }
}

/// What gh's failure says (its stderr, never printed whole: a short
/// reason for hub.log).
pub fn error_of(stderr: &str) -> ForgeError {
    let s = stderr.to_lowercase();
    let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("gh failed").trim();
    let short: String = first.chars().take(120).collect();
    if s.contains("http 401") || s.contains("bad credentials") || s.contains("gh auth login") || s.contains("not logged") {
        ForgeError::Auth(short)
    } else if s.contains("rate limit") {
        ForgeError::RateLimited(short)
    } else if s.contains("could not resolve")
        || s.contains("no such host")
        || s.contains("connection refused")
        || s.contains("network is unreachable")
        || s.contains("timeout")
        || s.contains("error connecting")
        || s.contains("http 502")
        || s.contains("http 503")
    {
        ForgeError::Offline(short)
    } else {
        ForgeError::Other(short)
    }
}

/// GitHub through the `gh` at `gh` (the hub's PATH lookup; a fake one in
/// the tests).
pub struct GitHub {
    pub gh: PathBuf,
}

impl GitHub {
    /// The `gh` on `path` (a PATH-like list), if any.
    pub fn find(path: &str) -> Option<GitHub> {
        crate::tools_env::which("gh", path).map(|gh| GitHub { gh })
    }

    fn gh(&self, args: &[&str]) -> Result<String, ForgeError> {
        self.gh_within(args, None)
    }

    /// `gh` with `args`; with `wait`, a gh still running then is stopped
    /// (killed and reaped: a hung gh never piles up) and the answer is
    /// [`ForgeError::Late`].
    fn gh_within(&self, args: &[&str], wait: Option<std::time::Duration>) -> Result<String, ForgeError> {
        let mut child = Command::new(&self.gh)
            .args(args)
            // gh asks nothing, pages nothing, prints no color
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("NO_COLOR", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ForgeError::Missing,
                _ => ForgeError::Other(format!("gh: {}", e)),
            })?;
        // read both pipes while it runs (a full pipe would stall gh)
        let drain = |p: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut p) = p {
                    let _ = p.read_to_end(&mut buf);
                }
                buf
            })
        };
        let out_t = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
        let err_t = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
        let until = wait.map(|w| std::time::Instant::now() + w);
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if until.is_some_and(|u| std::time::Instant::now() >= u) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ForgeError::Late);
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(e) => return Err(ForgeError::Other(format!("gh: {}", e))),
            }
        };
        let out = std::process::Output { status, stdout: out_t.join().unwrap_or_default(), stderr: err_t.join().unwrap_or_default() };
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            // a GraphQL error comes with a JSON body on stdout
            let body = String::from_utf8_lossy(&out.stdout);
            if body.trim_start().starts_with('{') && body.contains("\"errors\"") {
                return Ok(body.into_owned());
            }
            Err(error_of(&String::from_utf8_lossy(&out.stderr)))
        }
    }

    /// A PR's tab: its description, checks, reviews and comments
    /// ([`parse_detail`]), with his login.
    /// gh stopped after `wait` ([`ForgeError::Late`]).
    pub fn pr_detail(&self, url: &str, wait: std::time::Duration) -> Result<bise_proto::rows::PrDetail, ForgeError> {
        parse_detail(&self.gh_within(&["pr", "view", url, "--json", DETAIL_FIELDS], Some(wait))?)
    }

    /// Check `name`'s last log lines, from its Actions run (`details`
    /// its link, [`run_of`]); None: not a run the hub can read. gh
    /// stopped after `wait`.
    pub fn check_log(&self, details: &str, name: &str, wait: std::time::Duration) -> Option<Result<Vec<String>, ForgeError>> {
        let (repo, run) = run_of(details)?;
        let run = run.to_string();
        Some(self.gh_within(&["run", "view", run.as_str(), "--log-failed", "-R", repo.as_str()], Some(wait)).map(|log| check_log_lines(&log, name)))
    }

    /// Whether gh has a login for `host` (`gh auth status`'s exit code;
    /// its output, which names the token's scopes, is never read).
    pub fn logged_in(&self, host: &str) -> Result<bool, ForgeError> {
        let st = Command::new(&self.gh)
            .args(["auth", "status", "--hostname", host])
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ForgeError::Missing,
                _ => ForgeError::Other(format!("gh: {}", e)),
            })?;
        Ok(st.success())
    }
}

impl Forge for GitHub {
    fn fetch(&self, repo: &RepoRef, branches: &[String], numbers: &[u64]) -> Result<super::Answer, ForgeError> {
        if branches.is_empty() && numbers.is_empty() {
            return Ok(super::Answer::default());
        }
        let q = format!("query={}", query(repo, branches, numbers));
        let mut args = vec!["api", "graphql", "-f", q.as_str()];
        if repo.host != "github.com" {
            args.extend(["--hostname", repo.host.as_str()]);
        }
        let out = self.gh(&args)?;
        parse(&out, repo, branches, numbers)
    }

    fn activity(&self, repo: &RepoRef, pr: &PrSnapshot, logs: bool) -> Result<Activity, ForgeError> {
        let q = format!("query={}", activity_query(repo, pr.number));
        let mut args = vec!["api", "graphql", "-f", q.as_str()];
        if repo.host != "github.com" {
            args.extend(["--hostname", repo.host.as_str()]);
        }
        let out = self.gh(&args)?;
        let (mut act, runs) = parse_activity(&out)?;
        if logs {
            // one `gh run view` per run (a run's jobs share its log); a
            // log that can't be read leaves the check without a tail
            let r = format!("{}/{}/{}", repo.host, repo.owner, repo.name);
            let mut read: Vec<(u64, String)> = Vec::new();
            for (c, run) in act.failed.iter_mut().zip(runs) {
                let Some(run) = run else { continue };
                if !read.iter().any(|(id, _)| *id == run) {
                    let id = run.to_string();
                    let log = self.gh(&["run", "view", id.as_str(), "--log-failed", "-R", r.as_str()]).unwrap_or_default();
                    read.push((run, log));
                }
                let log: &str = read.iter().find(|(id, _)| *id == run).map(|(_, l)| l.as_str()).unwrap_or_default();
                c.tail = check_tail(log, &c.name);
            }
        }
        Ok(act)
    }

    fn merge(&self, repo: &RepoRef, number: u64, head: &str, method: MergeMethod) -> Result<(), ForgeError> {
        let args = merge_args(repo, number, head, method);
        self.gh(&args.iter().map(String::as_str).collect::<Vec<_>>()).map(|_| ())
    }

    fn close(&self, repo: &RepoRef, number: u64) -> Result<(), ForgeError> {
        let n = number.to_string();
        let r = repo_arg(repo);
        self.gh(&["pr", "close", n.as_str(), "--repo", r.as_str()]).map(|_| ())
    }
}

/// gh's `--repo`: `owner/name`, with its host when not github.com.
fn repo_arg(repo: &RepoRef) -> String {
    if repo.host == "github.com" {
        format!("{}/{}", repo.owner, repo.name)
    } else {
        format!("{}/{}/{}", repo.host, repo.owner, repo.name)
    }
}

/// `gh pr merge`'s arguments (pr-design §6.3): the repo named (never
/// guessed from the folder's remotes), the head pinned (`gh` refuses
/// when a commit came since), the branch left to GitHub (the hub removes
/// the worktree and its local branch after the merge, §6.4).
pub fn merge_args(repo: &RepoRef, number: u64, head: &str, method: MergeMethod) -> Vec<String> {
    let mut a: Vec<String> = ["pr", "merge"].iter().map(|s| s.to_string()).collect();
    a.push(number.to_string());
    a.push(method.flag().into());
    a.push("--repo".into());
    a.push(repo_arg(repo));
    if !head.is_empty() {
        a.push("--match-head-commit".into());
        a.push(head.into());
    }
    a
}

/// The forge of a workspace: its `origin`'s repo, when it is on GitHub
/// (`github.com`, or a host gh has a login for: GitHub Enterprise).
/// None: not a GitHub repo (no forge; GitLab later).
pub fn detect(remote_url: &str, gh: Option<&GitHub>) -> Option<RepoRef> {
    let repo = super::repo_of_url(remote_url)?;
    if repo.host == "github.com" || gh.is_some_and(|g| g.logged_in(&repo.host) == Ok(true)) {
        Some(repo)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoRef {
        RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() }
    }

    #[test]
    fn the_query_has_an_alias_per_branch() {
        let q = query(&repo(), &["sb/a".into(), "sb/\"b".into()], &[]);
        assert!(q.starts_with("query { viewer { login } repository(owner: \"o\", name: \"r\") {"));
        assert!(q.contains("b0: pullRequests(headRefName: \"sb/a\", first: 3"));
        assert!(q.contains("b1: pullRequests(headRefName: \"sb/\\\"b\""));
        assert!(!q.contains("b2:") && !q.contains("n0:"));
        // pr-integration rule 2: followed numbers in the same query
        let q = query(&repo(), &["sb/a".into()], &[75286, 12]);
        assert!(q.contains("b0: pullRequests(") && q.contains("n0: pullRequest(number: 75286) {") && q.contains("n1: pullRequest(number: 12) {"));
    }

    /// A followed PR answers its snapshot (its head branch read from the
    /// node); one that no longer resolves is the forge's typed NOT_FOUND
    /// at its alias, never its message; any other error fails the round.
    #[test]
    fn followed_numbers_read_or_are_gone() {
        let node = serde_json::json!({"number": 75286, "url": "https://github.com/o/r/pull/75286", "state": "OPEN", "isDraft": false,
            "reviewDecision": "APPROVED", "headRefName": "feat/x", "headRefOid": "h", "updatedAt": "t", "title": "Faster boot",
            "commits": {"nodes": []}});
        let out = serde_json::json!({
            "data": {"repository": {"n0": node, "n1": null}},
            "errors": [{"type": "NOT_FOUND", "path": ["repository", "n1"], "message": "Could not resolve to a PullRequest with the number of 12."}]
        });
        let a = parse(&out.to_string(), &repo(), &[], &[75286, 12]).unwrap();
        assert!(a.prs.is_empty());
        assert_eq!(a.numbered.len(), 2);
        let (n, pr) = &a.numbered[0];
        let pr = pr.as_ref().unwrap();
        assert_eq!((*n, pr.number, pr.branch.as_str(), pr.facts.title.as_str()), (75286, 75286, "feat/x", "Faster boot"));
        assert_eq!(a.numbered[1], (12, None));
        // NOT_FOUND elsewhere (the repository) is no followed PR: an error
        let mut o = out.clone();
        o["errors"][0]["path"] = serde_json::json!(["repository"]);
        assert!(parse(&o.to_string(), &repo(), &[], &[75286, 12]).is_err());
    }

    #[test]
    fn a_mixed_case_url_is_the_key_form_on_the_row_the_feed_and_the_artifact() {
        let out = serde_json::json!({"data": {"repository": {
            "b0": {"nodes": [{"number": 412, "url": "https://github.com/Acme/Web/pull/412", "state": "OPEN", "isDraft": false,
                "reviewDecision": null, "headRefOid": "h", "updatedAt": "t", "headRepositoryOwner": {"login": "o"},
                "commits": {"nodes": []}}]},
        }}});
        let prs = parse(&out.to_string(), &repo(), &["sb/perf".to_string()], &[]).unwrap().prs;
        let key = "https://github.com/acme/web/pull/412";
        assert_eq!(prs[0].url, key, "the snapshot");
        let row = crate::forge::news::pr_row(&crate::place::PrView::of(&prs[0], None), &["perf".into()]);
        assert_eq!(row.url, key, "the PR row (Place.pr, ArtifactPr.status)");
        let mut feed = crate::forge::news::News::default();
        let lines = feed.event(&crate::forge::PrEvent::Seen { place: "perf".into(), pr: prs[0].clone() }, None, false);
        assert!(lines.iter().any(|o| matches!(o, crate::forge::news::Out::Line { url, .. } if url == key)), "the feed line: {lines:?}");
        let ops = crate::forge::artifact::sync(&prs[0], Some("perf"), None);
        assert!(format!("{ops:?}").contains(key), "the artifact: {ops:?}");
        assert!(!format!("{ops:?}").contains("Acme"), "the artifact: {ops:?}");
        // one that doesn't read as a PR is kept as is
        assert_eq!(key_url("u9"), "u9");
    }

    #[test]
    fn an_answer_reads_into_snapshots() {
        let out = serde_json::json!({"data": {"repository": {
            "b0": {"nodes": [
                {"number": 9, "url": "u9", "state": "OPEN", "isDraft": false, "reviewDecision": null,
                 "headRefOid": "f", "updatedAt": "t", "headRepositoryOwner": {"login": "fork"}, "commits": {"nodes": []}},
                {"number": 7, "url": "u7", "state": "OPEN", "isDraft": true, "reviewDecision": "CHANGES_REQUESTED",
                 "headRefOid": "h", "updatedAt": "t7", "headRepositoryOwner": {"login": "O"},
                 "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE", "contexts": {"nodes": [
                    {"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE"},
                    {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "SUCCESS"},
                    {"__typename": "StatusContext", "context": "ci/legacy", "state": "ERROR"}]}}}}]}}]},
            "b1": {"nodes": []},
            "b2": {"nodes": [{"number": 3, "url": "u3", "state": "MERGED", "isDraft": false, "reviewDecision": "APPROVED",
                 "headRefOid": "m", "updatedAt": "t3", "headRepositoryOwner": {"login": "o"},
                 "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS", "contexts": {"nodes": []}}}}]}}]}
        }}});
        let bs = ["sb/a".to_string(), "sb/b".into(), "sb/c".into()];
        let prs = parse(&out.to_string(), &repo(), &bs, &[]).unwrap().prs;
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].number, 7);
        assert_eq!(prs[0].branch, "sb/a");
        assert_eq!(prs[0].state, PrState::Draft);
        assert_eq!(prs[0].review, Review::ChangesRequested);
        assert_eq!(prs[0].checks, Checks::Fail { failing: vec!["test".into(), "ci/legacy".into()] });
        assert_eq!(prs[1].branch, "sb/c");
        assert_eq!(prs[1].state, PrState::Merged);
        assert_eq!(prs[1].checks, Checks::Pass);
    }

    #[test]
    fn the_merge_facts() {
        let out = serde_json::json!({"data": {"repository": {
            "squashMergeAllowed": false, "mergeCommitAllowed": true, "rebaseMergeAllowed": true,
            "b0": {"nodes": [{"number": 409, "url": "u", "state": "OPEN", "isDraft": false, "reviewDecision": "APPROVED",
                "headRefOid": "h", "updatedAt": "t", "headRepositoryOwner": {"login": "o"},
                "title": "the cookie banner stops covering buy", "additions": 84, "deletions": 12,
                "mergeStateStatus": "CLEAN", "total": {"totalCount": 3},
                "latestReviews": {"nodes": [
                    {"state": "APPROVED", "author": {"login": "alice"}},
                    {"state": "COMMENTED", "author": {"login": "bob"}}]},
                "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS",
                    "contexts": {"totalCount": 6, "nodes": []}}}}]}}]}
        }}});
        let prs = parse(&out.to_string(), &repo(), &["sb/a".to_string()], &[]).unwrap().prs;
        let f = &prs[0].facts;
        assert_eq!(f.title, "the cookie banner stops covering buy");
        assert_eq!(f.approved_by, ["alice"]);
        assert_eq!((f.commits, f.additions, f.deletions, f.checks), (3, 84, 12, 6));
        assert!(f.mergeable);
        // no squash: merge first (pr-design §6.3)
        assert_eq!(f.methods, [MergeMethod::Merge, MergeMethod::Rebase]);
        // blocked (a required review or check missing), behind, conflicting: not now
        for s in ["BLOCKED", "BEHIND", "DIRTY", "UNKNOWN", "DRAFT"] {
            let mut o = out.clone();
            o["data"]["repository"]["b0"]["nodes"][0]["mergeStateStatus"] = s.into();
            assert!(!parse(&o.to_string(), &repo(), &["sb/a".to_string()], &[]).unwrap().prs[0].facts.mergeable, "{s}");
        }
        assert!(query(&repo(), &["sb/a".to_string()], &[]).contains("squashMergeAllowed mergeCommitAllowed rebaseMergeAllowed"));
    }

    /// pr-integration rule 3: base, conflicts, his review asked (a direct
    /// request of gh's viewer; a team's is not counted, v1), the head's
    /// commit time and how many checks finished.
    #[test]
    fn the_row_facts() {
        let out = serde_json::json!({"data": {"viewer": {"login": "Gabriel"}, "repository": {
            "b0": {"nodes": [{"number": 412, "url": "u", "state": "OPEN", "isDraft": false, "reviewDecision": "REVIEW_REQUIRED",
                "headRefOid": "h", "baseRefName": "main", "updatedAt": "t", "headRepositoryOwner": {"login": "o"},
                "mergeStateStatus": "DIRTY",
                "reviewRequests": {"nodes": [
                    {"requestedReviewer": {"__typename": "Team"}},
                    {"requestedReviewer": {"__typename": "User", "login": "gabriel"}}]},
                "commits": {"nodes": [{"commit": {"committedDate": "2026-11-06T10:00:00Z", "statusCheckRollup": {"state": "PENDING",
                    "contexts": {"totalCount": 3, "nodes": [
                        {"__typename": "CheckRun", "name": "a", "status": "COMPLETED", "conclusion": "SUCCESS"},
                        {"__typename": "CheckRun", "name": "b", "status": "IN_PROGRESS", "conclusion": null},
                        {"__typename": "StatusContext", "context": "c", "state": "SUCCESS"}]}}}}]}}]}
        }}});
        let prs = parse(&out.to_string(), &repo(), &["sb/a".to_string()], &[]).unwrap().prs;
        let f = &prs[0].facts;
        assert_eq!((f.base.as_str(), f.conflicts, f.asked_you), ("main", true, true));
        assert_eq!((f.pushed_at.as_str(), f.checks, f.checks_done), ("2026-11-06T10:00:00Z", 3, 2));
        assert!(!f.mergeable);
        // a team's request only, or no viewer: not his
        let mut o = out.clone();
        o["data"]["repository"]["b0"]["nodes"][0]["reviewRequests"]["nodes"] =
            serde_json::json!([{"requestedReviewer": {"__typename": "Team", "login": "gabriel"}}]);
        assert!(!parse(&o.to_string(), &repo(), &["sb/a".to_string()], &[]).unwrap().prs[0].facts.asked_you);
        let mut o = out.clone();
        o["data"]["viewer"] = Value::Null;
        assert!(!parse(&o.to_string(), &repo(), &["sb/a".to_string()], &[]).unwrap().prs[0].facts.asked_you);
        let q = query(&repo(), &["sb/a".to_string()], &[]);
        for f in ["viewer { login }", "baseRefName", "reviewRequests(first: 10)", "committedDate"] {
            assert!(q.contains(f), "{f}");
        }
    }

    #[test]
    fn checks_running_and_none() {
        assert_eq!(checks_of(&Value::Null), Checks::None);
        let r = serde_json::json!({"state": "PENDING", "contexts": {"nodes": [
            {"__typename": "CheckRun", "name": "t", "status": "IN_PROGRESS", "conclusion": null}]}});
        assert_eq!(checks_of(&r), Checks::Running);
        // one failed while others run: red already
        let r = serde_json::json!({"state": "PENDING", "contexts": {"nodes": [
            {"__typename": "CheckRun", "name": "t", "status": "COMPLETED", "conclusion": "TIMED_OUT"}]}});
        assert_eq!(checks_of(&r), Checks::Fail { failing: vec!["t".into()] });
    }

    #[test]
    fn errors() {
        let e = parse(r#"{"errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]}"#, &repo(), &[], &[]);
        assert!(matches!(e, Err(ForgeError::RateLimited(_))));
        assert!(matches!(error_of("HTTP 401: Bad credentials (https://api.github.com/graphql)"), ForgeError::Auth(_)));
        assert!(matches!(error_of("To get started with GitHub CLI, please run:  gh auth login"), ForgeError::Auth(_)));
        assert!(matches!(error_of("error connecting to api.github.com"), ForgeError::Offline(_)));
        assert!(matches!(error_of("HTTP 403: API rate limit exceeded for user"), ForgeError::RateLimited(_)));
        assert!(matches!(error_of("something else"), ForgeError::Other(_)));
        assert!(matches!(parse(r#"{"data": {"repository": null}}"#, &repo(), &[], &[]), Err(ForgeError::Other(_))));
    }

    #[test]
    fn detection() {
        assert_eq!(detect("https://github.com/o/r.git", None), Some(repo()));
        // another host: only when gh knows it
        assert_eq!(detect("https://gitlab.com/o/r.git", None), None);
        assert_eq!(detect("not a url", None), None);
    }

    /// A PR's tab: gh's JSON typed once here. A check state nobody knows
    /// is Unknown (never a pass); notes newest first, a draft or a
    /// dismissed review skipped, an empty "commented" review too; the
    /// caps say how many more.
    #[test]
    fn a_prs_detail_from_gh() {
        use bise_proto::rows::{PrCheckState as S, PrVerdict as V};
        let out = r#"{"body":"fixes the banner","statusCheckRollup":[
          {"__typename":"CheckRun","name":"ci / test","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":"https://github.com/acme/web/actions/runs/77/job/8"},
          {"__typename":"CheckRun","name":"lint","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":""},
          {"__typename":"CheckRun","name":"e2e","status":"IN_PROGRESS","conclusion":""},
          {"__typename":"CheckRun","name":"docs","status":"COMPLETED","conclusion":"SKIPPED"},
          {"__typename":"CheckRun","name":"new","status":"COMPLETED","conclusion":"SOMETHING_NEW"},
          {"__typename":"StatusContext","context":"vercel","state":"PENDING","targetUrl":"https://vercel.com/x"}],
          "reviews":[
          {"author":{"login":"alice"},"body":"","state":"APPROVED","submittedAt":"2026-10-01T12:00:00Z"},
          {"author":{"login":"bob"},"body":"","state":"COMMENTED","submittedAt":"2026-10-01T12:05:00Z"},
          {"author":{"login":"bob"},"body":"draft","state":"PENDING","submittedAt":"2026-10-01T12:06:00Z"},
          {"author":{"login":"carol"},"body":"rename it","state":"CHANGES_REQUESTED","submittedAt":"2026-10-01T11:00:00Z"}],
          "comments":[{"author":{"login":"dan"},"body":"<img src=x onerror=alert(1)> looks good","createdAt":"2026-10-01T12:10:00Z"}]}"#;
        let d = parse_detail(out).unwrap();
        assert_eq!((d.body.as_str(), d.body_cut), ("fixes the banner", false));
        let states: Vec<(&str, S)> = d.checks.iter().map(|c| (c.name.as_str(), c.state)).collect();
        assert_eq!(
            states,
            [("ci / test", S::Fail), ("lint", S::Pass), ("e2e", S::Running), ("docs", S::Skipped), ("new", S::Unknown), ("vercel", S::Running)]
        );
        assert_eq!(d.checks[5].url, "https://vercel.com/x");
        let notes: Vec<(&str, Option<V>)> = d.notes.iter().map(|n| (n.author.as_str(), n.verdict)).collect();
        assert_eq!(notes, [("dan", None), ("alice", Some(V::Approved)), ("carol", Some(V::Changes))]);
        // the body is the forge's text, as is: the client skips its HTML
        assert!(d.notes[0].body.starts_with("<img"));
        assert_eq!(d.more_notes, 0);
        // caps: a long body is cut, the oldest notes become a count
        let many: Vec<String> = (0..NOTES_CAP + 3)
            .map(|i| format!(r#"{{"author":{{"login":"u{i}"}},"body":"n","createdAt":"2026-10-01T12:{:02}:00Z"}}"#, i % 60))
            .collect();
        let big = format!(r#"{{"body":"{}","comments":[{}]}}"#, "x".repeat(DETAIL_BODY_CAP + 5), many.join(","));
        let d = parse_detail(&big).unwrap();
        assert!(d.body_cut && d.body.chars().count() == DETAIL_BODY_CAP + 1);
        assert_eq!((d.notes.len(), d.more_notes), (NOTES_CAP, 3));
        assert!(d.notes.windows(2).all(|w| w[0].at_ms >= w[1].at_ms), "newest first");
        assert!(parse_detail("not json").is_err());
    }

    /// A gh that hangs is stopped at its wait (architect m_54692): the
    /// answer is Late at once, its process gone, never one per click
    /// piling up; a quick one answers as before, its output whole.
    #[cfg(unix)]
    #[test]
    fn a_hung_gh_is_stopped_at_its_wait() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("sb-gh-wait-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let fake = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            GitHub { gh: p }
        };
        let hung = fake("hung", "echo $$ > \"$(dirname \"$0\")/pid\"; exec sleep 30");
        let t = std::time::Instant::now();
        assert!(matches!(hung.pr_detail("https://github.com/o/r/pull/1", std::time::Duration::from_millis(300)), Err(ForgeError::Late)));
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "{:?}", t.elapsed());
        let pid = std::fs::read_to_string(d.join("pid")).unwrap();
        let alive = std::process::Command::new("kill").args(["-0", pid.trim()]).status().unwrap().success();
        assert!(!alive, "the hung gh was stopped");
        let quick = fake("quick", r#"echo '{"body":"hi","statusCheckRollup":[],"reviews":[],"comments":[]}'"#);
        assert_eq!(quick.pr_detail("u", std::time::Duration::from_secs(10)).unwrap().body, "hi");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A check's log: only a GitHub Actions run has one the hub reads;
    /// its job's lines, the last 80, without gh's `job\tstep\t` prefix.
    #[test]
    fn a_checks_log_is_its_runs_last_lines() {
        assert_eq!(run_of("https://github.com/acme/web/actions/runs/77/job/8"), Some(("github.com/acme/web".into(), 77)));
        assert_eq!(run_of("https://ghe.acme.io/acme/web/actions/runs/9?pr=1"), Some(("ghe.acme.io/acme/web".into(), 9)));
        assert_eq!(run_of("https://vercel.com/acme/web/actions/runs/9"), Some(("vercel.com/acme/web".into(), 9)));
        assert_eq!(run_of("https://circleci.com/gh/acme/web/12"), None);
        assert_eq!(run_of(""), None);
        let mut log = String::new();
        for i in 0..100 {
            log.push_str(&format!("test\trun tests\tline {i}\nlint\trun\tother {i}\n"));
        }
        let lines = check_log_lines(&log, "ci / test");
        assert_eq!(lines.len(), CHECK_LOG_LINES);
        assert_eq!((lines[0].as_str(), lines[79].as_str()), ("line 20", "line 99"));
        // no line of that job: the log's own tail
        assert_eq!(check_log_lines("a\nb", "ci / build"), ["a", "b"]);
    }
}
