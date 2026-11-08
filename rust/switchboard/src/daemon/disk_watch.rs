//! The free space of the volume holding bise's home, read on the hub's tick (disk-hygiene,
//! point 8; architect m_56051: one `statvfs`, on the loop, no thread of its
//! own). Its whole GB goes to sb-core as `Input::DiskFree` only when it
//! changed; sb-core decides whether main hears of it (hub/disk.bend: once
//! under 40 GB, again only after 45). The shell keeps where it reads the
//! GB from and the last GB it sent, nothing else.
//!
//! Where it reads from is the hub's world, never assumed: the test setting
//! `BISE_DISK_FREE_GB` (read once, at the Shell's build) fakes the reading
//! (a number) or turns it off (`off`); unset, the host's volume. A test hub
//! never reads the host's disk (tests/bise_env.py's clean_env sets `off`):
//! the real Mac's free GB reached a test main as a note and broke
//! tui_reload_tmux (f4e148a82, architect m_56460).

use super::Shell;
use crate::core::Input;
use std::path::Path;

/// `BISE_DISK_FREE_GB`: the reading's test setting.
const ENV: &str = "BISE_DISK_FREE_GB";

impl Shell {
    /// On each tick: the free GB, to sb-core when it changed.
    pub(super) fn disk_check(&mut self) {
        // the volume of bise's home: the hub's state is in it (Paths, bise_home::hub_state)
        let Some(gb) = self.disk.source.read(&self.opts.paths.state) else { return };
        if let Some(gb) = self.disk.next(gb) {
            self.step(Input::DiskFree { gb });
        }
    }
}

/// Where the free GB come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DiskSource {
    /// The host's volume (`statvfs`).
    #[default]
    Host,
    /// A test's fake reading.
    Fake(u64),
    /// No reading: sb-core never hears of the disk.
    Off,
}

impl DiskSource {
    /// The source `BISE_DISK_FREE_GB` names: unset = the host, a number = that
    /// fake GB, anything else (`off`) = no reading (a typo never reads the host).
    fn from_setting(v: Option<&str>) -> DiskSource {
        match v.map(str::trim) {
            None => DiskSource::Host,
            Some(v) => v.parse().map_or(DiskSource::Off, DiskSource::Fake),
        }
    }

    /// The free GB of the volume holding `path`, as this source gives them.
    fn read(self, path: &Path) -> Option<u64> {
        match self {
            DiskSource::Host => free_gb(path),
            DiskSource::Fake(gb) => Some(gb),
            DiskSource::Off => None,
        }
    }
}

/// Where the GB come from and the last whole GB sent to sb-core.
#[derive(Default)]
pub(super) struct DiskWatch {
    source: DiskSource,
    sent: Option<u64>,
}

impl DiskWatch {
    /// The watch of the hub's world: its source from `BISE_DISK_FREE_GB`.
    pub(super) fn from_env() -> DiskWatch {
        let source = DiskSource::from_setting(bise_home::env::test_setting(ENV).as_deref());
        DiskWatch { source, sent: None }
    }

    /// The GB to send for a reading of `gb`: `None` when sb-core has it.
    pub(super) fn next(&mut self, gb: u64) -> Option<u64> {
        (self.sent != Some(gb)).then(|| {
            self.sent = Some(gb);
            gb
        })
    }
}

/// The free GB (whole, rounded down) of the volume holding `path`, as an
/// unprivileged user may write them (`f_bavail`); `None` when unreadable.
fn free_gb(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path and `s` a zeroed statvfs the call fills
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some((s.f_bavail as u64).saturating_mul(s.f_frsize as u64) >> 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One input per whole-GB change, never two for the same GB.
    #[test]
    fn only_a_changed_gb_is_sent() {
        let mut w = DiskWatch::default();
        assert_eq!(w.next(41), Some(41));
        assert_eq!(w.next(41), None);
        assert_eq!(w.next(39), Some(39));
        assert_eq!(w.next(39), None);
        assert_eq!(w.next(41), Some(41));
    }

    /// The setting picks the source; only an unset one reads the host.
    #[test]
    fn the_setting_picks_the_source() {
        assert_eq!(DiskSource::from_setting(None), DiskSource::Host);
        assert_eq!(DiskSource::from_setting(Some("30")), DiskSource::Fake(30));
        assert_eq!(DiskSource::from_setting(Some(" 7 ")), DiskSource::Fake(7));
        assert_eq!(DiskSource::from_setting(Some("off")), DiskSource::Off);
        assert_eq!(DiskSource::from_setting(Some("3O")), DiskSource::Off);
    }

    /// A test hub never reads the host's disk: off reads nothing, a fake
    /// its own GB, wherever the state is.
    #[test]
    fn a_test_hub_never_reads_the_host() {
        assert_eq!(DiskSource::Off.read(Path::new("/")), None);
        assert_eq!(DiskSource::Fake(30).read(Path::new("/")), Some(30));
        assert_eq!(DiskSource::Fake(30).read(Path::new("/no/such/volume")), Some(30));
    }

    #[test]
    fn a_real_volume_reads() {
        assert!(DiskSource::Host.read(Path::new("/")).is_some());
        assert_eq!(free_gb(Path::new("/no/such/volume")), None);
    }
}
