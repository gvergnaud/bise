//! The release offer (update-flow, architect m_45050): "a newer bise is
//! out" is a fact of the machine, not of one hub. Every hub reads the
//! same files with the same [`plan`], so each one says the same thing,
//! and `update now` in any project updates the machine.
//!
//! The files, under `cache/` (each with one writer):
//! - `latest.json`: the channel's manifest (`bise update --manifest`,
//!   fetched at most once per period for the whole machine:
//!   [`Home::release_stamp`] + a flock on it, [`try_lock`]);
//! - `update-later`: the release the user said `later` to on its inbox
//!   card ([`write_later`]; `/update` clears it): every hub's card for it
//!   closes;
//! - `update-go`: `{id, at}`, written by the hub that got `update now`,
//!   ONLY once `bise update` installed that release ([`write_go`]): then
//!   every hub on an older version switches to it, one at a time;
//! - `update-install`: a flock held by the hub installing (its content:
//!   the id), so the others say `installing` ([`installing`]);
//! - `switch.lock`: a flock held by a switcher for its switch and its
//!   probation: one hub switches at a time on the machine
//!   ([`try_lock`]; the kernel frees it when the switcher dies).
//!
//! A hub's own failed switch (its `versions.json` `failed`, a probation
//! that rolled back) is [`Facts::my_failed`]: that hub never retries the
//! same `update now` ([`State::Failed`]); a newer release or a new
//! `update now` (a later `go.at`) clears it.
//!
//! The desktop app's own updater (bar S.6, apps/desktop/src/main/update)
//! is separate: it swaps the app bundle, never the hubs.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The channel's latest release, as the offer needs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Latest {
    pub id: String,
    /// its name (`2026.10.2-5`), else the id
    pub version: String,
    pub notes: Vec<String>,
    /// its release page
    pub url: String,
    /// its whole notes as the manifest wrote them (markdown), cut at
    /// [`CHANGELOG_MAX`] bytes: the update card's changelog
    pub changelog: String,
}

/// `update now`, for the machine: switch every hub to `id`.
#[derive(Clone, Debug, PartialEq)]
pub struct Go {
    pub id: String,
    /// when it was asked (ms): a failure older than it is forgotten
    pub at: u64,
}

/// This hub's last switch that rolled back (its versions.json `failed`).
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub id: String,
    pub at: u64,
    pub why: String,
    /// the new version's last log lines (versions.json `failed.log`)
    pub log: Vec<String>,
}

/// What a hub knows when it plans.
#[derive(Clone, Debug, Default)]
pub struct Facts<'a> {
    pub latest: Option<&'a Latest>,
    /// the latest release replaces this hub's version (another id, not
    /// older: `release::is_update`)
    pub newer: bool,
    pub later: Option<&'a str>,
    pub go: Option<&'a Go>,
    /// the id a hub is installing now (the install lock held)
    pub installing: Option<&'a str>,
    /// this hub's version id
    pub my_id: &'a str,
    pub my_failed: Option<&'a Failed>,
    /// he turned on `Update automatically` (prefs `auto_update`)
    pub auto: bool,
}

/// Where the offer is.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// newer, nothing asked: the user may update
    Ready,
    /// `update now`: a hub is installing it
    Installing,
    /// installed: this hub switches to it (one hub at a time)
    Updating,
    /// this hub's switch to it rolled back: not tried again
    Failed { why: String, log: Vec<String> },
    /// the user said later to this release
    Later,
    /// this hub runs it, or nothing newer
    UpToDate,
}

/// The offer of a hub: the latest release and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub release: Latest,
    pub state: State,
    /// this hub should switch to `release` now (the shell checks the
    /// switch lock and the running turns)
    pub switch_me: bool,
    /// `Update automatically` is on and the release is ready: install it
    /// as `update now` would (the install flock: one hub does it)
    pub install_me: bool,
}

impl Offer {
    /// Something to show: ready, installing, updating.
    pub fn shown(&self) -> bool {
        matches!(self.state, State::Ready | State::Installing | State::Updating)
    }
}

/// The one decision: what a hub says about the latest release, and
/// whether it switches. Pure; every hub runs it on the same files.
pub fn plan(f: &Facts) -> Option<Offer> {
    let l = f.latest?;
    let offer = |state: State, switch_me: bool| {
        let install_me = f.auto && state == State::Ready;
        Some(Offer { release: l.clone(), state, switch_me, install_me })
    };
    if l.id == f.my_id || !f.newer {
        return offer(State::UpToDate, false);
    }
    // a go for an older release is stale: the newer one is offered again
    if let Some(g) = f.go.filter(|g| g.id == l.id) {
        return match f.my_failed.filter(|x| x.id == g.id && x.at >= g.at) {
            Some(x) => offer(State::Failed { why: x.why.clone(), log: x.log.clone() }, false),
            None => offer(State::Updating, true),
        };
    }
    if f.installing == Some(l.id.as_str()) {
        return offer(State::Installing, false);
    }
    if f.later == Some(l.id.as_str()) {
        return offer(State::Later, false);
    }
    offer(State::Ready, false)
}

