//! The workspace's aggregate activity state for herdr: one state per
//! pane, collapsed from the many agents and the inbox. The policy is
//! blocked > working > idle (the research page herdr-bise-recognition):
//! blocked when anything needs the user (a declared block, an agent
//! waiting on you, or an open inbox card), else working when anything
//! runs (busy, starting, or waiting on an event), else idle. Ordinary
//! agent chatter never fabricates a blocked state.

use super::Agent;
use crate::herdr::State;

/// One state for the pane: `open_cards` is the inbox's open cards (the
/// user's questions, approvals, confirms, drops…).
pub(super) fn aggregate(agents: &[Agent], open_cards: usize) -> State {
    let needs_you = open_cards > 0
        || agents.iter().any(|a| a.status == "blocked")
        || agents.iter().any(|a| a.status == "waiting" && a.waiting_on == "you");
    if needs_you {
        return State::Blocked;
    }
    let running = agents
        .iter()
        .any(|a| matches!(a.status.as_str(), "working" | "waiting" | "starting") || a.waits_event());
    if running {
        State::Working
    } else {
        State::Idle
    }
}

impl super::Sb {
    /// The workspace's herdr state from the current board and inbox.
    pub(super) fn herdr_state(&self) -> State {
        aggregate(&self.agents, self.sorted_cards().len())
    }

