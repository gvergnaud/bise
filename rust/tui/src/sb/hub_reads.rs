//! The hub's notifications the terminal reads typed (client-protocol
//! step 4, P4c; plan v2 signed by architect m_13977): its hello lists
//! them in `reads` ([`READS`]) and the hub sends them instead of their
//! older events (`bise_proto::rpc::OLDER`, one kind at a time, never
//! twice). Each reader takes the typed `HubEv` (`rpc::ev` of the
//! notification), so the terminal and the window read one shape; the
//! other kinds still come the older way (sb.rs's dispatch) until their
//! chunk moves them.
//!
//! - `hub/approvals`: the mode for the key bar, the checker and the rules
//!   (`/approvals`' read, approvals/set without a mode, opens the screen
//!   on the same fields: sb/rpc.rs's `Then::Approvals`);
//! - `confirm/ask`: the hub's yes/no question (`y`/`n`, esc: no).

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rows::{ApprovalMode, ApprovalRule, CheckerKind};
use bise_proto::rpc::{self, Message};

/// The notifications this terminal reads typed: whole rows of
/// `bise_proto::rpc::OLDER` (a half-listed row comes the older way).
// TODO(client-protocol step 4's end, P4e): the hello's `reads` goes when
// the terminal connects with `initialize`
pub(crate) const READS: &[&str] = &["hub/approvals", "confirm/ask"];

/// The terminal's first line on the hub's socket: `hello` with [`READS`].
pub fn hello_line() -> String {
    format!("{}\n", json!({"op": "hello", "reads": READS}))
}

/// A JSON-RPC notification from the hub (a line with `jsonrpc` and a
/// `method`): its typed reader. A method this terminal doesn't read
/// (a newer hub's): nothing.
pub(super) fn read(app: &mut App, v: Value) {
    let Ok(Message::Notification(n)) = Message::from_value(v) else { return };
    let Ok((ev, _watermark)) = rpc::ev(&n) else { return };
    match &ev {
        HubEv::Approvals { .. } => approvals(app, &ev, false),
        HubEv::Confirm { id, text, .. } => confirm(app, *id, text),
        _ => {}
    }
}

/// The word the key bar and the screen use; an older or unknown mode is
/// "" (no switch, as an older hub that never said one).
fn mode_word(m: ApprovalMode) -> String {
    match m {
        ApprovalMode::Yolo => "yolo".into(),
        ApprovalMode::Auto => "auto".into(),
        ApprovalMode::Unknown => String::new(),
    }
}

fn checker_word(c: CheckerKind) -> String {
    match c {
        CheckerKind::Jev => "jev".into(),
        CheckerKind::Model => "model".into(),
        CheckerKind::Off => "off".into(),
        CheckerKind::Unknown => String::new(),
    }
}

impl Rule {
    /// The hub's row; `raw` is what approvals/removeRule sends back.
    pub(crate) fn of_row(r: &ApprovalRule) -> Rule {
        Rule {
            raw: serde_json::to_value(r).unwrap_or(Value::Null),
            tool: r.tool.clone(),
            pattern: r.pattern.clone().unwrap_or_default(),
            path: r.path.clone().unwrap_or_default(),
            every: r.project.as_deref().is_none_or(str::is_empty),
            // the hub writes ms; a hand-written date is not read
            added: r.added_ms,
            from: r.from.clone().unwrap_or_default(),
            outside: r.sandbox == Some(false),
        }
    }
}

