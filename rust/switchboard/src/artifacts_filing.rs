//! Where a task's `sb artifact add` files a new artifact when it names no
//! `--folder` (artifact-folders, ported from art-filing's part 5;
//! architect m_39705, m_39723, m_46279): main
//! gives each task a folder at its spawn (`sb spawn --folder
//! "<theme>/<task>"`, kept by name on its brief); the hub makes it at the
//! task's first add, then the task's own artifacts carry the folder's id.
//! Pure: the shell (daemon/art.rs) reads the brief, the task's last
//! artifact and the library, then acts on the answer.

/// What the hub does with a new artifact of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filing {
    /// File it in this live folder (an id).
    Use(String),
    /// Find or make `<theme>/<task>` (by main), then file it there.
    Ensure(String),
    /// No folder.
    Unfiled,
}

/// The default folder of a task's add. A next version keeps the folder
/// its artifact has (`artifacts::Filed::Default` files a new artifact
/// only, artifacts_tests `law_a_default_folder_files_only_a_new_artifact`). `brief`: its brief's folder (None
/// or blank: none); `last`: the folder of its most recent artifact in this hub
/// (None: it has none yet; Some(None): that one is unfiled); `resolve`:
/// an id to the live folder it reads as (a merge redirects, a deleted
/// folder is None: `bise_home::library::resolve`).
///
/// No folder on the brief: unfiled (main, a task spawned without one).
/// Once the task has an artifact, its folder follows that artifact: a
/// rename keeps it, his drag moves the task's next adds with it, a
/// deleted folder stays deleted (never made again from the names).
///
/// Only what is for him makes a folder (his q-library pick, architect
/// m_62392): `last` is the task's last artifact for him, `for_you` this
/// add's audience; an add for the agents files into the task's folder
/// once one exists, never makes it.
pub fn filing(brief: Option<&str>, last: Option<Option<&str>>, for_you: bool, resolve: impl Fn(&str) -> Option<String>) -> Filing {
    let Some(brief) = brief.map(str::trim).filter(|b| !b.is_empty()) else { return Filing::Unfiled };
    match last {
        None if for_you => Filing::Ensure(brief.to_string()),
        None | Some(None) => Filing::Unfiled,
        Some(Some(id)) => resolve(id).map(Filing::Use).unwrap_or(Filing::Unfiled),
    }
}

/// What the shell reads about a task for its add's default folder.
#[derive(Clone, Debug, Default)]
pub struct Task<'a> {
    /// The folder main gave it at its spawn (`sb spawn --folder`, its
    /// brief's), None or blank: none.
    pub spawn_folder: Option<&'a str>,
    /// Its derived theme (`derived_folder`), used when main gave none.
    pub derived: Option<&'a str>,
    /// The folder of its most recent artifact, any audience (None: it has none).
    pub last_any: Option<Option<&'a str>>,
    /// The folder of its most recent artifact for him.
    pub last_for_you: Option<Option<&'a str>>,
}

/// The default folder of a task's add (folder-filing, the 11.11 fix of
/// e5be4f426): main's `--folder` is his library's place for that task,
/// so any add of it files there, the agents' ones too, and its last
/// artifact of any audience carries the folder on (the rule before
/// e5be4f426). Only the derived `<lead>/<task>` theme waits for an add
/// for him and follows its last one for him (architect m_62392).
pub fn task_filing(t: &Task, for_you: bool, resolve: impl Fn(&str) -> Option<String>) -> Filing {
    match t.spawn_folder.map(str::trim).filter(|f| !f.is_empty()) {
        Some(asked) => filing(Some(asked), t.last_any, true, resolve),
        None => filing(t.derived, t.last_for_you, for_you, resolve),
    }
}

/// The folder of a task main spawned with no `--folder` (architect
/// m_62392, designer m_62289): its lead's theme, `<lead>/<task>`, when its
/// parent is not main (a typed fact, never the spawn's free-text place);
/// else the task alone at the top, one level, never inside itself. Main
/// itself files nothing. Read once, at the task's first artifact for him:
/// that artifact's folder id carries it on, so his rename sticks.
pub fn derived_folder(task: &str, parent: Option<&str>) -> Option<String> {
    if task == crate::model::MAIN {
        return None;
    }
    Some(match parent.filter(|p| *p != crate::model::MAIN && !p.is_empty()) {
        Some(lead) => format!("{lead}/{task}"),
        None => task.to_string(),
    })
}

