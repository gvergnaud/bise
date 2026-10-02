//! The ready-to-merge item (pr-design §6.3, pr-merge): the hub's own
//! inbox item about a place's PR, kind `merge`, for the user only.
//!
//! - Opened when the PR is ready: open (not a draft), approved or no
//!   review required, its checks passing (or none at all), and GitHub
//!   would merge it now (`mergeStateStatus` clean, a method allowed).
//! - Withdrawn when the PR changes (new commits, a review, merged or
//!   closed on GitHub, the place gone): the hub closes it, main's thread
//!   says why. `3 not yet` closes it and stays quiet until the PR changes.
//! - `1` runs `gh pr merge <n> --<method> --match-head-commit <head>`
//!   with the user's login, off the hub's loop (`Effect::Merge`, the
//!   answer as `Input::Merged`). No agent ever merges.
//! - `2 open it on GitHub`: the TUI opens the link; the item stays.
//!
//! The generic part (any hub item with numbered options, `choice_kind`):
//! `open_card` / `close_card` here, the user's digit in [`Hub::card_choice`].

use super::*;
use crate::place::{Checks, MergeMethod, PrSnapshot, PrState, Review};

/// What the item was opened for: when it differs, the PR changed and
/// the item goes (a push moves the head, a review the approvers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeKey {
    number: u64,
    head: String,
    review: Review,
    approved_by: Vec<String>,
}

pub fn key(pr: &PrSnapshot) -> MergeKey {
    MergeKey { number: pr.number, head: pr.head_oid.clone(), review: pr.review, approved_by: pr.facts.approved_by.clone() }
}

/// pr-design §6.3: approved (or no review required), checks passing,
/// and the forge would merge it now with a method the repo allows.
pub fn ready(pr: &PrSnapshot) -> bool {
    pr.state == PrState::Open
        && matches!(pr.review, Review::Approved | Review::None)
        && (pr.checks == Checks::Pass || (pr.checks == Checks::None && pr.facts.checks == 0))
        && pr.facts.mergeable
        && !pr.facts.methods.is_empty()
}

/// The method `1` uses: squash, else merge, else rebase (the repo's).
pub fn method(pr: &PrSnapshot) -> Option<MergeMethod> {
    pr.facts.methods.first().copied()
}

/// The item's text (the TUI's `merge` shape reads it): the head line,
/// the PR's title, its facts, its link, then the options.
///
/// ```text
/// #409 is ready to merge
/// the cookie banner stops covering buy
/// approved by alice · 6 of 6 checks pass · 3 commits · +84 −12
/// github.com/acme/web/pull/409
///
/// 1. squash and merge
/// 2. open it on GitHub
/// 3. not yet
/// ```
pub fn text(pr: &PrSnapshot) -> String {
    let f = &pr.facts;
    let mut facts: Vec<String> = Vec::new();
    facts.push(match f.approved_by.len() {
        0 if pr.review == Review::Approved => "approved".into(),
        0 => "no review required".into(),
        _ => format!("approved by {}", f.approved_by.join(", ")),
    });
    facts.push(match f.checks {
        0 => "no checks".into(),
        n => format!("{n} of {n} checks pass"),
    });
    if f.commits > 0 {
        facts.push(format!("{} commit{}", f.commits, if f.commits == 1 { "" } else { "s" }));
    }
    if f.additions > 0 || f.deletions > 0 {
        facts.push(format!("+{} −{}", f.additions, f.deletions));
    }
    let link = pr.url.strip_prefix("https://").unwrap_or(&pr.url);
    let first = method(pr).map_or("merge", |m| m.label());
    let mut out = format!("#{} is ready to merge\n", pr.number);
    if !f.title.trim().is_empty() {
        out.push_str(&one_line(&f.title));
        out.push('\n');
    }
    out.push_str(&facts.join(" · "));
    out.push('\n');
    out.push_str(link);
    out.push_str(&format!("\n\n1. {first}\n2. open it on GitHub\n3. not yet"));
    out
}

