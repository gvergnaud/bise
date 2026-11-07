//! Feature branches, the daemon's side (dev-flow §5.1): `sb feature
//! new|sync|ready|merge|drop|list`, and the user's answers on a feature's
//! items (`try`, `later`, `keep`, `merge`, `drop`; the try item's `2 show
//! the diff` is a client's act: `ready` keeps the diff as the feature's
//! artifact, `Paths::feature_diff`, and the item's option opens it). Git, the check
//! and the try build run in a thread, in line on the land queue (the
//! feature's lands wait meanwhile; a merge waits for main's line too);
//! the registry and the facts are shared with those threads; the end
//! comes back to the hub as `Input::Feature`. His answer waits its turn
//! behind a step already running (an agent's `sb feature` is refused
//! then); when it fails, the end carries `feature::handoff`'s brief for
//! main ([`handed`]), never a re-asked item. The typed `features` rows
//! (bar A.6, `proto_view::features`) read the same registry and facts,
//! never git, on the loop (`daemon/proto.rs` sends them).

use super::{log_line, write_json, Msg, Shell};
use crate::core::{FeatureDone, Input, Token};
use bise_proto::core::Feed;
use crate::feature::{self, Facts, Registry};
use crate::flow::{FlowConfig, FlowMode};
use crate::proto_view;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Inner {
    reg: Registry,
    facts: BTreeMap<String, Facts>,
    /// Try builds running, by feature.
    building: BTreeSet<String>,
    /// The steps running or waiting, by feature: an agent's `sb feature`
    /// is refused while any runs; his answer waits its turn (the
    /// feature's land turn, which every git step takes).
    busy: BTreeMap<String, usize>,
    /// The trunk's short name at the last refresh ("" before it).
    main: String,
    /// When the last refresh began (ms; 0: never).
    refreshed_ms: u64,
    /// Each feature's diff artifact (`feature::diff_ids`), by feature:
    /// the try item's `2 show the diff` opens it. Built at boot from the
    /// store, set at `ready`, built again after an agent's artifact add
    /// or archive; the snapshot only reads it (architect m_52989).
    diffs: BTreeMap<String, String>,
}

/// The features' facts and the trunk's name are read again at least this
/// often (the tick), besides after a land: someone may switch the shared
/// folder's branch or fetch by hand.
pub(super) const FACTS_EVERY_MS: u64 = 60_000;

/// The registry, the facts and the builds, shared with the threads.
#[derive(Clone, Default)]
pub(super) struct Features {
    inner: Arc<Mutex<Inner>>,
}

impl Features {
    /// The registry, and its features' diff artifacts from the store
    /// (`diff_file`: `Paths::feature_diff`).
    pub(super) fn load(state: &Path, diff_file: &dyn Fn(&str) -> PathBuf) -> Features {
        let f = Features::default();
        f.lock().reg = Registry::load(state);
        f.diffs_from(state, diff_file);
        f
    }

    /// Each feature's diff artifact, read from the store again.
    pub(super) fn diffs_from(&self, state: &Path, diff_file: &dyn Fn(&str) -> PathBuf) {
        let store = crate::artifacts::Store::new(state);
        let names = self.names();
        let diffs = feature::diff_ids(&names, &store.stored(), &store.archived(), diff_file);
        self.lock().diffs = diffs;
    }

