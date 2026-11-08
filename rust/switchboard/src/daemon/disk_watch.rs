//! The free space of the volume holding bise's home, read on the hub's tick (disk-hygiene,
//! point 8; architect m_56051: one `statvfs`, on the loop, no thread of its
//! own). Its whole GB goes to sb-core as `Input::DiskFree` only when it
//! changed; sb-core decides whether main hears of it (hub/disk.bend: once
//! under 40 GB, again only after 45). The shell keeps the last GB it sent,
//! nothing else.

use super::Shell;
use crate::core::Input;
use std::path::Path;

impl Shell {
    /// On each tick: the free GB, to sb-core when it changed.
    pub(super) fn disk_check(&mut self) {
        // the volume of bise's home: the hub's state is in it (Paths, bise_home::hub_state)
        let Some(gb) = free_gb(&self.opts.paths.state) else { return };
        if let Some(gb) = self.disk.next(gb) {
            self.step(Input::DiskFree { gb });
        }
    }
}

/// The last whole GB sent to sb-core.
#[derive(Default)]
pub(super) struct DiskWatch {
    sent: Option<u64>,
}

impl DiskWatch {
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
pub(super) fn free_gb(path: &Path) -> Option<u64> {
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

    #[test]
    fn a_real_volume_reads() {
        assert!(free_gb(Path::new("/")).is_some());
        assert_eq!(free_gb(Path::new("/no/such/volume")), None);
    }
}
