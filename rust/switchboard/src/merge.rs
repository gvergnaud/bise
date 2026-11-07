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
//! - His Merge or Close on a PR's tab (pr-integration, `HubCmd::PrAct`)
//!   goes through the item's gate, [`plan`] (the head he saw, the
//!   forge's yes now): `Effect::PrAct`, the answer as `Input::PrActed`;
//!   done, main's thread says it and the PR's open `merge` and `review`
//!   items close in the same step; refused, nothing closes.
//!
//! The generic part (any hub item with numbered options, `choice_kind`):
//! `open_card` / `close_card` here, the user's digit in [`Hub::card_choice`].

use super::*;
use crate::place::{Checks, MergeMethod, PrSnapshot, PrState, Review};
use bise_proto::hub::{PrAct, PrActErr};

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

/// pr-integration rule 6: the item opens (and stays) only while the PR
/// waits on his merge as `forge::yours` says (the row's words read the
/// same owner): ready, and not asking his review first (that item comes
/// first: one item per PR, never two).
fn waits_merge(pr: &PrSnapshot) -> bool {
    crate::forge::yours(pr, true) == Some(bise_proto::rows::PrYours::Merge)
}

/// The method `1` uses: squash, else merge, else rebase (the repo's).
pub fn method(pr: &PrSnapshot) -> Option<MergeMethod> {
    pr.facts.methods.first().copied()
}

/// What the forge is asked once a click is planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Merge(MergeMethod),
    Close,
}

/// The one gate of a merge or a close (the item's `1`, the tab's click):
/// the PR as the hub knows it, at the head he saw. A push since is
/// refused (`Moved`), never merged; a merge needs the forge's yes now
/// (open, mergeable, a method the repo allows).
pub fn plan(pr: Option<&PrSnapshot>, head: &str, act: PrAct) -> Result<Action, PrActErr> {
    let pr = pr.ok_or(PrActErr::Unknown)?;
    if crate::forge::ended(pr.state) {
        return Err(PrActErr::Ended);
    }
    if pr.head_oid != head {
        return Err(PrActErr::Moved);
    }
    match act {
        PrAct::Close => Ok(Action::Close),
        PrAct::Merge if pr.state == PrState::Open && pr.facts.mergeable => method(pr).map(Action::Merge).ok_or(PrActErr::NotMergeable),
        PrAct::Merge => Err(PrActErr::NotMergeable),
    }
}

/// His click on a PR's tab, carried to its answer.
pub struct Click {
    pub client: ClientId,
    pub req: u64,
    pub url: String,
    pub act: PrAct,
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

impl Merges {
    /// The items whose `1` runs (`gh pr merge`): `Hub::running_cards`.
    pub(crate) fn running(&self) -> impl Iterator<Item = u64> + '_ {
        self.running.iter().copied()
    }
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
    /// His clicks on a PR's tab that run, by the PR's link.
    by_hand: BTreeSet<String>,
    /// gh missing or logged out, said to main (once a run).
    gh_off_said: bool,
}

impl Hub {
    /// Open a hub item (`choice_kind`); its id.
    pub(super) fn open_card(&mut self, fx: &mut Fx, env: &mut dyn Env, item: HubItem) -> Option<u64> {
        let HubItem { kind, agent, text, place, pr } = item;
        let open = ci::CardOpen { kind: kind.into(), agent: agent.into(), text: text.into(), place: Some(place.into()), pr };
        self.core(fx, env, None, In::CardOpen(open));
        self.st.open_cards().filter(|c| c.kind == kind && c.place.as_deref() == Some(place)).map(|c| c.id).max()
    }

    /// Close a hub item (done, or withdrawn): `res` is main's line.
    pub(super) fn close_card(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, res: &str) {
        self.core(fx, env, None, In::CardClose(ci::CardClose { card, res: res.into() }));
    }