    /// Feature `name`'s diff artifact, if any.
    pub(super) fn diff_id(&self, name: &str) -> Option<String> {
        self.lock().diffs.get(name).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The facts of every feature, against main (git; a thread's).
    pub(super) fn refresh(&self, shared: &Path) {
        let names: Vec<String> = self.lock().reg.features.iter().map(|f| f.name.clone()).collect();
        let Ok(main) = crate::trunk::trunk_ref(shared) else { return };
        let facts: BTreeMap<String, Facts> =
            names.into_iter().filter_map(|n| feature::facts(shared, &main, &n).ok().map(|f| (n, f))).collect();
        let mut g = self.lock();
        g.facts = facts;
        g.main = crate::land::short(&main).to_string();
    }

    /// The typed rows (bar A.6), from what the threads last read: no git.
    fn rows(&self, agents: &dyn Fn(&str) -> Vec<String>, cards: &[proto_view::PlaceCard]) -> Vec<bise_proto::rows::Feature> {
        let g = self.lock();
        let main = if g.main.is_empty() { "main" } else { g.main.as_str() };
        proto_view::features(&g.reg.features, &g.facts, &g.building, main, agents, cards)
    }

    /// The facts are older than [`FACTS_EVERY_MS`] (the tick refreshes
    /// them); marks them as being refreshed from `now`.
    pub(super) fn due(&self, now: u64) -> bool {
        let mut g = self.lock();
        let due = now.saturating_sub(g.refreshed_ms) >= FACTS_EVERY_MS;
        if due {
            g.refreshed_ms = now;
        }
        due
    }

    /// The trunk's short name the last refresh read ("" before it): no git.
    pub(super) fn trunk(&self) -> String {
        self.lock().main.clone()
    }

    /// The views' part (`Hub::feature_lids`, `Hub::trying`), by place id,
    /// against the trunk the last refresh read: no git (it ran per
    /// snapshot, 2-3 subprocesses each, hub-lag-7).
    pub(super) fn views(&self, now: u64) -> (BTreeMap<String, String>, BTreeSet<String>) {
        let g = self.lock();
        let main = if g.main.is_empty() { "main" } else { g.main.as_str() };
        let mut lids = BTreeMap::new();
        let mut trying = BTreeSet::new();
        for f in &g.reg.features {
            let id = feature::place_id(&f.name);
            let building = g.building.contains(&f.name);
            lids.insert(id.clone(), feature::lid(f, g.facts.get(&f.name), main, building, now));
            if building || f.trial {
                trying.insert(id);
            }
        }
        (lids, trying)
    }

    /// `/flow`'s line: the open features, with their agents' count.
    pub(super) fn flow_line(&self, agents: &dyn Fn(&str) -> usize) -> String {
        let g = self.lock();
        let list: Vec<(&feature::Feature, usize, Option<&Facts>)> =
            g.reg.features.iter().map(|f| (f, agents(&f.name), g.facts.get(&f.name))).collect();
        feature::flow_line(&list)
    }

    pub(super) fn names(&self) -> Vec<String> {
        self.lock().reg.features.iter().map(|f| f.name.clone()).collect()
    }

    fn update(&self, state: &Path, name: &str, f: impl FnOnce(&mut feature::Feature)) {
        let mut g = self.lock();
        if let Some(x) = g.reg.get_mut(name) {
            f(x);
        }
        let _ = g.reg.save(state);
    }
}

/// What one step needs, moved into its thread.
struct Job {
    op: String,
    name: String,
    agents: Vec<(String, String)>,
    shared: PathBuf,
    state: PathBuf,
    /// The feature's diff file (`Paths::feature_diff`), its artifact.
    diff: PathBuf,
    scratch: PathBuf,
    /// The land check's warm tree's root (`scratch::CheckTree`).
    checks: PathBuf,
    flow: FlowConfig,
    queue: crate::land::Queue,
    features: Features,
    tx: std::sync::mpsc::Sender<Msg>,
    /// An agent asked (`sb feature`), else the user answered an item.
    asked: bool,
}

/// A step's end: the agent's answer (ok, text) and the hub's part.
type Out = Result<(String, FeatureDone), feature::Failed>;

impl Shell {
    /// `Effect::Feature`: checks on the loop (no git), the step in a thread.
    pub(super) fn feature(&mut self, token: Option<Token>, op: String, name: String, agents: Vec<(String, String)>) {
        let mut stream = token.and_then(|t| self.replies.remove(&t));
        let flow = self.config.repo().flow.clone();
        // an agent's refusal is its answer; the user's (an item's number)
        // is handed to main with a brief, his item closed
        let tx = self.tx.clone();
        let (op_, name_, agents_) = (op.clone(), name.clone(), agents.clone());
        let picked = crate::util::now_ms();
        let refuse = |stream: &mut Option<std::os::unix::net::UnixStream>, e: String| match stream.as_mut() {
            Some(s) => {
                write_json(s, &json!({"ok": false, "error": e}));
            }
            None => {
                let failed = feature::Failed::Other(e);
                let d = handed(&op_, &name_, &agents_, picked, None, &failed);
                let _ = tx.send(Msg::input(Input::Feature(d)));
            }
        };
        if op == "list" || (op.is_empty() && name.is_empty()) {
            let hub = &self.hub;
            let line = self.features.flow_line(&|f| hub.feature_agents(f).len());
            let text = if line.is_empty() { "no feature branch. `sb feature new <name>` makes one.".to_string() } else { line };
            if let Some(s) = stream.as_mut() {
                write_json(s, &json!({"ok": true, "text": text}));
            }
            return;
        }
        if !matches!(op.as_str(), "new" | "sync" | "ready" | "merge" | "drop" | "retire" | "try" | "later" | "keep") {
            return refuse(&mut stream, "usage: sb feature new|sync|ready|merge|drop|retire|list <name>".into());
        }
        if name.is_empty() {
            return refuse(&mut stream, format!("usage: sb feature {} <name>", op));
        }
        if flow.mode == Some(FlowMode::Pr) {
            return refuse(&mut stream, "this repo ships through pull requests: a feature is a PR's branch here".into());
        }
        let known = self.features.names().contains(&name);
        if op == "new" && known {
            return refuse(&mut stream, format!("{} is a feature already: spawn its agents with --feature {}", name, name));
        }
        // a retire run again finishes what is left (its step says what)
        if op != "new" && op != "retire" && !known {
            return refuse(&mut stream, format!("no feature {}: `sb feature` lists them", name));
        }
        {
            let mut g = self.features.lock();
            if stream.is_some() && g.busy.get(&name).is_some_and(|n| *n > 0) {
                drop(g);
                return refuse(&mut stream, format!("{} is busy (a sync, a try or a merge runs): try again when it ends", name));
            }
            *g.busy.entry(name.clone()).or_default() += 1;
            if op == "try" {
                g.building.insert(name.clone());
            }
        }
        let job = Job {
            asked: stream.is_some(),
            op,
            diff: self.opts.paths.feature_diff(&name),
            name,
            agents,
            shared: self.opts.paths.workspace.clone(),
            state: self.opts.paths.state.clone(),
            scratch: self.opts.paths.worktrees.join(".features"),
            checks: self.opts.paths.worktrees.clone(),
            flow,
            queue: self.lands.clone(),
            features: self.features.clone(),
            tx: self.tx.clone(),
        };
        log_line(&self.opts.paths, &format!("feature {} {}", job.op, job.name));
        // the Δ at once when a try starts
        self.state_now();
        std::thread::spawn(move || {
            let res = step(&job);
            {
                let mut g = job.features.lock();
                if let Some(n) = g.busy.get_mut(&job.name) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        g.busy.remove(&job.name);
                    }
                }
                g.building.remove(&job.name);
            }
            job.features.refresh(&job.shared);
            let done = match res {
                Ok((text, done)) => {
                    if let Some(s) = stream.as_mut() {
                        write_json(s, &json!({"ok": true, "text": text}));
                    }
                    done
                }
                Err(e) => {
                    if let Some(s) = stream.as_mut() {
                        write_json(s, &json!({"ok": false, "error": e.to_string()}));
                    }
                    if job.asked {
                        FeatureDone { name: job.name.clone(), ..FeatureDone::default() }
                    } else {
                        // the user's answer failed: his item closes, main gets the brief
                        let mut d = handed(&job.op, &job.name, &job.agents, picked, Some(&job.shared), &e);
                        d.line = Some(Feed::warn(format!("{}: {} did not go through: {}", job.name, job.op, e)));
                        d
                    }
                }
            };
            let _ = job.tx.send(Msg::input(Input::Feature(done)));
        });
    }

