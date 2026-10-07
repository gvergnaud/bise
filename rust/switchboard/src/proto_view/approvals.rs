//! The typed `approvals` (bar V8/W21, architect m_10951): the same JSON
//! the TUI's `/approvals` reads (daemon/gate.rs `approvals_ev`, one read
//! of approvals.toml), as `HubEv::Approvals`. A rule's words come from
//! bise_proto's `approvals` (the TUI's own fns); its age is not words
//! here (they would go stale): `added_ms`, for the window's date helper.

use super::s;
use bise_proto::approvals::{self, RuleFacts};
use bise_proto::hub::HubEv;
use bise_proto::rows::{ApprovalMode, ApprovalRule, CheckerKind};
use serde_json::Value;

/// The typed event of `ev` (an `approvals` event of the hub) for
/// `project`; `home` turns paths into `~/…` in the words.
pub fn approvals(project: &str, ev: &Value, home: Option<&str>) -> HubEv {
    let word = |k: &str| Value::String(s(ev, k));
    let rules = ev.get("rules").and_then(Value::as_array).map(|a| a.iter().map(|r| rule(r, home)).collect()).unwrap_or_default();
    HubEv::Approvals {
        project: project.to_string(),
        mode: serde_json::from_value(word("mode")).unwrap_or(ApprovalMode::Unknown),
        env: ev.get("env").and_then(Value::as_bool).unwrap_or(false),
        checker: serde_json::from_value(word("checker")).unwrap_or(CheckerKind::Unknown),
        checker_who: s(ev, "checker_who"),
        checker_model: Some(s(ev, "checker_model")).filter(|m| !m.is_empty()),
        repo: s(ev, "repo"),
        rules,
        flash: ev.get("flash").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// One rule of gate.rs's `rule_json`: its fields as they came (its
/// identity for `remove_rule`), its words, its date.
fn rule(r: &Value, home: Option<&str>) -> ApprovalRule {
    let opt = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
    let (tool, pattern, path, project, from) = (s(r, "tool"), opt("pattern"), opt("path"), opt("project"), opt("from"));
    let sandbox = r.get("sandbox").and_then(Value::as_bool);
    let facts = RuleFacts {
        tool: &tool,
        pattern: pattern.as_deref().unwrap_or(""),
        path: path.as_deref().unwrap_or(""),
        from: from.as_deref().unwrap_or(""),
        every: project.is_none(),
        outside: sandbox == Some(false),
    };
    let (what, note) = (approvals::what(&facts, home), approvals::note(&facts, None, "·"));
    let added = opt("added");
    // the hub writes ms (gate.rs); a hand-written date is not read, as the TUI
    let added_ms = added.as_deref().and_then(|a| a.parse().ok());
    ApprovalRule { tool, pattern, path, project, added, from, sandbox, added_ms, what, note }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The event the TUI reads, typed: the mode and checker as enums (an
    /// unknown word kept as Unknown), each rule's fields as they came,
    /// its words from the TUI's fns, its date as data.
    #[test]
    fn the_tuis_approvals_typed() {
        let ev = json!({"ev": "approvals", "mode": "auto", "env": false, "checker": "jev", "checker_who": "TypeSafe",
            "checker_model": "jev-1.13", "repo": "/u/me/acme", "flash": true, "rules": [
                {"tool": "bash", "pattern": "cargo test *", "path": null, "project": "/u/me/acme", "added": "1791100000000", "from": "card #3, api", "sandbox": null},
                {"tool": "edit", "pattern": null, "path": "/u/me/notes", "project": null, "added": "2026-10-12", "from": null, "sandbox": false}]});
        let HubEv::Approvals { mode, checker, checker_model, rules, flash, .. } = approvals("p", &ev, Some("/u/me")) else { panic!() };
        assert_eq!((mode, checker, checker_model.as_deref(), flash), (ApprovalMode::Auto, CheckerKind::Jev, Some("jev-1.13"), true));
        assert_eq!((rules[0].what.as_str(), rules[0].note.as_str(), rules[0].added_ms), ("cargo test *", "from api", Some(1791100000000)));
        assert_eq!((rules[1].what.as_str(), rules[1].note.as_str(), rules[1].added_ms), ("edits to ~/notes", "every project · outside the sandbox", None));
        // remove_rule's identity: the fields gate.rs's rule_of_json reads
        let back = serde_json::to_value(&rules[0]).unwrap();
        assert_eq!((back["tool"].as_str(), back["pattern"].as_str(), back["added"].as_str()), (Some("bash"), Some("cargo test *"), Some("1791100000000")));
        let odd = json!({"mode": "maybe", "checker": "oracle", "rules": []});
        let HubEv::Approvals { mode, checker, checker_model, flash, .. } = approvals("p", &odd, None) else { panic!() };
        assert_eq!((mode, checker, checker_model, flash), (ApprovalMode::Unknown, CheckerKind::Unknown, None, false));
    }
}
