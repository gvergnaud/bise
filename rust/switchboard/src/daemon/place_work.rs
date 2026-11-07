//! The worktrees sb-core asks for, made and removed off the hub's loop
//! (hub-loop, docs/issues/32). Before, sb-core's `worktree_create` and
//! `worktree_drop` were git queries the loop answered inside the step:
//! `git worktree add` (a checkout of the whole tree, 2-10 s on his repo)
//! and `git worktree remove` held every agent's `sb` meanwhile. Now
//! sb-core decides and commits first (bend/hub/place.bend), and its fx
//! (`Effect::Place`) comes here: one worker thread runs the jobs in order
//! (one git at a time on the repo, as before), with its own `GitEnv`; a
//! made worktree goes back to the loop as `Msg::Placed`, which steps
//! sb-core's `placed`; a removed one (its work saved first) as
//! `Msg::Unplaced`, which steps sb-core's `unplaced` (issue 32 3b).
//!
//! Owns: the worker and its queue. Not here: what to make or remove and
//! what follows (sb-core), how (worktree.rs `GitEnv`).

use super::*;
use crate::core::{Env, Loss};
use crate::model::Workspace;
use crate::place_job::Job;

/// The loop's side: the worker's door.
pub(super) struct PlaceWork {
    to_worker: Sender<Job>,
}

impl PlaceWork {
    /// The worker, sending what it made on `to_loop`.
    /// `git`: the hub's is_git probe of the same workspace, shared.
    pub(super) fn start(paths: &Paths, to_loop: Sender<Msg>, git: crate::git_probe::GitProbe) -> PlaceWork {
        let (to_worker, rx) = channel();
        let paths = paths.clone();
        std::thread::spawn(move || worker(rx, paths, to_loop, git));
        PlaceWork { to_worker }
    }

    /// One job, after the ones already asked.
    pub(super) fn run(&self, j: Job) {
        let _ = self.to_worker.send(j);
    }
}

/// The hub's git, for one job (its config read now: a change of
/// `.switchboard/config.toml` counts from the next worktree).
fn git_env(paths: &Paths, git: &crate::git_probe::GitProbe) -> GitEnv {
    let log_paths = paths.clone();
    // this worker is off the loop already: its rm runs here
    GitEnv::with_git(paths, Box::new(move |s| log_line(&log_paths, s)), crate::trash::Trash::now(), git.clone())
}

fn worker(rx: Receiver<Job>, paths: Paths, to_loop: Sender<Msg>, git: crate::git_probe::GitProbe) {
    for j in rx {
        let mut env = git_env(&paths, &git);
        match j {
            Job::Create { name, with_changes } => {
                let t0 = now_ms();
                let (ws, err) = match env.worktree_create(&name, with_changes) {
                    Ok(ws) => (serde_json::to_value(ws).unwrap_or(Value::Null), String::new()),
                    Err(e) => (Value::Null, e),
                };
                log_line(&paths, &format!("worktree of {} made off the loop in {} ms{}", name, now_ms().saturating_sub(t0), if err.is_empty() { String::new() } else { format!(": {err}") }));
                if to_loop.send(Msg::Placed { name, ws, err }).is_err() {
                    return;
                }
            }
            Job::Drop { name, ws, dirty, unpushed } => {
                let t0 = now_ms();
                let (snapshot, err) = match serde_json::from_value::<Workspace>(ws) {
                    Ok(ws) => match env.worktree_drop(&name, &ws, &Loss { dirty: dirty as usize, unpushed: unpushed as usize }) {
                        Ok(snap) => (snap, String::new()),
                        Err(e) => (None, e),
                    },
                    Err(e) => (None, format!("its workspace does not read: {e}")),
                };
                log_line(&paths, &format!("worktree of {} removed off the loop in {} ms{}", name, now_ms().saturating_sub(t0), if err.is_empty() { String::new() } else { format!(": NOT removed: {err}") }));
                // sb-core ends the drop (a worktree it made for a task gone
                // while placing: no task dropping, nothing follows)
                if to_loop.send(Msg::Unplaced { name, snapshot, err }).is_err() {
                    return;
                }
            }
        }
    }
}