    /// The try items' `2 show the diff` (after `snapshot_card_links`, its
    /// one writer of `opens`): it opens the feature's diff artifact (the
    /// one whose file is `Paths::feature_diff`, `ready` keeps it), in the
    /// app, nothing sent, the item stays. From the features' map, not
    /// saved with the card: an item from before a restart, or one whose
    /// diff was added by hand, opens it too.
    // TODO: the feature from a typed card field, not its place's text
    // (feature::of_place; architect m_52989's follow-up)
    pub(super) fn snapshot_feature_acts(&self, snap: &mut crate::snapshot::Snapshot) {
        for c in snap.cards.iter_mut().filter(|c| c.kind == feature::TRY && !c.opens.contains_key(&feature::DIFF_OPTION)) {
            let Some(name) = c.place.as_deref().and_then(feature::of_place) else { continue };
            if let Some((n, act)) = feature::try_acts(&c.kind, self.features.diff_id(name).as_deref()) {
                c.opens.insert(n, act);
            }
        }
    }

    /// The features' diff artifacts read from the store again (an
    /// agent's artifact add or archive may have changed one).
    pub(super) fn feature_diffs_refresh(&self) {
        let paths = &self.opts.paths;
        self.features.diffs_from(&paths.state, &|n| paths.feature_diff(n));
    }

