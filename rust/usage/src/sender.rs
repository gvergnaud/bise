//! The one sender of the machine (plan usage-metrics-plan, transport;
//! architect m_53623 c, m_53687, m_55984). Every hub and the app's core
//! call [`tick`] each hour (through [`crate::hourly`]); its non-blocking
//! flock on `send.lock` lets one of them through and the others return
//! [`Tick::Busy`], so twelve hubs and the core make one sender. The flock
//! goes with the process: never a pid file.
//!
//! A tick, in this order:
//! 1. a pending "Delete my data" request ([`request_delete`]) goes first:
//!    the one send without a [`Permit`]; its id is deleted only after a
//!    2xx, else kept and tried again with the backoff;
//! 2. sharing off by his choice ([`Off::his_choice`]): [`wipe`] deletes
//!    what was counted, the id included; off for another reason (the
//!    switch, a dev build, a test, CI, his note not seen): nothing moves;
//! 3. with a [`Permit`]: the spool and `sent/` are pruned, the closed UTC
//!    days (settled [`SETTLE_SECS`] after midnight, so a process's last
//!    5-minute write is in) become packets, sent oldest first in bodies of
//!    at most [`MAX_BODY`] bytes. A 2xx moves each packet to `sent/`; a
//!    4xx drops them (the report says so, the caller logs it); no answer
//!    keeps them for the next try ([`crate::retry`]).
//!
//! Pure apart from its [`Ctx`] (the build, the endpoint, the transport,
//! the random bytes, the jitter) and the files under `Home::usage_dir`.
//! The install id never goes into a [`Report`]: what the caller logs.

use std::fs::File;
use std::io;

use bise_home::Home;

use crate::day::Day;
use crate::encode::MAX_BATCH;
use crate::install_id::{self, InstallId};
use crate::packet::{Build, Packet, Parts, Surface};
use crate::retry::Retry;
use crate::send::{send_delete, send_packets, Endpoint, Outcome, Transport};
use crate::sharing::{Off, Permit};
use crate::spool::{self, SourceKind};
use crate::state::LocalState;
use crate::tally::Tally;
use crate::{sent, Milestone};

/// A closed day is sent this long after its midnight at the earliest.
pub const SETTLE_SECS: u64 = 15 * 60;

/// The most bytes one body carries; a longer batch is split.
pub const MAX_BODY: usize = 64 * 1024;

/// What a tick needs besides the files.
pub struct Ctx<'a> {
    pub build: &'a Build,
    pub endpoint: &'a Endpoint,
    pub transport: &'a mut dyn Transport,
    /// 16 random bytes, for a new install id.
    pub random: &'a mut dyn FnMut() -> [u8; 16],
    /// In [0, 1): the backoff's random part.
    pub jitter: f64,
}

/// What a tick did.
#[derive(Debug, PartialEq, Eq)]
pub enum Tick {
    /// Another process of this machine is the sender right now.
    Busy,
    Ran(Report),
}

/// What a tick did, with no id in it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Why nothing was sent, when sharing is off.
    pub off: Option<Off>,
    /// He turned sharing off: what was counted is deleted.
    pub wiped: bool,
    /// Delete requests the server took.
    pub deleted: usize,
    /// The days whose packets the server took.
    pub sent: Vec<Day>,
    /// The days the server refused (a 4xx), with its status: dropped.
    pub refused: Vec<(Day, u16)>,
    /// The days whose packet alone passes [`MAX_BODY`]: dropped.
    pub too_big: Vec<Day>,
    /// A send failed: why, and when the next may go.
    pub retry: Option<(String, u64)>,
    /// A send was due but the backoff says to wait until then.
    pub waiting_until: Option<u64>,
}

impl Report {
    /// The line for the local log, when something happened worth one.
    pub fn log_line(&self) -> Option<String> {
        let days = |ds: &mut dyn Iterator<Item = Day>| ds.map(|d| d.to_string()).collect::<Vec<_>>().join(",");
        let mut parts = Vec::new();
        if self.wiped {
            parts.push("sharing off: local usage deleted".to_string());
        }
        if self.deleted > 0 {
            parts.push(format!("{} delete request(s) taken", self.deleted));
        }
        if !self.sent.is_empty() {
            parts.push(format!("sent {}", days(&mut self.sent.iter().copied())));
        }
        if !self.refused.is_empty() {
            let r: Vec<String> = self.refused.iter().map(|(d, s)| format!("{d}:{s}")).collect();
            parts.push(format!("refused (dropped) {}", r.join(",")));
        }
        if !self.too_big.is_empty() {
            parts.push(format!("too big (dropped) {}", days(&mut self.too_big.iter().copied())));
        }
        if let Some((why, next)) = &self.retry {
            parts.push(format!("send failed ({why}), next try at {next}"));
        }
        (!parts.is_empty()).then(|| format!("usage: {}", parts.join("; ")))
    }
}