/// The hub's approvals (approvals-design.md §8): the mode for the key
/// bar; `flash`: a switch (the 3-second flash, the first switch to auto's
/// tip); `show`: `/approvals` asked (its screen opens).
pub(super) fn approvals(app: &mut App, ev: &HubEv, show: bool) {
    let HubEv::Approvals { mode, env, checker, checker_who, checker_model, repo, rules, flash, .. } = ev else { return };
    let mode = mode_word(*mode);
    let was = app.sb.approvals.mode.clone();
    if was.is_empty() && mode == "yolo" {
        crate::hints::once(app, crate::hints::Hint::FirstYolo);
    }
    let a = &mut app.sb.approvals;
    a.mode = mode;
    a.env = *env;
    a.checker = checker_word(*checker);
    a.checker_who = checker_who.clone();
    a.checker_model = checker_model.clone().unwrap_or_default();
    a.repo = repo.clone();
    a.rules = rules.iter().map(Rule::of_row).collect();
    if *flash {
        a.flash = Some(std::time::Instant::now());
        if a.mode == "auto" && was != "auto" && a.checker != "off" {
            crate::hints::set_auto_text(&a.checker, checker_who);
            crate::hints::once(app, crate::hints::Hint::FirstAuto);
        }
    }
    if show {
        app.approvals = Some(crate::approvals_screen::Screen::default());
    }
}

/// The hub asks yes or no (`/archive` of a task with unpushed work...):
/// its words, how to answer, and the question kept for `y`/`n`.
fn confirm(app: &mut App, id: u64, text: &str) {
    push_event(&mut app.events, &mut app.cache, Ev::Warn(text.to_string()));
    push_event(&mut app.events, &mut app.cache, Ev::Info("answer y (yes) or n (no), then ⏎".into()));
    let sb = &mut app.sb;
    sb.confirm = Some((id, text.to_string()));
    sb.calls += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(ev: &HubEv) -> Value {
        Message::Notification(rpc::note(ev, None).unwrap()).to_value()
    }

    #[test]
    fn the_hello_lists_whole_older_rows_only() {
        let listed: Vec<String> = READS.iter().map(|s| s.to_string()).collect();
        let reads = rpc::reads_of(&listed);
        assert_eq!(reads.len(), READS.len(), "every method listed is part of a whole OLDER row");
        let hello: Value = serde_json::from_str(hello_line().trim()).unwrap();
        assert_eq!((hello["op"].as_str(), hello["reads"].as_array().map(Vec::len)), (Some("hello"), Some(READS.len())));
    }

    #[test]
    fn hub_approvals_sets_the_mode_rules_and_flash_as_the_older_event_did() {
        let mut app = crate::sb::bench::test_app();
        let rule = ApprovalRule {
            tool: "bash".into(),
            pattern: Some("cargo test *".into()),
            path: None,
            project: None,
            added: Some("1791100000000".into()),
            from: Some("card #3, api".into()),
            sandbox: Some(false),
            added_ms: Some(1791100000000),
            what: "cargo test *".into(),
            note: String::new(),
        };
        let ev = HubEv::Approvals {
            project: "p".into(),
            mode: ApprovalMode::Auto,
            env: false,
            checker: CheckerKind::Off,
            checker_who: String::new(),
            checker_model: None,
            repo: "/r".into(),
            rules: vec![rule],
            flash: true,
        };
        read(&mut app, note(&ev));
        let a = &app.sb.approvals;
        assert_eq!((a.mode.as_str(), a.checker.as_str(), a.repo.as_str()), ("auto", "off", "/r"));
        assert!(a.flash.is_some(), "a switch flashes");
        let r = &a.rules[0];
        assert_eq!((r.tool.as_str(), r.pattern.as_str(), r.every, r.added, r.outside), ("bash", "cargo test *", true, Some(1791100000000), true));
        assert_eq!(r.raw["added"], "1791100000000", "removeRule sends the hub's fields back");
        assert!(app.approvals.is_none(), "a notification never opens the screen");
    }

    #[test]
    fn confirm_ask_asks_and_keeps_the_question() {
        let mut app = crate::sb::bench::test_app();
        let n = app.events.len();
        read(&mut app, note(&HubEv::Confirm { project: "p".into(), id: 7, text: "t4 has 1 unpushed commit: archive it?".into() }));
        assert_eq!(app.sb.confirm, Some((7, "t4 has 1 unpushed commit: archive it?".into())));
        assert_eq!(app.events.len(), n + 2);
        // a method this terminal doesn't read: nothing
        read(&mut app, json!({"jsonrpc": "2.0", "method": "hub/newer", "params": {}}));
        assert_eq!(app.events.len(), n + 2);
    }
}
