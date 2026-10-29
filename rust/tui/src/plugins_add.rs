//! The add menu's effects for the terminal's Plugins screen
//! (plugins_screen.rs asks for them, plugins.rs hands them here):
//! bend_plugins::add (stage a preview, install on his ⏎, discard on esc)
//! and bend_plugins::marketplace (add one, read its plugins for Browse).
//! A stage or a marketplace read may clone a git repo: each runs on its
//! own thread and its result waits in [`DONE`] until the next frame
//! ([`pump`]). Nothing a plugin holds runs before his ⏎ on its preview.

use crate::plugins_screen::{Add, Effect, Found, Market, Preview};
use bend_plugins::add::{self, AddError, Ctx, Request, Staged};
use bise_proto::words::settings;
use std::sync::Mutex;

/// What a background step left for the next frame.
enum Done {
    Found(Vec<add::Found>),
    Staged(Box<Result<Staged, AddError>>),
    Markets(Vec<(Market, bend_plugins::marketplace::Market)>),
}

static DONE: Mutex<Vec<Done>> = Mutex::new(Vec::new());
/// The add staged and shown, until his ⏎ or esc.
static STAGED: Mutex<Option<Staged>> = Mutex::new(None);
/// The configs found, as plug-add gave them (the screen holds their
/// words; [`Effect::StageImport`] names one by index).
static FOUND: Mutex<Vec<add::Found>> = Mutex::new(Vec::new());
/// The marketplaces read, for [`Effect::StageItem`].
static MARKETS: Mutex<Vec<bend_plugins::marketplace::Market>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn done(d: Done) {
    lock(&DONE).push(d);
}

/// The words of a failed add, picked by its kind.
pub(crate) fn error_words(e: &AddError) -> String {
    match e {
        AddError::Exists { name } => settings::plugins_err_exists(name),
        AddError::BadName { name } => settings::plugins_err_bad_name(name),
        AddError::NoPlugin => settings::plugins_err_no_plugin.to_string(),
        AddError::Invalid { diagnostics } => format!("{}\n{}", settings::plugins_err_invalid, diagnostics.join("\n")),
        AddError::NoServer { why } => format!("{}: {}", settings::plugins_err_no_server, why.join("; ")),
        AddError::UnknownToken => settings::plugins_err_gone.to_string(),
        AddError::UnknownSecret { name } => settings::plugins_err_why(name),
        AddError::BadSource { why } | AddError::Git { why } | AddError::Secrets { why } | AddError::Io { why } => settings::plugins_err_why(why),
    }
}

fn from_word(f: &bise_proto::app_cmd::ImportFrom) -> String {
    use bise_proto::app_cmd::ImportFrom;
    match f {
        ImportFrom::Claude => "Claude Code",
        ImportFrom::Codex => "Codex",
        ImportFrom::Cursor => "Cursor",
    }
    .to_string()
}

fn home_short(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && s.starts_with(&h) => format!("~{}", &s[h.len()..]),
        _ => s,
    }
}

/// plug-add's preview as the screen's lines.
fn preview_of(p: &bise_proto::app_rows::PluginPreview) -> Preview {
    let runs = p
        .runs
        .iter()
        .map(|r| (r.server.clone(), r.host.clone().unwrap_or_else(|| r.argv.join(" "))))
        .collect();
    Preview {
        name: p.name.clone(),
        from: p.from.clone(),
        runs,
        skills: p.skills.clone(),
        secrets: p.secrets.clone(),
        dropped: p.dropped.clone(),
        notes: p.notes.clone(),
        replaces: p.replaces.clone(),
        views: p.views.clone(),
    }
}

