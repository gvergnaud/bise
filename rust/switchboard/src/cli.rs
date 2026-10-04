//! `sb`: what an agent runs, through its bash tool, to reach the group
//! (RFC 0003 §4, RFC 0001 §7.2 and §7.5). It speaks to the hub of its
//! workspace (`SB_SOCKET`) as `SB_AGENT`.

use serde_json::{json, Map, Value};
use std::io::Read;
use std::time::Duration;

/// Who may run a command (and whose system prompt lists it).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Everyone,
    /// A task (not main): listed in the task prompt only.
    Task,
    Main,
}

/// One `sb` command: its syntax, what it does, who runs it. The one
/// source of the CLI's usage text and of the command lists in the agents'
/// system prompts (prompts.rs): a flag changes here only.
pub struct CmdDoc {
    pub syntax: &'static str,
    pub doc: &'static str,
    pub who: Who,
}

const fn cmd(syntax: &'static str, who: Who, doc: &'static str) -> CmdDoc {
    CmdDoc { syntax, doc, who }
}

pub const COMMANDS: &[CmdDoc] = &[
    cmd("sb list", Who::Everyone, "every agent of the group, its status and what it is for."),
    cmd("sb tasks", Who::Everyone, "every task in detail: what it is doing now, its last report, its open questions."),
    cmd(
        "sb send <agent> \"<text>\" [--expect-reply] [--reply-to <id>] [--mode steer|queued]",
        Who::Everyone,
        "send a message (never blocks). `--mode steer` (default): a busy recipient gets it at once, mid-turn. `--mode queued`: it waits until the recipient's turn ends, then starts its next turn.",
    ),
    cmd(
        "sb ask <agent> \"<question>\" [--timeout <s>]",
        Who::Everyone,
        "send a question and wait up to ~25 s for the answer. No answer yet: end your turn, the reply wakes you up later.",
    ),
    cmd("sb wait <id> [--timeout <s>]", Who::Everyone, "wait for the reply to a message you sent."),
    cmd(
        "sb status working|done|blocked [--note \"<text>\"]",
        Who::Everyone,
        "declare your state (shown to everyone).",
    ),
    cmd(
        "sb report progress|done|failed|blocked \"<summary>\" [--decision \"<text>\"]...",
        Who::Everyone,
        "tell main.",
    ),
    cmd(
        "sb inspect <agent> [--query <text>] [--before|--after|--around|--at #<pos>] [--limit <n>]",
        Who::Everyone,
        "read another agent's thread in bounded pages: each entry carries a position `#<n>`; a search returns positions, then page before/after/around one, or read one entry whole with `--at`.",
    ),
    cmd(
        "sb history \"<words>\" [--agent <a>] [--role user|assistant|message|tool|hub] [--since 2w] [--until <date>] [--archived|--live] [--page <n>]",
        Who::Everyone,
        "search every agent's thread (main, tasks, archived tasks), from before any compaction too: ranked hits `<agent>#<pos>` with a one-line snippet, 10 per page. Use it when the user mentions past work (\"what you did last week on X\").",
    ),
    cmd(
        "sb show <agent>#<pos> [--context <n>]",
        Who::Everyone,
        "open a hit: the entry with its neighbors, the commands to move earlier/later, and the agents, messages and commits it mentions.",
    ),
    cmd(
        "sb land [--here] [--add <path>]... \"<message>\"",
        Who::Everyone,
        "commit the files you changed (only yours, never another agent's) with that message. `--here`: on your place's branch (the shared folder: its branch, main). Without it, from a worktree: the branch is rebased on main, checked, and main moves to it (pushed when the repo says so); from the shared folder, the same as `--here`. A file another agent also changed is refused: main decides. New files: in a worktree you have alone, every new file not ignored is yours and lands; elsewhere, new files you made with bash (a generator, a download) land only with `--add <file or folder>`, and the land names the new files it left out.",
    ),
    cmd(
        "sb artifact add <path or link> [--title \"<t>\"] [--kind <k>] | sb artifact list [<words>] [--agent <a>]",
        Who::Everyone,
        "artifacts: what you made for the user to look at (a doc, a sheet, a deck, a site, an image, a PR, a deploy), listed in the user's /artifacts with a copy of each version of a file (50 MB at most). `add` registers it, or its next version when the same path or link comes again, and prints its id; bise pages get in by themselves. Not the code you changed for a task (that's the commit), not scratch files. `list`: the ids, to link them.",
    ),
    cmd(
        "sb inspect main --origin",
        Who::Task,
        "the user message that led to your creation, verbatim, and main's turn up to the spawn.",
    ),
    cmd(
        "sb spawn <name> --objective \"…\" [--context \"…\"] [--constraint \"…\"]... [--done-when \"…\"] [--report-format \"…\"] [--place new|<agent>|<branch> [--with-changes]] [--feature <name>] [--model <provider/id>] [--effort low|medium|high] [--profile <name>]",
        Who::Main,
        "create a task (names: [a-z0-9-], at most 24 chars). Where it works: the shared folder by default; `--place new` a new git worktree (`--worktree` says the same); `--place <agent>` or `<branch>` the worktree of agents already on that change (they share it and its branch); `--feature <name>` its own worktree from that feature's tip, landing on the feature. Its model, for its whole life: `--model` the model the task runs on (default: the agents model, `bise config set agents`); `--effort` its reasoning effort (default: the model's); `--profile` a named model + effort from config.toml [profiles.<name>], e.g. fast, deep, review. A model that cannot run (unknown, no key) never fails the spawn: the task runs on the agents default and the answer says what was asked and why.",
    ),
    cmd(
        "sb send <task> --model <provider/id> [--effort <e>] [\"<text>\"]",
        Who::Main,
        "move a task to another model (and effort), from its next turn; with a text, the message goes too.",
    ),
    cmd(
        "sb feature [new|sync|ready|merge|drop|list] <name>",
        Who::Main,
        "feature branches (trunk flow): `new` a local branch from main's tip (an existing local branch is adopted), never pushed; `sync` rebases it on main and moves its agents' worktrees; `ready` runs the check and opens the user's \"ready to try\" item; `merge` (only on the user's go) rebases, checks, fast-forwards main, pushes, archives its agents; `drop` (only on the user's word) deletes it, its tip kept in refs/switchboard/trash/. No argument: the list.",
    ),
    cmd(
        "sb move <agent> new|shared|<agent>|<branch>",
        Who::Main,
        "move a task that has changed nothing yet to a new worktree, back to the shared folder, or into another agent's worktree.",
    ),
    cmd("sb interrupt <task>", Who::Main, "stop a task's current turn."),
    cmd("sb stop <task> \"<reason>\"", Who::Main, "stop the task."),
    cmd(
        "sb drop <task>",
        Who::Main,
        "stop and archive a task; refused when work could be lost (the user then decides).",
    ),
    cmd(
        "sb send <agent> --reply-to <id> --why \"<reason>\" \"<answer>\"",
        Who::Main,
        "answer an agent's question on the user's behalf; the user sees the answer and the one-sentence why.",
    ),
    cmd(
        "sb card \"<question for the user>\" [--for <id>]",
        Who::Main,
        "escalate to the user's inbox (only you can); with `--for`, the user's answer goes straight to the task that asked. From then on only the user answers or closes it: a reply to that message is refused.",
    ),
    cmd(
        "sb card --withdraw <card> \"<why>\"",
        Who::Main,
        "take back your own card when it became moot (the task stopped, the user answered you in chat); the user sees it withdrawn with your why, and the question is yours again. Never to answer in the user's place.",
    ),
    cmd(
        "sb flow [pr|trunk]",
        Who::Main,
        "how this repo ships code: PRs or straight to main, why, and the question to ask the user when it is not set; with `pr` or `trunk`, save the user's answer (never your own pick).",
    ),
    cmd("sb rename <task> <new-name>", Who::Main, "rename a task (unique name; the old name still works)."),
    cmd(
        "sb restore <task>",
        Who::Main,
        "reopen a stopped or archived task (and its saved worktree). Use it ONLY when the user explicitly asks; never on your own initiative.",
    ),
    cmd(
        "sb history \"<query>\"",
        Who::Main,
        "search your whole past thread and the hub journal. Use it before saying you do not remember.",
    ),
    cmd(
        "sb version [list | switch <commit|id|tree> | rollback]",
        Who::Main,
        "the versions of bise itself. Switch or roll back ONLY when the user explicitly asks. Prefer a commit over `tree` when the working tree has work in progress. Before a switch, warn the user about the probation period: the new version is watched for about 2 minutes and rolled back automatically if it fails.",
    ),
    cmd(
        "sb restart [current | <commit>]",
        Who::Main,
        "reload bise safely, like an editor's \"Reload Window\": the hub, every agent's REPL (at its next idle, same session) and the TUI restart on the running version, nothing built, nothing lost. Only when the workspace is bise's own source tree (dev mode), it is unchanged: plain `sb restart` builds the latest commit (HEAD) and restarts on it with the same probation as a switch, `sb restart <commit>` that commit, `sb restart current` restarts the hub on the running version without rebuilding (the agents keep running). Use it ONLY when the user explicitly asks.",
    ),
];

