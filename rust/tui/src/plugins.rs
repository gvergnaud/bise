//! `/plugins` in the TUI (docs/plugins.md).
//!
//! - `/plugins`: the plugins of the workspace (the static listing).
//! - `/plugins enable|disable NAME`: edits `~/.bend-harness/plugins.json`;
//!   applies at the next `/reload` (or task restart).
//! - `/plugins login [NAME]`: the browser login of a remote MCP server
//!   (bend_plugins::login), in the background: its lines come back
//!   through [`pump`]; every agent's bridge sees the new tokens and
//!   connects the server. `/plugins logout NAME` forgets them.
//! - [`pump`] also says once per server and per TUI run, when a session
//!   found that one needs a login: `linear needs a login: /plugins login`.

use std::path::Path;

/// The plugins of `workspace` matching `q` (the `/plugins enable|disable`
/// argument, BISE-117). Resolved again at most every 5 s: the popup asks
/// at every frame.
pub(crate) fn choices(workspace: &Path, q: &str, typed: &str) -> Vec<crate::commands::Choice> {
    if matches!(typed.split_whitespace().nth(1), Some("login" | "logout")) {
        return login_choices(workspace, q);
    }
    use std::cell::RefCell;
    use std::time::{Duration, Instant};
    type Cached = Option<(std::path::PathBuf, Instant, Vec<(String, String)>)>;
    thread_local! {
        static CACHE: RefCell<Cached> = const { RefCell::new(None) };
    }
    let all = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let fresh = c.as_ref().is_some_and(|(w, t, _)| w == workspace && t.elapsed() < Duration::from_secs(5));
        if !fresh {
            let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(workspace)));
            let list = res
                .plugins
                .iter()
                .map(|p| {
                    let what = p.description.clone().unwrap_or_default();
                    (p.name.clone(), format!("{} · {}", p.state.as_str(), what).trim_end_matches(" · ").to_string())
                })
                .collect();
            *c = Some((workspace.to_path_buf(), Instant::now(), list));
        }
        c.as_ref().map(|(_, _, l)| l.clone()).unwrap_or_default()
    });
    all.into_iter()
        .filter(|(n, _)| crate::commands::matches(q, &[n]))
        .map(|(n, d)| crate::commands::Choice { value: n.clone(), label: n, desc: d, mark: None })
        .collect()
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

fn say(l: String) {
    LINES.lock().unwrap_or_else(|e| e.into_inner()).push(l);
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
            return format!("the login to {} is still open in your browser", t.name);
        }
        r.push(t.name.clone());
    }
    let first = format!("opening your browser to log in to {}…", t.name);
    std::thread::spawn(move || {
        let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
        let name = t.name.clone();
        let open = move |u: &str| {
            bend_plugins::oauth::open_browser(u).or_else(|_| {
                say(format!("open this link to log in to {}: {}", name, u));
                Ok(())
            })
        };
        let r = bend_plugins::login::run(&t, &secrets, Some(&sd), &open, bend_plugins::login::WAIT, None);
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
            out.push(format!("{} needs a login: /plugins login", t.name));
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
        (Some("login"), Some(name)) => login(workspace, name),
        (Some(sub @ ("login" | "logout")), None) => {
            let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
            let ts = bend_plugins::login::targets(workspace);
            if ts.is_empty() {
                return "no remote MCP server that logs in (one without an Authorization header in its mcp.json)".into();
            }
            let rows: Vec<String> = ts.iter().map(|t| format!("  {}   {}", t.name, bend_plugins::login::state(t, &secrets, &sd))).collect();
            format!("/plugins {} NAME:\n{}", sub, rows.join("\n"))
        }
        (Some("logout"), Some(name)) => {
            let ts = bend_plugins::login::targets(workspace);
            match bend_plugins::login::find(&ts, name)
                .and_then(|t| bend_plugins::login::logout(t, &bend_plugins::oauth::store_dir(), Some(&bend_plugins::status::dir())).map(|_| t))
            {
                Ok(t) => format!("logged out of {}.", t.name),
                Err(e) => e,
            }
        }
        (Some(sub @ ("enable" | "disable")), Some(name)) => {
            let path = bend_plugins::state::state_path();
            match bend_plugins::state::set_enabled(&path, name, sub == "enable") {
                Ok(changed) => format!(
                    "{} {}{} — applies at the next /reload",
                    name,
                    if sub == "enable" { "enabled" } else { "disabled" },
                    if changed { "" } else { " (unchanged)" }
                ),
                Err(e) => format!("{}: {}", path.display(), e),
            }
        }
        (None, _) | (Some("list"), _) => {
            let text = bend_plugins::cli::listing(workspace);
            format!("plugins ({})\n{}", workspace.display(), text.trim_end())
        }
        _ => "usage: /plugins [list] | /plugins enable NAME | /plugins disable NAME | /plugins login [NAME] | /plugins logout NAME".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let _ = std::fs::remove_dir_all(&dir);
    }
}
