//! Slash lines that both the TUI and the hub read (architect m_11359):
//! one parse each, so the TUI's own arm and the hub's `slash` command
//! (switchboard proto_view::slash) can't drift apart.

/// What `/artifacts …` asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Artifacts {
    /// `/artifacts` (or anything after it but `add`): the list
    List,
    /// `/artifacts add <path or link>`: that target, trimmed
    Add(String),
    /// `/artifacts add` with nothing after it: [`ARTIFACTS_ADD`]
    Usage,
}

/// The usage of `/artifacts add`.
pub const ARTIFACTS_ADD: &str = "usage: /artifacts add <path or link>";

/// A line whose first word is `/artifacts`, read once for both sides.
/// `None` when it is another line.
pub fn artifacts(line: &str) -> Option<Artifacts> {
    let rest = line.trim().strip_prefix("/artifacts")?;
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    let rest = rest.trim();
    Some(match rest.strip_prefix("add") {
        Some(target) if target.is_empty() || target.starts_with(char::is_whitespace) => match target.trim() {
            "" => Artifacts::Usage,
            t => Artifacts::Add(t.to_string()),
        },
        _ => Artifacts::List,
    })
}

/// The first word of a slash line and the rest, when the first word is
/// exactly `cmd` (`/stopx` is not `/stop`).
fn after<'a>(line: &'a str, cmd: &str) -> Option<&'a str> {
    let rest = line.trim().strip_prefix(cmd)?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim())
}

/// The usage of `/stop`.
pub const STOP_USAGE: &str = "/stop <agent>";

/// `/stop <agent>` (computer-use-design §7.3): that agent's turn stops.
/// `Err` holds the usage words when no agent is named. `None` when it is
/// another line.
pub fn stop(line: &str) -> Option<Result<String, String>> {
    let rest = after(line, "/stop")?;
    Some(match rest.split_whitespace().next() {
        Some(name) => Ok(name.trim_start_matches('@').to_string()),
        None => Err(STOP_USAGE.to_string()),
    })
}

/// What `/version`, `/restart` and `/update` ask of the hub's versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Version {
    /// `/version` or `/version list`: the installed versions
    List,
    /// `/version back` (or `rollback`): the one before
    Rollback,
    /// `/version <v>`: switch to it
    Switch(String),
    /// `/restart [<v>]`: hub, REPLs and clients restart (on `<v>` if given,
    /// else on the running one)
    Restart(String),
    /// `/update`: look for a new release now
    Update,
}

impl Version {
    /// The hub's `version` op for it, the one the TUI sends (`do`, `to`).
    pub fn op(&self) -> (&'static str, &str) {
        match self {
            Version::List => ("list", ""),
            Version::Rollback => ("rollback", ""),
            Version::Switch(to) => ("switch", to),
            Version::Restart(to) => ("restart", to),
            Version::Update => ("update", ""),
        }
    }
}

/// A `/version`, `/restart` or `/update` line, read once for both sides.
pub fn version(line: &str) -> Option<Version> {
    let first = |rest: &str| rest.split_whitespace().next().unwrap_or("").to_string();
    if let Some(rest) = after(line, "/version") {
        return Some(match first(rest).as_str() {
            "" | "list" => Version::List,
            "back" | "rollback" => Version::Rollback,
            to => Version::Switch(to.to_string()),
        });
    }
    if let Some(rest) = after(line, "/restart") {
        return Some(Version::Restart(first(rest)));
    }
    after(line, "/update").map(|_| Version::Update)
}

/// `/approvals [yolo|auto]` (approvals-design.md §8): `Ok(None)` shows the
/// mode, the checker and the rules; `Ok(Some(mode))` switches; `Err` holds
/// the words for another word. `None` when it is another line.
pub fn approvals(line: &str) -> Option<Result<Option<crate::rows::ApprovalMode>, String>> {
    use crate::rows::ApprovalMode;
    let rest = after(line, "/approvals")?;
    Some(match rest.split_whitespace().next().map(str::to_lowercase).as_deref() {
        None => Ok(None),
        Some("yolo") => Ok(Some(ApprovalMode::Yolo)),
        Some("auto") => Ok(Some(ApprovalMode::Auto)),
        Some(other) => Err(format!("/approvals {other}: yolo or auto")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rows::ApprovalMode;

    /// Law: `/stop`, `/version`, `/restart`, `/update` and `/approvals`
    /// read the same in the TUI and the hub: the TUI's words and ops of
    /// before, one parse.
    #[test]
    fn stop_version_and_approvals_lines_read_once() {
        assert_eq!(stop("/stop docs"), Some(Ok("docs".into())));
        assert_eq!(stop("/stop @docs now"), Some(Ok("docs".into())));
        assert_eq!(stop("/stop"), Some(Err(STOP_USAGE.into())));
        assert_eq!(stop("/stopx docs"), None);
        assert_eq!(version("/version"), Some(Version::List));
        assert_eq!(version("/version list"), Some(Version::List));
        assert_eq!(version("/version back"), Some(Version::Rollback));
        assert_eq!(version("/version rollback"), Some(Version::Rollback));
        assert_eq!(version("/version v2026.10.2-27"), Some(Version::Switch("v2026.10.2-27".into())));
        assert_eq!(version("/restart"), Some(Version::Restart(String::new())));
        assert_eq!(version("/restart dev"), Some(Version::Restart("dev".into())));
        assert_eq!(version("/update"), Some(Version::Update));
        assert_eq!(version("/updates"), None);
        assert_eq!(Version::Switch("x".into()).op(), ("switch", "x"));
        assert_eq!(Version::Update.op(), ("update", ""));
        assert_eq!(approvals("/approvals"), Some(Ok(None)));
        assert_eq!(approvals("/approvals YOLO"), Some(Ok(Some(ApprovalMode::Yolo))));
        assert_eq!(approvals("/approvals auto"), Some(Ok(Some(ApprovalMode::Auto))));
        assert_eq!(approvals("/approvals maybe"), Some(Err("/approvals maybe: yolo or auto".into())));
        assert_eq!(approvals("/approvalsx"), None);
    }

    /// Law: the TUI and the hub read `/artifacts` the same way: add with a
    /// target (spaces kept), add alone is the usage, anything else the list.
    #[test]
    fn artifacts_lines_read_once() {
        assert_eq!(artifacts("/artifacts"), Some(Artifacts::List));
        assert_eq!(artifacts("  /artifacts  "), Some(Artifacts::List));
        assert_eq!(artifacts("/artifacts add https://x.dev/a b"), Some(Artifacts::Add("https://x.dev/a b".into())));
        assert_eq!(artifacts("/artifacts add   ./notes.md  "), Some(Artifacts::Add("./notes.md".into())));
        assert_eq!(artifacts("/artifacts add"), Some(Artifacts::Usage));
        assert_eq!(artifacts("/artifacts add   "), Some(Artifacts::Usage));
        assert_eq!(artifacts("/artifacts addx"), Some(Artifacts::List));
        assert_eq!(artifacts("/artifacts foo"), Some(Artifacts::List));
        assert_eq!(artifacts("/artifactsx"), None);
        assert_eq!(artifacts("/diff"), None);
    }
}
