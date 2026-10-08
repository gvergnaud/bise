//! The roots, the protected paths and the secret paths (design §7, §3
//! tier 0). Lexical work is pure; symlinks go through `Fs`, so the tests
//! can fake the disk.

use std::path::{Component, Path, PathBuf};

/// How a path's symlinks are resolved.
pub trait Fs {
    /// The path with the symlinks of its longest existing prefix resolved;
    /// the part that does not exist yet is kept as is.
    fn real(&self, p: &Path) -> PathBuf;
}

/// The disk: `canonicalize` on the longest existing prefix.
pub struct RealFs;

impl Fs for RealFs {
    fn real(&self, p: &Path) -> PathBuf {
        // the leaf: one `lstat`; only a link is followed whole
        if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
            return std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        }
        // the folders above it: the longest one that exists
        let mut rest: Vec<&std::ffi::OsStr> = vec![];
        let mut cur = p;
        while let (Some(parent), Some(name)) = (cur.parent(), cur.file_name()) {
            rest.push(name);
            if let Ok(c) = std::fs::canonicalize(parent) {
                return rest.iter().rev().fold(c, |acc, s| acc.join(s));
            }
            cur = parent;
        }
        p.to_path_buf()
    }
}

/// No symlinks: the lexical path (tests, and a disk-free judge).
pub struct LexicalFs;

impl Fs for LexicalFs {
    fn real(&self, p: &Path) -> PathBuf {
        p.to_path_buf()
    }
}

/// Where an agent may write (design §7).
#[derive(Clone, Debug)]
pub struct Roots {
    /// The agent's current folder (workspace or worktree).
    pub cwd: PathBuf,
    /// The user's home, for `~`.
    pub home: PathBuf,
    /// bise's home (`~/.bise`, or `$BISE_HOME`).
    pub bise: PathBuf,
    /// The agent's temp folder (`<bise>/hubs/<hub>/agents/<agent>/tmp`).
    pub tmp: PathBuf,
}

/// Why a path is protected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Protected {
    /// `.git/` internals (hooks, config, index…).
    Git,
    /// Another protected file: the hub state, `approvals.toml`,
    /// `auth.json`, `.envrc`, shell startup files, `~/.ssh`.
    File,
}

impl Roots {
    /// The roots with their own symlinks resolved.
    pub fn real(&self, fs: &dyn Fs) -> Roots {
        Roots {
            cwd: fs.real(&self.cwd),
            home: fs.real(&self.home),
            bise: fs.real(&self.bise),
            tmp: fs.real(&self.tmp),
        }
    }

    /// A word as a path: `~` expanded, joined to `base`, `.` and `..`
    /// folded, then the symlinks resolved. `None` when it is relative and
    /// the base is unknown (after `cd $X`).
    pub fn resolve(&self, base: Option<&Path>, word: &str, fs: &dyn Fs) -> Option<PathBuf> {
        let p = if word == "~" {
            self.home.clone()
        } else if let Some(rest) = word.strip_prefix("~/") {
            self.home.join(rest)
        } else if word.starts_with('/') {
            PathBuf::from(word)
        } else {
            base?.join(word)
        };
        Some(fs.real(&fold(&p)))
    }

    /// Inside a root and not protected (design §7).
    pub fn writable(&self, p: &Path) -> bool {
        self.protected(p).is_none() && (self.inside(p) || is_device(p))
    }

    /// Inside a root (protected or not).
    pub fn inside(&self, p: &Path) -> bool {
        p.starts_with(&self.cwd) || p.starts_with(&self.bise) || p.starts_with(&self.tmp)
    }