/// The manifest's facts the offer needs (`notes`: a list of lines or one
/// text; `url`: the release page, from `page` + its tag).
pub fn latest_of(manifest: &str, page: &str) -> Option<Latest> {
    let v: Value = serde_json::from_str(manifest).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    let id = s("id")?;
    let version = s("version").unwrap_or_else(|| id.clone());
    let notes: Vec<String> = match v.get("notes") {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).map(String::from).collect(),
        Some(Value::String(t)) => t.lines().map(String::from).collect(),
        _ => Vec::new(),
    };
    let changelog = cut_bytes(notes.join("\n").trim(), CHANGELOG_MAX);
    let notes = notes
        .iter()
        .map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim())
        .filter(|l| !l.is_empty())
        .take(NOTES_MAX)
        .map(|l| clip(l, NOTE_WIDTH))
        .collect();
    let tag = s("tag").unwrap_or_else(|| if version == id { id.clone() } else { format!("v{}", version) });
    let url = if page.is_empty() { String::new() } else { format!("{}{}", page, tag) };
    Some(Latest { id, version, notes, url, changelog })
}

/// Most lines of notes, and their width.
const NOTES_MAX: usize = 5;
const NOTE_WIDTH: usize = 120;
/// The changelog's most bytes (architect m_46059: 16 KB).
pub const CHANGELOG_MAX: usize = 16 * 1024;

/// `s` cut to at most `max` bytes at a line's end (else a char's), `…`
/// after a cut.
fn cut_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.saturating_sub('…'.len_utf8());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let head = &s[..end];
    let head = head.rfind('\n').map_or(head, |i| &head[..i]);
    format!("{}…", head.trim_end())
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

impl crate::Home {
    /// `update-go`: `{id, at}`, the machine's `update now` ([`write_go`]).
    pub fn update_go(&self) -> PathBuf {
        self.cache_dir().join("update-go")
    }

    /// `update-later`: the release the user said later to (one id).
    pub fn update_later(&self) -> PathBuf {
        self.cache_dir().join("update-later")
    }

    /// `update-install`: flocked by the hub installing a release.
    pub fn update_install(&self) -> PathBuf {
        self.cache_dir().join("update-install")
    }

    /// `release-check`: the machine's last release check (its mtime),
    /// flocked while one hub fetches.
    pub fn release_stamp(&self) -> PathBuf {
        self.cache_dir().join("release-check")
    }

    /// `switch.lock`: flocked by the switcher that switches a hub to a
    /// release and holds it through the probation.
    pub fn switch_lock(&self) -> PathBuf {
        self.cache_dir().join("switch.lock")
    }
}

/// Write `text` to `path` whole: a tmp file, then a rename.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

pub fn read_go(home: &crate::Home) -> Option<Go> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(home.update_go()).ok()?).ok()?;
    let id = v.get("id")?.as_str().filter(|s| !s.is_empty())?.to_string();
    Some(Go { id, at: v.get("at").and_then(Value::as_u64).unwrap_or(0) })
}

/// `update now` for the machine, once `id` is installed.
pub fn write_go(home: &crate::Home, go: &Go) -> std::io::Result<()> {
    write_atomic(&home.update_go(), &json!({"id": go.id, "at": go.at}).to_string())
}

pub fn read_later(home: &crate::Home) -> Option<String> {
    std::fs::read_to_string(home.update_later()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `later` to release `id` (None: `/update` asks again).
pub fn write_later(home: &crate::Home, id: Option<&str>) -> std::io::Result<()> {
    match id {
        Some(id) => write_atomic(&home.update_later(), id),
        None => match std::fs::remove_file(home.update_later()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// An exclusive flock, held until dropped (or its holder dies).
pub struct Lock(std::fs::File);

impl Drop for Lock {
    /// Unlock before the close: a child forked meanwhile (any thread's
    /// Command, between its fork and its exec) holds a copy of the
    /// descriptor, and a plain close would leave the lock held by it.
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        // SAFETY: flock on the descriptor this Lock owns
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

impl Lock {
    /// Say what the holder does (the install lock: the id).
    pub fn say(&mut self, text: &str) {
        use std::io::{Seek, Write};
        let _ = self.0.set_len(0);
        let _ = self.0.rewind();
        let _ = self.0.write_all(text.as_bytes());
    }
}

/// Take `path`'s exclusive flock without waiting: None when another
/// process holds it (or on an error).
pub fn try_lock(path: &Path) -> Option<Lock> {
    use std::os::unix::io::AsRawFd;
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let f = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path).ok()?;
    // SAFETY: flock on a descriptor this function owns; LOCK_NB never blocks
    (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(Lock(f))
}

/// Whether another process holds `path`'s flock now.
pub fn held(path: &Path) -> bool {
    path.exists() && try_lock(path).is_none()
}

/// The id a hub is installing now: the install lock is held.
pub fn installing(home: &crate::Home) -> Option<String> {
    let path = home.update_install();
    held(&path).then(|| std::fs::read_to_string(&path).ok()).flatten().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The machine's release check is due: its stamp is older than `every`
/// (or missing).
pub fn check_due(home: &crate::Home, every: std::time::Duration) -> bool {
    std::fs::metadata(home.release_stamp())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age >= every)
}

/// Mark the machine's release check done now. Written in place (never a
/// rename): the stamp is also the check's flock.
pub fn stamp_check(home: &crate::Home) {
    let _ = std::fs::write(home.release_stamp(), now_ms().to_string());
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
#[path = "release_offer_tests.rs"]
mod tests;
