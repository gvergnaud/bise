//! A workspace's hub state folder and its agents' folders in it: one
//! owner (switchboard::paths::Paths calls these; a client that reads an
//! agent's files on this machine asks here too). The hub's socket can
//! live elsewhere (`socket`: a long path moves it to
//! `/tmp/bise-<uid>/<hash>`), so a client finds the state from the
//! workspace, never from the socket's folder.

use std::path::{Path, PathBuf};

/// The hub state folder of `workspace` (canonical when it exists):
/// `SB_STATE_DIR` (a test setting), else `Home::hub_dir(hub_id)`.
pub fn state_dir(workspace: &Path) -> PathBuf {
    match crate::env::test_setting("SB_STATE_DIR") {
        Some(d) => PathBuf::from(d),
        None => {
            let ws = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
            crate::Home::from_env().hub_dir(&crate::hub_id(&ws))
        }
    }
}

/// An agent's folder (`dir`: its folder name, the agent row's `dir`).
pub fn agent_dir(state: &Path, dir: &str) -> PathBuf {
    state.join("agents").join(dir)
}

/// The agent's temp folder (approvals-design.md §7.1): its `TMPDIR`,
/// created at spawn, deleted at its /drop.
pub fn agent_tmp(state: &Path, dir: &str) -> PathBuf {
    agent_dir(state, dir).join("tmp")
}

/// `path` is inside an agent's temp folder of any hub of `home`
/// (`<hubs>/<hub>/agents/<a>/tmp/...`, [`agent_tmp`]'s shape): a
/// worktree an agent made there for itself is no place of the hub's, so
/// its polls skip it (switchboard daemon/worktrees.rs, git-calm).
pub fn is_agent_tmp(home: &crate::Home, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(home.hubs_dir()) else { return false };
    let c: Vec<_> = rel.components().map(|c| c.as_os_str()).collect();
    c.len() >= 4 && c[1] == "agents" && c[3] == "tmp"
}

/// Where an agent's background commands keep their files
/// (`BEND_BG_DIR`): `<n>.{slot,pid,cmd,out,in,rc}`, from its temp folder.
pub fn bg_dir_of_tmp(tmp: &Path) -> PathBuf {
    tmp.join("bg")
}

/// The agent's background commands' folder.
pub fn agent_bg(state: &Path, dir: &str) -> PathBuf {
    bg_dir_of_tmp(&agent_tmp(state, dir))
}

/// The views folder of the agent whose folder is `agent`: the full result
/// of each of its tool calls with a view, `<call>.json` (docs/mcp-apps.md
/// C2; `BEND_VIEWS_DIR`, the plugins bridge writes there).
pub fn views_of_agent_dir(agent: &Path) -> PathBuf {
    agent.join("views")
}

/// The agent's views folder ([`views_of_agent_dir`]).
pub fn agent_views(state: &Path, dir: &str) -> PathBuf {
    views_of_agent_dir(&agent_dir(state, dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agents_files_sit_in_its_folder() {
        let s = Path::new("/h/hubs/x-0a1b2c3d");
        assert_eq!(agent_tmp(s, "perf"), Path::new("/h/hubs/x-0a1b2c3d/agents/perf/tmp"));
        assert_eq!(agent_bg(s, "perf.2"), Path::new("/h/hubs/x-0a1b2c3d/agents/perf.2/tmp/bg"));
        assert_eq!(agent_bg(s, "my agent"), Path::new("/h/hubs/x-0a1b2c3d/agents/my agent/tmp/bg"));
        assert_eq!(agent_views(s, "perf"), Path::new("/h/hubs/x-0a1b2c3d/agents/perf/views"));
    }

    #[test]
    fn an_agents_temp_worktree_is_known_in_any_hub_of_the_home() {
        let home = crate::Home::from_lookup(&|k: &str| (k == "HOME").then(|| "/u".to_string()));
        let tmp = |p: &str| is_agent_tmp(&home, Path::new(p));
        assert!(tmp("/u/.bise/hubs/dash-0a1b/agents/vb-workflow/tmp/upgrade-wt"));
        assert!(tmp("/u/.bise/hubs/other-9f9f/agents/vb-agent/tmp/nonjson-wt"), "another hub of this home");
        assert!(tmp("/u/.bise/hubs/dash-0a1b/agents/a/tmp"));
        assert!(!tmp("/u/.bise/worktrees/dash-0a1b/perf/dashboard"), "an agent's worktree is the hub's place");
        assert!(!tmp("/u/.bise/hubs/dash-0a1b/agents/a/views/x"));
        assert!(!tmp("/u/.bise/hubs/dash-0a1b/tmp/x"));
        assert!(!tmp("/u/mistral/dashboard"));
        assert!(!tmp("/elsewhere/hubs/dash/agents/a/tmp/wt"), "not this home's hubs");
    }
}
