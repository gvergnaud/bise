//! The last state of each remote MCP server, for `/plugins` and
//! `bise plugins list`: the bridge of every session writes
//! `<dir>/<plugin>/<server>.json` when it connects (or fails), lists the
//! tools again, or gives up; the static listing reads it. Host, transport,
//! a tool count (and the tools' names and descriptions) or one error
//! line, and the time: never a header value or the URL's path.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// `$BEND_MCP_STATUS`, else `<bise home>/mcp-status`, through Home.
pub fn dir() -> PathBuf {
    bise_home::Home::from_env().mcp_status_dir()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub transport: String,
    pub host: String,
    /// the number of tools, or why the server is unavailable
    pub tools: Result<usize, String>,
    /// unix seconds
    pub at: u64,
    /// the server answered 401 and bise can log in (`/plugins login`)
    pub login: bool,
    /// its tools as it listed them (name, description), for the Plugins
    /// list's switches; empty when unknown
    pub listed: Vec<(String, String)>,
    /// a stdio server listed (from the shared tools cache, or started,
    /// listed and stopped) but not running: it starts on the first call
    /// (`crate::tools_cache`). Never an error.
    pub idle: bool,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Status {
    pub fn now(transport: &str, host: &str, tools: Result<usize, String>) -> Status {
        Status { transport: transport.into(), host: host.into(), tools, at: now(), login: false, listed: Vec::new(), idle: false }
    }

    pub fn login_needed(transport: &str, host: &str) -> Status {
        Status { login: true, ..Status::now(transport, host, Err("needs a login".into())) }
    }

    /// The same, with its tools by name (the bridge, after `tools/list`).
    pub fn listed(self, listed: Vec<(String, String)>) -> Status {
        Status { listed, ..self }
    }

    /// The same, listed but not running ([`Status::idle`]).
    pub fn idle(self, idle: bool) -> Status {
        Status { idle, ..self }
    }

    /// `connected · 12 tools · 3 min ago`, `listed · 12 tools · starts on
    /// first call · 3 min ago`, `✗ <error> · 3 min ago` or `needs a login
    /// · /plugins login`
    pub fn line(&self) -> String {
        let ago = ago(now().saturating_sub(self.at));
        if self.login {
            return "needs a login · /plugins login".into();
        }
        match &self.tools {
            Ok(n) if self.idle => format!("listed · {} tool{} · starts on first call · {}", n, if *n == 1 { "" } else { "s" }, ago),
            Ok(n) => format!("connected · {} tool{} · {}", n, if *n == 1 { "" } else { "s" }, ago),
            Err(e) => format!("✗ {} · {}", e, ago),
        }
    }
}

fn ago(s: u64) -> String {
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86399 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86400),
    }
}

fn file(dir: &Path, plugin: &str, server: &str) -> PathBuf {
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect::<String>();
    dir.join(safe(plugin)).join(format!("{}.json", safe(server)))
}

/// Best effort: a status that cannot be written is not an error.
pub fn write(dir: &Path, plugin: &str, server: &str, s: &Status) {
    let f = file(dir, plugin, server);
    let Some(parent) = f.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let v = match &s.tools {
        Ok(n) => json!({"transport": s.transport, "host": s.host, "tools": n, "at": s.at, "list": s.listed, "idle": s.idle}),
        Err(e) => json!({"transport": s.transport, "host": s.host, "error": e, "at": s.at, "login": s.login}),
    };
    let tmp = f.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Every server's status of `plugin` gone (the plugin was removed).
pub fn forget(dir: &Path, plugin: &str) {
    if let Some(d) = file(dir, plugin, "x").parent() {
        let _ = std::fs::remove_dir_all(d);
    }
}

pub fn read(dir: &Path, plugin: &str, server: &str) -> Option<Status> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file(dir, plugin, server)).ok()?).ok()?;
    let tools = match (v.get("tools").and_then(Value::as_u64), v.get("error").and_then(Value::as_str)) {
        (Some(n), _) => Ok(n as usize),
        (None, Some(e)) => Err(e.to_string()),
        _ => return None,
    };
    Some(Status {
        transport: v.get("transport").and_then(Value::as_str).unwrap_or("").into(),
        host: v.get("host").and_then(Value::as_str).unwrap_or("").into(),
        tools,
        at: v.get("at").and_then(Value::as_u64).unwrap_or(0),
        login: v.get("login").and_then(Value::as_bool).unwrap_or(false),
        listed: serde_json::from_value(v.get("list").cloned().unwrap_or(Value::Null)).unwrap_or_default(),
        idle: v.get("idle").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_round_trips_and_reads_as_one_line() {
        let d = std::env::temp_dir().join(format!("bp-status-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        write(&d, "p", "linear", &Status::now("http", "mcp.linear.app", Ok(3)).listed(vec![("search".into(), "find issues".into())]));
        let s = read(&d, "p", "linear").unwrap();
        assert_eq!((s.host.as_str(), s.tools.clone()), ("mcp.linear.app", Ok(3)));
        assert_eq!(s.listed, vec![("search".to_string(), "find issues".to_string())]);
        assert_eq!(s.line(), "connected · 3 tools · just now");
        assert!(!s.idle);
        // listed from the tools cache, not running: a state, not an error
        write(&d, "p", "linear", &Status::now("stdio", "node", Ok(1)).idle(true));
        let i = read(&d, "p", "linear").unwrap();
        assert!(i.idle && i.tools == Ok(1));
        assert_eq!(i.line(), "listed · 1 tool · starts on first call · just now");
        write(&d, "p", "linear", &Status::now("http", "mcp.linear.app", Err("HTTP 500 from mcp.linear.app".into())));
        assert!(read(&d, "p", "linear").unwrap().line().starts_with("✗ HTTP 500"));
        assert!(read(&d, "p", "other").is_none());
        write(&d, "p", "linear", &Status::login_needed("http", "mcp.linear.app"));
        let l = read(&d, "p", "linear").unwrap();
        assert!(l.login);
        assert_eq!(l.line(), "needs a login · /plugins login");
        write(&d, "q", "linear", &Status::now("http", "mcp.linear.app", Ok(1)));
        forget(&d, "p");
        assert!(read(&d, "p", "linear").is_none() && !d.join("p").exists());
        assert!(read(&d, "q", "linear").is_some(), "another plugin's kept");
        let _ = std::fs::remove_dir_all(&d);
    }
}
