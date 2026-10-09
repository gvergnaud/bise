//! The writer (§8): one per session (flock), append only, one write per
//! line, fsync at the key points, 0600 files in a 0700 folder, big texts
//! to blobs, rotation at 32 MiB.
use crate::blob;
use crate::reader::{self, Log, Open};
use crate::types::must_of;
use serde_json::{json, Map, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

/// A line stays under this; bigger texts go to blobs (§8.2).
pub const LINE_MAX: usize = 256 * 1024;
/// A text part bigger than this moves first when a line is too big.
const PART_MOVE: usize = 16 * 1024;
/// The current segment is rotated past this, at a turn boundary (§8.3).
pub const SEGMENT_MAX: u64 = 32 * 1024 * 1024;

/// The events after which the file is fsynced (§8.1).
const FSYNC: &[&str] = &[
    "session_start", "segment_start", "process_opened", "user_message", "agent_message",
    "input_queued", "turn_ended", "compaction_done", "checkpoint",
];

#[derive(Debug)]
pub enum OpenError {
    Io(std::io::Error),
    /// another live process writes this session
    Locked(String),
    /// the log cannot be appended to (unknown must event, newer format)
    ReadOnly(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Io(e) => write!(f, "{e}"),
            OpenError::Locked(w) => write!(f, "session is open in another process ({w})"),
            OpenError::ReadOnly(w) => write!(f, "session is read-only: {w}"),
        }
    }
}

impl From<std::io::Error> for OpenError {
    fn from(e: std::io::Error) -> Self {
        OpenError::Io(e)
    }
}

pub struct Writer {
    dir: PathBuf,
    blobs: PathBuf,
    file: File,
    _lock: File,
    seq: u64,
    size: u64,
    /// rotation threshold (SEGMENT_MAX; smaller in tests)
    pub segment_max: u64,
    /// secrets replaced in every event before it is written (BISE-193)
    pub redactor: Option<crate::redact::Redactor>,
}

fn now_iso() -> String {
    iso(std::time::SystemTime::now())
}

/// An instant as `2026-10-01T09:14:03.120Z` (UTC, ms).
pub fn iso(t: std::time::SystemTime) -> String {
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (secs, ms) = (d.as_secs() as i64, d.subsec_millis());
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// [`iso`]'s inverse: `2026-10-01T09:14:03.120Z` as ms since the epoch
/// (a tool result's event time joined to its transcript line).
pub fn ms_of_iso(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 24 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'.' || b[23] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hh, mm, ss, ms) = (n(11, 13)?, n(14, 16)?, n(17, 19)?, n(20, 23)?);
    // days from civil (Howard Hinnant), the inverse of iso's
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(((days * 86400 + hh * 3600 + mm * 60 + ss) * 1000) + ms).ok()
}

/// `s-<utc yyyymmdd-hhmmss>-<6 hex>` (§12 decision 2).
pub fn new_session_id() -> String {
    session_id_at(std::time::SystemTime::now())
}

/// The session id of a session started at `t`.
pub fn session_id_at(t: std::time::SystemTime) -> String {
    let t = iso(t);
    let mut rnd = [0u8; 3];
    if let Ok(mut f) = File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut rnd);
    }
    format!(
        "s-{}{}{}-{}{}{}-{:02x}{:02x}{:02x}",
        &t[0..4], &t[5..7], &t[8..10], &t[11..13], &t[14..16], &t[17..19], rnd[0], rnd[1], rnd[2]
    )
}

fn open_append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().append(true).create(true).mode(0o600).open(path)
}

/// Take the session's lock (flock + our pid in the file).
fn lock(dir: &Path) -> Result<File, OpenError> {
    let path = dir.join("lock");
    let mut f = OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(&path)?;
    // SAFETY: flock on a descriptor we own
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let who = std::fs::read_to_string(&path).unwrap_or_default();
        return Err(OpenError::Locked(format!("pid {}", who.trim())));
    }
    f.set_len(0)?;
    write!(f, "{}", std::process::id())?;
    Ok(f)
}

impl Writer {
    /// A new session folder with its first line (a `session_start`
    /// payload), then `process_opened`.
    pub fn create(dir: &Path, blobs: &Path, start: Value, writer: &str) -> Result<Writer, OpenError> {
        blob::mkdir_private(dir)?;
        let lock = lock(dir)?;
        let path = dir.join("events.jsonl");
        if path.exists() {
            return Err(OpenError::Io(std::io::Error::other(format!("{} exists", path.display()))));
        }
        let file = open_append(&path)?;
        if let Some(parent) = dir.parent() {
            if let Ok(d) = File::open(parent) {
                let _ = d.sync_all();
            }
        }
        let mut w = Writer { dir: dir.into(), blobs: blobs.into(), file, _lock: lock, seq: 0, size: 0, segment_max: SEGMENT_MAX, redactor: None };
        w.append("session_start", start, None)?;
        w.append("process_opened", json!({"writer": writer, "pid": std::process::id(), "resume": false}), None)?;
        Ok(w)
    }