    /// Note the aggregate state to the herdr reporter, if one is live.
    /// Cheap when nothing changed: the reporter dedupes and coalesces.
    pub(super) fn herdr_note(&mut self) {
        if let Some(reporter) = &self.herdr {
            reporter.report(self.herdr_state());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view agent with just the fields the aggregate reads; everything
    /// else defaults.
    fn agent(status: &str, waiting_on: &str) -> Agent {
        Agent {
            status: status.to_string(),
            waiting_on: waiting_on.to_string(),
            ..Default::default()
        }
    }

    fn watching(status: &str) -> Agent {
        Agent {
            status: status.to_string(),
            watching: vec![bise_proto::rows::AgentWatch {
                id: 1,
                kind: bise_proto::rows::WatchKind::Bg,
                what: "cargo test".into(),
                since_ms: 0,
                noted: false,
                resumes: true,
                until_ms: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn nothing_running_and_no_cards_is_idle() {
        assert_eq!(aggregate(&[agent("idle", "")], 0), State::Idle);
        assert_eq!(aggregate(&[agent("done", ""), agent("stopped", "")], 0), State::Idle);
        assert_eq!(aggregate(&[], 0), State::Idle);
    }

    #[test]
    fn anything_running_is_working() {
        assert_eq!(aggregate(&[agent("idle", ""), agent("working", "")], 0), State::Working);
        assert_eq!(aggregate(&[agent("starting", "")], 0), State::Working);
        // an idle agent waiting on an event still runs (the resume watch)
        assert_eq!(aggregate(&[watching("idle")], 0), State::Working);
    }

    #[test]
    fn anything_needing_the_user_is_blocked() {
        // a declared block
        assert_eq!(aggregate(&[agent("idle", ""), agent("blocked", "")], 0), State::Blocked);
        // an agent explicitly waiting on you
        assert_eq!(aggregate(&[agent("waiting", "you")], 0), State::Blocked);
        // an open inbox card (a question, an approval, a confirm…)
        assert_eq!(aggregate(&[agent("idle", "")], 1), State::Blocked);
    }

    #[test]
    fn blocked_wins_over_working() {
        // a blocked agent and a working agent at once: blocked
        assert_eq!(
            aggregate(&[agent("blocked", ""), agent("working", "")], 0),
            State::Blocked
        );
        // a card and a working agent: blocked (the user is the hold-up)
        assert_eq!(aggregate(&[agent("working", "")], 1), State::Blocked);
    }

    #[test]
    fn ordinary_chatter_is_not_blocked() {
        // an agent waiting on another agent (not you) is working, not blocked
        assert_eq!(aggregate(&[agent("waiting", "docs")], 0), State::Working);
        // a plain busy agent is working, never blocked
        assert_eq!(aggregate(&[agent("working", "")], 0), State::Working);
    }

    /// The dispatch-level regression: feeding hub lines through `dispatch`
    /// reports working, then idle, then releases, over a real unix socket
    /// (a fake herdr server). This exercises the TUI integration (dispatch
    /// -> herdr_note -> report -> worker -> socket), not just
    /// Reporter::report in isolation.
    #[test]
    fn dispatch_reports_working_then_idle_to_a_fake_herdr() {
        use std::io::BufRead;
        use std::os::unix::net::UnixListener;

        fn field(line: &str, key: &str) -> String {
            let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            if key == "method" {
                v["method"].as_str().unwrap().to_string()
            } else {
                v["params"][key].as_str().unwrap().to_string()
            }
        }

        // a fake herdr server: a real unix socket, never the user's
        let dir = std::env::temp_dir().join(format!("herdr-reg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("herdr.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        // a real reporter over that socket (never the ambient env)
        let reporter = crate::herdr::Reporter::reporter_for_test(&path, "w1:p1");

        // an App whose board reports to that reporter
        let mut app = crate::sb::bench::test_app();
        app.sb.herdr = Some(reporter);

        // working: main idle, docs working
        crate::sb::hub_reads::rows_for_tests::apply(
            &mut app,
            vec![
                crate::sb::hub_reads::rows_for_tests::agent("main", "idle", ""),
                crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write it"),
            ],
            vec![],
        );
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        let _ = std::io::BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
        assert_eq!(field(&line, "method"), "pane.report_agent");
        assert_eq!(field(&line, "state"), "working");

        // idle: only main, idle
        crate::sb::hub_reads::rows_for_tests::apply(
            &mut app,
            vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", "")],
            vec![],
        );
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        let _ = std::io::BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
        assert_eq!(field(&line, "method"), "pane.report_agent");
        assert_eq!(field(&line, "state"), "idle");

        // drop the App: the reporter releases the pane
        drop(app);
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        let _ = std::io::BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
        assert_eq!(field(&line, "method"), "pane.release_agent");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The startup/attach path: dispatching the real `initialize` answer
    /// (whose `hub.state` holds one working agent) must report the
    /// authoritative initial state — the snapshot alone delivers the
    /// initial report, not a later transition. Nothing is sent before the
    /// board is populated (never an idle from an empty board) and nothing
    /// more without a state change.
    #[test]
    fn initialize_answer_reports_the_authoritative_initial_state() {
        use std::io::BufRead;
        use std::os::unix::net::UnixListener;

        use bise_proto::rpc::{self, Id, Message, Notification, Watermark};

        fn field(line: &str, key: &str) -> String {
            let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            if key == "method" {
                v["method"].as_str().unwrap().to_string()
            } else {
                v["params"][key].as_str().unwrap().to_string()
            }
        }

        let dir = std::env::temp_dir().join(format!("herdr-init-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("herdr.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        let reporter = crate::herdr::Reporter::reporter_for_test(&path, "w1:p1");
        let mut app = crate::sb::bench::test_app();
        app.sb.herdr = Some(reporter);

        // the real initialize answer (the shape hub_reads::init reads):
        // hub.state holds one working agent, nothing else
        let rows = crate::sb::hub_reads::rows_for_tests::lines(
            vec![crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write it")],
            vec![],
        );
        let notes: Vec<Notification> = rows
            .iter()
            .map(|l| match Message::from_value(serde_json::from_str(l).unwrap()) {
                Ok(Message::Notification(n)) => n,
                _ => unreachable!(),
            })
            .collect();
        let res = serde_json::json!({
            "project": "p",
            "proto": bise_proto::PROTO,
            "workspace": "/ws",
            "name": "ws",
            "exe": "",
            "version": { "id": "v9" },
            "reload": "",
            "methods": [],
            "notifications": [],
            "hub": { "watermark": Watermark { epoch: 3, seq: 10 }, "state": notes }
        });
        let line = Message::Response(rpc::Response::ok(Id::Num(0), res)).to_value().to_string();
        crate::sb::dispatch(&mut app, &line);

        // the snapshot alone reports working — never a pre-populate idle
        let (stream, _) = listener.accept().unwrap();
        let mut first = String::new();
        let _ = std::io::BufReader::new(stream.try_clone().unwrap()).read_line(&mut first);
        assert_eq!(field(&first, "method"), "pane.report_agent");
        assert_eq!(field(&first, "state"), "working");

        // no subsequent state change: the reporter must stay quiet
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(200)))
            .unwrap();
        let mut again = String::new();
        let got = std::io::BufReader::new(stream).read_line(&mut again);
        assert!(got.is_err(), "no state change means no further report");

        drop(app);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