/// `KEY=value` pairs from the env field (`;` or newlines between them).
fn secrets_of(env: &str) -> Vec<(String, String)> {
    env.split([';', '\n'])
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn stage(req: Request) {
    std::thread::spawn(move || done(Done::Staged(Box::new(add::stage(&req, &Ctx::standard())))));
}

/// Run an add effect.
pub(crate) fn run(app: &mut crate::app::App, e: Effect) {
    match e {
        Effect::Found => {
            std::thread::spawn(|| done(Done::Found(add::found(&Ctx::standard()))));
        }
        Effect::StageMcp { name, target, env } => {
            let url = target.contains("://").then(|| target.clone());
            let command = url.is_none().then_some(target);
            stage(Request::Mcp { name, command, url, secrets: secrets_of(&env) });
        }
        Effect::StagePlugin(source) => {
            let p = std::path::PathBuf::from(source.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1));
            stage(if p.is_dir() { Request::Folder { path: p } } else { Request::Git { url: source, subdir: None } });
        }
        Effect::StageImport(i) => {
            if let Some(f) = lock(&FOUND).get(i) {
                stage(Request::Import { from: f.from, name: None });
            }
        }
        Effect::StageItem(url, id) => {
            let ms = lock(&MARKETS);
            let req = ms.iter().filter(|m| m.url == url).find_map(|m| {
                let rows = bend_plugins::marketplace::rows(m);
                m.items.iter().zip(rows).find(|(_, r)| r.id == id).map(|(i, _)| bend_plugins::marketplace::request(m, i))
            });
            if let Some(r) = req {
                stage(r);
            }
        }
        Effect::Install => {
            let Some(staged) = lock(&STAGED).take() else { return };
            let name = staged.preview.name.clone();
            let ctx = Ctx::standard();
            let r = add::install(staged, &ctx);
            if let Some(o) = app.plugins.as_mut() {
                match r {
                    Ok(_) => {
                        o.sc.add = None;
                        o.sc.note = Some((settings::plugins_added(&name), false));
                    }
                    Err(e) => {
                        if let Some(Add::Preview { error, .. }) = o.sc.add.as_mut() {
                            *error = Some(error_words(&e));
                        }
                    }
                }
            }
        }
        Effect::Discard => {
            if let Some(s) = lock(&STAGED).take() {
                add::discard(s);
            }
        }
        Effect::MarketAdd(url) => {
            let path = bise_home::Home::from_env().marketplaces();
            let r = bend_plugins::marketplace::add(&path, &url);
            if let Some(o) = app.plugins.as_mut() {
                match r {
                    Ok((_, url)) => {
                        o.sc.add = None;
                        o.sc.browse = true;
                        o.sc.note = Some((settings::plugins_market_added(&url), false));
                        read_markets(true);
                    }
                    Err(e) => {
                        if let Some(Add::Form { error, .. }) = o.sc.add.as_mut() {
                            *error = Some(error_words(&e));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Read his marketplaces for Browse, in the background (`refresh`:
/// clone again).
pub(crate) fn read_markets(refresh: bool) {
    std::thread::spawn(move || {
        let home = bise_home::Home::from_env();
        let out = bend_plugins::marketplace::list(&home.marketplaces())
            .into_iter()
            .filter_map(|url| bend_plugins::marketplace::read(&home.marketplace_cache(), &url, refresh).ok())
            .map(|m| (Market { url: m.url.clone(), items: bend_plugins::marketplace::rows(&m) }, m))
            .collect();
        done(Done::Markets(out));
    });
}

/// Into the open screen: what the background steps left.
pub(crate) fn pump(app: &mut crate::app::App) {
    let ds: Vec<Done> = std::mem::take(&mut *lock(&DONE));
    for d in ds {
        let Some(o) = app.plugins.as_mut() else {
            if let Done::Staged(r) = d {
                if let Ok(s) = *r {
                    add::discard(s);
                }
            }
            continue;
        };
        match d {
            Done::Found(f) => {
                let words: Vec<Found> = f
                    .iter()
                    .map(|x| Found { from: from_word(&x.from), path: home_short(&x.path), servers: x.servers.clone(), added: x.added.clone() })
                    .collect();
                o.found = Some(words.clone());
                if let Some(Add::Import { found, .. }) = o.sc.add.as_mut() {
                    *found = Some(words);
                }
                *lock(&FOUND) = f;
            }
            Done::Staged(r) => match (*r, o.sc.add.as_ref()) {
                // he left the step while it staged
                (Ok(s), a) if !matches!(a, Some(Add::Staging { .. })) => add::discard(s),
                (Ok(s), a) => {
                    let mut preview = preview_of(&s.preview);
                    // the title he picked keeps its case (`add LaTeX?`)
                    if let Some(Add::Staging { name }) = a {
                        if name.eq_ignore_ascii_case(&preview.name) {
                            preview.name = name.clone();
                        }
                    }
                    o.sc.add = Some(Add::Preview { preview, error: None });
                    if let Some(old) = lock(&STAGED).replace(s) {
                        add::discard(old);
                    }
                }
                (Err(e), Some(Add::Staging { .. })) => {
                    o.sc.add = None;
                    o.sc.note = Some((format!("✗ {}", error_words(&e)), true));
                }
                (Err(_), _) => {}
            },
            Done::Markets(ms) => {
                o.markets = ms.iter().map(|(m, _)| m.clone()).collect();
                *lock(&MARKETS) = ms.into_iter().map(|(_, m)| m).collect();
            }
        }
    }
}
