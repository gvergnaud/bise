//! C1's error codes and targets, and the small JSON helpers every side uses.

use serde_json::{json, Value};

/// C1 error codes (the last 4: uploads, `set_input_files`).
pub const CODES: [&str; 17] = [
    "not_set_up", "no_browser", "no_helper", "no_permission", "not_found", "ambiguous", "stale_ref", "stopped",
    "paused", "refused", "timeout", "needs_front", "bad_args", "file_not_found", "file_too_large", "no_file_input",
    "chooser_not_opened",
];

/// The raw tools (C1).
pub const TOOLS: [&str; 7] = ["status", "open", "tabs", "apps", "snapshot", "screenshot", "act"];

/// A C1 error: `{code, message}`.
pub fn err(code: &str, message: impl Into<String>) -> Value {
    debug_assert!(CODES.contains(&code), "{}", code);
    json!({"code": code, "message": message.into()})
}

/// The code of a C1 error value ("" when it has none).
pub fn code(e: &Value) -> &str {
    e.get("code").and_then(Value::as_str).unwrap_or("")
}

/// A target string: `tab:<chrome tab id>` or `app:<bundle id>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Tab(String),
    App(String),
}

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        let (kind, id) = s.split_once(':')?;
        if id.is_empty() || id.contains(char::is_whitespace) {
            return None;
        }
        match kind {
            "tab" => Some(Target::Tab(id.to_string())),
            "app" => Some(Target::App(id.to_string())),
            _ => None,
        }
    }
}

pub fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

/// The host of a URL (`https://www.amazon.fr/x` -> `www.amazon.fr`), lowercased,
/// without port or credentials; "" when there is none.
pub fn host_of(url: &str) -> String {
    let rest = match url.split_once("://") {
        Some((_, r)) => r,
        None => return String::new(),
    };
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or("");
    let host = if auth.starts_with('[') {
        auth.split(']').next().map(|h| format!("{}]", h)).unwrap_or_default()
    } else {
        auth.split(':').next().unwrap_or("").to_string()
    };
    host.to_ascii_lowercase()
}

/// The host as the TUI shows it: without `www.`.
pub fn short_host(url: &str) -> String {
    let h = host_of(url);
    h.strip_prefix("www.").map(String::from).unwrap_or(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_and_hosts() {
        assert_eq!(Target::parse("tab:17"), Some(Target::Tab("17".into())));
        assert_eq!(Target::parse("app:com.apple.TextEdit"), Some(Target::App("com.apple.TextEdit".into())));
        assert_eq!(Target::parse("tab:"), None);
        assert_eq!(Target::parse("window:1"), None);
        assert_eq!(Target::parse("17"), None);
        assert_eq!(host_of("https://user:pw@WWW.Amazon.fr:8443/x?y#z"), "www.amazon.fr");
        assert_eq!(host_of("http://[::1]:80/"), "[::1]");
        assert_eq!(host_of("about:blank"), "");
        assert_eq!(short_host("https://www.amazon.fr/"), "amazon.fr");
    }
}
