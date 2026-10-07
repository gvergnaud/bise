//! Feature branches, the daemon's side (dev-flow §5.1): `sb feature
//! new|sync|ready|merge|drop|list`, and the user's answers on a feature's
//! items (`try`, `diff`, `later`, `keep`, `merge`, `drop`). Git, the check
//! and the try build run in a thread, in line on the land queue (the
//! feature's lands wait meanwhile; a merge waits for main's line too);
//! the registry and the facts are shared with those threads; the end
//! comes back to the hub as `Input::Feature`. The typed `features` rows
//! (bar A.6, `proto_view::features`) read the same registry and facts,
//! never git, on the loop (`daemon/proto.rs` sends them).

use super::{log_line, write_json, Msg, Shell};
use crate::core::{FeatureDone, Input, Token};
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
    /// A step running, by feature: one at a time per feature.
    busy: BTreeSet<String>,
    /// The trunk's short name at the last refresh ("" before it).
    main: String,
}

/// The registry, the facts and the builds, shared with the threads.
#[derive(Clone, Default)]
pub(super) struct Features {
    inner: Arc<Mutex<Inner>>,
}

impl Features {
    pub(super) fn load(state: &Path) -> Features {
        let f = Features::default();
        f.lock().reg = Registry::load(state);
        f
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

    /// The views' part (`Hub::feature_lids`, `Hub::trying`), by place id.
    pub(super) fn views(&self, main: &str, now: u64) -> (BTreeMap<String, String>, BTreeSet<String>) {
        let g = self.lock();
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
    scratch: PathBuf,
    flow: FlowConfig,
    queue: crate::land::Queue,
    features: Features,
    tx: std::sync::mpsc::Sender<Msg>,
    /// An agent asked (`sb feature`), else the user answered an item.
    asked: bool,
}

/// A step's end: the agent's answer (ok, text) and the hub's part.
type Out = Result<(String, FeatureDone), String>;

impl Shell {
    /// `Effect::Feature`: checks on the loop (no git), the step in a thread.
    pub(super) fn feature(&mut self, token: Option<Token>, op: String, name: String, agents: Vec<(String, String)>) {
        let mut stream = token.and_then(|t| self.replies.remove(&t));
        let flow = FlowConfig::load(&self.opts.paths);
        let refuse = |stream: &mut Option<std::os::unix::net::UnixStream>, e: String| {
            if let Some(s) = stream.as_mut() {
                write_json(s, &json!({"ok": false, "error": e}));
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
        if !matches!(op.as_str(), "new" | "sync" | "ready" | "merge" | "drop" | "try" | "diff" | "later" | "keep") {
            return refuse(&mut stream, "usage: sb feature new|sync|ready|merge|drop|list <name>".into());
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
        if op != "new" && !known {
            return refuse(&mut stream, format!("no feature {}: `sb feature` lists them", name));
        }
        {
            let mut g = self.features.lock();
            if !g.busy.insert(name.clone()) {
                drop(g);
                return refuse(&mut stream, format!("{} is busy (a sync, a try or a merge runs): try again when it ends", name));
            }
            if op == "try" {
                g.building.insert(name.clone());
            }
        }
        let job = Job {
            asked: stream.is_some(),
            op,
            name,
            agents,
            shared: self.opts.paths.workspace.clone(),
            state: self.opts.paths.state.clone(),
            scratch: self.opts.paths.worktrees.join(".features"),
            flow,
            queue: self.lands.clone(),
            features: self.features.clone(),
            tx: self.tx.clone(),
        };
        log_line(&self.opts.paths, &format!("feature {} {}", job.op, job.name));
        // the Δ at once when a try starts
        let snap = self.snapshot();
        self.broadcast(&snap);
        std::thread::spawn(move || {
            let res = step(&job);
            {
                let mut g = job.features.lock();
                g.busy.remove(&job.name);
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
                        write_json(s, &json!({"ok": false, "error": e}));
                    }
                    // the user's answer failed: main hears it, the item waits
                    let line = (!job.asked).then(|| ("warn".to_string(), format!("{}: {} did not go through: {}", job.name, job.op, e)));
                    FeatureDone { name: job.name.clone(), line, ..FeatureDone::default() }
                }
            };
            let _ = job.tx.send(Msg::In(Input::Feature(done)));
        });
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
        let main = crate::trunk::trunk_ref(&self.opts.paths.workspace).unwrap_or_else(|_| "main".into());
        let (lids, trying) = self.features.views(crate::land::short(&main), crate::util::now_ms());
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
                return Err(format!("{} has no commit that {} lacks: nothing to try", name, main));
            }
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
            d.line = Some(("info".into(), format!("{} is built to try ({}): in another terminal, {}{}", name, tried.sha, tried.run, synced)));
            Ok((format!("built {}: {}", tried.sha, tried.run), d))
        }
        "diff" => {
            let file = job.state.join("features").join(format!("{}.diff", name));
            feature::write_diff(&job.shared, name, &file)?;
            let f = facts(&job.features).unwrap_or_default();
            let mut d = done();
            d.line = Some((
                "info".into(),
                format!(
                    "the diff of {}: {} commits, +{} −{} · {}",
                    name,
                    f.ahead,
                    feature::thousands(f.adds),
                    feature::thousands(f.dels),
                    feature::home_short(&file.to_string_lossy())
                ),
            ));
            Ok((format!("written to {}", file.display()), d))
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
            d.line = Some(("info".into(), format!("{} stays on its branch: its agents keep working ({}).", name, agents_line(&job.agents))));
            Ok(("the trial is over; the branch stays.".into(), d))
        }
        "merge" => {
            let _feat_turn = job.queue.wait_turn(&feature_ref, &place, &mut || {});
            let main_ref = crate::trunk::trunk_ref(&job.shared)?;
            let _main_turn = job.queue.wait_turn(&main_ref, &place, &mut || {});
            let m = feature::merge(&job.shared, &job.scratch, name, &job.flow)?;
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
            d.line = Some(("info".into(), line.clone()));
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
            d.line = Some(("info".into(), line.clone()));
            Ok((line, d))
        }
        op => Err(format!("unknown feature step: {}", op)),
    }
}
