//! The "your review" item (pr-integration rule 6): the hub's own inbox
//! item, kind `review`, for the user only, one per PR whose review is
//! asked of him (`forge::yours` says `Review`). Beside pr-merge's `merge`
//! item (merge.rs): the two read the same owner, so a PR has one item or
//! none, never two (a PR that asks his review and is ready to merge shows
//! the review; once reviewed, the merge item opens in the same sync).
//!
//! - Opened after each answer of the forge, for a place's PR or a PR an
//!   agent only reviews (followed, no place).
//! - Withdrawn when the request goes (reviewed, merged, closed, the place
//!   gone): main's thread says why.
//! - `1 open it on GitHub`: the option's typed act opens the link
//!   (`rows::opt_act`), the item stays. `2 not now`: closed, and quiet
//!   until the PR's head or the request changes (typed facts, never the
//!   item's words).

use super::*;
use crate::place::PrSnapshot;
use bise_proto::rows::PrYours;

/// The item's kind.
pub const KIND: &str = "review";

/// What `2 not now` stays quiet for: the PR, its head, the request.
#[derive(Clone, Debug, PartialEq, Eq)]
struct QuietKey {
    number: u64,
    head: String,
    asked: bool,
}

fn quiet_key(pr: &PrSnapshot) -> QuietKey {
    QuietKey { number: pr.number, head: pr.head_oid.clone(), asked: pr.facts.asked_you }
}

/// The hub's side of the items (runtime: a restart adopts the open ones
/// at the forge's first answer).
#[derive(Debug, Default)]
pub struct Reviews {
    /// `2 not now`, by the item's key ([`Hub::review_targets`]).
    quiet: BTreeMap<String, QuietKey>,
}

/// One PR that waits on his review: the item's key, its agent, the PR.
struct Target {
    /// A place's id; for a followed PR `pr:<url>`, an opaque dedupe key
    /// (the card's `place`) that nothing parses back: a reader that needs
    /// "followed or place" gets a typed field, never `starts_with`.
    key: String,
    agent: String,
    pr: PrSnapshot,
}

/// The item's text: what, why, the link, then the options.
///
/// ```text
/// #412 asks your review
/// faster signup
/// your review · checks ✓ 12/12
/// github.com/acme/web/pull/412
///
/// 1. open it on GitHub
/// 2. not now
/// ```
pub fn text(pr: &PrSnapshot, words: &str) -> String {
    let link = pr.url.strip_prefix("https://").unwrap_or(&pr.url);
    let mut out = format!("#{} asks your review\n", pr.number);
    if !pr.facts.title.trim().is_empty() {
        out.push_str(&one_line(&pr.facts.title));
        out.push('\n');
    }
    out.push_str(words);
    out.push('\n');
    out.push_str(link);
    out.push_str("\n\n1. open it on GitHub\n2. not now");
    out
}

fn withdrawn(pr: Option<&PrSnapshot>) -> &'static str {
    match pr.map(|p| p.state) {
        None => "withdrawn: no PR to review",
        Some(crate::place::PrState::Merged) => "withdrawn: merged on GitHub",
        Some(crate::place::PrState::Closed) => "withdrawn: closed on GitHub",
        Some(_) => "withdrawn: your review is no longer asked",
    }
}

impl Hub {
    /// The PRs that wait on his review now, by item key: the places' PRs
    /// (their first agent's), then the followed ones (main's).
    fn review_targets(&self) -> Vec<Target> {
        let mut out = Vec::new();
        for p in crate::place::places(&self.st, &self.prs).into_iter().filter(|p| p.kind == crate::place::PlaceKind::Worktree) {
            let Some(pr) = p.pr.filter(|pr| crate::forge::yours(pr, true) == Some(PrYours::Review)) else { continue };
            let agent = p.agents.first().cloned().unwrap_or_else(|| MAIN.to_string());
            out.push(Target { key: p.id, agent, pr });
        }
        for (url, pr) in &self.followed {
            if crate::forge::yours(pr, false) == Some(PrYours::Review) {
                out.push(Target { key: format!("pr:{url}"), agent: MAIN.to_string(), pr: pr.clone() });
            }
        }
        out
    }

    /// The items against the PRs, after each answer of the forge (after
    /// `merge_sync`): withdraw the ones whose request went, open one for
    /// each PR that asks his review and has none.
    pub(super) fn review_sync(&mut self, fx: &mut Fx, env: &mut dyn Env) {
        let targets = self.review_targets();
        let open: Vec<(u64, Option<String>, Option<u64>)> =
            self.st.open_cards().filter(|c| c.kind == KIND).map(|c| (c.id, c.place.clone(), c.pr)).collect();
        let mut has: BTreeSet<String> = BTreeSet::new();
        for (id, key, n) in open {
            let t = targets.iter().find(|t| Some(&t.key) == key.as_ref() && Some(t.pr.number) == n);
            match (t, key) {
                (Some(_), Some(k)) if !has.contains(&k) => {
                    has.insert(k);
                }
                (_, key) => {
                    // the PR as it is now, for main's line: a place's by its
                    // id, a followed one by its number (its key is opaque)
                    let pr = key
                        .as_deref()
                        .and_then(|k| self.prs.get(k))
                        .or_else(|| self.followed.values().find(|p| Some(p.number) == n));
                    let why = withdrawn(pr);
                    self.close_card(fx, env, id, why);
                }
            }
        }
        let keys: BTreeSet<&String> = targets.iter().map(|t| &t.key).collect();
        self.reviews.quiet.retain(|k, q| keys.contains(k) && targets.iter().any(|t| &t.key == k && quiet_key(&t.pr) == *q));
        for t in &targets {
            if has.contains(&t.key) || self.reviews.quiet.contains_key(&t.key) {
                continue;
            }
            let words = crate::forge::news::pr_words(&crate::place::PrView { yours: Some(PrYours::Review), ..crate::place::PrView::of(&t.pr, None) });
            let item = merge::HubItem { kind: KIND, agent: &t.agent, text: &text(&t.pr, &words), place: &t.key, pr: Some(t.pr.number) };
            self.open_card(fx, env, item);
        }
    }

    /// His digit on a review item: `1` opens the link (the client, by the
    /// option's act; the item stays), `2` closes it, quiet until the PR
    /// changes.
    pub(super) fn review_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, key: Option<String>, choice: &str) {
        if choice != "2" {
            return;
        }
        if let Some(t) = key.and_then(|k| self.review_targets().into_iter().find(|t| t.key == k)) {
            self.reviews.quiet.insert(t.key, quiet_key(&t.pr));
        }
        self.close_card(fx, env, card, "not now");
    }
}
