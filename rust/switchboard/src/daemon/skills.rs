//! The skills a REPL's startup scan reads: which folders (by role and the
//! desktop flag) and their fingerprint, which tells a live REPL that a
//! skill came, went or changed (it reloads at its next idle).

use std::path::{Path, PathBuf};

/// The skill folders a REPL's startup scan reads (runtime/skills.bend
/// `scan_script`): `~/.agents/skills`, `~/.vibe/skills`,
/// `<ws>/.agents/skills`, the app root's `prompts/skills-all` (every
/// agent's built-ins, bise-pages: only where the desktop is on, the same
/// flag as the prompt's page rules, architect m_12576) and, for main, the app root's
/// `prompts/skills` (main's own, bise-demo; the plugins' skills are in the
/// plugins fingerprint).
pub(super) fn skill_roots(ws: &Path, app_root: &Path, is_main: bool, desktop: bool) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        let home = PathBuf::from(home);
        roots.push(home.join(".agents/skills"));
        roots.push(home.join(".vibe/skills"));
    }
    roots.push(ws.join(".agents/skills"));
    if desktop {
        roots.push(app_root.join("prompts/skills-all"));
    }
    if is_main {
        roots.push(app_root.join("prompts/skills"));
    }
    roots
}

/// Each root, then each `<root>/<skill>/SKILL.md` (sorted) with its size
/// and mtime; a missing root or file hashes as absent.
pub(super) fn skills_fingerprint(roots: &[PathBuf]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for root in roots {
        root.hash(&mut h);
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let Ok(m) = std::fs::metadata(d.join("SKILL.md")) else { continue };
            d.hash(&mut h);
            m.len().hash(&mut h);
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A SKILL.md added, edited or removed in a root moves the skills
    /// fingerprint; nothing changing, or a folder without SKILL.md, does not.
    #[test]
    fn the_skills_fingerprint_moves_when_a_skill_comes_goes_or_changes() {
        let d = std::env::temp_dir().join(format!("sb-skills-fp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let roots = vec![d.join("user"), d.join("ws/.agents/skills")];
        let empty = skills_fingerprint(&roots);
        std::fs::create_dir_all(d.join("ws/.agents/skills/notes")).unwrap();
        assert_eq!(skills_fingerprint(&roots), empty, "a folder without SKILL.md is no skill");
        let f = d.join("ws/.agents/skills/a/SKILL.md");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "---\nname: a\ndescription: one\n---\n").unwrap();
        let added = skills_fingerprint(&roots);
        assert_ne!(added, empty);
        assert_eq!(skills_fingerprint(&roots), added, "stable while nothing changes");
        std::fs::write(&f, "---\nname: a\ndescription: two longer\n---\n").unwrap();
        let edited = skills_fingerprint(&roots);
        assert_ne!(edited, added);
        std::fs::remove_dir_all(f.parent().unwrap()).unwrap();
        assert_eq!(skills_fingerprint(&roots), empty);
        // main reads the app root's prompts/skills too, a task does not
        let ws = d.join("ws");
        assert!(skill_roots(&ws, &d, true, false).contains(&d.join("prompts/skills")));
        assert!(!skill_roots(&ws, &d, false, false).contains(&d.join("prompts/skills")));
        // law (architect m_12576): prompts/skills-all (bise-pages) only
        // where the desktop is on, the prompt's own flag; a plain
        // project's agents read none of it, main or task
        for main in [true, false] {
            assert!(skill_roots(&ws, &d, main, true).contains(&d.join("prompts/skills-all")));
            assert!(!skill_roots(&ws, &d, main, false).contains(&d.join("prompts/skills-all")));
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