/// The command list of a system prompt: `- `syntax` — doc` per line.
pub fn command_list(who: &[Who]) -> String {
    COMMANDS
        .iter()
        .filter(|c| who.contains(&c.who))
        .map(|c| format!("- `{}` — {}", c.syntax, c.doc))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `sb help`: the syntax of every command, main's last.
pub fn usage() -> String {
    let line = |w: Who| {
        COMMANDS
            .iter()
            .filter(move |c| c.who == w)
            .map(|c| c.syntax)
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{}\ntasks only:\n{}\nmain only:\n{}\ntools:
sb worktree <path>|none   (gate.sh new/done) tell the hub you work in that git worktree, or no longer
A text argument `-` reads the text from stdin; <id> is a message id (m_<n>).",
        line(Who::Everyone),
        line(Who::Task),
        line(Who::Main)
    )
}

/// `sb <cmd> --help` (or `-h`): the lines of `usage` for that command,
/// else the whole usage (qa-explore L: `--help` was an unknown option).
pub fn help_for(cmd: &str) -> String {
    let u = usage();
    let pre = format!("sb {}", cmd);
    let mine: Vec<&str> = u
        .lines()
        .filter(|l| l.split(" | ").any(|p| p == pre || p.starts_with(&format!("{} ", pre))))
        .collect();
    if mine.is_empty() {
        u
    } else {
        mine.join("\n")
    }
}

/// Split flags from positional words. `flags` take a value, `switches`
/// do not; a repeated flag accumulates.
/// `sb worktree <path>`: an absolute path must be a git worktree (a
/// `.git` in it) on this machine; the hub checks the rest (absolute,
/// not main). This process runs where the agent works, the hub may not.
fn worktree_exists(p: &str) -> Result<(), String> {
    let p = p.trim();
    if p.starts_with('/') && !std::path::Path::new(p).join(".git").exists() {
        return Err(format!("sb worktree: {} is not a git worktree (no .git in it)", p));
    }
    Ok(())
}

fn parse_args(
    args: &[String],
    flags: &[&str],
    switches: &[&str],
) -> Result<(Vec<String>, Map<String, Value>), String> {
    let mut pos: Vec<String> = Vec::new();
    let mut opts: Map<String, Value> = Map::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = a.strip_prefix("--") {
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (name.to_string(), None),
            };
            if switches.contains(&name.as_str()) {
                opts.insert(name, json!(true));
            } else if flags.contains(&name.as_str()) {
                let v = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        args.get(i)
                            .cloned()
                            .ok_or(format!("--{} expects a value", name))?
                    }
                };
                match opts.get_mut(&name) {
                    Some(Value::Array(a)) => a.push(json!(v)),
                    Some(prev) => {
                        let p = prev.clone();
                        *prev = json!([p, v]);
                    }
                    None => {
                        opts.insert(name, json!(v));
                    }
                }
            } else {
                return Err(format!("unknown option: --{}", name));
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Ok((pos, opts))
}

