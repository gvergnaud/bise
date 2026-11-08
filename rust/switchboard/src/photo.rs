//! The boot photo (issue 36 chunk 4): sb-core's durable state written to
//! a file, so a boot restores it and replays only the journal events after
//! it, instead of every event since the hub began.
//!
//! The journal stays the one durable store: the photo is a cache of it,
//! checked against it before use, and dropped whenever it may not match.
//! One file per sb-core build, `<state>/photo/<key>.json` (the key: a hash
//! of the sb-core binary, so a photo of another state shape is never
//! offered to this one), the last two builds kept (a rollback boots from
//! the other). Its content: `{"v":2, "key", "events": k, "prefix",
//! "paths", "st"}`, where `k` is the number of journal events (every line
//! the hub read, its own PR lines too) the photo holds, `prefix` a chained
//! hash of events 1..k (the journal's first k events unchanged since the
//! photo, not only event k), `paths` a hash of their workspace paths as
//! the boot reads them (a worktree moved since: full replay).
//!
//! sb-core's restore reads any JSON object as its state (St.photo_of is
//! lenient): the key (the same binary wrote it) and the prefix are what
//! keep a wrong photo out. A torn, truncated or non-object file reads as
//! none: a full replay, with its hub.log line.
//!
//! Pure here: the hashes ([`Mark`]), whether a photo fits the journal
//! ([`usable`]), when to take one ([`idle_due`]); the files: [`read`],
//! [`write`] (tmp + fsync + rename), [`clean`]. The hub calls them
//! (daemon.rs boot, crate::core's `boot_from`, daemon/photo_take.rs).

use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The photo file's shape version. 2 (c6): sb-core's messages lost their
/// text and parts (`has_parts` instead): an older hub refuses the new
/// photo and replays the journal, which keeps every message's words.
pub const V: u64 = 2;

/// A photo is taken at most this often when the hub is idle (ms).
pub const IDLE_EVERY_MS: u64 = 24 * 3600 * 1000;
/// ... and only after this long with no input (timers and watches aside).
pub const IDLE_AFTER_MS: u64 = 10 * 60 * 1000;
/// At a clean stop the photo is skipped when the last one took longer to
/// encode than this (the switcher kills a hub 5 s after its socket goes).
pub const STOP_BUDGET_MS: u64 = 2000;

/// FNV-1a, 64 bits: stable across Rust versions (a photo outlives the
/// binary that wrote it), no new dependency. A check, not a secret.
pub fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The key of an sb-core binary (its bytes' hash); none when unreadable.
pub fn key_of(bin: &Path) -> Option<String> {
    std::fs::read(bin).ok().map(|b| format!("{:016x}", fnv(&b)))
}

/// What a photo says of the journal it holds, kept up to date as the hub
/// writes events (the first `events` events, the chained hash of every
/// one of them, the hash of their workspace paths).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mark {
    pub events: usize,
    pub prefix: u64,
    pub paths: u64,
}

impl Mark {
    /// One more journal event.
    pub fn push(&mut self, ev: &Value) {
        self.events += 1;
        let mut b = self.prefix.to_le_bytes().to_vec();
        b.extend_from_slice(ev.to_string().as_bytes());
        self.prefix = fnv(&b);
        if let Some(p) = ev["ws"]["path"].as_str() {
            let mut b = self.paths.to_le_bytes().to_vec();
            b.extend_from_slice(p.as_bytes());
            self.paths = fnv(&b);
        }
    }

    pub fn of(events: &[Value]) -> Mark {
        let mut m = Mark::default();
        for ev in events {
            m.push(ev);
        }
        m
    }
}

/// A photo's head (the file without its state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub key: String,
    pub mark: Mark,
}

/// Whether the boot starts from the photo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Use {
    /// restore it, then replay the events from index `k` on
    Photo { k: usize },
    /// replay every event; `why` goes to hub.log
    Full { why: String },
}

/// Does the photo fit this boot's journal (`events` as the boot read them,
/// worktree moves followed)? Same sb-core key, no more events than the
/// journal holds, events 1..k and their paths the same, and no worktree
/// migration this boot (it rewrote paths the photo holds).
pub fn usable(head: Option<&Head>, key: &str, events: &[Value], migrated: bool) -> Use {
    let full = |why: &str| Use::Full { why: why.to_string() };
    let Some(h) = head else { return full("no photo") };
    if h.key != key {
        return full("a photo of another sb-core");
    }
    if migrated {
        return full("worktrees moved this boot");
    }
    let k = h.mark.events;
    if k == 0 || k > events.len() {
        return full(&format!("the photo holds {} events, the journal {}", k, events.len()));
    }
    let m = Mark::of(&events[..k]);
    if m.prefix != h.mark.prefix {
        return full(&format!("the journal's first {} events are not the photo's", k));
    }
    if m.paths != h.mark.paths {
        return full("a workspace path reads otherwise since the photo");
    }
    Use::Photo { k }
}

