//! `/plugins` in the TUI (docs/plugins.md).
//!
//! - [`rows`]: the Plugins list (bend_plugins::catalog::rows), for the
//!   desktop's core and the TUI's screen; [`connect`] and [`refresh`]
//!   run a connector's connect and a catalog fetch on their own thread.
//! - `/plugins`: the plugins of the workspace (the static listing).
//! - `/plugins on|off ID`: edits `plugins.json` (bend_plugins::state);
//!   applies at the next `/reload` (or task restart).
//! - `/plugins login [NAME]`: the browser login of a remote MCP server
//!   (bend_plugins::login), in the background: its lines come back
//!   through [`pump`]; every agent's bridge sees the new tokens and
//!   connects the server. `/plugins logout NAME` forgets them.
//! - `/plugins remove NAME`: asks once; `/plugins remove NAME yes`
//!   removes a plugin of his folder with its secrets
//!   (bend_plugins::remove).
//! - [`pump`] also says once per server and per TUI run, when a session
//!   found that one needs a login: `linear needs a login: /plugins login`.

use bise_proto::words::settings;
use std::path::Path;

/// The Plugins list of `workspace` (the desktop's `plugins` and `browse`
/// events, the TUI's /plugins screen): bend_plugins::catalog::rows, the
/// one builder, on what each owner reads now (`pending`: the logins whose
/// browser is open, by name). The connectors from their cache only, never
/// a fetch ([`refresh`] fetches, on its own thread).
pub(crate) fn rows(workspace: &Path, pending: &[String]) -> Vec<bise_proto::app_rows::Plugin> {
    use bend_plugins::{catalog, connectors, oauth, resolve, skills, state, status};
    let res = resolve::resolve(&resolve::Roots::standard(Some(workspace)));
    let cat = connectors::cached(&bise_home::Home::from_env().connectors_cache());
    let st = state::read(&state::state_path());
    let grants = state::grants(&state::state_path());
    let (sd, od) = (status::dir(), oauth::store_dir());
    let secrets = bise_home::Home::from_env().mcp_secrets_dir();
    // the skills folders main reads (the user's view), off ones too; a
    // short wait per folder: a stuck one is skipped (bend_plugins::skills)
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(std::path::PathBuf::from);
    let app = resolve::builtin_root().and_then(|r| r.parent().map(Path::to_path_buf));
    let bise = skills::BiseSkills::of(true, bise_home::projects::desktop_for(workspace));
    let found = skills::scan(&skills::roots(home.as_deref(), workspace, app.as_deref(), bise), std::time::Duration::from_millis(300));
    let inputs = catalog::Inputs {
        resolution: &res,
        catalog: &cat,
        skills: &found.skills,
        state: &st,
        grants: &grants,
        status_dir: Some(&sd),
        oauth_dir: Some(&od),
        pending_logins: pending,
        secrets_dir: Some(&secrets),
    };
    catalog::rows(&inputs)
}

/// Why his Mistral connectors may be missing or old (their cache's last
/// problem), for the `plugins` and `browse {mistral}` events.
pub(crate) fn connectors_problem() -> Option<bise_proto::app_rows::ConnectorsProblem> {
    let cat = bend_plugins::connectors::cached(&bise_home::Home::from_env().connectors_cache());
    bend_plugins::catalog::problem(&cat)
}

/// Connect Mistral connector `id` (`plugin_connect`, `/plugins
/// connect`): his browser or one click, then the catalog polled until it
/// is ready (up to 5 min), all on its own thread, never a draw or core
/// loop; `say` gets each step's line, `done` its end.
pub(crate) fn connect(id: &str, say: impl Fn(String) + Send + 'static, done: impl FnOnce(Result<(), String>) + Send + 'static) -> Result<(), String> {
    let id = bend_plugins::id::PluginId::parse(id)?;
    if !matches!(id, bend_plugins::id::PluginId::Mistral { .. }) {
        return Err(format!("{id} is not a Mistral connector"));
    }
    std::thread::spawn(move || {
        let r = bend_plugins::connectors::connect_now(&id, &|l| say(l.to_string()));
        done(r.map(|_| ()));
    });
    Ok(())
}

/// Log in account `account` of ready connector `id`
/// (`plugin_account_connect`, `/plugins` on a connector): another
/// account, or one to reconnect; his browser, then the list polled until
/// it works (up to 5 min), on its own thread; `say` gets each step's
/// line, `done` its end.
pub(crate) fn connect_account(id: &str, account: &str, say: impl Fn(String) + Send + 'static, done: impl FnOnce(Result<(), String>) + Send + 'static) -> Result<(), String> {
    let id = bend_plugins::id::PluginId::parse(id)?;
    if !matches!(id, bend_plugins::id::PluginId::Mistral { .. }) {
        return Err(format!("{id} is not a Mistral connector"));
    }
    let account = account.trim().to_string();
    if account.is_empty() {
        return Err("no account named".into());
    }
    std::thread::spawn(move || done(bend_plugins::connectors::connect_account_now(&id, &account, &|l| say(l.to_string()))));
    Ok(())
}

/// The command that asks for `change` (its error's name).
pub(crate) fn change_cmd(change: &bend_plugins::connectors::AccountChange) -> &'static str {
    use bend_plugins::connectors::AccountChange;
    match change {
        AccountChange::Rename(_) => "plugin_account_rename",
        AccountChange::MakeDefault => "plugin_account_default",
        AccountChange::Remove => "plugin_account_remove",
    }
}

/// One change to account `account` of connector `id` on Mistral
/// (`plugin_account_rename|default|remove`), then the catalog read
/// again, on its own thread; `done` with why not, if it failed.
pub(crate) fn change_account(id: &str, account: &str, change: bend_plugins::connectors::AccountChange, done: impl FnOnce(Result<(), String>) + Send + 'static) {
    let (id, account) = (id.to_string(), account.to_string());
    std::thread::spawn(move || {
        let r = bend_plugins::id::PluginId::parse(&id).and_then(|id| bend_plugins::connectors::change_account_now(&id, &account, &change));
        done(r);
    });
}

