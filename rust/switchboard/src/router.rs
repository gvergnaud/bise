//! What one line typed by the user asks for (RFC 0001 §7.1, RFC 0002 §3,
//! §5). Pure parsing: the core decides what happens.

use crate::model::MAIN;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserCmd {
    /// Plain text for the agent in focus.
    Say(String),
    /// `@name text`: an explicit route, no main turn (RFC 0001 §7.1).
    To {
        target: String,
        text: String,
    },
    /// `/new [-w] [--with-changes] [name:] brief`.
    New {
        name: Option<String>,
        brief: String,
        worktree: bool,
        with_changes: bool,
    },
    /// `/archive [name] [--force]` (no name: the task in focus). The
    /// hub's drop: `sb drop` is the CLI's word for it.
    Drop {
        name: Option<String>,
        force: bool,
    },
    Restore {
        name: String,
    },
    Isolate {
        name: String,
    },
    Rename {
        name: String,
        new_name: String,
    },
    /// `/answer N text`: the answer to attention card N.
    Answer {
        card: u64,
        text: String,
    },
    /// `/close N`: close attention card N without answering it.
    Close {
        card: u64,
    },
    /// `/tasks`: the board, printed locally.
    Tasks,
    /// `/prs` (pr-design §4): the open PRs of this repo's agents, printed
    /// locally.
    Prs,
    /// `/interrupt`: the agent in focus stops its turn.
    Interrupt,
    /// Any other `/command`: for the REPL of the agent in focus
    /// (`/compact`, `/status`...).
    Passthrough(String),
    /// `/model [<model>] [default]` (BISE-135): the model of the agent
    /// in focus from its next call; `default`: also config.toml's
    /// (`model` for main, `agent_model` for a sub-agent). No model: say
    /// which one it runs.
    Model {
        model: Option<String>,
        default: bool,
    },
    /// `/reasoning [<effort>]` (BISE-135): its reasoning effort.
    Reasoning {
        effort: Option<String>,
    },
    /// `/flow [pr|trunk]` (dev-flow §7): show the repo's flow and why, or
    /// switch it (saved in `.switchboard/config.toml`).
    Flow {
        set: Option<crate::flow::FlowMode>,
    },
    Help,
    /// `/stop <agent>` (computer-use-design §7.3): that agent's turn
    /// stops. The hub's daemon runs it (the TUI runs its own arm with the
    /// same parse); the core never gets it from a client.
    Stop {
        name: String,
    },
    /// `/version [list|back|<v>]`, `/restart [<v>]`, `/update`: the hub's
    /// versions (the daemon's `version` op, his authority, client socket
    /// only: docs/issues/16).
    Version(bise_proto::slash::Version),
    /// `/approvals [yolo|auto]`: show the approvals or switch the mode
    /// (the daemon's approvals, his authority, client socket only).
    Approvals {
        mode: Option<bise_proto::rows::ApprovalMode>,
    },
    Invalid(String),
}

/// `/cancel` is gone (book §13): an agent may already have acted on a
/// message, so an undo promises too much. Corrections go through main.
pub const NO_UNDO: &str =
    "no undo: an agent may already have acted. say the change to main instead (\"no, v1 for docs\").";

/// A task name (RFC 0001 §9.3): `[a-z0-9-]{1,24}`.
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 24
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
}

/// A task name from free text: the first words, lowercased and dashed.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        let w: String = word
            .chars()
            .map(fold_accent)
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if w.is_empty() || w.len() < 2 && out.is_empty() {
            continue;
        }
        let next = if out.is_empty() {
            w
        } else {
            format!("{}-{}", out, w)
        };
        if next.len() > 24 {
            break;
        }
        out = next;
        if out.matches('-').count() >= 2 {
            break;
        }
    }
    if out.is_empty() {
        "task".to_string()
    } else {
        out
    }
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ç' => 'c',
        'À' | 'Â' => 'a',
        'É' | 'È' | 'Ê' => 'e',
        _ => c,
    }
}

/// `name: rest` when `name` is a valid task name.
fn split_named(s: &str) -> (Option<String>, String) {
    if let Some((head, rest)) = s.split_once(':') {
        let head = head.trim();
        if valid_name(head) && !head.contains(' ') {
            return (Some(head.to_string()), rest.trim().to_string());
        }
    }
    (None, s.trim().to_string())
}

/// A task name as typed (`@name` or `name`).
fn bare(w: &str) -> String {
    w.trim_start_matches('@').to_string()
}

/// The task in focus, when the focus is not main (a command's default).
fn focus_task(focus: &str) -> Option<String> {
    (focus != MAIN).then(|| focus.to_string())
}