/// Whether a tick is a good moment for a photo (architect m_37035): sb-core
/// is one thread, so the encode holds the hub's loop (~350 s at load x5
/// on his -47 boot). Only with nothing waiting in the loop's inbox, no
/// land in line and the machine not loaded (crate::load's one rule);
/// else why not, for the one hub.log line.
pub fn moment(quiet: bool, lands: bool, factor: f64) -> Result<(), String> {
    if !quiet {
        return Err("messages waiting".into());
    }
    if lands {
        return Err("a land in line".into());
    }
    if crate::load::is_loaded(factor) {
        return Err(format!("load x{factor:.1}"));
    }
    Ok(())
}

/// What a tick does about the photo.
#[derive(Debug, PartialEq, Eq)]
pub enum Act {
    Take,
    /// the deferral's one hub.log line
    Say(String),
}

/// A photo waiting for its moment: `due` after a full replay (never taken
/// inside the boot), or the idle one. Not taken, it stays due; never
/// taken, the next boot replays in full again (the honest fallback).
#[derive(Debug, Default)]
pub struct Wait {
    pub due: bool,
    said: bool,
}

impl Wait {
    /// One tick: `idle` an idle photo is due ([`idle_due`]), the moment's
    /// facts as [`moment`] reads them.
    pub fn tick(&mut self, idle: bool, quiet: bool, lands: bool, factor: f64) -> Option<Act> {
        if !(self.due || idle) {
            return None;
        }
        match moment(quiet, lands, factor) {
            Ok(()) => {
                (self.due, self.said) = (false, false);
                Some(Act::Take)
            }
            Err(why) if !self.said => {
                self.said = true;
                Some(Act::Say(format!("photo deferred: {why}")))
            }
            Err(_) => None,
        }
    }
}

/// Is an idle photo due: nothing came in for [`IDLE_AFTER_MS`] and the last
/// photo is older than [`IDLE_EVERY_MS`] (0: never taken this session).
pub fn idle_due(now: u64, last_input: u64, last_photo: u64) -> bool {
    now.saturating_sub(last_input) >= IDLE_AFTER_MS && (last_photo == 0 || now.saturating_sub(last_photo) >= IDLE_EVERY_MS)
}

fn file(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.json"))
}

/// The photo of `key`: its head and its state; none when missing,
/// unreadable or of another file version.
pub fn read(dir: &Path, key: &str) -> Option<(Head, Value)> {
    let text = std::fs::read_to_string(file(dir, key)).ok()?;
    let mut v: Value = serde_json::from_str(&text).ok()?;
    if v["v"].as_u64() != Some(V) {
        return None;
    }
    let mark = Mark {
        events: v["events"].as_u64()? as usize,
        prefix: v["prefix"].as_str().and_then(|p| u64::from_str_radix(p, 16).ok())?,
        paths: v["paths"].as_str().and_then(|p| u64::from_str_radix(p, 16).ok())?,
    };
    let head = Head { key: v["key"].as_str()?.to_string(), mark };
    let st = v.get_mut("st").map(Value::take).filter(Value::is_object)?;
    Some((head, st))
}

/// Write the photo of `key` (tmp + fsync + rename), then keep it and the
/// newest photo of one other key (a rollback's), no other.
pub fn write(dir: &Path, key: &str, mark: &Mark, st: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let head = serde_json::json!({"v": V, "key": key, "events": mark.events, "prefix": format!("{:016x}", mark.prefix), "paths": format!("{:016x}", mark.paths)});
    let mut text = head.to_string();
    text.pop(); // its closing brace: the state goes in the same object
    text.push_str(",\"st\":");
    text.push_str(&st.to_string());
    text.push('}');
    let tmp = dir.join(format!("{key}.json.tmp"));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, file(dir, key))?;
    keep_two(dir, key);
    Ok(())
}

/// The photos of keys other than `key`: the newest stays, the rest go.
fn keep_two(dir: &Path, key: &str) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut others: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json") && p.file_stem().is_some_and(|s| s != key))
        .map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p))
        .collect();
    others.sort();
    others.pop();
    for (_, p) in others {
        let _ = std::fs::remove_file(p);
    }
}

/// At boot: a photo write a crash cut short leaves its tmp file; it goes.
pub fn clean(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for p in rd.flatten().map(|e| e.path()) {
        if p.to_string_lossy().ends_with(".json.tmp") {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
#[path = "photo_tests.rs"]
mod tests;
