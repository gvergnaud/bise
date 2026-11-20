//! The typed `worktrees` event on the hub's side (bise desktop S7 emitter
//! 3): the repo's worktrees, each with the agent working there, whether
//! it has changes not committed (untracked files included, architect
//! m_8711) and its commits ahead of and behind the trunk. Git runs in a
//! thread (never on the hub's loop); the rows are pure
//! (`proto_view::worktrees`). Asked at a typed hello, on `worktrees`, and
//! when an agent comes, goes, starts or ends a turn (`proto_on`): never on
//! a timer.
//!
//! Paced (git-calm, architect m_67753, crate::git_poll): one scan in
//! flight per hub, the asks meanwhile merged into one rerun after the
//! gap ([`Polls`], the "worktrees" Tick job starts it); a worktree
//! another poll's git runs in (an agent's Changes) keeps its last facts;
//! agents' own temp worktrees (`hub_state::is_agent_tmp`) are dropped
//! before any git.

use super::rpc::Typed;
use super::*;
use crate::git_poll::{Facts, Scans, Slots};
use crate::model::Mode;
use crate::proto_view;
use bise_proto::hub::HubEv;
use bise_proto::rows::Worktree;
use std::collections::HashMap;

/// The hub's git polls' state, touched on the loop only (the slots are
/// shared with the polls' threads).
#[derive(Default)]
pub(super) struct Polls {
    scans: Scans<Typed>,
    /// the worktrees a poll's git runs in now (the scan, Changes)
    pub(super) slots: Slots,
    /// the machine was loaded at the last pace read (its change goes
    /// to hub.log once)
    loaded: bool,
}

impl Shell {
    /// What makes the worktrees change: each agent's status and folder.
    pub(super) fn worktrees_key(&self) -> String {
        let st = &self.hub.st;
        st.agents.iter().map(|(n, a)| format!("{n}:{:?}:{}:{}", a.status(), a.ws.path, a.ws.dropped)).collect::<Vec<_>>().join("|")
    }

    /// `worktrees` goes `to` (an answer, or a hub-wide notification): a
    /// scan now, or after the scan in flight and the gap.
    pub(super) fn worktrees_typed(&mut self, to: Typed) {
        if self.nobody(&to) {
            return;
        }
        let gap = crate::git_poll::scan_gap(self.polls_pace());
        if let Some(to) = self.polls.scans.ask(to, crate::util::now_ms(), gap) {
            self.worktrees_scan(to);
        }
    }

    /// The Tick's job: the waiting asks' scan once the gap passed.
    pub(super) fn worktrees_tick(&mut self) {
        if !self.polls.scans.waiting() {
            return;
        }
        let gap = crate::git_poll::scan_gap(self.polls_pace());
        if let Some(to) = self.polls.scans.due(crate::util::now_ms(), gap) {
            self.worktrees_scan(to);
        }
    }

    /// The machine's load factor for the polls' gaps; a change between
    /// loaded and not goes to hub.log, with the gaps it gives.
    pub(super) fn polls_pace(&mut self) -> f64 {
        let factor = crate::load::factor();
        let loaded = crate::load::is_loaded(factor);
        if loaded != self.polls.loaded {
            self.polls.loaded = loaded;
            let (scan, changes) = (crate::git_poll::scan_gap(factor), crate::git_poll::changes_gap(factor));
            let how = if loaded { "loaded" } else { "not loaded" };
            log_line(
                &self.opts.paths,
                &format!("git polls: the machine is {how} (factor {factor:.1}): a worktrees scan at most every {} s, an agent's Changes every {} s after its edits", scan.as_secs(), changes.as_secs()),
            );
        }
        factor
    }

    /// Scan the worktrees in a thread for `to`; its end comes back as
    /// `Msg::Worktrees` ([`Shell::on_worktrees`]).
    fn worktrees_scan(&mut self, to: Vec<Typed>) {
        let shared = PathBuf::from(&self.hub.workspace);
        let agents: Vec<(String, String)> = self
            .hub
            .st
            .agents
            .iter()
            .filter(|(_, a)| a.ws.mode == Mode::Worktree && !a.ws.dropped && a.status() != crate::model::Status::Archived)
            .map(|(n, a)| (n.clone(), a.ws.path.clone()))
            .collect();
        let project = self.project();
        let (tx, how) = (self.tx.clone(), self.base_how());
        let (home, slots, last) = (self.opts.home.clone(), self.polls.slots.clone(), self.polls.scans.last().clone());
        std::thread::spawn(move || {
            let base = how.base(&shared);
            let git = |dir: &Path, args: &[&str]| crate::worktree::git(dir, args).ok();
            let tmp = |p: &Path| bise_home::hub_state::is_agent_tmp(&home, p);
            let ports = Ports { git: &git, tmp: &tmp, slots: &slots, last: &last };
            let (items, facts) = scan(&shared, &base, &agents, &ports);
            let v = HubEv::Worktrees { project, items }.to_value();
            let _ = tx.send(Msg::Worktrees { to, v, facts });
        });
    }

    /// A scan ended: its event to each target, its facts kept.
    pub(super) fn on_worktrees(&mut self, to: Vec<Typed>, v: Value, facts: HashMap<String, Facts>) {
        self.polls.scans.done(crate::util::now_ms(), facts);
        for t in to {
            if !self.nobody(&t) {
                self.typed_msg(t, v.clone());
            }
        }
        self.changes_retry();
    }
}

/// What a scan reads through: git in a folder (its stdout, None:
/// failed), an agent's temp folder, the polls' slots, the last facts.
pub(super) struct Ports<'a> {
    pub git: &'a dyn Fn(&Path, &[&str]) -> Option<String>,
    pub tmp: &'a dyn Fn(&Path) -> bool,
    pub slots: &'a Slots,
    pub last: &'a HashMap<String, Facts>,
}

/// The repo's worktrees at `shared` (none: not a git repo), ahead and
/// behind `base` (where they start: `BaseHow::base`), agents' temp
/// worktrees left out; with each listed worktree's facts.
pub(super) fn scan(shared: &Path, base: &str, agents: &[(String, String)], p: &Ports) -> (Vec<Worktree>, HashMap<String, Facts>) {
    let Some(list) = (p.git)(shared, &["worktree", "list", "--porcelain"]) else { return (Vec::new(), HashMap::new()) };
    let mut listed = proto_view::worktree_list(&list);
    listed.retain(|l| !(p.tmp)(Path::new(&l.path)));
    let seen = std::cell::RefCell::new(HashMap::new());
    let rows = proto_view::worktrees(&listed, agents, |path| {
        let f = facts(Path::new(path), base, p);
        seen.borrow_mut().insert(path.to_string(), f);
        f
    });
    (rows, seen.into_inner())
}

/// (dirty, ahead, behind) of the checkout at `dir` (a folder gone: clean
/// and even); another poll's git running there: its last facts.
fn facts(dir: &Path, trunk: &str, p: &Ports) -> Facts {
    let Some(_held) = p.slots.claim(dir) else {
        return p.last.get(&*dir.to_string_lossy()).copied().unwrap_or_default();
    };
    let git = |args: &[&str]| (p.git)(dir, args);
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.trim().is_empty());
    let (behind, ahead) = git(&["rev-list", "--left-right", "--count", &format!("{trunk}...HEAD")]).and_then(|s| proto_view::left_right(&s)).unwrap_or((0, 0));
    (dirty, ahead, behind)
}

#[cfg(test)]
#[path = "worktrees_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "worktrees_bench_tests.rs"]
mod bench;
