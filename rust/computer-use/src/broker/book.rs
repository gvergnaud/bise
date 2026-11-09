//! `profiles.json` (`Paths::profiles_file`): the browser profiles bise saw
//! and his default (`choose::Book`). The broker is its one writer: at a
//! hello that changes it, and on his `default` / `forget` (ctl).

use super::*;

/// The book on disk; none or unreadable: empty (the next hello fills it).
pub(super) fn load(paths: &Paths) -> Book {
    std::fs::read(paths.profiles_file()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| Book::from_json(&v)).unwrap_or_default()
}

/// Written whole, atomically (0600).
pub(super) fn save(paths: &Paths, book: &Book) {
    let f = paths.profiles_file();
    let tmp = f.with_extension(format!("json.tmp-{}", std::process::id()));
    let done = f.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| {
        std::fs::write(&tmp, serde_json::to_string_pretty(&book.to_json()).unwrap_or_default() + "\n")?;
        crate::paths::private(&tmp, 0o600)?;
        std::fs::rename(&tmp, &f)
    });
    if let Err(e) = done {
        let _ = std::fs::remove_file(&tmp);
        log(&format!("profiles.json: {}", e));
    }
}

/// A link said its profile: known, the default if none; saved when that changed.
pub(super) fn seen(sh: &Arc<Shared>, browser: &str, p: &Profile) {
    let changed = {
        let mut inner = lock(&sh.inner);
        inner.book.seen(browser, p).then(|| inner.book.clone())
    };
    if let Some(b) = changed {
        save(&sh.opts.paths, &b);
    }
}

/// ctl `default` / `forget` ({browser, dir}): his click in Settings or the CLI.
pub(super) fn edit(sh: &Arc<Shared>, op: &str, args: &Value) -> Reply {
    let Some(key) = choose::Key::from_json(args) else {
        return Err(err("bad_args", "name a profile: {\"browser\": \"chrome\", \"dir\": \"Profile 1\"}"));
    };
    let book = {
        let mut inner = lock(&sh.inner);
        match op {
            "default" => inner.book.set_default(&key).map_err(|m| err("not_found", m))?,
            _ => {
                if !inner.book.forget(&key) {
                    return Err(err("not_found", format!("no profile {} in {} is known", key.dir, key.browser)));
                }
            }
        }
        inner.book.clone()
    };
    save(&sh.opts.paths, &book);
    write_state(sh);
    Ok(json!({op: key.to_json()}))
}