/// Why an open item goes: the PR as it is now (None: no PR, or the place
/// is gone).
fn withdrawn(pr: Option<&PrSnapshot>) -> &'static str {
    match pr.map(|p| p.state) {
        None => "withdrawn: its place is gone",
        Some(PrState::Merged) => "withdrawn: merged on GitHub",
        Some(PrState::Closed) => "withdrawn: closed on GitHub",
        Some(PrState::Draft) => "withdrawn: back to draft",
        Some(PrState::Open) => "withdrawn: the PR changed",
    }
}

/// A hub item to open (`choice_kind`): about `place` (and its PR).
pub struct HubItem<'a> {
    pub kind: &'a str,
    pub agent: &'a str,
    pub text: &'a str,
    pub place: &'a str,
    pub pr: Option<u64>,
}

/// The hub's side of the item (runtime: a restart adopts the open items
/// at the forge's first answer).
#[derive(Debug, Default)]
pub struct Merges {
    /// The open items, by card id: what each was opened for.
    keys: BTreeMap<u64, MergeKey>,
    /// `3 not yet`, by place: quiet while the PR is the same.
    quiet: BTreeMap<String, MergeKey>,
    /// `1` sent, by card id: `gh pr merge` runs.
    running: BTreeSet<u64>,
    /// The last failed merge, by place: said on the next item.
    failed: BTreeMap<String, String>,
    /// gh missing or logged out, said to main (once a run).
    gh_off_said: bool,
}

impl Hub {
    /// Open a hub item (`choice_kind`); its id.
    pub(super) fn open_card(&mut self, fx: &mut Fx, env: &mut dyn Env, item: HubItem) -> Option<u64> {
        let HubItem { kind, agent, text, place, pr } = item;
        self.core(fx, env, None, json!({"t": "card_open", "kind": kind, "agent": agent, "text": text, "place": place, "pr": pr}));
        self.st.open_cards().filter(|c| c.kind == kind && c.place.as_deref() == Some(place)).map(|c| c.id).max()
    }

    /// Close a hub item (done, or withdrawn): `res` is main's line.
    pub(super) fn close_card(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, res: &str) {
        self.core(fx, env, None, json!({"t": "card_close", "card": card, "res": res}));
    }

