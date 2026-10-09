//! Places (dev-flow §3.1): where agents work, shared, not owned. The
//! shared folder is one place; each worktree (a folder and its branch)
//! is another, which several agents may join (`sb spawn --place`, `sb
//! move`). A PR belongs to a place's branch, never to an agent.
//!
//! Storage: a place is the `place` id in the `ws` of each agent in it
//! (journaled by sb-core with the rest of the agent, `Workspace::place`);
//! this module derives the table from the state. An older journal's
//! worktree has no id: it is its agent's own place (`wt:<dir>`), so the
//! journal needs no migration.
//!
//! THE CONTRACT (frozen by wave 1, docs/pr-briefs.md §Contracts): the
//! types below and their JSON (serde) are what wave 2 builds on:
//! pr-hub fills [`Place::pr`] (`places(st, prs)`), pr-tui reads
//! `places: [PlaceView]` in the TUI snapshot and `place_id` on each
//! agent. Change them only with main and the wave 2 agents.
//!
//! BISE-136 (designer's call 8) adds a kind of id, not a type: a private
//! worktree an agent told the hub about (`gate.sh new`) is a worktree
//! place `pt:<path>` ([`private_id`]), its branch what it has checked
//! out (None: detached, the views name it by its folder).

use crate::model::{Lifecycle, Mode, State};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The id of the shared folder's place.
pub const SHARED: &str = "shared";

/// The id of the worktree first made for the agent of dir `dir`.
pub fn worktree_id(dir: &str) -> String {
    format!("wt:{}", dir)
}

/// BISE-136, designer's call 8: the id of a private worktree an agent
/// told the hub about (`gate.sh new`, `sb worktree <path>`): `pt:<path>`.
/// It is a worktree like any other (a row with its mark alone, a box
/// shared), with no branch when detached: the views then name it by its
/// folder, the path's last part.
pub fn private_id(path: &str) -> String {
    format!("pt:{}", path)
}

/// The path of a private worktree's id (None: not one).
pub fn private_path(id: &str) -> Option<&str> {
    id.strip_prefix("pt:")
}

/// The id of the place agent `a` works in: its private worktree when it
/// told one (live agents only), else its workspace's place.
pub fn id_of(a: &crate::model::Agent) -> String {
    match &a.place {
        Some(p) if a.lifecycle != Lifecycle::Archived => private_id(p),
        _ => a.ws.place_id(&a.dir),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    Shared,
    Worktree,
    /// dev-flow §5.1: a feature branch, id `feature:<name>`: its agents
    /// (each in its own worktree, or sharing one) all land on it. `path`
    /// is the shared folder (the branch is checked out nowhere).
    Feature,
}

/// A place (dev-flow §3.1). `agents`: the agents in it, not archived,
/// in the hub's order (the first one names the box). `base`: the commit
/// the worktree was made from. `pr`: the PR of its branch (pr-hub, wave
/// 2; None until then).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub id: String,
    pub kind: PlaceKind,
    pub path: String,
    pub branch: Option<String>,
    pub base: Option<String>,
    pub agents: Vec<String>,
    pub pr: Option<PrSnapshot>,
}

/// pr-design §9: what the forge says about the PR whose head is a
/// place's branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrSnapshot {
    pub number: u64,
    pub url: String,
    pub branch: String,
    pub head_oid: String,
    pub state: PrState,
    pub review: Review,
    pub checks: Checks,
    /// The forge's `updatedAt` (ISO 8601).
    pub updated_at: String,
    /// What the ready-to-merge item says and needs (pr-merge, wave 3);
    /// boxed: the events carry two snapshots.
    #[serde(default)]
    pub facts: Box<PrFacts>,
}

/// pr-design §6.3: the ready-to-merge item's lines (`the cookie banner
/// stops covering buy`, `approved by alice · 6 of 6 checks pass · 3 commits ·
/// +84 −12`) and what `1` needs: whether the forge would merge it now
/// (GitHub's `mergeStateStatus` clean: the required reviews and checks
/// are there, no conflict, not behind a required base) and the methods
/// the repo allows, the one to use first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrFacts {
    pub title: String,
    /// The reviewers whose latest review approves it (logins).
    pub approved_by: Vec<String>,
    pub commits: u32,
    pub additions: u32,
    pub deletions: u32,
    /// The head commit's checks (check runs and statuses), counted.
    pub checks: u32,
    pub mergeable: bool,
    /// The repo's allowed methods, in pr-design §6.3's order: squash,
    /// else merge, else rebase.
    pub methods: Vec<MergeMethod>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    Squash,
    Merge,
    Rebase,
}

