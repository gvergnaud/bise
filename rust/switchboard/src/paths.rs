//! Where a workspace's hub keeps its state (RFC 0001 §5):
//! `$SB_STATE_DIR`, else `<hubs dir>/<name>-<hash>` (`bise_home::Home::hub_dir`:
//! `~/.bise/hubs/`),
//! and an agent's folders in it: `bise_home::hub_state`'s (one owner).
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
    /// The agents' run dir (`$BEND_RUN_DIR`, `bise_home::Home::run_dir`),
    /// read once: the hub gives it to each REPL it starts and finds their
    /// bridges in it ([`Paths::agent_bridge_json`]), so it never reads
    /// another home than its REPLs write.
    pub run: PathBuf,
}

/// The hub id of a workspace: `bise_home::hub_id` (one owner).
pub fn workspace_id(workspace: &Path) -> String {
    bise_home::hub_id(workspace)
}

/// `ws` is the home workspace: its hub's main is bise (bise desktop S2,
/// architect m_8474: the one flag, nothing written, nothing in sb-core).
pub fn is_home(ws: &Path) -> bool {
    bise_home::projects::is_home(ws)
}

/// Whether `ws` is in bise's projects registry (bise_home::projects, which
/// the desktop app writes when it shows a project): one of the two facts
/// of prompts::desktop_on, read once when a prompt is built.
pub fn is_registered(ws: &Path) -> bool {
    bise_home::projects::is_registered(ws)
}