fn text_of(words: &[String]) -> Result<String, String> {
    if words.len() == 1 && words[0] == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        return Ok(s.trim().to_string());
    }
    let t = words.join(" ");
    if t.trim().is_empty() {
        return Err("missing text".into());
    }
    Ok(t)
}

fn list_of(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The first positional word, an agent name (`@name` accepted).
fn agent_arg(pos: &[String], usage: impl Into<String>) -> Result<String, String> {
    pos.first()
        .map(|a| a.trim_start_matches('@').to_string())
        .ok_or_else(|| usage.into())
}

/// `--timeout <s>`, 20 s by default.
fn timeout_of(opts: &Map<String, Value>) -> u64 {
    str_of(opts, "timeout").parse::<u64>().unwrap_or(20)
}

fn str_of(opts: &Map<String, Value>, k: &str) -> String {
    match opts.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.last().and_then(|x| x.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

/// The JSON request for `sb <args>`.
pub fn build(args: &[String]) -> Result<Value, String> {
    let Some(cmd) = args.first() else {
        return Err(usage());
    };
    let rest = &args[1..];
    let mut req = Map::new();
    req.insert("cmd".into(), json!(cmd));
    match cmd.as_str() {
        "list" | "tasks" => {
            parse_args(rest, &[], &[])?;
        }
        "history" => {
            let (pos, o) = parse_args(
                rest,
                &["agent", "role", "since", "until", "limit", "page"],
                &["archived", "live"],
            )?;
            req.insert("query".into(), json!(text_of(&pos)?));
            let list = |k: &str| -> Vec<String> {
                match o.get(k) {
                    Some(Value::String(s)) => vec![s.clone()],
                    Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
                    _ => Vec::new(),
                }
                .iter()
                .flat_map(|s| s.split(',').map(|x| x.trim().trim_start_matches('@').to_string()))
                .filter(|x| !x.is_empty())
                .collect()
            };
            let roles = list("role");
            if let Some(r) = roles.iter().find(|r| crate::search::Role::parse(r).is_none()) {
                return Err(format!("unknown role: {} (user|assistant|message|tool|hub)", r));
            }
            req.insert("agents".into(), json!(list("agent")));
            req.insert("roles".into(), json!(roles));
            let now = crate::util::now_ms();
            for k in ["since", "until"] {
                if o.contains_key(k) {
                    req.insert(k.into(), json!(crate::search::parse_time(&str_of(&o, k), now)?));
                }
            }
            if o.contains_key("archived") && o.contains_key("live") {
                return Err("--archived and --live exclude each other".into());
            }
            let arch = if o.contains_key("archived") { "only" } else if o.contains_key("live") { "no" } else { "" };
            req.insert("archived".into(), json!(arch));
            for (k, d) in [("limit", crate::search::DEFAULT_HITS), ("page", 1)] {
                let n = if o.contains_key(k) {
                    str_of(&o, k).parse::<usize>().map_err(|_| format!("--{} expects a number", k))?
                } else {
                    d
                };
                req.insert(k.into(), json!(n));
            }
        }
        "show" => {
            let (pos, o) = parse_args(rest, &["context"], &[])?;
            let usage = "usage: sb show <agent>#<pos> [--context <n>]";
            let (agent, p) = crate::search::parse_ref(&pos).ok_or(usage)?;
            req.insert("agent".into(), json!(agent));
            req.insert("pos".into(), json!(p));
            let n = if o.contains_key("context") {
                str_of(&o, "context").parse::<usize>().map_err(|_| "--context expects a number".to_string())?
            } else {
                crate::search::DEFAULT_CONTEXT
            };
            req.insert("context".into(), json!(n));
        }
        "send" => {
            let (pos, o) = parse_args(rest, &["reply-to", "mode", "why", "model", "effort"], &["expect-reply"])?;
            let to = agent_arg(&pos, "usage: sb send <agent> \"<text>\"")?;
            req.insert("to".into(), json!(to));
            // issue #4: `--model`/`--effort` move the task from its next
            // turn; the text is optional then (a switch alone)
            let (model, effort) = (str_of(&o, "model"), str_of(&o, "effort"));
            if !model.is_empty() || !effort.is_empty() {
                req.insert("model".into(), json!(model));
                req.insert("effort".into(), json!(effort));
                if pos.len() < 2 {
                    req.insert("cmd".into(), json!("switch"));
                    return Ok(Value::Object(req));
                }
            }
            req.insert("text".into(), json!(text_of(&pos[1..])?));
            req.insert("expect_reply".into(), json!(o.contains_key("expect-reply")));
            if o.contains_key("mode") {
                match str_of(&o, "mode").as_str() {
                    m @ ("steer" | "queued") => {
                        req.insert("mode".into(), json!(m));
                    }
                    m => return Err(format!("unknown mode: {} (steer|queued)", m)),
                }
            }
            if o.contains_key("reply-to") {
                req.insert("reply_to".into(), json!(str_of(&o, "reply-to")));
            }
            // main answering a task for the user: the reason, shown to
            // the user in main's feed (C2 `answered`)
            if o.contains_key("why") {
                req.insert("why".into(), json!(str_of(&o, "why")));
            }
        }
        "ask" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            let to = agent_arg(&pos, "usage: sb ask <agent> \"<question>\"")?;
            req.insert("to".into(), json!(to));
            req.insert("text".into(), json!(text_of(&pos[1..])?));
            req.insert("timeout_s".into(), json!(timeout_of(&o)));
        }
        "wait" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            req.insert(
                "msg".into(),
                json!(pos.first().ok_or("usage : sb wait m_<n>")?),
            );
            req.insert("timeout_s".into(), json!(timeout_of(&o)));
        }
        "status" => {
            let (pos, o) = parse_args(rest, &["note"], &[])?;
            req.insert(
                "status".into(),
                json!(pos
                    .first()
                    .ok_or("usage: sb status working|done|blocked")?),
            );
            req.insert("note".into(), json!(str_of(&o, "note")));
        }
        "flow" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            match pos.as_slice() {
                [] => {}
                [m] => {
                    crate::devflow::parse_mode(m).map_err(|_| "usage: sb flow [pr|trunk]".to_string())?;
                    req.insert("set".into(), json!(m));
                }
                _ => return Err("usage: sb flow [pr|trunk]".into()),
            }
        }
        "worktree" => {
            // BISE-136: gate.sh new/done tell the hub where the agent works
            let (pos, _) = parse_args(rest, &[], &[])?;
            let p = pos.first().ok_or("usage: sb worktree <path>|none")?;
            worktree_exists(p)?;
            req.insert("path".into(), json!(p));
        }
        "report" => {
            let (pos, o) = parse_args(rest, &["decision"], &[])?;
            req.insert(
                "kind".into(),
                json!(pos.first().ok_or("usage: sb report <kind> \"<summary>\"")?),
            );
            req.insert("summary".into(), json!(text_of(&pos[1..])?));
            req.insert("decisions".into(), json!(list_of(o.get("decision"))));
        }
        "spawn" => {
            let (pos, o) = parse_args(
                rest,
                &[
                    "objective",
                    "context",
                    "constraint",
                    "done-when",
                    "report-format",
                    "place",
                    "feature",
                    "model",
                    "effort",
                    "profile",
                ],
                &["worktree", "with-changes"],
            )?;
            // issue #4: the task's model, all optional (none: the agents default)
            for k in ["model", "effort", "profile"] {
                req.insert(k.into(), json!(str_of(&o, k)));
            }
            // dev-flow §5.1: its own worktree from the feature's tip
            let feature = str_of(&o, "feature");
            if !feature.is_empty() && (o.contains_key("place") || o.contains_key("worktree")) {
                return Err("sb spawn: --feature gives the task its own worktree; no --place or --worktree with it".into());
            }
            req.insert("feature".into(), json!(feature));
            req.insert(
                "name".into(),
                json!(pos.first().cloned().unwrap_or_default()),
            );
            let mut objective = str_of(&o, "objective");
            if objective.is_empty() && pos.len() > 1 {
                objective = pos[1..].join(" ");
            }
            if objective.is_empty() {
                return Err("sb spawn: --objective is required".into());
            }
            req.insert("objective".into(), json!(objective));
            req.insert("context".into(), json!(str_of(&o, "context")));
            req.insert("constraints".into(), json!(list_of(o.get("constraint"))));
            req.insert("done_when".into(), json!(str_of(&o, "done-when")));
            req.insert("report_format".into(), json!(str_of(&o, "report-format")));
            // dev-flow §3.1: --place new (= --worktree) | <agent> | <branch>
            let place = str_of(&o, "place");
            req.insert("worktree".into(), json!(o.contains_key("worktree") || place == "new"));
            if !place.is_empty() && place != "new" {
                req.insert("place".into(), json!(place));
            }
            req.insert("with_changes".into(), json!(o.contains_key("with-changes")));
        }
        "move" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let usage = "usage: sb move <agent> new|shared|<agent>|<branch>";
            req.insert("agent".into(), json!(agent_arg(&pos, usage)?));
            req.insert("place".into(), json!(pos.get(1).ok_or(usage)?));
        }
        "feature" => {
            // dev-flow §5.1: sb feature [new|sync|ready|merge|drop|list] <name>
            let (pos, _) = parse_args(rest, &[], &[])?;
            let usage = "usage: sb feature [new|sync|ready|merge|drop|list] <name>";
            // `step`, not `op`: the wire's own key (`"op": "agent"`)
            match pos.as_slice() {
                [] => {
                    req.insert("step".into(), json!("list"));
                }
                [op] if op == "list" => {
                    req.insert("step".into(), json!("list"));
                }
                [op, name] if ["new", "sync", "ready", "merge", "drop"].contains(&op.as_str()) => {
                    req.insert("step".into(), json!(op));
                    req.insert("name".into(), json!(name));
                }
                _ => return Err(usage.into()),
            }
        }
        "land" => {
            let (pos, o) = parse_args(rest, &["add"], &["here"])?;
            req.insert("here".into(), json!(o.contains_key("here")));
            req.insert("message".into(), json!(pos.join(" ")));
            // --add paths, absolute: the hub resolves them in the place
            let cwd = std::env::current_dir().unwrap_or_default();
            let add: Vec<String> = match o.get("add") {
                Some(Value::Array(xs)) => xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                Some(Value::String(x)) => vec![x.clone()],
                _ => Vec::new(),
            };
            let add: Vec<String> = add.iter().map(|p| cwd.join(p).to_string_lossy().to_string()).collect();
            if !add.is_empty() {
                req.insert("add".into(), json!(add));
            }
        }
        "interrupt" | "drop" | "restore" | "isolate" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            req.insert("agent".into(), json!(agent_arg(&pos, format!("usage: sb {} <agent>", cmd))?));
        }
        "stop" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            req.insert("agent".into(), json!(agent_arg(&pos, "usage: sb stop <agent> \"<reason>\"")?));
            req.insert("reason".into(), json!(pos[1..].join(" ")));
        }
        "close" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let card = pos
                .first()
                .and_then(|c| c.trim_start_matches('#').parse::<u64>().ok())
                .ok_or("usage: sb close <card> [\"<note>\"]")?;
            req.insert("card".into(), json!(card));
            req.insert("note".into(), json!(pos[1..].join(" ")));
        }
        "rename" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            match pos.as_slice() {
                [a, b] => {
                    req.insert("agent".into(), json!(a.trim_start_matches('@')));
                    req.insert("new_name".into(), json!(b.trim_start_matches('@')));
                }
                _ => return Err("usage: sb rename <agent> <new-name>".into()),
            }
        }
        "card" if rest.iter().any(|a| a == "--withdraw" || a.starts_with("--withdraw=")) => {
            // BISE-299: `sb card --withdraw N "why"`, the hub's own command
            let (pos, o) = parse_args(rest, &["withdraw"], &[])?;
            let card = o
                .get("withdraw")
                .and_then(|c| c.as_str())
                .and_then(|c| c.trim_start_matches('#').parse::<u64>().ok())
                .ok_or("usage: sb card --withdraw <card> \"<why>\"")?;
            req.insert("cmd".into(), json!("withdraw"));
            req.insert("card".into(), json!(card));
            req.insert("why".into(), json!(pos.join(" ")));
        }
        "card" => {
            let (pos, o) = parse_args(rest, &["for"], &[])?;
            req.insert("text".into(), json!(text_of(&pos)?));
            if o.contains_key("for") {
                req.insert("for".into(), json!(str_of(&o, "for")));
            }
        }
        "inspect" => {
            let (pos, o) = parse_args(
                rest,
                &["last", "limit", "query", "before", "after", "around", "at"],
                &["origin"],
            )?;
            req.insert("agent".into(), json!(agent_arg(&pos, "usage: sb inspect <agent>")?));
            let n = if o.contains_key("limit") {
                str_of(&o, "limit")
            } else {
                str_of(&o, "last")
            };
            req.insert("last".into(), json!(n.parse::<u64>().unwrap_or(20)));
            req.insert("query".into(), json!(str_of(&o, "query")));
            for k in ["before", "after", "around", "at"] {
                if o.contains_key(k) {
                    let p = str_of(&o, k);
                    if crate::transcript::parse_pos(&p).is_none() {
                        return Err(format!("--{} expects a position (#<n>)", k));
                    }
                    req.insert(k.into(), json!(p));
                }
            }
            req.insert("origin".into(), json!(o.contains_key("origin")));
        }
        "artifact" => {
            let usage = "usage: sb artifact add <path or link> [--title \"<t>\"] [--kind <k>] | sb artifact list [<words>] [--agent <a>]";
            let (pos, o) = parse_args(rest, &["title", "kind", "agent"], &[])?;
            match pos.first().map(String::as_str) {
                Some("add") => {
                    let target = match &pos[1..] {
                        [t] => t.clone(),
                        _ => return Err(usage.into()),
                    };
                    req.insert("do".into(), json!("add"));
                    req.insert("target".into(), json!(target));
                    let cwd = std::env::current_dir().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                    req.insert("cwd".into(), json!(cwd));
                    for k in ["title", "kind"] {
                        if o.contains_key(k) {
                            req.insert(k.into(), json!(str_of(&o, k)));
                        }
                    }
                }
                Some("list") => {
                    req.insert("do".into(), json!("list"));
                    req.insert("words".into(), json!(pos[1..].join(" ")));
                    if o.contains_key("agent") {
                        req.insert("agent".into(), json!(str_of(&o, "agent").trim_start_matches('@')));
                    }
                }
                _ => return Err(usage.into()),
            }
        }
        "help" | "--help" | "-h" => return Err(usage()),
        other => return Err(format!("unknown command: {}\n{}", other, usage())),
    }
    Ok(Value::Object(req))
}