/// The Mistral catalog fetched again now (`plugins_refresh`, `/plugins
/// refresh`), on its own thread; `done` with the fetch's problem, if any.
pub(crate) fn refresh(done: impl FnOnce(Option<String>) + Send + 'static) {
    std::thread::spawn(move || {
        let cat = bend_plugins::connectors::refresh();
        done(cat.error.map(|p| p.to_string()));
    });
}

/// Turn plugin or part `id` (any `PluginId`) on or off through
/// bend_plugins' one writer.
pub(crate) fn set(id: &str, on: bool) -> Result<bool, String> {
    let id = bend_plugins::id::PluginId::parse(id)?;
    let path = bend_plugins::state::state_path();
    bend_plugins::state::set(&path, &id, on).map_err(|e| format!("{}: {e}", path.display()))
}

/// Grant view `uri` of plugin `plugin` its permissions `perms` (the add
/// screen's views step, the VIEWS group; docs/mcp-apps.md C8): through
/// bend_plugins::state's one writer. Returns whether the file changed.
pub(crate) fn grant(plugin: &str, uri: &str, perms: &[String]) -> Result<bool, String> {
    let id = bend_plugins::id::PluginId::plugin(plugin).part(bend_plugins::id::PartKind::View, None, uri);
    let path = bend_plugins::state::state_path();
    bend_plugins::state::grant(&path, &id, perms).map_err(|e| format!("{}: {e}", path.display()))
}

