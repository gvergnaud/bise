//! The local clock's time zone, one reader for every bise binary
//! (architect m_11673): the TUI's times, `sb every`'s daily timers and the
//! hub's thread fold (a scheduled task's clock times) all ask here, so
//! they agree on the zone. bise-proto stays pure: its callers pass
//! [`offset_at`] in.

/// The local time at `ms` (since the epoch), from the C library's
/// `localtime_r` (the machine's zone, TZ included). None when it can't say.
pub fn local_tm(ms: u64) -> Option<libc::tm> {
    let t = (ms / 1000) as libc::time_t;
    // SAFETY: `tm` is ours and zeroed (a valid `libc::tm`); localtime_r
    // only writes into it and returns null on failure, never keeps it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    (!r.is_null()).then_some(tm)
}

/// The local UTC offset at `ms`, in seconds east (`+0200` is 7200); 0
/// (UTC) when the C library can't say.
pub fn offset_at(ms: u64) -> i32 {
    local_tm(ms).map_or(0, |tm| tm.tm_gmtoff as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offset is the one `date +%z` says for the same moment (the
    /// TUI's reader before this one), and a whole number of minutes.
    #[test]
    fn the_offset_is_the_systems() {
        let ms = 1_790_000_000_000u64;
        let o = offset_at(ms);
        assert_eq!(o % 60, 0, "{o}");
        assert!((-14 * 3600..=14 * 3600).contains(&o), "{o}");
        let secs = (ms / 1000).to_string();
        let date = std::process::Command::new("date").args(["-r", &secs, "+%z"]).output();
        if let Some(out) = date.ok().filter(|o| o.status.success()) {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let (sign, d) = s.split_at(1);
            let (h, m): (i32, i32) = (d[..2].parse().unwrap(), d[2..4].parse().unwrap());
            let want = if sign == "-" { -(h * 3600 + m * 60) } else { h * 3600 + m * 60 };
            assert_eq!(o, want, "date says {s}");
        }
        assert!(local_tm(ms).is_some());
    }
}