    /// The typed `features` event (bar A.6): the registry's rows with each
    /// feature's live agents and its open card.
    pub(super) fn features_ev(&self) -> bise_proto::hub::HubEv {
        let hub = &self.hub;
        let cards: Vec<proto_view::PlaceCard> = hub.st.open_cards().map(|c| (c.id, c.kind.as_str(), c.place.as_deref())).collect();
        let agents = |f: &str| hub.feature_agents(f).into_iter().map(|(a, _)| a).collect();
        bise_proto::hub::HubEv::Features { project: self.project(), items: self.features.rows(&agents, &cards) }
    }

    /// The views' part, before a snapshot (`Shell::snapshot`).
    pub(super) fn feature_views(&mut self) {
        let (lids, trying) = self.features.views(crate::util::now_ms());
        self.hub.feature_lids = lids;
        self.hub.trying = trying;
    }

    /// The facts again, off the loop (after a land: main or a feature
    /// moved), then the views.
    pub(super) fn refresh_features(&self) {
        if self.features.names().is_empty() {
            return;
        }
        let (f, shared, tx) = (self.features.clone(), self.opts.paths.workspace.clone(), self.tx.clone());
        std::thread::spawn(move || {
            f.refresh(&shared);
            let _ = tx.send(Msg::Land { line: None });
        });
    }
}

/// His answer on feature `name`'s item failed (`failed`): the hub closes
/// the item and hands it to main with [`feature::handoff`]'s brief
/// (`shared`: the repo, for the tips; None: not read).
fn handed(op: &str, name: &str, agents: &[(String, String)], picked_ms: u64, shared: Option<&Path>, failed: &feature::Failed) -> FeatureDone {
    let (trunk, base, tip) = match shared {
        Some(s) => {
            let main = crate::trunk::trunk_ref(s).unwrap_or_default();
            let tip = |r: &str| crate::land::git(s, &["rev-parse", "--verify", r]).map(|t| crate::land::shorten(s, &t)).unwrap_or_default();
            (main_short(s), tip(&main), tip(&format!("refs/heads/{}", name)))
        }
        None => ("main".to_string(), String::new(), String::new()),
    };
    let agents: Vec<String> = agents.iter().map(|(a, _)| a.clone()).collect();
    let h = feature::Handoff { op, name, trunk: &trunk, base: &base, tip: &tip, picked_ms, failed, agents: &agents };
    FeatureDone { name: name.to_string(), handoff: Some(feature::handoff(&h)), ..FeatureDone::default() }
}

fn main_short(shared: &Path) -> String {
    crate::trunk::trunk_ref(shared).map(|m| crate::land::short(&m).to_string()).unwrap_or_else(|_| "main".into())
}

fn agents_line(agents: &[(String, String)]) -> String {
    agents.iter().map(|(a, _)| format!("@{}", a)).collect::<Vec<_>>().join(", ")
}

/// The worktrees follow a sync: who did not, said once.
fn followed(job: &Job, old: &str, new: &str) -> String {
    let wts: Vec<(String, PathBuf)> = job.agents.iter().map(|(a, p)| (a.clone(), PathBuf::from(p))).collect();
    let left = feature::follow(&wts, old, new);
    if left.is_empty() {
        return String::new();
    }
    let who: Vec<String> = left.iter().map(|(a, why)| format!("@{} ({})", a, why)).collect();
    format!(" not moved: {}; their next sb land rebases them.", who.join(", "))
}