/// `sb spawn --folder` as typed, made `<theme>/<task>` (or `<theme>`):
/// one or two names, `/` or ` › ` between them, each trimmed.
pub fn folder_path(s: &str) -> Result<String, String> {
    let parts: Vec<&str> = s.split(['/', '›']).map(str::trim).collect();
    if parts.is_empty() || parts.len() > 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(format!("--folder expects \"<theme>/<task>\" (two levels at most), not {s:?}"));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The library: f-a live, f-old merged into f-a, f-gone deleted.
    fn lib(id: &str) -> Option<String> {
        match id {
            "f-a" | "f-old" => Some("f-a".into()),
            "f-b" => Some("f-b".into()),
            _ => None,
        }
    }

    #[test]
    fn no_folder_on_the_brief_is_unfiled() {
        assert_eq!(filing(None, None, true, lib), Filing::Unfiled);
        assert_eq!(filing(Some("  "), Some(Some("f-a")), true, lib), Filing::Unfiled);
    }

    #[test]
    fn the_first_add_makes_the_briefs_folder() {
        assert_eq!(filing(Some("artifacts/art-filing"), None, true, lib), Filing::Ensure("artifacts/art-filing".into()));
    }

    #[test]
    fn the_next_adds_follow_its_last_artifact() {
        assert_eq!(filing(Some("artifacts/art-filing"), Some(Some("f-a")), true, lib), Filing::Use("f-a".into()));
    }

    #[test]
    fn renamed_keeps_its_id() {
        // a rename touches no id: the same folder, whatever its name now
        assert_eq!(filing(Some("artifacts/old name"), Some(Some("f-b")), true, lib), Filing::Use("f-b".into()));
    }

    #[test]
    fn merged_follows_the_redirect() {
        assert_eq!(filing(Some("t/x"), Some(Some("f-old")), true, lib), Filing::Use("f-a".into()));
    }

    #[test]
    fn deleted_stays_deleted() {
        assert_eq!(filing(Some("t/x"), Some(Some("f-gone")), true, lib), Filing::Unfiled);
    }

    #[test]
    fn he_moved_it_to_unfiled() {
        assert_eq!(filing(Some("t/x"), Some(None), true, lib), Filing::Unfiled);
    }

    #[test]
    fn main_never_follows_a_drag() {
        // main has no brief folder: his drag of main's latest artifact
        // files none of main's next adds
        assert_eq!(filing(None, Some(Some("f-a")), true, lib), Filing::Unfiled);
    }

    #[test]
    fn only_what_is_for_him_makes_a_folder() {
        // an add for the agents: no folder yet, none made; once the task has one, it files there
        assert_eq!(filing(Some("t/x"), None, false, lib), Filing::Unfiled);
        assert_eq!(filing(Some("t/x"), Some(Some("f-a")), false, lib), Filing::Use("f-a".into()));
    }

    #[test]
    fn a_spawn_folder_files_the_agents_adds_too() {
        // proto_e2e_agents' lists_round (red since e5be4f426): main spawned it with
        // --folder x/y, its first add is for the agents
        let first = Task { spawn_folder: Some("x/y"), derived: Some("x/y"), last_any: None, last_for_you: None };
        assert_eq!(task_filing(&first, false, lib), Filing::Ensure("x/y".into()));
        // its next add for the agents follows that artifact, though none is for him yet
        let next = Task { last_any: Some(Some("f-a")), ..first.clone() };
        assert_eq!(task_filing(&next, false, lib), Filing::Use("f-a".into()));
        // he renamed x/y (same id f-b, another name): its next agents' add keeps his name, never makes x/y again
        let renamed = Task { last_any: Some(Some("f-b")), ..first.clone() };
        assert_eq!(task_filing(&renamed, false, lib), Filing::Use("f-b".into()));
        // his drag of it to unfiled moves its next adds with it
        let dragged = Task { last_any: Some(None), last_for_you: Some(Some("f-b")), ..first.clone() };
        assert_eq!(task_filing(&dragged, true, lib), Filing::Unfiled);
        // a blank --folder is none: the derived theme's rule
        let blank = Task { spawn_folder: Some("  "), derived: Some("lead/t"), ..Task::default() };
        assert_eq!(task_filing(&blank, false, lib), Filing::Unfiled);
    }

    #[test]
    fn a_derived_theme_waits_for_an_add_for_him() {
        let t = Task { spawn_folder: None, derived: Some("lead/t"), last_any: Some(Some("f-a")), last_for_you: None };
        assert_eq!(task_filing(&t, false, lib), Filing::Unfiled, "an agents' add makes no derived folder");
        assert_eq!(task_filing(&t, true, lib), Filing::Ensure("lead/t".into()));
        let made = Task { last_for_you: Some(Some("f-b")), ..t };
        assert_eq!(task_filing(&made, false, lib), Filing::Use("f-b".into()));
        // main: no folder at all
        assert_eq!(task_filing(&Task::default(), true, lib), Filing::Unfiled);
    }

    #[test]
    fn a_task_spawned_with_no_folder_gets_its_leads_theme_or_its_own() {
        assert_eq!(derived_folder("apps-gallery", Some("apps-lead")).as_deref(), Some("apps-lead/apps-gallery"));
        // main's own tasks: one level, never inside itself
        assert_eq!(derived_folder("artifacts-keep", Some(crate::model::MAIN)).as_deref(), Some("artifacts-keep"));
        assert_eq!(derived_folder("artifacts-keep", None).as_deref(), Some("artifacts-keep"));
        assert_eq!(derived_folder(crate::model::MAIN, None), None, "main files nothing");
    }

    #[test]
    fn folder_paths() {
        assert_eq!(folder_path("artifacts/art-filing").as_deref(), Ok("artifacts/art-filing"));
        assert_eq!(folder_path(" launch › hero film ").as_deref(), Ok("launch/hero film"));
        assert_eq!(folder_path("launch").as_deref(), Ok("launch"));
        assert!(folder_path("a/b/c").is_err());
        assert!(folder_path("a/").is_err());
        assert!(folder_path("").is_err());
    }
}
