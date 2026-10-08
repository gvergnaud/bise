//! The shell around /usr/bin/security: runs the lines [`crate::security`]
//! builds, reads items, deletes them. Which keychain: the user's default
//! one (his login keychain), or `BISE_TEST_KEYCHAIN`'s throwaway file in
//! tests; a test home without it is refused ([`Keychain::here`]).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::security::{self, Answer};
use crate::ReadError;

/// The test setting naming the throwaway keychain every call goes to.
pub const TEST_KEYCHAIN: &str = "BISE_TEST_KEYCHAIN";

/// The keychain bise talks to.
#[derive(Clone, Debug)]
pub struct Keychain {
    /// None: the user's default keychain
    pub file: Option<PathBuf>,
}

impl Keychain {
    /// This process's keychain: `BISE_TEST_KEYCHAIN`'s when set; else the
    /// user's, but only when HOME is his real home and no test run's
    /// jail is set (a test never reaches his login keychain).
    pub fn here() -> Result<Keychain, String> {
        if let Some(f) = bise_home::env::test_setting(TEST_KEYCHAIN) {
            return Ok(Keychain { file: Some(PathBuf::from(f)) });
        }
        let real = bise_home::test_home::real_home();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let jailed = bise_home::env::test_setting(bise_home::test_home::JAIL_VAR).is_some();
        if jailed || real.is_none() || home != real {
            return Err(format!("a test home without {TEST_KEYCHAIN}: bise won't touch the user's keychain"));
        }
        Ok(Keychain { file: None })
    }

    fn command(&self) -> Command {
        let mut c = Command::new(security::PROGRAM);
        c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        c
    }

    fn with_file(&self, c: &mut Command) {
        if let Some(f) = &self.file {
            c.arg(f);
        }
    }

    /// One item's data; None: no such item. Arguments hold names only.
    pub fn find(&self, account: &str) -> Result<Option<String>, ReadError> {
        let mut c = self.command();
        c.args(["find-generic-password", "-s", security::SERVICE, "-a", account, "-w"]);
        self.with_file(&mut c);
        let out = c.output().map_err(|e| ReadError::Keychain(format!("cannot run {}: {e}", security::PROGRAM)))?;
        match security::answer(out.status.code(), &String::from_utf8_lossy(&out.stderr)) {
            Answer::Ok => Ok(Some(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']).to_string())),
            Answer::Absent => Ok(None),
            Answer::Locked => Err(ReadError::Locked),
            Answer::Failed(e) => Err(ReadError::Keychain(e)),
        }
    }

    /// Run `lines` in one `security -i`, on its standard input: the only
    /// way a secret reaches it.
    pub fn run(&self, lines: &[String]) -> Result<(), ReadError> {
        let mut c = self.command();
        c.arg("-i").stdin(Stdio::piped());
        let mut child = c.spawn().map_err(|e| ReadError::Keychain(format!("cannot run {}: {e}", security::PROGRAM)))?;
        let mut input = lines.join("\n");
        input.push('\n');
        if let Some(mut w) = child.stdin.take() {
            let _ = w.write_all(input.as_bytes());
        }
        let out = child.wait_with_output().map_err(|e| ReadError::Keychain(e.to_string()))?;
        match security::answer(out.status.code(), &String::from_utf8_lossy(&out.stderr)) {
            Answer::Ok => Ok(()),
            Answer::Locked => Err(ReadError::Locked),
            Answer::Absent => Err(ReadError::Keychain("security: an item could not be found".into())),
            Answer::Failed(e) => Err(ReadError::Keychain(e)),
        }
    }

    /// Delete one item; best effort (none is fine).
    pub fn delete(&self, account: &str) {
        let mut c = self.command();
        c.args(["delete-generic-password", "-s", security::SERVICE, "-a", account]);
        self.with_file(&mut c);
        let _ = c.output();
    }

    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }
}
