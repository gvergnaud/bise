//! Who keeps a workspace when a starting hub finds another one there
//! (update-safety): the decision is pure ([`plan`]), the facts are read by
//! [`read`], daemon.rs `claim` runs it.
//!
//! Gauthier's 11.5 -> 11.9 update: during the home hub's version switch
//! the old hub stops, and the desktop core (an older one: its own home
//! connector, harness ambient.rs) starts a hub again at once from ITS
//! runtime. That intruder raced the switcher's new hub for the workspace:
//! the new hub yielded ('a hub is already running') and the probation
//! watched the old one, or both ran (the switcher unlinked the intruder's
//! socket) and the new one never got main's session. The switcher and the
//! core that do this are already on his Mac; only the new hub runs new
//! code there, so the new hub takes the workspace back: a switch to its
//! own root runs, and the live hub there runs another root.
//!
//! TODO(switch-intruder): goes in v2026.12.15: by then every core and
//! TUI an update starts from waits for a running switch (client::open,
//! from 11.10) instead of starting a hub, and 11.9 is past the window
//! kept for updates from an old install.

use crate::paths::Paths;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a starting hub knows of its workspace, each fact read, never
/// guessed: None when it could not be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// a switcher runs for this workspace (`switch.pid` names a live process)
    pub switch_running: bool,
    /// versions.json `current` is this hub's own app root
    pub current_is_me: Option<bool>,
    /// versions.json `switch.phase` is `reloading` (the switch is replacing the hub now)
    pub reloading: bool,
    /// the live hub there (hub.pid names a live `sbd` that isn't this process)
    pub other: Option<Other>,
}

/// The live hub a starting hub found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Other {
    pub pid: u32,
    /// its `hub.root`; None: unreadable
    pub root: Option<PathBuf>,
    /// that root is this hub's own
    pub root_is_mine: Option<bool>,
}

/// What the starting hub does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claim {
    /// today's way: wait for a hub that is stopping, give way to one that runs
    Yield,
    /// this hub is the switch's, the live one an intruder from another
    /// root: this hub takes the workspace (its pid first, then the
    /// intruder stops, its agents kept)
    Take { pid: u32, root: Option<PathBuf> },
}

/// The decision (architect m_58855): take over only when every fact says
/// this hub is the one the running switch is starting and the live hub is
/// another version; any fact unread, or a hub of the same root: yield.
pub fn plan(f: &Facts) -> Claim {
    match &f.other {
        Some(o) if f.switch_running && f.reloading && f.current_is_me == Some(true) && o.root_is_mine == Some(false) => Claim::Take { pid: o.pid, root: o.root.clone() },
        _ => Claim::Yield,
    }
}

/// The facts of `paths`' workspace for the hub of `app_root`, process `me`.
pub fn read(paths: &Paths, app_root: &Path, me: u32) -> Facts {
    let canon = |p: &Path| p.canonicalize().ok();
    let mine = canon(app_root);
    let st: Option<Value> = std::fs::read_to_string(super::state_file(paths)).ok().and_then(|t| serde_json::from_str(&t).ok());
    let current_is_me = st
        .as_ref()
        .and_then(|v| v.get("current"))
        .and_then(Value::as_str)
        .and_then(|c| Some(canon(Path::new(c))? == mine.clone()?));
    let reloading = st.as_ref().and_then(|v| v.pointer("/switch/phase")).and_then(Value::as_str) == Some("reloading");
    let other = super::hub_pid(paths).filter(|&p| p != me && crate::procs::alive(p) && is_hub(p)).map(|pid| {
        let root = std::fs::read_to_string(paths.state.join("hub.root")).ok().map(|r| PathBuf::from(r.trim()));
        let root_is_mine = root.as_deref().and_then(|r| Some(canon(r)? == mine.clone()?));
        Other { pid, root, root_is_mine }
    });
    Facts { switch_running: super::switch_running(paths), current_is_me, reloading, other }
}

/// `pid` is a hub process (`... sbd ...`), not a later process that got its pid.
pub fn is_hub(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(" sbd"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn switching(other: Option<Other>) -> Facts {
        Facts { switch_running: true, current_is_me: Some(true), reloading: true, other }
    }
    fn hub(root_is_mine: Option<bool>) -> Option<Other> {
        Some(Other { pid: 42, root: Some(PathBuf::from("/v/old")), root_is_mine })
    }

    #[test]
    fn the_switchs_hub_takes_the_workspace_from_a_hub_of_another_root() {
        assert_eq!(plan(&switching(hub(Some(false)))), Claim::Take { pid: 42, root: Some(PathBuf::from("/v/old")) });
    }

    #[test]
    fn any_fact_missing_or_unread_gives_way_as_before() {
        assert_eq!(plan(&switching(None)), Claim::Yield, "no live hub: the usual claim");
        assert_eq!(plan(&switching(hub(Some(true)))), Claim::Yield, "a hub of this very root: the same version, never taken");
        assert_eq!(plan(&switching(hub(None))), Claim::Yield, "its root unread");
        assert_eq!(plan(&Facts { switch_running: false, ..switching(hub(Some(false))) }), Claim::Yield, "no switch runs: his own start");
        assert_eq!(plan(&Facts { reloading: false, ..switching(hub(Some(false))) }), Claim::Yield, "the switch is past its start (probation passed, rolled back)");
        assert_eq!(plan(&Facts { current_is_me: Some(false), ..switching(hub(Some(false))) }), Claim::Yield, "a switch to another version: a rollback's start, not this hub's");
        assert_eq!(plan(&Facts { current_is_me: None, ..switching(hub(Some(false))) }), Claim::Yield, "versions.json unread");
    }
}
