//! Every file and socket of computer use, from one bise home. Tests build
//! a `Paths` on a temp folder; the commands use [`Paths::from_env`].

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    /// `~/.bise/run` (`$BEND_RUN_DIR`)
    pub run: PathBuf,
    /// `~/.bise`: the shim goes in its `bin/`
    pub root: PathBuf,
    /// `$HOME`: the browsers' `Library/Application Support/...`
    pub home: PathBuf,
    /// the helper's socket (C5), `<run>/computer-use-app.sock` unless set
    pub app_socket: PathBuf,
}

impl Paths {
    pub fn from_env() -> Paths {
        let h = bise_home::Home::from_env();
        Paths::new(h.run_dir(), h.root(), h.user_home())
    }

    pub fn new(run: impl Into<PathBuf>, root: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Paths {
        let run = run.into();
        // a run dir too long for a unix socket: both sockets are reached
        // through one short link to it (`bise_home::socket`; the broker
        // makes it at its start, before the helper binds)
        let app_socket = bise_home::socket::socket_path(&run.join("computer-use-app.sock"));
        Paths { app_socket, run, root: root.into(), home: home.into() }
    }

    /// C3: the agents' (and the relays') socket: `<run>/computer-use.sock`,
    /// or its short path when that is too long.
    pub fn socket(&self) -> PathBuf {
        bise_home::socket::socket_path(&self.run.join("computer-use.sock"))
    }

    /// The commands' socket (docs/issues/18): the user's processes only;
    /// the agents' sandbox denies it (`bise_home::socket::computer_use_ctl`).
    pub fn ctl_socket(&self) -> PathBuf {
        bise_home::socket::computer_use_ctl(&self.run)
    }

    /// Make [`Paths::socket`], [`Paths::ctl_socket`] and the default
    /// `app_socket` bindable (the short link to `run`, when needed).
    pub fn prepare_sockets(&self) -> std::io::Result<()> {
        bise_home::socket::prepare_socket(&self.run.join("computer-use.sock"))?;
        bise_home::socket::prepare_socket(&self.run.join(bise_home::socket::COMPUTER_USE_CTL))?;
        bise_home::socket::prepare_socket(&self.run.join("computer-use-app.sock"))?;
        Ok(())
    }

    /// C6: `state.json`, `events.jsonl`, the broker's lock and log.
    pub fn dir(&self) -> PathBuf {
        self.run.join("computer-use")
    }

    pub fn state_file(&self) -> PathBuf {
        self.dir().join("state.json")
    }

    pub fn events_file(&self) -> PathBuf {
        self.dir().join("events.jsonl")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.dir().join("broker.lock")
    }

    pub fn log_file(&self) -> PathBuf {
        self.dir().join("broker.log")
    }

    /// The last `live-test` result (setup-check shows it).
    pub fn live_test_file(&self) -> PathBuf {
        self.dir().join("live-test.json")
    }

    /// C4: the native host shim, `~/.bise/bin/bise-chrome-host`.
    pub fn shim(&self) -> PathBuf {
        self.root.join("bin").join("bise-chrome-host")
    }

    /// `~/Library/Application Support`.
    pub fn app_support(&self) -> PathBuf {
        self.home.join("Library").join("Application Support")
    }

    /// The run dir and ours, private (0700).
    pub fn ensure(&self) -> std::io::Result<()> {
        for d in [&self.run, &self.dir()] {
            std::fs::create_dir_all(d)?;
            private(d, 0o700)?;
        }
        Ok(())
    }
}

pub fn private(p: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))
}