    /// A write here is a hard rule (tier 0).
    pub fn protected(&self, p: &Path) -> Option<Protected> {
        if p.components().any(|c| c.as_os_str() == ".git") {
            return Some(Protected::Git);
        }
        if p.file_name().is_some_and(|n| n == ".envrc") {
            return Some(Protected::File);
        }
        // the agent's own temp folder is the one writable place in `hubs/`
        if p.starts_with(&self.tmp) {
            return None;
        }
        let bise_files = [self.bise.join("hubs"), self.bise.join("approvals.toml")];
        let secrets = super::secrets::Secrets::of(&self.bise, &self.home);
        if bise_files.iter().any(|f| p.starts_with(f)) || secrets.held(p) == Some(super::secrets::Held::Bise) {
            return Some(Protected::File);
        }
        let home_files = [
            ".ssh",
            ".zshrc",
            ".zprofile",
            ".zshenv",
            ".bashrc",
            ".bash_profile",
            ".profile",
            ".config/git",
            ".gitconfig",
            "Library/LaunchAgents",
        ];
        if home_files.iter().any(|f| p.starts_with(self.home.join(f))) {
            return Some(Protected::File);
        }
        None
    }

    /// The path as the card shows it: relative to the folder, `~` for the
    /// home, cut in the middle when long (designer: keep the file name).
    pub fn show(&self, p: &Path) -> String {
        let s = match p.strip_prefix(&self.cwd) {
            Ok(r) if !r.as_os_str().is_empty() => r.display().to_string(),
            _ => match p.strip_prefix(&self.home) {
                Ok(r) => format!("~/{}", r.display()),
                Err(_) => p.display().to_string(),
            },
        };
        cut_middle(&s, 48)
    }
}

/// `/dev/null`, `/dev/stdout`, `/dev/fd/N`, ttys: writes that reach no file.
fn is_device(p: &Path) -> bool {
    let s = p.to_string_lossy();
    matches!(
        s.as_ref(),
        "/dev/null" | "/dev/stdout" | "/dev/stderr" | "/dev/tty"
    ) || s.starts_with("/dev/fd/")
}

/// Cut a long path in the middle with `…`, keeping its file name.
pub fn cut_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let name = s.rsplit('/').next().unwrap_or(s);
    let keep_tail = (name.chars().count() + 1).min(max.saturating_sub(4));
    let head_len = max.saturating_sub(keep_tail + 1);
    let head: String = s.chars().take(head_len).collect();
    let tail: String = s.chars().skip(n - keep_tail).collect();
    format!("{head}…{tail}")
}

/// `.` and `..` folded without the disk (`/a/b/../c` → `/a/c`).
pub fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// A path whose read is a hard rule: keys, tokens, credentials (design §3
/// tier 0). Examples and templates of `.env` are not secrets.
pub fn secret(p: &Path) -> bool {
    let comps: Vec<String> = p
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let dirs = [".ssh", ".aws", ".gnupg", ".kube", "Keychains"];
    if comps.iter().any(|c| dirs.contains(&c.as_str())) {
        return true;
    }
    let pairs = [
        (".docker", "config.json"),
        ("gh", "hosts.yml"),
        (".config", "gcloud"),
    ];
    if comps
        .windows(2)
        .any(|w| pairs.iter().any(|(a, b)| w[0] == *a && w[1] == *b))
    {
        return true;
    }
    let Some(name) = comps.last() else {
        return false;
    };
    let name = name.as_str();
    let files = [
        ".netrc",
        ".npmrc",
        ".pypirc",
        "auth.json",
        "credentials",
        "credentials.json",
        ".git-credentials",
        ".pgpass",
        ".env",
    ];
    if files.contains(&name) {
        return true;
    }
    if let Some(rest) = name.strip_prefix(".env.") {
        return !matches!(
            rest,
            "example" | "sample" | "template" | "dist" | "defaults"
        );
    }
    let keys = ["id_rsa", "id_ed25519", "id_ecdsa", "id_dsa"];
    if keys.iter().any(|k| name.starts_with(k)) {
        return true;
    }
    [".pem", ".p12", ".pfx", ".key"]
        .iter()
        .any(|e| name.ends_with(e))
}
