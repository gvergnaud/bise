//! The secrets a sandboxed command never reads (docs/issues/19): bise's
//! own (`auth.json`, `secrets/`, from their owner `bise_home`) and the
//! private keys in `~/.ssh`. ONE list, [`Secrets::of`], read by the three
//! places that need it: the profile's read deny ([`Secrets::profile_rules`],
//! `sandbox::profile`), the denial reader (a stopped command that named one:
//! [`Secrets::named_in`], never a card or a rerun without the sandbox) and
//! the command's words ([`Secrets::named_by`], `sandbox::allow_flags`: a
//! command that names one never skips the sandbox). Law:
//! `secrets_tests::law_the_three_uses_read_one_list`.
//!
//! `~/.ssh` is closed but an allow-list, exact names, of what ssh and git
//! read besides the keys ([`SSH_READABLE`], [`SSH_READABLE_DIRS`],
//! `*.pub`): a key with any other name is covered; git over ssh needs the
//! key in ssh-agent, not its file (measured, issue 19). `held` and the
//! profile agree on a name table under the real `sandbox-exec`
//! (`sandbox_tests::live::law_held_and_the_profile_agree_on_ssh_names`).
//!
//! Pure: paths only, the disk through [`Fs`].

use std::path::{Path, PathBuf};

use super::paths::{fold, Fs, Roots};

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
    /// both real for the profile: Seatbelt matches real paths).
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

    /// `held` of a word as the command or its output wrote it: `~`,
    /// `$HOME` expanded, relative to the agent's folder, `..` folded; its
    /// real path too (a link to a secret is one).
    fn held_word(&self, roots: &Roots, word: &str, fs: &dyn Fs) -> Option<(PathBuf, Held)> {
        let word = word.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`');
        if word.is_empty() {
            return None;
        }
        let home = roots.home.to_string_lossy();
        let word = ["$HOME", "${HOME}"]
            .iter()
            .find_map(|v| word.strip_prefix(v).filter(|r| r.is_empty() || r.starts_with('/')).map(|r| format!("{home}{r}")))
            .unwrap_or_else(|| word.to_string());
        let lexical = roots.resolve(Some(&roots.cwd), &word, &super::paths::LexicalFs)?;
        let lexical = fold(&lexical);
        [lexical.clone(), fs.real(&lexical)].into_iter().find_map(|p| self.held(&p).map(|h| (p, h)))
    }

    /// The secret a stopped command's output names on an "Operation not
    /// permitted" line (`cat: x: Operation…`, python's `…: '/x'`, ssh's
    /// `Load key "/x": Operation…`), shown as the card shows paths.
    pub fn named_in(&self, output: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)> {
        output
            .lines()
            .filter(|l| l.to_lowercase().contains("operation not permitted"))
            .flat_map(|l| {
                let quoted: Vec<String> = l.split(['"', '\'']).skip(1).step_by(2).map(str::to_string).collect();
                l.split(": ").map(str::to_string).chain(quoted).collect::<Vec<_>>()
            })
            .find_map(|w| self.held_word(roots, &w, fs))
            .map(|(p, h)| (roots.show(&p), h))
    }

    /// The secret a command's words name, if any (lexical: shell
    /// punctuation splits words; a false match only keeps it sandboxed).
    pub fn named_by(&self, cmd: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)> {
        cmd.split(|c: char| c.is_whitespace() || "'\"`;|&<>()=,".contains(c))
            .find_map(|w| self.held_word(roots, w, fs))
            .map(|(p, h)| (roots.show(&p), h))
    }

    /// The profile's read deny (SBPL), after `(allow default)`: the last
    /// match wins, so nothing later opens them. `lit`: a path as an
    /// SBPL literal.
    pub fn profile_rules(&self, lit: &dyn Fn(&Path) -> String) -> String {
        let mut o = String::from("(deny file-read-data\n");
        for f in &self.files {
            o.push_str(&format!("  (literal {})\n", lit(f)));
        }
        for d in &self.folders {
            o.push_str(&format!("  (subpath {})\n", lit(d)));
        }
        let ssh = &self.ssh;
        o.push_str(&format!("  (require-all (subpath {}) (require-not (literal {}))\n", lit(ssh), lit(ssh)));
        for f in SSH_READABLE {
            o.push_str(&format!("    (require-not (literal {}))\n", lit(&ssh.join(f))));
        }
        for d in SSH_READABLE_DIRS {
            o.push_str(&format!("    (require-not (subpath {}))\n", lit(&ssh.join(d))));
        }
        o.push_str(&format!("    (require-not (regex #\"{}$\"))))\n", SSH_PUBLIC.replace('.', "\\.")));
        o
    }

    /// The write deny's entries (SBPL): an agent never replaces or
    /// deletes a secret either.
    pub fn write_rules(&self, lit: &dyn Fn(&Path) -> String) -> Vec<String> {
        let files = self.files.iter().map(|f| format!("(literal {})", lit(f)));
        let folders = self.folders.iter().map(|d| format!("(subpath {})", lit(d)));
        files.chain(folders).collect()
    }
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod secrets_tests;
