//! `/keychain [on|off]` (designer, m_13193): the same step as `bise
//! secrets keychain on|off` (bise_catalog::secrets_cli), its lines in the
//! feed as info lines (errors as warnings). A move runs in a thread (a
//! locked keychain takes ~2 s an item); its lines reach the feed at the
//! next [`pump`]. Bare: where the secrets are now, nothing else.

use std::sync::Mutex;

use bise_secrets::Store;

use crate::wire::Ev;

/// (an error, the line) from the move in its thread.
static LINES: Mutex<Vec<(bool, String)>> = Mutex::new(Vec::new());

fn paths() -> bise_catalog::auth_cli::Paths {
    let h = bise_home::Home::from_env();
    bise_catalog::auth_cli::Paths { auth_file: h.auth_file(), config: h.config_file(), env_files: vec![], home: Some(h.user_home().to_path_buf()) }
}

fn evs(o: bise_catalog::secrets_cli::Outcome) -> Vec<Ev> {
    o.lines.into_iter().map(|l| if o.failed { Ev::Warn(l) } else { Ev::Info(l) }).collect()
}

/// What `/keychain [on|off]` says now (a move: nothing, its lines come
/// with [`pump`]).
pub(crate) fn command(typed: &str) -> Vec<Ev> {
    let to = match typed.split_whitespace().nth(1) {
        None => None,
        Some("on") => Some(Store::Keychain),
        Some("off") => Some(Store::File),
        Some(_) => return vec![Ev::Warn("/keychain takes on or off.".into())],
    };
    let macos = cfg!(target_os = "macos");
    let mcp = bend_plugins::oauth::store_dir();
    if to.is_none() || !macos {
        return evs(bise_catalog::secrets_cli::switch(&paths(), &mcp, to, macos));
    }
    std::thread::spawn(move || {
        let o = bise_catalog::secrets_cli::switch(&paths(), &mcp, to, true);
        let mut l = LINES.lock().unwrap_or_else(|e| e.into_inner());
        l.extend(o.lines.into_iter().map(|x| (o.failed, x)));
    });
    vec![]
}

/// The setting now (the `/` popup marks it `· now`).
pub(crate) fn now() -> Store {
    bise_secrets::setting::current()
}

/// Into the feed: the lines of a move that ended.
pub(crate) fn pump(app: &mut crate::app::App) {
    let lines = std::mem::take(&mut *LINES.lock().unwrap_or_else(|e| e.into_inner()));
    for (failed, l) in lines {
        let ev = if failed { Ev::Warn(l) } else { Ev::Info(l) };
        crate::feed::push_event(&mut app.events, &mut app.cache, ev);
    }
}
