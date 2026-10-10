//! One call to the small-jobs model, off the hub's loop (architect
//! m_14532): the role lines (BISE-126) and the timers' names
//! (every_name.rs) both go through [`Small::call`]. The small model
//! first; when it fails, the agent model, and once that one answers the
//! small model is marked broken for the hub's life (the shared flag); the
//! log says each failure. A setup with neither model fails at once.
//! The caller runs it in its own thread and sends the answer back as a
//! `Msg`.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What a call needs, taken from the shell before its thread starts.
pub(super) struct Small {
    setup: bise_catalog::Setup,
    broken: Arc<AtomicBool>,
    repl: PathBuf,
    root: PathBuf,
    paths: Paths,
    spawn_env: Option<SpawnEnv>,
}

impl Small {
    pub(super) fn of(sh: &Shell) -> Small {
        Small {
            setup: bise_catalog::Setup::load(&bise_home::Home::from_env().config_file()),
            broken: sh.small_broken.clone(),
            repl: sh.opts.repl_bin.clone(),
            root: sh.opts.app_root.clone(),
            paths: sh.opts.paths.clone(),
            spawn_env: sh.opts.spawn_env,
        }
    }

    pub(super) fn paths(&self) -> &Paths {
        &self.paths
    }

    /// The reply to the request in `req_file` (removed after), or why
    /// not. `what` names the call in the log (`role line of docs`).
    pub(super) fn call(&self, req_file: &Path, what: &str) -> Result<String, String> {
        let got = self.call_models(req_file, what);
        let _ = std::fs::remove_file(req_file);
        got
    }

    fn call_models(&self, req_file: &Path, what: &str) -> Result<String, String> {
        let (small, agent) = (&self.setup.small_model, &self.setup.agent_model);
        if small.is_empty() && agent.is_empty() {
            return Err("no model set up".into());
        }
        let keys = self.spawn_env.map(|f| f()).unwrap_or_default();
        let first = if self.broken.load(Ordering::Relaxed) || small.is_empty() { agent } else { small };
        let got = oneshot(&self.repl, &self.root, req_file, first, &keys);
        let Err(e) = &got else { return got };
        log_line(&self.paths, &format!("{}: {} failed: {}", what, first, e));
        if first == agent || agent.is_empty() {
            return got;
        }
        let got = oneshot(&self.repl, &self.root, req_file, agent, &keys);
        match &got {
            Ok(_) => {
                self.broken.store(true, Ordering::Relaxed);
                log_line(&self.paths, &format!("small jobs: {} failed, {} from now on", small, agent));
            }
            Err(e) => log_line(&self.paths, &format!("{}: {} failed: {}", what, agent, e)),
        }
        got
    }
}
