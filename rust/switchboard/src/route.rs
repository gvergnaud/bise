//! The route guess (bise desktop S2, architect m_8474, amb-hub m_8473): which
//! project the user's words to bise are about, from the fn context (the front
//! app's file, URL, title), the words themselves and each project's recent
//! threads. Pure: no I/O, no clock, no model; the home hub's shell gathers the
//! candidates (daemon/routing.rs) and calls [`guess`] then [`target`] once per
//! message, before any turn (the fast path: amb-hub's 2 s `route_hold`).
//! [`target`] is the only place the threshold lives: sb-core gets a project or
//! nothing, never a score. Nothing here sends anything.

use bise_proto::context::FnContext;
use std::path::{Path, PathBuf};

/// The confidence a guess needs to route without asking bise's main.
pub const THRESHOLD: f32 = 0.5;
/// How far ahead of the second project the first must be (else a tie).
pub const MARGIN: f32 = 0.1;

/// A project the words may be about.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidate {
    /// its hub id (`bise_home::hub_id`): what a route names
    pub id: String,
    /// its name in the projects list
    pub name: String,
    /// its folder (canonical)
    pub path: PathBuf,
    /// its git remotes' URLs, as written in `.git/config`
    pub remotes: Vec<String>,
    /// recent lines of its threads (the user's and main's), newest first
    pub recent: Vec<String>,
}

/// The best project for the words, how sure, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Guess {
    /// the best candidate's hub id (None: no signal at all)
    pub project: Option<String>,
    /// 0..1
    pub confidence: f32,
    /// the second best candidate's confidence (a tie when close)
    pub second: f32,
    /// the signal that decided: front file, front url, named, title, recent, none
    pub why: &'static str,
}

/// The project to route to, or None (bise's main gets the words): sure
/// enough, and clearly ahead of the second.
pub fn target(g: &Guess) -> Option<String> {
    let clear = g.confidence >= THRESHOLD && g.confidence - g.second >= MARGIN;
    g.project.clone().filter(|_| clear)
}

/// Score each candidate by its strongest signal; the best one wins.
pub fn guess(text: &str, ctx: &FnContext, projects: &[Candidate]) -> Guess {
    let mut scored: Vec<(f32, &'static str, &Candidate)> = projects.iter().map(|c| {
        let (s, why) = score(text, ctx, c, projects);
        (s, why, c)
    }).collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    match scored.as_slice() {
        [(s, why, c), rest @ ..] if *s > 0.0 => Guess {
            project: Some(c.id.clone()),
            confidence: *s,
            second: rest.first().map_or(0.0, |r| r.0),
            why,
        },
        _ => Guess { project: None, confidence: 0.0, second: 0.0, why: "none" },
    }
}

fn score(text: &str, ctx: &FnContext, c: &Candidate, all: &[Candidate]) -> (f32, &'static str) {
    if let Some(f) = ctx.file.as_deref().filter(|f| !f.is_empty()) {
        if longest_prefix(Path::new(f), all).is_some_and(|best| best.id == c.id) {
            return (0.95, "front file");
        }
    }
    if let Some(u) = ctx.url.as_deref().and_then(repo_of) {
        if c.remotes.iter().filter_map(|r| repo_of(r)).any(|r| r == u) {
            return (0.9, "front url");
        }
    }
    let names = names_of(c);
    if names.iter().any(|n| names_in(text, n)) {
        return (0.85, "named");
    }
    if let Some(t) = ctx.title.as_deref() {
        if names.iter().any(|n| names_in(t, n)) {
            return (0.6, "title");
        }
    }
    let r = recent_overlap(text, &c.recent);
    if r > 0.0 {
        return (r, "recent");
    }
    (0.0, "none")
}

/// The candidate whose folder holds `file` most closely (nested projects:
/// the inner one).
fn longest_prefix<'a>(file: &Path, all: &'a [Candidate]) -> Option<&'a Candidate> {
    all.iter()
        .filter(|c| !c.path.as_os_str().is_empty() && file.starts_with(&c.path))
        .max_by_key(|c| c.path.components().count())
}

/// `host/owner/repo`, lowercase, from a web or git URL: https, http, ssh
/// (`git@host:owner/repo.git`, `ssh://git@host/owner/repo`), with or
/// without `.git`, `www.` or more path after the repo.
pub fn repo_of(url: &str) -> Option<String> {
    let u = url.trim().to_lowercase();
    let rest = if let Some(r) = u.split_once("://").map(|(_, r)| r) {
        r.to_string()
    } else if let Some((user_host, path)) = u.split_once(':').filter(|(h, _)| h.contains('@')) {
        format!("{}/{}", user_host, path)
    } else {
        u.clone()
    };
    let rest = rest.rsplit_once('@').map_or(rest.as_str(), |(_, r)| r).trim_start_matches("www.");
    let mut parts = rest.split(['/', '?', '#']).filter(|p| !p.is_empty());
    let host = parts.next()?;
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    (!repo.is_empty() && host.contains('.')).then(|| format!("{host}/{owner}/{repo}"))
}

/// The words that name a project: its name and its folder's name, 3+ chars.
fn names_of(c: &Candidate) -> Vec<String> {
    let mut v = vec![c.name.to_lowercase()];
    if let Some(b) = c.path.file_name().and_then(|b| b.to_str()) {
        v.push(b.to_lowercase());
    }
    v.retain(|n| n.chars().count() >= 3);
    v.dedup();
    v
}

