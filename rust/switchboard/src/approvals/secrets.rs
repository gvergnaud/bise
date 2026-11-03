//! The secrets a sandboxed command never reads (docs/issues/19): the list
//! is `bise_secrets::held` ([`Secrets::of`]: bise's `auth.json` and
//! `secrets/`, the private keys in `~/.ssh`), re-exported here; its owner
//! is that crate because computer use's uploads refuse the same paths.
//! This module holds its three readers in the sandbox ([`SecretsUse`]):
//! the profile's read deny (`profile_rules`, `sandbox::profile`), the
//! denial reader (a stopped command that named one: `named_in`, never a
//! card or a rerun without the sandbox) and the command's words
//! (`named_by`, `sandbox::allow_flags`: a command that names one never
//! skips the sandbox). Law: `secrets_tests::law_the_three_uses_read_one_list`.
//!
//! `held` and the profile agree on a name table under the real
//! `sandbox-exec` (`sandbox_tests::live::law_held_and_the_profile_agree_on_ssh_names`).
//!
//! Pure: paths only, the disk through [`Fs`].

use std::path::{Path, PathBuf};

use super::paths::{fold, Fs, Roots};

// the list itself, its one owner: bise_secrets::held (computer use's
// uploads read it too, architect m_52652)
pub use bise_secrets::held::{Held, Secrets, SSH_PUBLIC, SSH_READABLE, SSH_READABLE_DIRS};

/// The list's readers in the sandbox: a word of a command or its output
/// that names a secret, and the profile's rules.
pub trait SecretsUse {
    fn named_in(&self, output: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)>;
    fn named_by(&self, cmd: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)>;
    fn profile_rules(&self, lit: &dyn Fn(&Path) -> String) -> String;
    fn write_rules(&self, lit: &dyn Fn(&Path) -> String) -> Vec<String>;
}

/// `held` of a word as the command or its output wrote it: `~`,
/// `$HOME` expanded, relative to the agent's folder, `..` folded; its
/// real path too (a link to a secret is one).
fn held_word(s: &Secrets, roots: &Roots, word: &str, fs: &dyn Fs) -> Option<(PathBuf, Held)> {
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
    [lexical.clone(), fs.real(&lexical)].into_iter().find_map(|p| s.held(&p).map(|h| (p, h)))
}

impl SecretsUse for Secrets {
    /// The secret a stopped command's output names on an "Operation not
    /// permitted" line (`cat: x: Operation…`, python's `…: '/x'`, ssh's
    /// `Load key "/x": Operation…`), shown as the card shows paths.
    fn named_in(&self, output: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)> {
        output
            .lines()
            .filter(|l| l.to_lowercase().contains("operation not permitted"))
            .flat_map(|l| {
                let quoted: Vec<String> = l.split(['"', '\'']).skip(1).step_by(2).map(str::to_string).collect();
                l.split(": ").map(str::to_string).chain(quoted).collect::<Vec<_>>()
            })
            .find_map(|w| held_word(self, roots, &w, fs))
            .map(|(p, h)| (roots.show(&p), h))
    }

    /// The secret a command's words name, if any (lexical: shell
    /// punctuation splits words; a false match only keeps it sandboxed).
    fn named_by(&self, cmd: &str, roots: &Roots, fs: &dyn Fs) -> Option<(String, Held)> {
        cmd.split(|c: char| c.is_whitespace() || "'\"`;|&<>()=,".contains(c))
            .find_map(|w| held_word(self, roots, w, fs))
            .map(|(p, h)| (roots.show(&p), h))
    }

    /// The profile's read deny (SBPL), after `(allow default)`: the last
    /// match wins, so nothing later opens them. `lit`: a path as an
    /// SBPL literal.
    fn profile_rules(&self, lit: &dyn Fn(&Path) -> String) -> String {
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
    fn write_rules(&self, lit: &dyn Fn(&Path) -> String) -> Vec<String> {
        let files = self.files.iter().map(|f| format!("(literal {})", lit(f)));
        let folders = self.folders.iter().map(|d| format!("(subpath {})", lit(d)));
        files.chain(folders).collect()
    }
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod secrets_tests;