/// Parse one line typed while `focus` has the focus.
pub fn parse(line: &str, focus: &str) -> UserCmd {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix('@') {
        let (target, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let text = text.trim();
        if text.is_empty() {
            return UserCmd::Invalid(format!("empty message for @{}", target));
        }
        return UserCmd::To {
            target: target.to_string(),
            text: text.to_string(),
        };
    }
    if !line.starts_with('/') {
        return UserCmd::Say(line.to_string());
    }
    let (cmd, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let words: Vec<&str> = rest.split_whitespace().collect();
    match cmd {
        "/new" => {
            let mut worktree = false;
            let mut with_changes = false;
            let mut body = rest;
            loop {
                let b = body.trim_start();
                if let Some(r) = b
                    .strip_prefix("-w ")
                    .or_else(|| b.strip_prefix("--worktree "))
                {
                    worktree = true;
                    body = r;
                } else if let Some(r) = b.strip_prefix("--with-changes ") {
                    with_changes = true;
                    body = r;
                } else if let Some(r) = b.strip_prefix("-s ") {
                    body = r;
                } else {
                    body = b;
                    break;
                }
            }
            let (name, brief) = split_named(body);
            if brief.is_empty() {
                return UserCmd::Invalid("usage: /new [-w] [name:] objective".into());
            }
            if with_changes && !worktree {
                return UserCmd::Invalid("--with-changes only works with -w".into());
            }
            UserCmd::New {
                name,
                brief,
                worktree,
                with_changes,
            }
        }
        "/archive" => {
            let force = words.contains(&"--force");
            let name = words
                .iter()
                .find(|w| !w.starts_with("--"))
                .map(|w| bare(w));
            let name = name.or_else(|| focus_task(focus));
            UserCmd::Drop { name, force }
        }
        "/restore" | "/isolate" => {
            let name = words
                .first()
                .map(|w| bare(w))
                .or_else(|| focus_task(focus));
            match name {
                Some(name) if cmd == "/restore" => UserCmd::Restore { name },
                Some(name) => UserCmd::Isolate { name },
                None => UserCmd::Invalid(format!("usage: {} <agent>", cmd)),
            }
        }
        "/rename" => match words.as_slice() {
            [a, b] => UserCmd::Rename {
                name: bare(a),
                new_name: bare(b),
            },
            [b] if focus != MAIN => UserCmd::Rename {
                name: focus.to_string(),
                new_name: bare(b),
            },
            _ => UserCmd::Invalid("usage: /rename <agent> <new-name>".into()),
        },
        "/answer" | "/reply" => {
            let (n, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            match crate::core_num::card_arg(n) {
                Ok(card) if !text.trim().is_empty() => UserCmd::Answer {
                    card,
                    text: text.trim().to_string(),
                },
                Ok(_) => UserCmd::Invalid("usage: /answer <card> <answer>".into()),
                Err(e) => UserCmd::Invalid(format!("{} (usage: /answer <card> <answer>)", e)),
            }
        }
        "/close" => match crate::core_num::card_arg(rest) {
            Ok(card) => UserCmd::Close { card },
            Err(e) => UserCmd::Invalid(format!("{} (usage: /close <card>)", e)),
        },
        "/cancel" | "/undo" => UserCmd::Invalid(NO_UNDO.into()),
        // `/agents`, the board of every agent (`/tasks`: the old name)
        "/agents" | "/tasks" => UserCmd::Tasks,
        "/prs" => UserCmd::Prs,
        "/interrupt" => UserCmd::Interrupt,
        "/model" => {
            let default = words.contains(&"default");
            let rest: Vec<&str> = words.iter().copied().filter(|w| *w != "default").collect();
            match rest.as_slice() {
                [] if default => UserCmd::Invalid("usage: /model <model> default".into()),
                [] => UserCmd::Model { model: None, default },
                [m] => UserCmd::Model { model: Some(m.to_string()), default },
                _ => UserCmd::Invalid("usage: /model [<model>] [default]".into()),
            }
        }
        "/reasoning" | "/effort" => match words.as_slice() {
            [] => UserCmd::Reasoning { effort: None },
            [e] => UserCmd::Reasoning { effort: Some(e.to_ascii_lowercase()) },
            _ => UserCmd::Invalid("usage: /reasoning [<effort>]".into()),
        },
        "/flow" => match words.as_slice() {
            [] => UserCmd::Flow { set: None },
            [m] => match crate::devflow::parse_mode(m) {
                Ok(m) => UserCmd::Flow { set: Some(m) },
                Err(e) => UserCmd::Invalid(e),
            },
            _ => UserCmd::Invalid("usage: /flow [pr|trunk]".into()),
        },
        "/help" => UserCmd::Help,
        // one parse with the TUI's own arms (bise_proto::slash)
        "/stop" => match bise_proto::slash::stop(line) {
            Some(Ok(name)) => UserCmd::Stop { name },
            Some(Err(usage)) => UserCmd::Invalid(usage),
            None => UserCmd::Passthrough(line.to_string()),
        },
        "/version" | "/restart" | "/update" => match bise_proto::slash::version(line) {
            Some(v) => UserCmd::Version(v),
            None => UserCmd::Passthrough(line.to_string()),
        },
        "/approvals" => match bise_proto::slash::approvals(line) {
            Some(Ok(mode)) => UserCmd::Approvals { mode },
            Some(Err(words)) => UserCmd::Invalid(words),
            None => UserCmd::Passthrough(line.to_string()),
        },
        _ => UserCmd::Passthrough(line.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_flow() {
        use crate::flow::FlowMode;
        assert_eq!(parse("/flow", MAIN), UserCmd::Flow { set: None });
        assert_eq!(parse("/flow pr", "docs"), UserCmd::Flow { set: Some(FlowMode::Pr) });
        assert_eq!(parse("/flow trunk", MAIN), UserCmd::Flow { set: Some(FlowMode::Trunk) });
        assert_eq!(parse("/flow nope", MAIN), UserCmd::Invalid("usage: /flow [pr|trunk]".into()));
        assert!(matches!(parse("/flow pr trunk", MAIN), UserCmd::Invalid(_)));
    }

    #[test]
    fn model_and_reasoning() {
        assert_eq!(parse("/model", MAIN), UserCmd::Model { model: None, default: false });
        assert_eq!(
            parse("/model anthropic/claude-sonnet-4-5 default", MAIN),
            UserCmd::Model { model: Some("anthropic/claude-sonnet-4-5".into()), default: true }
        );
        assert_eq!(parse("/model default opus-5.5", "docs"), UserCmd::Model { model: Some("opus-5.5".into()), default: true });
        assert!(matches!(parse("/model default", MAIN), UserCmd::Invalid(_)));
        assert!(matches!(parse("/model a b", MAIN), UserCmd::Invalid(_)));
        assert_eq!(parse("/reasoning", MAIN), UserCmd::Reasoning { effort: None });
        assert_eq!(parse("/reasoning High", MAIN), UserCmd::Reasoning { effort: Some("high".into()) });
        assert_eq!(parse("/effort low", MAIN), UserCmd::Reasoning { effort: Some("low".into()) });
    }

    #[test]
    fn plain_text_goes_to_the_focus() {
        assert_eq!(parse("  bonjour ", MAIN), UserCmd::Say("bonjour".into()));
    }

    #[test]
    fn at_name_is_an_explicit_route() {
        assert_eq!(
            parse("@docs v2 please", MAIN),
            UserCmd::To {
                target: "docs".into(),
                text: "v2 please".into()
            }
        );
        assert!(matches!(parse("@docs", MAIN), UserCmd::Invalid(_)));
    }

    #[test]
    fn new_with_flags_and_name() {
        assert_eq!(
            parse("/new -w --with-changes fix-safari: le login casse", MAIN),
            UserCmd::New {
                name: Some("fix-safari".into()),
                brief: "le login casse".into(),
                worktree: true,
                with_changes: true
            }
        );
        assert_eq!(
            parse("/new corrige: ceci", MAIN),
            UserCmd::New {
                name: Some("corrige".into()),
                brief: "ceci".into(),
                worktree: false,
                with_changes: false
            }
        );
        // a colon inside a sentence is not a name
        assert_eq!(
            parse("/new Regarde ceci: le bug", MAIN),
            UserCmd::New {
                name: None,
                brief: "Regarde ceci: le bug".into(),
                worktree: false,
                with_changes: false
            }
        );
        assert!(matches!(
            parse("/new --with-changes x: y", MAIN),
            UserCmd::Invalid(_)
        ));
    }

    #[test]
    fn archive_defaults_to_the_task_in_focus() {
        assert_eq!(
            parse("/archive", "docs"),
            UserCmd::Drop {
                name: Some("docs".into()),
                force: false
            }
        );
        assert_eq!(
            parse("/archive", MAIN),
            UserCmd::Drop {
                name: None,
                force: false
            }
        );
        assert_eq!(
            parse("/archive @bench --force", MAIN),
            UserCmd::Drop {
                name: Some("bench".into()),
                force: true
            }
        );
    }

    #[test]
    fn answer_and_passthrough() {
        assert_eq!(
            parse("/answer #3 v2", MAIN),
            UserCmd::Answer {
                card: 3,
                text: "v2".into()
            }
        );
        assert_eq!(
            parse("/compact", "docs"),
            UserCmd::Passthrough("/compact".into())
        );
    }

    #[test]
    fn there_is_no_undo() {
        assert_eq!(parse("/cancel", MAIN), UserCmd::Invalid(NO_UNDO.into()));
        assert_eq!(parse("/undo", "docs"), UserCmd::Invalid(NO_UNDO.into()));
        // `/agents` (book §4: agents, never tasks); `/tasks` stays an alias
        assert_eq!(parse("/agents", MAIN), UserCmd::Tasks);
        assert_eq!(parse("/tasks", "docs"), UserCmd::Tasks);
        assert_eq!(parse("/prs", "docs"), UserCmd::Prs);
    }

    #[test]
    fn names_and_slugs() {
        assert!(valid_name("fix-safari-2"));
        assert!(!valid_name("Fix"));
        assert!(!valid_name("-x"));
        assert!(!valid_name(&"a".repeat(25)));
        assert_eq!(slug("Corrige le login Safari, vite"), "corrige-le-login");
        assert_eq!(slug("Écrire la doc de l'API"), "ecrire-la-doc");
        assert_eq!(slug("!!!"), "task");
        assert!(valid_name(&slug("un nom extrêmement long qui dépasse")));
    }
}
