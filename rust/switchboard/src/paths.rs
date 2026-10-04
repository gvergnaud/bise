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

/// FNV-1a, 64 bits: a stable id for a workspace path.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub fn workspace_id(workspace: &Path) -> String {
    let base = workspace
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());
    let base: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(32)
        .collect();
    format!(
        "{}-{:08x}",
        base,
        fnv1a(&workspace.to_string_lossy()) as u32
    )
}

impl Paths {
    pub fn for_workspace(workspace: &Path) -> Paths {
        let workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());
        let (state, worktrees) = match std::env::var("SB_STATE_DIR") {
            Ok(d) if !d.is_empty() => (PathBuf::from(&d), PathBuf::from(d).join("worktrees")),
            _ => {
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
    /// Make [`Paths::socket`] bindable (the short link, when needed).
    pub fn prepare_socket(&self) -> std::io::Result<PathBuf> {
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

    #[test]
    fn ids_are_stable_and_readable() {
        let a = workspace_id(Path::new("/Users/me/my repo"));
        assert_eq!(a, workspace_id(Path::new("/Users/me/my repo")));
        assert!(a.starts_with("my-repo-"));
        assert_ne!(a, workspace_id(Path::new("/Users/you/my repo")));
    }
}
