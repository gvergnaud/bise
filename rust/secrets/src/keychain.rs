//! The shell around /usr/bin/security: runs the lines [`crate::security`]
//! builds, reads items, deletes them, on one keychain ([`Keychain`]).
//!
//! Two keychains ([`Keychains`], issue 19 step B): bise's own file,
//! `~/.bise/secrets/bise.keychain-db`, where every item goes (the agents'
//! sandbox can't read the file, so it can't read the items either:
//! measured, `security` answers 44 there), and the login keychain (the
//! user's default one, or `BISE_TEST_KEYCHAIN`'s throwaway in tests; a
//! test home without it is refused, [`Keychain::here`]) that holds bise's
//! keychain password (one item) and v2026.10.2-28's items. Before any
//! `security` call, [`Keychains::ready`] reads both states with no
//! window ([`crate::status`]) and does what [`crate::ready::next_step`]
//! says: never a call on a locked keychain, so never a dialog.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ready::{self, Intent, Step};
use crate::security::{self, Answer};
use crate::status::{self, Status};
use crate::ReadError;

/// bise's keychain file, in `secrets/`.
pub const FILE: &str = "bise.keychain-db";
/// The login keychain's item that holds bise's keychain password.
pub const PASSWORD_ACCOUNT: &str = "bise keychain password";
const PASSWORD_LABEL: &str = "bise: its keychain";

/// The test setting naming the throwaway keychain every call goes to.
pub const TEST_KEYCHAIN: &str = "BISE_TEST_KEYCHAIN";
/// The test setting that makes the throwaway keychain answer as a locked
/// one, without running `security`. Tests never lock a real keychain: a
/// locked keychain read from a process in his GUI session makes macOS
/// show an unlock dialog on his screen (it did, main m_13693).
pub const TEST_LOCKED: &str = "BISE_TEST_KEYCHAIN_LOCKED";

/// The keychain bise talks to.
#[derive(Clone, Debug)]
pub struct Keychain {
    /// None: the user's default keychain
    pub file: Option<PathBuf>,
    /// tests: every call answers "locked", `security` never runs
    pub locked: bool,
}

impl Keychain {
    /// This process's keychain: `BISE_TEST_KEYCHAIN`'s when set; else the
    /// user's, but only when HOME is his real home and no test run's
    /// jail is set (a test never reaches his login keychain).
    pub fn here() -> Result<Keychain, String> {
        if let Some(f) = bise_home::env::test_setting(TEST_KEYCHAIN) {
            let locked = bise_home::env::test_setting(TEST_LOCKED).is_some();
            return Ok(Keychain { file: Some(PathBuf::from(f)), locked });
        }
        let real = bise_home::test_home::real_home();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let jailed = bise_home::env::test_setting(bise_home::test_home::JAIL_VAR).is_some();
        if jailed || real.is_none() || home != real {
            return Err(format!("a test home without {TEST_KEYCHAIN}: bise won't touch the user's keychain"));
        }
        Ok(Keychain { file: None, locked: false })
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
        if self.locked {
            return Err(ReadError::Locked);
        }
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
        if self.locked {
            return Err(ReadError::Locked);
        }
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
        if self.locked {
            return;
        }
        let mut c = self.command();
        c.args(["delete-generic-password", "-s", security::SERVICE, "-a", account]);
        self.with_file(&mut c);
        let _ = c.output();
    }

    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }
}

/// bise's keychain and the login keychain.
#[derive(Clone, Debug)]
pub struct Keychains {
    /// bise's own file, `<secrets>/bise.keychain-db`
    pub bise: Keychain,
    /// the user's default keychain, or the test's throwaway
    pub login: Keychain,
}

impl Keychains {
    /// This process's two keychains (refused in a test home without
    /// `BISE_TEST_KEYCHAIN`, as [`Keychain::here`]).
    pub fn here() -> Result<Keychains, String> {
        let login = Keychain::here()?;
        let file = bise_home::Home::from_env().secrets_dir().join(FILE);
        Ok(Keychains { bise: Keychain { file: Some(file), locked: login.locked }, login })
    }

    fn states(&self) -> (Status, Status) {
        if self.login.locked {
            return (Status::Locked, Status::Locked);
        }
        (status::status(self.bise.file()), status::status(self.login.file()))
    }

