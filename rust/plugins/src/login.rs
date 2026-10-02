//! `/plugins login` and `bise plugins login|logout`: the remote servers
//! of the workspace's plugins that can log in (no `Authorization` in
//! their mcp.json), their state, and one login run (oauth.rs) followed
//! by a connect that counts the tools. Every agent's bridge sees the new
//! tokens in the store and connects the server within a few seconds.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::oauth;
use crate::remote::{self, Fail, Remote};
use crate::resolve::{self, HttpServer};
use crate::status::{self, Status};

/// How long the browser login may take.
pub const WAIT: Duration = Duration::from_secs(300);

/// A remote server that can log in.
#[derive(Clone, Debug)]
pub struct Target {
    pub plugin: String,
    pub server: HttpServer,
    /// what the server is called in messages: its id, or
    /// `plugin/server` when two plugins have a server of that id
    pub name: String,
}

/// The loaded plugins' remote servers that can log in.
pub fn targets(ws: &Path) -> Vec<Target> {
    let res = resolve::resolve(&resolve::Roots::standard(Some(ws)));
    let mut out: Vec<Target> = res
        .loaded()
        .flat_map(|p| {
            p.remotes
                .iter()
                .filter(|s| s.may_login())
                .map(|s| Target { plugin: p.name.clone(), server: s.clone(), name: s.id.clone() })
        })
        .collect();
    let ids: Vec<String> = out.iter().map(|t| t.server.id.clone()).collect();
    for t in out.iter_mut() {
        if ids.iter().filter(|i| **i == t.server.id).count() > 1 {
            t.name = format!("{}/{}", t.plugin, t.server.id);
        }
    }
    out
}

/// `linear` or `plugin/linear`.
pub fn find<'a>(ts: &'a [Target], name: &str) -> Result<&'a Target, String> {
    let hit: Vec<&Target> = ts.iter().filter(|t| t.name == name || format!("{}/{}", t.plugin, t.server.id) == name).collect();
    match hit.as_slice() {
        [t] => Ok(t),
        [] if ts.iter().any(|t| t.server.id == name) => Err(format!("two plugins have a server named {}: say plugin/{}", name, name)),
        [] => Err(format!("no remote MCP server named {} in the workspace's plugins", name)),
        _ => Err(format!("{} is ambiguous", name)),
    }
}

/// The store holds tokens for this server.
pub fn logged_in(t: &Target, secrets: &Path) -> bool {
    let env = |k: &str| std::env::var(k).ok();
    remote::target(&t.server, &env)
        .ok()
        .and_then(|(u, _)| oauth::load(secrets, &oauth::resource_of(&u)))
        .is_some_and(|s| s.access_token.is_some())
}

/// The popup's words: `mcp.linear.app · needs a login`, `… · logged in
/// · 23 tools`, `… · not logged in`.
pub fn state(t: &Target, secrets: &Path, status_dir: &Path) -> String {
    let last = status::read(status_dir, &t.plugin, &t.server.id);
    let host = t.server.host();
    if last.as_ref().is_some_and(|s| s.login) {
        return format!("{} · needs a login", host);
    }
    if logged_in(t, secrets) {
        return match last.map(|s| s.tools) {
            Some(Ok(n)) => format!("{} · logged in · {} tool{}", host, n, if n == 1 { "" } else { "s" }),
            _ => format!("{} · logged in", host),
        };
    }
    match last.map(|s| s.tools) {
        Some(Ok(n)) => format!("{} · connected · {} tool{}", host, n, if n == 1 { "" } else { "s" }),
        _ => format!("{} · not logged in", host),
    }
}

/// Log in to `t` through the browser, then connect and count its tools
/// (the status for /plugins is written). Err: one short reason.
pub fn run(
    t: &Target,
    secrets: &Path,
    status_dir: Option<&Path>,
    open: &dyn Fn(&str) -> Result<(), String>,
    wait: Duration,
    cancel: Option<&AtomicBool>,
    paste: Option<&oauth::Paste<'_>>,
) -> Result<usize, String> {
    let env = |k: &str| std::env::var(k).ok();
    let (url, _) = remote::target(&t.server, &env)?;
    let none: remote::OnChange = Arc::new(|| {});
    // the server's challenge names its login metadata
    let challenge = match Remote::start(&t.server, &env, none.clone(), Duration::from_secs(20)) {
        Err(Fail::Auth { challenge, .. }) => challenge,
        // a 403 insufficient_scope: its challenge names the scope (step-up)
        Err(Fail::Scope { challenge, .. }) => Some(challenge),
        Ok(_) => None,
        Err(e) => return Err(e.to_string()),
    };
    let config = t.server.oauth.clone().unwrap_or_default();
    oauth::login(&oauth::Login {
        server: &url,
        name: &t.name,
        config: &config,
        dir: secrets,
        challenge: challenge.as_deref(),
        open,
        wait,
        cancel,
        cimd: oauth::CLIENT_ID,
        paste,
    })?;
    let c = Remote::start_with(&t.server, &env, Some(secrets), none, Duration::from_secs(20)).map_err(|e| match e {
        Fail::Auth { .. } => "the server still answers 401 with the new token".to_string(),
        Fail::Scope { .. } => "the server still wants more access than the login gave".to_string(),
        e => e.to_string(),
    })?;
    let n = c.list_tools(Duration::from_secs(20)).map_err(|e| format!("tools/list: {}", e))?.len();
    if let Some(d) = status_dir {
        status::write(d, &t.plugin, &t.server.id, &Status::now(t.server.transport.as_str(), &t.server.host(), Ok(n)));
    }
    Ok(n)
}