    /// The user's digit on a hub item (sb-core's `card_choice`; words
    /// went to main). The item stays open until its kind's handler
    /// closes it.
    pub(super) fn card_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, f: &Value) {
        let card = f["card"].as_u64().unwrap_or(0);
        let place = f["place"].as_str().map(str::to_string);
        let pr = f["pr"].as_u64();
        let choice = jstr(f, "text");
        // one arm per kind
        match jstr(f, "kind").as_str() {
            "merge" => self.merge_choice(fx, env, card, place, pr, &choice),
            update_card::KIND => self.update_choice(fx, env, card, place, &choice),
            // dev-flow §5.1: a feature's try and merge items
            k @ (crate::feature::TRY | crate::feature::MERGE) => self.feature_choice(fx, k, place.as_deref(), &choice),
            _ => {}
        }
    }

    fn merge_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, place: Option<String>, n: Option<u64>, choice: &str) {
        let Some(place) = place else {
            return self.close_card(fx, env, card, "withdrawn: no place");
        };
        let pr = self.prs.get(&place).filter(|p| Some(p.number) == n).cloned();
        match choice {
            "1" => {
                if self.merges.running.contains(&card) {
                    return;
                }
                let Some((pr, m)) = pr.filter(ready).and_then(|p| method(&p).map(|m| (p, m))) else {
                    let why = withdrawn(self.prs.get(&place));
                    return self.close_card(fx, env, card, why);
                };
                self.merges.running.insert(card);
                self.merges.failed.remove(&place);
                self.dirty = true;
                fx.push(Effect::Merge { card, place, number: pr.number, head: pr.head_oid.clone(), method: m });
            }
            // the TUI opened the link: the item stays
            "2" => {}
            "3" => {
                if let Some(pr) = pr {
                    self.merges.quiet.insert(place, key(&pr));
                }
                self.close_card(fx, env, card, "not yet");
            }
            _ => {}
        }
    }

    /// `gh pr merge`'s answer (`Effect::Merge`). Merged: the item closes,
    /// the forge's next answer does the rest (pr-design §6.4). Refused:
    /// the item closes with gh's reason in main's thread, and opens again
    /// at the forge's next answer if the PR is still ready, the reason in
    /// its note.
    pub(super) fn merged(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, place: &str, number: u64, res: Result<(), String>) {
        self.merges.running.remove(&card);
        self.merges.keys.remove(&card);
        self.dirty = true;
        match res {
            Ok(()) => {
                // pr-news's PR line: tone, number, link, words
                let url = self.prs.get(place).map(|p| p.url.clone()).unwrap_or_default();
                let fields = ["dim".to_string(), number.to_string(), url, "you merged it".to_string()];
                fx.push(line(MAIN, "pr", &join_fields(&fields)));
                self.close_card(fx, env, card, "merged");
            }
            Err(e) => {
                let e = clip(&one_line(&e), 160);
                self.merges.failed.insert(place.to_string(), format!("gh couldn't merge it: {}", e));
                fx.push(line(MAIN, "warn", &format!("gh couldn't merge #{}: {}", number, e)));
                self.close_card(fx, env, card, "merge failed");
            }
        }
    }

    /// The items against the PRs, after each answer of the forge: withdraw
    /// the stale ones, open the ready ones.
    pub(super) fn merge_sync(&mut self, fx: &mut Fx, env: &mut dyn Env) {
        let open: Vec<(u64, Option<String>, Option<u64>)> =
            self.st.open_cards().filter(|c| c.kind == "merge").map(|c| (c.id, c.place.clone(), c.pr)).collect();
        self.merges.keys.retain(|id, _| open.iter().any(|(o, _, _)| o == id));
        let mut asked: BTreeSet<String> = BTreeSet::new();
        for (id, place, n) in open {
            if self.merges.running.contains(&id) {
                asked.extend(place);
                continue;
            }
            let pr = place.as_ref().and_then(|p| self.prs.get(p)).cloned();
            let now = pr.as_ref().filter(|p| Some(p.number) == n && ready(p)).map(key);
            match (now, self.merges.keys.get(&id).cloned()) {
                (None, _) => self.close_card(fx, env, id, withdrawn(pr.as_ref())),
                (Some(k), Some(was)) if k != was => self.close_card(fx, env, id, withdrawn(pr.as_ref())),
                (Some(k), was) => {
                    // a restart: the item is the PR's as it is now
                    if was.is_none() {
                        self.merges.keys.insert(id, k);
                    }
                    asked.extend(place);
                }
            }
        }
        let places = crate::place::places(&self.st, &self.prs);
        self.merges.quiet.retain(|p, k| self.prs.get(p).is_some_and(|pr| key(pr) == *k));
        for p in places.iter().filter(|p| p.kind == crate::place::PlaceKind::Worktree) {
            let Some(pr) = p.pr.as_ref().filter(|pr| ready(pr)) else { continue };
            if asked.contains(&p.id) || self.merges.quiet.contains_key(&p.id) {
                continue;
            }
            let agent = p.agents.first().cloned().unwrap_or_else(|| MAIN.to_string());
            let item = HubItem { kind: "merge", agent: &agent, text: &text(pr), place: &p.id, pr: Some(pr.number) };
            if let Some(id) = self.open_card(fx, env, item) {
                self.merges.keys.insert(id, key(pr));
            }
        }
    }

    /// pr-design §8: gh missing or logged out while a worktree has a
    /// branch to follow: main says it once (this run of the hub); an
    /// outage or the rate limit only fades the boxes.
    pub(super) fn gh_off(&mut self, fx: &mut Fx, e: &crate::forge::ForgeError) {
        use crate::forge::ForgeError;
        let what = match e {
            ForgeError::Missing => "gh isn't installed",
            ForgeError::Auth(_) => "gh isn't logged in",
            _ => return,
        };
        if self.merges.gh_off_said || self.pr_watches().is_empty() {
            return;
        }
        self.merges.gh_off_said = true;
        fx.push(line(
            MAIN,
            "warn",
            &format!("i can't follow the PRs: {}. run `gh auth login` once (or set GITHUB_TOKEN) and their state shows here", what),
        ));
    }

    /// The view's note on a `merge` item: merging now, or why the last
    /// `1` failed.
    pub(super) fn merge_note(&self, c: &Card) -> Option<String> {
        if self.merges.running.contains(&c.id) {
            return Some("merging…".into());
        }
        self.merges.failed.get(c.place.as_deref()?).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::PrFacts;

    fn pr() -> PrSnapshot {
        PrSnapshot {
            number: 409,
            url: "https://github.com/acme/web/pull/409".into(),
            branch: "sb/cookies".into(),
            head_oid: "h1".into(),
            state: PrState::Open,
            review: Review::Approved,
            checks: Checks::Pass,
            updated_at: "t".into(),
            facts: Box::new(PrFacts {
                title: "the cookie banner stops covering buy".into(),
                approved_by: vec!["alice".into()],
                commits: 3,
                additions: 84,
                deletions: 12,
                checks: 6,
                mergeable: true,
                methods: vec![MergeMethod::Squash, MergeMethod::Merge],
            }),
        }
    }

    #[test]
    fn the_item_s_text() {
        assert_eq!(
            text(&pr()),
            "#409 is ready to merge\nthe cookie banner stops covering buy\napproved by alice · 6 of 6 checks pass · 3 commits · +84 −12\ngithub.com/acme/web/pull/409\n\n1. squash and merge\n2. open it on GitHub\n3. not yet"
        );
        let mut p = pr();
        p.review = Review::None;
        p.facts.approved_by.clear();
        p.facts.checks = 0;
        p.facts.methods = vec![MergeMethod::Rebase];
        let t = text(&p);
        assert!(t.contains("\nno review required · no checks · 3 commits"), "{t}");
        assert!(t.ends_with("1. rebase and merge\n2. open it on GitHub\n3. not yet"), "{t}");
    }

    #[test]
    fn ready_to_merge() {
        assert!(ready(&pr()));
        let not = |f: &dyn Fn(&mut PrSnapshot)| {
            let mut p = pr();
            f(&mut p);
            !ready(&p)
        };
        assert!(not(&|p| p.state = PrState::Draft));
        assert!(not(&|p| p.state = PrState::Merged));
        assert!(not(&|p| p.review = Review::ChangesRequested));
        assert!(not(&|p| p.review = Review::Pending));
        assert!(not(&|p| p.checks = Checks::Running));
        assert!(not(&|p| p.checks = Checks::Fail { failing: vec!["t".into()] }));
        assert!(not(&|p| p.facts.mergeable = false));
        assert!(not(&|p| p.facts.methods.clear()));
        // checks not reported yet while some exist: not yet
        assert!(not(&|p| p.checks = Checks::None));
        // a repo with no checks at all, no review required: ready
        let mut p = pr();
        p.checks = Checks::None;
        p.facts.checks = 0;
        p.review = Review::None;
        assert!(ready(&p));
    }

    #[test]
    fn a_change_is_a_new_key() {
        let k = key(&pr());
        let mut p = pr();
        p.updated_at = "later".into();
        p.facts.title = "renamed".into();
        assert_eq!(key(&p), k, "a comment or a new title keeps the item");
        p.head_oid = "h2".into();
        assert_ne!(key(&p), k);
        let mut p = pr();
        p.facts.approved_by.push("bob".into());
        assert_ne!(key(&p), k);
    }
}