    /// bise's keychain, ready for `intent`: open (unlocked with its
    /// password when it was locked, made on a first write), or `None`
    /// for a read when it doesn't exist. Never runs `security` on a locked
    /// keychain ([`crate::ready`]).
    pub fn ready(&self, intent: Intent) -> Result<Option<&Keychain>, ReadError> {
        let (b, l) = self.states();
        match ready::next_step(intent, b, l) {
            Step::Go => Ok(Some(&self.bise)),
            Step::Absent => Ok(None),
            Step::Locked => Err(ReadError::Locked),
            Step::Unreadable => Err(ReadError::Keychain("bise's keychain can't be read here".into())),
            Step::UnlockWith => {
                let pw = self.login.find(PASSWORD_ACCOUNT)?.ok_or_else(|| ReadError::Keychain("bise's keychain password is not in the login keychain".into()))?;
                status::unlock(self.bise_file(), &pw).map_err(ReadError::Keychain)?;
                Ok(Some(&self.bise))
            }
            Step::Create => {
                self.create()?;
                Ok(Some(&self.bise))
            }
        }
    }

    /// The login keychain, ready to read a stub of before (no call on it
    /// locked); `None`: there is none.
    pub fn login_ready(&self) -> Result<Option<&Keychain>, ReadError> {
        let (_, l) = self.states();
        match ready::login_step(l) {
            Step::Go => Ok(Some(&self.login)),
            Step::Locked => Err(ReadError::Locked),
            Step::Absent => Ok(None),
            _ => Err(ReadError::Keychain("the login keychain can't be read here".into())),
        }
    }

    fn bise_file(&self) -> &Path {
        self.bise.file().unwrap_or(Path::new(FILE))
    }

    /// Make bise's keychain: a random password, written to the login
    /// keychain and read back first (never a keychain whose password is
    /// lost), then the file, unlocked, with no lock timeout and no lock
    /// on sleep (it locks at logout; then [`Keychains::ready`] unlocks it
    /// with the password). Everything on `security -i`'s standard input.
    fn create(&self) -> Result<(), ReadError> {
        let file = self.bise_file();
        if let Some(d) = file.parent() {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d);
        }
        // one maker at a time (two processes' first write): the second
        // finds it made
        let mut lock = file.as_os_str().to_owned();
        lock.push(".lock");
        let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock).map_err(ReadError::Io)?;
        lock.lock().map_err(ReadError::Io)?;
        if status::status(Some(file)) == Status::Unlocked {
            return Ok(());
        }
        let pw = crate::random_hex(32);
        let add = security::add_line(PASSWORD_ACCOUNT, PASSWORD_LABEL, &pw, self.login.file())
            .ok_or_else(|| ReadError::Keychain("the login keychain's path holds a control character".into()))?;
        self.login.run(&[add])?;
        if self.login.find(PASSWORD_ACCOUNT)?.as_deref() != Some(pw.as_str()) {
            return Err(ReadError::Keychain("the login keychain didn't keep bise's keychain password".into()));
        }
        let f = security::quote(&file.display().to_string()).ok_or_else(|| ReadError::Keychain("bise's keychain path holds a control character".into()))?;
        self.bise.run(&[format!("create-keychain -p {pw} {f}"), format!("set-keychain-settings {f}")])?;
        match status::status(Some(file)) {
            Status::Unlocked => Ok(()),
            s => Err(ReadError::Keychain(format!("bise's keychain was made but is {s:?}"))),
        }
    }

    /// Forget bise's keychain (`off`, every secret moved back to files):
    /// its file and its password item. Best effort.
    pub fn forget(&self) {
        if self.login.locked {
            return;
        }
        if let Some(f) = self.bise.file() {
            if f.exists() {
                if let Some(q) = security::quote(&f.display().to_string()) {
                    let _ = self.bise.run(&[format!("delete-keychain {q}")]);
                }
                let _ = std::fs::remove_file(f);
            }
            let mut lock = f.as_os_str().to_owned();
            lock.push(".lock");
            let _ = std::fs::remove_file(lock);
        }
        if status::status(self.login.file()) == Status::Unlocked {
            self.login.delete(PASSWORD_ACCOUNT);
        }
    }
}