    /// The user's digit on a hub item (sb-core's `card_choice`; words
    /// went to main). The item stays open until its kind's handler
    /// closes it.
    pub(super) fn card_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, f: bise_proto::core::fx::CardChoice) {
        let bise_proto::core::fx::CardChoice { card, kind, place, pr, text: choice, .. } = f;
        // one arm per kind
        match kind.as_str() {
            "merge" => self.merge_choice(fx, env, card, place, pr, &choice),
            review::KIND => self.review_choice(fx, env, card, place, &choice),
            release_line::KIND => self.update_choice(fx, env, card, place, &choice),
            // dev-flow §5.1: a feature's try and merge items
            k @ (crate::feature::TRY | crate::feature::MERGE) => self.feature_choice(fx, card, k, place.as_deref(), &choice),
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
                // the item's PR as it was opened for (a restart: as it is)
                let head = self.merges.keys.get(&card).map(|k| k.head.clone());
                let planned = pr.filter(ready).and_then(|p| {
                    let head = head.unwrap_or_else(|| p.head_oid.clone());
                    match plan(Some(&p), &head, PrAct::Merge) {
                        Ok(Action::Merge(m)) => Some((p, m)),
                        _ => None,
                    }
                });
                let Some((pr, m)) = planned else {
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
                // pr-news's PR line: nothing for him to do, the PR, its words
                let url = self.prs.get(place).map(|p| p.url.clone()).unwrap_or_default();
                let rec = feed_rec::Pr { v: 1, state: "done".into(), number, url, text: "you merged it".into() };
                fx.push(feed(MAIN, Feed::Pr(rec)));
                self.close_card(fx, env, card, "merged");
            }
            Err(e) => {
                let e = clip(&one_line(&e), 160);
                self.merges.failed.insert(place.to_string(), format!("gh couldn't merge it: {}", e));
                fx.push(feed(MAIN, Feed::warn(format!("gh couldn't merge #{}: {}", number, e))));
                self.close_card(fx, env, card, "merge failed");
            }
        }
    }

    /// The PR the hub follows at `url` (pr_link's one form): a place's
    /// first, else a followed one.
    fn pr_at(&self, url: &str) -> Option<PrSnapshot> {
        let link = crate::forge::pr_link(url)?.url;
        let same = |p: &&PrSnapshot| crate::forge::pr_link(&p.url).is_some_and(|l| l.url == link);
        self.prs.values().find(same).or_else(|| self.followed.values().find(same)).cloned()
    }

    /// His Merge or Close on a PR's tab (pr-integration, his click only):
    /// `plan` says yes and the daemon runs it, or the click is answered
    /// with why not and nothing runs.
    pub(super) fn pr_act(&mut self, fx: &mut Fx, c: Click, head: &str) {
        let pr = self.pr_at(&c.url);
        let running = pr.as_ref().is_some_and(|p| {
            self.merges.by_hand.contains(&p.url) || self.st.open_cards().any(|k| k.kind == "merge" && k.pr == Some(p.number) && self.merges.running.contains(&k.id))
        });
        let planned = if running { Err(PrActErr::Running) } else { plan(pr.as_ref(), head, c.act) };
        match (planned, pr) {
            (Ok(action), Some(pr)) => {
                self.merges.by_hand.insert(pr.url.clone());
                fx.push(Effect::PrAct { client: c.client, req: c.req, url: pr.url.clone(), number: pr.number, head: pr.head_oid.clone(), action });
            }
            (Err(e), _) => fx.push(Effect::PrActed { client: c.client, req: c.req, url: c.url, act: c.act, err: Some(e), why: None }),
            (Ok(_), None) => fx.push(Effect::PrActed { client: c.client, req: c.req, url: c.url, act: c.act, err: Some(PrActErr::Unknown), why: None }),
        }
    }

    /// The forge's answer to his click. Done: main's thread says it as
    /// the item's `1` does, and every open `merge` or `review` item of
    /// that PR closes in the same step (the forge's next answer does the
    /// rest). Refused: nothing closes, main's thread says gh's reason.
    pub(super) fn pr_acted(&mut self, fx: &mut Fx, env: &mut dyn Env, c: Click, number: u64, res: Result<(), String>) {
        self.merges.by_hand.remove(&c.url);
        self.dirty = true;
        let (did, verb) = match c.act {
            PrAct::Merge => ("merged", "merge"),
            PrAct::Close => ("closed", "close"),
        };
        match res {
            Ok(()) => {
                let rec = feed_rec::Pr { v: 1, state: "done".into(), number, url: c.url.clone(), text: format!("you {did} it") };
                fx.push(feed(MAIN, Feed::Pr(rec)));
                for card in self.cards_of_pr(&c.url, number) {
                    self.merges.running.remove(&card);
                    self.merges.keys.remove(&card);
                    self.close_card(fx, env, card, did);
                }
                fx.push(Effect::PrActed { client: c.client, req: c.req, url: c.url, act: c.act, err: None, why: None });
            }
            Err(e) => {
                let e = clip(&one_line(&e), 160);
                fx.push(feed(MAIN, Feed::warn(format!("gh couldn't {verb} #{number}: {e}"))));
                fx.push(Effect::PrActed { client: c.client, req: c.req, url: c.url, act: c.act, err: Some(PrActErr::Forge), why: Some(e) });
            }
        }
    }

    /// The open `merge` and `review` items of the PR at `url`: a place's
    /// (its PR's link) or a followed one's (`pr:<url>`).
    fn cards_of_pr(&self, url: &str, number: u64) -> Vec<u64> {
        let link = crate::forge::pr_link(url).map(|l| l.url);
        let is_it = |place: &str| {
            place.strip_prefix("pr:").map_or_else(
                || self.prs.get(place).is_some_and(|p| crate::forge::pr_link(&p.url).map(|l| l.url) == link),
                |u| crate::forge::pr_link(u).map(|l| l.url) == link,
            )
        };
        self.st
            .open_cards()
            .filter(|k| (k.kind == "merge" || k.kind == review::KIND) && k.pr == Some(number) && k.place.as_deref().is_some_and(is_it))
            .map(|k| k.id)
            .collect()
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
            let now = pr.as_ref().filter(|p| Some(p.number) == n && waits_merge(p)).map(key);
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
            let Some(pr) = p.pr.as_ref().filter(|pr| waits_merge(pr)) else { continue };
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
        fx.push(feed(
            MAIN,
            Feed::warn(format!("i can't follow the PRs: {}. run `gh auth login` once (or set GITHUB_TOKEN) and their state shows here", what)),
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
                ..Default::default()
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

    /// The one gate of a merge or a close: the head he saw, the forge's
    /// yes now, never an ended PR.
    #[test]
    fn a_click_is_planned_at_the_head_he_saw() {
        assert_eq!(plan(Some(&pr()), "h1", PrAct::Merge), Ok(Action::Merge(MergeMethod::Squash)));
        assert_eq!(plan(Some(&pr()), "h1", PrAct::Close), Ok(Action::Close));
        assert_eq!(plan(None, "h1", PrAct::Merge), Err(PrActErr::Unknown));
        // a push since his click: refused, never merged (nor closed)
        assert_eq!(plan(Some(&pr()), "h0", PrAct::Merge), Err(PrActErr::Moved));
        assert_eq!(plan(Some(&pr()), "h0", PrAct::Close), Err(PrActErr::Moved));
        let with = |f: &dyn Fn(&mut PrSnapshot)| {
            let mut p = pr();
            f(&mut p);
            p
        };
        for s in [PrState::Merged, PrState::Closed] {
            assert_eq!(plan(Some(&with(&|p| p.state = s)), "h1", PrAct::Close), Err(PrActErr::Ended));
        }
        // a draft closes, never merges; nor does a PR the forge refuses now
        assert_eq!(plan(Some(&with(&|p| p.state = PrState::Draft)), "h1", PrAct::Close), Ok(Action::Close));
        assert_eq!(plan(Some(&with(&|p| p.state = PrState::Draft)), "h1", PrAct::Merge), Err(PrActErr::NotMergeable));
        assert_eq!(plan(Some(&with(&|p| p.facts.mergeable = false)), "h1", PrAct::Merge), Err(PrActErr::NotMergeable));
        assert_eq!(plan(Some(&with(&|p| p.facts.methods.clear())), "h1", PrAct::Merge), Err(PrActErr::NotMergeable));
        // his merge needs no approval the forge doesn't: pending review, mergeable
        assert_eq!(plan(Some(&with(&|p| p.review = Review::Pending)), "h1", PrAct::Merge), Ok(Action::Merge(MergeMethod::Squash)));
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