/// What the agent reads back.
pub fn render(cmd: &str, v: &Value) -> (bool, String) {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    if v.get("ok") != Some(&json!(true)) {
        let mut e = format!("error: {}", s("error"));
        if !s("hint").is_empty() {
            e.push_str(&format!("\n{}", s("hint")));
        }
        return (false, e);
    }
    let text = match cmd {
        "list" | "tasks" | "inspect" | "history" | "show" | "artifact" => s("text"),
        // issue #4: `sb send <task> --model <id>` alone: the hub's line
        "send" if s("cmd") == "switch" => s("text"),
        "send" => format!("sent {} to {} ({}, thread {})", s("message_id"), s("to"), s("delivery"), s("thread")),
        "ask" | "wait" => match s("type").as_str() {
            // `answers m_8`: the question's id (BISE-110: the TUI hides
            // an `sb ask` box whose question and reply both show)
            "reply" => format!(
                "reply from {} ({}{}{}):\n{}",
                s("from"),
                s("message_id"),
                if s("asked").is_empty() { String::new() } else { format!(", answers {}", s("asked")) },
                if v.get("auto") == Some(&json!(true)) { ", automatic: the end of its turn" } else { "" },
                s("message")
            ),
            _ => format!(
                "incoming message from {} ({}, thread {}{}) — your wait ended so you can answer it:\n{}",
                s("from"),
                s("message_id"),
                s("thread"),
                if v.get("expects_reply") == Some(&json!(true)) { ", expects a reply" } else { "" },
                s("message")
            ),
        },
        "spawn" => format!(
            "agent {} created{}{} — it starts now; its answer will come back as a message",
            s("name"),
            // issue #4: `on gpt-9 · low (profile fast)`, or the fallback's words
            if s("model").is_empty() { String::new() } else { format!(" {}", s("model")) },
            v.get("branch").and_then(|b| b.as_str()).map(|b| format!(" in worktree {} (branch {})", s("path"), b)).unwrap_or_default()
        ),
        "switch" => s("text"),
        "drop" => {
            if v.get("dropped") == Some(&json!(true)) {
                "dropped".to_string()
            } else {
                format!("not dropped: {} (card #{})", s("reason"), v.get("card").and_then(|c| c.as_u64()).unwrap_or(0))
            }
        }
        "card" => format!("card #{} opened for the user", v.get("card").and_then(|c| c.as_u64()).unwrap_or(0)),
        "report" => format!("reported ({})", s("message_id")),
        "close" => format!("card #{} closed", v.get("card").and_then(|c| c.as_u64()).unwrap_or(0)),
        "withdraw" => format!("card #{} withdrawn: the user sees why; the question is yours again", v.get("card").and_then(|c| c.as_u64()).unwrap_or(0)),
        "rename" => format!("renamed: now @{} (the old name still works)", s("name")),
        "restore" => format!("@{} restored", s("name")),
        "isolate" => format!("@{} now works in its own git worktree", s("name")),
        "move" => format!("@{} moved", s("name")),
        // the hub's line: `✓ x landed 1 commit on main (abc1234)`
        "land" => s("text"),
        "flow" => s("text"),
        "feature" => s("text"),
        "worktree" if s("path").is_empty() => "the hub knows you work in your own workspace again".to_string(),
        "worktree" => format!("the hub knows you work in {}", s("path")),
        _ => "ok".to_string(),
    };
    (true, text)
}