/// Forget a server's tokens (its app registration stays), revoked at
/// the login server first when it can (RFC 7009).
pub fn logout(t: &Target, secrets: &Path, status_dir: Option<&Path>) -> Result<(), String> {
    let env = |k: &str| std::env::var(k).ok();
    let (url, _) = remote::target(&t.server, &env)?;
    oauth::logout(secrets, &oauth::resource_of(&url), &t.server.oauth.clone().unwrap_or_default());
    if let Some(d) = status_dir {
        status::write(d, &t.plugin, &t.server.id, &Status::login_needed(t.server.transport.as_str(), &t.server.host()));
    }
    Ok(())
}

/// The thread's line after a login (the designer's words).
pub fn done_line(name: &str, r: &Result<usize, String>) -> String {
    match r {
        Ok(n) => format!("logged in to {}: {} tool{}, your agents have them now.", name, n, if *n == 1 { "" } else { "s" }),
        Err(e) => format!("▲ couldn't log in to {}: {}. /plugins login tries again.", name, e.trim_end_matches('.')),
    }
}

/// The CLI's lines after the opening one (designer m_4625).
pub fn cli_link_lines(url: &str) -> Vec<String> {
    vec![
        "or open this link:".into(),
        format!("  {}", url),
        String::new(),
        "browser on another machine? log in there. the last page won't load:".into(),
        "paste its address here and press enter.".into(),
    ]
}

/// The TUI's lines after the opening one (designer m_4625).
pub fn tui_link_lines(name: &str, url: &str) -> Vec<String> {
    vec![
        format!("or open this link: {}", url),
        format!("browser on another machine? log in there. the last page won't load: copy its address, then /plugins login {} <address>", name),
    ]
}

/// A pasted address that is not this login's answer.
pub fn bad_paste_cli(name: &str) -> String {
    format!("that isn't the address {} sent you back to. paste the whole address from the browser's address bar.", name)
}

pub fn bad_paste_tui(name: &str) -> String {
    format!("▲ that isn't the address {} sent you back to. copy the whole address from the browser's address bar.", name)
}

/// An address pasted while no login to that server waits.
pub fn no_login_waiting(name: &str) -> String {
    format!("▲ no login to {} is waiting. /plugins login starts one.", name)
}

/// `bise plugins login [NAME]`, `bise plugins logout NAME`; the exit code.
pub fn main(args: &[String], ws: &Path, logout_cmd: bool) -> i32 {
    let ts = targets(ws);
    let secrets = oauth::store_dir();
    let sd = status::dir();
    let out = bise_home::style::Style::stdout();
    let err = bise_home::style::Style::stderr();
    let name = args.iter().find(|a| !a.starts_with("--"));
    let Some(name) = name else {
        if ts.is_empty() {
            println!("{}", out.dim("no remote MCP server that logs in (one without an Authorization header in its mcp.json)"));
        }
        for t in &ts {
            println!("{}  {}", t.name, out.dim(&state(t, &secrets, &sd)));
        }
        return 0;
    };
    let t = match find(&ts, name) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            return 1;
        }
    };
    if logout_cmd {
        return match logout(t, &secrets, Some(&sd)) {
            Ok(()) => {
                println!("{}", out.ok(&format!("logged out of {}.", t.name)));
                0
            }
            Err(e) => {
                eprintln!("{}", err.fail(&e));
                1
            }
        };
    }
    println!("{}", out.dim(&format!("opening your browser to log in to {}…", t.name)));
    // the link always (another browser, another machine), then a pasted
    // address: the redirect a browser elsewhere could not load
    let open = |u: &str| {
        let _ = oauth::open_browser(u);
        for l in cli_link_lines(u) {
            println!("{}", l);
        }
        print!("> ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        Ok(())
    };
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut line = String::new();
        while std::io::stdin().read_line(&mut line).is_ok_and(|n| n > 0) {
            if !line.trim().is_empty() && tx.send(line.trim().to_string()).is_err() {
                break;
            }
            line.clear();
        }
    });
    let pasted = AtomicBool::new(false);
    let next = || {
        let l = rx.try_recv().ok();
        if l.is_some() {
            pasted.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        l
    };
    let bad = || {
        println!("{}", bad_paste_cli(&t.name));
        print!("> ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    };
    let paste = oauth::Paste { next: &next, bad: &bad };
    let r = run(t, &secrets, Some(&sd), &open, WAIT, None, Some(&paste));
    if !pasted.load(std::sync::atomic::Ordering::SeqCst) {
        // the browser's redirect came: end the prompt's row
        println!();
    }
    let line = done_line(&t.name, &r);
    match r {
        Ok(_) => {
            println!("{}", out.ok(&line));
            0
        }
        Err(_) => {
            eprintln!("{}", err.fail(&line));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paste_lines_say_the_designers_words() {
        assert_eq!(
            tui_link_lines("linear", "https://x/a"),
            [
                "or open this link: https://x/a",
                "browser on another machine? log in there. the last page won't load: copy its address, then /plugins login linear <address>"
            ]
        );
        assert_eq!(cli_link_lines("https://x/a")[1], "  https://x/a");
        assert_eq!(bad_paste_tui("linear"), "▲ that isn't the address linear sent you back to. copy the whole address from the browser's address bar.");
        assert_eq!(bad_paste_cli("linear"), "that isn't the address linear sent you back to. paste the whole address from the browser's address bar.");
        assert_eq!(no_login_waiting("linear"), "▲ no login to linear is waiting. /plugins login starts one.");
    }

    #[test]
    fn the_lines_say_the_designers_words() {
        assert_eq!(done_line("linear", &Ok(23)), "logged in to linear: 23 tools, your agents have them now.");
        assert_eq!(
            done_line("linear", &Err("the browser login wasn't finished in 5 min".into())),
            "▲ couldn't log in to linear: the browser login wasn't finished in 5 min. /plugins login tries again."
        );
    }
}