fn step(job: &Job) -> Out {
    let name = job.name.as_str();
    let now = crate::util::now_ms();
    let main = main_short(&job.shared);
    let feature_ref = format!("refs/heads/{}", name);
    let place = feature::place_id(name);
    let done = || FeatureDone { name: name.to_string(), ..FeatureDone::default() };
    let facts = |f: &Features| f.lock().facts.get(name).cloned();
    match job.op.as_str() {
        "new" => {
            let feat = feature::create(&job.shared, name, now)?;
            let adopted = feat.adopted;
            {
                let mut g = job.features.lock();
                g.reg.features.push(feat);
                g.reg.save(&job.state)?;
            }
            let f = feature::facts(&job.shared, &format!("refs/heads/{}", main), name)?;
            let text = if adopted {
                format!(
                    "{} is a feature now: the local branch as it was ({} ahead of {}, {} behind), never pushed. spawn its agents with `sb spawn <name> --feature {}`.",
                    name, f.ahead, main, f.behind, name
                )
            } else {
                format!(
                    "{} is a feature: a local branch from {}'s tip ({}), never pushed. spawn its agents with `sb spawn <name> --feature {}`; they land on it, never on {}.",
                    name,
                    main,
                    crate::land::shorten(&job.shared, &f.tip),
                    name,
                    main
                )
            };
            Ok((text, done()))
        }
        "sync" => {
            let _turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            match feature::rebase(&job.shared, &job.scratch, name, job.flow.check.as_deref())? {
                None => Ok((format!("{} is on {}'s tip already.", name, main), done())),
                Some((old, new)) => {
                    job.features.update(&job.state, name, |f| f.checked = job.flow.check.as_ref().map(|_| new.clone()));
                    let left = followed(job, &old, &new);
                    Ok((
                        format!(
                            "{} is on {}'s tip now ({}){}.{}",
                            name,
                            main,
                            crate::land::shorten(&job.shared, &new),
                            if job.flow.check.is_some() { ", the check passes" } else { "" },
                            left
                        ),
                        done(),
                    ))
                }
            }
        }
        "ready" => {
            let _turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            let tip = feature::check(&job.shared, &job.scratch, name, job.flow.check.as_deref())?;
            job.features.update(&job.state, name, |f| f.checked = job.flow.check.as_ref().map(|_| tip.clone()));
            let f = feature::facts(&job.shared, &format!("refs/heads/{}", main), name)?;
            if f.ahead == 0 {
                return Err(format!("{} has no commit that {} lacks: nothing to try", name, main).into());
            }
            // what is ready, kept: the item's `2 show the diff` opens it
            // (its act, Shell::snapshot_card_links), nothing sent
            let at = crate::land::shorten(&job.shared, &f.tip);
            feature::write_diff(&job.shared, name, &job.diff)?;
            let store = crate::artifacts::Store::new(&job.state);
            let kept = store.add(&feature::diff_artifact(name, &at, &job.diff), now)?;
            job.features.lock().diffs.insert(name.to_string(), kept.meta.id);
            let mut d = done();
            d.open = Some((feature::TRY.into(), feature::try_text(name, &f, &main, job.flow.check.is_some())));
            Ok((format!("the user's inbox asks them to try {} now.", name), d))
        }
        "try" => {
            let _turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            // behind main: synced first (the user tries what would land)
            let mut synced = String::new();
            if let Some((old, new)) = feature::rebase(&job.shared, &job.scratch, name, job.flow.check.as_deref())? {
                job.features.update(&job.state, name, |f| f.checked = job.flow.check.as_ref().map(|_| new.clone()));
                synced = format!(" (synced on {} first.{})", main, followed(job, &old, &new));
            }
            let f = feature::facts(&job.shared, &format!("refs/heads/{}", main), name)?;
            let checked = job.features.lock().reg.get(name).and_then(|x| x.checked.clone()) == Some(f.tip.clone());
            let tried = match &job.flow.try_cmd {
                Some(cmd) => feature::try_build(&job.shared, &job.scratch, name, cmd, job.flow.try_run.as_deref(), now)?,
                // no try command: the user checks the branch out
                None => feature::Tried {
                    sha: crate::land::shorten(&job.shared, &f.tip),
                    run: format!("git checkout {}", name),
                    at_ms: now,
                },
            };
            job.features.update(&job.state, name, |x| {
                x.tried = Some(tried.clone());
                x.trial = true;
            });
            let mut d = done();
            d.open = Some((feature::MERGE.into(), feature::merge_text(name, &f, &main, &tried, checked)));
            d.line = Some(Feed::info(format!("{} is built to try ({}): in another terminal, {}{}", name, tried.sha, tried.run, synced)));
            Ok((format!("built {}: {}", tried.sha, tried.run), d))
        }
        "later" => {
            let mut d = done();
            d.close = Some("not yet".into());
            Ok(("the item is closed; `sb feature ready` opens it again.".into(), d))
        }
        "keep" => {
            job.features.update(&job.state, name, |f| f.trial = false);
            let mut d = done();
            d.close = Some("keep working".into());
            d.line = Some(Feed::info(format!("{} stays on its branch: its agents keep working ({}).", name, agents_line(&job.agents))));
            Ok(("the trial is over; the branch stays.".into(), d))
        }
        "merge" => {
            let _feat_turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            let main_ref = crate::trunk::trunk_ref(&job.shared)?;
            let main_turn = job.queue.wait_turn(&main_ref, &place, &mut || {});
            let m = feature::merge(&job.shared, &job.scratch, &job.checks, name, &job.flow, &main_turn)?;
            let kept = feature::trash(&job.shared, name, now).unwrap_or_default();
            {
                let mut g = job.features.lock();
                g.reg.remove(name);
                let _ = g.reg.save(&job.state);
            }
            let line = feature::merged_line(name, &main, &m, job.agents.len());
            let mut d = done();
            d.close = Some("merged".into());
            d.archive = job.agents.iter().map(|(a, _)| a.clone()).collect();
            d.line = Some(Feed::info(line.clone()));
            Ok((format!("{} (the branch's tip kept in {})", line, kept), d))
        }
        "drop" => {
            let _turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            let commits = facts(&job.features).map(|f| f.ahead).unwrap_or(0);
            let kept = feature::trash(&job.shared, name, now)?;
            {
                let mut g = job.features.lock();
                g.reg.remove(name);
                let _ = g.reg.save(&job.state);
            }
            let line = format!(
                "{} dropped: {} commit{} off the branch, its tip kept in {}{}",
                name,
                commits,
                if commits == 1 { "" } else { "s" },
                kept,
                if job.agents.is_empty() { String::new() } else { format!(" · {} archived", agents_line(&job.agents)) }
            );
            let mut d = done();
            d.close = Some("dropped".into());
            d.archive = job.agents.iter().map(|(a, _)| a.clone()).collect();
            d.line = Some(Feed::info(line.clone()));
            Ok((line, d))
        }
        "retire" => retire(job, &feature_ref, &place, &main),
        op => Err(format!("unknown feature step: {}", op).into()),
    }
}