/// The home workspace ([`bise_home::Home::bise_root`]), created on first
/// use (never git-initialised).
pub fn ensure_home_workspace() -> std::io::Result<PathBuf> {
    let d = bise_home::Home::from_env().bise_root();
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

impl Paths {
    pub fn for_workspace(workspace: &Path) -> Paths {
        let workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());
        // the state folder: bise_home's (one owner, the TUI reads it too)
        let state = bise_home::hub_state::state_dir(&workspace);
        let worktrees = match bise_home::env::test_setting("SB_STATE_DIR") {
            Some(_) => state.join("worktrees"),
            None => bise_home::Home::from_env().worktrees_dir().join(workspace_id(&workspace)),
        };
        let run = bise_home::Home::from_env().run_dir();
        Paths { workspace, state, worktrees, run }
    }

    /// The hub's socket as clients and the hub reach it: `<state>/hub.sock`
    /// when that fits in a unix socket address, else its short path
    /// through `/tmp/bise-<uid>/<hash>/` (`bise_home::socket`; the hub
    /// makes the link before it binds, [`Paths::prepare_socket`]).
    pub fn socket(&self) -> PathBuf {
        bise_home::socket::socket_path(&self.natural_socket())
    }
    /// This hub's id in its agents' tags (`BISE_OWNERS`, `procs`): the
    /// hash of [`Paths::socket`], the path it is reached by (the short one
    /// when relocated). The TUI hashes the same path (`bise`'s
    /// `tui_socket`) to show this hub's agents in computer use's state.
    pub fn proc_hub(&self) -> String {
        crate::procs::hub_id(&self.socket())
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
    /// One line per land (`crate::land_record`): who landed which commits.
    pub fn lands(&self) -> PathBuf {
        self.state.join("lands.jsonl")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.state.join("hub.pid")
    }
    pub fn log(&self) -> PathBuf {
        self.state.join("hub.log")
    }
    /// The image store's care (image_care.rs): touched when this hub
    /// last touched the images it names, and when it last pruned.
    pub fn images_touched(&self) -> PathBuf {
        self.state.join("images-touched")
    }
    pub fn images_pruned(&self) -> PathBuf {
        self.state.join("images-pruned")
    }
    /// When the rule started on this hub (its first tick after the
    /// update): no prune before keep_days after it (the grace).
    pub fn images_since(&self) -> PathBuf {
        self.state.join("images-since")
    }
    /// The processes that ended in the last 24 h (`ended.jsonl`) and
    /// their kept output (`<agent>-<id>.out`), daemon/processes.rs's.
    pub fn procs_dir(&self) -> PathBuf {
        self.state.join("procs")
    }
    /// The diffs' guides (`<diff hash>.json`, page review-guided):
    /// guide.rs's, its one reader and writer, the newest 200 kept.
    pub fn guides_dir(&self) -> PathBuf {
        self.state.join("guides")
    }
    /// The output of the local sites' servers the hub started again
    /// (`<id>.out`, daemon/revive.rs's; site-revive).
    pub fn revive_dir(&self) -> PathBuf {
        self.state.join("revive")
    }
    pub fn bin_dir(&self) -> PathBuf {
        self.state.join("bin")
    }
    /// Feature `name`'s diff against the trunk, written at each `sb
    /// feature ready` (daemon/features.rs) and kept as one artifact: the
    /// try item's `2 show the diff` opens it.
    pub fn feature_diff(&self, name: &str) -> PathBuf {
        self.state.join("features").join(format!("{name}.diff"))
    }
    /// The last login-shell PATH a hub of this workspace read
    /// (tools_env::remembered_login's): the next hub's agents get it when
    /// its own probe fails.
    pub fn login_path_file(&self) -> PathBuf {
        self.state.join("login-path")
    }
    pub fn agent_dir(&self, name: &str) -> PathBuf {
        bise_home::hub_state::agent_dir(&self.state, name)
    }
    /// The agent's temp folder (approvals-design.md §7.1): its `TMPDIR`,
    /// created at spawn, deleted at its /drop (bise_home::hub_state).
    pub fn agent_tmp(&self, name: &str) -> PathBuf {
        bise_home::hub_state::agent_tmp(&self.state, name)
    }
    /// The agent's background commands' folder (`BEND_BG_DIR`).
    pub fn agent_bg(&self, name: &str) -> PathBuf {
        bise_home::hub_state::agent_bg(&self.state, name)
    }
    /// The hub's views bridge (mcp-apps, C5): its `bridge.json`, log
    /// and pid, so a restarted hub ends an orphan.
    pub fn views_bridge_dir(&self) -> PathBuf {
        self.state.join("views-bridge")
    }
    /// The views bridge's `bridge.json` (in [`Paths::views_bridge_dir`]).
    pub fn views_bridge_json(&self) -> PathBuf {
        self.views_bridge_dir().join(bend_plugins::bridge::BRIDGE_FILE)
    }
    /// An agent's own bridge's `bridge.json`, the REPL on `port`'s
    /// (`$BEND_RUN_DIR/<port>/plugins`, docs/mcp-apps.md C1): the one path
    /// from the hub to an agent's bridge.
    pub fn agent_bridge_json(&self, port: u16) -> PathBuf {
        self.run.join(port.to_string()).join("plugins").join(bend_plugins::bridge::BRIDGE_FILE)
    }
    /// The agent's MCP Apps views folder (`BEND_VIEWS_DIR`,
    /// docs/mcp-apps.md C2): pruned with the images (`image_care`).
    pub fn agent_views_dir(&self, name: &str) -> PathBuf {
        bise_home::hub_state::agent_views(&self.state, name)
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
        // the hub tag (bise_peer::tags::hub_id): tests/e2e.py hub_tag, the
        // one Python copy (proc_cleanup, long_socket_e2e, worktree_home,
        // tui_computer_use_tmux read it; architect m_13639)
        assert_eq!(crate::procs::hub_id(Path::new("/Users/me/.bise/hubs/harness-af1b2326/hub.sock")), "5d6d31bf066b5443");
        assert_eq!(crate::procs::hub_id(Path::new("/tmp/bise-501/0123456789abcdef/hub.sock")), "5bee91326dd20541");
    }

    /// The one flag of bise's role (architect m_8474): the home workspace,
    /// by its canonical path (a trailing slash or a symlink to it counts),
    /// never another folder.
    #[test]
    fn only_the_home_workspace_is_home() {
        let home = bise_home::Home::from_env().bise_root();
        assert!(is_home(&home));
        assert!(is_home(&home.join(".")));
        assert!(!is_home(&home.join("projects")) && !is_home(Path::new("/")));
    }

    /// The hub and a client find an agent's files at the same place
    /// (bise_home::hub_state, architect m_38397): where the agent's
    /// BEND_BG_DIR puts its bg jobs.
    #[test]
    fn an_agents_bg_folder_is_bise_homes() {
        let p = Paths { workspace: "/w".into(), state: "/h/hubs/w-0a1b2c3d".into(), worktrees: "/h/wt".into(), run: "/h/run".into() };
        for dir in ["perf", "perf.2", "x.2", "my agent"] {
            let tmp = p.agent_tmp(dir);
            let bg = crate::tools_env::temp_env(&tmp, &p.agent_run(dir)).into_iter().find(|(k, _)| *k == "BEND_BG_DIR").map(|(_, v)| v);
            assert_eq!(bg.as_deref(), Some(p.agent_bg(dir).as_path()), "{dir}");
            assert_eq!(p.agent_bg(dir), bise_home::hub_state::agent_bg(&p.state, dir));
        }
    }

    #[test]
    fn ids_are_stable_and_readable() {
        let a = workspace_id(Path::new("/Users/me/my repo"));
        assert_eq!(a, workspace_id(Path::new("/Users/me/my repo")));
        assert!(a.starts_with("my-repo-"));
        assert_ne!(a, workspace_id(Path::new("/Users/you/my repo")));
    }
}