impl MergeMethod {
    /// `gh pr merge`'s flag.
    pub fn flag(self) -> &'static str {
        match self {
            MergeMethod::Squash => "--squash",
            MergeMethod::Merge => "--merge",
            MergeMethod::Rebase => "--rebase",
        }
    }

    /// The item's option 1 (`squash and merge`).
    pub fn label(self) -> &'static str {
        match self {
            MergeMethod::Squash => "squash and merge",
            MergeMethod::Merge => "merge",
            MergeMethod::Rebase => "rebase and merge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Draft,
    Open,
    Merged,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    None,
    Pending,
    Approved,
    ChangesRequested,
}

/// JSON: `{"state": "pass"}`, `{"state": "fail", "failing": ["ci/test"]}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Checks {
    None,
    Running,
    Pass,
    Fail { failing: Vec<String> },
}

/// A place as the TUI draws it (pr-design §4.1): the snapshot's
/// `places`, worktrees only (the shared folder is never a box), in the
/// order of their first agent. `lid`: the held line under the border
/// (`waits to land · 2nd`; pr-hub adds the PR's), None when nothing to say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    pub id: String,
    pub branch: Option<String>,
    pub agents: Vec<String>,
    pub pr: Option<PrView>,
    pub lid: Option<String>,
    /// dev-flow §5.1, pr-design §4.1 item 8: a feature branch's place
    /// (`id` = `feature:<name>`, `branch` = the name, never a PR): a box
    /// with 2+ live agents even in separate worktrees. `trying`: its try
    /// build builds or is on trial, the mark is `Δ` (else `ψ`).
    #[serde(default)]
    pub feature: bool,
    #[serde(default)]
    pub trying: bool,
}

/// The PR as the TUI shows it. `stale_ms`: how old the last answer of
/// the forge is, when it is late (offline, rate limit); None when fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrView {
    pub number: u64,
    pub url: String,
    /// its head branch (P4c-4a: the typed row's, `rows::Pr.branch`)
    #[serde(default)]
    pub branch: String,
    pub state: PrState,
    pub review: Review,
    pub checks: Checks,
    pub stale_ms: Option<u64>,
}

impl PrView {
    /// The forge's PR as the views carry it, `stale_ms` its age when late.
    pub fn of(pr: &PrSnapshot, stale_ms: Option<u64>) -> PrView {
        PrView {
            number: pr.number,
            url: pr.url.clone(),
            branch: pr.branch.clone(),
            state: pr.state,
            review: pr.review,
            checks: pr.checks.clone(),
            stale_ms,
        }
    }
}

/// The places of a state: the shared folder first (always), then each
/// worktree with a live folder, in the order of its first agent. A
/// worktree whose agents are all archived stays while its folder does
/// (a drop that could not remove it, or, from wave 2, an open PR),
/// with no agents. `prs`: the PR of each place id (pr-hub).
pub fn places(st: &State, prs: &BTreeMap<String, PrSnapshot>) -> Vec<Place> {
    let mut out = vec![Place {
        id: SHARED.to_string(),
        kind: PlaceKind::Shared,
        path: st.agents.get(crate::model::MAIN).map(|a| a.ws.path.clone()).unwrap_or_default(),
        branch: None,
        base: None,
        agents: Vec::new(),
        pr: prs.get(SHARED).cloned(),
    }];
    for a in st.order.iter().filter_map(|n| st.agents.get(n)) {
        let live = a.lifecycle != Lifecycle::Archived;
        if a.ws.mode == Mode::Worktree && a.ws.dropped {
            continue;
        }
        // dev-flow §5.1: a feature's agents are its place, whatever their
        // worktrees (a feature's place lives while one of them is live)
        let feature = a.ws.feature().filter(|_| live);
        if a.ws.feature().is_some() && feature.is_none() {
            continue;
        }
        let id = match feature {
            Some(f) => crate::feature::place_id(f),
            None => a.ws.place_id(&a.dir),
        };
        let i = match out.iter().position(|p| p.id == id) {
            Some(i) => i,
            None => {
                out.push(match feature {
                    Some(f) => Place {
                        id: id.clone(),
                        kind: PlaceKind::Feature,
                        path: out[0].path.clone(),
                        branch: Some(f.to_string()),
                        base: None,
                        agents: Vec::new(),
                        pr: None,
                    },
                    None => Place {
                        id: id.clone(),
                        kind: PlaceKind::Worktree,
                        path: a.ws.path.clone(),
                        branch: a.ws.branch.clone(),
                        base: a.ws.base_commit.clone(),
                        agents: Vec::new(),
                        pr: prs.get(&id).cloned(),
                    },
                });
                out.len() - 1
            }
        };
        // BISE-136: an agent in a private worktree is in that place (its
        // workspace's worktree stays, for its folder and its PR)
        let i = match a.place.as_ref().filter(|_| live) {
            Some(path) => {
                let pid = private_id(path);
                match out.iter().position(|p| p.id == pid) {
                    Some(j) => j,
                    None => {
                        out.push(Place {
                            id: pid.clone(),
                            kind: PlaceKind::Worktree,
                            path: path.clone(),
                            branch: a.place_branch.clone(),
                            base: None,
                            agents: Vec::new(),
                            pr: prs.get(&pid).cloned(),
                        });
                        out.len() - 1
                    }
                }
            }
            None => i,
        };
        if live {
            out[i].agents.push(a.name.clone());
        }
    }
    out
}