/// `sb feature retire <name>` (main's): the feature's work is in the
/// trunk already; its agents stay and land on the trunk from now on.
/// Unlike merge and drop (git first, then the state), the state goes
/// first: the check proved the tip is in the trunk, so agents landing
/// there is right whatever the branch is called; a crash after sb-core's
/// step leaves a branch still named X and a registry entry with no
/// agents, both harmless, where git first would leave agents pointing at
/// a branch that's gone. So: the check, sb-core's step (who moves, their
/// note, main's line), then the rename (never a delete), then the
/// registry, all under the feature's land turn: a land queued behind
/// reads its agent's new place once in line (`land::run`'s `onto_now`)
/// and goes to the trunk. Run again, it finishes whatever is left.
fn retire(job: &Job, feature_ref: &str, place: &str, main: &str) -> Out {
    let name = job.name.as_str();
    let _turn = job.queue.wait_turn(feature_ref, place, &mut || {});
    let trunk = crate::trunk::trunk_ref(&job.shared)?;
    let branch = feature::exists(&job.shared, name);
    if branch {
        let missing = feature::missing(&job.shared, &trunk, name)?;
        if missing > 0 {
            return Err(feature::retire_refusal(name, main, missing).into());
        }
    }
    let known = job.features.names().iter().any(|n| n == name);
    let kept = if branch {
        feature::retired_name(name, &|b| feature::exists(&job.shared, b))
    } else {
        feature::last_retired(&job.shared, name).unwrap_or_default()
    };
    if !branch && !known && job.agents.is_empty() {
        return match kept.is_empty() {
            true => Err(format!("no feature {}: `sb feature` lists them", name).into()),
            false => {
                let d = FeatureDone { name: name.to_string(), ..FeatureDone::default() };
                Ok((format!("{} is retired already (its branch is {}): nothing left to do.", name, kept), d))
            }
        };
    }
    let mut did = Vec::new();
    if branch || !job.agents.is_empty() {
        // sb-core's step, applied before the branch moves
        let (tx, rx) = std::sync::mpsc::channel();
        let step = FeatureDone {
            name: name.to_string(),
            retire: Some((kept.clone(), main.to_string())),
            ack: Some(crate::core::feature::Ack(tx)),
            ..FeatureDone::default()
        };
        if job.tx.send(Msg::input(Input::Feature(step))).is_err() || rx.recv().is_err() {
            return Err(feature::Failed::Other("the hub stopped before the retire: nothing changed; run it again".into()));
        }
        did.push(match job.agents.len() {
            0 => "no live agent to move".to_string(),
            _ => format!("{} land on {} now", agents_line(&job.agents), main),
        });
    }
    if branch {
        feature::rename(&job.shared, name, &kept)?;
        did.push(format!("the branch renamed {}", kept));
    }
    if known {
        let mut g = job.features.lock();
        g.reg.remove(name);
        g.reg.save(&job.state)?;
        did.push("off the feature list".to_string());
    }
    let mut d = FeatureDone { name: name.to_string(), ..FeatureDone::default() };
    d.close = Some("retired".into());
    Ok((format!("{} retired: {}.", name, did.join(", ")), d))
}