/// `sb …` entry point: the process exit code.
/// `sb version [list | switch <commit|id|tree> | rollback]`: the
/// versions of Switchboard itself (a hub op, not an agent request).
fn version(args: &[String]) -> i32 {
    let socket = std::env::var("SB_SOCKET").unwrap_or_default();
    if socket.is_empty() {
        eprintln!("sb : SB_SOCKET manque");
        return 2;
    }
    let what = args.get(1).map(|s| s.as_str()).unwrap_or("list");
    let from = std::env::var("SB_AGENT").unwrap_or_default();
    let req = json!({"op": "version", "do": what, "to": args.get(2).cloned().unwrap_or_default(), "from": from});
    match crate::client::request_retry(
        std::path::Path::new(&socket),
        &req,
        Duration::from_secs(30),
        what == "list",
    ) {
        Ok(v) if v.get("ok") == Some(&json!(false)) => {
            eprintln!(
                "error: {}",
                v.get("error").and_then(|t| t.as_str()).unwrap_or("")
            );
            1
        }
        Ok(v) => {
            println!("{}", v.get("text").and_then(|t| t.as_str()).unwrap_or(""));
            0
        }
        Err(e) => {
            eprintln!("sb : {}", e);
            1
        }
    }
}

pub fn main(args: &[String]) -> i32 {
    if args.first().map(|s| s.as_str()) == Some("version") {
        return version(args);
    }
    if args.first().map(|s| s.as_str()) == Some("restart") {
        // `sb restart [current|<commit>]` (main only): the hub op
        let mut a = vec!["version".to_string(), "restart".to_string()];
        a.extend(args.get(1).cloned());
        return version(&a);
    }
    if let Some(c) = args.first().filter(|_| args.iter().any(|a| a == "--help" || a == "-h")) {
        println!("{}", help_for(c));
        return 0;
    }
    if matches!(args.first().map(String::as_str), Some("help")) {
        println!("{}", usage());
        return 0;
    }
    let req = match build(args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let socket = std::env::var("SB_SOCKET").unwrap_or_default();
    let from = std::env::var("SB_AGENT").unwrap_or_default();
    if socket.is_empty() || from.is_empty() {
        eprintln!(
            "sb: SB_SOCKET and SB_AGENT are missing (sb only runs inside a bise agent)"
        );
        return 2;
    }
    let mut req = req;
    req["op"] = json!("agent");
    req["from"] = json!(from);
    let cmd = args[0].clone();
    let idempotent = matches!(
        cmd.as_str(),
        "list" | "tasks" | "history" | "show" | "inspect" | "wait"
    );
    // hub-lag: a request that changes something (send, report, spawn...)
    // is in the hub's queue once written: wait longer for its answer
    // rather than give up on a busy hub and have it retried (a second
    // message, a second task)
    let slack = if idempotent { 30 } else { 180 };
    let timeout = req.get("timeout_s").and_then(|t| t.as_u64()).unwrap_or(0) + slack;
    match crate::client::request_retry(
        std::path::Path::new(&socket),
        &req,
        Duration::from_secs(timeout),
        idempotent,
    ) {
        Ok(v) => {
            let (ok, text) = render(&cmd, &v);
            if ok {
                println!("{}", text);
                0
            } else {
                eprintln!("{}", text);
                1
            }
        }
        Err(e) => {
            eprintln!("sb : {}", e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    /// Every command of the table (usage, prompts) is one `build` knows.
    #[test]
    fn every_documented_command_is_known() {
        for c in COMMANDS {
            for alt in c.syntax.split(" | sb ") {
                let name = alt.trim_start_matches("sb ").split(' ').next().unwrap();
                if matches!(name, "version" | "restart") {
                    continue; // main() runs them, not a hub request
                }
                let r = build(&a(&[name]));
                assert!(
                    !matches!(&r, Err(e) if e.starts_with("unknown command")),
                    "{}: {:?}",
                    c.syntax,
                    r
                );
            }
        }
    }

    #[test]
    fn inspect_cursors() {
        let r = build(&a(&[
            "inspect",
            "@main",
            "--query",
            "mode sombre",
            "--before",
            "#120",
            "--limit",
            "5",
        ]))
        .unwrap();
        assert_eq!(r["agent"], "main");
        assert_eq!(r["query"], "mode sombre");
        assert_eq!(r["before"], "#120");
        assert_eq!(r["last"], 5);
        assert_eq!(r["origin"], false);
        let o = build(&a(&["inspect", "main", "--origin"])).unwrap();
        assert_eq!(o["origin"], true);
        assert!(build(&a(&["inspect", "main", "--around", "abc"])).is_err());
    }

    #[test]
    fn send_flags() {
        let r = build(&a(&[
            "send",
            "@docs",
            "v2",
            "please",
            "--expect-reply",
            "--reply-to",
            "m_4",
        ]))
        .unwrap();
        assert_eq!(r["to"], "docs");
        assert_eq!(r["text"], "v2 please");
        assert_eq!(r["expect_reply"], true);
        assert_eq!(r["reply_to"], "m_4");
        assert!(r.get("mode").is_none());
        assert!(r.get("why").is_none());
        let w = build(&a(&["send", "docs", "v2", "--reply-to", "m_4", "--why", "the brief says v2"])).unwrap();
        assert_eq!(w["why"], "the brief says v2");
        assert_eq!(w["text"], "v2");
        let q = build(&a(&["send", "docs", "later", "--mode", "queued"])).unwrap();
        assert_eq!(q["mode"], "queued");
        assert!(build(&a(&["send", "docs", "x", "--mode", "soon"])).is_err());
    }

    /// qa-explore L: `sb <cmd> --help` shows its usage; `sb` lists worktree.
    #[test]
    fn help_is_per_command_and_complete() {
        assert!(usage().contains("sb worktree <path>|none"));
        let h = help_for("spawn");
        assert!(h.starts_with("sb spawn <name> --objective") && !h.contains("sb send"), "{h}");
        assert!(help_for("restore").starts_with("sb restore <task>"));
        assert!(help_for("move").starts_with("sb move <agent> new|shared|<agent>|<branch>"));
        assert!(help_for("land").starts_with("sb land [--here]"));
        assert!(help_for("worktree").starts_with("sb worktree"));
        assert_eq!(help_for("nosuch"), usage());
        assert_eq!(main(&a(&["spawn", "--help"])), 0);
        assert_eq!(main(&a(&["spawn", "-h"])), 0);
    }

    /// qa-explore B: `sb worktree` refused a relative path only.
    #[test]
    fn worktree_must_exist() {
        let e = build(&a(&["worktree", "/does/not/exist"])).unwrap_err();
        assert!(e.contains("not a git worktree"), "{}", e);
        let d = std::env::temp_dir().join(format!("sb-cli-wt-{}", std::process::id()));
        std::fs::create_dir_all(d.join(".git")).unwrap();
        let p = d.to_string_lossy().into_owned();
        assert_eq!(build(&a(&["worktree", &p])).unwrap()["path"], p.as_str());
        assert_eq!(build(&a(&["worktree", "none"])).unwrap()["path"], "none");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn spawn_repeats_constraints() {
        let r = build(&a(&[
            "spawn",
            "fix",
            "--objective",
            "fix it",
            "--constraint",
            "no push",
            "--constraint",
            "tests",
            "--worktree",
        ]))
        .unwrap();
        assert_eq!(r["constraints"], json!(["no push", "tests"]));
        assert_eq!(r["worktree"], true);
        assert!(build(&a(&["spawn", "fix"])).is_err());
    }

    #[test]
    fn requests_parse_in_the_core() {
        for args in [
            vec!["list"],
            vec!["ask", "main", "why?"],
            vec!["wait", "m_3"],
            vec!["status", "blocked", "--note", "need key"],
            vec!["report", "done", "all good", "--decision", "v2"],
            vec!["card", "--for", "m_2", "v1 or v2?"],
            vec!["stop", "x", "no", "longer", "needed"],
            vec!["close", "#3", "handled"],
            vec!["close", "4"],
            vec!["rename", "@a", "b"],
            vec!["restore", "a"],
            vec!["isolate", "a"],
        ] {
            let r = build(&a(&args)).unwrap();
            crate::core::AgentReq::from_json(&r).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
        }
    }

    #[test]
    fn main_controls_parse() {
        let r = build(&a(&["close", "#3", "handled", "by", "docs"])).unwrap();
        assert_eq!(r["card"], 3);
        assert_eq!(r["note"], "handled by docs");
        assert_eq!(build(&a(&["close", "3"])).unwrap()["note"], "");
        assert!(build(&a(&["close", "x"])).is_err());
        assert!(build(&a(&["close"])).is_err());
        let r = build(&a(&["rename", "@old", "new"])).unwrap();
        assert_eq!((r["agent"].as_str(), r["new_name"].as_str()), (Some("old"), Some("new")));
        assert!(build(&a(&["rename", "old"])).is_err());
        assert_eq!(build(&a(&["restore", "@x"])).unwrap()["agent"], "x");
        assert_eq!(build(&a(&["isolate", "x"])).unwrap()["agent"], "x");
        assert!(build(&a(&["isolate"])).is_err());
    }

    #[test]
    fn rendering() {
        let (ok, t) = render(
            "ask",
            &json!({"ok": true, "type": "reply", "from": "main", "message_id": "m_3", "message": "v2", "auto": false}),
        );
        assert!(ok && t.starts_with("reply from main (m_3):\nv2"), "{}", t);
        let (_, t) = render(
            "ask",
            &json!({"ok": true, "type": "reply", "from": "docs", "message_id": "m_9", "asked": "m_8", "message": "v2", "auto": true}),
        );
        assert!(t.starts_with("reply from docs (m_9, answers m_8, automatic: the end of its turn):\nv2"), "{}", t);
        let (ok, t) = render(
            "wait",
            &json!({"ok": false, "error": "timeout", "hint": "end your turn"}),
        );
        assert!(!ok && t.contains("timeout") && t.contains("end your turn"));
    }
}