    /// Open an existing session to append: the lock, the log, the torn
    /// tail cut off and saved to `events.torn-<time>` (§7 step 3).
    /// `process_opened` is the caller's (after its repair, §7 step 6).
    pub fn open(dir: &Path, blobs: &Path) -> Result<(Writer, Log), OpenError> {
        let lock = lock(dir)?;
        let log = reader::read_dir(dir)?;
        match &log.open {
            Open::Ok => {}
            Open::ReadOnly => return Err(OpenError::ReadOnly("it has an event from a newer bise".into())),
            Open::Refused(why) => return Err(OpenError::ReadOnly(why.clone())),
        }
        let path = dir.join("events.jsonl");
        if let Some(torn) = &log.torn {
            let save = dir.join(format!("events.torn-{}", now_iso().replace([':', '.'], "")));
            let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&save)?;
            f.write_all(torn)?;
            f.sync_all()?;
            let len = std::fs::metadata(&path)?.len() - torn.len() as u64;
            OpenOptions::new().write(true).open(&path)?.set_len(len)?;
        }
        let file = open_append(&path)?;
        let size = file.metadata()?.len();
        let seq = log.last_seq();
        Ok((Writer { dir: dir.into(), blobs: blobs.into(), file, _lock: lock, seq, size, segment_max: SEGMENT_MAX, redactor: None }, log))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn blobs(&self) -> &Path {
        &self.blobs
    }
    pub fn last_seq(&self) -> u64 {
        self.seq
    }
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Append one event; its seq. `must` comes from the type (§4).
    pub fn append(&mut self, typ: &str, data: Value, turn: Option<u64>) -> std::io::Result<u64> {
        self.append_data(typ, data, turn).map(|(seq, _)| seq)
    }

    /// `append`, with the data as written (redacted, big texts in blobs).
    pub fn append_data(&mut self, typ: &str, mut data: Value, turn: Option<u64>) -> std::io::Result<(u64, Value)> {
        if let Some(r) = &self.redactor {
            r.value(&mut data);
        }
        let data = self.fit(data)?;
        let seq = self.seq + 1;
        let mut o = Map::new();
        o.insert("seq".into(), seq.into());
        o.insert("at".into(), now_iso().into());
        if let Some(t) = turn {
            o.insert("turn".into(), t.into());
        }
        o.insert("type".into(), typ.into());
        o.insert("v".into(), 1.into());
        if must_of(typ) {
            o.insert("must".into(), true.into());
        }
        o.insert("data".into(), data.clone());
        let mut line = serde_json::to_string(&Value::Object(o)).map_err(std::io::Error::other)?;
        line.push('\n');
        // one write call: O_APPEND puts the whole line at the end
        self.file.write_all(line.as_bytes())?;
        if FSYNC.contains(&typ) {
            self.file.sync_data()?;
        }
        self.seq = seq;
        self.size += line.len() as u64;
        Ok((seq, data))
    }

    /// fsync now (before the hub is told a message was delivered).
    pub fn sync(&self) -> std::io::Result<()> {
        self.file.sync_data()
    }

    /// The 256 KiB rule: big text parts (and a big system text) become
    /// blobs until the line fits.
    fn fit(&self, mut data: Value) -> std::io::Result<Value> {
        if serde_json::to_string(&data).map(|s| s.len()).unwrap_or(0) + 200 <= LINE_MAX {
            return Ok(data);
        }
        let blobs = self.blobs.clone();
        let mut err = None;
        let mut move_text = |v: &mut Value| {
            let Some(o) = v.as_object_mut() else { return };
            let big = o.get("kind").and_then(Value::as_str) == Some("text")
                && o.get("text").and_then(Value::as_str).is_some_and(|t| t.len() > PART_MOVE);
            if big {
                let t = o.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                match blob::put(&blobs, t.as_bytes(), "text/plain") {
                    Ok(r) => *v = json!({"kind": "text_blob", "blob": r}),
                    Err(e) => err = Some(e),
                }
            }
        };
        for key in ["content", "parts", "summary", "partial"] {
            if let Some(Value::Array(a)) = data.get_mut(key) {
                a.iter_mut().for_each(&mut move_text);
            }
        }
        if let Some(sys) = data.get_mut("system") {
            if sys.get("text").and_then(Value::as_str).is_some_and(|t| t.len() > PART_MOVE) {
                let t = sys["text"].as_str().unwrap_or("").to_string();
                *sys = json!({"blob": blob::put(&self.blobs, t.as_bytes(), "text/plain")?});
            }
        }
        match err {
            Some(e) => Err(e),
            None => Ok(data),
        }
    }

    /// Whether the current segment is past its size (the caller rotates
    /// at the next turn boundary).
    pub fn wants_rotation(&self) -> bool {
        self.size > self.segment_max
    }

    /// Rotate (§8.3): events.jsonl → events.<n>.jsonl, a new events.jsonl
    /// starting with segment_start and the given checkpoint payload.
    pub fn rotate(&mut self, session: &str, checkpoint: Value) -> std::io::Result<()> {
        self.file.sync_all()?;
        let n = reader::segments(&self.dir).len() as u64; // the old ones + the current
        let old_name = format!("events.{n:06}.jsonl");
        let cur = self.dir.join("events.jsonl");
        let sha = blob::sha256_hex(&std::fs::read(&cur)?);
        std::fs::rename(&cur, self.dir.join(&old_name))?;
        self.file = open_append(&cur)?;
        self.size = 0;
        if let Ok(d) = File::open(&self.dir) {
            let _ = d.sync_all();
        }
        let last = self.seq;
        self.append(
            "segment_start",
            json!({"session": session, "format": reader::FORMAT, "index": n + 1,
                   "prev": {"file": old_name, "last_seq": last, "sha256": sha}}),
            None,
        )?;
        self.append("checkpoint", checkpoint, None)?;
        Ok(())
    }
}
