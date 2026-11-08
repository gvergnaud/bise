//! One locked write for the files of bise's home that several processes
//! change (config.toml's roles, checks.json, architect m_57509): an
//! exclusive flock on `<name>.lock` around the whole read-edit-write, a
//! tmp name only this call uses (`<name>.tmp-<pid>-<n>`), then a rename:
//! two writers at once never lose a change.

/// Each write's own tmp name in this process (with the pid: across them).
static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `file` rewritten by `change` (its text, "" when none), under its lock.
pub fn rewrite(file: &std::path::Path, change: impl Fn(&str) -> String) -> Result<(), String> {
    let name = file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't write {name}: {e}"))?;
    }
    let _lock = lock(&file.with_file_name(format!("{name}.lock"))).map_err(|e| format!("couldn't lock {name}: {e}"))?;
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let tmp = file.with_file_name(format!("{name}.tmp-{}-{}", std::process::id(), TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::write(&tmp, change(&text)).map_err(|e| format!("couldn't write {name}: {e}"))?;
    std::fs::rename(&tmp, file).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write {name}: {e}")
    })
}

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}