/// `name` appears in `text` as a whole word (not inside another word).
fn names_in(text: &str, name: &str) -> bool {
    let t = text.to_lowercase();
    let word = |ch: Option<char>| ch.is_some_and(|c| c.is_alphanumeric() || c == '_');
    t.match_indices(name).any(|(i, _)| !word(t[..i].chars().next_back()) && !word(t[i + name.len()..].chars().next()))
}

const STOP: &[&str] = &[
    "about", "after", "again", "also", "because", "been", "before", "could", "does", "doing", "done", "from", "have",
    "just", "know", "like", "make", "more", "need", "that", "their", "them", "then", "there", "these", "they", "this",
    "what", "when", "where", "which", "while", "will", "with", "would", "your", "the", "and", "for", "why", "how",
    "can", "not", "you", "are", "was", "his", "her", "its", "our", "out", "but", "all", "any", "get", "got", "has",
];

/// The text's content words (3+ chars, no stop words), a plural `s`
/// dropped (`rises` and `rise` match), sorted and unique.
fn words(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(w))
        .map(|w| if w.chars().count() > 4 && w.ends_with('s') && !w.ends_with("ss") { &w[..w.len() - 1] } else { w })
        .map(String::from)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The share of the text's words its recent threads used, capped at 0.6.
fn recent_overlap(text: &str, recent: &[String]) -> f32 {
    let ws = words(text);
    if ws.is_empty() || recent.is_empty() {
        return 0.0;
    }
    let seen = words(&recent.join(" "));
    let hits = ws.iter().filter(|w| seen.binary_search(w).is_ok()).count();
    0.6 * hits as f32 / ws.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, path: &str, remote: &str, recent: &[&str]) -> Candidate {
        Candidate {
            id: id.into(),
            name: id.into(),
            path: path.into(),
            remotes: (!remote.is_empty()).then(|| remote.to_string()).into_iter().collect(),
            recent: recent.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn world() -> Vec<Candidate> {
        vec![
            cand("telemetry", "/w/telemetry", "git@github.com:acme/telemetry.git", &["p99 latency of the ingest workers rises after deploy"]),
            cand("checkout", "/w/checkout", "https://github.com/acme/checkout", &["the payment form on safari drops the card number"]),
            cand("rapid-site", "/w/site", "", &["the landing page hero"]),
        ]
    }

    fn ctx() -> FnContext {
        FnContext::default()
    }

    #[test]
    fn the_front_file_routes_with_the_innermost_project() {
        let mut all = world();
        all.push(cand("ingest", "/w/telemetry/ingest", "", &[]));
        let c = FnContext { file: Some("/w/telemetry/ingest/src/main.rs".into()), ..ctx() };
        let g = guess("why is it slow", &c, &all);
        assert_eq!((g.project.as_deref(), g.why), (Some("ingest"), "front file"));
        assert_eq!(target(&g).as_deref(), Some("ingest"));
        let c = FnContext { file: Some("/w/checkout/README.md".into()), ..ctx() };
        assert_eq!(target(&guess("fix this", &c, &all)).as_deref(), Some("checkout"));
    }

    #[test]
    fn the_front_url_routes_by_its_repo_in_every_form() {
        for url in ["https://github.com/acme/checkout/pull/42", "http://www.github.com/ACME/checkout.git", "github.com/acme/checkout"] {
            let g = guess("review this", &FnContext { url: Some(url.into()), ..ctx() }, &world());
            assert_eq!((g.project.as_deref(), g.why), (Some("checkout"), "front url"), "{url}");
        }
        assert_eq!(repo_of("git@github.com:acme/telemetry.git").as_deref(), Some("github.com/acme/telemetry"));
        assert_eq!(repo_of("ssh://git@github.com/acme/telemetry").as_deref(), Some("github.com/acme/telemetry"));
        assert_eq!(repo_of("https://example.com/"), None);
        assert_eq!(repo_of("not a url"), None);
    }

    #[test]
    fn a_name_counts_only_as_a_whole_word() {
        let g = guess("what's the state of telemetry?", &ctx(), &world());
        assert_eq!((target(&g).as_deref(), g.why), (Some("telemetry"), "named"));
        // 'rapid-site''s folder is 'site': 'website' does not name it, 'site' does
        assert_ne!(guess("the website copy", &ctx(), &world()).why, "named");
        assert_eq!(guess("deploy the site", &ctx(), &world()).project.as_deref(), Some("rapid-site"));
        // a window title names it, less surely
        let g = guess("look at this", &FnContext { title: Some("checkout — PR #42".into()), ..ctx() }, &world());
        assert_eq!((g.project.as_deref(), g.why, g.confidence), (Some("checkout"), "title", 0.6));
    }

    #[test]
    fn recent_threads_route_only_when_clearly_ahead() {
        let g = guess("why does p99 latency rise", &ctx(), &world());
        assert_eq!((g.project.as_deref(), g.why), (Some("telemetry"), "recent"));
        assert_eq!(target(&g).as_deref(), Some("telemetry"));
        // one shared word of three: not sure enough, bise's main decides
        let g = guess("latency of the coffee machine", &ctx(), &world());
        assert_eq!(g.why, "recent");
        assert_eq!(target(&g), None);
    }

    #[test]
    fn ties_and_silence_stay_with_bise() {
        // both named: a tie, never a coin flip
        let g = guess("compare telemetry and checkout", &ctx(), &world());
        assert_eq!(g.confidence, g.second);
        assert_eq!(target(&g), None);
        // nothing points anywhere
        let g = guess("draft my weekly update", &ctx(), &world());
        assert_eq!((g.project, g.why, target(&guess("hi", &ctx(), &[]))), (None, "none", None));
    }
}
