//! One call to the small-jobs model, off the hub's loop (architect
//! m_14532): the role lines (BISE-126) and the timers' names
//! (every_name.rs) both go through [`Small::call`]. The small model
//! first; when it fails, the agent model, and once that one answers the
//! small model is marked broken for the hub's life (the shared flag); the
//! log says each failure. A setup with neither model fails at once.
//! The caller runs it in its own thread and sends the answer back as a
//! `Msg`. A call nobody clicked (a guide written in the background,
//! daemon/guide.rs) takes [`Small::cheap`]: the small model alone, never
//! the main or agents model, never a fallback.

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
    pub(super) fn of(sh: &mut Shell) -> Small {
        Small {
            setup: sh.config.setup().clone(),
            broken: sh.small_broken.clone(),
            repl: sh.opts.repl_bin.clone(),
            root: sh.opts.app_root.clone(),
            paths: sh.opts.paths.clone(),
            spawn_env: sh.opts.spawn_env,
        }
    }

    /// The small model alone, for a call nobody clicked (a guide written
    /// in the background, architect m_57259): None when it is the main or
    /// the agents model (unset, it follows them), or marked broken. Its
    /// call never falls back to the agents model.
    pub(super) fn cheap(sh: &mut Shell) -> Option<Small> {
        let s = Small::of(sh);
        let st = &s.setup;
        let big = st.small_model.is_empty() || st.small_model == st.model || st.small_model == st.agent_model;
        (!big && !s.broken.load(Ordering::Relaxed)).then(|| Small { setup: bise_catalog::Setup { agent_model: String::new(), ..s.setup.clone() }, ..s })
    }

    /// Some model is set up (the small one or the agents').
    pub(super) fn any(sh: &mut Shell) -> bool {
        let st = sh.config.setup();
        !st.small_model.is_empty() || !st.agent_model.is_empty()
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

/// The longest a role-line call may take.
const ONESHOT_TIMEOUT: Duration = Duration::from_secs(60);

/// One provider call through `repl-live`'s one-shot mode (BISE_ONESHOT,
/// runtime/oneshot.bend) with `model`: the reply's text, or why not.
/// `keys`: the REPLs' spawn env (the keys, BISE_MODELS_FILE). The
/// provider's error text never holds a key.
fn oneshot(repl: &Path, root: &Path, req_file: &Path, model: &str, keys: &[(String, Option<String>)]) -> Result<String, String> {
    let mut cmd = Command::new(repl);
    crate::approvals::check::oneshot_env(req_file, model, keys).apply(&mut cmd);
    cmd.current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|e| format!("cannot start {}: {}", repl.display(), e))?;
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < ONESHOT_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("no answer in {} s", ONESHOT_TIMEOUT.as_secs()));
            }
        }
    }
    crate::role::oneshot_reply(&reader.join().unwrap_or_default())
}
