//! The folders of his home macOS guards (TCC: « would like to access
//! files in your Documents folder »): Documents, Desktop, Downloads,
//! iCloud Drive, the cloud folders, and any volume. A read, a listing or
//! a stat inside one makes macOS ask him, so bise enters one only on his
//! act (the setup's scan, the files popup, the skills scan:
//! setup-scan-tcc, skills-scan-tcc).
//!
//! - [`protected`]: whether a path is one of those folders;
//! - [`within`]: the guarded folder a path is in (itself or an ancestor);
//! - [`label`]: the folder's name for him (`Documents`, `iCloud Drive`);
//! - [`link_into`]: whether reaching a path follows a link into one,
//!   found by reading links only, never a stat inside the folder.

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

/// The folders of his home macOS guards, in the order the setup offers
/// them.
pub const GUARDED: [&str; 5] = ["Documents", "Desktop", "Downloads", "Library/Mobile Documents", "Library/CloudStorage"];

/// A folder macOS guards: one of [`GUARDED`] under `home`, or a volume
/// (`/Volumes/<name>`).
pub fn protected(home: Option<&Path>, path: &Path) -> bool {
    let in_home = home.is_some_and(|h| GUARDED.iter().any(|g| path == h.join(g)));
    let volume = path.parent() == Some(Path::new("/Volumes"));
    in_home || volume
}

/// The guarded folder `path` is in: itself or its nearest guarded
/// ancestor (None: not in one). Pure.
pub fn within(home: Option<&Path>, path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|a| protected(home, a)).map(Path::to_path_buf)
}

/// A guarded folder's name for him: `iCloud Drive` for his Mobile
/// Documents, else its own name (`Documents`, `CloudStorage`, a volume's).
pub fn label(home: Option<&Path>, folder: &Path) -> String {
    if home.is_some_and(|h| folder == h.join("Library/Mobile Documents")) {
        return "iCloud Drive".into();
    }
    folder.file_name().map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Where a path's link leads into a guarded folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkInto {
    /// the link that leads in (itself outside every guarded folder: its
    /// own lstat asks nothing)
    pub link: PathBuf,
    /// the guarded folder it leads into
    pub folder: PathBuf,
}

/// Whether reaching `path` follows a link into a guarded folder (of
/// `home`). Resolves `path` one component at a time, reading links
/// (`lstat` and `readlink` of each prefix) and never touching a path
/// inside a guarded folder: each prefix is checked before its lstat.
/// A path that is itself in a guarded folder without a link (a project
/// he opened in ~/Documents) is his own place: None. A loop of links,
/// or a component that cannot be read, ends the walk: None (whatever
/// reads it next fails the same way, outside any guarded folder).
pub fn link_into(home: Option<&Path>, path: &Path) -> Option<LinkInto> {
    if within(home, path).is_some() {
        return None;
    }
    let mut todo: VecDeque<PathBuf> = parts(path).collect();
    let mut cur = PathBuf::new();
    let mut hops = 0;
    let mut last_link: Option<PathBuf> = None;
    while let Some(part) = todo.pop_front() {
        match part.components().next() {
            Some(Component::Prefix(_) | Component::RootDir) => cur = part,
            Some(Component::ParentDir) => {
                cur.pop();
            }
            Some(Component::Normal(n)) => {
                let next = cur.join(n);
                if let Some(folder) = within(home, &next) {
                    // reached through a link: `path` itself is in none
                    return Some(LinkInto { link: last_link.unwrap_or(next), folder });
                }
                let m = std::fs::symlink_metadata(&next).ok()?;
                if !m.file_type().is_symlink() {
                    cur = next;
                    continue;
                }
                hops += 1;
                if hops > 40 {
                    return None;
                }
                let target = std::fs::read_link(&next).ok()?;
                last_link = Some(next);
                for (i, t) in parts(&target).enumerate() {
                    todo.insert(i, t);
                }
            }
            Some(Component::CurDir) | None => {}
        }
    }
    None
}

/// A path's components, each as a path of its own.
fn parts(p: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    p.components().map(|c| PathBuf::from(c.as_os_str()))
}

#[cfg(test)]
#[path = "guarded_tests.rs"]
mod tests;