/// A land's `onto_now` (`land::run`): asks the hub's loop where `agent`
/// lands now; `was` (the request's) when the hub does not answer.
pub(super) fn onto_now(tx: &std::sync::mpsc::Sender<Msg>, agent: &str, was: Option<String>) -> impl FnMut() -> Option<String> {
    let (tx, agent) = (tx.clone(), agent.to_string());
    move || {
        let (reply, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Msg::input(Input::Onto { agent: agent.clone(), reply }));
        rx.recv().unwrap_or_else(|_| was.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// architect m_52989: after a hub restart, the try item's `2 show the
    /// diff` still opens its diff: the map is built at boot from the store
    /// (the artifact whose source is the feature's diff file, not
    /// archived); a feature with no diff kept gets none.
    #[test]
    fn a_try_item_opens_its_diff_after_a_restart() {
        let state = std::env::temp_dir().join(format!("sb-feat-diffs-{}-{}", std::process::id(), crate::util::now_ms()));
        let file = |n: &str| state.join("features").join(format!("{n}.diff"));
        let reg = Registry {
            features: ["cu", "bims"].iter().map(|n| feature::Feature { name: (*n).into(), ..feature::Feature::default() }).collect(),
        };
        reg.save(&state).unwrap();
        std::fs::create_dir_all(state.join("features")).unwrap();
        std::fs::write(file("cu"), "diff --git a/x b/x\n").unwrap();
        let store = crate::artifacts::Store::new(&state);
        store.add(&feature::diff_artifact("cu", "abc1234", &file("cu")), 1).unwrap();
        // the restart: the registry and the map, read again
        let f = Features::load(&state, &file);
        assert_eq!(f.diff_id("cu").as_deref(), Some("cu-diff"));
        assert_eq!(f.diff_id("bims"), None, "no diff kept: its item as it is");
        assert_eq!(feature::try_acts(feature::TRY, f.diff_id("cu").as_deref()), Some((2, bise_proto::rows::OptAct::artifact("cu-diff", None, None))));
        // archived: it opens no more
        store.archive(&crate::artifacts::Pick { ids: vec!["cu-diff".into()], ..Default::default() }, 2).unwrap();
        f.diffs_from(&state, &file);
        assert_eq!(f.diff_id("cu"), None);
        let _ = std::fs::remove_dir_all(&state);
    }
}
