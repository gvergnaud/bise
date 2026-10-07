//! The words of a saved approvals rule (`~/.bise/approvals.toml`), as
//! the TUI's `/approvals` screen and the window's settings show them:
//! one fn each, moved from the TUI's approvals_screen.rs (bar V8/W21,
//! architect m_10951). Pure: the caller passes the home folder (for
//! `~`) and the dot it draws (the TUI's ascii mode). A rule's age is
//! words only for the TUI, which redraws them each frame; the hub sends
//! the window the date (`added_ms`), never 'today'.

/// The edit tools: one family for rules.
pub const EDIT_TOOLS: [&str; 3] = ["edit", "write_file", "apply_patch"];

/// The facts of a rule the words are made from ("" when absent).
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleFacts<'a> {
    pub tool: &'a str,
    pub pattern: &'a str,
    pub path: &'a str,
    /// `card #12, api-v2`, or the file's own words
    pub from: &'a str,
    /// no project: it applies in every repo
    pub every: bool,
    /// `sandbox = false`: its commands run outside the sandbox
    pub outside: bool,
}

/// What a rule allows, as the list shows it: `cargo test *`, `edits to
/// ~/notes`, `gmail.send_email`.
pub fn what(r: &RuleFacts, home: Option<&str>) -> String {
    if !r.path.is_empty() {
        format!("edits to {}", tilde(r.path, home))
    } else if r.tool == "bash" || (EDIT_TOOLS.contains(&r.tool) && r.pattern.is_empty()) {
        if r.pattern.is_empty() { r.tool.to_string() } else { r.pattern.to_string() }
    } else if r.pattern.is_empty() {
        r.tool.to_string()
    } else {
        format!("{} {}", r.tool, r.pattern)
    }
}

/// Its age in words: `today`, `yesterday`, `3 days ago`, `2 weeks ago`,
/// `5 months ago`; "" when the file does not say.
pub fn age(days: Option<i64>) -> String {
    match days {
        None => String::new(),
        Some(d) if d <= 0 => "today".into(),
        Some(1) => "yesterday".into(),
        Some(d) if d < 14 => format!("{d} days ago"),
        Some(d) if d < 60 => format!("{} weeks ago", d / 7),
        Some(d) if d < 730 => format!("{} months ago", d / 30),
        Some(d) => format!("{} years ago", d / 365),
    }
}

/// Where it came from: the card's agents (`from api-v2`, `3 agents`),
/// or the file's own words.
pub fn source(from: &str) -> String {
    let mut parts = from.split(", ").filter(|p| !p.is_empty());
    match parts.next() {
        None => String::new(),
        Some(first) if first.starts_with("card") => {
            let names: Vec<&str> = parts.collect();
            match names.len() {
                0 => "from a card".into(),
                1 => format!("from {}", names[0]),
                n => format!("{n} agents"),
            }
        }
        Some(_) => format!("from {from}"),
    }
}

/// The right column of a rule: its age (`days`: None leaves it out),
/// its source, where it applies, joined by `dot`.
pub fn note(r: &RuleFacts, days: Option<i64>, dot: &str) -> String {
    let mut v: Vec<String> = Vec::new();
    for w in [age(days), source(r.from)] {
        if !w.is_empty() {
            v.push(w);
        }
    }
    let connector = r.tool.contains('.') && r.pattern.is_empty() && r.path.is_empty();
    if connector && v.is_empty() {
        v.push("a connector".into());
    }
    if r.every {
        v.push("every project".into());
    }
    if r.outside {
        v.push("outside the sandbox".into());
    }
    v.join(&format!(" {dot} "))
}

/// `p` with the home folder as `~`.
fn tilde(p: &str, home: Option<&str>) -> String {
    match home {
        Some(h) if !h.is_empty() && p.starts_with(h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_of_a_rule() {
        assert_eq!(age(Some(0)), "today");
        assert_eq!(age(Some(1)), "yesterday");
        assert_eq!(age(Some(9)), "9 days ago");
        assert_eq!(age(Some(21)), "3 weeks ago");
        assert_eq!(age(None), "");
        assert_eq!(source("card #3, web"), "from web");
        assert_eq!(source("card"), "from a card");
        assert_eq!(source("card #3, a, b, c"), "3 agents");
        assert_eq!(source("me, by hand"), "from me, by hand");
        let bash = RuleFacts { tool: "bash", pattern: "cargo test *", ..Default::default() };
        assert_eq!(what(&bash, None), "cargo test *");
        let edits = RuleFacts { tool: "edit", path: "/u/me/notes", every: true, outside: true, ..Default::default() };
        assert_eq!(what(&edits, Some("/u/me")), "edits to ~/notes");
        assert_eq!(note(&edits, Some(1), "·"), "yesterday · every project · outside the sandbox");
        let conn = RuleFacts { tool: "gmail.send_email", ..Default::default() };
        assert_eq!(what(&conn, None), "gmail.send_email");
        assert_eq!(note(&conn, None, "-"), "a connector");
        let mcp = RuleFacts { tool: "linear", pattern: "create_issue", from: "card #2, api", ..Default::default() };
        assert_eq!(what(&mcp, None), "linear create_issue");
        assert_eq!(note(&mcp, None, "·"), "from api");
    }
}
