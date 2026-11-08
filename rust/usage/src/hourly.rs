//! The sender's hourly thread, one line in each hub and in the app's core
//! ([`spawn_hourly`]). Every hub of the machine and the core run it; the
//! flock in [`crate::sender::tick`] makes one of them the sender each
//! hour, so a user with the terminal only is covered too. Its own named
//! thread: curl's 10 s never run on a hub's or the core's loop.
//!
//! The edge of the sender: it reads the facts ([`Facts::read`]), the
//! clock, the random bytes (`/dev/urandom`) and the test endpoint
//! (`BISE_TELEMETRY_URL`), and posts through [`Curl`]. Until step 9 turns
//! [`crate::SENDING_ON`] on, every tick ends at the gate, writing nothing.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bise_home::release::{root_kind, DesktopChannel, RootKind};
use bise_home::Home;

use crate::curl::Curl;
use crate::packet::{Arch, Build, Channel};
use crate::send::{endpoint, URL_VAR};
use crate::sender::{tick, Ctx, Tick};
use crate::sharing::{sharing_allowed, Facts};

/// The first tick waits this long after the start (a boot does enough).
pub const FIRST_TICK: Duration = Duration::from_secs(120);

/// Then one tick an hour.
pub const EVERY: Duration = Duration::from_secs(3600);

/// The thread's name.
pub const THREAD: &str = "usage-sender";

impl Build {
    /// This build: its version id and channel from the caller, macOS's
    /// major version from `sw_vers`, the CPU from the compiler.
    pub fn this(version: &str, channel: Channel) -> Build {
        Build { version: version.to_string(), channel, os_major: os_major().unwrap_or(0), arch: Arch::of(std::env::consts::ARCH) }
    }

    /// The build of the app root `root`: its `VERSION`'s id and
    /// [`Channel::read`].
    pub fn of_root(home: &Home, root: &Path) -> Build {
        Build::this(bise_home::release::read_version(root).get("id").map_or("unknown", String::as_str), Channel::read(home, root))
    }
}

impl Channel {
    /// The channel of a root of kind `kind` when he follows `desktop`: a
    /// dev tree is [`Channel::Dev`] (it never shares); any install is
    /// beta when he follows the desktop's beta (Settings › General), else
    /// stable.
    pub fn of(kind: &RootKind, desktop: DesktopChannel) -> Channel {
        match (kind, desktop) {
            (RootKind::Dev, _) => Channel::Dev,
            (RootKind::Install(_) | RootKind::Homebrew | RootKind::Nix, DesktopChannel::Beta) => Channel::Beta,
            (RootKind::Install(_) | RootKind::Homebrew | RootKind::Nix, DesktopChannel::Stable) => Channel::Stable,
        }
    }

    /// [`Channel::of`] the app root `root` (the runtime a hub or the core
    /// runs from) and `home`'s `prefs.json` (`desktop_channel`). The
    /// collectors pass it to [`Facts::read`] too.
    pub fn read(home: &Home, root: &Path) -> Channel {
        let prefs = std::fs::read_to_string(home.prefs_file()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let desktop = DesktopChannel::from_pref(prefs.as_ref().and_then(|p| p.get("desktop_channel")).and_then(serde_json::Value::as_str));
        Channel::of(&root_kind(root), desktop)
    }
}

fn os_major() -> Option<u16> {
    let out = std::process::Command::new("sw_vers").arg("-productVersion").output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().split('.').next()?.parse().ok()
}

fn random16() -> [u8; 16] {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// One tick of this process, as the thread runs it; its log line if any.
pub fn tick_now(home: &Home, build: &Build, offscreen: bool) -> Option<String> {
    let env = |k: &str| std::env::var(k).ok();
    let gate = sharing_allowed(&Facts::read(home, &env, build.channel, offscreen));
    let to = endpoint(bise_home::env::test_setting(URL_VAR));
    let r = random16();
    let jitter = f64::from(u32::from_le_bytes([r[0], r[1], r[2], r[3]])) / (f64::from(u32::MAX) + 1.0);
    let mut random = random16;
    let mut ctx = Ctx { build, endpoint: &to, transport: &mut Curl, random: &mut random, jitter };
    match tick(home, now_secs(), gate, &mut ctx) {
        Tick::Busy => None,
        Tick::Ran(report) => report.log_line(),
    }
}

/// Start the hourly sender of this process (a hub, or the app's core with
/// its `offscreen`), its build read from `app_root` on its own thread;
/// `log` gets the lines worth the local log, never with the install id.
pub fn spawn_hourly(home: Home, app_root: PathBuf, offscreen: bool, log: impl Fn(&str) + Send + 'static) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name(THREAD.into()).spawn(move || {
        std::thread::sleep(FIRST_TICK);
        let build = Build::of_root(&home, &app_root);
        loop {
            if let Some(line) = tick_now(&home, &build, offscreen) {
                log(&line);
            }
            std::thread::sleep(EVERY);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dev tree never shares, whatever he follows; an installed bise
    /// is beta only when he follows the desktop's beta.
    #[test]
    fn channel_is_dev_for_a_dev_tree_and_beta_only_when_he_follows_it() {
        for desktop in [DesktopChannel::Stable, DesktopChannel::Beta] {
            assert_eq!(Channel::of(&RootKind::Dev, desktop), Channel::Dev);
        }
        for kind in [RootKind::Homebrew, RootKind::Nix] {
            assert_eq!(Channel::of(&kind, DesktopChannel::Stable), Channel::Stable);
            assert_eq!(Channel::of(&kind, DesktopChannel::Beta), Channel::Beta);
        }
    }

    #[test]
    fn read_takes_his_desktop_channel_from_prefs() {
        let root = std::env::temp_dir().join(format!("bise-usage-channel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let home = Home::at(&root);
        // a temp folder is no install: a dev tree, whatever the pref
        std::fs::write(home.prefs_file(), r#"{"desktop_channel":"beta"}"#).unwrap();
        assert_eq!(Channel::read(&home, &root), Channel::Dev);
        let _ = std::fs::remove_dir_all(&root);
    }
}
