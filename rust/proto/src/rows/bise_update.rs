//! A newer bise is out (update-flow): what a hub says about it
//! (`hub/bise_update`, its own version: switchboard's release_line.rs from
//! `bise_home::release_offer`), and the machine's one line the desktop
//! core folds from every project's (`core/bise_update`,
//! `bise_home::release_offer::machine`). The TUI's status line reads its
//! hub's; the window's status pill the machine's. Words: the catalog's.

use serde::{Deserialize, Serialize};

/// Where an update is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BiseUpdateState {
    /// newer, nothing asked: `update` updates it
    #[default]
    Ready,
    /// `update` asked: a hub installs it
    Installing,
    /// installed: the projects switch to it, one at a time
    Updating,
    /// a project's switch rolled back (`why`): it stays where it is
    Failed,
    /// a newer core's state
    #[serde(other)]
    Unknown,
}

/// A release to update to, and where that is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BiseUpdate {
    /// the release's id, and its name as he reads it (`v2026.10.3`)
    pub id: String,
    pub version: String,
    /// what's new, 0 to 5 plain lines
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// its release page
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// the version this hub runs, as he reads it (the machine's line: the
    /// first project's)
    #[serde(default)]
    pub running: String,
    pub state: BiseUpdateState,
    /// failed: why, one line
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// failed: the new version's last log lines (at most 50, plain text),
    /// for his 'Copy report' (update-safety B, designer m_59048)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<String>,
    /// the machine's line only: the projects not on it yet (updating),
    /// or the one whose switch failed (their names)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting: Vec<String>,
}

/// The machine's one line (update-flow, architect m_45149): what every
/// project's hub says (`hub/bise_update`, None: on it), folded. They are
/// the same fact seen by hubs on different versions. None when nothing
/// is to show (every project on it); failed beats updating beats
/// installing beats ready; `waiting` names the projects not on it yet
/// (failed: the ones whose switch rolled back). `running`: the first
/// project's.
pub fn machine(hubs: &[(String, Option<BiseUpdate>)]) -> Option<BiseUpdate> {
    let rank = |s: &BiseUpdateState| match s {
        BiseUpdateState::Failed => 4,
        BiseUpdateState::Updating => 3,
        BiseUpdateState::Installing => 2,
        BiseUpdateState::Ready => 1,
        BiseUpdateState::Unknown => 0,
    };
    let shown = |u: &Option<BiseUpdate>| u.as_ref().filter(|u| rank(&u.state) > 0).cloned();
    let top = hubs.iter().filter_map(|(_, u)| shown(u)).max_by_key(|u| rank(&u.state))?;
    let waiting = hubs
        .iter()
        .filter(|(_, u)| match (&top.state, shown(u)) {
            (BiseUpdateState::Failed, Some(u)) => u.state == BiseUpdateState::Failed,
            (_, u) => u.is_some(),
        })
        .map(|(p, _)| p.clone())
        .collect();
    Some(BiseUpdate { waiting, ..top })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub(project: &str, state: Option<BiseUpdateState>) -> (String, Option<BiseUpdate>) {
        (project.into(), state.map(|state| BiseUpdate { id: "6".into(), version: "v2026.10.3".into(), state, ..BiseUpdate::default() }))
    }

    #[test]
    fn the_machine_line_folds_every_project() {
        use BiseUpdateState as S;
        let m = |hubs: &[(String, Option<BiseUpdate>)]| machine(hubs).map(|r| (r.state, r.waiting));
        // every project on it: nothing
        assert_eq!(m(&[hub("a", None), hub("b", None)]), None);
        assert_eq!(m(&[]), None);
        // ready, naming every project not on it
        assert_eq!(m(&[hub("a", Some(S::Ready)), hub("b", None)]), Some((S::Ready, vec!["a".into()])));
        // installing beats ready
        assert_eq!(m(&[hub("a", Some(S::Ready)), hub("b", Some(S::Installing))]).unwrap().0, S::Installing);
        // updating names the projects not on it yet
        assert_eq!(
            m(&[hub("a", None), hub("b", Some(S::Updating)), hub("c", Some(S::Updating))]),
            Some((S::Updating, vec!["b".into(), "c".into()]))
        );
        // failed on one: failed, naming it
        assert_eq!(m(&[hub("a", None), hub("b", Some(S::Failed)), hub("c", Some(S::Updating))]), Some((S::Failed, vec!["b".into()])));
        // a newer core's state alone shows nothing
        assert_eq!(m(&[hub("a", Some(S::Unknown))]), None);
    }
}
