//! The secrets no agent reads (docs/issues/19): bise's own (`auth.json`,
//! `secrets/`, from their owner `bise_home`) and the private keys in
//! `~/.ssh`. ONE list, [`Secrets::of`], with its two readers: the
//! sandbox of an agent's commands (switchboard `approvals::secrets`,
//! `approvals::sandbox`) and computer use's uploads
//! (`bise_computer_use::files`: an agent can't attach to a web page a file
//! its sandbox denies it).
//!
//! `~/.ssh` is closed but an allow-list, exact names, of what ssh and git
//! read besides the keys ([`SSH_READABLE`], [`SSH_READABLE_DIRS`],
//! `*.pub`): a key with any other name is covered.
//!
//! Pure: paths only, folded and absolute; the callers resolve links.

use std::path::{Path, PathBuf};

/// Files at the top of `~/.ssh` a sandboxed command still reads, exact
/// names (architect m_13613: a prefix would open `config-work`): ssh's
/// config, the known hosts and ssh-keygen's backup of them, the public-key
/// lists sshd reads.
pub const SSH_READABLE: &[&str] = &["config", "known_hosts", "known_hosts.old", "authorized_keys", "authorized_keys2"];
/// Folders of `~/.ssh` a command still reads, whole: `Include`d
/// configs, ssh-agent sockets.
pub const SSH_READABLE_DIRS: &[&str] = &["config.d", "agent"];
/// Public keys, anywhere under `~/.ssh`.
pub const SSH_PUBLIC: &str = ".pub";

/// Whose a secret is: the refusal line says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// bise's keys and sign-ins (`auth.json`, `secrets/`).
    Bise,
    /// an ssh private key.
    Ssh,
}

/// The secret paths of one agent's homes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secrets {
    /// Files: `auth.json`.
    pub files: Vec<PathBuf>,
    /// Folders, whole: `secrets/`.
    pub folders: Vec<PathBuf>,
    /// `~/.ssh`, but [`SSH_READABLE`].
    pub ssh: PathBuf,
}

impl Secrets {
    /// The list for bise's home `bise` and the user's home `home` (give
    /// both real for the sandbox's profile: Seatbelt matches real paths).
    pub fn of(bise: &Path, home: &Path) -> Secrets {
        let h = bise_home::Home::at(bise);
        Secrets { files: vec![h.auth_file()], folders: vec![h.secrets_dir()], ssh: home.join(".ssh") }
    }

    /// Whose secret `p` is (a folded absolute path), if it is one.
    pub fn held(&self, p: &Path) -> Option<Held> {
        if self.files.iter().any(|f| p == f) || self.folders.iter().any(|d| p.starts_with(d)) {
            return Some(Held::Bise);
        }
        let rel = p.strip_prefix(&self.ssh).ok()?;
        let first = rel.components().next()?.as_os_str().to_string_lossy();
        let readable = SSH_READABLE.iter().any(|r| rel == Path::new(r))
            || SSH_READABLE_DIRS.contains(&first.as_ref())
            || p.to_string_lossy().ends_with(SSH_PUBLIC);
        (!readable).then_some(Held::Ssh)
    }
}
