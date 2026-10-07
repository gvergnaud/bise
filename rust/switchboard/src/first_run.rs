//! The home workspace's first run (ambient-lead m_5675, m_5796): bise
//! ambient's `start here` page (amb-kit's kit/examples/
//! start-here.html) is published once, as main, so it shows in `for you`
//! as new. A marker in the hub's state says it was done: it never comes
//! back, not even after the user deletes the page.

use std::path::{Path, PathBuf};

pub const PAGE_ID: &str = "start-here";
pub const TITLE: &str = "start here";
/// The marker, in the hub's state folder.
pub const MARKER: &str = "start-here.published";

/// The page's file in the app root.
pub fn page_file(app_root: &Path) -> PathBuf {
    app_root.join("kit/examples/start-here.html")
}

fn same(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// The page to publish now: only for the home workspace, only before the
/// marker exists, only when the app root has the file.
pub fn due(state: &Path, workspace: &Path, home: &Path, app_root: &Path) -> Option<String> {
    if !same(workspace, home) || state.join(MARKER).exists() {
        return None;
    }
    std::fs::read_to_string(page_file(app_root)).ok().filter(|h| !h.trim().is_empty())
}

/// Once published: never again.
pub fn mark(state: &Path, version: u64) -> std::io::Result<()> {
    std::fs::write(state.join(MARKER), format!("{PAGE_ID} v{version}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_here_is_due_once_on_the_home_workspace_only() {
        let t = std::env::temp_dir().join(format!("sb-first-run-{}", std::process::id()));
        let (state, home, repo, root) = (t.join("st"), t.join("bise"), t.join("repo"), t.join("app"));
        for d in [&state, &home, &repo, &root.join("kit/examples")] {
            std::fs::create_dir_all(d).unwrap();
        }
        // no file in the app root: nothing (and no marker, so a later run tries)
        assert_eq!(due(&state, &home, &home, &root), None);
        std::fs::write(page_file(&root), "<section data-kit=\"heading\" data-id=\"title\"><h1>start here</h1></section>\n").unwrap();
        // a repo workspace: never
        assert_eq!(due(&state, &repo, &home, &root), None);
        // the home: due, until marked
        assert!(due(&state, &home, &home, &root).unwrap().contains("start here"));
        mark(&state, 1).unwrap();
        assert_eq!(due(&state, &home, &home, &root), None);
        assert_eq!(std::fs::read_to_string(state.join(MARKER)).unwrap(), "start-here v1\n");
        let _ = std::fs::remove_dir_all(&t);
    }
}
