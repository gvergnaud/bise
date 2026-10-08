//! The slash commands' catalog (client-protocol P2, plan signed by
//! architect m_13089): the 34 commands the TUI's `/` popup offers and
//! `/help` lists, each with its arguments and what completes them, and
//! `client` for the 19 a client parses itself (a screen or a state of
//! its own); the others are the hub's (the typed line goes to its
//! router). Moved as is from the TUI's commands.rs: the TUI reads the
//! static table (same binary), a client over the wire gets it serialized
//! (`commands/list`).
//!
//! Wire shape: `{name, desc, args, client}`, an argument
//! `{"kind": "words", "words": [{value, desc}]}` (the same for `version`
//! and `dev_version`) or `{"kind": "task"}` for the others.

use serde::ser::{SerializeSeq, Serializer};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Cmd {
    pub name: &'static str,
    /// what it does, then its usage after `: ` when it takes arguments
    pub desc: &'static str,
    /// its arguments in order, each with what completes it (BISE-117):
    /// the `/` popup offers them after the command's name
    pub args: &'static [Arg],
    /// the client parses and runs it itself (a screen of its own); false:
    /// the hub's router does (the line goes to the hub)
    pub client: bool,
}

/// What completes one argument of a command (BISE-117).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "words", rename_all = "snake_case")]
pub enum Arg {
    /// one of these words, and what each does
    Words(#[serde(serialize_with = "words")] &'static [(&'static str, &'static str)]),
    /// a live agent (not main)
    Task,
    /// an archived agent
    Archived,
    /// an open card, by its number
    Card,
    /// a version of switchboard (the hub's list: the commits, tree,
    /// back), after these words
    Version(#[serde(serialize_with = "words")] &'static [(&'static str, &'static str)]),
    /// the same, the versions only in bise's source tree (`/restart`:
    /// elsewhere it only reloads, a commit is refused)
    DevVersion(#[serde(serialize_with = "words")] &'static [(&'static str, &'static str)]),
    /// a plugin of the workspace
    Plugin,
    /// a model of the catalog, or an alias (BISE-135)
    Model,
    /// an effort the model of the agent in view takes (BISE-135)
    Effort,
    /// `/computer-use`'s own, by the plugin's state: off → "on" (runs the
    /// bare `/computer-use`); on → setup (the bare one), off, uninstall
    ComputerUse,
    /// a branch to diff against main (the hub's `branches`: agents'
    /// branches, shared worktrees, PRs, branches with no agent)
    Branch,
    /// `/keychain`'s on and off, the setting now marked `· now`
    Keychain,
    /// free text, required: the rest of the line (nothing to complete)
    Text,
    /// free text, optional: the value before it can already run
    Note,
}

/// One word of an argument on the wire, the shape of the window's
/// `PickChoice`.
#[derive(Serialize)]
struct Word {
    value: &'static str,
    desc: &'static str,
}

fn words<S: Serializer>(ws: &&'static [(&'static str, &'static str)], s: S) -> Result<S::Ok, S::Error> {
    let mut seq = s.serialize_seq(Some(ws.len()))?;
    for (value, desc) in ws.iter() {
        seq.serialize_element(&Word { value, desc })?;
    }
    seq.end()
}

const THEMES: &[(&str, &str)] =
    &[("auto", "your terminal's background"), ("light", "the light palette"), ("dark", "the dark palette")];

/// The slash commands the popup offers and `/help` lists.
pub const COMMANDS: &[Cmd] = &[
    // voice-menu (designer): one row, one ⏎, one screen; `/voice setup`
    // typed still opens it, on speech to text, but the menu offers no
    // argument
    Cmd { name: "/voice", desc: "dictation and voice mode: the model, the voice, the language", args: &[], client: true },
    Cmd {
        name: "/restart",
        desc: "reload bise, nothing lost (a <commit>: bise's own sources only, built then switched): /restart [current|<commit>]",
        args: &[Arg::DevVersion(&[("current", "the version running now, nothing built")])],
        client: false,
    },
    Cmd {
        name: "/update",
        desc: "look for a new bise release now, and install it from its item (bise's source tree: build HEAD and restart on it)",
        args: &[],
        client: false,
    },
    Cmd {
        name: "/version",
        desc: "switchboard versions: /version [<commit>|tree|back]",
        // the hub's list holds tree and back
        args: &[Arg::Version(&[])],
        client: false,
    },
    Cmd {
        name: "/new",
        desc: "start an agent: /new [-w] [name:] objective",
        args: &[Arg::Words(&[("-w", "in its own git worktree")]), Arg::Text],
        client: false,
    },
    Cmd {
        name: "/archive",
        desc: "stop an agent and archive it, with its worktree: /archive <agent>",
        args: &[Arg::Task],
        client: false,
    },
    Cmd { name: "/restore", desc: "bring an archived agent back: /restore <agent>", args: &[Arg::Archived], client: false },
    Cmd { name: "/archived", desc: "show or hide the archived agents in the panel", args: &[], client: true },
    Cmd { name: "/isolate", desc: "give an agent its own git worktree: /isolate <agent>", args: &[Arg::Task], client: false },
    Cmd { name: "/rename", desc: "rename an agent: /rename <agent> <new-name>", args: &[Arg::Task, Arg::Text], client: false },
    Cmd { name: "/answer", desc: "answer an inbox item: /answer N text", args: &[Arg::Card, Arg::Text], client: false },
    Cmd { name: "/close", desc: "close an inbox item without answering: /close N [note]", args: &[Arg::Card, Arg::Note], client: false },
    Cmd {
        name: "/plugins",
        desc: "the workspace's agent plugins: /plugins [list|enable|disable|login|logout] [<name>]",
        args: &[
            Arg::Words(&[
                ("list", "the plugins and their state"),
                ("enable", "turn a plugin on"),
                ("disable", "turn a plugin off"),
                ("login", "log in to a remote MCP server"),
                ("logout", "forget a remote MCP server's login"),
            ]),
            Arg::Plugin,
        ],
        client: true,
    },
    Cmd { name: "/artifacts", desc: "what your agents made: pages, docs, files, links", args: &[], client: true },
    Cmd { name: "/scheduled", desc: "your scheduled tasks: list, open, run now, stop", args: &[], client: true },
    Cmd {
        name: "/diff",
        desc: "a branch's changes against main, in a panel on the right: /diff [<branch>]",
        args: &[Arg::Branch],
        client: true,
    },
    Cmd { name: "/inbox", desc: "open what waits for you, the most blocking first (also /cards)", args: &[], client: true },
    Cmd { name: "/agents", desc: "list the agents and what they do", args: &[], client: true },
    Cmd {
        name: "/switch",
        desc: "find an agent by name, archived ones too, and open it (also cmd+k, ctrl+s)",
        args: &[],
        client: true,
    },
    Cmd {
        name: "/model",
        desc: "the model of the agent in view: /model [<model>] [default]",
        args: &[
            Arg::Model,
            Arg::Words(&[("default", "also for new sessions (config.toml [roles]: main, or agents for an agent)")]),
        ],
        client: false,
    },
    Cmd {
        name: "/models",
        desc: "which model does what: main, agents, small jobs (titles, summaries), voice",
        args: &[],
        client: true,
    },
    Cmd { name: "/provider", desc: "set up a provider's key, or change it", args: &[], client: true },
    // designer m_13193: macOS only (the TUI's popup hides it elsewhere)
    Cmd { name: "/keychain", desc: "keep your keys and sign-ins in the macOS keychain: /keychain [on|off]", args: &[Arg::Keychain], client: true },
    Cmd { name: "/reasoning", desc: "its reasoning effort: /reasoning [<effort>]", args: &[Arg::Effort], client: false },
    Cmd { name: "/interrupt", desc: "interrupt the turn of the agent in view", args: &[], client: false },
    Cmd {
        name: "/stop",
        desc: "stop an agent's turn and its hands on Chrome or an app until you write to it: /stop <agent>",
        args: &[Arg::Task],
        client: false,
    },
    Cmd {
        name: "/computer-use",
        desc: "turn on and set up computer use: agents drive Chrome and your apps, step by step: /computer-use [off|uninstall]",
        args: &[Arg::ComputerUse],
        client: true,
    },
    Cmd { name: "/compact", desc: "compact the conversation of the agent in view", args: &[], client: false },
    Cmd {
        name: "/theme",
        desc: "light, dark, or auto (your terminal's background): /theme [auto|light|dark]",
        args: &[Arg::Words(THEMES)],
        client: true,
    },
    Cmd { name: "/welcome", desc: "replay the welcome of the first launch", args: &[], client: true },
    Cmd { name: "/setup", desc: "check your terminal and repo again, and offer what would help", args: &[], client: true },
    Cmd { name: "/help", desc: "the commands and the essential keys", args: &[], client: true },
    Cmd { name: "/shortcuts", desc: "every keyboard shortcut (also /keys)", args: &[], client: true },
    Cmd { name: "/quit", desc: "quit (the agents keep running)", args: &[], client: true },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Law: 34 commands, each name once and a slash word; 19 the
    /// client's own, the 15 others the hub's router parses.
    #[test]
    fn the_catalog_holds_34_commands_19_of_them_the_clients() {
        assert_eq!(COMMANDS.len(), 34);
        assert_eq!(COMMANDS.iter().filter(|c| c.client).count(), 19);
        let hub: Vec<&str> = COMMANDS.iter().filter(|c| !c.client).map(|c| c.name).collect();
        assert_eq!(
            hub,
            [
                "/restart", "/update", "/version", "/new", "/archive", "/restore", "/isolate", "/rename", "/answer", "/close", "/model",
                "/reasoning", "/interrupt", "/stop", "/compact",
            ]
        );
        for (i, c) in COMMANDS.iter().enumerate() {
            assert!(c.name.starts_with('/') && !c.name.contains(' '), "{}", c.name);
            assert!(COMMANDS[..i].iter().all(|o| o.name != c.name), "{} twice", c.name);
            assert!(!c.desc.is_empty(), "{}", c.name);
        }
    }

    /// Law: the wire shape of a command and of each kind of argument.
    #[test]
    fn a_command_serializes_with_its_args_and_who_runs_it() {
        let find = |n: &str| COMMANDS.iter().find(|c| c.name == n).unwrap();
        assert_eq!(
            serde_json::to_value(find("/new")).unwrap(),
            json!({"name": "/new", "desc": "start an agent: /new [-w] [name:] objective", "client": false,
                "args": [{"kind": "words", "words": [{"value": "-w", "desc": "in its own git worktree"}]}, {"kind": "text"}]})
        );
        let args = |n: &str| serde_json::to_value(find(n).args).unwrap();
        assert_eq!(args("/version"), json!([{"kind": "version", "words": []}]));
        assert_eq!(args("/restart")[0]["kind"], "dev_version");
        assert_eq!(args("/close"), json!([{"kind": "card"}, {"kind": "note"}]));
        assert_eq!(args("/computer-use"), json!([{"kind": "computer_use"}]));
        assert_eq!(args("/restore"), json!([{"kind": "archived"}]));
        assert_eq!(serde_json::to_value(find("/inbox")).unwrap()["client"], true);
    }
}