/// The worktree a task joins (`sb spawn --place`, `sb move`): the one of
/// agent `target` (a name or an old name), or the live place whose
/// branch is `target`. Its `ws`, with the place id set.
pub fn find_worktree(st: &State, target: &str) -> Result<crate::model::Workspace, String> {
    let live = |a: &&crate::model::Agent| {
        a.ws.mode == Mode::Worktree && !a.ws.dropped && a.lifecycle != Lifecycle::Archived
    };
    let by_agent = st.resolve(target).and_then(|n| st.agents.get(&n));
    if let Some(a) = by_agent {
        if !live(&a) {
            return Err(format!(
                "@{} works in the shared folder, not a worktree: `--place new` makes one",
                a.name
            ));
        }
    }
    let a = by_agent
        .or_else(|| {
            st.order
                .iter()
                .filter_map(|n| st.agents.get(n))
                .filter(live)
                .find(|a| a.ws.branch.as_deref() == Some(target))
        })
        .ok_or_else(|| format!("no place {}: name an agent in a worktree, or its branch", target))?;
    let mut ws = a.ws.clone();
    ws.place = Some(a.ws.place_id(&a.dir));
    Ok(ws)
}

/// The place of agent `name` (its id), if it has one.
pub fn place_of(st: &State, name: &str) -> Option<String> {
    st.agents.get(name).map(|a| a.ws.place_id(&a.dir))
}

/// The place an agent shows in (the snapshot's `place_id`): its
/// feature's (`feature:<name>`, dev-flow §5.1), else [`place_of`]'s.
pub fn view_id(a: &crate::model::Agent) -> String {
    let live = a.lifecycle != Lifecycle::Archived;
    match (&a.place, a.ws.feature()) {
        // BISE-136: its private worktree first ([`id_of`])
        (Some(_), _) if live => id_of(a),
        (_, Some(f)) if live => crate::feature::place_id(f),
        _ => id_of(a),
    }
}

