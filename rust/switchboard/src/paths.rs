//! Where a workspace's hub keeps its state (RFC 0001 §5):
//! `$SB_STATE_DIR`, else `<hubs dir>/<name>-<hash>` (`bise_home::Home::hub_dir`:
//! `~/.bise/hubs/`, or `~/.local/state/switchboard/` in the old layout).
//! Its task worktrees: `<home>/worktrees/<name>-<hash>/<task>/` (the
//! `sweep` module), or `$SB_STATE_DIR/worktrees/<task>/`.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub workspace: PathBuf,
    pub state: PathBuf,
    /// This workspace's task worktrees, one folder per task:
    /// `<home>/worktrees/<id>` (`bise_home::Home::worktrees_dir`), or
    /// `<state>/worktrees` when `SB_STATE_DIR` is set (tests: nothing
    /// outside the state dir).
    pub worktrees: PathBuf,
}

/// The hub id of a workspace: `bise_home::hub_id` (one owner).
pub fn workspace_id(workspace: &Path) -> String {
    bise_home::hub_id(workspace)
}

/// The home workspace (docs/ambient-pages.md §5.1): `~/bise`, a plain
/// folder without git for non-code work; `$BISE_HOME_WORKSPACE` when set
/// (the tests: a throwaway folder, never the user's).
pub fn home_workspace() -> PathBuf {
    if let Some(d) = bise_home::env::test_setting("BISE_HOME_WORKSPACE") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join("bise")
}

/// `ws` is the home workspace: its hub's main is bise (bise desktop S2,
/// architect m_8474: the one flag, nothing written, nothing in sb-core).
pub fn is_home(ws: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(ws) == canon(&home_workspace())
}

/// Whether `ws` is in bise's projects registry (bise_home::projects, which
/// the desktop app writes when it shows a project): one of the two facts
/// of prompts::desktop_on, read once when a prompt is built.
pub fn is_registered(ws: &Path) -> bool {
    let ws = bise_home::projects::canonical(ws);
    bise_home::projects::read(&bise_home::Home::from_env()).iter().any(|p| bise_home::projects::canonical(&p.path) == ws)
}

/// The home workspace, created on first use (never git-initialised).
pub fn ensure_home_workspace() -> std::io::Result<PathBuf> {
    let d = home_workspace();
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

impl Paths {
    pub fn for_workspace(workspace: &Path) -> Paths {
        let workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());
        let (state, worktrees) = match bise_home::env::test_setting("SB_STATE_DIR") {
            Some(d) => (PathBuf::from(&d), PathBuf::from(d).join("worktrees")),
            None => {
                let home = bise_home::Home::from_env();
                let id = workspace_id(&workspace);
                (home.hub_dir(&id), home.worktrees_dir().join(id))
            }
        };
        Paths { workspace, state, worktrees }
    }

    /// The hub's socket as clients and the hub reach it: `<state>/hub.sock`
    /// when that fits in a unix socket address, else its short path
    /// through `/tmp/bise-<uid>/<hash>/` (`bise_home::socket`; the hub
    /// makes the link before it binds, [`Paths::prepare_socket`]).
    pub fn socket(&self) -> PathBuf {
        bise_home::socket::socket_path(&self.natural_socket())
    }
    /// Where the socket file lives: `<state>/hub.sock`.
    pub fn natural_socket(&self) -> PathBuf {
        self.state.join("hub.sock")
    }
    /// The agents' socket (their `SB_SOCKET`, `crate::peer`):
    /// `<state>/agent.sock`, or its short path in the same short folder
    /// as [`Paths::socket`].
    pub fn agent_socket(&self) -> PathBuf {
        bise_home::socket::socket_path(&self.natural_agent_socket())
    }
    /// Where the agents' socket file lives: `<state>/agent.sock`.
    pub fn natural_agent_socket(&self) -> PathBuf {
        self.state.join("agent.sock")
    }
    /// Make [`Paths::socket`] and [`Paths::agent_socket`] bindable (the
    /// short link, when needed: one link, the folder's).
    pub fn prepare_socket(&self) -> std::io::Result<PathBuf> {
        bise_home::socket::prepare_socket(&self.natural_agent_socket())?;
        bise_home::socket::prepare_socket(&self.natural_socket())
    }
    pub fn journal(&self) -> PathBuf {
        self.state.join("journal.jsonl")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.state.join("hub.pid")
    }
    pub fn log(&self) -> PathBuf {
        self.state.join("hub.log")
    }
    pub fn bin_dir(&self) -> PathBuf {
        self.state.join("bin")
    }
    pub fn agent_dir(&self, name: &str) -> PathBuf {
        self.state.join("agents").join(name)
    }
    /// The agent's temp folder (approvals-design.md §7.1): its `TMPDIR`,
    /// created at spawn, deleted at its /drop.
    pub fn agent_tmp(&self, name: &str) -> PathBuf {
        self.agent_dir(name).join("tmp")
    }
    /// The harness's own files of the agent (bash wrappers, steer,
    /// interrupt, run_typescript files): `BEND_AGENT_RUN`, never deleted
    /// while the agent lives.
    pub fn agent_run(&self, name: &str) -> PathBuf {
        self.agent_dir(name).join("run")
    }
    /// Where the hub put task worktrees before BISE-230 (`<state>/worktrees/<task>`):
    /// moved to [`Paths::worktrees`] at the hub's start (`sweep::migrate`).
    pub fn legacy_worktrees(&self) -> PathBuf {
        self.state.join("worktrees")
    }
    pub fn config(&self) -> PathBuf {
        self.workspace.join(".switchboard").join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Python copies of `workspace_id` (tests/gate.sh `new`/`done`,
    /// tests/worktree_home.py, tests/proc_cleanup.py) give these values:
    /// a change here must change them too, or `gate.sh new` puts a task's
    /// worktree where the hub never looks (BISE-292).
    #[test]
    fn ids_match_the_python_copies() {
        assert_eq!(workspace_id(Path::new("/Users/me/lab/harness")), "harness-af1b2326");
        assert_eq!(workspace_id(Path::new("/tmp/my repo")), "my-repo-b50e38fe");
        assert_eq!(workspace_id(Path::new("/")), "root-860189fe");
    }

    /// The one flag of bise's role (architect m_8474): the home workspace,
    /// by its canonical path (a trailing slash or a symlink to it counts),
    /// never another folder.
    #[test]
    fn only_the_home_workspace_is_home() {
        let home = home_workspace();
        assert!(is_home(&home));
        assert!(is_home(&home.join(".")));
        assert!(!is_home(&home.join("projects")) && !is_home(Path::new("/")));
    }

    #[test]
    fn ids_are_stable_and_readable() {
        let a = workspace_id(Path::new("/Users/me/my repo"));
        assert_eq!(a, workspace_id(Path::new("/Users/me/my repo")));
        assert!(a.starts_with("my-repo-"));
        assert_ne!(a, workspace_id(Path::new("/Users/you/my repo")));
    }
}
