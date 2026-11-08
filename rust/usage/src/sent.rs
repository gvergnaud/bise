//! `sent/` (`Home::usage_sent_dir`): the packets the server took, kept
//! [`KEEP_DAYS`] days so he can see what was sent ("See what's sent",
//! `bise telemetry show`), in clear. One folder per day of sending, one
//! `<packet day>.json` in it: a packet sent late (offline for a while)
//! still stays its 7 days.

use std::io;
use std::path::{Path, PathBuf};

use crate::day::Day;
use crate::packet::Packet;
use crate::spool::{days_in, prune as prune_days, write_atomic};

/// How many days a sent packet is kept, the day it was sent included.
pub const KEEP_DAYS: u32 = 7;

fn file(sent_dir: &Path, sent_on: Day, day: Day) -> PathBuf {
    sent_dir.join(sent_on.to_string()).join(format!("{day}.json"))
}

/// Keep a packet the server took on `sent_on`.
pub fn keep(sent_dir: &Path, sent_on: Day, p: &Packet) -> io::Result<()> {
    write_atomic(&file(sent_dir, sent_on, p.day), &serde_json::to_vec_pretty(p)?)
}

/// Whether the packet of `day` is among those kept: a spool folder of that
/// day that shows up again (a late write) is not sent twice.
pub fn holds(sent_dir: &Path, day: Day) -> bool {
    days_in(sent_dir).into_iter().any(|on| file(sent_dir, on, day).exists())
}

/// Delete what was sent more than [`KEEP_DAYS`] days before `today`.
pub fn prune(sent_dir: &Path, today: Day) {
    prune_days(sent_dir, today, KEEP_DAYS);
}

/// The packets kept, as `(sent on, packet day, its text)`, oldest first.
pub fn list(sent_dir: &Path) -> Vec<(Day, Day, String)> {
    let mut out = Vec::new();
    for on in days_in(sent_dir) {
        let Ok(rd) = std::fs::read_dir(sent_dir.join(on.to_string())) else { continue };
        let mut here: Vec<(Day, String)> = rd
            .flatten()
            .filter_map(|e| {
                let day = Day::parse(e.file_name().to_str()?.strip_suffix(".json")?)?;
                Some((day, std::fs::read_to_string(e.path()).ok()?))
            })
            .collect();
        here.sort();
        out.extend(here.into_iter().map(|(day, text)| (on, day, text)));
    }
    out
}