/// The views the TUI draws: worktrees only. `lids`: the held line of a
/// place, by id (the land queue's `waits to land · 2nd`, else pr-hub's
/// `no PR yet · 2 commits`). `stale`: the age of a PR's state, by place
/// id, when the forge is late (pr-hub: its last ask failed, or it is
/// older than `core::PR_STALE_MS`).
/// Features: `lids` holds their lid too (`feature · 14 commits · not
/// tried`, the daemon's), `trying` the ids whose try build builds or is
/// on trial.
pub fn views(
    places: &[Place],
    lids: &BTreeMap<String, String>,
    stale: &BTreeMap<String, u64>,
    trying: &std::collections::BTreeSet<String>,
) -> Vec<PlaceView> {
    places
        .iter()
        .filter(|p| p.kind != PlaceKind::Shared)
        .map(|p| PlaceView {
            id: p.id.clone(),
            branch: p.branch.clone(),
            agents: p.agents.clone(),
            pr: p.pr.as_ref().map(|pr| PrView::of(pr, stale.get(&p.id).copied())),
            lid: lids.get(&p.id).cloned(),
            feature: p.kind == PlaceKind::Feature,
            trying: trying.contains(&p.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Workspace;

    fn wt(id: Option<&str>, path: &str, branch: &str) -> Workspace {
        Workspace {
            mode: Mode::Worktree,
            path: path.into(),
            branch: Some(branch.into()),
            base_commit: Some("abc".into()),
            dropped: false,
            place: id.map(Into::into),
            feature: None,
        }
    }

    #[test]
    fn a_shared_worktree_is_one_place_with_its_agents_in_order() {
        let mut st = State::new("/w");
        for n in ["a", "b", "c", "d"] {
            st.test_task(n, "x");
        }
        st.agents.get_mut("a").unwrap().ws = wt(Some("wt:a"), "/wt/a", "sb/a");
        st.agents.get_mut("c").unwrap().ws = wt(Some("wt:a"), "/wt/a", "sb/a");
        // an older journal's worktree: no id, its agent's own place
        st.agents.get_mut("d").unwrap().ws = wt(None, "/wt/d", "sb/d");
        let ps = places(&st, &BTreeMap::new());
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["shared", "wt:a", "wt:d"]);
        assert_eq!(ps[0].kind, PlaceKind::Shared);
        assert_eq!(ps[0].agents, ["main", "b"]);
        assert_eq!(ps[1].agents, ["a", "c"]);
        assert_eq!(ps[1].branch.as_deref(), Some("sb/a"));
        assert_eq!(ps[2].agents, ["d"]);
        assert_eq!(place_of(&st, "c").as_deref(), Some("wt:a"));
        assert_eq!(place_of(&st, "b").as_deref(), Some("shared"));
        // an archived agent leaves the box; a dropped worktree is no place
        st.agents.get_mut("a").unwrap().lifecycle = Lifecycle::Archived;
        st.agents.get_mut("d").unwrap().ws.dropped = true;
        let ps = places(&st, &BTreeMap::new());
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[1].agents, ["c"]);
        // the views: worktrees only, the lid by id
        let lids = BTreeMap::from([("wt:a".to_string(), "waits to land · 2nd".to_string())]);
        let v = views(&ps, &lids, &BTreeMap::new(), &Default::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].lid.as_deref(), Some("waits to land · 2nd"));
        assert_eq!(v[0].pr, None);
        assert!(!v[0].feature);
    }

    #[test]
    fn a_features_agents_are_one_place_whatever_their_worktrees() {
        let mut st = State::new("/w");
        for n in ["a", "b", "c"] {
            st.test_task(n, "x");
        }
        // a and b: each its own worktree, both on the feature cu
        for (n, p) in [("a", "/wt/a"), ("b", "/wt/b")] {
            let mut w = wt(Some(&format!("wt:{}", n)), p, &format!("sb/{}", n));
            w.feature = Some("cu".into());
            st.agents.get_mut(n).unwrap().ws = w;
        }
        let ps = places(&st, &BTreeMap::new());
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["shared", "feature:cu"]);
        assert_eq!(ps[1].kind, PlaceKind::Feature);
        assert_eq!(ps[1].agents, ["a", "b"]);
        assert_eq!(ps[1].branch.as_deref(), Some("cu"));
        assert_eq!(ps[1].pr, None);
        assert_eq!(view_id(&st.agents["a"]), "feature:cu");
        // land and overlaps stay per worktree
        assert_eq!(place_of(&st, "a").as_deref(), Some("wt:a"));
        let trying = std::collections::BTreeSet::from(["feature:cu".to_string()]);
        let lids = BTreeMap::from([("feature:cu".to_string(), "feature · 2 commits · not tried".to_string())]);
        let v = views(&ps, &lids, &BTreeMap::new(), &trying);
        assert_eq!((v[0].feature, v[0].trying, v[0].lid.as_deref()), (true, true, Some("feature · 2 commits · not tried")));
        // all archived: no place left
        for n in ["a", "b"] {
            st.agents.get_mut(n).unwrap().lifecycle = Lifecycle::Archived;
        }
        assert_eq!(places(&st, &BTreeMap::new()).len(), 1);
        assert_eq!(view_id(&st.agents["a"]), "wt:a");
    }

    /// BISE-136, call 8: agents in a private worktree are in its place
    /// (`pt:<path>`, a worktree, its branch the one read there), shared
    /// when two are; an archived one leaves it; an agent of a hub
    /// worktree that works in a private one leaves its box, the box
    /// stays (its folder, its PR).
    #[test]
    fn a_private_worktree_is_a_place() {
        let mut st = State::new("/w");
        for n in ["a", "b", "c", "d"] {
            st.test_task(n, "x");
        }
        st.agents.get_mut("a").unwrap().place = Some("/p/a-wt".into());
        st.agents.get_mut("c").unwrap().place = Some("/p/a-wt".into());
        st.agents.get_mut("c").unwrap().place_branch = Some("feat/x".into());
        st.agents.get_mut("d").unwrap().ws = wt(Some("wt:d"), "/wt/d", "sb/d");
        st.agents.get_mut("d").unwrap().place = Some("/p/d-wt".into());
        let ps = places(&st, &BTreeMap::new());
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["shared", "pt:/p/a-wt", "wt:d", "pt:/p/d-wt"]);
        assert_eq!(ps[0].agents, ["main", "b"]);
        assert_eq!((ps[1].kind, ps[1].path.as_str()), (PlaceKind::Worktree, "/p/a-wt"));
        assert_eq!(ps[1].agents, ["a", "c"]);
        // the first agent's read names it (they read the same folder)
        assert_eq!(ps[1].branch, None);
        assert!(ps[2].agents.is_empty() && ps[3].agents == ["d"]);
        assert_eq!(id_of(&st.agents["c"]), "pt:/p/a-wt");
        assert_eq!(private_path("pt:/p/a-wt"), Some("/p/a-wt"));
        assert_eq!(private_path("wt:d"), None);
        // archived: out of it, its id is its workspace's again
        st.agents.get_mut("a").unwrap().lifecycle = Lifecycle::Archived;
        let ps = places(&st, &BTreeMap::new());
        assert_eq!(ps[1].agents, ["c"]);
        assert_eq!(ps[1].branch.as_deref(), Some("feat/x"));
        assert_eq!(id_of(&st.agents["a"]), "shared");
        // the views: a worktree like any other
        let v = views(&ps, &BTreeMap::new(), &BTreeMap::new(), &Default::default());
        assert!(v.iter().any(|p| p.id == "pt:/p/a-wt" && p.agents == ["c"]));
    }

    #[test]
    fn the_contract_json() {
        let pr = PrSnapshot {
            number: 412,
            url: "https://github.com/o/r/pull/412".into(),
            branch: "sb/a".into(),
            head_oid: "abc".into(),
            state: PrState::Draft,
            review: Review::ChangesRequested,
            checks: Checks::Fail { failing: vec!["ci/test".into()] },
            updated_at: "2026-10-01T10:00:00Z".into(),
            facts: Box::new(PrFacts {
                title: "dark mode".into(),
                approved_by: vec!["alice".into()],
                commits: 3,
                additions: 84,
                deletions: 12,
                checks: 6,
                mergeable: false,
                methods: vec![MergeMethod::Squash, MergeMethod::Rebase],
            }),
        };
        let v = PlaceView {
            id: "wt:a".into(),
            branch: Some("sb/a".into()),
            agents: vec!["a".into()],
            pr: Some(PrView::of(&pr, None)),
            lid: None,
            feature: false,
            trying: false,
        };
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"id": "wt:a", "branch": "sb/a", "agents": ["a"], "lid": null,
                "feature": false, "trying": false,
                "pr": {"number": 412, "url": "https://github.com/o/r/pull/412", "branch": pr.branch, "state": "draft",
                       "review": "changes_requested", "checks": {"state": "fail", "failing": ["ci/test"]},
                       "stale_ms": null}})
        );
        assert_eq!(serde_json::to_value(Checks::Pass).unwrap(), serde_json::json!({"state": "pass"}));
        let back: PrSnapshot = serde_json::from_value(serde_json::to_value(&pr).unwrap()).unwrap();
        assert_eq!(back, pr);
        // the facts (pr-merge): snake_case methods; a snapshot without
        // them (an older answer) reads with empty facts
        let j = serde_json::to_value(&pr).unwrap();
        assert_eq!(j["facts"]["methods"], serde_json::json!(["squash", "rebase"]));
        let mut old = j.clone();
        old.as_object_mut().unwrap().remove("facts");
        let back: PrSnapshot = serde_json::from_value(old).unwrap();
        assert_eq!(*back.facts, PrFacts::default());
    }
}
