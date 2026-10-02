//! The last state of each remote MCP server, for `/plugins` and
//! `bise plugins list`: the bridge of every session writes
//! `<dir>/<plugin>/<server>.json` when it connects (or fails), lists the
//! tools again, or gives up; the static listing reads it. Host, transport,
//! a tool count or one error line, and the time: never a header value or
//! the URL's path.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// `$BEND_MCP_STATUS`, else `<bise home>/mcp-status`.
pub fn dir() -> PathBuf {
    std::env::var("BEND_MCP_STATUS")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| bise_home::Home::from_env().root().join("mcp-status"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub transport: String,
    pub host: String,
    /// the number of tools, or why the server is unavailable
    pub tools: Result<usize, String>,
    /// unix seconds
    pub at: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Status {
    pub fn now(transport: &str, host: &str, tools: Result<usize, String>) -> Status {
        Status { transport: transport.into(), host: host.into(), tools, at: now() }
    }

    /// `connected · 12 tools · 3 min ago` or `✗ <error> · 3 min ago`
    pub fn line(&self) -> String {
        let ago = ago(now().saturating_sub(self.at));
        match &self.tools {
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
        Ok(n) => json!({"transport": s.transport, "host": s.host, "tools": n, "at": s.at}),
        Err(e) => json!({"transport": s.transport, "host": s.host, "error": e, "at": s.at}),
    };
    let tmp = f.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    } else {
        let _ = std::fs::remove_file(&tmp);
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_round_trips_and_reads_as_one_line() {
        let d = std::env::temp_dir().join(format!("bp-status-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        write(&d, "p", "linear", &Status::now("http", "mcp.linear.app", Ok(3)));
        let s = read(&d, "p", "linear").unwrap();
        assert_eq!((s.host.as_str(), s.tools.clone()), ("mcp.linear.app", Ok(3)));
        assert_eq!(s.line(), "connected · 3 tools · just now");
        write(&d, "p", "linear", &Status::now("http", "mcp.linear.app", Err("HTTP 500 from mcp.linear.app".into())));
        assert!(read(&d, "p", "linear").unwrap().line().starts_with("✗ HTTP 500"));
        assert!(read(&d, "p", "other").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