/// His pick of the app that opens type `key` (Open with's `Always open
/// with`, Settings › Plugins › File types; docs/plugin-apps.md A2): `to`
/// (plugin, tool), none for A2's order, through bend_plugins::state's
/// one writer. Returns whether the file changed.
pub(crate) fn opener_set(key: &str, to: Option<(&str, &str)>) -> Result<bool, String> {
    let to = to.map(|(plugin, tool)| bend_plugins::state::OpenerRef { plugin: plugin.into(), tool: tool.into() });
    let path = bend_plugins::state::state_path();
    bend_plugins::state::pick_set(&path, key, to.as_ref()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Give (or forget: `value` none) setting `var` of plugin `name` (the
/// plugins page's field, `/plugins set NAME VAR`): its declared settings
/// only, kept by bend_plugins::secrets' one writer (the add menu's). The
/// bridge restarts its servers with it.
pub(crate) fn setting(workspace: &Path, name: &str, var: &str, value: Option<&str>) -> Result<(), String> {
    use bend_plugins::{resolve, secrets};
    let res = resolve::resolve(&resolve::Roots::standard(Some(workspace)));
    let p = res.plugins.iter().find(|p| p.name == name).ok_or_else(|| secrets::SettingError::NoPlugin(name.into()).to_string())?;
    secrets::set_setting(&bise_home::Home::from_env().mcp_secrets_dir(), p, var, value).map_err(|e| e.to_string())
}

/// The setting the composer asks a value for (`/plugins set NAME VAR`,
/// or ⏎ on its line in the screen): the next ⏎ saves what he typed through
/// [`setting`]; it is never sent, never in the history, a draft or the
/// feed. esc keeps the value as it was.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ask {
    pub(crate) plugin: String,
    pub(crate) var: String,
    pub(crate) label: String,
    pub(crate) secret: bool,
}

/// Ask for `a`'s value: the composer empties and waits for it (shown as •
/// when secret); the line that asks.
pub(crate) fn start_ask(app: &mut crate::app::App, a: Ask) -> String {
    app.ed.clear();
    let line = if a.secret { settings::plugins_setting_prompt(&a.label) } else { settings::plugins_setting_prompt_plain(&a.label) };
    app.plugin_ask = Some(a);
    line
}

/// The composer types a secret setting: each character shows as •.
pub(crate) fn masked(app: &crate::app::App) -> bool {
    app.plugin_ask.as_ref().is_some_and(|a| a.secret)
}

/// The composer's tab while it asks: the setting, `· hidden` for a
/// secret, dim, no agent (the keystrokes go to none, designer m_51698).
pub(crate) fn ask_label(app: &crate::app::App) -> Option<Vec<ratatui::text::Span<'static>>> {
    let a = app.plugin_ask.as_ref()?;
    let words = if a.secret { settings::plugins_setting_tab(&a.label) } else { a.label.clone() };
    Some(vec![ratatui::text::Span::styled(format!(" {words} "), ratatui::style::Style::default().fg(crate::theme::dim()))])
}

/// The key bar while it asks: ⏎ save, esc keep it as is.
pub(crate) const ASK_KEYS: [(&str, &str); 2] = [("⏎", settings::plugins_setting_key_save), ("esc", settings::plugins_setting_key_keep)];

/// ⏎ while it asks: the value saved (or nothing, for an empty line); the
/// line that says so.
pub(crate) fn answer(app: &mut crate::app::App) -> crate::wire::Ev {
    let typed = app.ed.take();
    let Some(a) = app.plugin_ask.take() else { return crate::wire::Ev::Info(String::new()) };
    if typed.trim().is_empty() {
        return crate::wire::Ev::Info(settings::plugins_setting_kept.to_string());
    }
    let ws = crate::sb::workspace(app).unwrap_or_default();
    match setting(Path::new(&ws), &a.plugin, &a.var, Some(&typed)) {
        Ok(()) => crate::wire::Ev::Info(settings::plugins_setting_saved.to_string()),
        Err(e) => crate::wire::Ev::Err(e),
    }
}

/// esc while it asks: nothing written.
pub(crate) fn keep(app: &mut crate::app::App) -> crate::wire::Ev {
    app.ed.clear();
    app.plugin_ask = None;
    crate::wire::Ev::Info(settings::plugins_setting_kept.to_string())
}

/// Forget the login of server `name` (`/plugins logout NAME`).
pub(crate) fn logout(workspace: &Path, name: &str) -> Result<String, String> {
    let ts = bend_plugins::login::targets(workspace);
    let t = bend_plugins::login::find(&ts, name)?;
    bend_plugins::login::logout(t, &bend_plugins::oauth::store_dir(), Some(&bend_plugins::status::dir()))?;
    Ok(t.name.clone())
}

/// Remove plugin `name` of his plugins folder (`/plugins remove NAME
/// yes`): bend_plugins::remove, its refusals as their words.
pub(crate) fn remove(workspace: &Path, name: &str) -> Result<(), String> {
    let id = bend_plugins::id::PluginId::parse(name)?;
    bend_plugins::remove::remove(&bend_plugins::add::Ctx::standard(), Some(workspace), &id).map_err(|e| e.to_string())
}

/// The plugins of `workspace` matching `q` (the `/plugins on|off`
/// argument, BISE-117), from the rows the screen shows: `blender   plugin
/// · 1 skill · 23 tools · on`; once he types, their parts and tools too
/// (`blender#tool:blender/execute_python   a tool of blender · on`).
/// The rows are read again at most every 5 s: the popup asks at every
/// frame. `/skills on|off`: the skills folders and their skills.
pub(crate) fn choices(workspace: &Path, q: &str, typed: &str) -> Vec<crate::commands::Choice> {
    if matches!(typed.split_whitespace().nth(1), Some("login" | "logout")) {
        return login_choices(workspace, q);
    }
    if typed.split_whitespace().nth(1) == Some("connect") {
        return connect_choices(&cached_rows(workspace), q);
    }
    let skills = typed.starts_with("/skills");
    let all = cached_rows(workspace);
    let state = |on: bool| if on { settings::plugins_mark_on } else { settings::plugins_mark_off };
    let mut out = Vec::new();
    for p in all.iter().filter(|p| p.state != bise_proto::app_rows::PluginState::Available) {
        if skills && p.kind != bise_proto::app_rows::PluginKind::Skills && !p.parts.iter().any(|x| x.kind == bise_proto::app_rows::PartKind::Skill) {
            continue;
        }
        if crate::commands::matches(q, &[&p.id, &p.title]) {
            let desc = [screen::kind_word(p), &screen::summary(p), state(p.on)].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
            out.push(crate::commands::Choice { value: p.id.clone(), label: p.id.clone(), desc, mark: None });
        }
        if q.is_empty() {
            continue;
        }
        for x in p.parts.iter().flat_map(|x| std::iter::once(x).chain(x.tools.iter())) {
            if skills && x.kind != bise_proto::app_rows::PartKind::Skill {
                continue;
            }
            if crate::commands::matches(q, &[&x.id]) {
                let desc = settings::plugins_choice_part(screen::part_word(x.kind), &p.title, state(x.on));
                out.push(crate::commands::Choice { value: x.id.clone(), label: x.id.clone(), desc, mark: None });
            }
        }
    }
    out
}

/// `/plugins connect`'s rows: the connectors that wait for him, then
/// those in Browse.
fn connect_choices(all: &[Plugin], q: &str) -> Vec<crate::commands::Choice> {
    all.iter()
        .filter(|p| p.kind == bise_proto::app_rows::PluginKind::Mistral && p.action.is_some())
        .filter(|p| crate::commands::matches(q, &[&p.id, &p.title]))
        .map(|p| crate::commands::Choice { value: p.id.clone(), label: p.title.clone(), desc: p.what.clone().unwrap_or_default(), mark: None })
        .collect()
}

/// The popup's rows: the worker's latest (the popup asks at every
/// frame), a new read asked when they are older than 5 s.
fn cached_rows(workspace: &Path) -> Vec<Plugin> {
    let fresh = lock(&BUILT).as_ref().is_some_and(|b| b.workspace == workspace && b.at.elapsed() < Duration::from_secs(5));
    if !fresh {
        ask_rows(workspace);
    }
    lock(&BUILT).as_ref().filter(|b| b.workspace == workspace).map(|b| b.rows.clone()).unwrap_or_default()
}

// ---- the rows' worker ----

/// Rows the worker built, for one workspace, at one moment.
struct Built {
    workspace: std::path::PathBuf,
    at: Instant,
    rows: Vec<Plugin>,
}

/// The worker's latest rows; whether it runs; whether it must run again
/// (asked while it ran: a toggle it may have read before).
static BUILT: Mutex<Option<Built>> = Mutex::new(None);
static BUSY: Mutex<bool> = Mutex::new(false);
static AGAIN: Mutex<bool> = Mutex::new(false);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build the rows off the draw thread (`catalog::rows` reads files and
/// scans skills folders): one build at a time; asked while one runs, it
/// runs once more after.
fn ask_rows(workspace: &Path) {
    {
        let mut busy = lock(&BUSY);
        if *busy {
            *lock(&AGAIN) = true;
            return;
        }
        *busy = true;
    }
    let ws = workspace.to_path_buf();
    std::thread::spawn(move || loop {
        let rows = rows(&ws, &pending());
        *lock(&BUILT) = Some(Built { workspace: ws.clone(), at: Instant::now(), rows });
        let mut busy = lock(&BUSY);
        if !std::mem::take(&mut *lock(&AGAIN)) {
            *busy = false;
            return;
        }
    });
}

/// `/plugins login`'s rows (designer): `linear   mcp.linear.app · needs
/// a login`, `linear   mcp.linear.app · logged in · 23 tools`.
fn login_choices(workspace: &Path, q: &str) -> Vec<crate::commands::Choice> {
    let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
    bend_plugins::login::targets(workspace)
        .into_iter()
        .filter(|t| crate::commands::matches(q, &[&t.name]))
        .map(|t| {
            let desc = bend_plugins::login::state(&t, &secrets, &sd);
            crate::commands::Choice { value: t.name.clone(), label: t.name, desc, mark: None }
        })
        .collect()
}

use std::sync::Mutex;

/// Lines for the thread from the background logins, and the logins
/// running (one per server at a time).
static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static RUNNING: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Addresses pasted with `/plugins login NAME <address>`, for the login
/// of NAME that waits (the browser on another machine).
static PASTED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn say(l: String) {
    LINES.lock().unwrap_or_else(|e| e.into_inner()).push(l);
}

/// `/plugins login NAME <address>`: the address a browser elsewhere
/// ended on, for the login of NAME that waits.
fn paste(workspace: &Path, name: &str, address: &str) -> String {
    let ts = bend_plugins::login::targets(workspace);
    let t = match bend_plugins::login::find(&ts, name) {
        Ok(t) => t.clone(),
        Err(e) => return e,
    };
    if !RUNNING.lock().unwrap_or_else(|e| e.into_inner()).contains(&t.name) {
        return bend_plugins::login::no_login_waiting(&t.name);
    }
    PASTED.lock().unwrap_or_else(|e| e.into_inner()).push((t.name, address.to_string()));
    String::new()
}

/// `/plugins login NAME`: the login in a thread; the first line now.
fn login(workspace: &Path, name: &str) -> String {
    let ts = bend_plugins::login::targets(workspace);
    let t = match bend_plugins::login::find(&ts, name) {
        Ok(t) => t.clone(),
        Err(e) => return e,
    };
    {
        let mut r = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if r.contains(&t.name) {
            return settings::plugins_login_open(&t.name);
        }
        r.push(t.name.clone());
    }
    let first = settings::plugins_login_opening(&t.name);
    std::thread::spawn(move || {
        let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
        let name = t.name.clone();
        // the link always (another browser, another machine)
        let open = move |u: &str| {
            let _ = bend_plugins::oauth::open_browser(u);
            for l in bend_plugins::login::tui_link_lines(&name, u) {
                say(l);
            }
            Ok(())
        };
        PASTED.lock().unwrap_or_else(|e| e.into_inner()).retain(|(n, _)| *n != t.name);
        let next = || {
            let mut p = PASTED.lock().unwrap_or_else(|e| e.into_inner());
            let i = p.iter().position(|(n, _)| *n == t.name)?;
            Some(p.remove(i).1)
        };
        let bad = || say(bend_plugins::login::bad_paste_tui(&t.name));
        let paste = bend_plugins::oauth::Paste { next: &next, bad: &bad };
        let r = bend_plugins::login::run(&t, &secrets, Some(&sd), &open, bend_plugins::login::WAIT, None, Some(&paste));
        say(bend_plugins::login::done_line(&t.name, &r));
        RUNNING.lock().unwrap_or_else(|e| e.into_inner()).retain(|n| *n != t.name);
    });
    first
}

/// The servers already said to need a login (once per TUI run), and
/// when the status files were last read.
struct Watch {
    said: Vec<String>,
    at: Option<std::time::Instant>,
}

static WATCH: Mutex<Watch> = Mutex::new(Watch { said: Vec::new(), at: None });

/// The lines that wait, then (every 5 s) a quiet line for a server a
/// session found needing a login: `linear needs a login: /plugins login`.
pub(crate) fn pending_lines(workspace: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::mem::take(&mut *LINES.lock().unwrap_or_else(|e| e.into_inner()));
    let mut w = WATCH.lock().unwrap_or_else(|e| e.into_inner());
    if w.at.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(5)) {
        return out;
    }
    w.at = Some(std::time::Instant::now());
    let sd = bend_plugins::status::dir();
    let running = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for t in bend_plugins::login::targets(workspace) {
        let needs = bend_plugins::status::read(&sd, &t.plugin, &t.server.id).is_some_and(|s| s.login);
        if needs && !w.said.contains(&t.name) && !running.contains(&t.name) {
            w.said.push(t.name.clone());
            out.push(settings::plugins_needs_login(&t.name));
        } else if !needs {
            // logged in since: a later 401 is said again
            w.said.retain(|n| *n != t.name);
        }
    }
    out
}

/// Into the feed: the background logins' lines and the quiet ones.
pub(crate) fn pump(app: &mut crate::app::App) {
    let ws = crate::sb::workspace(app).unwrap_or_default();
    if ws.is_empty() {
        return;
    }
    for l in pending_lines(Path::new(&ws)) {
        crate::feed::push_event(&mut app.events, &mut app.cache, crate::wire::Ev::Info(l));
    }
}

/// The text `/plugins [args]` prints.
pub(crate) fn command(typed: &str, workspace: &Path) -> String {
    let mut words = typed.split_whitespace().skip(1);
    match (words.next(), words.next()) {
        (Some("login"), Some(name)) => match words.next() {
            Some(address) => paste(workspace, name, address),
            None => login(workspace, name),
        },
        (Some(sub @ ("login" | "logout")), None) => {
            let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
            let ts = bend_plugins::login::targets(workspace);
            if ts.is_empty() {
                return settings::plugins_no_login.into();
            }
            let rows: Vec<String> = ts.iter().map(|t| format!("  {}   {}", t.name, bend_plugins::login::state(t, &secrets, &sd))).collect();
            format!("/plugins {} NAME:\n{}", sub, rows.join("\n"))
        }
        (Some("logout"), Some(name)) => match logout(workspace, name) {
            Ok(n) => settings::plugins_logged_out(n),
            Err(e) => e,
        },
        // asked once: the same line with `yes` removes it
        (Some("remove"), Some(name)) => match words.next() {
            Some("yes") => match remove(workspace, name) {
                Ok(()) => settings::plugins_removed(name),
                Err(e) => e,
            },
            _ => settings::plugins_remove_ask(name),
        },
        (Some(sub @ ("on" | "off")), Some(name)) => match set(name, sub == "on") {
            Ok(changed) => settings::plugins_set(
                name,
                if sub == "on" { settings::plugins_enabled } else { settings::plugins_disabled },
                if changed { "" } else { settings::plugins_unchanged },
            ),
            Err(e) => e,
        },
        (None, _) | (Some("list"), _) => {
            let text = bend_plugins::cli::listing(workspace);
            format!("plugins ({})\n{}", workspace.display(), text.trim_end())
        }
        _ => settings::plugins_usage.into(),
    }
}


// ---- the screen (plugins_screen.rs: its state, lines and keys) ----

use crate::plugins_screen::{self as screen, Effect, Found, Market};
use bise_proto::app_rows::Plugin;
use std::time::{Duration, Instant};

/// How often the open screen reads the rows again (a toggle elsewhere, a
/// login that ended, a server that connected).
const REREAD: Duration = Duration::from_secs(2);
/// How long a note stays on the divider.
const NOTE_FOR: Duration = Duration::from_secs(6);

/// `/plugins` open: the pure screen and what the shell read for it.
pub(crate) struct Open {
    pub(crate) sc: screen::Screen,
    pub(crate) rows: Vec<Plugin>,
    pub(crate) markets: Vec<Market>,
    pub(crate) found: Option<Vec<Found>>,
    /// when the worker was last asked, and the build shown
    read_at: Option<Instant>,
    got: Option<Instant>,
    note_at: Option<Instant>,
    /// screen row → the id it selects (the last frame's)
    hits: Vec<(u16, String)>,
}

impl Open {
    pub(crate) fn new(sc: screen::Screen) -> Open {
        Open { sc, rows: Vec::new(), markets: Vec::new(), found: None, read_at: None, got: None, note_at: None, hits: Vec::new() }
    }

    fn note(&mut self, words: String, bad: bool) {
        self.sc.note = Some((words, bad));
        self.note_at = Some(Instant::now());
    }
}

/// The logins open in his browser, by server name.
fn pending() -> Vec<String> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `/plugins` (every kind) or `/skills` (the skills folders), the add
/// menu open when `add`.
pub(crate) fn open(app: &mut crate::app::App, kind: screen::Kind, add: bool) {
    let sc = screen::Screen { kind, add: add.then_some(screen::Add::Menu { sel: 0 }), ..Default::default() };
    app.plugins = Some(Open::new(sc));
    // Browse's marketplaces, from their clones (cloned once if missing)
    crate::plugins_add::read_markets(false);
    reread(app, true);
}

/// Ask the worker for the rows again when [`REREAD`] passed (or `now`:
/// after his toggle), and take its latest.
fn reread(app: &mut crate::app::App, now: bool) {
    let ws = std::path::PathBuf::from(crate::sb::workspace(app).unwrap_or_default());
    let Some(o) = app.plugins.as_mut() else { return };
    if now || o.read_at.is_none_or(|t| t.elapsed() >= REREAD) {
        o.read_at = Some(Instant::now());
        ask_rows(&ws);
    }
    if let Some(b) = lock(&BUILT).as_ref().filter(|b| b.workspace == ws && o.got.is_none_or(|g| b.at > g)) {
        o.rows = b.rows.clone();
        o.got = Some(b.at);
    }
    // a failure stays until his next key
    if o.note_at.is_some_and(|t| t.elapsed() >= NOTE_FOR) && !o.sc.note.as_ref().is_some_and(|(_, bad)| *bad) {
        o.sc.note = None;
        o.note_at = None;
    }
    screen::settle(&mut o.sc, &o.rows, &o.markets);
}

pub(crate) fn draw(app: &mut crate::app::App, frame: &mut ratatui::Frame) {
    use ratatui::widgets::{Clear, Paragraph};
    if app.plugins.is_none() {
        return;
    }
    crate::plugins_add::pump(app);
    reread(app, false);
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 30 || full.height < 10 {
        return;
    }
    let Some(o) = app.plugins.as_ref() else { return };
    let title = screen::title(&o.sc, &o.rows);
    let area = crate::artifacts_screen::draw_frame(app, frame, full, &title);
    let Some(o) = app.plugins.as_mut() else { return };
    let (w, h) = (area.width as usize + 2, area.height as usize);
    let found = o.found.as_ref().map(|f| f.iter().map(|x| format!("{} {}", x.from, x.servers.len())).collect::<Vec<_>>().join(", "));
    let body = screen::body(&o.sc, &o.rows, &o.markets, found.as_deref(), w);
    // the lines take the frame's margin but its outer column (the `›`
    // of plugins-ui's screens sits there)
    let area = ratatui::layout::Rect { x: area.x.saturating_sub(1).max(full.x + 1), width: (area.width + 2).min(full.width.saturating_sub(2)), ..area };
    let (lines, hits) = screen::window(&mut o.sc, body, h.saturating_sub(2));
    o.hits = hits.into_iter().map(|(i, id)| (area.y + i as u16, id)).collect();
    let body_area = ratatui::layout::Rect { height: area.height.saturating_sub(2), ..area };
    frame.render_widget(Paragraph::new(lines), body_area);
    // the divider: `├─ plugins ── 27 · 1 off ─┤`, then the keys
    let y = area.bottom().saturating_sub(2);
    let (right, bad) = screen::divider_right(&o.sc, &o.rows);
    divider(frame.buffer_mut(), full, y, &title, &right, bad);
    let keys = screen::key_bar(&o.sc, &o.rows, w);
    frame.render_widget(Paragraph::new(keys), ratatui::layout::Rect { y: y + 1, height: 1, ..area });
    crate::textlayer::text(area);
    for (y, _) in &o.hits {
        crate::pointer::region(ratatui::layout::Rect { x: area.x, y: *y, width: area.width, height: 1 }, crate::pointer::Shape::Pointer);
    }
}

/// The screen's divider on row `y`: its name left, `right` right (the
/// note in the accent, a failure in the error color).
fn divider(buf: &mut ratatui::buffer::Buffer, full: ratatui::layout::Rect, y: u16, name: &str, right: &str, bad: bool) {
    use ratatui::style::Style;
    use unicode_width::UnicodeWidthStr;
    let st = crate::chrome::line_style();
    let (l, r) = (full.x, full.right().saturating_sub(1));
    if y >= full.bottom() || r <= l + 4 {
        return;
    }
    for x in l..=r {
        buf[(x, y)].set_symbol("─").set_style(st);
    }
    buf[(l, y)].set_symbol("├");
    buf[(r, y)].set_symbol("┤");
    let label = format!(" {name} ");
    buf.set_stringn(l + 2, y, &label, (r - l) as usize - 3, Style::default().fg(crate::theme::dim()));
    let room = (r - l) as usize - label.width() - 6;
    if !right.is_empty() && room > 4 {
        let t = format!(" {} ", screen_cut(right, room));
        let c = if bad { crate::theme::error() } else if right.starts_with('✓') { crate::theme::accent() } else { crate::theme::dim() };
        buf.set_string(r - 1 - t.width() as u16, y, &t, Style::default().fg(c));
    }
}

fn screen_cut(s: &str, w: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if s.width() <= w {
        return s.to_string();
    }
    let mut out: String = String::new();
    for c in s.chars() {
        if out.width() + 2 > w {
            break;
        }
        out.push(c);
    }
    out + "…"
}

/// Keys while open: the pure screen decides, the shell runs its effect.
pub(crate) fn on_key(app: &mut crate::app::App, k: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    let Some(o) = app.plugins.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let e = screen::key(&mut o.sc, &o.rows, &o.markets, k);
    run(app, e);
    true
}

/// The mouse while open: the wheel moves, a click selects a row, a click
/// on the selected one opens it.
pub(crate) fn mouse(app: &mut crate::app::App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
    let Some(o) = app.plugins.as_mut() else { return false };
    let press = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
    match m.kind {
        MouseEventKind::ScrollUp => {
            for _ in 0..3 {
                screen::key(&mut o.sc, &o.rows, &o.markets, &press(KeyCode::Up));
            }
        }
        MouseEventKind::ScrollDown => {
            for _ in 0..3 {
                screen::key(&mut o.sc, &o.rows, &o.markets, &press(KeyCode::Down));
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(id) = o.hits.iter().find(|(y, _)| *y == m.row).map(|(_, id)| id.clone()) else { return true };
            let again = match (&mut o.sc.add, o.sc.opened.is_some()) {
                (Some(screen::Add::Menu { sel }), _) | (Some(screen::Add::Import { sel, .. }), _) => {
                    let i = id.parse().unwrap_or(0);
                    std::mem::replace(sel, i) == i
                }
                (Some(_), _) => false,
                (None, true) => o.sc.part.replace(id.clone()).as_deref() == Some(id.as_str()),
                (None, false) => o.sc.sel.replace(id.clone()).as_deref() == Some(id.as_str()),
            };
            if again {
                let e = screen::key(&mut o.sc, &o.rows, &o.markets, &press(KeyCode::Enter));
                run(app, e);
            }
        }
        _ => {}
    }
    true
}

/// Run what a key asked.
fn run(app: &mut crate::app::App, e: Effect) {
    let ws = crate::sb::workspace(app).unwrap_or_default();
    match e {
        Effect::None => return,
        Effect::Close => {
            app.plugins = None;
            return;
        }
        Effect::Set { id, title, on, turn } => {
            let said = match set(&id, on) {
                Ok(_) => (screen::done_note(&title, on, turn), false),
                Err(e) => (e, true),
            };
            if let Some(o) = app.plugins.as_mut() {
                o.note(said.0, said.1);
            }
        }
        Effect::Login(name) => {
            let first = login(Path::new(&ws), &name);
            if let Some(o) = app.plugins.as_mut() {
                o.note(first, false);
            }
        }
        Effect::Connect(id) => {
            let title = app.plugins.as_ref().and_then(|o| o.rows.iter().find(|p| p.id == id)).map_or(id.clone(), |p| p.title.clone());
            connect_said(&id, &title);
            if let Some(o) = app.plugins.as_mut() {
                o.note(settings::plugins_connecting(&title), false);
            }
        }
        Effect::Try(text) => {
            app.plugins = None;
            crate::sb::focus(app, "main");
            app.ed.set(&text, text.len());
            return;
        }
        Effect::OpenFolder(path) => {
            crate::links::open(&format!("file://{path}"));
        }
        Effect::Agent(words) => {
            app.plugins = None;
            let evs = crate::sb::handle_input(app, &format!("/new {}", bend_plugins::add::agent_brief(&words)));
            for ev in evs {
                crate::feed::push_event(&mut app.events, &mut app.cache, ev);
            }
            app.flash = Some((settings::plugins_agent_started.to_string(), Instant::now()));
            return;
        }
        Effect::Account { id, account, change } => {
            let p = app.plugins.as_ref().and_then(|o| o.rows.iter().find(|p| p.id == id));
            let title = p.map_or(id.clone(), |p| p.title.clone());
            let shown = p.and_then(|p| p.accounts.iter().find(|a| a.name == account)).and_then(|a| a.title.clone()).unwrap_or_else(|| account.clone());
            let note = account_said(&id, &account, change, &title, &shown);
            if let Some(o) = app.plugins.as_mut() {
                o.note(note, false);
            }
        }
        Effect::AskAccountTitle { id, account } => {
            app.plugins = None;
            crate::sb::focus(app, "main");
            let line = format!("/plugins account {id} rename {account} ");
            app.ed.set(&line, line.len());
            return;
        }
        Effect::AskSetting { plugin, var, label, secret } => {
            app.plugins = None;
            let line = start_ask(app, Ask { plugin, var, label, secret });
            crate::feed::push_event(&mut app.events, &mut app.cache, crate::wire::Ev::Info(line));
            return;
        }
        other => crate::plugins_add::run(app, other),
    }
    reread(app, true);
}

/// Connect a Mistral connector ([`connect`], its own thread): its lines
/// come back to the thread like a login's, then how it ended.
fn connect_said(id: &str, title: &str) {
    let title = title.to_string();
    let r = connect(id, say, move |r| match r {
        Ok(()) => say(settings::plugins_connect_done(&title)),
        Err(e) => say(e),
    });
    if let Err(e) = r {
        say(e);
    }
}

/// One of his accounts on connector `id` ([`connect_account`] or
/// [`change_account`], their own threads): their end comes back to the
/// thread; the note for the screen now.
fn account_said(id: &str, account: &str, change: Option<bend_plugins::connectors::AccountChange>, title: &str, shown: &str) -> String {
    let done = settings::plugins_account_changed(shown);
    let end = move |r: Result<(), String>| say(r.map_or_else(|e| e, |()| done));
    match change {
        None => match connect_account(id, account, say, end) {
            Ok(()) => settings::plugins_account_opened(title),
            Err(e) => e,
        },
        Some(c) => {
            change_account(id, account, c, end);
            settings::plugins_account_sent(shown)
        }
    }
}

/// A plugin or a part by what he typed (its id, a plugin's name or
/// title): its id, its words for the note, whether it reaches the prompt.
fn named(rows: &[Plugin], typed: &str) -> Option<(String, String, bool)> {
    let t = typed.trim();
    if let Some(p) = rows.iter().find(|p| p.id == t || p.name == t || p.title.eq_ignore_ascii_case(t)) {
        return Some((p.id.clone(), p.title.clone(), screen::reaches_prompt(p, None)));
    }
    rows.iter().find_map(|p| {
        p.parts.iter().flat_map(|x| std::iter::once(x).chain(x.tools.iter())).find(|x| x.id == t).map(|x| (x.id.clone(), format!("{} › {}", p.title, x.name), screen::reaches_prompt(p, Some(x))))
    })
}

/// `/plugins …` and `/skills …`: the screen opens (bare, `add`), a
/// toggle says what happened on the divider, the rest prints lines.
pub(crate) fn slash(app: &mut crate::app::App, typed: &str) -> Vec<String> {
    let ws = std::path::PathBuf::from(crate::sb::workspace(app).unwrap_or_default());
    let mut words = typed.split_whitespace();
    let skills = words.next() == Some("/skills");
    let kind = if skills { screen::Kind::Skills } else { screen::Kind::All };
    match (words.next(), words.next()) {
        (None, _) => open(app, kind, false),
        (Some("add"), None) if !skills => open(app, kind, true),
        (Some(sub @ ("on" | "off")), Some(name)) => {
            let rows = rows(&ws, &pending());
            let on = sub == "on";
            let (id, title, turn) = named(&rows, name).unwrap_or_else(|| (name.to_string(), name.to_string(), true));
            if rows.iter().any(|p| p.id == id && p.locked) {
                return vec![settings::plugins_locked(&title)];
            }
            match set(&id, on) {
                Ok(_) => app.flash = Some((screen::done_note(&title, on, turn), Instant::now())),
                Err(e) => return vec![e],
            }
        }
        (Some("connect"), Some(name)) if !skills => {
            let rows = rows(&ws, &pending());
            let p = rows.iter().find(|p| p.kind == bise_proto::app_rows::PluginKind::Mistral && (p.id == name || p.name == name || p.title.eq_ignore_ascii_case(name)));
            let id = p.map_or_else(|| bend_plugins::id::PluginId::mistral(name).to_string(), |p| p.id.clone());
            let title = p.map_or(name, |p| p.title.as_str());
            connect_said(&id, title);
            return vec![settings::plugins_connecting(title)];
        }
        (Some("refresh"), None) if !skills => refresh_said(),
        (Some("account"), Some(id)) if !skills => {
            use bend_plugins::connectors::AccountChange;
            let (verb, account) = (words.next(), words.next());
            let rest: Vec<&str> = words.by_ref().collect();
            let change = match (verb, rest.join(" ").trim()) {
                (Some("connect"), _) => None,
                (Some("default"), _) => Some(AccountChange::MakeDefault),
                (Some("remove"), _) => Some(AccountChange::Remove),
                (Some("rename"), t) if !t.is_empty() => Some(AccountChange::Rename(t.to_string())),
                _ => return vec![settings::plugins_account_usage.to_string()],
            };
            let Some(account) = account else { return vec![settings::plugins_account_usage.to_string()] };
            let rows = rows(&ws, &pending());
            let p = rows.iter().find(|p| p.kind == bise_proto::app_rows::PluginKind::Mistral && (p.id == id || p.name == id || p.title.eq_ignore_ascii_case(id)));
            let (id, title) = p.map_or_else(|| (id.to_string(), id.to_string()), |p| (p.id.clone(), p.title.clone()));
            return vec![account_said(&id, account, change, &title, account)];
        }
        (Some(sub @ ("set" | "unset")), name) if !skills => {
            let (Some(name), Some(var)) = (name, words.next()) else { return vec![settings::plugins_setting_usage.to_string()] };
            let rows = rows(&ws, &pending());
            let p = rows.iter().find(|p| p.kind == bise_proto::app_rows::PluginKind::Plugin && (p.name == name || p.id == name || p.title.eq_ignore_ascii_case(name)));
            let found = p.and_then(|p| p.settings.iter().find(|s| s.var == var).map(|s| (p, s)));
            return match (sub, found) {
                ("set", Some((p, s))) => vec![start_ask(app, Ask { plugin: p.name.clone(), var: s.var.clone(), label: s.label.clone(), secret: s.secret })],
                // not one of its settings: the writer's own refusal
                (_, None) => vec![setting(&ws, p.map_or(name, |p| p.name.as_str()), var, None).err().unwrap_or_else(|| settings::plugins_setting_usage.to_string())],
                (_, Some((p, s))) => match setting(&ws, &p.name, &s.var, None) {
                    Ok(()) => vec![settings::plugins_setting_forgot(&s.label)],
                    Err(e) => vec![e],
                },
            };
        }
        _ if skills => return vec![settings::skills_usage.to_string()],
        _ => return command(typed, &ws).lines().map(String::from).collect(),
    }
    Vec::new()
}

/// `/plugins refresh` ([`refresh`], its own thread): its line comes back
/// to the thread.
fn refresh_said() {
    refresh(|e| {
        say(match e {
            None => settings::plugins_refreshed.to_string(),
            Some(e) => settings::plugins_refresh_failed(e),
        })
    });
}

/// The screen on given rows (the render tests: no file read for 2 s).
#[cfg(test)]
pub(crate) fn open_on(app: &mut crate::app::App, sc: screen::Screen, rows: Vec<Plugin>, markets: Vec<Market>) {
    let mut o = Open::new(sc);
    o.rows = rows;
    o.markets = markets;
    o.read_at = Some(Instant::now());
    o.got = Some(Instant::now());
    screen::settle(&mut o.sc, &o.rows, &o.markets);
    app.plugins = Some(o);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// `/plugins set`: the composer asks, each character drawn as •, the
    /// key bar's placeholder says ⏎ and esc; esc and an empty line write
    /// nothing; the value is never in the history, a draft or the feed.
    #[test]
    fn a_setting_is_asked_hidden_and_never_kept() {
        let mut app = crate::sb::bench::test_app();
        let ask = Ask { plugin: "maps".into(), var: "GOOGLE_MAPS_API_KEY".into(), label: "Google Maps API key".into(), secret: true };
        assert_eq!(start_ask(&mut app, ask.clone()), "value for Google Maps API key (hidden):");
        assert!(masked(&app));
        let draw = |app: &mut crate::app::App| {
            let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
            term.draw(|f| crate::ui::draw(app, f)).unwrap();
            let buf = term.backend().buffer().clone();
            buf.content.chunks(100).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        // the tab names the setting, no agent; the key bar ⏎ and esc only
        let f = draw(&mut app);
        assert!(f.contains(&settings::plugins_setting_tab("Google Maps API key")) && !f.contains("you → main"), "{f}");
        assert!(f.contains(&format!("⏎ {}   esc {}", settings::plugins_setting_key_save, settings::plugins_setting_key_keep)) && !f.contains("@ file"), "{f}");
        // no @ / $ / / list opens
        for c in "/AIzaSECRET".chars() {
            crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert!(crate::commands::popup_items(&app).is_empty());
        let f = draw(&mut app);
        assert!(f.contains("•••••••••••") && !f.contains("AIzaSECRET"), "{f}");
        if let Ok(dir) = std::env::var("PLUGINS_SHOTS") {
            // the designer's shot: the line that asks, then the composer
            let line = settings::plugins_setting_prompt("Google Maps API key");
            crate::feed::push_event(&mut app.events, &mut app.cache, crate::wire::Ev::Info(line));
            let _ = std::fs::write(format!("{dir}/a_setting_asked-100.txt"), draw(&mut app));
        }
        assert!(!crate::sb::drafts::snapshot_texts(&app).iter().any(|t| t.contains("AIza")));
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.plugin_ask.is_none() && app.ed.text.is_empty());
        // an empty line: nothing written either
        start_ask(&mut app, Ask { secret: false, ..ask });
        assert!(!masked(&app));
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.plugin_ask.is_none());
        let said: Vec<String> = app.events.iter().filter_map(|e| if let crate::wire::Ev::Info(t) = e { Some(t.clone()) } else { None }).collect();
        // (the shot's own prompt line comes first when PLUGINS_SHOTS is set)
        assert!(said.ends_with(&[settings::plugins_setting_kept.to_string(), settings::plugins_setting_kept.to_string()]), "{said:?}");
        assert_eq!(said.len(), if std::env::var_os("PLUGINS_SHOTS").is_some() { 3 } else { 2 }, "{said:?}");
        assert!(app.history.is_empty());
    }

    #[test]
    fn lists_the_workspace_plugins() {
        let dir = std::env::temp_dir().join(format!("tui-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let t = command("/plugins", &dir);
        assert!(t.starts_with("plugins ("), "{}", t);
        assert!(command("/plugins bogus", &dir).starts_with("usage:"));
        // no remote server in this workspace: login says so, a name too
        assert!(command("/plugins login", &dir).starts_with("no remote MCP server that logs in"));
        assert!(command("/plugins login nope", &dir).starts_with("no remote MCP server named nope"));
        assert!(choices(&dir, "", "/plugins login ").is_empty());
        // remove asks once and refuses what isn't his
        assert_eq!(command("/plugins remove pay", &dir), settings::plugins_remove_ask("pay"));
        assert!(command("/plugins remove mistral:gmail yes", &dir).contains("Mistral connector"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