/// The flock on `send.lock`, released when dropped (or when the process dies).
struct SendLock(#[allow(dead_code)] File);

impl SendLock {
    fn open(home: &Home) -> io::Result<File> {
        std::fs::create_dir_all(home.usage_dir())?;
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(home.usage_send_lock())
    }

    /// The lock if no other process holds it.
    fn try_take(home: &Home) -> io::Result<Option<SendLock>> {
        let f = Self::open(home)?;
        match f.try_lock() {
            Ok(()) => Ok(Some(SendLock(f))),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// The lock, waiting for a sender's tick to end (at most its sends).
    fn take(home: &Home) -> io::Result<SendLock> {
        let f = Self::open(home)?;
        f.lock()?;
        Ok(SendLock(f))
    }
}

/// One tick of the sender at `now_secs`; `gate` is
/// [`crate::sharing_allowed`]'s answer for this process.
pub fn tick(home: &Home, now_secs: u64, gate: Result<Permit, Off>, ctx: &mut Ctx) -> Tick {
    // off with nothing ever written: not even the lock file is made
    if gate.is_err() && !home.usage_dir().exists() {
        return Tick::Ran(Report { off: gate.err(), ..Report::default() });
    }
    let _lock = match SendLock::try_take(home) {
        Ok(Some(l)) => l,
        Ok(None) => return Tick::Busy,
        Err(e) => return Tick::Ran(Report { off: gate.err(), retry: Some((format!("send.lock: {e}"), now_secs)), ..Report::default() }),
    };
    let mut report = Report { off: gate.as_ref().err().copied(), ..Report::default() };
    let mut retry = Retry::load(&home.usage_retry());
    let deletes_done = send_deletes(home, now_secs, ctx, &mut retry, &mut report);
    match gate {
        Err(off) if off.his_choice() => report.wiped = wipe(home).is_ok(),
        Err(_) => {}
        Ok(permit) if deletes_done => send_days(home, now_secs, &permit, ctx, &mut retry, &mut report),
        Ok(_) => {}
    }
    if let Err(e) = retry.save(&home.usage_retry()) {
        report.retry.get_or_insert((format!("retry.json: {e}"), retry.next_secs));
    }
    Tick::Ran(report)
}

/// The pending delete requests, oldest first; true when none is left.
fn send_deletes(home: &Home, now: u64, ctx: &mut Ctx, retry: &mut Retry, report: &mut Report) -> bool {
    let mut ids = pending_deletes(home);
    if ids.is_empty() {
        return true;
    }
    if !retry.due(now) {
        report.waiting_until = Some(retry.next_secs);
        return false;
    }
    while let Some(id) = ids.first() {
        match send_delete(ctx.endpoint, id, ctx.transport) {
            Outcome::Taken => {
                ids.remove(0);
                report.deleted += 1;
                *retry = Retry::default();
            }
            // the server will never take it: keeping it would block every send
            Outcome::Refused(s) => {
                ids.remove(0);
                report.retry = Some((format!("delete request refused, status {s}"), now));
            }
            Outcome::Retry(why) => {
                *retry = retry.failed(now, ctx.jitter);
                report.retry = Some((why, retry.next_secs));
                break;
            }
        }
    }
    let _ = save_deletes(home, &ids);
    ids.is_empty()
}

fn pending_deletes(home: &Home) -> Vec<InstallId> {
    std::fs::read_to_string(home.usage_delete_request()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_deletes(home: &Home, ids: &[InstallId]) -> io::Result<()> {
    if ids.is_empty() {
        return remove_file(&home.usage_delete_request());
    }
    spool::write_atomic(&home.usage_delete_request(), &serde_json::to_vec(ids)?)
}

fn remove_file(path: &std::path::Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Delete what was counted and kept (the spool, `sent/`, `state.json`, the
/// install id), but a pending delete request and its backoff. The
/// caller holds the lock.
fn wipe(home: &Home) -> io::Result<()> {
    for d in spool::days_in(&home.usage_dir()) {
        std::fs::remove_dir_all(home.usage_day_dir(&d.to_string()))?;
    }
    match std::fs::remove_dir_all(home.usage_sent_dir()) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    remove_file(&home.usage_state())?;
    remove_file(&home.usage_install_id())?;
    if pending_deletes(home).is_empty() {
        remove_file(&home.usage_retry())?;
    }
    Ok(())
}

/// He turned sharing off (`bise telemetry off`, Settings → Privacy):
/// [`wipe`] now, after a sender's tick in progress.
pub fn forget(home: &Home) -> io::Result<()> {
    if !home.usage_dir().exists() {
        return Ok(());
    }
    let _lock = SendLock::take(home)?;
    wipe(home)
}

/// "Delete my data": the install id stops being used at once. If the
/// server ever took a packet from this machine, the id goes to the
/// delete request the next tick sends (and keeps until a 2xx); if not,
/// the id is just deleted and nothing is posted (architect m_55984: a
/// delete request is never bise's first contact with the server).
/// True when a request is queued.
pub fn request_delete(home: &Home) -> io::Result<bool> {
    if !home.usage_dir().exists() {
        return Ok(false);
    }
    let _lock = SendLock::take(home)?;
    let id = std::fs::read_to_string(home.usage_install_id()).ok().and_then(|t| InstallId::from_file(&t));
    let ever_sent = load_state(home).is_some_and(|s| s.ever_sent);
    let queued = match id {
        Some(id) if ever_sent => {
            let mut ids = pending_deletes(home);
            ids.push(id);
            save_deletes(home, &ids)?;
            true
        }
        _ => false,
    };
    remove_file(&home.usage_install_id())?;
    Ok(queued)
}

fn load_state(home: &Home) -> Option<LocalState> {
    std::fs::read_to_string(home.usage_state()).ok().and_then(|t| serde_json::from_str(&t).ok())
}

fn save_state(home: &Home, s: &LocalState) -> io::Result<()> {
    spool::write_atomic(&home.usage_state(), &serde_json::to_vec(s)?)
}

/// The closed days the spool holds, settled, oldest first, at most a batch.
fn closed_days(home: &Home, now: u64) -> Vec<Day> {
    let today = Day::from_unix_secs(now);
    let mut days = Vec::new();
    for d in spool::days_in(&home.usage_dir()) {
        let settled = d < today && now >= d.plus(1).start_secs() + SETTLE_SECS;
        if !settled {
            continue;
        }
        // a late write of a day already sent: never sent twice
        if sent::holds(&home.usage_sent_dir(), d) {
            let _ = std::fs::remove_dir_all(home.usage_day_dir(&d.to_string()));
            continue;
        }
        days.push(d);
    }
    days.truncate(MAX_BATCH);
    days
}

/// The packets of `days` (marking the active ones in `state`), the unsent
/// milestones on the oldest.
fn packets(home: &Home, days: &[Day], id: &InstallId, build: &Build, state: &mut LocalState) -> Vec<Packet> {
    let sums: Vec<(Day, Tally, Surface)> = days
        .iter()
        .map(|d| {
            let sources = spool::read_day(&home.usage_day_dir(&d.to_string()));
            let has = |k: SourceKind| sources.iter().any(|(s, _)| *s == k);
            let surface = Surface::of(has(SourceKind::Core), has(SourceKind::Tui));
            (*d, Tally::sum(sources.iter().map(|(_, t)| t)), surface)
        })
        .collect();
    for (d, t, _) in &sums {
        if t.active() {
            state.mark_active(*d);
        }
    }
    let mut milestones = state.unsent();
    sums.iter()
        .map(|(day, tally, surface)| {
            Packet::build(Parts { day: *day, install_id: id, view: state.view(*day), build, surface: *surface, tally, milestones: std::mem::take(&mut milestones) })
        })
        .collect()
}

/// What would go next, for `bise telemetry show` and "See what's sent":
/// a packet for each day of the spool, today's so far included, made as
/// [`tick`] makes them; nothing is written or sent. Without an id yet
/// (it is drawn at the first send), the packets carry a zero one; the
/// bool says whether the id is the real one.
pub fn preview(home: &Home, now_secs: u64, build: &Build) -> (Vec<Packet>, bool) {
    let today = Day::from_unix_secs(now_secs);
    let mut state = load_state(home).unwrap_or_else(|| LocalState::new(now_secs));
    let stored = std::fs::read_to_string(home.usage_install_id()).ok().and_then(|t| InstallId::from_file(&t));
    let real = stored.is_some();
    let id = stored.unwrap_or_else(|| InstallId::draw([0; 16], today));
    let mut days: Vec<Day> = spool::days_in(&home.usage_dir()).into_iter().filter(|d| *d <= today).collect();
    days.drain(..days.len().saturating_sub(MAX_BATCH));
    (packets(home, &days, &id, build, &mut state), real)
}

/// The id in use and the ids waiting for their delete request, for
/// `bise telemetry show` (the only place it is shown).
pub fn ids(home: &Home) -> (Option<InstallId>, Vec<InstallId>) {
    let id = std::fs::read_to_string(home.usage_install_id()).ok().and_then(|t| InstallId::from_file(&t));
    (id, pending_deletes(home))
}

/// How many of `sizes` (each packet's own JSON length), from the first,
/// fit one body of at most `max` bytes.
pub(crate) fn fitting(sizes: &[usize], max: usize) -> usize {
    // `{"packets":[` + the packets, comma-separated + `]}`
    let mut len = 14;
    for (i, s) in sizes.iter().enumerate() {
        len += s + usize::from(i > 0);
        if len > max {
            return i;
        }
    }
    sizes.len()
}

/// Send the closed days, with a permit.
fn send_days(home: &Home, now: u64, permit: &Permit, ctx: &mut Ctx, retry: &mut Retry, report: &mut Report) {
    let today = Day::from_unix_secs(now);
    spool::prune(&home.usage_dir(), today, spool::KEEP_DAYS);
    sent::prune(&home.usage_sent_dir(), today);
    let days = closed_days(home, now);
    if days.is_empty() {
        return;
    }
    if !retry.due(now) {
        report.waiting_until = Some(retry.next_secs);
        return;
    }
    let mut state = load_state(home).unwrap_or_else(|| LocalState::new(now));
    let stored = std::fs::read_to_string(home.usage_install_id()).ok().and_then(|t| InstallId::from_file(&t));
    let (id, new) = install_id::current(stored, today, &mut *ctx.random);
    if new {
        if let Err(e) = spool::write_atomic(&home.usage_install_id(), id.to_file().as_bytes()) {
            report.retry = Some((format!("install-id: {e}"), now));
            return;
        }
    }
    let mut todo = packets(home, &days, &id, ctx.build, &mut state);
    let _ = save_state(home, &state);
    let drop_day = |d: Day| {
        let _ = std::fs::remove_dir_all(home.usage_day_dir(&d.to_string()));
    };
    while !todo.is_empty() {
        let sizes: Vec<usize> = todo.iter().map(|p| serde_json::to_vec(p).map_or(usize::MAX, |b| b.len())).collect();
        let n = fitting(&sizes, MAX_BODY);
        if n == 0 {
            let p = todo.remove(0);
            drop_day(p.day);
            report.too_big.push(p.day);
            continue;
        }
        let batch: Vec<Packet> = todo.drain(..n).collect();
        match send_packets(permit, ctx.endpoint, &batch, ctx.transport) {
            Outcome::Taken => {
                for p in &batch {
                    let _ = sent::keep(&home.usage_sent_dir(), today, p);
                    drop_day(p.day);
                    report.sent.push(p.day);
                }
                let ms: Vec<Milestone> = batch.iter().flat_map(|p| p.milestones.keys().copied()).collect();
                state.mark_sent(&ms);
                state.ever_sent = true;
                let _ = save_state(home, &state);
                *retry = Retry::default();
            }
            Outcome::Refused(status) => {
                for p in &batch {
                    drop_day(p.day);
                    report.refused.push((p.day, status));
                }
                *retry = Retry::default();
            }
            Outcome::Retry(why) => {
                *retry = retry.failed(now, ctx.jitter);
                report.retry = Some((why, retry.next_secs));
                return;
            }
        }
    }
}
