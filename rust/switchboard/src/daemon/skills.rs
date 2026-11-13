//! The skills a REPL reads, as the hub sees them: its skill folders (by
//! role and the desktop flag) and their fingerprint, which tells a live
//! REPL that a skill came, went, changed or was turned on or off (it
//! reloads at its next idle). The folders, the scan and the fingerprint
//! are `bend_plugins::skills`'s; this module only picks the agent's
//! [`BiseSkills`] and names a stuck path in hub.log. The fingerprint
//! runs at hub start, under the app, with no act of his: it follows his
//! switch (`skills::mode_of`), so a link into Documents is never stat'd
//! while it is off (skills-scan-tcc, architect m_64240).

use std::path::Path;

use bend_plugins::skills::{self, BiseSkills, Root};

use crate::paths::Paths;

/// An agent's skill folders (`bend_plugins::skills::roots`): main reads
/// the app root's `prompts/skills`, every agent its `prompts/skills-all`
/// where the desktop is on (architect m_12576); his own folders from
/// Home, none under a throwaway `$BISE_HOME`.
pub(super) fn skill_roots(ws: &Path, app_root: &Path, is_main: bool, desktop: bool) -> Vec<Root> {
    let home = bise_home::Home::from_env();
    skills::roots(skills::user_home(&home), ws, Some(app_root), BiseSkills::of(is_main, desktop))
}

/// The fingerprint of `roots` with the on/off state; a path stuck for
/// the first time (a SKILL.md behind macOS's privacy wait) is named once
/// in hub.log.
pub(super) fn skills_fingerprint(paths: &Paths, roots: &[Root]) -> u64 {
    let st = bend_plugins::state::read(&bend_plugins::state::state_path());
    let mode = skills::mode_of(&bise_home::Home::from_env());
    let (fp, stuck) = skills::fingerprint(roots, &st, skills::SCAN_LIMIT, &mode);
    for p in stuck {
        super::log_line(paths, &format!("skills scan: skipped {} (no answer in {} s)", p.display(), skills::SCAN_LIMIT.as_secs()));
    }
    fp
}
