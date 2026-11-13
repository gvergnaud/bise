//! Dictation's takes on disk (`bise_home::Home::dictation_dir`): the
//! sound of each stopped take, one raw file each (`<id>.pcm`, 16-bit
//! little-endian mono at the voice's sample rate), so a take survives a
//! chat switch, a window reload and a quit: a take still transcribed when
//! the app quit, or one that failed, is transcribed again on relaunch
//! (`dictate_retry`). The file is the one copy (no memory list): written
//! when the take stops, gone when the window says it has the take
//! (`dictate_cancel`: his esc, or its words landed in a draft), and
//! pruned past [`MAX_AGE`] or past [`MAX_TOTAL`] bytes, the oldest first.
//! The folder is 0700 and each file 0600, written as a temp file and a
//! rename (architect m_49881). File operations only: no clock of its own
//! (`now` is an input), no log line with his words.

use std::collections::VecDeque;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// An unsent take is kept this long (the privacy doc says so).
pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// All takes together: past this, the oldest go.
pub const MAX_TOTAL: u64 = 200 * 1024 * 1024;
/// The core prunes at its start and this often while it runs.
pub const PRUNE_EVERY: Duration = Duration::from_secs(24 * 3600);

/// A take's file name from its id; None for an id that is not a plain
/// name (the window's ids are `d<n>-<ms>`): never a path out of the folder.
fn file(dir: &Path, id: &str) -> Option<PathBuf> {
    let plain = !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain.then(|| dir.join(format!("{id}.pcm")))
}

#[cfg(unix)]
fn private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)
}

#[cfg(unix)]
fn create_private(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(true).open(path)
}

/// The take's sound to its file (replaced whole). Nothing to keep (no
/// sound, an id that is not a name): Ok(false).
pub fn save(dir: &Path, id: &str, pcm: &VecDeque<i16>) -> io::Result<bool> {
    let Some(path) = file(dir, id) else { return Ok(false) };
    if pcm.is_empty() {
        return Ok(false);
    }
    private_dir(dir)?;
    let part = dir.join(format!(".{id}.pcm.part"));
    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    let mut f = create_private(&part)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    fs::rename(&part, &path)?;
    Ok(true)
}

/// The take's sound, if its file is there.
pub fn load(dir: &Path, id: &str) -> Option<VecDeque<i16>> {
    let bytes = fs::read(file(dir, id)?).ok()?;
    Some(bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect())
}

/// Its file is there.
#[cfg(test)]
pub fn has(dir: &Path, id: &str) -> bool {
    file(dir, id).is_some_and(|p| p.is_file())
}

/// The take's file goes (true: there was one).
pub fn remove(dir: &Path, id: &str) -> bool {
    file(dir, id).is_some_and(|p| fs::remove_file(p).is_ok())
}

/// The files older than `max_age` go, then the oldest until the rest
/// weighs at most `max_total` bytes. Returns how many went.
pub fn prune(dir: &Path, now: SystemTime, max_age: Duration, max_total: u64) -> usize {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let m = e.metadata().ok()?;
            (m.is_file() && p.extension().is_some_and(|x| x == "pcm" || x == "part")).then(|| (m.modified().unwrap_or(now), m.len(), p))
        })
        .collect();
    files.sort_by_key(|f| f.0);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    let mut gone = 0;
    for (at, len, p) in files {
        let old = now.duration_since(at).is_ok_and(|age| age > max_age);
        if (old || total > max_total) && fs::remove_file(&p).is_ok() {
            total -= len;
            gone += 1;
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("amb-takes-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_take_is_saved_private_read_back_whole_and_removed() {
        let d = dir("save");
        let pcm: VecDeque<i16> = [1, -2, 300, i16::MIN, i16::MAX].into_iter().collect();
        assert!(save(&d, "d1-1700", &pcm).unwrap());
        assert_eq!(load(&d, "d1-1700"), Some(pcm));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&d).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(fs::metadata(d.join("d1-1700.pcm")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert!(has(&d, "d1-1700"));
        assert!(remove(&d, "d1-1700"));
        assert!(!has(&d, "d1-1700"));
        assert_eq!(load(&d, "d1-1700"), None);
        assert!(!remove(&d, "d1-1700"));
    }

    #[test]
    fn an_id_that_is_not_a_name_never_reaches_the_disk() {
        let d = dir("names");
        let pcm: VecDeque<i16> = [1].into_iter().collect();
        for id in ["", "../x", "a/b", "x.pcm", "."] {
            assert!(!save(&d, id, &pcm).unwrap(), "{id}");
            assert_eq!(load(&d, id), None);
        }
        assert!(!save(&d, "ok", &VecDeque::new()).unwrap(), "no sound, no file");
        assert!(!d.exists());
    }

    #[test]
    fn prune_drops_an_old_take_then_the_oldest_past_the_total() {
        let d = dir("prune");
        let pcm: VecDeque<i16> = vec![5; 50].into_iter().collect(); // 100 bytes each
        for id in ["a", "b", "c"] {
            save(&d, id, &pcm).unwrap();
        }
        let set = |id: &str, ago: u64| {
            let f = fs::File::options().write(true).open(d.join(format!("{id}.pcm"))).unwrap();
            f.set_modified(SystemTime::now() - Duration::from_secs(ago)).unwrap();
        };
        set("a", 8 * 24 * 3600); // 8 days old
        set("b", 60);
        set("c", 10);
        let now = SystemTime::now();
        assert_eq!(prune(&d, now, MAX_AGE, MAX_TOTAL), 1);
        assert!(!has(&d, "a") && has(&d, "b") && has(&d, "c"));
        // 200 bytes left, a cap of 150: the older (b) goes
        assert_eq!(prune(&d, now, MAX_AGE, 150), 1);
        assert!(!has(&d, "b") && has(&d, "c"));
        assert_eq!(prune(&dir("none"), now, MAX_AGE, MAX_TOTAL), 0);
    }
}
